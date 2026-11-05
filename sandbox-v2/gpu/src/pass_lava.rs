//! Persistent lava pass (viscosity countdown, two row-parity move phases).
//! Rule defined by [`crate::reference_thermal`].
//!
//! Bindings: 1 uniform + 3 storage in the shared layout (src cells, dst
//! cells; the counter is unused).

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

    /// Runs one lava tick for `tick` with hash `seed`: countdown in place,
    /// phase 0 (current -> other), phase 1 (other -> current). Three
    /// dispatches; the result is in `grid`'s current buffer.
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState, tick: u32, seed: u32) {
        let st = |entry, flow, slot| Stage { entry, flow, slot };
        let stages = [
            st(0, Flow::Back, 0),
            st(1, Flow::Forward, 0),
            st(1, Flow::Back, 1),
        ];
        self.0.dispatch(gpu, grid, tick, seed, &stages, false);
    }
}
