//! Interactive destinations over the same scalar tracks used by video export.
//! Navigation appends spring retargets on a local, pausable clock. It never seeks
//! backward through the authored animation or replaces in-flight motion state.
use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};

use crate::{
    motion::MotionState,
    plan::{PresentationStepPlan, ScenePlan, TrackEventPlan},
    timeline::{PropertyId, SpringProfile, TimedEvent, Timeline},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackPhase {
    Held,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackCommand {
    Next,
    Previous,
    Replay,
    TogglePause,
    First,
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackSample {
    pub step_index: usize,
    /// Time on the local presentation clock, not the authored video clock.
    pub at_nanos: u64,
    pub phase: PlaybackPhase,
    pub revision: u64,
}

struct Destination {
    values: Vec<f32>,
    entry: Vec<f32>,
    profiles: Vec<SpringProfile>,
}

pub struct Playback {
    steps: Vec<PresentationStepPlan>,
    destinations: Vec<Destination>,
    properties: Vec<PropertyId>,
    initial: Vec<(PropertyId, f32)>,
    events: Vec<TimedEvent>,
    timeline: Arc<Timeline>,
    index: usize,
    revision: u64,
    reduced_motion: bool,
    // A stopped clock retains the trajectory's velocity. Resuming changes only
    // its wall-clock anchor; a navigation retarget samples that full state.
    local_time: Duration,
    anchor: Option<Duration>,
    phase: PlaybackPhase,
    continuous: bool,
}

impl Playback {
    /// `authored` has already resolved renderer-owned semantic target geometry.
    pub fn new(plan: &ScenePlan, authored: &Timeline, reduced_motion: bool) -> Result<Self> {
        Self::with_default_profiles(plan, authored, reduced_motion, &HashMap::new())
    }

    /// Renderer-generated dimensionless tracks can supply unit-aware fallback
    /// profiles. Authored spring profiles still take precedence.
    pub fn with_default_profiles(
        plan: &ScenePlan,
        authored: &Timeline,
        reduced_motion: bool,
        defaults: &HashMap<String, SpringProfile>,
    ) -> Result<Self> {
        plan.validate()?;
        if plan.presentation_steps.is_empty() {
            bail!(
                "scene '{}' has no presentation steps; use PlanBuilder::presentation_step",
                plan.id
            );
        }
        let properties = plan
            .continuous_channels
            .iter()
            .map(|channel| PropertyId::new(&channel.id))
            .collect::<Vec<_>>();
        let values_at = |nanos| {
            properties
                .iter()
                .map(|property| {
                    authored
                        .sample_at(property, nanos as f64 / 1_000_000_000.0)
                        .map(|state| state.position)
                        .with_context(|| {
                            format!("missing compiled channel '{}'", property.as_str())
                        })
                })
                .collect::<Result<Vec<_>>>()
        };
        let destinations =
            plan.presentation_steps
                .iter()
                .map(|step| {
                    let profiles =
                        plan.continuous_channels
                            .iter()
                            .map(|channel| {
                                let springs = || {
                                    channel.events.iter().filter(|event| {
                                        matches!(event, TrackEventPlan::Spring { .. })
                                    })
                                };
                                let event = springs()
                                    .rfind(|event| event.at_nanos() <= step.hold_nanos)
                                    .or_else(|| springs().next());
                                match event {
                                    Some(TrackEventPlan::Spring {
                                        response_seconds,
                                        damping_ratio,
                                        position_threshold,
                                        velocity_threshold,
                                        ..
                                    }) => SpringProfile::new(
                                        *response_seconds,
                                        *damping_ratio,
                                        *position_threshold,
                                        *velocity_threshold,
                                    ),
                                    _ => defaults.get(&channel.id).copied().unwrap_or_else(|| {
                                        SpringProfile::from_visual_duration(0.4, 0.0, 0.001, 0.001)
                                    }),
                                }
                            })
                            .collect();
                    Ok(Destination {
                        values: values_at(step.hold_nanos)?,
                        entry: values_at(step.start_nanos)?,
                        profiles,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
        let initial = properties
            .iter()
            .cloned()
            .zip(destinations[0].values.iter().copied())
            .collect::<Vec<_>>();
        let timeline = Arc::new(Timeline::compile_events(initial.clone(), [], 0.0)?);
        Ok(Self {
            steps: plan.presentation_steps.clone(),
            destinations,
            properties,
            initial,
            events: Vec::new(),
            timeline,
            index: 0,
            revision: 0,
            reduced_motion,
            local_time: Duration::ZERO,
            anchor: None,
            phase: PlaybackPhase::Held,
            continuous: false,
        })
    }

    pub fn steps(&self) -> &[PresentationStepPlan] {
        &self.steps
    }
    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }
    pub fn timeline(&self) -> Arc<Timeline> {
        self.timeline.clone()
    }

    /// Observe the local clock and stop it once all channels are exactly at rest.
    /// Property Track evaluation itself remains deterministic and arbitrary-time.
    pub fn sample(&mut self, now: Duration) -> PlaybackSample {
        let time = self.time(now);
        if self.phase == PlaybackPhase::Playing && !self.continuous && self.at_rest(time) {
            self.local_time = time;
            self.anchor = None;
            self.phase = PlaybackPhase::Held;
        }
        PlaybackSample {
            step_index: self.index,
            at_nanos: time.as_nanos().min(u128::from(u64::MAX)) as u64,
            phase: self.phase,
            revision: self.revision,
        }
    }

    pub fn command(&mut self, command: PlaybackCommand, now: Duration) -> bool {
        match command {
            PlaybackCommand::Next => self.go(self.index + 1, false, now),
            PlaybackCommand::Previous => self
                .index
                .checked_sub(1)
                .is_some_and(|index| self.go(index, false, now)),
            PlaybackCommand::First => self.go(0, false, now),
            PlaybackCommand::Last => self.go(self.steps.len() - 1, false, now),
            // Replay is an explicit restart. Ordinary navigation never resets.
            PlaybackCommand::Replay => self.go(self.index, true, now),
            PlaybackCommand::TogglePause => match self.sample(now).phase {
                PlaybackPhase::Playing => self.pause(now),
                PlaybackPhase::Paused => {
                    self.anchor = Some(now);
                    self.phase = PlaybackPhase::Playing;
                    self.revision += 1;
                    true
                }
                PlaybackPhase::Held => false,
            },
        }
    }

    /// Renderer-owned ambient motion (for example a running Task) can keep the
    /// local clock alive after its scalar destinations settle. Pause still wins.
    pub fn keep_running(&mut self, enabled: bool, now: Duration) {
        self.continuous = enabled;
        if enabled && self.phase == PlaybackPhase::Held {
            self.anchor = Some(now);
            self.phase = PlaybackPhase::Playing;
        }
    }

    pub fn pause(&mut self, now: Duration) -> bool {
        if self.sample(now).phase != PlaybackPhase::Playing {
            return false;
        }
        self.local_time = self.time(now);
        self.anchor = None;
        self.phase = PlaybackPhase::Paused;
        self.revision += 1;
        true
    }

    pub fn set_reduced_motion(&mut self, enabled: bool, now: Duration) {
        self.reduced_motion = enabled;
        if enabled {
            self.go(self.index, false, now);
        }
    }

    fn time(&self, now: Duration) -> Duration {
        self.local_time.saturating_add(
            self.anchor
                .map_or(Duration::ZERO, |since| now.saturating_sub(since)),
        )
    }

    fn at_rest(&self, time: Duration) -> bool {
        self.properties
            .iter()
            .zip(&self.destinations[self.index].values)
            .all(|(property, target)| {
                self.timeline.sample_at(property, time.as_secs_f64())
                    == Some(MotionState::at(*target))
            })
    }

    fn go(&mut self, index: usize, replay: bool, now: Duration) -> bool {
        let Some(destination) = self.destinations.get(index) else {
            return false;
        };
        let time = self.time(now);
        for (component, property) in self.properties.iter().enumerate() {
            let target = destination.values[component];
            if self.reduced_motion {
                self.events.push(TimedEvent::set(
                    time.as_secs_f64(),
                    property.clone(),
                    target,
                ));
            } else if replay {
                self.events.push(TimedEvent::set(
                    time.as_secs_f64(),
                    property.clone(),
                    destination.entry[component],
                ));
                self.events.push(TimedEvent::spring(
                    time.as_secs_f64(),
                    property.clone(),
                    target,
                    destination.profiles[component],
                ));
            } else if target != self.destinations[self.index].values[component] {
                // The shared Timeline compiler starts from sampled position AND
                // velocity. Unchanged destinations keep their existing trajectory.
                self.events.push(TimedEvent::spring(
                    time.as_secs_f64(),
                    property.clone(),
                    target,
                    destination.profiles[component],
                ));
            }
        }
        self.timeline = Arc::new(
            Timeline::compile_events(
                self.initial.clone(),
                self.events.clone(),
                time.as_secs_f64(),
            )
            .expect("validated presentation events refer to declared channels on a finite clock"),
        );
        self.index = index;
        self.local_time = time;
        self.anchor = Some(now);
        self.phase = PlaybackPhase::Playing;
        self.revision += 1;
        self.sample(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::PlanBuilder;
    use PlaybackCommand::*;

    fn playback() -> Playback {
        let mut plan = PlanBuilder::new("demo", 20_000_000_000);
        let actor = plan.actor("code", "editor", ()).unwrap();
        let x = plan.continuous(&actor, "reveal", 0.0);
        let y = plan.continuous(&actor, "second", 0.0);
        plan.spring(&x, 1_000_000_000, 1.0, 0.4, 0.0);
        plan.spring(&y, 5_000_000_000, 1.0, 0.4, 0.0);
        plan.presentation_step("initial", "Initial", 0, 0);
        plan.presentation_step("reveal", "Reveal", 1_000_000_000, 3_000_000_000);
        plan.presentation_step("second", "Second", 5_000_000_000, 7_000_000_000);
        let authored = Timeline::compile_events(
            [
                (PropertyId::new(x.id()), 0.0),
                (PropertyId::new(y.id()), 0.0),
            ],
            [
                TimedEvent::spring(1.0, PropertyId::new(x.id()), 1.0, profile()),
                TimedEvent::spring(5.0, PropertyId::new(y.id()), 1.0, profile()),
            ],
            20.0,
        )
        .unwrap();
        Playback::new(&plan.finish().unwrap(), &authored, false).unwrap()
    }
    fn profile() -> SpringProfile {
        SpringProfile::from_visual_duration(0.4, 0.0, 0.001, 0.001)
    }
    fn seconds(n: f64) -> Duration {
        Duration::from_secs_f64(n)
    }
    fn state(playback: &mut Playback, now: f64, property: &str) -> MotionState {
        let sample = playback.sample(seconds(now));
        playback
            .timeline
            .sample_at(
                &PropertyId::new(property),
                sample.at_nanos as f64 / 1_000_000_000.0,
            )
            .unwrap()
    }

    #[test]
    fn reversing_in_flight_preserves_position_and_velocity_then_returns_to_zero() {
        let mut playback = playback();
        playback.command(Next, seconds(0.));
        let before = state(&mut playback, 0.15, "code.reveal");
        assert!(before.position > 0. && before.position < 1. && before.velocity > 0.);
        playback.command(Previous, seconds(0.15));
        assert_eq!(state(&mut playback, 0.15, "code.reveal"), before);
        assert!(state(&mut playback, 0.45, "code.reveal").velocity < 0.);
        assert_eq!(state(&mut playback, 2., "code.reveal"), MotionState::at(0.));
    }

    #[test]
    fn rapid_next_retargets_immediately_without_restarting_unchanged_channels() {
        let mut playback = playback();
        playback.command(Next, seconds(0.));
        let original = playback.timeline();
        playback.command(Next, seconds(0.1));
        assert_eq!(playback.sample(seconds(0.1)).step_index, 2);
        for time in [0.1, 0.2, 0.5] {
            assert_eq!(
                playback
                    .timeline
                    .sample_at(&PropertyId::new("code.reveal"), time),
                original.sample_at(&PropertyId::new("code.reveal"), time)
            );
        }
        assert!(state(&mut playback, 0.3, "code.second").position > 0.);
    }

    #[test]
    fn pause_freezes_time_and_resume_preserves_the_trajectory() {
        let mut playback = playback();
        playback.command(Next, seconds(100.));
        let before = state(&mut playback, 100.2, "code.reveal");
        playback.pause(seconds(100.2));
        assert_eq!(state(&mut playback, 500., "code.reveal"), before);
        playback.command(TogglePause, seconds(500.));
        assert_eq!(state(&mut playback, 500., "code.reveal"), before);
        assert_eq!(
            state(&mut playback, 502., "code.reveal"),
            MotionState::at(1.)
        );
        let held = playback.sample(seconds(502.));
        assert_eq!(playback.sample(seconds(600.)), held);
    }

    #[test]
    fn trajectories_can_be_sampled_out_of_order_after_multiple_interruptions() {
        let mut playback = playback();
        playback.command(Next, seconds(0.));
        playback.command(Previous, seconds(0.12));
        playback.command(Next, seconds(0.20));
        let property = PropertyId::new("code.reveal");
        let expected = playback.timeline.sample_at(&property, 0.25).unwrap();
        playback.timeline.sample_at(&property, 5.);
        playback.timeline.sample_at(&property, 0.05);
        assert_eq!(
            playback.timeline.sample_at(&property, 0.25).unwrap(),
            expected
        );
    }

    #[test]
    fn replay_is_explicit_and_reduced_motion_snaps_to_the_destination() {
        let mut playback = playback();
        assert!(!playback.command(Previous, seconds(0.)));
        playback.command(Next, seconds(0.));
        state(&mut playback, 2., "code.reveal");
        playback.command(Replay, seconds(3.));
        assert_eq!(state(&mut playback, 3., "code.reveal"), MotionState::at(0.));
        playback.set_reduced_motion(true, seconds(3.1));
        assert_eq!(
            state(&mut playback, 3.1, "code.reveal"),
            MotionState::at(1.)
        );
        playback.command(First, seconds(4.));
        assert_eq!(state(&mut playback, 4., "code.reveal"), MotionState::at(0.));
        playback.command(Last, seconds(5.));
        assert!(!playback.command(Next, seconds(5.)));
    }

    #[test]
    fn held_channels_cannot_wake_when_an_unrelated_step_starts() {
        let mut playback = playback();
        let x = PropertyId::new("code.reveal");
        let y = PropertyId::new("code.second");
        playback.destinations[1].profiles[0] = SpringProfile::new(0.5, 0.8, 0.02, 0.05);
        playback.command(Next, seconds(0.));
        // This momentary threshold crossing used to freeze the local clock.
        assert_eq!(playback.sample(seconds(0.41)).phase, PlaybackPhase::Playing);
        assert_eq!(playback.sample(seconds(2.)).phase, PlaybackPhase::Held);
        playback.command(Next, seconds(10.));
        for index in 0..240 {
            let at = 2.0 + f64::from(index) / 120.0;
            assert_eq!(
                playback.timeline.sample_at(&x, at),
                Some(MotionState::at(1.))
            );
        }
        assert!(playback.timeline.sample_at(&y, 2.05).unwrap().position > 0.);
    }

    #[test]
    fn tiny_moves_do_not_falsely_settle_before_velocity_peaks() {
        let id = PropertyId::new("tiny");
        let timeline = Timeline::compile_events(
            [(id.clone(), 0.)],
            [TimedEvent::spring(0., id.clone(), 0.0009, profile())],
            2.,
        )
        .unwrap();
        assert_eq!(timeline.sample_at(&id, 0.), Some(MotionState::at(0.)));
        assert!(timeline.sample_at(&id, 0.05).unwrap().velocity > 0.001);
        let mut settled = false;
        for index in 1..2000 {
            let at = f64::from(index) / 1000.;
            let state = timeline.sample_at(&id, at).unwrap();
            if settled {
                assert_eq!(state, MotionState::at(0.0009));
            }
            settled |= state == MotionState::at(0.0009);
        }
        assert!(settled);
        assert!(timeline.sample_at(&id, 0.05).unwrap().velocity > 0.001);
    }
}
