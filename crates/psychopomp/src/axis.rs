//! Axes shared by Plots and Lanes: a numeric range, the values that carry tick
//! marks, an optional label, and a unit suffix. Scene Programs choose the ticks
//! (by hand, `every`, or `nice`); renderers only draw them.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::math::inverse_lerp;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AxisPlan {
    /// The values at the start and end of the axis, increasing.
    pub range: [f32; 2],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ticks: Vec<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Appended to every tick label, as `s` for seconds.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
}

impl AxisPlan {
    pub fn new(range: [f32; 2]) -> Self {
        Self {
            range,
            ticks: Vec::new(),
            label: String::new(),
            unit: String::new(),
        }
    }

    pub fn ticks(mut self, ticks: impl IntoIterator<Item = f32>) -> Self {
        self.ticks = ticks.into_iter().collect();
        self
    }

    /// Ticks at every multiple of `step` inside the range.
    pub fn every(mut self, step: f32) -> Self {
        self.ticks = multiples(self.range, step);
        self
    }

    /// At most `count` ticks on a 1, 2, 2.5, or 5 × 10ⁿ step.
    pub fn nice(mut self, count: usize) -> Self {
        self.ticks = nice_ticks(self.range, count);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Where `value` sits along the axis, 0 at the start and 1 at the end.
    pub fn fraction(&self, value: f32) -> f32 {
        inverse_lerp(self.range[0], self.range[1], value)
    }

    pub fn contains(&self, value: f32) -> bool {
        let slack = (self.range[1] - self.range[0]) * 1e-4;
        value >= self.range[0] - slack && value <= self.range[1] + slack
    }

    /// The fewest decimals (at most three) that print every tick exactly, so
    /// one axis never mixes `0.5` with `1`.
    pub fn decimals(&self) -> usize {
        (0..3)
            .find(|&decimals| {
                let scale = 10_f32.powi(decimals as i32);
                self.ticks
                    .iter()
                    .all(|tick| ((tick * scale).round() - tick * scale).abs() < 1e-3)
            })
            .unwrap_or(3)
    }

    /// `value` printed at the axis' decimals with its unit; never `-0`.
    pub fn tick_label(&self, value: f32) -> String {
        let decimals = self.decimals();
        let text = format!("{value:.decimals$}");
        let text = match text.strip_prefix('-') {
            Some(digits) if digits.chars().all(|c| c == '0' || c == '.') => digits.to_owned(),
            _ => text,
        };
        format!("{text}{}", self.unit)
    }

    pub fn validate(&self, name: &str) -> Result<()> {
        ensure!(
            self.range.iter().all(|v| v.is_finite()) && self.range[0] < self.range[1],
            "{name} axis range must be finite and increasing"
        );
        ensure!(self.ticks.len() <= 40, "{name} axis has at most 40 ticks");
        ensure!(
            self.ticks.iter().all(|&tick| self.contains(tick)),
            "{name} axis ticks must lie inside its range"
        );
        ensure!(
            self.label.chars().count() <= 60 && !self.label.contains('\n'),
            "{name} axis label is one line of at most 60 characters"
        );
        ensure!(
            self.unit.chars().count() <= 8,
            "{name} axis unit is at most 8 characters"
        );
        Ok(())
    }
}

fn multiples([start, end]: [f32; 2], step: f32) -> Vec<f32> {
    if !(step.is_finite() && step > 0.0 && start < end) {
        return Vec::new();
    }
    let first = (start / step - 1e-4).ceil() as i64;
    let last = (end / step + 1e-4).floor() as i64;
    (first..=last.min(first + 1000))
        .map(|index| index as f32 * step)
        .collect()
}

/// At most `count` (at least two) ticks on a 1, 2, 2.5, or 5 × 10ⁿ step.
pub fn nice_ticks(range: [f32; 2], count: usize) -> Vec<f32> {
    let span = range[1] - range[0];
    if !(span.is_finite() && span > 0.0) {
        return Vec::new();
    }
    let count = count.max(2);
    let rough = span / (count - 1) as f32;
    let magnitude = 10_f32.powf(rough.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|multiple| multiple * magnitude)
        .map(|step| multiples(range, step))
        .find(|ticks| ticks.len() <= count)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ticks_pick_round_steps_inside_the_range() {
        assert_eq!(
            nice_ticks([0.0, 1.0], 6),
            vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        );
        assert_eq!(
            nice_ticks([0.0, 14.0], 8),
            vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0]
        );
        assert_eq!(nice_ticks([-0.2, 1.4], 5), vec![0.0, 0.5, 1.0]);
        assert!(nice_ticks([1.0, 1.0], 5).is_empty());
    }

    #[test]
    fn tick_labels_share_decimals_and_units() {
        let axis = AxisPlan::new([0.0, 1.0]).every(0.25).unit("s");
        assert_eq!(axis.ticks.len(), 5);
        assert_eq!(axis.decimals(), 2);
        assert_eq!(axis.tick_label(0.5), "0.50s");
        let whole = AxisPlan::new([-1.0, 3.0]).every(1.0);
        assert_eq!(whole.tick_label(-0.0001), "0");
        assert_eq!(whole.tick_label(-1.0), "-1");
    }

    #[test]
    fn invalid_axes_are_rejected() {
        assert!(AxisPlan::new([1.0, 0.0]).validate("x").is_err());
        assert!(
            AxisPlan::new([0.0, 1.0])
                .ticks([2.0])
                .validate("x")
                .is_err()
        );
        AxisPlan::new([0.0, 1.0]).every(0.1).validate("x").unwrap();
    }
}
