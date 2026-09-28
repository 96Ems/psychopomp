//! Closed-form motion for deterministic particles: no integration history.
use super::Vec3;

/// Displacement under constant acceleration and linear drag. `drag` is the
/// reciprocal damping time; zero gives ordinary ballistic motion.
pub fn ballistic(velocity: Vec3, acceleration: Vec3, drag: f32, seconds: f32) -> Vec3 {
    let t = seconds.max(0.0);
    if drag <= 1e-4 {
        return velocity * t + acceleration * (0.5 * t * t);
    }
    let travel = -(-drag * t).exp_m1() / drag;
    velocity * travel + acceleration * ((t - travel) / drag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ballistic_motion_starts_at_rest_and_drag_dissipates_velocity() {
        let velocity = Vec3::new(100.0, -50.0, 0.0);
        let gravity = Vec3::new(0.0, 180.0, 0.0);
        assert_eq!(ballistic(velocity, gravity, 0.7, 0.0), Vec3::ZERO);
        assert_eq!(
            ballistic(velocity, gravity, 0.0, 1.0),
            Vec3::new(100.0, 40.0, 0.0)
        );
        assert!(ballistic(velocity, gravity, 0.7, 1.0).x < 100.0);
        assert!(ballistic(velocity, gravity, 0.7, 3.0).y > 0.0);
        assert!(
            (ballistic(velocity, gravity, 0.001, 1.0) - ballistic(velocity, gravity, 0.0, 1.0))
                .length()
                < 0.1
        );
    }
}
