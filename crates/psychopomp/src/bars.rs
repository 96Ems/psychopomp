//! Benchmark Bars: a horizontal bar chart comparing one or more series per
//! row (before/after, or several runs) on one shared Axis. Bars grow on
//! springs, each with a Readout at its end that counts with it; a delta chip
//! such as `−34%` compares two series; and rows can re-sort (a race) while
//! keeping their identity, because a row's place is its own `slot` channel.
use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    axis::AxisPlan,
    readout::ReadoutFormat,
    tone::Tone,
};

pub const BARS_RECIPE: &str = "bars";

/// Rows grow, enter, and reveal their chips this far apart.
pub const ROW_STAGGER_NANOS: u64 = 90_000_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BarsPlan {
    /// Where bars start (x) and the top of the first row (y).
    pub origin: [f32; 2],
    /// The axis length: a bar at the end of the range is this long.
    pub width: f32,
    pub axis: AxisPlan,
    pub series: Vec<BarSeriesPlan>,
    pub rows: Vec<BarRowPlan>,
    #[serde(
        default = "default_row_height",
        skip_serializing_if = "is_default_row_height"
    )]
    pub row_height: f32,
    /// Label and readout size.
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub size: f32,
    /// How each bar's value prints at its end.
    #[serde(default, skip_serializing_if = "is_default_readout")]
    pub readout: ReadoutFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<BarDeltaPlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BarSeriesPlan {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "Tone::is_default")]
    pub tone: Tone,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BarRowPlan {
    pub id: String,
    pub label: String,
}

/// A chip comparing series `to` against `from` in every row.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BarDeltaPlan {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Better::is_default")]
    pub better: Better,
    #[serde(default, skip_serializing_if = "DeltaFormat::is_default")]
    pub format: DeltaFormat,
}

/// Which direction is an improvement: lower for latency, higher for throughput.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Better {
    #[default]
    Lower,
    Higher,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeltaFormat {
    /// The change relative to `from`: `−34%`.
    #[default]
    Percent,
    /// How many times better: `2.4×`.
    Factor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Better {
    fn is_default(&self) -> bool {
        *self == Self::Lower
    }
}

impl DeltaFormat {
    fn is_default(&self) -> bool {
        *self == Self::Percent
    }
}

fn default_row_height() -> f32 {
    64.0
}

fn is_default_row_height(value: &f32) -> bool {
    *value == default_row_height()
}

fn default_size() -> f32 {
    24.0
}

fn is_default_size(value: &f32) -> bool {
    *value == default_size()
}

fn is_default_readout(format: &ReadoutFormat) -> bool {
    *format == ReadoutFormat::default()
}

impl BarSeriesPlan {
    pub fn new(id: impl Into<String>, label: impl Into<String>, tone: Tone) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tone,
        }
    }
}

impl BarDeltaPlan {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            better: Better::Lower,
            format: DeltaFormat::Percent,
        }
    }

    pub fn higher_is_better(mut self) -> Self {
        self.better = Better::Higher;
        self
    }

    pub fn factor(mut self) -> Self {
        self.format = DeltaFormat::Factor;
        self
    }

    /// The chip's text and tone comparing `to` with `from`; `None` when
    /// `from` is zero. An improvement is success and a regression an error.
    pub fn text(&self, from: f32, to: f32) -> Option<(String, Tone)> {
        if from.abs() < 1e-6 || !from.is_finite() || !to.is_finite() {
            return None;
        }
        let change = (to - from) / from;
        let improved = match self.better {
            Better::Lower => change < 0.0,
            Better::Higher => change > 0.0,
        };
        Some(match self.format {
            DeltaFormat::Percent => {
                let percent = (change * 100.0).round();
                if percent == 0.0 {
                    ("±0%".to_owned(), Tone::Muted)
                } else {
                    let sign = if percent < 0.0 { '−' } else { '+' };
                    (
                        format!("{sign}{}%", percent.abs()),
                        if improved { Tone::Success } else { Tone::Error },
                    )
                }
            }
            DeltaFormat::Factor => {
                let ratio = match self.better {
                    Better::Lower => from / to.max(1e-6),
                    Better::Higher => to / from,
                };
                let digits = if ratio >= 10.0 { 0 } else { 1 };
                (
                    format!("{ratio:.digits$}×"),
                    if ratio >= 1.0 {
                        Tone::Success
                    } else {
                        Tone::Error
                    },
                )
            }
        })
    }
}

impl BarsPlan {
    pub fn new(origin: [f32; 2], width: f32, axis: AxisPlan) -> Self {
        Self {
            origin,
            width,
            axis,
            series: Vec::new(),
            rows: Vec::new(),
            row_height: default_row_height(),
            size: default_size(),
            readout: ReadoutFormat::default(),
            delta: None,
        }
    }

    pub fn series(mut self, series: BarSeriesPlan) -> Self {
        self.series.push(series);
        self
    }

    pub fn row(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.rows.push(BarRowPlan {
            id: id.into(),
            label: label.into(),
        });
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn readout(mut self, format: ReadoutFormat) -> Self {
        self.readout = format;
        self
    }

    pub fn delta(mut self, delta: BarDeltaPlan) -> Self {
        self.delta = Some(delta);
        self
    }

    pub fn find_row(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|row| row.id == id)
    }

    pub fn find_series(&self, id: &str) -> Option<usize> {
        self.series.iter().position(|series| series.id == id)
    }

    /// Thickness of one bar: the row's bars share about 62% of its height.
    pub fn bar_thickness(&self) -> f32 {
        let count = self.series.len().max(1) as f32;
        ((self.row_height * 0.62 - BAR_GAP * (count - 1.0)) / count)
            .clamp(4.0, 30.0)
            .round()
    }

    /// Vertical center of a row in `slot` (fractional while sorting).
    pub fn row_center(&self, slot: f32) -> f32 {
        self.origin[1] + (slot + 0.5) * self.row_height
    }

    /// Offset of series `index`'s bar from its row's center.
    pub fn bar_offset(&self, index: usize) -> f32 {
        let count = self.series.len() as f32;
        let pitch = self.bar_thickness() + BAR_GAP;
        (index as f32 - (count - 1.0) * 0.5) * pitch
    }

    /// Canvas x of a bar's end at `value`, before offsets; a bar may run 4%
    /// past the end of the range, no further.
    pub fn bar_end(&self, value: f32) -> f32 {
        self.origin[0] + self.axis.fraction(value).clamp(0.0, 1.04) * self.width
    }

    /// Where the axis line sits, below the last row.
    pub fn axis_y(&self) -> f32 {
        self.origin[1] + self.rows.len() as f32 * self.row_height + 10.0
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite())
                && self.width.is_finite()
                && self.width >= 160.0,
            "bars need a finite origin and a width of at least 160"
        );
        self.axis.validate("bars")?;
        ensure!(
            (12.0..=64.0).contains(&self.size),
            "bars size must be between 12 and 64"
        );
        ensure!(
            self.row_height.is_finite() && self.row_height >= self.size * 1.3,
            "bars row height must be at least 1.3 × size"
        );
        ensure!(
            (1..=4).contains(&self.series.len()),
            "bars have 1 to 4 series"
        );
        ensure!(
            (1..=12).contains(&self.rows.len()),
            "bars have 1 to 12 rows"
        );
        self.readout.validate()?;
        let ids = |kind: &str, ids: Vec<(&String, &String)>| -> Result<()> {
            let mut seen = HashSet::new();
            for (id, label) in ids {
                ensure!(
                    !id.is_empty() && !id.contains(['.', ' ', '\t', '\n']),
                    "{kind} id '{id}' must be non-empty without dots or whitespace"
                );
                ensure!(seen.insert(id), "duplicate {kind} id '{id}'");
                ensure!(
                    label.chars().count() <= 32 && !label.contains('\n'),
                    "{kind} '{id}' label is one line of at most 32 characters"
                );
            }
            Ok(())
        };
        ids(
            "row",
            self.rows.iter().map(|row| (&row.id, &row.label)).collect(),
        )?;
        ids(
            "series",
            self.series.iter().map(|s| (&s.id, &s.label)).collect(),
        )?;
        if let Some(delta) = &self.delta {
            ensure!(
                self.find_series(&delta.from).is_some()
                    && self.find_series(&delta.to).is_some()
                    && delta.from != delta.to,
                "bars delta compares two different declared series"
            );
        }
        Ok(())
    }

    /// Whether `property` names a bars channel, as preflight checks.
    pub fn accepts(&self, property: &str) -> bool {
        if matches!(property, "opacity" | "x" | "y" | "axes") {
            return true;
        }
        let parts = property.split('.').collect::<Vec<_>>();
        match parts.as_slice() {
            ["row", row, "slot" | "opacity"] => self.find_row(row).is_some(),
            ["bar", row, series] => {
                self.find_row(row).is_some() && self.find_series(series).is_some()
            }
            ["delta", row] => self.delta.is_some() && self.find_row(row).is_some(),
            _ => false,
        }
    }
}

const BAR_GAP: f32 = 5.0;

/// Row indices in display order: by `values` in `order`, ties keeping
/// declaration order, so equal rows never swap.
pub fn ranking(values: &[f32], order: SortOrder) -> Vec<usize> {
    let mut indices = (0..values.len()).collect::<Vec<_>>();
    indices.sort_by(|&a, &b| {
        let ordering = values[a].total_cmp(&values[b]);
        match order {
            SortOrder::Ascending => ordering,
            SortOrder::Descending => ordering.reverse(),
        }
        .then(a.cmp(&b))
    });
    indices
}

/// Row indices in paint order while rows re-sort, from each row's slot
/// velocity (slots per second): rows at rest first, then rows moving down,
/// then rows moving up, so a rising row passes over the one it overtakes.
pub fn paint_order(slot_speeds: &[f32]) -> Vec<usize> {
    let rank = |speed: f32| {
        if speed.abs() < MOVING_SLOT_SPEED {
            0
        } else if speed > 0.0 {
            1
        } else {
            2
        }
    };
    let mut order = (0..slot_speeds.len()).collect::<Vec<_>>();
    order.sort_by_key(|&row| (rank(slot_speeds[row]), row));
    order
}

/// Below this slot speed a row counts as resting.
pub const MOVING_SLOT_SPEED: f32 = 0.05;

/// Authoring handle for one bars actor. It remembers each bar's latest
/// target, so `sort` orders rows by where their bars are going.
pub struct BarsActor {
    actor: ActorHandle,
    plan: BarsPlan,
    targets: HashMap<(usize, usize), f32>,
    slots: Vec<f32>,
}

impl BarsActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &BarsPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, BARS_RECIPE, plan)?;
        Ok(Self {
            actor,
            plan: plan.clone(),
            targets: HashMap::new(),
            slots: (0..plan.rows.len()).map(|index| index as f32).collect(),
        })
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

    /// Fade and rise in while the axis and gridlines draw on.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::show(scene, &self.actor, at_nanos);
        let axes = self.channel(scene, "axes", 0.0);
        scene.ease(&axes, at_nanos, 1.0, 0.9, crate::plot::DRAW_EASE);
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::hide(scene, &self.actor, at_nanos);
    }

    /// Fade rows in from the top of the display order, 90 ms apart.
    pub fn reveal_rows(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> u64 {
        let mut at = at_nanos;
        for (rank, row) in self.display_order().into_iter().enumerate() {
            at = at_nanos + ROW_STAGGER_NANOS * rank as u64;
            let id = self.plan.rows[row].id.clone();
            let opacity = self.channel(scene, &format!("row.{id}.opacity"), 0.0);
            scene.spring(&opacity, at, 1.0, 0.45, 0.0);
        }
        at
    }

    /// Spring one bar to `value`.
    pub fn set(
        &mut self,
        scene: &mut PlanBuilder,
        row: &str,
        series: &str,
        at_nanos: u64,
        value: f32,
    ) -> Result<()> {
        let key = self.key(row, series)?;
        let bar = self.channel(scene, &format!("bar.{row}.{series}"), 0.0);
        scene.spring(&bar, at_nanos, value, 0.9, 0.08);
        self.targets.insert(key, value);
        Ok(())
    }

    /// Grow `series`' bars to `values` (row id, value), staggered down the
    /// current display order. Returns when the last bar starts.
    pub fn grow(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        series: &str,
        values: &[(&str, f32)],
    ) -> Result<u64> {
        let order = self.display_order();
        let mut writes = values
            .iter()
            .map(|(row, value)| {
                let index = self
                    .plan
                    .find_row(row)
                    .with_context(|| format!("bars '{}' have no row '{row}'", self.id()))?;
                let rank = order.iter().position(|&r| r == index).unwrap_or(index);
                Ok((rank, *row, *value))
            })
            .collect::<Result<Vec<_>>>()?;
        writes.sort_by_key(|(rank, ..)| *rank);
        let mut last = at_nanos;
        for (step, (_, row, value)) in writes.into_iter().enumerate() {
            last = at_nanos + ROW_STAGGER_NANOS * step as u64;
            self.set(scene, row, series, last, value)?;
        }
        Ok(last)
    }

    /// Re-sort rows by `series`' latest targets: each row springs to its
    /// new slot, keeping its identity and velocity. Returns the new order.
    pub fn sort(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        series: &str,
        order: SortOrder,
    ) -> Result<Vec<String>> {
        let column = self
            .plan
            .find_series(series)
            .with_context(|| format!("bars '{}' have no series '{series}'", self.id()))?;
        let values = (0..self.plan.rows.len())
            .map(|row| self.targets.get(&(row, column)).copied().unwrap_or(0.0))
            .collect::<Vec<_>>();
        let ranked = ranking(&values, order);
        for (slot, &row) in ranked.iter().enumerate() {
            let id = self.plan.rows[row].id.clone();
            let channel = self.channel(scene, &format!("row.{id}.slot"), row as f32);
            if self.slots[row] != slot as f32 {
                scene.spring(&channel, at_nanos, slot as f32, 0.7, 0.1);
                self.slots[row] = slot as f32;
            }
        }
        Ok(ranked
            .into_iter()
            .map(|row| self.plan.rows[row].id.clone())
            .collect())
    }

    /// Pop every row's delta chip in, down the display order.
    pub fn reveal_deltas(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> Result<u64> {
        ensure!(
            self.plan.delta.is_some(),
            "bars '{}' have no delta",
            self.id()
        );
        let mut at = at_nanos;
        for (rank, row) in self.display_order().into_iter().enumerate() {
            at = at_nanos + ROW_STAGGER_NANOS * rank as u64;
            let id = self.plan.rows[row].id.clone();
            let chip = self.channel(scene, &format!("delta.{id}"), 0.0);
            scene.spring(&chip, at, 1.0, 0.45, 0.25);
        }
        Ok(at)
    }

    /// Row indices from top to bottom, by their latest slot targets.
    fn display_order(&self) -> Vec<usize> {
        ranking(&self.slots, SortOrder::Ascending)
    }

    fn key(&self, row: &str, series: &str) -> Result<(usize, usize)> {
        Ok((
            self.plan
                .find_row(row)
                .with_context(|| format!("bars '{}' have no row '{row}'", self.id()))?,
            self.plan
                .find_series(series)
                .with_context(|| format!("bars '{}' have no series '{series}'", self.id()))?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> BarsPlan {
        BarsPlan::new(
            [600.0, 300.0],
            900.0,
            AxisPlan::new([0.0, 2000.0]).every(500.0),
        )
        .series(BarSeriesPlan::new("before", "before", Tone::Muted))
        .series(BarSeriesPlan::new("after", "after", Tone::Accent))
        .row("cold", "cold start")
        .row("warm", "warm start")
        .row("build", "build")
        .delta(BarDeltaPlan::new("before", "after"))
    }

    #[test]
    fn bars_round_trip_with_compact_defaults_and_lay_out_rows() {
        let plan = plan();
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert!(json.get("rowHeight").is_none() && json.get("readout").is_none());
        assert!(json["delta"].get("better").is_none());
        assert_eq!(serde_json::from_value::<BarsPlan>(json).unwrap(), plan);
        assert_eq!(plan.bar_end(1000.0), 1050.0);
        assert_eq!(plan.bar_end(9000.0), 600.0 + 900.0 * 1.04);
        // Two bars sit symmetrically about the row center.
        assert_eq!(plan.bar_offset(0), -plan.bar_offset(1));
        assert!(plan.row_center(1.0) - plan.row_center(0.0) == plan.row_height);
    }

    #[test]
    fn deltas_print_improvements_and_regressions() {
        let delta = BarDeltaPlan::new("before", "after");
        assert_eq!(
            delta.text(1840.0, 1214.0),
            Some(("−34%".to_owned(), Tone::Success))
        );
        assert_eq!(
            delta.text(100.0, 112.0),
            Some(("+12%".to_owned(), Tone::Error))
        );
        assert_eq!(delta.text(100.0, 100.2).unwrap().0, "±0%");
        assert_eq!(delta.text(0.0, 5.0), None);
        let factor = BarDeltaPlan::new("before", "after").factor();
        assert_eq!(factor.text(240.0, 100.0).unwrap().0, "2.4×");
        let throughput = BarDeltaPlan::new("before", "after")
            .higher_is_better()
            .factor();
        assert_eq!(
            throughput.text(100.0, 80.0),
            Some(("0.8×".to_owned(), Tone::Error))
        );
    }

    #[test]
    fn rankings_are_stable_and_keep_identity() {
        assert_eq!(
            ranking(&[3.0, 1.0, 2.0], SortOrder::Ascending),
            vec![1, 2, 0]
        );
        assert_eq!(
            ranking(&[3.0, 1.0, 2.0], SortOrder::Descending),
            vec![0, 2, 1]
        );
        // Ties keep declaration order in both directions.
        assert_eq!(
            ranking(&[1.0, 1.0, 0.0], SortOrder::Ascending),
            vec![2, 0, 1]
        );
        assert_eq!(
            ranking(&[1.0, 1.0, 0.0], SortOrder::Descending),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn rising_rows_paint_over_the_rows_they_overtake() {
        assert_eq!(paint_order(&[0.0, 0.0, 0.0]), vec![0, 1, 2]);
        // Row 2 rises past row 1, which sinks; row 0 rests.
        assert_eq!(paint_order(&[0.0, 1.5, -1.5]), vec![0, 1, 2]);
        assert_eq!(paint_order(&[-1.5, 1.5, 0.01]), vec![2, 1, 0]);
    }

    #[test]
    fn invalid_bars_and_channels_are_rejected() {
        let mut dotted = plan();
        dotted.rows[0].id = "cold.start".into();
        assert!(dotted.validate().is_err());
        let mut same = plan();
        same.delta = Some(BarDeltaPlan::new("after", "after"));
        assert!(same.validate().is_err());
        let plan = plan();
        for property in ["axes", "row.cold.slot", "bar.warm.after", "delta.build"] {
            assert!(plan.accepts(property), "{property}");
        }
        for property in ["row.cold.y", "bar.warm.during", "bar.cold", "delta.nope"] {
            assert!(!plan.accepts(property), "{property}");
        }
    }

    #[test]
    fn sorting_springs_only_rows_that_move_and_grows_follow_display_order() {
        let mut scene = PlanBuilder::new("bars", 10_000_000_000);
        let mut bars = BarsActor::declare(&mut scene, "bench", &plan()).unwrap();
        bars.grow(
            &mut scene,
            1_000_000_000,
            "after",
            &[("cold", 1200.0), ("warm", 300.0), ("build", 900.0)],
        )
        .unwrap();
        let order = bars
            .sort(&mut scene, 3_000_000_000, "after", SortOrder::Ascending)
            .unwrap();
        assert_eq!(order, vec!["warm", "build", "cold"]);
        // Sorting again by the same values moves nothing.
        bars.sort(&mut scene, 4_000_000_000, "after", SortOrder::Ascending)
            .unwrap();
        // Later grows stagger down the new order: warm first.
        bars.grow(
            &mut scene,
            5_000_000_000,
            "before",
            &[("cold", 1840.0), ("warm", 410.0), ("build", 1000.0)],
        )
        .unwrap();
        let plan = scene.finish().unwrap();
        let channel = |id: &str| {
            plan.continuous_channels
                .iter()
                .find(|channel| channel.id == id)
                .unwrap()
        };
        assert_eq!(channel("bench.row.cold.slot").events.len(), 1);
        assert_eq!(channel("bench.row.warm.slot").events.len(), 1);
        assert_eq!(
            channel("bench.bar.warm.before").events[0].at_nanos(),
            5_000_000_000
        );
        assert_eq!(
            channel("bench.bar.cold.before").events[0].at_nanos(),
            5_000_000_000 + 2 * ROW_STAGGER_NANOS
        );
    }
}
