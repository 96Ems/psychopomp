//! Cancellation-safe numeric scheduling shared by live navigation and authored
//! resting entrances. No wall clock, step selection, typography or renderer.
use super::{PropertyId, SpringProfile, TimedEvent, Timeline};
use crate::motion::MotionState;
use anyhow::{Context, Result, bail};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub struct StartDelay {
    pub from: f32,
    pub to: f32,
    pub delay: Duration,
}

/// Preserve the caller's timestamp arithmetic: export adds integer nanoseconds;
/// native scheduling historically adds seconds on its local Timeline clock.
#[derive(Clone, Copy, Debug)]
pub enum ScheduleTime {
    Seconds(f64),
    Nanos(u64),
}
impl ScheduleTime {
    fn at_or_before(self, other: Self) -> bool {
        match (self, other) {
            (Self::Nanos(left), Self::Nanos(right)) => left <= right,
            _ => self.seconds() <= other.seconds(),
        }
    }
    pub fn seconds(self) -> f64 {
        match self {
            Self::Seconds(at) => at,
            Self::Nanos(at) => at as f64 / 1e9,
        }
    }
    pub fn nanos(self) -> Option<u64> {
        match self {
            Self::Nanos(at) => Some(at),
            Self::Seconds(_) => None,
        }
    }
    fn delayed(self, delay: Duration) -> Result<Self> {
        Ok(match self {
            Self::Seconds(at) => Self::Seconds(at + delay.as_secs_f64()),
            Self::Nanos(at) => Self::Nanos(
                at.checked_add(u64::try_from(delay.as_nanos())?)
                    .context("scheduled time overflow")?,
            ),
        })
    }
    fn within(self, limit: u64) -> bool {
        match self {
            Self::Nanos(at) => at <= limit,
            Self::Seconds(at) => at <= limit as f64 / 1e9,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RetargetMode {
    Animate,
    Replay { from: f32 },
    Immediate,
}

pub struct Retarget {
    pub property: PropertyId,
    pub target: f32,
    pub profile: SpringProfile,
    pub delay: Option<StartDelay>,
    pub mode: RetargetMode,
}

#[derive(Clone)]
pub struct ScheduledWrite {
    pub property: PropertyId,
    pub at: ScheduleTime,
    pub target: f32,
    pub spring: Option<SpringProfile>,
}
impl ScheduledWrite {
    fn event(&self) -> TimedEvent {
        match self.spring {
            Some(profile) => TimedEvent::spring(
                self.at.seconds(),
                self.property.clone(),
                self.target,
                profile,
            ),
            None => TimedEvent::set(self.at.seconds(), self.property.clone(), self.target),
        }
    }
}

pub struct RetargetSchedule {
    initial: Vec<(PropertyId, f32)>,
    targets: HashMap<PropertyId, f32>,
    writes: Vec<ScheduledWrite>,
    timeline: Arc<Timeline>,
}
impl RetargetSchedule {
    pub fn new(initial: Vec<(PropertyId, f32)>) -> Result<Self> {
        Ok(Self {
            targets: initial.iter().cloned().collect(),
            timeline: Arc::new(Timeline::compile_events(initial.clone(), [], 0.)?),
            initial,
            writes: Vec::new(),
        })
    }
    pub fn timeline(&self) -> &Arc<Timeline> {
        &self.timeline
    }
    pub fn writes(&self) -> &[ScheduledWrite] {
        &self.writes
    }

    /// Validate the whole decision before mutation, sample all tracks from the
    /// previous immutable revision, then compile once. An unchanged Animate
    /// request retains its pending due time and existing trajectory. A decision
    /// has at most one request per property. A rejected batch changes nothing.
    pub fn retarget(
        &mut self,
        at: ScheduleTime,
        requests: impl IntoIterator<Item = Retarget>,
        limit_nanos: Option<u64>,
    ) -> Result<()> {
        if !at.seconds().is_finite() || at.seconds() < 0. {
            bail!("retarget time must be finite and nonnegative");
        }
        let mut changes = Vec::new();
        let mut seen = HashSet::new();
        for request in requests {
            if !seen.insert(request.property.clone()) {
                bail!(
                    "retarget decision repeats property '{}'",
                    request.property.as_str()
                );
            }
            let previous = self
                .targets
                .get(&request.property)
                .context("unknown retarget property")?;
            if !request.target.is_finite() {
                bail!("retarget destination must be finite");
            }
            if matches!(request.mode, RetargetMode::Animate) && *previous == request.target {
                continue;
            }
            let current = match request.mode {
                RetargetMode::Replay { from } => {
                    if !from.is_finite() {
                        bail!("replay pose must be finite");
                    }
                    MotionState::at(from)
                }
                _ => self
                    .timeline
                    .sample_at(&request.property, at.seconds())
                    .expect("declared schedule track"),
            };
            let delay = if matches!(request.mode, RetargetMode::Immediate) {
                Duration::ZERO
            } else {
                request
                    .delay
                    .filter(|d| request.target == d.to && current == MotionState::at(d.from))
                    .map_or(Duration::ZERO, |d| d.delay)
            };
            let start = at.delayed(delay)?;
            if !start.seconds().is_finite() || limit_nanos.is_some_and(|limit| !start.within(limit))
            {
                bail!("scheduled start exceeds scene duration");
            }
            changes.push((request, start));
        }
        let mut writes = self.writes.clone();
        let mut targets = self.targets.clone();
        for (request, start) in changes {
            writes.retain(|old| old.property != request.property || old.at.at_or_before(at));
            targets.insert(request.property.clone(), request.target);
            if let RetargetMode::Replay { from } = request.mode {
                writes.push(ScheduledWrite {
                    property: request.property.clone(),
                    at,
                    target: from,
                    spring: None,
                });
            }
            writes.push(ScheduledWrite {
                property: request.property,
                at: start,
                target: request.target,
                spring: if matches!(request.mode, RetargetMode::Immediate) {
                    None
                } else {
                    Some(request.profile)
                },
            });
        }
        let horizon = writes
            .iter()
            .map(|w| w.at.seconds())
            .fold(at.seconds(), f64::max);
        let timeline = Arc::new(Timeline::compile_events(
            self.initial.clone(),
            writes.iter().map(ScheduledWrite::event),
            horizon,
        )?);
        self.writes = writes;
        self.targets = targets;
        self.timeline = timeline;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_decisions_and_late_compile_errors_are_transactional() {
        let id = PropertyId::new("x");
        let mut schedule = RetargetSchedule::new(vec![(id.clone(), 0.)]).unwrap();
        let old = schedule.timeline().clone();
        assert!(
            schedule
                .retarget(
                    ScheduleTime::Seconds(0.),
                    [request(&id, 1.), request(&id, 0.)],
                    None
                )
                .is_err()
        );
        assert!(schedule.writes().is_empty());
        assert!(Arc::ptr_eq(&old, schedule.timeline()));
        let mut schedule = RetargetSchedule::new(vec![(id.clone(), f32::MAX)]).unwrap();
        schedule
            .retarget(ScheduleTime::Seconds(0.), [request(&id, -f32::MAX)], None)
            .unwrap();
        let old = schedule.timeline().clone();
        let count = schedule.writes().len();
        let targets = schedule.targets.clone();
        assert!(
            schedule
                .retarget(ScheduleTime::Seconds(0.1), [request(&id, 0.)], None)
                .is_err()
        );
        assert_eq!(schedule.writes().len(), count);
        assert_eq!(schedule.targets, targets);
        assert!(Arc::ptr_eq(&old, schedule.timeline()));
    }
    #[test]
    fn delay_preserves_each_callers_arithmetic_and_exact_cancellation_order() {
        let at = (1_u64 << 54) - 1;
        let delay = Duration::from_millis(60);
        assert_eq!(
            ScheduleTime::Nanos(at).delayed(delay).unwrap().nanos(),
            Some(at + 60_000_000)
        );
        let seconds = at as f64 / 1e9;
        assert_eq!(
            ScheduleTime::Seconds(seconds)
                .delayed(delay)
                .unwrap()
                .seconds()
                .to_bits(),
            (seconds + delay.as_secs_f64()).to_bits()
        );
        assert!(!ScheduleTime::Nanos(at + 1).at_or_before(ScheduleTime::Nanos(at)));
    }
    fn request(property: &PropertyId, target: f32) -> Retarget {
        Retarget {
            property: property.clone(),
            target,
            profile: SpringProfile::from_visual_duration(0.4, 0., 0.00001, 0.00001),
            delay: Some(StartDelay {
                from: 0.,
                to: 1.,
                delay: Duration::from_millis(120),
            }),
            mode: RetargetMode::Animate,
        }
    }
    #[test]
    fn unchanged_waits_survive_and_changed_waits_cancel_without_erasing_history() {
        let id = PropertyId::new("reveal");
        let mut schedule = RetargetSchedule::new(vec![(id.clone(), 0.)]).unwrap();
        schedule
            .retarget(ScheduleTime::Nanos(0), [request(&id, 1.)], None)
            .unwrap();
        schedule
            .retarget(ScheduleTime::Nanos(50_000_000), [request(&id, 1.)], None)
            .unwrap();
        assert_eq!(schedule.writes()[0].at.nanos(), Some(120_000_000));
        schedule
            .retarget(ScheduleTime::Nanos(60_000_000), [request(&id, 0.)], None)
            .unwrap();
        assert_eq!(
            schedule.timeline().sample_at(&id, 0.2),
            Some(MotionState::at(0.))
        );
        schedule
            .retarget(ScheduleTime::Nanos(80_000_000), [request(&id, 1.)], None)
            .unwrap();
        assert_eq!(
            schedule.writes().last().unwrap().at.nanos(),
            Some(200_000_000)
        );
        let old = schedule.timeline().clone();
        let before = old.sample_at(&id, 0.25);
        schedule
            .retarget(ScheduleTime::Nanos(250_000_000), [request(&id, 0.)], None)
            .unwrap();
        assert_eq!(schedule.timeline().sample_at(&id, 0.25), before);
        assert_eq!(old.sample_at(&id, 0.25), before);
        assert_eq!(
            schedule.writes().last().unwrap().at.nanos(),
            Some(250_000_000)
        );
    }
    #[test]
    fn rejected_start_does_not_mutate_the_previous_revision() {
        let id = PropertyId::new("x");
        let mut schedule = RetargetSchedule::new(vec![(id.clone(), 0.)]).unwrap();
        let old = schedule.timeline().clone();
        assert!(
            schedule
                .retarget(ScheduleTime::Nanos(1), [request(&id, 1.)], Some(100))
                .is_err()
        );
        assert!(Arc::ptr_eq(&old, schedule.timeline()));
        assert!(schedule.writes().is_empty());
    }
}
