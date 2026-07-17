use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use anyhow::{Result, bail};
use kinograph::{
    motion::MotionState,
    state::{StateTrack, TimedState},
    timeline::{PropertyId, SpringProfile, TimedEvent, Timeline},
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct LayoutTarget<K> {
    pub key: K,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub(super) struct LayoutSnapshot<K> {
    targets: Vec<LayoutTarget<K>>,
}

impl<K: Eq + Hash> LayoutSnapshot<K> {
    pub(super) fn new(targets: impl IntoIterator<Item = LayoutTarget<K>>) -> Result<Self> {
        let targets = targets.into_iter().collect::<Vec<_>>();
        let mut keys = HashSet::with_capacity(targets.len());
        for target in &targets {
            if !target.x.is_finite() || !target.y.is_finite() {
                bail!("layout target coordinates must be finite");
            }
            if !keys.insert(&target.key) {
                bail!("layout snapshot contains a duplicate key");
            }
        }
        Ok(Self { targets })
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LayoutMotion {
    pub position: SpringProfile,
    pub presence: SpringProfile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LayoutSample<'a, K> {
    pub key: &'a K,
    pub present: bool,
    pub x: MotionState,
    pub y: MotionState,
    pub presence: MotionState,
}

struct KeyTrack<K> {
    key: K,
    x: PropertyId,
    y: PropertyId,
    presence: PropertyId,
}

pub(super) struct KeyedLayoutTrack<K> {
    keys: Vec<KeyTrack<K>>,
    timeline: Timeline,
    membership: StateTrack<HashSet<K>>,
}

impl<K: Clone + Eq + Hash> KeyedLayoutTrack<K> {
    pub(super) fn compile(
        initial: LayoutSnapshot<K>,
        changes: impl IntoIterator<Item = TimedState<LayoutSnapshot<K>>>,
        motion: LayoutMotion,
        duration: f64,
    ) -> Result<Self> {
        if !duration.is_finite() || duration < 0.0 {
            bail!("layout track duration must be finite and non-negative");
        }

        let mut changes = changes.into_iter().enumerate().collect::<Vec<_>>();
        for (_, change) in &changes {
            if !change.at.is_finite() || change.at < 0.0 {
                bail!("layout snapshot time must be finite and non-negative");
            }
            if change.at > duration {
                bail!(
                    "layout snapshot at {:.3}s exceeds duration {duration:.3}s",
                    change.at
                );
            }
        }
        changes.sort_by(|(left_index, left), (right_index, right)| {
            left.at
                .total_cmp(&right.at)
                .then(left_index.cmp(right_index))
        });
        let mut effective_changes: Vec<(usize, TimedState<LayoutSnapshot<K>>)> = Vec::new();
        for change in changes {
            if effective_changes
                .last()
                .is_some_and(|(_, previous)| previous.at == change.1.at)
            {
                *effective_changes
                    .last_mut()
                    .expect("equal-time layout change has a predecessor") = change;
            } else {
                effective_changes.push(change);
            }
        }
        let changes = effective_changes;

        let mut key_indexes = HashMap::new();
        let mut ordered_keys = Vec::new();
        let mut first_positions = Vec::new();
        for snapshot in
            std::iter::once(&initial).chain(changes.iter().map(|(_, event)| &event.value))
        {
            for target in &snapshot.targets {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    key_indexes.entry(target.key.clone())
                {
                    let index = ordered_keys.len();
                    entry.insert(index);
                    ordered_keys.push(target.key.clone());
                    first_positions.push([target.x, target.y]);
                }
            }
        }

        let keys = ordered_keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| KeyTrack {
                key,
                x: PropertyId::new(format!("keyed-layout.{index}.x")),
                y: PropertyId::new(format!("keyed-layout.{index}.y")),
                presence: PropertyId::new(format!("keyed-layout.{index}.presence")),
            })
            .collect::<Vec<_>>();
        let initial_membership = initial
            .targets
            .iter()
            .map(|target| target.key.clone())
            .collect::<HashSet<_>>();
        let mut initial_values = Vec::with_capacity(keys.len() * 3);
        for (key, position) in keys.iter().zip(&first_positions) {
            initial_values.push((key.x.clone(), position[0]));
            initial_values.push((key.y.clone(), position[1]));
            initial_values.push((
                key.presence.clone(),
                if initial_membership.contains(&key.key) {
                    1.0
                } else {
                    0.0
                },
            ));
        }

        let mut timeline_events = Vec::new();
        let mut membership_events = Vec::with_capacity(changes.len());
        let mut membership = initial_membership.clone();
        let mut target_positions = first_positions;
        for (_, change) in changes {
            let next_membership = change
                .value
                .targets
                .iter()
                .map(|target| target.key.clone())
                .collect::<HashSet<_>>();

            for target in &change.value.targets {
                let index = key_indexes[&target.key];
                let key = &keys[index];
                let target_position = &mut target_positions[index];
                if target.x != target_position[0] {
                    timeline_events.push(TimedEvent::spring(
                        change.at,
                        key.x.clone(),
                        target.x,
                        motion.position,
                    ));
                    target_position[0] = target.x;
                }
                if target.y != target_position[1] {
                    timeline_events.push(TimedEvent::spring(
                        change.at,
                        key.y.clone(),
                        target.y,
                        motion.position,
                    ));
                    target_position[1] = target.y;
                }
            }

            for key in &keys {
                let was_present = membership.contains(&key.key);
                let is_present = next_membership.contains(&key.key);
                if was_present != is_present {
                    timeline_events.push(TimedEvent::spring(
                        change.at,
                        key.presence.clone(),
                        if is_present { 1.0 } else { 0.0 },
                        motion.presence,
                    ));
                }
            }

            membership_events.push(TimedState {
                at: change.at,
                value: next_membership.clone(),
            });
            membership = next_membership;
        }

        Ok(Self {
            keys,
            timeline: Timeline::compile_events(initial_values, timeline_events, duration)?,
            membership: StateTrack::compile(initial_membership, membership_events, duration)?,
        })
    }

    pub(super) fn sample_at(&self, time: f64) -> Vec<LayoutSample<'_, K>> {
        let membership = self.membership.sample_at(time).current;
        self.keys
            .iter()
            .filter_map(|key| {
                let present = membership.contains(&key.key);
                let presence = self
                    .timeline
                    .sample_at(&key.presence, time)
                    .expect("layout presence property must be compiled");
                if !present && presence == MotionState::at(0.0) {
                    return None;
                }
                Some(LayoutSample {
                    key: &key.key,
                    present,
                    x: self
                        .timeline
                        .sample_at(&key.x, time)
                        .expect("layout x property must be compiled"),
                    y: self
                        .timeline
                        .sample_at(&key.y, time)
                        .expect("layout y property must be compiled"),
                    presence,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use kinograph::state::TimedState;

    use super::{KeyedLayoutTrack, LayoutMotion, LayoutSample, LayoutSnapshot, LayoutTarget};

    fn snapshot(
        targets: impl IntoIterator<Item = (&'static str, f32, f32)>,
    ) -> LayoutSnapshot<String> {
        LayoutSnapshot::new(targets.into_iter().map(|(key, x, y)| LayoutTarget {
            key: key.to_owned(),
            x,
            y,
        }))
        .unwrap()
    }

    fn motion() -> LayoutMotion {
        LayoutMotion {
            position: kinograph::timeline::SpringProfile::new(0.5, 0.8, 0.0001, 0.0001),
            presence: kinograph::timeline::SpringProfile::new(0.3, 1.0, 0.0001, 0.0001),
        }
    }

    fn sample<'a>(
        track: &'a KeyedLayoutTrack<String>,
        key: &str,
        time: f64,
    ) -> LayoutSample<'a, String> {
        track
            .sample_at(time)
            .into_iter()
            .find(|sample| sample.key == key)
            .unwrap()
    }

    #[test]
    fn position_retargeting_preserves_interrupted_position_and_velocity() {
        let first_target = snapshot([("item", 100.0, 40.0)]);
        let uninterrupted = KeyedLayoutTrack::compile(
            snapshot([("item", 0.0, 0.0)]),
            [TimedState::new(0.1, first_target.clone())],
            motion(),
            2.0,
        )
        .unwrap();
        let interrupted = KeyedLayoutTrack::compile(
            snapshot([("item", 0.0, 0.0)]),
            [
                TimedState::new(0.1, first_target),
                TimedState::new(0.3, snapshot([("item", -20.0, 80.0)])),
            ],
            motion(),
            2.0,
        )
        .unwrap();

        let expected = sample(&uninterrupted, "item", 0.3);
        let actual = sample(&interrupted, "item", 0.3);
        assert_eq!(actual.x, expected.x);
        assert_eq!(actual.y, expected.y);
        assert!(actual.x.velocity.abs() > 0.001);
        assert!(actual.y.velocity.abs() > 0.001);
        assert!(sample(&interrupted, "item", 1.5).x.position < -19.9);
        assert!(sample(&interrupted, "item", 1.5).y.position > 79.9);
    }

    #[test]
    fn presence_supports_enter_exit_and_momentum_preserving_reentry() {
        let through_exit = KeyedLayoutTrack::compile(
            snapshot([]),
            [
                TimedState::new(0.1, snapshot([("item", 10.0, 20.0)])),
                TimedState::new(0.3, snapshot([])),
            ],
            motion(),
            2.0,
        )
        .unwrap();
        let reentered = KeyedLayoutTrack::compile(
            snapshot([]),
            [
                TimedState::new(0.1, snapshot([("item", 10.0, 20.0)])),
                TimedState::new(0.3, snapshot([])),
                TimedState::new(0.4, snapshot([("item", 10.0, 20.0)])),
                TimedState::new(1.5, snapshot([])),
            ],
            motion(),
            3.0,
        )
        .unwrap();

        assert!(through_exit.sample_at(0.0).is_empty());
        let entering = sample(&reentered, "item", 0.1);
        assert!(entering.present);
        assert_eq!(entering.presence, kinograph::motion::MotionState::at(0.0));
        assert!(sample(&reentered, "item", 0.2).presence.position > 0.0);

        let exiting = sample(&reentered, "item", 0.3);
        assert!(!exiting.present);
        assert!(exiting.presence.position > 0.0);
        let before_reentry = sample(&through_exit, "item", 0.4).presence;
        let at_reentry = sample(&reentered, "item", 0.4);
        assert!(at_reentry.present);
        assert_eq!(at_reentry.presence, before_reentry);
        assert!(at_reentry.presence.velocity < 0.0);
        assert_eq!(sample(&reentered, "item", 1.4).presence.position, 1.0);

        let final_exit = sample(&reentered, "item", 1.5);
        assert!(!final_exit.present);
        assert_eq!(final_exit.presence.position, 1.0);
        assert!(reentered.sample_at(3.0).is_empty());
    }

    #[test]
    fn repeated_snapshots_do_not_restart_active_motion() {
        let target = snapshot([("item", 100.0, 50.0)]);
        let once = KeyedLayoutTrack::compile(
            snapshot([("item", 0.0, 0.0)]),
            [TimedState::new(0.1, target.clone())],
            motion(),
            1.0,
        )
        .unwrap();
        let repeated = KeyedLayoutTrack::compile(
            snapshot([("item", 0.0, 0.0)]),
            [
                TimedState::new(0.1, target.clone()),
                TimedState::new(0.2, target),
            ],
            motion(),
            1.0,
        )
        .unwrap();

        assert_eq!(sample(&repeated, "item", 0.35), sample(&once, "item", 0.35));
    }

    #[test]
    fn samples_keep_chronological_first_appearance_order() {
        let track = KeyedLayoutTrack::compile(
            snapshot([("second", 20.0, 0.0), ("first", 10.0, 0.0)]),
            [
                TimedState::new(
                    0.1,
                    snapshot([
                        ("third", 30.0, 0.0),
                        ("first", 10.0, 0.0),
                        ("second", 20.0, 0.0),
                    ]),
                ),
                TimedState::new(0.5, snapshot([("third", 30.0, 0.0), ("first", 10.0, 0.0)])),
            ],
            motion(),
            2.0,
        )
        .unwrap();

        let during_exit = track
            .sample_at(0.5)
            .into_iter()
            .map(|sample| sample.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(during_exit, ["second", "first", "third"]);

        let settled = track
            .sample_at(2.0)
            .into_iter()
            .map(|sample| sample.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(settled, ["first", "third"]);
    }

    #[test]
    fn arbitrary_time_sampling_is_deterministic_and_history_independent() {
        let track = KeyedLayoutTrack::compile(
            snapshot([("item", 0.0, 0.0)]),
            [
                TimedState::new(0.2, snapshot([("item", 100.0, 50.0)])),
                TimedState::new(0.5, snapshot([])),
                TimedState::new(0.7, snapshot([("item", -30.0, 10.0)])),
            ],
            motion(),
            2.0,
        )
        .unwrap();

        let later = track.sample_at(0.85);
        let _earlier = track.sample_at(0.1);
        let repeated = track.sample_at(0.85);
        assert_eq!(later, repeated);
    }

    #[test]
    fn snapshots_and_track_times_are_validated() {
        let duplicate = LayoutSnapshot::new([
            LayoutTarget {
                key: "item",
                x: 0.0,
                y: 0.0,
            },
            LayoutTarget {
                key: "item",
                x: 1.0,
                y: 1.0,
            },
        ])
        .unwrap_err();
        assert!(duplicate.to_string().contains("duplicate key"));

        for (x, y) in [(f32::NAN, 0.0), (0.0, f32::INFINITY)] {
            let error = LayoutSnapshot::new([LayoutTarget { key: "item", x, y }]).unwrap_err();
            assert!(error.to_string().contains("coordinates must be finite"));
        }

        let invalid_time = KeyedLayoutTrack::compile(
            snapshot([]),
            [TimedState {
                at: f64::NAN,
                value: snapshot([]),
            }],
            motion(),
            1.0,
        )
        .err()
        .unwrap();
        assert!(invalid_time.to_string().contains("time must be finite"));
        assert!(KeyedLayoutTrack::compile(snapshot([]), [], motion(), f64::INFINITY).is_err());
    }

    #[test]
    fn zero_duration_appearances_do_not_seed_future_motion_or_order() {
        let track = KeyedLayoutTrack::compile(
            snapshot([("stable", 0.0, 0.0)]),
            [
                TimedState::new(
                    0.2,
                    snapshot([("transient", 10.0, 10.0), ("stable", 0.0, 0.0)]),
                ),
                TimedState::new(0.2, snapshot([("stable", 0.0, 0.0)])),
                TimedState::new(
                    0.5,
                    snapshot([("stable", 0.0, 0.0), ("transient", 100.0, 80.0)]),
                ),
            ],
            motion(),
            2.0,
        )
        .unwrap();

        assert!(
            track
                .sample_at(0.2)
                .iter()
                .all(|row| row.key != "transient")
        );
        let entering = sample(&track, "transient", 0.5);
        assert_eq!(entering.x, kinograph::motion::MotionState::at(100.0));
        assert_eq!(entering.y, kinograph::motion::MotionState::at(80.0));
        assert_eq!(
            track
                .sample_at(0.5)
                .iter()
                .map(|row| row.key.as_str())
                .collect::<Vec<_>>(),
            ["stable", "transient"]
        );
    }
}
