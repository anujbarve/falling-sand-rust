//! Liquids (water, oil, acid): fall, slide diagonally, flow sideways, and layer by density.

use crate::engine::System;
use crate::granular::{can_enter, swap};
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Maximum number of cells a liquid travels sideways in one tick.
const FLOW: i32 = 4;

fn is_liquid(m: Material) -> bool {
    matches!(m, Material::Water | Material::Oil | Material::Acid)
}

/// Fluid rules for liquids. Acid is only moved here; its reactions belong to chemistry.
pub struct Liquid;

impl System for Liquid {
    fn name(&self) -> &'static str {
        "liquid"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
        for y in (0..height).rev() {
            let left_first = rng.chance(0.5);
            for i in 0..width {
                let x = if left_first { i } else { width - 1 - i };
                let c = *w.cells.get(x, y).unwrap();
                if c.updated || !is_liquid(c.mat) {
                    continue;
                }
                if can_enter(w, c.mat, x, y + 1) {
                    swap(w, (x, y), (x, y + 1));
                    continue;
                }
                let dir = if rng.chance(0.5) { -1 } else { 1 };
                if let Some(dx) = [dir, -dir].into_iter().find(|&dx| can_enter(w, c.mat, x + dx, y + 1)) {
                    swap(w, (x, y), (x + dx, y + 1));
                    continue;
                }
                // Sideways flow into empty space only, up to FLOW cells in one direction.
                for dx in [dir, -dir] {
                    let mut reach = 0;
                    while reach < FLOW
                        && w.cells.get(x + dx * (reach + 1), y).is_some_and(|n| n.mat == Material::Empty && !n.updated)
                    {
                        reach += 1;
                    }
                    if reach > 0 {
                        swap(w, (x, y), (x + dx * reach, y));
                        break;
                    }
                }
            }
        }
    }
}
