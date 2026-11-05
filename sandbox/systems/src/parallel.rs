//! Multi-threaded granular + liquid tick: horizontal row bands on rayon, same result on any thread count.
//!
//! Bands are `split_at_mut`-style disjoint slices of the cell array (via `par_chunks_mut`), so no
//! `unsafe` is needed. Results equal the serial `Granular` + `Liquid` systems exactly, because both
//! share one banded algorithm with hashed per-cell randomness and no cross-band reductions.

use crate::granular::Granular;
use crate::liquid::Liquid;
use rayon::prelude::*;
use sandbox_core::rng::Rng;
use sandbox_core::time::DT;
use sandbox_core::world::World;

/// Persistent state (dirty-chunk maps) for the parallel granular and liquid passes.
#[derive(Default)]
pub struct ParallelMovers {
    /// Sand rules.
    pub granular: Granular,
    /// Water, oil and acid rules.
    pub liquid: Liquid,
}

/// Parallel equivalent of `run_tick` over `[Granular, Liquid]`: advances `world.time` by `DT`,
/// clears every `updated` flag, then moves sand and liquids. Runs on the current rayon pool
/// (use `ThreadPool::install` to pick a thread count); other systems are not run.
pub fn tick_parallel(world: &mut World, movers: &mut ParallelMovers, rng: &mut Rng) {
    world.time += DT;
    world.cells.as_mut_slice().par_iter_mut().for_each(|c| c.updated = false);
    movers.granular.step_with(world, rng, true);
    movers.liquid.step_with(world, rng, true);
}
