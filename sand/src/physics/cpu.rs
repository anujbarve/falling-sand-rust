//! The CPU backend: the same two passes, either on one thread or spread
//! over every core with rayon. Kept as the reference the GPU is tested
//! against, and as the baseline in the benchmark.
//!
//! Like the GPU it skips sleeping chunks (see "Chunk sleeping" in mod.rs).
//! Sleeping never changes the result; `step_no_sleep` is the plain version
//! the tests compare against.

use super::{block_can_move, block_offset, chunk_dims, hash3, heat_cell, move_block, World, CHUNK};
use rayon::prelude::*;

pub struct CpuSim {
    /// Scratch grid for the heat pass. After a step it holds the state from
    /// the start of that step, which is what makes skipping the heat pass of
    /// a sleeping chunk correct (mod.rs, point 4).
    next: Vec<u32>,
    /// The world as this simulator left it after its last step. The viewer
    /// paints into `world.cells` (or swaps in a whole new world) between
    /// steps without telling us; comparing against this finds those edits.
    snapshot: Vec<u32>,
    /// `world.frame` we expect next time; anything else means the history
    /// is unknown and every chunk is woken.
    expected_frame: Option<u32>,
    width: usize,
    height: usize,
    chunks_x: usize,
    /// Per chunk: the last frame in which it was active.
    active: Vec<u32>,
    /// Per chunk: does it run in the step being computed.
    awake: Vec<bool>,
}

impl CpuSim {
    pub fn new(world: &World) -> Self {
        let mut sim = CpuSim {
            next: Vec::new(),
            snapshot: Vec::new(),
            expected_frame: None,
            width: 0,
            height: 0,
            chunks_x: 0,
            active: Vec::new(),
            awake: Vec::new(),
        };
        sim.reset(world);
        sim
    }

    /// Forget all history: every chunk counts as active in the last step,
    /// so all of them run for the next two steps.
    fn reset(&mut self, world: &World) {
        let (cw, ch) = chunk_dims(world.width, world.height);
        self.width = world.width;
        self.height = world.height;
        self.chunks_x = cw;
        self.next = vec![0; world.cells.len()];
        self.snapshot = world.cells.clone();
        self.expected_frame = Some(world.frame);
        self.active = vec![world.frame.wrapping_sub(1); cw * ch];
        self.awake = vec![true; cw * ch];
    }

    /// Total number of chunks.
    pub fn chunk_count(&self) -> usize {
        self.active.len()
    }

    /// How many chunks would run in the next step (if the world is not
    /// edited before it).
    pub fn awake_chunks(&self) -> usize {
        let frame = self.expected_frame.unwrap_or(0);
        let ch = self.active.len() / self.chunks_x.max(1);
        (0..self.active.len())
            .filter(|&c| is_awake(&self.active, self.chunks_x, ch, c % self.chunks_x, c / self.chunks_x, frame))
            .count()
    }

    /// Before a step: notice edits made behind our back, then decide which
    /// chunks run in this step.
    fn prepare(&mut self, world: &World, parallel: bool) {
        if (world.width, world.height) != (self.width, self.height) || self.expected_frame != Some(world.frame) {
            self.reset(world);
            return;
        }
        let (w, cw) = (self.width, self.chunks_x);
        let stamp = world.frame.wrapping_sub(1);
        let rows = CHUNK * w;
        let detect = |(cells, (snap, active)): (&[u32], (&[u32], &mut [u32]))| {
            mark_edits(cells, snap, active, w, stamp);
        };
        if parallel {
            world
                .cells
                .par_chunks(rows)
                .zip(self.snapshot.par_chunks(rows).zip(self.active.par_chunks_mut(cw)))
                .for_each(detect);
        } else {
            world.cells.chunks(rows).zip(self.snapshot.chunks(rows).zip(self.active.chunks_mut(cw))).for_each(detect);
        }
        let ch = self.active.len() / cw;
        for c in 0..self.active.len() {
            self.awake[c] = is_awake(&self.active, cw, ch, c % cw, c / cw, world.frame);
        }
    }

    /// After a step: remember what we left behind.
    fn finish(&mut self, world: &mut World, frame: u32, parallel: bool) {
        if parallel {
            self.snapshot.par_chunks_mut(CHUNK * self.width).zip(world.cells.par_chunks(CHUNK * self.width)).for_each(
                |(s, c)| s.copy_from_slice(c),
            );
        } else {
            self.snapshot.copy_from_slice(&world.cells);
        }
        world.frame = frame.wrapping_add(1);
        self.expected_frame = Some(world.frame);
    }

    /// One step on one thread.
    pub fn step(&mut self, world: &mut World) {
        self.step_impl(world, false);
    }

    /// One step with every CPU core. Identical result to `step`.
    pub fn step_parallel(&mut self, world: &mut World) {
        self.step_impl(world, true);
    }

    /// Both passes work one chunk row (CHUNK grid rows) at a time, so the
    /// single-threaded and the rayon versions run literally the same code:
    /// rayon just hands different chunk rows to different threads. Each task
    /// writes only its own rows and its own row of `active`, which
    /// `par_chunks_mut` hands out as separate &mut slices.
    fn step_impl(&mut self, world: &mut World, parallel: bool) {
        self.prepare(world, parallel);
        let (w, h, frame, cw) = (world.width, world.height, world.frame, self.chunks_x);
        let awake = &self.awake;

        // Heat pass: old grid `cells` in, new grid `next` out.
        let cells = &world.cells;
        let heat = |(cy, (out, active)): (usize, (&mut [u32], &mut [u32]))| {
            heat_chunk_row(cells, out, active, &awake[cy * cw..(cy + 1) * cw], w, h, cy, frame);
        };
        if parallel {
            self.next.par_chunks_mut(CHUNK * w).zip(self.active.par_chunks_mut(cw)).enumerate().for_each(heat);
        } else {
            self.next.chunks_mut(CHUNK * w).zip(self.active.chunks_mut(cw)).enumerate().for_each(heat);
        }
        // The new grid becomes the current one. Swapping two Vecs only swaps
        // their pointers: no cells are copied.
        std::mem::swap(&mut world.cells, &mut self.next);

        // Move pass, in place. Chunk row cy owns the blocks whose top-left
        // cell is in grid rows cy*CHUNK .. cy*CHUNK+CHUNK, so its slice
        // starts at row cy*CHUNK + o and is CHUNK rows long (the last block
        // row reaches one row into the next chunk row: that is allowed, the
        // slices are cut from o onwards so they still do not overlap).
        let o = block_offset(frame);
        let moves = |(cy, (rows, active)): (usize, (&mut [u32], &mut [u32]))| {
            move_chunk_row(rows, active, &awake[cy * cw..(cy + 1) * cw], w, cy, o, frame);
        };
        let grid = &mut world.cells[o * w..];
        if parallel {
            grid.par_chunks_mut(CHUNK * w).zip(self.active.par_chunks_mut(cw)).enumerate().for_each(moves);
        } else {
            grid.chunks_mut(CHUNK * w).zip(self.active.chunks_mut(cw)).enumerate().for_each(moves);
        }
        self.finish(world, frame, parallel);
    }

    /// One step with no chunk sleeping at all: every cell, every block.
    /// The reference that the sleeping versions must match bit for bit.
    pub fn step_no_sleep(&mut self, world: &mut World) {
        let (w, h, frame) = (world.width, world.height, world.frame);
        if self.next.len() != world.cells.len() {
            self.next = vec![0; world.cells.len()];
        }
        for y in 0..h {
            for x in 0..w {
                self.next[y * w + x] = heat_cell(&world.cells, w, h, x, y, frame).0;
            }
        }
        std::mem::swap(&mut world.cells, &mut self.next);
        let o = block_offset(frame);
        for by in 0..h / 2 {
            let y = 2 * by + o;
            if y + 1 >= h {
                continue;
            }
            for bx in 0..w / 2 {
                let x = 2 * bx + o;
                if x + 1 >= w {
                    continue;
                }
                let idx = [y * w + x, y * w + x + 1, (y + 1) * w + x, (y + 1) * w + x + 1];
                let mut cells = idx.map(|i| world.cells[i]);
                move_block(&mut cells, hash3(bx as u32, by as u32, frame));
                for k in 0..4 {
                    world.cells[idx[k]] = cells[k];
                }
            }
        }
        world.frame = frame.wrapping_add(1);
        // Our sleeping history no longer matches this world.
        self.expected_frame = None;
    }
}

/// Chunk (cx, cy) runs in step `frame` if it or a neighbour was active in
/// step frame-1 or frame-2.
fn is_awake(active: &[u32], cw: usize, ch: usize, cx: usize, cy: usize, frame: u32) -> bool {
    let last = frame.wrapping_sub(1);
    for ny in cy.saturating_sub(1)..(cy + 2).min(ch) {
        for nx in cx.saturating_sub(1)..(cx + 2).min(cw) {
            if last.wrapping_sub(active[ny * cw + nx]) < 2 {
                return true;
            }
        }
    }
    false
}

/// One chunk row: mark each chunk whose cells differ from the snapshot as
/// active in the previous step (so it and its neighbours wake up).
fn mark_edits(cells: &[u32], snap: &[u32], active: &mut [u32], w: usize, stamp: u32) {
    for (cx, a) in active.iter_mut().enumerate() {
        let (x0, x1) = (cx * CHUNK, ((cx + 1) * CHUNK).min(w));
        if cells.chunks(w).zip(snap.chunks(w)).any(|(r, s)| r[x0..x1] != s[x0..x1]) {
            *a = stamp;
        }
    }
}

/// Heat pass for one chunk row. `out` is the rows cy*CHUNK.. of the new
/// grid; `cells` is the whole old grid (heat reads one row above and below).
#[allow(clippy::too_many_arguments)]
fn heat_chunk_row(
    cells: &[u32],
    out: &mut [u32],
    active: &mut [u32],
    awake: &[bool],
    w: usize,
    h: usize,
    cy: usize,
    frame: u32,
) {
    let (y0, rows) = (cy * CHUNK, out.len() / w);
    for cx in 0..awake.len() {
        if !awake[cx] {
            continue; // `out` already holds these cells (mod.rs, point 4)
        }
        let (x0, x1) = (cx * CHUNK, ((cx + 1) * CHUNK).min(w));
        let mut changed = false;
        for ly in 0..rows {
            let y = y0 + ly;
            for x in x0..x1 {
                // Active if it changed, or if a random number could have
                // changed it (stochastic rounding, fire, gravity): see mod.rs.
                let (v, maybe) = heat_cell(cells, w, h, x, y, frame);
                changed |= maybe;
                out[ly * w + x] = v;
            }
        }
        if changed {
            active[cx] = frame;
        }
    }
}

/// Move pass for the blocks owned by one chunk row. `rows` starts at grid
/// row cy*CHUNK + o.
fn move_chunk_row(rows: &mut [u32], active: &mut [u32], awake: &[bool], w: usize, cy: usize, o: usize, frame: u32) {
    let n_rows = rows.len() / w;
    let half = CHUNK / 2; // blocks per chunk side
    for cx in 0..awake.len() {
        if !awake[cx] {
            continue;
        }
        let mut live = false;
        for lb in 0..half {
            let ly = 2 * lb;
            if ly + 1 >= n_rows {
                break; // a lone last row has no partner row
            }
            let by = cy * half + lb;
            for bx in cx * half..((cx + 1) * half).min(w / 2) {
                let x = 2 * bx + o;
                if x + 1 >= w {
                    continue;
                }
                let idx = [ly * w + x, ly * w + x + 1, (ly + 1) * w + x, (ly + 1) * w + x + 1];
                let mut cells = idx.map(|i| rows[i]);
                // A block that cannot move for any random number is left as
                // it is (same result as running move_block, but no writes).
                if !block_can_move(&cells) {
                    continue;
                }
                live = true;
                move_block(&mut cells, hash3(bx as u32, by as u32, frame));
                for k in 0..4 {
                    rows[idx[k]] = cells[k];
                }
            }
        }
        if live {
            active[cx] = frame;
        }
    }
}
