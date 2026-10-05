//! Thin-surface optics for glass: how far a ray falling straight down through
//! a tilted surface lands sideways, and the slope of a rounded bevel.

/// Crown glass.
pub const GLASS_IOR: f32 = 1.5;

/// Sideways travel per unit of depth for a ray falling straight down through a
/// surface of `slope` (rise over run) into a medium of index `ior`: the tangent
/// of its deviation, `θ - asin(sin θ / ior)`. Zero through flat glass, rising
/// toward `tan(90° - asin(1 / ior))` (1.118 for crown glass) as the surface
/// turns vertical.
pub fn refraction_offset(slope: f32, ior: f32) -> f32 {
    let tilt = slope.abs().atan();
    let bent = (tilt.sin() / ior).asin();
    (tilt - bent).tan()
}

/// The slope of a superellipse bevel, `(1 - (1 - t)^power)^(1 / power)`, that
/// rises from a vertical rim (`t` = 0) to a flat top (`t` = 1), in heights per
/// width. Power 2 is a quarter circle; higher powers keep the top flatter and
/// turn the rim more sharply. The slope reaches zero smoothly at the top, so a
/// flat center meets the bevel without a crease.
pub fn superellipse_slope(t: f32, power: f32) -> f32 {
    let u = (1.0 - t).clamp(0.0, 1.0);
    if u >= 1.0 {
        return f32::INFINITY;
    }
    u.powf(power - 1.0) * (1.0 - u.powf(power)).powf(1.0 / power - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_glass_does_not_bend_and_a_vertical_face_bends_most() {
        assert_eq!(refraction_offset(0.0, GLASS_IOR), 0.0);
        let limit = refraction_offset(f32::INFINITY, GLASS_IOR);
        assert!((limit - 1.118).abs() < 1e-3, "{limit}");
        let mut previous = 0.0;
        for slope in [0.1, 0.5, 1.0, 3.0, 30.0] {
            let offset = refraction_offset(slope, GLASS_IOR);
            assert!(offset > previous && offset < limit);
            previous = offset;
        }
        assert_eq!(
            refraction_offset(-2.0, GLASS_IOR),
            refraction_offset(2.0, GLASS_IOR)
        );
        assert!(
            refraction_offset(2.0, 1.0).abs() < 1e-6,
            "no index change, no bend"
        );
    }

    #[test]
    fn a_bevel_rises_from_vertical_to_flat() {
        assert_eq!(superellipse_slope(0.0, 3.0), f32::INFINITY);
        assert_eq!(superellipse_slope(1.0, 3.0), 0.0);
        assert_eq!(
            superellipse_slope(1.4, 3.0),
            0.0,
            "beyond the bevel is flat"
        );
        let circle = superellipse_slope(0.5, 2.0);
        assert!((circle - 0.5 / 0.75_f32.sqrt()).abs() < 1e-5);
        assert!(
            superellipse_slope(0.6, 4.0) < superellipse_slope(0.6, 2.0),
            "higher powers keep the top flatter"
        );
        let near = superellipse_slope(0.999, 3.0);
        assert!(near < 1e-5, "the slope fades out without a crease: {near}");
    }
}
