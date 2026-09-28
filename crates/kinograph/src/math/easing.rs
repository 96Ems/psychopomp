//! Easing curves over `t` in 0..1, named like pmndrs `math/time`. They do not
//! clamp, so a caller can deliberately extrapolate.

/// Fast start, gentle landing: most entrances and travel.
pub fn cubic_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Gentle start and landing with a quick middle: camera moves and zooms.
pub fn cubic_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easings_run_from_zero_to_one() {
        for ease in [cubic_out, cubic_in_out] {
            assert_eq!((ease(0.0), ease(1.0)), (0.0, 1.0));
        }
        assert!(cubic_out(0.5) > 0.5, "ease-out is ahead of linear");
        assert_eq!(cubic_in_out(0.5), 0.5);
        assert!(cubic_in_out(0.25) < 0.25 && cubic_in_out(0.75) > 0.75);
    }
}
