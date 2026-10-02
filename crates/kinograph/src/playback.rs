//! Interactive destinations over the same scalar tracks used by video export.
//! Navigation schedules spring retargets on a local, pausable clock and cancels
//! superseded unstarted writes. It never seeks backward or replaces in-flight state.
use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};

use crate::{
    motion::MotionState,
    plan::{PresentationStepPlan, ScenePlan, TrackEventPlan},
    timeline::{
        PropertyId, Retarget, RetargetMode, RetargetSchedule, ScheduleTime, SpringProfile, Timeline,
    },
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

pub use crate::timeline::StartDelay;

/// Diagnostic clock rates, not alternate spring profiles or export timing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackSpeed {
    #[default]
    Normal,
    Half,
    Quarter,
    Tenth,
}

impl PlaybackSpeed {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Half, Self::Quarter, Self::Tenth];
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "1x",
            Self::Half => "0.5x",
            Self::Quarter => "0.25x",
            Self::Tenth => "0.1x",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "1" | "1.0" => Ok(Self::Normal),
            "0.5" => Ok(Self::Half),
            "0.25" => Ok(Self::Quarter),
            "0.1" => Ok(Self::Tenth),
            _ => bail!("speed must be 1, 0.5, 0.25, or 0.1"),
        }
    }
    pub fn cycle(self, reverse: bool) -> Self {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap();
        Self::ALL[(i + if reverse { 3 } else { 1 }) % 4]
    }
    fn divisor(self) -> u32 {
        match self {
            Self::Normal => 1,
            Self::Half => 2,
            Self::Quarter => 4,
            Self::Tenth => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PendingStarts {
    pub count: usize,
    pub until_next: Option<Duration>,
}

pub struct Playback {
    steps: Vec<PresentationStepPlan>,
    destinations: Vec<Destination>,
    properties: Vec<PropertyId>,
    start_delays: Vec<Option<StartDelay>>,
    schedule: RetargetSchedule,
    index: usize,
    revision: u64,
    reduced_motion: bool,
    // A stopped clock retains the trajectory's velocity. Resuming changes only
    // its wall-clock anchor; a navigation retarget samples that full state.
    local_time: Duration,
    anchor: Option<Duration>,
    phase: PlaybackPhase,
    continuous: bool,
    speed: PlaybackSpeed,
    navigation_start: Duration,
}

impl Playback {
    /// `authored` has already resolved renderer-owned semantic target geometry.
    pub fn new(plan: &ScenePlan, authored: &Timeline, reduced_motion: bool) -> Result<Self> {
        Self::with_start_delays(
            plan,
            authored,
            reduced_motion,
            &HashMap::new(),
            &HashMap::new(),
        )
    }

    /// Renderer-generated dimensionless tracks can supply unit-aware fallback
    /// profiles. Authored spring profiles still take precedence.
    pub fn with_start_delays(
        plan: &ScenePlan,
        authored: &Timeline,
        reduced_motion: bool,
        defaults: &HashMap<String, SpringProfile>,
        delays: &HashMap<String, StartDelay>,
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
        for (id, delay) in delays {
            if !properties.iter().any(|p| p.as_str() == id)
                || !delay.from.is_finite()
                || !delay.to.is_finite()
                || delay.delay.as_secs_f64() > 60.
            {
                bail!("start delay needs a declared channel, finite poses, and at most 60 seconds");
            }
        }
        let start_delays = properties
            .iter()
            .map(|p| delays.get(p.as_str()).copied())
            .collect();
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
        let destinations = plan
            .presentation_steps
            .iter()
            .map(|step| {
                let profiles = plan
                    .continuous_channels
                    .iter()
                    .map(|channel| {
                        let springs = || {
                            channel
                                .events
                                .iter()
                                .filter(|event| matches!(event, TrackEventPlan::Spring { .. }))
                        };
                        let event = springs()
                            .rfind(|event| event.at_nanos() <= step.hold_nanos)
                            .or_else(|| springs().next());
                        event
                            .and_then(TrackEventPlan::spring_plan)
                            .map(|spring| spring.profile())
                            .unwrap_or_else(|| {
                                defaults.get(&channel.id).copied().unwrap_or_else(|| {
                                    SpringProfile::from_visual_duration(0.4, 0.0, 0.001, 0.001)
                                })
                            })
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
        let schedule = RetargetSchedule::new(initial)?;
        Ok(Self {
            steps: plan.presentation_steps.clone(),
            destinations,
            properties,
            start_delays,
            schedule,
            index: 0,
            revision: 0,
            reduced_motion,
            local_time: Duration::ZERO,
            anchor: None,
            phase: PlaybackPhase::Held,
            continuous: false,
            speed: PlaybackSpeed::Normal,
            navigation_start: Duration::ZERO,
        })
    }

    pub fn steps(&self) -> &[PresentationStepPlan] {
        &self.steps
    }
    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }
    pub fn timeline(&self) -> Arc<Timeline> {
        self.schedule.timeline().clone()
    }

    pub fn speed(&self) -> PlaybackSpeed {
        self.speed
    }
    pub fn navigation_start(&self) -> Duration {
        self.navigation_start
    }

    /// Reanchor at the current scene time. Position and scene-time velocity,
    /// pending start times, selected destination, and the Timeline stay intact.
    pub fn set_speed(&mut self, speed: PlaybackSpeed, now: Duration) -> bool {
        if self.speed == speed {
            return false;
        }
        self.local_time = self.time(now);
        if self.anchor.is_some() {
            self.anchor = Some(now);
        }
        self.speed = speed;
        self.revision += 1;
        true
    }

    /// Explicit diagnostic sampling, distinct from Previous destination navigation.
    /// Freeze after a 16.667ms scene-time step, independent of speed/display FPS.
    /// Backward inspection stops at the latest navigation boundary.
    pub fn step_frame(&mut self, backward: bool, now: Duration) -> bool {
        if self.reduced_motion {
            return false;
        }
        let time = self.time(now);
        let frame = Duration::from_nanos(16_666_667);
        self.local_time = if backward {
            time.saturating_sub(frame).max(self.navigation_start)
        } else {
            time.saturating_add(frame)
        };
        self.anchor = None;
        self.phase = PlaybackPhase::Paused;
        self.revision += 1;
        true
    }

    pub fn pending_starts(&self, at: Duration) -> PendingStarts {
        let mut pending = PendingStarts::default();
        for event in self.schedule.writes() {
            let due = Duration::from_secs_f64(event.at.seconds());
            if due > at {
                pending.count += 1;
                let remaining = due - at;
                pending.until_next = Some(
                    pending
                        .until_next
                        .map_or(remaining, |old| old.min(remaining)),
                );
            }
        }
        pending
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
        self.local_time
            .saturating_add(self.anchor.map_or(Duration::ZERO, |since| {
                now.saturating_sub(since) / self.speed.divisor()
            }))
    }

    fn at_rest(&self, time: Duration) -> bool {
        self.properties
            .iter()
            .zip(&self.destinations[self.index].values)
            .all(|(property, target)| {
                self.schedule
                    .timeline()
                    .sample_at(property, time.as_secs_f64())
                    == Some(MotionState::at(*target))
            })
    }

    fn go(&mut self, index: usize, replay: bool, now: Duration) -> bool {
        let Some(destination) = self.destinations.get(index) else {
            return false;
        };
        let time = self.time(now);
        let requests = self
            .properties
            .iter()
            .enumerate()
            .map(|(component, property)| Retarget {
                property: property.clone(),
                target: destination.values[component],
                profile: destination.profiles[component],
                delay: self.start_delays[component],
                mode: if self.reduced_motion {
                    RetargetMode::Immediate
                } else if replay {
                    RetargetMode::Replay {
                        from: destination.entry[component],
                    }
                } else {
                    RetargetMode::Animate
                },
            });
        self.schedule
            .retarget(ScheduleTime::Seconds(time.as_secs_f64()), requests, None)
            .expect("validated presentation events refer to declared channels on a finite clock");
        self.index = index;
        self.navigation_start = time;
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
    use crate::timeline::TimedEvent;
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
    fn staggered() -> Playback {
        let mut p = playback();
        p.start_delays[0] = Some(StartDelay {
            from: 0.,
            to: 1.,
            delay: Duration::from_millis(100),
        });
        p
    }

    #[test]
    fn slowing_the_clock_preserves_the_pose_velocity_and_timeline() {
        let mut p = playback();
        p.command(Next, seconds(0.));
        let before = state(&mut p, 0.2, "code.reveal");
        let timeline = p.timeline();
        assert!(p.set_speed(PlaybackSpeed::Quarter, seconds(0.2)));
        assert_eq!(before, state(&mut p, 0.2, "code.reveal"));
        assert!(Arc::ptr_eq(&timeline, &p.timeline()));
        assert_eq!(p.sample(seconds(0.6)).at_nanos, 300_000_000);
        assert_eq!(
            state(&mut p, 0.6, "code.reveal"),
            timeline
                .sample_at(&PropertyId::new("code.reveal"), 0.3)
                .unwrap()
        );
        let current = state(&mut p, 0.6, "code.reveal");
        p.command(Previous, seconds(0.6));
        assert_eq!(
            current,
            state(&mut p, 0.6, "code.reveal"),
            "slow reversal inherits scene-time velocity"
        );
        p.pause(seconds(0.8));
        let paused = p.sample(seconds(0.8));
        p.set_speed(PlaybackSpeed::Tenth, seconds(10.));
        assert_eq!(p.sample(seconds(50.)).at_nanos, paused.at_nanos);
        assert_eq!(p.sample(seconds(50.)).phase, PlaybackPhase::Paused);
    }

    #[test]
    fn slow_motion_scales_waits_and_frame_steps_do_not_retarget() {
        let mut p = staggered();
        p.set_speed(PlaybackSpeed::Quarter, Duration::ZERO);
        p.command(Next, Duration::ZERO);
        assert_eq!(p.sample(seconds(0.2)).at_nanos, 50_000_000);
        assert_eq!(state(&mut p, 0.2, "code.reveal"), MotionState::at(0.));
        assert_eq!(
            p.pending_starts(Duration::from_millis(50)),
            PendingStarts {
                count: 1,
                until_next: Some(Duration::from_millis(50))
            }
        );
        assert!(state(&mut p, 0.5, "code.reveal").position > 0.);
        p.pause(seconds(0.5));
        let timeline = p.timeline();
        let before = p.sample(seconds(0.5)).at_nanos;
        p.step_frame(false, seconds(10.));
        let next = p.sample(seconds(100.));
        assert_eq!(next.at_nanos, before + 16_666_667);
        assert_eq!(next.phase, PlaybackPhase::Paused);
        p.step_frame(true, seconds(101.));
        assert_eq!(p.sample(seconds(102.)).at_nanos, before);
        assert!(Arc::ptr_eq(&timeline, &p.timeline()));
        for _ in 0..20 {
            p.step_frame(true, seconds(200.));
        }
        assert_eq!(
            p.sample(seconds(200.)).at_nanos,
            0,
            "inspection cannot cross its navigation boundary"
        );
        p.set_reduced_motion(true, seconds(200.));
        assert!(!p.step_frame(false, seconds(201.)));
        assert_eq!(p.pending_starts(Duration::ZERO).count, 0);
    }

    #[test]
    fn diagnostic_rates_cycle_and_preserve_the_default_clock() {
        for speed in PlaybackSpeed::ALL {
            assert_eq!(speed.cycle(false).cycle(true), speed);
        }
        assert_eq!(PlaybackSpeed::default(), PlaybackSpeed::Normal);
        assert!(PlaybackSpeed::parse("0").is_err());
        assert!(PlaybackSpeed::parse("NaN").is_err());
        let mut p = playback();
        assert!(!p.set_speed(PlaybackSpeed::Normal, seconds(10.)));
        p.set_speed(PlaybackSpeed::Half, seconds(20.));
        assert_eq!(p.sample(seconds(30.)).phase, PlaybackPhase::Held);
        assert_eq!(p.sample(seconds(30.)).at_nanos, 0);
    }

    #[test]
    fn pending_entrances_are_cancelled_without_changing_executed_history() {
        let mut p = staggered();
        let id = PropertyId::new("code.reveal");
        p.command(Next, seconds(0.));
        let original = p.timeline();
        assert_eq!(p.sample(seconds(0.05)).phase, PlaybackPhase::Playing);
        assert_eq!(state(&mut p, 0.05, "code.reveal"), MotionState::at(0.));
        p.command(Previous, seconds(0.05));
        for at in [0.05, 0.11, 0.5, 2.] {
            assert_eq!(p.timeline().sample_at(&id, at), Some(MotionState::at(0.)));
        }
        assert!(
            original.sample_at(&id, 0.2).unwrap().position > 0.,
            "prior immutable revision retains its original schedule"
        );
    }

    #[test]
    fn unchanged_pending_words_keep_their_due_time_and_reversal_is_immediate_after_start() {
        let mut p = staggered();
        let id = PropertyId::new("code.reveal");
        p.command(Next, seconds(0.));
        let original = p.timeline();
        p.command(Next, seconds(0.04));
        for at in [0.04, 0.09, 0.12, 0.3] {
            assert_eq!(p.timeline().sample_at(&id, at), original.sample_at(&id, at));
        }
        p.command(First, seconds(0.13));
        let reversed = state(&mut p, 0.13, "code.reveal");
        assert_eq!(reversed, original.sample_at(&id, 0.13).unwrap());
        let before = state(&mut p, 0.15, "code.reveal");
        p.command(Next, seconds(0.15));
        assert_eq!(before, state(&mut p, 0.15, "code.reveal"));
        assert!(
            p.schedule
                .writes()
                .iter()
                .all(|write| write.property != id || write.at.seconds() <= 0.15),
            "moving words must redirect now, not coast toward a delayed reversal"
        );
    }

    #[test]
    fn stagger_pause_replay_reduced_motion_and_rapid_a_b_a_c_do_not_leak_starts() {
        let mut p = staggered();
        p.command(Next, seconds(0.));
        p.pause(seconds(0.04));
        assert_eq!(state(&mut p, 10., "code.reveal"), MotionState::at(0.));
        p.command(TogglePause, seconds(10.));
        assert_eq!(state(&mut p, 10.05, "code.reveal"), MotionState::at(0.));
        assert!(state(&mut p, 10.08, "code.reveal").position > 0.);
        p.command(Replay, seconds(10.1));
        assert_eq!(state(&mut p, 10.15, "code.reveal"), MotionState::at(0.));
        p.set_reduced_motion(true, seconds(10.15));
        assert_eq!(state(&mut p, 20., "code.reveal"), MotionState::at(1.));
        let mut p = staggered();
        p.command(Next, seconds(0.));
        p.command(Previous, seconds(0.04));
        p.command(Last, seconds(0.06));
        let id = PropertyId::new("code.reveal");
        assert_eq!(p.timeline().sample_at(&id, 0.14), Some(MotionState::at(0.)));
        let sample = p.timeline().sample_at(&id, 0.19).unwrap();
        assert!(sample.position > 0.);
        p.timeline().sample_at(&id, 4.);
        assert_eq!(p.timeline().sample_at(&id, 0.19), Some(sample));
    }
    fn seconds(n: f64) -> Duration {
        Duration::from_secs_f64(n)
    }
    fn state(playback: &mut Playback, now: f64, property: &str) -> MotionState {
        let sample = playback.sample(seconds(now));
        playback
            .timeline()
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
                    .timeline()
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
        let expected = playback.timeline().sample_at(&property, 0.25).unwrap();
        playback.timeline().sample_at(&property, 5.);
        playback.timeline().sample_at(&property, 0.05);
        assert_eq!(
            playback.timeline().sample_at(&property, 0.25).unwrap(),
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
                playback.timeline().sample_at(&x, at),
                Some(MotionState::at(1.))
            );
        }
        assert!(playback.timeline().sample_at(&y, 2.05).unwrap().position > 0.);
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
