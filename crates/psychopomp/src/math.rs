//! Shared math for motion graphics, grouped like pmndrs `math`: scalar
//! interpolation here, then easing curves, arc-length paths, shapes with
//! connector ports, and deterministic noise. Vectors are glam's.
//!
//! Renderers and Scene Programs compose these instead of inlining their own
//! lerps, easings, or geometry.
pub mod curve;
pub mod dynamics;
pub mod easing;
pub mod optics;
pub mod random;
pub mod shapes;

pub use glam::{Quat, Vec2, Vec3, vec2, vec3};

/// `a` at 0 and `b` at 1, extrapolating outside that range.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Where `value` sits from `a` (0) to `b` (1); zero for an empty range.
pub fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a == b { 0.0 } else { (value - a) / (b - a) }
}

/// `value` mapped from the `from` range onto the `to` range, extrapolating.
pub fn remap(value: f32, from: [f32; 2], to: [f32; 2]) -> f32 {
    lerp(to[0], to[1], inverse_lerp(from[0], from[1], value))
}

/// Like [`remap`], held at the ends of the `to` range: "fade in while the
/// packet covers its first eighth" is `remap_clamp(travel, [0.0, 0.125], [0.0, 1.0])`.
pub fn remap_clamp(value: f32, from: [f32; 2], to: [f32; 2]) -> f32 {
    lerp(
        to[0],
        to[1],
        inverse_lerp(from[0], from[1], value).clamp(0.0, 1.0),
    )
}

/// The piecewise-linear curve through `stops` (ascending x) at `x`, held flat
/// beyond its ends: gradient stops, falloffs, and envelopes.
pub fn stops(x: f32, stops: &[(f32, f32)]) -> f32 {
    let Some(&(first_x, first_y)) = stops.first() else {
        return 0.0;
    };
    if x <= first_x {
        return first_y;
    }
    stops.windows(2).find(|pair| x <= pair[1].0).map_or_else(
        || stops[stops.len() - 1].1,
        |pair| lerp(pair[0].1, pair[1].1, inverse_lerp(pair[0].0, pair[1].0, x)),
    )
}

/// Hermite ease from 0 to 1. Unlike the easing curves, it clamps `t`.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_round_trips_and_clamps_only_when_asked() {
        assert_eq!(lerp(10.0, 20.0, 0.25), 12.5);
        assert_eq!(lerp(10.0, 20.0, 1.5), 25.0);
        assert_eq!(inverse_lerp(10.0, 20.0, 12.5), 0.25);
        assert_eq!(inverse_lerp(3.0, 3.0, 7.0), 0.0);
        assert_eq!(remap(0.5, [0.0, 1.0], [100.0, 200.0]), 150.0);
        assert_eq!(remap(2.0, [0.0, 1.0], [100.0, 200.0]), 300.0);
        assert_eq!(remap_clamp(2.0, [0.0, 1.0], [100.0, 200.0]), 200.0);
        assert_eq!(remap_clamp(-1.0, [0.0, 1.0], [200.0, 100.0]), 200.0);
    }

    #[test]
    fn stops_interpolate_and_hold_at_the_ends() {
        let falloff = [(0.0, 1.0), (0.3, 0.65), (0.7, 0.16), (1.0, 0.0)];
        assert_eq!(stops(-1.0, &falloff), 1.0);
        assert_eq!(stops(0.15, &falloff), 0.825);
        assert!((stops(0.5, &falloff) - 0.405).abs() < 1e-6);
        assert_eq!(stops(2.0, &falloff), 0.0);
        assert_eq!(stops(0.5, &[]), 0.0);
    }

    #[test]
    fn smoothstep_is_flat_at_both_ends() {
        assert_eq!(
            (smoothstep(-1.0), smoothstep(0.5), smoothstep(2.0)),
            (0.0, 0.5, 1.0)
        );
        assert!(smoothstep(0.01) < 0.001 && smoothstep(0.99) > 0.999);
    }
}
