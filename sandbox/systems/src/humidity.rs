//! Humidity: steam dissolves into air vapor, hot water evaporates, cold humid air condenses to water.
//!
//! One Steam or Water cell is worth exactly 1.0 vapor, so every transition conserves mass.
//! Rain needs no code here: condensed Water is an ordinary liquid cell that `Liquid` makes fall.

use crate::engine::System;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Water hotter than this slowly evaporates into vapor.
pub const EVAP_TEMP: f32 = 350.0;
/// Vapor in a cell colder than this may condense.
pub const DEW_POINT: f32 = 285.0;
/// A cell's own vapor must exceed this before it condenses.
pub const CONDENSE_VAPOR: f32 = 0.8;
/// Chance per tick that a hot water cell evaporates.
const EVAP_CHANCE: f32 = 0.05;
/// Vapor equivalent of one Steam or Water cell.
const CELL_MASS: f32 = 1.0;
/// Float slack when checking that enough vapor was gathered.
const EPS: f32 = 1e-5;

/// Converts between Steam, Water and air vapor.
pub struct Humidity;

impl System for Humidity {
    fn name(&self) -> &'static str {
        "humidity"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
        for y in 0..height {
            for x in 0..width {
                let i = w.cells.index(x, y).unwrap();
                let c = *w.cells.get(x, y).unwrap();
                match c.mat {
                    // A cell that already holds vapor keeps its steam: absorbing it would exceed 1.0.
                    Material::Steam if w.air.vapor[i] <= 0.0 => {
                        w.cells.get_mut(x, y).unwrap().mat = Material::Empty;
                        w.air.vapor[i] = CELL_MASS;
                    }
                    Material::Water if c.temp > EVAP_TEMP && w.air.vapor[i] <= 0.0 && rng.chance(EVAP_CHANCE) => {
                        w.cells.get_mut(x, y).unwrap().mat = Material::Empty;
                        w.air.vapor[i] = CELL_MASS;
                    }
                    Material::Empty if c.temp < DEW_POINT && w.air.vapor[i] > CONDENSE_VAPOR => {
                        condense(w, x, y, i);
                    }
                    _ => {}
                }
            }
        }
        for v in w.air.vapor.iter_mut() {
            *v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
        }
    }
}

/// Turns the cell at `(x, y)` into Water, drawing 1.0 vapor from itself first and then its
/// 4-neighbours. Does nothing if the surroundings cannot supply a full unit.
fn condense(w: &mut World, x: i32, y: i32, i: usize) {
    let mut taken = [(i, w.air.vapor[i].min(CELL_MASS)), (0, 0.0), (0, 0.0), (0, 0.0), (0, 0.0)];
    let mut total = taken[0].1;
    for (n, (dx, dy)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
        if let Some(j) = w.cells.index(x + dx, y + dy) {
            let take = w.air.vapor[j].min(CELL_MASS - total).max(0.0);
            taken[n + 1] = (j, take);
            total += take;
        }
    }
    if total < CELL_MASS - EPS {
        return;
    }
    for (j, take) in taken {
        w.air.vapor[j] -= take;
    }
    w.cells.get_mut(x, y).unwrap().mat = Material::Water;
}
