#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionState {
    pub position: f32,
    pub velocity: f32,
}

impl MotionState {
    pub const fn at(position: f32) -> Self {
        Self {
            position,
            velocity: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    angular_frequency: f32,
    damping_ratio: f32,
}

impl Spring {
    pub fn new(response_seconds: f32, damping_ratio: f32) -> Self {
        assert!(
            response_seconds.is_finite() && response_seconds > 0.0,
            "spring response must be finite and positive"
        );
        assert!(
            (0.0..=1.0).contains(&damping_ratio) && damping_ratio > 0.0,
            "spring damping must be positive and no greater than critical"
        );
        let angular_frequency = std::f32::consts::TAU / response_seconds;
        assert!(
            angular_frequency.is_finite(),
            "spring response is too small"
        );
        Self {
            angular_frequency,
            damping_ratio,
        }
    }

    pub fn sample(self, initial: MotionState, target: f32, elapsed_seconds: f32) -> MotionState {
        if elapsed_seconds <= 0.0 {
            return initial;
        }

        let omega = self.angular_frequency;
        let damping = self.damping_ratio;
        let displacement = initial.position - target;

        if damping == 1.0 {
            let coefficient = initial.velocity + omega * displacement;
            let decay = (-omega * elapsed_seconds).exp();
            return MotionState {
                position: target + decay * (displacement + coefficient * elapsed_seconds),
                velocity: decay * (initial.velocity - omega * coefficient * elapsed_seconds),
            };
        }

        let damped_omega = omega * (1.0 - damping * damping).sqrt();
        let sine_coefficient = (initial.velocity + damping * omega * displacement) / damped_omega;
        let phase = damped_omega * elapsed_seconds;
        let decay = (-damping * omega * elapsed_seconds).exp();
        let oscillation = displacement * phase.cos() + sine_coefficient * phase.sin();
        let oscillation_velocity = -displacement * damped_omega * phase.sin()
            + sine_coefficient * damped_omega * phase.cos();

        MotionState {
            position: target + decay * oscillation,
            velocity: decay * (oscillation_velocity - damping * omega * oscillation),
        }
    }

    /// A deterministic time after which BOTH error and speed stay below their
    /// thresholds. A momentary threshold crossing is not physical settling.
    pub(crate) fn settling_time(
        self,
        initial: MotionState,
        target: f32,
        position_threshold: f32,
        velocity_threshold: f32,
    ) -> f64 {
        let omega = f64::from(self.angular_frequency);
        let damping = f64::from(self.damping_ratio);
        let displacement = f64::from(initial.position) - f64::from(target);
        let velocity = f64::from(initial.velocity);
        let position_threshold = f64::from(position_threshold);
        let velocity_threshold = f64::from(velocity_threshold);
        if damping < 1.0 {
            let decay = damping * omega;
            let frequency = omega * (1.0 - damping * damping).sqrt();
            let sine = (velocity + decay * displacement) / frequency;
            let position_amplitude = displacement.hypot(sine);
            let velocity_amplitude = velocity.hypot(-decay * sine - frequency * displacement);
            return ((position_amplitude / position_threshold)
                .ln()
                .max((velocity_amplitude / velocity_threshold).ln())
                / decay)
                .max(0.0);
        }

        // For critical damping, each component is (a + b*t) * exp(-omega*t).
        // Its remaining absolute maximum is at t or its one future extremum.
        let tail_max = |a: f64, b: f64, time: f64| {
            let at = |t: f64| ((a + b * t) * (-omega * t).exp()).abs();
            let extremum = if b == 0.0 { -1.0 } else { 1.0 / omega - a / b };
            at(time).max(if extremum > time { at(extremum) } else { 0.0 })
        };
        let coefficient = velocity + omega * displacement;
        let settled = |time| {
            tail_max(displacement, coefficient, time) <= position_threshold
                && tail_max(velocity, -omega * coefficient, time) <= velocity_threshold
        };
        if settled(0.0) {
            return 0.0;
        }
        let mut upper = 1.0 / omega;
        while !settled(upper) {
            upper *= 2.0;
        }
        let mut lower = 0.0;
        for _ in 0..48 {
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

#[cfg(test)]
mod tests {
    use super::{MotionState, Spring};

    #[test]
    fn starts_at_the_full_initial_state() {
        let initial = MotionState {
            position: 12.0,
            velocity: -3.0,
        };
        assert_eq!(Spring::new(0.5, 0.8).sample(initial, 40.0, 0.0), initial);
    }

    #[test]
    fn converges_to_the_target() {
        let sampled = Spring::new(0.5, 0.8).sample(MotionState::at(0.0), 1.0, 4.0);
        assert!((sampled.position - 1.0).abs() < 0.0001);
        assert!(sampled.velocity.abs() < 0.0001);
    }

    #[test]
    fn arbitrary_time_sampling_is_deterministic() {
        let spring = Spring::new(0.62, 0.84);
        let initial = MotionState::at(0.0);
        assert_eq!(
            spring.sample(initial, 1.0, 0.73),
            spring.sample(initial, 1.0, 0.73)
        );
    }

    #[test]
    fn critically_damped_spring_converges_without_overshoot() {
        let spring = Spring::new(0.3, 1.0);
        let initial = MotionState::at(0.0);
        let mut previous = 0.0;

        for step in 1..=100 {
            let sampled = spring.sample(initial, 1.0, step as f32 / 100.0);
            assert!(sampled.position >= previous);
            assert!(sampled.position <= 1.0);
            previous = sampled.position;
        }
    }

    #[test]
    fn settling_bounds_the_entire_remaining_trajectory() {
        for damping in [0.05, 0.8, 0.999, 1.0] {
            let spring = Spring::new(0.5, damping);
            for initial in [
                MotionState::at(0.0),
                MotionState {
                    position: 0.9,
                    velocity: 20.0,
                },
            ] {
                let settled = spring.settling_time(initial, 1.0, 0.02, 0.05);
                for index in 0..1000 {
                    let state =
                        spring.sample(initial, 1.0, (settled + f64::from(index) * 0.01) as f32);
                    assert!((state.position - 1.0).abs() <= 0.020001);
                    assert!(state.velocity.abs() <= 0.050001);
                }
            }
        }
    }
}
