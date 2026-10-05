//! Lower Third pixels: an accent bar drawn up from its foot, and the name
//! (bold sans) and role (muted sans) sliding out from behind it through a
//! stationary edge, so neither ever shows on the bar's far side.
use cosmic_text::{Attrs, Color, Metrics, Weight};
use psychopomp::{
    lower_third::LowerThirdPlan,
    math::{remap_clamp, smoothstep},
};

use super::{
    HeadlessRenderer, TextDraw, TextSprite, composite_text, fonts, make_sprite,
    theme::ThemedCache,
    ui::{Bounds, card::UiCanvas},
    window::solid,
};

/// The name and role sprites, colored for the current theme.
#[derive(Default)]
pub(crate) struct LowerThirdGlyphs(ThemedCache<(TextSprite, Option<TextSprite>)>);

impl HeadlessRenderer {
    fn sans_sprite(&mut self, text: &str, size: f32, weight: Weight, color: [u8; 3]) -> TextSprite {
        let [r, g, b] = color;
        let attrs = Attrs::new()
            .family(fonts::SANS)
            .weight(weight)
            .color(Color::rgb(r, g, b));
        let height = (size * 1.4).ceil() as u32;
        let width = (size * text.chars().count() as f32 * 0.8 + size * 2.0).ceil() as u32;
        make_sprite(
            &mut self.font_system,
            &mut self.swash_cache,
            vec![(text, attrs.clone())],
            attrs,
            Metrics::new(size, height as f32),
            width,
            height,
        )
    }

    pub(crate) fn composite_lower_third(
        &mut self,
        pixels: &mut [u8],
        plan: &LowerThirdPlan,
        glyphs: &LowerThirdGlyphs,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let palette = self.theme.palette();
        let sprites = glyphs.0.get(self.theme, || {
            let name = self.sans_sprite(&plan.name, plan.size, Weight::BOLD, palette.text);
            let role = plan.role.as_deref().map(|role| {
                self.sans_sprite(role, plan.role_size(), Weight::NORMAL, palette.muted)
            });
            (name, role)
        });
        let offset = [sample("x", 0.0), sample("y", 0.0)];
        let canvas = [self.spec.width, self.spec.height];
        let bar = sample("bar", 1.0).clamp(0.0, 1.0);
        let [top, bottom] = plan.bar_span();
        if bar > 0.001 {
            let height = (bottom - top) * bar;
            UiCanvas::new(pixels, canvas).fill(
                Bounds {
                    origin: [plan.origin[0] + offset[0], bottom - height + offset[1]],
                    size: [plan.bar_width(), height],
                },
                plan.bar_width() * 0.5,
                solid(self.theme.tone(plan.tone)),
                opacity,
            );
        }
        // Text emerges from behind the bar: its clip edge sits flush with the
        // bar's right side, and each line slides by its full measured advance
        // plus the gap so nothing is ever clipped in open air.
        let edge = plan.origin[0] + offset[0] + plan.bar_width();
        let gap = plan.text_left() - plan.origin[0] - plan.bar_width();
        let lines = [
            (Some(&sprites.0), sample("name", 1.0), plan.origin[1]),
            (sprites.1.as_ref(), sample("role", 1.0), plan.role_center()),
        ];
        for (sprite, phase, center) in lines {
            let Some(sprite) = sprite else {
                continue;
            };
            let phase = phase.clamp(0.0, 1.0);
            let alpha = opacity * smoothstep(remap_clamp(phase, [0.0, 0.35], [0.0, 1.0]));
            if alpha <= 0.001 {
                continue;
            }
            let slide = sprite.advance + gap;
            let x = plan.text_left() + offset[0] - (1.0 - phase) * slide;
            let source_left = (edge - x).max(0.0);
            let width = sprite.advance + 4.0 - source_left;
            if width <= 0.0 {
                continue;
            }
            composite_text(
                pixels,
                canvas,
                TextDraw {
                    source_left,
                    clip_width: width.min(sprite.width as f32 - source_left),
                    opacity: alpha,
                    ..TextDraw::new(
                        sprite,
                        [
                            x + source_left,
                            center + offset[1] - sprite.height as f32 * 0.5,
                        ],
                    )
                },
            );
        }
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::lower_third::LowerThirdPlan;

    use super::LowerThirdGlyphs;
    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; text never shows left of the bar while it slides out"]
    fn names_emerge_from_behind_the_bar() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "lower-third-proof".into(),
        }))
        .unwrap();
        let plan = LowerThirdPlan::new([400.0, 800.0], "Dax Raad").role("opencode");
        let glyphs = LowerThirdGlyphs::default();
        let background = renderer.render_title_card("", None, 0.0);
        let mut draw =
            |phase: f32| {
                let mut pixels = background.clone();
                renderer.composite_lower_third(&mut pixels, &plan, &glyphs, |property, d| {
                    match property {
                        "name" | "role" => phase,
                        "bar" => 0.0,
                        _ => d,
                    }
                });
                pixels
            };
        let sliding = draw(0.5);
        assert!(sliding != background, "half out, the name shows");
        let left_of_bar = |pixels: &[u8]| {
            (700..900).all(|y| {
                (0..400).all(|x| {
                    let i = (y * 1920 + x) * 4;
                    pixels[i..i + 4] == background[i..i + 4]
                })
            })
        };
        assert!(left_of_bar(&sliding), "nothing shows beyond the bar");
        assert!(draw(0.0) == background, "hidden text leaves no ink");
    }
}
