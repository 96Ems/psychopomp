//! Host shim around the UNMODIFIED native grid renderer. Only delivery and glyph
//! provisioning differ. The unfortunate HeadlessRenderer/read_frame names are
//! retained to avoid refactoring the real implementation for a throwaway probe.
use anyhow::{Context, Result, bail};
use bytemuck::{Pod, Zeroable};
use cosmic_text::{Attrs, Color, Family, Metrics, SwashCache};

mod glyphs;
use glyphs::{FontSystem, TextSprite, make_sprite};

include!(concat!(env!("OUT_DIR"), "/render.rs"));
#[path = "../../../crates/psychopomp-render/src/render/theme.rs"]
#[allow(dead_code)] // Only grid palette roles are exercised by the spike.
mod theme;
pub(crate) use diagram::DiagramGlyphs;
pub use grid::{
    GridFrame, GridItemFrame, GridLabelStyle, GridLinePalette, GridTextClip, GridTextDisclosure,
};
pub use theme::Theme;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

struct RenderSpec {
    width: u32,
    height: u32,
}

pub struct HeadlessRenderer {
    spec: RenderSpec,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    #[cfg(not(target_arch = "wasm32"))]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    font_system: FontSystem,
    swash_cache: SwashCache,
    grid_renderer: Option<grid::GridRenderer>,
    grid_line_palette: Option<GridLinePalette>,
    theme: Theme,
    #[cfg(target_arch = "wasm32")]
    canvas: Canvas,
}

#[cfg(target_arch = "wasm32")]
struct Canvas {
    surface: wgpu::Surface<'static>,
    format: wgpu::TextureFormat,
    frame: std::cell::RefCell<Option<wgpu::SurfaceTexture>>,
}

impl HeadlessRenderer {
    pub async fn new(
        #[cfg(target_arch = "wasm32")] canvas: web_sys::HtmlCanvasElement,
        #[cfg(target_arch = "wasm32")] glyphs: &[u8],
    ) -> Result<Self> {
        let instance = wgpu::Instance::default();
        #[cfg(target_arch = "wasm32")]
        let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                #[cfg(target_arch = "wasm32")]
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("same 1080p grid target as native"),
            size: wgpu::Extent3d {
                // Native baking needs readback; WASM replaces this placeholder
                // view with the acquired canvas texture before every sample.
                width: if cfg!(target_arch = "wasm32") {
                    1
                } else {
                    WIDTH
                },
                height: if cfg!(target_arch = "wasm32") {
                    1
                } else {
                    HEIGHT
                },
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        #[cfg(target_arch = "wasm32")]
        let canvas = {
            let mut config = surface
                .get_default_config(&adapter, WIDTH, HEIGHT)
                .context("canvas configuration")?;
            // WebGPU supports RGBA canvas textures. Match the shared recipe's
            // attachment format instead of allocating and blitting another frame.
            config.format = wgpu::TextureFormat::Rgba8Unorm;
            let format = FORMAT;
            config.view_formats = vec![format];
            surface.configure(&device, &config);
            Canvas {
                surface,
                format,
                frame: std::cell::RefCell::new(None),
            }
        };
        Ok(Self {
            spec: RenderSpec {
                width: WIDTH,
                height: HEIGHT,
            },
            device,
            queue,
            #[cfg(not(target_arch = "wasm32"))]
            texture,
            view,
            #[cfg(not(target_arch = "wasm32"))]
            font_system: FontSystem::native()?,
            #[cfg(target_arch = "wasm32")]
            font_system: FontSystem::baked(glyphs)?,
            swash_cache: SwashCache::new(),
            grid_renderer: None,
            grid_line_palette: None,
            theme: Theme::Original,
            #[cfg(target_arch = "wasm32")]
            canvas,
        })
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn bake_glyphs(&self, path: &std::path::Path) -> Result<()> {
        self.font_system.save(path)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn begin_frame(&mut self) -> Result<()> {
        let frame = match self.canvas.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            other => bail!("canvas unavailable: {other:?}"),
        };
        self.view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.canvas.format),
            ..Default::default()
        });
        *self.canvas.frame.borrow_mut() = Some(frame);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn read_frame(&self, encoder: wgpu::CommandEncoder) -> Result<Vec<u8>> {
        let frame = self
            .canvas
            .frame
            .borrow_mut()
            .take()
            .context("begin canvas frame before rendering")?;
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        // Shared call site expects pixels; browser playback intentionally returns none.
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_frame(&self, mut encoder: wgpu::CommandEncoder) -> Result<Vec<u8>> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(WIDTH * HEIGHT * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH * 4),
                    rows_per_image: Some(HEIGHT),
                },
            },
            self.texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = send.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        receive.recv()??;
        let pixels = buffer.slice(..).get_mapped_range()?.to_vec();
        buffer.unmap();
        Ok(pixels)
    }
}
