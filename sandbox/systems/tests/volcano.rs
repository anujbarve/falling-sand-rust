//! Behaviour tests for the `Volcano` system.

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::thermal::Thermal;
use sandbox_systems::volcano::{Volcano, VolcanoSource};
use sandbox_systems::System;

fn volcano(x: i32, y: i32, rate: f32) -> Volcano {
    Volcano { sources: vec![VolcanoSource { x, y, rate }] }
}

fn run(w: &mut World, systems: &mut [Box<dyn System>], ticks: usize) {
    let mut rng = Rng::new(5);
    for _ in 0..ticks {
        run_tick(w, systems, &mut rng);
    }
}

fn count(w: &World, m: Material) -> usize {
    w.cells.as_slice().iter().filter(|c| c.mat == m).count()
}

#[test]
fn emits_one_lava_per_tick_at_rate_one() {
    let mut w = World::new(60, 100);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(volcano(30, 20, 1.0))];
    run(&mut w, &mut s, 60);
    let n = count(&w, Material::Lava);
    assert!((58..=62).contains(&n), "lava count {n}");
}

#[test]
fn fractional_rate_emits_proportionally() {
    let mut w = World::new(60, 100);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(volcano(30, 20, 0.5))];
    run(&mut w, &mut s, 60);
    let n = count(&w, Material::Lava);
    assert!((29..=31).contains(&n), "lava count {n}");
}

#[test]
fn emitted_lava_is_hot() {
    let mut w = World::new(10, 10);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(volcano(5, 5, 1.0))];
    run(&mut w, &mut s, 1);
    let c = w.cells.get(5, 4).unwrap();
    assert_eq!(c.mat, Material::Lava);
    assert_eq!(c.temp, 1500.0);
}

#[test]
fn lava_spreads_sideways_slowly() {
    let mut w = World::new(80, 100);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(volcano(40, 20, 1.0))];
    run(&mut w, &mut s, 40);
    let mut max_dx = 0;
    for y in 0..100 {
        for x in 0..80 {
            if w.cells.get(x, y).unwrap().mat == Material::Lava {
                max_dx = max_dx.max((x - 40i32).abs());
            }
        }
    }
    assert!(max_dx <= 10, "spread {max_dx}");
}

#[test]
fn lava_falls_to_the_floor() {
    let mut w = World::new(10, 20);
    w.cells.set(5, 3, Cell { mat: Material::Lava, temp: 1500.0, fuel: 0, updated: false });
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Volcano::default())];
    run(&mut w, &mut s, 30);
    // The drop is done; the cell has only crept sideways along the floor.
    assert_eq!(count(&w, Material::Lava), 1);
    assert!((0..10).any(|x| w.cells.get(x, 19).unwrap().mat == Material::Lava));
}

#[test]
fn hot_lava_cools_to_rock() {
    let mut w = World::new(9, 9);
    w.cells.set(4, 8, Cell { mat: Material::Lava, temp: 800.0, fuel: 0, updated: false });
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Thermal), Box::new(Volcano::default())];
    run(&mut w, &mut s, 300);
    assert_eq!(count(&w, Material::Lava), 0);
    assert_eq!(count(&w, Material::Rock), 1);
}

#[test]
fn source_off_the_grid_emits_nothing() {
    for (x, y) in [(-1, 5), (10, 5), (5, 0), (5, -3), (5, 99)] {
        let mut w = World::new(10, 10);
        let mut s: Vec<Box<dyn System>> = vec![Box::new(volcano(x, y, 1.0))];
        run(&mut w, &mut s, 10);
        assert_eq!(count(&w, Material::Lava), 0, "source at ({x},{y})");
    }
}
