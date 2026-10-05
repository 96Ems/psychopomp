//! Plots: function curves over two axes for explaining springs, easing,
//! interruption, and metrics. The Scene Program computes every curve as
//! sampled points (with exact slopes when it knows them); the renderer only
//! draws them on, rides a playhead dot along them, and shows its tangent.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    axis::AxisPlan,
    math::{easing::Ease, inverse_lerp, lerp},
    motion::MotionState,
    tone::Tone,
};

pub const PLOT_RECIPE: &str = "plot";

/// Draw-on: neither jumps off the start nor parks at the end.
pub const DRAW_EASE: Ease = Ease::CubicBezier([0.45, 0.0, 0.2, 1.0]);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlotPlan {
    /// Top-left corner of the data frame, in canvas pixels.
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub x: AxisPlan,
    pub y: AxisPlan,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<PlotSeriesPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<PlotMarkPlan>,
}

/// One curve: points with non-decreasing x (a repeated x draws a vertical
/// step). `slopes`, when present, are exact dy/dx values at each point.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlotSeriesPlan {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "Tone::is_default")]
    pub tone: Tone,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dashed: bool,
    pub points: Vec<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slopes: Vec<f32>,
}

/// A labelled vertical rule at one x value, such as the moment of a retarget.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlotMarkPlan {
    pub id: String,
    pub x: f32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

impl PlotSeriesPlan {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        tone: Tone,
        points: Vec<[f32; 2]>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tone,
            dashed: false,
            points,
            slopes: Vec::new(),
        }
    }

    /// `f` sampled at `samples` evenly spaced x values across `range`.
    pub fn sampled(
        id: impl Into<String>,
        label: impl Into<String>,
        tone: Tone,
        range: [f32; 2],
        samples: usize,
        f: impl Fn(f32) -> f32,
    ) -> Self {
        let points = sample_xs(range, samples).map(|x| [x, f(x)]).collect();
        Self::new(id, label, tone, points)
    }

    /// A motion sampled over time: position is y and velocity the exact slope.
    pub fn motion(
        id: impl Into<String>,
        label: impl Into<String>,
        tone: Tone,
        range: [f32; 2],
        samples: usize,
        f: impl Fn(f32) -> MotionState,
    ) -> Self {
        let states = sample_xs(range, samples)
            .map(|x| (x, f(x)))
            .collect::<Vec<_>>();
        Self {
            slopes: states.iter().map(|(_, state)| state.velocity).collect(),
            ..Self::new(
                id,
                label,
                tone,
                states
                    .iter()
                    .map(|(x, state)| [*x, state.position])
                    .collect(),
            )
        }
    }

    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }

    /// The span of x this curve covers.
    pub fn x_range(&self) -> [f32; 2] {
        [
            self.points.first().map_or(0.0, |p| p[0]),
            self.points.last().map_or(0.0, |p| p[0]),
        ]
    }

    /// The segment containing `x` (the later one at a repeated x) and the
    /// fraction of the way along it, held at the ends.
    fn segment(&self, x: f32) -> (usize, f32) {
        let last = self.points.len().saturating_sub(1).max(1);
        let index = self
            .points
            .partition_point(|point| point[0] <= x)
            .clamp(1, last);
        let (a, b) = (self.points[index - 1], self.points[index]);
        (index, inverse_lerp(a[0], b[0], x).clamp(0.0, 1.0))
    }

    /// The curve's y at `x`, interpolated linearly and held at its ends.
    pub fn y_at(&self, x: f32) -> f32 {
        match self.points.len() {
            0 => 0.0,
            1 => self.points[0][1],
            _ => {
                let (index, t) = self.segment(x);
                lerp(self.points[index - 1][1], self.points[index][1], t)
            }
        }
    }

    /// dy/dx at `x`: exact slopes when given, otherwise central differences at
    /// the points. Either way it is interpolated between points, so a dot
    /// riding the curve turns its tangent smoothly instead of segment by segment.
    pub fn slope_at(&self, x: f32) -> f32 {
        if self.points.len() < 2 {
            return 0.0;
        }
        let (index, t) = self.segment(x);
        lerp(self.slope(index - 1), self.slope(index), t)
    }

    fn slope(&self, index: usize) -> f32 {
        if let Some(&slope) = self.slopes.get(index) {
            return slope;
        }
        let a = self.points[index.saturating_sub(1)];
        let b = self.points[(index + 1).min(self.points.len() - 1)];
        if b[0] > a[0] {
            (b[1] - a[1]) / (b[0] - a[0])
        } else {
            0.0
        }
    }
}

fn sample_xs([start, end]: [f32; 2], samples: usize) -> impl Iterator<Item = f32> {
    let samples = samples.max(2);
    (0..samples).map(move |index| lerp(start, end, index as f32 / (samples - 1) as f32))
}

impl PlotPlan {
    pub fn new(origin: [f32; 2], size: [f32; 2], x: AxisPlan, y: AxisPlan) -> Self {
        Self {
            origin,
            size,
            x,
            y,
            series: Vec::new(),
            marks: Vec::new(),
        }
    }

    pub fn series(mut self, series: PlotSeriesPlan) -> Self {
        self.series.push(series);
        self
    }

    pub fn mark(mut self, id: impl Into<String>, x: f32, label: impl Into<String>) -> Self {
        self.marks.push(PlotMarkPlan {
            id: id.into(),
            x,
            label: label.into(),
        });
        self
    }

    pub fn find_series(&self, id: &str) -> Option<&PlotSeriesPlan> {
        self.series.iter().find(|series| series.id == id)
    }

    /// Canvas position of a data point, before the actor's `x`/`y` offsets.
    pub fn to_canvas(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + self.x.fraction(x) * self.size[0],
            self.origin[1] + (1.0 - self.y.fraction(y)) * self.size[1],
        ]
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite())
                && self.size.iter().all(|v| v.is_finite() && *v >= 40.0),
            "plot frame must be finite and at least 40 px on each side"
        );
        self.x.validate("x")?;
        self.y.validate("y")?;
        ensure!(self.series.len() <= 12, "a plot has at most 12 series");
        let mut ids = HashSet::new();
        let mut total = 0;
        for series in &self.series {
            valid_id(&series.id, "series")?;
            ensure!(
                ids.insert(&series.id),
                "duplicate series id '{}'",
                series.id
            );
            ensure!(
                (2..=4096).contains(&series.points.len()),
                "series '{}' needs 2 to 4096 points",
                series.id
            );
            total += series.points.len();
            ensure!(
                series.points.iter().flatten().all(|v| v.is_finite()),
                "series '{}' points must be finite",
                series.id
            );
            ensure!(
                series
                    .points
                    .windows(2)
                    .all(|pair| pair[0][0] <= pair[1][0]),
                "series '{}' x values must not decrease",
                series.id
            );
            ensure!(
                series.slopes.is_empty()
                    || (series.slopes.len() == series.points.len()
                        && series.slopes.iter().all(|v| v.is_finite())),
                "series '{}' slopes must be finite, one per point",
                series.id
            );
            ensure!(
                series.label.chars().count() <= 40 && !series.label.contains('\n'),
                "series '{}' label is one line of at most 40 characters",
                series.id
            );
        }
        ensure!(total <= 16384, "a plot has at most 16384 points");
        let mut marks = HashSet::new();
        for mark in &self.marks {
            valid_id(&mark.id, "mark")?;
            ensure!(marks.insert(&mark.id), "duplicate mark id '{}'", mark.id);
            ensure!(
                mark.x.is_finite() && self.x.contains(mark.x),
                "mark '{}' must lie inside the x range",
                mark.id
            );
        }
        ensure!(self.marks.len() <= 8, "a plot has at most 8 marks");
        Ok(())
    }

    /// Whether `property` names a plot channel, as preflight checks.
    pub fn accepts(&self, property: &str) -> bool {
        if matches!(
            property,
            "opacity" | "x" | "y" | "axes" | "playhead" | "playhead.opacity"
        ) {
            return true;
        }
        let nested = |prefix: &str, properties: &[&str]| {
            property.strip_prefix(prefix).and_then(|rest| {
                properties
                    .iter()
                    .find_map(|name| rest.strip_suffix(&format!(".{name}")))
            })
        };
        if let Some(id) = nested("series.", &["draw", "opacity", "ride", "velocity"]) {
            return self.find_series(id).is_some();
        }
        nested("mark.", &["opacity"]).is_some_and(|id| self.marks.iter().any(|m| m.id == id))
    }
}

pub(crate) fn valid_id(id: &str, kind: &str) -> Result<()> {
    ensure!(
        !id.is_empty() && !id.chars().any(char::is_whitespace),
        "{kind} id '{id}' must be non-empty without whitespace"
    );
    Ok(())
}

/// Authoring handle for one plot actor. Channels are declared on first use.
#[derive(Clone, Debug)]
pub struct PlotActor {
    actor: ActorHandle,
}

impl PlotActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &PlotPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, PLOT_RECIPE, plan)?;
        Ok(Self { actor })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    /// The channel for `property`, declared on first use with `initial`.
    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// `series.<series>.<property>`: `draw`, `opacity`, `ride`, or `velocity`.
    pub fn series_channel(
        &mut self,
        scene: &mut PlanBuilder,
        series: &str,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        self.channel(scene, &format!("series.{series}.{property}"), initial)
    }

    /// Fade and rise in, then draw the axes over `axes_seconds`.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64, axes_seconds: f32) {
        crate::caption::show(scene, &self.actor, at_nanos);
        let axes = self.channel(scene, "axes", 0.0);
        scene.ease(&axes, at_nanos, 1.0, axes_seconds, DRAW_EASE);
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::hide(scene, &self.actor, at_nanos);
    }

    /// Draw `series` on along its length over `seconds`; returns when it ends.
    pub fn draw(
        &mut self,
        scene: &mut PlanBuilder,
        series: &str,
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let draw = self.series_channel(scene, series, "draw", 0.0);
        scene.ease(&draw, at_nanos, 1.0, seconds, DRAW_EASE);
        at_nanos + crate::author::whole_millis(seconds)
    }

    /// Spring a series' opacity, to dim it behind the one being explained.
    pub fn fade(&mut self, scene: &mut PlanBuilder, series: &str, at_nanos: u64, opacity: f32) {
        let channel = self.series_channel(scene, series, "opacity", 1.0);
        scene.spring(&channel, at_nanos, opacity, 0.35, 0.0);
    }

    /// Move the playhead from `from_x` to `to_x` at constant speed, with a dot
    /// riding `series`. The playhead and dot fade in as it starts. Returns
    /// when it arrives.
    pub fn ride(
        &mut self,
        scene: &mut PlanBuilder,
        series: &str,
        [from_x, to_x]: [f32; 2],
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let playhead = self.channel(scene, "playhead", from_x);
        let opacity = self.channel(scene, "playhead.opacity", 0.0);
        let ride = self.series_channel(scene, series, "ride", 0.0);
        scene.set(&playhead, at_nanos, from_x);
        scene.ease(&playhead, at_nanos, to_x, seconds, Ease::Linear);
        scene.spring(&opacity, at_nanos, 1.0, 0.3, 0.0);
        scene.spring(&ride, at_nanos, 1.0, 0.3, 0.0);
        at_nanos + crate::author::whole_millis(seconds)
    }

    /// Fade the playhead and every riding dot out.
    pub fn stop_ride(&mut self, scene: &mut PlanBuilder, series: &str, at_nanos: u64) {
        let opacity = self.channel(scene, "playhead.opacity", 0.0);
        let ride = self.series_channel(scene, series, "ride", 0.0);
        scene.spring(&opacity, at_nanos, 0.0, 0.3, 0.0);
        scene.spring(&ride, at_nanos, 0.0, 0.3, 0.0);
    }

    /// Show (1) or hide (0) the tangent arrow on `series`' riding dot.
    pub fn velocity(&mut self, scene: &mut PlanBuilder, series: &str, at_nanos: u64, shown: f32) {
        let channel = self.series_channel(scene, series, "velocity", 0.0);
        scene.spring(&channel, at_nanos, shown, 0.3, 0.0);
    }

    /// Fade a mark in.
    pub fn mark(&mut self, scene: &mut PlanBuilder, mark: &str, at_nanos: u64) {
        let channel = self.channel(scene, &format!("mark.{mark}.opacity"), 0.0);
        scene.spring(&channel, at_nanos, 1.0, 0.35, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> PlotPlan {
        PlotPlan::new(
            [200.0, 200.0],
            [1000.0, 500.0],
            AxisPlan::new([0.0, 1.0]).every(0.25),
            AxisPlan::new([0.0, 2.0]).every(1.0),
        )
        .series(PlotSeriesPlan::sampled(
            "square",
            "x²",
            Tone::Accent,
            [0.0, 1.0],
            101,
            |x| x * x,
        ))
        .mark("half", 0.5, "half")
    }

    #[test]
    fn plots_round_trip_with_compact_defaults() {
        let plan = plan();
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert!(json["series"][0].get("dashed").is_none());
        assert!(json["series"][0].get("slopes").is_none());
        assert_eq!(serde_json::from_value::<PlotPlan>(json).unwrap(), plan);
        assert_eq!(plan.to_canvas([0.5, 1.0]), [700.0, 450.0]);
    }

    #[test]
    fn sampled_series_interpolate_values_and_smooth_slopes() {
        let plan = plan();
        let square = plan.find_series("square").unwrap();
        assert!((square.y_at(0.5) - 0.25).abs() < 1e-4);
        assert_eq!(square.y_at(-1.0), 0.0);
        assert_eq!(square.y_at(2.0), 1.0);
        // Central differences of x² are exact at interior points.
        assert!((square.slope_at(0.5) - 1.0).abs() < 1e-3);
        // Interpolated slopes vary continuously between points.
        let a = square.slope_at(0.501);
        let b = square.slope_at(0.509);
        assert!(b > a && b - a < 0.02);
    }

    #[test]
    fn motion_series_carry_exact_velocities() {
        let series = PlotSeriesPlan::motion("v", "", Tone::Plain, [0.0, 1.0], 3, |t| MotionState {
            position: t,
            velocity: 7.0,
        });
        assert_eq!(series.slopes, vec![7.0; 3]);
        assert_eq!(series.slope_at(0.3), 7.0);
    }

    #[test]
    fn steps_take_the_later_value_at_a_repeated_x() {
        let step = PlotSeriesPlan::new(
            "target",
            "",
            Tone::Muted,
            vec![[0.0, 1.0], [0.3, 1.0], [0.3, 0.4], [1.0, 0.4]],
        );
        assert_eq!(step.y_at(0.3), 0.4);
        assert_eq!(step.y_at(0.29), 1.0);
    }

    #[test]
    fn invalid_plots_and_channels_are_rejected() {
        let mut backwards = plan();
        backwards.series[0].points.swap(0, 5);
        assert!(backwards.validate().is_err());
        let mut slopes = plan();
        slopes.series[0].slopes = vec![1.0];
        assert!(slopes.validate().is_err());
        let mut mark = plan();
        mark.marks[0].x = 4.0;
        assert!(mark.validate().is_err());
        let plan = plan();
        for property in [
            "axes",
            "playhead",
            "series.square.draw",
            "mark.half.opacity",
        ] {
            assert!(plan.accepts(property), "{property}");
        }
        for property in [
            "series.square.drawn",
            "series.cube.draw",
            "mark.full.opacity",
            "scale",
        ] {
            assert!(!plan.accepts(property), "{property}");
        }
    }

    #[test]
    fn the_handle_writes_draw_and_ride_channels() {
        let mut scene = PlanBuilder::new("plot-demo", 10_000_000_000);
        let mut plot = PlotActor::declare(&mut scene, "plot", &plan()).unwrap();
        plot.show(&mut scene, 0, 0.8);
        let drawn = plot.draw(&mut scene, "square", 1_000_000_000, 1.2);
        assert_eq!(drawn, 2_200_000_000);
        let arrived = plot.ride(&mut scene, "square", [0.0, 1.0], drawn, 2.0);
        assert_eq!(arrived, 4_200_000_000);
        plot.velocity(&mut scene, "square", drawn, 1.0);
        let plan = scene.finish().unwrap();
        let properties = plan
            .continuous_channels
            .iter()
            .map(|channel| channel.property.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            properties,
            vec![
                "opacity",
                "y",
                "axes",
                "series.square.draw",
                "playhead",
                "playhead.opacity",
                "series.square.ride",
                "series.square.velocity",
            ]
        );
    }
}
