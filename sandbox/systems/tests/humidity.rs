//! Behaviour tests for the `Humidity` system.

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::humidity::Humidity;
use sandbox_systems::liquid::Liquid;
use sandbox_systems::System;

fn put(w: &mut World, x: i32, y: i32, m: Material, temp: f32) {
    w.cells.set(x, y, Cell { mat: m, temp, fuel: 0, updated: false });
}

fn run(w: &mut World, s: Vec<Box<dyn System>>, ticks: usize) {
    let mut rng = Rng::new(5);
    let mut s = s;
    for _ in 0..ticks {
        run_tick(w, &mut s, &mut rng);
    }
}

fn count(w: &World, m: Material) -> usize {
    w.cells.as_slice().iter().filter(|c| c.mat == m).count()
}

/// Vapor plus one unit per Steam or Water cell.
fn mass(w: &World) -> f32 {
    w.air.vapor.iter().sum::<f32>() + (count(w, Material::Steam) + count(w, Material::Water)) as f32
}

#[test]
fn steam_becomes_vapor_in_one_tick() {
    let mut w = World::new(9, 9);
    put(&mut w, 4, 4, Material::Steam, 400.0);
    let before = mass(&w);
    run(&mut w, vec![Box::new(Humidity)], 1);
    let i = w.cells.index(4, 4).unwrap();
    assert_eq!(w.cells.get(4, 4).unwrap().mat, Material::Empty);
    assert!((w.air.vapor[i] - 1.0).abs() < 1e-5);
    assert!((mass(&w) - before).abs() < 1e-4);
}

#[test]
fn vapor_condenses_into_water_when_cold_and_conserves_mass() {
    // A Water cell is one unit, so a cell holding 0.9 draws the missing 0.1 from its neighbours.
    let mut w = World::new(9, 9);
    let i = w.cells.index(4, 4).unwrap();
    let left = w.cells.index(3, 4).unwrap();
    w.air.vapor[i] = 0.9;
    w.air.vapor[left] = 0.1;
    for c in w.cells.as_mut_slice() {
        c.temp = 280.0;
    }
    let before = mass(&w);
    run(&mut w, vec![Box::new(Humidity)], 1);
    assert_eq!(count(&w, Material::Water), 1);
    assert_eq!(w.cells.get(4, 4).unwrap().mat, Material::Water);
    assert!((mass(&w) - before).abs() < 1e-4);
}

#[test]
fn thin_or_warm_vapor_does_not_condense() {
    let mut w = World::new(9, 9);
    let i = w.cells.index(4, 4).unwrap();
    w.air.vapor[i] = 0.9;
    w.air.vapor[w.cells.index(3, 4).unwrap()] = 0.5;
    run(&mut w, vec![Box::new(Humidity)], 1); // ambient 293 is above the dew point
    assert_eq!(count(&w, Material::Water), 0);

    let mut w = World::new(9, 9);
    w.air.vapor[i] = 0.5; // below the 0.8 threshold
    for c in w.cells.as_mut_slice() {
        c.temp = 280.0;
    }
    run(&mut w, vec![Box::new(Humidity)], 1);
    assert_eq!(count(&w, Material::Water), 0);
}

#[test]
fn condensation_needs_an_empty_cell() {
    let mut w = World::new(9, 9);
    put(&mut w, 4, 4, Material::Sand, 280.0);
    w.air.vapor[w.cells.index(4, 4).unwrap()] = 1.0;
    run(&mut w, vec![Box::new(Humidity)], 1);
    assert_eq!(count(&w, Material::Water), 0);
}

#[test]
fn hot_water_evaporates_into_vapor() {
    let mut w = World::new(9, 9);
    put(&mut w, 4, 4, Material::Water, 360.0);
    let before = mass(&w);
    run(&mut w, vec![Box::new(Humidity)], 200);
    assert_eq!(count(&w, Material::Water), 0);
    assert!((mass(&w) - before).abs() < 1e-3);
}

#[test]
fn rain_falls_one_row_per_tick_without_new_mass() {
    let mut w = World::new(9, 9);
    put(&mut w, 4, 2, Material::Water, 280.0);
    let mut rng = Rng::new(1);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Liquid), Box::new(Humidity)];
    for t in 1..=4 {
        run_tick(&mut w, &mut s, &mut rng);
        assert_eq!(w.cells.get(4, 2 + t).unwrap().mat, Material::Water);
        assert_eq!(count(&w, Material::Water), 1);
    }
}

#[test]
fn vapor_stays_within_zero_and_one() {
    let mut w = World::new(16, 16);
    let mut rng = Rng::new(9);
    for v in w.air.vapor.iter_mut() {
        *v = rng.next_f32();
    }
    for (n, c) in w.cells.as_mut_slice().iter_mut().enumerate() {
        c.temp = rng.range_f32(270.0, 400.0);
        if n % 17 == 0 {
            c.mat = Material::Steam;
        } else if n % 23 == 0 {
            c.mat = Material::Water;
        }
    }
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Liquid), Box::new(Humidity)];
    for _ in 0..500 {
        run_tick(&mut w, &mut s, &mut rng);
        assert!(w.air.vapor.iter().all(|&v| (0.0..=1.0).contains(&v)));
    }
}

#[test]
fn closed_system_conserves_mass_over_time() {
    let mut w = World::new(16, 16);
    let mut rng = Rng::new(2);
    for v in w.air.vapor.iter_mut() {
        *v = rng.next_f32() * 0.5;
    }
    for (n, c) in w.cells.as_mut_slice().iter_mut().enumerate() {
        c.temp = if n % 2 == 0 { 280.0 } else { 355.0 };
        if n % 19 == 0 {
            c.mat = Material::Steam;
        } else if n % 29 == 0 {
            c.mat = Material::Water;
        }
    }
    let before = mass(&w);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Liquid), Box::new(Humidity)];
    for _ in 0..300 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    assert!((mass(&w) - before).abs() < 1e-2, "{} vs {}", mass(&w), before);
}
