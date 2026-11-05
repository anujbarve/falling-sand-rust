//! CPU-side mirror of the GPU cell layout (`shaders/common.wgsl`).

use bytemuck::{Pod, Zeroable};

/// Largest allowed grid, in cells (1024 x 1024).
pub const MAX_CELLS: u32 = 1 << 20;

/// One grid cell, exactly 32 bytes, matching `struct Cell` in `common.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CellRaw {
    /// Material id.
    pub mat: u32,
    /// Temperature.
    pub temp: f32,
    /// Remaining fuel.
    pub fuel: f32,
    /// Horizontal velocity.
    pub vx: f32,
    /// Vertical velocity.
    pub vy: f32,
    /// Vapor amount.
    pub vapor: f32,
    /// Pressure.
    pub pressure: f32,
    /// Bit flags.
    pub flags: u32,
}
