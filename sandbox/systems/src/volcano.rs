//! Volcano sources: emit hot lava and move it as a viscous liquid.
//!
//! Cooling lava into rock is `Thermal`'s job (its `PHASES` table); this module
//! only emits lava at 1500 and moves it.

use crate::engine::System;
use crate::granular::swap;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::time::DT;
use sandbox_core::world::World;

/// Temperature of freshly emitted lava.
pub const LAVA_TEMP: f32 = 1500.0;

/// Lava moves diagonally or sideways only on every this-many-th tick.
pub const VISCOSITY: u64 = 4;

/// A point that emits lava into the cell directly above it.
#[derive(Clone, Copy, Debug)]
pub struct VolcanoSource {
    /// Column of the source; off-grid sources emit nothing.
    pub x: i32,
    /// Row of the source; lava appears at `y - 1`.
    pub y: i32,
    /// Lava cells emitted per tick (fractions accumulate).
    pub rate: f32,
}

/// Emits lava from its sources and flows every Lava cell.
#[derive(Default)]
pub struct Volcano {
    /// Active lava sources.
    pub sources: Vec<VolcanoSource>,
}

/// Moves lava from `a` to the empty cell `b`; only the destination stays marked,
/// so lava queued behind it can follow in the same tick (a falling column stays dense).
fn mv(w: &mut World, a: (i32, i32), b: (i32, i32)) {
    swap(w, a, b);
    if let Some(c) = w.cells.get_mut(a.0, a.1) {
        c.updated = false;
    }
}

/// Lava motion: falls straight every tick; slides diagonally or sideways into empty cells every `VISCOSITY`th tick.
fn flow(w: &mut World, rng: &mut Rng, slow_tick: bool) {
    let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
    let open = |w: &World, x: i32, y: i32| w.cells.get(x, y).is_some_and(|c| c.mat == Material::Empty && !c.updated);
    for y in (0..height).rev() {
        let left_first = rng.chance(0.5);
        for i in 0..width {
            let x = if left_first { i } else { width - 1 - i };
            let c = *w.cells.get(x, y).unwrap();
            if c.updated || c.mat != Material::Lava {
                continue;
            }
            if open(w, x, y + 1) {
                mv(w, (x, y), (x, y + 1));
                continue;
            }
            if !slow_tick {
                continue;
            }
            let dir = if rng.chance(0.5) { -1 } else { 1 };
            if let Some(dx) = [dir, -dir].into_iter().find(|&dx| open(w, x + dx, y + 1)) {
                mv(w, (x, y), (x + dx, y + 1));
            } else if let Some(dx) = [dir, -dir].into_iter().find(|&dx| open(w, x + dx, y)) {
                mv(w, (x, y), (x + dx, y));
            }
        }
    }
}

impl System for Volcano {
    fn name(&self) -> &'static str {
        "volcano"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        // Tick index recovered from world time keeps the sources stateless.
        let n = (w.time / DT).round().max(0.0) as u64;
        flow(w, rng, n % VISCOSITY == 0);
        for s in &self.sources {
            // Whole units of accumulated rate crossed this tick.
            let emit = (n as f64 * s.rate as f64).floor() - (n.saturating_sub(1) as f64 * s.rate as f64).floor();
            if emit < 1.0 {
                continue;
            }
            if let Some(c) = w.cells.get_mut(s.x, s.y - 1) {
                if c.mat == Material::Empty {
                    c.mat = Material::Lava;
                    c.temp = LAVA_TEMP;
                    c.fuel = 0;
                    c.updated = true;
                }
            }
        }
    }
}
