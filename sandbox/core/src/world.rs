//! The full simulation state.

use crate::cell::Cell;
use crate::event::Event;
use crate::field::Air;
use crate::grid::Grid;
use crate::material::Material;
use crate::particle::Particle;
use crate::vec2::Vec2;

/// All simulation layers. Invariant: per-cell layers have `width * height` entries
/// and `waves` has `width` entries.
pub struct World {
    /// Material grid.
    pub cells: Grid<Cell>,
    /// Air velocity, pressure and vapor.
    pub air: Air,
    /// Free particles.
    pub particles: Vec<Particle>,
    /// Events drained by the app each tick.
    pub events: Vec<Event>,
    /// Ocean height per column.
    pub waves: Vec<f32>,
    /// Per-cell light 0..1, filled by the lighting system.
    pub light: Vec<f32>,
    /// Seconds of sim time.
    pub time: f32,
}

impl World {
    /// Creates a `w` by `h` world: all cells `Empty` at 293.0, everything else zeroed.
    pub fn new(w: usize, h: usize) -> World {
        let n = w * h;
        let cell = Cell { mat: Material::Empty, temp: 293.0, fuel: 0, updated: false };
        World {
            cells: Grid::new(w, h, cell),
            air: Air {
                w,
                h,
                vel: vec![Vec2::default(); n],
                pressure: vec![0.0; n],
                vapor: vec![0.0; n],
            },
            particles: Vec::new(),
            events: Vec::new(),
            waves: vec![0.0; w],
            light: vec![0.0; n],
            time: 0.0,
        }
    }
}
