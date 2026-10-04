//! Prepared Benchmark Bars: decoded once; every channel must name a real
//! row, series pair, or bars property.
use anyhow::Result;
use psychopomp::{
    bars::BarsPlan,
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct PreparedBars {
    id: String,
    plan: BarsPlan,
}

impl PreparedBars {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "bars", BarsPlan::validate)?;
        strict_channels(&actor.id, channels, "bars", |property| {
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
        velocity: impl Fn(&str, &str) -> f32,
    ) {
        renderer.composite_bars(
            pixels,
            &self.plan,
            |property, default| sample(&self.id, property, default),
            |property| velocity(&self.id, property),
        );
    }
}
