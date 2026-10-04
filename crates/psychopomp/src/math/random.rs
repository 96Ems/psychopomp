//! Deterministic noise: the same inputs give the same value in any frame order.
use super::{Vec2, easing::smootherstep};

/// A well-mixed 0..1 value for an integer and a salt.
pub fn hash(value: u32, salt: u32) -> f32 {
    let mut h = value.wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1 << 24) as f32
}

/// Smooth deterministic noise in -1..1: hashed values at whole `t`, joined
/// by smoothstep, so it is continuous and has no fixed period.
pub fn smooth_noise(t: f32, salt: u32) -> f32 {
    let cell = t.floor();
    let f = t - cell;
    let at = |offset: i64| hash((cell as i64 + offset) as u32, salt) * 2.0 - 1.0;
    let eased = f * f * (3.0 - 2.0 * f);
    at(0) + (at(1) - at(0)) * eased
}

/// Smooth deterministic value noise in 0..1 over the plane: hashed values at
/// whole coordinates, joined by a minimum-jerk blend in each axis.
pub fn value_noise2(point: Vec2, salt: u32) -> f32 {
    let cell = point.floor();
    let f = point - cell;
    let (x, y) = (cell.x as i64, cell.y as i64);
    let at = |dx: i64, dy: i64| {
        let row = hash((y + dy) as u32, salt);
        hash((x + dx) as u32, salt ^ row.to_bits())
    };
    let (u, v) = (smootherstep(f.x), smootherstep(f.y));
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * u;
    let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * u;
    top + (bottom - top) * v
}

/// Fractal value noise in 0..1: `octaves` layers, each twice the frequency
/// and half the amplitude of the last, for organic edges with fine detail.
pub fn fractal_noise2(point: Vec2, octaves: u32, salt: u32) -> f32 {
    let (mut sum, mut amplitude, mut total, mut frequency) = (0.0, 1.0, 0.0, 1.0);
    for octave in 0..octaves {
        // Rotate each octave so the lattices do not line up.
        let turned = Vec2::new(0.8, 0.6).rotate(point * frequency);
        sum += amplitude * value_noise2(turned, salt.wrapping_add(octave * 0x9E37));
        total += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }
    sum / total.max(1e-6)
}

#[cfg(test)]
mod tests {
    #[test]
    fn plane_noise_is_continuous_bounded_and_repeatable() {
        use super::{Vec2, fractal_noise2, value_noise2};
        let samples = (0..2000)
            .map(|i| value_noise2(Vec2::new(i as f32 * 0.01, 3.3 - i as f32 * 0.007), 5))
            .collect::<Vec<_>>();
        assert!(samples.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(
            samples
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < 0.05)
        );
        let at = Vec2::new(12.25, -7.5);
        assert_eq!(fractal_noise2(at, 5, 1), fractal_noise2(at, 5, 1));
        assert_ne!(fractal_noise2(at, 5, 1), fractal_noise2(at, 5, 2));
        assert!((0.0..=1.0).contains(&fractal_noise2(at, 5, 1)));
        // Whole coordinates are the hashed lattice values.
        assert_eq!(value_noise2(Vec2::new(2.0, -1.0), 9), {
            let row = super::hash((-1_i64) as u32, 9);
            super::hash(2, 9 ^ row.to_bits())
        });
    }

    #[test]
    fn hashes_are_stable_and_spread() {
        let values = (0..1000).map(|i| super::hash(i, 7)).collect::<Vec<_>>();
        assert_eq!(
            values,
            (0..1000).map(|i| super::hash(i, 7)).collect::<Vec<_>>()
        );
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "mean {mean}");
        assert_ne!(super::hash(1, 7), super::hash(1, 8), "the salt matters");
    }

    #[test]
    fn smooth_noise_is_continuous_and_bounded() {
        let samples = (0..4000)
            .map(|i| super::smooth_noise(i as f32 * 0.001, 3))
            .collect::<Vec<_>>();
        assert!(samples.iter().all(|v| (-1.0..=1.0).contains(v)));
        assert!(
            samples
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < 0.01)
        );
    }
}
