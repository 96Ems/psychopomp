//! One centered screen-space stroke per visible geometric edge. Nearest-depth
//! selection and max coverage union prevent shared faces from doubling its width.
use super::*;

const INK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub(super) const WIDTH: f32 = 1.7;

pub(super) struct Edges {
    pub base: wgpu::TextureView,
    ink: wgpu::TextureView,
    resolved: wgpu::TextureView,
    depth_pipeline: wgpu::RenderPipeline,
    ink_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    binding: wgpu::BindGroup,
    composite_binding: wgpu::BindGroup,
}

impl Edges {
    pub fn new(device: &wgpu::Device, spec: &RenderSpec, uniform: &wgpu::Buffer) -> Self {
        let texture = |format, samples, label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: spec.width,
                        height: spec.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let base = texture(FORMAT, 1, "grid material");
        let ink = texture(INK_FORMAT, SAMPLES, "grid stroke union");
        let resolved = texture(INK_FORMAT, 1, "grid resolved strokes");
        let shader = device.create_shader_module(wgpu::include_wgsl!("edges.wgsl"));
        let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("grid stroke camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("grid stroke layout"),
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
        let max = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        };
        let targets = [Some(wgpu::ColorTargetState {
            format: INK_FORMAT,
            blend: Some(wgpu::BlendState {
                color: max,
                alpha: max,
            }),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let pipeline = |depth_only| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if depth_only {
                    "grid nearest strokes"
                } else {
                    "grid stroke coverage union"
                }),
                layout: Some(&layout),
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
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(depth_only),
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
                    entry_point: Some(if depth_only {
                        "fragment_depth"
                    } else {
                        "fragment_ink"
                    }),
                    compilation_options: Default::default(),
                    targets: if depth_only { &[] } else { &targets },
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let depth_pipeline = pipeline(true);
        let ink_pipeline = pipeline(false);
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid stroke camera"),
            layout: &bindings,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("edges_composite.wgsl"));
        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid stroke composite"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
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
        let composite_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid material and ink"),
            layout: &composite_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&base),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&resolved),
                },
            ],
        });
        Self {
            base,
            ink,
            resolved,
            depth_pipeline,
            ink_pipeline,
            composite_pipeline,
            binding,
            composite_binding,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        instances: &wgpu::Buffer,
        count: u32,
        depth: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        for depth_only in [true, false] {
            let colors = [Some(wgpu::RenderPassColorAttachment {
                view: &self.ink,
                depth_slice: None,
                resolve_target: Some(&self.resolved),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Discard,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("grid strokes"),
                color_attachments: if depth_only { &[] } else { &colors },
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    stencil_ops: None,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if depth_only {
                &self.depth_pipeline
            } else {
                &self.ink_pipeline
            });
            pass.set_bind_group(0, &self.binding, &[]);
            pass.set_vertex_buffer(0, instances.slice(..));
            pass.draw(0..72, 0..count);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("grid composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
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
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, &self.composite_binding, &[]);
        pass.draw(0..3, 0..1);
    }
}
