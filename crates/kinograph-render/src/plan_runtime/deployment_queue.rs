use std::{cmp::Reverse, collections::HashMap, mem::discriminant};

use anyhow::{Context, Result, bail};
use kinograph::{
    deployment::{
        DeploymentItemPlan, DeploymentPhasePlan, DeploymentQueueRecipePlan,
        DeploymentQueueSnapshotPlan,
    },
    plan::{ActorPlan, StateChannelPlan},
    state::{StateTrack, TimedState},
    timeline::{PropertyId, SpringProfile, TimedEvent, Timeline},
};

use crate::render::{
    DeploymentItemFrame, DeploymentQueueFrame, HeadlessRenderer, deployment_row_center_y,
};

use super::keyed_layout::{KeyedLayoutTrack, LayoutMotion, LayoutSnapshot, LayoutTarget};

pub(super) struct PreparedDeploymentQueue {
    actor_id: String,
    product: String,
    title: String,
    subtitle: String,
    environment: String,
    release: String,
    catalog: Vec<DeploymentItemPlan>,
    snapshots: StateTrack<DeploymentQueueSnapshotPlan>,
    layout: KeyedLayoutTrack<String>,
    item_timeline: Timeline,
    item_properties: HashMap<String, ItemProperties>,
    attention_transition: PropertyId,
}

struct ItemProperties {
    progress: PropertyId,
    transition: PropertyId,
}

#[derive(Debug, PartialEq)]
pub(super) struct DeploymentQueueVisualKey {
    items: Vec<DeploymentItemVisualKey>,
    attention_transition: u32,
}

#[derive(Debug, PartialEq)]
struct DeploymentItemVisualKey {
    item_id: String,
    previous_phase: Option<DeploymentPhasePlan>,
    phase: DeploymentPhasePlan,
    values: [u32; 5],
}

#[derive(Clone)]
struct SnapshotEvent {
    at: f64,
    snapshot: DeploymentQueueSnapshotPlan,
}

impl PreparedDeploymentQueue {
    pub(super) fn new(
        actor: &ActorPlan,
        state_channels: &[StateChannelPlan],
        duration_nanos: u64,
    ) -> Result<Self> {
        let recipe = serde_json::from_value::<DeploymentQueueRecipePlan>(actor.data.clone())
            .with_context(|| format!("parse deployment queue recipe for actor '{}'", actor.id))?;
        recipe
            .validate()
            .with_context(|| format!("validate deployment queue actor '{}' recipe", actor.id))?;

        let mut snapshot_channels = state_channels
            .iter()
            .filter(|channel| channel.actor_id == actor.id && channel.state == "snapshot");
        let snapshot_channel = snapshot_channels.next().with_context(|| {
            format!(
                "deployment queue actor '{}' requires exactly one snapshot state channel",
                actor.id
            )
        })?;
        if snapshot_channels.next().is_some() {
            bail!(
                "deployment queue actor '{}' requires exactly one snapshot state channel",
                actor.id
            );
        }

        let initial = parse_snapshot(&actor.id, "initial", &snapshot_channel.initial)?;
        recipe.validate_snapshot(&initial).with_context(|| {
            format!(
                "validate deployment queue actor '{}' initial snapshot",
                actor.id
            )
        })?;
        let mut events = snapshot_channel
            .events
            .iter()
            .enumerate()
            .map(|(index, event)| {
                let snapshot = parse_snapshot(&actor.id, &format!("event {index}"), &event.value)?;
                recipe.validate_snapshot(&snapshot).with_context(|| {
                    format!(
                        "validate deployment queue actor '{}' event {index}",
                        actor.id
                    )
                })?;
                Ok((
                    index,
                    SnapshotEvent {
                        at: seconds_f64(event.at_nanos),
                        snapshot,
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        events.sort_by(|(left_index, left), (right_index, right)| {
            left.at
                .total_cmp(&right.at)
                .then(left_index.cmp(right_index))
        });
        let mut effective_events: Vec<SnapshotEvent> = Vec::new();
        for (_, event) in events {
            if effective_events
                .last()
                .is_some_and(|previous| previous.at == event.at)
            {
                *effective_events
                    .last_mut()
                    .expect("equal-time deployment snapshot has a predecessor") = event;
            } else {
                effective_events.push(event);
            }
        }
        let events = effective_events;
        let duration = seconds_f64(duration_nanos);

        let snapshots = StateTrack::compile(
            initial.clone(),
            events
                .iter()
                .map(|event| TimedState::new(event.at, event.snapshot.clone())),
            duration,
        )?;
        let layout = KeyedLayoutTrack::compile(
            layout_snapshot(&initial)?,
            events
                .iter()
                .map(|event| Ok(TimedState::new(event.at, layout_snapshot(&event.snapshot)?)))
                .collect::<Result<Vec<_>>>()?,
            LayoutMotion {
                position: position_spring(),
                presence: presence_spring(),
            },
            duration,
        )?;
        let (item_timeline, item_properties, attention_transition) =
            compile_item_timeline(&recipe.items, &initial, &events, duration)?;

        Ok(Self {
            actor_id: actor.id.clone(),
            product: recipe.product,
            title: recipe.title,
            subtitle: recipe.subtitle,
            environment: recipe.environment,
            release: recipe.release,
            catalog: recipe.items,
            snapshots,
            layout,
            item_timeline,
            item_properties,
            attention_transition,
        })
    }

    pub(super) fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub(super) fn file_name(&self) -> &str {
        &self.actor_id
    }

    pub(super) fn render(
        &self,
        renderer: &mut HeadlessRenderer,
        time: f64,
        value: impl Fn(&str, f32) -> f32,
    ) -> Result<Vec<u8>> {
        let snapshot = self.snapshots.sample_at(time);
        let attention_transition =
            self.sample_item_property(&self.attention_transition, time, "attention transition")?;
        let mut items = Vec::new();
        for row in self.layout.sample_at(time) {
            let catalog = self.catalog_item(row.key)?;
            let current_phase = phase_ref_in_snapshot(snapshot.current, row.key);
            let previous_phase = phase_ref_in_snapshot(snapshot.previous, row.key);
            let phase = current_phase.or(if row.present { None } else { previous_phase });
            let phase = phase.with_context(|| {
                format!(
                    "deployment queue actor '{}' has no phase for visible item '{}'",
                    self.actor_id, row.key
                )
            })?;
            let properties = &self.item_properties[row.key.as_str()];
            items.push(DeploymentItemFrame {
                item: catalog,
                phase,
                previous_phase,
                present: row.present,
                row_y: row.y.position,
                presence: row.presence.position,
                progress: self.sample_item_property(&properties.progress, time, "item progress")?,
                transition: self.sample_item_property(
                    &properties.transition,
                    time,
                    "item phase transition",
                )?,
            });
        }
        items.sort_by_key(|item| {
            Reverse(
                snapshot
                    .current
                    .items
                    .iter()
                    .position(|current| current.item_id == item.item.id)
                    .unwrap_or(usize::MAX),
            )
        });

        renderer.render_deployment_queue(&DeploymentQueueFrame {
            product: &self.product,
            title: &self.title,
            subtitle: &self.subtitle,
            environment: &self.environment,
            release: &self.release,
            items: &items,
            attention: snapshot.current.attention.as_ref(),
            attention_transition,
            center: [value("x", 960.0), value("y", 540.0)],
            scale: value("scale", 1.0),
            rotation: value("rotation", 0.0),
            tilt_x: value("tilt-x", 0.0),
            tilt_y: value("tilt-y", 0.0),
            near_edge_blur: value("near-edge-blur", 0.0),
            opacity: value("opacity", 1.0),
        })
    }

    pub(super) fn visual_key(&self, time: f64) -> DeploymentQueueVisualKey {
        let snapshot = self.snapshots.sample_at(time);
        let items = self
            .layout
            .sample_at(time)
            .into_iter()
            .map(|row| {
                let properties = &self.item_properties[row.key.as_str()];
                let current_phase = phase_ref_in_snapshot(snapshot.current, row.key);
                let previous_phase = phase_ref_in_snapshot(snapshot.previous, row.key);
                let phase = current_phase.or(if row.present { None } else { previous_phase });
                let phase = phase.expect("visible deployment item must have a display phase");
                DeploymentItemVisualKey {
                    item_id: row.key.clone(),
                    previous_phase: previous_phase.copied(),
                    phase: *phase,
                    values: [
                        row.x.position.to_bits(),
                        row.y.position.to_bits(),
                        row.presence.position.to_bits(),
                        self.item_timeline
                            .sample_at(&properties.progress, time)
                            .expect("deployment item progress property must be compiled")
                            .position
                            .to_bits(),
                        self.item_timeline
                            .sample_at(&properties.transition, time)
                            .expect("deployment item transition property must be compiled")
                            .position
                            .to_bits(),
                    ],
                }
            })
            .collect();
        let attention_transition = self
            .item_timeline
            .sample_at(&self.attention_transition, time)
            .expect("deployment attention transition property must be compiled")
            .position
            .to_bits();
        DeploymentQueueVisualKey {
            items,
            attention_transition,
        }
    }

    fn catalog_item(&self, item_id: &str) -> Result<&DeploymentItemPlan> {
        self.catalog
            .iter()
            .find(|item| item.id == item_id)
            .with_context(|| {
                format!(
                    "deployment queue actor '{}' has no catalog item '{}'",
                    self.actor_id, item_id
                )
            })
    }

    fn sample_item_property(
        &self,
        property: &PropertyId,
        time: f64,
        description: &str,
    ) -> Result<f32> {
        self.item_timeline
            .sample_at(property, time)
            .map(|sample| sample.position)
            .with_context(|| {
                format!(
                    "sample deployment queue actor '{}' {description} at {time}",
                    self.actor_id
                )
            })
    }
}

fn parse_snapshot(
    actor_id: &str,
    location: &str,
    value: &serde_json::Value,
) -> Result<DeploymentQueueSnapshotPlan> {
    serde_json::from_value(value.clone())
        .with_context(|| format!("parse deployment queue actor '{actor_id}' snapshot {location}"))
}

fn phase_in_snapshot(
    snapshot: &DeploymentQueueSnapshotPlan,
    item_id: &str,
) -> Option<DeploymentPhasePlan> {
    phase_ref_in_snapshot(snapshot, item_id).copied()
}

fn phase_ref_in_snapshot<'a>(
    snapshot: &'a DeploymentQueueSnapshotPlan,
    item_id: &str,
) -> Option<&'a DeploymentPhasePlan> {
    snapshot
        .items
        .iter()
        .find(|item| item.item_id == item_id)
        .map(|item| &item.phase)
}

fn layout_snapshot(snapshot: &DeploymentQueueSnapshotPlan) -> Result<LayoutSnapshot<String>> {
    LayoutSnapshot::new(
        snapshot
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| LayoutTarget {
                key: item.item_id.clone(),
                x: 0.0,
                y: deployment_row_center_y(index),
            }),
    )
}

fn compile_item_timeline(
    catalog: &[DeploymentItemPlan],
    initial: &DeploymentQueueSnapshotPlan,
    events: &[SnapshotEvent],
    duration: f64,
) -> Result<(Timeline, HashMap<String, ItemProperties>, PropertyId)> {
    let mut item_properties = HashMap::new();
    let mut initial_values = Vec::with_capacity(catalog.len() * 2 + 1);
    let mut phases = HashMap::new();
    let mut progress_targets = HashMap::new();
    for item in catalog {
        let properties = ItemProperties {
            progress: PropertyId::new(format!("deployment-queue.{}.progress", item.id)),
            transition: PropertyId::new(format!("deployment-queue.{}.transition", item.id)),
        };
        let phase = phase_in_snapshot(initial, &item.id);
        let progress = phase.map_or(0.0, |phase| progress_target(phase, 0.0));
        initial_values.push((properties.progress.clone(), progress));
        initial_values.push((properties.transition.clone(), 1.0));
        phases.insert(item.id.clone(), phase);
        progress_targets.insert(item.id.clone(), progress);
        item_properties.insert(item.id.clone(), properties);
    }
    let attention_transition = PropertyId::new("deployment-queue.attention-transition");
    initial_values.push((attention_transition.clone(), 1.0));

    let mut timeline_events = Vec::new();
    let mut attention = initial.attention.clone();
    for (event_index, event) in events.iter().enumerate() {
        let next_event_at = events.get(event_index + 1).map_or(duration, |next| next.at);
        let mut phase_stagger = 0_u32;
        for item in catalog {
            let previous_phase = phases[&item.id];
            let next_phase = phase_in_snapshot(&event.snapshot, &item.id);
            let properties = &item_properties[&item.id];
            if let Some(next_phase) = next_phase {
                let previous_progress = progress_targets[&item.id];
                let next_progress = progress_target(next_phase, previous_progress);
                if next_progress + f32::EPSILON < previous_progress {
                    bail!(
                        "deployment queue item '{}' progress cannot decrease from {:.3} to {:.3}",
                        item.id,
                        previous_progress,
                        next_progress
                    );
                }
                if next_progress != previous_progress {
                    timeline_events.push(TimedEvent::spring(
                        event.at,
                        properties.progress.clone(),
                        next_progress,
                        progress_spring(),
                    ));
                    progress_targets.insert(item.id.clone(), next_progress);
                }
            }
            if phases_differ(previous_phase, next_phase) {
                let transition_at = (event.at + f64::from(phase_stagger) * 0.045)
                    .min(next_event_at)
                    .min(duration);
                timeline_events.push(TimedEvent::set(
                    event.at,
                    properties.transition.clone(),
                    0.0,
                ));
                timeline_events.push(TimedEvent::spring(
                    transition_at,
                    properties.transition.clone(),
                    1.0,
                    transition_spring(),
                ));
                phase_stagger += 1;
            }
            phases.insert(item.id.clone(), next_phase);
        }

        if event.snapshot.attention != attention {
            timeline_events.push(TimedEvent::set(event.at, attention_transition.clone(), 0.0));
            timeline_events.push(TimedEvent::spring(
                event.at,
                attention_transition.clone(),
                1.0,
                transition_spring(),
            ));
            attention = event.snapshot.attention.clone();
        }
    }

    Ok((
        Timeline::compile_events(initial_values, timeline_events, duration)?,
        item_properties,
        attention_transition,
    ))
}

fn phases_differ(
    previous: Option<DeploymentPhasePlan>,
    current: Option<DeploymentPhasePlan>,
) -> bool {
    match (previous, current) {
        (Some(previous), Some(current)) => discriminant(&previous) != discriminant(&current),
        (None, None) | (None, Some(_)) | (Some(_), None) => false,
    }
}

fn progress_target(phase: DeploymentPhasePlan, previous: f32) -> f32 {
    match phase {
        DeploymentPhasePlan::Queued => 0.0,
        DeploymentPhasePlan::Building { progress }
        | DeploymentPhasePlan::Deploying { progress }
        | DeploymentPhasePlan::Verifying { progress } => progress,
        DeploymentPhasePlan::Succeeded => 1.0,
        DeploymentPhasePlan::Failed => previous,
    }
}

fn position_spring() -> SpringProfile {
    SpringProfile::from_visual_duration(0.42, 0.0, 0.001, 0.001)
}

fn presence_spring() -> SpringProfile {
    SpringProfile::from_visual_duration(0.28, 0.0, 0.001, 0.001)
}

fn progress_spring() -> SpringProfile {
    SpringProfile::from_visual_duration(0.36, 0.0, 0.001, 0.001)
}

fn transition_spring() -> SpringProfile {
    SpringProfile::from_visual_duration(0.24, 0.0, 0.001, 0.001)
}

fn seconds_f64(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000_000.0
}

#[cfg(test)]
mod tests {
    use kinograph::{
        deployment::{
            DeploymentItemPlan, DeploymentPhasePlan, DeploymentQueueRecipePlan,
            DeploymentQueueSnapshotPlan,
        },
        plan::{ActorPlan, StateChannelPlan, StateEventPlan},
    };
    use serde_json::json;

    use crate::render::deployment_row_center_y;

    use super::PreparedDeploymentQueue;

    const DURATION: u64 = 3_000_000_000;

    fn actor(items: Vec<DeploymentItemPlan>) -> ActorPlan {
        ActorPlan {
            id: "deployments".to_owned(),
            recipe: "deployment-queue".to_owned(),
            data: serde_json::to_value(DeploymentQueueRecipePlan::new(
                "NORTHSTAR",
                "Release Control",
                "Live deployment telemetry",
                "PRODUCTION",
                "release-2026.07.16",
                items,
            ))
            .unwrap(),
        }
    }

    fn channel(
        initial: DeploymentQueueSnapshotPlan,
        events: impl IntoIterator<Item = (u64, DeploymentQueueSnapshotPlan)>,
    ) -> StateChannelPlan {
        StateChannelPlan {
            id: "deployments.snapshot".to_owned(),
            actor_id: "deployments".to_owned(),
            state: "snapshot".to_owned(),
            initial: serde_json::to_value(initial).unwrap(),
            events: events
                .into_iter()
                .map(|(at_nanos, snapshot)| StateEventPlan {
                    at_nanos,
                    value: serde_json::to_value(snapshot).unwrap(),
                })
                .collect(),
        }
    }

    #[test]
    fn validates_catalog_channel_snapshots_and_attention() {
        let api = DeploymentItemPlan::new("api", "API");
        let worker = DeploymentItemPlan::new("worker", "Worker");
        let valid = channel(DeploymentQueueSnapshotPlan::new([api.queued()]), []);
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![api.clone(), worker.clone()]),
                std::slice::from_ref(&valid),
                DURATION,
            )
            .is_ok()
        );
        assert!(PreparedDeploymentQueue::new(&actor(vec![api.clone()]), &[], DURATION,).is_err());
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![api.clone()]),
                &[valid.clone(), valid.clone()],
                DURATION,
            )
            .is_err()
        );
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![DeploymentItemPlan::new("api", " ")]),
                std::slice::from_ref(&valid),
                DURATION,
            )
            .is_err()
        );
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![api.clone(), worker.clone()]),
                &[channel(
                    DeploymentQueueSnapshotPlan::new([api.queued(), api.failed()]),
                    [],
                )],
                DURATION,
            )
            .is_err()
        );
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![api.clone(), worker.clone()]),
                &[channel(
                    DeploymentQueueSnapshotPlan::new([api.queued()]).attend(worker.target()),
                    [],
                )],
                DURATION,
            )
            .is_err()
        );
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![api.clone(), worker.clone()]),
                &[channel(
                    DeploymentQueueSnapshotPlan::new([api.building(1.1)]),
                    [],
                )],
                DURATION,
            )
            .is_err()
        );

        let unknown = DeploymentItemPlan::new("unknown", "Unknown");
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![api]),
                &[channel(
                    DeploymentQueueSnapshotPlan::new([unknown.queued()]),
                    [],
                )],
                DURATION,
            )
            .is_err()
        );

        let malformed = StateChannelPlan {
            initial: json!({ "items": [{ "itemId": "api", "phase": "building", "progress": "NaN" }] }),
            ..valid
        };
        assert!(
            PreparedDeploymentQueue::new(
                &actor(vec![DeploymentItemPlan::new("api", "API")]),
                &[malformed],
                DURATION,
            )
            .is_err()
        );
    }

    #[test]
    fn interrupted_reorder_retains_position_and_velocity() {
        let api = DeploymentItemPlan::new("api", "API");
        let worker = DeploymentItemPlan::new("worker", "Worker");
        let initial = DeploymentQueueSnapshotPlan::new([api.queued(), worker.queued()]);
        let first = DeploymentQueueSnapshotPlan::new([worker.queued(), api.queued()]);
        let uninterrupted = PreparedDeploymentQueue::new(
            &actor(vec![api.clone(), worker.clone()]),
            &[channel(initial.clone(), [(500_000_000, first.clone())])],
            DURATION,
        )
        .unwrap();
        let interrupted = PreparedDeploymentQueue::new(
            &actor(vec![api.clone(), worker.clone()]),
            &[channel(
                initial,
                [
                    (500_000_000, first),
                    (
                        650_000_000,
                        DeploymentQueueSnapshotPlan::new([api.queued(), worker.queued()]),
                    ),
                ],
            )],
            DURATION,
        )
        .unwrap();

        let expected = uninterrupted
            .layout
            .sample_at(0.65)
            .into_iter()
            .find(|row| row.key == "api")
            .unwrap();
        let actual = interrupted
            .layout
            .sample_at(0.65)
            .into_iter()
            .find(|row| row.key == "api")
            .unwrap();
        assert_eq!(actual.y, expected.y);
        assert!(actual.y.velocity.abs() > 0.001);
        assert!(
            interrupted
                .layout
                .sample_at(2.0)
                .into_iter()
                .find(|row| row.key == "api")
                .unwrap()
                .y
                .position
                < deployment_row_center_y(0) + 0.1
        );
    }

    #[test]
    fn visual_keys_preserve_active_motion_and_merge_settled_samples() {
        let api = DeploymentItemPlan::new("api", "API");
        let prepared = PreparedDeploymentQueue::new(
            &actor(vec![api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([api.building(0.1)]),
                [(
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([api.deploying(0.8)]).attend(api.target()),
                )],
            )],
            DURATION,
        )
        .unwrap();

        assert_ne!(prepared.visual_key(0.6), prepared.visual_key(0.61));
        assert_eq!(prepared.visual_key(2.0), prepared.visual_key(2.1));
    }

    #[test]
    fn staggered_phase_replacements_hold_every_outgoing_phase_at_the_boundary() {
        let web = DeploymentItemPlan::new("web", "Web");
        let api = DeploymentItemPlan::new("api", "API");
        let prepared = PreparedDeploymentQueue::new(
            &actor(vec![web.clone(), api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([web.queued(), api.queued()]),
                [(
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([web.succeeded(), api.failed()]),
                )],
            )],
            DURATION,
        )
        .unwrap();

        let web_transition = &prepared.item_properties["web"].transition;
        let api_transition = &prepared.item_properties["api"].transition;
        assert_eq!(
            prepared
                .item_timeline
                .sample_at(web_transition, 0.5)
                .unwrap()
                .position,
            0.0
        );
        assert_eq!(
            prepared
                .item_timeline
                .sample_at(api_transition, 0.5)
                .unwrap()
                .position,
            0.0
        );
        assert!(
            prepared
                .item_timeline
                .sample_at(web_transition, 0.53)
                .unwrap()
                .position
                > 0.0
        );
        assert_eq!(
            prepared
                .item_timeline
                .sample_at(api_transition, 0.53)
                .unwrap()
                .position,
            0.0
        );
    }

    #[test]
    fn staggered_phase_replacements_do_not_outlive_a_newer_snapshot() {
        let web = DeploymentItemPlan::new("web", "Web");
        let api = DeploymentItemPlan::new("api", "API");
        let prepared = PreparedDeploymentQueue::new(
            &actor(vec![web.clone(), api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([web.queued(), api.queued()]),
                [
                    (
                        500_000_000,
                        DeploymentQueueSnapshotPlan::new([web.building(0.2), api.building(0.2)]),
                    ),
                    (
                        520_000_000,
                        DeploymentQueueSnapshotPlan::new([web.deploying(0.3), api.deploying(0.3)]),
                    ),
                ],
            )],
            DURATION,
        )
        .unwrap();

        let api_transition = &prepared.item_properties["api"].transition;
        assert_eq!(
            prepared
                .item_timeline
                .sample_at(api_transition, 0.54)
                .unwrap()
                .position,
            0.0
        );
        assert!(
            prepared
                .item_timeline
                .sample_at(api_transition, 0.58)
                .unwrap()
                .position
                > 0.0
        );
    }

    #[test]
    fn visual_keys_use_the_immediately_previous_complete_snapshot() {
        let web = DeploymentItemPlan::new("web", "Web");
        let api = DeploymentItemPlan::new("api", "API");
        let prepared = PreparedDeploymentQueue::new(
            &actor(vec![web.clone(), api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([web.queued(), api.queued()]),
                [
                    (
                        500_000_000,
                        DeploymentQueueSnapshotPlan::new([web.succeeded(), api.failed()]),
                    ),
                    (
                        1_000_000_000,
                        DeploymentQueueSnapshotPlan::new([web.succeeded(), api.building(0.6)]),
                    ),
                ],
            )],
            DURATION,
        )
        .unwrap();

        let key = prepared.visual_key(1.0);
        let web_key = key.items.iter().find(|item| item.item_id == "web").unwrap();
        let api_key = key.items.iter().find(|item| item.item_id == "api").unwrap();
        assert_eq!(web_key.previous_phase, Some(DeploymentPhasePlan::Succeeded));
        assert_eq!(api_key.previous_phase, Some(DeploymentPhasePlan::Failed));
    }

    #[test]
    fn decreasing_aggregate_progress_is_rejected() {
        let api = DeploymentItemPlan::new("api", "API");
        let result = PreparedDeploymentQueue::new(
            &actor(vec![api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([api.building(0.8)]),
                [(
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([api.building(0.2)]),
                )],
            )],
            DURATION,
        );

        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("progress cannot decrease")
        );
    }

    #[test]
    fn equal_time_snapshots_compile_only_the_final_ui_state() {
        let api = DeploymentItemPlan::new("api", "API");
        let initial = DeploymentQueueSnapshotPlan::new([api.queued()]);
        let expected = PreparedDeploymentQueue::new(
            &actor(vec![api.clone()]),
            &[channel(
                initial.clone(),
                [(
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([api.succeeded()]),
                )],
            )],
            DURATION,
        )
        .unwrap();
        let actual = PreparedDeploymentQueue::new(
            &actor(vec![api.clone()]),
            &[channel(
                initial,
                [
                    (
                        500_000_000,
                        DeploymentQueueSnapshotPlan::new([api.building(0.25)]),
                    ),
                    (
                        500_000_000,
                        DeploymentQueueSnapshotPlan::new([api.succeeded()]),
                    ),
                ],
            )],
            DURATION,
        )
        .unwrap();

        assert_eq!(actual.visual_key(0.5), expected.visual_key(0.5));
        assert_eq!(actual.visual_key(0.65), expected.visual_key(0.65));
    }

    #[test]
    fn visual_keys_retain_the_phase_of_an_exiting_item() {
        let api = DeploymentItemPlan::new("api", "API");
        let prepared = PreparedDeploymentQueue::new(
            &actor(vec![api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([api.queued()]),
                [
                    (500_000_000, DeploymentQueueSnapshotPlan::new([])),
                    (
                        1_500_000_000,
                        DeploymentQueueSnapshotPlan::new([api.failed()]),
                    ),
                    (2_500_000_000, DeploymentQueueSnapshotPlan::new([])),
                ],
            )],
            DURATION,
        )
        .unwrap();

        let queued_exit = prepared.visual_key(0.6);
        let failed_exit = prepared.visual_key(2.6);
        assert_eq!(queued_exit.items[0].values, failed_exit.items[0].values);
        assert_ne!(queued_exit, failed_exit);
    }

    #[test]
    fn visual_keys_include_the_outgoing_phase_during_replacement() {
        let api = DeploymentItemPlan::new("api", "API");
        let from_queued = PreparedDeploymentQueue::new(
            &actor(vec![api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([api.queued()]),
                [(
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([api.failed()]),
                )],
            )],
            DURATION,
        )
        .unwrap();
        let from_deploying = PreparedDeploymentQueue::new(
            &actor(vec![api.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([api.deploying(0.0)]),
                [(
                    500_000_000,
                    DeploymentQueueSnapshotPlan::new([api.failed()]),
                )],
            )],
            DURATION,
        )
        .unwrap();

        assert_ne!(from_queued.visual_key(0.5), from_deploying.visual_key(0.5));
    }

    #[test]
    fn out_of_order_sampling_is_deterministic() {
        let api = DeploymentItemPlan::new("api", "API");
        let worker = DeploymentItemPlan::new("worker", "Worker");
        let prepared = PreparedDeploymentQueue::new(
            &actor(vec![api.clone(), worker.clone()]),
            &[channel(
                DeploymentQueueSnapshotPlan::new([api.queued(), worker.queued()]),
                [
                    (
                        400_000_000,
                        DeploymentQueueSnapshotPlan::new([
                            worker.building(0.3),
                            api.deploying(0.6),
                        ]),
                    ),
                    (
                        800_000_000,
                        DeploymentQueueSnapshotPlan::new([api.succeeded(), worker.failed()]),
                    ),
                ],
            )],
            DURATION,
        )
        .unwrap();

        let later = prepared.visual_key(0.9);
        let _earlier = prepared.visual_key(0.1);
        assert_eq!(later, prepared.visual_key(0.9));
    }
}
