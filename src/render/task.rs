use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
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
    pub nodes: &'a [TaskFrame<'a>],
}

impl HeadlessRenderer {
    pub fn render_task_scene(&mut self, frame: &TaskSceneFrame<'_>) -> Result<Vec<u8>> {
        let mut pixels = vec![0_u8; self.spec.width as usize * self.spec.height as usize * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[1, 2, 4, 255]);
        }
        if let Some(quote) = frame.quote {
            self.composite_quote(&mut pixels, quote);
        }
        for node in frame.nodes {
            self.composite_task_node(&mut pixels, *node);
        }
        Ok(pixels)
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
        let scale_transition = motion_spring_progress(node.state_age, 3.0 / 18.0, 0.0);
        let previous_size = task_node_size(node.previous_state);
        let target_size = task_node_size(node.state);
        let mut width = previous_size[0] + (target_size[0] - previous_size[0]) * width_transition;
        let mut height = previous_size[1] + (target_size[1] - previous_size[1]) * height_transition;
        let previous_scale = task_node_scale(node.previous_state);
        let target_scale = task_node_scale(node.state);
        let scale = enter * (previous_scale + (target_scale - previous_scale) * scale_transition);
        let mut offset = [0.0, 0.0];
        let mut rotation = 0.0;
        let target_color = match node.state {
            TaskState::Hidden => return,
            TaskState::Idle => [71, 85, 105],
            TaskState::Running => {
                offset = [
                    (node.state_age * 37.0).sin() * 1.8,
                    (node.state_age * 29.0).sin() * 0.8,
                ];
                rotation = (node.state_age * 31.0).sin() * 0.04;
                [59, 130, 246]
            }
            TaskState::Succeeded(_) => [21, 128, 61],
            TaskState::Failed(_) | TaskState::Death(_) => {
                let duration = if matches!(node.state, TaskState::Death(_)) {
                    0.7
                } else {
                    0.42
                };
                let jitter = failure_jitter(node.id.as_str(), node.state_age, duration);
                offset = [jitter[0], jitter[1]];
                rotation = jitter[2];
                if matches!(node.state, TaskState::Death(_)) {
                    [8, 8, 9]
                } else {
                    [239, 68, 68]
                }
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
            TaskState::Succeeded(_) => (1.0, 0.6, [55, 163, 95]),
            TaskState::Failed(_) => (0.6, 0.5, [244, 92, 92]),
            TaskState::Death(_) => (0.6, 0.5, [255, 45, 45]),
            TaskState::Running => (1.0, 0.2, [92, 158, 248]),
            TaskState::Idle | TaskState::Hidden => (1.0, 0.2, [100, 112, 130]),
        };
        let remaining = (1.0 - node.state_age / flash_duration).clamp(0.0, 1.0);
        let flash = (remaining * std::f32::consts::FRAC_PI_2).sin() * flash_mix;
        let color = [
            mix_channel(color[0], flash_color[0], flash),
            mix_channel(color[1], flash_color[1], flash),
            mix_channel(color[2], flash_color[2], flash),
        ];
        let center = [node.x + offset[0], node.y + offset[1]];
        width *= scale;
        height *= scale;

        if matches!(node.state, TaskState::Running) {
            let glow = 0.2 + (0.5 + 0.5 * (node.state_age / 0.18).sin()) * 0.15;
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
                0.4 + 0.6 * (0.5 + 0.5 * (node.state_age / 0.22).sin()),
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
            TaskState::Hidden => {}
            TaskState::Idle => self.composite_task_icon(
                pixels,
                "sparkle",
                center,
                58.0 * enter,
                0.0,
                enter,
                [245, 245, 245],
            ),
            TaskState::Running => {}
            TaskState::Succeeded(_) => {
                let content = spring_progress(node.state_age, 0.25, 0.68);
                if let Some(result) = result {
                    self.composite_task_text(
                        pixels,
                        result,
                        center[0],
                        center[1],
                        32.0 * (0.6 + content * 0.4),
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
                draw_state_pulse(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    center,
                    [width, height],
                    node.state_age,
                    1.0,
                    [120, 255, 170, 255],
                    0.12,
                    true,
                );
            }
            TaskState::Failed(_) | TaskState::Death(_) => {
                let icon = spring_progress(node.state_age, 0.25, 0.72);
                self.composite_task_icon(
                    pixels,
                    "error",
                    center,
                    58.0 * icon,
                    rotation,
                    icon.clamp(0.0, 1.0),
                    if matches!(node.state, TaskState::Death(_)) {
                        [255, 45, 45]
                    } else {
                        [255, 245, 245]
                    },
                );
                draw_state_pulse(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    center,
                    [width, height],
                    node.state_age,
                    0.55,
                    if matches!(node.state, TaskState::Death(_)) {
                        [255, 45, 45, 255]
                    } else {
                        [255, 110, 110, 255]
                    },
                    0.9,
                    false,
                );
                if node.state_age < 3.0
                    && let Some(error) = error
                {
                    let rise = spring_progress(node.state_age, 0.25, 0.65);
                    let bubble_center = [
                        center[0],
                        center[1] - height * 0.5 - 58.0 + (1.0 - rise) * 32.0,
                    ];
                    let bubble_width = (error.len() as f32 * 17.0 + 36.0).clamp(120.0, 360.0);
                    fill_rect(
                        pixels,
                        self.spec.width,
                        self.spec.height,
                        bubble_center,
                        [bubble_width, 52.0],
                        [190, 35, 45, 255],
                        rise.clamp(0.0, 1.0),
                    );
                    self.composite_task_text(
                        pixels,
                        error,
                        bubble_center[0],
                        bubble_center[1],
                        24.0,
                        [255, 240, 240],
                        rise.clamp(0.0, 1.0),
                    );
                }
            }
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
            enter * 0.8,
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
            (1.0 - opacity) * 8.0,
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

fn task_node_size(state: &TaskState) -> [f32; 2] {
    match state {
        TaskState::Running => [NODE_SIZE, NODE_SIZE * 0.4],
        TaskState::Succeeded(Some(result)) => [
            (result.chars().count() as f32 * 32.0 * 0.56 + 72.0).clamp(NODE_SIZE, 520.0),
            NODE_SIZE,
        ],
        TaskState::Succeeded(None) => [NODE_SIZE, NODE_SIZE],
        TaskState::Hidden => [0.0, 0.0],
        TaskState::Idle | TaskState::Failed(_) | TaskState::Death(_) => [NODE_SIZE, NODE_SIZE],
    }
}

fn task_node_scale(state: &TaskState) -> f32 {
    if matches!(state, TaskState::Running) {
        0.95
    } else {
        1.0
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
    let interval = 0.045;
    let sample = age / interval;
    let step = sample.floor() as u32;
    let blend = smoothstep(sample.fract());
    let envelope = (1.0 - age / duration).powi(2);
    let channel = |index, amplitude| {
        let from = jitter_noise(id, step, index);
        let to = jitter_noise(id, step + 1, index);
        (from + (to - from) * blend) * amplitude * envelope
    };
    [channel(0, 10.0), channel(1, 7.0), channel(2, 0.12)]
}

fn jitter_noise(id: &str, step: u32, channel: u32) -> f32 {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    step.hash(&mut hasher);
    channel.hash(&mut hasher);
    let value = hasher.finish() as u32;
    value as f32 / u32::MAX as f32 * 2.0 - 1.0
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
    let extent_x = size[0] * 0.5 + radius * 3.0;
    let extent_y = size[1] * 0.5 + radius * 3.0;
    let extent = extent_x.hypot(extent_y);
    let min_x = (center[0] - extent).floor() as i32;
    let max_x = (center[0] + extent).ceil() as i32;
    let min_y = (center[1] - extent).floor() as i32;
    let max_y = (center[1] + extent).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let [local_x, local_y] = rotated_local(center, rotation, x, y);
            let dx = (local_x.abs() - size[0] * 0.5).max(0.0);
            let dy = (local_y.abs() - size[1] * 0.5).max(0.0);
            let distance = dx.hypot(dy);
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
    let travel = size[0] + 40.0 * 2.0 + 122.0 * 2.0;
    let start = (time * 500.0) % travel - 40.0;
    let radius = size[0].hypot(size[1]).ceil() as i32;
    let min_x = (center[0] - radius as f32).floor() as i32;
    let max_x = (center[0] + radius as f32).ceil() as i32;
    let min_y = (center[1] - radius as f32).floor() as i32;
    let max_y = (center[1] + radius as f32).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - center[0];
            let dy = y as f32 + 0.5 - center[1];
            let local_x = dx * cosine + dy * sine;
            let local_y = -dx * sine + dy * cosine;
            let coverage = rect_coverage(local_x, local_y, size);
            if coverage <= 0.0 {
                continue;
            }
            let position = local_x + size[0] * 0.5;
            let mut band = 0.0_f32;
            for index in 0..3 {
                let distance = (position - (start - index as f32 * 122.0)).abs();
                band = band.max(1.0 - smoothstep(((distance - 1.0) / 39.0).clamp(0.0, 1.0)));
            }
            let pulse = 0.85 + 0.15 * (time * 7.6).sin();
            let alpha = band * pulse * 0.5 * coverage;
            if alpha > 0.002 {
                paint(pixels, width, height, x, y, [150, 215, 255, 255], alpha);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_state_pulse(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
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
    let half_width = (size[0] * 0.5).ceil() as i32;
    let half_height = (size[1] * 0.5).ceil() as i32;
    for y in -half_height..=half_height {
        for x in -half_width..=half_width {
            let distance = (x as f32).hypot(y as f32);
            let alpha =
                (-(distance - radius).powi(2) / (2.0 * softness.powi(2))).exp() * fade * intensity;
            if alpha > 0.003 {
                paint(
                    pixels,
                    width,
                    height,
                    center[0].round() as i32 + x,
                    center[1].round() as i32 + y,
                    color,
                    alpha,
                );
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
            paint(pixels, width, height, x, y, color, opacity);
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
    let radius = size[0].hypot(size[1]) * 0.5 + 1.0;
    let min_x = (center[0] - radius).floor() as i32;
    let max_x = (center[0] + radius).ceil() as i32;
    let min_y = (center[1] - radius).floor() as i32;
    let max_y = (center[1] + radius).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let [local_x, local_y] = rotated_local(center, rotation, x, y);
            let coverage = rect_coverage(local_x, local_y, size);
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
    let radius = size[0].hypot(size[1]) * 0.5 + 1.0;
    let min_x = (center[0] - radius).floor() as i32;
    let max_x = (center[0] + radius).ceil() as i32;
    let min_y = (center[1] - radius).floor() as i32;
    let max_y = (center[1] + radius).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let [local_x, local_y] = rotated_local(center, rotation, x, y);
            let edge_x = size[0] * 0.5 - local_x.abs();
            let edge_y = size[1] * 0.5 - local_y.abs();
            let coverage =
                rect_coverage(local_x, local_y, size) * (1.75 - edge_x.min(edge_y)).clamp(0.0, 1.0);
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

fn rect_coverage(local_x: f32, local_y: f32, size: [f32; 2]) -> f32 {
    let edge = (size[0] * 0.5 - local_x.abs()).min(size[1] * 0.5 - local_y.abs());
    (edge + 0.5).clamp(0.0, 1.0)
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
