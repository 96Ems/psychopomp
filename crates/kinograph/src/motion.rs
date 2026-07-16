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
        assert!(response_seconds > 0.0, "spring response must be positive");
        assert!(
            (0.0..=1.0).contains(&damping_ratio) && damping_ratio > 0.0,
            "spring damping must be positive and no greater than critical"
        );
        Self {
            angular_frequency: std::f32::consts::TAU / response_seconds,
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
}
