//! Liquids (water, oil, acid): fall, slide diagonally, flow sideways, and layer by density.

use crate::engine::System;
use crate::movers::{self, Kind, Tracker};
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Fluid rules for liquids, run as banded sweeps (see `movers`). Acid is only moved here;
/// its reactions belong to chemistry. Sideways flow reaches up to 4 cells per tick.
pub struct Liquid {
    /// Skip chunks where nothing changed nearby. Output is identical either way; `false` is the reference path.
    pub use_chunks: bool,
    tracker: Tracker,
}

impl Default for Liquid {
    /// Chunk skipping on.
    fn default() -> Liquid {
        Liquid { use_chunks: true, tracker: Tracker::new() }
    }
}

impl Liquid {
    /// Creates the system with chunk skipping on or off.
    pub fn with_chunks(use_chunks: bool) -> Liquid {
        Liquid { use_chunks, tracker: Tracker::new() }
    }

    /// Draws the tick seed from `rng` and sweeps the world, in parallel bands when `parallel`.
    pub(crate) fn step_with(&mut self, w: &mut World, rng: &mut Rng, parallel: bool) {
        let seed = rng.next_u32();
        movers::run(w, Kind::Liquid, seed, &mut self.tracker, self.use_chunks, parallel);
    }
}

impl System for Liquid {
    fn name(&self) -> &'static str {
        "liquid"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        self.step_with(w, rng, false);
    }
}
