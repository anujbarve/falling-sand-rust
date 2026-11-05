//! Granular (sand) compute pass. Rule defined by [`crate::reference`].

use crate::materials::{Material, MATERIALS};
use crate::pass_rows::RowPass;
use crate::reference::SAND;
use crate::{Gpu, GridState};

/// Persistent granular pass: the shader is compiled once in [`GranularPass::new`].
pub struct GranularPass(RowPass);

impl GranularPass {
    /// Compiles the granular pipeline with sand's terminal speed.
    pub fn new(gpu: &Gpu) -> GranularPass {
        let cps = MATERIALS[Material::Sand as usize].terminal_cps;
        GranularPass(RowPass::new(gpu, "granular", SAND, false, cps))
    }

    /// Records both row phases for `tick` into `pass` (no submit); the result
    /// is in `grid`'s current buffer once the pass runs.
    pub fn record(
        &self,
        gpu: &Gpu,
        pass: &mut wgpu::ComputePass<'_>,
        grid: &GridState,
        tick: u32,
        seed: u32,
    ) {
        self.0.record(gpu, pass, grid, tick, seed);
    }

    /// Runs both row phases for `tick` in one submit; the result is in
    /// `grid`'s current buffer.
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState, tick: u32, seed: u32) {
        self.0.dispatch(gpu, grid, tick, seed);
    }
}
