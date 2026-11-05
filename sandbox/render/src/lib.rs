//! Rendering helpers built on `sandbox_core` data: colour palette, CPU pixel buffer,
//! and presenters that put the buffer on screen (macroquad by default, wgpu with feature `gpu`).

pub mod buffer;
pub mod palette;
pub mod presenter;
pub mod smooth;
#[cfg(feature = "gpu")]
pub mod gpu;
