//! Granular (sand) compute pass. Rule defined by [`crate::reference`].

use crate::pass_rows::RowPass;
use crate::{Gpu, GridState};

/// Reusable compiled granular pass.
pub struct GranularPass(RowPass);

impl GranularPass {
    /// Compiles the granular pipeline.
    pub fn new(gpu: &Gpu) -> GranularPass {
        GranularPass(RowPass::new(gpu, "granular", include_str!("shaders/granular.wgsl")))
    }

    /// Runs both row phases for `tick`; leaves the result in `grid`'s current buffer.
    pub fn dispatch(&self, gpu: &Gpu, grid: &mut GridState, tick: u32, seed: u32) {
        self.0.dispatch(gpu, grid, tick, seed);
    }
}

/// One-shot convenience: compiles a [`GranularPass`] and runs it. Prefer
/// reusing a `GranularPass` in loops (this recompiles the shader every call).
pub fn dispatch_granular(gpu: &Gpu, grid: &mut GridState, tick: u32, seed: u32) {
    GranularPass::new(gpu).dispatch(gpu, grid, tick, seed);
}
