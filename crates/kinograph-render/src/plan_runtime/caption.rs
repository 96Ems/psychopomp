//! Prepared captions: decoded and validated once; channel names are strict.
use anyhow::{Context, Result, bail};
use kinograph::{
    caption::CaptionPlan,
    plan::{ActorPlan, ContinuousChannelPlan},
};

use crate::render::HeadlessRenderer;

pub(super) struct PreparedCaption {
    id: String,
    plan: CaptionPlan,
}

impl PreparedCaption {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan: CaptionPlan = serde_json::from_value(actor.data.clone())
            .with_context(|| format!("parse caption recipe for actor '{}'", actor.id))?;
        plan.validate()
            .with_context(|| format!("caption actor '{}'", actor.id))?;
        for channel in channels
            .iter()
            .filter(|channel| channel.actor_id == actor.id)
        {
            if !matches!(
                channel.property.as_str(),
                "opacity" | "x" | "y" | "typed" | "caret"
            ) {
                bail!(
                    "caption actor '{}' has unknown property '{}'",
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
