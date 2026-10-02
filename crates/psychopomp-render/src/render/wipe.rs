//! Wipe pixels: the incoming frame behind an antialiased divider, a thin
//! theme-ink line with a soft shadow cast onto the outgoing frame, and
//! optional side labels that ride the divider and give way at the edges.
use psychopomp::{
    caption::{CaptionAlign, CaptionPlan, CaptionSpanPlan},
    math::{lerp, smoothstep},
    plan::{WipeDirection, WipePhase},
    tone::Tone,
};

use super::{HeadlessRenderer, PlainTextSpec};

/// How far, in pixels, the divider's shadow reaches onto the outgoing frame.
const SHADOW_REACH: f32 = 26.0;
const SHADOW_OPACITY: f32 = 0.34;
/// Half the divider line's width.
const LINE_HALF_WIDTH: f32 = 1.0;
/// The line and labels fade in over this distance from the frame edge.
const EDGE_FADE: f32 = 40.0;
const LABEL_SIZE: f32 = 15.0;
/// Gap between the divider and a label's chip.
const LABEL_GAP: f32 = 14.0;

impl HeadlessRenderer {
    /// Show `incoming` behind the divider of `wipe` over `outgoing`.
    pub(crate) fn composite_wipe(
        &mut self,
        outgoing: &mut [u8],
        incoming: &[u8],
        wipe: WipePhase,
        labels: Option<&[String; 2]>,
    ) {
        let (width, height) = (self.spec.width as usize, self.spec.height as usize);
        let horizontal = matches!(wipe.direction, WipeDirection::Right | WipeDirection::Left);
        let extent = if horizontal { width } else { height } as f32;
        // The divider's coordinate along its axis, and which side is incoming.
        let (divider, incoming_before) = match wipe.direction {
            WipeDirection::Right | WipeDirection::Down => (wipe.position * extent, true),
            WipeDirection::Left | WipeDirection::Up => ((1.0 - wipe.position) * extent, false),
        };
        let edge = smoothstep((divider.min(extent - divider) / EDGE_FADE).clamp(0.0, 1.0));
        let [lr, lg, lb] = self.theme.palette().text;
        // Every pixel's coverage depends only on its distance along one axis.
        let profile = (0..extent as usize)
            .map(|index| {
                let center = index as f32 + 0.5;
                // Positive on the incoming side of the divider.
                let side = if incoming_before {
                    divider - center
                } else {
                    center - divider
                };
                let cover = (side + 0.5).clamp(0.0, 1.0);
                let shadow = if side < 0.0 {
                    SHADOW_OPACITY * edge * (-(side / SHADOW_REACH).powi(2) * 2.0).exp()
                } else {
                    0.0
                };
                let line = (LINE_HALF_WIDTH + 0.5 - side.abs()).clamp(0.0, 1.0) * 0.92 * edge;
                (cover, shadow, line)
            })
            .collect::<Vec<_>>();
        for y in 0..height {
            for x in 0..width {
                let (cover, shadow, line) = profile[if horizontal { x } else { y }];
                if cover <= 0.0 && shadow <= 0.0 && line <= 0.0 {
                    continue;
                }
                let index = (y * width + x) * 4;
                for channel in 0..3 {
                    let below = f32::from(outgoing[index + channel]) * (1.0 - shadow);
                    let mixed = lerp(below, f32::from(incoming[index + channel]), cover);
                    let ink = f32::from([lr, lg, lb][channel]);
                    outgoing[index + channel] = lerp(mixed, ink, line).round() as u8;
                }
            }
        }
        if let Some(labels) = labels {
            self.composite_wipe_labels(
                outgoing,
                labels,
                divider,
                incoming_before,
                horizontal,
                edge,
            );
        }
    }

    /// One chip on each side of the divider, near the top (or left) edge.
    /// A label fades as its side narrows past its own width.
    fn composite_wipe_labels(
        &mut self,
        pixels: &mut [u8],
        [outgoing, incoming]: &[String; 2],
        divider: f32,
        incoming_before: bool,
        horizontal: bool,
        edge: f32,
    ) {
        let extent = if horizontal {
            self.spec.width
        } else {
            self.spec.height
        } as f32;
        let spec = PlainTextSpec {
            font_size: LABEL_SIZE,
            color: [0, 0, 0],
            size: [1600, (LABEL_SIZE * 1.5).ceil() as u32],
            semibold: false,
            crop_to_advance: true,
        };
        let chip_padding = LABEL_SIZE * 0.7;
        for (text, tone, is_incoming) in [
            (outgoing, Tone::Muted, false),
            (incoming, Tone::Plain, true),
        ] {
            let before = is_incoming == incoming_before;
            let advance = self.plain_text_sprite(text, spec).advance;
            let reach = if horizontal {
                advance + chip_padding * 2.0
            } else {
                LABEL_SIZE * 1.45 + 8.0
            } + LABEL_GAP;
            let room = if before { divider } else { extent - divider };
            let opacity = edge * smoothstep(((room - reach - 12.0) / 36.0).clamp(0.0, 1.0));
            if opacity <= 0.001 {
                continue;
            }
            let plan = if horizontal {
                let offset = LABEL_GAP + chip_padding;
                let (x, align) = if before {
                    (divider - offset, CaptionAlign::Right)
                } else {
                    (divider + offset, CaptionAlign::Left)
                };
                CaptionPlan::line(
                    [x, 56.0],
                    LABEL_SIZE,
                    vec![CaptionSpanPlan::new(text, tone)],
                )
                .aligned(align)
            } else {
                let offset = LABEL_GAP + LABEL_SIZE * 1.45 * 0.5 + 4.0;
                let y = if before {
                    divider - offset
                } else {
                    divider + offset
                };
                CaptionPlan::line(
                    [56.0 + chip_padding, y],
                    LABEL_SIZE,
                    vec![CaptionSpanPlan::new(text, tone)],
                )
            };
            self.composite_caption(pixels, &plan.chip(), |property, default| {
                if property == "opacity" {
                    opacity
                } else {
                    default
                }
            });
        }
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::plan::{WipeDirection, WipePhase};

    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; the incoming frame shows only behind the divider"]
    fn a_left_wipe_keeps_the_outgoing_frame_on_the_left() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "wipe-proof".into(),
        }))
        .unwrap();
        let mut outgoing = vec![0_u8; 1920 * 1080 * 4];
        let incoming = vec![200_u8; 1920 * 1080 * 4];
        let wipe = WipePhase {
            position: 0.25,
            direction: WipeDirection::Left,
        };
        renderer.composite_wipe(&mut outgoing, &incoming, wipe, None);
        let at = |x: usize| outgoing[(540 * 1920 + x) * 4];
        assert_eq!(at(100), 0, "outgoing stays left of the divider");
        assert_eq!(at(1800), 200, "incoming shows right of it");
        assert!(at(1440) > 200, "the divider line is theme ink");
        assert!(at(1400) == 0, "the shadow darkens, never lightens");
    }
}
