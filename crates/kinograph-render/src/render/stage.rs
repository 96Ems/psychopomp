//! Stage pixels. The CPU samples channels, projects every element through the
//! camera, and emits depth-sorted signed-distance primitives; the GPU draws them
//! into an HDR target, blooms the bright light, and composites with highlight
//! rolloff, chroma, vignette, and grain (stage.wgsl, stage_post.wgsl).
use std::collections::HashMap;
use std::f32::consts::TAU;

use super::*;
use kinograph::{
    caption::CaptionAlign,
    stage::{Camera, StageElement, StagePlan, beam_point, fibonacci_sphere, shatter_offset},
    tone::Tone,
};

const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const MAX_PRIMS: usize = 24_000;
const MAX_POINTS: usize = 48_000;
const BLOOM_LEVELS: usize = 5;
/// Text rasterizes at twice its size so camera push-ins stay crisp.
const TEXT_RASTER: f32 = 2.0;
const BEAM_SAMPLES: usize = 48;

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct Prim {
    bbox: [f32; 4],
    a: [f32; 4],
    b: [f32; 4],
    fill: [f32; 4],
    stroke: [f32; 4],
    glow: [f32; 4],
    uv: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    texel: [f32; 4],
    params: [f32; 4],
    look: [f32; 4],
}

struct AtlasText {
    /// x, y, width, height in atlas texels.
    rect: [f32; 4],
}

#[derive(Clone, Copy, PartialEq)]
enum PassKind {
    Prefilter,
    Down,
    Up,
    Composite,
}

struct PostPass {
    kind: PassKind,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    texel: [f32; 2],
    /// Index into the bloom chain, or `None` for the final target.
    target: Option<usize>,
}

/// Unit-sphere points and per-point seeds for one orb.
type OrbPoints = Vec<([f32; 3], [f32; 3])>;

pub(crate) struct StageGpu {
    primitives: wgpu::RenderPipeline,
    primitive_binding: wgpu::BindGroup,
    prims: wgpu::Buffer,
    points: wgpu::Buffer,
    hdr: wgpu::TextureView,
    bloom: Vec<wgpu::TextureView>,
    prefilter: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    passes: Vec<PostPass>,
    texts: HashMap<String, AtlasText>,
    orbs: HashMap<String, OrbPoints>,
}

/// A shader source: the file under `KINOGRAPH_SHADER_DIR` when set (live
/// editing without recompiling), otherwise the compiled-in copy.
fn shader(file: &str, builtin: &'static str) -> std::borrow::Cow<'static, str> {
    if let Some(dir) = std::env::var_os("KINOGRAPH_SHADER_DIR")
        && let Ok(source) = std::fs::read_to_string(std::path::Path::new(&dir).join(file))
    {
        return source.into();
    }
    builtin.into()
}

fn text_key(element: &str, part: &str) -> String {
    format!("{element}#{part}")
}

impl HeadlessRenderer {
    pub(crate) fn prepare_stage(&mut self, plan: &StagePlan) -> Result<StageGpu> {
        let texts = self.stage_atlas(plan)?;
        let (atlas_view, atlas_size, rects) = texts;
        let device = &self.device;
        let (width, height) = (self.spec.width, self.spec.height);
        let texture = |label: &str, size: [u32; 2], format: wgpu::TextureFormat| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0].max(1),
                        height: size[1].max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let hdr = texture("stage HDR", [width, height], HDR_FORMAT);
        let sizes = (0..BLOOM_LEVELS)
            .map(|level| [width >> (level + 1), height >> (level + 1)])
            .collect::<Vec<_>>();
        let bloom = sizes
            .iter()
            .map(|size| texture("stage bloom level", *size, HDR_FORMAT))
            .collect::<Vec<_>>();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stage linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let buffer = |label: &str, size: usize, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let globals = buffer(
            "stage globals",
            std::mem::size_of::<Globals>(),
            wgpu::BufferUsages::UNIFORM,
        );
        let prims = buffer(
            "stage primitives",
            MAX_PRIMS * std::mem::size_of::<Prim>(),
            wgpu::BufferUsages::STORAGE,
        );
        let points = buffer(
            "stage polyline points",
            MAX_POINTS * 16,
            wgpu::BufferUsages::STORAGE,
        );
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty,
            count: None,
        };
        let uniform_ty = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let storage_ty = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let texture_ty = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let sampler_ty = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let both = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let primitive_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage primitives"),
            entries: &[
                entry(0, both, uniform_ty),
                entry(1, both, storage_ty),
                entry(2, fragment, storage_ty),
                entry(3, fragment, texture_ty),
                entry(4, fragment, sampler_ty),
            ],
        });
        let primitive_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage primitives"),
            layout: &primitive_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: prims.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: points.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let primitive_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stage primitives"),
            source: wgpu::ShaderSource::Wgsl(shader("stage.wgsl", include_str!("stage.wgsl"))),
        });
        let premultiplied = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let primitives = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stage primitives"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[Some(&primitive_layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &primitive_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &primitive_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(premultiplied),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage post"),
            entries: &[
                entry(0, fragment, uniform_ty),
                entry(1, fragment, texture_ty),
                entry(2, fragment, sampler_ty),
                entry(3, fragment, texture_ty),
            ],
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stage post"),
            source: wgpu::ShaderSource::Wgsl(shader(
                "stage_post.wgsl",
                include_str!("stage_post.wgsl"),
            )),
        });
        let post_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&post_layout)],
            immediate_size: 0,
        });
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let post = |entry_point: &str, format, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry_point),
                layout: Some(&post_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &post_shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &post_shader,
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let prefilter = post("prefilter", HDR_FORMAT, None);
        let down = post("down", HDR_FORMAT, None);
        let up = post("up", HDR_FORMAT, Some(additive));
        let composite = post("composite", FORMAT, None);

        // Passes: prefilter HDR → level 0, down 0→1…, up …→0, composite.
        let mut passes = Vec::new();
        let mut add = |kind,
                       source: &wgpu::TextureView,
                       second: &wgpu::TextureView,
                       texel: [f32; 2],
                       target| {
            let uniform = buffer(
                "stage post pass",
                std::mem::size_of::<PostUniform>(),
                wgpu::BufferUsages::UNIFORM,
            );
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("stage post pass"),
                layout: &post_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(second),
                    },
                ],
            });
            passes.push(PostPass {
                kind,
                bind,
                uniform,
                texel,
                target,
            });
        };
        let texel = |size: [u32; 2]| [1.0 / size[0].max(1) as f32, 1.0 / size[1].max(1) as f32];
        // The second texture is only read by the composite; other passes bind the
        // atlas there so no pass reads the texture it writes.
        add(
            PassKind::Prefilter,
            &hdr,
            &atlas_view,
            texel([width, height]),
            Some(0),
        );
        for level in 1..BLOOM_LEVELS {
            add(
                PassKind::Down,
                &bloom[level - 1],
                &atlas_view,
                texel(sizes[level - 1]),
                Some(level),
            );
        }
        for level in (1..BLOOM_LEVELS).rev() {
            add(
                PassKind::Up,
                &bloom[level],
                &atlas_view,
                texel(sizes[level]),
                Some(level - 1),
            );
        }
        add(
            PassKind::Composite,
            &hdr,
            &bloom[0],
            texel([width, height]),
            None,
        );

        self.queue.write_buffer(
            &globals,
            0,
            bytemuck::bytes_of(&Globals {
                viewport: [width as f32, height as f32, atlas_size[0], atlas_size[1]],
            }),
        );
        let orbs = plan
            .elements
            .iter()
            .filter_map(|element| match element {
                StageElement::Orb { id, points, .. } => {
                    Some((id.clone(), fibonacci_sphere(*points)))
                }
                _ => None,
            })
            .collect();
        Ok(StageGpu {
            primitives,
            primitive_binding,
            prims,
            points,
            hdr,
            bloom,
            prefilter,
            down,
            up,
            composite,
            passes,
            texts: rects,
            orbs,
        })
    }

    /// Rasterize every string the stage can show into one R8 coverage atlas.
    fn stage_atlas(
        &mut self,
        plan: &StagePlan,
    ) -> Result<(wgpu::TextureView, [f32; 2], HashMap<String, AtlasText>)> {
        let mut strings: Vec<(String, String, f32)> = Vec::new();
        for element in &plan.elements {
            match element {
                StageElement::Card {
                    id, title, status, ..
                } => {
                    strings.push((text_key(id, "title"), title.clone(), 26.0));
                    for (index, entry) in status.iter().enumerate() {
                        strings.push((
                            text_key(id, &format!("status{index}")),
                            entry.text.clone(),
                            18.0,
                        ));
                    }
                }
                StageElement::Packet { id, label, .. } if !label.is_empty() => {
                    strings.push((text_key(id, "label"), label.clone(), 19.0));
                }
                StageElement::Label {
                    id, size, spans, ..
                } => {
                    for (index, span) in spans.iter().enumerate() {
                        if !span.text.is_empty() {
                            strings.push((
                                text_key(id, &format!("span{index}")),
                                span.text.clone(),
                                *size,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        let sprites = strings
            .iter()
            .map(|(_, text, size)| {
                let raster = size * TEXT_RASTER;
                let line = (raster * 1.35).ceil();
                let attrs = Attrs::new()
                    .family(Family::Name("CommitMono"))
                    .color(Color::rgb(255, 255, 255));
                let width = ((text.chars().count() as f32 * raster * 0.7) as u32 + 64).min(4096);
                make_sprite(
                    &mut self.font_system,
                    &mut self.swash_cache,
                    vec![(text.as_str(), attrs.clone())],
                    attrs,
                    Metrics::new(raster, line),
                    width,
                    line as u32,
                )
            })
            .collect::<Vec<_>>();
        // Shelf packing, rows of the tallest sprite.
        let atlas_width = 4096_u32;
        let mut placements = Vec::with_capacity(sprites.len());
        let (mut x, mut y, mut row) = (1_u32, 1_u32, 0_u32);
        for sprite in &sprites {
            let w = (sprite.advance.ceil() as u32 + 4).min(sprite.width);
            if x + w + 1 > atlas_width {
                x = 1;
                y += row + 2;
                row = 0;
            }
            placements.push((x, y, w, sprite.height));
            x += w + 2;
            row = row.max(sprite.height);
        }
        let atlas_height = (y + row + 2).max(4).next_power_of_two();
        if atlas_height > self.device.limits().max_texture_dimension_2d {
            bail!("stage text atlas exceeds device limits");
        }
        let mut pixels = vec![0_u8; (atlas_width * atlas_height) as usize];
        for (sprite, &(px, py, w, h)) in sprites.iter().zip(&placements) {
            for row in 0..h {
                for column in 0..w {
                    pixels[((py + row) * atlas_width + px + column) as usize] =
                        sprite.pixels[((row * sprite.width + column) * 4 + 3) as usize];
                }
            }
        }
        let atlas = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stage text atlas"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            atlas.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas_width),
                rows_per_image: Some(atlas_height),
            },
            atlas.size(),
        );
        let rects = strings
            .into_iter()
            .zip(placements)
            .map(|((key, _, _), (x, y, w, h))| {
                (
                    key,
                    AtlasText {
                        rect: [x as f32, y as f32, w as f32, h as f32],
                    },
                )
            })
            .collect();
        Ok((
            atlas.create_view(&Default::default()),
            [atlas_width as f32, atlas_height as f32],
            rects,
        ))
    }

    pub(crate) fn render_stage(
        &mut self,
        plan: &StagePlan,
        gpu: &StageGpu,
        time: f64,
        value: impl Fn(&str, f32) -> f32,
    ) -> Result<Vec<u8>> {
        let palette = self.theme.palette();
        let lin = |rgb: [u8; 3]| super::theme::linear(rgb);
        let background = lin(palette.background);
        let mut frame = StageFrame {
            prims: Vec::new(),
            points: Vec::new(),
            width: self.spec.width as f32,
            height: self.spec.height as f32,
            shade: [0.0; 3],
        };
        let t = time as f32;

        // Backdrop: a faint neutral light behind the scene. Warmth comes from
        // the bloom of what is actually lit.
        let backdrop = plan.post.backdrop;
        let lift = mix3(background, lin(palette.raised), backdrop);
        frame.shade = background;
        frame.prims.push(Prim {
            bbox: [0.0, 0.0, frame.width, frame.height],
            a: [
                5.0,
                frame.width * 0.5,
                frame.height * 0.44,
                frame.width * 0.72,
            ],
            fill: rgba(lift, 1.0),
            stroke: rgba(background, 1.0),
            ..Default::default()
        });

        let shake = value("camera.shake", 0.0);
        let camera = Camera {
            x: value("camera.x", 0.0)
                + shake * ((t * 47.0).sin() * 0.6 + (t * 83.0 + 1.3).sin() * 0.4),
            y: value("camera.y", 0.0)
                + shake * ((t * 53.0 + 0.7).sin() * 0.6 + (t * 71.0 + 2.1).sin() * 0.4),
            z: value("camera.z", 0.0),
            width: frame.width,
            height: frame.height,
        };
        let focus = value("camera.focus", 0.0);
        let dof = value("camera.dof", 0.0).max(0.0);
        let blur_at = |z: f32| (dof * (z - focus).abs() / 100.0).min(24.0);

        // World position of positioned elements, including their offsets.
        let world = |element: &StageElement| -> Option<[f32; 3]> {
            let at = element.anchor()?;
            let id = element.id();
            Some([
                at[0] + value(&format!("{id}.x"), 0.0),
                at[1] + value(&format!("{id}.y"), 0.0),
                at[2] + value(&format!("{id}.z"), 0.0),
            ])
        };

        let mut layers: Vec<(f32, usize, usize, usize)> = Vec::new();
        for (order, element) in plan.elements.iter().enumerate() {
            let id = element.id();
            let v = |property: &str, default: f32| value(&format!("{id}.{property}"), default);
            let first = frame.prims.len();
            let depth = match element {
                StageElement::Card {
                    size, status, tone, ..
                } => {
                    let position = world(element).expect("card has a position");
                    let opacity = v("opacity", 1.0).clamp(0.0, 1.0);
                    let Some((center, scale)) = camera.project(position) else {
                        continue;
                    };
                    if opacity > 0.001 {
                        let scale = scale * v("scale", 1.0).max(0.01);
                        let blur = blur_at(position[2]);
                        let glow = v("glow", 0.0).clamp(0.0, 1.5);
                        let flash = v("flash", 0.0).clamp(0.0, 1.5);
                        let alarm = v("alarm", 0.0).clamp(0.0, 1.5);
                        let dim = v("dim", 0.0).clamp(0.0, 1.0);
                        let tone_rgb = lin(self.theme.tone(*tone));
                        let red = lin(self.theme.tone(Tone::Error));
                        let surface = lin(palette.surface);
                        let border = mix3(lin(palette.raised), lin(palette.muted), 0.35);
                        let half = [size[0] * 0.5 * scale, size[1] * 0.5 * scale];
                        let glow_radius = 26.0 * scale;
                        let pad = glow_radius * 4.0 + blur + 2.0;
                        // Alarm is a red flash, independent of the card's own tone.
                        let accent_rgb =
                            mix3(tone_rgb, red, (alarm / (flash + alarm).max(1e-3)).min(1.0));
                        let lit = (flash + alarm).min(1.5);
                        frame.prims.push(Prim {
                            bbox: [
                                center[0] - half[0] - pad,
                                center[1] - half[1] - pad,
                                center[0] + half[0] + pad,
                                center[1] + half[1] + pad,
                            ],
                            a: [0.0, center[0], center[1], 14.0 * scale],
                            b: [half[0], half[1], 1.5 * scale.max(0.5), blur],
                            fill: rgba(
                                mix3(surface, accent_rgb, 0.22 * lit),
                                0.97 * opacity * (1.0 - 0.45 * dim),
                            ),
                            stroke: rgba(
                                mix3(
                                    border,
                                    if lit > glow { accent_rgb } else { tone_rgb },
                                    (glow + lit).min(1.0),
                                ),
                                opacity * (1.0 - 0.5 * dim),
                            ),
                            glow: glow4(
                                add3(
                                    scale3(tone_rgb, glow * 1.3 * opacity * (1.0 - alarm.min(1.0))),
                                    scale3(accent_rgb, lit * 0.9 * opacity),
                                ),
                                glow_radius,
                            ),
                            ..Default::default()
                        });
                        // A faint inner rim catches light along the edge, like glass.
                        let rim = [half[0] - 1.5 * scale, half[1] - 1.5 * scale];
                        frame.prims.push(Prim {
                            bbox: [
                                center[0] - half[0],
                                center[1] - half[1],
                                center[0] + half[0],
                                center[1] + half[1],
                            ],
                            a: [0.0, center[0], center[1], 12.5 * scale],
                            b: [rim[0], rim[1], 1.0, blur],
                            stroke: rgba([1.0, 1.0, 1.0], 0.045 * opacity * (1.0 - dim)),
                            ..Default::default()
                        });
                        let text_alpha = opacity * (1.0 - 0.55 * dim);
                        let has_status = !status.is_empty();
                        let title_y = center[1] - if has_status { 13.0 * scale } else { 0.0 };
                        frame.text(
                            gpu,
                            &text_key(id, "title"),
                            26.0,
                            [center[0], title_y],
                            scale,
                            CaptionAlign::Center,
                            rgba(lin(palette.text), text_alpha),
                            f32::MAX,
                            blur,
                        );
                        if has_status {
                            let index = v("status", 0.0).clamp(0.0, (status.len() - 1) as f32);
                            let low = index.floor() as usize;
                            let blend = index - low as f32;
                            for (entry_index, weight) in
                                [(low, 1.0 - blend), ((low + 1).min(status.len() - 1), blend)]
                            {
                                if weight <= 0.001 {
                                    continue;
                                }
                                let entry = &status[entry_index];
                                let color = if entry.tone == Tone::Plain {
                                    lin(palette.muted)
                                } else {
                                    lin(self.theme.tone(entry.tone))
                                };
                                frame.text(
                                    gpu,
                                    &text_key(id, &format!("status{entry_index}")),
                                    18.0,
                                    [center[0], center[1] + 19.0 * scale],
                                    scale,
                                    CaptionAlign::Center,
                                    rgba(color, text_alpha * weight),
                                    f32::MAX,
                                    blur,
                                );
                            }
                        }
                    }
                    position[2]
                }
                StageElement::Orb { radius, tone, .. } => {
                    let position = world(element).expect("orb has a position");
                    let opacity = v("opacity", 1.0).clamp(0.0, 1.0);
                    if opacity > 0.001 {
                        let scale_value = v("scale", 1.0).max(0.01);
                        let shatter = v("shatter", 0.0).clamp(0.0, 1.0);
                        let pulse = v("pulse", 0.0);
                        let hurt = v("hurt", 0.0).clamp(0.0, 1.0);
                        let spin = v("spin", 1.0);
                        let tone_rgb = lin(self.theme.tone(*tone));
                        let red = lin(self.theme.tone(Tone::Error));
                        let breathe = 1.0 + 0.015 * (t * 2.1).sin() + 0.08 * pulse;
                        let yaw = t * 0.35 * spin;
                        let (cy, sy) = (yaw.cos(), yaw.sin());
                        let (cp, sp) = (0.42_f32.cos(), 0.42_f32.sin());
                        let blur = blur_at(position[2]);
                        // Core light, fading as the orb breaks apart.
                        if let Some((center, scale)) = camera.project(position) {
                            let r = radius * scale * scale_value;
                            let core = (0.2 + 0.45 * pulse.max(0.0)) * (1.0 - shatter) * opacity;
                            frame.prims.push(Prim {
                                bbox: [
                                    center[0] - r * 2.4,
                                    center[1] - r * 2.4,
                                    center[0] + r * 2.4,
                                    center[1] + r * 2.4,
                                ],
                                a: [1.0, center[0], center[1], 0.0],
                                glow: glow4(scale3(mix3(tone_rgb, red, hurt), core), r * 0.42),
                                ..Default::default()
                            });
                        }
                        let mut dots = gpu.orbs[id]
                            .iter()
                            .map(|&(unit, seed)| {
                                let x1 = unit[0] * cy + unit[2] * sy;
                                let z1 = -unit[0] * sy + unit[2] * cy;
                                let rotated = [x1, unit[1] * cp - z1 * sp, unit[1] * sp + z1 * cp];
                                let near = (1.0 - rotated[2]) * 0.5;
                                let offset = shatter_offset(
                                    rotated,
                                    seed,
                                    radius * breathe * scale_value,
                                    shatter,
                                );
                                (
                                    [
                                        position[0] + offset[0],
                                        position[1] + offset[1],
                                        position[2] + offset[2],
                                    ],
                                    near,
                                    seed,
                                )
                            })
                            .collect::<Vec<_>>();
                        dots.sort_by(|a, b| b.0[2].total_cmp(&a.0[2]));
                        let fade = (1.0 - shatter).powf(0.7);
                        for (point, near, seed) in dots {
                            let Some((p, scale)) = camera.project(point) else {
                                continue;
                            };
                            let alpha = (0.2 + 0.8 * near) * opacity * fade;
                            if alpha < 0.01 {
                                continue;
                            }
                            let r = (1.3 + 2.1 * near) * scale * (1.0 + 0.5 * shatter * seed[2]);
                            let white = [1.0, 1.0, 1.0];
                            let color = mix3(
                                mix3(tone_rgb, white, 0.16 * near),
                                red,
                                (shatter * 2.4 + hurt * 0.8).min(1.0),
                            );
                            let emissive = scale3(color, 0.8 + 0.45 * near);
                            let glow_radius = 4.0 * scale;
                            let pad = r + glow_radius * 4.0 + blur + 1.0;
                            frame.prims.push(Prim {
                                bbox: [p[0] - pad, p[1] - pad, p[0] + pad, p[1] + pad],
                                a: [1.0, p[0], p[1], r],
                                b: [0.0, 0.0, 0.0, blur],
                                fill: rgba(emissive, alpha),
                                glow: glow4(scale3(color, 0.35 * alpha), glow_radius),
                                ..Default::default()
                            });
                        }
                    }
                    position[2]
                }
                StageElement::Beam {
                    from,
                    to,
                    bend,
                    tone,
                    ..
                } => {
                    let ends = plan
                        .element(from)
                        .and_then(&world)
                        .zip(plan.element(to).and_then(&world));
                    let opacity = v("opacity", 1.0).clamp(0.0, 1.0);
                    let Some((a, b)) = ends else { continue };
                    let depth = (a[2] + b[2]) * 0.5 + 1.0;
                    if opacity > 0.001 {
                        let draw = v("draw", 1.0).clamp(0.0, 1.0);
                        let broken = v("break", 0.0).clamp(0.0, 1.0);
                        let flow = v("flow", 0.0).clamp(0.0, 1.5);
                        let emphasis = v("emphasis", 0.0).clamp(0.0, 1.0);
                        let tone_rgb = lin(self.theme.tone(*tone));
                        let red = lin(self.theme.tone(Tone::Error));
                        let samples = (0..=BEAM_SAMPLES)
                            .filter_map(|i| {
                                camera.project(beam_point(
                                    a,
                                    b,
                                    *bend,
                                    i as f32 / BEAM_SAMPLES as f32,
                                ))
                            })
                            .collect::<Vec<_>>();
                        if samples.len() >= 2 {
                            let scale =
                                samples.iter().map(|(_, s)| s).sum::<f32>() / samples.len() as f32;
                            let screen = samples.iter().map(|(p, _)| *p).collect::<Vec<_>>();
                            let base = mix3(
                                scale3(lin(palette.muted), 0.7),
                                tone_rgb,
                                0.35 + 0.65 * emphasis,
                            );
                            let color = mix3(base, red, (broken * 3.0).min(1.0));
                            let alpha = 0.7 * opacity * (1.0 - 0.65 * broken);
                            let glow = glow4(
                                scale3(color, 0.22 * (0.4 + emphasis) * opacity),
                                9.0 * scale,
                            );
                            let blur = blur_at(depth);
                            if broken <= 0.001 {
                                frame.polyline(
                                    &screen,
                                    draw,
                                    2.2 * scale,
                                    rgba(color, alpha),
                                    glow,
                                    blur,
                                    [0.0; 4],
                                );
                            } else {
                                // Snap in the middle; both halves recoil all the way to their ends.
                                let keep = 0.5 * (1.0 - broken).powf(1.5);
                                frame.polyline(
                                    &screen,
                                    keep,
                                    2.2 * scale,
                                    rgba(color, alpha),
                                    glow,
                                    blur,
                                    [0.0; 4],
                                );
                                let reversed = screen.iter().rev().copied().collect::<Vec<_>>();
                                frame.polyline(
                                    &reversed,
                                    keep,
                                    2.2 * scale,
                                    rgba(color, alpha),
                                    glow,
                                    blur,
                                    [0.0; 4],
                                );
                            }
                            if flow > 0.001 && broken <= 0.001 && draw > 0.98 {
                                let bright = scale3(tone_rgb, 1.9);
                                frame.polyline(
                                    &screen,
                                    1.0,
                                    3.0 * scale,
                                    rgba(bright, (flow * opacity).min(1.0)),
                                    glow4(scale3(tone_rgb, 0.9 * flow * opacity), 10.0 * scale),
                                    blur,
                                    [6.0 * scale, 38.0 * scale, -t * 160.0 * scale, 0.0],
                                );
                            }
                        }
                    }
                    depth
                }
                StageElement::Packet {
                    beam,
                    reverse,
                    label,
                    tone,
                    ..
                } => {
                    let Some(StageElement::Beam { from, to, bend, .. }) = plan.element(beam) else {
                        continue;
                    };
                    let ends = plan
                        .element(from)
                        .and_then(&world)
                        .zip(plan.element(to).and_then(&world));
                    let Some((a, b)) = ends else { continue };
                    let opacity = v("opacity", 0.0).clamp(0.0, 1.0);
                    let travel = v("travel", 0.0).clamp(0.0, 1.0);
                    let impact = v("impact", 0.0).clamp(0.0, 1.0);
                    let tone_rgb = lin(self.theme.tone(*tone));
                    let at = |u: f32| beam_point(a, b, *bend, if *reverse { 1.0 - u } else { u });
                    let head = at(travel);
                    if opacity > 0.001 {
                        // Comet trail: the last stretch of path, brightest at the head.
                        let tail = (travel - 0.2).max(0.0);
                        let trail = (0..=14)
                            .filter_map(|i| {
                                camera.project(at(tail + (travel - tail) * i as f32 / 14.0))
                            })
                            .map(|(p, _)| p)
                            .collect::<Vec<_>>();
                        if let Some((p, scale)) = camera.project(head) {
                            if trail.len() >= 2 && travel > tail {
                                frame.polyline(
                                    &trail,
                                    1.0,
                                    3.4 * scale,
                                    rgba(scale3(tone_rgb, 1.7), 0.9 * opacity),
                                    glow4(scale3(tone_rgb, 0.6 * opacity), 10.0 * scale),
                                    0.0,
                                    [0.0, 0.0, 0.0, 1.0],
                                );
                            }
                            let r = 5.5 * scale;
                            let glow_radius = 16.0 * scale;
                            let pad = r + glow_radius * 4.0;
                            frame.prims.push(Prim {
                                bbox: [p[0] - pad, p[1] - pad, p[0] + pad, p[1] + pad],
                                a: [1.0, p[0], p[1], r],
                                fill: rgba(scale3(mix3(tone_rgb, [1.0; 3], 0.45), 2.2), opacity),
                                glow: glow4(scale3(tone_rgb, 1.3 * opacity), glow_radius),
                                ..Default::default()
                            });
                            if !label.is_empty() {
                                let label_alpha = opacity * (travel * 8.0).min(1.0);
                                let color = if *tone == Tone::Plain {
                                    lin(palette.text)
                                } else {
                                    tone_rgb
                                };
                                frame.text(
                                    gpu,
                                    &text_key(id, "label"),
                                    19.0,
                                    [p[0], p[1] - 28.0 * scale],
                                    scale,
                                    CaptionAlign::Center,
                                    rgba(color, label_alpha),
                                    f32::MAX,
                                    0.0,
                                );
                            }
                        }
                    }
                    if impact > 0.001
                        && impact < 0.999
                        && let Some((p, scale)) = camera.project(at(1.0))
                    {
                        let r = (10.0 + 90.0 * impact) * scale;
                        let fade = (1.0 - impact).powf(1.5);
                        let pad = r + 60.0 * scale;
                        frame.prims.push(Prim {
                            bbox: [p[0] - pad, p[1] - pad, p[0] + pad, p[1] + pad],
                            a: [2.0, p[0], p[1], r],
                            b: [(1.0 + 3.0 * (1.0 - impact)) * scale, 0.0, TAU, 0.0],
                            stroke: rgba(scale3(tone_rgb, 1.6), fade),
                            glow: glow4(scale3(tone_rgb, 0.8 * fade), 12.0 * scale),
                            ..Default::default()
                        });
                    }
                    head[2] - 2.0
                }
                StageElement::Label {
                    size, align, spans, ..
                } => {
                    let position = world(element).expect("label has a position");
                    let opacity = v("opacity", 1.0).clamp(0.0, 1.0);
                    if opacity > 0.001
                        && let Some((anchor, scale)) = camera.project(position)
                    {
                        let scale = scale * v("scale", 1.0).max(0.01);
                        let typed = v("typed", 1.0).clamp(0.0, 1.0);
                        let blur = blur_at(position[2]);
                        let parts = spans
                            .iter()
                            .enumerate()
                            .filter(|(_, span)| !span.text.is_empty())
                            .map(|(index, span)| (text_key(id, &format!("span{index}")), span))
                            .collect::<Vec<_>>();
                        let widths = parts
                            .iter()
                            .map(|(key, _)| {
                                gpu.texts.get(key).map_or(0.0, |text| {
                                    (text.rect[2] - 4.0).max(0.0) / TEXT_RASTER * scale
                                })
                            })
                            .collect::<Vec<_>>();
                        let total: f32 = widths.iter().sum();
                        let chars: usize = parts
                            .iter()
                            .map(|(_, span)| span.text.chars().count())
                            .sum();
                        let mut remaining = if typed >= 1.0 {
                            usize::MAX
                        } else {
                            (typed * chars as f32 + 1e-3).floor() as usize
                        };
                        let mut x = match align {
                            CaptionAlign::Left => anchor[0],
                            CaptionAlign::Center => anchor[0] - total * 0.5,
                            CaptionAlign::Right => anchor[0] - total,
                        };
                        for ((key, span), width) in parts.iter().zip(&widths) {
                            let count = span.text.chars().count();
                            let shown = remaining.min(count);
                            let color = if span.tone == Tone::Plain {
                                lin(palette.text)
                            } else {
                                lin(self.theme.tone(span.tone))
                            };
                            if shown > 0 {
                                let reveal = if shown == count {
                                    f32::MAX
                                } else {
                                    width * shown as f32 / count as f32
                                };
                                frame.text(
                                    gpu,
                                    key,
                                    *size,
                                    [x, anchor[1]],
                                    scale,
                                    CaptionAlign::Left,
                                    rgba(color, opacity),
                                    reveal,
                                    blur,
                                );
                            }
                            remaining -= shown;
                            x += width;
                        }
                    }
                    position[2] - 0.5
                }
                StageElement::Ring {
                    radius,
                    thickness,
                    tone,
                    ..
                } => {
                    let position = world(element).expect("ring has a position");
                    let opacity = v("opacity", 1.0).clamp(0.0, 1.0);
                    let expand = v("expand", 0.0).clamp(0.0, 1.0);
                    let alpha = opacity * (1.0 - expand);
                    if alpha > 0.001
                        && let Some((center, scale)) = camera.project(position)
                    {
                        let scale = scale * v("scale", 1.0).max(0.01);
                        let sweep = v("sweep", 1.0).clamp(0.0, 1.0);
                        let tone_rgb = lin(self.theme.tone(*tone));
                        let r = radius * (1.0 + 1.3 * expand) * scale;
                        let pad = r + thickness * scale + 60.0 * scale;
                        frame.prims.push(Prim {
                            bbox: [
                                center[0] - pad,
                                center[1] - pad,
                                center[0] + pad,
                                center[1] + pad,
                            ],
                            a: [2.0, center[0], center[1], r],
                            b: [
                                thickness * scale,
                                -std::f32::consts::FRAC_PI_2,
                                TAU * sweep,
                                blur_at(position[2]),
                            ],
                            stroke: rgba(tone_rgb, alpha),
                            glow: glow4(scale3(tone_rgb, 0.45 * alpha), 10.0 * scale),
                            ..Default::default()
                        });
                    }
                    position[2]
                }
            };
            layers.push((depth, order, first, frame.prims.len()));
        }

        // Far elements first; the backdrop stays at the bottom.
        layers.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut prims = Vec::with_capacity(frame.prims.len());
        prims.push(frame.prims[0]);
        for (_, _, first, last) in &layers {
            prims.extend_from_slice(&frame.prims[*first..*last]);
        }
        if prims.len() > MAX_PRIMS || frame.points.len() > MAX_POINTS {
            bail!(
                "stage frame needs {} primitives and {} points; limits are {MAX_PRIMS} and {MAX_POINTS}",
                prims.len(),
                frame.points.len()
            );
        }
        self.queue
            .write_buffer(&gpu.prims, 0, bytemuck::cast_slice(&prims));
        if !frame.points.is_empty() {
            self.queue
                .write_buffer(&gpu.points, 0, bytemuck::cast_slice(&frame.points));
        }

        let bloom = value("post.bloom", plan.post.bloom).max(0.0);
        let look = [
            value("post.chroma", 0.0).max(0.0),
            value("post.vignette", plan.post.vignette).clamp(0.0, 1.0),
            plan.post.grain,
            ((time * 60.0).floor() % 997.0) as f32,
        ];
        let exposure = value("post.exposure", 1.0).max(0.0);
        for pass in &gpu.passes {
            self.queue.write_buffer(
                &pass.uniform,
                0,
                bytemuck::bytes_of(&PostUniform {
                    texel: [pass.texel[0], pass.texel[1], 0.0, 0.0],
                    params: [bloom, 0.95, 0.25, exposure],
                    look,
                }),
            );
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("stage sample"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stage primitives"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &gpu.hdr,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(background[0]),
                            g: f64::from(background[1]),
                            b: f64::from(background[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&gpu.primitives);
            pass.set_bind_group(0, &gpu.primitive_binding, &[]);
            pass.draw(0..4, 0..prims.len() as u32);
        }
        for post in &gpu.passes {
            let target = post.target.map_or(&self.view, |level| &gpu.bloom[level]);
            let pipeline = match post.kind {
                PassKind::Prefilter => &gpu.prefilter,
                PassKind::Down => &gpu.down,
                PassKind::Up => &gpu.up,
                PassKind::Composite => &gpu.composite,
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stage post"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if post.kind == PassKind::Up {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &post.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.read_frame(encoder)
    }
}

struct StageFrame {
    prims: Vec<Prim>,
    points: Vec<[f32; 4]>,
    width: f32,
    height: f32,
    /// Background color for the soft backing behind text.
    shade: [f32; 3],
}

impl StageFrame {
    /// A polyline through `screen`, drawn to `fraction` of its length.
    /// `style` is dash, gap, phase, fade.
    #[allow(clippy::too_many_arguments)]
    fn polyline(
        &mut self,
        screen: &[[f32; 2]],
        fraction: f32,
        width: f32,
        stroke: [f32; 4],
        glow: [f32; 4],
        blur: f32,
        style: [f32; 4],
    ) {
        if screen.len() < 2 || fraction <= 0.0 || stroke[3] <= 0.001 {
            return;
        }
        let first = self.points.len();
        let mut total = 0.0;
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for (index, point) in screen.iter().enumerate() {
            if index > 0 {
                let previous = screen[index - 1];
                total += (point[0] - previous[0]).hypot(point[1] - previous[1]);
            }
            self.points.push([point[0], point[1], total, 0.0]);
            bounds = [
                bounds[0].min(point[0]),
                bounds[1].min(point[1]),
                bounds[2].max(point[0]),
                bounds[3].max(point[1]),
            ];
        }
        let pad = width + glow[3] * 4.0 + blur + 2.0;
        self.prims.push(Prim {
            bbox: [
                bounds[0] - pad,
                bounds[1] - pad,
                bounds[2] + pad,
                bounds[3] + pad,
            ],
            a: [3.0, width, total * fraction.clamp(0.0, 1.0), blur],
            b: style,
            stroke,
            glow,
            uv: [first as f32, screen.len() as f32, 0.0, 0.0],
            ..Default::default()
        });
    }

    /// One atlas string. `at` is the anchor on the text's vertical center.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        gpu: &StageGpu,
        key: &str,
        size: f32,
        at: [f32; 2],
        scale: f32,
        align: CaptionAlign,
        fill: [f32; 4],
        reveal: f32,
        blur: f32,
    ) {
        let Some(text) = gpu.texts.get(key) else {
            return;
        };
        if fill[3] <= 0.001 {
            return;
        }
        let draw = scale * size / (size * TEXT_RASTER);
        let width = text.rect[2] * draw;
        let height = text.rect[3] * draw;
        let left = match align {
            CaptionAlign::Left => at[0],
            CaptionAlign::Center => at[0] - (text.rect[2] - 4.0) * draw * 0.5,
            CaptionAlign::Right => at[0] - (text.rect[2] - 4.0) * draw,
        };
        let top = at[1] - height * 0.5;
        let uv = [
            text.rect[0],
            text.rect[1],
            text.rect[0] + text.rect[2],
            text.rect[1] + text.rect[3],
        ];
        // A soft dark backing keeps text legible where it crosses beams and glow.
        self.prims.push(Prim {
            bbox: [
                left - 8.0,
                top - 8.0,
                left + width + 8.0,
                top + height + 8.0,
            ],
            a: [4.0, left, top, 5.0 + blur * 0.5],
            b: [width, height, reveal.min(width + 4.0), 0.0],
            fill: rgba(self.shade, fill[3] * 0.85),
            uv,
            ..Default::default()
        });
        self.prims.push(Prim {
            bbox: [
                left - 2.0,
                top - 2.0,
                left + width + 2.0,
                top + height + 2.0,
            ],
            a: [4.0, left, top, blur * 0.5],
            b: [width, height, reveal.min(width + 4.0), 0.0],
            fill,
            uv,
            ..Default::default()
        });
    }
}

fn rgba(rgb: [f32; 3], alpha: f32) -> [f32; 4] {
    [rgb[0], rgb[1], rgb[2], alpha.clamp(0.0, 1.0)]
}

fn glow4(rgb: [f32; 3], radius: f32) -> [f32; 4] {
    [rgb[0], rgb[1], rgb[2], radius]
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}

fn scale3(rgb: [f32; 3], k: f32) -> [f32; 3] {
    rgb.map(|c| c * k)
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

#[cfg(test)]
mod tests {
    use kinograph::stage::StagePlan;

    use crate::render::HeadlessRenderer;

    #[test]
    #[ignore = "requires a headless GPU; stage frames are pure functions of time and channels"]
    fn stage_frames_are_deterministic_and_respond_to_channels() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(crate::render::RenderSpec {
            width: 1920,
            height: 1080,
            font_path: std::path::PathBuf::from(crate::scenes::FONT_PATH),
            file_name: "stage-proof".into(),
        }))
        .unwrap();
        let plan: StagePlan = serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "orb", "id": "service", "at": [960, 480, 0], "radius": 150, "points": 400 },
                { "kind": "card", "id": "client", "at": [420, 300, -40], "size": [300, 110], "title": "client" },
                { "kind": "beam", "id": "link", "from": "client", "to": "service" },
                { "kind": "packet", "id": "probe", "beam": "link", "label": "GET" }
            ]
        }))
        .unwrap();
        let gpu = renderer.prepare_stage(&plan).unwrap();
        let draw = |renderer: &mut HeadlessRenderer, time: f64, shatter: f32, dolly: f32| {
            renderer
                .render_stage(&plan, &gpu, time, |property, default| match property {
                    "service.shatter" => shatter,
                    "camera.z" => dolly,
                    "probe.opacity" => 1.0,
                    "probe.travel" => 0.5,
                    _ => default,
                })
                .unwrap()
        };
        let first = draw(&mut renderer, 1.0, 0.0, 0.0);
        draw(&mut renderer, 3.0, 1.0, 200.0);
        assert!(
            draw(&mut renderer, 1.0, 0.0, 0.0) == first,
            "sampling order cannot change a frame"
        );
        assert!(
            draw(&mut renderer, 1.0, 0.6, 0.0) != first,
            "shattering reaches pixels"
        );
        assert!(
            draw(&mut renderer, 1.0, 0.0, 120.0) != first,
            "the camera dolly reaches pixels"
        );
        assert!(
            draw(&mut renderer, 1.2, 0.0, 0.0) != first,
            "the orb spins with time"
        );
    }
}
