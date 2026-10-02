//! A contact dimple and a damped wave traveling over a spherical surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceResponse {
    /// Radial offset in world pixels on a 150-pixel-radius sphere.
    pub displacement: f32,
    /// Local emissive response, strongest at the traveling wavefront.
    pub light: f32,
}

/// `angle` is the geodesic angle from the contact point, in radians.
pub fn impact(age: f32, angle: f32) -> SurfaceResponse {
    if !(0.0..1.4).contains(&age) {
        return SurfaceResponse::default();
    }
    let attack = 1.0 - (-age / 0.025).exp();
    let decay = (-age / 0.48).exp() * (1.0 - crate::math::smoothstep((age - 1.0) / 0.4));
    let front = 3.6 * age;
    let wave = (-((angle - front) / 0.22).powi(2)).exp() * attack * decay;
    let contact = (-(angle / 0.36).powi(2)).exp() * attack * (-age / 0.12).exp();
    SurfaceResponse {
        displacement: 2.5 * wave - 5.0 * contact,
        light: 2.2 * wave + 1.2 * contact,
    }
}

/// Traveling ring angle and emission, for renderers with a continuous surface trace.
pub fn wavefront(age: f32) -> (f32, f32) {
    let angle = 3.6 * age;
    (angle, impact(age, angle).light)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_starts_locally_then_propagates_and_settles() {
        assert_eq!(impact(0.0, 0.0), SurfaceResponse::default());
        assert!(impact(0.06, 0.2).light > impact(0.06, 1.5).light * 100.0);
        assert!(impact(0.35, 1.26).light > impact(0.35, 0.0).light * 10.0);
        assert_eq!(impact(1.4, 2.0), SurfaceResponse::default());
        assert_eq!(impact(-0.1, 0.0), SurfaceResponse::default());
    }
}
