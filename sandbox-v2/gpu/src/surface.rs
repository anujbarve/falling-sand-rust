//! Window surface: configuration, resize and frame clearing.

use crate::Gpu;

/// Whether a `width` x `height` surface can be drawn (both non-zero).
pub fn drawable(width: u32, height: u32) -> bool {
    width > 0 && height > 0
}

/// A configured presentable surface.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl WindowSurface {
    pub(crate) fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Option<WindowSurface> {
        let config = surface.get_default_config(&gpu.adapter, width, height)?;
        let mut s = WindowSurface { surface, config };
        s.resize(gpu, width, height);
        Some(s)
    }

    /// Reconfigures the surface to `width` by `height` pixels. A zero dimension
    /// is recorded and frames are skipped until a non-zero size arrives.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        if width > 0 && height > 0 {
            self.surface.configure(&gpu.device, &self.config);
        }
    }

    /// Current surface size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Texture format of the surface (pass it to `Renderer::new`).
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Whether frames can be drawn: both dimensions non-zero (a minimised
    /// window reports 0x0).
    pub fn is_drawable(&self) -> bool {
        drawable(self.config.width, self.config.height)
    }

    /// Acquires the next frame, calls `draw` with its view, and presents.
    /// Returns `false` (drawing nothing) while the surface has zero size or
    /// no frame could be acquired. Reconfigures once on a lost or outdated
    /// surface, and after presenting a suboptimal frame.
    pub fn render(&self, gpu: &Gpu, draw: impl FnOnce(&wgpu::TextureView)) -> bool {
        if !self.is_drawable() {
            return false;
        }
        for attempt in 0..2 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) => {
                    draw(&frame.texture.create_view(&Default::default()));
                    gpu.queue.present(frame);
                    return true;
                }
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    draw(&frame.texture.create_view(&Default::default()));
                    gpu.queue.present(frame);
                    self.surface.configure(&gpu.device, &self.config);
                    return true;
                }
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated
                    if attempt == 0 =>
                {
                    self.surface.configure(&gpu.device, &self.config);
                }
                _ => return false,
            }
        }
        false
    }

    /// Clears the surface to `color` and presents. Does nothing while the
    /// surface has zero size; reconfigures once on a lost or outdated surface.
    pub fn clear_frame(&self, gpu: &Gpu, color: wgpu::Color) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }
        for attempt in 0..2 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    let view = frame.texture.create_view(&Default::default());
                    let mut enc = gpu.device.create_command_encoder(&Default::default());
                    drop(enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(color),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    }));
                    gpu.queue.submit([enc.finish()]);
                    gpu.queue.present(frame);
                    return;
                }
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated
                    if attempt == 0 =>
                {
                    self.surface.configure(&gpu.device, &self.config);
                }
                _ => return,
            }
        }
    }
}
