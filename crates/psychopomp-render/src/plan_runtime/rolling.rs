//! Rolling Numbers: the recipe is decoded and its channels checked during
//! preflight; preparation measures its glyphs and compiles every change into
//! closed-form tracks once.
use anyhow::{Result, ensure};
use psychopomp::{
    anchor::AnchorPlan,
    math::Vec2,
    plan::{ActorPlan, ContinuousChannelPlan},
    rolling::{CompiledRoll, RollingNumberPlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct RollingNumberInput {
    id: String,
    plan: RollingNumberPlan,
}

impl RollingNumberInput {
    pub(super) fn new(
        actor: &ActorPlan,
        channels: &[ContinuousChannelPlan],
        duration_nanos: u64,
    ) -> Result<Self> {
        let plan = decode(actor, "rolling number", RollingNumberPlan::validate)?;
        ensure!(
            plan.rolls
                .last()
                .is_none_or(|roll| roll.at_nanos <= duration_nanos),
            "rolling number actor '{}' changes after the scene ends",
            actor.id
        );
        strict_channels(&actor.id, channels, "rolling number", |property| {
            plan.accepts(property)
        })?;
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn anchors(&self) -> &[AnchorPlan] {
        &self.plan.anchors
    }

    pub(super) fn prepare(self, renderer: &mut HeadlessRenderer) -> PreparedRollingNumber {
        let roll = renderer.compile_rolling_number(&self.plan);
        PreparedRollingNumber {
            id: self.id,
            plan: self.plan,
            roll,
        }
    }
}

pub(super) struct PreparedRollingNumber {
    id: String,
    plan: RollingNumberPlan,
    roll: CompiledRoll,
}

impl PreparedRollingNumber {
    /// Samples differ while a change settles, even with no channel moving.
    pub(super) fn moving(&self, time: f64) -> bool {
        self.roll.moving(time)
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn anchors(&self) -> &[AnchorPlan] {
        &self.plan.anchors
    }

    /// The literal origin, used while the number has no anchors.
    pub(super) fn origin(&self) -> Vec2 {
        Vec2::from(self.plan.origin)
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        time: f64,
        origin: Vec2,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_rolling_number_at(
            pixels,
            &self.plan,
            &self.roll,
            time,
            origin.to_array(),
            |property, d| sample(&self.id, property, d),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::validate_renderer_plan;
    use psychopomp::{
        author::PlanBuilder,
        rolling::{RollingNumberActor, RollingNumberPlan},
    };

    #[test]
    fn preflight_rejects_unknown_channels_and_late_changes_without_a_gpu() {
        let build = |property: &str, at: u64| {
            let mut scene = PlanBuilder::new("rolling", 2_000_000_000);
            let mut number = RollingNumberActor::declare(
                &mut scene,
                "count",
                RollingNumberPlan::new([960.0, 540.0], 64.0, "0/8").roll(at, "8/8"),
            )
            .unwrap();
            number.channel(&mut scene, property, 1.0);
            scene.finish().unwrap()
        };
        validate_renderer_plan(&build("opacity", 1_000_000_000)).unwrap();
        assert!(validate_renderer_plan(&build("typed", 1_000_000_000)).is_err());
        assert!(validate_renderer_plan(&build("opacity", 3_000_000_000)).is_err());
    }
}
