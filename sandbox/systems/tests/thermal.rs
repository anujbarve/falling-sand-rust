//! Behaviour tests for the `Thermal` system.

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::thermal::Thermal;
use sandbox_systems::System;

const AMBIENT: f32 = 293.0;

fn systems() -> Vec<Box<dyn System>> {
    vec![Box::new(Thermal)]
}

fn fill(w: &mut World, m: Material) {
    for c in w.cells.as_mut_slice() {
        *c = Cell { mat: m, temp: AMBIENT, fuel: 0, updated: false };
    }
}

fn single(m: Material, temp: f32) -> World {
    let mut w = World::new(5, 5);
    w.cells.set(2, 2, Cell { mat: m, temp, fuel: 0, updated: false });
    w
}

fn run(w: &mut World, ticks: usize) {
    let mut rng = Rng::new(3);
    let mut s = systems();
    for _ in 0..ticks {
        run_tick(w, &mut s, &mut rng);
    }
}

fn mat_at(w: &World, x: i32, y: i32) -> Material {
    w.cells.get(x, y).unwrap().mat
}

#[test]
fn heat_spreads_and_is_conserved() {
    let mut w = World::new(9, 9);
    fill(&mut w, Material::Stone);
    w.cells.get_mut(4, 4).unwrap().temp = 1000.0;
    let total = |w: &World| w.cells.as_slice().iter().map(|c| c.temp - AMBIENT).sum::<f32>();
    let before = total(&w);
    run(&mut w, 100);
    let temps: Vec<f32> = w.cells.as_slice().iter().map(|c| c.temp).collect();
    let max = temps.iter().cloned().fold(f32::MIN, f32::max);
    let min = temps.iter().cloned().fold(f32::MAX, f32::min);
    assert!(max - min < 200.0, "spread {}", max - min);
    assert!(max < 1000.0);
    assert!(((total(&w) - before) / before).abs() < 1e-2);
}

#[test]
fn sand_melts_to_glass_only_when_hot() {
    let mut w = single(Material::Sand, 1700.0);
    run(&mut w, 1);
    assert_eq!(mat_at(&w, 2, 2), Material::Glass);
    let mut w = single(Material::Sand, 1000.0);
    run(&mut w, 1);
    assert_eq!(mat_at(&w, 2, 2), Material::Sand);
}

#[test]
fn water_boils_and_freezes() {
    let mut w = single(Material::Water, 400.0);
    run(&mut w, 1);
    assert_eq!(mat_at(&w, 2, 2), Material::Steam);
    let mut w = single(Material::Water, 290.0);
    run(&mut w, 1);
    assert_eq!(mat_at(&w, 2, 2), Material::Water);
    let mut w = single(Material::Water, 250.0);
    run(&mut w, 1);
    assert_eq!(mat_at(&w, 2, 2), Material::Ice);
}

#[test]
fn ice_melts_when_warm() {
    let mut w = single(Material::Ice, 300.0);
    run(&mut w, 1);
    assert_eq!(mat_at(&w, 2, 2), Material::Water);
}

#[test]
fn lava_cools_to_rock() {
    let mut w = single(Material::Lava, 800.0);
    run(&mut w, 300);
    assert_eq!(mat_at(&w, 2, 2), Material::Rock);
}

#[test]
fn ambient_is_stable() {
    let mut w = World::new(12, 12);
    run(&mut w, 200);
    assert!(w.cells.as_slice().iter().all(|c| (c.temp - AMBIENT).abs() < 1e-3));
}
