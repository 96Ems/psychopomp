use psychopomp::math::{lerp, smoothstep};

use psychopomp::task::TaskState;

use super::{
    HeadlessRenderer, TextDraw, TextSprite, blend_pixel_at, composite_sprite_rotated,
    composite_sprite_rotated_with_coverage, composite_text, rasterize_svg, text::PlainTextSpec,
};

mod content;
pub use content::{BubblePose, ContentPose, TaskContentFrame};

const NODE_SIZE: f32 = 128.0;
const ENERGY_SPEED: f32 = 500.0;
const ENERGY_SPACING: f32 = 122.0;

#[derive(Clone, Copy)]
struct TaskClip {
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
}

/// A sampled interactive pose. No edge-triggered state or frame integration is
/// hidden in this recipe; every state layer retains its own continuous presence.
pub struct TaskVisualFrame<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub scale: f32,
    pub opacity: f32,
    pub states: &'a [(&'a TaskState, f32)],
    pub contents: &'a [TaskContentFrame<'a>],
    pub activity: f32,
    pub time: f64,
}

impl HeadlessRenderer {
    pub fn composite_task_visual(&mut self, pixels: &mut [u8], node: TaskVisualFrame<'_>) {
        if node.opacity <= 0.001 {
            return;
        }
        let running = node.activity.clamp(0., 1.);
        let jitter = if running > 0.0 {
            ambient_running_jitter(node.id, node.time)
        } else {
            [0.; 3]
        };
        let center = [
            node.center[0] + jitter[0] * running,
            node.center[1] + jitter[1] * running,
        ];
        let rotation = jitter[2] * running;
        let size = [node.size[0] * node.scale, node.size[1] * node.scale];
        let clip = TaskClip {
            center,
            size,
            rotation,
        };
        let mut color = [0.; 3];
        for (state, weight) in node.states {
            let rgb = if matches!(state, TaskState::Idle | TaskState::Hidden) {
                self.theme.surface(task_state_color(state))
            } else {
                task_state_color(state)
            };
            for (channel, value) in color.iter_mut().enumerate() {
                *value += f32::from(rgb[channel]) * weight;
            }
        }
        if running > 0.001 {
            draw_soft_rect_glow(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                size,
                rotation,
                8.,
                [59, 130, 246, 255],
                running * node.opacity * 0.3,
            );
        }
        fill_rotated_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            center,
            size,
            rotation,
            [
                color[0].round() as u8,
                color[1].round() as u8,
                color[2].round() as u8,
                255,
            ],
            node.opacity,
        );
        if running > 0.001 {
            draw_energy_sweep(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                size,
                rotation,
                (node
                    .time
                    .rem_euclid(f64::from(ENERGY_SPACING / ENERGY_SPEED))) as f32,
                running * node.opacity,
            );
            stroke_rotated_rect(
                pixels,
                self.spec.width,
                self.spec.height,
                center,
                size,
                rotation,
                [130, 200, 255, 255],
                running * node.opacity * 0.6,
            );
        }
        for frame in node.contents {
            let content = frame.pose;
            let opacity = content.opacity * node.opacity;
            // Bubble opacity has its own slower spring. Do not cull it with the
            // faster icon fade when a failure is retried or skipped.
            if let Some(bubble) = frame.bubble
                && let TaskState::Failed(error) | TaskState::Death(error) = frame.state
                && !error.is_empty()
            {
                self.composite_task_bubble_content(
                    pixels,
                    error,
                    center,
                    size[1],
                    bubble,
                    node.opacity,
                );
            }
            if opacity <= 0.001 {
                continue;
            }
            let content_scale = node.scale * content.scale;
            let content_rotation = rotation + content.rotation;
            match frame.state {
                TaskState::Hidden | TaskState::Running => {}
                TaskState::Idle => self.composite_task_icon_in(
                    pixels,
                    "sparkle",
                    clip,
                    58. * content_scale,
                    content_rotation,
                    opacity,
                    content.blur,
                    [245, 245, 245],
                ),
                TaskState::Succeeded(result) => self.composite_task_text_effect(
                    pixels,
                    result.as_deref().unwrap_or("✓"),
                    clip,
                    32. * 1.15,
                    [245, 250, 247],
                    content_scale,
                    rotation,
                    content.blur,
                    opacity,
                ),
                TaskState::Failed(_) | TaskState::Death(_) => {
                    self.composite_task_icon_in(
                        pixels,
                        "error",
                        clip,
                        58. * content_scale,
                        content_rotation,
                        opacity,
                        content.blur,
                        [255, 245, 245],
                    );
                }
            }
        }
        self.composite_centered_text(
            pixels,
            node.name,
            [node.center[0], node.center[1] + 104.],
            26.,
            [200, 205, 215],
            node.opacity * 0.85,
        );
    }

    pub fn measure_task_result_width(&mut self, result: &str) -> f32 {
        let sprite = self.task_text_sprite(result, 32.0 * 1.15, [245, 250, 247]);
        (sprite.advance + 72.0).ceil().clamp(NODE_SIZE, 520.0)
    }

    fn composite_task_bubble_content(
        &mut self,
        pixels: &mut [u8],
        error: &str,
        center: [f32; 2],
        node_height: f32,
        bubble: BubblePose,
        opacity: f32,
    ) {
        let pose = bubble.content;
        if pose.opacity * opacity <= 0.001 {
            return;
        }
        let width = self.spec.width;
        let height = self.spec.height;
        let sprite = self.task_bubble_sprite(error);
        // Scale and deblur the complete bubble, including its text and tail.
        // Its body-center pivot is 28 pixels down inside the padded sprite.
        let y = center[1] - node_height * 0.5 - 58. + bubble.y;
        composite_sprite_rotated(
            pixels,
            width,
            height,
            sprite,
            sprite.width as f32 * pose.scale,
            sprite.height as f32 * pose.scale,
            center[0],
            y + (sprite.height as f32 * 0.5 - 28.) * pose.scale,
            0.,
            pose.blur,
            pose.opacity * opacity,
        );
    }

    fn task_bubble_sprite(&mut self, error: &str) -> &TextSprite {
        let key = format!("task-bubble:{error}");
        if !self.part_sprites.contains_key(&key) {
            let text = self.task_text_sprite(error, 24., [255, 240, 240]);
            let body_width = (text.advance + 36.).clamp(120., 360.);
            let width = body_width.ceil() as u32 + 4;
            let height = 64;
            let center = [width as f32 * 0.5, 28.];
            let mut pixels = vec![0; (width * height * 4) as usize];
            fill_rect(
                &mut pixels,
                width,
                height,
                center,
                [body_width, 52.],
                [190, 35, 45, 255],
                1.,
            );
            draw_bubble_tail(&mut pixels, width, height, center, 1.);
            composite_text(
                &mut pixels,
                [width, height],
                TextDraw::new(
                    text,
                    [
                        center[0] - text.advance * 0.5,
                        center[1] - text.height as f32 * 0.5,
                    ],
                ),
            );
            self.part_sprites.insert(
                key.clone(),
                (
                    0,
                    TextSprite {
                        width,
                        height,
                        advance: body_width,
                        pixels,
                    },
                ),
            );
        }
        &self.part_sprites[&key].1
    }

    #[allow(clippy::too_many_arguments)]
    fn composite_task_icon_in(
        &mut self,
        pixels: &mut [u8],
        kind: &str,
        clip: TaskClip,
        size: f32,
        rotation: f32,
        opacity: f32,
        blur: f32,
        color: [u8; 3],
    ) {
        let width = self.spec.width;
        let height = self.spec.height;
        let sprite = self.task_icon_sprite(kind, color);
        composite_sprite_rotated_with_coverage(
            pixels,
            width,
            height,
            sprite,
            size,
            size,
            clip.center[0],
            clip.center[1],
            rotation,
            blur,
            opacity,
            |x, y| task_clip_coverage(clip, x, y),
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
    fn composite_task_text_effect(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        clip: TaskClip,
        font_size: f32,
        color: [u8; 3],
        scale: f32,
        rotation: f32,
        blur: f32,
        opacity: f32,
    ) {
        let canvas_width = self.spec.width;
        let canvas_height = self.spec.height;
        let sprite = self.task_text_sprite(text, font_size, color);
        composite_sprite_rotated_with_coverage(
            pixels,
            canvas_width,
            canvas_height,
            sprite,
            sprite.width as f32 * scale,
            sprite.height as f32 * scale,
            clip.center[0],
            clip.center[1],
            rotation,
            blur,
            opacity,
            move |x, y| task_clip_coverage(clip, x, y),
        );
    }

    fn task_text_sprite(&mut self, text: &str, font_size: f32, color: [u8; 3]) -> &TextSprite {
        let color = self.theme.ink(color);
        let height = (font_size * 1.5).ceil() as u32;
        self.plain_text_sprite(
            text,
            PlainTextSpec {
                font_size,
                color,
                size: [720, height],
                semibold: false,
                crop_to_advance: true,
            },
        )
    }
}

fn draw_bubble_tail(pixels: &mut [u8], width: u32, height: u32, center: [f32; 2], opacity: f32) {
    let tail_y = center[1] + 25.5;
    for y in (tail_y - 1.).floor() as i32..=(tail_y + 7.).ceil() as i32 {
        for x in (center[0] - 7.).floor() as i32..=(center[0] + 7.).ceil() as i32 {
            let dy = y as f32 + 0.5 - tail_y;
            let dx = (x as f32 + 0.5 - center[0]).abs();
            let distance = ((dx + dy - 6.) * std::f32::consts::FRAC_1_SQRT_2)
                .max(-dy)
                .max(dy - 6.);
            blend_pixel_at(
                pixels,
                width,
                height,
                x,
                y,
                [190, 35, 45, 255],
                opacity * (0.5 - distance).clamp(0., 1.),
            );
        }
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

/// Bounded-cost C1 noise for indefinite native running states. The older authored
/// Task recipe retains its original finite-history jitter for video parity.
fn ambient_running_jitter(id: &str, time: f64) -> [f32; 3] {
    let noise = |interval: f64, axis: u32| {
        let phase = time.max(0.) / interval;
        let index = phase.floor() as u64 as u32;
        let progress = smoothstep(phase.fract() as f32);
        let a = jitter_target(id, index, axis);
        let b = jitter_target(id, index.wrapping_add(1), axis);
        lerp(a, b, progress)
    };
    std::array::from_fn(|axis| {
        (noise(0.085, axis as u32) * 0.75 + noise(0.137, axis as u32 + 4) * 0.25)
            * [3.4, 1.6, 0.055][axis]
    })
}

fn task_clip_coverage(clip: TaskClip, x: f32, y: f32) -> f32 {
    if clip.size[0] <= 0. || clip.size[1] <= 0. {
        return 0.;
    }
    let (sine, cosine) = clip.rotation.sin_cos();
    let dx = x - clip.center[0];
    let dy = y - clip.center[1];
    rounded_rect_coverage(dx * cosine + dy * sine, -dx * sine + dy * cosine, clip.size)
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
                blend_pixel_at(pixels, width, height, x, y, color, alpha);
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
    opacity: f32,
) {
    let (sine, cosine) = rotation.sin_cos();
    let phase = energy_band_phase(time);
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
            let distance = energy_band_distance_from_phase(position, phase);
            let band = 1.0 - smoothstep(((distance - 1.0) / 39.0).clamp(0.0, 1.0));
            let alpha = band * 0.5 * coverage * opacity;
            if alpha > 0.002 {
                blend_pixel_at(pixels, width, height, x, y, [150, 215, 255, 255], alpha);
            }
        }
    }
}

fn energy_band_phase(time: f32) -> f32 {
    (time * ENERGY_SPEED).rem_euclid(ENERGY_SPACING)
}

fn energy_band_distance_from_phase(position: f32, phase: f32) -> f32 {
    ((position - phase + ENERGY_SPACING * 0.5).rem_euclid(ENERGY_SPACING) - ENERGY_SPACING * 0.5)
        .abs()
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
            blend_pixel_at(pixels, width, height, x, y, color, opacity * coverage);
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
                blend_pixel_at(pixels, width, height, x, y, color, opacity * coverage);
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
                blend_pixel_at(pixels, width, height, x, y, color, opacity * coverage);
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

#[cfg(test)]
mod tests {
    #[test]
    fn ambient_jitter_is_bounded_and_deterministic_after_hours_of_playback() {
        let expected = super::ambient_running_jitter("loadUser", 21_600.125);
        for time in [
            0.0,
            0.085 - 0.00001,
            0.085 + 0.00001,
            2.,
            3600.,
            21_600.125,
            100_000.125,
        ] {
            let value = super::ambient_running_jitter("loadUser", time);
            for (value, bound) in value.into_iter().zip([3.4, 1.6, 0.055]) {
                assert!(value.is_finite() && value.abs() <= bound);
            }
        }
        assert_eq!(
            expected,
            super::ambient_running_jitter("loadUser", 21_600.125)
        );
        let a = super::ambient_running_jitter("loadUser", 0.085 - 0.00001);
        let b = super::ambient_running_jitter("loadUser", 0.085 + 0.00001);
        assert!(a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 0.001));
    }

    use super::{ENERGY_SPACING, ENERGY_SPEED, energy_band_distance_from_phase, energy_band_phase};

    #[test]
    fn running_energy_bands_arrive_at_equal_intervals() {
        let interval = ENERGY_SPACING / ENERGY_SPEED;
        let position = 64.0;
        let first = position / ENERGY_SPEED;

        for pulse in 0..6 {
            let time = first + pulse as f32 * interval;
            assert!(energy_band_distance_from_phase(position, energy_band_phase(time)) < 0.001);
        }
    }
}
