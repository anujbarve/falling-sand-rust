//! Part of the physics engine (see mod.rs).

use super::*;

// ---------------------------------------------------------------------------
// Chunk sleeping: skip regions where nothing can happen.
//
// CHOICE: sleeping is a pure optimisation. It never changes the result: a
// chunk is only skipped when every one of its cells provably comes out of the
// step unchanged. A run with sleeping is bit-identical to a run without it
// (tests check this), so the CPU and the GPU agree no matter which chunks each
// one happens to skip.
//
// The grid is cut into fixed CHUNK x CHUNK chunks (independent of the block
// offset). During step n a chunk is ACTIVE if
//   - the heat pass of one of its cells changed it OR could have changed it
//     for SOME random number (`heat_cell`'s second result: any temperature
//     difference to a neighbour, a gas away from 20°C, fire, smoke, or a
//     cell free to fall, all of which use random rounding), or
//   - a 2x2 block it OWNS (the chunk holding the block's top-left cell) could
//     move for SOME random number (`block_can_move`), whether it moved or not.
// Counting "could" and not just "did" matters: a water/oil pair that
// happened not to swap for two steps is not settled, it is unlucky.
//
// Rule: chunk c is AWAKE in step n if c or any of its 8 neighbours was active
// in step n-1 or n-2. Asleep chunks run neither pass (no reads, no writes).
//
// Why skipping an asleep chunk c in step n is exact (by induction on n):
// 1. Every cell within 2 cells of c lies in c's 3x3 chunk neighbourhood N(c)
//    (CHUNK >= 3). Such a cell can only be changed by its own heat update or
//    by a block whose top-left is within 3 cells of c, i.e. owned by a chunk
//    in N(c). So "N(c) inactive in steps n-1 and n-2" means those cells are
//    the same at the start of steps n-2, n-1 and n.
// 2. Heat: a cell's heat update reads it and its 4 neighbours (all within 2
//    of c). An update that could not change the cell for any random number
//    in step n-1 sees the same inputs in step n, so it cannot change it now
//    either. So the post-heat values within 1 cell of c equal those of
//    step n-2.
// 3. Move: step n uses the same block offset as step n-2. Every block that
//    touches c (including blocks owned by an awake neighbour that straddle
//    into c) has its top-left in N(c), sees the same post-heat cells as in
//    step n-2, and could not move then for any random number => cannot now.
// 4. Ping-pong buffers: the heat pass writes into the buffer that holds the
//    state from the start of step n-1. c did not change in step n-1, so that
//    buffer already holds exactly what the skipped heat pass would write.
//    (The CPU swaps two Vecs the same way, so the same argument holds.)
// Waking is always safe, only sleeping needs the argument above. When the
// history is unknown (a fresh upload, a world painted or swapped behind the
// simulator's back) the touched chunks are marked active in the previous
// step, which wakes them and their neighbours for two steps, and both
// buffers get rewritten there before anything sleeps.
//
// The bookkeeping is a per-chunk stamp: the last frame number in which the
// chunk was active. "Active in step n-1 or n-2" is `(n - 1) - stamp < 2`,
// in wrapping u32 maths. (A stale stamp that wraps around after 2^32 steps
// only wakes a chunk for nothing, which is harmless.)
// ---------------------------------------------------------------------------

/// Chunk side, in cells. `sim.wgsl` repeats it; its workgroups map onto
/// chunks (a 16x16 heat workgroup is a quarter chunk; a 16x16 move
/// workgroup of 2x2 blocks is exactly one chunk), so keep it at 32.
pub const CHUNK: usize = 32;

/// Number of chunks across and down for a `w` x `h` grid.
pub fn chunk_dims(w: usize, h: usize) -> (usize, usize) {
    (w.div_ceil(CHUNK), h.div_ceil(CHUNK))
}

/// Could `move_block` change this block for SOME random number `r`?
/// Each candidate swap is checked on the input: if no column swap is
/// possible the later stages see the same cells, so this is exact. When it
/// is false, `move_block` leaves the block as it is.
pub fn block_can_move(cells: &[u32; 4]) -> bool {
    pair_chance(cells[0], cells[2]) > 0
        || pair_chance(cells[1], cells[3]) > 0
        || can_slide(cells[0], cells[2], cells[3])
        || can_slide(cells[1], cells[3], cells[2])
        || can_steam_vent(cells[2], cells[0], cells[3])
        || can_steam_vent(cells[3], cells[1], cells[2])
        || spread_chance(cells[0], cells[1]) > 0
        || spread_chance(cells[2], cells[3]) > 0
}

