//! Camera shake from trauma (after Squirrel Eiserloh's "juicy" camera talk).
//! Trauma is 0..1 for ordinary hits; motion grows with its square, so a light
//! knock stays subtle and a full hit jolts. Sustained quakes may overdrive it
//! up to 2 (four times a full hit). Smooth noise rather than sines, so the
//! rumble never repeats or reads as a wobble.
use crate::math::{Vec2, random::smooth_noise, vec2};

/// Peak rumble offset in world pixels at full trauma.
pub const MAX_OFFSET: f32 = 10.0;
/// Peak roll in radians (about 0.45 degrees) at full trauma.
pub const MAX_ROLL: f32 = 0.008;
/// Noise frequency in hertz: fast enough to jolt, slow enough that a
/// 180-degree shutter keeps each excursion legible.
const FREQUENCY: f32 = 16.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rumble {
    pub offset: Vec2,
    pub roll: f32,
}

/// Highest trauma: a sustained quake's overdrive.
pub const MAX_TRAUMA: f32 = 2.0;

/// The rumble at scene time `time` for a trauma level.
pub fn rumble(time: f32, trauma: f32) -> Rumble {
    let shake = trauma.clamp(0.0, MAX_TRAUMA).powi(2);
    if shake == 0.0 {
        return Rumble::default();
    }
    let t = time * FREQUENCY;
    let octave = |salt: u32| smooth_noise(t, salt) * 0.7 + smooth_noise(t * 2.1, salt + 7) * 0.3;
    Rumble {
        offset: vec2(octave(1), octave(2)) * (MAX_OFFSET * shake),
        roll: octave(3) * MAX_ROLL * shake,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_trauma_is_still_and_full_trauma_stays_bounded() {
        assert_eq!(rumble(1.3, 0.0), Rumble::default());
        for i in 0..2000 {
            let r = rumble(i as f32 / 240.0, 1.0);
            assert!(r.offset.length() <= MAX_OFFSET * 1.5 && r.roll.abs() <= MAX_ROLL);
        }
        let quarter = rumble(0.37, 0.5).offset.length();
        let full = rumble(0.37, 1.0).offset.length();
        assert!(
            (quarter * 4.0 - full).abs() < 1e-4,
            "motion grows with trauma squared"
        );
    }

    #[test]
    fn overdrive_quadruples_a_full_hit_and_then_stops_growing() {
        let full = rumble(0.37, 1.0);
        let overdrive = rumble(0.37, MAX_TRAUMA);
        assert!((overdrive.offset.length() - full.offset.length() * 4.0).abs() < 1e-4);
        assert!((overdrive.roll - full.roll * 4.0).abs() < 1e-6);
        assert_eq!(rumble(0.37, 9.0), overdrive);
    }
}
