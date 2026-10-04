//! Prepared captions: decoded and validated once; channel names are strict.
use anyhow::Result;
use psychopomp::{
    caption::{CAPTION_RECIPE, CaptionPlan},
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct PreparedCaption {
    id: String,
    plan: CaptionPlan,
}

impl PreparedCaption {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, CAPTION_RECIPE, CaptionPlan::validate)?;
        strict_channels(&actor.id, channels, CAPTION_RECIPE, CaptionPlan::accepts)?;
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
        renderer.composite_caption(pixels, &self.plan, |property, default| {
            sample(&self.id, property, default)
        });
    }
}
