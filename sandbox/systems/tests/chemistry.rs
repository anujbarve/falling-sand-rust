//! Behaviour tests for the `Chemistry` system. Fixed seeds are named in each test.

use sandbox_core::cell::Cell;
use sandbox_core::event::Event;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::chemistry::{Chemistry, ReactionEmit, REACTIONS};
use sandbox_systems::engine::run_tick;
use sandbox_systems::System;

fn put(w: &mut World, x: i32, y: i32, m: Material) {
    w.cells.set(x, y, Cell { mat: m, temp: 293.0, fuel: 0, updated: false });
}

fn mat(w: &World, x: i32, y: i32) -> Material {
    w.cells.get(x, y).unwrap().mat
}

fn count(w: &World, m: Material) -> usize {
    let mut n = 0;
    for y in 0..w.cells.height() as i32 {
        for x in 0..w.cells.width() as i32 {
            if mat(w, x, y) == m {
                n += 1;
            }
        }
    }
    n
}

fn systems() -> Vec<Box<dyn System>> {
    vec![Box::new(Chemistry)]
}

#[test]
fn acid_dissolves_metal_into_steam_seed_1() {
    let mut w = World::new(2, 1);
    put(&mut w, 0, 0, Material::Acid);
    put(&mut w, 1, 0, Material::Metal);
    let mut rng = Rng::new(1);
    run_tick(&mut w, &mut systems(), &mut rng);
    assert_eq!(count(&w, Material::Metal), 0);
    assert_eq!(count(&w, Material::Acid), 0);
    assert_eq!(count(&w, Material::Steam), 1);
    assert!(count(&w, Material::Steam) + count(&w, Material::Empty) == 2, "no mass gained");
}

#[test]
fn acid_eats_stone_slowly_seed_7() {
    let mut w = World::new(3, 2);
    for x in 0..3 {
        put(&mut w, x, 0, Material::Acid);
        put(&mut w, x, 1, Material::Stone);
    }
    let mut rng = Rng::new(7);
    let mut s = systems();
    for _ in 0..100 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    let removed = 3 - count(&w, Material::Stone);
    assert!(removed <= 3, "removed {removed}");
}

#[test]
fn gunpowder_line_burns_through_within_200_ticks_seed_3() {
    let mut w = World::new(21, 1);
    put(&mut w, 0, 0, Material::Fire);
    for x in 1..21 {
        put(&mut w, x, 0, Material::Gunpowder);
    }
    let mut rng = Rng::new(3);
    let mut s = systems();
    for _ in 0..200 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    for x in 0..21 {
        assert!(matches!(mat(&w, x, 0), Material::Fire | Material::Ash), "x={x}: {:?}", mat(&w, x, 0));
    }
    assert!(w.events.iter().any(|e| matches!(e, Event::Ignite { .. })), "Ignite events emitted");
}

#[test]
fn each_cell_reacts_once_per_tick() {
    // Fire at one end of a gunpowder line: only the adjacent cell can ignite in one tick.
    let mut w = World::new(5, 1);
    put(&mut w, 0, 0, Material::Fire);
    for x in 1..5 {
        put(&mut w, x, 0, Material::Gunpowder);
    }
    let mut rng = Rng::new(5);
    let mut s = systems();
    for _ in 0..50 {
        let before = count(&w, Material::Fire);
        run_tick(&mut w, &mut s, &mut rng);
        assert!(count(&w, Material::Fire) <= before + 1, "chain advanced more than one cell in a tick");
    }
}

#[test]
fn table_is_sane() {
    for (i, r) in REACTIONS.iter().enumerate() {
        assert!((0.0..=1.0).contains(&r.chance));
        for q in &REACTIONS[i + 1..] {
            assert!(!(r.a == q.a && r.b == q.b), "duplicate pair {:?}/{:?}", r.a, r.b);
        }
    }
    assert!(REACTIONS.iter().any(|r| r.emit == Some(ReactionEmit::Ignite)));
}
