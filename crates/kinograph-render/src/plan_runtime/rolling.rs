//! Rolling Numbers: the recipe is decoded and its channels checked during
//! preflight; preparation measures its glyphs and compiles every change into
//! closed-form tracks once.
use anyhow::{Context, Result, bail, ensure};
use kinograph::{
    plan::{ActorPlan, ContinuousChannelPlan},
    rolling::{CompiledRoll, RollingNumberPlan},
};

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
        let plan: RollingNumberPlan = serde_json::from_value(actor.data.clone())
            .with_context(|| format!("parse rolling number recipe for actor '{}'", actor.id))?;
        plan.validate()
            .with_context(|| format!("rolling number actor '{}'", actor.id))?;
        ensure!(
            plan.rolls
                .last()
                .is_none_or(|roll| roll.at_nanos <= duration_nanos),
            "rolling number actor '{}' changes after the scene ends",
            actor.id
        );
        for channel in channels.iter().filter(|c| c.actor_id == actor.id) {
            if !matches!(channel.property.as_str(), "opacity" | "x" | "y") {
                bail!(
                    "rolling number actor '{}' has unknown property '{}'",
                    actor.id,
                    channel.property
                );
            }
        }
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
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

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        time: f64,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_rolling_number(pixels, &self.plan, &self.roll, time, |property, d| {
            sample(&self.id, property, d)
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::validate_renderer_plan;
    use kinograph::{
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
