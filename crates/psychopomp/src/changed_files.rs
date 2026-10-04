//! Changed Files: the opener of a pull-request film. A card lists file paths
//! with added, modified, deleted, or renamed badges, per-file `+N −M` counts,
//! and GitHub's five-block diffstat; rows reveal in a stagger, one can be
//! highlighted while the rest dim, and the header's totals roll as rows land.
//!
//! Rows hold fixed slots (identity is the file's ID), so revealing one never
//! moves another. The totals are a Rolling Number schedule carried in the
//! recipe, like a Rolling Number's own values: plans whose totals roll are
//! export-only; a list whose totals are static is channel-only.
use std::collections::HashSet;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder, seconds},
    tone::Tone,
    window,
};

pub const CHANGED_FILES_RECIPE: &str = "changed-files";

const DEFAULT_SIZE: f32 = 22.0;
/// Row pitch and header height, in font sizes.
const ROW_EM: f32 = 2.05;
const HEADER_EM: f32 = 2.9;
pub const PADDING: f32 = 24.0;
const MAX_FILES: usize = 300;

const REVEAL_SECONDS: f32 = 0.42;
/// Totals roll a moment after a row starts to land.
const TOTALS_LAG: f64 = 0.12;
const FOCUS_SECONDS: f32 = 0.35;
const HIGHLIGHT_IN_SECONDS: f32 = 0.2;
const HIGHLIGHT_OUT_SECONDS: f32 = 0.45;
const SCROLL_SECONDS: f32 = 0.55;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangedFilesPlan {
    /// Top-left corner of the card.
    pub origin: [f32; 2],
    pub width: f32,
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub size: f32,
    /// Rows visible at once; more scroll through the card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rows: Option<u32>,
    /// A title bar above the list, such as the pull request's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub files: Vec<ChangedFilePlan>,
    /// Totals that roll as rows land, in increasing time. Before the first
    /// they are zero; without any they are the sum of every file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub totals: Vec<ChangedTotalsPlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangedFilePlan {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "FileStatus::is_default")]
    pub status: FileStatus,
    #[serde(default)]
    pub added: u32,
    #[serde(default)]
    pub removed: u32,
    /// The previous path of a renamed file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileStatus {
    Added,
    #[default]
    Modified,
    Deleted,
    Renamed,
}

impl FileStatus {
    pub fn is_default(&self) -> bool {
        *self == Self::Modified
    }

    pub fn letter(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Self::Added => Tone::Success,
            Self::Modified => Tone::Warning,
            Self::Deleted => Tone::Error,
            Self::Renamed => Tone::Request,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangedTotalsPlan {
    #[serde(default)]
    pub at_nanos: u64,
    pub files: u32,
    pub added: u32,
    pub removed: u32,
}

/// One of a diffstat's five blocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Block {
    Added,
    Removed,
    Neutral,
}

/// GitHub's five-block diffstat: a change of fewer than five lines shows one
/// block per line, the rest neutral; a larger one splits all five by the
/// share of additions, keeping at least one block for each nonzero side.
pub fn diffstat(added: u32, removed: u32) -> [Block; 5] {
    let total = added + removed;
    let (green, red) = if total == 0 {
        (0, 0)
    } else if total < 5 {
        (added as usize, removed as usize)
    } else {
        let mut green = (5.0 * added as f64 / total as f64).round() as usize;
        if added > 0 {
            green = green.max(1);
        }
        if removed > 0 {
            green = green.min(4);
        }
        (green, 5 - green)
    };
    std::array::from_fn(|index| {
        if index < green {
            Block::Added
        } else if index < green + red {
            Block::Removed
        } else {
            Block::Neutral
        }
    })
}

/// `1383` as `1,383`.
pub fn grouped(count: u32) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A path's directory (with its trailing `/`) and file name.
pub fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(slash) => path.split_at(slash + 1),
        None => ("", path),
    }
}

fn default_size() -> f32 {
    DEFAULT_SIZE
}

fn is_default_size(size: &f32) -> bool {
    *size == DEFAULT_SIZE
}

impl ChangedFilePlan {
    pub fn new(
        id: impl Into<String>,
        path: impl Into<String>,
        status: FileStatus,
        added: u32,
        removed: u32,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            status,
            added,
            removed,
            from: None,
        }
    }

    pub fn renamed_from(mut self, path: impl Into<String>) -> Self {
        self.status = FileStatus::Renamed;
        self.from = Some(path.into());
        self
    }
}

/// A per-row channel: `row.<id>.<name>`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowChannel {
    /// Presence, 0 to 1: the row fades and rises into its slot.
    Reveal,
    /// An accent bar and wash behind the row.
    Highlight,
    /// Recedes the row, 0 to 1, while another is in focus.
    Dim,
}

impl RowChannel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Reveal => "reveal",
            Self::Highlight => "highlight",
            Self::Dim => "dim",
        }
    }

    pub fn parse(property: &str) -> Option<(&str, Self)> {
        let (id, name) = window::split_channel(property, "row")?;
        let channel = match name {
            "reveal" => Self::Reveal,
            "highlight" => Self::Highlight,
            "dim" => Self::Dim,
            _ => return None,
        };
        Some((id, channel))
    }
}

pub fn row_property(id: &str, channel: RowChannel) -> String {
    format!("row.{id}.{}", channel.name())
}

/// Window channels plus `scroll` (the first visible row).
pub fn accepts_property(property: &str) -> bool {
    window::is_window_property(property) || property == "scroll"
}

impl ChangedFilesPlan {
    pub fn new(origin: [f32; 2], width: f32, files: Vec<ChangedFilePlan>) -> Self {
        Self {
            origin,
            width,
            size: DEFAULT_SIZE,
            max_rows: None,
            title: None,
            files,
            totals: Vec::new(),
        }
    }

    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn max_rows(mut self, rows: u32) -> Self {
        self.max_rows = Some(rows);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.files.iter().position(|file| file.id == id)
    }

    pub fn row_height(&self) -> f32 {
        (self.size * ROW_EM).round()
    }

    pub fn header_height(&self) -> f32 {
        (self.size * HEADER_EM).round()
    }

    fn title_bar(&self) -> f32 {
        if self.title.is_some() {
            window::TITLE_BAR
        } else {
            0.0
        }
    }

    pub fn visible_rows(&self) -> usize {
        self.max_rows.map_or(self.files.len(), |rows| {
            (rows as usize).min(self.files.len())
        })
    }

    /// The card's `[width, height]`.
    pub fn card_size(&self) -> [f32; 2] {
        [
            self.width,
            self.title_bar()
                + self.header_height()
                + self.visible_rows() as f32 * self.row_height()
                + PADDING * 0.5,
        ]
    }

    /// Canvas y of the header row's center.
    pub fn header_center(&self) -> f32 {
        self.origin[1] + self.title_bar() + self.header_height() * 0.5
    }

    /// Canvas y of the top of the rows' window.
    pub fn rows_top(&self) -> f32 {
        self.origin[1] + self.title_bar() + self.header_height()
    }

    /// Canvas y of row `index`'s top when `scroll` rows have scrolled away.
    pub fn row_top(&self, index: usize, scroll: f32) -> f32 {
        self.rows_top() + (index as f32 - scroll) * self.row_height()
    }

    /// Totals of every file.
    pub fn sum(&self) -> ChangedTotalsPlan {
        ChangedTotalsPlan {
            at_nanos: 0,
            files: self.files.len() as u32,
            added: self.files.iter().map(|file| file.added).sum(),
            removed: self.files.iter().map(|file| file.removed).sum(),
        }
    }

    /// The totals shown from time zero, and the later ones.
    pub fn totals_schedule(&self) -> (ChangedTotalsPlan, &[ChangedTotalsPlan]) {
        if self.totals.is_empty() {
            (self.sum(), &[])
        } else {
            (ChangedTotalsPlan::default(), &self.totals)
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "changed files origin must be finite"
        );
        ensure!(
            self.width.is_finite() && (400.0..=3800.0).contains(&self.width),
            "changed files width must be between 400 and 3800"
        );
        ensure!(
            (12.0..=48.0).contains(&self.size),
            "changed files size must be between 12 and 48"
        );
        ensure!(
            !self.files.is_empty() && self.files.len() <= MAX_FILES,
            "changed files lists one to {MAX_FILES} files"
        );
        ensure!(
            self.max_rows.is_none_or(|rows| (1..=60).contains(&rows)),
            "changed files maxRows must be between 1 and 60"
        );
        let mut ids = HashSet::new();
        for file in &self.files {
            ensure!(
                window::valid_id(&file.id),
                "changed file id '{}' must be letters, digits, '-', or '_'",
                file.id
            );
            ensure!(
                ids.insert(file.id.as_str()),
                "changed file '{}' is repeated",
                file.id
            );
            ensure!(
                !file.path.is_empty() && !file.path.contains('\n'),
                "changed file '{}' needs a one-line path",
                file.id
            );
            ensure!(
                file.from.is_none() || file.status == FileStatus::Renamed,
                "changed file '{}' has a previous path but is not renamed",
                file.id
            );
        }
        ensure!(
            self.totals
                .windows(2)
                .all(|pair| pair[0].at_nanos < pair[1].at_nanos),
            "changed files totals must be in strictly increasing time order"
        );
        Ok(())
    }
}

/// Authoring handle for one changed-files card.
pub struct ChangedFilesActor {
    actor: ActorHandle,
    plan: ChangedFilesPlan,
    revealed: Vec<bool>,
    focused: Option<String>,
}

impl ChangedFilesActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: ChangedFilesPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, CHANGED_FILES_RECIPE, &plan)?;
        Ok(Self {
            actor,
            revealed: vec![false; plan.files.len()],
            plan,
            focused: None,
        })
    }

    pub fn plan(&self) -> &ChangedFilesPlan {
        &self.plan
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    fn row_channel(
        &mut self,
        scene: &mut PlanBuilder,
        id: &str,
        channel: RowChannel,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, &row_property(id, channel), initial)
    }

    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> u64 {
        window::settle_in(scene, &self.actor, at_nanos)
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        window::dismiss(scene, &self.actor, at_nanos);
    }

    /// Reveal one row at `at_nanos`; the totals roll to include it. Once any
    /// row is revealed through the handle, every row starts hidden.
    pub fn reveal_row(&mut self, scene: &mut PlanBuilder, id: &str, at_nanos: u64) -> Result<()> {
        let index = self
            .plan
            .find(id)
            .with_context(|| format!("changed file '{id}' is not listed"))?;
        ensure!(
            !self.revealed[index],
            "changed file '{id}' is already revealed"
        );
        let ids = self
            .plan
            .files
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        for other in &ids {
            self.row_channel(scene, other, RowChannel::Reveal, 0.0);
        }
        let reveal = self.row_channel(scene, id, RowChannel::Reveal, 0.0);
        scene.spring(&reveal, at_nanos, 1.0, REVEAL_SECONDS, 0.0);
        self.revealed[index] = true;
        let mut totals = ChangedTotalsPlan {
            at_nanos: at_nanos + seconds(TOTALS_LAG),
            ..ChangedTotalsPlan::default()
        };
        for (file, _) in self
            .plan
            .files
            .iter()
            .zip(&self.revealed)
            .filter(|(_, shown)| **shown)
        {
            totals.files += 1;
            totals.added += file.added;
            totals.removed += file.removed;
        }
        if let Some(last) = self.plan.totals.last() {
            ensure!(
                last.at_nanos < totals.at_nanos,
                "changed files rows must be revealed in increasing time order"
            );
        }
        self.plan.totals.push(totals);
        self.plan.validate()?;
        scene.replace_actor_data(&self.actor, &self.plan)?;
        Ok(())
    }

    /// Reveal every row in order from `at_nanos`, `stagger` seconds apart.
    /// Returns when the last row has landed.
    pub fn reveal(&mut self, scene: &mut PlanBuilder, at_nanos: u64, stagger: f32) -> Result<u64> {
        let ids = self
            .plan
            .files
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        // Rows share no instant, so each one's totals roll has its own time.
        let gap = seconds(f64::from(stagger.max(0.0))).max(1_000_000);
        let mut last = at_nanos;
        for (index, id) in ids.iter().enumerate() {
            last = at_nanos + gap * index as u64;
            self.reveal_row(scene, id, last)?;
        }
        Ok(last + seconds(f64::from(REVEAL_SECONDS)))
    }

    /// Light a row's bar at `at_nanos` and let it go `for_seconds` later.
    pub fn highlight(
        &mut self,
        scene: &mut PlanBuilder,
        id: &str,
        at_nanos: u64,
        for_seconds: f32,
    ) -> Result<()> {
        ensure!(
            self.plan.find(id).is_some(),
            "changed file '{id}' is not listed"
        );
        let channel = self.row_channel(scene, id, RowChannel::Highlight, 0.0);
        scene.spring(&channel, at_nanos, 1.0, HIGHLIGHT_IN_SECONDS, 0.0);
        let off = at_nanos + seconds(f64::from(for_seconds.max(0.0)));
        scene.spring(&channel, off, 0.0, HIGHLIGHT_OUT_SECONDS, 0.0);
        Ok(())
    }

    /// Highlight `id` and recede every other row until [`Self::unfocus`].
    pub fn focus(&mut self, scene: &mut PlanBuilder, id: &str, at_nanos: u64) -> Result<()> {
        ensure!(
            self.plan.find(id).is_some(),
            "changed file '{id}' is not listed"
        );
        let ids = self
            .plan
            .files
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        for other in &ids {
            let dim = self.row_channel(scene, other, RowChannel::Dim, 0.0);
            scene.spring(
                &dim,
                at_nanos,
                f32::from(u8::from(other != id)),
                FOCUS_SECONDS,
                0.0,
            );
            let highlight = self.row_channel(scene, other, RowChannel::Highlight, 0.0);
            scene.spring(
                &highlight,
                at_nanos,
                f32::from(u8::from(other == id)),
                FOCUS_SECONDS,
                0.0,
            );
        }
        self.focused = Some(id.to_owned());
        Ok(())
    }

    pub fn unfocus(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        if self.focused.take().is_none() {
            return;
        }
        let ids = self
            .plan
            .files
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        for id in &ids {
            let dim = self.row_channel(scene, id, RowChannel::Dim, 0.0);
            scene.spring(&dim, at_nanos, 0.0, FOCUS_SECONDS, 0.0);
            let highlight = self.row_channel(scene, id, RowChannel::Highlight, 0.0);
            scene.spring(&highlight, at_nanos, 0.0, HIGHLIGHT_OUT_SECONDS, 0.0);
        }
    }

    /// Scroll so `row` is the first visible.
    pub fn scroll_to(&mut self, scene: &mut PlanBuilder, row: f32, at_nanos: u64) {
        let channel = self.channel(scene, "scroll", 0.0);
        scene.spring(&channel, at_nanos, row, SCROLL_SECONDS, 0.0);
    }
}

#[cfg(test)]
mod tests;
