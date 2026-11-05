//! Heat diffusion and temperature-driven phase changes.

use crate::engine::System;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Fraction of the neighbour temperature difference exchanged per tick.
pub const CONDUCT: f32 = 0.2;

// Explicit 4-neighbour diffusion is only stable for CONDUCT <= 0.25.
const _: () = assert!(CONDUCT <= 0.25);

/// Phase-change table entry: `from` becomes `to` when temperature crosses `threshold`.
#[derive(Clone, Copy, Debug)]
pub struct Phase {
    /// Material that changes.
    pub from: Material,
    /// Material it becomes.
    pub to: Material,
    /// Temperature at which the change happens.
    pub threshold: f32,
    /// If true the change happens at or above `threshold`, otherwise at or below.
    pub above: bool,
}

/// All phase changes. Freeze and melt thresholds differ to avoid flicker.
pub const PHASES: [Phase; 5] = [
    Phase { from: Material::Sand, to: Material::Glass, threshold: 1700.0, above: true },
    Phase { from: Material::Water, to: Material::Steam, threshold: 373.0, above: true },
    Phase { from: Material::Water, to: Material::Ice, threshold: 270.0, above: false },
    Phase { from: Material::Ice, to: Material::Water, threshold: 276.0, above: true },
    Phase { from: Material::Lava, to: Material::Rock, threshold: 700.0, above: false },
];

/// Diffuses heat between 4-neighbours (edges insulated) and applies [`PHASES`].
pub struct Thermal;

impl System for Thermal {
    fn name(&self) -> &'static str {
        "thermal"
    }

    fn step(&mut self, w: &mut World, _rng: &mut Rng, _dt: f32) {
        // Phase changes first, on the temperatures the cells had entering the tick.
        for c in w.cells.as_mut_slice() {
            for p in &PHASES {
                if c.mat == p.from && (if p.above { c.temp >= p.threshold } else { c.temp <= p.threshold }) {
                    c.mat = p.to;
                    break;
                }
            }
        }
        let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
        let old: Vec<f32> = w.cells.as_slice().iter().map(|c| c.temp).collect();
        for y in 0..height {
            for x in 0..width {
                let i = w.cells.index(x, y).unwrap();
                let mut delta = 0.0;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if let Some(j) = w.cells.index(x + dx, y + dy) {
                        delta += old[j] - old[i];
                    }
                }
                w.cells.as_mut_slice()[i].temp = old[i] + CONDUCT * delta;
            }
        }
    }
}
