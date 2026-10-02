use anyhow::{Result, bail};
use psychopomp::deployment::{
    DeploymentAttentionTargetPlan, DeploymentItemPlan, DeploymentPhasePlan,
};
use psychopomp::math::{lerp, smoothstep};

use super::theme::mix;
use super::{
    HeadlessRenderer, TextSprite, composite_sprite,
    text::PlainTextSpec,
    ui::{
        Bounds, Edges,
        card::{
            CardFrame, CardProjection, CardStyle, ContentFit, Fill, RgbaSource, SurfaceStyle,
            UiCanvas, UiColor,
        },
    },
};

const CARD_SIZE: [u32; 2] = [1_500, 800];
const CARD_WIDTH: f32 = CARD_SIZE[0] as f32;
const CARD_HEIGHT: f32 = CARD_SIZE[1] as f32;
const CARD_PADDING: f32 = 52.0;
const HEADER_HEIGHT: f32 = 126.0;
const FOOTER_HEIGHT: f32 = 72.0;
const QUEUE_WIDTH: f32 = 1_040.0;
const QUEUE_ROW_HEIGHT: f32 = 104.0;
const HEALTH_PANEL_WIDTH: f32 = 320.0;
const COLUMN_GAP: f32 = 36.0;

pub(crate) fn deployment_row_center_y(index: usize) -> f32 {
    180.0 + index as f32 * (QUEUE_ROW_HEIGHT + 8.0) + QUEUE_ROW_HEIGHT * 0.5
}

pub struct DeploymentQueueFrame<'a> {
    pub product: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub environment: &'a str,
    pub release: &'a str,
    pub items: &'a [DeploymentItemFrame<'a>],
    pub attention: Option<&'a DeploymentAttentionTargetPlan>,
    pub attention_transition: f32,
    pub center: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub near_edge_blur: f32,
    pub opacity: f32,
}

pub struct DeploymentItemFrame<'a> {
    pub item: &'a DeploymentItemPlan,
    pub phase: &'a DeploymentPhasePlan,
    pub previous_phase: Option<&'a DeploymentPhasePlan>,
    pub present: bool,
    pub row_y: f32,
    pub presence: f32,
    pub progress: f32,
    pub transition: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HealthState {
    Empty,
    Queued,
    Rolling,
    Blocked,
    Finalizing,
    Healthy,
}

impl HealthState {
    fn title(self) -> &'static str {
        match self {
            Self::Empty => "No services",
            Self::Queued => "Ready to deploy",
            Self::Rolling => "Rollout in progress",
            Self::Blocked => "Release blocked",
            Self::Finalizing => "Finalizing release",
            Self::Healthy => "Release healthy",
        }
    }

    fn eyebrow(self) -> &'static str {
        match self {
            Self::Empty => "NO TARGETS",
            Self::Queued => "AWAITING START",
            Self::Rolling => "AUTOMATION RUNNING",
            Self::Blocked => "ACTION REQUIRED",
            Self::Finalizing => "VERIFYING COMPLETION",
            Self::Healthy => "ALL SYSTEMS NOMINAL",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HealthSummary {
    state: HealthState,
    total: usize,
    queued: usize,
    active: usize,
    succeeded: usize,
    failed: usize,
    readiness: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HealthPresentation {
    previous: HealthSummary,
    current: HealthSummary,
    transition: f32,
}

impl HealthPresentation {
    fn displayed(self) -> HealthSummary {
        if self.transition < 0.5 {
            self.previous
        } else {
            self.current
        }
    }

    fn readiness(self) -> f32 {
        lerp(
            self.previous.readiness,
            self.current.readiness,
            unit(self.transition),
        )
    }

    fn color(self) -> [u8; 4] {
        mix(
            health_color(self.previous.state),
            health_color(self.current.state),
            unit(self.transition),
        )
    }

    fn text_opacity(self) -> f32 {
        if self.previous == self.current {
            return 1.0;
        }
        if self.transition < 0.5 {
            1.0 - self.transition * 2.0
        } else {
            (self.transition - 0.5) * 2.0
        }
    }
}

#[derive(Clone, Copy)]
struct RowPresentation {
    bounds: Bounds,
    opacity: f32,
}

#[derive(Clone, Copy)]
enum TextAlign {
    Left,
    Center,
    Right,
}

impl HeadlessRenderer {
    pub fn render_deployment_queue(&mut self, frame: &DeploymentQueueFrame<'_>) -> Result<Vec<u8>> {
        validate_frame(frame)?;

        let output_size = [self.spec.width, self.spec.height];
        if self.deployment_background_pixels.is_empty() {
            self.deployment_background_pixels =
                vec![0_u8; self.spec.width as usize * self.spec.height as usize * 4];
            let mut canvas = UiCanvas::new(&mut self.deployment_background_pixels, output_size);
            draw_background(&mut canvas);
        }
        let mut pixels = self.deployment_background_pixels.clone();

        let health = health_presentation(frame.items);
        let mut card_pixels = vec![0_u8; CARD_SIZE[0] as usize * CARD_SIZE[1] as usize * 4];
        {
            let mut canvas = UiCanvas::new(&mut card_pixels, CARD_SIZE);
            draw_dashboard_geometry(&mut canvas, frame, health);
        }
        self.composite_dashboard_text(&mut card_pixels, frame, health);

        let source = RgbaSource::packed(&card_pixels, CARD_SIZE)?;
        self.composite_ui(&mut pixels, |ui| {
            ui.card_source(
                CardFrame {
                    bounds: Bounds::from_center(frame.center, [CARD_WIDTH, CARD_HEIGHT]),
                    style: dashboard_card_style(),
                    projection: CardProjection {
                        scale: frame.scale,
                        rotation_z: frame.rotation,
                        tilt_x: frame.tilt_x,
                        tilt_y: frame.tilt_y,
                        surface_blur: 0.0,
                        near_edge_blur: frame.near_edge_blur.max(0.0),
                    },
                    opacity: unit(frame.opacity),
                },
                source,
                ContentFit::Fill,
            )
        })?;
        Ok(pixels)
    }

    fn composite_dashboard_text(
        &mut self,
        pixels: &mut [u8],
        frame: &DeploymentQueueFrame<'_>,
        health: HealthPresentation,
    ) {
        let displayed_health = health.displayed();
        let health_text_opacity = health.text_opacity();
        self.composite_deployment_text(
            pixels,
            "N",
            [76.0, 63.0],
            23.0,
            [229, 247, 255],
            true,
            1.0,
            TextAlign::Center,
        );
        self.composite_deployment_text(
            pixels,
            frame.product,
            [112.0, 43.0],
            13.0,
            [91, 202, 246],
            true,
            1.0,
            TextAlign::Left,
        );
        self.composite_deployment_text(
            pixels,
            frame.title,
            [112.0, 72.0],
            29.0,
            [235, 243, 250],
            true,
            1.0,
            TextAlign::Left,
        );
        self.composite_deployment_text(
            pixels,
            frame.subtitle,
            [112.0, 101.0],
            13.0,
            [117, 139, 164],
            false,
            1.0,
            TextAlign::Left,
        );
        self.composite_deployment_text(
            pixels,
            frame.environment,
            [1_169.0, 53.0],
            12.0,
            [116, 225, 178],
            true,
            1.0,
            TextAlign::Center,
        );
        self.composite_deployment_text(
            pixels,
            frame.release,
            [1_340.0, 53.0],
            12.0,
            [181, 201, 220],
            false,
            1.0,
            TextAlign::Center,
        );

        self.composite_deployment_text(
            pixels,
            "DEPLOYMENT QUEUE",
            [CARD_PADDING, 158.0],
            12.0,
            [137, 162, 188],
            true,
            1.0,
            TextAlign::Left,
        );
        let service_count = format!("{} SERVICES", displayed_health.total);
        self.composite_deployment_text(
            pixels,
            &service_count,
            [CARD_PADDING + QUEUE_WIDTH, 158.0],
            11.0,
            [80, 111, 142],
            false,
            health_text_opacity,
            TextAlign::Right,
        );

        for item in frame.items {
            {
                let mut canvas = UiCanvas::new(pixels, CARD_SIZE);
                draw_deployment_row(&mut canvas, frame, item);
            }
            let row = row_presentation(item);
            if row.opacity <= 0.001 {
                continue;
            }
            let x = |offset: f32| row.bounds.origin[0] + offset;
            let monogram = item
                .item
                .label
                .chars()
                .next()
                .map(|character| character.to_uppercase().collect::<String>())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "?".to_owned());
            self.composite_deployment_text(
                pixels,
                &monogram,
                [x(48.0), row.bounds.center()[1]],
                20.0,
                phase_transition_text_color(item),
                true,
                row.opacity,
                TextAlign::Center,
            );
            self.composite_deployment_text(
                pixels,
                &item.item.label,
                [x(92.0), row.bounds.center()[1] - 15.0],
                19.0,
                [226, 236, 246],
                true,
                row.opacity,
                TextAlign::Left,
            );
            let service_id = format!("{} / SERVICE", item.item.id.to_ascii_uppercase());
            self.composite_deployment_text(
                pixels,
                &service_id,
                [x(92.0), row.bounds.center()[1] + 18.0],
                11.0,
                [94, 119, 145],
                false,
                row.opacity,
                TextAlign::Left,
            );
            let chip = phase_chip_bounds(row.bounds, item);
            let (previous_opacity, current_opacity) = phase_text_opacities(item);
            if let Some(previous) = item.previous_phase {
                self.composite_deployment_text(
                    pixels,
                    phase_label(previous),
                    chip.center(),
                    11.0,
                    phase_text_color(previous),
                    true,
                    row.opacity * previous_opacity,
                    TextAlign::Center,
                );
            }
            self.composite_deployment_text(
                pixels,
                phase_label(item.phase),
                chip.center(),
                11.0,
                phase_text_color(item.phase),
                true,
                row.opacity * current_opacity,
                TextAlign::Center,
            );
            let track = progress_track_bounds(row.bounds);
            if let Some(previous) = item.previous_phase {
                self.composite_deployment_text(
                    pixels,
                    phase_detail(previous),
                    [track.origin[0], row.bounds.center()[1] - 17.0],
                    11.0,
                    [141, 145, 153],
                    false,
                    row.opacity * previous_opacity,
                    TextAlign::Left,
                );
            }
            self.composite_deployment_text(
                pixels,
                phase_detail(item.phase),
                [track.origin[0], row.bounds.center()[1] - 17.0],
                11.0,
                [141, 145, 153],
                false,
                row.opacity * current_opacity,
                TextAlign::Left,
            );
            let percent = format!("{:.0}%", progress_value(item.progress) * 100.0);
            self.composite_deployment_text(
                pixels,
                &percent,
                [row.bounds.right() - 30.0, row.bounds.center()[1] - 17.0],
                12.0,
                phase_transition_text_color(item),
                true,
                row.opacity,
                TextAlign::Right,
            );
        }

        let health_center_x = CARD_PADDING + QUEUE_WIDTH + COLUMN_GAP + HEALTH_PANEL_WIDTH * 0.5;
        self.composite_deployment_text(
            pixels,
            "RELEASE HEALTH",
            [health_center_x - HEALTH_PANEL_WIDTH * 0.5 + 24.0, 184.0],
            12.0,
            [137, 162, 188],
            true,
            1.0,
            TextAlign::Left,
        );
        self.composite_deployment_text(
            pixels,
            displayed_health.state.eyebrow(),
            [health_center_x, 212.0],
            11.0,
            health_text_color(displayed_health.state),
            true,
            health_text_opacity,
            TextAlign::Center,
        );
        let readiness = format!("{:.0}", health.readiness() * 100.0);
        self.composite_deployment_text(
            pixels,
            &readiness,
            [health_center_x, 296.0],
            39.0,
            [235, 244, 250],
            true,
            1.0,
            TextAlign::Center,
        );
        self.composite_deployment_text(
            pixels,
            "% READY",
            [health_center_x, 330.0],
            10.0,
            [105, 130, 155],
            true,
            1.0,
            TextAlign::Center,
        );
        self.composite_deployment_text(
            pixels,
            displayed_health.state.title(),
            [health_center_x, 385.0],
            18.0,
            [225, 235, 245],
            true,
            health_text_opacity,
            TextAlign::Center,
        );
        let health_detail = match displayed_health.state {
            HealthState::Empty => "No deployment targets configured".to_owned(),
            HealthState::Queued => format!("{} services waiting", displayed_health.queued),
            HealthState::Rolling => {
                format!("{} services currently in flight", displayed_health.active)
            }
            HealthState::Blocked => {
                format!("{} service health gate failed", displayed_health.failed)
            }
            HealthState::Finalizing => "Waiting for completion signal".to_owned(),
            HealthState::Healthy => format!("{} services verified", displayed_health.succeeded),
        };
        self.composite_deployment_text(
            pixels,
            &health_detail,
            [health_center_x, 414.0],
            11.0,
            [105, 130, 155],
            false,
            health_text_opacity,
            TextAlign::Center,
        );

        for (index, (label, value, color)) in [
            ("SUCCEEDED", displayed_health.succeeded, [80, 211, 151]),
            ("IN FLIGHT", displayed_health.active, [81, 186, 236]),
            ("ISSUES", displayed_health.failed, [242, 104, 113]),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 479.0 + index as f32 * 52.0;
            self.composite_deployment_text(
                pixels,
                label,
                [health_center_x - 118.0, y],
                10.0,
                [102, 128, 154],
                false,
                1.0,
                TextAlign::Left,
            );
            self.composite_deployment_text(
                pixels,
                &value.to_string(),
                [health_center_x + 118.0, y],
                14.0,
                color,
                true,
                health_text_opacity,
                TextAlign::Right,
            );
        }
        self.composite_deployment_text(
            pixels,
            "ROLLOUT READINESS",
            [health_center_x - 118.0, 651.0],
            10.0,
            [102, 128, 154],
            false,
            1.0,
            TextAlign::Left,
        );

        let footer_y = CARD_HEIGHT - FOOTER_HEIGHT * 0.5;
        self.composite_deployment_text(
            pixels,
            "NORTHSTAR / MAIN",
            [CARD_PADDING, footer_y],
            11.0,
            [116, 142, 168],
            true,
            1.0,
            TextAlign::Left,
        );
        self.composite_deployment_text(
            pixels,
            "4f8c2ae",
            [CARD_WIDTH * 0.5, footer_y],
            11.0,
            [84, 111, 138],
            false,
            1.0,
            TextAlign::Center,
        );
        self.composite_deployment_text(
            pixels,
            "ZERO-DOWNTIME POLICY",
            [CARD_WIDTH - CARD_PADDING, footer_y],
            11.0,
            [116, 225, 178],
            true,
            1.0,
            TextAlign::Right,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn composite_deployment_text(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        position: [f32; 2],
        font_size: f32,
        color: [u8; 3],
        semibold: bool,
        opacity: f32,
        align: TextAlign,
    ) -> f32 {
        let sprite = self.deployment_text_sprite(text, font_size, color, semibold);
        let advance = sprite.advance;
        let x = match align {
            TextAlign::Left => position[0],
            TextAlign::Center => position[0] - advance * 0.5,
            TextAlign::Right => position[0] - advance,
        };
        composite_sprite(
            pixels,
            CARD_SIZE[0],
            CARD_SIZE[1],
            sprite,
            x.round() as i32,
            (position[1] - sprite.height as f32 * 0.5).round() as i32,
            opacity,
        );
        advance
    }

    fn deployment_text_sprite(
        &mut self,
        text: &str,
        font_size: f32,
        color: [u8; 3],
        semibold: bool,
    ) -> &TextSprite {
        let height = (font_size * 1.55).ceil() as u32;
        self.plain_text_sprite(
            text,
            PlainTextSpec {
                font_size,
                color,
                size: [760, height],
                semibold,
                crop_to_advance: true,
            },
        )
    }
}

fn validate_frame(frame: &DeploymentQueueFrame<'_>) -> Result<()> {
    let values = [
        frame.attention_transition,
        frame.center[0],
        frame.center[1],
        frame.scale,
        frame.rotation,
        frame.tilt_x,
        frame.tilt_y,
        frame.near_edge_blur,
        frame.opacity,
    ];
    if values.into_iter().any(|value| !value.is_finite()) {
        bail!("deployment queue frame values must be finite");
    }
    if frame.scale <= 0.0 {
        bail!("deployment queue scale must be positive");
    }
    for item in frame.items {
        if [item.row_y, item.presence, item.progress, item.transition]
            .into_iter()
            .any(|value| !value.is_finite())
        {
            bail!(
                "deployment queue item '{}' frame values must be finite",
                item.item.id
            );
        }
    }
    Ok(())
}

fn dashboard_card_style() -> CardStyle {
    CardStyle {
        material: Fill::Solid(UiColor::srgb8(18, 19, 22, 255)),
        corner_radius: 30.0,
        border_width: 1.25,
        border_color: UiColor::srgb8(255, 255, 255, 38),
        shadow_offset: [0.0, 28.0],
        shadow_blur: 42.0,
        shadow_opacity: 0.28,
    }
}

fn draw_background(canvas: &mut UiCanvas<'_>) {
    canvas.fill(
        canvas.bounds(),
        0.0,
        Fill::Solid(UiColor::srgb8(225, 225, 220, 255)),
        1.0,
    );
}

fn draw_dashboard_geometry(
    canvas: &mut UiCanvas<'_>,
    frame: &DeploymentQueueFrame<'_>,
    health: HealthPresentation,
) {
    let card = canvas.bounds();
    canvas.fill(card, 0.0, Fill::Solid(UiColor::srgb8(18, 19, 22, 255)), 1.0);

    let (header, below_header) = card.split_top(HEADER_HEIGHT);
    canvas.fill(
        Bounds {
            origin: [CARD_PADDING, header.bottom() - 1.0],
            size: [CARD_WIDTH - CARD_PADDING * 2.0, 1.0],
        },
        0.0,
        Fill::Solid(UiColor::srgb8(255, 255, 255, 255)),
        0.1,
    );
    canvas.surface(
        Bounds::from_center([76.0, 63.0], [48.0, 48.0]),
        SurfaceStyle::new(Fill::Solid(UiColor::srgb8(69, 126, 235, 255)), 14.0).border(
            1.0,
            UiColor::srgb8(255, 255, 255, 255),
            0.18,
        ),
        1.0,
    );
    canvas.surface(
        Bounds::from_center([1_169.0, 53.0], [126.0, 34.0]),
        SurfaceStyle::new(Fill::Solid(UiColor::srgb8(26, 77, 63, 255)), 17.0).border(
            1.0,
            UiColor::srgb8(72, 194, 143, 255),
            0.74,
        ),
        0.68,
    );
    canvas.surface(
        Bounds::from_center([1_340.0, 53.0], [190.0, 34.0]),
        SurfaceStyle::new(Fill::Solid(UiColor::srgb8(15, 35, 56, 255)), 17.0).border(
            1.0,
            UiColor::srgb8(78, 111, 142, 255),
            0.6,
        ),
        0.9,
    );

    let (_, footer) = below_header.split_bottom(FOOTER_HEIGHT);
    canvas.fill(
        Bounds {
            origin: [CARD_PADDING, footer.origin[1]],
            size: [CARD_WIDTH - CARD_PADDING * 2.0, 1.0],
        },
        0.0,
        Fill::Solid(UiColor::srgb8(255, 255, 255, 255)),
        0.1,
    );

    let health_panel = Bounds {
        origin: [CARD_PADDING + QUEUE_WIDTH + COLUMN_GAP, 154.0],
        size: [HEALTH_PANEL_WIDTH, footer.origin[1] - 174.0],
    };
    draw_health_panel(canvas, health_panel, health);

    let queue_focus = Bounds {
        origin: [CARD_PADDING - 12.0, 178.0],
        size: [QUEUE_WIDTH + 24.0, footer.origin[1] - 194.0],
    };
    if matches!(frame.attention, Some(DeploymentAttentionTargetPlan::Queue)) {
        draw_focus_outline(
            canvas,
            queue_focus,
            24.0,
            [76, 211, 154, 255],
            transition_pulse(frame.attention_transition),
        );
    }
}

fn draw_deployment_row(
    canvas: &mut UiCanvas<'_>,
    frame: &DeploymentQueueFrame<'_>,
    item: &DeploymentItemFrame<'_>,
) {
    let row = row_presentation(item);
    if row.opacity <= 0.001 {
        return;
    }
    let color = phase_transition_color(item);
    let attention = if attention_targets_item(frame.attention, &item.item.id) {
        transition_pulse(frame.attention_transition)
    } else {
        0.0
    };
    let emphasis_color = if attention > 0.001 {
        phase_color(item.phase)
    } else {
        color
    };
    if attention > 0.001 {
        draw_focus_outline(
            canvas,
            row.bounds.expand(2.0),
            20.0,
            emphasis_color,
            row.opacity * attention * 0.75,
        );
    }
    canvas.surface(
        row.bounds,
        SurfaceStyle::new(Fill::Solid(UiColor::srgb8(25, 27, 31, 255)), 18.0).border(
            1.0,
            UiColor::srgb8(255, 255, 255, 255),
            0.1,
        ),
        row.opacity,
    );
    canvas.stroke(
        row.bounds.inset(Edges::all(1.0)),
        17.0,
        1.0,
        UiColor::srgb8(emphasis_color[0], emphasis_color[1], emphasis_color[2], 255),
        row.opacity * (transition_pulse(item.transition) * 0.16 + attention * 0.42),
    );

    let icon = Bounds::from_center(
        [row.bounds.origin[0] + 48.0, row.bounds.center()[1]],
        [52.0, 52.0],
    );
    canvas.surface(
        icon,
        SurfaceStyle::new(
            Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
            15.0,
        )
        .border(1.0, UiColor::srgb8(color[0], color[1], color[2], 255), 0.48),
        row.opacity * 0.12,
    );
    canvas.fill(
        Bounds {
            origin: [row.bounds.origin[0], row.bounds.origin[1] + 23.0],
            size: [3.0, row.bounds.size[1] - 46.0],
        },
        1.5,
        Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
        row.opacity * 0.9,
    );

    let chip = phase_chip_bounds(row.bounds, item);
    canvas.fill(
        chip,
        chip.size[1] * 0.5,
        Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
        row.opacity * 0.1,
    );
    canvas.stroke(
        chip,
        chip.size[1] * 0.5,
        1.0,
        UiColor::srgb8(color[0], color[1], color[2], 255),
        row.opacity * 0.45,
    );
    canvas.fill(
        Bounds::from_center([chip.origin[0] + 16.0, chip.center()[1]], [6.0, 6.0]),
        3.0,
        Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
        row.opacity,
    );

    let track = progress_track_bounds(row.bounds);
    canvas.fill(
        track,
        track.size[1] * 0.5,
        Fill::Solid(UiColor::srgb8(25, 48, 70, 255)),
        row.opacity,
    );
    let fill = progress_fill_bounds(track, progress_value(item.progress));
    if fill.size[0] > 0.0 {
        canvas.fill(
            fill,
            fill.size[1] * 0.5,
            Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
            row.opacity,
        );
    }
}

fn draw_health_panel(canvas: &mut UiCanvas<'_>, bounds: Bounds, health: HealthPresentation) {
    let color = health.color();
    let readiness = health.readiness();
    canvas.surface(
        bounds,
        SurfaceStyle::new(Fill::Solid(UiColor::srgb8(13, 14, 17, 255)), 22.0).border(
            1.0,
            UiColor::srgb8(255, 255, 255, 255),
            0.1,
        ),
        1.0,
    );

    let gauge = Bounds::from_center([bounds.center()[0], 300.0], [148.0, 148.0]);
    canvas.fill(
        gauge,
        74.0,
        Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
        0.06,
    );
    canvas.stroke(gauge, 74.0, 7.0, UiColor::srgb8(41, 43, 49, 255), 0.94);
    let progress_ring = gauge.inset(Edges::all(1.0 + (1.0 - readiness) * 5.0));
    canvas.stroke(
        progress_ring,
        progress_ring.size[0] * 0.5,
        4.0 + readiness * 2.0,
        UiColor::srgb8(color[0], color[1], color[2], 255),
        0.34 + readiness * 0.58,
    );

    for index in 0..2 {
        canvas.fill(
            Bounds {
                origin: [bounds.origin[0] + 24.0, 505.0 + index as f32 * 52.0],
                size: [bounds.size[0] - 48.0, 1.0],
            },
            0.0,
            Fill::Solid(UiColor::srgb8(69, 101, 130, 255)),
            0.28,
        );
    }
    let readiness_track = Bounds {
        origin: [bounds.origin[0] + 24.0, 672.0],
        size: [bounds.size[0] - 48.0, 7.0],
    };
    canvas.fill(
        readiness_track,
        3.5,
        Fill::Solid(UiColor::srgb8(24, 49, 70, 255)),
        1.0,
    );
    let readiness_fill = progress_fill_bounds(readiness_track, readiness);
    if readiness_fill.size[0] > 0.0 {
        canvas.fill(
            readiness_fill,
            3.5,
            Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
            1.0,
        );
    }
}

fn draw_focus_outline(
    canvas: &mut UiCanvas<'_>,
    bounds: Bounds,
    radius: f32,
    color: [u8; 4],
    intensity: f32,
) {
    let intensity = unit(intensity);
    if intensity <= 0.001 {
        return;
    }
    canvas.stroke(
        bounds,
        radius,
        2.0,
        UiColor::srgb8(color[0], color[1], color[2], color[3]),
        intensity * 0.72,
    );
}

fn row_presentation(item: &DeploymentItemFrame<'_>) -> RowPresentation {
    let presence = unit(item.presence);
    RowPresentation {
        bounds: Bounds::from_center(
            [
                CARD_PADDING + QUEUE_WIDTH * 0.5 + (1.0 - presence) * 18.0,
                item.row_y + (1.0 - presence) * 8.0,
            ],
            [QUEUE_WIDTH, QUEUE_ROW_HEIGHT],
        ),
        opacity: presence,
    }
}

fn phase_chip_bounds(row: Bounds, item: &DeploymentItemFrame<'_>) -> Bounds {
    let current_width = phase_chip_width(item.phase);
    let width = item
        .previous_phase
        .map(|previous| {
            lerp(
                phase_chip_width(previous),
                current_width,
                unit(phase_transition_mix(item)),
            )
        })
        .unwrap_or(current_width);
    Bounds::from_center([row.origin[0] + 510.0, row.center()[1]], [width, 34.0])
}

fn phase_chip_width(phase: &DeploymentPhasePlan) -> f32 {
    match phase {
        DeploymentPhasePlan::Queued => 112.0,
        DeploymentPhasePlan::Building { .. } => 126.0,
        DeploymentPhasePlan::Deploying { .. } => 136.0,
        DeploymentPhasePlan::Verifying { .. } => 132.0,
        DeploymentPhasePlan::Succeeded => 140.0,
        DeploymentPhasePlan::Failed => 108.0,
    }
}

fn progress_track_bounds(row: Bounds) -> Bounds {
    Bounds {
        origin: [row.origin[0] + 650.0, row.center()[1] + 15.0],
        size: [300.0, 8.0],
    }
}

fn progress_fill_bounds(track: Bounds, progress: f32) -> Bounds {
    Bounds {
        size: [track.size[0] * unit(progress), track.size[1]],
        ..track
    }
}

fn health_presentation(items: &[DeploymentItemFrame<'_>]) -> HealthPresentation {
    let transition = items
        .iter()
        .map(|item| {
            if phase_changed(item) {
                phase_transition_mix(item)
            } else if item.previous_phase.is_none() && item.present {
                unit(item.presence)
            } else if !item.present {
                1.0 - unit(item.presence)
            } else {
                1.0
            }
        })
        .fold(1.0, f32::min);
    HealthPresentation {
        previous: derive_health_for(items, true),
        current: derive_health(items),
        transition,
    }
}

fn derive_health(items: &[DeploymentItemFrame<'_>]) -> HealthSummary {
    derive_health_for(items, false)
}

fn derive_health_for(items: &[DeploymentItemFrame<'_>], previous: bool) -> HealthSummary {
    let mut summary = HealthSummary {
        state: HealthState::Empty,
        total: 0,
        queued: 0,
        active: 0,
        succeeded: 0,
        failed: 0,
        readiness: 0.0,
    };
    let mut readiness = 0.0;
    for item in items.iter().filter(|item| {
        if previous {
            item.previous_phase.is_some()
        } else {
            item.present
        }
    }) {
        let phase = if previous {
            item.previous_phase
                .expect("previously present deployment item must retain its phase")
        } else {
            item.phase
        };
        summary.total += 1;
        match phase {
            DeploymentPhasePlan::Queued => summary.queued += 1,
            DeploymentPhasePlan::Building { .. }
            | DeploymentPhasePlan::Deploying { .. }
            | DeploymentPhasePlan::Verifying { .. } => {
                summary.active += 1;
                readiness += progress_value(item.progress);
            }
            DeploymentPhasePlan::Succeeded => {
                summary.succeeded += 1;
                readiness += progress_value(item.progress);
            }
            DeploymentPhasePlan::Failed => {
                summary.failed += 1;
                readiness += progress_value(item.progress);
            }
        }
    }
    if summary.total > 0 {
        summary.readiness = readiness / summary.total as f32;
    }
    summary.state = if summary.total == 0 {
        HealthState::Empty
    } else if summary.failed > 0 {
        HealthState::Blocked
    } else if summary.succeeded == summary.total {
        if summary.readiness >= 0.999 {
            HealthState::Healthy
        } else {
            HealthState::Finalizing
        }
    } else if summary.queued == summary.total {
        HealthState::Queued
    } else {
        HealthState::Rolling
    };
    summary
}

fn progress_value(progress: f32) -> f32 {
    unit(progress)
}

fn phase_label(phase: &DeploymentPhasePlan) -> &'static str {
    match phase {
        DeploymentPhasePlan::Queued => "QUEUED",
        DeploymentPhasePlan::Building { .. } => "BUILDING",
        DeploymentPhasePlan::Deploying { .. } => "DEPLOYING",
        DeploymentPhasePlan::Verifying { .. } => "VERIFYING",
        DeploymentPhasePlan::Succeeded => "SUCCEEDED",
        DeploymentPhasePlan::Failed => "FAILED",
    }
}

fn phase_detail(phase: &DeploymentPhasePlan) -> &'static str {
    match phase {
        DeploymentPhasePlan::Queued => "Waiting for capacity",
        DeploymentPhasePlan::Building { .. } => "Compiling release artifact",
        DeploymentPhasePlan::Deploying { .. } => "Rolling out globally",
        DeploymentPhasePlan::Verifying { .. } => "Running health checks",
        DeploymentPhasePlan::Succeeded => "Verified and serving",
        DeploymentPhasePlan::Failed => "Health gate rejected",
    }
}

fn phase_color(phase: &DeploymentPhasePlan) -> [u8; 4] {
    match phase {
        DeploymentPhasePlan::Queued => [120, 143, 168, 255],
        DeploymentPhasePlan::Building { .. } => [241, 176, 93, 255],
        DeploymentPhasePlan::Deploying { .. } => [71, 185, 238, 255],
        DeploymentPhasePlan::Verifying { .. } => [96, 205, 181, 255],
        DeploymentPhasePlan::Succeeded => [72, 207, 146, 255],
        DeploymentPhasePlan::Failed => [244, 91, 104, 255],
    }
}

fn phase_transition_mix(item: &DeploymentItemFrame<'_>) -> f32 {
    if phase_changed(item) {
        smoothstep(unit(item.transition))
    } else {
        1.0
    }
}

fn phase_text_opacities(item: &DeploymentItemFrame<'_>) -> (f32, f32) {
    if !phase_changed(item) {
        return (0.0, 1.0);
    }
    let transition = phase_transition_mix(item);
    (
        1.0 - (transition * 2.0).min(1.0),
        ((transition - 0.5) * 2.0).max(0.0),
    )
}

fn phase_changed(item: &DeploymentItemFrame<'_>) -> bool {
    item.previous_phase.is_some_and(|previous| {
        std::mem::discriminant(previous) != std::mem::discriminant(item.phase)
    })
}

fn phase_transition_color(item: &DeploymentItemFrame<'_>) -> [u8; 4] {
    let current = phase_color(item.phase);
    item.previous_phase
        .map(|previous| {
            mix(
                phase_color(previous),
                current,
                unit(phase_transition_mix(item)),
            )
        })
        .unwrap_or(current)
}

fn phase_transition_text_color(item: &DeploymentItemFrame<'_>) -> [u8; 3] {
    let color = phase_transition_color(item);
    [color[0], color[1], color[2]]
}

fn phase_text_color(phase: &DeploymentPhasePlan) -> [u8; 3] {
    let color = phase_color(phase);
    [color[0], color[1], color[2]]
}

fn health_color(state: HealthState) -> [u8; 4] {
    match state {
        HealthState::Empty | HealthState::Queued => [120, 143, 168, 255],
        HealthState::Rolling | HealthState::Finalizing => [71, 185, 238, 255],
        HealthState::Blocked => [244, 91, 104, 255],
        HealthState::Healthy => [72, 207, 146, 255],
    }
}

fn health_text_color(state: HealthState) -> [u8; 3] {
    let color = health_color(state);
    [color[0], color[1], color[2]]
}

fn attention_targets_item(
    attention: Option<&DeploymentAttentionTargetPlan>,
    item_id: &str,
) -> bool {
    matches!(
        attention,
        Some(DeploymentAttentionTargetPlan::Item { item_id: target }) if target == item_id
    )
}

fn transition_pulse(transition: f32) -> f32 {
    let transition = smoothstep(unit(transition));
    4.0 * transition * (1.0 - transition)
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use psychopomp::deployment::{DeploymentItemPlan, DeploymentPhasePlan};

    use super::{
        Bounds, DeploymentItemFrame, HealthState, derive_health, health_presentation, phase_label,
        phase_text_opacities, progress_fill_bounds, progress_value, row_presentation,
    };

    #[test]
    fn phase_status_and_progress_are_derived_from_the_sampled_phase() {
        let building = DeploymentPhasePlan::Building { progress: 0.25 };
        let succeeded = DeploymentPhasePlan::Succeeded;
        let failed = DeploymentPhasePlan::Failed;

        assert_eq!(phase_label(&building), "BUILDING");
        assert_eq!(phase_label(&succeeded), "SUCCEEDED");
        assert_eq!(phase_label(&failed), "FAILED");
        assert_eq!(progress_value(0.42), 0.42);
        assert_eq!(progress_value(1.4), 1.0);
        assert_eq!(progress_value(-0.2), 0.0);
    }

    #[test]
    fn health_prioritizes_failures_and_reports_weighted_readiness() {
        let web = DeploymentItemPlan::new("web", "Web Gateway");
        let api = DeploymentItemPlan::new("api", "Public API");
        let worker = DeploymentItemPlan::new("worker", "Queue Worker");
        let succeeded = DeploymentPhasePlan::Succeeded;
        let failed = DeploymentPhasePlan::Failed;
        let deploying = DeploymentPhasePlan::Deploying { progress: 0.5 };
        let frames = [
            DeploymentItemFrame {
                item: &web,
                phase: &succeeded,
                previous_phase: None,
                present: true,
                row_y: 0.0,
                presence: 1.0,
                progress: 1.0,
                transition: 0.0,
            },
            DeploymentItemFrame {
                item: &api,
                phase: &failed,
                previous_phase: None,
                present: true,
                row_y: 0.0,
                presence: 1.0,
                progress: 1.0,
                transition: 0.0,
            },
            DeploymentItemFrame {
                item: &worker,
                phase: &deploying,
                previous_phase: None,
                present: true,
                row_y: 0.0,
                presence: 1.0,
                progress: 0.5,
                transition: 0.0,
            },
        ];

        let health = derive_health(&frames);
        assert_eq!(health.state, HealthState::Blocked);
        assert_eq!(health.total, 3);
        assert_eq!(health.succeeded, 1);
        assert_eq!(health.active, 1);
        assert_eq!(health.failed, 1);
        assert!((health.readiness - (2.5 / 3.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn all_successful_services_are_healthy_and_exiting_rows_are_ignored() {
        let web = DeploymentItemPlan::new("web", "Web Gateway");
        let api = DeploymentItemPlan::new("api", "Public API");
        let succeeded = DeploymentPhasePlan::Succeeded;
        let failed = DeploymentPhasePlan::Failed;
        let frames = [
            DeploymentItemFrame {
                item: &web,
                phase: &succeeded,
                previous_phase: None,
                present: true,
                row_y: 0.0,
                presence: 1.0,
                progress: 1.0,
                transition: 1.0,
            },
            DeploymentItemFrame {
                item: &api,
                phase: &failed,
                previous_phase: None,
                present: false,
                row_y: 0.0,
                presence: 0.0,
                progress: 0.0,
                transition: 1.0,
            },
        ];

        let health = derive_health(&frames);
        assert_eq!(health.state, HealthState::Healthy);
        assert_eq!(health.total, 1);
        assert_eq!(health.readiness, 1.0);
    }

    #[test]
    fn succeeded_phases_wait_for_readiness_before_declaring_health() {
        let api = DeploymentItemPlan::new("api", "Public API");
        let succeeded = DeploymentPhasePlan::Succeeded;
        let frame = DeploymentItemFrame {
            item: &api,
            phase: &succeeded,
            previous_phase: None,
            present: true,
            row_y: 0.0,
            presence: 1.0,
            progress: 0.8,
            transition: 1.0,
        };

        assert_eq!(derive_health(&[frame]).state, HealthState::Finalizing);
    }

    #[test]
    fn progress_geometry_clamps_to_the_track() {
        let track = Bounds {
            origin: [10.0, 20.0],
            size: [240.0, 8.0],
        };

        assert_eq!(progress_fill_bounds(track, 0.25).origin, track.origin);
        assert_eq!(progress_fill_bounds(track, 0.25).size, [60.0, 8.0]);
        assert_eq!(progress_fill_bounds(track, -1.0).size, [0.0, 8.0]);
        assert_eq!(progress_fill_bounds(track, 2.0).size, track.size);
        assert_eq!(progress_fill_bounds(track, f32::NAN).size, [0.0, 8.0]);
    }

    #[test]
    fn phase_replacement_preserves_geometry_and_sequences_text() {
        let api = DeploymentItemPlan::new("api", "Public API");
        let queued = DeploymentPhasePlan::Queued;
        let failed = DeploymentPhasePlan::Failed;
        let frame = |transition| DeploymentItemFrame {
            item: &api,
            phase: &failed,
            previous_phase: Some(&queued),
            present: true,
            row_y: 300.0,
            presence: 1.0,
            progress: 0.5,
            transition,
        };

        assert_eq!(
            row_presentation(&frame(0.0)).bounds,
            row_presentation(&frame(1.0)).bounds
        );
        assert_eq!(phase_text_opacities(&frame(0.0)), (1.0, 0.0));
        assert_eq!(phase_text_opacities(&frame(0.5)), (0.0, 0.0));
        assert_eq!(phase_text_opacities(&frame(1.0)), (0.0, 1.0));
    }

    #[test]
    fn row_layout_is_fixed_size_even_during_presence_and_phase_changes() {
        let item = DeploymentItemPlan::new("api", "Public API");
        let queued = DeploymentPhasePlan::Queued;
        for (index, expected_y) in [232.0_f32, 344.0, 456.0, 568.0, 680.0, 792.0]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                super::deployment_row_center_y(index).to_bits(),
                expected_y.to_bits()
            );
        }
        for presence in [-0.1, 0.0, 0.001, 0.37, 0.999, 1.0, 1.1] {
            for phase in [
                queued,
                DeploymentPhasePlan::Building { progress: 0.25 },
                DeploymentPhasePlan::Failed,
            ] {
                let frame = DeploymentItemFrame {
                    item: &item,
                    phase: &phase,
                    previous_phase: Some(&queued),
                    present: presence > 0.5,
                    row_y: 344.375,
                    presence,
                    progress: 0.5,
                    transition: 0.31,
                };
                let row = row_presentation(&frame);
                assert_eq!(row.bounds.size, [1040.0, 104.0]);
                assert_eq!(row.opacity, presence.clamp(0.0, 1.0));
                assert_eq!(super::progress_track_bounds(row.bounds).size, [300.0, 8.0]);
                assert_eq!(super::phase_chip_bounds(row.bounds, &frame).size[1], 34.0);
            }
        }
    }

    #[test]
    fn health_presentation_starts_from_the_previous_phase() {
        let api = DeploymentItemPlan::new("api", "Public API");
        let deploying = DeploymentPhasePlan::Deploying { progress: 0.5 };
        let failed = DeploymentPhasePlan::Failed;
        let frame = |transition| DeploymentItemFrame {
            item: &api,
            phase: &failed,
            previous_phase: Some(&deploying),
            present: true,
            row_y: 300.0,
            presence: 1.0,
            progress: 0.5,
            transition,
        };

        let start = health_presentation(&[frame(0.0)]);
        let end = health_presentation(&[frame(1.0)]);
        assert_eq!(start.displayed().state, HealthState::Rolling);
        assert_eq!(end.displayed().state, HealthState::Blocked);
        assert_eq!(start.text_opacity(), 1.0);
        assert_eq!(end.text_opacity(), 1.0);
    }

    #[test]
    fn health_presentation_does_not_rewind_unchanged_items() {
        let web = DeploymentItemPlan::new("web", "Web Gateway");
        let api = DeploymentItemPlan::new("api", "Public API");
        let worker = DeploymentItemPlan::new("worker", "Queue Worker");
        let succeeded = DeploymentPhasePlan::Succeeded;
        let failed = DeploymentPhasePlan::Failed;
        let building = DeploymentPhasePlan::Building { progress: 0.6 };
        let deploying = DeploymentPhasePlan::Deploying { progress: 0.8 };
        let frames = [
            DeploymentItemFrame {
                item: &web,
                phase: &succeeded,
                previous_phase: Some(&succeeded),
                present: true,
                row_y: 0.0,
                presence: 1.0,
                progress: 1.0,
                transition: 1.0,
            },
            DeploymentItemFrame {
                item: &api,
                phase: &building,
                previous_phase: Some(&failed),
                present: true,
                row_y: 0.0,
                presence: 1.0,
                progress: 0.5,
                transition: 0.0,
            },
            DeploymentItemFrame {
                item: &worker,
                phase: &deploying,
                previous_phase: Some(&deploying),
                present: true,
                row_y: 0.0,
                presence: 1.0,
                progress: 0.8,
                transition: 1.0,
            },
        ];

        let outgoing = health_presentation(&frames).displayed();
        assert_eq!(outgoing.succeeded, 1);
        assert_eq!(outgoing.active, 1);
        assert_eq!(outgoing.failed, 1);
    }

    #[test]
    fn insertion_interpolates_aggregate_readiness_continuously() {
        let web = DeploymentItemPlan::new("web", "Web Gateway");
        let worker = DeploymentItemPlan::new("worker", "Queue Worker");
        let succeeded = DeploymentPhasePlan::Succeeded;
        let queued = DeploymentPhasePlan::Queued;
        let readiness = |presence| {
            health_presentation(&[
                DeploymentItemFrame {
                    item: &web,
                    phase: &succeeded,
                    previous_phase: Some(&succeeded),
                    present: true,
                    row_y: 0.0,
                    presence: 1.0,
                    progress: 1.0,
                    transition: 1.0,
                },
                DeploymentItemFrame {
                    item: &worker,
                    phase: &queued,
                    previous_phase: None,
                    present: true,
                    row_y: 0.0,
                    presence,
                    progress: 0.0,
                    transition: 1.0,
                },
            ])
            .readiness()
        };

        assert!((readiness(0.49) - readiness(0.5)).abs() < 0.01);
    }
}
