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
    events: Vec<StateSegment<T>>,
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
        if !duration.is_finite() || duration < 0.0 {
            bail!("state track duration must be finite and non-negative");
        }
        let mut events = events.into_iter().enumerate().collect::<Vec<_>>();
        events.sort_by(|(left_index, left), (right_index, right)| {
            left.at
                .total_cmp(&right.at)
                .then(left_index.cmp(right_index))
        });
        let mut segments = Vec::new();
        let mut previous_value = initial.clone();
        let mut previous_at = 0.0;
        for (_, event) in events {
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
            events: segments,
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
                age: time as f32,
                transition_at: 0.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StateSample, StateTrack, TimedState};

    fn assert_sample(sample: StateSample<'_, &str>, expected: (&str, &str, [f32; 3])) {
        assert_eq!(
            (*sample.previous, *sample.current),
            (expected.0, expected.1)
        );
        assert_eq!(
            [sample.previous_duration, sample.age, sample.transition_at].map(f32::to_bits),
            expected.2.map(f32::to_bits)
        );
    }

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
    fn sampled_cell_mutation_does_not_change_other_snapshot_copies() {
        use std::cell::Cell;

        let track = StateTrack::compile(
            Cell::new(0),
            [
                TimedState::new(1.0, Cell::new(1)),
                TimedState::new(2.0, Cell::new(2)),
            ],
            3.0,
        )
        .unwrap();

        track.sample_at(1.0).current.set(99);
        assert_eq!(track.sample_at(2.0).previous.get(), 1);
        assert_eq!(track.sample_at(1.0).current.get(), 99);

        track.sample_at(0.0).current.set(7);
        assert_eq!(track.sample_at(1.0).previous.get(), 0);
        track.sample_at(2.0).previous.set(88);
        assert_eq!(track.sample_at(1.0).current.get(), 99);
        assert_eq!(track.sample_at(2.0).current.get(), 2);
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

    #[test]
    fn every_sample_field_retains_equal_time_history_and_ignores_repeated_values() {
        let track = StateTrack::compile(
            "initial",
            [
                TimedState::new(6.0, "done"),
                TimedState::new(2.0, "running"),
                TimedState::new(3.0, "running"),
                TimedState::new(4.0, "done"),
                TimedState::new(4.0, "initial"),
                TimedState::new(4.0, "initial"),
                TimedState::new(5.0, "initial"),
            ],
            8.0,
        )
        .unwrap();

        // Deliberately sample backward and beyond duration. At 4s, "done" is
        // the previous state even though it was never current for positive time.
        for (time, expected) in [
            (9.0, ("initial", "done", [2.0, 3.0, 6.0])),
            (0.0, ("initial", "initial", [0.0, 0.0, 0.0])),
            (4.0, ("done", "initial", [0.0, 0.0, 4.0])),
            (3.5, ("initial", "running", [2.0, 1.5, 2.0])),
            (-2.0, ("initial", "initial", [0.0, 0.0, 0.0])),
            (6.0, ("initial", "done", [2.0, 0.0, 6.0])),
            (4.5, ("done", "initial", [0.0, 0.5, 4.0])),
            (1.0, ("initial", "initial", [0.0, 1.0, 0.0])),
            (2.0, ("initial", "running", [2.0, 0.0, 2.0])),
            (1.5, ("initial", "initial", [0.0, 1.5, 0.0])),
            (4.0, ("done", "initial", [0.0, 0.0, 4.0])),
        ] {
            assert_sample(track.sample_at(time), expected);
            assert_sample(track.sample(time as f32), expected);
        }
    }

    #[test]
    fn empty_and_initial_only_tracks_age_from_the_start() {
        for events in [
            Vec::new(),
            vec![TimedState::new(2.0, "idle"), TimedState::new(3.0, "idle")],
        ] {
            let track = StateTrack::compile("idle", events, 4.0).unwrap();
            for (time, age) in [(10.0, 10.0), (-1.0, 0.0), (2.0, 2.0), (3.0, 3.0)] {
                assert_sample(track.sample_at(time), ("idle", "idle", [0.0, age, 0.0]));
            }
        }
    }

    #[test]
    fn adjacent_nanoseconds_preserve_all_sample_fields_after_deduplication() {
        let at = |offset: i64| (100_000_000_000_i64 + offset) as f64 / 1e9;
        let track = StateTrack::compile(
            "initial",
            [
                TimedState::new(at(3), "third"),
                TimedState::new(at(0), "first"),
                TimedState::new(at(1), "second"),
                TimedState::new(at(2), "second"),
            ],
            101.0,
        )
        .unwrap();
        // Differences are taken on the public f64 clock before the sample's f32
        // conversion, not rounded to an assumed exact 1e-9-second interval.
        let first_duration = (at(1) - at(0)) as f32;
        let second_duration = (at(3) - at(1)) as f32;
        for (offset, expected) in [
            (3, ("second", "third", [second_duration, 0.0, at(3) as f32])),
            (-1, ("initial", "initial", [0.0, at(-1) as f32, 0.0])),
            (0, ("initial", "first", [100.0, 0.0, 100.0])),
            (1, ("first", "second", [first_duration, 0.0, at(1) as f32])),
            (
                2,
                (
                    "first",
                    "second",
                    [first_duration, (at(2) - at(1)) as f32, at(1) as f32],
                ),
            ),
            (
                4,
                (
                    "second",
                    "third",
                    [second_duration, (at(4) - at(3)) as f32, at(3) as f32],
                ),
            ),
            (1, ("first", "second", [first_duration, 0.0, at(1) as f32])),
        ] {
            assert_sample(track.sample_at(at(offset)), expected);
        }
    }

    #[test]
    fn invalid_state_times_are_checked_before_deduplication() {
        for (duration, events, expected) in [
            (
                f64::NAN,
                vec![TimedState::new(3.0, "initial")],
                "state track duration must be finite and non-negative",
            ),
            (
                2.0,
                vec![TimedState::new(3.0, "initial")],
                "state event at 3.000s exceeds duration 2.000s",
            ),
        ] {
            assert_eq!(
                StateTrack::compile("initial", events, duration)
                    .unwrap_err()
                    .to_string(),
                expected
            );
        }
    }
}
