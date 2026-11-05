//! Persistent thermal pass (conduction, moisture, phase changes). Rule
//! defined by [`crate::reference_thermal`].
//!
//! Bindings: 1 uniform + 3 storage (src cells, dst cells, debug counter).

use crate::pass_cells::{CellPass, Flow, Stage};
use crate::reference_thermal::wgsl_header;
use crate::{Gpu, GridState};

/// Persistent thermal pass: compiled once in [`ThermalPass::new`].
pub struct ThermalPass(CellPass);

impl ThermalPass {
    /// Compiles the thermal pipeline.
    pub fn new(gpu: &Gpu) -> ThermalPass {
        let src = [
            wgsl_header().as_str(),
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/cells.wgsl"),
            include_str!("shaders/thermal.wgsl"),
        ]
        .join("\n");
        ThermalPass(CellPass::new(gpu, "thermal", src, &["stage_thermal"]))
    }

    /// Runs one thermal tick (current -> other, then copied back), so the
    /// result is in `grid`'s current buffer. One dispatch plus one copy.
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState) {
        let stage = Stage {
            entry: 0,
            flow: Flow::Forward,
            slot: 0,
        };
        self.0.dispatch(gpu, grid, 0, 0, &[stage], true);
    }

    /// Number of non-finite `temp`/`fuel` values reset so far (blocking
    /// readback, intended for tests).
    pub fn sanitized_count(&self, gpu: &Gpu) -> u32 {
        self.0.counter(gpu)
    }
}
