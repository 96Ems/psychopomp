use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
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
pub use task::{
    BubblePose, ContentPose, QuoteFrame, TaskContentFrame, TaskLinkFrame, TaskSceneFrame,
    TaskVisualFrame,
};
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

/// One partition of a line, shaped exactly as the inline compositor shapes it.
/// Selection bounds are local to this partition; hidden partitions occupy zero width.
pub(super) struct InlineRangeMetrics {
    pub spans: std::ops::Range<usize>,
    pub advance: f32,
    pub selection: Option<[f32; 2]>,
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

/// Stationary canvas-space aperture. Text moves through it; the fade does not
/// move with the glyphs or paint over the already-composited background.
#[derive(Clone, Copy, Debug)]
pub struct VerticalMask {
    pub top: f32,
    pub bottom: f32,
    pub fade: f32,
}

impl VerticalMask {
    pub fn is_valid(self) -> bool {
        let height = self.bottom - self.top;
        self.top.is_finite()
            && self.bottom.is_finite()
            && height.is_finite()
            && height > 0.
            && self.fade.is_finite()
            && self.fade >= 0.
            && self.fade <= height * 0.5
    }

    fn coverage(self, start: f32, end: f32) -> f32 {
        let start = start.max(self.top);
        let end = end.min(self.bottom);
        if start >= end {
            return 0.;
        }
        if self.fade == 0. {
            return (end - start).clamp(0., 1.);
        }
        // Integrate the linear mask over this pixel's covered row interval.
        // Fractional mask edges cannot expose an opaque row at once.
        let fade = f64::from(self.fade);
        let ramp = |y: f64| {
            if y <= 0. {
                0.
            } else if y < fade {
                y * y / (2. * fade)
            } else {
                y - fade * 0.5
            }
        };
        let integral = |y: f32| {
            ramp(f64::from(y) - f64::from(self.top))
                - ramp(f64::from(y) - (f64::from(self.bottom) - fade))
        };
        (integral(end) - integral(start)).clamp(0., 1.) as f32
    }
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
    interactive_preview: bool,
    preview_editor_backgrounds: VecDeque<(String, Vec<u8>)>,
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
            interactive_preview: false,
            preview_editor_backgrounds: VecDeque::new(),
        })
    }

    /// Live flat-editor preview skips the final optical resampling of glyphs.
    /// Export never enables this profile; unsupported poses/effects use the full path.
    pub fn set_interactive_preview(&mut self, enabled: bool) {
        self.interactive_preview = enabled;
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
            None,
        );
        if let Some(subtitle) = subtitle {
            self.composite_title_card_text(
                &mut pixels,
                subtitle,
                [center_x, center_y + 64.0],
                26.0,
                [135, 145, 160],
                opacity * 0.9,
                None,
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
        self.composite_title_card_text(pixels, text, center, font_size, color, opacity, None);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn composite_centered_text_masked(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        center: [f32; 2],
        font_size: f32,
        color: [u8; 3],
        opacity: f32,
        mask: Option<VerticalMask>,
    ) {
        self.composite_title_card_text(pixels, text, center, font_size, color, opacity, mask);
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
        let y = self.spec.height as f32 * 0.5 - 48.0;
        for (start, end, progress) in segments {
            let key = format!("centered-code:{}:{start}:{end}", line.id.as_str());
            let sprite = &self.part_sprites[&key].1;
            let progress = progress.unwrap_or(1.0).clamp(0.0, 1.0);
            let visible_width = sprite.advance * progress;
            composite_text_sprite(
                &mut pixels,
                [self.spec.width, self.spec.height],
                sprite,
                [x, y],
                visible_width,
                (1.0 - progress) * 4.0,
                progress,
                [0.0, self.spec.height as f32],
            );
            x += visible_width;
        }
        Ok(pixels)
    }

    #[allow(clippy::too_many_arguments)]
    fn composite_title_card_text(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        center: [f32; 2],
        font_size: f32,
        color: [u8; 3],
        opacity: f32,
        mask: Option<VerticalMask>,
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
        composite_text_region(
            pixels,
            [self.spec.width, self.spec.height],
            sprite,
            [
                center[0] - sprite.advance * 0.5,
                center[1] - sprite.height as f32 * 0.5,
            ],
            0.,
            sprite.width as f32,
            0.0,
            opacity,
            [0.0, self.spec.height as f32],
            mask,
        );
    }

    pub fn render_shapes(&mut self, frame: &EditorFrame<'_>) -> Result<Vec<u8>> {
        self.render_shapes_pass(frame, true)
    }

    fn render_shapes_pass(&mut self, frame: &EditorFrame<'_>, chrome: bool) -> Result<Vec<u8>> {
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
                if chrome { 1.0 } else { 0.0 },
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
        if self.interactive_preview
            && can_preview_editor(frame, [self.spec.width, self.spec.height])
        {
            if let Some(index) = self
                .preview_editor_backgrounds
                .iter()
                .position(|(name, _)| name == &self.spec.file_name)
            {
                let cached = self
                    .preview_editor_backgrounds
                    .remove(index)
                    .expect("located cached chrome");
                self.preview_editor_backgrounds.push_back(cached);
            } else {
                let background_frame = EditorFrame {
                    lines: &[],
                    focus_intensity: 0.,
                    token_highlight: TokenHighlight {
                        opacity: 0.,
                        ..frame.token_highlight
                    },
                    ..*frame
                };
                let pixels = self.render_editor_full(&background_frame)?;
                // Two editor slides are the demonstrated reuse; bound retained
                // RGBA memory instead of keeping one full canvas for every file.
                if self.preview_editor_backgrounds.len() == 2 {
                    self.preview_editor_backgrounds.pop_front();
                }
                self.preview_editor_backgrounds
                    .push_back((self.spec.file_name.clone(), pixels));
            }
            let mut pixels = self
                .preview_editor_backgrounds
                .back()
                .expect("background prepared")
                .1
                .clone();
            if frame.focus_intensity > 0. || frame.token_highlight.opacity > 0. {
                // The same WGSL recipe draws dynamic overlays, without sending
                // unchanged chrome through the expensive optical card compositor.
                let overlay = self.render_shapes_pass(frame, false)?;
                for (pixel, source) in pixels.chunks_exact_mut(4).zip(overlay.chunks_exact(4)) {
                    if source[3] > 0 {
                        blend_pixel(pixel, [source[0], source[1], source[2], source[3]], 1.);
                    }
                }
            }
            self.composite_text_untransformed(&mut pixels, frame)?;
            return Ok(pixels);
        }
        self.render_editor_full(frame)
    }

    fn render_editor_full(&mut self, frame: &EditorFrame<'_>) -> Result<Vec<u8>> {
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
        self.composite_editor_title(&mut flat_pixels, flat_frame.panel_offset_y);
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
        let mut card_style = ui::card::CardStyle::standard();
        card_style.border_width = 0.75;
        card_style.border_color = ui::card::UiColor::srgb8(255, 255, 255, 10);
        self.composite_ui(&mut pixels, |ui| {
            ui.card(
                ui::card::CardFrame {
                    bounds: ui::Bounds::from_center(destination_center, destination_size),
                    style: card_style,
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
                        canvas.stroke_fill(
                            bounds,
                            25.5,
                            1.0,
                            ui::card::Fill::Linear {
                                from: [0.0, bounds.origin[1]],
                                to: [0.0, bounds.bottom()],
                                start: ui::card::UiColor::srgb8(255, 255, 255, 38),
                                end: ui::card::UiColor::srgb8(255, 255, 255, 5),
                            },
                            1.0,
                        );
                        Ok(())
                    })
                },
            )
        })?;
        Ok(pixels)
    }

    fn composite_editor_title(&self, pixels: &mut [u8], panel_offset_y: f32) {
        let panel_top = self.spec.height as f32 * 0.17 + panel_offset_y;
        composite_sprite(
            pixels,
            self.spec.width,
            self.spec.height,
            &self.title_sprite,
            (self.spec.width as f32 * 0.135).round() as i32,
            (panel_top + 16.0).round() as i32,
            1.0,
        );
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
        let code_top = panel_top + 104.0;
        let code_bottom = panel_top + self.spec.height as f32 * 0.70;
        let code_right = self.spec.width as f32 * 0.89 - 32.0;
        for placed in frame.lines {
            if placed.opacity <= 0.001 {
                continue;
            }
            let line_x = self.spec.width as f32 * 0.145 + placed.x;
            let line_y = code_top + placed.y;
            if line_y + LINE_HEIGHT <= code_top || line_y >= code_bottom {
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
                    pixels,
                    placed,
                    &reveals,
                    line_x,
                    line_y,
                    code_right,
                    [code_top, code_bottom],
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
            composite_text_sprite(
                pixels,
                [self.spec.width, self.spec.height],
                sprite,
                [line_x, line_y],
                clip_width,
                line_blur,
                placed.opacity,
                [code_top, code_bottom],
            );
        }
        for bright in frame.bright_text {
            let y = code_top + bright.y;
            if y + LINE_HEIGHT <= code_top || y >= code_bottom {
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
            composite_text_region(
                pixels,
                [self.spec.width, self.spec.height],
                sprite,
                [self.spec.width as f32 * 0.145 + source_x, y],
                source_x,
                width,
                bright.blur,
                bright.opacity,
                [code_top, code_bottom],
                None,
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

    #[allow(clippy::too_many_arguments)]
    fn composite_inline_reveals(
        &mut self,
        pixels: &mut [u8],
        placed: &PlacedLine<'_>,
        reveals: &[InlineRevealFrame],
        x: f32,
        y: f32,
        right: f32,
        clip_y: [f32; 2],
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
        let line_blur = placed.blur;
        for (start, end, progress) in segments {
            let key = format!("{}:{start}:{end}", placed.line.id.as_str());
            let sprite = &self.part_sprites[&key].1;
            let available = (right - cursor_x).max(0.0);
            let progress = progress.unwrap_or(1.0).clamp(0.0, 1.0);
            let width = sprite.advance * progress;
            composite_text_sprite(
                pixels,
                [self.spec.width, self.spec.height],
                sprite,
                [cursor_x, y],
                width.min(available),
                ((1.0 - progress) * 4.0).max(line_blur),
                placed.opacity * progress,
                clip_y,
            );
            cursor_x += width;
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

    pub(super) fn measure_inline_target(
        &mut self,
        line: &CodeLine,
        reveals: &[InlineRevealFrame<'_>],
        selected: std::ops::Range<usize>,
    ) -> Result<Vec<InlineRangeMetrics>> {
        inline_reveal_segments(line.spans().len(), reveals)?
            .into_iter()
            .map(|(start, end, _)| {
                let spans = &line.spans()[start..end];
                let advance =
                    make_spans_sprite(&mut self.font_system, &mut self.swash_cache, spans).advance;
                let from = start.max(selected.start);
                let to = end.min(selected.end);
                let selection = if from < to {
                    let start_byte = line.spans()[start..from]
                        .iter()
                        .map(|span| span.text.len())
                        .sum();
                    let end_byte = line.spans()[start..to]
                        .iter()
                        .map(|span| span.text.len())
                        .sum();
                    let bounds = self.measure_text_byte_range(
                        &CodeLine::new("measured-part", spans.to_vec()),
                        start_byte,
                        end_byte,
                    )?;
                    Some([bounds.x, bounds.x + bounds.width])
                } else {
                    None
                };
                Ok(InlineRangeMetrics {
                    spans: start..end,
                    advance,
                    selection,
                })
            })
            .collect()
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

fn can_preview_editor(frame: &EditorFrame<'_>, [width, height]: [u32; 2]) -> bool {
    let width = width as f32;
    let height = height as f32;
    if frame.panel_offset_y != 0.0
        || frame.panel_rotation != 0.0
        || frame.panel_tilt_x != 0.0
        || frame.panel_tilt_y != 0.0
        || frame.panel_scale != 1.0
        || frame.panel_near_blur != 0.0
        || frame.pointer.opacity > 0.001
        || !frame.bright_text.is_empty()
        || !frame.squiggles.is_empty()
        || !frame.annotations.is_empty()
        || frame.lines.iter().any(|line| line.x < 0.0)
    {
        return false;
    }
    // Overlays outside the flat code body need the full rounded-card clip and
    // chrome draw order. Keep the cheap pass only where the two cannot overlap.
    let body_top = height * 0.17 + 64.0;
    let body_bottom = height * 0.87 - 28.0;
    if frame.focus_intensity > 0.0 {
        let top = height * 0.17 + 104.0 + frame.focus_line_y + LINE_HEIGHT * 0.5
            - frame.focus_height * 0.5
            - 2.0;
        if top < body_top || top + frame.focus_height + 4.0 > body_bottom {
            return false;
        }
    }
    if frame.token_highlight.opacity > 0.0 {
        let highlight = frame.token_highlight;
        let left = width * 0.145 + highlight.x - 15.0;
        let top = height * 0.17 + 104.0 + highlight.y - 5.0;
        if left < width * 0.11 + 28.0
            || left + highlight.width + 30.0 > width * 0.89 - 28.0
            || top < body_top
            || top + 54.0 > body_bottom
        {
            return false;
        }
    }
    true
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
fn composite_text_sprite(
    canvas: &mut [u8],
    size: [u32; 2],
    sprite: &TextSprite,
    origin: [f32; 2],
    clip_width: f32,
    blur: f32,
    opacity: f32,
    clip_y: [f32; 2],
) {
    composite_text_region(
        canvas, size, sprite, origin, 0., clip_width, blur, opacity, clip_y, None,
    );
}

#[allow(clippy::too_many_arguments)]
fn composite_text_region(
    canvas: &mut [u8],
    [canvas_width, canvas_height]: [u32; 2],
    sprite: &TextSprite,
    [x, y]: [f32; 2],
    source_left: f32,
    clip_width: f32,
    blur: f32,
    opacity: f32,
    clip_y: [f32; 2],
    mask: Option<VerticalMask>,
) {
    if opacity <= 0.0 || clip_width <= 0.0 {
        return;
    }
    let source_clip = [
        source_left,
        (source_left + clip_width).min(sprite.width as f32),
    ];
    let clip_y = mask.map_or(clip_y, |mask| {
        [clip_y[0].max(mask.top), clip_y[1].min(mask.bottom)]
    });
    // The source mask is filtered with the glyph. Include its complete support;
    // otherwise changing floor/ceil bounds would discard nonzero filtered texels.
    let left = x - source_left + source_clip[0].floor() - blur - 1.0;
    let right = x - source_left + source_clip[1].ceil() + blur + 1.0;
    let top = (y - blur - 1.0).max(clip_y[0]);
    let bottom = (y + sprite.height as f32 + blur + 1.0).min(clip_y[1]);
    for target_y in (top.floor() as i32).max(0)..(bottom.ceil() as i32).min(canvas_height as i32) {
        let row_start = (target_y as f32).max(clip_y[0]);
        let row_end = (target_y as f32 + 1.).min(clip_y[1]);
        let coverage_y = mask.map_or_else(
            || (row_end - row_start).clamp(0., 1.),
            |mask| mask.coverage(row_start, row_end),
        );
        if coverage_y <= 0. {
            continue;
        }
        for target_x in (left.floor() as i32).max(0)..(right.ceil() as i32).min(canvas_width as i32)
        {
            let source_x = source_left + target_x as f32 - x;
            let source_y = target_y as f32 - y;
            let mut color = [0.0; 4];
            if blur <= 0.0 {
                color = sample_text_sprite(sprite, source_x, source_y, source_clip);
            } else {
                // Bilinear sample locations vary continuously with blur; no
                // rounded taps that suddenly turn a sharp glyph into a 3x3 copy.
                for (dy, wy) in [(-1.0, 0.25), (0.0, 0.5), (1.0, 0.25)] {
                    for (dx, wx) in [(-1.0, 0.25), (0.0, 0.5), (1.0, 0.25)] {
                        let sample = sample_text_sprite(
                            sprite,
                            source_x + dx * blur,
                            source_y + dy * blur,
                            source_clip,
                        );
                        for channel in 0..4 {
                            color[channel] += sample[channel] * wx * wy;
                        }
                    }
                }
            }
            if color[3] <= 0.0 {
                continue;
            }
            let source = [
                (color[0] / color[3]).round() as u8,
                (color[1] / color[3]).round() as u8,
                (color[2] / color[3]).round() as u8,
                (color[3] * coverage_y).round() as u8,
            ];
            let target_index = (target_y as usize * canvas_width as usize + target_x as usize) * 4;
            blend_pixel(&mut canvas[target_index..target_index + 4], source, opacity);
        }
    }
}

/// Premultiplied RGBA interpolation avoids dark fringes around transparent glyphs.
fn sample_text_sprite(sprite: &TextSprite, x: f32, y: f32, clip: [f32; 2]) -> [f32; 4] {
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let dx = x - left as f32;
    let dy = y - top as f32;
    let mut color = [0.0; 4];
    for (row, wy) in [(top, 1.0 - dy), (top + 1, dy)] {
        for (column, wx) in [(left, 1.0 - dx), (left + 1, dx)] {
            let weight = wx * wy;
            if weight == 0.0
                || column < 0
                || row < 0
                || column >= sprite.width as i32
                || row >= sprite.height as i32
            {
                continue;
            }
            let offset = (row as usize * sprite.width as usize + column as usize) * 4;
            // Apply intentional source-range clipping once, before filtering.
            // Transparent texture borders already provide raster-edge coverage.
            let coverage =
                ((column as f32 + 1.0).min(clip[1]) - (column as f32).max(clip[0])).clamp(0.0, 1.0);
            let alpha = f32::from(sprite.pixels[offset + 3]) * weight * coverage;
            for (channel, value) in color[..3].iter_mut().enumerate() {
                *value += f32::from(sprite.pixels[offset + channel]) * alpha;
            }
            color[3] += alpha;
        }
    }
    color
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
    if display_width <= 0.0 || display_height <= 0.0 {
        return;
    }
    let source_scale = [
        sprite.width as f32 / display_width,
        sprite.height as f32 / display_height,
    ];
    let filter_radius = [
        blur * source_scale[0] + (source_scale[0] - 1.).max(0.) * 0.5,
        blur * source_scale[1] + (source_scale[1] - 1.).max(0.) * 0.5,
    ];
    let radius = (display_width * display_width + display_height * display_height)
        .sqrt()
        .mul_add(
            0.5,
            blur + (1.0 / source_scale[0]).max(1.0 / source_scale[1]) + 1.0,
        )
        .ceil() as i32;
    let sine = rotation.sin();
    let cosine = rotation.cos();
    let center_pixel_x = center_x.round() as i32;
    let center_pixel_y = center_y.round() as i32;

    for target_y in center_pixel_y.saturating_sub(radius).max(0)
        ..=center_pixel_y
            .saturating_add(radius)
            .min(canvas_height as i32 - 1)
    {
        for target_x in center_pixel_x.saturating_sub(radius).max(0)
            ..=center_pixel_x
                .saturating_add(radius)
                .min(canvas_width as i32 - 1)
        {
            let coverage = coverage_at(target_x as f32 + 0.5, target_y as f32 + 0.5);
            if coverage <= 0.0 {
                continue;
            }
            let dx = target_x as f32 + 0.5 - center_x;
            let dy = target_y as f32 + 0.5 - center_y;
            let source_x =
                ((dx * cosine + dy * sine) / display_width + 0.5) * sprite.width as f32 - 0.5;
            let source_y =
                ((-dx * sine + dy * cosine) / display_height + 0.5) * sprite.height as f32 - 0.5;
            let mut color = [0.; 4];
            if filter_radius == [0., 0.] {
                color = sample_text_sprite(sprite, source_x, source_y, [0., sprite.width as f32]);
            } else {
                // Denser binomial taps soften transformed content instead of
                // showing three displaced copies of a blurred letter/border.
                // Fixed support and continuously varying offsets avoid kernel
                // changes at integer blur radii.
                const TAPS: [(f32, f32); 5] = [
                    (-1., 0.0625),
                    (-0.5, 0.25),
                    (0., 0.375),
                    (0.5, 0.25),
                    (1., 0.0625),
                ];
                for (y, wy) in TAPS {
                    for (x, wx) in TAPS {
                        let sample = sample_text_sprite(
                            sprite,
                            source_x + x * filter_radius[0],
                            source_y + y * filter_radius[1],
                            [0., sprite.width as f32],
                        );
                        for channel in 0..4 {
                            color[channel] += sample[channel] * wx * wy;
                        }
                    }
                }
            }
            if color[3] <= 0.0 {
                continue;
            }
            let source = [
                (color[0] / color[3]).round() as u8,
                (color[1] / color[3]).round() as u8,
                (color[2] / color[3]).round() as u8,
                color[3].round() as u8,
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
    #[test]
    fn vertical_mask_integrates_linear_fades_and_fractional_edges() {
        let mask = super::VerticalMask {
            top: 2.,
            bottom: 8.,
            fade: 2.,
        };
        assert!(mask.is_valid());
        for (y, expected) in [0., 0., 0.25, 0.75, 1., 1., 0.75, 0.25, 0.]
            .into_iter()
            .enumerate()
        {
            assert_eq!(mask.coverage(y as f32, y as f32 + 1.), expected);
        }
        let fractional = super::VerticalMask {
            top: 2.5,
            bottom: 8.5,
            fade: 2.,
        };
        assert_eq!(fractional.coverage(2., 3.), 0.0625);
        assert_eq!(fractional.coverage(8., 9.), 0.0625);
        let a = super::VerticalMask {
            top: 2.49,
            ..fractional
        };
        let b = super::VerticalMask {
            top: 2.51,
            ..fractional
        };
        assert!((a.coverage(2., 3.) - b.coverage(2., 3.)).abs() < 0.006);
        assert_eq!(
            super::VerticalMask {
                fade: 0.,
                ..fractional
            }
            .coverage(2., 3.),
            0.5
        );
    }

    #[test]
    fn stationary_mask_fades_glyph_rows_not_the_existing_background() {
        let sprite = opaque_test_sprite();
        let mask = super::VerticalMask {
            top: 4.,
            bottom: 12.,
            fade: 2.,
        };
        let mut pixels = vec![0; 16 * 16 * 4];
        super::composite_text_region(
            &mut pixels,
            [16, 16],
            &sprite,
            [4., 3.],
            0.,
            4.,
            0.,
            1.,
            [0., 16.],
            Some(mask),
        );
        let alpha = |y: usize| pixels[(y * 16 + 5) * 4 + 3];
        assert_eq!([alpha(3), alpha(4), alpha(5), alpha(6)], [0, 64, 191, 255]);
        for pixel in pixels.chunks_exact(4).filter(|p| p[3] != 0) {
            assert_eq!(&pixel[..3], &[255; 3]);
        }

        let background = [17, 33, 49, 255].repeat(16 * 16);
        let mut pixels = background.clone();
        super::composite_text_region(
            &mut pixels,
            [16, 16],
            &sprite,
            [4., 3.],
            0.,
            4.,
            0.,
            1.,
            [0., 16.],
            Some(mask),
        );
        for y in 0..16 {
            for x in 0..16 {
                if x == 0 || !(4..12).contains(&y) {
                    let offset = (y * 16 + x) * 4;
                    assert_eq!(&pixels[offset..offset + 4], &background[offset..offset + 4]);
                }
            }
        }
    }

    #[test]
    fn filtered_text_footprint_has_no_integer_boundary_pops() {
        let sprite = super::TextSprite {
            width: 12,
            height: 12,
            advance: 12.,
            pixels: vec![255; 12 * 12 * 4],
        };
        let draw = |origin, width, blur| {
            let mut pixels = vec![0; 40 * 24 * 4];
            super::composite_text_sprite(
                &mut pixels,
                [40, 24],
                &sprite,
                origin,
                width,
                blur,
                1.,
                [0., 24.],
            );
            pixels
        };
        for (a, b) in [
            (draw([10.5, 4.], 4.5, 0.), draw([10.5, 4.], 4.51, 0.)),
            (draw([9.99, 4.], 12., 0.5), draw([10., 4.], 12., 0.5)),
            (draw([10., 3.99], 12., 0.5), draw([10., 4.], 12., 0.5)),
        ] {
            assert!(
                a.chunks_exact(4)
                    .zip(b.chunks_exact(4))
                    .map(|(a, b)| a[3].abs_diff(b[3]))
                    .max()
                    .unwrap()
                    <= 4
            );
        }
    }

    #[test]
    fn rotated_content_uses_the_fractional_parent_center() {
        let sprite = opaque_test_sprite();
        let draw = |x| {
            let mut pixels = vec![0; 32 * 32 * 4];
            super::composite_sprite_rotated(
                &mut pixels,
                32,
                32,
                &sprite,
                5.5,
                5.5,
                x,
                12.25,
                0.12,
                0.2,
                1.,
            );
            pixels
        };
        let centroid = |pixels: &[u8]| {
            let total = pixels.chunks_exact(4).map(|p| f64::from(p[3])).sum::<f64>();
            pixels
                .chunks_exact(4)
                .enumerate()
                .map(|(index, p)| (index % 32) as f64 * f64::from(p[3]))
                .sum::<f64>()
                / total
        };
        let delta = centroid(&draw(12.51)) - centroid(&draw(12.49));
        assert!((delta - 0.02).abs() < 0.01, "centroid delta={delta}");
    }

    #[test]
    fn transformed_content_blur_remains_continuous_at_zero_and_fractional_radii() {
        let sprite = opaque_test_sprite();
        let draw = |blur| {
            let mut pixels = vec![0; 32 * 32 * 4];
            super::composite_sprite_rotated(
                &mut pixels,
                32,
                32,
                &sprite,
                4.,
                4.,
                16.25,
                16.5,
                0.12,
                blur,
                1.,
            );
            pixels
        };
        for (from, to) in [(0., 0.001), (0.49, 0.51), (3.49, 3.51), (5.99, 6.01)] {
            assert!(
                draw(from)
                    .chunks_exact(4)
                    .zip(draw(to).chunks_exact(4))
                    .all(|(a, b)| a[3].abs_diff(b[3]) <= 3),
                "blur {from} -> {to}"
            );
        }
    }

    #[test]
    fn fractional_raster_edges_conserve_alpha_and_color() {
        let sprite = super::TextSprite {
            width: 1,
            height: 1,
            advance: 1.,
            pixels: vec![255, 32, 0, 255],
        };
        for origin in [[2., 2.], [2.5, 2.], [2.5, 2.5], [2.25, 2.75]] {
            let mut pixels = vec![0; 8 * 8 * 4];
            super::composite_text_sprite(
                &mut pixels,
                [8, 8],
                &sprite,
                origin,
                1.,
                0.,
                1.,
                [0., 8.],
            );
            let alpha = pixels
                .chunks_exact(4)
                .map(|pixel| u32::from(pixel[3]))
                .sum::<u32>();
            assert!(alpha.abs_diff(255) <= 1, "{origin:?}: alpha={alpha}");
            for pixel in pixels.chunks_exact(4).filter(|pixel| pixel[3] > 0) {
                assert_eq!(&pixel[..3], &[255, 32, 0]);
            }
        }
    }

    #[test]
    fn spotlight_region_registers_with_fractional_base_text_and_viewport_clip() {
        let mut sprite = super::TextSprite {
            width: 12,
            height: 12,
            advance: 12.,
            pixels: vec![0; 12 * 12 * 4],
        };
        for row in 0..12 {
            sprite.pixels[(row * 12 + 5) * 4..(row * 12 + 5) * 4 + 4].copy_from_slice(&[255; 4]);
        }
        for y in [4.25, 4.49, 4.51] {
            for blur in [0., 0.49, 0.51] {
                let mut base = vec![0; 40 * 24 * 4];
                let mut bright = base.clone();
                super::composite_text_sprite(
                    &mut base,
                    [40, 24],
                    &sprite,
                    [10.25, y],
                    12.,
                    blur,
                    1.,
                    [5.5, 14.5],
                );
                super::composite_text_region(
                    &mut bright,
                    [40, 24],
                    &sprite,
                    [12.75, y],
                    2.5,
                    6.,
                    blur,
                    1.,
                    [5.5, 14.5],
                    None,
                );
                assert_eq!(base, bright, "origin y={y}, blur={blur}");
            }
        }
    }

    #[test]
    fn text_positions_clip_edges_and_blur_preserve_fractional_motion() {
        let mut sprite = super::TextSprite {
            width: 12,
            height: 12,
            advance: 12.,
            pixels: vec![0; 12 * 12 * 4],
        };
        sprite.pixels[(5 * 12 + 5) * 4..(5 * 12 + 5) * 4 + 4].copy_from_slice(&[255; 4]);
        let draw = |sprite: &super::TextSprite, x, width, blur| {
            let mut pixels = vec![0; 40 * 24 * 4];
            super::composite_text_sprite(
                &mut pixels,
                [40, 24],
                sprite,
                [x, 4.25],
                width,
                blur,
                1.,
                [0., 24.],
            );
            pixels
        };
        let centroid = |pixels: &[u8]| {
            let total = pixels.chunks_exact(4).map(|p| f64::from(p[3])).sum::<f64>();
            pixels
                .chunks_exact(4)
                .enumerate()
                .map(|(i, p)| (i % 40) as f64 * f64::from(p[3]))
                .sum::<f64>()
                / total
        };
        let a = draw(&sprite, 10.49, 12., 0.);
        let b = draw(&sprite, 10.51, 12., 0.);
        assert_ne!(a, b);
        assert!((centroid(&b) - centroid(&a) - 0.02).abs() < 0.01);
        let a = draw(&sprite, 10., 12., 0.49);
        let b = draw(&sprite, 10., 12., 0.51);
        assert!(a.iter().zip(&b).map(|(a, b)| a.abs_diff(*b)).max().unwrap() < 10);
        sprite.pixels.fill(255);
        let a = draw(&sprite, 10., 4., 0.);
        let b = draw(&sprite, 10., 4.01, 0.);
        let edge = (8 * 40 + 14) * 4 + 3;
        assert_eq!(a[edge], 0);
        assert!((1..=3).contains(&b[edge]));
        assert_eq!(draw(&sprite, 10., 4.01, 0.), b);
    }

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
    fn preview_keeps_dynamic_overlays_separate_and_falls_back_for_optical_motion() {
        use super::{EditorFrame, PointerFrame, TokenHighlight, can_preview_editor};
        let mut frame = EditorFrame {
            panel_offset_y: 0.,
            panel_rotation: 0.,
            panel_tilt_x: 0.,
            panel_tilt_y: 0.,
            panel_scale: 1.,
            panel_near_blur: 0.,
            focus_intensity: 0.,
            focus_line_y: 0.,
            focus_height: 44.,
            token_highlight: TokenHighlight {
                x: 0.,
                y: 0.,
                width: 0.,
                opacity: 0.,
            },
            pointer: PointerFrame {
                x: 0.,
                y: 0.,
                opacity: 0.,
                rotation: 0.,
                scale: 1.,
                blur: 0.,
            },
            bright_text: &[],
            inline_reveals: &[],
            squiggles: &[],
            annotations: &[],
            lines: &[],
        };
        let hidden = can_preview_editor(&frame, [1920, 1080]);
        frame.focus_line_y = 100.;
        assert_eq!(hidden, can_preview_editor(&frame, [1920, 1080]));
        frame.focus_intensity = 1.;
        assert_eq!(hidden, can_preview_editor(&frame, [1920, 1080]));
        frame.panel_rotation = 0.01;
        assert!(!can_preview_editor(&frame, [1920, 1080]));
        frame.panel_rotation = 0.;
        frame.pointer.opacity = 1.;
        assert!(!can_preview_editor(&frame, [1920, 1080]));
        frame.pointer.opacity = 0.;
        frame.token_highlight.opacity = 1.;
        frame.token_highlight.x = -150.;
        frame.token_highlight.width = 100.;
        assert!(!can_preview_editor(&frame, [1920, 1080]));
        frame.token_highlight.x = 20.;
        frame.token_highlight.y = -90.;
        assert!(!can_preview_editor(&frame, [1920, 1080]));
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
