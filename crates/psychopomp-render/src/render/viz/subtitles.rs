//! Subtitle pixels: a backing surface that morphs between pages, a pill
//! gliding under the spoken word, and words inked as upcoming (dim), past
//! (plain), or current (the highlight tone). Pages and inks come from the
//! compiled `SubtitleLayout`; this module only paints them.
use psychopomp::subtitles::{PageFrame, SubtitleLayout, SubtitlesPlan};

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

/// How wide each line of `frame` is up to and including the words said so
/// far, easing out to each word's right edge as it lands.
fn revealed(frame: &PageFrame<'_>) -> Vec<f32> {
    let mut shown = vec![0.0_f32; frame.page.lines.len()];
    for ink in &frame.words {
        let said = (ink.past + ink.current).clamp(0.0, 1.0);
        if said > 0.0 {
            let right = ink.word.x + ink.word.width;
            shown[ink.line] += (right - shown[ink.line]) * said;
        }
    }
    shown
}

impl HeadlessRenderer {
    pub(crate) fn compile_subtitles(&mut self, plan: &SubtitlesPlan) -> SubtitleLayout {
        plan.layout(|text| {
            self.plain_text_sprite_in(plan.face, text, text_spec(plan.size, [255; 3]))
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
        // A tilted line staggers its words up or down by their distance from
        // the center; each word stays upright.
        let slope = sample("tilt", 0.0).clamp(-0.3, 0.3).tan();
        let lean = |x: f32| slope * (x - origin[0]);
        let frames = layout.sample(seconds);
        if plan.backing
            && let Some(mut backing) = layout.backing(seconds)
        {
            if plan.upcoming <= 0.0 {
                // Word by word, the backing holds only what has been said.
                let said = frames
                    .iter()
                    .flat_map(|frame| revealed(frame))
                    .fold(0.0_f32, f32::max);
                backing.width = backing.width.min(said);
                if backing.width <= 1.0 {
                    backing.opacity = 0.0;
                }
            }
            let tilt = slope.abs() * backing.width * 0.5;
            let pad = [plan.size * 0.7, plan.size * 0.32 + tilt];
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
        for frame in frames {
            let alpha = opacity * frame.opacity;
            if alpha <= 0.001 {
                continue;
            }
            let lines = frame.page.lines.len();
            let line_y = |line: usize| {
                origin[1] - (lines - 1 - line) as f32 * line_height + frame.rise * line_height
            };
            // Revealed word by word, a line stays centered on what has been
            // said so far, gliding left as each word lands.
            let shown = revealed(&frame);
            let line_left = |line: usize| {
                let width = if plan.upcoming <= 0.0 {
                    shown[line]
                } else {
                    frame.page.lines[line].width
                };
                origin[0] - width * 0.5
            };
            for pill in &frame.pills {
                let pad = plan.size * 0.2;
                let left = line_left(pill.line) + pill.x;
                UiCanvas::new(pixels, canvas).fill(
                    Bounds {
                        origin: [
                            left - pad,
                            line_y(pill.line) - plan.size * 0.66 + lean(left + pill.width * 0.5),
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
                let left = line_left(ink.line) + ink.word.x;
                let at = [left, line_y(ink.line) + lean(left + ink.word.width * 0.5)];
                for (color, weight) in [
                    (palette.text, ink.upcoming * plan.upcoming + ink.past),
                    (highlight, ink.current),
                ] {
                    self.viz_text_in(
                        plan.face,
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
