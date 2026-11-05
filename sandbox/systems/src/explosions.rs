//! Explosions: pressure blast, debris, destruction and scorching.
//!
//! Consumes `Event::Explosion` (other events are left in the queue). Units are
//! cells and seconds. A blast writes a squared-falloff pressure field at once,
//! then an expanding ring is added on later ticks from the `shocks` list.
//! All writes are clipped to the grid.

use crate::engine::System;
use sandbox_core::event::Event;
use sandbox_core::material::Material;
use sandbox_core::particle::Particle;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;

/// Seconds the shock ring takes to expand from the centre to the blast radius.
pub const SHOCK_TIME: f32 = 0.3;

/// Temperature of cells scorched by a blast.
pub const SCORCH_TEMP: f32 = 1200.0;

/// Seconds a debris particle lives.
pub const DEBRIS_LIFE: f32 = 2.0;

/// An expanding pressure ring left behind by an explosion.
#[derive(Clone, Copy, Debug)]
pub struct Shock {
    /// Centre column in cell units.
    pub x: f32,
    /// Centre row in cell units.
    pub y: f32,
    /// Blast radius; the ring reaches this at `SHOCK_TIME`.
    pub radius: f32,
    /// Blast energy; the ring's pressure fades from this to zero.
    pub energy: f32,
    /// Seconds since the explosion.
    pub age: f32,
}

/// Applies explosion events and advances active shock rings.
#[derive(Default)]
pub struct Explosions {
    /// Rings still expanding.
    pub shocks: Vec<Shock>,
}

fn add_pressure(w: &mut World, x: i32, y: i32, amount: f32) {
    if let Some(i) = w.cells.index(x, y) {
        w.air.pressure[i] += amount;
    }
}

/// Adds the thin pressure band of `s` at its current ring radius.
fn apply_ring(w: &mut World, s: &Shock) {
    let ring = s.radius * (s.age / SHOCK_TIME);
    let amp = s.energy * (1.0 - s.age / SHOCK_TIME);
    let reach = ring.ceil() as i32 + 1;
    let (cx, cy) = (s.x.round() as i32, s.y.round() as i32);
    for y in cy - reach..=cy + reach {
        for x in cx - reach..=cx + reach {
            let d = ((x as f32 - s.x).powi(2) + (y as f32 - s.y).powi(2)).sqrt();
            if (d - ring).abs() <= 0.5 {
                add_pressure(w, x, y, amp);
            }
        }
    }
}

/// Applies the instant effects of one blast and queues its ring.
fn detonate(w: &mut World, rng: &mut Rng, x: f32, y: f32, r: f32, energy: f32) -> Option<Shock> {
    if r <= 0.0 {
        return None;
    }
    let reach = r.ceil() as i32;
    let (cx, cy) = (x.round() as i32, y.round() as i32);
    let mut debris = 0usize;
    let cap = energy.max(0.0) as usize;
    for gy in cy - reach..=cy + reach {
        for gx in cx - reach..=cx + reach {
            let (dx, dy) = (gx as f32 - x, gy as f32 - y);
            let d = (dx * dx + dy * dy).sqrt();
            let Some(cell) = w.cells.get(gx, gy).copied() else { continue };
            if d > r {
                continue;
            }
            add_pressure(w, gx, gy, energy * (1.0 - d / r).powi(2));
            let mut next = Material::Empty;
            let solid = matches!(cell.mat, Material::Sand | Material::Stone | Material::Wood | Material::Metal);
            if solid && d <= r * 0.5 {
                if debris < cap {
                    debris += 1;
                    let dir = if d > 1e-3 {
                        Vec2 { x: dx / d, y: dy / d }
                    } else {
                        let a = rng.range_f32(0.0, std::f32::consts::TAU);
                        Vec2 { x: a.cos(), y: a.sin() }
                    };
                    let speed = energy * rng.range_f32(0.25, 0.5);
                    w.particles.push(Particle {
                        pos: Vec2 { x: gx as f32, y: gy as f32 },
                        vel: dir * speed,
                        mat: cell.mat,
                        temp: SCORCH_TEMP,
                        life: DEBRIS_LIFE,
                    });
                }
            } else if matches!(cell.mat, Material::Wood | Material::Oil | Material::Gunpowder | Material::Ash) {
                next = Material::Ash;
            }
            if let Some(c) = w.cells.get_mut(gx, gy) {
                c.mat = next;
                c.temp = SCORCH_TEMP;
                c.fuel = 0;
            }
        }
    }
    Some(Shock { x, y, radius: r, energy, age: 0.0 })
}

impl System for Explosions {
    fn name(&self) -> &'static str {
        "explosions"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, dt: f32) {
        // Rings from earlier ticks first, so a new blast's first tick is just the falloff.
        for s in self.shocks.iter_mut() {
            s.age += dt;
        }
        self.shocks.retain(|s| s.age <= SHOCK_TIME);
        for s in self.shocks.clone() {
            apply_ring(w, &s);
        }

        let events = std::mem::take(&mut w.events);
        for e in events {
            match e {
                Event::Explosion { x, y, radius, energy } => {
                    if let Some(s) = detonate(w, rng, x, y, radius, energy) {
                        self.shocks.push(s);
                    }
                }
                other => w.events.push(other),
            }
        }
    }
}
