//! Behaviour tests for the `AirSystem`.

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use sandbox_systems::air::AirSystem;
use sandbox_systems::engine::run_tick;
use sandbox_systems::System;

fn run(w: &mut World, wind: Vec2, ticks: usize) {
    let mut rng = Rng::new(1);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(AirSystem { wind })];
    for _ in 0..ticks {
        run_tick(w, &mut s, &mut rng);
    }
}

#[test]
fn name_is_air() {
    assert_eq!(AirSystem::default().name(), "air");
}

#[test]
fn vapor_blob_is_carried_by_wind_stably() {
    // Wide grid so the blob stays clear of the right wall for the whole run.
    let mut w = World::new(64, 16);
    let stride = 64usize;
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0), (0, -1)] {
        w.air.vapor[(8 + dy) as usize * stride + (8 + dx) as usize] = 1.0;
    }
    let com = |w: &World| {
        let (mut m, mut mx) = (0.0f32, 0.0f32);
        for (i, v) in w.air.vapor.iter().enumerate() {
            m += v;
            mx += v * (i % stride) as f32;
        }
        mx / m
    };
    let before = com(&w);
    run(&mut w, Vec2 { x: 2.0, y: 0.0 }, 30);
    let moved = com(&w) - before;
    assert!((moved - 20.0).abs() <= 2.0, "centre of mass moved {moved}");
    assert!(w.air.vapor.iter().all(|v| v.is_finite() && *v >= 0.0));
}

#[test]
fn pressure_spike_equalises_and_conserves() {
    let mut w = World::new(16, 16);
    w.air.pressure[4 * 16 + 4] = 10.0;
    let total: f32 = w.air.pressure.iter().sum();
    let spread = |w: &World| {
        let mean = w.air.pressure.iter().sum::<f32>() / w.air.pressure.len() as f32;
        w.air.pressure.iter().map(|p| (p - mean).abs()).fold(0.0, f32::max)
    };
    assert!(spread(&w) > 0.5, "setup must start unequal");
    run(&mut w, Vec2::default(), 200);
    assert!(spread(&w) < 0.5, "max deviation {}", spread(&w));
    let after: f32 = w.air.pressure.iter().sum();
    assert!(((after - total) / total).abs() < 1e-3, "total {total} -> {after}");
}

#[test]
fn hot_cell_gains_upward_velocity() {
    let mut w = World::new(16, 16);
    w.cells.set(8, 8, Cell { mat: Material::Empty, temp: 600.0, fuel: 0, updated: false });
    run(&mut w, Vec2::default(), 5);
    let v = w.air.vel[8 * 16 + 8];
    assert!(v.y < 0.0, "vel.y = {}", v.y);
    // A cold neighbour far away is unaffected.
    assert_eq!(w.air.vel[2 * 16 + 2].y, 0.0);
}

#[test]
fn wind_alone_fills_grid() {
    let mut w = World::new(16, 16);
    run(&mut w, Vec2 { x: 1.0, y: 0.0 }, 60);
    for v in &w.air.vel {
        assert!((v.x - 1.0).abs() < 0.05, "vel.x = {}", v.x);
        assert!(v.y.abs() < 0.05);
    }
}

#[test]
fn velocity_is_clamped() {
    let mut w = World::new(8, 8);
    run(&mut w, Vec2 { x: 100.0, y: 0.0 }, 60);
    assert!(w.air.vel.iter().all(|v| v.length() <= 4.0 + 1e-4));
}
