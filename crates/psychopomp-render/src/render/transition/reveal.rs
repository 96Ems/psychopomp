//! Reveals: the incoming frame shows through a growing shape, a circle for an
//! iris or an ink blot for ink, over the outgoing frame, which is shaded where
//! the edge passes.
use std::sync::{Arc, Mutex};

use psychopomp::{
    math::{Vec2, lerp, random::fractal_noise2, smoothstep, vec2},
    plan::{
        TransitionPhase,
        transition::{ink_threshold, iris_center, iris_radius},
    },
};

use super::{Frames, Mips, Paint, SAMPLE_GAP, add, linear, mix, paint_rows, put, scale};

/// Antialiasing plus a little softness on a revealing edge, in pixels.
const EDGE_SOFTNESS: f32 = 2.4;
/// The shadow the incoming frame's edge casts onto the outgoing one.
const SHADOW_OPACITY: f32 = 0.42;
const SHADOW_REACH: f32 = 34.0;
/// The iris ring: a crisp core and a wide halo, in pixels.
const RING_CORE: f32 = 1.6;
const RING_HALO: f32 = 11.0;
/// The incoming frame settles from this scale inside the iris.
const IRIS_SETTLE: f32 = 0.05;

pub(super) fn iris(frames: Frames, phase: TransitionPhase, ring: bool, paint: Paint) -> Vec<u8> {
    let Frames {
        outgoing,
        incoming,
        width,
        height,
    } = frames;
    let size = vec2(width as f32, height as f32);
    let center = iris_center(phase.focus, size);
    let reach = RING_HALO * 2.0 + SHADOW_REACH;
    let radius = iris_radius(phase.progress, center, size, reach);
    let step = 1e-3;
    let speed = (iris_radius(phase.progress + step, center, size, reach)
        - iris_radius(phase.progress - step, center, size, reach))
        / (2.0 * step)
        / phase.seconds.max(1e-3);
    let softness = EDGE_SOFTNESS + speed * SAMPLE_GAP;
    let progress = phase.progress;
    // The ring lights as the iris opens and cools before it leaves the frame.
    let glow = if ring {
        smoothstep(progress / 0.12) * (1.0 - smoothstep((progress - 0.55) / 0.4))
    } else {
        0.0
    };
    let settle = 1.0 + IRIS_SETTLE * (1.0 - psychopomp::math::easing::smootherstep(progress));
    let incoming = Mips::new(incoming, width, height, 1);
    let ring_color = mix(paint.accent, paint.ink, 0.35);
    paint_rows(width, height, |y, row| {
        for (x, pixel) in row.iter_mut().enumerate() {
            let point = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let distance = point.distance(center);
            // A circle smaller than its soft edge is that much fainter, so it
            // opens from nothing.
            let inside = (radius - distance) / softness;
            let cover = smoothstep(inside * 0.5 + 0.5) * (radius / softness).min(1.0);
            let below = linear(outgoing, y * width + x);
            let outside = (distance - radius).max(0.0);
            let shadow = SHADOW_OPACITY
                * smoothstep(radius / 40.0)
                * (-(outside / SHADOW_REACH).powi(2) * 2.0).exp();
            let mut rgb = scale(below, 1.0 - shadow * (1.0 - cover));
            if cover > 0.0 {
                let source = center + (point - center) / settle;
                rgb = mix(rgb, incoming.sample(source.x, source.y, 1.0), cover);
            }
            if glow > 0.0 {
                let offset = distance - radius;
                let light = (-(offset / RING_CORE).powi(2)).exp() * 0.75
                    + (-(offset / RING_HALO).powi(2)).exp() * 0.08;
                rgb = add(rgb, scale(ring_color, light * glow));
            }
            put(pixel, rgb);
        }
    })
}

/// How coarse the ink's noise field is, in pixels; it is smooth between.
const INK_CELL: usize = 4;
/// A wet rim darkens the outgoing frame just ahead of the ink.
const INK_RIM: f32 = 0.38;
const INK_RIM_REACH: f32 = 9.0;

pub(super) fn ink(frames: Frames, phase: TransitionPhase, paint: Paint) -> Vec<u8> {
    let Frames {
        outgoing,
        incoming,
        width,
        height,
    } = frames;
    let field = InkField::cached(width, height, phase.focus);
    // Start and end a soft edge's width beyond the field, so no point is
    // partly inked at the first instant or uninked at the last.
    let spread = field.steepest * EDGE_SOFTNESS * 1.5;
    let threshold = ink_threshold(phase.progress, spread);
    let step = 1e-3;
    let rate = (ink_threshold(phase.progress + step, spread)
        - ink_threshold(phase.progress - step, spread))
        / (2.0 * step)
        / phase.seconds.max(1e-3);
    // The rim wets in with the first ink rather than appearing at once.
    let wet = INK_RIM * smoothstep(phase.progress / 0.12);
    paint_rows(width, height, |y, row| {
        for (x, pixel) in row.iter_mut().enumerate() {
            let (value, slope) = field.sample(x as f32 + 0.5, y as f32 + 0.5);
            let slope = slope.max(1e-6);
            // Distance from the ink's edge in pixels; positive inside.
            let inside = (threshold - value) / slope;
            let softness = EDGE_SOFTNESS + rate / slope * SAMPLE_GAP;
            let cover = smoothstep(inside / softness * 0.5 + 0.5);
            let below = linear(outgoing, y * width + x);
            let rim = wet * (-(inside.min(0.0) / INK_RIM_REACH).powi(2)).exp();
            let mut rgb = mix(below, paint.background, rim * (1.0 - cover));
            if cover > 0.0 {
                rgb = mix(rgb, linear(incoming, y * width + x), cover);
            }
            put(pixel, rgb);
        }
    })
}

/// A noise field over the frame, evenly distributed in 0..1 so a threshold
/// covers that fraction of the frame, with its slope per pixel.
struct InkField {
    columns: usize,
    rows: usize,
    values: Vec<f32>,
    slopes: Vec<f32>,
    steepest: f32,
}

type InkKey = (usize, usize, Option<[u32; 4]>);

impl InkField {
    fn cached(width: usize, height: usize, focus: Option<[f32; 4]>) -> Arc<Self> {
        static CACHE: Mutex<Option<(InkKey, Arc<InkField>)>> = Mutex::new(None);
        let key = (width, height, focus.map(|rect| rect.map(f32::to_bits)));
        let mut cache = CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((cached, field)) = cache.as_ref()
            && *cached == key
        {
            return field.clone();
        }
        let field = Arc::new(Self::new(width, height, focus));
        *cache = Some((key, field.clone()));
        field
    }

    fn new(width: usize, height: usize, focus: Option<[f32; 4]>) -> Self {
        let columns = width.div_ceil(INK_CELL) + 1;
        let rows = height.div_ceil(INK_CELL) + 1;
        let origin = focus.map(|[x, y, w, h]| vec2(x + w * 0.5, y + h * 0.5));
        let reach = origin.map_or(1.0, |origin| {
            [
                vec2(0.0, 0.0),
                vec2(width as f32, 0.0),
                vec2(0.0, height as f32),
            ]
            .into_iter()
            .chain([vec2(width as f32, height as f32)])
            .map(|corner| corner.distance(origin))
            .fold(1.0, f32::max)
        });
        let mut values = Vec::with_capacity(columns * rows);
        for row in 0..rows {
            for column in 0..columns {
                let point = vec2((column * INK_CELL) as f32, (row * INK_CELL) as f32);
                let noise = ink_noise(point / 420.0);
                values.push(match origin {
                    // From a focus the ink spreads outward, its front torn by noise.
                    Some(origin) => lerp(point.distance(origin) / reach, noise, 0.42),
                    None => noise,
                });
            }
        }
        // Even out the distribution: each value becomes its rank.
        let mut order = (0..values.len()).collect::<Vec<_>>();
        order.sort_by(|a, b| values[*a].total_cmp(&values[*b]).then(a.cmp(b)));
        let count = (values.len() - 1).max(1) as f32;
        let mut ranked = vec![0.0; values.len()];
        for (rank, index) in order.into_iter().enumerate() {
            ranked[index] = rank as f32 / count;
        }
        let at = |column: usize, row: usize| {
            ranked[row.min(rows - 1) * columns + column.min(columns - 1)]
        };
        let slopes = (0..rows)
            .flat_map(|row| (0..columns).map(move |column| (column, row)))
            .map(|(column, row)| {
                let dx = at(column + 1, row) - at(column.saturating_sub(1), row);
                let dy = at(column, row + 1) - at(column, row.saturating_sub(1));
                vec2(dx, dy).length() / (2.0 * INK_CELL as f32)
            })
            .collect::<Vec<_>>();
        Self {
            columns,
            rows,
            values: ranked,
            steepest: slopes.iter().copied().fold(0.0, f32::max),
            slopes,
        }
    }

    /// The field's value and slope per pixel at a point.
    fn sample(&self, x: f32, y: f32) -> (f32, f32) {
        let (gx, gy) = (x / INK_CELL as f32, y / INK_CELL as f32);
        let (column, row) = (gx.floor() as usize, gy.floor() as usize);
        let (column, row) = (column.min(self.columns - 2), row.min(self.rows - 2));
        let (fx, fy) = (gx - column as f32, gy - row as f32);
        let bilinear = |grid: &[f32]| {
            let at = |c: usize, r: usize| grid[r * self.columns + c];
            let top = lerp(at(column, row), at(column + 1, row), fx);
            let bottom = lerp(at(column, row + 1), at(column + 1, row + 1), fx);
            lerp(top, bottom, fy)
        };
        (bilinear(&self.values), bilinear(&self.slopes))
    }
}

/// Domain-warped fractal noise: soft blots with torn, fibrous edges.
fn ink_noise(point: Vec2) -> f32 {
    let warp = vec2(
        fractal_noise2(point + vec2(1.7, 9.2), 3, 11),
        fractal_noise2(point + vec2(8.3, 2.8), 3, 12),
    );
    fractal_noise2(point + warp * 1.6, 6, 13)
}
