//! Optional wgpu presenter (cargo feature `gpu`).
//!
//! [`GpuPresenter`] uploads the CPU [`FrameBuffer`] into a texture each frame and draws it
//! with a fullscreen triangle and nearest filtering. It can target a window surface
//! ([`GpuPresenter::from_surface`]) or an offscreen texture ([`GpuPresenter::new_offscreen`])
//! whose pixels can be read back for tests.

use crate::buffer::FrameBuffer;
use crate::presenter::Presenter;

const FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Why a [`GpuPresenter`] could not be created.
#[derive(Debug)]
pub enum GpuError {
    /// No suitable GPU adapter exists on this machine.
    NoAdapter,
    /// Any other failure while setting up the device or surface.
    Setup(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no GPU adapter available"),
            GpuError::Setup(s) => write!(f, "GPU setup failed: {s}"),
        }
    }
}

impl std::error::Error for GpuError {}

enum Target {
    Offscreen { tex: wgpu::Texture, w: u32, h: u32 },
    Surface { surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration },
}

struct Source {
    tex: wgpu::Texture,
    bind: wgpu::BindGroup,
    size: (u32, u32),
}

/// Presents a [`FrameBuffer`] through wgpu.
pub struct GpuPresenter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    source: Option<Source>,
    target: Target,
}

fn setup(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'static>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), GpuError> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: surface,
        ..Default::default()
    }))
    .map_err(|_| GpuError::NoAdapter)?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| GpuError::Setup(e.to_string()))?;
    Ok((adapter, device, queue))
}

impl GpuPresenter {
    fn build(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        target: Target,
    ) -> GpuPresenter {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("frame pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        GpuPresenter { device, queue, pipeline, layout, sampler, source: None, target }
    }

    fn offscreen_texture(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen target"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Creates a presenter that renders into an offscreen `w` by `h` texture.
    /// Returns [`GpuError::NoAdapter`] on machines without a usable GPU.
    pub fn new_offscreen(w: u32, h: u32) -> Result<GpuPresenter, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let (_adapter, device, queue) = setup(&instance, None)?;
        let tex = Self::offscreen_texture(&device, w, h);
        Ok(Self::build(device, queue, FRAME_FORMAT, Target::Offscreen { tex, w, h }))
    }

    /// Creates a presenter that draws to an existing window surface of `w` by `h` pixels.
    pub fn from_surface(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        w: u32,
        h: u32,
    ) -> Result<GpuPresenter, GpuError> {
        let (adapter, device, queue) = setup(instance, Some(&surface))?;
        let mut config = surface
            .get_default_config(&adapter, w.max(1), h.max(1))
            .ok_or_else(|| GpuError::Setup("surface unsupported by adapter".into()))?;
        // Prefer a non-sRGB format so frame bytes pass through unchanged.
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().copied().find(|f| !f.is_srgb()) {
            config.format = f;
        }
        if w > 0 && h > 0 {
            surface.configure(&device, &config);
        }
        config.width = w;
        config.height = h;
        let format = config.format;
        Ok(Self::build(device, queue, format, Target::Surface { surface, config }))
    }

    /// Tells the presenter the target size changed. A zero width or height makes
    /// [`present`](Presenter::present) skip frames until a non-zero size arrives.
    pub fn resize(&mut self, w: u32, h: u32) {
        match &mut self.target {
            Target::Offscreen { tex, w: tw, h: th } => {
                if (*tw, *th) != (w, h) && w > 0 && h > 0 {
                    *tex = Self::offscreen_texture(&self.device, w, h);
                }
                *tw = w;
                *th = h;
            }
            Target::Surface { surface, config } => {
                config.width = w;
                config.height = h;
                if w > 0 && h > 0 {
                    surface.configure(&self.device, config);
                }
            }
        }
    }

    fn ensure_source(&mut self, w: u32, h: u32) {
        if matches!(&self.source, Some(s) if s.size == (w, h)) {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame source"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame bind"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        self.source = Some(Source { tex, bind, size: (w, h) });
    }

    fn draw(&self, target_view: &wgpu::TextureView) {
        let Some(src) = &self.source else { return };
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &src.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(enc.finish()));
    }

    /// Reads the offscreen target back as tightly packed RGBA8 rows.
    /// Returns `None` for a surface target or a zero-sized target.
    pub fn read_pixels(&self) -> Option<Vec<u8>> {
        let Target::Offscreen { tex, w, h } = &self.target else { return None };
        let (w, h) = (*w, *h);
        if w == 0 || h == 0 {
            return None;
        }
        let row = w * 4;
        let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(enc.finish()));
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let data = slice.get_mapped_range().ok()?;
        let mut out = Vec::with_capacity((row * h) as usize);
        for y in 0..h as usize {
            let start = y * padded as usize;
            out.extend_from_slice(&data[start..start + row as usize]);
        }
        drop(data);
        buf.unmap();
        Some(out)
    }
}

impl Presenter for GpuPresenter {
    fn present(&mut self, fb: &FrameBuffer) {
        if fb.w == 0 || fb.h == 0 || fb.pixels.len() != fb.w * fb.h * 4 {
            return;
        }
        let (fw, fh) = (fb.w as u32, fb.h as u32);
        self.ensure_source(fw, fh);
        if let Some(src) = &self.source {
            self.queue.write_texture(
                src.tex.as_image_copy(),
                &fb.pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(fw * 4), rows_per_image: Some(fh) },
                wgpu::Extent3d { width: fw, height: fh, depth_or_array_layers: 1 },
            );
        }
        match &self.target {
            Target::Offscreen { tex, w, h } => {
                if *w == 0 || *h == 0 {
                    return;
                }
                let view = tex.create_view(&Default::default());
                self.draw(&view);
            }
            Target::Surface { surface, config } => {
                if config.width == 0 || config.height == 0 {
                    return;
                }
                // Lost/Outdated: reconfigure and retry once.
                for attempt in 0..2 {
                    match surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(frame)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                            let view = frame.texture.create_view(&Default::default());
                            self.draw(&view);
                            self.queue.present(frame);
                            return;
                        }
                        wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated
                            if attempt == 0 =>
                        {
                            surface.configure(&self.device, config);
                        }
                        _ => return,
                    }
                }
            }
        }
    }
}
