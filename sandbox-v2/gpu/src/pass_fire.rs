//! Persistent fire pass (ignition, burn countdown). Rule defined by
//! [`crate::reference_thermal`].
//!
//! Bindings: 1 uniform + 4 storage in the shared layout; the shader uses only
//! the destination cells (in place) and the event counters.

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

    /// Records one fire tick, in place on `grid`'s current buffer, into `pass`.
    pub fn record(&self, gpu: &Gpu, pass: &mut wgpu::ComputePass<'_>, grid: &GridState) {
        let stage = Stage {
            entry: 0,
            flow: Flow::Back,
            slot: 0,
        };
        self.0.record(gpu, pass, grid, 0, 0, &[stage]);
    }

    /// Runs one fire tick in place on `grid`'s current buffer (one dispatch,
    /// one submit).
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState) {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        self.record(gpu, &mut enc.begin_compute_pass(&Default::default()), grid);
        gpu.queue.submit(Some(enc.finish()));
    }
}
