//! Behaviour tests for the `Ocean` system. Fixed seeds are named in each test.

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::ocean::{Ocean, AMP_MAX};
use sandbox_systems::System;

fn systems(wind: f32, sea: i32) -> Vec<Box<dyn System>> {
    vec![Box::new(Ocean { wind_speed: wind, sea_level: sea })]
}

fn crest(w: &World) -> usize {
    let mut best = 0;
    for (i, v) in w.waves.iter().enumerate() {
        if *v > w.waves[best] {
            best = i;
        }
    }
    best
}

fn water_count(w: &World) -> usize {
    w.cells.as_slice().iter().filter(|c| c.mat == Material::Water).count()
}

#[test]
fn calm_is_flat_seed_1() {
    let mut w = World::new(64, 32);
    let mut s = systems(0.0, 16);
    let mut rng = Rng::new(1);
    for _ in 0..300 {
        run_tick(&mut w, &mut s, &mut rng);
        assert!(w.waves.iter().all(|v| v.abs() <= 1e-6));
    }
}

#[test]
fn bounded_with_wind_seed_2() {
    let mut w = World::new(128, 32);
    let mut s = systems(5.0, 16);
    let mut rng = Rng::new(2);
    let mut peak = 0.0f32;
    for _ in 0..2000 {
        run_tick(&mut w, &mut s, &mut rng);
        for v in &w.waves {
            assert!(v.abs() <= AMP_MAX);
            peak = peak.max(v.abs());
        }
    }
    assert!(peak > 1.0, "wind must actually raise waves, peak {peak}");
}

#[test]
fn crest_moves_seed_3() {
    let mut w = World::new(200, 32);
    let mut s = systems(5.0, 16);
    let mut rng = Rng::new(3);
    run_tick(&mut w, &mut s, &mut rng);
    let c0 = crest(&w);
    for _ in 0..99 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    assert_ne!(crest(&w), c0);
}

#[test]
fn wave_array_matches_width_seed_4() {
    let mut w = World::new(77, 10);
    let mut s = systems(3.0, 5);
    let mut rng = Rng::new(4);
    run_tick(&mut w, &mut s, &mut rng);
    assert_eq!(w.waves.len(), w.cells.width());
}

#[test]
fn no_nan_over_10000_ticks_seed_5() {
    let mut w = World::new(64, 16);
    let mut s = systems(5.0, 8);
    let mut rng = Rng::new(5);
    for _ in 0..10_000 {
        run_tick(&mut w, &mut s, &mut rng);
        assert!(w.waves.iter().all(|v| v.is_finite()));
    }
}

#[test]
fn water_mass_conserved_and_surface_follows_wave_seed_6() {
    let (width, height) = (60usize, 40usize);
    let mut w = World::new(width, height);
    let sea = 20;
    for y in sea..height as i32 {
        for x in 0..width as i32 {
            w.cells.set(x, y, Cell { mat: Material::Water, temp: 293.0, fuel: 0, updated: false });
        }
    }
    let before = water_count(&w);
    let mut s = systems(5.0, sea);
    let mut rng = Rng::new(6);
    let mut moved = false;
    for _ in 0..300 {
        run_tick(&mut w, &mut s, &mut rng);
        assert_eq!(water_count(&w), before);
        for x in 0..width as i32 {
            let top = (0..height as i32).find(|&y| w.cells.get(x, y).unwrap().mat != Material::Empty);
            if top != Some(sea) {
                moved = true;
            }
        }
    }
    assert!(moved, "surface should deviate from flat sea level");
}
