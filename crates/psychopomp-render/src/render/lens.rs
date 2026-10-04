//! Lens pixels: thick glass over the frame composed beneath it. The geometry
//! and optics are the lightweight `lens::Glass`; this module reads the page
//! in linear light, refracts it through the glass (sharp cubic sampling in the
//! flat middle, color split and softening at the rim), then lights the glass:
//! a fresnel sheen, a crisp specular line where the rim faces the light, a
//! fainter one opposite, and a soft drop shadow with darker contact. Frosted,
//! dimmed glass (a pane) mixes in a heavily softened page.
//! Pixels outside `Glass::bounds` are never touched.
use psychopomp::{
    lens::{Glass, MAX_BEND},
    math::{
        Vec2, lerp,
        shapes::{Box2, RoundedBox},
        smoothstep, vec2,
    },
};

use crate::exposure::linear_tables;

/// Toward the light: upper left, mostly from above, like the Stage's key.
const LIGHT: Vec2 = Vec2::new(-0.5, -0.866);
/// Extra page pixels around the source for cubic taps and the blur.
const SOURCE_MARGIN: f32 = 10.0;
/// How much brighter the glass body is than what it shows (linear).
const LIFT: f32 = 0.0015;
const FRESNEL: f32 = 0.035;
const SPECULAR: f32 = 0.9;
const COUNTER_SPECULAR: f32 = 0.35;
/// The soft light around the specular line, inside the rim.
const SPECULAR_GLOW: f32 = 0.08;
const INNER_GLOW: f32 = 0.06;
const EDGE: f32 = 0.10;
/// Rim softening where it compresses the page most.
const RIM_SOFTEN: f32 = 0.55;
/// The cubic's sharpness at 2× and beyond: Keys' `a`, from Catmull-Rom's
/// -0.5 at rest toward a crisper kernel as the glass enlarges.
const SHARPEST: f32 = -0.75;

/// Refract the page beneath `glass` and light it, in place.
pub(crate) fn composite_lens(pixels: &mut [u8], [width, height]: [u32; 2], glass: &Glass) {
    let Some(out) = PixelRect::covering(glass.bounds(), width, height) else {
        return;
    };
    let source = glass.source_bounds();
    let margin = Vec2::splat(SOURCE_MARGIN);
    let Some(rect) = PixelRect::covering(
        Box2 {
            min: source.min - margin,
            max: source.max + margin,
        },
        width,
        height,
    )
    .or_else(|| PixelRect::nearest(source.center(), width, height)) else {
        return;
    };
    let page = Tile::linear(pixels, width, rect);
    let half = glass.outline.half;
    let lens = Composite {
        glass,
        bends: BendTable::new(glass),
        soft: page.blurred(2, 2),
        frosted: (glass.frost > 0.0).then(|| page.blurred(8, 3)),
        page,
        sharpness: lerp(-0.5, SHARPEST, (glass.magnification - 1.0).clamp(0.0, 1.0)),
        strength: smoothstep(glass.presence),
        shadow_box: RoundedBox {
            center: glass.outline.center + vec2(0.0, glass.drop()),
            ..glass.outline
        },
        sigma: (8.0 + 0.05 * half.min_element()).min(14.0),
    };
    // Rows are independent: each pixel reads only the page copy and itself.
    let row_bytes = width as usize * 4;
    let rows = &mut pixels[out.y0 * row_bytes..out.y1 * row_bytes];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(16));
    let band = (out.y1 - out.y0).div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (index, chunk) in rows.chunks_mut(band * row_bytes).enumerate() {
            let lens = &lens;
            scope.spawn(move || {
                for (offset, row) in chunk.chunks_exact_mut(row_bytes).enumerate() {
                    let y = out.y0 + index * band + offset;
                    for x in out.x0..out.x1 {
                        lens.pixel(x, y, &mut row[x * 4..x * 4 + 3]);
                    }
                }
            });
        }
    });
}

/// Everything one composite shares across its pixels.
struct Composite<'a> {
    glass: &'a Glass,
    bends: BendTable,
    page: Tile,
    /// The page softened by about two pixels, for the rim.
    soft: Tile,
    /// The page softened by about eight pixels, for frosted glass.
    frosted: Option<Tile>,
    /// Keys' cubic parameter, crisper as the glass enlarges.
    sharpness: f32,
    /// How lit the glass is: its presence, eased.
    strength: f32,
    shadow_box: RoundedBox,
    sigma: f32,
}

impl Composite<'_> {
    /// One frame pixel's sRGB color, in place.
    fn pixel(&self, x: usize, y: usize, rgb: &mut [u8]) {
        let glass = self.glass;
        let point = vec2(x as f32 + 0.5, y as f32 + 0.5);
        let distance = glass.outline.distance(point);
        let coverage = (0.5 - distance).clamp(0.0, 1.0);
        // The drop shadow falls below; contact darkens right at the edge.
        let offset = (self.shadow_box.distance(point) + 2.0).max(0.0);
        let fall = (-(offset * offset) / (2.0 * self.sigma * self.sigma)).exp();
        let contact = (-(distance.max(0.0).powi(2)) / 8.0).exp();
        let dark = (glass.shadow * self.strength * (0.55 * fall + 0.45 * contact)).min(1.0)
            * (1.0 - coverage);
        if coverage <= 0.0 && dark < 1e-4 {
            return;
        }
        let tables = linear_tables();
        let base = [0, 1, 2].map(|c| tables.to_linear[rgb[c] as usize] * (1.0 - 0.9 * dark));
        let color = if coverage > 0.0 {
            let lit = self.shade(point, distance);
            [0, 1, 2].map(|c| base[c] + (lit[c] - base[c]) * coverage)
        } else {
            base
        };
        for (value, linear) in rgb.iter_mut().zip(color) {
            *value = tables.encode(linear);
        }
    }

    /// The glass's color at `point`, inside its outline: the refracted page
    /// plus its own light.
    fn shade(&self, point: Vec2, distance: f32) -> [f32; 3] {
        let glass = self.glass;
        let depth = -distance;
        let t = (depth / glass.bevel).clamp(0.0, 1.0);
        let normal = glass.outline.normal(point);
        let local = point - glass.outline.center;
        let bend = self.bends.at(t);
        let source = |spread: f32| glass.source_bent(point, bend, spread);
        let sample = |spread: f32| self.page.cubic(source(spread), self.sharpness);
        let mut color = if bend * glass.dispersion > 0.02 {
            let spread = glass.dispersion;
            [
                sample(1.0 - spread)[0],
                sample(1.0)[1],
                sample(1.0 + spread)[2],
            ]
        } else {
            sample(1.0)
        };
        // The rim squeezes the page into a thin band; soften it rather than alias.
        let rim = if glass.depth > 0.0 {
            bend / (MAX_BEND * glass.depth)
        } else {
            0.0
        };
        let soften = (rim.powf(1.5) * RIM_SOFTEN).min(1.0);
        if soften > 1e-3 {
            let blurred = self.soft.bilinear(source(1.0));
            color = [0, 1, 2].map(|c| color[c] + (blurred[c] - color[c]) * soften);
        }
        if let Some(frosted) = &self.frosted {
            let blurred = frosted.bilinear(source(1.0));
            color = [0, 1, 2].map(|c| color[c] + (blurred[c] - color[c]) * glass.frost);
        }
        let dim = 1.0 - glass.dim * self.strength;
        // Light: a sheen that reflects a sky brighter above, a crisp line where
        // the rim faces the light, a fainter one opposite where light leaving
        // the glass catches the far wall, and a hairline all around the edge.
        let facing = normal.dot(LIGHT).max(0.0);
        let away = (-normal.dot(LIGHT)).max(0.0);
        let sky = (0.5 - 0.5 * local.y / glass.outline.half.y).clamp(0.0, 1.0);
        let fresnel = (1.0 - t).powi(3) * FRESNEL * (0.25 + 0.75 * sky);
        let line = (-((depth - 1.2) / 1.0).powi(2)).exp();
        let glow = (-((depth - 2.0) / 3.5).powi(2)).exp();
        let specular = line * (SPECULAR * facing.powf(2.5) + COUNTER_SPECULAR * away.powi(3))
            + glow * SPECULAR_GLOW * facing.powi(4);
        let inner = (-((t - 0.32) / 0.18).powi(2)).exp() * away * away * INNER_GLOW;
        let edge = (-((depth - 0.5) / 0.7).powi(2)).exp() * EDGE;
        let light =
            (LIFT + glass.frost * 0.012 + fresnel + specular + inner + edge) * self.strength;
        color.map(|value| value * dim + light)
    }
}

/// `Glass::bend` across the rim, sampled once per composite.
struct BendTable([f32; Self::STEPS + 1]);

impl BendTable {
    const STEPS: usize = 256;

    fn new(glass: &Glass) -> Self {
        Self(std::array::from_fn(|i| {
            glass.bend(i as f32 / Self::STEPS as f32)
        }))
    }

    fn at(&self, t: f32) -> f32 {
        let x = t.clamp(0.0, 1.0) * Self::STEPS as f32;
        let i = (x as usize).min(Self::STEPS - 1);
        let f = x - i as f32;
        self.0[i] + (self.0[i + 1] - self.0[i]) * f
    }
}

/// Whole frame pixels, `x0..x1` by `y0..y1`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PixelRect {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl PixelRect {
    /// The frame pixels `bounds` touches; `None` when it misses the frame.
    fn covering(bounds: Box2, width: u32, height: u32) -> Option<Self> {
        let frame = vec2(width as f32, height as f32);
        let min = bounds.min.floor().clamp(Vec2::ZERO, frame);
        let max = bounds.max.ceil().clamp(Vec2::ZERO, frame);
        (min.x < max.x && min.y < max.y).then_some(Self {
            x0: min.x as usize,
            y0: min.y as usize,
            x1: max.x as usize,
            y1: max.y as usize,
        })
    }

    /// The one frame pixel nearest `point`, for a focus beyond the frame.
    fn nearest(point: Vec2, width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 || !point.is_finite() {
            return None;
        }
        let x = (point.x.max(0.0) as usize).min(width as usize - 1);
        let y = (point.y.max(0.0) as usize).min(height as usize - 1);
        Some(Self {
            x0: x,
            y0: y,
            x1: x + 1,
            y1: y + 1,
        })
    }
}

/// A copy of part of the frame in linear light, sampled with its edges
/// clamped (the frame's own edges, or far enough outside the source).
struct Tile {
    origin: Vec2,
    width: usize,
    height: usize,
    texels: Vec<[f32; 3]>,
}

impl Tile {
    fn linear(pixels: &[u8], frame_width: u32, rect: PixelRect) -> Self {
        let tables = linear_tables();
        let mut texels = Vec::with_capacity((rect.x1 - rect.x0) * (rect.y1 - rect.y0));
        for y in rect.y0..rect.y1 {
            for x in rect.x0..rect.x1 {
                let index = (y * frame_width as usize + x) * 4;
                texels.push([0, 1, 2].map(|c| tables.to_linear[pixels[index + c] as usize]));
            }
        }
        Self {
            origin: vec2(rect.x0 as f32, rect.y0 as f32),
            width: rect.x1 - rect.x0,
            height: rect.y1 - rect.y0,
            texels,
        }
    }

    fn texel(&self, x: isize, y: isize) -> [f32; 3] {
        let x = x.clamp(0, self.width as isize - 1) as usize;
        let y = y.clamp(0, self.height as isize - 1) as usize;
        self.texels[y * self.width + x]
    }

    /// A softened copy, close to a Gaussian: `passes` box passes of
    /// `2 * radius + 1` texels along each axis, with running sums. Two passes
    /// of radius 2 make a Gaussian of two pixels.
    fn blurred(&self, radius: isize, passes: usize) -> Self {
        let mut texels = self.texels.clone();
        for horizontal in [true, false] {
            for _ in 0..passes {
                texels = self.box_pass(&texels, horizontal, radius);
            }
        }
        Self {
            origin: self.origin,
            width: self.width,
            height: self.height,
            texels,
        }
    }

    fn box_pass(&self, texels: &[[f32; 3]], horizontal: bool, radius: isize) -> Vec<[f32; 3]> {
        let (lines, length) = if horizontal {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        let at = |line: usize, i: isize| {
            let i = i.clamp(0, length as isize - 1) as usize;
            if horizontal {
                texels[line * self.width + i]
            } else {
                texels[i * self.width + line]
            }
        };
        let mut out = vec![[0.0; 3]; texels.len()];
        let scale = 1.0 / (2 * radius + 1) as f32;
        for line in 0..lines {
            let mut sum = [0.0; 3];
            for i in -radius..=radius {
                let texel = at(line, i);
                for c in 0..3 {
                    sum[c] += texel[c];
                }
            }
            for i in 0..length {
                let index = if horizontal {
                    line * self.width + i
                } else {
                    i * self.width + line
                };
                out[index] = sum.map(|value| value * scale);
                let (enter, leave) = (
                    at(line, i as isize + radius + 1),
                    at(line, i as isize - radius),
                );
                for c in 0..3 {
                    sum[c] += enter[c] - leave[c];
                }
            }
        }
        out
    }

    /// Cubic sampling at a canvas point with Keys' parameter `a` (-0.5 is
    /// Catmull-Rom; lower is crisper), sharp enough to keep enlarged text
    /// legible, and held within its four nearest texels so bright strokes on a
    /// dark page neither ring nor halo.
    fn cubic(&self, point: Vec2, a: f32) -> [f32; 3] {
        let (value, low, high) = self.cubic_bounds(point, a);
        [0, 1, 2].map(|c| value[c].clamp(low[c], high[c]))
    }

    /// The cubic value at `point` and the range of its four nearest texels.
    fn cubic_bounds(&self, point: Vec2, a: f32) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let p = point - self.origin - 0.5;
        let base = p.floor();
        let f = p - base;
        let (x0, y0) = (base.x as isize, base.y as isize);
        let wx = keys(f.x, a);
        let wy = keys(f.y, a);
        let mut sum = [0.0; 3];
        let mut low = [f32::INFINITY; 3];
        let mut high = [f32::NEG_INFINITY; 3];
        for (j, wy) in wy.iter().enumerate() {
            for (i, wx) in wx.iter().enumerate() {
                let texel = self.texel(x0 + i as isize - 1, y0 + j as isize - 1);
                let near = (1..=2).contains(&i) && (1..=2).contains(&j);
                for c in 0..3 {
                    sum[c] += texel[c] * wx * wy;
                    if near {
                        low[c] = low[c].min(texel[c]);
                        high[c] = high[c].max(texel[c]);
                    }
                }
            }
        }
        (sum, low, high)
    }

    fn bilinear(&self, point: Vec2) -> [f32; 3] {
        let p = point - self.origin - 0.5;
        let base = p.floor();
        let f = p - base;
        let (x, y) = (base.x as isize, base.y as isize);
        let [a, b, c, d] = [
            self.texel(x, y),
            self.texel(x + 1, y),
            self.texel(x, y + 1),
            self.texel(x + 1, y + 1),
        ];
        [0, 1, 2].map(|k| {
            let top = a[k] + (b[k] - a[k]) * f.x;
            let bottom = c[k] + (d[k] - c[k]) * f.x;
            top + (bottom - top) * f.y
        })
    }
}

/// Keys' cubic convolution weights for the taps at -1, 0, 1, and 2 around
/// `t`: Catmull-Rom at `a` = -0.5. Every `a` reproduces texels exactly at
/// `t` = 0.
fn keys(t: f32, a: f32) -> [f32; 4] {
    let near = |x: f32| ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0;
    let far = |x: f32| ((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a;
    [far(1.0 + t), near(t), near(1.0 - t), far(2.0 - t)]
}

#[cfg(test)]
mod tests {
    use psychopomp::{callout::CalloutAnchorPlan, lens::LensPlan, math::vec2};

    use super::{PixelRect, composite_lens};

    const SIZE: [u32; 2] = [320, 240];

    /// A page of thin vertical stripes on a dark ground.
    fn page() -> Vec<u8> {
        let mut pixels = vec![0u8; (SIZE[0] * SIZE[1] * 4) as usize];
        for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = index % SIZE[0] as usize;
            let value = if x % 8 < 2 { 230 } else { 18 };
            pixel.copy_from_slice(&[value, value, value, 255]);
        }
        pixels
    }

    fn glass(presence: f32) -> Option<psychopomp::lens::Glass> {
        let plan = LensPlan::circle(
            CalloutAnchorPlan::Point {
                id: "here".into(),
                at: [160.0, 120.0],
                side: None,
            },
            120.0,
        )
        .magnification(2.0);
        plan.glass(vec2(160.0, 120.0), |name, default| match name {
            "presence" => presence,
            _ => default,
        })
    }

    fn at(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
        let i = (y * SIZE[0] as usize + x) * 4;
        pixels[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn a_lens_changes_only_pixels_inside_its_bounds() {
        let before = page();
        let mut after = before.clone();
        let glass = glass(1.0).unwrap();
        composite_lens(&mut after, SIZE, &glass);
        assert!(after != before);
        let bounds = PixelRect::covering(glass.bounds(), SIZE[0], SIZE[1]).unwrap();
        for y in 0..SIZE[1] as usize {
            for x in 0..SIZE[0] as usize {
                if !(bounds.x0..bounds.x1).contains(&x) || !(bounds.y0..bounds.y1).contains(&y) {
                    assert_eq!(at(&after, x, y), at(&before, x, y), "{x},{y}");
                }
            }
        }
        // Alpha is never touched.
        assert!(after.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn the_middle_shows_its_focus_enlarged() {
        let before = page();
        let mut after = before.clone();
        composite_lens(&mut after, SIZE, &glass(1.0).unwrap());
        // At 2× a two-pixel stripe every eight pixels becomes four every
        // sixteen: count bright runs across the flat middle.
        let row = 120;
        let bright = |pixels: &[u8], x: usize| at(pixels, x, row)[0] > 120;
        let runs = |pixels: &[u8]| {
            (130..190)
                .filter(|&x| bright(pixels, x) && !bright(pixels, x - 1))
                .count()
        };
        let (plain, enlarged) = (runs(&before), runs(&after));
        assert!(enlarged.abs_diff(plain / 2) <= 1, "{plain} → {enlarged}");
        // The center shows the focus: the bright column at x = 160.
        assert!(bright(&after, 160));
    }

    #[test]
    fn an_absent_lens_changes_nothing_and_a_faint_one_little() {
        assert!(glass(0.0).is_none());
        // A smooth page, so a slight enlargement changes values only slightly.
        let mut before = page();
        for (index, pixel) in before.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = (index % SIZE[0] as usize) as u8;
            pixel[..3].fill(x / 2 + 20);
        }
        let mut faint = before.clone();
        composite_lens(&mut faint, SIZE, &glass(0.01).unwrap());
        let changed = faint
            .iter()
            .zip(&before)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        let mut full = before.clone();
        composite_lens(&mut full, SIZE, &glass(1.0).unwrap());
        let strong = full
            .iter()
            .zip(&before)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(changed <= 2 && strong > 40, "{changed} vs {strong}");
    }

    #[test]
    fn cubic_sampling_reproduces_texels_and_does_not_ring() {
        let pixels = page();
        let tile = super::Tile::linear(
            &pixels,
            SIZE[0],
            PixelRect {
                x0: 0,
                y0: 0,
                x1: 32,
                y1: 4,
            },
        );
        for x in 0..32 {
            let center = vec2(x as f32 + 0.5, 1.5);
            assert_eq!(tile.cubic(center, -0.5), tile.texel(x, 1));
        }
        let dark = tile.texel(4, 1)[0];
        for step in 0..40 {
            let value = tile.cubic(vec2(2.0 + step as f32 * 0.1, 1.5), -1.0)[0];
            assert!(value >= dark - 1e-6, "no undershoot below the dark ground");
        }
    }
}
