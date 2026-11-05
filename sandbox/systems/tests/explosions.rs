//! Behaviour tests for the `Explosions` system.

use sandbox_core::cell::Cell;
use sandbox_core::event::{Event, SoundKind};
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::time::DT;
use sandbox_core::world::World;
use sandbox_systems::explosions::{Explosions, SHOCK_TIME};
use sandbox_systems::System;

fn put(w: &mut World, x: i32, y: i32, m: Material) {
    w.cells.set(x, y, Cell { mat: m, temp: 293.0, fuel: 0, updated: false });
}

fn boom(w: &mut World, x: f32, y: f32, radius: f32, energy: f32) {
    w.events.push(Event::Explosion { x, y, radius, energy });
}

fn step(e: &mut Explosions, w: &mut World) {
    e.step(w, &mut Rng::new(3), DT);
}

#[test]
fn pressure_falls_off_with_distance() {
    let mut w = World::new(40, 40);
    boom(&mut w, 20.0, 20.0, 10.0, 100.0);
    let mut e = Explosions::default();
    step(&mut e, &mut w);
    let row: Vec<f32> = (0..=10).map(|d| w.air.pressure[20 * 40 + 20 + d]).collect();
    assert!(row[0] > 0.0);
    for pair in row.windows(2) {
        assert!(pair[1] <= pair[0], "{row:?}");
    }
}

#[test]
fn destroys_inside_radius_and_spares_outside() {
    let mut w = World::new(30, 30);
    put(&mut w, 15, 16, Material::Sand);
    put(&mut w, 17, 15, Material::Stone);
    put(&mut w, 15, 20, Material::Stone);
    boom(&mut w, 15.0, 15.0, 3.0, 50.0);
    step(&mut Explosions::default(), &mut w);
    assert_eq!(w.cells.get(15, 16).unwrap().mat, Material::Empty);
    assert_eq!(w.cells.get(17, 15).unwrap().mat, Material::Empty);
    assert_eq!(w.cells.get(15, 20).unwrap().mat, Material::Stone);
}

#[test]
fn spawns_outward_debris() {
    let mut w = World::new(40, 40);
    for y in 14..27 {
        for x in 14..27 {
            put(&mut w, x, y, Material::Stone);
        }
    }
    boom(&mut w, 20.0, 20.0, 6.0, 50.0);
    step(&mut Explosions::default(), &mut w);
    let n = w.particles.len();
    assert!((5..=50).contains(&n), "{n}");
    for p in &w.particles {
        assert!(p.vel.length() > 0.0);
        let out = (p.pos.x - 20.0) * p.vel.x + (p.pos.y - 20.0) * p.vel.y;
        assert!(out >= 0.0);
    }
}

#[test]
fn scorches_wood_to_hot_ash() {
    let mut w = World::new(30, 30);
    put(&mut w, 15, 18, Material::Wood);
    boom(&mut w, 15.0, 15.0, 5.0, 50.0);
    step(&mut Explosions::default(), &mut w);
    let c = w.cells.get(15, 18).unwrap();
    assert_eq!(c.mat, Material::Ash);
    assert!(c.temp > 1000.0);
}

#[test]
fn edges_do_not_panic() {
    let mut w = World::new(20, 10);
    boom(&mut w, 0.0, 0.0, 8.0, 80.0);
    boom(&mut w, 19.0, 9.0, 8.0, 80.0);
    boom(&mut w, -5.0, 50.0, 8.0, 80.0);
    let mut e = Explosions::default();
    for _ in 0..30 {
        step(&mut e, &mut w);
    }
}

#[test]
fn shock_ring_expands_then_expires_and_other_events_stay() {
    let mut w = World::new(60, 60);
    w.events.push(Event::Sound { kind: SoundKind::Boom, intensity: 1.0, x: 0.0, y: 0.0 });
    boom(&mut w, 30.0, 30.0, 10.0, 100.0);
    let mut e = Explosions::default();
    step(&mut e, &mut w);
    assert_eq!(e.shocks.len(), 1);
    assert_eq!(w.events.len(), 1);
    // Clear the initial blast and measure the ring half-way through.
    w.air.pressure.iter_mut().for_each(|p| *p = 0.0);
    let ticks = (SHOCK_TIME / DT / 2.0) as usize;
    for _ in 0..ticks {
        step(&mut e, &mut w);
        w.air.pressure.iter_mut().for_each(|p| *p = 0.0);
    }
    step(&mut e, &mut w);
    let at = |d: usize| w.air.pressure[30 * 60 + 30 + d];
    assert!(at(5) + at(6) > 0.0, "ring near r/2");
    assert_eq!(at(0), 0.0);
    for _ in 0..40 {
        step(&mut e, &mut w);
    }
    assert!(e.shocks.is_empty());
}
