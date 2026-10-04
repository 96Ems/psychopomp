//! Video card pixels: decoded footage sampled straight through the shared
//! projected card (one resample, even while zoomed into a region), with a
//! theme-aware material, border, and shadow, and an optional title bar.
use anyhow::Result;
use psychopomp::video::{TITLE_BAR, VideoPlan};

use super::{
    HeadlessRenderer, PlainTextSpec, TextDraw, composite_text,
    ui::{
        Bounds,
        card::{
            CardFrame, CardProjection, CardStyle, ContentFit, Fill, RgbaSource, UiCanvas, UiColor,
        },
    },
};

/// The title bar is drawn at twice its size, so it stays crisp on a card
/// scaled past one.
const TITLE_DENSITY: f32 = 2.0;

/// One sample of a video card's channels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VideoPose {
    pub offset: [f32; 2],
    pub scale: f32,
    pub opacity: f32,
    pub rotation: f32,
    pub tilt: [f32; 2],
    pub blur: f32,
    /// The visible source window `[x, y, width, height]` in source pixels.
    pub view: [f32; 4],
}

impl HeadlessRenderer {
    pub(crate) fn composite_video(
        &mut self,
        pixels: &mut [u8],
        plan: &VideoPlan,
        frame: &[u8],
        size: [u32; 2],
        pose: VideoPose,
    ) -> Result<()> {
        let palette = self.theme.palette();
        let card_size = plan.card_size();
        let [r, g, b] = palette.surface;
        let [br, bg, bb] = palette.raised;
        let card = CardFrame {
            bounds: Bounds::from_center(
                [
                    plan.center[0] + pose.offset[0],
                    plan.center[1] + pose.offset[1],
                ],
                card_size,
            ),
            style: CardStyle {
                material: Fill::Solid(UiColor::srgb8(r, g, b, 255)),
                corner_radius: 18.0,
                border_width: 1.25,
                border_color: UiColor::srgb8(br, bg, bb, 255),
                shadow_offset: [0.0, 18.0],
                shadow_blur: 30.0,
                shadow_opacity: 0.55,
            },
            projection: CardProjection {
                scale: pose.scale.max(0.01),
                rotation_z: pose.rotation,
                tilt_x: pose.tilt[0],
                tilt_y: pose.tilt[1],
                surface_blur: 0.0,
                near_edge_blur: pose.blur,
            },
            opacity: pose.opacity,
        };
        let [fx, fy, fw, fh] = plan.footage_rect();
        let [vx, vy, vw, vh] = pose.view;
        let fit = ContentFit::Region {
            source: Bounds {
                origin: [vx, vy],
                size: [vw, vh],
            },
            content: Bounds {
                origin: [fx, fy],
                size: [fw, fh],
            },
        };
        let title = plan
            .title
            .as_deref()
            .map(|title| self.video_title_bar(title, card_size[0]));
        let source = RgbaSource::packed(frame, size)?;
        self.composite_ui(pixels, |ui| {
            ui.card_source(card, source, fit)?;
            if let Some((bar, bar_size)) = &title {
                ui.card_layer(
                    card,
                    RgbaSource::packed(bar, *bar_size)?,
                    Bounds {
                        origin: [0.0, 0.0],
                        size: [card_size[0], TITLE_BAR],
                    },
                )?;
            }
            Ok(())
        })
    }

    /// A transparent title bar: three quiet window dots, the centered title,
    /// and a hairline above the footage.
    pub(super) fn video_title_bar(&mut self, title: &str, width: f32) -> (Vec<u8>, [u32; 2]) {
        let palette = self.theme.palette();
        let size = [
            (width * TITLE_DENSITY).ceil() as u32,
            (TITLE_BAR * TITLE_DENSITY).ceil() as u32,
        ];
        let mut pixels = vec![0_u8; size[0] as usize * size[1] as usize * 4];
        let d = TITLE_DENSITY;
        let middle = TITLE_BAR * 0.5 * d;
        {
            let mut canvas = UiCanvas::new(&mut pixels, size);
            let [mr, mg, mb] = palette.muted;
            for index in 0..3 {
                canvas.fill(
                    Bounds::from_center([(24.0 + index as f32 * 18.0) * d, middle], [10.0 * d; 2]),
                    5.0 * d,
                    Fill::Solid(UiColor::srgb8(mr, mg, mb, 255)),
                    0.4,
                );
            }
            let [r, g, b] = palette.raised;
            canvas.fill(
                Bounds {
                    origin: [0.0, (TITLE_BAR - 1.0) * d],
                    size: [size[0] as f32, d],
                },
                0.0,
                Fill::Solid(UiColor::srgb8(r, g, b, 255)),
                1.0,
            );
        }
        let spec = PlainTextSpec {
            font_size: 15.0 * d,
            color: palette.muted,
            size: [size[0], (15.0 * d * 1.5).ceil() as u32],
            semibold: false,
            crop_to_advance: true,
        };
        let sprite = self.plain_text_sprite(title, spec);
        composite_text(
            &mut pixels,
            size,
            TextDraw::new(
                sprite,
                [
                    (size[0] as f32 - sprite.advance) * 0.5,
                    middle - sprite.height as f32 * 0.5,
                ],
            ),
        );
        (pixels, size)
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::video::VideoPlan;

    use super::VideoPose;
    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; the focus window crops footage inside the card"]
    fn focus_crops_into_the_footage_and_hidden_cards_leave_no_ink() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "video-proof".into(),
        }))
        .unwrap();
        // Left half black, right half white footage.
        let frame = (0..64 * 32)
            .flat_map(|i| {
                if i % 64 < 32 {
                    [0, 0, 0, 255]
                } else {
                    [255; 4]
                }
            })
            .collect::<Vec<u8>>();
        let plan = VideoPlan::new("clip", [64, 32], 30).at([960.0, 540.0], 1200.0);
        let background = renderer.render_title_card("", None, 0.0);
        let draw = |renderer: &mut HeadlessRenderer, view: [f32; 4], opacity: f32| {
            let mut pixels = background.clone();
            let pose = VideoPose {
                offset: [0.0, 0.0],
                scale: 1.0,
                opacity,
                rotation: 0.0,
                tilt: [0.0, 0.0],
                blur: 0.0,
                view,
            };
            renderer
                .composite_video(&mut pixels, &plan, &frame, [64, 32], pose)
                .unwrap();
            pixels
        };
        let left = |pixels: &[u8]| pixels[(540 * 1920 + 600) * 4];
        let whole = draw(&mut renderer, plan.view([0.5, 0.5], 1.0), 1.0);
        assert_eq!(left(&whole), 0, "the card's left shows the black half");
        let focused = draw(&mut renderer, plan.view([0.75, 0.5], 0.5), 1.0);
        assert_eq!(
            left(&focused),
            255,
            "focusing right fills the card with white"
        );
        assert!(draw(&mut renderer, plan.view([0.5, 0.5], 1.0), 0.0) == background);
    }
}
