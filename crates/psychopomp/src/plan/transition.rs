//! Composited reel transitions: the closed-form timing and geometry of pushes,
//! slides, whips, irises, matched zooms, card flips, cubes, ink, glitches,
//! flashes, and light leaks. Each is a pure function of the transition's
//! progress, so a frame samples the same pose in any order and shutter
//! samples blur real motion. The renderer turns these poses into pixels.
use std::f32::consts::PI;

use super::{ReelTransitionStyle, ReelZoom};
use crate::math::{Vec2, dynamics::settle, easing::smootherstep, lerp, smoothstep, vec2};

/// The incoming layer's part in a composited transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionPhase {
    pub style: ReelTransitionStyle,
    /// Linear progress through the transition, 0 to 1.
    pub progress: f32,
    /// The transition's length, to turn rates per progress into speeds.
    pub seconds: f32,
    /// `transitionFocus`, for the styles that start from a rectangle.
    pub focus: Option<[f32; 4]>,
}

/// How far a push, slide, or whip has carried the incoming frame across (0
/// to 1 of the frame), and its rate of change per unit of progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Travel {
    pub position: f32,
    pub rate: f32,
}

/// A minimum-jerk push: both frames leave and arrive at rest.
pub fn push_travel(progress: f32) -> Travel {
    let t = progress.clamp(0.0, 1.0);
    Travel {
        position: smootherstep(t),
        rate: smootherstep_slope(t),
    }
}

/// A slide arrives fast and settles like a critically damped spring, resting
/// exactly at the end.
pub fn slide_travel(progress: f32) -> Travel {
    let (position, rate) = settle(0.0, 1.0, 0.0, 1.0, progress.clamp(0.0, 1.0));
    Travel { position, rate }
}

/// A whip pan leans in, tears across at three and a half times the average
/// speed, and catches: minimum-jerk travel through a minimum-jerk clock.
pub fn whip_travel(progress: f32) -> Travel {
    let t = progress.clamp(0.0, 1.0);
    let inner = smootherstep(t);
    Travel {
        position: smootherstep(inner),
        rate: smootherstep_slope(inner) * smootherstep_slope(t),
    }
}

fn smootherstep_slope(t: f32) -> f32 {
    30.0 * t * t * (1.0 - t) * (1.0 - t)
}

/// The iris center: the focus rectangle's center, or the frame's.
pub fn iris_center(focus: Option<[f32; 4]>, size: Vec2) -> Vec2 {
    focus.map_or(size * 0.5, |[x, y, w, h]| vec2(x + w * 0.5, y + h * 0.5))
}

/// The iris radius at `progress`: zero at the start, and past the farthest
/// corner by `feather` at the end, so the frame is fully open.
pub fn iris_radius(progress: f32, center: Vec2, size: Vec2, feather: f32) -> f32 {
    let reach = [vec2(0.0, 0.0), vec2(size.x, 0.0), vec2(0.0, size.y), size]
        .into_iter()
        .map(|corner| corner.distance(center))
        .fold(0.0, f32::max);
    smootherstep(progress.clamp(0.0, 1.0)) * (reach + feather)
}

/// A card's turn: its rotation in radians, and how far (0..1 of the full
/// pull-back) the camera has backed away so its near edge stays in frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    pub angle: f32,
    pub lift: f32,
}

/// A flip turns the frame over: 180 degrees, pulling back most when the card
/// is edge-on.
pub fn flip_turn(progress: f32) -> Turn {
    let eased = smootherstep(progress.clamp(0.0, 1.0));
    Turn {
        angle: PI * eased,
        lift: (PI * eased).sin(),
    }
}

/// A cube turns a quarter so the next face comes to the front, pulling back
/// most when its edge points at the camera.
pub fn cube_turn(progress: f32) -> Turn {
    let eased = smootherstep(progress.clamp(0.0, 1.0));
    Turn {
        angle: PI * 0.5 * eased,
        lift: (PI * eased).sin(),
    }
}

impl ReelZoom {
    /// Where the layer sits at `progress` of a match from `from` in the
    /// outgoing frame to `to` in the incoming frame. One camera carries both:
    /// at the start the outgoing frame is at rest and the incoming frame is
    /// shrunk so `to` lies on `from`; at the end the incoming frame is at rest
    /// and the outgoing frame is scaled so `from` lies on `to`. The scale
    /// changes geometrically while the matched center travels in a straight
    /// line, so the element keeps one pace.
    pub fn matched(from: [f32; 4], to: [f32; 4], progress: f32, incoming: bool) -> Self {
        let eased = smootherstep(progress.clamp(0.0, 1.0));
        let (from_center, to_center) = (rect_center(from), rect_center(to));
        // Uniform scale between the rectangles by area, so neither side wins
        // when their shapes differ.
        let ratio = ((from[2] * from[3]) / (to[2] * to[3]).max(1.0)).sqrt();
        let anchor = from_center.lerp(to_center, eased);
        let (scale, pivot) = if incoming {
            (ratio.powf(1.0 - eased), to_center)
        } else {
            (ratio.powf(-eased), from_center)
        };
        Self {
            scale,
            offset: (anchor - pivot * scale).to_array(),
            radius: 0.0,
        }
    }
}

/// Where the matched element is on screen at `progress`: the incoming
/// element `to`, carried by the match's camera.
pub fn match_element(from: [f32; 4], to: [f32; 4], progress: f32) -> [f32; 4] {
    let zoom = ReelZoom::matched(from, to, progress, true);
    [
        to[0] * zoom.scale + zoom.offset[0],
        to[1] * zoom.scale + zoom.offset[1],
        to[2] * zoom.scale,
        to[3] * zoom.scale,
    ]
}

fn rect_center([x, y, w, h]: [f32; 4]) -> Vec2 {
    vec2(x + w * 0.5, y + h * 0.5)
}

/// How much of the incoming frame shows during a match: inside the matched
/// element, which turns into its counterpart early in the flight, and
/// around it, where the rest of the scene follows.
pub fn match_mix(progress: f32) -> (f32, f32) {
    (
        smoothstep((progress - 0.1) / 0.36),
        smoothstep((progress - 0.34) / 0.56),
    )
}

/// The ink threshold at `progress`: ink has covered every point whose field
/// value (0..1) is below it, with `edge` of softness on each side.
pub fn ink_threshold(progress: f32, edge: f32) -> f32 {
    lerp(-edge, 1.0 + edge, smoothstep(progress))
}

/// Where in a glitch or flash the picture cuts.
pub const CUT_AT: f32 = 0.5;

/// A glitch's strength: it stutters in, peaks at the cut, and drops out
/// quickly after, so the incoming frame lands clean.
pub fn glitch_strength(progress: f32) -> f32 {
    let t = progress.clamp(0.0, 1.0);
    if t <= CUT_AT {
        smoothstep(t / CUT_AT).powf(1.6)
    } else {
        (1.0 - smoothstep((t - CUT_AT) / (1.0 - CUT_AT))).powf(2.2)
    }
}

/// The glitch's discrete frame at `progress` of a `seconds`-long transition:
/// digital corruption holds for a few output frames, then jumps.
pub fn glitch_frame(progress: f32, seconds: f32) -> u32 {
    (progress.clamp(0.0, 1.0) * seconds * 24.0).floor() as u32
}

/// Where a flash peaks and the picture cuts beneath it.
pub const FLASH_PEAK: f32 = 0.24;

/// A flash's brightness: it swells into a white-out that hides the cut, then
/// decays convexly, fast and then with a long tail.
pub fn flash_intensity(progress: f32) -> f32 {
    let t = progress.clamp(0.0, 1.0);
    if t <= FLASH_PEAK {
        let rise = t / FLASH_PEAK;
        rise * rise * rise
    } else {
        (1.0 - (t - FLASH_PEAK) / (1.0 - FLASH_PEAK)).powi(3)
    }
}

/// A light leak's strength, swelling and fading around the middle, and how
/// far its glow has drifted across the frame.
pub fn leak_strength(progress: f32) -> (f32, f32) {
    let t = progress.clamp(0.0, 1.0);
    ((PI * t).sin().powf(1.5), smootherstep(t))
}

/// How much of the incoming frame shows under a light leak: the swap happens
/// while the leak is brightest.
pub fn leak_mix(progress: f32) -> f32 {
    smoothstep((progress - 0.34) / 0.32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_rests_at_both_ends_and_its_rate_is_the_derivative() {
        for travel in [push_travel, slide_travel, whip_travel] {
            assert_eq!(travel(0.0).position, 0.0);
            assert!((travel(1.0).position - 1.0).abs() < 1e-6);
            assert_eq!(travel(1.0).rate, 0.0);
            for step in 1..20 {
                let t = step as f32 / 20.0;
                let h = 1e-3;
                let numeric = (travel(t + h).position - travel(t - h).position) / (2.0 * h);
                assert!((numeric - travel(t).rate).abs() < 2e-2, "at {t}");
            }
        }
        assert_eq!(push_travel(0.0).rate, 0.0, "a push leaves from rest");
        assert!(slide_travel(0.2).position > 0.5, "a slide arrives early");
        let peak = (0..=100)
            .map(|step| whip_travel(step as f32 / 100.0).rate)
            .fold(0.0, f32::max);
        assert!(peak > 3.4, "a whip tears across the middle ({peak})");
        assert!(whip_travel(0.15).position < 0.01, "and leans in first");
    }

    #[test]
    fn the_iris_opens_from_its_center_past_the_farthest_corner() {
        let size = vec2(1920.0, 1080.0);
        let center = iris_center(Some([100.0, 100.0, 200.0, 100.0]), size);
        assert_eq!(center, vec2(200.0, 150.0));
        assert_eq!(iris_radius(0.0, center, size, 12.0), 0.0);
        let open = iris_radius(1.0, center, size, 12.0);
        assert!((open - (center.distance(size) + 12.0)).abs() < 1e-3);
        assert_eq!(iris_center(None, size), vec2(960.0, 540.0));
    }

    #[test]
    fn a_match_carries_one_rectangle_onto_the_other() {
        let from = [300.0, 400.0, 340.0, 124.0];
        let to = [560.0, 200.0, 800.0, 292.0];
        let map = |zoom: ReelZoom, [x, y, w, h]: [f32; 4]| {
            [
                x * zoom.scale + zoom.offset[0],
                y * zoom.scale + zoom.offset[1],
                w * zoom.scale,
                h * zoom.scale,
            ]
        };
        let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.05);
        // At rest at each end, and the element sits on its counterpart.
        assert!(close(
            map(ReelZoom::matched(from, to, 0.0, false), from),
            from
        ));
        assert!(close(map(ReelZoom::matched(from, to, 1.0, true), to), to));
        let outgoing = ReelZoom::matched(from, to, 1.0, false);
        let mapped = map(outgoing, from);
        let center = |[x, y, w, h]: [f32; 4]| [x + w * 0.5, y + h * 0.5];
        assert!(
            center(mapped)
                .iter()
                .zip(center(to))
                .all(|(a, b)| (a - b).abs() < 0.05)
        );
        assert!(
            (mapped[2] * mapped[3] - to[2] * to[3]).abs() < 1.0,
            "by area"
        );
        // Mid-flight, both layers place the shared element in the same spot.
        for progress in [0.25, 0.5, 0.8] {
            let a = map(ReelZoom::matched(from, to, progress, false), from);
            let b = map(ReelZoom::matched(from, to, progress, true), to);
            assert!(close(center4(a), center4(b)), "at {progress}");
        }
        // The element starts on the outgoing one and lands on its counterpart.
        let start = match_element(from, to, 0.0);
        assert!(close(center4(start), center4(from)));
        assert!((start[2] * start[3] - from[2] * from[3]).abs() < 1.0);
        assert!(close(match_element(from, to, 1.0), to));
        assert_eq!(match_mix(0.0), (0.0, 0.0));
        assert_eq!(match_mix(1.0), (1.0, 1.0));
        let (element, surroundings) = match_mix(0.4);
        assert!(element > surroundings, "the element turns first");
    }

    fn center4([x, y, w, h]: [f32; 4]) -> [f32; 4] {
        [x + w * 0.5, y + h * 0.5, 0.0, 0.0]
    }

    #[test]
    fn turns_rest_square_to_the_camera_at_both_ends() {
        assert_eq!(
            flip_turn(0.0),
            Turn {
                angle: 0.0,
                lift: 0.0
            }
        );
        let flipped = flip_turn(1.0);
        assert!((flipped.angle - PI).abs() < 1e-6 && flipped.lift.abs() < 1e-6);
        assert!(
            (flip_turn(0.5).lift - 1.0).abs() < 1e-6,
            "edge-on is farthest"
        );
        let cubed = cube_turn(1.0);
        assert!((cubed.angle - PI * 0.5).abs() < 1e-6 && cubed.lift.abs() < 1e-6);
        assert!((cube_turn(0.5).angle - PI * 0.25).abs() < 1e-6);
    }

    #[test]
    fn cuts_hide_under_their_peak() {
        assert_eq!(glitch_strength(0.0), 0.0);
        assert_eq!(glitch_strength(CUT_AT), 1.0);
        assert_eq!(glitch_strength(1.0), 0.0);
        assert!(
            glitch_strength(0.75) < glitch_strength(0.25),
            "it drops out fast"
        );
        assert_eq!(glitch_frame(0.5, 0.5), 6);
        assert_eq!(flash_intensity(0.0), 0.0);
        assert_eq!(flash_intensity(FLASH_PEAK), 1.0);
        assert_eq!(flash_intensity(1.0), 0.0);
        // Convex decay: below the straight line from the peak to the end.
        let halfway = (FLASH_PEAK + 1.0) * 0.5;
        assert!(flash_intensity(halfway) < 0.5 * 0.5);
        assert_eq!(leak_strength(0.0).0, 0.0);
        assert!((leak_strength(0.5).0 - 1.0).abs() < 1e-6);
        assert_eq!((leak_mix(0.0), leak_mix(1.0)), (0.0, 1.0));
        assert!(ink_threshold(0.0, 0.05) < 0.0 && ink_threshold(1.0, 0.05) > 1.0);
    }
}
