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

use kinograph::code::{CodeLine, LineId, PlacedLine, StyledSpan, SyntaxStyle};
use kinograph::dsl::AnnotationFrame;

mod deployment_queue;
mod effects;
mod task;
mod terminal;
mod ui;

pub(crate) use deployment_queue::deployment_row_center_y;
pub use deployment_queue::{DeploymentItemFrame, DeploymentQueueFrame};
pub use task::{QuoteFrame, TaskLinkFrame, TaskSceneFrame};
pub use terminal::{CommandFileFrame, TerminalBackground, TerminalSceneFrame};

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
    pub panel_rotation: f32,
    pub panel_tilt_x: f32,
    pub panel_tilt_y: f32,
    pub panel_scale: f32,
    pub panel_near_blur: f32,
    pub focus_intensity: f32,
    pub focus_line_y: f32,
    pub focus_height: f32,
    pub token_highlight: TokenHighlight,
    pub bright_text: &'a [BrightTextFrame],
    pub pointer: PointerFrame,
    pub inline_reveals: &'a [InlineRevealFrame<'a>],
    pub squiggles: &'a [SquiggleFrame],
    pub annotations: &'a [AnnotationFrame],
    pub lines: &'a [PlacedLine<'a>],
}

pub struct BrightTextFrame {
    pub line: CodeLine,
    pub source_x: f32,
    pub width: f32,
    pub y: f32,
    pub opacity: f32,
    pub blur: f32,
}

#[derive(Clone, Copy)]
pub struct TokenHighlight {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub opacity: f32,
}

#[derive(Clone, Copy)]
pub struct PointerFrame {
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
    pub rotation: f32,
    pub scale: f32,
    pub blur: f32,
}

#[derive(Clone, Copy)]
pub struct InlineRevealFrame<'a> {
    pub line_id: &'a str,
    pub start_span: usize,
    pub end_span: usize,
    pub progress: f32,
}

#[derive(Clone, Copy)]
pub struct SquiggleFrame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
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
    resolution: [f32; 4],
    focus: [f32; 4],
    token_highlight: [f32; 4],
}

struct TextSprite {
    width: u32,
    height: u32,
    advance: f32,
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
    pointer_sprite: TextSprite,
    code_column_width: f32,
    line_sprites: HashMap<LineId, (u64, TextSprite)>,
    part_sprites: HashMap<String, (u64, TextSprite)>,
    task_layer_pixels: Vec<u8>,
    task_blur_source: Vec<[f32; 4]>,
    task_blur_scratch: Vec<[f32; 4]>,
    editor_background_pixels: Vec<u8>,
    terminal_background_pixels: Vec<u8>,
    terminal_neutral_background_pixels: Vec<u8>,
    deployment_background_pixels: Vec<u8>,
    ui_card_pixels: Vec<u8>,
    ui_overlay_pixels: Vec<u8>,
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
        eprintln!("GPU: {} ({:?})", info.name, info.backend);

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
            resolution: [spec.width as f32, spec.height as f32, 0.0, 0.0],
            focus: [0.0, 0.0, LINE_HEIGHT, 0.0],
            token_highlight: [0.0; 4],
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
        let pointer_sprite = make_pointer_sprite()?;
        let code_column_width = make_sprite(
            &mut font_system,
            &mut swash_cache,
            vec![("M", Attrs::new().family(Family::Name("CommitMono")))],
            Attrs::new().family(Family::Name("CommitMono")),
            Metrics::new(28.0, LINE_HEIGHT),
            64,
            LINE_HEIGHT as u32,
        )
        .advance;

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
            pointer_sprite,
            code_column_width,
            line_sprites: HashMap::new(),
            part_sprites: HashMap::new(),
            task_layer_pixels: Vec::new(),
            task_blur_source: Vec::new(),
            task_blur_scratch: Vec::new(),
            editor_background_pixels: Vec::new(),
            terminal_background_pixels: Vec::new(),
            terminal_neutral_background_pixels: Vec::new(),
            deployment_background_pixels: Vec::new(),
            ui_card_pixels: Vec::new(),
            ui_overlay_pixels: Vec::new(),
        })
    }

    pub fn composite_ui<R>(
        &mut self,
        pixels: &mut [u8],
        draw: impl FnOnce(&mut ui::card::FrameUi<'_>) -> Result<R>,
    ) -> Result<R> {
        let mut card_pixels = std::mem::take(&mut self.ui_card_pixels);
        let mut overlay_pixels = std::mem::take(&mut self.ui_overlay_pixels);
        let result = {
            let mut frame = ui::card::FrameUi::new(
                pixels,
                [self.spec.width, self.spec.height],
                &mut card_pixels,
                &mut overlay_pixels,
            )?;
            draw(&mut frame)
        };
        self.ui_card_pixels = card_pixels;
        self.ui_overlay_pixels = overlay_pixels;
        result
    }

    pub fn set_file_name(&mut self, file_name: &str) {
        if self.spec.file_name == file_name {
            return;
        }
        self.spec.file_name = file_name.to_owned();
        self.title_sprite = make_title_sprite(
            &mut self.font_system,
            &mut self.swash_cache,
            &self.spec.file_name,
        );
    }

    pub fn code_column_width(&self) -> f32 {
        self.code_column_width
    }

    pub fn render_title_card(
        &mut self,
        title: &str,
        subtitle: Option<&str>,
        opacity: f32,
    ) -> Vec<u8> {
        let mut pixels = vec![0_u8; self.spec.width as usize * self.spec.height as usize * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[1, 2, 4, 255]);
        }
        let center_x = self.spec.width as f32 * 0.5;
        let center_y = self.spec.height as f32 * 0.5;
        self.composite_title_card_text(
            &mut pixels,
            title,
            [center_x, center_y - 20.0],
            64.0,
            [238, 240, 244],
            opacity,
        );
        if let Some(subtitle) = subtitle {
            self.composite_title_card_text(
                &mut pixels,
                subtitle,
                [center_x, center_y + 64.0],
                26.0,
                [135, 145, 160],
                opacity * 0.9,
            );
        }
        pixels
    }

    pub fn composite_centered_text(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        center: [f32; 2],
        font_size: f32,
        color: [u8; 3],
        opacity: f32,
    ) {
        self.composite_title_card_text(pixels, text, center, font_size, color, opacity);
    }

    pub fn render_centered_code_line(
        &mut self,
        line: &CodeLine,
        reveals: &[InlineRevealFrame<'_>],
    ) -> Result<Vec<u8>> {
        let mut pixels = vec![0_u8; self.spec.width as usize * self.spec.height as usize * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[1, 2, 4, 255]);
        }
        let segments = inline_reveal_segments(line.spans().len(), reveals)?;
        for (start, end, _) in &segments {
            let spans = &line.spans()[*start..*end];
            let key = format!("centered-code:{}:{start}:{end}", line.id.as_str());
            let fingerprint = spans_fingerprint(spans);
            if self
                .part_sprites
                .get(&key)
                .is_none_or(|(cached, _)| *cached != fingerprint)
            {
                let sprite = make_spans_sprite_at_size(
                    &mut self.font_system,
                    &mut self.swash_cache,
                    spans,
                    64.0,
                    96.0,
                );
                self.part_sprites.insert(key, (fingerprint, sprite));
            }
        }
        let width = segments
            .iter()
            .map(|(start, end, progress)| {
                let key = format!("centered-code:{}:{start}:{end}", line.id.as_str());
                self.part_sprites[&key].1.advance * progress.unwrap_or(1.0).clamp(0.0, 1.0)
            })
            .sum::<f32>();
        let mut x = self.spec.width as f32 * 0.5 - width * 0.5;
        let y = (self.spec.height as f32 * 0.5 - 48.0).round() as i32;
        for (start, end, progress) in segments {
            let key = format!("centered-code:{}:{start}:{end}", line.id.as_str());
            let sprite = &self.part_sprites[&key].1;
            let progress = progress.unwrap_or(1.0).clamp(0.0, 1.0);
            let visible_width = sprite.advance * progress;
            if progress < 0.999 {
                composite_sprite_clipped_blurred(
                    &mut pixels,
                    self.spec.width,
                    self.spec.height,
                    sprite,
                    x.round() as i32,
                    y,
                    visible_width,
                    (1.0 - progress) * 4.0,
                    progress,
                );
            } else {
                composite_sprite_clipped(
                    &mut pixels,
                    self.spec.width,
                    self.spec.height,
                    sprite,
                    x.round() as i32,
                    y,
                    visible_width,
                    1.0,
                );
            }
            x += visible_width;
        }
        Ok(pixels)
    }

    fn composite_title_card_text(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        center: [f32; 2],
        font_size: f32,
        color: [u8; 3],
        opacity: f32,
    ) {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        font_size.to_bits().hash(&mut hasher);
        color.hash(&mut hasher);
        let key = format!("title-card:{:x}", hasher.finish());
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
                self.spec.width.saturating_sub(160),
                height,
            );
            self.part_sprites.insert(key.clone(), (0, sprite));
        }
        let sprite = &self.part_sprites[&key].1;
        composite_sprite(
            pixels,
            self.spec.width,
            self.spec.height,
            sprite,
            (center[0] - sprite.advance * 0.5).round() as i32,
            (center[1] - sprite.height as f32 * 0.5).round() as i32,
            opacity,
        );
    }

    pub fn render_shapes(&mut self, frame: &EditorFrame<'_>) -> Result<Vec<u8>> {
        let panel_center_y = self.spec.height as f32 * 0.52;
        let panel_top = self.spec.height as f32 * 0.17;
        let code_top = panel_top + 104.0;
        let focus_offset_y = code_top + frame.focus_line_y + LINE_HEIGHT * 0.5 - panel_center_y;
        let uniforms = SceneUniforms {
            resolution: [self.spec.width as f32, self.spec.height as f32, 0.0, 0.0],
            focus: [
                frame.focus_intensity,
                focus_offset_y,
                frame.focus_height,
                0.0,
            ],
            token_highlight: [
                frame.token_highlight.x,
                frame.token_highlight.y,
                frame.token_highlight.width,
                frame.token_highlight.opacity,
            ],
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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

    pub fn render_editor(&mut self, frame: &EditorFrame<'_>) -> Result<Vec<u8>> {
        let flat_frame = EditorFrame {
            panel_offset_y: 0.0,
            panel_rotation: 0.0,
            panel_tilt_x: 0.0,
            panel_tilt_y: 0.0,
            panel_scale: 1.0,
            panel_near_blur: 0.0,
            focus_intensity: frame.focus_intensity,
            focus_line_y: frame.focus_line_y,
            focus_height: frame.focus_height,
            token_highlight: frame.token_highlight,
            bright_text: frame.bright_text,
            pointer: frame.pointer,
            inline_reveals: frame.inline_reveals,
            squiggles: frame.squiggles,
            annotations: frame.annotations,
            lines: frame.lines,
        };
        let mut flat_pixels = self.render_shapes(&flat_frame)?;
        self.composite_text_untransformed(&mut flat_pixels, &flat_frame)?;

        let card_size = [
            (self.spec.width as f32 * 0.78).round() as u32,
            (self.spec.height as f32 * 0.70).round() as u32,
        ];
        let source_origin = [
            ((self.spec.width - card_size[0]) / 2),
            (self.spec.height as f32 * 0.17).round() as u32,
        ];
        let source = ui::card::RgbaSource::strided_region(
            &flat_pixels,
            [self.spec.width, self.spec.height],
            self.spec.width as usize * BYTES_PER_PIXEL as usize,
            source_origin,
            card_size,
        )?;
        if self.editor_background_pixels.is_empty() {
            let mut background =
                vec![0_u8; self.spec.width as usize * self.spec.height as usize * 4];
            self.composite_ui(&mut background, |ui| {
                ui.paint(|canvas| {
                    let bounds = canvas.bounds();
                    canvas.fill(
                        bounds,
                        0.0,
                        ui::card::Fill::Solid(ui::card::UiColor::srgb8(4, 4, 5, 255)),
                        1.0,
                    );
                    canvas.fill(
                        bounds,
                        0.0,
                        ui::card::Fill::Radial {
                            center: [bounds.size[0] * 0.5, bounds.size[1] * 0.46],
                            radius: bounds.size[0] * 0.62,
                            inner: ui::card::UiColor::srgb8(18, 18, 21, 150),
                            outer: ui::card::UiColor::srgb8(0, 0, 0, 0),
                        },
                        1.0,
                    );
                    Ok(())
                })
            })?;
            self.editor_background_pixels = background;
        }
        let mut pixels = self.editor_background_pixels.clone();
        let destination_size = [card_size[0] as f32, card_size[1] as f32];
        let destination_center = [
            self.spec.width as f32 * 0.5,
            self.spec.height as f32 * 0.52 + frame.panel_offset_y,
        ];
        self.composite_ui(&mut pixels, |ui| {
            ui.card(
                ui::card::CardFrame {
                    bounds: ui::Bounds::from_center(destination_center, destination_size),
                    style: ui::card::CardStyle::standard(),
                    projection: ui::card::CardProjection {
                        scale: frame.panel_scale,
                        rotation_z: frame.panel_rotation,
                        tilt_x: frame.panel_tilt_x,
                        tilt_y: frame.panel_tilt_y,
                        surface_blur: 0.0,
                        near_edge_blur: frame.panel_near_blur,
                    },
                    opacity: 1.0,
                },
                |card| {
                    card.content(|canvas| {
                        let bounds = canvas.bounds();
                        canvas.clipped(ui::card::Clip::rounded(bounds, 26.0), |canvas| {
                            canvas.rgba(bounds, source, ui::card::ContentFit::Contain, 1.0);
                            Ok(())
                        })
                    })?;
                    card.overlay(|canvas| {
                        let bounds = canvas.bounds().inset(ui::Edges::all(2.5));
                        canvas.stroke(
                            bounds,
                            25.5,
                            1.0,
                            ui::card::UiColor::srgb8(255, 255, 255, 38),
                            1.0,
                        );
                        Ok(())
                    })
                },
            )
        })?;
        Ok(pixels)
    }

    fn composite_text_untransformed(
        &mut self,
        pixels: &mut [u8],
        frame: &EditorFrame<'_>,
    ) -> Result<()> {
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
        for bright in frame.bright_text {
            let fingerprint = line_fingerprint(&bright.line);
            let is_stale = self
                .line_sprites
                .get(&bright.line.id)
                .is_none_or(|(cached, _)| *cached != fingerprint);
            if is_stale {
                let sprite =
                    make_line_sprite(&mut self.font_system, &mut self.swash_cache, &bright.line);
                self.line_sprites
                    .insert(bright.line.id.clone(), (fingerprint, sprite));
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
        let code_bottom = panel_top + self.spec.height as f32 * 0.70;
        let code_right = self.spec.width as f32 * 0.89 - 32.0;
        for placed in frame.lines {
            if placed.opacity <= 0.001 {
                continue;
            }
            let line_x = self.spec.width as f32 * 0.145 + placed.x;
            let line_y = code_top + placed.y;
            if line_y < code_top || line_y + LINE_HEIGHT > code_bottom {
                continue;
            }
            if frame
                .inline_reveals
                .iter()
                .any(|reveal| placed.line.id.as_str() == reveal.line_id)
            {
                let reveals = frame
                    .inline_reveals
                    .iter()
                    .filter(|reveal| placed.line.id.as_str() == reveal.line_id)
                    .copied()
                    .collect::<Vec<_>>();
                self.composite_inline_reveals(
                    pixels, placed, &reveals, line_x, line_y, code_right,
                )?;
                continue;
            }
            let sprite = self
                .line_sprites
                .get(&placed.line.id)
                .map(|(_, sprite)| sprite)
                .expect("line sprite was populated above");
            let line_blur = placed.blur;
            let clip_width = sprite.advance.min((code_right - line_x).max(0.0));
            if line_blur > 0.01 {
                composite_sprite_clipped_blurred(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    sprite,
                    line_x.round() as i32,
                    line_y.round() as i32,
                    clip_width,
                    line_blur,
                    placed.opacity,
                );
            } else {
                composite_sprite_clipped(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    sprite,
                    line_x.round() as i32,
                    line_y.round() as i32,
                    clip_width,
                    placed.opacity,
                );
            }
        }
        for bright in frame.bright_text {
            let y = code_top + bright.y;
            if y < code_top || y + LINE_HEIGHT > code_bottom {
                continue;
            }
            let sprite = self
                .line_sprites
                .get(&bright.line.id)
                .map(|(_, sprite)| sprite)
                .expect("bright text sprite was populated above");
            let source_x = bright.source_x.max(0.0);
            let width = bright
                .width
                .min(sprite.width as f32 - source_x)
                .min(code_right - (self.spec.width as f32 * 0.145 + source_x));
            composite_sprite_region(
                pixels,
                self.spec.width,
                self.spec.height,
                sprite,
                source_x,
                width,
                (self.spec.width as f32 * 0.145 + source_x).round() as i32,
                y.round() as i32,
                bright.blur,
                bright.opacity,
            );
        }
        for squiggle in frame.squiggles {
            composite_squiggle(
                pixels,
                self.spec.width,
                self.spec.height,
                self.spec.width as f32 * 0.145 + squiggle.x,
                code_top + squiggle.y + LINE_HEIGHT - 7.0,
                squiggle.width,
                squiggle.opacity,
            );
        }
        for annotation in frame.annotations {
            effects::composite(
                pixels,
                self.spec.width,
                self.spec.height,
                [self.spec.width as f32 * 0.145, code_top],
                LINE_HEIGHT,
                *annotation,
            );
        }
        composite_sprite_rotated(
            pixels,
            self.spec.width,
            self.spec.height,
            &self.pointer_sprite,
            36.0 * frame.pointer.scale,
            36.0 * frame.pointer.scale,
            self.spec.width as f32 * 0.145 + frame.pointer.x,
            code_top + frame.pointer.y,
            frame.pointer.rotation,
            frame.pointer.blur,
            frame.pointer.opacity,
        );
        Ok(())
    }

    fn composite_inline_reveals(
        &mut self,
        pixels: &mut [u8],
        placed: &PlacedLine<'_>,
        reveals: &[InlineRevealFrame],
        x: f32,
        y: f32,
        right: f32,
    ) -> Result<()> {
        let segments = inline_reveal_segments(placed.line.spans().len(), reveals)?;

        for (start, end, _) in &segments {
            let spans = &placed.line.spans()[*start..*end];
            let key = format!("{}:{start}:{end}", placed.line.id.as_str());
            let fingerprint = spans_fingerprint(spans);
            let stale = self
                .part_sprites
                .get(&key)
                .is_none_or(|(cached, _)| *cached != fingerprint);
            if stale {
                let sprite = make_spans_sprite(&mut self.font_system, &mut self.swash_cache, spans);
                self.part_sprites.insert(key, (fingerprint, sprite));
            }
        }

        let mut cursor_x = x;
        let y = y.round() as i32;
        let line_blur = placed.blur;
        for (start, end, progress) in segments {
            let key = format!("{}:{start}:{end}", placed.line.id.as_str());
            let sprite = &self.part_sprites[&key].1;
            let available = (right - cursor_x).max(0.0);
            if let Some(progress) = progress {
                let progress = progress.clamp(0.0, 1.0);
                composite_sprite_clipped_blurred(
                    pixels,
                    self.spec.width,
                    self.spec.height,
                    sprite,
                    cursor_x.round() as i32,
                    y,
                    (sprite.advance * progress).min(available),
                    ((1.0 - progress) * 4.0).max(line_blur),
                    placed.opacity * progress,
                );
                cursor_x += sprite.advance * progress;
            } else {
                let clip_width = sprite.advance.min(available);
                if line_blur > 0.01 {
                    composite_sprite_clipped_blurred(
                        pixels,
                        self.spec.width,
                        self.spec.height,
                        sprite,
                        cursor_x.round() as i32,
                        y,
                        clip_width,
                        line_blur,
                        placed.opacity,
                    );
                } else {
                    composite_sprite_clipped(
                        pixels,
                        self.spec.width,
                        self.spec.height,
                        sprite,
                        cursor_x.round() as i32,
                        y,
                        clip_width,
                        placed.opacity,
                    );
                }
                cursor_x += sprite.advance;
            }
        }
        Ok(())
    }

    pub fn measure_text_range(&mut self, line: &CodeLine, text: &str) -> Result<TextRangeBounds> {
        let full_text: String = line.spans().iter().map(|span| span.text.as_str()).collect();
        let start = full_text
            .find(text)
            .with_context(|| format!("line '{}' does not contain '{text}'", line.id.as_str()))?;
        let end = start + text.len();
        self.measure_text_byte_range(line, start, end)
    }

    pub fn measure_text_byte_range(
        &mut self,
        line: &CodeLine,
        start: usize,
        end: usize,
    ) -> Result<TextRangeBounds> {
        let full_text: String = line.spans().iter().map(|span| span.text.as_str()).collect();
        if start >= end
            || end > full_text.len()
            || !full_text.is_char_boundary(start)
            || !full_text.is_char_boundary(end)
        {
            bail!("text byte range is outside the code line");
        }
        let base = Attrs::new().family(Family::Name("CommitMono"));
        let spans: Vec<_> = line
            .spans()
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

type InlineSegment = (usize, usize, Option<f32>);

fn inline_reveal_segments(
    span_count: usize,
    reveals: &[InlineRevealFrame],
) -> Result<Vec<InlineSegment>> {
    let mut reveals = reveals.to_vec();
    reveals.sort_by_key(|reveal| reveal.start_span);
    let mut segments = Vec::with_capacity(reveals.len() * 2 + 1);
    let mut cursor = 0;
    for reveal in reveals {
        if reveal.start_span >= reveal.end_span || reveal.end_span > span_count {
            bail!("inline reveal span range is outside the code line");
        }
        if reveal.start_span < cursor {
            bail!("inline reveal span ranges overlap on the same code line");
        }
        if cursor < reveal.start_span {
            segments.push((cursor, reveal.start_span, None));
        }
        segments.push((reveal.start_span, reveal.end_span, Some(reveal.progress)));
        cursor = reveal.end_span;
    }
    if cursor < span_count {
        segments.push((cursor, span_count, None));
    }
    Ok(segments)
}

fn make_pointer_sprite() -> Result<TextSprite> {
    const SIZE: u32 = 36 * 4;
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="36" height="36" viewBox="0 0 256 256">
      <path fill="#f4f7f5" d="M224,104v50.93c0,46.2-36.85,84.55-83,85.06A83.71,83.71,0,0,1,80.6,215.4C58.79,192.33,34.15,136,34.15,136a16,16,0,0,1,6.53-22.23c7.66-4,17.1-.84,21.4,6.62l21,36.44a6.09,6.09,0,0,0,6,3.09l.12,0A8.19,8.19,0,0,0,96,151.74V32a16,16,0,0,1,16.77-16c8.61.4,15.23,7.82,15.23,16.43V104a8,8,0,0,0,8.53,8,8.17,8.17,0,0,0,7.47-8.25V88a16,16,0,0,1,16.77-16c8.61.4,15.23,7.82,15.23,16.43V112a8,8,0,0,0,8.53,8,8.17,8.17,0,0,0,7.47-8.25v-7.28c0-8.61,6.62-16,15.23-16.43A16,16,0,0,1,224,104Z"/>
    </svg>"##;

    rasterize_svg(SVG, SIZE, SIZE).context("rasterize Phosphor hand pointer")
}

fn rasterize_svg(svg: &str, width: u32, height: u32) -> Result<TextSprite> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .context("parse SVG sprite")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).context("allocate SVG sprite")?;
    let transform = resvg::tiny_skia::Transform::from_scale(
        width as f32 / tree.size().width(),
        height as f32 / tree.size().height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut pixels = pixmap.data().to_vec();
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha > 0 {
            for channel in &mut pixel[..3] {
                *channel = u32::from(*channel)
                    .saturating_mul(255)
                    .checked_div(alpha)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
    }
    Ok(TextSprite {
        width,
        height,
        advance: width as f32,
        pixels,
    })
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
    spans_fingerprint(line.spans())
}

fn spans_fingerprint(spans: &[StyledSpan]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for span in spans {
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
    make_spans_sprite(font_system, swash_cache, line.spans())
}

fn make_spans_sprite(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    line_spans: &[StyledSpan],
) -> TextSprite {
    make_spans_sprite_at_size(font_system, swash_cache, line_spans, 28.0, LINE_HEIGHT)
}

fn make_spans_sprite_at_size(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    line_spans: &[StyledSpan],
    font_size: f32,
    line_height: f32,
) -> TextSprite {
    let base = Attrs::new().family(Family::Name("CommitMono"));
    let spans: Vec<_> = if line_spans.is_empty() {
        vec![(" ", attributes(base.clone(), SyntaxStyle::Plain))]
    } else {
        line_spans
            .iter()
            .map(|span| (span.text.as_str(), attributes(base.clone(), span.style)))
            .collect()
    };
    make_sprite(
        font_system,
        swash_cache,
        spans,
        base,
        Metrics::new(font_size, line_height),
        1320,
        line_height.ceil() as u32,
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
    buffer.shape_until_scroll(font_system, false);
    let advance = buffer.layout_runs().next().map_or(0.0, |run| run.line_w);

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
        advance,
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
    composite_sprite_clipped(
        canvas,
        canvas_width,
        canvas_height,
        sprite,
        x,
        y,
        sprite.width as f32,
        opacity,
    );
}

#[allow(clippy::too_many_arguments)]
fn composite_sprite_clipped(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    sprite: &TextSprite,
    x: i32,
    y: i32,
    clip_width: f32,
    opacity: f32,
) {
    if opacity <= 0.001 || clip_width <= 0.0 {
        return;
    }
    let visible_width = clip_width.ceil().min(sprite.width as f32) as i32;
    for sprite_y in 0..sprite.height as i32 {
        let target_y = y + sprite_y;
        if !(0..canvas_height as i32).contains(&target_y) {
            continue;
        }
        for sprite_x in 0..visible_width {
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

#[allow(clippy::too_many_arguments)]
fn composite_sprite_region(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    sprite: &TextSprite,
    source_x: f32,
    width: f32,
    target_x: i32,
    target_y: i32,
    blur: f32,
    opacity: f32,
) {
    if opacity <= 0.001 || width <= 0.0 {
        return;
    }
    let source_left = source_x.floor().max(0.0) as i32;
    let visible_width = width.ceil().max(0.0) as i32;
    for sprite_y in 0..sprite.height as i32 {
        let canvas_y = target_y + sprite_y;
        if !(0..canvas_height as i32).contains(&canvas_y) {
            continue;
        }
        for offset_x in 0..visible_width {
            let canvas_x = target_x + offset_x;
            if !(0..canvas_width as i32).contains(&canvas_x) {
                continue;
            }
            let source_center_x = source_left + offset_x;
            if !(0..sprite.width as i32).contains(&source_center_x) {
                continue;
            }
            let source = if blur <= 0.01 {
                let index =
                    (sprite_y as usize * sprite.width as usize + source_center_x as usize) * 4;
                sprite.pixels[index..index + 4]
                    .try_into()
                    .expect("RGBA pixel has four channels")
            } else {
                let mut alpha_sum = 0_u32;
                let mut premultiplied = [0_u32; 3];
                for sample_y in 0..3 {
                    for sample_x in 0..3 {
                        let x = source_center_x + ((sample_x as f32 - 1.0) * blur).round() as i32;
                        let y = sprite_y + ((sample_y as f32 - 1.0) * blur).round() as i32;
                        if !(source_left..source_left + visible_width).contains(&x)
                            || !(0..sprite.height as i32).contains(&y)
                        {
                            continue;
                        }
                        let index = (y as usize * sprite.width as usize + x as usize) * 4;
                        let alpha = u32::from(sprite.pixels[index + 3]);
                        alpha_sum += alpha;
                        for (channel, sum) in premultiplied.iter_mut().enumerate() {
                            *sum += u32::from(sprite.pixels[index + channel]) * alpha;
                        }
                    }
                }
                if alpha_sum == 0 {
                    continue;
                }
                [
                    (premultiplied[0] / alpha_sum) as u8,
                    (premultiplied[1] / alpha_sum) as u8,
                    (premultiplied[2] / alpha_sum) as u8,
                    (alpha_sum / 9) as u8,
                ]
            };
            if source[3] == 0 {
                continue;
            }
            let target_index = (canvas_y as usize * canvas_width as usize + canvas_x as usize) * 4;
            blend_pixel(&mut canvas[target_index..target_index + 4], source, opacity);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_sprite_clipped_blurred(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    sprite: &TextSprite,
    x: i32,
    y: i32,
    clip_width: f32,
    blur: f32,
    opacity: f32,
) {
    if opacity <= 0.001 || clip_width <= 0.0 {
        return;
    }
    let visible_width = clip_width.ceil().min(sprite.width as f32) as i32;
    for sprite_y in 0..sprite.height as i32 {
        let target_y = y + sprite_y;
        if !(0..canvas_height as i32).contains(&target_y) {
            continue;
        }
        for sprite_x in 0..visible_width {
            let target_x = x + sprite_x;
            if !(0..canvas_width as i32).contains(&target_x) {
                continue;
            }
            const SAMPLE_GRID: i32 = 3;
            let mut alpha_sum = 0_u32;
            let mut premultiplied = [0_u32; 3];
            for sample_y in 0..SAMPLE_GRID {
                for sample_x in 0..SAMPLE_GRID {
                    let offset_x = (sample_x as f32 / (SAMPLE_GRID - 1) as f32 - 0.5) * blur * 2.0;
                    let offset_y = (sample_y as f32 / (SAMPLE_GRID - 1) as f32 - 0.5) * blur * 2.0;
                    let source_x = (sprite_x as f32 + offset_x).round() as i32;
                    let source_y = (sprite_y as f32 + offset_y).round() as i32;
                    if !(0..sprite.width as i32).contains(&source_x)
                        || !(0..sprite.height as i32).contains(&source_y)
                    {
                        continue;
                    }
                    let source_index =
                        (source_y as usize * sprite.width as usize + source_x as usize) * 4;
                    let alpha = u32::from(sprite.pixels[source_index + 3]);
                    alpha_sum += alpha;
                    for (channel, sum) in premultiplied.iter_mut().enumerate() {
                        *sum += u32::from(sprite.pixels[source_index + channel]) * alpha;
                    }
                }
            }
            if alpha_sum == 0 {
                continue;
            }
            let sample_count = (SAMPLE_GRID * SAMPLE_GRID) as u32;
            let source = [
                premultiplied[0].checked_div(alpha_sum).unwrap_or(0) as u8,
                premultiplied[1].checked_div(alpha_sum).unwrap_or(0) as u8,
                premultiplied[2].checked_div(alpha_sum).unwrap_or(0) as u8,
                (alpha_sum / sample_count) as u8,
            ];
            let target_index = (target_y as usize * canvas_width as usize + target_x as usize) * 4;
            blend_pixel(&mut canvas[target_index..target_index + 4], source, opacity);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_sprite_rotated(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    sprite: &TextSprite,
    display_width: f32,
    display_height: f32,
    center_x: f32,
    center_y: f32,
    rotation: f32,
    blur: f32,
    opacity: f32,
) {
    composite_sprite_rotated_with_coverage(
        canvas,
        canvas_width,
        canvas_height,
        sprite,
        display_width,
        display_height,
        center_x,
        center_y,
        rotation,
        blur,
        opacity,
        |_, _| 1.0,
    );
}

#[allow(clippy::too_many_arguments)]
fn composite_sprite_rotated_with_coverage(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    sprite: &TextSprite,
    display_width: f32,
    display_height: f32,
    center_x: f32,
    center_y: f32,
    rotation: f32,
    blur: f32,
    opacity: f32,
    coverage_at: impl Fn(f32, f32) -> f32,
) {
    if opacity <= 0.001 {
        return;
    }
    let radius = (display_width * display_width + display_height * display_height)
        .sqrt()
        .mul_add(0.5, blur + 1.0)
        .ceil() as i32;
    let sine = rotation.sin();
    let cosine = rotation.cos();
    let center_pixel_x = center_x.round() as i32;
    let center_pixel_y = center_y.round() as i32;

    for offset_y in -radius..=radius {
        let target_y = center_pixel_y + offset_y;
        if !(0..canvas_height as i32).contains(&target_y) {
            continue;
        }
        for offset_x in -radius..=radius {
            let target_x = center_pixel_x + offset_x;
            if !(0..canvas_width as i32).contains(&target_x) {
                continue;
            }
            let coverage = coverage_at(target_x as f32 + 0.5, target_y as f32 + 0.5);
            if coverage <= 0.0 {
                continue;
            }
            const SAMPLE_GRID: i32 = 4;
            let mut alpha_sum = 0_u32;
            let mut premultiplied = [0_u32; 3];
            for sample_y in 0..SAMPLE_GRID {
                for sample_x in 0..SAMPLE_GRID {
                    let subpixel_x = offset_x as f32
                        + ((sample_x as f32 + 0.5) / SAMPLE_GRID as f32 - 0.5) * (1.0 + blur * 2.0);
                    let subpixel_y = offset_y as f32
                        + ((sample_y as f32 + 0.5) / SAMPLE_GRID as f32 - 0.5) * (1.0 + blur * 2.0);
                    let local_x = subpixel_x * cosine + subpixel_y * sine;
                    let local_y = -subpixel_x * sine + subpixel_y * cosine;
                    let source_x =
                        ((local_x / display_width + 0.5) * sprite.width as f32).floor() as i32;
                    let source_y =
                        ((local_y / display_height + 0.5) * sprite.height as f32).floor() as i32;
                    if !(0..sprite.width as i32).contains(&source_x)
                        || !(0..sprite.height as i32).contains(&source_y)
                    {
                        continue;
                    }
                    let source_index =
                        (source_y as usize * sprite.width as usize + source_x as usize) * 4;
                    let alpha = u32::from(sprite.pixels[source_index + 3]);
                    alpha_sum += alpha;
                    for (channel, sum) in premultiplied.iter_mut().enumerate() {
                        *sum += u32::from(sprite.pixels[source_index + channel]) * alpha;
                    }
                }
            }
            if alpha_sum == 0 {
                continue;
            }
            let sample_count = (SAMPLE_GRID * SAMPLE_GRID) as u32;
            let source = [
                premultiplied[0].checked_div(alpha_sum).unwrap_or(0) as u8,
                premultiplied[1].checked_div(alpha_sum).unwrap_or(0) as u8,
                premultiplied[2].checked_div(alpha_sum).unwrap_or(0) as u8,
                (alpha_sum / sample_count) as u8,
            ];
            let target_index = (target_y as usize * canvas_width as usize + target_x as usize) * 4;
            blend_pixel(
                &mut canvas[target_index..target_index + 4],
                source,
                opacity * coverage,
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

fn composite_squiggle(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    x: f32,
    y: f32,
    width: f32,
    opacity: f32,
) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.001 {
        return;
    }
    for offset_x in 0..width.max(0.0).round() as i32 {
        let wave_y = ((offset_x as f32 * 0.48).sin() * 2.0).round() as i32;
        for thickness in 0..2 {
            let target_x = x.round() as i32 + offset_x;
            let target_y = y.round() as i32 + wave_y + thickness;
            if target_x < 0
                || target_y < 0
                || target_x >= canvas_width as i32
                || target_y >= canvas_height as i32
            {
                continue;
            }
            let index = (target_y as usize * canvas_width as usize + target_x as usize) * 4;
            blend_pixel(&mut canvas[index..index + 4], [248, 113, 113, 255], opacity);
        }
    }
}

fn attributes(base: Attrs<'static>, style: SyntaxStyle) -> Attrs<'static> {
    match style {
        SyntaxStyle::Plain => base.color(Color::rgb(228, 228, 231)),
        SyntaxStyle::Keyword => base.color(Color::rgb(196, 181, 253)),
        SyntaxStyle::Type => base.color(Color::rgb(125, 211, 252)),
        SyntaxStyle::String => base.color(Color::rgb(190, 242, 100)),
        SyntaxStyle::Accent => base.color(Color::rgb(110, 231, 183)),
        SyntaxStyle::Rgb(red, green, blue) => base.color(Color::rgb(red, green, blue)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InlineRevealFrame, TextSprite, composite_sprite_rotated,
        composite_sprite_rotated_with_coverage, inline_reveal_segments,
    };

    fn opaque_test_sprite() -> TextSprite {
        TextSprite {
            width: 4,
            height: 4,
            advance: 4.0,
            pixels: vec![255; 4 * 4 * 4],
        }
    }

    #[test]
    fn unmasked_rotated_sprite_matches_constant_coverage() {
        let sprite = opaque_test_sprite();
        let mut wrapped = vec![0_u8; 20 * 20 * 4];
        let mut generic = wrapped.clone();

        composite_sprite_rotated(
            &mut wrapped,
            20,
            20,
            &sprite,
            8.0,
            8.0,
            10.0,
            10.0,
            0.2,
            1.0,
            0.8,
        );
        composite_sprite_rotated_with_coverage(
            &mut generic,
            20,
            20,
            &sprite,
            8.0,
            8.0,
            10.0,
            10.0,
            0.2,
            1.0,
            0.8,
            |_, _| 1.0,
        );

        assert_eq!(wrapped, generic);
    }

    #[test]
    fn rotated_sprite_coverage_clips_after_blur_sampling() {
        let sprite = opaque_test_sprite();
        let mut pixels = vec![0_u8; 20 * 20 * 4];
        let coverage = |x: f32, y: f32| {
            if (8.0..12.0).contains(&x) && (8.0..12.0).contains(&y) {
                1.0
            } else {
                0.0
            }
        };

        composite_sprite_rotated_with_coverage(
            &mut pixels,
            20,
            20,
            &sprite,
            12.0,
            12.0,
            10.0,
            10.0,
            0.35,
            2.0,
            1.0,
            coverage,
        );

        let mut painted = 0;
        for y in 0..20 {
            for x in 0..20 {
                let alpha = pixels[(y * 20 + x) * 4 + 3];
                if alpha > 0 {
                    painted += 1;
                    assert!(coverage(x as f32 + 0.5, y as f32 + 0.5) > 0.0);
                }
            }
        }
        assert!(painted > 0);
    }

    #[test]
    fn multiple_inline_reveals_partition_one_stable_line() {
        let segments = inline_reveal_segments(
            8,
            &[
                InlineRevealFrame {
                    line_id: "line",
                    start_span: 5,
                    end_span: 7,
                    progress: 0.25,
                },
                InlineRevealFrame {
                    line_id: "line",
                    start_span: 1,
                    end_span: 3,
                    progress: 0.75,
                },
            ],
        )
        .unwrap();

        assert_eq!(
            segments,
            vec![
                (0, 1, None),
                (1, 3, Some(0.75)),
                (3, 5, None),
                (5, 7, Some(0.25)),
                (7, 8, None),
            ]
        );
    }

    #[test]
    fn overlapping_inline_reveals_are_rejected() {
        let error = inline_reveal_segments(
            5,
            &[
                InlineRevealFrame {
                    line_id: "line",
                    start_span: 1,
                    end_span: 3,
                    progress: 1.0,
                },
                InlineRevealFrame {
                    line_id: "line",
                    start_span: 2,
                    end_span: 4,
                    progress: 1.0,
                },
            ],
        )
        .unwrap_err();

        assert!(error.to_string().contains("overlap"));
    }
}
