//! Lower Thirds: decoded and validated once; channel names are strict.
//! Channel-only, so they run in native playback as well as video.
use anyhow::Result;
use psychopomp::{
    lower_third::{LOWER_THIRD_PROPERTIES, LowerThirdPlan},
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::{HeadlessRenderer, LowerThirdGlyphs};

pub(super) struct PreparedLowerThird {
    id: String,
    plan: LowerThirdPlan,
    glyphs: LowerThirdGlyphs,
}

impl PreparedLowerThird {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "lower third", LowerThirdPlan::validate)?;
        strict_channels(&actor.id, channels, "lower third", |property| {
            LOWER_THIRD_PROPERTIES.contains(&property)
        })?;
        Ok(Self {
            id: actor.id.clone(),
            plan,
            glyphs: LowerThirdGlyphs::default(),
        })
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &mut HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) {
        renderer.composite_lower_third(pixels, &self.plan, &self.glyphs, |property, d| {
            sample(&self.id, property, d)
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::plan_runtime::{preflight, validate_renderer_plan};
    use psychopomp::{
        author::PlanBuilder,
        lower_third::{LowerThirdActor, LowerThirdPlan},
    };

    #[test]
    fn preflight_rejects_unknown_channels_without_a_gpu() {
        let build = |property: &str| {
            let mut scene = PlanBuilder::new("lower-third", 3_000_000_000);
            let mut third = LowerThirdActor::declare(
                &mut scene,
                "who",
                &LowerThirdPlan::new([120.0, 880.0], "Kit"),
            )
            .unwrap();
            third.show(&mut scene, 0);
            third.channel(&mut scene, property, 1.0);
            scene.finish().unwrap()
        };
        validate_renderer_plan(&build("x")).unwrap();
        assert!(preflight::Plan::new(build("role")).unwrap().native());
        assert!(validate_renderer_plan(&build("typed")).is_err());
    }
}
