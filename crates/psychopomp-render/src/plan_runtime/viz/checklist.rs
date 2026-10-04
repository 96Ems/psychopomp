//! Prepared checklists: decoded once; every channel must name a real item
//! property or a checklist property.
use anyhow::Result;
use psychopomp::{
    checklist::ChecklistPlan,
    plan::{ActorPlan, ContinuousChannelPlan},
};

use super::super::preflight::{decode, strict_channels};
use crate::render::HeadlessRenderer;

pub(super) struct PreparedChecklist {
    id: String,
    plan: ChecklistPlan,
}

impl PreparedChecklist {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, "checklist", ChecklistPlan::validate)?;
        strict_channels(&actor.id, channels, "checklist", |property| {
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
        renderer.composite_checklist(pixels, &self.plan, |property, default| {
            sample(&self.id, property, default)
        });
    }
}
