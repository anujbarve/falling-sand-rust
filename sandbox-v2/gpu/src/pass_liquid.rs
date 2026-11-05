//! Liquid (water) compute pass. Rule defined by [`crate::reference`].

use crate::pass_rows::RowPass;
use crate::{Gpu, GridState};

/// Reusable compiled liquid pass.
pub struct LiquidPass(RowPass);

impl LiquidPass {
    /// Compiles the liquid pipeline.
    pub fn new(gpu: &Gpu) -> LiquidPass {
        LiquidPass(RowPass::new(gpu, "liquid", include_str!("shaders/liquid.wgsl")))
    }

    /// Runs both row phases for `tick`; leaves the result in `grid`'s current buffer.
    pub fn dispatch(&self, gpu: &Gpu, grid: &mut GridState, tick: u32, seed: u32) {
        self.0.dispatch(gpu, grid, tick, seed);
    }
}

/// One-shot convenience: compiles a [`LiquidPass`] and runs it. Prefer
/// reusing a `LiquidPass` in loops (this recompiles the shader every call).
pub fn dispatch_liquid(gpu: &Gpu, grid: &mut GridState, tick: u32, seed: u32) {
    LiquidPass::new(gpu).dispatch(gpu, grid, tick, seed);
}
