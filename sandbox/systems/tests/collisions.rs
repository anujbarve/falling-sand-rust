//! Behaviour tests for the `Collisions` system.

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::particle::Particle;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use sandbox_systems::collisions::{Collisions, RESTITUTION};
use sandbox_systems::engine::run_tick;
use sandbox_systems::particles::{Particles, GRAVITY};
use sandbox_systems::System;

fn particle(pos: Vec2, vel: Vec2) -> Particle {
    Particle { pos, vel, mat: Material::Sand, temp: 293.0, life: 100.0 }
}

fn stone(w: &mut World, x: i32, y: i32) {
    w.cells.set(x, y, Cell { mat: Material::Stone, temp: 293.0, fuel: 0, updated: false });
}

fn run(w: &mut World, systems: Vec<Box<dyn System>>, ticks: usize) {
    let mut s = systems;
    let mut rng = Rng::new(1);
    for _ in 0..ticks {
        run_tick(w, &mut s, &mut rng);
    }
}

#[test]
fn bounce_off_stone_uses_restitution() {
    let mut w = World::new(20, 20);
    for x in 0..20 {
        stone(&mut w, x, 10);
    }
    w.particles.push(particle(Vec2 { x: 5.5, y: 9.9 }, Vec2 { x: 0.0, y: 30.0 }));
    run(&mut w, vec![Box::new(Particles), Box::new(Collisions)], 1);
    let incoming = 30.0 + GRAVITY.y * sandbox_core::time::DT;
    let p = &w.particles[0];
    assert!(((p.vel.y / -incoming) - RESTITUTION).abs() < 1e-3, "vy {}", p.vel.y);
    assert!(p.pos.y < 10.0, "pushed out of stone: {}", p.pos.y);
    assert!(p.vel.x.abs() < 1e-6);
}

#[test]
fn particle_stays_in_free_space_untouched() {
    let mut w = World::new(20, 20);
    w.particles.push(particle(Vec2 { x: 5.5, y: 5.5 }, Vec2 { x: 1.0, y: 2.0 }));
    run(&mut w, vec![Box::new(Collisions)], 1);
    assert_eq!(w.particles[0].vel, Vec2 { x: 1.0, y: 2.0 });
}

fn momentum(w: &World) -> Vec2 {
    w.particles.iter().fold(Vec2::default(), |a, p| a + p.vel)
}

#[test]
fn head_on_particles_exchange_momentum() {
    let mut w = World::new(20, 20);
    w.particles.push(particle(Vec2 { x: 5.0, y: 5.0 }, Vec2 { x: 3.0, y: 0.5 }));
    w.particles.push(particle(Vec2 { x: 5.6, y: 5.0 }, Vec2 { x: -1.0, y: 0.0 }));
    let before = momentum(&w);
    run(&mut w, vec![Box::new(Collisions)], 1);
    let after = momentum(&w);
    assert!((after.x - before.x).abs() < 1e-4 && (after.y - before.y).abs() < 1e-4);
    assert!((w.particles[0].vel.x - -1.0).abs() < 1e-4, "equal masses swap normal velocity");
    assert!((w.particles[1].vel.x - 3.0).abs() < 1e-4);
}

#[test]
fn separating_particles_are_left_alone() {
    let mut w = World::new(20, 20);
    w.particles.push(particle(Vec2 { x: 5.0, y: 5.0 }, Vec2 { x: -1.0, y: 0.0 }));
    w.particles.push(particle(Vec2 { x: 5.5, y: 5.0 }, Vec2 { x: 1.0, y: 0.0 }));
    run(&mut w, vec![Box::new(Collisions)], 1);
    assert_eq!(w.particles[0].vel.x, -1.0);
}

#[test]
fn bucketed_path_conserves_momentum() {
    // More than 512 particles takes the grid-bucket path.
    let mut w = World::new(40, 40);
    let mut rng = Rng::new(9);
    for _ in 0..800 {
        let pos = Vec2 { x: rng.range_f32(5.0, 15.0), y: rng.range_f32(5.0, 15.0) };
        let vel = Vec2 { x: rng.range_f32(-3.0, 3.0), y: rng.range_f32(-3.0, 3.0) };
        w.particles.push(particle(pos, vel));
    }
    let before = momentum(&w);
    run(&mut w, vec![Box::new(Collisions)], 1);
    let after = momentum(&w);
    assert!((after.x - before.x).abs() < 1e-3 && (after.y - before.y).abs() < 1e-3, "{before:?} {after:?}");
}
