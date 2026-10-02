//! Prepared Lanes: the recipe is decoded and validated once, and every channel
//! on the actor must name a real lane or lanes property.
use anyhow::Result;
use psychopomp::{
    lanes::LanesPlan,
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct PreparedLanes {
    id: String,
    plan: LanesPlan,
}

impl PreparedLanes {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "lanes", LanesPlan::validate)?;
        strict_channels(&actor.id, channels, "lanes", |property| {
            plan.accepts(property)
        })?;
        Ok(Self {
            id: actor.id.clone(),
            plan,
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_lanes(pixels, &self.plan, |property, default| {
            sample(&self.id, property, default)
        });
    }
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        author::PlanBuilder,
        axis::AxisPlan,
        lanes::{LanePlan, LanesActor, LanesPlan},
    };

    use super::super::validate_renderer_plan;

    #[test]
    fn lanes_preflight_accepts_known_channels_and_rejects_typos() {
        let build = |property: Option<&str>| {
            let recipe = LanesPlan::new([160.0, 200.0], 1600.0, AxisPlan::new([0.0, 4.0]))
                .lane(LanePlan::new("card.x", "card.x").keys([1.0]));
            let mut scene = PlanBuilder::new("lanes-preflight", 2_000_000_000);
            let mut lanes = LanesActor::declare(&mut scene, "lanes", &recipe).unwrap();
            lanes.show(&mut scene, 0, 1.0);
            lanes.scrub(&mut scene, [0.0, 4.0], 0, 2.0);
            lanes.emphasize(&mut scene, "card.x", 0, 1.0);
            if let Some(property) = property {
                lanes.channel(&mut scene, property, 1.0);
            }
            scene.finish().unwrap()
        };
        validate_renderer_plan(&build(None)).unwrap();
        for typo in ["lane.card.y.opacity", "lane.card.x.glow", "playhead.x"] {
            let error = validate_renderer_plan(&build(Some(typo))).unwrap_err();
            assert!(
                format!("{error:#}").contains("unknown property"),
                "{typo}: {error:#}"
            );
        }
    }
}
