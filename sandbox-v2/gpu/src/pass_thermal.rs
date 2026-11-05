//! Persistent thermal pass (conduction, moisture, phase changes). Rule
//! defined by [`crate::reference_thermal`].
//!
//! Bindings: 1 uniform + 4 storage (src cells, dst cells, debug counter,
//! event counters).
//!
//! The pass writes current -> other and then flips the grid (no copy back),
//! so it takes `&mut GridState`.

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

    /// Records one thermal tick (current -> other) into `pass` and flips
    /// `grid`, so the result is the current buffer once the pass runs.
    pub fn record(&self, gpu: &Gpu, pass: &mut wgpu::ComputePass<'_>, grid: &mut GridState) {
        let stage = Stage {
            entry: 0,
            flow: Flow::Forward,
            slot: 0,
        };
        self.0.record(gpu, pass, grid, 0, 0, &[stage]);
        grid.swap();
    }

    /// [`ThermalPass::record`] in its own encoder, then one submit. One
    /// dispatch, no copy.
    pub fn dispatch(&self, gpu: &Gpu, grid: &mut GridState) {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        self.record(gpu, &mut enc.begin_compute_pass(&Default::default()), grid);
        gpu.queue.submit(Some(enc.finish()));
    }

    /// Number of non-finite `temp`/`fuel` values reset so far (blocking
    /// readback, intended for tests).
    pub fn sanitized_count(&self, gpu: &Gpu) -> u32 {
        self.0.counter(gpu)
    }
}
