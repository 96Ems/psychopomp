//! Stage pixels. The CPU samples channels, projects every element through the
//! camera, and emits depth-sorted signed-distance primitives; the GPU draws them
//! into an HDR target, blooms the bright light, and composites with highlight
//! rolloff, chroma, vignette, and grain (stage.wgsl, stage_post.wgsl).
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::*;
use kinograph::{
    caption::{CaptionAlign, CaptionSpanPlan},
    effects::combustion::{self, Burst},
    effects::surface,
    math::{
        Quat, Vec2, Vec3,
        curve::Polyline,
        easing::{cubic_out, quad_out},
        lerp, remap_clamp,
        shapes::{Box2, Shape, connect, sphere_ring},
        smoothstep, stops, vec2, vec3,
    },
    stage::{
        Camera, OrbPoint, StageElement, StagePlan, StatusText, orb_points, packet, shatter_offset,
    },
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
    /// Rounded rects: a reflection on the edge (x, y, radius, strength) and a
    /// pool of light in the glass, in the same form.
    light: [f32; 4],
    light_color: [f32; 4],
    pool: [f32; 4],
    pool_color: [f32; 4],
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
    shock: [f32; 4],
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
    orbs: HashMap<String, Vec<OrbPoint>>,
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
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}\n{}",
                    shader("effects/noise.wgsl", include_str!("effects/noise.wgsl")),
                    shader(
                        "effects/combustion.wgsl",
                        include_str!("effects/combustion.wgsl")
                    ),
                    shader("stage.wgsl", include_str!("stage.wgsl")),
                )
                .into(),
            ),
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
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    shader(
                        "effects/pressure.wgsl",
                        include_str!("effects/pressure.wgsl")
                    ),
                    shader("stage_post.wgsl", include_str!("stage_post.wgsl")),
                )
                .into(),
            ),
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
                StageElement::Orb { id, points, .. } => Some((id.clone(), orb_points(*points))),
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
        let look = Look::new(self.theme);
        let size = vec2(self.spec.width as f32, self.spec.height as f32);
        let scene = Scene::sample(plan, &value, time as f32, size);
        let mut painter = Painter {
            scene: &scene,
            look,
            orbs: &gpu.orbs,
            frame: StageFrame::new(&gpu.texts, look.background),
        };
        painter.backdrop(plan.post.backdrop);
        for (order, element) in plan.elements.iter().enumerate() {
            painter.element(order, element);
        }
        let (prims, points) = painter.frame.finish()?;
        let params = [
            value("post.bloom", plan.post.bloom).max(0.0),
            0.95,
            0.25,
            value("post.exposure", 1.0).max(0.0),
        ];
        let post = [
            value("post.chroma", 0.0).max(0.0),
            value("post.vignette", plan.post.vignette).clamp(0.0, 1.0),
            plan.post.grain,
            ((time * 60.0).floor() % 997.0) as f32,
        ];
        let shock = plan
            .elements
            .iter()
            .find_map(|element| {
                let StageElement::Orb { id, .. } = element else {
                    return None;
                };
                let age = scene.v(id, "burst", -1.0);
                let place = scene.placements.get(id.as_str())?;
                (0.0..2.4).contains(&age).then_some([
                    place.center.x,
                    place.center.y,
                    age,
                    place.scale,
                ])
            })
            .unwrap_or([0.0, 0.0, -1.0, 0.0]);
        self.draw_stage(gpu, &prims, &points, [params, post, shock], look.background)
    }

    /// Draw the primitives into the HDR target, bloom, and composite into the
    /// frame. `post` is the bloom parameters and the composite look.
    fn draw_stage(
        &mut self,
        gpu: &StageGpu,
        prims: &[Prim],
        points: &[[f32; 4]],
        post: [[f32; 4]; 3],
        clear: Vec3,
    ) -> Result<Vec<u8>> {
        self.queue
            .write_buffer(&gpu.prims, 0, bytemuck::cast_slice(prims));
        if !points.is_empty() {
            self.queue
                .write_buffer(&gpu.points, 0, bytemuck::cast_slice(points));
        }
        for pass in &gpu.passes {
            self.queue.write_buffer(
                &pass.uniform,
                0,
                bytemuck::bytes_of(&PostUniform {
                    texel: [pass.texel[0], pass.texel[1], 0.0, 0.0],
                    params: post[0],
                    look: post[1],
                    shock: post[2],
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
                            r: f64::from(clear.x),
                            g: f64::from(clear.y),
                            b: f64::from(clear.z),
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

/// The theme's colors in linear light.
#[derive(Clone, Copy)]
struct Look {
    theme: Theme,
    background: Vec3,
    surface: Vec3,
    raised: Vec3,
    text: Vec3,
    muted: Vec3,
}

impl Look {
    fn new(theme: Theme) -> Self {
        let palette = theme.palette();
        Self {
            theme,
            background: linear3(palette.background),
            surface: linear3(palette.surface),
            raised: linear3(palette.raised),
            text: linear3(palette.text),
            muted: linear3(palette.muted),
        }
    }

    fn tone(&self, tone: Tone) -> Vec3 {
        linear3(self.theme.tone(tone))
    }
}

fn linear3(rgb: [u8; 3]) -> Vec3 {
    Vec3::from(super::theme::linear(rgb))
}

/// Where a positioned element sits this sample.
#[derive(Clone, Copy)]
struct Placement {
    world: Vec3,
    /// Projected center on screen.
    center: Vec2,
    /// Perspective times the element's own scale (and an orb's breath).
    scale: f32,
    /// The on-screen outline beams attach to.
    outline: Shape,
}

/// A beam's path on screen and what sits at its ends.
struct Link {
    path: Polyline,
    /// World depth and on-screen scale at the `from` and `to` ends.
    depth: [f32; 2],
    scale: [f32; 2],
    /// Ends that plug into a card side and show a socket there.
    socket: [bool; 2],
    /// The source card's frame, for the light that sweeps it before drawing.
    source: Option<Box2>,
}

impl Link {
    fn far(&self) -> f32 {
        self.depth[0].max(self.depth[1])
    }

    fn scale_at(&self, fraction: f32) -> f32 {
        lerp(self.scale[0], self.scale[1], fraction)
    }
}

/// Light cast by something that moves. A reflection lights only the edges it
/// nears (a packet gathering, flying, and landing, or a drawing beam's bead); a
/// pool also enters a card's glass (the ember left at a port, the flood where a
/// packet arrives, a beam's surge on contact).
#[derive(Clone, Copy)]
struct Light {
    at: Vec2,
    tone: Tone,
    strength: f32,
    /// Falloff radius in pixels at unit scale.
    radius: f32,
    pool: bool,
    scale: f32,
}

impl Light {
    /// How strongly it lights a point `distance` pixels away.
    fn falloff(&self, distance: f32) -> f32 {
        let r = distance / (self.radius * self.scale).max(1.0);
        if self.pool {
            0.5 * (-2.0 * r * r).exp()
        } else {
            stops(r, &REFLECTION)
        }
    }

    /// The light in the form a primitive carries it.
    fn uniform(&self, look: &Look, opacity: f32) -> ([f32; 4], [f32; 4]) {
        (
            [
                self.at.x,
                self.at.y,
                self.radius * self.scale,
                self.strength * opacity,
            ],
            rgba(look.tone(self.tone), 1.0),
        )
    }
}

/// The diagrams' reflection: full at the light, 0.65 at 0.3 of its radius,
/// 0.16 at 0.7, gone at the radius. Mirrored in `stage.wgsl`.
const REFLECTION: [(f32, f32); 4] = [(0.0, 1.0), (0.3, 0.65), (0.7, 0.16), (1.0, 0.0)];
/// Diagram pixels to stage pixels: the diagrams sit about this much smaller
/// than a 1080p frame.
const DIAGRAM_SCALE: f32 = 1.4;
const REFLECTION_RADIUS: f32 = 80.0 * DIAGRAM_SCALE;

/// Opacity of a bead that travels a path: born as it leaves one end, gone as
/// it reaches the other.
fn bead(progress: f32) -> f32 {
    smoothstep(progress / 0.08) * (1.0 - smoothstep((progress - 0.92) / 0.08))
}

/// Channel values, camera, placements, and beam paths of one sample.
struct Scene<'a> {
    plan: &'a StagePlan,
    value: &'a dyn Fn(&str, f32) -> f32,
    time: f32,
    camera: Camera,
    focus: f32,
    dof: f32,
    placements: HashMap<&'a str, Placement>,
    links: HashMap<&'a str, Link>,
    lights: Vec<Light>,
}

impl<'a> Scene<'a> {
    fn sample(
        plan: &'a StagePlan,
        value: &'a dyn Fn(&str, f32) -> f32,
        time: f32,
        size: Vec2,
    ) -> Self {
        let wobble = vec2(
            (time * 47.0).sin() * 0.6 + (time * 83.0 + 1.3).sin() * 0.4,
            (time * 53.0 + 0.7).sin() * 0.6 + (time * 71.0 + 2.1).sin() * 0.4,
        );
        let position = vec3(
            value("camera.x", 0.0),
            value("camera.y", 0.0),
            value("camera.z", 0.0),
        );
        let mut scene = Self {
            plan,
            value,
            time,
            camera: Camera {
                position: position + (wobble * value("camera.shake", 0.0)).extend(0.0),
                size,
            },
            focus: value("camera.focus", 0.0),
            dof: value("camera.dof", 0.0).max(0.0),
            placements: HashMap::new(),
            links: HashMap::new(),
            lights: Vec::new(),
        };
        scene.placements = plan
            .elements
            .iter()
            .filter_map(|element| Some((element.id(), scene.place(element)?)))
            .collect();
        scene.links = plan
            .elements
            .iter()
            .filter_map(|element| match element {
                StageElement::Beam {
                    id, from, to, bend, ..
                } => Some((id.as_str(), scene.link(id, from, to, *bend)?)),
                _ => None,
            })
            .collect();
        let mut lights = Vec::new();
        for element in &plan.elements {
            scene.lights_of(element, &mut lights);
        }
        scene.lights = lights;
        scene
    }

    fn v(&self, id: &str, property: &str, default: f32) -> f32 {
        (self.value)(&format!("{id}.{property}"), default)
    }

    /// Depth-of-field blur, in pixels, of something at depth `z`.
    fn blur_at(&self, z: f32) -> f32 {
        (self.dof * (z - self.focus).abs() / 100.0).min(24.0)
    }

    /// The shell barely breathes. A pulse is light, never a discontinuous scale
    /// change: beam ports must stay attached when an arrival strikes.
    fn breath(&self) -> f32 {
        1.0 + 0.006 * (self.time * 1.4).sin()
    }

    fn place(&self, element: &StageElement) -> Option<Placement> {
        let id = element.id();
        let offset = vec3(
            self.v(id, "x", 0.0),
            self.v(id, "y", 0.0),
            self.v(id, "z", 0.0),
        );
        let world = Vec3::from(element.anchor()?) + offset;
        let (center, perspective) = self.camera.project(world)?;
        let breath = match element {
            StageElement::Orb { .. } => self.breath(),
            _ => 1.0,
        };
        let scale = perspective * self.v(id, "scale", 1.0).max(0.01) * breath;
        Some(Placement {
            world,
            center,
            scale,
            outline: element.outline(center, scale),
        })
    }

    fn link(&self, id: &str, from: &str, to: &str, bend: f32) -> Option<Link> {
        let (a, b) = (self.placements.get(from)?, self.placements.get(to)?);
        // A twang sags the curve downward, as if it had weight, then vibrates
        // back to rest.
        let downward = if (b.center - a.center).perp().y >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let bend = bend + 10.0 * downward * self.v(id, "twang", 0.0);
        // Circular ends continue beneath the shell. Occlusion hides the cap;
        // the visible wire meets the silhouette rather than a floating socket.
        let submerged = |shape| match shape {
            Shape::Circle(mut circle) => {
                circle.radius *= 0.68;
                Shape::Circle(circle)
            }
            shape => shape,
        };
        let curve = connect(
            submerged(a.outline),
            submerged(b.outline),
            bend * (a.scale + b.scale) * 0.5,
        );
        let card = |id: &str| matches!(self.plan.element(id), Some(StageElement::Card { .. }));
        Some(Link {
            path: curve.flatten(BEAM_SAMPLES),
            depth: [a.world.z, b.world.z],
            scale: [a.scale, b.scale],
            socket: [card(from), card(to)],
            source: match a.outline {
                Shape::Box(frame) if card(from) => Some(frame),
                _ => None,
            },
        })
    }
}

impl Scene<'_> {
    /// Packet contact begins at the visible shell, before the submerged endpoint.
    fn orb_contacts(&self, id: &str, place: Placement) -> Vec<(Vec3, f32, Tone, f32)> {
        let Shape::Circle(circle) = place.outline else {
            return Vec::new();
        };
        self.plan
            .elements
            .iter()
            .filter_map(|element| {
                let StageElement::Packet {
                    id: packet_id,
                    beam,
                    reverse,
                    tone,
                    ..
                } = element
                else {
                    return None;
                };
                let StageElement::Beam { from, to, .. } = self.plan.element(beam)? else {
                    return None;
                };
                if (if *reverse { from } else { to }) != id {
                    return None;
                }
                let link = self.links.get(beam.as_str())?;
                let path = if *reverse {
                    link.path.reversed()
                } else {
                    link.path.clone()
                };
                let fraction = circle.entry_fraction(&path)?;
                let since = packet::since_crossing(
                    self.v(packet_id, "age", -1.0),
                    self.v(packet_id, "flight", 0.8).max(0.05),
                    fraction,
                )?;
                if since >= 1.4 {
                    return None;
                }
                let normal = (path.at(fraction) - place.center).normalize_or(Vec2::X);
                let direction = vec3(normal.x, normal.y, -0.34).normalize();
                Some((
                    direction,
                    since,
                    *tone,
                    self.v(packet_id, "opacity", 1.0).clamp(0.0, 1.0),
                ))
            })
            .collect()
    }

    /// The lights a beam, packet, or combusting orb casts this sample.
    fn lights_of(&self, element: &StageElement, lights: &mut Vec<Light>) {
        match element {
            StageElement::Orb { id, radius, .. } => {
                let age = self.v(id, "burst", -1.0);
                let burst = Burst::sample(age);
                if burst.rim_strength == 0.0 {
                    return;
                }
                let Some(place) = self.placements.get(id.as_str()) else {
                    return;
                };
                let strength = burst.rim_strength * self.v(id, "opacity", 1.0).clamp(0.0, 1.0);
                lights.push(Light {
                    at: place.center,
                    tone: Tone::Accent,
                    strength,
                    radius: radius * 4.4,
                    pool: false,
                    scale: place.scale,
                });
            }
            StageElement::Beam { id, tone, .. } => {
                let Some(link) = self.links.get(id.as_str()) else {
                    return;
                };
                let opacity = self.v(id, "opacity", 1.0).clamp(0.0, 1.0);
                let draw = self.v(id, "draw", 1.0).clamp(0.0, 1.0);
                // The bead lights what it passes; on contact the surge pools in the target.
                let (fraction, strength, pool) = if draw < 0.999 {
                    (draw, bead(draw), false)
                } else {
                    (1.0, self.v(id, "surge", 0.0).clamp(0.0, 1.0) * 0.5, true)
                };
                if strength * opacity > 0.01 {
                    lights.push(Light {
                        at: link.path.at(fraction),
                        tone: *tone,
                        strength: strength * opacity,
                        radius: if pool { 150.0 } else { REFLECTION_RADIUS },
                        pool,
                        scale: link.scale_at(fraction),
                    });
                }
            }
            StageElement::Packet {
                id,
                beam,
                reverse,
                tone,
                ..
            } => {
                let Some(link) = self.links.get(beam.as_str()) else {
                    return;
                };
                let age = self.v(id, "age", -1.0);
                if !(0.0..packet::LIFETIME).contains(&age) {
                    return;
                }
                let flight = self.v(id, "flight", 0.8).max(0.05);
                let opacity = self.v(id, "opacity", 1.0).clamp(0.0, 1.0);
                let mut cast = |fraction: f32, strength: f32, radius: f32, pool: bool| {
                    let fraction = if *reverse { 1.0 - fraction } else { fraction };
                    if strength * opacity > 0.01 {
                        lights.push(Light {
                            at: link.path.at(fraction),
                            tone: *tone,
                            strength: strength * opacity,
                            radius,
                            pool,
                            scale: link.scale_at(fraction),
                        });
                    }
                };
                // The reflection rides the packet: it gathers at the port, flies,
                // and fades as the packet is absorbed.
                if let Some(g) = packet::gather(age) {
                    cast(0.0, cubic_out(g).powf(1.5), REFLECTION_RADIUS, false);
                }
                if packet::flight(age, flight).is_some() {
                    cast(packet::travel(age, flight), 1.0, REFLECTION_RADIUS, false);
                }
                if let Some(q) = packet::landing(age, flight) {
                    cast(1.0, (1.0 - q).powi(2), REFLECTION_RADIUS, false);
                }
                // An ember glows where it left, seeping outward as it cools.
                let t = age / packet::EMBER;
                if t < 1.0 {
                    let spread = (0.1 + 0.7 * t.sqrt()) * 210.0 * DIAGRAM_SCALE;
                    let core = 0.6 * (t / 0.04).min(1.0) * (1.0 - t).powf(0.9);
                    cast(0.0, core, spread, true);
                }
                // Light floods into whatever it reached, spreading and fading.
                if let Some(since) = packet::since_arrival(age, flight) {
                    let t = since / packet::FLOOD;
                    if t < 1.0 {
                        let travel = 1.0 - (1.0 - t).powi(4);
                        let width = 0.07 + 0.6 * travel.sqrt();
                        let fade = (-0.9 * t).exp() * (1.0 - smoothstep((t - 0.65) / 0.35));
                        let strength =
                            1.4 * smoothstep(t / 0.07) * (0.12 / (0.12 + width)).sqrt() * fade;
                        cast(1.0, strength, width * 300.0 * DIAGRAM_SCALE, true);
                    }
                }
            }
            _ => {}
        }
    }

    /// The lights that can reach `outline`.
    fn lights_on(&self, outline: Shape) -> impl Iterator<Item = &Light> + '_ {
        self.lights
            .iter()
            .filter(move |light| outline.distance(light.at) < light.radius * light.scale)
    }
}

/// Polyline styles: dash, gap, phase, and fade toward the start.
const SOLID: [f32; 4] = [0.0; 4];
const COMET: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Draws each element of one sample into its frame.
struct Painter<'a> {
    scene: &'a Scene<'a>,
    look: Look,
    orbs: &'a HashMap<String, Vec<OrbPoint>>,
    frame: StageFrame<'a>,
}

impl<'a> Painter<'a> {
    /// A faint neutral light behind the scene. Warmth comes only from the bloom
    /// of what is actually lit.
    fn backdrop(&mut self, amount: f32) {
        let size = self.scene.camera.size;
        let look = self.look;
        self.frame.prims.push(Prim {
            bbox: [0.0, 0.0, size.x, size.y],
            a: [5.0, size.x * 0.5, size.y * 0.44, size.x * 0.72],
            fill: rgba(look.background.lerp(look.raised, amount), 1.0),
            stroke: rgba(look.background, 1.0),
            ..Default::default()
        });
        self.frame.close(f32::INFINITY, 0);
    }

    fn element(&mut self, order: usize, element: &StageElement) {
        let scene = self.scene;
        let id = element.id();
        let place = scene.placements.get(id).copied();
        match (element, place) {
            (
                StageElement::Card {
                    size, status, tone, ..
                },
                Some(place),
            ) => self.card(order, id, Vec2::from(*size), status, *tone, place),
            (StageElement::Orb { radius, tone, .. }, Some(place)) => {
                self.orb(order, id, *radius, *tone, place)
            }
            (StageElement::Label { align, spans, .. }, Some(place)) => {
                self.label(order, id, *align, spans, place)
            }
            (
                StageElement::Ring {
                    radius,
                    thickness,
                    tone,
                    ..
                },
                Some(place),
            ) => self.ring(order, id, [*radius, *thickness], *tone, place),
            (StageElement::Beam { tone, .. }, _) => {
                if let Some(link) = scene.links.get(id) {
                    self.beam(order, id, *tone, link);
                }
            }
            (
                StageElement::Packet {
                    beam,
                    reverse,
                    tone,
                    ..
                },
                _,
            ) => {
                if let Some(link) = scene.links.get(beam.as_str()) {
                    self.packet(order, id, *reverse, *tone, link);
                }
            }
            // A positioned element behind the camera.
            _ => {}
        }
    }

    fn card(
        &mut self,
        order: usize,
        id: &str,
        size: Vec2,
        status: &[StatusText],
        tone: Tone,
        place: Placement,
    ) {
        let (scene, look) = (self.scene, self.look);
        let opacity = scene.v(id, "opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let glow = scene.v(id, "glow", 0.0).clamp(0.0, 1.5);
        let flash = scene.v(id, "flash", 0.0).clamp(0.0, 1.5);
        let alarm = scene.v(id, "alarm", 0.0).clamp(0.0, 1.5);
        let dim = scene.v(id, "dim", 0.0).clamp(0.0, 1.0);
        let scale = place.scale;
        let blur = scene.blur_at(place.world.z) + scene.v(id, "blur", 0.0).max(0.0) * scale;
        let own = look.tone(tone);
        // Ink and rim respond; the substrate stays dark. In linear light even
        // a modest full-card tint overwhelms the directional socket reflection.
        let lit = (flash + alarm).min(1.5);
        let light = own.lerp(look.tone(Tone::Error), alarm / (flash + alarm).max(1e-3));
        let border = look.raised.lerp(look.muted, 0.22);
        let edge = border.lerp(
            if lit > glow { light } else { own },
            (glow * 0.32 + lit * 0.28).min(0.65),
        );
        let half = size * 0.5 * scale;
        // The strongest reflection on the edge, and the strongest pool in the glass.
        let strongest = |pool: bool| {
            scene
                .lights_on(place.outline)
                .filter(|light| light.pool == pool)
                .max_by(|a, b| a.strength.total_cmp(&b.strength))
                .map_or(([0.0; 4], [0.0; 4]), |light| light.uniform(&look, opacity))
        };
        let (reflection, reflection_color) = strongest(false);
        let (pool, pool_color) = strongest(true);
        self.frame.rounded_rect(
            place.center,
            half,
            [14.0 * scale, 1.0 * scale.max(0.5)],
            blur,
            Paint {
                fill: rgba(look.surface, 0.97 * opacity * (1.0 - 0.45 * dim)),
                stroke: rgba(edge, opacity * (1.0 - 0.5 * dim)),
                glow: glow4(own * (glow * 0.018 * opacity), 7.0 * scale),
                light: reflection,
                light_color: reflection_color,
                pool,
                pool_color,
            },
        );
        // A faint inner rim catches light along the edge, like glass.
        self.frame.rounded_rect(
            place.center,
            half - 1.5 * scale,
            [12.5 * scale, 1.0],
            blur,
            Paint {
                stroke: rgba(Vec3::ONE, 0.014 * opacity * (1.0 - dim)),
                ..Default::default()
            },
        );
        let content = scene.v(id, "content", 1.0).clamp(0.0, 1.0);
        let ink = opacity * content * (1.0 - 0.55 * dim);
        let text_center = place.center + vec2(0.0, 7.0 * (1.0 - content) * scale);
        let text_blur = blur + 1.5 * (1.0 - content) * scale;
        let title_lift = if status.is_empty() { 0.0 } else { 13.0 * scale };
        self.frame.text(
            &text_key(id, "title"),
            text_center - vec2(0.0, title_lift),
            scale,
            CaptionAlign::Center,
            rgba(look.text.lerp(Vec3::ONE, (flash * 0.35).min(1.0)), ink),
            f32::MAX,
            text_blur,
        );
        if !status.is_empty() {
            // Statuses cross-fade by the fractional `status` channel.
            let index = scene
                .v(id, "status", 0.0)
                .clamp(0.0, (status.len() - 1) as f32);
            let low = index.floor() as usize;
            let high = (low + 1).min(status.len() - 1);
            for (entry, weight) in [(low, 1.0 - index.fract()), (high, index.fract())] {
                if weight <= 0.001 {
                    continue;
                }
                let color = match status[entry].tone {
                    Tone::Plain => look.muted,
                    tone => look.tone(tone),
                };
                // Separate the outgoing and incoming ink instead of showing
                // two readable words on top of each other at mid-transition.
                let visibility = smoothstep((weight - 0.2) / 0.8);
                let drift = if entry == low { -1.0 } else { 1.0 };
                self.frame.text(
                    &text_key(id, &format!("status{entry}")),
                    text_center + vec2(0.0, (19.0 + drift * 6.0 * (1.0 - weight)) * scale),
                    scale,
                    CaptionAlign::Center,
                    rgba(color, ink * visibility),
                    f32::MAX,
                    text_blur + (1.0 - weight) * 1.5,
                );
            }
        }
        self.frame.close(place.world.z, order);
    }

    fn orb(&mut self, order: usize, id: &str, radius: f32, tone: Tone, place: Placement) {
        let age = self.scene.v(id, "burst", -1.0);
        self.orb_shell(order, id, radius, tone, place, age);
        if age >= 0.12 {
            let opacity = self.scene.v(id, "opacity", 1.0).clamp(0.0, 1.0);
            if opacity > 0.001 {
                self.burst(order, id, radius, place, age, opacity);
            }
        }
    }

    fn orb_shell(
        &mut self,
        order: usize,
        id: &str,
        radius: f32,
        tone: Tone,
        mut place: Placement,
        age: f32,
    ) {
        let (scene, look) = (self.scene, self.look);
        let bursting = age >= 0.0;
        let burst = Burst::sample(age);
        let opacity = scene.v(id, "opacity", 1.0).clamp(0.0, 1.0) * burst.shell_opacity;
        if opacity <= 0.001 {
            return;
        }
        let collapse = burst.shell_scale;
        place.scale *= collapse;
        let shatter = if bursting {
            0.0
        } else {
            scene.v(id, "shatter", 0.0).clamp(0.0, 1.0)
        };
        let pulse = scene.v(id, "pulse", 0.0);
        let hurt = scene.v(id, "hurt", 0.0).clamp(0.0, 1.0);
        let spin = scene.v(id, "spin", 1.0);
        let world_radius = radius * scene.v(id, "scale", 1.0).max(0.01) * scene.breath() * collapse;
        let own = look.tone(tone);
        let red = look.tone(Tone::Error);
        let blur = scene.blur_at(place.world.z) + scene.v(id, "blur", 0.0).max(0.0) * place.scale;
        // A dark, softly feathered body occludes connections behind the shell.
        self.frame.circle(
            place.center,
            [radius * place.scale, 0.0],
            5.0 * place.scale + blur,
            Paint {
                fill: rgba(look.background, opacity * (1.0 - shatter)),
                ..Default::default()
            },
        );
        // Core light, fading as the orb breaks apart.
        let core = (0.025 + 0.075 * pulse.max(0.0)) * (1.0 - shatter) * opacity;
        self.frame.circle(
            place.center,
            [0.0, 0.0],
            0.0,
            Paint {
                glow: glow4(own.lerp(red, hurt) * core, radius * place.scale * 0.42),
                ..Default::default()
            },
        );
        let rotation = Quat::from_rotation_x(0.42)
            * Quat::from_rotation_y(scene.time * 0.14 * spin + scene.v(id, "rotation", 0.0));
        let contacts = scene.orb_contacts(id, place);
        let mut dots = self.orbs[id]
            .iter()
            .map(|point| {
                let unit = rotation * point.unit;
                let (displacement, emission) = contacts.iter().fold(
                    (0.0, Vec3::ZERO),
                    |(offset, light), (direction, age, tone, strength)| {
                        let response =
                            surface::impact(*age, unit.dot(*direction).clamp(-1.0, 1.0).acos());
                        (
                            offset + response.displacement * strength,
                            light
                                + look.tone(*tone).lerp(Vec3::ONE, 0.35)
                                    * response.light
                                    * strength,
                        )
                    },
                );
                let offset = shatter_offset(
                    OrbPoint {
                        unit,
                        seed: point.seed,
                    },
                    world_radius,
                    shatter,
                ) + unit * (displacement * world_radius / 150.0);
                (
                    place.world + offset,
                    (1.0 - unit.z) * 0.5,
                    point.seed.z,
                    emission,
                )
            })
            .collect::<Vec<_>>();
        dots.sort_by(|a, b| b.0.z.total_cmp(&a.0.z));
        let fade = (1.0 - shatter).powf(0.7);
        let lights = scene.lights_on(place.outline).collect::<Vec<_>>();
        for (point, near, seed, emission) in dots {
            let Some((center, scale)) = scene.camera.project(point) else {
                continue;
            };
            let alpha = (0.1 + 0.75 * near) * opacity * fade;
            if alpha < 0.01 {
                continue;
            }
            let lit = lights.iter().fold(Vec3::ZERO, |sum, light| {
                sum + look.tone(light.tone)
                    * (light.strength * light.falloff(center.distance(light.at)))
            });
            let color = own
                .lerp(Vec3::ONE, 0.16 * near)
                .lerp(red, (shatter * 2.4 + hurt * 0.8).min(1.0))
                + lit * 0.85
                + emission;
            self.frame.circle(
                center,
                [
                    (0.85 + 1.15 * near) * scale * (1.0 + 0.25 * shatter * seed),
                    0.0,
                ],
                blur,
                Paint {
                    fill: rgba(color * (0.6 + 0.3 * near + 0.15 * pulse), alpha),
                    glow: glow4(
                        color * (0.035 * alpha) + emission * (0.10 * alpha),
                        2.5 * scale,
                    ),
                    ..Default::default()
                },
            );
        }
        for (direction, age, tone, strength) in contacts {
            let (angle, emission) = surface::wavefront(age);
            if angle >= PI || emission * strength < 0.01 {
                continue;
            }
            let points = sphere_ring(direction, angle, 72)
                .into_iter()
                .filter_map(|unit| {
                    scene
                        .camera
                        .project(place.world + unit * world_radius)
                        .map(|(point, _)| (point, smoothstep(-unit.z / 0.20)))
                })
                .collect::<Vec<_>>();
            let ink = look.tone(tone).lerp(Vec3::ONE, 0.4);
            let energy = emission * strength * opacity;
            self.frame.trail(
                &points,
                [1.15 * place.scale, 0.0],
                Paint {
                    stroke: rgba(ink * 1.1, energy * 0.55),
                    glow: glow4(ink * energy * 0.055, 4.0 * place.scale),
                    ..Default::default()
                },
            );
        }
        self.frame.close(place.world.z, order);
    }

    /// One deterministic impact clock owns collapse, combustion, smoke, and
    /// ballistic embers. Reverse this clock to reassemble the same performance.
    fn burst(
        &mut self,
        order: usize,
        id: &str,
        radius: f32,
        place: Placement,
        age: f32,
        opacity: f32,
    ) {
        if age >= combustion::DURATION {
            return;
        }
        let scale = place.scale;
        let radius_px = radius * scale;
        self.frame.prims.push(Prim {
            bbox: [
                place.center.x - radius_px * 4.4,
                place.center.y - radius_px * 4.4,
                place.center.x + radius_px * 4.4,
                place.center.y + radius_px * 4.4,
            ],
            a: [6.0, place.center.x, place.center.y, radius_px],
            b: [age, opacity, 0.0, 0.0],
            ..Default::default()
        });
        let burst = Burst::sample(age);
        let rotation = Quat::from_rotation_x(0.42)
            * Quat::from_rotation_y(
                self.scene.time * 0.14 * self.scene.v(id, "spin", 1.0)
                    + self.scene.v(id, "rotation", 0.0),
            );
        let world_radius = radius * self.scene.v(id, "scale", 1.0).max(0.01) * self.scene.breath();
        for (index, point) in self.orbs[id].iter().enumerate() {
            let unit = rotation * point.unit;
            let ember = burst.ember(unit, point.seed);
            let anchor = place.world + unit * (world_radius * burst.shell_scale);
            let position = anchor + ember.offset;
            let Some((center, perspective)) = self.scene.camera.project(position) else {
                continue;
            };
            let ignition = burst.ignition;
            let fade = ember.opacity * opacity;
            if fade <= 0.001 {
                continue;
            }
            let hot = vec3(4.5, 1.6, 0.35).lerp(vec3(0.8, 0.025, 0.006), 1.0 - ember.heat);
            let color = self.look.tone(Tone::Accent).lerp(hot, ignition);
            if index % 4 == 0 && ignition > 0.0 {
                let tail = anchor + ember.tail_offset;
                if let Some((tail, _)) = self.scene.camera.project(tail) {
                    self.frame.polyline(
                        &Polyline::new(vec![tail, center]),
                        1.0,
                        [1.3 * perspective, 0.0],
                        Paint {
                            stroke: rgba(color, fade * 0.8),
                            glow: glow4(color * fade * 0.12, 3.0 * perspective),
                            ..Default::default()
                        },
                        COMET,
                    );
                }
            }
            self.frame.circle(
                center,
                [ember.radius * perspective, 0.0],
                0.0,
                Paint {
                    fill: rgba(color, fade),
                    glow: glow4(color * fade * ignition * 0.15, 3.0 * perspective),
                    ..Default::default()
                },
            );
        }
        self.frame.close(place.world.z, order);
    }

    fn beam(&mut self, order: usize, id: &str, tone: Tone, link: &Link) {
        let (scene, look) = (self.scene, self.look);
        let opacity = scene.v(id, "opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let draw = scene.v(id, "draw", 1.0).clamp(0.0, 1.0);
        let broken = scene.v(id, "break", 0.0).clamp(0.0, 1.0);
        let flow = scene.v(id, "flow", 0.0).clamp(0.0, 1.5);
        let emphasis = scene.v(id, "emphasis", 0.0).clamp(0.0, 1.0);
        let surge = scene.v(id, "surge", 0.0).clamp(0.0, 1.5);
        let own = look.tone(tone);
        let scale = link.scale_at(0.5);
        let blur = scene.blur_at(link.far());
        // Idle wires are matte. Only a contact surge briefly emits light.
        let color = (look.muted * 0.7)
            .lerp(own, 0.12 + 0.45 * emphasis)
            .lerp(own, 0.35 * surge.min(1.0))
            .lerp(look.tone(Tone::Error), (broken * 3.0).min(1.0));
        let line = Paint {
            stroke: rgba(
                color,
                0.75 * opacity * (1.0 - 0.65 * broken) * (1.0 + 0.3 * surge),
            ),
            glow: glow4(color * (0.05 * surge * opacity), 4.0 * scale),
            ..Default::default()
        };
        let width = 1.4 * scale * (1.0 + 0.25 * surge);
        if broken <= 0.001 {
            self.frame
                .polyline(&link.path, draw, [width, blur], line, SOLID);
        } else {
            // Snapped in the middle, both halves recoil all the way to their ends.
            let keep = 0.5 * (1.0 - broken).powf(1.5);
            for half in [link.path.slice(0.0, keep), link.path.slice(1.0 - keep, 1.0)] {
                self.frame.polyline(&half, 1.0, [width, blur], line, SOLID);
            }
        }
        if draw > 0.001 && draw < 0.999 && broken <= 0.001 {
            // A bead of light draws the wire: born as it leaves the port, gone
            // as it reaches the target.
            let light = bead(draw) * opacity;
            let head = 13.0 * DIAGRAM_SCALE * scale / link.path.length().max(1.0);
            self.frame.polyline(
                &link.path.slice(draw - head, draw),
                1.0,
                [2.8 * scale, blur],
                Paint {
                    stroke: rgba(own.lerp(Vec3::ONE, 0.45) * 1.1, light),
                    glow: glow4(own * (0.1 * light), 4.0 * scale),
                    ..Default::default()
                },
                COMET,
            );
            self.frame.circle(
                link.path.at(draw),
                [2.6 * scale, 0.0],
                blur,
                Paint {
                    fill: rgba(own.lerp(Vec3::ONE, 0.6) * 1.1, light),
                    glow: glow4(own * (0.12 * light), 4.0 * scale),
                    ..Default::default()
                },
            );
        }
        if flow > 0.001 && broken <= 0.001 && draw > 0.98 {
            // Small beads of light travel toward the `to` end.
            let bead = Paint {
                stroke: rgba(own, (flow * opacity * 0.65).min(1.0)),
                ..Default::default()
            };
            let beads = [7.0 * scale, 190.0 * scale, -scene.time * 90.0 * scale, 0.0];
            self.frame
                .polyline(&link.path, 1.0, [1.8 * scale, blur], bead, beads);
        }
        // Behind both ends, so a beam never crosses the cards it connects.
        self.frame.close(link.far() + 1.0, order);
        // Before drawing, light runs once around the source card's frame, from
        // its port back to it.
        let sweep = scene.v(id, "sweep", 0.0);
        if sweep > 0.001
            && sweep < 0.999
            && let Some(frame) = link.source
        {
            let scale = link.scale[0];
            let trace = frame.perimeter_from(link.path.at(0.0), 14.0 * scale);
            let dash = 36.0 * DIAGRAM_SCALE * scale / trace.length().max(1.0);
            let light = bead(sweep) * opacity;
            self.frame.polyline(
                &trace.slice(sweep - dash, sweep),
                1.0,
                [1.5 * DIAGRAM_SCALE * scale, blur],
                Paint {
                    stroke: rgba(own.lerp(Vec3::ONE, 0.6) * 1.3, light),
                    glow: glow4(own * (0.3 * light), 6.0 * scale),
                    ..Default::default()
                },
                COMET,
            );
            self.frame.close(link.depth[0] - 0.3, order);
        }
        // Sockets where the beam plugs into cards, in front of them. The source
        // port pops in (large and soft, then crisp) before the wire draws; the
        // target's pops with the surge.
        let popped = if draw > 0.001 {
            1.0
        } else {
            scene.v(id, "port", 0.0).clamp(0.0, 1.0)
        };
        for end in [0, 1] {
            let (shown, size, soft) = if end == 0 {
                (popped, 1.6 - 0.6 * popped, 2.0 * (1.0 - popped))
            } else {
                (f32::from(u8::from(draw > 0.999)), 1.0 + 0.5 * surge, 0.0)
            };
            if !link.socket[end] || shown <= 0.001 {
                continue;
            }
            let scale = link.scale[end];
            self.frame.circle(
                link.path.at(end as f32),
                [4.4 * scale * size, 1.3 * scale],
                blur + soft * scale,
                Paint {
                    fill: rgba(color, opacity * shown),
                    stroke: rgba(look.background, opacity * shown),
                    ..Default::default()
                },
            );
            self.frame.close(link.depth[end] - 0.25, order);
        }
    }

    /// A packet's whole life from its clock: light gathers at the port, a solid
    /// dot flies with a cooling trail, then it opens into a small ring as it is
    /// absorbed. Its light on nearby edges comes from `Scene::lights_of`.
    fn packet(&mut self, order: usize, id: &str, reverse: bool, tone: Tone, link: &Link) {
        let scene = self.scene;
        let age = scene.v(id, "age", -1.0);
        let opacity = scene.v(id, "opacity", 1.0).clamp(0.0, 1.0);
        if !(0.0..packet::LIFETIME).contains(&age) || opacity <= 0.001 {
            return;
        }
        let flight = scene.v(id, "flight", 0.8).max(0.05);
        let path = if reverse {
            link.path.reversed()
        } else {
            link.path.clone()
        };
        let scale_at =
            |fraction: f32| link.scale_at(if reverse { 1.0 - fraction } else { fraction });
        let own = self.look.tone(tone);
        // The dot is nearly white; its tone lives in the trail and its reflections.
        let ink = own.lerp(Vec3::ONE, 0.55) * 1.3;
        let dot = 4.0 * DIAGRAM_SCALE;
        if let Some(g) = packet::gather(age) {
            // A soft disc closes in on the port while the dot grows in.
            let g = cubic_out(g);
            let (port, scale) = (path.at(0.0), scale_at(0.0));
            self.frame.circle(
                port,
                [(4.0 + 14.0 * (1.0 - g)) * DIAGRAM_SCALE * scale, 0.0],
                6.0 * scale,
                Paint {
                    fill: rgba(ink * 0.7, 0.5 * (PI * g).sin() * opacity),
                    ..Default::default()
                },
            );
            self.frame.circle(
                port,
                [dot * g * scale, 0.0],
                0.0,
                Paint {
                    fill: rgba(ink, g.powf(1.5) * opacity),
                    ..Default::default()
                },
            );
        }
        self.trail(&path, age, flight, own * opacity, scale_at(0.5));
        let label_alpha = if packet::flight(age, flight).is_some() {
            let travel = packet::travel(age, flight);
            let (head, scale) = (path.at(travel), scale_at(travel));
            self.frame.circle(
                head,
                [dot * scale, 0.0],
                0.0,
                Paint {
                    fill: rgba(ink, opacity),
                    glow: glow4(own * (0.3 * opacity), 6.0 * scale),
                    ..Default::default()
                },
            );
            let label = remap_clamp(travel, [0.0, 0.1], [0.0, 1.0]);
            if link.socket[usize::from(!reverse)] {
                label
            } else {
                label * (1.0 - smoothstep((travel - 0.65) / 0.25))
            }
        } else {
            if link.socket[usize::from(!reverse)] {
                packet::landing(age, flight).map_or(0.0, |q| 1.0 - smoothstep(q / 0.4))
            } else {
                0.0
            }
        };
        if label_alpha > 0.001 {
            let travel = packet::travel(age, flight);
            let scale = scale_at(travel);
            self.frame.text(
                &text_key(id, "label"),
                path.at(travel) - vec2(0.0, 26.0 * scale),
                scale,
                CaptionAlign::Center,
                rgba(own, opacity * label_alpha),
                f32::MAX,
                0.0,
            );
        }
        if let Some(q) = packet::landing(age, flight)
            && link.socket[usize::from(!reverse)]
        {
            // The dot is the ring: a 2 px ring with a 4 px stroke looks like the
            // dot, then opens, grows a little, and fades out.
            let opening = smoothstep(q / 0.24);
            let scale = scale_at(1.0);
            self.frame.arc(
                path.at(1.0),
                [
                    (2.0 + 2.0 * opening + 9.1 * quad_out(q)) * DIAGRAM_SCALE * scale,
                    (4.0 - 2.5 * opening) * DIAGRAM_SCALE * scale,
                ],
                1.0,
                0.0,
                Paint {
                    stroke: rgba(
                        ink,
                        (1.0 - 0.734 * opening) * (1.0 - q).powf(2.52) * opacity,
                    ),
                    ..Default::default()
                },
            );
        }
        self.frame.close(link.far() + 0.5, order);
    }

    /// Every point the packet crossed within the cooling time, dimming with
    /// the time since it crossed: long and bright mid-flight, short near the
    /// ends, and still cooling after it lands.
    fn trail(&mut self, path: &Polyline, age: f32, flight: f32, color: Vec3, scale: f32) {
        let head = packet::travel(age, flight);
        let tail = packet::travel(age - packet::COOLING, flight);
        if head - tail <= 1e-4 {
            return;
        }
        let stretch = path.slice(tail, head);
        let length = path.length().max(1.0);
        let points = stretch
            .points()
            .iter()
            .zip(stretch.lengths())
            .map(|(point, along)| {
                let fraction = (tail * length + along) / length;
                let since = packet::since_crossing(age, flight, fraction);
                (*point, since.map_or(0.0, packet::heat))
            })
            .collect::<Vec<_>>();
        self.frame.trail(
            &points,
            [1.6 * DIAGRAM_SCALE * scale, 0.0],
            Paint {
                stroke: rgba(color * 1.25, 1.0),
                glow: glow4(color * 0.2, 5.0 * scale),
                ..Default::default()
            },
        );
    }

    fn label(
        &mut self,
        order: usize,
        id: &str,
        align: CaptionAlign,
        spans: &[CaptionSpanPlan],
        place: Placement,
    ) {
        let (scene, look) = (self.scene, self.look);
        let opacity = scene.v(id, "opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let typed = scene.v(id, "typed", 1.0).clamp(0.0, 1.0);
        let blur = scene.blur_at(place.world.z);
        let parts = spans
            .iter()
            .enumerate()
            .filter(|(_, span)| !span.text.is_empty())
            .map(|(index, span)| (text_key(id, &format!("span{index}")), span))
            .collect::<Vec<_>>();
        let widths = parts
            .iter()
            .map(|(key, _)| self.frame.width(key, place.scale))
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
        let mut x = place.center.x
            - match align {
                CaptionAlign::Left => 0.0,
                CaptionAlign::Center => total * 0.5,
                CaptionAlign::Right => total,
            };
        for ((key, span), width) in parts.iter().zip(&widths) {
            let count = span.text.chars().count();
            let shown = remaining.min(count);
            if shown > 0 {
                let reveal = if shown == count {
                    f32::MAX
                } else {
                    width * shown as f32 / count as f32
                };
                self.frame.text(
                    key,
                    vec2(x, place.center.y),
                    place.scale,
                    CaptionAlign::Left,
                    rgba(look.tone(span.tone), opacity),
                    reveal,
                    blur,
                );
            }
            remaining -= shown;
            x += width;
        }
        self.frame.close(place.world.z - 0.5, order);
    }

    /// `size` is the radius and thickness.
    fn ring(&mut self, order: usize, id: &str, size: [f32; 2], tone: Tone, place: Placement) {
        let scene = self.scene;
        let opacity = scene.v(id, "opacity", 1.0).clamp(0.0, 1.0);
        let expand = scene.v(id, "expand", 0.0).clamp(0.0, 1.0);
        let alpha = opacity * (1.0 - expand);
        if alpha <= 0.001 {
            return;
        }
        let own = self.look.tone(tone);
        self.frame.arc(
            place.center,
            [
                size[0] * (1.0 + 1.3 * expand) * place.scale,
                size[1] * place.scale,
            ],
            scene.v(id, "sweep", 1.0).clamp(0.0, 1.0),
            scene.blur_at(place.world.z),
            Paint {
                stroke: rgba(own, alpha),
                glow: glow4(own * (0.035 * alpha), 5.0 * place.scale),
                ..Default::default()
            },
        );
        self.frame.close(place.world.z, order);
    }
}

/// Colors of one primitive: straight linear RGBA fill and stroke, and glow as
/// linear RGB intensity with its radius in pixels.
#[derive(Clone, Copy, Default)]
struct Paint {
    fill: [f32; 4],
    stroke: [f32; 4],
    glow: [f32; 4],
    /// Rounded rects only: see `Prim::light` and `Prim::pool`.
    light: [f32; 4],
    light_color: [f32; 4],
    pool: [f32; 4],
    pool_color: [f32; 4],
}

/// The primitives of one sample, grouped into depth-sorted layers.
struct StageFrame<'a> {
    texts: &'a HashMap<String, AtlasText>,
    prims: Vec<Prim>,
    points: Vec<[f32; 4]>,
    /// Depth, declaration order, and primitive range of each layer.
    layers: Vec<(f32, usize, usize, usize)>,
    /// First primitive of the layer being drawn.
    open: usize,
    /// Background color for the soft backing behind text.
    shade: Vec3,
}

impl<'a> StageFrame<'a> {
    fn new(texts: &'a HashMap<String, AtlasText>, shade: Vec3) -> Self {
        Self {
            texts,
            prims: Vec::new(),
            points: Vec::new(),
            layers: Vec::new(),
            open: 0,
            shade,
        }
    }

    /// End the layer drawn since the last close, at world depth `depth`.
    fn close(&mut self, depth: f32, order: usize) {
        self.layers
            .push((depth, order, self.open, self.prims.len()));
        self.open = self.prims.len();
    }

    /// Primitives far to near, keeping declaration order among equal depths.
    fn finish(mut self) -> Result<(Vec<Prim>, Vec<[f32; 4]>)> {
        self.layers
            .sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let prims = self
            .layers
            .iter()
            .flat_map(|&(_, _, first, last)| self.prims[first..last].iter().copied())
            .collect::<Vec<_>>();
        if prims.len() > MAX_PRIMS || self.points.len() > MAX_POINTS {
            bail!(
                "stage frame needs {} primitives and {} points; limits are {MAX_PRIMS} and {MAX_POINTS}",
                prims.len(),
                self.points.len()
            );
        }
        Ok((prims, self.points))
    }

    /// `shape` is the corner radius and border width.
    fn rounded_rect(&mut self, center: Vec2, half: Vec2, shape: [f32; 2], blur: f32, paint: Paint) {
        let pad = half + Vec2::splat(paint.glow[3] * 4.0 + blur + 2.0);
        self.prims.push(Prim {
            bbox: [
                center.x - pad.x,
                center.y - pad.y,
                center.x + pad.x,
                center.y + pad.y,
            ],
            a: [0.0, center.x, center.y, shape[0]],
            b: [half.x, half.y, shape[1], blur],
            fill: paint.fill,
            stroke: paint.stroke,
            glow: paint.glow,
            light: paint.light,
            light_color: paint.light_color,
            pool: paint.pool,
            pool_color: paint.pool_color,
            ..Default::default()
        });
    }

    /// `shape` is the radius and border width.
    fn circle(&mut self, center: Vec2, shape: [f32; 2], blur: f32, paint: Paint) {
        let pad = shape[0] + paint.glow[3] * 4.0 + blur + 1.0;
        self.prims.push(Prim {
            bbox: [
                center.x - pad,
                center.y - pad,
                center.x + pad,
                center.y + pad,
            ],
            a: [1.0, center.x, center.y, shape[0]],
            b: [shape[1], 0.0, 0.0, blur],
            fill: paint.fill,
            stroke: paint.stroke,
            glow: paint.glow,
            ..Default::default()
        });
    }

    /// A ring, or an arc clockwise from twelve o'clock over `sweep` of a turn.
    /// `shape` is the radius and thickness.
    fn arc(&mut self, center: Vec2, shape: [f32; 2], sweep: f32, blur: f32, paint: Paint) {
        let pad = shape[0] + shape[1] + paint.glow[3] * 4.0 + blur + 2.0;
        self.prims.push(Prim {
            bbox: [
                center.x - pad,
                center.y - pad,
                center.x + pad,
                center.y + pad,
            ],
            a: [2.0, center.x, center.y, shape[0]],
            b: [shape[1], -FRAC_PI_2, TAU * sweep, blur],
            stroke: paint.stroke,
            glow: paint.glow,
            ..Default::default()
        });
    }

    /// `path` drawn to `fraction` of its length. `stroke` is the width and blur;
    /// `style` is dash, gap, phase, and fade toward the start.
    fn polyline(
        &mut self,
        path: &Polyline,
        fraction: f32,
        stroke: [f32; 2],
        paint: Paint,
        style: [f32; 4],
    ) {
        self.path(
            path,
            &vec![1.0; path.points().len()],
            fraction,
            stroke,
            paint,
            style,
        );
    }

    /// A path whose points each carry a heat that scales its light, such as a
    /// trail cooling behind a packet.
    fn trail(&mut self, points: &[(Vec2, f32)], stroke: [f32; 2], paint: Paint) {
        let path = Polyline::new(points.iter().map(|(point, _)| *point).collect());
        let heat = points.iter().map(|(_, heat)| *heat).collect::<Vec<_>>();
        self.path(&path, &heat, 1.0, stroke, paint, SOLID);
    }

    fn path(
        &mut self,
        path: &Polyline,
        heat: &[f32],
        fraction: f32,
        stroke: [f32; 2],
        paint: Paint,
        style: [f32; 4],
    ) {
        let points = path.points();
        if points.len() < 2 || fraction <= 0.0 || paint.stroke[3] <= 0.001 {
            return;
        }
        let first = self.points.len();
        self.points.extend(
            points
                .iter()
                .zip(path.lengths())
                .zip(heat)
                .map(|((point, along), heat)| [point.x, point.y, *along, *heat]),
        );
        let (low, high) = points
            .iter()
            .fold((Vec2::MAX, Vec2::MIN), |(low, high), point| {
                (low.min(*point), high.max(*point))
            });
        let pad = Vec2::splat(stroke[0] + paint.glow[3] * 4.0 + stroke[1] + 2.0);
        let (low, high) = (low - pad, high + pad);
        self.prims.push(Prim {
            bbox: [low.x, low.y, high.x, high.y],
            a: [
                3.0,
                stroke[0],
                path.length() * fraction.clamp(0.0, 1.0),
                stroke[1],
            ],
            b: style,
            stroke: paint.stroke,
            glow: paint.glow,
            uv: [first as f32, points.len() as f32, 0.0, 0.0],
            ..Default::default()
        });
    }

    /// On-screen width of an atlas string's ink at `scale`.
    fn width(&self, key: &str, scale: f32) -> f32 {
        self.texts.get(key).map_or(0.0, |text| {
            (text.rect[2] - 4.0).max(0.0) * scale / TEXT_RASTER
        })
    }

    /// One atlas string at `scale`, anchored on its vertical center. `reveal`
    /// clips it to a width, for typing.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        key: &str,
        at: Vec2,
        scale: f32,
        align: CaptionAlign,
        fill: [f32; 4],
        reveal: f32,
        blur: f32,
    ) {
        let Some(text) = self.texts.get(key) else {
            return;
        };
        if fill[3] <= 0.001 {
            return;
        }
        let size = vec2(text.rect[2], text.rect[3]) * (scale / TEXT_RASTER);
        let ink = self.width(key, scale);
        let left = at.x
            - match align {
                CaptionAlign::Left => 0.0,
                CaptionAlign::Center => ink * 0.5,
                CaptionAlign::Right => ink,
            };
        let top = at.y - size.y * 0.5;
        let uv = [
            text.rect[0],
            text.rect[1],
            text.rect[0] + text.rect[2],
            text.rect[1] + text.rect[3],
        ];
        let reveal = reveal.min(size.x + 4.0);
        // A soft dark backing keeps text legible where it crosses beams and glow.
        self.prims.push(Prim {
            bbox: [
                left - 8.0,
                top - 8.0,
                left + size.x + 8.0,
                top + size.y + 8.0,
            ],
            a: [4.0, left, top, 5.0 + blur * 0.5],
            b: [size.x, size.y, reveal, 0.0],
            fill: rgba(self.shade, fill[3] * 0.85),
            uv,
            ..Default::default()
        });
        self.prims.push(Prim {
            bbox: [
                left - 2.0,
                top - 2.0,
                left + size.x + 2.0,
                top + size.y + 2.0,
            ],
            a: [4.0, left, top, blur * 0.5],
            b: [size.x, size.y, reveal, 0.0],
            fill,
            uv,
            ..Default::default()
        });
    }
}

fn rgba(color: Vec3, alpha: f32) -> [f32; 4] {
    color.extend(alpha.clamp(0.0, 1.0)).to_array()
}

fn glow4(color: Vec3, radius: f32) -> [f32; 4] {
    color.extend(radius).to_array()
}

#[cfg(test)]
mod tests {
    use super::Scene;
    use kinograph::math::vec2;
    use kinograph::stage::StagePlan;

    use crate::render::HeadlessRenderer;

    #[test]
    fn an_orb_pulse_does_not_displace_attached_ports() {
        let plan: StagePlan = serde_json::from_value(serde_json::json!({
            "elements": [
                { "kind": "orb", "id": "service", "at": [960, 480, 0], "radius": 150 },
                { "kind": "card", "id": "client", "at": [420, 300, -40], "size": [300, 110], "title": "client" },
                { "kind": "beam", "id": "link", "from": "client", "to": "service" }
            ]
        }))
        .unwrap();
        let quiet = |_: &str, default| default;
        let pulse = |property: &str, default| {
            if property == "service.pulse" {
                1.0
            } else {
                default
            }
        };
        let before = Scene::sample(&plan, &quiet, 1.0, vec2(1920.0, 1080.0));
        let impact = Scene::sample(&plan, &pulse, 1.0, vec2(1920.0, 1080.0));
        assert_eq!(
            before.placements["service"].scale,
            impact.placements["service"].scale
        );
        assert_eq!(
            before.links["link"].path.points(),
            impact.links["link"].path.points()
        );
        let link = &before.links["link"];
        let orb = before.placements["service"];
        assert!(
            link.path.at(1.0).distance(orb.center) < 150.0 * orb.scale * 0.8,
            "the cap is submerged beneath the orb's occluding body"
        );
    }

    #[test]
    fn combustion_light_is_local_and_cools_with_the_burst_clock() {
        let plan: StagePlan = serde_json::from_value(serde_json::json!({
            "elements": [{ "kind": "orb", "id": "service", "at": [960, 480, 0], "radius": 150 }]
        }))
        .unwrap();
        let mut strengths = Vec::new();
        for age in [-1.0, 0.0, 0.3, 1.0, 3.0] {
            let values = |property: &str, default| {
                if property == "service.burst" {
                    age
                } else {
                    default
                }
            };
            let scene = Scene::sample(&plan, &values, 2.0, vec2(1920.0, 1080.0));
            strengths.push(scene.lights.iter().map(|light| light.strength).sum::<f32>());
            for light in &scene.lights {
                assert!(light.falloff(400.0) > 0.0);
                assert_eq!(light.falloff(900.0), 0.0, "far rims remain dark");
                assert!(
                    !light.pool,
                    "combustion lights rims without washing the card"
                );
            }
        }
        assert_eq!([strengths[0], strengths[1], strengths[4]], [0.0; 3]);
        assert!(strengths[2] > strengths[3] && strengths[3] > 0.0);
    }

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
                    "probe.age" => 0.74,
                    "probe.flight" => 0.8,
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
        let burst = |renderer: &mut HeadlessRenderer, age: f32| {
            renderer
                .render_stage(&plan, &gpu, 2.0, |property, default| match property {
                    "service.burst" => age,
                    _ => default,
                })
                .unwrap()
        };
        let fire = burst(&mut renderer, 0.45);
        let intact = burst(&mut renderer, -1.0);
        assert!(
            burst(&mut renderer, 0.0) == intact,
            "entering Burst preserves the intact pixels"
        );
        let early = burst(&mut renderer, 0.0001);
        let error = intact
            .iter()
            .zip(&early)
            .map(|(a, b)| a.abs_diff(*b) as f64)
            .sum::<f64>()
            / intact.len() as f64;
        assert!(
            error < 0.01,
            "compression begins continuously, mean byte error {error}"
        );
        let smoke = burst(&mut renderer, 2.8);
        assert!(fire != smoke, "combustion cools into smoke");
        assert!(
            burst(&mut renderer, 0.45) == fire,
            "reverse sampling reconstructs the fire"
        );
        assert!(
            burst(&mut renderer, -1.0) != smoke,
            "negative age restores the intact orb"
        );
    }
}
