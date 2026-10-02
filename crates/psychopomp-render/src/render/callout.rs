//! Callout pixels: a dot and ring marking the anchor, a crisp analytic leader
//! drawn out from the ring by `draw`, and the label (optionally on a chip)
//! that rises in at the leader's end. Geometry comes from the lightweight
//! `callout::layout`; this module only measures the label and paints.
use psychopomp::{
    callout::{CalloutLeg, CalloutPlan, layout},
    math::{Vec2, curve::Polyline, lerp, shapes::Box2, smoothstep, vec2},
};

use super::{
    HeadlessRenderer, PlainTextSpec, TextDraw, composite_text,
    ui::{
        Bounds,
        card::{Fill, SurfaceStyle, UiCanvas, UiColor},
    },
};

const DOT: f32 = 3.5;
const RING: f32 = 8.0;
const RING_WIDTH: f32 = 1.5;
const LEADER: f32 = 1.6;
/// Labels keep this far from the frame edge.
const MARGIN: f32 = 40.0;
const RISE: f32 = 8.0;

/// A callout at one sample: its resolved anchor, its blended leader shape,
/// and its channels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CalloutPose {
    pub anchor: Vec2,
    pub leg: CalloutLeg,
    pub opacity: f32,
    pub draw: f32,
    pub label: f32,
    pub emphasis: f32,
}

impl HeadlessRenderer {
    pub(crate) fn composite_callout(
        &mut self,
        pixels: &mut [u8],
        plan: &CalloutPlan,
        pose: CalloutPose,
    ) {
        let opacity = pose.opacity.clamp(0.0, 1.0);
        let draw = pose.draw.clamp(0.0, 1.0);
        let label = pose.label.clamp(0.0, 1.0);
        let emphasis = pose.emphasis.clamp(0.0, 1.0);
        if opacity <= 0.001 || (draw <= 0.001 && label <= 0.001) {
            return;
        }
        let canvas = self.size();
        let spec = PlainTextSpec {
            font_size: plan.size,
            color: [0, 0, 0],
            size: [1400, (plan.size * 1.5).ceil() as u32],
            semibold: false,
            crop_to_advance: true,
        };
        let widths = plan
            .lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|span| {
                        if span.text.is_empty() {
                            return 0.0;
                        }
                        let color = self.theme.tone(span.tone);
                        self.plain_text_sprite(&span.text, PlainTextSpec { color, ..spec })
                            .advance
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let line_height = plan.line_height();
        let text = vec2(
            widths
                .iter()
                .map(|line| line.iter().sum::<f32>())
                .fold(0.0, f32::max),
            line_height * plan.lines.len() as f32,
        );
        let padding = if plan.chip {
            vec2(plan.size * 0.6, plan.size * 0.3)
        } else {
            Vec2::ZERO
        };
        let size = text + padding * 2.0;
        let frame = Box2 {
            min: Vec2::splat(MARGIN),
            max: vec2(canvas[0] as f32, canvas[1] as f32) - MARGIN,
        };
        let gap = if plan.chip { 0.0 } else { 10.0 };
        let placed = layout(pose.anchor, pose.leg, size, gap, frame);
        let tone = self.theme.ink(self.theme.tone(plan.tone));

        // The leader leaves the ring's edge and draws out toward the label.
        let [anchor, knee, end] = placed.leader;
        let start = anchor + (knee - anchor).normalize_or_zero() * (RING + RING_WIDTH);
        let leader = Polyline::new(vec![start, knee, end]).slice(0.0, draw);
        if leader.points().len() > 1 {
            let points = leader
                .points()
                .iter()
                .map(|point| point.to_array())
                .collect::<Vec<_>>();
            self.composite_prototype_path(
                pixels,
                &points,
                LEADER + 0.6 * emphasis,
                tone,
                opacity * lerp(0.85, 1.0, emphasis),
            );
        }

        // The mark arrives as the line leaves it and goes as the line retracts.
        let mark = smoothstep(draw / 0.2);
        if mark > 0.001 {
            let mut ui = UiCanvas::new(pixels, canvas);
            let color = UiColor::srgb8(tone[0], tone[1], tone[2], 255);
            if emphasis > 0.001 {
                let halo = RING + 2.0 + 8.0 * emphasis;
                ui.fill(
                    Bounds::from_center(anchor.to_array(), [halo * 2.0; 2]),
                    halo,
                    Fill::Solid(color),
                    opacity * 0.16 * emphasis,
                );
            }
            let ring = (RING + RING_WIDTH * 0.5) * lerp(0.6, 1.0, mark);
            ui.stroke(
                Bounds::from_center(anchor.to_array(), [ring * 2.0; 2]),
                ring,
                RING_WIDTH,
                color,
                opacity * mark * lerp(0.7, 1.0, emphasis),
            );
            let dot = DOT * mark;
            ui.fill(
                Bounds::from_center(anchor.to_array(), [dot * 2.0; 2]),
                dot,
                Fill::Solid(color),
                opacity * mark,
            );
        }

        // The label fades and rises into place.
        let alpha = opacity * label;
        if alpha <= 0.001 {
            return;
        }
        let rise = (1.0 - label) * RISE;
        let origin = placed.label.min + vec2(0.0, rise);
        if plan.chip {
            let palette = self.theme.palette();
            let border = [0, 1, 2].map(|i| {
                lerp(
                    f32::from(palette.raised[i]),
                    f32::from(tone[i]),
                    emphasis * 0.7,
                )
                .round() as u8
            });
            let [r, g, b] = palette.surface;
            UiCanvas::new(pixels, canvas).surface(
                Bounds {
                    origin: origin.to_array(),
                    size: size.to_array(),
                },
                SurfaceStyle::new(Fill::Solid(UiColor::srgb8(r, g, b, 255)), 8.0).border(
                    1.2,
                    UiColor::srgb8(border[0], border[1], border[2], 255),
                    1.0,
                ),
                alpha,
            );
        }
        for (index, line) in plan.lines.iter().enumerate() {
            let center_y = origin.y + padding.y + line_height * (index as f32 + 0.5);
            let mut x = origin.x + padding.x;
            for (span, width) in line.iter().zip(&widths[index]) {
                if span.text.is_empty() {
                    continue;
                }
                let color = self.theme.tone(span.tone);
                let sprite = self.plain_text_sprite(&span.text, PlainTextSpec { color, ..spec });
                composite_text(
                    pixels,
                    canvas,
                    TextDraw {
                        clip_width: width.ceil() + 1.0,
                        opacity: alpha,
                        ..TextDraw::new(sprite, [x, center_y - spec.size[1] as f32 * 0.5])
                    },
                );
                x += width;
            }
        }
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::{
        callout::{CalloutAnchorPlan, CalloutLeg, CalloutPlan, CalloutSide},
        caption::CaptionSpanPlan,
        math::vec2,
        tone::Tone,
    };

    use super::CalloutPose;
    use crate::render::HeadlessRenderer;

    #[test]
    #[ignore = "requires a headless GPU; the leader draws on from the anchor and leaves no residue"]
    fn callouts_draw_on_from_their_anchor() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(crate::render::RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "callout-proof".into(),
        }))
        .unwrap();
        let plan = CalloutPlan::new(
            CalloutAnchorPlan::Point {
                id: "here".into(),
                at: [600.0, 600.0],
                side: None,
            },
            vec![CaptionSpanPlan::new("retries here", Tone::Plain)],
        )
        .side(CalloutSide::Right)
        .reach(200.0);
        let background = renderer.render_title_card("", None, 0.);
        let draw = |renderer: &mut HeadlessRenderer, draw: f32, label: f32| {
            let mut pixels = background.clone();
            renderer.composite_callout(
                &mut pixels,
                &plan,
                CalloutPose {
                    anchor: vec2(600.0, 600.0),
                    leg: CalloutLeg::new(CalloutSide::Right, 200.0, false),
                    opacity: 1.0,
                    draw,
                    label,
                    emphasis: 0.0,
                },
            );
            pixels
        };
        assert!(
            draw(&mut renderer, 0.0, 0.0) == background,
            "hidden callouts leave no ink"
        );
        let inked = |pixels: &[u8], x: usize| {
            let i = (600 * 1920 + x) * 4;
            pixels[i..i + 3] != background[i..i + 3]
        };
        let half = draw(&mut renderer, 0.5, 0.0);
        assert!(inked(&half, 600), "the dot marks the anchor");
        assert!(inked(&half, 650), "the leader starts at the anchor");
        assert!(!inked(&half, 790), "and has not reached the label yet");
        let full = draw(&mut renderer, 1.0, 1.0);
        assert!(inked(&full, 790));
    }
}
