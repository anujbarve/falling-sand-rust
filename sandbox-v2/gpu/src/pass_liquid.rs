//! Liquid (water) compute pass. Rule defined by [`crate::reference`].

use crate::materials::{Material, MATERIALS};
use crate::pass_rows::RowPass;
use crate::reference::WATER;
use crate::{Gpu, GridState};

/// Persistent liquid pass: the shader is compiled once in [`LiquidPass::new`].
pub struct LiquidPass(RowPass);

impl LiquidPass {
    /// Compiles the liquid pipeline with water's terminal speed.
    pub fn new(gpu: &Gpu) -> LiquidPass {
        let cps = MATERIALS[Material::Water as usize].terminal_cps;
        LiquidPass(RowPass::new(gpu, "liquid", WATER, true, cps))
    }

    /// Runs both row phases for `tick`; the result is in `grid`'s current buffer.
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState, tick: u32, seed: u32) {
        self.0.dispatch(gpu, grid, tick, seed);
    }
}
