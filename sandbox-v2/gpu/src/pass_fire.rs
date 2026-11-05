//! Persistent fire pass (ignition, burn countdown). Rule defined by
//! [`crate::reference_thermal`].
//!
//! Bindings: 1 uniform + 3 storage in the shared layout; the shader uses only
//! the destination cells (in place).

use crate::pass_cells::{CellPass, Flow, Stage};
use crate::reference_thermal::wgsl_header;
use crate::{Gpu, GridState};

/// Persistent fire pass: compiled once in [`FirePass::new`].
pub struct FirePass(CellPass);

impl FirePass {
    /// Compiles the fire pipeline.
    pub fn new(gpu: &Gpu) -> FirePass {
        let src = [
            wgsl_header().as_str(),
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/cells.wgsl"),
            include_str!("shaders/fire.wgsl"),
        ]
        .join("\n");
        FirePass(CellPass::new(gpu, "fire", src, &["stage_fire"]))
    }

    /// Runs one fire tick in place on `grid`'s current buffer (one dispatch).
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState) {
        let stage = Stage {
            entry: 0,
            flow: Flow::Back,
            slot: 0,
        };
        self.0.dispatch(gpu, grid, 0, 0, &[stage], false);
    }
}
