//! CPU-side mirrors of the GPU cell layout (`shaders/common.wgsl`) and the
//! particle layout (`shaders/particles.wgsl`).

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

/// One particle, exactly 32 bytes, matching `struct Particle` in
/// `shaders/particles.wgsl`.
///
/// Byte layout: `pos` 0..8, `vel` 8..16, `mat` 16..20, `temp` 20..24,
/// `life` 24..28, `_pad` 28..32. Units: cells and cells/s (+y is down),
/// kelvin, seconds. A particle is live while `life > 0` (`+inf` = never
/// expires); the step sets `life = 0` when it dies.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ParticleRaw {
    /// Position in cells (x right, y down).
    pub pos: [f32; 2],
    /// Velocity in cells per second.
    pub vel: [f32; 2],
    /// Material id (carried for rendering; not used by the motion rule).
    pub mat: u32,
    /// Temperature in kelvin (carried; not used by the motion rule).
    pub temp: f32,
    /// Seconds left to live; `<= 0` is dead.
    pub life: f32,
    /// Unused; keep 0.
    pub _pad: f32,
}

impl ParticleRaw {
    /// A particle launched from `pos` with velocity `vel` (cells/s) at t = 0.
    ///
    /// The step is semi-implicit Euler (kick, then drift), whose stored
    /// velocity is effectively the half-step velocity. This stores
    /// `vel.y - g*DT/2` (leapfrog start) so the integrated positions lie on
    /// the exact parabola of `vel` instead of one launched `g*DT/2` slower.
    ///
    /// Note: the half kick hard-codes the fixed tick `DT = 1/60` s. Stepping
    /// with another `dt` (e.g. 1/120) leaves a start bias of `g*(DT-dt)/2`;
    /// taking `dt` as an argument is a carried follow-up.
    pub fn launched(pos: [f32; 2], vel: [f32; 2], mat: u32, temp: f32, life: f32) -> ParticleRaw {
        use crate::units::{DT, GRAVITY_CELLS};
        ParticleRaw {
            pos,
            vel: [vel[0], vel[1] - 0.5 * GRAVITY_CELLS * DT],
            mat,
            temp,
            life,
            _pad: 0.0,
        }
    }
}
