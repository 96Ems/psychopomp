//! Trees: a JSON value shown as a foldable, syntax-tinted outline, for "the
//! program emits this plan", config files, payloads, and state shapes.
//!
//! Every row's identity is its JSONPath (`$.actors[0].id`). Folding is one
//! `node.<path>.open` channel per container, 0 (folded) to 1 (open), and the
//! layout is a pure function of those channels: an opening node's room grows
//! by `open × (children + closing row)`, so rows above it never move, rows
//! below slide by exactly the room it takes, and its children are revealed in
//! place inside it. Values change in place through `node.<path>.value`, a
//! variant index that rolls the text through its row's window.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    caption,
};

pub const TREE_RECIPE: &str = "tree";
pub const ROOT_PATH: &str = "$";

const DEFAULT_SIZE: f32 = 24.0;
const DEFAULT_INDENT: u32 = 2;
/// Row pitch, in font sizes.
const ROW_EM: f32 = 1.6;
const MAX_NODES: usize = 4000;
/// A fold below this is treated as fully closed.
const CLOSED: f32 = 1e-4;

/// Fold and value springs: the 0.45 s zero-bounce line motion of Effect
/// Institute code, so a tree moves like the code beside it.
const OPEN_SECONDS: f32 = 0.45;
const VALUE_SECONDS: f32 = 0.5;
const SCROLL_SECONDS: f32 = 0.55;
const HIGHLIGHT_IN_SECONDS: f32 = 0.18;
const HIGHLIGHT_OUT_SECONDS: f32 = 0.35;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TreePlan {
    /// Top-left corner of the first row.
    pub origin: [f32; 2],
    /// Row width: the highlight bar's extent and where long values truncate.
    pub width: f32,
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub size: f32,
    /// Rows visible at once; more scroll through a stationary window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rows: Option<u32>,
    /// Columns per nesting level.
    #[serde(default = "default_indent", skip_serializing_if = "is_default_indent")]
    pub indent: u32,
    pub value: Value,
    /// Containers open from time zero. A path with an `open` channel starts
    /// at that channel's initial value instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expanded: Vec<String>,
    /// Later scalar values, in order per path: the `n`th change of a path is
    /// variant `n` of its `node.<path>.value` channel; variant 0 is `value`'s.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<TreeChangePlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TreeChangePlan {
    pub path: String,
    pub value: Value,
}

fn default_size() -> f32 {
    DEFAULT_SIZE
}

fn is_default_size(size: &f32) -> bool {
    *size == DEFAULT_SIZE
}

fn default_indent() -> u32 {
    DEFAULT_INDENT
}

fn is_default_indent(indent: &u32) -> bool {
    *indent == DEFAULT_INDENT
}

impl TreePlan {
    pub fn new(origin: [f32; 2], width: f32, value: Value) -> Self {
        Self {
            origin,
            width,
            size: DEFAULT_SIZE,
            max_rows: None,
            indent: DEFAULT_INDENT,
            value,
            expanded: Vec::new(),
            changes: Vec::new(),
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn max_rows(mut self, rows: u32) -> Self {
        self.max_rows = Some(rows);
        self
    }

    pub fn indent(mut self, columns: u32) -> Self {
        self.indent = columns;
        self
    }

    /// Start `paths` open.
    pub fn expanded<S: Into<String>>(mut self, paths: impl IntoIterator<Item = S>) -> Self {
        self.expanded.extend(paths.into_iter().map(Into::into));
        self
    }

    pub fn row_height(&self) -> f32 {
        self.size * ROW_EM
    }

    pub fn model(&self) -> Result<TreeModel> {
        TreeModel::new(&self.value, &self.changes)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "tree origin must be finite"
        );
        ensure!(
            self.width.is_finite() && (120.0..=4000.0).contains(&self.width),
            "tree width must be between 120 and 4000"
        );
        ensure!(
            (10.0..=72.0).contains(&self.size),
            "tree size must be between 10 and 72"
        );
        ensure!(
            self.max_rows.is_none_or(|rows| (1..=200).contains(&rows)),
            "tree maxRows must be between 1 and 200"
        );
        ensure!(
            (1..=8).contains(&self.indent),
            "tree indent must be between 1 and 8 columns"
        );
        let model = self.model()?;
        for path in &self.expanded {
            let node = model
                .find(path)
                .with_context(|| format!("expanded path '{path}' is not in the tree"))?;
            ensure!(
                model.nodes[node].foldable(),
                "expanded path '{path}' is not a non-empty object or array"
            );
        }
        Ok(())
    }
}

/// How a row names its value.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeLabel {
    Root,
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TreeNodeKind {
    Object {
        len: usize,
    },
    Array {
        len: usize,
    },
    /// A string, number, boolean, or null, then each later value.
    Scalar {
        values: Vec<Value>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    pub path: String,
    pub label: TreeLabel,
    pub depth: usize,
    pub kind: TreeNodeKind,
    pub children: Vec<usize>,
}

impl TreeNode {
    /// A non-empty object or array: it has a chevron and an `open` channel.
    pub fn foldable(&self) -> bool {
        matches!(
            self.kind,
            TreeNodeKind::Object { len } | TreeNodeKind::Array { len } if len > 0
        )
    }

    /// Opening and closing brackets of an object or array.
    pub fn brackets(&self) -> Option<(char, char)> {
        match self.kind {
            TreeNodeKind::Object { .. } => Some(('{', '}')),
            TreeNodeKind::Array { .. } => Some(('[', ']')),
            TreeNodeKind::Scalar { .. } => None,
        }
    }

    /// The folded summary after the opening bracket: `…} 4 keys`.
    pub fn summary(&self) -> Option<(String, String)> {
        let (close, count, noun) = match self.kind {
            TreeNodeKind::Object { len } if len > 0 => ('}', len, "key"),
            TreeNodeKind::Array { len } if len > 0 => (']', len, "item"),
            _ => return None,
        };
        let plural = if count == 1 { "" } else { "s" };
        Some((format!("…{close}"), format!("{count} {noun}{plural}")))
    }

    /// Number of value variants: 1 for containers and unchanging scalars.
    pub fn variants(&self) -> usize {
        match &self.kind {
            TreeNodeKind::Scalar { values } => values.len(),
            _ => 1,
        }
    }
}

/// The flattened tree in pre-order. Index 0 is the root.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeModel {
    pub nodes: Vec<TreeNode>,
}

/// One drawn row at some fold state. Positions are in rows from the top of
/// the unscrolled tree; `clip` is the window its ancestors have opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeLine {
    pub node: usize,
    /// The closing-bracket row of an open container.
    pub closing: bool,
    pub y: f32,
    pub clip: [f32; 2],
    /// Product of every ancestor's fold: 1 when fully revealed.
    pub reveal: f32,
    /// Rows the node's block takes, from its row through its closing bracket.
    pub extent: f32,
}

impl TreeLine {
    /// How much of the row its ancestors' rooms have opened, 0 to 1.
    pub fn room(&self) -> f32 {
        let top = self.y.max(self.clip[0]);
        let bottom = (self.y + 1.0).min(self.clip[1]);
        (bottom - top).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeLayout {
    pub lines: Vec<TreeLine>,
    /// Total height, in rows.
    pub height: f32,
}

impl TreeModel {
    pub fn new(value: &Value, changes: &[TreeChangePlan]) -> Result<Self> {
        let mut model = Self { nodes: Vec::new() };
        model.push(value, ROOT_PATH.to_owned(), TreeLabel::Root, 0)?;
        for change in changes {
            let node = model
                .find(&change.path)
                .with_context(|| format!("changed path '{}' is not in the tree", change.path))?;
            ensure!(
                is_scalar(&change.value),
                "tree change at '{}' must be a string, number, boolean, or null",
                change.path
            );
            let TreeNodeKind::Scalar { values } = &mut model.nodes[node].kind else {
                bail!(
                    "tree change at '{}' replaces an object or array; only scalars change in place",
                    change.path
                );
            };
            values.push(change.value.clone());
        }
        Ok(model)
    }

    fn push(
        &mut self,
        value: &Value,
        path: String,
        label: TreeLabel,
        depth: usize,
    ) -> Result<usize> {
        ensure!(
            self.nodes.len() < MAX_NODES,
            "trees are limited to {MAX_NODES} rows"
        );
        let index = self.nodes.len();
        let kind = match value {
            Value::Object(map) => TreeNodeKind::Object { len: map.len() },
            Value::Array(items) => TreeNodeKind::Array { len: items.len() },
            scalar => TreeNodeKind::Scalar {
                values: vec![scalar.clone()],
            },
        };
        self.nodes.push(TreeNode {
            path: path.clone(),
            label,
            depth,
            kind,
            children: Vec::new(),
        });
        let children = match value {
            Value::Object(map) => map
                .iter()
                .map(|(key, child)| {
                    self.push(
                        child,
                        key_path(&path, key),
                        TreeLabel::Key(key.clone()),
                        depth + 1,
                    )
                })
                .collect::<Result<Vec<_>>>()?,
            Value::Array(items) => items
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    self.push(child, index_path(&path, i), TreeLabel::Index(i), depth + 1)
                })
                .collect::<Result<Vec<_>>>()?,
            _ => Vec::new(),
        };
        self.nodes[index].children = children;
        Ok(index)
    }

    pub fn find(&self, path: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.path == path)
    }

    /// Every visible row given each node's fold (ignored for scalars).
    pub fn layout(&self, opens: &[f32]) -> TreeLayout {
        assert_eq!(opens.len(), self.nodes.len(), "one fold per node");
        let fold = |i: usize| {
            if self.nodes[i].foldable() {
                opens[i].clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        // Children follow their parent in pre-order, so heights resolve in reverse.
        let mut heights = vec![1.0_f32; self.nodes.len()];
        for i in (0..self.nodes.len()).rev() {
            let open = fold(i);
            if open > CLOSED {
                let children: f32 = self.nodes[i].children.iter().map(|&c| heights[c]).sum();
                heights[i] = 1.0 + open * (children + 1.0);
            }
        }
        let mut lines = Vec::new();
        self.place(
            0,
            0.0,
            [f32::NEG_INFINITY, f32::INFINITY],
            1.0,
            &fold,
            &heights,
            &mut lines,
        );
        TreeLayout {
            lines,
            height: heights[0],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        node: usize,
        y: f32,
        clip: [f32; 2],
        reveal: f32,
        fold: &impl Fn(usize) -> f32,
        heights: &[f32],
        lines: &mut Vec<TreeLine>,
    ) {
        let extent = heights[node];
        lines.push(TreeLine {
            node,
            closing: false,
            y,
            clip,
            reveal,
            extent,
        });
        let open = fold(node);
        if open <= CLOSED {
            return;
        }
        // The closing bracket slides out from under the node's row, leading
        // the room open; children sit at full pitch in the room above it.
        let closing_y = y + extent - 1.0;
        let room = [clip[0].max(y + 1.0), clip[1].min(y + extent)];
        let inner = [room[0], room[1].min(closing_y)];
        let mut child_y = y + 1.0;
        for &child in &self.nodes[node].children {
            if child_y >= inner[1] {
                break;
            }
            self.place(child, child_y, inner, reveal * open, fold, heights, lines);
            child_y += heights[child];
        }
        if room[0] < room[1] {
            lines.push(TreeLine {
                node,
                closing: true,
                y: closing_y,
                clip: room,
                reveal: reveal * open,
                extent: 1.0,
            });
        }
    }

    /// The settled rows when exactly the `open` nodes are open.
    pub fn settled(&self, open: &[bool]) -> Vec<TreeLine> {
        let opens = open
            .iter()
            .map(|&o| f32::from(u8::from(o)))
            .collect::<Vec<_>>();
        self.layout(&opens).lines
    }
}

fn is_scalar(value: &Value) -> bool {
    !matches!(value, Value::Object(_) | Value::Array(_))
}

/// `$.parent.key`, or `$.parent["odd key"]` when the key is not an
/// identifier. Whitespace is escaped so the path can name a channel.
pub fn key_path(parent: &str, key: &str) -> String {
    let mut chars = key.chars();
    let identifier = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if identifier {
        return format!("{parent}.{key}");
    }
    let quoted = serde_json::to_string(key).expect("strings serialize");
    let escaped = quoted
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                format!("\\u{:04x}", u32::from(c))
            } else {
                c.to_string()
            }
        })
        .collect::<String>();
    format!("{parent}[{escaped}]")
}

pub fn index_path(parent: &str, index: usize) -> String {
    format!("{parent}[{index}]")
}

/// The channel property for one node: `node.<path>.open`, `.highlight`, or `.value`.
pub fn node_property(path: &str, channel: TreeChannel) -> String {
    format!("node.{path}.{}", channel.name())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeChannel {
    Open,
    Highlight,
    Value,
}

impl TreeChannel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Highlight => "highlight",
            Self::Value => "value",
        }
    }

    /// Split `node.<path>.<channel>` into its path and channel.
    pub fn parse(property: &str) -> Option<(&str, Self)> {
        let (path, name) = property.strip_prefix("node.")?.rsplit_once('.')?;
        let channel = match name {
            "open" => Self::Open,
            "highlight" => Self::Highlight,
            "value" => Self::Value,
            _ => return None,
        };
        Some((path, channel))
    }
}

/// The text a scalar shows: JSON, so strings keep their quotes and escapes.
pub fn scalar_text(value: &Value) -> String {
    value.to_string()
}

/// Authoring handle for one tree actor. Calls are applied in authored order;
/// the [`PlanBuilder`] owns the tree's changes, fold channels, and scroll state.
#[derive(Clone, Debug)]
pub struct TreeActor {
    actor: ActorHandle,
    model: TreeModel,
}

impl TreeActor {
    pub fn declare(scene: &mut PlanBuilder, id: impl Into<String>, plan: TreePlan) -> Result<Self> {
        plan.validate()?;
        let model = plan.model()?;
        let actor = scene.actor(id, TREE_RECIPE, &plan)?;
        Ok(Self { actor, model })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    pub fn model(&self) -> &TreeModel {
        &self.model
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    fn foldable(&self, path: &str) -> Result<usize> {
        let node = self
            .model
            .find(path)
            .with_context(|| format!("path '{path}' is not in the tree"))?;
        ensure!(
            self.model.nodes[node].foldable(),
            "path '{path}' is not a non-empty object or array"
        );
        Ok(node)
    }

    pub(crate) fn fold_at(
        &self,
        scene: &mut PlanBuilder,
        path: &str,
        at_nanos: u64,
        open: bool,
    ) -> Result<()> {
        let _ = self.foldable(path)?;
        let plan: TreePlan = scene.actor_data(&self.actor)?;
        let initial = f32::from(u8::from(plan.expanded.iter().any(|p| p == path)));
        let channel = scene.channel(
            &self.actor,
            &node_property(path, TreeChannel::Open),
            initial,
        );
        scene.spring(
            &channel,
            at_nanos,
            f32::from(u8::from(open)),
            OPEN_SECONDS,
            0.0,
        );
        Ok(())
    }

    /// Open a container: rows below slide down as its children are revealed.
    pub fn open(&mut self, scene: &mut PlanBuilder, path: &str, at_nanos: u64) -> Result<()> {
        self.fold_at(scene, path, at_nanos, true)
    }

    /// Fold a container back to its summary. Open descendants stay open.
    pub fn close(&mut self, scene: &mut PlanBuilder, path: &str, at_nanos: u64) -> Result<()> {
        self.fold_at(scene, path, at_nanos, false)
    }

    /// Light a row's highlight bar at `at_nanos` and let it go `seconds` later.
    pub fn highlight(
        &mut self,
        scene: &mut PlanBuilder,
        path: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> Result<()> {
        self.highlight_at(scene, path, at_nanos, seconds)
    }

    pub(crate) fn highlight_at(
        &self,
        scene: &mut PlanBuilder,
        path: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> Result<()> {
        self.model
            .find(path)
            .with_context(|| format!("path '{path}' is not in the tree"))?;
        let channel = scene.channel(
            &self.actor,
            &node_property(path, TreeChannel::Highlight),
            0.0,
        );
        scene.spring(&channel, at_nanos, 1.0, HIGHLIGHT_IN_SECONDS, 0.0);
        let off = at_nanos + crate::author::seconds(f64::from(seconds.max(0.0)));
        scene.spring(&channel, off, 0.0, HIGHLIGHT_OUT_SECONDS, 0.0);
        Ok(())
    }

    /// Change a scalar in place at `at_nanos`: the old value rolls up out of
    /// its row as the new one rolls in.
    pub fn set(
        &mut self,
        scene: &mut PlanBuilder,
        path: &str,
        value: impl Into<Value>,
        at_nanos: u64,
    ) -> Result<()> {
        self.set_at(scene, path, value, at_nanos)
    }

    pub(crate) fn set_at(
        &self,
        scene: &mut PlanBuilder,
        path: &str,
        value: impl Into<Value>,
        at_nanos: u64,
    ) -> Result<()> {
        let mut next: TreePlan = scene.actor_data(&self.actor)?;
        next.changes.push(TreeChangePlan {
            path: path.to_owned(),
            value: value.into(),
        });
        let model = next.model()?;
        let node = model.find(path).expect("validated by the model");
        let variant = model.nodes[node].variants() - 1;
        scene.replace_actor_data(&self.actor, &next)?;
        let channel = scene.channel(&self.actor, &node_property(path, TreeChannel::Value), 0.0);
        scene.spring(&channel, at_nanos, variant as f32, VALUE_SECONDS, 0.0);
        Ok(())
    }

    /// Scroll so `row` (fractional, from the top of the tree) is the first visible.
    pub fn scroll_to(&mut self, scene: &mut PlanBuilder, row: f32, at_nanos: u64) {
        self.scroll_to_at(scene, row, at_nanos);
    }

    pub(crate) fn scroll_to_at(&self, scene: &mut PlanBuilder, row: f32, at_nanos: u64) {
        let channel = scene.channel(&self.actor, "scroll", 0.0);
        scene.spring(&channel, at_nanos, row, SCROLL_SECONDS, 0.0);
    }

    /// Scroll the least distance that shows `path`'s whole block (its row
    /// through its closing bracket) once the authored folds settle, or its
    /// first rows if the block is taller than the window. Without `maxRows`
    /// nothing scrolls.
    pub fn reveal(&mut self, scene: &mut PlanBuilder, path: &str, at_nanos: u64) -> Result<()> {
        self.reveal_at(scene, path, at_nanos)
    }

    pub(crate) fn reveal_at(
        &self,
        scene: &mut PlanBuilder,
        path: &str,
        at_nanos: u64,
    ) -> Result<()> {
        let plan: TreePlan = scene.actor_data(&self.actor)?;
        let Some(rows) = plan.max_rows.map(|rows| rows as f32) else {
            return Ok(());
        };
        let node = self
            .model
            .find(path)
            .with_context(|| format!("path '{path}' is not in the tree"))?;
        let open: Vec<bool> = self
            .model
            .nodes
            .iter()
            .map(|n| {
                scene
                    .latest_literal(&self.actor, &node_property(&n.path, TreeChannel::Open))
                    .map_or_else(|| plan.expanded.contains(&n.path), |v| v > 0.5)
            })
            .collect();
        let current_scroll = scene.latest_literal(&self.actor, "scroll").unwrap_or(0.0);
        let lines = self.model.settled(&open);
        let first = lines
            .iter()
            .position(|line| line.node == node && !line.closing)
            .with_context(|| format!("path '{path}' is inside a folded node"))?
            as f32;
        let last = lines
            .iter()
            .position(|line| line.node == node && line.closing)
            .map_or(first, |index| index as f32);
        let mut scroll = current_scroll;
        if last + 1.0 - first > rows || first < scroll {
            scroll = first;
        } else if last + 1.0 > scroll + rows {
            scroll = last + 1.0 - rows;
        }
        scroll = scroll.clamp(0.0, (lines.len() as f32 - rows).max(0.0));
        if scroll != current_scroll {
            self.scroll_to_at(scene, scroll, at_nanos);
        }
        Ok(())
    }

    /// Fade and rise in, like a caption. A tree with a `show` starts hidden.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        caption::show(scene, &self.actor, at_nanos);
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        caption::hide(scene, &self.actor, at_nanos);
    }
}

#[cfg(test)]
mod tests;
