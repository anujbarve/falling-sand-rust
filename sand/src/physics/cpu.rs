//! The CPU backend: the same two passes, either on one thread or spread
//! over every core with rayon. Kept as the reference the GPU is tested
//! against, and as the baseline in the benchmark.

use super::{block_offset, hash3, heat_cell, move_block, World};
use rayon::prelude::*;

/// Scratch buffer for the heat pass, which needs an old and a new grid.
pub struct CpuSim {
    next: Vec<u32>,
}

impl CpuSim {
    pub fn new(world: &World) -> Self {
        CpuSim { next: vec![0; world.cells.len()] }
    }

    /// One step on one thread.
    pub fn step(&mut self, world: &mut World) {
        let (w, h, frame) = (world.width, world.height, world.frame);
        for y in 0..h {
            for x in 0..w {
                self.next[y * w + x] = heat_cell(&world.cells, w, h, x, y, frame);
            }
        }
        // The new grid becomes the current one. Swapping two Vecs only swaps
        // their pointers: no cells are copied.
        std::mem::swap(&mut world.cells, &mut self.next);

        let o = block_offset(frame);
        for by in 0..h / 2 {
            let y = 2 * by + o;
            if y + 1 >= h {
                continue;
            }
            let (top, bottom) = (y * w, (y + 1) * w);
            for bx in 0..w / 2 {
                let x = 2 * bx + o;
                if x + 1 >= w {
                    continue;
                }
                let idx = [top + x, top + x + 1, bottom + x, bottom + x + 1];
                let mut cells = idx.map(|i| world.cells[i]);
                move_block(&mut cells, hash3(bx as u32, by as u32, frame));
                for k in 0..4 {
                    world.cells[idx[k]] = cells[k];
                }
            }
        }
        world.frame = frame.wrapping_add(1);
    }

    /// One step with every CPU core. Identical result to `step`.
    pub fn step_parallel(&mut self, world: &mut World) {
        let (w, h, frame) = (world.width, world.height, world.frame);

        // Heat pass: one row per task. Each task writes only its own row of
        // the new grid and only reads the old grid, so rayon can hand the
        // rows to different threads safely. The borrow checker proves this:
        // par_chunks_mut gives each task a separate &mut slice.
        let cells = &world.cells;
        self.next.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                row[x] = heat_cell(cells, w, h, x, y, frame);
            }
        });
        std::mem::swap(&mut world.cells, &mut self.next);

        // Move pass: one row of blocks (= 2 grid rows) per task.
        let o = block_offset(frame);
        world.cells[o * w..].par_chunks_mut(2 * w).enumerate().for_each(|(by, rows)| {
            if rows.len() < 2 * w {
                return; // a lone last row has no partner row
            }
            for bx in 0..w / 2 {
                let x = 2 * bx + o;
                if x + 1 >= w {
                    continue;
                }
                let idx = [x, x + 1, w + x, w + x + 1];
                let mut cells = idx.map(|i| rows[i]);
                move_block(&mut cells, hash3(bx as u32, by as u32, frame));
                for k in 0..4 {
                    rows[idx[k]] = cells[k];
                }
            }
        });
        world.frame = frame.wrapping_add(1);
    }
}
