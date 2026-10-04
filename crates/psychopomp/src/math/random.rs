//! Deterministic noise: the same inputs give the same value in any frame order.

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

/// Smooth 2D value noise in 0..1: hashed values on the integer lattice,
/// joined by smoothstep. Integer hashing makes it bit-identical to
/// `fx_lattice2` in the shaders' `effects/dissolve.wgsl`, so CPU particles
/// and GPU pixels agree on the same field.
pub fn lattice_noise2(x: f32, y: f32, salt: u32) -> f32 {
    let (cx, cy) = (x.floor(), y.floor());
    let (fx, fy) = (x - cx, y - cy);
    let at = |dx: i32, dy: i32| {
        let ix = (cx as i32).wrapping_add(dx) as u32;
        let iy = (cy as i32).wrapping_add(dy) as u32;
        hash(ix.wrapping_add(iy.wrapping_mul(0x27D4_EB2F)), salt)
    };
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let bottom = at(0, 0) + (at(1, 0) - at(0, 0)) * ux;
    let top = at(0, 1) + (at(1, 1) - at(0, 1)) * ux;
    bottom + (top - bottom) * uy
}

/// Three octaves of [`lattice_noise2`], normalized to 0..1.
pub fn lattice_fbm2(x: f32, y: f32, salt: u32) -> f32 {
    let mut sum = 0.0;
    let (mut px, mut py, mut amplitude) = (x, y, 0.5);
    for octave in 0..3 {
        sum += lattice_noise2(px, py, salt.wrapping_add(octave)) * amplitude;
        (px, py) = (px * 2.03 + 17.0, py * 2.03 + 5.0);
        amplitude *= 0.5;
    }
    sum / 0.875
}

#[cfg(test)]
mod tests {
    #[test]
    fn lattice_noise_is_continuous_bounded_and_hits_its_lattice() {
        use super::{hash, lattice_fbm2, lattice_noise2};
        assert_eq!(
            lattice_noise2(3.0, -2.0, 9),
            hash(
                3_u32.wrapping_add((-2_i32 as u32).wrapping_mul(0x27D4_EB2F)),
                9
            )
        );
        let mut previous = lattice_fbm2(0.0, 0.3, 4);
        for i in 1..4000 {
            let value = lattice_fbm2(i as f32 * 0.002, 0.3, 4);
            assert!((0.0..1.0).contains(&value));
            assert!((value - previous).abs() < 0.02);
            previous = value;
        }
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
