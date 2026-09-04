//! An opaque connected 3D grid with depth-tested labels. This is intentionally a
//! concrete diagram pass, not a mesh/material/scene-graph abstraction.
use super::*;

const SAMPLES: u32 = 4;
const TILE: [u32; 2] = [256, 128];
const INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4];

pub struct GridItemFrame<'a> {
    pub label: &'a str,
    pub detail: &'a str,
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub color: [f32; 3],
    pub presence: f32,
    pub reveal: [f32; 3],
    pub trim: [f32; 3],
    pub fill: [f32; 3],
    pub label_opacity: f32,
    pub emphasis: f32,
    pub group: bool,
    pub heading: bool,
}

pub struct GridFrame<'a> {
    pub items: &'a [GridItemFrame<'a>],
    pub yaw: f32,
    pub pitch: f32,
    pub scale: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    center: [f32; 4],
    size: [f32; 4],
    color: [f32; 4],
    atlas: [f32; 4],
    label: [f32; 4],
    reveal: [f32; 4],
    trim: [f32; 4],
    fill: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Camera {
    viewport: [f32; 4],
    orbit: [f32; 4],
    depth: [f32; 4],
}

pub(super) struct GridRenderer {
    pipeline: wgpu::RenderPipeline,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    uniform: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: usize,
    binding: wgpu::BindGroup,
    labels: Vec<(String, String, bool)>,
    advances: Vec<f32>,
    atlas_rows: u32,
    atlas_columns: u32,
}

impl HeadlessRenderer {
    pub fn render_grid(&mut self, frame: GridFrame<'_>) -> Result<Vec<u8>> {
        // The recipe's immutable catalog warms all label resources before the
        // window opens, including cells that start hidden.
        if self.grid_renderer.as_ref().is_none_or(|grid| {
            !grid
                .labels
                .iter()
                .map(|(label, detail, heading)| (label.as_str(), detail.as_str(), *heading))
                .eq(frame
                    .items
                    .iter()
                    .map(|item| (item.label, item.detail, item.heading || item.group)))
        }) {
            self.grid_renderer = Some(GridRenderer::new(
                &self.device,
                &self.queue,
                &self.spec,
                &mut self.font_system,
                &mut self.swash_cache,
                frame.items,
            ));
        }
        let grid = self.grid_renderer.as_ref().expect("prepared grid renderer");
        let mut instances = Vec::with_capacity(frame.items.len());
        for (index, item) in frame.items.iter().enumerate() {
            if item.presence <= 0.00001 || item.reveal.iter().any(|v| *v <= 0.00001) {
                continue;
            }
            let label_scale = ((item.size[0] - 16.) / grid.advances[index].max(1.)).min(1.);
            instances.push(Instance {
                center: [
                    item.center[0],
                    item.center[1],
                    item.center[2],
                    item.presence,
                ],
                size: [
                    item.size[0],
                    item.size[1],
                    item.size[2],
                    if item.group {
                        1.
                    } else if item.heading {
                        2.
                    } else {
                        0.
                    },
                ],
                color: [
                    item.color[0],
                    item.color[1],
                    item.color[2],
                    item.emphasis * item.presence,
                ],
                atlas: [
                    (index as u32 % grid.atlas_columns) as f32 / grid.atlas_columns as f32,
                    (index as u32 / grid.atlas_columns) as f32 / grid.atlas_rows as f32,
                    1. / grid.atlas_columns as f32,
                    1. / grid.atlas_rows as f32,
                ],
                label: [
                    TILE[0] as f32 * label_scale,
                    TILE[1] as f32 * label_scale,
                    0.,
                    item.label_opacity,
                ],
                reveal: [item.reveal[0], item.reveal[1], item.reveal[2], 0.],
                trim: [item.trim[0], item.trim[1], item.trim[2], 0.],
                fill: [item.fill[0], item.fill[1], item.fill[2], item.presence],
            });
        }
        assert!(instances.len() <= grid.capacity);
        self.queue
            .write_buffer(&grid.instances, 0, bytemuck::cast_slice(&instances));
        let bounds = visible_bounds(&frame).unwrap_or([0.; 6]);
        let center = [(bounds[0] + bounds[2]) / 2., (bounds[1] + bounds[3]) / 2.];
        self.queue.write_buffer(
            &grid.uniform,
            0,
            bytemuck::bytes_of(&Camera {
                viewport: [
                    self.spec.width as f32,
                    self.spec.height as f32,
                    0.5 - center[0] * frame.scale / self.spec.width as f32,
                    0.5 + center[1] * frame.scale / self.spec.height as f32,
                ],
                orbit: [frame.yaw, frame.pitch, frame.scale, 0.],
                // Fit depth to sampled geometry too, rather than spending most
                // of the depth buffer's precision on empty space.
                depth: [
                    (bounds[4] + bounds[5]) / 2.,
                    1. / (bounds[5] - bounds[4] + 2.),
                    0.,
                    0.,
                ],
            }),
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("keyed grid sample"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth-tested grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &grid.color,
                    depth_slice: None,
                    resolve_target: Some(&self.view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.009,
                            g: 0.012,
                            b: 0.020,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &grid.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&grid.pipeline);
            pass.set_bind_group(0, &grid.binding, &[]);
            pass.set_vertex_buffer(0, grid.instances.slice(..));
            pass.draw(0..36, 0..instances.len() as u32);
        }
        self.read_frame(encoder)
    }
}

impl GridRenderer {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        spec: &RenderSpec,
        fonts: &mut FontSystem,
        cache: &mut SwashCache,
        items: &[GridItemFrame<'_>],
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("grid.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("instanced keyed grid"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &INSTANCE_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let attachment = |format, label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: spec.width,
                        height: spec.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: SAMPLES,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = attachment(FORMAT, "grid multisample color");
        let depth = attachment(wgpu::TextureFormat::Depth32Float, "grid depth");
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid orthographic orbit"),
            size: std::mem::size_of::<Camera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let capacity = items.len().max(1);
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid instances"),
            size: (capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas_columns = ((items.len() as f32 * TILE[1] as f32 / TILE[0] as f32)
            .sqrt()
            .ceil() as u32)
            .clamp(1, device.limits().max_texture_dimension_2d / TILE[0]);
        let atlas_rows = (items.len() as u32).div_ceil(atlas_columns).max(1);
        let atlas_width = atlas_columns * TILE[0];
        let atlas_height = atlas_rows * TILE[1];
        let mut pixels = vec![0_u8; (atlas_width * atlas_height * 4) as usize];
        let mut advances = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let sprite = label_sprite(
                fonts,
                cache,
                item.label,
                item.detail,
                item.heading || item.group,
            );
            advances.push(sprite.advance);
            let base_x = index as u32 % atlas_columns * TILE[0];
            let base_y = index as u32 / atlas_columns * TILE[1];
            for y in 0..TILE[1] {
                for x in 0..TILE[0] {
                    let source = ((y * TILE[0] + x) * 4) as usize;
                    let target = (((base_y + y) * atlas_width + base_x + x) * 4) as usize;
                    pixels[target..target + 4].copy_from_slice(&sprite.pixels[source..source + 4]);
                }
            }
        }
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("grid face label atlas"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            atlas.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas_width * 4),
                rows_per_image: Some(atlas_height),
            },
            atlas.size(),
        );
        let view = atlas.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid camera and labels"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            pipeline,
            color,
            depth,
            uniform,
            instances,
            capacity,
            binding,
            labels: items
                .iter()
                .map(|item| {
                    (
                        item.label.to_owned(),
                        item.detail.to_owned(),
                        item.heading || item.group,
                    )
                })
                .collect(),
            advances,
            atlas_rows,
            atlas_columns,
        }
    }
}

fn label_sprite(
    fonts: &mut FontSystem,
    cache: &mut SwashCache,
    primary: &str,
    detail: &str,
    heading: bool,
) -> TextSprite {
    let mut pixels = vec![0; (TILE[0] * TILE[1] * 4) as usize];
    let mut advance = 0_f32;
    for (text, size, height, top) in if heading {
        [(primary, 24., 32, 48), ("", 18., 28, 96)]
    } else {
        [(primary, 78., 96, 0), (detail, 18., 28, 96)]
    } {
        if text.is_empty() {
            continue;
        }
        let mut rasterize = |size| {
            let attrs = Attrs::new()
                .family(Family::Name("CommitMono"))
                .color(Color::rgb(255, 255, 255));
            make_sprite(
                fonts,
                cache,
                vec![(text, attrs.clone())],
                attrs,
                Metrics::new(size, height as f32),
                TILE[0],
                height,
            )
        };
        let mut line = rasterize(size);
        let available = (TILE[0] - 8) as f32;
        if line.advance > available {
            line = rasterize(size * available / line.advance);
        }
        advance = advance.max(line.advance);
        let left = (TILE[0] as f32 - line.advance).max(0.) as u32 / 2;
        for y in 0..height {
            for x in 0..TILE[0] - left {
                let from = ((y * TILE[0] + x) * 4) as usize;
                let to = (((y + top) * TILE[0] + x + left) * 4) as usize;
                pixels[to..to + 4].copy_from_slice(&line.pixels[from..from + 4]);
            }
        }
    }
    // Lines are already centered inside this atlas tile.
    TextSprite {
        width: TILE[0],
        height: TILE[1],
        advance,
        pixels,
    }
}

fn project(point: [f32; 3], yaw: f32, pitch: f32) -> [f32; 3] {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let [x, y, z] = point;
    let turned = [cy * x + sy * z, y, -sy * x + cy * z];
    [
        turned[0],
        cp * turned[1] - sp * turned[2],
        sp * turned[1] + cp * turned[2],
    ]
}

/// Bounds of sampled *cell geometry*, not the final catalog or side headings.
/// This uses the same fractional clipping and projection as the vertex shader.
fn visible_bounds(frame: &GridFrame<'_>) -> Option<[f32; 6]> {
    let mut bounds: Option<[f32; 6]> = None;
    for item in frame.items.iter().filter(|item| {
        !item.group
            && !item.heading
            && item.presence > 0.00001
            && item.reveal.iter().all(|v| *v > 0.00001)
    }) {
        for corner in 0..8 {
            let p = std::array::from_fn(|axis| {
                let fraction = item.trim[axis]
                    + if corner & (1 << axis) == 0 {
                        0.
                    } else {
                        item.reveal[axis]
                    };
                item.center[axis]
                    + item.size[axis]
                        * if axis == 0 {
                            fraction - 0.5
                        } else {
                            0.5 - fraction
                        }
            });
            let [x, y, z] = project(p, frame.yaw, frame.pitch);
            bounds = Some(match bounds {
                None => [x, y, x, y, z, z],
                Some([l, b, r, t, near, far]) => [
                    l.min(x),
                    b.min(y),
                    r.max(x),
                    t.max(y),
                    near.min(z),
                    far.max(z),
                ],
            });
        }
    }
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_tuple_labels_fit_before_rasterization() {
        let mut fonts = FontSystem::new();
        let mut cache = SwashCache::new();
        let sprite = label_sprite(
            &mut fonts,
            &mut cache,
            "longname,longname,longname",
            "",
            false,
        );
        assert!(sprite.advance <= (TILE[0] - 7) as f32);
        assert!(sprite.pixels.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    #[ignore = "requires a headless GPU; measures the actual visible silhouette through fractional growth and rotation"]
    fn growing_grid_pixels_stay_centered_on_both_axes() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            font_path: PathBuf::from(crate::scenes::FONT_PATH),
            file_name: "grid-center".into(),
        }))
        .unwrap();
        let blank = renderer
            .render_grid(GridFrame {
                items: &[],
                yaw: 0.,
                pitch: 0.,
                scale: 1.,
            })
            .unwrap();
        for (extent, yaw, pitch, cut) in [
            ([1., 1., 1.], 0., 0., 0.),
            ([2.37, 1., 1.], 0., 0., 0.),
            ([3., 1.46, 1.], 0.23, 0.18, 0.),
            ([3., 2., 2.71], 0.62, 0.58, 0.),
            ([3., 2., 4.], 0.62, 0.58, 0.),
            ([3., 2., 2.], 0.4, 0.3, 0.65),
        ] {
            let mut items = Vec::new();
            for c in 0..4 {
                for b in 0..2 {
                    for a in 0..3 {
                        let trim = [0., 0., (cut - c as f32).clamp(0., 1.)];
                        let reveal = [
                            (extent[0] - a as f32).clamp(0., 1.),
                            (extent[1] - b as f32).clamp(0., 1.),
                            ((extent[2] - c as f32).clamp(0., 1.) - trim[2]).max(0.),
                        ];
                        items.push(GridItemFrame {
                            label: "",
                            detail: "",
                            center: [
                                a as f32 * 150. - 150.,
                                75. - b as f32 * 150.,
                                225. - c as f32 * 150.,
                            ],
                            size: [150.; 3],
                            color: [0.9, 0.3, 0.05],
                            fill: [0.03, 0.04, 0.05],
                            presence: 1.,
                            reveal,
                            trim,
                            label_opacity: 0.,
                            emphasis: 1.,
                            group: false,
                            heading: false,
                        });
                    }
                }
            }
            let frame = GridFrame {
                items: &items,
                yaw,
                pitch,
                scale: 0.72,
            };
            let pixels = renderer.render_grid(frame).unwrap();
            let mut bounds = [1920, 1080, 0, 0];
            for (index, (pixel, background)) in pixels
                .chunks_exact(4)
                .zip(blank.chunks_exact(4))
                .enumerate()
            {
                if pixel == background {
                    continue;
                }
                let x = index as u32 % 1920;
                let y = index as u32 / 1920;
                bounds = [
                    bounds[0].min(x),
                    bounds[1].min(y),
                    bounds[2].max(x),
                    bounds[3].max(y),
                ];
            }
            assert!(
                ((bounds[0] + bounds[2] + 1) as f32 / 2. - 960.).abs() <= 0.75,
                "x {extent:?}: {bounds:?}"
            );
            assert!(
                ((bounds[1] + bounds[3] + 1) as f32 / 2. - 540.).abs() <= 0.75,
                "y {extent:?}: {bounds:?}"
            );
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; verifies real depth occlusion and fractional geometry"]
    fn grid_depth_is_not_draw_order_and_fractional_motion_changes_coverage() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            font_path: PathBuf::from(crate::scenes::FONT_PATH),
            file_name: "grid-depth".into(),
        }))
        .unwrap();
        let item = |label, color, z| GridItemFrame {
            label,
            detail: "",
            color,
            center: [0., 0., z],
            size: [200., 160., 80.],
            presence: 1.,
            reveal: [1.; 3],
            trim: [0.; 3],
            fill: [0.03, 0.04, 0.05],
            label_opacity: 1.,
            emphasis: 1.,
            group: false,
            heading: false,
        };
        let render = |renderer: &mut HeadlessRenderer, items: &[GridItemFrame<'_>]| {
            renderer
                .render_grid(GridFrame {
                    items,
                    yaw: 0.,
                    pitch: 0.,
                    scale: 1.,
                })
                .unwrap()
        };
        let front = render(&mut renderer, &[item("", [0.7, 0.2, 0.1], 200.)]);
        let back = render(&mut renderer, &[item("", [0.1, 0.2, 0.7], -200.)]);
        assert_ne!(front, back);
        for reversed in [false, true] {
            let mut items = vec![
                item("", [0.7, 0.2, 0.1], 200.),
                item("", [0.1, 0.2, 0.7], -200.),
            ];
            if reversed {
                items.reverse();
            }
            assert_eq!(
                front,
                render(&mut renderer, &items),
                "near coincident strokes must win regardless of submission order"
            );
        }
        let blank = render(&mut renderer, &[]);
        let center = (540 * 1920 + 960) * 4;
        assert_ne!(
            &front[center..center + 4],
            &blank[center..center + 4],
            "cell faces must be opaque, not an X-ray of the rear grid"
        );
        let mut moved = item("", [0.7, 0.2, 0.1], 200.);
        moved.reveal[0] = 0.999;
        assert_ne!(
            front,
            render(&mut renderer, &[moved]),
            "fractional growth must survive centering and final window scaling"
        );
        assert_eq!(
            front,
            render(&mut renderer, &[item("", [0.7, 0.2, 0.1], 200.)])
        );
    }
}
