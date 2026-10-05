//! Checklists: rows of items that wait, run, and resolve, like CI checks or
//! the steps of a migration. A running item shows the closed-form status
//! spinner (`effects::spinner`); resolving hands its tip off into a drawn ✓
//! or ✕, or a skipped item strikes through and dims. An optional rail joins
//! the icons and fills as items resolve.
//!
//! Every state is a channel per item: `reveal` (0..1), the `spinner` and
//! `mark` clocks (seconds; -1 inactive), and `outcome` (0 pending, 1 done,
//! 2 failed, 3 skipped). Poses derive from them, so any frame samples alone.
use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder, whole_millis},
    effects::spinner::{self, Mark},
    math::{easing::Ease, smoothstep},
    plot::valid_id,
};

pub const CHECKLIST_RECIPE: &str = "checklist";

/// Rows enter this far apart.
pub const ROW_STAGGER_NANOS: u64 = 120_000_000;
/// From a skip to its strike and dash being fully drawn.
pub const SKIP_SECONDS: f32 = 0.4;
/// The rail below a resolved item fills over this long.
pub const RAIL_SECONDS: f32 = 0.5;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChecklistPlan {
    /// Top-left corner of the list (of its title row, when it has one).
    pub origin: [f32; 2],
    /// From the icon column's left edge to where results right-align.
    pub width: f32,
    /// Label size in pixels.
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub size: f32,
    /// Row pitch; defaults to 1.9 × `size`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_height: Option<f32>,
    pub items: Vec<ChecklistItemPlan>,
    /// A heading above the rows with a done count, such as `checks  3/5`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// A line joining the icons that fills as items resolve.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rail: bool,
    /// A rounded surface behind the list, for legibility over a busy root.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub panel: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChecklistItemPlan {
    pub id: String,
    pub label: String,
    /// Right-aligned text revealed with the item's resolution, such as `4.2s`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result: String,
    /// Shown instead of `result` when the item fails, such as `2 failed`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub failure: String,
}

/// How an item resolves.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Done,
    Failed,
    Skipped,
}

impl Outcome {
    /// The `outcome` channel's value.
    pub fn value(self) -> f32 {
        match self {
            Self::Done => 1.0,
            Self::Failed => 2.0,
            Self::Skipped => 3.0,
        }
    }

    /// The outcome an `outcome` channel value names; 0 (pending) is `None`.
    /// Values round, so an interpolated native transition still reads.
    pub fn from_value(value: f32) -> Option<Self> {
        match value.round() as i32 {
            1 => Some(Self::Done),
            2 => Some(Self::Failed),
            3 => Some(Self::Skipped),
            _ => None,
        }
    }

    /// The spinner mark this outcome draws; a skip draws none.
    pub fn mark(self) -> Option<Mark> {
        match self {
            Self::Done => Some(Mark::Check),
            Self::Failed => Some(Mark::Cross),
            Self::Skipped => None,
        }
    }
}

impl From<Mark> for Outcome {
    fn from(mark: Mark) -> Self {
        match mark {
            Mark::Check => Self::Done,
            Mark::Cross => Self::Failed,
        }
    }
}

fn default_size() -> f32 {
    28.0
}

fn is_default_size(size: &f32) -> bool {
    *size == default_size()
}

impl ChecklistItemPlan {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            result: String::new(),
            failure: String::new(),
        }
    }

    pub fn result(mut self, result: impl Into<String>) -> Self {
        self.result = result.into();
        self
    }

    pub fn failure(mut self, failure: impl Into<String>) -> Self {
        self.failure = failure.into();
        self
    }

    /// The text revealed with `outcome`.
    pub fn result_for(&self, outcome: Option<Outcome>) -> &str {
        match outcome {
            Some(Outcome::Failed) if !self.failure.is_empty() => &self.failure,
            _ => &self.result,
        }
    }
}

impl ChecklistPlan {
    pub fn new(origin: [f32; 2], width: f32) -> Self {
        Self {
            origin,
            width,
            size: default_size(),
            row_height: None,
            items: Vec::new(),
            title: String::new(),
            rail: false,
            panel: false,
        }
    }

    pub fn item(mut self, item: ChecklistItemPlan) -> Self {
        self.items.push(item);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn rail(mut self) -> Self {
        self.rail = true;
        self
    }

    pub fn panel(mut self) -> Self {
        self.panel = true;
        self
    }

    pub fn pitch(&self) -> f32 {
        self.row_height.unwrap_or(self.size * 1.9)
    }

    /// The icon's box edge; the spinner's 16-unit pose scales to it.
    pub fn icon_size(&self) -> f32 {
        (self.size * 0.95).round()
    }

    /// Height of the title row, or zero without a title.
    pub fn title_height(&self) -> f32 {
        if self.title.is_empty() {
            0.0
        } else {
            (self.size * 1.7).round()
        }
    }

    /// Vertical center of row `index`, before the actor's offsets.
    pub fn row_center(&self, index: usize) -> f32 {
        self.origin[1] + self.title_height() + (index as f32 + 0.5) * self.pitch()
    }

    /// Center of the icon column.
    pub fn icon_x(&self) -> f32 {
        self.origin[0] + self.icon_size() * 0.5
    }

    /// Left edge of every label.
    pub fn label_x(&self) -> f32 {
        self.origin[0] + self.icon_size() + (self.size * 0.6).round()
    }

    /// The list's full extent: `[left, top, right, bottom]`.
    pub fn extent(&self) -> [f32; 4] {
        [
            self.origin[0],
            self.origin[1],
            self.origin[0] + self.width,
            self.origin[1] + self.title_height() + self.pitch() * self.items.len() as f32,
        ]
    }

    pub fn find_item(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|item| item.id == id)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()) && self.width.is_finite(),
            "checklist origin and width must be finite"
        );
        ensure!(
            (14.0..=72.0).contains(&self.size),
            "checklist size must be between 14 and 72"
        );
        ensure!(
            self.width >= self.size * 6.0,
            "checklist width must be at least six times its size"
        );
        ensure!(
            self.row_height
                .is_none_or(|height| height.is_finite() && height >= self.size * 1.2),
            "checklist row height must be at least 1.2 × size"
        );
        ensure!(
            (1..=16).contains(&self.items.len()),
            "a checklist has 1 to 16 items"
        );
        ensure!(
            self.title.chars().count() <= 40 && !self.title.contains('\n'),
            "checklist title is one line of at most 40 characters"
        );
        let mut ids = HashSet::new();
        let label_offset = self.icon_size() + self.size * 0.65;
        let result_size = (self.size * 0.85).round();
        for item in &self.items {
            valid_id(&item.id, "item")?;
            ensure!(ids.insert(&item.id), "duplicate item id '{}'", item.id);
            ensure!(
                !item.label.trim().is_empty()
                    && item.label.chars().count() <= 60
                    && !item.label.contains('\n'),
                "item '{}' label is one line of 1 to 60 characters",
                item.id
            );
            let label_width = item.label.chars().count() as f32 * (self.size * 0.6);
            for text in [&item.result, &item.failure] {
                ensure!(
                    text.chars().count() <= 24 && !text.contains('\n'),
                    "item '{}' results are one line of at most 24 characters",
                    item.id
                );
                let result_width = if text.is_empty() {
                    0.0
                } else {
                    self.size * 0.75 + text.chars().count() as f32 * (result_size * 0.6)
                };
                let needed = label_offset + label_width + result_width;
                ensure!(
                    self.width + 1e-3 >= needed,
                    "checklist item '{}' needs width >= {:.0}, got {:.0}",
                    item.id,
                    needed.ceil(),
                    self.width
                );
            }
        }
        Ok(())
    }

    /// Whether `property` names a checklist channel, as preflight checks.
    pub fn accepts(&self, property: &str) -> bool {
        if matches!(property, "opacity" | "x" | "y") {
            return true;
        }
        property
            .strip_prefix("item.")
            .and_then(|rest| {
                ["reveal", "spinner", "mark", "outcome"]
                    .iter()
                    .find_map(|name| rest.strip_suffix(&format!(".{name}")))
            })
            .is_some_and(|id| self.find_item(id).is_some())
    }
}

/// The derived look of one item from its four channels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemPose {
    /// Row entrance, 0 to 1.
    pub reveal: f32,
    /// The hollow pending ring, fading as the item starts or resolves.
    pub pending: f32,
    /// Label brightness: muted while pending, plain once started.
    pub active: f32,
    /// The resolution while it lands (`None` until the mark clock starts).
    pub outcome: Option<Outcome>,
    /// How far the resolution has drawn, 0 to 1.
    pub resolved: f32,
    /// A skipped label's strike-through, drawn left to right.
    pub strike: f32,
    /// The rail segment below this item, filled top to bottom.
    pub rail: f32,
    /// The result text's presence.
    pub result: f32,
}

impl ItemPose {
    pub fn new(reveal: f32, spinner_age: f32, mark_age: f32, outcome: f32) -> Self {
        let reveal = reveal.clamp(0.0, 1.0);
        let outcome = (mark_age >= 0.0)
            .then(|| Outcome::from_value(outcome))
            .flatten();
        let started = if spinner_age >= 0.0 {
            smoothstep(spinner_age / 0.2)
        } else {
            0.0
        };
        let since = mark_age.max(0.0);
        let resolved = match outcome {
            Some(Outcome::Skipped) => smoothstep(since / SKIP_SECONDS),
            Some(_) => smoothstep(since / spinner::DRAW),
            None => 0.0,
        };
        let active = match outcome {
            Some(Outcome::Skipped) => started.max(resolved) * (1.0 - 0.55 * resolved),
            Some(_) => started.max(resolved),
            None => started,
        };
        Self {
            reveal,
            pending: (1.0 - started).min(1.0 - resolved),
            active,
            outcome,
            resolved,
            strike: match outcome {
                Some(Outcome::Skipped) => {
                    Ease::CubicOut.sample(((since - 0.08) / (SKIP_SECONDS - 0.08)).clamp(0.0, 1.0))
                }
                _ => 0.0,
            },
            rail: if outcome.is_some() {
                Ease::CubicBezier([0.45, 0.0, 0.2, 1.0])
                    .sample((since / RAIL_SECONDS).clamp(0.0, 1.0))
            } else {
                0.0
            },
            result: if outcome.is_some() {
                smoothstep((since - 0.2) / 0.3)
            } else {
                0.0
            },
        }
    }
}

/// An elapsed-seconds clock on `channel`: 0 at `at_nanos`, rising one per
/// second for `seconds` (to the scene's end when `None`), as
/// `StageActor::clock` writes. Whole milliseconds keep the slope exactly 1.
pub(crate) fn clock(
    scene: &mut PlanBuilder,
    channel: &ContinuousHandle,
    at_nanos: u64,
    seconds: Option<f32>,
) {
    let seconds = seconds.unwrap_or_else(|| {
        (scene.duration_nanos().saturating_sub(at_nanos) / 1_000_000) as f32 / 1000.0
    });
    scene.set(channel, at_nanos, 0.0);
    if seconds > 0.0 {
        scene.ease(channel, at_nanos, seconds, seconds, Ease::Linear);
    }
}

/// Authoring handle for one checklist actor. It remembers when each item
/// started, so a resolution waits for the spinner's next handoff crossing.
#[derive(Clone, Debug)]
pub struct ChecklistActor {
    actor: ActorHandle,
    plan: ChecklistPlan,
    started: HashMap<String, u64>,
}

impl ChecklistActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &ChecklistPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, CHECKLIST_RECIPE, plan)?;
        Ok(Self {
            actor,
            plan: plan.clone(),
            started: HashMap::new(),
        })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// `item.<item>.<property>`: `reveal`, `spinner`, `mark`, or `outcome`.
    pub fn item_channel(
        &mut self,
        scene: &mut PlanBuilder,
        item: &str,
        property: &str,
    ) -> Result<ContinuousHandle> {
        self.plan
            .find_item(item)
            .with_context(|| format!("checklist '{}' has no item '{item}'", self.id()))?;
        let initial = match property {
            "reveal" | "outcome" => 0.0,
            _ => -1.0,
        };
        Ok(self.channel(scene, &format!("item.{item}.{property}"), initial))
    }

    /// Fade and rise the whole list in. A list with a `show` starts hidden.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::show(scene, &self.actor, at_nanos);
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::hide(scene, &self.actor, at_nanos);
    }

    /// Bring every row in, top to bottom, 120 ms apart; returns when the last
    /// row starts. Rows without a reveal are visible from the start.
    pub fn reveal(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> Result<u64> {
        let ids = self
            .plan
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        let mut at = at_nanos;
        for (index, id) in ids.iter().enumerate() {
            at = at_nanos + ROW_STAGGER_NANOS * index as u64;
            self.reveal_item(scene, id, at)?;
        }
        Ok(at)
    }

    pub fn reveal_item(
        &mut self,
        scene: &mut PlanBuilder,
        item: &str,
        at_nanos: u64,
    ) -> Result<()> {
        let reveal = self.item_channel(scene, item, "reveal")?;
        scene.spring(&reveal, at_nanos, 1.0, 0.5, 0.0);
        Ok(())
    }

    /// Start `item` running: its spinner's motor builds from rest. Starting
    /// a resolved item again clears its mark, as for a retry.
    pub fn start(&mut self, scene: &mut PlanBuilder, item: &str, at_nanos: u64) -> Result<()> {
        let spinner = self.item_channel(scene, item, "spinner")?;
        if self.started.contains_key(item) {
            let mark = self.item_channel(scene, item, "mark")?;
            let outcome = self.item_channel(scene, item, "outcome")?;
            scene.set(&mark, at_nanos, -1.0);
            scene.set(&outcome, at_nanos, 0.0);
        }
        clock(scene, &spinner, at_nanos, None);
        self.started.insert(item.to_owned(), at_nanos);
        Ok(())
    }

    /// Resolve `item` at `at_nanos`. A running spinner keeps turning until
    /// its tip next crosses the handoff angle, then draws the mark; an item
    /// that never ran draws its mark at once. Returns when the mark (or a
    /// skip's strike) finishes.
    pub fn resolve(
        &mut self,
        scene: &mut PlanBuilder,
        item: &str,
        at_nanos: u64,
        outcome: impl Into<Outcome>,
    ) -> Result<u64> {
        let outcome = outcome.into();
        let mark = self.item_channel(scene, item, "mark")?;
        let value = self.item_channel(scene, item, "outcome")?;
        let running = self.started.get(item).copied();
        let start = match running {
            Some(started) => started,
            None if outcome == Outcome::Skipped => at_nanos,
            None => {
                // The mark draws from a standing motor: no ring, no wait.
                self.start(scene, item, at_nanos)?;
                at_nanos
            }
        };
        let landed = match (outcome, running) {
            (Outcome::Skipped, _) | (_, None) => at_nanos,
            _ => {
                let age = at_nanos.saturating_sub(start) as f64 * 1e-9;
                let handoff = f64::from(spinner::handoff(age as f32));
                start + crate::author::seconds(handoff).max(at_nanos - start)
            }
        };
        scene.set(&value, landed, outcome.value());
        clock(scene, &mark, landed, None);
        self.started.entry(item.to_owned()).or_insert(start);
        let draw = match outcome {
            Outcome::Skipped => SKIP_SECONDS,
            _ => spinner::DRAW,
        };
        Ok(landed + whole_millis(draw))
    }

    /// Skip `item`: a running spinner coasts out, a dash draws, and the label
    /// strikes through and dims. Returns when the strike finishes.
    pub fn skip(&mut self, scene: &mut PlanBuilder, item: &str, at_nanos: u64) -> Result<u64> {
        self.resolve(scene, item, at_nanos, Outcome::Skipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{ScalarPlan, TrackEventPlan};

    fn plan() -> ChecklistPlan {
        ChecklistPlan::new([200.0, 200.0], 700.0)
            .item(ChecklistItemPlan::new("typecheck", "typecheck").result("4.2s"))
            .item(
                ChecklistItemPlan::new("unit", "unit tests")
                    .result("812 passed")
                    .failure("2 failed"),
            )
            .item(ChecklistItemPlan::new("e2e", "e2e"))
            .title("checks")
            .rail()
    }

    #[test]
    fn checklists_round_trip_with_compact_defaults_and_lay_out_rows() {
        let plan = plan();
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert!(json.get("size").is_none() && json.get("panel").is_none());
        assert_eq!(serde_json::from_value::<ChecklistPlan>(json).unwrap(), plan);
        let pitch = plan.pitch();
        assert!((plan.row_center(1) - plan.row_center(0) - pitch).abs() < 1e-4);
        assert!(plan.row_center(0) > plan.origin[1] + plan.title_height());
        assert!(plan.label_x() > plan.icon_x() + plan.icon_size() * 0.5);
        let unit = &plan.items[1];
        assert_eq!(unit.result_for(Some(Outcome::Failed)), "2 failed");
        assert_eq!(unit.result_for(Some(Outcome::Done)), "812 passed");
        assert_eq!(plan.items[0].result_for(Some(Outcome::Failed)), "4.2s");
    }

    #[test]
    fn invalid_checklists_and_channels_are_rejected() {
        let mut duplicate = plan();
        duplicate.items[1].id = "typecheck".into();
        assert!(duplicate.validate().is_err());
        let mut empty = plan();
        empty.items.clear();
        assert!(empty.validate().is_err());
        let mut narrow = plan();
        narrow.width = 240.0;
        assert!(narrow.validate().is_err());
        let plan = plan();
        for property in ["opacity", "item.unit.spinner", "item.e2e.outcome"] {
            assert!(plan.accepts(property), "{property}");
        }
        for property in ["item.unit.spin", "item.lint.mark", "typed"] {
            assert!(!plan.accepts(property), "{property}");
        }
    }

    #[test]
    fn poses_follow_pending_running_and_each_outcome() {
        let pending = ItemPose::new(1.0, -1.0, -1.0, 0.0);
        assert_eq!((pending.pending, pending.active), (1.0, 0.0));
        assert_eq!(pending.outcome, None);
        let running = ItemPose::new(1.0, 1.0, -1.0, 0.0);
        assert_eq!((running.pending, running.active), (0.0, 1.0));
        // The outcome channel only counts once the mark clock runs.
        assert_eq!(ItemPose::new(1.0, 1.0, -1.0, 1.0).outcome, None);
        let done = ItemPose::new(1.0, 2.0, 1.0, 1.0);
        assert_eq!(done.outcome, Some(Outcome::Done));
        assert_eq!((done.resolved, done.rail, done.result), (1.0, 1.0, 1.0));
        assert_eq!(done.strike, 0.0);
        let skipped = ItemPose::new(1.0, -1.0, 1.0, 3.0);
        assert_eq!(skipped.strike, 1.0);
        assert!(skipped.active < 0.5, "a skipped label dims");
        // A rounded native transition still names an outcome.
        assert_eq!(Outcome::from_value(1.9), Some(Outcome::Failed));
        // Mid-draw is between the endpoints.
        let landing = ItemPose::new(1.0, 2.0, 0.2, 2.0);
        assert!(landing.resolved > 0.0 && landing.resolved < 1.0);
    }

    fn events<'a>(plan: &'a crate::plan::ScenePlan, property: &str) -> &'a [TrackEventPlan] {
        &plan
            .continuous_channels
            .iter()
            .find(|channel| channel.property == property)
            .unwrap()
            .events
    }

    #[test]
    fn resolutions_wait_for_the_spinners_handoff_and_retries_clear_the_mark() {
        let mut scene = PlanBuilder::new("checks", 10_000_000_000);
        let mut checks = ChecklistActor::declare(&mut scene, "checks", &plan()).unwrap();
        assert_eq!(checks.reveal(&mut scene, 0).unwrap(), 2 * ROW_STAGGER_NANOS);
        checks
            .start(&mut scene, "typecheck", 1_000_000_000)
            .unwrap();
        let done = checks
            .resolve(&mut scene, "typecheck", 2_000_000_000, Mark::Check)
            .unwrap();
        let landed = done - whole_millis(spinner::DRAW);
        let handoff = spinner::handoff(1.0);
        assert_eq!(
            landed,
            1_000_000_000 + crate::author::seconds(f64::from(handoff))
        );
        assert!(landed >= 2_000_000_000);
        // Never started: the mark draws at once, from a standing start.
        let unit = checks
            .resolve(&mut scene, "unit", 3_000_000_000, Outcome::Failed)
            .unwrap();
        assert_eq!(unit, 3_000_000_000 + whole_millis(spinner::DRAW));
        // A retry clears the cross and runs again.
        checks.start(&mut scene, "unit", 4_000_000_000).unwrap();
        checks.skip(&mut scene, "e2e", 4_000_000_000).unwrap();
        let plan = scene.finish().unwrap();
        let unit_mark = plan
            .continuous_channels
            .iter()
            .find(|channel| channel.id == "checks.item.unit.mark")
            .unwrap();
        assert!(unit_mark.events.iter().any(|event| matches!(
            event,
            TrackEventPlan::Set { at_nanos: 4_000_000_000, value: ScalarPlan::Literal(v) } if *v == -1.0
        )));
        assert!(!events(&plan, "item.e2e.outcome").is_empty());
        assert!(
            plan.continuous_channels
                .iter()
                .all(|channel| channel.property != "item.e2e.spinner"),
            "a skip that never ran has no spinner"
        );
    }
}
