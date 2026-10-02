//! Rolling Number pixels: each digit is a wheel drawn through a stationary
//! window with linear edge fades, smearing vertically while it turns fast;
//! symbols and static spans fade in place. Columns come from the compiled
//! roll; this module only paints them.
use psychopomp::{
    caption::CaptionPlan,
    rolling::{CompiledRoll, RollGlyph, RollingNumberPlan},
};

use super::{
    HeadlessRenderer, PlainTextSpec, TextDraw, TextFilter, VerticalMask, composite_text,
    ui::{
        Bounds,
        card::{Fill, SurfaceStyle, UiCanvas, UiColor},
    },
};

/// Smear deviation per row of wheel travel (`.035em` of the slot height).
const SMEAR_PER_ROW: f32 = 0.035;

impl HeadlessRenderer {
    pub(crate) fn compile_rolling_number(&mut self, plan: &RollingNumberPlan) -> CompiledRoll {
        plan.compile(|text| {
            self.plain_text_sprite(text, rolling_spec(plan, [0, 0, 0]))
                .advance
        })
    }

    pub(crate) fn composite_rolling_number(
        &mut self,
        pixels: &mut [u8],
        plan: &RollingNumberPlan,
        roll: &CompiledRoll,
        seconds: f64,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let origin = [
            plan.origin[0] + sample("x", 0.0),
            plan.origin[1] + sample("y", 0.0),
        ];
        let glyphs = roll.sample(seconds);
        if plan.chip {
            self.rolling_chip(pixels, plan, &glyphs, origin, opacity);
        }
        for glyph in &glyphs {
            let alpha = opacity * glyph.opacity;
            if alpha <= 0.001 {
                continue;
            }
            match glyph.wheel {
                Some(wheel) => self.rolling_wheel(pixels, plan, glyph, wheel, origin, alpha),
                None => self.rolling_symbol(pixels, plan, glyph, origin, alpha),
            }
        }
    }

    /// The faces of one digit wheel in its window, offset by the wheel's
    /// fractional position and any remaining rise.
    fn rolling_wheel(
        &mut self,
        pixels: &mut [u8],
        plan: &RollingNumberPlan,
        glyph: &RollGlyph<'_>,
        wheel: f32,
        [x, y]: [f32; 2],
        alpha: f32,
    ) {
        let canvas = [self.spec.width, self.spec.height];
        let row = plan.row_height();
        let sigma = row * SMEAR_PER_ROW * plan.blur;
        let mask = VerticalMask {
            top: y - row * 0.5,
            bottom: y + row * 0.5,
            fade: plan.edge_fade(),
        };
        let spec = rolling_spec(plan, self.theme.tone(glyph.token.tone));
        // Only the two faces straddling the wheel can reach its one-row
        // window; a rising digit carries just its own face, as in the library.
        for face in (wheel.floor() as i64)..=(wheel.ceil() as i64) {
            let offset = (face as f32 - wheel) * row + glyph.rise;
            let digit = char::from(b'0' + face.rem_euclid(10) as u8).to_string();
            let sprite = self.plain_text_sprite(&digit, spec);
            let left = x + glyph.x + (glyph.width - sprite.advance) * 0.5;
            composite_text(
                pixels,
                canvas,
                TextDraw {
                    filter: TextFilter::Smear {
                        sigma,
                        amount: glyph.smear,
                    },
                    opacity: alpha,
                    mask: Some(mask),
                    ..TextDraw::new(sprite, [left, y + offset - sprite.height as f32 * 0.5])
                },
            );
        }
    }

    /// Separators, literals, and static spans stay sharp and fade in place.
    fn rolling_symbol(
        &mut self,
        pixels: &mut [u8],
        plan: &RollingNumberPlan,
        glyph: &RollGlyph<'_>,
        [x, y]: [f32; 2],
        alpha: f32,
    ) {
        let canvas = [self.spec.width, self.spec.height];
        let spec = rolling_spec(plan, self.theme.tone(glyph.token.tone));
        let sprite = self.plain_text_sprite(&glyph.token.text, spec);
        composite_text(
            pixels,
            canvas,
            TextDraw {
                opacity: alpha,
                ..TextDraw::new(sprite, [x + glyph.x, y - sprite.height as f32 * 0.5])
            },
        );
    }

    /// A caption-style chip around the sampled glyphs. Entering and exiting
    /// columns count in proportion to their opacity, so its edges glide.
    fn rolling_chip(
        &mut self,
        pixels: &mut [u8],
        plan: &RollingNumberPlan,
        glyphs: &[RollGlyph<'_>],
        [x, y]: [f32; 2],
        opacity: f32,
    ) {
        let left = glyphs
            .iter()
            .map(|g| g.x + g.width * (1.0 - g.opacity))
            .fold(f32::INFINITY, f32::min);
        let right = glyphs
            .iter()
            .map(|g| g.x + g.width * g.opacity)
            .fold(f32::NEG_INFINITY, f32::max);
        if !(left.is_finite() && right.is_finite()) {
            return;
        }
        // Match a one-line caption chip of the same size.
        let caption = CaptionPlan::line([0.0, 0.0], plan.size, Vec::new());
        let half = caption.line_height() * 0.5 + 4.0;
        let bounds = Bounds {
            origin: [x + left - plan.size * 0.7, y - half],
            size: [right - left + plan.size * 1.4, half * 2.0],
        };
        let palette = self.theme.palette();
        let [r, g, b] = palette.surface;
        let [br, bg, bb] = palette.raised;
        UiCanvas::new(pixels, [self.spec.width, self.spec.height]).surface(
            bounds,
            SurfaceStyle::new(Fill::Solid(UiColor::srgb8(r, g, b, 255)), half).border(
                1.2,
                UiColor::srgb8(br, bg, bb, 255),
                1.0,
            ),
            opacity,
        );
    }
}

fn rolling_spec(plan: &RollingNumberPlan, color: [u8; 3]) -> PlainTextSpec {
    PlainTextSpec {
        font_size: plan.size,
        color,
        size: [1600, (plan.size * 1.5).ceil() as u32],
        semibold: plan.bold,
        crop_to_advance: true,
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::{
        caption::{CaptionAlign, CaptionSpanPlan},
        rolling::RollingNumberPlan,
        tone::Tone,
    };

    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; wheels roll through a still window and settle to sharp text"]
    fn rolling_numbers_roll_inside_their_window_and_settle_sharp() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "rolling-proof".into(),
        }))
        .unwrap();
        let plan = RollingNumberPlan::new([960.0, 540.0], 96.0, "999")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![CaptionSpanPlan::new("n=", Tone::Muted)])
            .roll(1_000_000_000, "1,000");
        let roll = renderer.compile_rolling_number(&plan);
        let background = renderer.render_title_card("", None, 0.);
        let mut draw = |seconds: f64, opacity: f32| {
            let mut pixels = background.clone();
            renderer.composite_rolling_number(&mut pixels, &plan, &roll, seconds, |p, d| {
                if p == "opacity" { opacity } else { d }
            });
            pixels
        };
        assert!(draw(0.5, 0.0) == background, "hidden numbers leave no ink");
        let before = draw(0.5, 1.0);
        let mid = draw(1.08, 1.0);
        let after = draw(2.0, 1.0);
        assert!(before != after && mid != after && mid != before);
        assert!(draw(2.0, 1.0) == after, "sampling is deterministic");
        // Nothing paints outside the digits' window: rows a full line above
        // and below stay background while wheels are mid-roll.
        let row = plan.row_height();
        let ink_rows = |pixels: &[u8]| {
            (0..1080)
                .filter(|y| {
                    let start = y * 1920 * 4;
                    pixels[start..start + 1920 * 4] != background[start..start + 1920 * 4]
                })
                .collect::<Vec<_>>()
        };
        let rows = ink_rows(&mid);
        let (top, bottom) = (rows[0] as f32, *rows.last().unwrap() as f32);
        assert!(top >= 540.0 - row * 0.5 - 1.0 && bottom <= 540.0 + row * 0.5 + 1.0);
        // Centered text widens symmetrically about the origin.
        let extent = |pixels: &[u8]| {
            let ink = (0..1920)
                .filter(|x| {
                    (440..640).any(|y| {
                        let i = (y * 1920 + x) * 4;
                        pixels[i..i + 3] != background[i..i + 3]
                    })
                })
                .collect::<Vec<_>>();
            (ink[0] as f32, *ink.last().unwrap() as f32)
        };
        let (left, right) = extent(&after);
        assert!(((left + right) * 0.5 - 960.0).abs() < 40.0);
        assert!(extent(&before).0 > left, "the number grew to the left too");
    }
}
