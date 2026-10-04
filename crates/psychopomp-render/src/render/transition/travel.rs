//! Pushes, slides, and whips: frames travel along one axis, each smeared along
//! it by its own speed. A smear is a box filter over the line, integrated from
//! prefix sums, so its length can be any fraction of a pixel or a whole frame
//! at the same cost.
use psychopomp::{
    math::smoothstep,
    plan::{
        TransitionPhase, WipeDirection,
        transition::{push_travel, slide_travel, whip_travel},
    },
};

use super::{Frames, Paint, SAMPLE_GAP, add, linear, mix, paint_rows, par_runs, put, scale};

pub(super) enum Kind {
    Push,
    Slide,
    Whip,
}

/// How fast the outgoing frame drifts back under a slide, against the incoming.
const PARALLAX: f32 = 0.3;
/// How much a slide darkens the frame it covers, and the shadow its edge casts.
const SLIDE_DIM: f32 = 0.45;
const SHADOW_OPACITY: f32 = 0.5;
const SHADOW_REACH: f32 = 70.0;
/// A whip exposes each sample this long, like a slow shutter on a fast pan.
const WHIP_EXPOSURE: f32 = 1.4 * crate::exposure::SHUTTER_SECONDS as f32;
/// The faint long tail of a whip's streak, against its average speed.
const STREAK_LENGTH: f32 = 3.0;
const STREAK_MIX: f32 = 0.35;

pub(super) fn travel(
    frames: Frames,
    phase: TransitionPhase,
    direction: WipeDirection,
    kind: Kind,
    paint: Paint,
) -> Vec<u8> {
    let Frames {
        outgoing,
        incoming,
        width,
        height,
    } = frames;
    let horizontal = matches!(direction, WipeDirection::Left | WipeDirection::Right);
    let sign = if matches!(direction, WipeDirection::Left | WipeDirection::Up) {
        -1.0
    } else {
        1.0
    };
    let (length, lines) = if horizontal {
        (width, height)
    } else {
        (height, width)
    };
    let extent = length as f32;
    let travel = match kind {
        Kind::Push => push_travel,
        Kind::Slide => slide_travel,
        Kind::Whip => whip_travel,
    }(phase.progress);
    let covered = travel.position;
    // Pixels per second of the incoming frame.
    let speed = travel.rate * extent / phase.seconds.max(1e-3);
    let (parallax, exposure) = match kind {
        Kind::Push => (1.0, SAMPLE_GAP),
        Kind::Slide => (PARALLAX, SAMPLE_GAP),
        Kind::Whip => (1.0, SAMPLE_GAP + WHIP_EXPOSURE),
    };
    let incoming_smear = (speed * exposure).max(1.0);
    let outgoing_smear = (speed * parallax * exposure).max(1.0);
    // A layer's own coordinate is the screen coordinate plus its shift.
    let outgoing_shift = -sign * parallax * covered * extent;
    let incoming_shift = sign * (1.0 - covered) * extent;
    // The incoming frame's edge over the outgoing one, for a slide's shadow.
    let edge = if sign < 0.0 {
        (1.0 - covered) * extent
    } else {
        covered * extent
    };
    let streak = match kind {
        Kind::Whip => STREAK_MIX * (travel.rate / 3.5).min(1.0),
        _ => 0.0,
    };
    let mut values = vec![[0.0_f32; 3]; length * lines];
    par_runs(&mut values, length, |line, run| {
        let index = |a: usize| {
            if horizontal {
                line * width + a
            } else {
                a * width + line
            }
        };
        let outgoing = Line::new(outgoing, length, index);
        let incoming = Line::new(incoming, length, index);
        for (a, value) in run.iter_mut().enumerate() {
            let at = a as f32 + 0.5;
            let (mut front, front_alpha) = incoming.smeared(at + incoming_shift, incoming_smear);
            let (mut back, back_alpha) = outgoing.smeared(at + outgoing_shift, outgoing_smear);
            if streak > 0.0 {
                let (long, _) =
                    incoming.smeared(at + incoming_shift, incoming_smear * STREAK_LENGTH);
                front = mix(front, long, streak);
                let (long, _) =
                    outgoing.smeared(at + outgoing_shift, outgoing_smear * STREAK_LENGTH);
                back = mix(back, long, streak);
            }
            let mut below = add(back, scale(paint.background, 1.0 - back_alpha));
            if let Kind::Slide = kind {
                let distance = (if sign < 0.0 { edge - at } else { at - edge }).max(0.0);
                // The shadow grows as the edge enters and fades as it lands.
                let entered = smoothstep(covered * extent / SHADOW_REACH);
                let shadow = SHADOW_OPACITY
                    * entered
                    * (1.0 - covered).sqrt()
                    * (-(distance / SHADOW_REACH).powi(2) * 2.0).exp();
                below = scale(below, (1.0 - SLIDE_DIM * covered) * (1.0 - shadow));
            }
            *value = add(front, scale(below, 1.0 - front_alpha));
        }
    });
    from_lines(&values, horizontal, width, height)
}

/// One line of a frame in linear light, as running sums for box filtering.
struct Line {
    sums: Vec<[f64; 3]>,
}

impl Line {
    fn new(pixels: &[u8], length: usize, index: impl Fn(usize) -> usize) -> Self {
        let mut sums = Vec::with_capacity(length + 1);
        let mut total = [0.0_f64; 3];
        sums.push(total);
        for a in 0..length {
            let rgb = linear(pixels, index(a));
            for channel in 0..3 {
                total[channel] += f64::from(rgb[channel]);
            }
            sums.push(total);
        }
        Self { sums }
    }

    /// The premultiplied color and coverage of a `length`-pixel box centered
    /// at `center` along the line; the line is empty outside its extent.
    fn smeared(&self, center: f32, length: f32) -> ([f32; 3], f32) {
        let extent = (self.sums.len() - 1) as f64;
        let half = f64::from(length) * 0.5;
        let from = (f64::from(center) - half).clamp(0.0, extent);
        let to = (f64::from(center) + half).clamp(0.0, extent);
        if to <= from {
            return ([0.0; 3], 0.0);
        }
        let integral = |at: f64| {
            let index = (at.floor() as usize).min(self.sums.len() - 2);
            let fraction = at - index as f64;
            let (low, high) = (self.sums[index], self.sums[index + 1]);
            [0, 1, 2].map(|channel| low[channel] + fraction * (high[channel] - low[channel]))
        };
        let (start, end) = (integral(from), integral(to));
        let length = f64::from(length);
        (
            [0, 1, 2].map(|channel| ((end[channel] - start[channel]) / length) as f32),
            ((to - from) / length) as f32,
        )
    }
}

/// Lines of linear color (rows, or columns when not `horizontal`) as an
/// sRGB frame.
fn from_lines(values: &[[f32; 3]], horizontal: bool, width: usize, height: usize) -> Vec<u8> {
    paint_rows(width, height, |y, row| {
        for (x, pixel) in row.iter_mut().enumerate() {
            let index = if horizontal {
                y * width + x
            } else {
                x * height + y
            };
            put(pixel, values[index]);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::Line;

    #[test]
    fn a_one_pixel_box_reads_pixels_exactly_and_longer_boxes_average() {
        let pixels = [0_u8, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255];
        let line = Line::new(&pixels, 3, |a| a);
        let (rgb, alpha) = line.smeared(1.5, 1.0);
        assert_eq!((rgb, alpha), ([1.0; 3], 1.0));
        let (rgb, alpha) = line.smeared(1.5, 3.0);
        assert!((rgb[0] - 1.0 / 3.0).abs() < 1e-6 && alpha == 1.0);
        // Half a pixel past the end is half covered.
        let (rgb, alpha) = line.smeared(3.0, 1.0);
        assert_eq!((rgb, alpha), ([0.0; 3], 0.5));
        assert_eq!(line.smeared(-4.0, 1.0), ([0.0; 3], 0.0));
    }
}
