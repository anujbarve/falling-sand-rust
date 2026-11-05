//! GPU device creation and window-surface handling for sandbox v2.
//!
//! [`Gpu`] owns the wgpu instance, adapter, device and queue. [`WindowSurface`]
//! wraps a presentable surface and handles resize and per-frame clearing.
//! [`GridState`] holds the double-buffered cell grid ([`CellRaw`]).
//! [`GranularPass`] and [`LiquidPass`] are persistent compute passes whose
//! behaviour is defined by [`mod@reference`]. [`units`] fixes the physical scale and
//! [`materials`] holds per-material constants.

mod layout;
pub mod materials;
mod pass_granular;
mod pass_liquid;
mod pass_rows;
pub mod reference;
mod state;
mod surface;
pub mod units;

pub use layout::{CellRaw, MAX_CELLS};
pub use pass_granular::GranularPass;
pub use pass_liquid::LiquidPass;
pub use state::{GridError, GridState};
pub use surface::WindowSurface;

/// Core wgpu handles shared by every GPU feature.
pub struct Gpu {
    /// The wgpu instance that created the adapter (and any surface).
    pub instance: wgpu::Instance,
    /// The physical adapter in use.
    pub adapter: wgpu::Adapter,
    /// The logical device.
    pub device: wgpu::Device,
    /// The command queue of `device`.
    pub queue: wgpu::Queue,
}

fn new_instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle())
}

async fn init(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Option<Gpu> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: surface,
            ..Default::default()
        })
        .await
        .ok()?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .ok()?;
    Some(Gpu {
        instance,
        adapter,
        device,
        queue,
    })
}

impl Gpu {
    /// Creates a device with no surface. Returns `None` when the machine has no
    /// usable GPU adapter.
    pub fn new_headless() -> Option<Gpu> {
        pollster::block_on(init(new_instance(), None))
    }

    /// Creates a device plus a surface for `target` (for example an
    /// `Arc<winit::window::Window>`) sized `width` by `height` pixels.
    /// Returns `None` when no adapter or surface is available.
    pub fn new_windowed(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Option<(Gpu, WindowSurface)> {
        let instance = new_instance();
        let surface = instance.create_surface(target).ok()?;
        let gpu = pollster::block_on(init(instance, Some(&surface)))?;
        let surface = WindowSurface::new(&gpu, surface, width, height)?;
        Some((gpu, surface))
    }
}
