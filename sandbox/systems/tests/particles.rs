//! Behaviour tests for the `Particles` system.

use sandbox_core::material::Material;
use sandbox_core::particle::Particle;
use sandbox_core::rng::Rng;
use sandbox_core::time::DT;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::particles::{Particles, GRAVITY};
use sandbox_systems::System;

fn particle(pos: Vec2, vel: Vec2, life: f32) -> Particle {
    Particle { pos, vel, mat: Material::Sand, temp: 293.0, life }
}

fn run(w: &mut World, ticks: usize) {
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Particles)];
    let mut rng = Rng::new(1);
    for _ in 0..ticks {
        run_tick(w, &mut s, &mut rng);
    }
}

#[test]
fn projectile_range_matches_v2_over_g() {
    // Units: cells and cells/second; GRAVITY.y = 9.81 cells/s^2, speed 20 cells/s.
    let mut w = World::new(200, 200);
    let v = 20.0_f32;
    let c = std::f32::consts::FRAC_1_SQRT_2;
    let start = Vec2 { x: 10.0, y: 150.0 };
    w.particles.push(particle(start, Vec2 { x: v * c, y: -v * c }, 100.0));
    let mut prev = start;
    let mut landed = None;
    for _ in 0..600 {
        run(&mut w, 1);
        let p = w.particles[0].pos;
        if p.y >= start.y {
            let t = (start.y - prev.y) / (p.y - prev.y);
            landed = Some(prev.x + t * (p.x - prev.x));
            break;
        }
        prev = p;
    }
    let range = landed.expect("projectile never landed") - start.x;
    let expected = v * v / GRAVITY.y;
    assert!((range - expected).abs() / expected < 0.01, "range {range} vs {expected}");
}

#[test]
fn dropped_particle_speed_after_one_second() {
    let mut w = World::new(50, 50);
    w.particles.push(particle(Vec2 { x: 25.0, y: 1.0 }, Vec2::default(), 100.0));
    run(&mut w, 60);
    let vy = w.particles[0].vel.y;
    assert!((vy - 9.81).abs() < 0.05, "vy {vy}");
}

#[test]
fn particle_removed_when_life_runs_out() {
    let mut w = World::new(50, 50);
    w.particles.push(particle(Vec2 { x: 25.0, y: 1.0 }, Vec2::default(), 0.5));
    run(&mut w, 29);
    assert_eq!(w.particles.len(), 1, "alive after 29 ticks");
    run(&mut w, 2);
    assert!(w.particles.is_empty(), "removed by tick 31 ({} s of DT)", 31.0 * DT);
}

#[test]
fn particle_leaving_grid_is_removed() {
    let mut w = World::new(20, 20);
    w.particles.push(particle(Vec2 { x: 10.0, y: 10.0 }, Vec2 { x: 600.0, y: 0.0 }, 100.0));
    w.particles.push(particle(Vec2 { x: 5.0, y: 5.0 }, Vec2::default(), 100.0));
    run(&mut w, 3);
    assert_eq!(w.particles.len(), 1);
    assert_eq!(w.particles[0].pos.x.floor(), 5.0);
}
