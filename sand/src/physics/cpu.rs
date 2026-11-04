//! The CPU backend: the same two passes, either on one thread or spread
//! over every core with rayon.

use super::{block_offset, hash3, heat_cell, move_block, World};
use rayon::prelude::*;

/// Scratch buffers for the heat pass, which needs an old and a new grid.
pub struct CpuSim {
    next_mat: Vec<u32>,
    next_temp: Vec<i32>,
}

impl CpuSim {
    pub fn new(world: &World) -> Self {
        CpuSim {
            next_mat: vec![0; world.mat.len()],
            next_temp: vec![0; world.temp.len()],
        }
    }

    /// One step on one thread.
    pub fn step(&mut self, world: &mut World) {
        let (w, h, frame) = (world.width, world.height, world.frame);
        for y in 0..h {
            for x in 0..w {
                let (m, t) = heat_cell(&world.mat, &world.temp, w, h, x, y, frame);
                self.next_mat[y * w + x] = m;
                self.next_temp[y * w + x] = t;
            }
        }
        // The new grid becomes the current one. Swapping two Vecs only swaps
        // their pointers: no cells are copied.
        std::mem::swap(&mut world.mat, &mut self.next_mat);
        std::mem::swap(&mut world.temp, &mut self.next_temp);

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
                let mut cells = idx.map(|i| world.mat[i]);
                let mut temps = idx.map(|i| world.temp[i]);
                move_block(&mut cells, &mut temps, hash3(bx as u32, by as u32, frame));
                for k in 0..4 {
                    world.mat[idx[k]] = cells[k];
                    world.temp[idx[k]] = temps[k];
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
        let (mat, temp) = (&world.mat, &world.temp);
        self.next_mat
            .par_chunks_mut(w)
            .zip(self.next_temp.par_chunks_mut(w))
            .enumerate()
            .for_each(|(y, (mrow, trow))| {
                for x in 0..w {
                    let (m, t) = heat_cell(mat, temp, w, h, x, y, frame);
                    mrow[x] = m;
                    trow[x] = t;
                }
            });
        std::mem::swap(&mut world.mat, &mut self.next_mat);
        std::mem::swap(&mut world.temp, &mut self.next_temp);

        // Move pass: one row of blocks (= 2 grid rows) per task.
        let o = block_offset(frame);
        world.mat[o * w..]
            .par_chunks_mut(2 * w)
            .zip(world.temp[o * w..].par_chunks_mut(2 * w))
            .enumerate()
            .for_each(|(by, (mrows, trows))| {
                if mrows.len() < 2 * w {
                    return; // a lone last row has no partner row
                }
                for bx in 0..w / 2 {
                    let x = 2 * bx + o;
                    if x + 1 >= w {
                        continue;
                    }
                    let idx = [x, x + 1, w + x, w + x + 1];
                    let mut cells = idx.map(|i| mrows[i]);
                    let mut temps = idx.map(|i| trows[i]);
                    move_block(&mut cells, &mut temps, hash3(bx as u32, by as u32, frame));
                    for k in 0..4 {
                        mrows[idx[k]] = cells[k];
                        trows[idx[k]] = temps[k];
                    }
                }
            });
        world.frame = frame.wrapping_add(1);
    }
}
