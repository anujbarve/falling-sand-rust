//! Per-cell simulation state.

use crate::material::Material;

/// One grid cell. Invariant: a fresh world cell is `Empty` at ambient 293.0.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// What the cell contains.
    pub mat: Material,
    /// Temperature, Kelvin-ish scale; ambient is 293.0.
    pub temp: f32,
    /// Remaining burn time for flammable materials.
    pub fuel: u8,
    /// Tick marker so a cell is not moved twice in one tick.
    pub updated: bool,
}
