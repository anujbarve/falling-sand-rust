//! Persistent lava pass (viscosity countdown, two row-parity move phases).
//! Rule defined by [`crate::reference_thermal`].
//!
//! Bindings: 1 uniform + 4 storage in the shared layout (src cells, dst
//! cells, event counters; the debug counter is unused).

use crate::pass_cells::{CellPass, Flow, Stage};
use crate::reference_thermal::wgsl_header;
use crate::{Gpu, GridState};

/// Persistent lava pass: compiled once in [`LavaPass::new`].
pub struct LavaPass(CellPass);

impl LavaPass {
    /// Compiles the lava pipelines.
    pub fn new(gpu: &Gpu) -> LavaPass {
        let src = [
            wgsl_header().as_str(),
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/cells.wgsl"),
            include_str!("shaders/hash.wgsl"),
            include_str!("shaders/lava.wgsl"),
        ]
        .join("\n");
        LavaPass(CellPass::new(
            gpu,
            "lava",
            src,
            &["stage_countdown", "stage_move"],
        ))
    }

    /// Records one lava tick for `tick` with hash `seed` into `pass`:
    /// countdown in place, phase 0 (current -> other), phase 1 (other ->
    /// current). Three dispatches; the result is in `grid`'s current buffer.
    pub fn record(
        &self,
        gpu: &Gpu,
        pass: &mut wgpu::ComputePass<'_>,
        grid: &GridState,
        tick: u32,
        seed: u32,
    ) {
        let st = |entry, flow, slot| Stage { entry, flow, slot };
        let stages = [
            st(0, Flow::Back, 0),
            st(1, Flow::Forward, 0),
            st(1, Flow::Back, 1),
        ];
        self.0.record(gpu, pass, grid, tick, seed, &stages);
    }

    /// [`LavaPass::record`] in its own encoder, then one submit.
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState, tick: u32, seed: u32) {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        self.record(
            gpu,
            &mut enc.begin_compute_pass(&Default::default()),
            grid,
            tick,
            seed,
        );
        gpu.queue.submit(Some(enc.finish()));
    }
}
