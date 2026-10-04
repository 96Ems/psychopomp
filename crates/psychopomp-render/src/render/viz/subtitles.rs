//! Subtitle pixels: a backing surface that morphs between pages, a pill
//! gliding under the spoken word, and words inked as upcoming (dim), past
//! (plain), or current (the highlight tone). Pages and inks come from the
//! compiled `SubtitleLayout`; this module only paints them.
use psychopomp::subtitles::{SubtitleLayout, SubtitlesPlan};

use super::{
    super::{
        chart::{Anchor, solid},
        ui::{
            Bounds,
            card::{Fill, SurfaceStyle, UiCanvas, UiColor},
        },
    },
    text_spec,
};
use crate::render::HeadlessRenderer;

/// Upcoming words show at this share of full ink.
const UPCOMING: f32 = 0.5;

impl HeadlessRenderer {
    pub(crate) fn compile_subtitles(&mut self, plan: &SubtitlesPlan) -> SubtitleLayout {
        plan.layout(|text| {
            self.plain_text_sprite(text, text_spec(plan.size, [255; 3]))
                .advance
        })
    }

    pub(crate) fn composite_subtitles(
        &mut self,
        pixels: &mut [u8],
        plan: &SubtitlesPlan,
        layout: &SubtitleLayout,
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
        let canvas = self.viz_canvas();
        let palette = self.theme.palette();
        let line_height = plan.line_height();
        let highlight = self.theme.tone(plan.highlight);
        if plan.backing
            && let Some(backing) = layout.backing(seconds)
        {
            let pad = [plan.size * 0.7, plan.size * 0.32];
            let rise = backing.rise * line_height;
            let top = origin[1] - (backing.lines - 0.5) * line_height - pad[1] + rise;
            let bottom = origin[1] + line_height * 0.5 + pad[1] + rise;
            let [r, g, b] = palette.background;
            let [br, bg, bb] = palette.raised;
            UiCanvas::new(pixels, canvas).surface(
                Bounds {
                    origin: [origin[0] - backing.width * 0.5 - pad[0], top],
                    size: [backing.width + pad[0] * 2.0, bottom - top],
                },
                SurfaceStyle::new(Fill::Solid(UiColor::srgb8(r, g, b, 255)), 16.0).border(
                    1.0,
                    UiColor::srgb8(br, bg, bb, 255),
                    0.8,
                ),
                opacity * backing.opacity * 0.82,
            );
        }
        for frame in layout.sample(seconds) {
            let alpha = opacity * frame.opacity;
            if alpha <= 0.001 {
                continue;
            }
            let lines = frame.page.lines.len();
            let line_y = |line: usize| {
                origin[1] - (lines - 1 - line) as f32 * line_height + frame.rise * line_height
            };
            let line_left = |line: usize| origin[0] - frame.page.lines[line].width * 0.5;
            for pill in &frame.pills {
                let pad = plan.size * 0.2;
                UiCanvas::new(pixels, canvas).fill(
                    Bounds {
                        origin: [
                            line_left(pill.line) + pill.x - pad,
                            line_y(pill.line) - plan.size * 0.66,
                        ],
                        size: [pill.width + pad * 2.0, plan.size * 1.32],
                    },
                    plan.size * 0.28,
                    solid(highlight),
                    alpha * pill.opacity * 0.2,
                );
            }
            for ink in &frame.words {
                let text = &plan.words[ink.word.index].text;
                let at = [line_left(ink.line) + ink.word.x, line_y(ink.line)];
                for (color, weight) in [
                    (palette.text, ink.upcoming * UPCOMING + ink.past),
                    (highlight, ink.current),
                ] {
                    self.viz_text(
                        pixels,
                        text,
                        plan.size,
                        color,
                        Anchor::Left,
                        at,
                        alpha * weight,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::subtitles::SubtitlesPlan;

    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; words highlight as spoken and frames sample in any order"]
    fn subtitles_highlight_the_spoken_word() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "subtitles-proof".into(),
        }))
        .unwrap();
        let plan = SubtitlesPlan::new([960.0, 900.0], 1200.0)
            .word("One", 0, 280_000_000)
            .word("server.", 300_000_000, 580_000_000);
        let layout = renderer.compile_subtitles(&plan);
        let background = renderer.render_title_card("", None, 0.);
        let mut draw = |seconds: f64| {
            let mut pixels = background.clone();
            renderer.composite_subtitles(&mut pixels, &plan, &layout, seconds, |_, d| d);
            pixels
        };
        let first = draw(0.2);
        let second = draw(0.45);
        assert!(first != second && first != background);
        assert!(draw(5.0) == background, "gone after the hold");
        assert!(draw(0.2) == first, "sampling order cannot change a frame");
    }
}
