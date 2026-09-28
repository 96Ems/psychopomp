//! Reversible combustion: shell compression, ignition, cooling, and embers.
use crate::math::{Vec3, dynamics::ballistic, smoothstep, vec3};

pub const DURATION: f32 = 5.2;

/// A sampled effect pose. Negative age means intact; age >= DURATION is spent.
/// Sample the same age in any order, including backwards, without retained state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst {
    pub shell_scale: f32,
    pub shell_opacity: f32,
    pub ignition: f32,
    pub rim_strength: f32,
    released: f32,
}

/// One seed-stable ember, relative to its compressed shell anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ember {
    pub offset: Vec3,
    pub tail_offset: Vec3,
    pub opacity: f32,
    pub heat: f32,
    pub radius: f32,
}

impl Burst {
    pub fn sample(age_seconds: f32) -> Self {
        let released = (age_seconds - 0.12).max(0.0);
        let ignition = smoothstep(released / 0.055);
        let rim_strength = if (0.0..2.4).contains(&(age_seconds - 0.12)) {
            0.8 * smoothstep(released / 0.06) * (-2.5 * released).exp()
        } else {
            0.0
        };
        Self {
            shell_scale: 1.0 - 0.55 * smoothstep(age_seconds / 0.12),
            shell_opacity: 1.0 - ignition,
            ignition,
            rim_strength,
            released,
        }
    }

    /// `direction` is a unit vector; `seed` contains stable values in 0..1.
    /// Positions use world pixels and seconds; the caller places the emitter.
    pub fn ember(self, direction: Vec3, seed: Vec3) -> Ember {
        let velocity = direction * (170.0 + 540.0 * seed.x);
        let gravity = vec3(0.0, 150.0 + 90.0 * seed.y, 0.0);
        let drag = 0.55 + 0.8 * seed.z;
        let life = 1.6 + 3.2 * seed.y;
        Ember {
            offset: ballistic(velocity, gravity, drag, self.released),
            tail_offset: ballistic(velocity, gravity, drag, (self.released - 0.045).max(0.0)),
            opacity: (1.0 - smoothstep((self.released - life * 0.45) / (life * 0.55)))
                * self.ignition,
            heat: (-self.released * (0.9 + seed.z)).exp(),
            radius: 0.9 + seed.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_preserves_shell_and_ignition_hands_over_presence() {
        let entry = Burst::sample(0.0);
        assert_eq!(entry.shell_scale, 1.0);
        assert_eq!(entry.shell_opacity, 1.0);
        assert_eq!(entry.rim_strength, 0.0);
        for age in [0.0, 0.12, 0.14, 0.175] {
            let burst = Burst::sample(age);
            let ember = burst.ember(Vec3::X, Vec3::splat(0.5));
            assert!((burst.shell_opacity + ember.opacity - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn embers_reconstruct_out_of_order_and_eventually_extinguish() {
        let sample = |age| Burst::sample(age).ember(Vec3::X, vec3(0.3, 0.7, 0.4));
        let first = sample(0.5);
        assert!(sample(2.0).offset.y > first.offset.y, "gravity pulls down");
        assert_eq!(sample(0.5), first);
        assert!(first.tail_offset.x < first.offset.x);
        assert_eq!(sample(DURATION).opacity, 0.0);
        assert_eq!(Burst::sample(DURATION).shell_opacity, 0.0);
        assert_eq!(Burst::sample(DURATION).rim_strength, 0.0);
    }
}
