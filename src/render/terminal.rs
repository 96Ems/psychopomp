use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use anyhow::{Result, bail};
use cosmic_text::{Attrs, Color, Family, Metrics, Weight};

use super::{
    HeadlessRenderer, TextSprite, blend_pixel, composite_sprite, make_sprite,
    ui::{Bounds, Edges, VerticalFlow},
};

const COMMAND_SOURCE: &str = include_str!("../../assets/opencode-hot-reload/fire-the-missiles.md");

pub struct TerminalSceneFrame<'a> {
    pub source_pixels: &'a [u8],
    pub source_size: [u32; 2],
    pub panel_center: [f32; 2],
    pub panel_scale: f32,
    pub panel_rotation: f32,
    pub command_file: Option<CommandFileFrame>,
    pub missile_age: Option<f32>,
    pub tagline_opacity: f32,
}

#[derive(Clone, Copy)]
pub struct CommandFileFrame {
    pub enter: f32,
    pub opacity: f32,
    pub write_progress: f32,
    pub saved: bool,
}

impl HeadlessRenderer {
    pub fn render_terminal_scene(&mut self, frame: &TerminalSceneFrame<'_>) -> Result<Vec<u8>> {
        let expected = frame.source_size[0] as usize * frame.source_size[1] as usize * 4;
        if frame.source_pixels.len() != expected {
            bail!(
                "expected {expected} terminal source bytes, received {}",
                frame.source_pixels.len()
            );
        }
        if frame.panel_scale <= 0.0 {
            bail!("terminal panel scale must be positive");
        }

        if self.terminal_background_pixels.is_empty() {
            self.terminal_background_pixels =
                vec![0_u8; self.spec.width as usize * self.spec.height as usize * 4];
            draw_background(
                &mut self.terminal_background_pixels,
                self.spec.width,
                self.spec.height,
            );
        }
        let mut pixels = self.terminal_background_pixels.clone();
        if let Some(age) = frame.missile_age {
            draw_missiles(&mut pixels, self.spec.width, self.spec.height, age);
        }
        composite_terminal_panel(&mut pixels, self.spec.width, self.spec.height, frame);
        if let Some(file) = frame.command_file {
            self.composite_command_file(&mut pixels, file);
        }
        let impact_intensity = frame.missile_age.map_or(0.0, missile_impact_intensity);
        if impact_intensity > 0.001 {
            draw_impact_flash(
                &mut pixels,
                self.spec.width,
                self.spec.height,
                impact_intensity,
            );
        }
        if frame.tagline_opacity > 0.001 {
            self.composite_terminal_text(
                &mut pixels,
                "HOT RELOAD EVERYTHING.",
                [self.spec.width as f32 * 0.5, self.spec.height as f32 - 43.0],
                34.0,
                [231, 237, 242],
                frame.tagline_opacity,
                TextAlign::Center,
            );
        }
        Ok(pixels)
    }

    fn composite_command_file(&mut self, pixels: &mut [u8], frame: CommandFileFrame) {
        let enter = frame.enter.clamp(0.0, 1.0);
        let opacity = frame.opacity.clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let blur = (1.0 - enter) * 8.0;
        if blur > 0.35 {
            for (offset, weight) in [
                ([-blur, 0.0], 0.14),
                ([blur, 0.0], 0.14),
                ([0.0, -blur], 0.14),
                ([0.0, blur], 0.14),
                ([0.0, 0.0], 0.44),
            ] {
                self.composite_command_file_contents(pixels, frame, offset, opacity * weight);
            }
            return;
        }
        self.composite_command_file_contents(pixels, frame, [0.0, 0.0], opacity);
    }

    fn composite_command_file_contents(
        &mut self,
        pixels: &mut [u8],
        frame: CommandFileFrame,
        offset: [f32; 2],
        opacity: f32,
    ) {
        let enter = frame.enter.clamp(0.0, 1.0);
        let center = [
            1425.0 + (1.0 - enter) * 120.0 + offset[0],
            510.0 + (1.0 - enter) * 30.0 + offset[1],
        ];
        let card = Bounds::from_center(center, [760.0, 260.0]);
        let shadow = card.expand(9.0).translate([0.0, 14.0]);
        fill_rounded_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            shadow.center(),
            shadow.size,
            22.0,
            [0, 0, 0, 255],
            opacity * 0.3,
        );
        fill_rounded_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            card.center(),
            card.size,
            18.0,
            [8, 12, 18, 255],
            opacity,
        );
        stroke_rounded_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            card.center(),
            card.size,
            18.0,
            [71, 84, 104, 255],
            opacity * 0.5,
        );

        let (header, body) = card.split_top(54.0);
        for (x, color) in [
            (header.origin[0] + 24.0, [255, 104, 95, 255]),
            (header.origin[0] + 42.0, [255, 189, 74, 255]),
            (header.origin[0] + 60.0, [72, 199, 116, 255]),
        ] {
            fill_rounded_rect(
                pixels,
                self.spec.width,
                self.spec.height,
                [x, header.center()[1]],
                [8.0, 8.0],
                4.0,
                color,
                opacity * 0.86,
            );
        }
        self.composite_terminal_text(
            pixels,
            ".opencode/commands/fire-the-missiles.md",
            [header.origin[0] + 84.0, header.center()[1]],
            13.0,
            [213, 221, 231],
            opacity,
            TextAlign::Left,
        );
        self.composite_terminal_text(
            pixels,
            if frame.saved { "[saved]" } else { "[writing]" },
            [header.right() - 34.0, header.center()[1]],
            12.0,
            if frame.saved {
                [112, 211, 160]
            } else {
                [242, 181, 128]
            },
            opacity,
            TextAlign::Center,
        );
        fill_rounded_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            [card.center()[0], header.bottom()],
            [card.size[0] - 2.0, 1.0],
            0.0,
            [86, 99, 119, 255],
            opacity * 0.42,
        );

        let visible_source =
            reveal_text(COMMAND_SOURCE.trim_end(), frame.write_progress, 0.0, 0.96);
        let visible_lines = visible_source.split('\n').collect::<Vec<_>>();
        let current_line = visible_lines.len().saturating_sub(1);
        let editor = body.inset(Edges::symmetric(22.0, 18.0));
        let (gutter, content) = editor.split_left(38.0);
        let content = content.inset(Edges {
            left: 16.0,
            ..Edges::default()
        });
        fill_rounded_rect(
            pixels,
            self.spec.width,
            self.spec.height,
            [gutter.right(), editor.center()[1]],
            [1.0, editor.size[1]],
            0.0,
            [86, 99, 119, 255],
            opacity * 0.22,
        );

        let mut rows = VerticalFlow::new(content, 30.0);
        let mut caret = content.origin;
        for (index, text) in visible_lines.iter().enumerate() {
            let row = rows.next();
            let baseline = row.center()[1];
            self.composite_terminal_text(
                pixels,
                &(index + 1).to_string(),
                [gutter.center()[0], baseline],
                11.0,
                if index == current_line {
                    [148, 161, 179]
                } else {
                    [72, 84, 101]
                },
                opacity,
                TextAlign::Center,
            );
            let advance = self.composite_markdown_source_line(
                pixels,
                index,
                text,
                [row.origin[0], baseline],
                opacity,
            );
            if index == current_line {
                caret = [row.origin[0] + advance + 2.0, baseline];
            }
        }
        if !frame.saved {
            fill_rounded_rect(
                pixels,
                self.spec.width,
                self.spec.height,
                caret,
                [2.0, 20.0],
                0.0,
                [238, 194, 137, 255],
                opacity,
            );
        }
    }

    fn composite_markdown_source_line(
        &mut self,
        pixels: &mut [u8],
        line_index: usize,
        visible_text: &str,
        position: [f32; 2],
        opacity: f32,
    ) -> f32 {
        let segments: &[(&str, [u8; 3])] = match line_index {
            0 | 2 => &[("---", [106, 137, 175])],
            1 => &[
                ("description", [121, 192, 224]),
                (":", [132, 147, 166]),
                (" Turn up the heat", [214, 181, 129]),
            ],
            3 => &[
                ("Reply with exactly:", [207, 216, 227]),
                (" Everything is live.", [151, 210, 164]),
            ],
            _ => &[("", [207, 216, 227])],
        };
        let mut remaining = visible_text.chars().count();
        let mut x = position[0];
        for (text, color) in segments {
            if remaining == 0 {
                break;
            }
            let shown = reveal_characters(text, remaining.min(text.chars().count()));
            x += self.composite_terminal_text(
                pixels,
                shown,
                [x, position[1]],
                15.0,
                *color,
                opacity,
                TextAlign::Left,
            );
            remaining = remaining.saturating_sub(text.chars().count());
        }
        x - position[0]
    }

    #[allow(clippy::too_many_arguments)]
    fn composite_terminal_text(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        position: [f32; 2],
        font_size: f32,
        color: [u8; 3],
        opacity: f32,
        align: TextAlign,
    ) -> f32 {
        let canvas_width = self.spec.width;
        let canvas_height = self.spec.height;
        let sprite = self.terminal_text_sprite(text, font_size, color);
        let advance = sprite.advance;
        let x = match align {
            TextAlign::Left => position[0],
            TextAlign::Center => position[0] - sprite.advance * 0.5,
        };
        composite_sprite(
            pixels,
            canvas_width,
            canvas_height,
            sprite,
            x.round() as i32,
            (position[1] - sprite.height as f32 * 0.5).round() as i32,
            opacity,
        );
        advance
    }

    fn terminal_text_sprite(&mut self, text: &str, font_size: f32, color: [u8; 3]) -> &TextSprite {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        font_size.to_bits().hash(&mut hasher);
        color.hash(&mut hasher);
        let key = format!("terminal:{:x}", hasher.finish());
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
                900,
                height,
            );
            self.part_sprites.insert(key.clone(), (0, sprite));
        }
        &self.part_sprites[&key].1
    }
}

#[derive(Clone, Copy)]
enum TextAlign {
    Left,
    Center,
}

fn draw_background(pixels: &mut [u8], width: u32, height: u32) {
    for y in 0..height as usize {
        let phase = y as f32 / height as f32;
        let color = [
            (4.0 + phase * 2.0) as u8,
            (7.0 + phase * 3.0) as u8,
            (13.0 + phase * 6.0) as u8,
            255,
        ];
        for x in 0..width as usize {
            let index = (y * width as usize + x) * 4;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
}

fn composite_terminal_panel(
    pixels: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    frame: &TerminalSceneFrame<'_>,
) {
    const PANEL_SIZE: [f32; 2] = [1400.0, 840.0];
    let missile_age = frame.missile_age.unwrap_or(-1.0);
    let impact_intensity = missile_impact_intensity(missile_age);
    let impact_age = (missile_age - 0.92).max(0.0);
    let center = [
        frame.panel_center[0] + impact_intensity * (impact_age * 91.0).sin() * 11.0,
        frame.panel_center[1] + impact_intensity * (impact_age * 73.0).sin() * 7.0,
    ];
    let size = [
        PANEL_SIZE[0] * frame.panel_scale,
        PANEL_SIZE[1] * frame.panel_scale,
    ];
    let padding = 64.0;
    let (min_x, max_x, min_y, max_y) =
        rotated_rect_bounds(center, size, frame.panel_rotation, padding);
    let sine = frame.panel_rotation.sin();
    let cosine = frame.panel_rotation.cos();
    for y in min_y.max(0)..=max_y.min(canvas_height as i32 - 1) {
        for x in min_x.max(0)..=max_x.min(canvas_width as i32 - 1) {
            let dx = x as f32 + 0.5 - center[0];
            let dy = y as f32 + 0.5 - center[1];
            let local_x = dx * cosine + dy * sine;
            let local_y = -dx * sine + dy * cosine;
            let distance = rounded_rect_distance(local_x, local_y, size, 30.0);
            let target = (y as usize * canvas_width as usize + x as usize) * 4;
            if distance > 0.0 {
                if distance < padding {
                    let shadow = (-distance * distance / (2.0 * 22.0_f32.powi(2))).exp() * 0.55;
                    blend_pixel(&mut pixels[target..target + 4], [0, 0, 0, 255], shadow);
                }
                continue;
            }
            let u = local_x / size[0] + 0.5;
            let v = local_y / size[1] + 0.5;
            let source_width = frame.source_size[0] as f32;
            let source_height = frame.source_size[1] as f32;
            let source_x = source_width * 0.5 + (u - 0.5) * source_width - 0.5;
            let source_y = source_height * 0.5 + (v - 0.5) * source_height - 0.5;
            let source = sample_terminal_source(
                frame.source_pixels,
                frame.source_size[0],
                frame.source_size[1],
                source_x,
                source_y,
                impact_intensity * 3.2,
            );
            let edge = (-distance).clamp(0.0, 1.0);
            blend_pixel(&mut pixels[target..target + 4], source, edge);
        }
    }
}

fn sample_terminal_source(
    pixels: &[u8],
    width: u32,
    height: u32,
    x: f32,
    y: f32,
    blur: f32,
) -> [u8; 4] {
    if blur < 0.1 {
        return sample_bilinear(pixels, width, height, x, y);
    }
    let offsets = [
        [0.0, 0.0],
        [-blur, 0.0],
        [blur, 0.0],
        [0.0, -blur],
        [0.0, blur],
    ];
    let mut channels = [0_u32; 4];
    for offset in offsets {
        let sample = sample_bilinear(pixels, width, height, x + offset[0], y + offset[1]);
        for (sum, value) in channels.iter_mut().zip(sample) {
            *sum += u32::from(value);
        }
    }
    channels.map(|value| (value / offsets.len() as u32) as u8)
}

fn sample_bilinear(pixels: &[u8], width: u32, height: u32, x: f32, y: f32) -> [u8; 4] {
    let x = x.clamp(0.0, width as f32 - 1.0);
    let y = y.clamp(0.0, height as f32 - 1.0);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width as usize - 1);
    let y1 = (y0 + 1).min(height as usize - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let pixel =
        |x: usize, y: usize, channel: usize| pixels[(y * width as usize + x) * 4 + channel] as f32;
    std::array::from_fn(|channel| {
        let top = pixel(x0, y0, channel) + (pixel(x1, y0, channel) - pixel(x0, y0, channel)) * tx;
        let bottom =
            pixel(x0, y1, channel) + (pixel(x1, y1, channel) - pixel(x0, y1, channel)) * tx;
        (top + (bottom - top) * ty).round() as u8
    })
}

fn draw_missiles(pixels: &mut [u8], width: u32, height: u32, age: f32) {
    draw_missile(
        pixels,
        width,
        height,
        age,
        0.0,
        [225.0, 930.0],
        [95.0, 535.0],
        [165.0, 145.0],
    );
    draw_missile(
        pixels,
        width,
        height,
        age,
        0.12,
        [1695.0, 930.0],
        [1825.0, 515.0],
        [1760.0, 160.0],
    );
}

fn missile_impact_intensity(age: f32) -> f32 {
    let pulse =
        |at: f32| smoothstep((age - at) / 0.045) * (1.0 - smoothstep((age - at - 0.045) / 0.3));
    pulse(0.92).max(pulse(1.04))
}

#[allow(clippy::too_many_arguments)]
fn draw_missile(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    age: f32,
    delay: f32,
    start: [f32; 2],
    control: [f32; 2],
    end: [f32; 2],
) {
    const FLIGHT_DURATION: f32 = 0.92;
    const IMPACT_DURATION: f32 = 0.34;
    let local_age = age - delay;
    if local_age <= 0.0 {
        return;
    }
    let raw = (local_age / FLIGHT_DURATION).clamp(0.0, 1.0);
    let progress = smoothstep(raw);
    let impact_age = local_age - FLIGHT_DURATION;
    let impact_fade = if impact_age <= 0.0 {
        1.0
    } else {
        1.0 - smoothstep(impact_age / IMPACT_DURATION)
    };
    let trail_start = (progress - 0.42).max(0.0);
    for index in 0..38 {
        let phase = index as f32 / 37.0;
        let t = trail_start + (progress - trail_start) * phase;
        let point = quadratic(start, control, end, t);
        let alpha = phase.powf(2.2) * (1.0 - raw).mul_add(0.35, 0.65) * impact_fade;
        draw_glow(
            pixels,
            width,
            height,
            point,
            3.0 + phase * 5.0,
            [255, 149, 82, 255],
            alpha * 0.22,
        );
    }
    if local_age < FLIGHT_DURATION {
        let tip = quadratic(start, control, end, progress);
        let previous = quadratic(start, control, end, (progress - 0.018).max(0.0));
        draw_line(
            pixels,
            width,
            height,
            previous,
            tip,
            4.5,
            [255, 244, 215, 255],
            0.95,
        );
        draw_glow(pixels, width, height, tip, 14.0, [255, 177, 98, 255], 0.5);
    } else if impact_age < IMPACT_DURATION {
        let impact = (impact_age / IMPACT_DURATION).clamp(0.0, 1.0);
        let fade = 1.0 - smoothstep(impact);
        draw_ring(
            pixels,
            width,
            height,
            end,
            16.0 + impact * 116.0,
            [255, 172, 98, 255],
            fade * 0.72,
        );
        if impact > 0.16 {
            let secondary = ((impact - 0.16) / 0.84).clamp(0.0, 1.0);
            draw_ring(
                pixels,
                width,
                height,
                end,
                8.0 + secondary * 82.0,
                [255, 224, 178, 255],
                (1.0 - secondary) * 0.45,
            );
        }
        draw_glow(
            pixels,
            width,
            height,
            end,
            30.0 + impact * 42.0,
            [255, 200, 132, 255],
            fade * 0.82,
        );
        for index in 0..14 {
            let angle = index as f32 / 14.0 * std::f32::consts::TAU + delay * 3.0;
            let direction = [angle.cos(), angle.sin()];
            let distance = 18.0 + impact * (68.0 + (index % 4) as f32 * 13.0);
            draw_line(
                pixels,
                width,
                height,
                [
                    end[0] + direction[0] * distance * 0.45,
                    end[1] + direction[1] * distance * 0.45,
                ],
                [
                    end[0] + direction[0] * distance,
                    end[1] + direction[1] * distance,
                ],
                1.4,
                [255, 185, 111, 255],
                fade * 0.6,
            );
        }
        for index in 0..5 {
            let angle = -2.8 + index as f32 * 0.54 + delay;
            let drift = impact * (28.0 + index as f32 * 9.0);
            draw_glow(
                pixels,
                width,
                height,
                [
                    end[0] + angle.cos() * drift,
                    end[1] + angle.sin() * drift - impact * 16.0,
                ],
                12.0 + impact * 10.0,
                [92, 86, 82, 255],
                fade * 0.13,
            );
        }
    }
}

fn draw_impact_flash(pixels: &mut [u8], width: u32, height: u32, intensity: f32) {
    let opacity = intensity.clamp(0.0, 1.0) * 0.055;
    for pixel in pixels.chunks_exact_mut(4) {
        blend_pixel(pixel, [255, 170, 105, 255], opacity);
    }
    draw_glow(
        pixels,
        width,
        height,
        [165.0, 145.0],
        92.0,
        [255, 181, 112, 255],
        intensity * 0.24,
    );
    draw_glow(
        pixels,
        width,
        height,
        [1760.0, 160.0],
        92.0,
        [255, 181, 112, 255],
        intensity * 0.24,
    );
}

fn quadratic(start: [f32; 2], control: [f32; 2], end: [f32; 2], t: f32) -> [f32; 2] {
    let one_minus = 1.0 - t;
    [
        one_minus * one_minus * start[0] + 2.0 * one_minus * t * control[0] + t * t * end[0],
        one_minus * one_minus * start[1] + 2.0 * one_minus * t * control[1] + t * t * end[1],
    ]
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    from: [f32; 2],
    to: [f32; 2],
    thickness: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let length = (to[0] - from[0]).hypot(to[1] - from[1]).max(1.0);
    let steps = length.ceil() as usize;
    for step in 0..=steps {
        let phase = step as f32 / steps as f32;
        let point = [
            from[0] + (to[0] - from[0]) * phase,
            from[1] + (to[1] - from[1]) * phase,
        ];
        draw_glow(pixels, width, height, point, thickness, color, opacity);
    }
}

fn draw_glow(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let extent = (radius * 2.5).ceil() as i32;
    for y in -extent..=extent {
        for x in -extent..=extent {
            let distance = (x as f32).hypot(y as f32);
            let alpha = (-distance * distance / (2.0 * radius * radius)).exp() * opacity;
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

fn draw_ring(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let extent = (radius + 3.0).ceil() as i32;
    for y in -extent..=extent {
        for x in -extent..=extent {
            let distance = (x as f32).hypot(y as f32);
            let alpha = (1.5 - (distance - radius).abs()).clamp(0.0, 1.0) * opacity;
            if alpha > 0.0 {
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

#[allow(clippy::too_many_arguments)]
fn fill_rounded_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let min_x = (center[0] - size[0] * 0.5 - 1.0).floor() as i32;
    let max_x = (center[0] + size[0] * 0.5 + 1.0).ceil() as i32;
    let min_y = (center[1] - size[1] * 0.5 - 1.0).floor() as i32;
    let max_y = (center[1] + size[1] * 0.5 + 1.0).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let local_x = x as f32 + 0.5 - center[0];
            let local_y = y as f32 + 0.5 - center[1];
            let coverage =
                (0.5 - rounded_rect_distance(local_x, local_y, size, radius)).clamp(0.0, 1.0);
            if coverage > 0.0 {
                paint(pixels, width, height, x, y, color, opacity * coverage);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stroke_rounded_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center: [f32; 2],
    size: [f32; 2],
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let min_x = (center[0] - size[0] * 0.5 - 2.0).floor() as i32;
    let max_x = (center[0] + size[0] * 0.5 + 2.0).ceil() as i32;
    let min_y = (center[1] - size[1] * 0.5 - 2.0).floor() as i32;
    let max_y = (center[1] + size[1] * 0.5 + 2.0).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let local_x = x as f32 + 0.5 - center[0];
            let local_y = y as f32 + 0.5 - center[1];
            let distance = rounded_rect_distance(local_x, local_y, size, radius);
            let coverage = (1.25 - distance.abs()).clamp(0.0, 1.0);
            if coverage > 0.0 {
                paint(pixels, width, height, x, y, color, opacity * coverage);
            }
        }
    }
}

fn rounded_rect_distance(local_x: f32, local_y: f32, size: [f32; 2], radius: f32) -> f32 {
    let radius = radius.min(size[0].min(size[1]) * 0.5);
    let dx = local_x.abs() - (size[0] * 0.5 - radius);
    let dy = local_y.abs() - (size[1] * 0.5 - radius);
    dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - radius
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

fn paint(pixels: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: [u8; 4], opacity: f32) {
    if !(0..width as i32).contains(&x) || !(0..height as i32).contains(&y) {
        return;
    }
    let index = (y as usize * width as usize + x as usize) * 4;
    blend_pixel(&mut pixels[index..index + 4], color, opacity);
}

fn reveal_text(text: &str, progress: f32, start: f32, end: f32) -> &str {
    let phase = ((progress - start) / (end - start)).clamp(0.0, 1.0);
    let characters = (text.chars().count() as f32 * phase).floor() as usize;
    reveal_characters(text, characters)
}

fn reveal_characters(text: &str, characters: usize) -> &str {
    let byte = text
        .char_indices()
        .nth(characters)
        .map_or(text.len(), |(index, _)| index);
    &text[..byte]
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::{COMMAND_SOURCE, reveal_text, sample_bilinear};

    #[test]
    fn terminal_video_sampling_interpolates_rgba_channels() {
        let pixels = [
            0, 0, 0, 255, 100, 0, 0, 255, 0, 100, 0, 255, 100, 100, 0, 255,
        ];

        assert_eq!(sample_bilinear(&pixels, 2, 2, 0.5, 0.5), [50, 50, 0, 255]);
        assert_eq!(sample_bilinear(&pixels, 2, 2, -1.0, 3.0), [0, 100, 0, 255]);
    }

    #[test]
    fn command_source_reveal_preserves_line_breaks() {
        assert_eq!(reveal_text("one\n\ntwo", 0.5, 0.0, 1.0), "one\n");
        assert_eq!(reveal_text("one\n\ntwo", 1.0, 0.0, 1.0), "one\n\ntwo");

        let source = COMMAND_SOURCE.trim_end();
        assert_eq!(reveal_text(source, 1.0, 0.0, 0.96), source);
        assert_eq!(source.lines().count(), 4);
        assert!(source.lines().nth(1).unwrap().starts_with("description:"));
        assert_eq!(
            source.lines().nth(3),
            Some("Reply with exactly: Everything is live.")
        );
    }
}
