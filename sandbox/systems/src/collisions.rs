//! Collisions: particles against solid cells and against each other.

use crate::engine::System;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use std::collections::HashMap;

/// Fraction of normal speed kept after bouncing off a solid cell.
pub const RESTITUTION: f32 = 0.6;

/// Two particles collide when their centres are closer than this (cells).
pub const CONTACT_DISTANCE: f32 = 1.0;

/// At most this many particles use the all-pairs check; more use a grid bucket.
pub const PAIRWISE_LIMIT: usize = 512;

/// Resolves particle vs grid and particle vs particle collisions.
pub struct Collisions;

/// Whether a material blocks particles.
fn is_solid(m: Material) -> bool {
    matches!(m, Material::Stone | Material::Rock | Material::Wood | Material::Metal | Material::Glass | Material::Ice)
}

fn solid_at(w: &World, x: f32, y: f32) -> bool {
    w.cells.get(x.floor() as i32, y.floor() as i32).is_some_and(|c| is_solid(c.mat))
}

/// Reflects particles that ended the move inside a solid cell, back to where they came from.
fn grid_bounce(w: &mut World, dt: f32) {
    for i in 0..w.particles.len() {
        let (pos, vel) = (w.particles[i].pos, w.particles[i].vel);
        if !solid_at(w, pos.x, pos.y) {
            continue;
        }
        let prev = pos - vel * dt;
        // Which single-axis move was legal tells which wall was hit.
        let y_blocked = !solid_at(w, pos.x, prev.y);
        let x_blocked = !solid_at(w, prev.x, pos.y);
        let mut new_vel = vel;
        let mut new_pos = prev;
        if y_blocked || !x_blocked {
            new_vel.y = -vel.y * RESTITUTION;
        }
        if x_blocked || !y_blocked {
            new_vel.x = -vel.x * RESTITUTION;
        }
        // Keep the free axis's motion when only one wall was hit.
        if y_blocked && !x_blocked {
            new_pos.x = pos.x;
        } else if x_blocked && !y_blocked {
            new_pos.y = pos.y;
        }
        w.particles[i].vel = new_vel;
        w.particles[i].pos = new_pos;
    }
}

/// Equal-mass elastic collision: swaps the velocity components along the contact normal.
fn collide_pair(w: &mut World, i: usize, j: usize) {
    let d = w.particles[j].pos - w.particles[i].pos;
    let dist = d.length();
    if dist >= CONTACT_DISTANCE || dist == 0.0 {
        return;
    }
    let n = d * (1.0 / dist);
    let rel = w.particles[j].vel - w.particles[i].vel;
    let approach = rel.x * n.x + rel.y * n.y;
    if approach >= 0.0 {
        return;
    }
    let impulse: Vec2 = n * approach;
    w.particles[i].vel = w.particles[i].vel + impulse;
    w.particles[j].vel = w.particles[j].vel - impulse;
}

fn particle_pairs(w: &mut World) {
    let n = w.particles.len();
    if n <= PAIRWISE_LIMIT {
        for i in 0..n {
            for j in i + 1..n {
                collide_pair(w, i, j);
            }
        }
        return;
    }
    let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, p) in w.particles.iter().enumerate() {
        buckets.entry((p.pos.x.floor() as i32, p.pos.y.floor() as i32)).or_default().push(i);
    }
    for i in 0..n {
        let (bx, by) = (w.particles[i].pos.x.floor() as i32, w.particles[i].pos.y.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let Some(list) = buckets.get(&(bx + dx, by + dy)) else { continue };
                for &j in list {
                    if j > i {
                        collide_pair(w, i, j);
                    }
                }
            }
        }
    }
}

impl System for Collisions {
    fn name(&self) -> &'static str { "collisions" }

    fn step(&mut self, w: &mut World, _rng: &mut Rng, dt: f32) {
        grid_bounce(w, dt);
        particle_pairs(w);
    }
}
