//! Prepared meters: decoded once; channel names are strict.
use anyhow::Result;
use psychopomp::{
    meter::MeterPlan,
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct PreparedMeter {
    id: String,
    plan: MeterPlan,
}

impl PreparedMeter {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "meter", MeterPlan::validate)?;
        strict_channels(&actor.id, channels, "meter", MeterPlan::accepts)?;
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
        velocity: impl Fn(&str, &str) -> f32,
    ) {
        renderer.composite_meter(
            pixels,
            &self.plan,
            |property, default| sample(&self.id, property, default),
            |property| velocity(&self.id, property),
        );
    }
}
