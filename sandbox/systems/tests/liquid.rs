//! Behaviour tests for the `Liquid` system.

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
fn water_spreads_along_floor() {
    let mut w = World::new(16, 16);
    let mut rng = Rng::new(11);
    let mut s = systems();
    for y in 0..4 {
        for x in 4..7 {
            if (y * 3 + (x - 4)) < 10 {
                put(&mut w, x, y, Material::Water);
            }
        }
    }
    assert_eq!(count(&w, Material::Water), 10);
    for _ in 0..120 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    let cols: Vec<i32> = (0..16).filter(|&x| mat(&w, x, 15) == Material::Water).collect();
    let span = cols.last().unwrap() - cols.first().unwrap() + 1;
    assert!(span >= 8, "span = {span}");
}

#[test]
fn water_sinks_below_oil() {
    let mut w = World::new(8, 8);
    let mut rng = Rng::new(2);
    let mut s = systems();
    for y in 0..8 {
        put(&mut w, 2, y, Material::Stone);
        put(&mut w, 4, y, Material::Stone);
    }
    put(&mut w, 3, 6, Material::Oil);
    put(&mut w, 3, 7, Material::Stone);
    put(&mut w, 3, 2, Material::Water);
    for _ in 0..40 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    let find = |m: Material| (0..8).find(|&y| mat(&w, 3, y) == m);
    let (water, oil) = (find(Material::Water), find(Material::Oil));
    let (water, oil) = (water.expect("water left column"), oil.expect("oil left column"));
    assert!(water > oil, "water at y={water} should be below oil at y={oil}");
}

#[test]
fn acid_flows_like_a_liquid() {
    let mut w = World::new(8, 8);
    let mut rng = Rng::new(4);
    let mut s = systems();
    put(&mut w, 2, 0, Material::Acid);
    for _ in 0..20 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    assert_eq!(count(&w, Material::Acid), 1);
    assert!((0..8).any(|x| mat(&w, x, 7) == Material::Acid), "acid did not reach the floor");
}

#[test]
fn mass_is_conserved() {
    let mut w = World::new(32, 32);
    let mut rng = Rng::new(1);
    let mut s = systems();
    for y in 0..32 {
        for x in 0..32 {
            match rng.next_u32() % 5 {
                0 => put(&mut w, x, y, Material::Sand),
                1 => put(&mut w, x, y, Material::Water),
                2 => put(&mut w, x, y, Material::Oil),
                _ => {}
            }
        }
    }
    let (sand, water, oil) =
        (count(&w, Material::Sand), count(&w, Material::Water), count(&w, Material::Oil));
    for _ in 0..500 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    assert_eq!(count(&w, Material::Sand), sand);
    assert_eq!(count(&w, Material::Water), water);
    assert_eq!(count(&w, Material::Oil), oil);
}
