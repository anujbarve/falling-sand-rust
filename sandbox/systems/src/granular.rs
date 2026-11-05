//! Granular solids (currently sand): fall, slide diagonally, sink through lighter liquids.

use crate::engine::System;
use crate::movers::{self, Kind, Tracker};
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Relative density of materials that can be displaced or that displace others.
/// `None` means the material is not part of the density-swap rules (solids, gases).
/// Order: Empty < Oil < Acid < Water < Sand.
pub(crate) fn density(m: Material) -> Option<u8> {
    match m {
        Material::Empty => Some(0),
        Material::Oil => Some(1),
        Material::Acid => Some(2),
        Material::Water => Some(3),
        Material::Sand => Some(4),
        _ => None,
    }
}

/// Swaps two in-bounds cells wholesale and marks both as updated this tick.
pub(crate) fn swap(w: &mut World, a: (i32, i32), b: (i32, i32)) {
    let (ia, ib) = (w.cells.index(a.0, a.1).unwrap(), w.cells.index(b.0, b.1).unwrap());
    let cells = w.cells.as_mut_slice();
    cells.swap(ia, ib);
    cells[ia].updated = true;
    cells[ib].updated = true;
}

/// Falling-sand rules for granular materials, run as banded sweeps (see `movers`).
pub struct Granular {
    /// Skip chunks where nothing changed nearby. Output is identical either way; `false` is the reference path.
    pub use_chunks: bool,
    tracker: Tracker,
}

impl Default for Granular {
    /// Chunk skipping on.
    fn default() -> Granular {
        Granular { use_chunks: true, tracker: Tracker::new() }
    }
}

impl Granular {
    /// Creates the system with chunk skipping on or off.
    pub fn with_chunks(use_chunks: bool) -> Granular {
        Granular { use_chunks, tracker: Tracker::new() }
    }

    /// Draws the tick seed from `rng` and sweeps the world, in parallel bands when `parallel`.
    pub(crate) fn step_with(&mut self, w: &mut World, rng: &mut Rng, parallel: bool) {
        let seed = rng.next_u32();
        movers::run(w, Kind::Granular, seed, &mut self.tracker, self.use_chunks, parallel);
    }
}

impl System for Granular {
    fn name(&self) -> &'static str {
        "granular"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        self.step_with(w, rng, false);
    }
}
