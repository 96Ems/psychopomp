use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::{
    math::easing::Ease,
    motion::{MotionState, Spring},
};

mod retarget;
pub use retarget::{
    Retarget, RetargetMode, RetargetSchedule, ScheduleTime, ScheduledWrite, StartDelay,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PropertyId(String);

impl PropertyId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpringProfile {
    spring: Spring,
    position_threshold: f32,
    velocity_threshold: f32,
}

impl SpringProfile {
    pub fn new(
        response_seconds: f32,
        damping_ratio: f32,
        position_threshold: f32,
        velocity_threshold: f32,
    ) -> Self {
        assert!(
            position_threshold.is_finite() && position_threshold > 0.0,
            "position threshold must be positive"
        );
        assert!(
            velocity_threshold.is_finite() && velocity_threshold > 0.0,
            "velocity threshold must be positive"
        );
        Self {
            spring: Spring::new(response_seconds, damping_ratio),
            position_threshold,
            velocity_threshold,
        }
    }

    /// Matches Motion's `visualDuration` and `bounce` spring parameterization.
    pub fn from_visual_duration(
        visual_duration_seconds: f32,
        bounce: f32,
        position_threshold: f32,
        velocity_threshold: f32,
    ) -> Self {
        assert!(
            visual_duration_seconds > 0.0,
            "visual duration must be positive"
        );
        assert!((0.0..1.0).contains(&bounce), "bounce must be in [0, 1)");
        Self::new(
            visual_duration_seconds * 1.2,
            1.0 - bounce,
            position_threshold,
            velocity_threshold,
        )
    }

    pub fn advance_time(self) -> f32 {
        const STEP: f32 = 1.0 / 240.0;
        const LIMIT: f32 = 60.0;

        let settled = |time| {
            let state = self.spring.sample(MotionState::at(0.0), 1.0, time);
            (state.position - 1.0).abs() <= self.position_threshold
                && state.velocity.abs() <= self.velocity_threshold
        };

        let mut upper = STEP;
        while upper < LIMIT && !settled(upper) {
            upper += STEP;
        }
        assert!(upper < LIMIT, "spring did not reach its advance threshold");

        let mut lower = upper - STEP;
        for _ in 0..12 {
            let middle = (lower + upper) * 0.5;
            if settled(middle) {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        upper
    }
}

#[derive(Clone, Debug)]
pub enum Animation {
    Set {
        property: PropertyId,
        value: f32,
    },
    Spring {
        property: PropertyId,
        target: f32,
        profile: SpringProfile,
    },
    /// Moves from wherever the property is to `target` along `curve` over
    /// `seconds`, independent of spring physics.
    Ease {
        property: PropertyId,
        target: f32,
        seconds: f32,
        curve: Ease,
    },
    Sequence(Vec<Animation>),
    Parallel(Vec<Animation>),
    Delay {
        seconds: f32,
        animation: Box<Animation>,
    },
    Hold(f32),
}

#[derive(Clone, Debug)]
pub struct TimedEvent {
    at: f64,
    animation: Animation,
}

impl TimedEvent {
    pub fn set(at: f64, property: PropertyId, value: f32) -> Self {
        Self::new(at, Animation::set(property, value))
    }

    pub fn spring(at: f64, property: PropertyId, target: f32, profile: SpringProfile) -> Self {
        Self::new(at, Animation::spring(property, target, profile))
    }

    pub fn ease(at: f64, property: PropertyId, target: f32, seconds: f32, curve: Ease) -> Self {
        Self::new(at, Animation::ease(property, target, seconds, curve))
    }

    fn new(at: f64, animation: Animation) -> Self {
        assert!(
            at.is_finite() && at >= 0.0,
            "event time must be finite and non-negative"
        );
        Self { at, animation }
    }

    pub fn at(&self) -> f64 {
        self.at
    }

    pub fn is_spring(&self) -> bool {
        matches!(self.animation, Animation::Spring { .. })
    }
}

impl Animation {
    pub fn set(property: PropertyId, value: f32) -> Self {
        Self::Set { property, value }
    }

    pub fn spring(property: PropertyId, target: f32, profile: SpringProfile) -> Self {
        Self::Spring {
            property,
            target,
            profile,
        }
    }

    pub fn ease(property: PropertyId, target: f32, seconds: f32, curve: Ease) -> Self {
        Self::Ease {
            property,
            target,
            seconds,
            curve,
        }
    }

    pub fn sequence(animations: impl IntoIterator<Item = Animation>) -> Self {
        Self::Sequence(animations.into_iter().collect())
    }

    pub fn parallel(animations: impl IntoIterator<Item = Animation>) -> Self {
        Self::Parallel(animations.into_iter().collect())
    }

    pub fn delay(seconds: f32, animation: Animation) -> Self {
        assert!(seconds >= 0.0, "animation delay must be non-negative");
        Self::Delay {
            seconds,
            animation: Box::new(animation),
        }
    }

    pub fn hold(seconds: f32) -> Self {
        assert!(seconds >= 0.0, "hold duration must be non-negative");
        Self::Hold(seconds)
    }

    pub fn duration(&self) -> f32 {
        match self {
            Self::Set { .. } => 0.0,
            Self::Spring { profile, .. } => profile.advance_time(),
            Self::Ease { seconds, .. } => *seconds,
            Self::Sequence(animations) => animations.iter().map(Self::duration).sum(),
            Self::Parallel(animations) => animations.iter().map(Self::duration).fold(0.0, f32::max),
            Self::Delay { seconds, animation } => seconds + animation.duration(),
            Self::Hold(seconds) => *seconds,
        }
    }

    fn leaves_at<'a>(&'a self, start: f32, leaves: &mut Vec<(f32, &'a Self)>) {
        match self {
            Self::Set { .. } | Self::Spring { .. } | Self::Ease { .. } => {
                leaves.push((start, self))
            }
            Self::Sequence(animations) => {
                let mut cursor = start;
                for animation in animations {
                    animation.leaves_at(cursor, leaves);
                    cursor += animation.duration();
                }
            }
            Self::Parallel(animations) => {
                for animation in animations {
                    animation.leaves_at(start, leaves);
                }
            }
            Self::Delay { seconds, animation } => {
                animation.leaves_at(start + seconds, leaves);
            }
            Self::Hold(_) => {}
        }
    }

    fn validate_leaves(leaves: &[(f32, &Self)]) -> Result<()> {
        for (index, (start, leaf)) in leaves.iter().enumerate() {
            let (Self::Set { property, .. }
            | Self::Spring { property, .. }
            | Self::Ease { property, .. }) = leaf
            else {
                unreachable!("only animation leaves are validated");
            };
            if leaves[..index].iter().any(|(other_start, other)| {
                matches!(other,
                    Self::Set { property: other, .. }
                        | Self::Spring { property: other, .. }
                        | Self::Ease { property: other, .. }
                    if other == property && other_start == start)
            }) {
                bail!(
                    "parallel animations both write property '{}' at {start:.3}s",
                    property.as_str()
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
enum SegmentKind {
    Set,
    Spring {
        spring: Spring,
        target: f32,
        settled_after: f64,
    },
    Ease {
        target: f32,
        seconds: f64,
        curve: Ease,
    },
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    start: f64,
    initial: MotionState,
    kind: SegmentKind,
}

impl Segment {
    fn sample(self, time: f64) -> MotionState {
        match self.kind {
            SegmentKind::Set => self.initial,
            SegmentKind::Spring {
                spring,
                target,
                settled_after,
            } => {
                let elapsed = time - self.start;
                if elapsed <= 0.0 {
                    self.initial
                } else if elapsed >= settled_after {
                    MotionState::at(target)
                } else {
                    spring.sample(self.initial, target, elapsed as f32)
                }
            }
            SegmentKind::Ease {
                target,
                seconds,
                curve,
            } => {
                let elapsed = time - self.start;
                if elapsed >= seconds {
                    return MotionState::at(target);
                }
                let progress = (elapsed.max(0.0) / seconds) as f32;
                let span = target - self.initial.position;
                MotionState {
                    position: self.initial.position + span * curve.sample(progress),
                    velocity: span * curve.slope(progress) / seconds as f32,
                }
            }
        }
    }
}

pub struct Timeline {
    tracks: HashMap<PropertyId, Vec<Segment>>,
    duration: f64,
}

impl Timeline {
    pub fn compile(
        initial_values: impl IntoIterator<Item = (PropertyId, f32)>,
        animation: &Animation,
    ) -> Result<Self> {
        let mut leaves = Vec::new();
        animation.leaves_at(0.0, &mut leaves);
        // Conflicts are diagnosed in authored order, before initial values.
        Animation::validate_leaves(&leaves)?;
        let mut timeline =
            Self::with_initial_values(initial_values, f64::from(animation.duration()))?;
        leaves.sort_by(|(left, _), (right, _)| left.total_cmp(right));
        for (start, leaf) in leaves {
            timeline.compile_leaf(leaf, f64::from(start))?;
        }
        Ok(timeline)
    }

    /// Compiles ordered writes authored on an explicit scene clock.
    ///
    /// Equal timestamps preserve input order, allowing imported formats to
    /// express a deterministic set followed by a spring retarget. This is a
    /// lowering interface; relative Rust choreography should continue to use
    /// `Animation`.
    pub fn compile_events(
        initial_values: impl IntoIterator<Item = (PropertyId, f32)>,
        events: impl IntoIterator<Item = TimedEvent>,
        duration: f64,
    ) -> Result<Self> {
        if !duration.is_finite() || duration < 0.0 {
            bail!("timeline duration must be finite and non-negative");
        }
        let mut timeline = Self::with_initial_values(initial_values, duration)?;
        let mut events = events.into_iter().enumerate().collect::<Vec<_>>();
        events.sort_by(|(left_index, left), (right_index, right)| {
            left.at
                .total_cmp(&right.at)
                .then(left_index.cmp(right_index))
        });
        for (_, event) in events {
            if event.at > duration {
                bail!(
                    "timeline event at {:.3}s exceeds duration {duration:.3}s",
                    event.at
                );
            }
            timeline.compile_leaf(&event.animation, event.at)?;
        }
        Ok(timeline)
    }

    pub fn duration(&self) -> f32 {
        self.duration as f32
    }

    pub(crate) fn compile_with_duration(
        initial_values: impl IntoIterator<Item = (PropertyId, f32)>,
        animation: &Animation,
        duration: f32,
    ) -> Result<Self> {
        let mut timeline = Self::compile(initial_values, animation)?;
        timeline.duration = f64::from(duration);
        Ok(timeline)
    }

    pub fn sample(&self, property: &PropertyId, time: f32) -> Option<MotionState> {
        self.sample_at(property, f64::from(time))
    }

    pub fn sample_at(&self, property: &PropertyId, time: f64) -> Option<MotionState> {
        let track = self.tracks.get(property)?;
        let time = time.max(0.0);
        let segment = track.iter().rev().find(|segment| segment.start <= time)?;
        Some(segment.sample(time))
    }

    fn compile_leaf(&mut self, animation: &Animation, start: f64) -> Result<()> {
        match animation {
            Animation::Set { property, value } => {
                if !value.is_finite() {
                    bail!("property '{}' set value must be finite", property.as_str());
                }
                self.push_segment(
                    property,
                    Segment {
                        start,
                        initial: MotionState::at(*value),
                        kind: SegmentKind::Set,
                    },
                );
            }
            Animation::Spring {
                property,
                target,
                profile,
            } => {
                if !target.is_finite() {
                    bail!(
                        "property '{}' spring target must be finite",
                        property.as_str()
                    );
                }
                let initial = self.sample_at(property, start).ok_or_else(|| {
                    anyhow::anyhow!("property '{}' has no initial value", property.as_str())
                })?;
                if !initial.position.is_finite() || !initial.velocity.is_finite() {
                    bail!(
                        "property '{}' has non-finite motion at retarget",
                        property.as_str()
                    );
                }
                self.push_segment(
                    property,
                    Segment {
                        start,
                        initial,
                        kind: SegmentKind::Spring {
                            spring: profile.spring,
                            target: *target,
                            settled_after: profile.spring.settling_time(
                                initial,
                                *target,
                                profile.position_threshold,
                                profile.velocity_threshold,
                            ),
                        },
                    },
                );
            }
            Animation::Ease {
                property,
                target,
                seconds,
                curve,
            } => {
                if !target.is_finite()
                    || !seconds.is_finite()
                    || *seconds <= 0.0
                    || !curve.is_valid()
                {
                    bail!(
                        "property '{}' ease needs a finite target, a positive duration, and a valid curve",
                        property.as_str()
                    );
                }
                let initial = self.sample_at(property, start).ok_or_else(|| {
                    anyhow::anyhow!("property '{}' has no initial value", property.as_str())
                })?;
                self.push_segment(
                    property,
                    Segment {
                        start,
                        initial,
                        kind: SegmentKind::Ease {
                            target: *target,
                            seconds: f64::from(*seconds),
                            curve: *curve,
                        },
                    },
                );
            }
            Animation::Sequence(_)
            | Animation::Parallel(_)
            | Animation::Delay { .. }
            | Animation::Hold(_) => unreachable!("only animation leaves are compiled"),
        }
        Ok(())
    }

    fn with_initial_values(
        initial_values: impl IntoIterator<Item = (PropertyId, f32)>,
        duration: f64,
    ) -> Result<Self> {
        let mut tracks = HashMap::new();
        for (property, value) in initial_values {
            if !value.is_finite() {
                bail!(
                    "property '{}' initial value must be finite",
                    property.as_str()
                );
            }
            let id = property.as_str().to_owned();
            if tracks
                .insert(
                    property,
                    vec![Segment {
                        start: 0.0,
                        initial: MotionState::at(value),
                        kind: SegmentKind::Set,
                    }],
                )
                .is_some()
            {
                bail!("property '{id}' has more than one initial value");
            }
        }
        Ok(Self { tracks, duration })
    }

    fn push_segment(&mut self, property: &PropertyId, segment: Segment) {
        let track = self.tracks.entry(property.clone()).or_default();
        let insertion = track.partition_point(|existing| existing.start <= segment.start);
        track.insert(insertion, segment);
    }
}

#[cfg(test)]
mod tests {
    use super::{Animation, PropertyId, SpringProfile, TimedEvent, Timeline};
    use crate::math::easing::Ease;

    #[test]
    fn eased_moves_follow_their_curve_and_hand_velocity_to_a_spring() {
        let x = PropertyId::new("beam.draw");
        let timeline = Timeline::compile_events(
            [(x.clone(), 0.0)],
            [
                TimedEvent::ease(1.0, x.clone(), 1.0, 0.5, Ease::Decelerate(0.4)),
                TimedEvent::spring(1.25, x.clone(), 0.2, profile()),
            ],
            4.0,
        )
        .unwrap();
        assert_eq!(timeline.sample_at(&x, 0.5).unwrap().position, 0.0);
        let launch = timeline.sample_at(&x, 1.0).unwrap();
        assert_eq!(launch.position, 0.0);
        assert!(
            (launch.velocity - 3.2).abs() < 1e-5,
            "fast launch: 1.6 x the average speed"
        );
        let middle = timeline.sample_at(&x, 1.2499).unwrap();
        let retarget = timeline.sample_at(&x, 1.25).unwrap();
        assert!((middle.position - retarget.position).abs() < 1e-3);
        assert!(
            (middle.velocity - retarget.velocity).abs() < 1e-2,
            "the spring inherits velocity"
        );
        let settled = Timeline::compile_events(
            [(x.clone(), 0.0)],
            [TimedEvent::ease(0.0, x.clone(), 1.0, 0.5, Ease::CubicOut)],
            1.0,
        )
        .unwrap();
        assert_eq!(
            settled.sample_at(&x, 0.5).unwrap(),
            crate::motion::MotionState::at(1.0)
        );
        assert!(
            Timeline::compile_events(
                [(x.clone(), 0.0)],
                [TimedEvent::ease(0.0, x, 1.0, 0.0, Ease::Linear)],
                1.0,
            )
            .is_err(),
            "an ease needs a positive duration"
        );
    }

    fn profile() -> SpringProfile {
        SpringProfile::new(0.5, 0.8, 0.02, 0.05)
    }

    #[test]
    fn composition_derives_duration() {
        let x = PropertyId::new("panel.x");
        let spring_duration = profile().advance_time();
        let animation = Animation::sequence([
            Animation::parallel([
                Animation::spring(x.clone(), 1.0, profile()),
                Animation::hold(0.2),
            ]),
            Animation::delay(0.1, Animation::set(x, 2.0)),
        ]);

        assert!((animation.duration() - (spring_duration + 0.1)).abs() < 0.0001);
    }

    #[test]
    fn parallel_writes_to_the_same_property_are_rejected() {
        let x = PropertyId::new("panel.x");
        let animation = Animation::parallel([
            Animation::spring(x.clone(), 1.0, profile()),
            Animation::spring(x, 2.0, profile()),
        ]);

        let error = Timeline::compile([], &animation).err().unwrap();
        assert!(error.to_string().contains("both write property 'panel.x'"));
    }

    #[test]
    fn relative_write_conflicts_use_authored_order_before_initial_validation() {
        let late = PropertyId::new("late");
        let early = PropertyId::new("early");
        let animation = Animation::parallel([
            Animation::delay(
                2.0,
                Animation::parallel([
                    Animation::spring(late.clone(), 1.0, profile()),
                    Animation::set(late.clone(), 2.0),
                ]),
            ),
            Animation::sequence([
                Animation::hold(0.5),
                Animation::parallel([
                    Animation::set(early.clone(), 1.0),
                    Animation::spring(early, 2.0, profile()),
                ]),
            ]),
        ]);
        for initial in [
            Vec::new(),
            vec![(late.clone(), f32::NAN)],
            vec![(late.clone(), 0.0), (late.clone(), 1.0)],
        ] {
            assert_eq!(
                Timeline::compile(initial, &animation)
                    .err()
                    .unwrap()
                    .to_string(),
                "parallel animations both write property 'late' at 2.000s"
            );
        }
        let initial = std::iter::from_fn(|| -> Option<(PropertyId, f32)> {
            panic!("conflict validation must not consume initial values")
        });
        assert!(Timeline::compile(initial, &animation).is_err());
    }

    #[test]
    fn initial_validation_still_precedes_sorted_leaf_validation() {
        let initial = PropertyId::new("initial");
        let animation = Animation::parallel([
            Animation::delay(2.0, Animation::set(PropertyId::new("bad-set"), f32::NAN)),
            Animation::delay(
                0.5,
                Animation::spring(PropertyId::new("missing"), 1.0, profile()),
            ),
        ]);
        for (values, expected) in [
            (
                vec![(initial.clone(), f32::NAN)],
                "property 'initial' initial value must be finite",
            ),
            (
                vec![(initial.clone(), 0.0), (initial, 1.0)],
                "property 'initial' has more than one initial value",
            ),
            (Vec::new(), "property 'missing' has no initial value"),
        ] {
            assert_eq!(
                Timeline::compile(values, &animation)
                    .err()
                    .unwrap()
                    .to_string(),
                expected
            );
        }
    }

    #[test]
    fn relative_sequence_rounds_in_f32_before_widening_event_time() {
        let marker = PropertyId::new("marker");
        let timeline = Timeline::compile(
            [(marker.clone(), 0.0)],
            &Animation::sequence([
                Animation::hold(0.1),
                Animation::hold(0.2),
                Animation::set(marker.clone(), 1.0),
            ]),
        )
        .unwrap();
        let widened_operands_sum = f64::from(0.1_f32) + f64::from(0.2_f32);
        let rounded_boundary = f64::from(0.1_f32 + 0.2_f32);
        let between = (widened_operands_sum + rounded_boundary) * 0.5;
        assert!(widened_operands_sum < between && between < rounded_boundary);
        for time in [widened_operands_sum, between] {
            assert_eq!(
                timeline.sample_at(&marker, time),
                Some(crate::motion::MotionState::at(0.0))
            );
        }
        assert_eq!(
            timeline.sample_at(&marker, rounded_boundary),
            Some(crate::motion::MotionState::at(1.0))
        );
    }

    #[test]
    fn nested_relative_timing_matches_explicit_events_in_position_and_velocity() {
        let x = PropertyId::new("x");
        let y = PropertyId::new("y");
        let z = PropertyId::new("z");
        let marker = PropertyId::new("marker");
        let first = profile();
        let redirect = SpringProfile::new(0.35, 0.65, 0.001, 0.001);
        assert!(first.advance_time() + 0.125 < 1.0);
        assert!(redirect.advance_time() + 0.1875 < 1.0);
        let animation = Animation::sequence([
            Animation::hold(0.125),
            Animation::parallel([
                Animation::sequence([
                    Animation::spring(x.clone(), 100.0, first),
                    Animation::delay(0.125, Animation::set(marker.clone(), 2.0)),
                ]),
                Animation::delay(
                    0.1875,
                    Animation::parallel([
                        Animation::spring(x.clone(), -40.0, redirect),
                        Animation::spring(y.clone(), 20.0, first),
                    ]),
                ),
                Animation::delay(
                    0.0625,
                    Animation::sequence([
                        Animation::set(z.clone(), 3.0),
                        Animation::hold(0.0625),
                        Animation::spring(z.clone(), 9.0, redirect),
                    ]),
                ),
                Animation::hold(1.0),
            ]),
            Animation::set(y.clone(), -10.0),
            Animation::hold(0.25),
        ]);
        let initial = [
            (x.clone(), 0.0),
            (y.clone(), -4.0),
            (z.clone(), 0.0),
            (marker.clone(), 0.0),
        ];
        let relative = Timeline::compile(initial.clone(), &animation).unwrap();
        // Spell out the expected leaf clock without walking the relative tree.
        // The marker retains the original f32 sequence-addition order.
        let marker_at = f64::from((0.125_f32 + first.advance_time()) + 0.125);
        let explicit = Timeline::compile_events(
            initial,
            [
                TimedEvent::spring(0.125, x.clone(), 100.0, first),
                TimedEvent::set(marker_at, marker.clone(), 2.0),
                TimedEvent::spring(0.3125, x.clone(), -40.0, redirect),
                TimedEvent::spring(0.3125, y.clone(), 20.0, first),
                TimedEvent::set(0.1875, z.clone(), 3.0),
                TimedEvent::spring(0.25, z.clone(), 9.0, redirect),
                TimedEvent::set(1.125, y.clone(), -10.0),
            ],
            1.375,
        )
        .unwrap();
        assert_eq!(relative.duration().to_bits(), 1.375_f32.to_bits());
        assert_eq!(relative.duration().to_bits(), explicit.duration().to_bits());
        let mut times = vec![4.0, 0.0, 0.45, 0.2, 1.375, 0.45];
        for at in [0.125, 0.1875, 0.25, 0.3125, marker_at, 1.125] {
            times.extend([at - 1e-9, at, at + 1e-9]);
        }
        for time in times {
            for property in [&x, &y, &z, &marker] {
                let actual = relative.sample_at(property, time).unwrap();
                let expected = explicit.sample_at(property, time).unwrap();
                assert_eq!(
                    [actual.position, actual.velocity].map(f32::to_bits),
                    [expected.position, expected.velocity].map(f32::to_bits),
                    "{} at {time}",
                    property.as_str()
                );
            }
        }
        let uninterrupted = Timeline::compile_events(
            [(x.clone(), 0.0)],
            [TimedEvent::spring(0.125, x.clone(), 100.0, first)],
            1.375,
        )
        .unwrap();
        let at_retarget = relative.sample_at(&x, 0.3125).unwrap();
        assert_eq!(at_retarget, uninterrupted.sample_at(&x, 0.3125).unwrap());
        assert!(at_retarget.velocity > 1.0);
        assert_eq!(
            relative.sample_at(&y, 1.125),
            Some(crate::motion::MotionState::at(-10.0))
        );
    }

    #[test]
    fn duplicate_initial_values_are_rejected() {
        let x = PropertyId::new("panel.x");
        let error = Timeline::compile([(x.clone(), 0.0), (x, 1.0)], &Animation::hold(1.0))
            .err()
            .unwrap();

        assert!(
            error
                .to_string()
                .contains("property 'panel.x' has more than one initial value")
        );
    }

    #[test]
    fn delayed_parallel_writes_retarget_the_same_property() {
        let x = PropertyId::new("pointer.x");
        let animation = Animation::parallel([
            Animation::spring(x.clone(), 10.0, profile()),
            Animation::delay(0.4, Animation::spring(x.clone(), 20.0, profile())),
        ]);
        let timeline = Timeline::compile([(x.clone(), 0.0)], &animation).unwrap();

        assert!(timeline.sample(&x, 0.4).unwrap().velocity.abs() > 0.0);
        assert!(timeline.sample(&x, 2.0).unwrap().position > 19.9);
    }

    #[test]
    fn delayed_parallel_retargeting_is_independent_of_child_order() {
        let x = PropertyId::new("pointer.x");
        let forward = Animation::parallel([
            Animation::spring(x.clone(), 10.0, profile()),
            Animation::delay(0.4, Animation::spring(x.clone(), 20.0, profile())),
        ]);
        let reversed = Animation::parallel([
            Animation::delay(0.4, Animation::spring(x.clone(), 20.0, profile())),
            Animation::spring(x.clone(), 10.0, profile()),
        ]);
        let forward = Timeline::compile([(x.clone(), 0.0)], &forward).unwrap();
        let reversed = Timeline::compile([(x.clone(), 0.0)], &reversed).unwrap();

        for time in [0.0, 0.2, 0.4, 0.8, 2.0] {
            let forward = forward.sample(&x, time).unwrap();
            let reversed = reversed.sample(&x, time).unwrap();
            assert!((forward.position - reversed.position).abs() < 0.0001);
            assert!((forward.velocity - reversed.velocity).abs() < 0.0001);
        }
    }

    #[test]
    fn sequential_springs_preserve_velocity_when_retargeted() {
        let x = PropertyId::new("camera.x");
        let first_duration = profile().advance_time();
        let first = Animation::spring(x.clone(), 100.0, profile());
        let first_timeline = Timeline::compile([(x.clone(), 0.0)], &first).unwrap();
        let before_retarget = first_timeline.sample(&x, first_duration).unwrap();

        let animation =
            Animation::sequence([first, Animation::spring(x.clone(), -40.0, profile())]);
        let timeline = Timeline::compile([(x.clone(), 0.0)], &animation).unwrap();
        let at_retarget = timeline.sample(&x, first_duration).unwrap();

        assert!((timeline.duration() - first_duration * 2.0).abs() < 0.0001);
        assert!((at_retarget.position - before_retarget.position).abs() < 0.0001);
        assert!((at_retarget.velocity - before_retarget.velocity).abs() < 0.0001);
        assert!(at_retarget.velocity.abs() > 0.001);
    }

    #[test]
    fn set_is_discontinuous_and_resets_velocity() {
        let x = PropertyId::new("panel.x");
        let animation = Animation::sequence([
            Animation::spring(x.clone(), 100.0, profile()),
            Animation::set(x.clone(), -20.0),
        ]);
        let timeline = Timeline::compile([(x.clone(), 0.0)], &animation).unwrap();
        let state = timeline.sample(&x, profile().advance_time()).unwrap();

        assert_eq!(state.position, -20.0);
        assert_eq!(state.velocity, 0.0);
    }

    #[test]
    fn spring_becomes_exactly_stationary_after_settling() {
        let x = PropertyId::new("panel.x");
        let animation = Animation::spring(x.clone(), 100.0, profile());
        let timeline = Timeline::compile([(x.clone(), 0.0)], &animation).unwrap();
        let state = timeline.sample(&x, 10.0).unwrap();

        assert_eq!(state.position, 100.0);
        assert_eq!(state.velocity, 0.0);
    }

    #[test]
    fn visual_duration_matches_motion_parameterization() {
        let profile = SpringProfile::from_visual_duration(0.25, 0.0, 0.001, 0.001);
        let spring = Animation::spring(PropertyId::new("x"), 1.0, profile);
        let x = PropertyId::new("x");
        let timeline = Timeline::compile([(x.clone(), 0.0)], &spring).unwrap();
        let state = timeline.sample(&x, 0.25).unwrap();

        assert!((state.position - 0.9668).abs() < 0.0001);
        assert!(state.position < 1.0);
    }

    #[test]
    fn explicit_events_preserve_same_time_source_order() {
        let x = PropertyId::new("x");
        let profile = SpringProfile::new(0.4, 1.0, 0.001, 0.001);
        let timeline = Timeline::compile_events(
            [(x.clone(), 0.0)],
            [
                TimedEvent::set(0.5, x.clone(), 4.0),
                TimedEvent::spring(0.5, x.clone(), 10.0, profile),
            ],
            2.0,
        )
        .unwrap();

        let at = timeline.sample(&x, 0.5).unwrap();
        let moving = timeline.sample(&x, 0.6).unwrap();
        assert_eq!(at.position, 4.0);
        assert!(moving.position > 4.0);
    }

    #[test]
    fn explicit_events_sort_by_time_without_losing_equal_time_order() {
        let x = PropertyId::new("x");
        let timeline = Timeline::compile_events(
            [(x.clone(), 0.0)],
            [
                TimedEvent::set(1.0, x.clone(), 1.0),
                TimedEvent::set(0.5, x.clone(), 2.0),
                TimedEvent::set(0.5, x.clone(), 3.0),
            ],
            2.0,
        )
        .unwrap();

        assert_eq!(timeline.sample(&x, 0.5).unwrap().position, 3.0);
        assert_eq!(timeline.sample(&x, 1.0).unwrap().position, 1.0);
    }

    #[test]
    fn explicit_event_selection_retains_sub_microsecond_clock_precision() {
        let x = PropertyId::new("x");
        let timeline = Timeline::compile_events(
            [(x.clone(), 0.0)],
            [
                TimedEvent::set(100.0, x.clone(), 1.0),
                TimedEvent::set(100.000_000_001, x.clone(), 2.0),
            ],
            101.0,
        )
        .unwrap();

        assert_eq!(timeline.sample_at(&x, 100.0).unwrap().position, 1.0);
        assert_eq!(
            timeline.sample_at(&x, 100.000_000_001).unwrap().position,
            2.0
        );
    }
}
