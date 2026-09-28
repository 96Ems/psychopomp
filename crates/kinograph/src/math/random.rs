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

#[cfg(test)]
mod tests {
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
}
