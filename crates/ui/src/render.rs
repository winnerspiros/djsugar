use std::fmt;

use crate::text::ATLAS;
use crate::{Glyph, Painter, Quad};

#[derive(Debug)]
pub enum RendererError {
    Surface(wgpu::CreateSurfaceError),
    Adapter(wgpu::RequestAdapterError),
    Device(wgpu::RequestDeviceError),
    Unsupported,
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(e) => write!(f, "surface: {e}"),
            Self::Adapter(e) => write!(f, "adapter: {e}"),
            Self::Device(e) => write!(f, "device: {e}"),
            Self::Unsupported => f.write_str("the window surface is not supported by the GPU"),
        }
    }
}

impl std::error::Error for RendererError {}

const GLYPH_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Uint16x4, 2 => Uint32];
const QUAD_ATTRIBUTES: [wgpu::VertexAttribute; 6] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Uint32, 3 => Uint32, 4 => Uint32, 5 => Uint32];
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const ROW_ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

#[derive(Debug)]
enum Target<'w> {
    Surface { surface: wgpu::Surface<'w>, config: wgpu::SurfaceConfiguration },
    Offscreen { texture: wgpu::Texture, view: wgpu::TextureView, readback: wgpu::Buffer, width: u32, height: u32 },
}

#[derive(Debug)]
pub struct Renderer<'w> {
    target: Target<'w>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    viewport: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    glyphs: wgpu::Buffer,
    atlas: wgpu::Texture,
}

#[derive(Debug)]
pub struct Pixels<'a> {
    pub bytes: &'a [u8],
    pub stride: usize,
    pub width: u32,
    pub height: u32,
}

impl<'w> Renderer<'w> {
    pub fn new(target: impl Into<wgpu::SurfaceTarget<'w>>, width: u32, height: u32) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(target).map_err(RendererError::Surface)?;
        let adapter = adapter(&instance, Some(&surface))?;
        let mut config =
            surface.get_default_config(&adapter, width.max(1), height.max(1)).ok_or(RendererError::Unsupported)?;
        if let Some(&f) = surface.get_capabilities(&adapter).formats.iter().find(|f| f.is_srgb()) {
            config.format = f;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.desired_maximum_frame_latency = 1;
        let format = config.format;
        Self::build(&adapter, format, |device| {
            surface.configure(device, &config);
            Target::Surface { surface, config }
        })
    }

    pub fn offscreen(width: u32, height: u32) -> Result<Renderer<'static>, RendererError> {
        let instance = wgpu::Instance::default();
        let adapter = adapter(&instance, None)?;
        Renderer::build(&adapter, OFFSCREEN_FORMAT, |device| offscreen_target(device, width, height))
    }

    fn build(
        adapter: &wgpu::Adapter,
        format: wgpu::TextureFormat,
        target: impl FnOnce(&wgpu::Device) -> Target<'w>,
    ) -> Result<Self, RendererError> {
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(RendererError::Device)?;
        let target = target(&device);

        let viewport = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: ATLAS as u32, height: ATLAS as u32, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: viewport.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&atlas_view) },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |shader: &wgpu::ShaderModule, stride: usize, attributes: &[wgpu::VertexAttribute]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: stride as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline =
            make(&device.create_shader_module(wgpu::include_wgsl!("quad.wgsl")), size_of::<Quad>(), &QUAD_ATTRIBUTES);
        let glyph_pipeline = make(
            &device.create_shader_module(wgpu::include_wgsl!("glyph.wgsl")),
            size_of::<Glyph>(),
            &GLYPH_ATTRIBUTES,
        );
        let instances = instance_buffer(&device, 256 * size_of::<Quad>());
        let glyphs = instance_buffer(&device, 1024 * size_of::<Glyph>());
        let r =
            Self { target, device, queue, pipeline, glyph_pipeline, viewport, bind_group, instances, glyphs, atlas };
        r.write_viewport();
        Ok(r)
    }

    pub fn size(&self) -> (u32, u32) {
        match &self.target {
            Target::Surface { config, .. } => (config.width, config.height),
            Target::Offscreen { width, height, .. } => (*width, *height),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == self.size() {
            return;
        }
        match &mut self.target {
            Target::Surface { surface, config } => {
                config.width = width;
                config.height = height;
                surface.configure(&self.device, config);
            }
            offscreen => *offscreen = offscreen_target(&self.device, width, height),
        }
        self.write_viewport();
    }

    fn write_viewport(&self) {
        let (w, h) = self.size();
        self.queue.write_buffer(&self.viewport, 0, bytemuck::cast_slice(&[w as f32, h as f32, 0.0, 0.0]));
    }

    pub fn render(&mut self, painter: &mut Painter, clear: u32) {
        let frame = match &self.target {
            Target::Surface { surface, config } => match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => Some(f),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    surface.configure(&self.device, config);
                    return;
                }
                _ => return,
            },
            Target::Offscreen { .. } => None,
        };
        let view = match (&frame, &self.target) {
            (Some(f), _) => f.texture.create_view(&Default::default()),
            (None, Target::Offscreen { view, .. }) => view.clone(),
            (None, Target::Surface { .. }) => return,
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.draw(&mut encoder, &view, painter, clear);
        if let Target::Offscreen { texture, readback, width, height, .. } = &self.target {
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_row(*width)),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
            );
        }
        self.queue.submit([encoder.finish()]);
        if let Some(f) = frame {
            self.queue.present(f);
        }
    }

    pub fn read(&self, sink: impl FnOnce(Pixels<'_>)) {
        let Target::Offscreen { readback, width, height, .. } = &self.target else { return };
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        if self.device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
            return;
        }
        if let Ok(bytes) = slice.get_mapped_range() {
            sink(Pixels { bytes: &bytes, stride: padded_row(*width) as usize, width: *width, height: *height });
        }
        readback.unmap();
    }

    fn draw(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        painter: &mut Painter,
        clear: u32,
    ) {
        if let Some((row, pixels)) = painter.atlas_update() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: row as u32, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ATLAS as u32), rows_per_image: None },
                wgpu::Extent3d { width: ATLAS as u32, height: (pixels.len() / ATLAS) as u32, depth_or_array_layers: 1 },
            );
        }
        let quads = painter.quads();
        let glyphs = painter.glyphs();
        let quad_bytes = upload(&self.device, &self.queue, &mut self.instances, bytemuck::cast_slice(quads));
        let glyph_bytes = upload(&self.device, &self.queue, &mut self.glyphs, bytemuck::cast_slice(glyphs));
        let [r, g, b, _] = clear.to_le_bytes().map(|c| srgb_to_linear(f64::from(c) / 255.0));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        for (q, g) in painter.spans() {
            if !q.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, self.instances.slice(..quad_bytes));
                pass.draw(0..4, q);
            }
            if !g.is_empty() {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_vertex_buffer(0, self.glyphs.slice(..glyph_bytes));
                pass.draw(0..4, g);
            }
        }
    }
}

fn adapter(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<wgpu::Adapter, RendererError> {
    block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: surface,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .map_err(RendererError::Adapter)
}

fn padded_row(width: u32) -> u32 {
    (width * 4).div_ceil(ROW_ALIGN) * ROW_ALIGN
}

fn offscreen_target<'w>(device: &wgpu::Device, width: u32, height: u32) -> Target<'w> {
    let (width, height) = (width.max(1), height.max(1));
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(padded_row(width)) * u64::from(height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    Target::Offscreen { texture, view, readback, width, height }
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, buffer: &mut wgpu::Buffer, data: &[u8]) -> u64 {
    let len = data.len() as u64;
    if len > buffer.size() {
        *buffer = instance_buffer(device, data.len().next_power_of_two());
    }
    if len > 0 {
        queue.write_buffer(buffer, 0, data);
    }
    len
}

fn instance_buffer(device: &wgpu::Device, bytes: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn block_on<F: Future>(f: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_padded_for_copies() {
        assert_eq!(padded_row(64), 256);
        assert_eq!(padded_row(65), 512);
        assert_eq!(padded_row(1920), 7680);
    }
}
