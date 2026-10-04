//! Cuts hidden by light or corruption: a flash whites out over the cut, a
//! warm light leak drifts across it, and a glitch tears the picture apart for
//! a few frames around it.
use psychopomp::{
    math::{random::hash, vec2},
    plan::{
        TransitionPhase,
        transition::{
            CUT_AT, FLASH_PEAK, flash_intensity, glitch_frame, glitch_strength, leak_mix,
            leak_strength,
        },
    },
};

use super::{Frames, Paint, add, linear, mix, paint_rows, put, scale};

/// How far a flash overexposes the frame before it whites out.
const FLASH_GAIN: f32 = 6.0;

pub(super) fn flash(frames: Frames, phase: TransitionPhase, paint: Paint) -> Vec<u8> {
    let Frames {
        outgoing,
        incoming,
        width,
        height,
    } = frames;
    let intensity = flash_intensity(phase.progress);
    let source = if phase.progress < FLASH_PEAK {
        outgoing
    } else {
        incoming
    };
    // The theme's ink, lifted toward white: a warm flash in a warm theme.
    let white = mix(paint.ink, [1.0; 3], 0.6);
    let gain = 1.0 + FLASH_GAIN * intensity;
    let wash = intensity.powf(1.8);
    paint_rows(width, height, |y, row| {
        for (x, pixel) in row.iter_mut().enumerate() {
            let rgb = scale(linear(source, y * width + x), gain);
            put(pixel, mix(rgb.map(|value| value.min(1.0)), white, wash));
        }
    })
}

/// One glow of a light leak: where it drifts from and to (fractions of the
/// frame), its radii against the frame height, its color, and strength.
struct Glow {
    from: [f32; 2],
    to: [f32; 2],
    radii: [f32; 2],
    color: [f32; 3],
    weight: f32,
}

const fn glow(from: [f32; 2], to: [f32; 2], radii: [f32; 2], color: [f32; 3], weight: f32) -> Glow {
    Glow {
        from,
        to,
        radii,
        color,
        weight,
    }
}

/// The leak burns in from the left edge, deep red outside and hot inside.
const LEAKS: [Glow; 5] = [
    glow(
        [-0.30, 0.30],
        [0.80, 0.40],
        [1.15, 0.70],
        [1.0, 0.18, 0.04],
        1.1,
    ),
    glow(
        [-0.20, 0.80],
        [1.10, 0.62],
        [0.95, 0.50],
        [0.85, 0.08, 0.10],
        0.9,
    ),
    glow(
        [-0.25, 0.45],
        [0.70, 0.50],
        [0.55, 0.40],
        [1.0, 0.55, 0.12],
        1.2,
    ),
    glow(
        [-0.15, 0.50],
        [0.62, 0.48],
        [0.30, 0.22],
        [1.0, 0.86, 0.58],
        1.6,
    ),
    glow(
        [0.10, -0.05],
        [0.95, 0.05],
        [0.80, 0.22],
        [1.0, 0.42, 0.10],
        0.7,
    ),
];
/// How brightly the leak burns at its peak, before it is screened in.
const LEAK_GAIN: f32 = 2.4;

pub(super) fn leak(frames: Frames, phase: TransitionPhase, paint: Paint) -> Vec<u8> {
    let Frames {
        outgoing,
        incoming,
        width,
        height,
    } = frames;
    let (strength, drift) = leak_strength(phase.progress);
    let swap = leak_mix(phase.progress);
    let size = vec2(width as f32, height as f32);
    let glows = LEAKS.map(
        |Glow {
             from,
             to,
             radii,
             color,
             weight,
         }| {
            let at = vec2(
                from[0] + (to[0] - from[0]) * drift,
                from[1] + (to[1] - from[1]) * drift,
            ) * size;
            // Each glow leans a little toward the theme's accent.
            let color = mix(color, paint.accent, 0.12);
            (
                at,
                vec2(radii[0], radii[1]) * size.y,
                scale(color, weight * strength * LEAK_GAIN),
            )
        },
    );
    paint_rows(width, height, |y, row| {
        for (x, pixel) in row.iter_mut().enumerate() {
            let index = y * width + x;
            let base = mix(linear(outgoing, index), linear(incoming, index), swap);
            let point = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let mut light = [0.0_f32; 3];
            for (at, radii, color) in glows {
                let reach = ((point - at) / radii).length_squared();
                light = add(light, scale(color, (-reach).exp()));
            }
            // Screen the light over the picture: it brightens without clipping.
            let leak = light.map(|value| 1.0 - (-value).exp());
            put(
                pixel,
                std::array::from_fn(|channel| 1.0 - (1.0 - base[channel]) * (1.0 - leak[channel])),
            );
        }
    })
}

/// The widest a torn band shifts sideways, and the widest color split, in pixels.
const GLITCH_SHIFT: f32 = 180.0;
const GLITCH_SPLIT: f32 = 16.0;
/// Corrupted macroblocks are multiples of this many pixels.
const BLOCK: usize = 16;

/// One horizontal band of a glitch frame.
struct Band {
    end: usize,
    shift: i64,
    /// Shows the frame on the other side of the cut.
    swapped: bool,
    split: i64,
    /// Repeats one row of the source, like a stalled decoder.
    smeared: bool,
    /// Reads its color planes out of order.
    misread: bool,
}

/// A corrupted macroblock: its pixels come from somewhere else, maybe from
/// the frame on the other side of the cut.
struct Block {
    x: std::ops::Range<usize>,
    y: std::ops::Range<usize>,
    from: [i64; 2],
    swapped: bool,
}

pub(super) fn glitch(frames: Frames, phase: TransitionPhase, paint: Paint) -> Vec<u8> {
    let Frames {
        outgoing,
        incoming,
        width,
        height,
    } = frames;
    let strength = glitch_strength(phase.progress);
    let seed = glitch_frame(phase.progress, phase.seconds).wrapping_mul(7919) ^ 0x5EED;
    let random = |index: u32, salt: u32| hash(index, seed.wrapping_add(salt));
    let after = phase.progress >= CUT_AT;
    let (main, other) = if after {
        (incoming, outgoing)
    } else {
        (outgoing, incoming)
    };
    // The bands of this glitch frame, top to bottom.
    let mut bands = Vec::new();
    let mut top = 0;
    while top < height {
        let index = bands.len() as u32;
        let tall = 8.0 + random(index, 1).powi(2) * 120.0;
        let end = (top + (tall as usize).max(4)).min(height);
        let torn = random(index, 2) < strength * 0.6;
        bands.push(Band {
            end,
            shift: if torn {
                (((random(index, 3) - 0.5) * 2.0 * strength * GLITCH_SHIFT) / 4.0).round() as i64
                    * 4
            } else {
                0
            },
            swapped: random(index, 4) < strength * 0.3,
            split: (strength * GLITCH_SPLIT * (0.5 + random(index, 5))).round() as i64,
            smeared: torn && random(index, 6) < 0.18,
            misread: torn && random(index, 7) < 0.3,
        });
        top = end;
    }
    let blocks = (0..(strength * 7.0) as u32)
        .map(|index| {
            let salt = 100 + index * 8;
            let cells = |value: f32, count: usize| (value * count as f32) as usize * BLOCK;
            let (x, y) = (
                cells(random(index, salt), width / BLOCK),
                cells(random(index, salt + 1), height / BLOCK),
            );
            let (w, h) = (
                BLOCK * (2 + (random(index, salt + 2) * 10.0) as usize),
                BLOCK * (1 + (random(index, salt + 3) * 4.0) as usize),
            );
            let offset = |value: f32| ((value - 0.5) * 24.0).round() as i64 * BLOCK as i64;
            Block {
                x: x..(x + w).min(width),
                y: y..(y + h).min(height),
                from: [
                    offset(random(index, salt + 4)),
                    offset(random(index, salt + 5)) / 4,
                ],
                swapped: random(index, salt + 6) < 0.5,
            }
        })
        .collect::<Vec<_>>();
    // Torn bands take a faint cast of the accent; it tints, never lights.
    let brightest = paint.accent.into_iter().fold(1e-3, f32::max);
    let cast = paint.accent.map(|value| value / brightest);
    let (bands, blocks) = (&bands, &blocks);
    paint_rows(width, height, |y, row| {
        let band_index = bands.partition_point(|band| band.end <= y);
        let band = &bands[band_index.min(bands.len() - 1)];
        let source_y = if band.smeared {
            bands
                .get(band_index.wrapping_sub(1))
                .map_or(y, |above| above.end)
                .min(height - 1)
        } else {
            y
        };
        let scanline = if y % 3 == 0 {
            1.0 - 0.18 * strength
        } else {
            1.0
        };
        for (x, pixel) in row.iter_mut().enumerate() {
            let block = blocks
                .iter()
                .find(|block| block.x.contains(&x) && block.y.contains(&y));
            let (source, sx, sy) = match block {
                Some(block) => (
                    if block.swapped { other } else { main },
                    x as i64 + block.from[0],
                    (y as i64 + block.from[1]).clamp(0, height as i64 - 1) as usize,
                ),
                None => (
                    if band.swapped { other } else { main },
                    x as i64 + band.shift,
                    source_y,
                ),
            };
            let at = |dx: i64| {
                let sx = (sx + dx).clamp(0, width as i64 - 1) as usize;
                linear(source, sy * width + sx)
            };
            let (red, green, blue) = (at(band.split), at(0), at(-band.split));
            let mut rgb = if band.misread {
                [green[1], blue[2], red[0]]
            } else {
                [red[0], green[1], blue[2]]
            };
            if band.shift != 0 {
                rgb = std::array::from_fn(|channel| {
                    rgb[channel] * (1.0 + (cast[channel] - 1.0) * 0.35 * strength)
                });
            }
            let grain =
                1.0 + (hash((y * width + x) as u32, seed.wrapping_add(17)) - 0.5) * 0.3 * strength;
            put(pixel, rgb.map(|value| value * scanline * grain));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::LEAKS;

    #[test]
    fn leaks_drift_across_the_frame() {
        for glow in LEAKS {
            assert!(glow.to[0] > glow.from[0], "every glow drifts rightward");
            assert!(glow.radii.iter().all(|radius| *radius > 0.0) && glow.weight > 0.0);
        }
    }
}
