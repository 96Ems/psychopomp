//! Confetti: a success burst overlay. One `burst` clock (seconds since
//! launch; -1 before) drives every piece through `effects::confetti`, so the
//! burst is deterministic for its seed and samples in any order.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder, whole_millis},
    effects::confetti::{Burst, LIFETIME},
    tone::Tone,
};

pub const CONFETTI_RECIPE: &str = "confetti";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfettiPlan {
    /// Where the burst launches, in canvas pixels.
    pub origin: [f32; 2],
    #[serde(default = "default_count", skip_serializing_if = "is_default_count")]
    pub count: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub seed: u32,
    /// Launch speed of the fastest pieces, px/s.
    #[serde(default = "default_speed", skip_serializing_if = "is_default_speed")]
    pub speed: f32,
    /// Aim in degrees clockwise from straight up.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub angle: f32,
    /// Half-angle of the launch cone, in degrees.
    #[serde(default = "default_spread", skip_serializing_if = "is_default_spread")]
    pub spread: f32,
    #[serde(
        default = "default_gravity",
        skip_serializing_if = "is_default_gravity"
    )]
    pub gravity: f32,
    /// Piece colors by role; status tones stay fixed across themes.
    #[serde(default = "default_tones", skip_serializing_if = "is_default_tones")]
    pub tones: Vec<Tone>,
}

fn default_count() -> u32 {
    140
}
fn is_default_count(value: &u32) -> bool {
    *value == default_count()
}
fn is_zero(value: &u32) -> bool {
    *value == 0
}
fn is_zero_f32(value: &f32) -> bool {
    *value == 0.0
}
fn default_speed() -> f32 {
    1700.0
}
fn is_default_speed(value: &f32) -> bool {
    *value == default_speed()
}
fn default_spread() -> f32 {
    55.0
}
fn is_default_spread(value: &f32) -> bool {
    *value == default_spread()
}
fn default_gravity() -> f32 {
    1200.0
}
fn is_default_gravity(value: &f32) -> bool {
    *value == default_gravity()
}
fn default_tones() -> Vec<Tone> {
    vec![
        Tone::Accent,
        Tone::Success,
        Tone::Request,
        Tone::Warning,
        Tone::Plain,
    ]
}
fn is_default_tones(tones: &Vec<Tone>) -> bool {
    *tones == default_tones()
}

impl ConfettiPlan {
    pub fn new(origin: [f32; 2]) -> Self {
        Self {
            origin,
            count: default_count(),
            seed: 0,
            speed: default_speed(),
            angle: 0.0,
            spread: default_spread(),
            gravity: default_gravity(),
            tones: default_tones(),
        }
    }

    pub fn count(mut self, count: u32) -> Self {
        self.count = count;
        self
    }

    pub fn seed(mut self, seed: u32) -> Self {
        self.seed = seed;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Aim `angle` degrees clockwise from up, within a `spread` half-angle.
    pub fn aim(mut self, angle: f32, spread: f32) -> Self {
        self.angle = angle;
        self.spread = spread;
        self
    }

    pub fn tones(mut self, tones: Vec<Tone>) -> Self {
        self.tones = tones;
        self
    }

    pub fn burst(&self) -> Burst {
        Burst {
            count: self.count,
            seed: self.seed,
            speed: self.speed,
            angle: self.angle,
            spread: self.spread,
            gravity: self.gravity,
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "confetti origin must be finite"
        );
        ensure!(
            (1..=400).contains(&self.count),
            "confetti has 1 to 400 pieces"
        );
        ensure!(
            self.speed.is_finite() && (50.0..=4000.0).contains(&self.speed),
            "confetti speed must be between 50 and 4000 px/s"
        );
        ensure!(
            self.angle.is_finite() && (0.0..=180.0).contains(&self.spread),
            "confetti aim must be finite with a spread of 0 to 180 degrees"
        );
        ensure!(
            self.gravity.is_finite() && (0.0..=6000.0).contains(&self.gravity),
            "confetti gravity must be between 0 and 6000 px/s²"
        );
        ensure!(
            (1..=8).contains(&self.tones.len()),
            "confetti has 1 to 8 tones"
        );
        Ok(())
    }

    pub fn accepts(property: &str) -> bool {
        matches!(property, "opacity" | "x" | "y" | "burst")
    }
}

/// Authoring handle for one confetti actor.
#[derive(Clone, Debug)]
pub struct ConfettiActor {
    actor: ActorHandle,
}

impl ConfettiActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &ConfettiPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, CONFETTI_RECIPE, plan)?;
        Ok(Self { actor })
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

    /// Launch the burst at `at_nanos`; returns when its last piece is gone.
    /// A later burst relaunches the same pieces.
    pub fn burst(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> u64 {
        let clock = self.channel(scene, "burst", -1.0);
        crate::checklist::clock(scene, &clock, at_nanos, Some(LIFETIME));
        at_nanos + whole_millis(LIFETIME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confetti_round_trips_compactly_and_bursts_on_a_clock() {
        let plan = ConfettiPlan::new([960.0, 700.0]).seed(3);
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 2, "{json}");
        assert_eq!(serde_json::from_value::<ConfettiPlan>(json).unwrap(), plan);
        assert!(ConfettiPlan::new([0.0, 0.0]).count(0).validate().is_err());
        let mut scene = PlanBuilder::new("confetti", 6_000_000_000);
        let mut confetti = ConfettiActor::declare(&mut scene, "pop", &plan).unwrap();
        assert_eq!(confetti.burst(&mut scene, 1_000_000_000), 4_600_000_000);
        let plan = scene.finish().unwrap();
        assert_eq!(plan.continuous_channels[0].events.len(), 2);
        assert!(ConfettiPlan::accepts("burst") && !ConfettiPlan::accepts("age"));
    }
}
