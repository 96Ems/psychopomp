//! Terminal pixels: the window shell, CommitMono rows on a fixed column grid
//! through a stationary aperture under the title bar, prompts and typed
//! commands, a block caret, streamed output, highlight bars, and task lines
//! led by the blog's spinner. The layout comes from `TerminalPlan::layout`.
use anyhow::Result;
use psychopomp::{
    caption::CaptionSpanPlan,
    effects::spinner::{self, Mark},
    math::{remap_clamp, smoothstep, vec2},
    terminal::{PADDING, TerminalChannel, TerminalLinePlan, TerminalPlan, line_property},
    tone::Tone,
    window::TITLE_BAR,
};

use super::{
    HeadlessRenderer, VerticalMask,
    theme::mix,
    ui::{Bounds, card::UiCanvas},
    window::{ShellCache, WindowChrome, WindowPose, solid},
};

/// Revealed rows rise into place from this far below, in rows.
const RISE: f32 = 0.3;
const BAR_ALPHA: f32 = 0.12;

/// Channel property names per line, formatted once, and the composed shell.
pub(crate) struct TerminalNames {
    lines: Vec<[String; 6]>,
    shell: ShellCache,
}

impl TerminalNames {
    pub(crate) fn new(plan: &TerminalPlan) -> Self {
        use TerminalChannel::*;
        Self {
            shell: ShellCache::default(),
            lines: plan
                .lines
                .iter()
                .map(|line| {
                    [Reveal, Typed, Highlight, Spin, Mark, Status]
                        .map(|channel| line_property(line.id(), channel))
                })
                .collect(),
        }
    }
}

/// Opacity of a row from its reveal.
fn presence(reveal: f32) -> f32 {
    smoothstep(remap_clamp(reveal, [0.15, 1.0], [0.0, 1.0]))
}

impl HeadlessRenderer {
    pub(crate) fn composite_terminal(
        &mut self,
        pixels: &mut [u8],
        plan: &TerminalPlan,
        names: &TerminalNames,
        sample: impl Fn(&str, f32) -> f32,
    ) -> Result<()> {
        let pose = WindowPose::sample(&sample);
        if !pose.visible() {
            return Ok(());
        }
        let palette = self.theme.palette();
        let size = plan.window_size();
        let bounds = Bounds {
            origin: plan.origin,
            size,
        };
        let fill = mix(palette.background, palette.surface, 0.75);
        self.composite_window(
            pixels,
            bounds,
            pose,
            WindowChrome {
                bar: true,
                title: plan.title.as_deref(),
                fill,
                corner_radius: 14.0,
            },
            &names.shell,
        )?;
        let ink = pose.ink();
        if ink <= 0.001 {
            return Ok(());
        }
        let row = plan.row_height();
        let column = self.mono_column(plan.size);
        let origin = pose.place(plan.content_origin());
        let body = pose.place([plan.origin[0], plan.origin[1] + TITLE_BAR]);
        let mask = VerticalMask {
            top: body[1] + 1.0,
            bottom: body[1] + size[1] - TITLE_BAR - 3.0,
            fade: PADDING[1] * 0.85,
        };
        let reveals = names
            .lines
            .iter()
            .map(|line| sample(&line[0], 1.0))
            .collect::<Vec<_>>();
        let layout = plan.layout(&reveals, sample("scroll", 0.0));
        let width = size[0] - PADDING[0] * 2.0;
        // Bars first, so no surface paints over a glyph.
        for visible in &layout.rows {
            let highlight = sample(&names.lines[visible.line][2], 0.0).clamp(0.0, 1.0);
            if highlight > 0.001 {
                let top = origin[1] + visible.y * row;
                self.terminal_bar(
                    pixels,
                    Bounds {
                        origin: [origin[0] - 12.0, top],
                        size: [width + 24.0, row],
                    },
                    mask,
                    ink * presence(visible.reveal) * highlight,
                );
            }
        }
        let caret_line = layout
            .rows
            .iter()
            .rev()
            .find(|row| matches!(plan.lines[row.line], TerminalLinePlan::Command { .. }))
            .map(|row| row.line);
        let caret = sample("caret", 0.0).clamp(0.0, 1.0);
        for visible in &layout.rows {
            let names = &names.lines[visible.line];
            let alpha = ink * presence(visible.reveal);
            if alpha <= 0.001 {
                continue;
            }
            let rise = (1.0 - presence(visible.reveal)) * RISE * row;
            let center = [origin[0], origin[1] + (visible.y + 0.5) * row + rise];
            match &plan.lines[visible.line] {
                TerminalLinePlan::Command { text, .. } => {
                    let chars = text.chars().count();
                    let typed = sample(&names[1], 1.0).clamp(0.0, 1.0);
                    let shown = ((typed * chars as f32) + 1e-3).floor() as usize;
                    let prompt = self.toned(&plan.prompt);
                    let pen = self.mono_spans(
                        pixels,
                        prompt.iter().map(|(t, c)| (t.as_str(), *c)),
                        plan.size,
                        center,
                        alpha,
                        usize::MAX,
                        Some(mask),
                    );
                    let pen = self.mono_spans(
                        pixels,
                        [(text.as_str(), palette.text)],
                        plan.size,
                        [pen, center[1]],
                        alpha,
                        shown,
                        Some(mask),
                    );
                    if caret > 0.001 && caret_line == Some(visible.line) {
                        let [r, g, b] = palette.text;
                        let mut canvas = UiCanvas::new(pixels, [self.spec.width, self.spec.height]);
                        canvas.fill(
                            Bounds::from_center(
                                [pen + column * 0.5 + 1.0, center[1]],
                                [column, (plan.size * 1.18).round()],
                            ),
                            2.0,
                            solid([r, g, b]),
                            alpha * caret * 0.9 * mask_coverage(mask, center[1]),
                        );
                    }
                }
                TerminalLinePlan::Output { spans, .. } => {
                    let chars = spans.iter().map(|s| s.text.chars().count()).sum::<usize>();
                    let typed = sample(&names[1], 1.0).clamp(0.0, 1.0);
                    let shown = ((typed * chars as f32) + 1e-3).floor() as usize;
                    let toned = self.toned(spans);
                    self.mono_spans(
                        pixels,
                        toned.iter().map(|(t, c)| (t.as_str(), *c)),
                        plan.size,
                        center,
                        alpha,
                        shown,
                        Some(mask),
                    );
                }
                TerminalLinePlan::Task {
                    spans, done, mark, ..
                } => {
                    let spin = sample(&names[3], -1.0);
                    let marked = sample(&names[4], -1.0);
                    let status = sample(&names[5], 0.0).clamp(0.0, 1.0);
                    self.terminal_spinner(
                        pixels,
                        [center[0] + column * 0.8, center[1] + plan.size * 0.03],
                        column * 0.74,
                        (spin, marked, *mark),
                        alpha,
                        mask,
                    );
                    let text = [center[0] + column * 2.0, center[1]];
                    let before = self.toned(spans);
                    // Separate the outgoing and incoming text rather than
                    // showing both readable at once.
                    let (out, incoming) = if done.is_empty() {
                        (1.0, 0.0)
                    } else {
                        (
                            smoothstep(1.0 - status / 0.55),
                            smoothstep((status - 0.45) / 0.55),
                        )
                    };
                    self.mono_spans(
                        pixels,
                        before.iter().map(|(t, c)| (t.as_str(), *c)),
                        plan.size,
                        [text[0], text[1] - status * 5.0],
                        alpha * out,
                        usize::MAX,
                        Some(mask),
                    );
                    if incoming > 0.001 {
                        let after = self.toned(done);
                        self.mono_spans(
                            pixels,
                            after.iter().map(|(t, c)| (t.as_str(), *c)),
                            plan.size,
                            [text[0], text[1] + (1.0 - status) * 5.0],
                            alpha * incoming,
                            usize::MAX,
                            Some(mask),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn toned(&self, spans: &[CaptionSpanPlan]) -> Vec<(String, [u8; 3])> {
        spans
            .iter()
            .map(|span| (span.text.clone(), self.theme.tone(span.tone)))
            .collect()
    }

    fn terminal_bar(&mut self, pixels: &mut [u8], bounds: Bounds, mask: VerticalMask, alpha: f32) {
        let accent = self.theme.palette().accent;
        let canvas = [self.spec.width, self.spec.height];
        let clip = Bounds {
            origin: [0.0, mask.top],
            size: [canvas[0] as f32, mask.bottom - mask.top],
        };
        super::window::clipped(pixels, canvas, clip, 0.0, |canvas| {
            canvas.fill(bounds, 6.0, solid(accent), alpha * BAR_ALPHA);
            canvas.fill(
                Bounds {
                    origin: bounds.origin,
                    size: [3.0, bounds.size[1]],
                },
                1.5,
                solid(accent),
                alpha,
            );
        });
    }

    /// The task spinner, a ring of `radius` centered at `center`, in the
    /// theme's ink: accent while it spins, then the mark's status tone with a
    /// cooling flash.
    fn terminal_spinner(
        &mut self,
        pixels: &mut [u8],
        center: [f32; 2],
        radius: f32,
        (spin, marked, mark): (f32, f32, Mark),
        alpha: f32,
        mask: VerticalMask,
    ) {
        let pose = spinner::sample(spin, -1.0, marked, mark);
        if pose.strokes.is_empty() {
            return;
        }
        let unit = radius / spinner::RADIUS;
        let tone = match (marked >= 0.0, mark) {
            (false, _) => Tone::Accent,
            (true, Mark::Check) => Tone::Success,
            (true, Mark::Cross) => Tone::Error,
        };
        let resting = self.theme.tone(tone);
        let color = mix(resting, [255; 3], pose.flash * 0.7);
        let center = vec2(center[0], center[1]);
        for stroke in &pose.strokes {
            let points = stroke
                .iter()
                .map(|(point, weight)| (center + (*point - vec2(8.0, 8.0)) * unit, *weight))
                .collect::<Vec<_>>();
            self.weighted_stroke(
                pixels,
                &points,
                1.7 * unit,
                color,
                alpha * pose.opacity,
                Some(mask),
            );
        }
    }
}

fn mask_coverage(mask: VerticalMask, y: f32) -> f32 {
    mask.coverage(y - 0.5, y + 0.5)
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::{caption::CaptionSpanPlan, terminal::TerminalPlan, tone::Tone};

    use super::TerminalNames;
    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; a terminal opens lines in place and hidden windows leave no ink"]
    fn opening_a_line_leaves_rows_above_untouched() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "terminal-proof".into(),
        }))
        .unwrap();
        let mut plan = TerminalPlan::new([200.0, 120.0], 1200.0, 10);
        plan.lines = (0..3)
            .map(|i| psychopomp::terminal::TerminalLinePlan::Output {
                id: format!("l{i}"),
                spans: vec![CaptionSpanPlan::new(format!("line {i}"), Tone::Plain)],
            })
            .collect();
        let names = TerminalNames::new(&plan);
        let background = renderer.render_title_card("", None, 0.0);
        let mut draw = |opacity: f32, third: f32| {
            let mut pixels = background.clone();
            renderer
                .composite_terminal(&mut pixels, &plan, &names, |property, d| match property {
                    "opacity" => opacity,
                    "line.l2.reveal" => third,
                    _ => d,
                })
                .unwrap();
            pixels
        };
        assert!(draw(0.0, 1.0) == background, "hidden windows leave no ink");
        let closed = draw(1.0, 0.0);
        let half = draw(1.0, 0.5);
        assert!(closed != half);
        let top = plan.content_origin()[1] as usize;
        let above = top + 2 * plan.row_height() as usize;
        let rows = |pixels: &[u8]| pixels[top * 1920 * 4..above * 1920 * 4].to_vec();
        assert_eq!(
            rows(&closed),
            rows(&half),
            "rows above an opening line stay still"
        );
        assert!(
            draw(1.0, 0.7) == draw(1.0, 0.7),
            "sampling is deterministic"
        );
    }
}
