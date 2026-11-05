//! Behaviour tests for the `Granular` system.

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::granular::Granular;
use sandbox_systems::liquid::Liquid;
use sandbox_systems::System;

fn put(w: &mut World, x: i32, y: i32, m: Material) {
    w.cells.set(x, y, Cell { mat: m, temp: 293.0, fuel: 0, updated: false });
}

fn mat(w: &World, x: i32, y: i32) -> Material {
    w.cells.get(x, y).unwrap().mat
}

fn systems() -> Vec<Box<dyn System>> {
    vec![Box::new(Granular), Box::new(Liquid)]
}

fn count(w: &World, m: Material) -> usize {
    w.cells.as_slice().iter().filter(|c| c.mat == m).count()
}

#[test]
fn sand_column_falls_to_floor() {
    let mut w = World::new(16, 16);
    let mut rng = Rng::new(7);
    let mut s = systems();
    for y in 0..5 {
        put(&mut w, 10, y, Material::Sand);
    }
    for _ in 0..60 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    for x in 0..16 {
        assert_ne!(mat(&w, x, 0), Material::Sand, "sand left in row 0");
    }
    assert_eq!(count(&w, Material::Sand), 5);
    let on_floor = (0..16).filter(|&x| mat(&w, x, 15) == Material::Sand).count();
    assert!(on_floor >= 1);
}

#[test]
fn sand_pile_settles() {
    let mut w = World::new(16, 16);
    let mut rng = Rng::new(3);
    let mut s = systems();
    for _ in 0..40 {
        put(&mut w, 8, 0, Material::Sand);
        run_tick(&mut w, &mut s, &mut rng);
    }
    for _ in 0..200 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    assert_eq!(count(&w, Material::Sand), 40);
    let floor = (0..16).filter(|&x| mat(&w, x, 15) == Material::Sand).count();
    assert!(floor >= 3, "floor sand = {floor}");
    // Every sand cell is supported: floor, sand below, or both diagonals blocked.
    let blocked = |x: i32, y: i32| match w.cells.get(x, y) {
        None => true,
        Some(c) => c.mat != Material::Empty,
    };
    for y in 0..16 {
        for x in 0..16 {
            if mat(&w, x, y) == Material::Sand {
                assert!(
                    blocked(x, y + 1) && blocked(x - 1, y + 1) && blocked(x + 1, y + 1),
                    "unsettled sand at ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn sand_sinks_through_water() {
    let mut w = World::new(16, 16);
    let mut rng = Rng::new(5);
    let mut s = systems();
    for y in 7..16 {
        put(&mut w, 4, y, Material::Stone);
        put(&mut w, 6, y, Material::Stone);
    }
    for y in 8..16 {
        put(&mut w, 5, y, Material::Water);
    }
    put(&mut w, 5, 5, Material::Sand);
    for _ in 0..60 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    assert_eq!(mat(&w, 5, 15), Material::Sand);
    assert_eq!(count(&w, Material::Water), 8);
    // The displaced water column is intact apart from the sand: column rises by one.
    assert_eq!(mat(&w, 5, 7), Material::Water);
    assert_eq!(mat(&w, 5, 14), Material::Water);
}

#[test]
fn sand_does_not_move_after_updated() {
    let mut w = World::new(4, 4);
    let mut rng = Rng::new(1);
    put(&mut w, 1, 0, Material::Sand);
    w.cells.get_mut(1, 0).unwrap().updated = true;
    Granular.step(&mut w, &mut rng, 0.0);
    assert_eq!(mat(&w, 1, 0), Material::Sand);
}
