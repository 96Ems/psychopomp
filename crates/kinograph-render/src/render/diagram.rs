//! Bare finite diagrams: one shared GPU pixel recipe for native and WASM.
//! Motion/identity remain ordinary authored channels. No frame-history state.
use super::*;
use kinograph::component_prototype::{DiagramAnchor, DiagramNode, DiagramPlan, DiagramView, Side};

const SAMPLES: u32 = 4;
const MAX_NODES: usize = 16;
const MAX_LINKS: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Node {
    pose: [f32; 4],
    optics: [f32; 4],
    content: [f32; 4],
    label_a: [f32; 4],
    label_b: [f32; 4],
    ink: [f32; 4],
    border: [f32; 4],
    volume: [f32; 4], // top Z, emphasis, sampled camera depth, reserved
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Link {
    ends: [f32; 4],
    state: [f32; 4],
    depth: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Frame {
    viewport: [f32; 4],
    background: [f32; 4],
    surface: [f32; 4],
    accent: [f32; 4],
    wire: [f32; 4],
    nodes: [Node; MAX_NODES],
    links: [Link; MAX_LINKS],
}

pub(crate) struct DiagramGlyphs {
    pipeline: wgpu::RenderPipeline,
    binding: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    color: wgpu::TextureView,
    labels: Vec<([f32; 4], [f32; 4])>,
}

#[derive(Clone, Copy, Debug)]
struct Pose {
    center: [f32; 2],
    size: [f32; 2],
    scale: f32,
    depth: f32,
    lift: f32,
}
impl Pose {
    fn sample(node: &DiagramNode, value: &impl Fn(&str, f32) -> f32) -> Self {
        let v = |name, default| value(&format!("node.{}.{name}", node.id), default);
        Self {
            center: [v("x", node.center[0]), v("y", node.center[1])],
            size: [
                (v("width", node.size[0]) * v("width-reveal", 1.).clamp(0., 2.)).clamp(1., 1000.),
                v("height", node.size[1]).clamp(1., 300.),
            ],
            scale: v("scale", 1.).clamp(0.01, 2.),
            depth: v("depth", 28.).clamp(1., 120.),
            lift: v("lift", 0.).clamp(-120., 200.),
        }
    }
    fn anchor(self, side: Side, offset: f32) -> [f32; 2] {
        let [w, h] = self.size.map(|v| v * self.scale * 0.5);
        let [x, y] = self.center;
        match side {
            Side::Left => [x - w, y + (offset * self.scale).clamp(-h, h)],
            Side::Right => [x + w, y + (offset * self.scale).clamp(-h, h)],
            Side::Top => [x + (offset * self.scale).clamp(-w, w), y - h],
            Side::Bottom => [x + (offset * self.scale).clamp(-w, w), y + h],
        }
    }
    fn port(self, side: Side, offset: f32, view: DiagramView) -> [f32; 2] {
        let mut p = project(self.anchor(side, offset), view);
        if view == DiagramView::Isometric {
            p[1] -= self.port_z();
        }
        p
    }
    fn top_z(self) -> f32 {
        // Preserve the original ground plane while the volume grows upward.
        self.depth * self.scale + self.lift - 28.
    }
    fn port_z(self) -> f32 {
        self.top_z() - self.depth * self.scale * 0.5
    }
    fn camera_depth(self) -> f32 {
        // The projection's view ray is parallel to (1, 1, 1). Use the sampled
        // solid center, not screen Y (which decreases as a box rises).
        self.center[0] + self.center[1] + self.port_z()
    }
    fn top_center(self, view: DiagramView) -> [f32; 2] {
        let mut p = project(self.center, view);
        if view == DiagramView::Isometric {
            p[1] -= self.top_z();
        }
        p
    }
}
fn project(point: [f32; 2], view: DiagramView) -> [f32; 2] {
    if view == DiagramView::Flat {
        return point;
    }
    let [x, y] = [point[0] - 960., point[1] - 549.];
    [960. + (x - y) * 3_f32.sqrt() / 2., 524. + (x + y) / 2.]
}
fn rgb(c: [u8; 3]) -> [f32; 4] {
    [
        c[0] as f32 / 255.,
        c[1] as f32 / 255.,
        c[2] as f32 / 255.,
        1.,
    ]
}
fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t.clamp(0., 1.))
}

impl HeadlessRenderer {
    pub(crate) fn prepare_diagram(&mut self, p: &DiagramPlan) -> Result<DiagramGlyphs> {
        let mut sprites = Vec::new();
        let mut indices = Vec::new();
        for node in &p.nodes {
            let mut add = |text: &str| -> Result<usize> {
                let attrs = Attrs::new()
                    .family(Family::Name("CommitMono"))
                    .color(Color::rgb(255, 255, 255));
                let sprite = make_sprite(
                    &mut self.font_system,
                    &mut self.swash_cache,
                    vec![(text, attrs.clone())],
                    attrs,
                    Metrics::new(24., 34.),
                    self.spec.width,
                    34,
                );
                if sprite.advance > node.size[0] - 32. {
                    bail!("diagram label does not fit its box: {}", node.id);
                }
                let index = sprites.len();
                sprites.push(sprite);
                Ok(index)
            };
            let a = add(&node.label)?;
            let b = node.alternate_label.as_deref().map(&mut add).transpose()?;
            indices.push((a, b));
        }
        let width = sprites
            .iter()
            .map(|s| s.advance.ceil() as u32 + 6)
            .max()
            .unwrap_or(8)
            .next_power_of_two();
        let height = (sprites.len() as u32 * 38).max(1);
        if width > self.device.limits().max_texture_dimension_2d
            || height > self.device.limits().max_texture_dimension_2d
        {
            bail!("diagram glyph atlas exceeds device limits");
        }
        let mut pixels = vec![0; (width * height) as usize];
        let mut rects = Vec::new();
        for (i, s) in sprites.iter().enumerate() {
            let y = i as u32 * 38 + 2;
            let copy_width = (s.advance.ceil() as u32 + 2).min(s.width);
            for row in 0..s.height {
                for x in 0..copy_width {
                    pixels[((y + row) * width + x + 2) as usize] =
                        s.pixels[((row * s.width + x) * 4 + 3) as usize];
                }
            }
            rects.push([2., y as f32, s.advance, s.height as f32]);
        }
        let atlas = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("diagram R8 glyph coverage"),
            size: wgpu::Extent3d {
                width,
                height,
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
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            atlas.size(),
        );
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fractional diagram text"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sampled diagram values"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &atlas.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shared flat/isometric diagram"),
                source: wgpu::ShaderSource::Wgsl(include_str!("diagram.wgsl").into()),
            });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("bare diagram GPU compositor"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
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
        let color = self
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("diagram 4x spatial AA"),
                size: wgpu::Extent3d {
                    width: self.spec.width,
                    height: self.spec.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: SAMPLES,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Ok(DiagramGlyphs {
            pipeline,
            binding,
            uniform,
            color,
            labels: indices
                .into_iter()
                .map(|(a, b)| (rects[a], b.map_or([0.; 4], |b| rects[b])))
                .collect(),
        })
    }

    pub(crate) fn render_diagram(
        &mut self,
        p: &DiagramPlan,
        g: &DiagramGlyphs,
        value: impl Fn(&str, f32) -> f32,
    ) -> Result<Vec<u8>> {
        let palette = self.theme.palette();
        let original = self.theme == Theme::Original;
        let background = rgb(if original { [5; 3] } else { palette.background });
        let surface = rgb(if original {
            if p.view == DiagramView::Isometric {
                [18, 22, 29]
            } else {
                [10; 3]
            }
        } else {
            palette.surface
        });
        let accent = rgb(if original {
            [179, 146, 240]
        } else {
            palette.accent
        });
        let line = rgb(if original {
            if p.view == DiagramView::Isometric {
                [68, 74, 87]
            } else {
                [58; 3]
            }
        } else {
            palette.raised
        });
        let wire = rgb(if original { [90; 3] } else { palette.muted });
        let mut frame = Frame {
            viewport: [
                self.spec.width as f32,
                self.spec.height as f32,
                f32::from(p.view == DiagramView::Isometric),
                p.nodes.len() as f32,
            ],
            background,
            surface,
            accent,
            wire,
            ..Frame::zeroed()
        };
        frame.wire[3] = p.links.len() as f32;
        let poses = p
            .nodes
            .iter()
            .map(|n| Pose::sample(n, &value))
            .collect::<Vec<_>>();
        for (i, n) in p.nodes.iter().enumerate() {
            let v = |name, default| value(&format!("node.{}.{name}", n.id), default);
            let pose = poses[i];
            let center = pose.top_center(p.view);
            frame.nodes[i] = Node {
                pose: [center[0], center[1], pose.size[0], pose.size[1]],
                optics: [
                    pose.scale,
                    v("blur", 0.).clamp(0., 16.),
                    v("opacity", 1.).clamp(0., 1.),
                    if p.view == DiagramView::Isometric {
                        pose.depth
                    } else {
                        0.
                    },
                ],
                content: [
                    v("shell", 1.).clamp(0., 1.),
                    v("ink", 1.).clamp(0., 1.),
                    v("label", 0.).clamp(0., 1.),
                    v("glow", 0.).clamp(0., 1.),
                ],
                label_a: g.labels[i].0,
                label_b: g.labels[i].1,
                ink: rgb(if original {
                    if n.muted {
                        [136, 134, 129]
                    } else {
                        [225, 228, 232]
                    }
                } else if n.muted {
                    palette.muted
                } else {
                    palette.text
                }),
                border: mix(line, accent, v("emphasis", 0.)),
                volume: [
                    pose.top_z(),
                    v("emphasis", 0.).clamp(0., 1.),
                    pose.camera_depth(),
                    0.,
                ],
            };
        }
        if p.view == DiagramView::Isometric {
            // Move the complete paint packet, including its glyphs and optics.
            // Stable ties preserve authored ownership when boxes coincide at
            // the merge destination; that priority must not beat a nearer box.
            frame.nodes[..p.nodes.len()].sort_by(|a, b| a.volume[2].total_cmp(&b.volume[2]));
        }
        let anchor = |a: &DiagramAnchor, offset| {
            let i = p
                .nodes
                .iter()
                .position(|n| n.id == a.node)
                .expect("validated diagram port");
            (poses[i].port(a.side, offset, p.view), poses[i].port_z())
        };
        for (i, l) in p.links.iter().enumerate() {
            let v = |name, default| value(&format!("link.{}.{name}", l.id), default);
            let (a, az) = anchor(&l.from, v("from-offset", l.from.offset));
            let (b, bz) = anchor(&l.to, v("to-offset", l.to.offset));
            frame.links[i] = Link {
                ends: [a[0], a[1], b[0], b[1]],
                state: [
                    v("draw", 1.).clamp(0., 1.),
                    v("opacity", 1.).clamp(0., 1.),
                    v("emphasis", 0.).clamp(0., 1.),
                    0.,
                ],
                depth: [az, bz, 0., 0.],
            };
        }
        self.queue
            .write_buffer(&g.uniform, 0, bytemuck::bytes_of(&frame));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bare diagram sample"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("boxes, attached wires, glyphs"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &g.color,
                    depth_slice: None,
                    resolve_target: Some(&self.view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&g.pipeline);
            pass.set_bind_group(0, &g.binding, &[]);
            pass.draw(0..3, 0..1);
        }
        self.read_frame(encoder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_reveal_changes_only_sampled_width_and_attached_side_ports() {
        let node = DiagramNode {
            id: "box".into(),
            center: [960., 549.],
            size: [300., 112.],
            label: "TUI 1".into(),
            alternate_label: None,
            muted: false,
        };
        assert_eq!(Pose::sample(&node, &|_, d| d).size, node.size);
        for reveal in [1., 0.25, 8. / 300., 1.] {
            let pose = Pose::sample(&node, &|p, d| {
                if p.ends_with(".width-reveal") {
                    reveal
                } else {
                    d
                }
            });
            assert_eq!(pose.center, node.center);
            assert_eq!(pose.size, [300. * reveal, 112.]);
            assert_eq!(pose.scale, 1., "width growth must not scale type or height");
            assert_eq!(pose.anchor(Side::Right, 0.), [960. + 150. * reveal, 549.]);
            assert_eq!(pose.anchor(Side::Top, 500.), [960. + 150. * reveal, 493.]);
        }
    }

    #[test]
    #[ignore = "requires headless GPU; upright ink must disclose within the growing top face"]
    fn isometric_labels_do_not_escape_a_narrow_top_face() {
        let scene = kinograph_opencode_architecture::build_deck()
            .unwrap()
            .slides
            .remove(1)
            .plan;
        let mut plan: DiagramPlan = serde_json::from_value(scene.actors[0].data.clone()).unwrap();
        plan.nodes.retain(|n| n.id == "client-2");
        plan.links.clear();
        plan.delays.clear();
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "width-ink-proof".into(),
        }))
        .unwrap();
        let glyphs = renderer.prepare_diagram(&plan).unwrap();
        let value = |p: &str, d| if p.ends_with(".width") { 32. } else { d };
        let with = renderer.render_diagram(&plan, &glyphs, value).unwrap();
        let without = renderer
            .render_diagram(&plan, &glyphs, |p, d| {
                if p.ends_with(".ink") { 0. } else { value(p, d) }
            })
            .unwrap();
        let pose = Pose::sample(&plan.nodes[0], &value);
        let center = pose.top_center(plan.view);
        let mut inside = 0;
        let mut outside = 0;
        for y in center[1] as usize - 40..center[1] as usize + 40 {
            for x in center[0] as usize - 100..center[0] as usize + 100 {
                let p = [x as f32 + 0.5 - center[0], y as f32 + 0.5 - center[1]];
                let local = [p[0] / 3_f32.sqrt() + p[1], -p[0] / 3_f32.sqrt() + p[1]];
                let edge =
                    (pose.size[0] * 0.5 - local[0].abs()).min(pose.size[1] * 0.5 - local[1].abs());
                let offset = (y * 1920 + x) * 4;
                let changed = with[offset..offset + 3] != without[offset..offset + 3];
                if edge > 2. {
                    inside += usize::from(changed);
                }
                if edge < -2. {
                    outside += usize::from(changed);
                }
            }
        }
        assert!(
            inside > 20,
            "do not hide the complete label while width opens"
        );
        assert_eq!(outside, 0, "label painted beyond the sampled top face");
    }

    #[test]
    #[ignore = "requires headless GPU; foreground boxes must not depend on declaration order"]
    fn isometric_foreground_occludes_middle_pair_in_either_catalog_order() {
        let scene = kinograph_opencode_architecture::build_deck()
            .unwrap()
            .slides
            .remove(1)
            .plan;
        let mut plan: DiagramPlan = serde_json::from_value(scene.actors[0].data.clone()).unwrap();
        plan.nodes
            .retain(|n| n.id == "server-1" || n.id == "server-2");
        plan.links.clear();
        plan.delays.clear();
        for n in &mut plan.nodes {
            n.center = [if n.id == "server-2" { 1040. } else { 960. }, 658.];
            n.size = [380., 92.];
        }
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "depth-order-proof".into(),
        }))
        .unwrap();
        let mut reversed = plan.clone();
        reversed.nodes.reverse();
        let glyphs = renderer.prepare_diagram(&plan).unwrap();
        let reverse_glyphs = renderer.prepare_diagram(&reversed).unwrap();
        let mut foreground = plan.clone();
        foreground.nodes.retain(|n| n.id == "server-2");
        let foreground_glyphs = renderer.prepare_diagram(&foreground).unwrap();
        for (x, depth, lift, opacity, blur) in [
            (1040., 48., 0., 1., 0.),
            (880., 24., 12., 0.6, 3.),
            (1040., 24., 12., 0.6, 3.),
            (1040., 48., 0., 1., 0.),
        ] {
            let value = |property: &str, default| {
                if property == "node.server-2.x" {
                    x
                } else if property.ends_with(".depth") {
                    depth
                } else if property.ends_with(".lift") {
                    lift
                } else if property == "node.server-1.label" || property == "node.server-1.emphasis"
                {
                    1.
                } else if property == "node.server-2.opacity" {
                    opacity
                } else if property == "node.server-2.blur" {
                    blur
                } else {
                    default
                }
            };
            let actual = renderer.render_diagram(&plan, &glyphs, value).unwrap();
            let expected = renderer
                .render_diagram(&reversed, &reverse_glyphs, value)
                .unwrap();
            let differences = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
            assert_eq!(
                differences, 0,
                "foreground changed with catalog order: x={x}, depth={depth}, lift={lift}, opacity={opacity}, blur={blur}"
            );
            if x > 960. && opacity == 1. && blur == 0. {
                // Order independence alone would also accept a consistently
                // backwards sort. The opaque foreground's interior and label
                // must match that box rendered alone.
                let alone = renderer
                    .render_diagram(&foreground, &foreground_glyphs, value)
                    .unwrap();
                let center = Pose {
                    center: [x, 658.],
                    size: [380., 92.],
                    scale: 1.,
                    depth,
                    lift,
                }
                .top_center(DiagramView::Isometric);
                for y in center[1] as usize - 10..center[1] as usize + 10 {
                    let start = (y * 1920 + center[0] as usize - 50) * 4;
                    assert_eq!(
                        &actual[start..start + 100 * 4],
                        &alone[start..start + 100 * 4],
                        "rear box painted over the foreground label at y={y}"
                    );
                }
            }
        }

        // At coincident depth, authored order must still preserve the retained
        // daemon's ink rather than letting outgoing "server" cover it.
        let mut retained = plan.clone();
        retained.nodes.retain(|n| n.id == "server-1");
        let retained_glyphs = renderer.prepare_diagram(&retained).unwrap();
        let coincident = |property: &str, default| {
            if property.ends_with(".x") {
                960.
            } else if property.ends_with(".depth") {
                48.
            } else if property == "node.server-1.label" || property == "node.server-1.emphasis" {
                1.
            } else {
                default
            }
        };
        let actual = renderer.render_diagram(&plan, &glyphs, coincident).unwrap();
        let alone = renderer
            .render_diagram(&retained, &retained_glyphs, coincident)
            .unwrap();
        let center = Pose {
            center: [960., 658.],
            size: [380., 92.],
            scale: 1.,
            depth: 48.,
            lift: 0.,
        }
        .top_center(DiagramView::Isometric);
        for y in center[1] as usize - 10..center[1] as usize + 10 {
            let start = (y * 1920 + center[0] as usize - 50) * 4;
            assert_eq!(
                &actual[start..start + 100 * 4],
                &alone[start..start + 100 * 4],
                "coincident outgoing ink covered daemon at y={y}"
            );
        }
    }

    #[test]
    fn camera_depth_uses_the_view_ray_not_projected_screen_height() {
        let pose = Pose {
            center: [960., 658.],
            size: [380., 92.],
            scale: 1.,
            depth: 48.,
            lift: 0.,
        };
        let nearer = Pose {
            center: [1040., 658.],
            ..pose
        };
        let raised = Pose { lift: 40., ..pose };
        let thicker = Pose { depth: 64., ..pose };
        for next in [nearer, raised, thicker] {
            assert!(next.camera_depth() > pose.camera_depth());
        }
        assert!(
            raised.top_center(DiagramView::Isometric)[1]
                < pose.top_center(DiagramView::Isometric)[1]
        );
        assert_eq!(
            Pose {
                center: [1000., 618.],
                ..pose
            }
            .camera_depth(),
            pose.camera_depth()
        );
    }

    #[test]
    fn isometric_port_is_halfway_down_the_side() {
        let pose = Pose {
            center: [560., 440.],
            size: [300., 112.],
            scale: 1.,
            depth: 28.,
            lift: 0.,
        };
        let top = project(pose.anchor(Side::Bottom, 0.), DiagramView::Isometric);
        assert_eq!(
            pose.port(Side::Bottom, 0., DiagramView::Isometric),
            [top[0], top[1] + 14.]
        );
    }

    #[test]
    fn side_ports_follow_depth_lift_and_scale_in_any_sampling_order() {
        for depth in [48., 8., 53., 28., 48.] {
            for lift in [0., 40., -2., 0.] {
                for scale in [1., 1.08, 0.94] {
                    let pose = Pose {
                        center: [631.25, 402.75],
                        size: [380., 112.],
                        scale,
                        depth,
                        lift,
                    };
                    for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
                        let xy = project(pose.anchor(side, 37.), DiagramView::Isometric);
                        let top = [xy[0], xy[1] - pose.top_z()];
                        let bottom = [top[0], top[1] + depth * scale];
                        let port = pose.port(side, 37., DiagramView::Isometric);
                        assert_eq!(port[0], xy[0]);
                        assert!((port[1] - (top[1] + bottom[1]) * 0.5).abs() < 0.0001);
                        assert_eq!(
                            pose.port(side, 37., DiagramView::Flat),
                            pose.anchor(side, 37.)
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires headless GPU; the delivered connector must reach the side-face center, not be hidden until the lower rim"]
    fn isometric_wire_is_visible_at_the_side_face_center() {
        let plan = kinograph_opencode_architecture::build_scene().unwrap();
        let mut p: DiagramPlan = serde_json::from_value(plan.actors[0].data.clone()).unwrap();
        p.view = DiagramView::Isometric;
        p.nodes.retain(|n| n.id == "client-0" || n.id == "server-0");
        p.nodes[0].center = [560., 440.];
        p.nodes[1].center = [560., 658.];
        p.links.retain(|l| l.id == "wire-0");
        p.delays.clear();
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "side-port-proof".into(),
        }))
        .unwrap();
        let glyphs = renderer.prepare_diagram(&p).unwrap();
        let with = renderer.render_diagram(&p, &glyphs, |_, d| d).unwrap();
        let without = renderer
            .render_diagram(&p, &glyphs, |prop, d| {
                if prop == "link.wire-0.opacity" { 0. } else { d }
            })
            .unwrap();
        let top = project([560., 496.], DiagramView::Isometric);
        let x = top[0].floor() as usize;
        let y = (top[1] + 14.).floor() as usize;
        let mut difference = 0u32;
        for yy in y.saturating_sub(1)..=y + 1 {
            for xx in x.saturating_sub(1)..=x + 1 {
                let index = (yy * 1920 + xx) * 4;
                difference += with[index..index + 3]
                    .iter()
                    .zip(&without[index..index + 3])
                    .map(|(a, b)| u32::from(a.abs_diff(*b)))
                    .sum::<u32>();
            }
        }
        assert!(
            difference > 20,
            "wire never reaches the center of the visible side face: {difference}"
        );
        // The opposite port is genuinely behind an opaque top face. Fixing the
        // visible endpoint must not paint an x-ray wire through the destination.
        let top = project([560., 612.], DiagramView::Isometric);
        let x = top[0].floor() as usize;
        let y = (top[1] + 14.).floor() as usize;
        for yy in y - 1..=y + 1 {
            for xx in x - 1..=x + 1 {
                let index = (yy * 1920 + xx) * 4;
                assert_eq!(
                    &with[index..index + 4],
                    &without[index..index + 4],
                    "wire painted through its destination at {xx},{yy}"
                );
            }
        }
    }
    #[test]
    fn ports_follow_sampled_size_position_and_scale() {
        let pose = Pose {
            center: [640., 380.],
            size: [380., 92.],
            scale: 1.08,
            depth: 28.,
            lift: 0.,
        };
        assert_eq!(
            pose.anchor(Side::Top, 88.),
            [640. + 88. * 1.08, 380. - 46. * 1.08]
        );
        assert_eq!(
            pose.anchor(Side::Bottom, 1000.),
            [640. + 190. * 1.08, 380. + 46. * 1.08]
        );
    }
    #[test]
    fn isometric_projection_has_equal_axis_lengths_and_attached_ports() {
        let origin = project([960., 549.], DiagramView::Isometric);
        for point in [[1060., 549.], [960., 649.]] {
            let p = project(point, DiagramView::Isometric);
            assert!(((p[0] - origin[0]).hypot(p[1] - origin[1]) - 100.).abs() < 0.001);
        }
        let p = Pose {
            center: [960., 549.],
            size: [300., 112.],
            scale: 1.,
            depth: 28.,
            lift: 0.,
        };
        assert_eq!(
            project(p.anchor(Side::Bottom, 0.), DiagramView::Isometric),
            project([960., 605.], DiagramView::Isometric)
        );
    }
}
