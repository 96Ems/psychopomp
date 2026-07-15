use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::OnceLock,
};

use anyhow::Result;
use cosmic_text::{Attrs, Color, Family, Metrics, Weight};

use crate::dsl::{TaskFrame, TaskState};
use crate::motion::{MotionState, Spring};

use super::{
    HeadlessRenderer, TextSprite, blend_pixel, composite_sprite, composite_sprite_rotated,
    make_sprite, rasterize_svg,
};

const QUOTE_WORDS: [&str; 8] = ["why", "would", "I", "ever", "want", "to", "use", "Effect?"];
const NODE_SIZE: f32 = 128.0;
const RUNNING_JITTER_MIN_INTERVAL: f32 = 0.055;
const RUNNING_JITTER_MAX_INTERVAL: f32 = 0.14;
const RUNNING_JITTER_LAMBDA: f32 = 28.0;
const ENERGY_SPEED: f32 = 500.0;
const ENERGY_SPACING: f32 = 122.0;

#[derive(Clone, Copy)]
pub struct QuoteFrame<'a> {
    pub time: f32,
    pub highlight: Option<usize>,
    pub show_hypnotic: bool,
    pub hide_quote: bool,
    pub subliminal: Option<&'a str>,
}

pub struct TaskSceneFrame<'a> {
    pub quote: Option<QuoteFrame<'a>>,
    pub links: &'a [TaskLinkFrame],
    pub nodes: &'a [TaskFrame<'a>],
}

#[derive(Clone, Copy)]
pub struct TaskLinkFrame {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub pulse: Option<f32>,
}

impl HeadlessRenderer {
    pub fn measure_task_result_width(&mut self, result: &str) -> f32 {
        let sprite = self.task_text_sprite(result, 32.0 * 1.15, [245, 250, 247]);
        (sprite.advance + 72.0).ceil().clamp(NODE_SIZE, 520.0)
    }

    pub fn render_task_scene(&mut self, frame: &TaskSceneFrame<'_>) -> Result<Vec<u8>> {
        let mut pixels = vec![0_u8; self.spec.width as usize * self.spec.height as usize * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[1, 2, 4, 255]);
        }
        self.composite_task_scene(&mut pixels, frame)?;
        Ok(pixels)
    }

    pub fn composite_task_scene(
        &mut self,
        pixels: &mut [u8],
        frame: &TaskSceneFrame<'_>,
    ) -> Result<()> {
        let expected = self.spec.width as usize * self.spec.height as usize * 4;
        if pixels.len() != expected {
            anyhow::bail!("expected {expected} frame bytes, received {}", pixels.len());
        }
        if let Some(quote) = frame.quote {
            self.composite_quote(pixels, quote);
        }
        for link in frame.links {
            draw_task_link(pixels, self.spec.width, self.spec.height, *link);
        }
        for node in frame.nodes {
            self.composite_task_node(pixels, *node);
        }
        Ok(())
    }

    fn composite_quote(&mut self, pixels: &mut [u8], frame: QuoteFrame<'_>) {
        let center = [self.spec.width as f32 * 0.5, self.spec.height as f32 * 0.5];
        if frame.show_hypnotic {
            for index in 0..12 {
                let phase = (frame.time / 10.0 + index as f32 / 12.0).fract();
                let radius = 60.0 + phase * 510.0;
                let opacity = (1.0 - (phase * 2.0 - 1.0).abs()).max(0.0) * 0.16;
                draw_ring(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    center,
                    radius,
                    [210, 220, 230, 255],
                    opacity,
                );
            }
        }
        if let Some(word) = frame.subliminal {
            self.composite_task_text(
                pixels,
                word,
                center[0],
                center[1],
                50.0,
                [255, 255, 255],
                0.18,
            );
        }
        if frame.hide_quote {
            draw_face(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                62.0,
                frame.time,
            );
            return;
        }

        let widths = QUOTE_WORDS.map(|word| word.len() as f32 * 18.0 + 12.0);
        let total_width = widths.iter().sum::<f32>();
        let mut x = center[0] - total_width * 0.5;
        for (index, word) in QUOTE_WORDS.into_iter().enumerate() {
            let current = frame.highlight == Some(index);
            let past = frame.highlight.is_some_and(|highlight| index < highlight);
            self.composite_task_text(
                pixels,
                word,
                x + widths[index] * 0.5,
                center[1],
                if current { 38.0 } else { 34.0 },
                if current {
                    [245, 245, 245]
                } else {
                    [130, 135, 145]
                },
                if past { 0.42 } else { 1.0 },
            );
            x += widths[index];
        }
    }

    fn composite_task_node(&mut self, pixels: &mut [u8], node: TaskFrame<'_>) {
        let entrance_blur = 6.0 * (-18.0 * node.visible_age).exp();
        let exit_blur = if matches!(node.state, TaskState::Hidden) {
            smoothstep((node.state_age / 0.25).clamp(0.0, 1.0)) * 4.0
        } else {
            0.0
        };
        let blur = entrance_blur + exit_blur;
        if blur < 0.1 {
            self.composite_task_node_contents(pixels, node);
            return;
        }

        let mut layer = std::mem::take(&mut self.task_layer_pixels);
        layer.resize(pixels.len(), 0);
        layer.fill(0);
        self.composite_task_node_contents(&mut layer, node);
        let mut blur_source = std::mem::take(&mut self.task_blur_source);
        let mut blur_scratch = std::mem::take(&mut self.task_blur_scratch);
        let bounds = blur_task_layer(
            &mut layer,
            self.spec.width,
            self.spec.height,
            [node.x, node.y],
            blur,
            &mut blur_source,
            &mut blur_scratch,
        );
        if let Some(bounds) = bounds {
            for y in bounds.min_y..=bounds.max_y {
                for x in bounds.min_x..=bounds.max_x {
                    let index = (y * self.spec.width as usize + x) * 4;
                    let source = &layer[index..index + 4];
                    if source[3] > 0 {
                        blend_pixel_linear(
                            &mut pixels[index..index + 4],
                            [source[0], source[1], source[2], source[3]],
                        );
                    }
                }
            }
        }
        self.task_layer_pixels = layer;
        self.task_blur_source = blur_source;
        self.task_blur_scratch = blur_scratch;
    }

    fn composite_task_node_contents(&mut self, pixels: &mut [u8], node: TaskFrame<'_>) {
        let result = match node.state {
            TaskState::Succeeded(result) => result.as_deref(),
            _ => None,
        };
        let error = match node.state {
            TaskState::Failed(error) | TaskState::Death(error) => {
                (!error.is_empty()).then_some(error.as_str())
            }
            _ => None,
        };
        let enter = spring_progress(node.visible_age, 0.45, 0.72).max(0.0);
        let width_transition = motion_spring_progress(node.state_age, 0.35, 0.35);
        let height_transition = motion_spring_progress(node.state_age, 0.2, 0.5);
        let scale_transition = if matches!(node.state, TaskState::Death(_)) {
            motion_spring_progress(node.state_age, 0.45, 0.0)
        } else {
            motion_spring_progress(node.state_age, 3.0 / 18.0, 0.0)
        };
        let previous_size = task_node_size(node.previous_state, node.result_width);
        let target_size = if matches!(node.state, TaskState::Hidden) {
            previous_size
        } else {
            task_node_size(node.state, node.result_width)
        };
        let mut width = previous_size[0] + (target_size[0] - previous_size[0]) * width_transition;
        let mut height = previous_size[1] + (target_size[1] - previous_size[1]) * height_transition;
        let previous_scale = task_node_scale(node.previous_state);
        let target_scale = if matches!(node.state, TaskState::Hidden) {
            previous_scale
        } else {
            task_node_scale(node.state)
        };
        let mut scale =
            enter * (previous_scale + (target_scale - previous_scale) * scale_transition);
        let exit = if matches!(node.state, TaskState::Hidden) {
            1.0 - smoothstep((node.state_age / 0.25).clamp(0.0, 1.0))
        } else {
            1.0
        };
        scale *= exit;
        let mut offset = [0.0, 0.0];
        let mut rotation = 0.0;
        let mut charge_energy = 0.0;
        let target_color = match node.state {
            TaskState::Hidden => task_state_color(node.previous_state),
            TaskState::Idle => [71, 85, 105],
            TaskState::Running => {
                let jitter = running_jitter(node.id.as_str(), node.state_age);
                offset = [jitter[0], jitter[1]];
                rotation = jitter[2];
                charge_energy =
                    (jitter[0].abs() / 3.4 * 0.6 + jitter[1].abs() / 1.6 * 0.4).min(1.0);
                [59, 130, 246]
            }
            TaskState::Succeeded(_) => [21, 128, 61],
            TaskState::Failed(_) => {
                let jitter = failure_jitter(node.id.as_str(), node.state_age, 0.32);
                offset = [jitter[0], jitter[1]];
                rotation = jitter[2];
                [239, 68, 68]
            }
            TaskState::Death(_) => {
                offset[1] = 6.0 * (1.0 - (-8.0 * node.state_age).exp());
                [8, 8, 9]
            }
        };
        let previous_color = task_state_color(node.previous_state);
        let color_mix = 1.0 - (-12.0 * node.state_age).exp();
        let color = [
            mix_channel(previous_color[0], target_color[0], color_mix),
            mix_channel(previous_color[1], target_color[1], color_mix),
            mix_channel(previous_color[2], target_color[2], color_mix),
        ];
        let (flash_duration, flash_mix, flash_color) = match node.state {
            TaskState::Succeeded(_) => (0.45, 0.38, [55, 163, 95]),
            TaskState::Failed(_) => (0.32, 0.45, [244, 92, 92]),
            TaskState::Death(_) => (0.45, 0.5, [255, 45, 45]),
            TaskState::Running => (0.18, 0.12, [92, 158, 248]),
            TaskState::Idle | TaskState::Hidden => (0.2, 0.12, [100, 112, 130]),
        };
        let remaining = (1.0 - node.state_age / flash_duration).clamp(0.0, 1.0);
        let attack = smoothstep((node.state_age / 0.025).clamp(0.0, 1.0));
        let flash = (remaining * std::f32::consts::FRAC_PI_2).sin() * flash_mix * attack;
        let color = [
            mix_channel(color[0], flash_color[0], flash),
            mix_channel(color[1], flash_color[1], flash),
            mix_channel(color[2], flash_color[2], flash),
        ];
        let center = [node.x + offset[0], node.y + offset[1]];
        width *= scale;
        height *= scale;

        if matches!(node.state, TaskState::Running) {
            let glow = 0.2 + charge_energy * 0.18;
            draw_soft_rect_glow(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                [width, height],
                rotation,
                8.0,
                [59, 130, 246, 255],
                glow,
            );
        }
        fill_rotated_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            center,
            [width, height],
            rotation,
            [color[0], color[1], color[2], 255],
            1.0,
        );
        if matches!(node.state, TaskState::Running) {
            draw_energy_sweep(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                [width, height],
                rotation,
                node.state_age,
            );
            stroke_rotated_rect(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                [width, height],
                rotation,
                [130, 200, 255, 255],
                0.45 + charge_energy * 0.5,
            );
        }
        if matches!(node.state, TaskState::Death(_)) {
            stroke_rotated_rect(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                [width, height],
                rotation,
                [255, 45, 45, 255],
                0.9,
            );
        }

        match node.state {
            TaskState::Idle => self.composite_task_icon(
                pixels,
                "sparkle",
                center,
                58.0 * enter,
                0.0,
                enter,
                0.0,
                [245, 245, 245],
            ),
            TaskState::Running => {
                let overlap = 1.0 - smoothstep((node.state_age / 0.14).clamp(0.0, 1.0));
                match node.previous_state {
                    TaskState::Idle => self.composite_task_icon(
                        pixels,
                        "sparkle",
                        center,
                        58.0 * (0.9 + overlap * 0.1),
                        rotation,
                        overlap,
                        (1.0 - overlap) * 3.0,
                        [245, 245, 245],
                    ),
                    TaskState::Failed(_) | TaskState::Death(_) => self.composite_task_icon(
                        pixels,
                        "error",
                        center,
                        58.0 * (0.9 + overlap * 0.1),
                        rotation,
                        overlap,
                        (1.0 - overlap) * 4.0,
                        [255, 245, 245],
                    ),
                    _ => {}
                }
                if let TaskState::Failed(error) = node.previous_state
                    && !error.is_empty()
                    && node.state_age < 0.2
                {
                    let fade = 1.0 - smoothstep((node.state_age / 0.2).clamp(0.0, 1.0));
                    self.composite_task_bubble(
                        pixels,
                        error,
                        center,
                        height,
                        fade,
                        node.state_age * -18.0,
                    );
                }
            }
            TaskState::Succeeded(_) => {
                let content = spring_progress(node.state_age, 0.25, 0.68);
                draw_state_pulse(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    center,
                    [width, height],
                    rotation,
                    node.state_age,
                    0.55,
                    [120, 255, 170, 255],
                    0.08,
                    true,
                );
                if let Some(result) = result {
                    self.composite_task_text(
                        pixels,
                        result,
                        center[0],
                        center[1],
                        32.0 * 1.15 * (0.5 + content * 0.5),
                        [245, 250, 247],
                        content.clamp(0.0, 1.0),
                    );
                } else {
                    self.composite_task_text(
                        pixels,
                        "✓",
                        center[0],
                        center[1],
                        46.0 * (0.6 + content * 0.4),
                        [245, 250, 247],
                        content.clamp(0.0, 1.0),
                    );
                }
            }
            TaskState::Failed(_) | TaskState::Death(_) => {
                let icon = spring_progress(node.state_age, 0.25, 0.72);
                draw_state_pulse(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    center,
                    [width, height],
                    rotation,
                    node.state_age,
                    0.55,
                    if matches!(node.state, TaskState::Death(_)) {
                        [255, 45, 45, 255]
                    } else {
                        [255, 110, 110, 255]
                    },
                    0.65,
                    false,
                );
                self.composite_task_icon(
                    pixels,
                    "error",
                    center,
                    58.0 * icon,
                    rotation,
                    icon.clamp(0.0, 1.0),
                    (1.0 - icon.clamp(0.0, 1.0)) * 6.0,
                    if matches!(node.state, TaskState::Death(_)) {
                        [255, 45, 45]
                    } else {
                        [255, 245, 245]
                    },
                );
                if (0.06..3.0).contains(&node.state_age)
                    && let Some(error) = error
                {
                    let rise = spring_progress(node.state_age - 0.06, 0.25, 0.65);
                    self.composite_task_bubble(
                        pixels,
                        error,
                        center,
                        height,
                        rise.clamp(0.0, 1.0),
                        (1.0 - rise) * 32.0,
                    );
                }
            }
            TaskState::Hidden => match node.previous_state {
                TaskState::Idle => self.composite_task_icon(
                    pixels,
                    "sparkle",
                    center,
                    58.0 * exit,
                    rotation,
                    exit,
                    (1.0 - exit) * 5.0,
                    [245, 245, 245],
                ),
                TaskState::Succeeded(Some(result)) => self.composite_task_text(
                    pixels,
                    result,
                    center[0],
                    center[1],
                    32.0 * 1.15,
                    [245, 250, 247],
                    exit,
                ),
                TaskState::Succeeded(None) => self.composite_task_text(
                    pixels,
                    "✓",
                    center[0],
                    center[1],
                    46.0,
                    [245, 250, 247],
                    exit,
                ),
                TaskState::Running => {
                    draw_energy_sweep(
                        pixels,
                        self.spec.width,
                        self.spec.height,
                        center,
                        [width, height],
                        rotation,
                        node.state_age,
                    );
                    stroke_rotated_rect(
                        pixels,
                        self.spec.width,
                        self.spec.height,
                        center,
                        [width, height],
                        rotation,
                        [130, 200, 255, 255],
                        exit * 0.5,
                    );
                }
                TaskState::Failed(_) | TaskState::Death(_) => self.composite_task_icon(
                    pixels,
                    "error",
                    center,
                    58.0 * exit,
                    rotation,
                    exit,
                    (1.0 - exit) * 5.0,
                    [255, 245, 245],
                ),
                TaskState::Hidden => {}
            },
        }
        self.composite_task_text(
            pixels,
            node.name,
            node.x,
            node.y + 98.0,
            26.0,
            if matches!(node.state, TaskState::Idle) {
                [145, 150, 160]
            } else {
                [210, 215, 225]
            },
            enter * exit * 0.8,
        );
    }

    fn composite_task_bubble(
        &mut self,
        pixels: &mut [u8],
        error: &str,
        center: [f32; 2],
        node_height: f32,
        opacity: f32,
        y_offset: f32,
    ) {
        let bubble_center = [center[0], center[1] - node_height * 0.5 - 58.0 + y_offset];
        let text_width = self.task_text_sprite(error, 24.0, [255, 240, 240]).advance;
        let bubble_width = (text_width + 36.0).clamp(120.0, 360.0);
        fill_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            bubble_center,
            [bubble_width, 52.0],
            [190, 35, 45, 255],
            opacity,
        );
        for y in 0..=6 {
            let half_width = 6 - y;
            for x in -half_width..=half_width {
                paint(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    bubble_center[0].round() as i32 + x,
                    (bubble_center[1] + 26.0).round() as i32 + y,
                    [190, 35, 45, 255],
                    opacity,
                );
            }
        }
        self.composite_task_text(
            pixels,
            error,
            bubble_center[0],
            bubble_center[1],
            24.0,
            [255, 240, 240],
            opacity,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn composite_task_icon(
        &mut self,
        pixels: &mut [u8],
        kind: &str,
        center: [f32; 2],
        size: f32,
        rotation: f32,
        opacity: f32,
        blur: f32,
        color: [u8; 3],
    ) {
        let canvas_width = self.spec.width;
        let canvas_height = self.spec.height;
        let sprite = self.task_icon_sprite(kind, color);
        composite_sprite_rotated(
            pixels,
            canvas_width,
            canvas_height,
            sprite,
            size,
            size,
            center[0],
            center[1],
            rotation,
            blur,
            opacity,
        );
    }

    fn task_icon_sprite(&mut self, kind: &str, color: [u8; 3]) -> &TextSprite {
        let key = format!("task-icon:{kind}:{color:?}");
        if !self.part_sprites.contains_key(&key) {
            let path = match kind {
                "error" => {
                    "M128,16C70.65,16,24,60.86,24,116c0,34.1,18.27,66,48,84.28V216a16,16,0,0,0,16,16h8a4,4,0,0,0,4-4V200.27a8.17,8.17,0,0,1,7.47-8.25,8,8,0,0,1,8.53,8v28a4,4,0,0,0,4,4h16a4,4,0,0,0,4-4V200.27a8.17,8.17,0,0,1,7.47-8.25,8,8,0,0,1,8.53,8v28a4,4,0,0,0,4,4h8a16,16,0,0,0,16-16V200.28C213.73,182,232,150.1,232,116,232,60.86,185.35,16,128,16ZM92,152a20,20,0,1,1,20-20A20,20,0,0,1,92,152Zm72,0a20,20,0,1,1,20-20A20,20,0,0,1,164,152Z"
                }
                _ => {
                    "M240,128a15.79,15.79,0,0,1-10.5,15l-63.44,23.07L143,229.5a16,16,0,0,1-30,0L89.94,166.06,26.5,143a16,16,0,0,1,0-30L89.94,89.94,113,26.5a16,16,0,0,1,30,0l23.07,63.44L229.5,113A15.79,15.79,0,0,1,240,128Z"
                }
            };
            let svg = format!(
                "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 256 256'><path fill='rgb({},{},{})' d='{}'/></svg>",
                color[0], color[1], color[2], path,
            );
            let sprite = rasterize_svg(&svg, 256, 256).expect("task icon SVG is valid");
            self.part_sprites.insert(key.clone(), (0, sprite));
        }
        &self.part_sprites[&key].1
    }

    #[allow(clippy::too_many_arguments)]
    fn composite_task_text(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        center_x: f32,
        center_y: f32,
        font_size: f32,
        color: [u8; 3],
        opacity: f32,
    ) {
        let canvas_width = self.spec.width;
        let canvas_height = self.spec.height;
        let sprite = self.task_text_sprite(text, font_size, color);
        composite_sprite(
            pixels,
            canvas_width,
            canvas_height,
            sprite,
            (center_x - sprite.advance * 0.5).round() as i32,
            (center_y - sprite.height as f32 * 0.5).round() as i32,
            opacity,
        );
    }

    fn task_text_sprite(&mut self, text: &str, font_size: f32, color: [u8; 3]) -> &TextSprite {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        font_size.to_bits().hash(&mut hasher);
        color.hash(&mut hasher);
        let key = format!("task:{:x}", hasher.finish());
        if !self.part_sprites.contains_key(&key) {
            let attrs = Attrs::new()
                .family(Family::Name("CommitMono"))
                .weight(Weight::NORMAL)
                .color(Color::rgb(color[0], color[1], color[2]));
            let height = (font_size * 1.5).ceil() as u32;
            let sprite = make_sprite(
                &mut self.font_system,
                &mut self.swash_cache,
                vec![(text, attrs.clone())],
                attrs,
                Metrics::new(font_size, height as f32),
                720,
                height,
            );
            self.part_sprites.insert(key.clone(), (0, sprite));
        }
        &self.part_sprites[&key].1
    }
}

fn draw_task_link(pixels: &mut [u8], width: u32, height: u32, link: TaskLinkFrame) {
    let dx = link.to[0] - link.from[0];
    let dy = link.to[1] - link.from[1];
    let length = dx.hypot(dy).max(1.0);
    let steps = length.ceil() as usize;
    for step in 0..=steps {
        let phase = step as f32 / steps as f32;
        let x = link.from[0] + dx * phase;
        let y = link.from[1] + dy * phase;
        for offset in -1..=1 {
            paint(
                pixels,
                width,
                height,
                x.round() as i32,
                y.round() as i32 + offset,
                [72, 112, 170, 255],
                0.14 * (1.0 - offset.unsigned_abs() as f32 * 0.35),
            );
        }
    }
    let Some(progress) = link.pulse else {
        return;
    };
    let progress = progress.clamp(0.0, 1.0);
    let center = [link.from[0] + dx * progress, link.from[1] + dy * progress];
    for y in -16..=16 {
        for x in -16..=16 {
            let distance = (x as f32).hypot(y as f32);
            let alpha = (-distance.powi(2) / (2.0 * 6.0_f32.powi(2))).exp() * 0.55;
            if alpha > 0.003 {
                paint(
                    pixels,
                    width,
                    height,
                    center[0].round() as i32 + x,
                    center[1].round() as i32 + y,
                    [125, 205, 255, 255],
                    alpha,
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
struct TaskLayerBounds {
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
}

fn blur_task_layer(
    pixels: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    center: [f32; 2],
    strength: f32,
    source: &mut Vec<[f32; 4]>,
    scratch: &mut Vec<[f32; 4]>,
) -> Option<TaskLayerBounds> {
    let radius = strength.ceil().clamp(1.0, 6.0) as usize;
    let padding = radius as i32 + 2;
    let min_x = (center[0] as i32 - 340 - padding).max(0);
    let max_x = (center[0] as i32 + 340 + padding).min(canvas_width as i32 - 1);
    let min_y = (center[1] as i32 - 260 - padding).max(0);
    let max_y = (center[1] as i32 + 220 + padding).min(canvas_height as i32 - 1);
    if min_x > max_x || min_y > max_y {
        return None;
    }
    let (min_x, max_x, min_y, max_y) = (
        min_x as usize,
        max_x as usize,
        min_y as usize,
        max_y as usize,
    );
    let width = max_x - min_x + 1;
    let height = max_y - min_y + 1;
    source.resize(width * height, [0.0; 4]);
    scratch.resize(width * height, [0.0; 4]);
    let decode = srgb_to_linear_lut();

    for y in 0..height {
        for x in 0..width {
            let canvas_index = ((min_y + y) * canvas_width as usize + min_x + x) * 4;
            let alpha = f32::from(pixels[canvas_index + 3]) / 255.0;
            source[y * width + x] = [
                decode[pixels[canvas_index] as usize] * alpha,
                decode[pixels[canvas_index + 1] as usize] * alpha,
                decode[pixels[canvas_index + 2] as usize] * alpha,
                alpha,
            ];
        }
    }

    box_blur_pass(source, scratch, width, height, radius, true);
    box_blur_pass(scratch, source, width, height, radius, false);
    let encode = linear_to_srgb_lut();

    for y in 0..height {
        for x in 0..width {
            let value = source[y * width + x];
            let canvas_index = ((min_y + y) * canvas_width as usize + min_x + x) * 4;
            if value[3] <= 0.0001 {
                pixels[canvas_index..canvas_index + 4].fill(0);
                continue;
            }
            for channel in 0..3 {
                let linear = (value[channel] / value[3]).clamp(0.0, 1.0);
                pixels[canvas_index + channel] = encode[(linear * 65535.0).round() as usize];
            }
            pixels[canvas_index + 3] = (value[3] * 255.0).round() as u8;
        }
    }
    Some(TaskLayerBounds {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn srgb_to_linear_lut() -> &'static [f32; 256] {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|value| {
            let encoded = value as f32 / 255.0;
            if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}

fn linear_to_srgb_lut() -> &'static [u8; 65536] {
    static LUT: OnceLock<[u8; 65536]> = OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|value| {
            let linear = value as f32 / 65535.0;
            let encoded = if linear <= 0.003_130_8 {
                linear * 12.92
            } else {
                1.055 * linear.powf(1.0 / 2.4) - 0.055
            };
            (encoded * 255.0).round() as u8
        })
    })
}

fn blend_pixel_linear(destination: &mut [u8], source: [u8; 4]) {
    let source_alpha = f32::from(source[3]) / 255.0;
    if source_alpha <= 0.0 {
        return;
    }
    let destination_alpha = f32::from(destination[3]) / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    let decode = srgb_to_linear_lut();
    let encode = linear_to_srgb_lut();
    for channel in 0..3 {
        let source_channel = decode[source[channel] as usize];
        let destination_channel = decode[destination[channel] as usize];
        let output = (source_channel * source_alpha
            + destination_channel * destination_alpha * (1.0 - source_alpha))
            / output_alpha;
        destination[channel] = encode[(output.clamp(0.0, 1.0) * 65535.0).round() as usize];
    }
    destination[3] = (output_alpha * 255.0).round() as u8;
}

fn box_blur_pass(
    source: &[[f32; 4]],
    destination: &mut [[f32; 4]],
    width: usize,
    height: usize,
    radius: usize,
    horizontal: bool,
) {
    let kernel = (radius * 2 + 1) as f32;
    if horizontal {
        for y in 0..height {
            let mut sum = [0.0_f32; 4];
            for x in 0..=radius.min(width - 1) {
                let sample = source[y * width + x];
                for channel in 0..4 {
                    sum[channel] += sample[channel];
                }
            }
            for x in 0..width {
                destination[y * width + x] = sum.map(|channel| channel / kernel);
                if x >= radius {
                    let sample = source[y * width + x - radius];
                    for channel in 0..4 {
                        sum[channel] -= sample[channel];
                    }
                }
                if x + radius + 1 < width {
                    let sample = source[y * width + x + radius + 1];
                    for channel in 0..4 {
                        sum[channel] += sample[channel];
                    }
                }
            }
        }
    } else {
        for x in 0..width {
            let mut sum = [0.0_f32; 4];
            for y in 0..=radius.min(height - 1) {
                let sample = source[y * width + x];
                for channel in 0..4 {
                    sum[channel] += sample[channel];
                }
            }
            for y in 0..height {
                destination[y * width + x] = sum.map(|channel| channel / kernel);
                if y >= radius {
                    let sample = source[(y - radius) * width + x];
                    for channel in 0..4 {
                        sum[channel] -= sample[channel];
                    }
                }
                if y + radius + 1 < height {
                    let sample = source[(y + radius + 1) * width + x];
                    for channel in 0..4 {
                        sum[channel] += sample[channel];
                    }
                }
            }
        }
    }
}

fn smoothstep(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn spring_progress(age: f32, response: f32, damping_ratio: f32) -> f32 {
    Spring::new(response, damping_ratio)
        .sample(MotionState::at(0.0), 1.0, age)
        .position
}

fn motion_spring_progress(age: f32, visual_duration: f32, bounce: f32) -> f32 {
    spring_progress(age, visual_duration * 1.2, 1.0 - bounce)
}

fn task_node_size(state: &TaskState, result_width: Option<f32>) -> [f32; 2] {
    match state {
        TaskState::Running => [NODE_SIZE, NODE_SIZE * 0.4],
        TaskState::Succeeded(Some(result)) => [
            result_width.unwrap_or_else(|| task_result_width(result)),
            NODE_SIZE,
        ],
        TaskState::Succeeded(None) => [NODE_SIZE, NODE_SIZE],
        TaskState::Hidden => [0.0, 0.0],
        TaskState::Idle | TaskState::Failed(_) | TaskState::Death(_) => [NODE_SIZE, NODE_SIZE],
    }
}

fn task_result_width(result: &str) -> f32 {
    ((result.chars().count() as f32 * 16.0 * 1.15 * 0.56 + 36.0).ceil() * 2.0)
        .clamp(NODE_SIZE, 520.0)
}

fn task_node_scale(state: &TaskState) -> f32 {
    match state {
        TaskState::Running => 0.95,
        TaskState::Death(_) => 0.9,
        _ => 1.0,
    }
}

fn task_state_color(state: &TaskState) -> [u8; 3] {
    match state {
        TaskState::Hidden | TaskState::Idle => [71, 85, 105],
        TaskState::Running => [59, 130, 246],
        TaskState::Succeeded(_) => [21, 128, 61],
        TaskState::Failed(_) => [239, 68, 68],
        TaskState::Death(_) => [8, 8, 9],
    }
}

fn mix_channel(from: u8, to: u8, progress: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * progress.clamp(0.0, 1.0)).round() as u8
}

fn failure_jitter(id: &str, age: f32, duration: f32) -> [f32; 3] {
    if !(0.0..duration).contains(&age) {
        return [0.0; 3];
    }
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let direction = if hasher.finish() & 1 == 0 { 1.0 } else { -1.0 };
    let envelope = (-10.0 * age).exp() * (1.0 - age / duration);
    let primary = (std::f32::consts::TAU * 11.0 * age).sin();
    let accent = (std::f32::consts::TAU * 17.0 * age).sin() * 0.18;
    let x = direction * (primary + accent) * 10.0 * envelope;
    [x, primary * 0.6 * envelope, -x * 0.008]
}

fn running_jitter(id: &str, age: f32) -> [f32; 3] {
    let age = age.max(0.0);
    let amplitudes = [3.4, 1.6, 0.055];
    let mut position = [0.0; 3];
    let mut cursor = 0.0;
    let mut segment = 0;

    loop {
        let interval = jitter_interval(id, segment);
        let elapsed = (age - cursor).clamp(0.0, interval);
        let decay = (-RUNNING_JITTER_LAMBDA * elapsed).exp();
        for axis in 0..3 {
            let target = jitter_target(id, segment, axis as u32) * amplitudes[axis];
            position[axis] = target + (position[axis] - target) * decay;
        }
        if age <= cursor + interval {
            return position;
        }
        cursor += interval;
        segment += 1;
    }
}

fn jitter_interval(id: &str, segment: u32) -> f32 {
    let unit = jitter_target(id, segment, 3) * 0.5 + 0.5;
    RUNNING_JITTER_MIN_INTERVAL + unit * (RUNNING_JITTER_MAX_INTERVAL - RUNNING_JITTER_MIN_INTERVAL)
}

fn jitter_target(id: &str, segment: u32, axis: u32) -> f32 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in id
        .bytes()
        .chain(segment.to_le_bytes())
        .chain(axis.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let unit = (hash >> 40) as f32 / ((1_u32 << 24) - 1) as f32;
    unit * 2.0 - 1.0
}

#[allow(clippy::too_many_arguments)]
fn draw_soft_rect_glow(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let (min_x, max_x, min_y, max_y) = rotated_rect_bounds(center, size, rotation, radius * 3.0);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let [local_x, local_y] = rotated_local(center, rotation, x, y);
            let distance = rounded_rect_distance(local_x, local_y, size).max(0.0);
            let alpha = (-distance * distance / (2.0 * radius * radius)).exp() * opacity;
            if alpha > 0.002 {
                paint(pixels, width, height, x, y, color, alpha);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_energy_sweep(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
    time: f32,
) {
    let (sine, cosine) = rotation.sin_cos();
    let (min_x, max_x, min_y, max_y) = rotated_rect_bounds(center, size, rotation, 1.0);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - center[0];
            let dy = y as f32 + 0.5 - center[1];
            let local_x = dx * cosine + dy * sine;
            let local_y = -dx * sine + dy * cosine;
            let coverage = rounded_rect_coverage(local_x, local_y, size);
            if coverage <= 0.0 {
                continue;
            }
            let position = local_x + size[0] * 0.5;
            let distance = energy_band_distance(position, time);
            let band = 1.0 - smoothstep(((distance - 1.0) / 39.0).clamp(0.0, 1.0));
            let alpha = band * 0.5 * coverage;
            if alpha > 0.002 {
                paint(pixels, width, height, x, y, [150, 215, 255, 255], alpha);
            }
        }
    }
}

fn energy_band_distance(position: f32, time: f32) -> f32 {
    let phase = (time * ENERGY_SPEED).rem_euclid(ENERGY_SPACING);
    ((position - phase + ENERGY_SPACING * 0.5).rem_euclid(ENERGY_SPACING) - ENERGY_SPACING * 0.5)
        .abs()
}

#[allow(clippy::too_many_arguments)]
fn draw_state_pulse(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
    age: f32,
    duration: f32,
    color: [u8; 4],
    intensity: f32,
    success: bool,
) {
    if !(0.0..duration).contains(&age) {
        return;
    }
    let progress = (age / duration).clamp(0.0, 1.0);
    let ease = if success { 1.5 } else { 4.0 };
    let eased = 1.0 - (1.0 - progress).powf(ease);
    let radius = size[0].hypot(size[1]) * 0.5 * eased;
    let softness: f32 = if success { 24.0 } else { 16.0 };
    let fade_out = if success { 0.25 } else { 0.85 };
    let fade = smoothstep((progress / 0.12).clamp(0.0, 1.0))
        * (1.0 - smoothstep(((progress - fade_out) / (1.0 - fade_out)).clamp(0.0, 1.0)));
    let (min_x, max_x, min_y, max_y) = rotated_rect_bounds(center, size, rotation, 1.0);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let [local_x, local_y] = rotated_local(center, rotation, x, y);
            let distance = local_x.hypot(local_y);
            let coverage = rounded_rect_coverage(local_x, local_y, size);
            let alpha = (-(distance - radius).powi(2) / (2.0 * softness.powi(2))).exp()
                * fade
                * intensity
                * coverage;
            if alpha > 0.003 {
                paint(pixels, width, height, x, y, color, alpha);
            }
        }
    }
}

fn fill_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    color: [u8; 4],
    opacity: f32,
) {
    let min_x = (center[0] - size[0] * 0.5).floor() as i32;
    let max_x = (center[0] + size[0] * 0.5).ceil() as i32;
    let min_y = (center[1] - size[1] * 0.5).floor() as i32;
    let max_y = (center[1] + size[1] * 0.5).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let coverage =
                rounded_rect_coverage(x as f32 + 0.5 - center[0], y as f32 + 0.5 - center[1], size);
            paint(pixels, width, height, x, y, color, opacity * coverage);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rotated_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let (min_x, max_x, min_y, max_y) = rotated_rect_bounds(center, size, rotation, 1.0);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let [local_x, local_y] = rotated_local(center, rotation, x, y);
            let coverage = rounded_rect_coverage(local_x, local_y, size);
            if coverage > 0.0 {
                paint(pixels, width, height, x, y, color, opacity * coverage);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stroke_rotated_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let (min_x, max_x, min_y, max_y) = rotated_rect_bounds(center, size, rotation, 1.0);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let [local_x, local_y] = rotated_local(center, rotation, x, y);
            let outer = rounded_rect_coverage(local_x, local_y, size);
            let inner = rounded_rect_coverage(
                local_x,
                local_y,
                [(size[0] - 2.5).max(0.0), (size[1] - 2.5).max(0.0)],
            );
            let coverage = (outer - inner).clamp(0.0, 1.0);
            if coverage > 0.0 {
                paint(pixels, width, height, x, y, color, opacity * coverage);
            }
        }
    }
}

fn rotated_local(center: [f32; 2], rotation: f32, x: i32, y: i32) -> [f32; 2] {
    let dx = x as f32 + 0.5 - center[0];
    let dy = y as f32 + 0.5 - center[1];
    let (sine, cosine) = rotation.sin_cos();
    [dx * cosine + dy * sine, -dx * sine + dy * cosine]
}

fn rotated_rect_bounds(
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
    padding: f32,
) -> (i32, i32, i32, i32) {
    let (sine, cosine) = rotation.sin_cos();
    let half_width = size[0] * 0.5 + padding;
    let half_height = size[1] * 0.5 + padding;
    let extent_x = cosine.abs() * half_width + sine.abs() * half_height;
    let extent_y = sine.abs() * half_width + cosine.abs() * half_height;
    (
        (center[0] - extent_x).floor() as i32,
        (center[0] + extent_x).ceil() as i32,
        (center[1] - extent_y).floor() as i32,
        (center[1] + extent_y).ceil() as i32,
    )
}

fn rounded_rect_coverage(local_x: f32, local_y: f32, size: [f32; 2]) -> f32 {
    (0.5 - rounded_rect_distance(local_x, local_y, size)).clamp(0.0, 1.0)
}

fn rounded_rect_distance(local_x: f32, local_y: f32, size: [f32; 2]) -> f32 {
    let radius = 8.0_f32.min(size[0].min(size[1]) * 0.5 - 1.0).max(0.0);
    let x = local_x.abs() - (size[0] * 0.5 - radius);
    let y = local_y.abs() - (size[1] * 0.5 - radius);
    x.max(0.0).hypot(y.max(0.0)) + x.max(y).min(0.0) - radius
}

fn draw_face(pixels: &mut [u8], width: u32, height: u32, center: [f32; 2], radius: f32, time: f32) {
    draw_ring(
        pixels,
        width,
        height,
        center,
        radius,
        [180, 185, 195, 255],
        0.5,
    );
    for x in [-22.0, 22.0] {
        draw_disc(
            pixels,
            width,
            height,
            [center[0] + x, center[1] - 14.0],
            4.0,
            [180, 185, 195, 255],
            0.6,
        );
    }
    let wobble = (time * 2.0).sin() * 3.0;
    for x in -24..=24 {
        let y = center[1] + 18.0 + (x as f32 / 24.0).powi(2) * -10.0 + wobble;
        paint(
            pixels,
            width,
            height,
            center[0] as i32 + x,
            y as i32,
            [180, 185, 195, 255],
            0.6,
        );
    }
}

fn draw_disc(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let radius_i = radius.ceil() as i32;
    for y in -radius_i..=radius_i {
        for x in -radius_i..=radius_i {
            if (x * x + y * y) as f32 <= radius * radius {
                paint(
                    pixels,
                    width,
                    height,
                    center[0] as i32 + x,
                    center[1] as i32 + y,
                    color,
                    opacity,
                );
            }
        }
    }
}

fn draw_ring(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let steps = (radius * std::f32::consts::TAU).max(24.0) as usize;
    for step in 0..steps {
        let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
        let x = center[0] + angle.cos() * radius;
        let y = center[1] + angle.sin() * radius;
        paint(
            pixels,
            width,
            height,
            x.round() as i32,
            y.round() as i32,
            color,
            opacity,
        );
    }
}

fn paint(pixels: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: [u8; 4], opacity: f32) {
    if !(0..width as i32).contains(&x) || !(0..height as i32).contains(&y) {
        return;
    }
    let index = (y as usize * width as usize + x as usize) * 4;
    blend_pixel(&mut pixels[index..index + 4], color, opacity);
}

#[cfg(test)]
mod tests {
    use super::{
        ENERGY_SPACING, ENERGY_SPEED, blur_task_layer, energy_band_distance, jitter_interval,
        running_jitter,
    };

    #[test]
    fn running_jitter_is_deterministic_for_out_of_order_samples() {
        let later = running_jitter("clock", 1.37);
        let earlier = running_jitter("clock", 0.41);

        assert_eq!(later, running_jitter("clock", 1.37));
        assert_eq!(earlier, running_jitter("clock", 0.41));
        assert_eq!(running_jitter("clock", 0.0), [0.0; 3]);
        assert_ne!(jitter_interval("clock", 0), jitter_interval("clock", 1));
    }

    #[test]
    fn running_energy_bands_arrive_at_equal_intervals() {
        let interval = ENERGY_SPACING / ENERGY_SPEED;
        let position = 64.0;
        let first = position / ENERGY_SPEED;

        for pulse in 0..6 {
            let time = first + pulse as f32 * interval;
            assert!(energy_band_distance(position, time) < 0.001);
        }
    }

    #[test]
    fn off_canvas_task_blur_has_no_bounds_or_allocation() {
        let mut pixels = vec![0_u8; 16 * 16 * 4];
        let mut source = Vec::new();
        let mut scratch = Vec::new();

        assert!(
            blur_task_layer(
                &mut pixels,
                16,
                16,
                [10_000.0, 10_000.0],
                4.0,
                &mut source,
                &mut scratch,
            )
            .is_none()
        );
        assert!(source.is_empty());
        assert!(scratch.is_empty());

        assert!(
            blur_task_layer(
                &mut pixels,
                16,
                16,
                [-10_000.0, -10_000.0],
                4.0,
                &mut source,
                &mut scratch,
            )
            .is_none()
        );
        assert!(source.is_empty());
        assert!(scratch.is_empty());
    }
}
