//! Semantic coordinates are a numeric spring plus a smoothly weighted correction
//! from expanded to sampled code geometry. The companion weights use the same
//! Timeline and Playback as authored channels, preserving interruption continuity.
use std::collections::{HashMap, HashSet};

use anyhow::Result;
use kinograph::{
    dsl::TargetGeometry,
    plan::{ContinuousChannelPlan, ScalarPlan, ScenePlan, TargetComponentPlan, TrackEventPlan},
};

pub(super) struct Attachment {
    pub channel: String,
    pub weight: String,
    pub target: String,
    pub component: TargetComponentPlan,
    pub base: f32,
    pub default_profile: kinograph::timeline::SpringProfile,
}

struct CompanionRequest<'a> {
    channel: &'a ContinuousChannelPlan,
    reference: &'a kinograph::plan::TargetScalarPlan,
    property: String,
    id: String,
}
fn requests(plan: &ScenePlan) -> Vec<CompanionRequest<'_>> {
    let mut requests = Vec::new();
    for (index, channel) in plan.continuous_channels.iter().enumerate() {
        let values =
            std::iter::once(&channel.initial).chain(channel.events.iter().map(
                |event| match event {
                    TrackEventPlan::Set { value, .. } => value,
                    TrackEventPlan::Spring { target, .. } => target,
                },
            ));
        let mut seen = HashSet::new();
        for value in values {
            let ScalarPlan::Target(reference) = value else {
                continue;
            };
            if !seen.insert((reference.target_id.clone(), reference.component)) {
                continue;
            }
            let property = format!("__attachment-{index}-{}", seen.len());
            requests.push(CompanionRequest {
                channel,
                reference,
                id: format!("{}.{property}", channel.actor_id),
                property,
            });
        }
    }
    requests
}
pub(super) fn reserve(
    plan: &ScenePlan,
    reservations: &mut super::generated::Reservations,
) -> Result<()> {
    for request in requests(plan) {
        reservations.reserve(
            &request.id,
            &request.channel.actor_id,
            &request.property,
            super::generated::Owner::Attachment,
        )?;
    }
    Ok(())
}

pub(super) fn compile(
    plan: &mut ScenePlan,
    geometry: &HashMap<String, TargetGeometry>,
    scales: &HashMap<String, f32>,
) -> Result<Vec<Attachment>> {
    let mut reservations = super::generated::Reservations::new(&plan.continuous_channels);
    let mut channels = Vec::new();
    let mut attachments = Vec::new();
    for CompanionRequest {
        channel,
        reference,
        property,
        id,
    } in requests(plan)
    {
        reservations.reserve(
            &id,
            &channel.actor_id,
            &property,
            super::generated::Owner::Attachment,
        )?;
        let weight = |value: &ScalarPlan| -> ScalarPlan {
            ScalarPlan::Literal(
                if matches!(value, ScalarPlan::Target(other) if other.target_id == reference.target_id && other.component == reference.component)
                {
                    1.0
                } else {
                    0.0
                },
            )
        };
        let target_geometry = &geometry[&reference.target_id];
        let scale = scales[&reference.target_id].max(1.0);
        let base = match reference.component {
            TargetComponentPlan::X => target_geometry.x,
            TargetComponentPlan::Width => target_geometry.width,
            TargetComponentPlan::CenterX => target_geometry.center_x(),
            TargetComponentPlan::LineY => target_geometry.line_y,
        };
        attachments.push(Attachment {
            channel: channel.id.clone(),
            weight: id.clone(),
            target: reference.target_id.clone(),
            component: reference.component,
            base,
            default_profile: kinograph::timeline::SpringProfile::from_visual_duration(
                0.4,
                0.,
                (0.001 / scale).max(f32::MIN_POSITIVE),
                (0.001 / scale).max(f32::MIN_POSITIVE),
            ),
        });
        channels.push(ContinuousChannelPlan {
            id,
            actor_id: channel.actor_id.clone(),
            property,
            initial: weight(&channel.initial),
            events: channel
                .events
                .iter()
                .map(|event| match event {
                    TrackEventPlan::Set { at_nanos, value } => TrackEventPlan::Set {
                        at_nanos: *at_nanos,
                        value: weight(value),
                    },
                    TrackEventPlan::Spring {
                        at_nanos,
                        target,
                        response_seconds,
                        damping_ratio,
                        position_threshold,
                        velocity_threshold,
                    } => TrackEventPlan::Spring {
                        at_nanos: *at_nanos,
                        target: weight(target),
                        response_seconds: *response_seconds,
                        damping_ratio: *damping_ratio,
                        position_threshold: (*position_threshold / scale).max(f32::MIN_POSITIVE),
                        velocity_threshold: (*velocity_threshold / scale).max(f32::MIN_POSITIVE),
                    },
                })
                .collect(),
        });
    }
    plan.continuous_channels.extend(channels);
    Ok(attachments)
}
