use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::motion::{MotionState, Spring};

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
            position_threshold > 0.0,
            "position threshold must be positive"
        );
        assert!(
            velocity_threshold > 0.0,
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
    Sequence(Vec<Animation>),
    Parallel(Vec<Animation>),
    Delay {
        seconds: f32,
        animation: Box<Animation>,
    },
    Hold(f32),
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
            Self::Sequence(animations) => animations.iter().map(Self::duration).sum(),
            Self::Parallel(animations) => animations.iter().map(Self::duration).fold(0.0, f32::max),
            Self::Delay { seconds, animation } => seconds + animation.duration(),
            Self::Hold(seconds) => *seconds,
        }
    }

    fn writes_at(&self, start: f32, writes: &mut Vec<(PropertyId, f32)>) {
        match self {
            Self::Set { property, .. } | Self::Spring { property, .. } => {
                writes.push((property.clone(), start));
            }
            Self::Sequence(animations) => {
                let mut cursor = start;
                for animation in animations {
                    animation.writes_at(cursor, writes);
                    cursor += animation.duration();
                }
            }
            Self::Parallel(animations) => {
                for animation in animations {
                    animation.writes_at(start, writes);
                }
            }
            Self::Delay { seconds, animation } => animation.writes_at(start + seconds, writes),
            Self::Hold(_) => {}
        }
    }

    fn validate(&self) -> Result<()> {
        let mut writes = Vec::new();
        self.writes_at(0.0, &mut writes);
        for (index, (property, start)) in writes.iter().enumerate() {
            if writes[..index]
                .iter()
                .any(|(other, other_start)| other == property && other_start == start)
            {
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
        position_threshold: f32,
        velocity_threshold: f32,
    },
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    start: f32,
    initial: MotionState,
    kind: SegmentKind,
}

impl Segment {
    fn sample(self, time: f32) -> MotionState {
        match self.kind {
            SegmentKind::Set => self.initial,
            SegmentKind::Spring {
                spring,
                target,
                position_threshold,
                velocity_threshold,
            } => {
                let state = spring.sample(self.initial, target, time - self.start);
                if (state.position - target).abs() <= position_threshold
                    && state.velocity.abs() <= velocity_threshold
                {
                    MotionState::at(target)
                } else {
                    state
                }
            }
        }
    }
}

pub struct Timeline {
    tracks: HashMap<PropertyId, Vec<Segment>>,
    duration: f32,
}

impl Timeline {
    pub fn compile(
        initial_values: impl IntoIterator<Item = (PropertyId, f32)>,
        animation: &Animation,
    ) -> Result<Self> {
        animation.validate()?;
        let mut timeline = Self {
            tracks: initial_values
                .into_iter()
                .map(|(property, value)| {
                    (
                        property,
                        vec![Segment {
                            start: 0.0,
                            initial: MotionState::at(value),
                            kind: SegmentKind::Set,
                        }],
                    )
                })
                .collect(),
            duration: animation.duration(),
        };
        timeline.compile_at(animation, 0.0)?;
        Ok(timeline)
    }

    pub fn duration(&self) -> f32 {
        self.duration
    }

    pub fn sample(&self, property: &PropertyId, time: f32) -> Option<MotionState> {
        let track = self.tracks.get(property)?;
        let segment = track
            .iter()
            .rev()
            .find(|segment| segment.start <= time.max(0.0))?;
        Some(segment.sample(time.max(0.0)))
    }

    fn compile_at(&mut self, animation: &Animation, start: f32) -> Result<()> {
        match animation {
            Animation::Set { property, value } => {
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
                let initial = self.sample(property, start).ok_or_else(|| {
                    anyhow::anyhow!("property '{}' has no initial value", property.as_str())
                })?;
                self.push_segment(
                    property,
                    Segment {
                        start,
                        initial,
                        kind: SegmentKind::Spring {
                            spring: profile.spring,
                            target: *target,
                            position_threshold: profile.position_threshold,
                            velocity_threshold: profile.velocity_threshold,
                        },
                    },
                );
            }
            Animation::Sequence(animations) => {
                let mut cursor = start;
                for child in animations {
                    self.compile_at(child, cursor)?;
                    cursor += child.duration();
                }
            }
            Animation::Parallel(animations) => {
                for child in animations {
                    self.compile_at(child, start)?;
                }
            }
            Animation::Delay { seconds, animation } => {
                self.compile_at(animation, start + seconds)?;
            }
            Animation::Hold(_) => {}
        }
        Ok(())
    }

    fn push_segment(&mut self, property: &PropertyId, segment: Segment) {
        let track = self.tracks.entry(property.clone()).or_default();
        let insertion = track.partition_point(|existing| existing.start <= segment.start);
        track.insert(insertion, segment);
    }
}

#[cfg(test)]
mod tests {
    use super::{Animation, PropertyId, SpringProfile, Timeline};

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
}
