//! Fire: burns down its fuel, heats neighbours, and spreads to flammable cells.

use crate::engine::System;
use sandbox_core::cell::Cell;
use sandbox_core::event::Event;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Base per-tick, per-neighbour ignition chance with no wind and no water.
pub const IGNITE_P: f32 = 0.15;
/// Heat added to each neighbour per tick by a burning cell.
pub const HEAT: f32 = 40.0;
/// Temperature cap for heat emitted by fire.
pub const MAX_TEMP: f32 = 1500.0;
/// Fuel given to a flammable cell that is ignited with no fuel of its own.
pub const DEFAULT_FUEL: u8 = 60;
/// Chance per tick that a burning cell releases Smoke into an empty cell above.
pub const SMOKE_P: f32 = 0.05;
/// Chance that a burnt-out cell with no flammable neighbour left becomes Ember instead of Ash.
pub const EMBER_P: f32 = 0.05;

const ORTHO: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const NEIGHBOURS: [(i32, i32); 8] =
    [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// Burning system. Consumes `Event::Ignite`; a Fire cell burns `fuel` ticks, then becomes Ash
/// (or, rarely and only with no flammable neighbour left, Ember).
#[derive(Default)]
pub struct Fire {
    /// Scratch list of burning cells, reused across ticks.
    burning: Vec<(i32, i32)>,
}

fn flammable(m: Material) -> bool {
    matches!(m, Material::Wood | Material::Gunpowder)
}

fn ignite(w: &mut World, x: i32, y: i32) {
    if let Some(c) = w.cells.get_mut(x, y) {
        if flammable(c.mat) {
            c.mat = Material::Fire;
            if c.fuel == 0 {
                c.fuel = DEFAULT_FUEL;
            }
        }
    }
}

impl System for Fire {
    fn name(&self) -> &'static str {
        "fire"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        let events = std::mem::take(&mut w.events);
        for e in events {
            match e {
                Event::Ignite { x, y } => ignite(w, x, y),
                other => w.events.push(other),
            }
        }

        let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
        // Snapshot so cells ignited this tick start acting next tick.
        self.burning.clear();
        for y in 0..height {
            for x in 0..width {
                if w.cells.get(x, y).map(|c| c.mat) == Some(Material::Fire) {
                    self.burning.push((x, y));
                }
            }
        }

        for k in 0..self.burning.len() {
            let (x, y) = self.burning[k];
            let wind = w.cells.index(x, y).map(|i| w.air.vel[i]).unwrap_or_default().normalized();
            for (dx, dy) in NEIGHBOURS {
                let (nx, ny) = (x + dx, y + dy);
                let Some(c) = w.cells.get_mut(nx, ny) else { continue };
                c.temp = (c.temp + HEAT).min(MAX_TEMP);
                if !flammable(c.mat) {
                    continue;
                }
                let len = ((dx * dx + dy * dy) as f32).sqrt();
                let dot = ((dx as f32 * wind.x + dy as f32 * wind.y) / len).clamp(-0.5, 1.0);
                let wet = ORTHO
                    .iter()
                    .filter(|(wx, wy)| w.cells.get(nx + wx, ny + wy).map(|c| c.mat) == Some(Material::Water))
                    .count();
                let p = IGNITE_P * (1.0 + dot) / (1.0 + wet as f32);
                if rng.chance(p) {
                    ignite(w, nx, ny);
                }
            }

            if w.cells.get(x, y - 1).map(|c| c.mat) == Some(Material::Empty) && rng.chance(SMOKE_P) {
                let t = w.cells.get(x, y).unwrap().temp;
                w.cells.set(x, y - 1, Cell { mat: Material::Smoke, temp: t, fuel: 0, updated: false });
            }

            let c = w.cells.get_mut(x, y).unwrap();
            c.fuel = c.fuel.saturating_sub(1);
            if c.fuel == 0 {
                c.mat = Material::Ash;
                let ember = rng.chance(EMBER_P)
                    && !NEIGHBOURS.iter().any(|(dx, dy)| {
                        w.cells.get(x + dx, y + dy).is_some_and(|n| flammable(n.mat))
                    });
                if ember {
                    w.cells.get_mut(x, y).unwrap().mat = Material::Ember;
                }
            }
        }
    }
}
