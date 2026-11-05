//! Lighting: emitters (Lava, Fire, Ember) light nearby cells, and solids cast shadows.
//!
//! Each tick `world.light` is cleared and rebuilt from the emitter set only. A cell at squared
//! distance `d2` from an emitter receives `1 / (1 + d2)` if the straight line between them
//! crosses no solid cell; contributions add and the result is clamped to `[0, 1]`.
//! The system only reads `world.cells`.

use crate::engine::System;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Emitter light reach in cells.
pub const RADIUS: i32 = 12;

/// Fills `world.light` from emitters.
pub struct Lighting;

fn emits(m: Material) -> bool {
    matches!(m, Material::Lava | Material::Fire | Material::Ember)
}

fn blocks(m: Material) -> bool {
    matches!(m, Material::Stone | Material::Rock | Material::Metal | Material::Glass)
}

/// True if a solid lies strictly between `(x0, y0)` and `(x1, y1)` (Bresenham line).
fn shadowed(w: &World, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    if x0 == x1 && y0 == y1 {
        return false;
    }
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        if x == x1 && y == y1 {
            return false;
        }
        if w.cells.get(x, y).is_some_and(|c| blocks(c.mat)) {
            return true;
        }
    }
}

impl System for Lighting {
    fn name(&self) -> &'static str {
        "lighting"
    }

    fn step(&mut self, w: &mut World, _rng: &mut Rng, _dt: f32) {
        w.light.fill(0.0);
        let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
        for ey in 0..height {
            for ex in 0..width {
                if !w.cells.get(ex, ey).is_some_and(|c| emits(c.mat)) {
                    continue;
                }
                for y in (ey - RADIUS).max(0)..=(ey + RADIUS).min(height - 1) {
                    for x in (ex - RADIUS).max(0)..=(ex + RADIUS).min(width - 1) {
                        let d2 = (x - ex) * (x - ex) + (y - ey) * (y - ey);
                        if d2 > RADIUS * RADIUS || shadowed(w, ex, ey, x, y) {
                            continue;
                        }
                        let i = (y * width + x) as usize;
                        w.light[i] = (w.light[i] + 1.0 / (1.0 + d2 as f32)).min(1.0);
                    }
                }
            }
        }
    }
}
