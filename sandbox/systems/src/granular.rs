//! Granular solids (currently sand): fall, slide diagonally, sink through lighter liquids.

use crate::engine::System;
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

/// True if a mover of `mover` may enter cell `(x, y)`: it is in bounds, not yet
/// updated this tick, and holds empty space or a strictly lighter material.
pub(crate) fn can_enter(w: &World, mover: Material, x: i32, y: i32) -> bool {
    match (w.cells.get(x, y), density(mover)) {
        (Some(c), Some(dm)) => !c.updated && density(c.mat).is_some_and(|dt| dt < dm),
        _ => false,
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

/// Falling-sand rules for granular materials.
pub struct Granular;

impl System for Granular {
    fn name(&self) -> &'static str {
        "granular"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
        for y in (0..height).rev() {
            let left_first = rng.chance(0.5);
            for i in 0..width {
                let x = if left_first { i } else { width - 1 - i };
                let c = *w.cells.get(x, y).unwrap();
                if c.updated || c.mat != Material::Sand {
                    continue;
                }
                if can_enter(w, c.mat, x, y + 1) {
                    swap(w, (x, y), (x, y + 1));
                    continue;
                }
                let dir = if rng.chance(0.5) { -1 } else { 1 };
                for dx in [dir, -dir] {
                    if can_enter(w, c.mat, x + dx, y + 1) {
                        swap(w, (x, y), (x + dx, y + 1));
                        break;
                    }
                }
            }
        }
    }
}
