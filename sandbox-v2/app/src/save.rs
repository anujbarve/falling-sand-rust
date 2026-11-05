//! Save and load in the v1 text format, via `sandbox_core::save`.
//!
//! A [`Snapshot`] is what the GPU readback gives (cells plus air velocity).
//! Mapping to v1 `World`: `mat`, `temp` exact; `fuel` (an f32 timer in v2)
//! is rounded and clamped to v1's `u8` (LOSSY above 255 ticks: a fire
//! timer or stored latent heat beyond that is cut); air velocity, pressure
//! and vapor exact. Per-cell fall state (`vx`, `vy`) and `flags` are not in
//! the format and load as 0. Material ids outside v1's 18 save as Stone.

use std::path::Path;

use sandbox_core::cell::Cell;
use sandbox_core::material::Material as V1;
use sandbox_core::save::{load_world, save_world, SaveError};
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use sandbox_v2_gpu::CellRaw;

/// v1 materials by id (the ids of v2 `Material` mirror these).
const V1_BY_ID: [V1; 18] = [
    V1::Empty,
    V1::Sand,
    V1::Water,
    V1::Oil,
    V1::Stone,
    V1::Rock,
    V1::Wood,
    V1::Ash,
    V1::Metal,
    V1::Glass,
    V1::Lava,
    V1::Acid,
    V1::Gunpowder,
    V1::Ice,
    V1::Fire,
    V1::Steam,
    V1::Smoke,
    V1::Ember,
];

/// The saved part of the sim state.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Grid width in cells.
    pub w: u32,
    /// Grid height in cells.
    pub h: u32,
    /// Row-major cells.
    pub cells: Vec<CellRaw>,
    /// Row-major air velocity, cells/s.
    pub air_vel: Vec<[f32; 2]>,
}

/// The v1 fuel byte for a v2 timer value (rounded, clamped to 0..=255).
pub fn fuel_byte(fuel: f32) -> u8 {
    fuel.round().clamp(0.0, 255.0) as u8
}

/// Converts a snapshot to a v1 world.
pub fn to_world(s: &Snapshot) -> World {
    let mut w = World::new(s.w as usize, s.h as usize);
    for (dst, c) in w.cells.as_mut_slice().iter_mut().zip(&s.cells) {
        *dst = Cell {
            mat: *V1_BY_ID.get(c.mat as usize).unwrap_or(&V1::Stone),
            temp: c.temp,
            fuel: fuel_byte(c.fuel),
            updated: false,
        };
    }
    for (i, c) in s.cells.iter().enumerate() {
        w.air.pressure[i] = c.pressure;
        w.air.vapor[i] = c.vapor;
        w.air.vel[i] = Vec2 {
            x: s.air_vel[i][0],
            y: s.air_vel[i][1],
        };
    }
    w
}

/// Converts a v1 world to a snapshot (fall state and flags 0).
pub fn from_world(w: &World) -> Snapshot {
    let cells = w
        .cells
        .as_slice()
        .iter()
        .enumerate()
        .map(|(i, c)| CellRaw {
            mat: c.mat as u32,
            temp: c.temp,
            fuel: f32::from(c.fuel),
            vapor: w.air.vapor[i],
            pressure: w.air.pressure[i],
            ..Default::default()
        })
        .collect();
    Snapshot {
        w: w.cells.width() as u32,
        h: w.cells.height() as u32,
        cells,
        air_vel: w.air.vel.iter().map(|v| [v.x, v.y]).collect(),
    }
}

/// Writes `s` to `path` in the v1 format (atomic: tmp file then rename),
/// creating the parent directory if needed.
pub fn write(path: &Path, s: &Snapshot) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    save_world(&to_world(s), path)
}

/// Reads a v1-format save. Never panics; errors say what is wrong.
pub fn read(path: &Path) -> Result<Snapshot, SaveError> {
    load_world(path).map(|w| from_world(&w))
}
