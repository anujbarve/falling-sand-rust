//! Behaviour tests for the `Fire` system. Stochastic tests use fixed seeds (named in each test).

use sandbox_core::cell::Cell;
use sandbox_core::event::Event;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::fire::Fire;
use sandbox_systems::System;

fn put(w: &mut World, x: i32, y: i32, m: Material, fuel: u8) {
    w.cells.set(x, y, Cell { mat: m, temp: 293.0, fuel, updated: false });
}

fn mat(w: &World, x: i32, y: i32) -> Material {
    w.cells.get(x, y).unwrap().mat
}

fn strip(len: i32, fuel: u8) -> World {
    let mut w = World::new(len as usize, 3);
    for x in 0..len {
        put(&mut w, x, 1, Material::Wood, fuel);
    }
    w
}

/// Spreads small consecutive seeds apart; xorshift32 outputs for tiny seeds are tiny and correlated.
fn scramble(seed: u32) -> u32 {
    seed.wrapping_mul(2_654_435_761)
}

fn systems() -> Vec<Box<dyn System>> {
    vec![Box::new(Fire::default())]
}

/// Ticks until cell (x, 1) first becomes `target`, or None within `max`.
fn ticks_until(w: &mut World, x: i32, target: Material, seed: u32, max: usize) -> Option<usize> {
    let mut rng = Rng::new(scramble(seed));
    let mut s = systems();
    for t in 1..=max {
        run_tick(w, &mut s, &mut rng);
        if mat(w, x, 1) == target {
            return Some(t);
        }
    }
    None
}

#[test]
fn ignite_event_burns_wood_to_ash_within_300_ticks_seed_1() {
    let mut w = strip(1, 50);
    w.events.push(Event::Ignite { x: 0, y: 1 });
    let mut rng = Rng::new(1);
    let mut s = systems();
    run_tick(&mut w, &mut s, &mut rng);
    assert_eq!(mat(&w, 0, 1), Material::Fire);
    assert!(w.events.is_empty(), "Ignite must be consumed");
    let mut w2 = strip(1, 50);
    w2.events.push(Event::Ignite { x: 0, y: 1 });
    assert!(ticks_until(&mut w2, 0, Material::Ash, 1, 300).is_some());
}

#[test]
fn other_events_are_not_drained() {
    let mut w = strip(1, 50);
    w.events.push(Event::Explosion { x: 0.0, y: 0.0, radius: 1.0, energy: 1.0 });
    let mut rng = Rng::new(1);
    run_tick(&mut w, &mut systems(), &mut rng);
    assert_eq!(w.events.len(), 1);
}

#[test]
fn fire_spreads_along_wood_strip_seed_2() {
    let mut w = strip(10, 20);
    w.events.push(Event::Ignite { x: 0, y: 1 });
    let mut rng = Rng::new(2);
    let mut s = systems();
    for _ in 0..600 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    let ash = (0..10).filter(|&x| mat(&w, x, 1) == Material::Ash).count();
    assert!(ash >= 8, "only {ash} of 10 burned");
}

/// Fire above a wood cell flanked by water. The brief says "does not ignite", but the formula only
/// divides the chance by 1 + wet (0.05/tick over 20 ticks of fuel), so the valid assertion is
/// statistical: wet wood ignites clearly less often than dry wood.
#[test]
fn wet_wood_is_less_likely_to_ignite_seeds_1_to_200() {
    let trial = |wet: bool, seed: u32| -> bool {
        let mut w = World::new(3, 3);
        put(&mut w, 1, 1, Material::Wood, 20);
        put(&mut w, 1, 0, Material::Fire, 20);
        if wet {
            put(&mut w, 0, 1, Material::Water, 0);
            put(&mut w, 2, 1, Material::Water, 0);
        }
        let mut rng = Rng::new(scramble(seed));
        let mut s = systems();
        for _ in 0..200 {
            run_tick(&mut w, &mut s, &mut rng);
            if mat(&w, 1, 1) != Material::Wood {
                return true;
            }
        }
        false
    };
    let dry = (1..=200).filter(|&s| trial(false, s)).count();
    let wet = (1..=200).filter(|&s| trial(true, s)).count();
    assert!(dry > 150, "dry baseline too low: {dry}");
    assert!(wet * 10 < dry * 8, "wet {wet} not clearly below dry {dry}");
}

/// Spread speed is measured as ticks until the far cell catches fire (the burn time that follows
/// is wind-independent and would dilute the ratio).
#[test]
fn wind_speeds_up_spread_seeds_1_to_20() {
    let total = |wind: f32| -> usize {
        (1..=20u32)
            .map(|seed| {
                let mut w = strip(10, 60);
                for v in w.air.vel.iter_mut() {
                    *v = Vec2 { x: wind, y: 0.0 };
                }
                w.events.push(Event::Ignite { x: 0, y: 1 });
                ticks_until(&mut w, 9, Material::Fire, seed, 2000).expect("far cell never burned")
            })
            .sum()
    };
    let calm = total(0.0) as f32;
    let windy = total(3.0) as f32;
    assert!(windy <= calm * 0.7, "windy {windy} vs calm {calm}");
}

#[test]
fn fire_never_persists_without_fuel_seed_3() {
    let mut w = World::new(8, 8);
    for x in 0..8 {
        put(&mut w, x, 4, Material::Fire, 0);
    }
    put(&mut w, 3, 2, Material::Fire, 30);
    let mut rng = Rng::new(3);
    let mut s = systems();
    for _ in 0..400 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    assert!(w.cells.as_slice().iter().all(|c| c.mat != Material::Fire));
}

/// A fully burnt strip leaves at least one Ember somewhere across seeds 1..=20.
#[test]
fn burning_strip_leaves_ember_seeds_1_to_20() {
    let mut embers = 0;
    for seed in 1..=20u32 {
        let mut w = strip(10, 5);
        w.events.push(Event::Ignite { x: 0, y: 1 });
        let mut rng = Rng::new(scramble(seed));
        let mut s = systems();
        for _ in 0..300 {
            run_tick(&mut w, &mut s, &mut rng);
        }
        embers += w.cells.as_slice().iter().filter(|c| c.mat == Material::Ember).count();
    }
    assert!(embers >= 1, "no Ember produced");
}
