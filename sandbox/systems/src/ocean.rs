//! Ocean: a wind-driven height field of travelling waves that shapes the water surface.
//!
//! `world.waves[x]` holds the wave height in cells (positive = crest, toward smaller `y`).
//! Water above the wave-shaped surface is moved, never deleted, into the nearest column with room.

use crate::engine::System;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Largest possible wave magnitude in cells (sum of the component amplitudes).
pub const AMP_MAX: f32 = 6.0;
/// Wind speed at which waves reach full amplitude.
pub const FULL_WIND: f32 = 5.0;
/// Wave components as `(amplitude in cells, wavenumber rad/cell, angular speed rad/s)`.
const COMPONENTS: [(f32, f32, f32); 3] = [(3.0, 0.15, 1.1), (2.0, 0.31, 1.7), (1.0, 0.57, 2.6)];

/// Writes the wave height field and settles water to it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ocean {
    /// Wind speed driving the waves; amplitude scales with `clamp(wind_speed / 5, 0, 1)`.
    pub wind_speed: f32,
    /// Row of the undisturbed water surface.
    pub sea_level: i32,
}

impl System for Ocean {
    fn name(&self) -> &'static str {
        "ocean"
    }

    fn step(&mut self, w: &mut World, _rng: &mut Rng, _dt: f32) {
        let width = w.cells.width();
        let scale = if self.wind_speed.is_nan() { 0.0 } else { (self.wind_speed / FULL_WIND).clamp(0.0, 1.0) };
        w.waves.resize(width, 0.0);
        // f64 phase math keeps precision over long runs.
        let t = w.time as f64;
        for (x, out) in w.waves.iter_mut().enumerate() {
            let sum: f64 = COMPONENTS
                .iter()
                .map(|&(a, k, om)| a as f64 * (k as f64 * x as f64 - om as f64 * t).sin())
                .sum();
            *out = (sum as f32 * scale).clamp(-AMP_MAX, AMP_MAX);
        }
        settle(w, self.sea_level);
    }
}

/// Row of the first non-empty cell in column `x` (`height` if the column is empty).
fn top_row(w: &World, x: i32) -> i32 {
    let h = w.cells.height() as i32;
    (0..h).find(|&y| w.cells.get(x, y).unwrap().mat != Material::Empty).unwrap_or(h)
}

/// Row the water surface of column `x` should sit at.
fn target_row(w: &World, x: i32, sea_level: i32) -> i32 {
    sea_level - w.waves[x as usize].round() as i32
}

/// Moves one surface water cell per column that pokes above its wave level into the nearest column with room.
fn settle(w: &mut World, sea_level: i32) {
    let width = w.cells.width() as i32;
    for x in 0..width {
        let top = top_row(w, x);
        if top >= w.cells.height() as i32 || w.cells.get(x, top).unwrap().mat != Material::Water {
            continue;
        }
        if top >= target_row(w, x, sea_level) {
            continue;
        }
        let Some((dx, land)) = nearest_room(w, x, sea_level) else { continue };
        let cell = *w.cells.get(x, top).unwrap();
        w.cells.get_mut(x, top).unwrap().mat = Material::Empty;
        w.cells.get_mut(dx, land).unwrap().mat = cell.mat;
        w.cells.get_mut(dx, land).unwrap().temp = cell.temp;
    }
}

/// Nearest other column whose free cell on top of its stack lies at or below its own wave level.
fn nearest_room(w: &World, x: i32, sea_level: i32) -> Option<(i32, i32)> {
    let width = w.cells.width() as i32;
    for d in 1..width {
        for dx in [x - d, x + d] {
            if dx < 0 || dx >= width {
                continue;
            }
            let land = top_row(w, dx) - 1;
            if land >= 0 && land >= target_row(w, dx, sea_level) {
                return Some((dx, land));
            }
        }
    }
    None
}
