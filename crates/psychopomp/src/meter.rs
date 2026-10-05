//! Meters: a circular gauge, a countdown ring, or a linear progress bar. One
//! `value` channel drives the arc, the lit ticks, the tone, and a centered
//! Readout, so the number always agrees with the ring. A value springs to a
//! new reading (`set`) or sweeps on a clock (`countdown`); thresholds change
//! its tone, and a countdown flashes as it crosses them.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder, whole_millis},
    axis::AxisPlan,
    math::{easing::Ease, smoothstep},
    readout::{ReadoutFormat, Rounding},
    tone::Tone,
};

pub const METER_RECIPE: &str = "meter";

/// A tone change spreads over this share of the range just below its
/// threshold, so the color blends rather than cuts and is whole at the
/// threshold itself.
const THRESHOLD_BAND: f32 = 0.012;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeterKind {
    #[default]
    Ring,
    Bar,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeterPlan {
    pub kind: MeterKind,
    /// The ring's center, or the bar's center.
    pub center: [f32; 2],
    /// The ring's radius or the bar's length, in pixels.
    pub size: f32,
    /// Range, tick values, and the unit printed on tick labels.
    pub scale: AxisPlan,
    /// Degrees of arc the range covers, centered on twelve o'clock; 360
    /// closes the ring and starts it at twelve.
    #[serde(default = "default_sweep", skip_serializing_if = "is_default_sweep")]
    pub sweep: f32,
    /// Stroke thickness; defaults to 9% of a ring's radius or 16 px for a bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f32>,
    /// The number shown with the value; none hides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readout: Option<ReadoutFormat>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// The value's color below the first threshold.
    #[serde(default = "accent", skip_serializing_if = "is_accent")]
    pub tone: Tone,
    /// Tones taking over at and above each value, in increasing order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thresholds: Vec<MeterThresholdPlan>,
    /// Print the scale's tick values beside their ticks.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tick_labels: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeterThresholdPlan {
    pub at: f32,
    pub tone: Tone,
}

fn default_sweep() -> f32 {
    270.0
}

fn is_default_sweep(sweep: &f32) -> bool {
    *sweep == default_sweep()
}

fn accent() -> Tone {
    Tone::Accent
}

fn is_accent(tone: &Tone) -> bool {
    *tone == Tone::Accent
}

/// The value's color: `from` blending into `to` by `weight`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneBlend {
    pub from: Tone,
    pub to: Tone,
    pub weight: f32,
}

impl MeterPlan {
    /// A gauge: a 270° ring with its opening at the bottom.
    pub fn ring(center: [f32; 2], radius: f32, scale: AxisPlan) -> Self {
        Self {
            kind: MeterKind::Ring,
            center,
            size: radius,
            scale,
            sweep: default_sweep(),
            thickness: None,
            readout: Some(ReadoutFormat::default()),
            label: String::new(),
            tone: Tone::Accent,
            thresholds: Vec::new(),
            tick_labels: false,
        }
    }

    /// A horizontal progress bar of `length` centered on `center`.
    pub fn bar(center: [f32; 2], length: f32, scale: AxisPlan) -> Self {
        Self {
            kind: MeterKind::Bar,
            ..Self::ring(center, length, scale)
        }
    }

    /// A closed ring counting `seconds` down: a tick per second (up to 60),
    /// whole seconds rounded up, warning under a third and error under a
    /// tenth of the time.
    pub fn countdown(center: [f32; 2], radius: f32, seconds: f32) -> Self {
        let scale = AxisPlan::new([0.0, seconds]);
        let scale = if seconds <= 60.0 {
            scale.every(1.0)
        } else {
            scale.nice(13)
        };
        Self {
            sweep: 360.0,
            readout: Some(ReadoutFormat::new(0).rounding(Rounding::Up).unit("s")),
            tone: Tone::Error,
            thresholds: vec![
                MeterThresholdPlan {
                    at: seconds * 0.1,
                    tone: Tone::Warning,
                },
                MeterThresholdPlan {
                    at: seconds / 3.0,
                    tone: Tone::Accent,
                },
            ],
            ..Self::ring(center, radius, scale)
        }
    }

    pub fn sweep(mut self, degrees: f32) -> Self {
        self.sweep = degrees;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness);
        self
    }

    pub fn readout(mut self, format: ReadoutFormat) -> Self {
        self.readout = Some(format);
        self
    }

    pub fn no_readout(mut self) -> Self {
        self.readout = None;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// `tone` takes over at and above `at`.
    pub fn threshold(mut self, at: f32, tone: Tone) -> Self {
        self.thresholds.push(MeterThresholdPlan { at, tone });
        self
    }

    pub fn tick_labels(mut self) -> Self {
        self.tick_labels = true;
        self
    }

    pub fn stroke(&self) -> f32 {
        self.thickness.unwrap_or(match self.kind {
            MeterKind::Ring => (self.size * 0.09).max(4.0),
            MeterKind::Bar => 16.0,
        })
    }

    /// Where `value` sits along the meter, held at its ends.
    pub fn fraction(&self, value: f32) -> f32 {
        self.scale.fraction(value).clamp(0.0, 1.0)
    }

    /// The ring angle at `fraction`, in radians clockwise from twelve o'clock.
    pub fn angle(&self, fraction: f32) -> f32 {
        let start = if self.sweep >= 360.0 {
            0.0
        } else {
            -self.sweep * 0.5
        };
        (start + self.sweep * fraction).to_radians()
    }

    /// The canvas point on the ring at `fraction` and `radius`, before offsets.
    pub fn ring_point(&self, fraction: f32, radius: f32) -> [f32; 2] {
        let angle = self.angle(fraction);
        [
            self.center[0] + radius * angle.sin(),
            self.center[1] - radius * angle.cos(),
        ]
    }

    /// The value's tone, blending across a narrow band at each threshold.
    pub fn tone_at(&self, value: f32) -> ToneBlend {
        let band = (self.scale.range[1] - self.scale.range[0]) * THRESHOLD_BAND;
        let mut below = self.tone;
        for threshold in &self.thresholds {
            let weight = smoothstep((value - threshold.at) / band + 1.0);
            if weight < 1.0 {
                return ToneBlend {
                    from: below,
                    to: threshold.tone,
                    weight,
                };
            }
            below = threshold.tone;
        }
        ToneBlend {
            from: below,
            to: below,
            weight: 0.0,
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.center.iter().all(|v| v.is_finite()),
            "meter center must be finite"
        );
        self.scale.validate("meter scale")?;
        match self.kind {
            MeterKind::Ring => ensure!(
                (24.0..=600.0).contains(&self.size),
                "meter ring radius must be between 24 and 600"
            ),
            MeterKind::Bar => ensure!(
                (80.0..=1900.0).contains(&self.size),
                "meter bar length must be between 80 and 1900"
            ),
        }
        ensure!(
            (30.0..=360.0).contains(&self.sweep),
            "meter sweep must be between 30 and 360 degrees"
        );
        ensure!(
            self.thickness
                .is_none_or(|t| t.is_finite() && (1.0..=120.0).contains(&t)),
            "meter thickness must be between 1 and 120"
        );
        if let Some(readout) = &self.readout {
            readout.validate()?;
        }
        ensure!(
            self.label.chars().count() <= 40 && !self.label.contains('\n'),
            "meter label is one line of at most 40 characters"
        );
        ensure!(
            self.thresholds.len() <= 6,
            "a meter has at most 6 thresholds"
        );
        ensure!(
            self.thresholds
                .iter()
                .all(|t| t.at.is_finite() && self.scale.contains(t.at))
                && self
                    .thresholds
                    .windows(2)
                    .all(|pair| pair[0].at < pair[1].at),
            "meter thresholds must lie inside the scale in increasing order"
        );
        Ok(())
    }

    /// Whether `property` names a meter channel, as preflight checks.
    pub fn accepts(property: &str) -> bool {
        matches!(
            property,
            "opacity" | "x" | "y" | "value" | "reveal" | "flash"
        )
    }
}

/// Authoring handle for one meter actor.
#[derive(Clone, Debug)]
pub struct MeterActor {
    actor: ActorHandle,
    plan: MeterPlan,
    value: ContinuousHandle,
}

impl MeterActor {
    /// Declare the meter showing `initial` until its first write.
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &MeterPlan,
        initial: f32,
    ) -> Result<Self> {
        plan.validate()?;
        ensure!(initial.is_finite(), "meter initial value must be finite");
        let actor = scene.actor(id, METER_RECIPE, plan)?;
        let value = scene.channel(&actor, "value", initial);
        Ok(Self {
            actor,
            plan: plan.clone(),
            value,
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

    /// Fade and rise in while the track and ticks draw on.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::show(scene, &self.actor, at_nanos);
        let reveal = self.channel(scene, "reveal", 0.0);
        scene.ease(&reveal, at_nanos, 1.0, 0.9, crate::plot::DRAW_EASE);
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::hide(scene, &self.actor, at_nanos);
    }

    /// Spring to `value`, keeping the current velocity.
    pub fn set(&mut self, scene: &mut PlanBuilder, at_nanos: u64, value: f32) {
        scene.spring(&self.value, at_nanos, value, 0.7, 0.12);
    }

    /// Sweep linearly to `value` over `seconds`, as a timer runs; returns
    /// when it arrives.
    pub fn sweep(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        value: f32,
        seconds: f32,
    ) -> u64 {
        scene.ease(&self.value, at_nanos, value, seconds, Ease::Linear);
        at_nanos + whole_millis(seconds)
    }

    /// Count `seconds` down to zero from `at_nanos`: the ring sweeps closed,
    /// and the meter flashes as it crosses each threshold and as it runs
    /// out. Returns when it reaches zero.
    pub fn countdown(&mut self, scene: &mut PlanBuilder, at_nanos: u64, seconds: f32) -> u64 {
        scene.set(&self.value, at_nanos, seconds);
        let end = self.sweep(scene, at_nanos, 0.0, seconds);
        let crossings = self
            .plan
            .thresholds
            .iter()
            .rev()
            .filter(|threshold| threshold.at > 0.0 && threshold.at < seconds)
            .map(|threshold| at_nanos + whole_millis(seconds - threshold.at))
            .collect::<Vec<_>>();
        for crossing in crossings {
            self.flash(scene, crossing, 0.6);
        }
        self.flash(scene, end, 1.0);
        end
    }

    /// Light the meter to `peak` at once, then let it decay.
    pub fn flash(&mut self, scene: &mut PlanBuilder, at_nanos: u64, peak: f32) {
        let flash = self.channel(scene, "flash", 0.0);
        scene.set(&flash, at_nanos, peak);
        scene.ease(&flash, at_nanos, 0.0, 0.9, Ease::CubicOut);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meters_round_trip_with_compact_defaults() {
        let plan = MeterPlan::ring(
            [960.0, 540.0],
            160.0,
            AxisPlan::new([0.0, 100.0]).every(25.0),
        )
        .label("cpu")
        .threshold(70.0, Tone::Warning)
        .threshold(90.0, Tone::Error);
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert!(json.get("sweep").is_none() && json.get("tone").is_none());
        assert_eq!(serde_json::from_value::<MeterPlan>(json).unwrap(), plan);
    }

    #[test]
    fn rings_map_fractions_to_angles_around_twelve() {
        let gauge = MeterPlan::ring([0.0, 0.0], 100.0, AxisPlan::new([0.0, 1.0]));
        assert!((gauge.angle(0.5)).abs() < 1e-6, "the middle is at twelve");
        assert!((gauge.angle(0.0) + 135_f32.to_radians()).abs() < 1e-6);
        let top = gauge.ring_point(0.5, 100.0);
        assert!(top[0].abs() < 1e-3 && (top[1] + 100.0).abs() < 1e-3);
        let countdown = MeterPlan::countdown([0.0, 0.0], 100.0, 10.0);
        assert_eq!(countdown.angle(0.0), 0.0);
        let quarter = countdown.ring_point(0.25, 100.0);
        assert!((quarter[0] - 100.0).abs() < 1e-3, "clockwise: {quarter:?}");
        assert_eq!(countdown.scale.ticks.len(), 11);
        assert_eq!(countdown.fraction(12.0), 1.0);
    }

    #[test]
    fn thresholds_change_tone_with_a_narrow_blend() {
        let plan = MeterPlan::ring([0.0, 0.0], 100.0, AxisPlan::new([0.0, 100.0]))
            .threshold(70.0, Tone::Warning)
            .threshold(90.0, Tone::Error);
        let solid = |value: f32| {
            let blend = plan.tone_at(value);
            if blend.weight == 0.0 {
                Some(blend.from)
            } else if blend.weight == 1.0 {
                Some(blend.to)
            } else {
                None
            }
        };
        assert_eq!(solid(10.0), Some(Tone::Accent));
        assert_eq!(solid(80.0), Some(Tone::Warning));
        assert_eq!(solid(95.0), Some(Tone::Error));
        assert_eq!(solid(70.0), Some(Tone::Warning), "whole at the threshold");
        let crossing = plan.tone_at(69.4);
        assert_eq!((crossing.from, crossing.to), (Tone::Accent, Tone::Warning));
        assert!((crossing.weight - 0.5).abs() < 1e-3);
        let countdown = MeterPlan::countdown([0.0, 0.0], 100.0, 10.0);
        assert_eq!(countdown.tone_at(0.2).from, Tone::Error);
        assert_eq!(countdown.tone_at(2.0).from, Tone::Warning);
        assert_eq!(countdown.tone_at(9.0).from, Tone::Accent);
    }

    #[test]
    fn invalid_meters_are_rejected() {
        let base = MeterPlan::ring([0.0, 0.0], 100.0, AxisPlan::new([0.0, 1.0]));
        assert!(base.clone().sweep(10.0).validate().is_err());
        assert!(
            base.clone()
                .threshold(0.8, Tone::Error)
                .threshold(0.4, Tone::Warning)
                .validate()
                .is_err()
        );
        assert!(base.clone().threshold(3.0, Tone::Error).validate().is_err());
        assert!(MeterPlan::accepts("flash") && !MeterPlan::accepts("progress"));
    }

    #[test]
    fn countdowns_sweep_to_zero_and_flash_at_each_crossing() {
        let mut scene = PlanBuilder::new("meter", 20_000_000_000);
        let plan = MeterPlan::countdown([960.0, 540.0], 160.0, 10.0);
        let mut timer = MeterActor::declare(&mut scene, "timer", &plan, 10.0).unwrap();
        timer.show(&mut scene, 0);
        let end = timer.countdown(&mut scene, 1_000_000_000, 10.0);
        assert_eq!(end, 11_000_000_000);
        let plan = scene.finish().unwrap();
        let flash = plan
            .continuous_channels
            .iter()
            .find(|channel| channel.property == "flash")
            .unwrap();
        let times = flash
            .events
            .iter()
            .map(|event| event.at_nanos())
            .collect::<Vec<_>>();
        // Accent → warning at 3.33 s left, warning → error at 1 s, then zero.
        assert_eq!(times.len(), 6);
        assert_eq!(times[0], 1_000_000_000 + whole_millis(10.0 - 10.0 / 3.0));
        assert_eq!(times[2], 10_000_000_000);
        assert_eq!(times[4], end);
    }
}
