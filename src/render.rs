use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::mpsc,
};

use anyhow::{Context, Result, bail};
use bytemuck::{Pod, Zeroable};
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap,
};
use wgpu::util::DeviceExt;

use crate::code::{CodeLine, LineId, PlacedLine, SyntaxStyle};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const BYTES_PER_PIXEL: u32 = 4;
const COPY_ROW_ALIGNMENT: u32 = 256;
const LINE_HEIGHT: f32 = 44.0;

#[derive(Clone)]
pub struct RenderSpec {
    pub width: u32,
    pub height: u32,
    pub font_path: PathBuf,
    pub file_name: String,
}

pub struct EditorFrame<'a> {
    pub panel_offset_y: f32,
    pub focus_intensity: f32,
    pub focus_line_y: f32,
    pub token_highlight: TokenHighlight,
    pub pointer: PointerFrame,
    pub lines: &'a [PlacedLine<'a>],
}

#[derive(Clone, Copy)]
pub struct TokenHighlight {
    pub x: f32,
    pub width: f32,
    pub opacity: f32,
}

#[derive(Clone, Copy)]
pub struct PointerFrame {
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TextRangeBounds {
    pub x: f32,
    pub width: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneUniforms {
    resolution: [f32; 2],
    panel_offset_y: f32,
    _padding_0: f32,
    focus: [f32; 2],
    _padding_1: [f32; 2],
    token_highlight: [f32; 4],
    pointer: [f32; 4],
}

struct TextSprite {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

pub struct HeadlessRenderer {
    spec: RenderSpec,
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded_bytes_per_row: u32,
    scene_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    scene_bind_group: wgpu::BindGroup,
    font_system: FontSystem,
    swash_cache: SwashCache,
    title_sprite: TextSprite,
    line_sprites: HashMap<LineId, (u64, TextSprite)>,
}

impl HeadlessRenderer {
    pub async fn new(spec: RenderSpec) -> Result<Self> {
        if spec.width == 0 || spec.height == 0 {
            bail!("render dimensions must be non-zero");
        }

        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .context("request a headless GPU adapter")?;
        let info = adapter.get_info();
        println!("GPU: {} ({:?})", info.name, info.backend);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("kinograph prototype device"),
                ..Default::default()
            })
            .await
            .context("request a wgpu device")?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output frame"),
            size: wgpu::Extent3d {
                width: spec.width,
                height: spec.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let unpadded_bytes_per_row = spec.width * BYTES_PER_PIXEL;
        let padded_bytes_per_row =
            unpadded_bytes_per_row.div_ceil(COPY_ROW_ALIGNMENT) * COPY_ROW_ALIGNMENT;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame readback"),
            size: u64::from(padded_bytes_per_row) * u64::from(spec.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("scene.wgsl"));
        let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let initial_uniforms = SceneUniforms {
            resolution: [spec.width as f32, spec.height as f32],
            panel_offset_y: 0.0,
            _padding_0: 0.0,
            focus: [0.0, 0.0],
            _padding_1: [0.0; 2],
            token_highlight: [0.0; 4],
            pointer: [0.0; 4],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene uniforms"),
            contents: bytemuck::bytes_of(&initial_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bind group"),
            layout: &scene_pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let mut font_system = FontSystem::new();
        if let Err(error) = font_system.db_mut().load_font_file(&spec.font_path) {
            eprintln!(
                "Could not load {}: {error}; using the system monospace font",
                spec.font_path.display()
            );
        }
        let mut swash_cache = SwashCache::new();
        let title_sprite = make_title_sprite(&mut font_system, &mut swash_cache, &spec.file_name);

        Ok(Self {
            spec,
            device,
            queue,
            texture,
            view,
            readback,
            padded_bytes_per_row,
            scene_pipeline,
            uniform_buffer,
            scene_bind_group,
            font_system,
            swash_cache,
            title_sprite,
            line_sprites: HashMap::new(),
        })
    }

    pub fn render_shapes(&mut self, frame: &EditorFrame<'_>) -> Result<Vec<u8>> {
        let panel_center_y = self.spec.height as f32 * 0.52 + frame.panel_offset_y;
        let panel_top = self.spec.height as f32 * 0.17 + frame.panel_offset_y;
        let code_top = panel_top + 104.0;
        let focus_offset_y = code_top + frame.focus_line_y + LINE_HEIGHT * 0.5 - panel_center_y;
        let uniforms = SceneUniforms {
            resolution: [self.spec.width as f32, self.spec.height as f32],
            panel_offset_y: frame.panel_offset_y,
            _padding_0: 0.0,
            focus: [frame.focus_intensity, focus_offset_y],
            _padding_1: [0.0; 2],
            token_highlight: [
                frame.token_highlight.x,
                frame.focus_line_y,
                frame.token_highlight.width,
                frame.token_highlight.opacity,
            ],
            pointer: [frame.pointer.x, frame.pointer.y, frame.pointer.opacity, 0.0],
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render scene geometry"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene geometry pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.scene_pipeline);
            pass.set_bind_group(0, &self.scene_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.spec.height),
                },
            },
            wgpu::Extent3d {
                width: self.spec.width,
                height: self.spec.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let slice = self.readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .context("wait for GPU frame")?;
        receiver
            .recv()
            .context("receive GPU readback result")?
            .context("map GPU readback buffer")?;

        let unpadded_bytes_per_row = self.spec.width as usize * BYTES_PER_PIXEL as usize;
        let bytes = slice.get_mapped_range().context("read mapped GPU frame")?;
        let mut frame_bytes =
            Vec::with_capacity(unpadded_bytes_per_row * self.spec.height as usize);
        for row in bytes
            .chunks_exact(self.padded_bytes_per_row as usize)
            .take(self.spec.height as usize)
        {
            frame_bytes.extend_from_slice(&row[..unpadded_bytes_per_row]);
        }
        drop(bytes);
        self.readback.unmap();
        Ok(frame_bytes)
    }

    pub fn composite_text(&mut self, pixels: &mut [u8], frame: &EditorFrame<'_>) -> Result<()> {
        let expected = self.spec.width as usize * self.spec.height as usize * 4;
        if pixels.len() != expected {
            bail!("expected {expected} frame bytes, received {}", pixels.len());
        }

        for placed in frame.lines {
            let fingerprint = line_fingerprint(placed.line);
            let is_stale = self
                .line_sprites
                .get(&placed.line.id)
                .is_none_or(|(cached, _)| *cached != fingerprint);
            if is_stale {
                let sprite =
                    make_line_sprite(&mut self.font_system, &mut self.swash_cache, placed.line);
                self.line_sprites
                    .insert(placed.line.id.clone(), (fingerprint, sprite));
            }
        }

        let panel_top = self.spec.height as f32 * 0.17 + frame.panel_offset_y;
        composite_sprite(
            pixels,
            self.spec.width,
            self.spec.height,
            &self.title_sprite,
            (self.spec.width as f32 * 0.135).round() as i32,
            (panel_top + 16.0).round() as i32,
            1.0,
        );
        let code_top = panel_top + 104.0;
        for placed in frame.lines {
            if placed.opacity <= 0.001 {
                continue;
            }
            let sprite = self
                .line_sprites
                .get(&placed.line.id)
                .map(|(_, sprite)| sprite)
                .expect("line sprite was populated above");
            composite_sprite(
                pixels,
                self.spec.width,
                self.spec.height,
                sprite,
                (self.spec.width as f32 * 0.145 + placed.x).round() as i32,
                (code_top + placed.y).round() as i32,
                placed.opacity,
            );
        }
        Ok(())
    }

    pub fn measure_text_range(&mut self, line: &CodeLine, text: &str) -> Result<TextRangeBounds> {
        let full_text: String = line.spans.iter().map(|span| span.text.as_str()).collect();
        let start = full_text
            .find(text)
            .with_context(|| format!("line '{}' does not contain '{text}'", line.id.as_str()))?;
        let end = start + text.len();
        let base = Attrs::new().family(Family::Name("CommitMono"));
        let spans: Vec<_> = line
            .spans
            .iter()
            .map(|span| (span.text.as_str(), attributes(base.clone(), span.style)))
            .collect();
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(28.0, LINE_HEIGHT));
        buffer.set_size(Some(1320.0), Some(LINE_HEIGHT));
        buffer.set_wrap(Wrap::None);
        buffer.set_rich_text(spans, &base, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);
        let run = buffer
            .layout_runs()
            .next()
            .context("shaped code line has no layout run")?;
        let mut selected = run
            .glyphs
            .iter()
            .filter(|glyph| glyph.end > start && glyph.start < end);
        let first = selected.next().context("text range has no shaped glyphs")?;
        let mut left = first.x;
        let mut right = first.x + first.w;
        for glyph in selected {
            left = left.min(glyph.x);
            right = right.max(glyph.x + glyph.w);
        }
        Ok(TextRangeBounds {
            x: left,
            width: right - left,
        })
    }
}

fn make_title_sprite(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    file_name: &str,
) -> TextSprite {
    let attrs = Attrs::new()
        .family(Family::Name("CommitMono"))
        .weight(Weight::NORMAL)
        .color(Color::rgb(161, 161, 170));
    make_sprite(
        font_system,
        swash_cache,
        vec![(file_name, attrs.clone())],
        attrs,
        Metrics::new(20.0, 28.0),
        320,
        40,
    )
}

fn line_fingerprint(line: &CodeLine) -> u64 {
    let mut hasher = DefaultHasher::new();
    for span in &line.spans {
        span.text.hash(&mut hasher);
        span.style.hash(&mut hasher);
    }
    hasher.finish()
}

fn make_line_sprite(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    line: &CodeLine,
) -> TextSprite {
    let base = Attrs::new().family(Family::Name("CommitMono"));
    let spans: Vec<_> = if line.spans.is_empty() {
        vec![(" ", attributes(base.clone(), SyntaxStyle::Plain))]
    } else {
        line.spans
            .iter()
            .map(|span| (span.text.as_str(), attributes(base.clone(), span.style)))
            .collect()
    };
    make_sprite(
        font_system,
        swash_cache,
        spans,
        base,
        Metrics::new(28.0, LINE_HEIGHT),
        1320,
        LINE_HEIGHT as u32,
    )
}

fn make_sprite<'a>(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    spans: Vec<(&'a str, Attrs<'a>)>,
    base: Attrs<'a>,
    metrics: Metrics,
    width: u32,
    height: u32,
) -> TextSprite {
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(Some(width as f32), Some(height as f32));
    buffer.set_wrap(Wrap::None);
    buffer.set_rich_text(spans, &base, Shaping::Advanced, None);

    let mut pixels = vec![0_u8; width as usize * height as usize * 4];
    buffer.draw(
        font_system,
        swash_cache,
        Color::rgb(228, 228, 231),
        |x, y, rect_width, rect_height, color| {
            paint_rect(
                &mut pixels,
                width,
                height,
                x,
                y,
                rect_width,
                rect_height,
                [color.r(), color.g(), color.b(), color.a()],
            );
        },
    );
    TextSprite {
        width,
        height,
        pixels,
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    rect_width: u32,
    rect_height: u32,
    color: [u8; 4],
) {
    for row in 0..rect_height as i32 {
        let target_y = y + row;
        if !(0..height as i32).contains(&target_y) {
            continue;
        }
        for column in 0..rect_width as i32 {
            let target_x = x + column;
            if !(0..width as i32).contains(&target_x) {
                continue;
            }
            let index = (target_y as usize * width as usize + target_x as usize) * 4;
            blend_pixel(&mut pixels[index..index + 4], color, 1.0);
        }
    }
}

fn composite_sprite(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    sprite: &TextSprite,
    x: i32,
    y: i32,
    opacity: f32,
) {
    for sprite_y in 0..sprite.height as i32 {
        let target_y = y + sprite_y;
        if !(0..canvas_height as i32).contains(&target_y) {
            continue;
        }
        for sprite_x in 0..sprite.width as i32 {
            let target_x = x + sprite_x;
            if !(0..canvas_width as i32).contains(&target_x) {
                continue;
            }
            let source_index = (sprite_y as usize * sprite.width as usize + sprite_x as usize) * 4;
            if sprite.pixels[source_index + 3] == 0 {
                continue;
            }
            let target_index = (target_y as usize * canvas_width as usize + target_x as usize) * 4;
            blend_pixel(
                &mut canvas[target_index..target_index + 4],
                sprite.pixels[source_index..source_index + 4]
                    .try_into()
                    .expect("RGBA pixel has four channels"),
                opacity,
            );
        }
    }
}

fn blend_pixel(destination: &mut [u8], source: [u8; 4], opacity: f32) {
    let source_alpha = f32::from(source[3]) / 255.0 * opacity.clamp(0.0, 1.0);
    if source_alpha <= 0.0 {
        return;
    }
    let destination_alpha = f32::from(destination[3]) / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    for channel in 0..3 {
        let source_channel = f32::from(source[channel]) / 255.0;
        let destination_channel = f32::from(destination[channel]) / 255.0;
        let output = (source_channel * source_alpha
            + destination_channel * destination_alpha * (1.0 - source_alpha))
            / output_alpha;
        destination[channel] = (output * 255.0).round() as u8;
    }
    destination[3] = (output_alpha * 255.0).round() as u8;
}

fn attributes(base: Attrs<'static>, style: SyntaxStyle) -> Attrs<'static> {
    match style {
        SyntaxStyle::Plain => base.color(Color::rgb(228, 228, 231)),
        SyntaxStyle::Keyword => base.color(Color::rgb(196, 181, 253)),
        SyntaxStyle::Type => base.color(Color::rgb(125, 211, 252)),
        SyntaxStyle::String => base.color(Color::rgb(190, 242, 100)),
        SyntaxStyle::Accent => base.color(Color::rgb(110, 231, 183)),
    }
}
