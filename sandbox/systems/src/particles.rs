//! Particles: gravity, motion, and lifetime of free particles.
//!
//! Units are cells and seconds. Integration is semi-implicit Euler: velocity
//! gains gravity first, then position advances with the new velocity.

use crate::engine::System;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;

/// Gravity in cells per second squared (the single place the scale is fixed).
pub const GRAVITY: Vec2 = Vec2 { x: 0.0, y: 9.81 };

/// Integrates particles with gravity and removes expired or off-grid ones.
pub struct Particles;

impl System for Particles {
    fn name(&self) -> &'static str { "particles" }

    fn step(&mut self, w: &mut World, _rng: &mut Rng, dt: f32) {
        let (width, height) = (w.cells.width() as f32, w.cells.height() as f32);
        for p in w.particles.iter_mut() {
            p.vel = p.vel + GRAVITY * dt;
            p.pos = p.pos + p.vel * dt;
            p.life -= dt;
        }
        w.particles.retain(|p| {
            p.life > 0.0 && p.pos.x >= 0.0 && p.pos.y >= 0.0 && p.pos.x < width && p.pos.y < height
        });
    }
}
