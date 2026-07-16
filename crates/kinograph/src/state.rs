use anyhow::{Result, bail};

#[derive(Clone, Debug)]
pub struct TimedState<T> {
    pub at: f64,
    pub value: T,
}

impl<T> TimedState<T> {
    pub fn new(at: f64, value: T) -> Self {
        assert!(
            at.is_finite() && at >= 0.0,
            "state event time must be finite and non-negative"
        );
        Self { at, value }
    }
}

#[derive(Clone, Debug)]
struct StateSegment<T> {
    at: f64,
    value: T,
    previous: T,
    previous_duration: f64,
}

#[derive(Clone, Debug)]
pub struct StateTrack<T> {
    initial: T,
    initial_at: f64,
    events: Vec<StateSegment<T>>,
    duration: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct StateSample<'a, T> {
    pub previous: &'a T,
    pub current: &'a T,
    pub previous_duration: f32,
    pub age: f32,
    pub transition_at: f32,
}

impl<T: Clone + PartialEq> StateTrack<T> {
    pub fn compile(
        initial: T,
        events: impl IntoIterator<Item = TimedState<T>>,
        duration: f64,
    ) -> Result<Self> {
        Self::compile_at(0.0, initial, events, duration)
    }

    pub fn compile_at(
        initial_at: f64,
        initial: T,
        events: impl IntoIterator<Item = TimedState<T>>,
        duration: f64,
    ) -> Result<Self> {
        if !duration.is_finite() || duration < 0.0 {
            bail!("state track duration must be finite and non-negative");
        }
        if !initial_at.is_finite() || initial_at < 0.0 || initial_at > duration {
            bail!("state track initial time must be within its duration");
        }
        let mut events = events.into_iter().enumerate().collect::<Vec<_>>();
        events.sort_by(|(left_index, left), (right_index, right)| {
            left.at
                .total_cmp(&right.at)
                .then(left_index.cmp(right_index))
        });
        let mut segments = Vec::new();
        let mut previous_value = initial.clone();
        let mut previous_at = initial_at;
        for (_, event) in events {
            if event.at < initial_at {
                bail!("state event cannot precede the initial state");
            }
            if event.at > duration {
                bail!(
                    "state event at {:.3}s exceeds duration {duration:.3}s",
                    event.at
                );
            }
            if event.value == previous_value {
                continue;
            }
            segments.push(StateSegment {
                at: event.at,
                value: event.value.clone(),
                previous: previous_value,
                previous_duration: event.at - previous_at,
            });
            previous_value = event.value;
            previous_at = event.at;
        }
        Ok(Self {
            initial,
            initial_at,
            events: segments,
            duration,
        })
    }

    pub fn sample(&self, time: f32) -> StateSample<'_, T> {
        self.sample_at(f64::from(time))
    }

    pub fn sample_at(&self, time: f64) -> StateSample<'_, T> {
        let time = time.max(0.0);
        if let Some(event) = self.events.iter().rev().find(|event| event.at <= time) {
            StateSample {
                previous: &event.previous,
                current: &event.value,
                previous_duration: event.previous_duration as f32,
                age: (time - event.at) as f32,
                transition_at: event.at as f32,
            }
        } else {
            StateSample {
                previous: &self.initial,
                current: &self.initial,
                previous_duration: 0.0,
                age: (time - self.initial_at).max(0.0) as f32,
                transition_at: self.initial_at as f32,
            }
        }
    }

    pub fn duration(&self) -> f32 {
        self.duration as f32
    }

    pub fn last_time_matching(
        &self,
        time: f32,
        mut predicate: impl FnMut(&T) -> bool,
    ) -> Option<f32> {
        let time = f64::from(time.max(0.0));
        self.events
            .iter()
            .rev()
            .find(|event| event.at <= time && predicate(&event.value))
            .map(|event| event.at as f32)
            .or_else(|| predicate(&self.initial).then_some(self.initial_at as f32))
    }

    pub fn last_interval_start(
        &self,
        time: f32,
        mut predicate: impl FnMut(&T) -> bool,
    ) -> Option<f32> {
        let time = f64::from(time.max(0.0));
        let mut active = predicate(&self.initial);
        let mut current_start = active.then_some(self.initial_at);
        let mut last_start = current_start;
        for event in self.events.iter().filter(|event| event.at <= time) {
            let next = predicate(&event.value);
            if next && !active {
                current_start = Some(event.at);
                last_start = current_start;
            } else if !next && active {
                current_start = None;
            }
            active = next;
        }
        current_start.or(last_start).map(|start| start as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::{StateTrack, TimedState};

    #[test]
    fn repeated_states_do_not_restart_age() {
        let track = StateTrack::compile(
            "idle",
            [
                TimedState::new(0.5, "running"),
                TimedState::new(0.7, "running"),
                TimedState::new(1.0, "done"),
            ],
            2.0,
        )
        .unwrap();

        assert_eq!(track.sample(0.9).current, &"running");
        assert!((track.sample(0.9).age - 0.4).abs() < 0.0001);
        assert_eq!(track.sample(1.0).previous, &"running");
        assert!((track.sample(1.0).previous_duration - 0.5).abs() < 0.0001);
    }

    #[test]
    fn same_time_states_preserve_source_order() {
        let track = StateTrack::compile(
            "idle",
            [
                TimedState::new(0.5, "running"),
                TimedState::new(0.5, "done"),
            ],
            1.0,
        )
        .unwrap();
        let sample = track.sample(0.5);

        assert_eq!(sample.previous, &"running");
        assert_eq!(sample.current, &"done");
        assert_eq!(sample.previous_duration, 0.0);
    }

    #[test]
    fn out_of_order_sampling_is_deterministic() {
        let track = StateTrack::compile(0, [TimedState::new(0.5, 1), TimedState::new(1.0, 2)], 2.0)
            .unwrap();

        let later = track.sample(1.2);
        let _earlier = track.sample(0.3);
        let repeated = track.sample(1.2);
        assert_eq!(later.current, repeated.current);
        assert_eq!(later.age, repeated.age);
    }

    #[test]
    fn event_selection_retains_sub_microsecond_clock_precision() {
        let track = StateTrack::compile(
            "initial",
            [
                TimedState::new(100.0, "first"),
                TimedState::new(100.000_000_001, "second"),
            ],
            101.0,
        )
        .unwrap();

        assert_eq!(track.sample_at(100.0).current, &"first");
        assert_eq!(track.sample_at(100.000_000_001).current, &"second");
    }
}
