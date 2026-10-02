//! Closed-form motion for deterministic particles and settling springs: no
//! integration history.
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

/// A critically damped spring (ω = 10 / `duration`) from `from`, moving at
/// `velocity` units per second, toward `target`, `elapsed` seconds in. Returns
/// position and velocity; from `duration` on it rests exactly at `target`.
/// Inherited speed is bounded to `12 · max(|distance|, 1) / duration`, so a
/// redirected wheel never spins through hundreds of faces. This is
/// `@kitlangton/rolling-number`'s one trajectory for rolls, slides, and fades.
pub fn settle(from: f32, target: f32, velocity: f32, duration: f32, elapsed: f32) -> (f32, f32) {
    if duration <= 0.0 || elapsed >= duration {
        return (target, 0.0);
    }
    let duration = f64::from(duration);
    let t = f64::from(elapsed.max(0.0)) / duration;
    let distance = f64::from(from) - f64::from(target);
    let limit = distance.abs().max(1.0) * 12.0 / duration;
    let inherited = f64::from(velocity).clamp(-limit, limit) * duration;
    let coefficient = inherited + 10.0 * distance;
    let decay = (-10.0 * t).exp();
    (
        (f64::from(target) + (distance + coefficient * t) * decay) as f32,
        ((inherited - 10.0 * coefficient * t) * decay / duration) as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settle_starts_with_its_state_and_rests_exactly_at_its_duration() {
        assert_eq!(settle(2.0, 7.0, 3.0, 0.5, 0.0), (2.0, 3.0));
        assert_eq!(settle(2.0, 7.0, 3.0, 0.5, 0.5), (7.0, 0.0));
        let (near, _) = settle(2.0, 7.0, 0.0, 0.5, 0.499);
        assert!((near - 7.0).abs() < 0.01);
        // Velocity is the derivative of position.
        let (a, velocity) = settle(0.0, 10.0, -4.0, 0.5, 0.1);
        let (b, _) = settle(0.0, 10.0, -4.0, 0.5, 0.1 + 1e-4);
        assert!(((b - a) / 1e-4 - velocity).abs() < 0.05 * velocity.abs());
        // Inherited speed is bounded by the distance it has to cover.
        assert_eq!(settle(0.0, 1.0, 1e6, 0.5, 0.0).1, 24.0);
    }

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
