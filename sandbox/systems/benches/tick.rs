//! Tick-time benchmarks on the default scene (256x256: one volcano, one forest, one ocean strip).
//!
//! Run with `cargo bench -p sandbox_systems`. Budget: under 8 ms per full tick in release.

use criterion::{criterion_group, criterion_main, Criterion};
use sandbox_core::cell::Cell;
use sandbox_core::event::Event;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::air::AirSystem;
use sandbox_systems::chemistry::Chemistry;
use sandbox_systems::collisions::Collisions;
use sandbox_systems::engine::run_tick;
use sandbox_systems::explosions::Explosions;
use sandbox_systems::fire::Fire;
use sandbox_systems::granular::Granular;
use sandbox_systems::humidity::Humidity;
use sandbox_systems::lighting::Lighting;
use sandbox_systems::liquid::Liquid;
use sandbox_systems::ocean::Ocean;
use sandbox_systems::parallel::{tick_parallel, ParallelMovers};
use sandbox_systems::particles::Particles;
use sandbox_systems::thermal::Thermal;
use sandbox_systems::volcano::{Volcano, VolcanoSource};
use sandbox_systems::System;

const N: usize = 256;

fn put(w: &mut World, x: usize, y: usize, m: Material) {
    w.cells.set(x as i32, y as i32, Cell { mat: m, temp: 293.0, fuel: 0, updated: false });
}

/// Ground at row 200+, ocean strip on the left, a stone volcano cone in the middle, a wood forest on the right.
fn default_scene() -> World {
    let mut w = World::new(N, N);
    for y in 200..N {
        for x in 0..N {
            put(&mut w, x, y, Material::Stone);
        }
    }
    for y in 190..N - 8 {
        for x in 6..80 {
            put(&mut w, x, y, if y < 200 { Material::Water } else { Material::Empty });
        }
    }
    for y in 200..N - 8 {
        for x in 6..80 {
            put(&mut w, x, y, Material::Water);
        }
    }
    for x in 80..100 {
        for y in 190..200 {
            put(&mut w, x, y, Material::Sand);
        }
    }
    for h in 0..40usize {
        for x in 120 - (40 - h)..120 + (40 - h) {
            put(&mut w, x, 199 - h, Material::Stone);
        }
    }
    for t in 0..12usize {
        let x = 160 + t * 6;
        for y in 188..200 {
            put(&mut w, x, y, Material::Wood);
        }
        for y in 180..188 {
            for dx in 0..5 {
                put(&mut w, x + dx - 2, y, Material::Wood);
            }
        }
    }
    w.events.push(Event::Ignite { x: 160, y: 195 });
    w
}

fn systems() -> Vec<Box<dyn System>> {
    vec![
        Box::new(Granular::default()),
        Box::new(Liquid::default()),
        Box::new(AirSystem::default()),
        Box::new(Thermal),
        Box::new(Humidity),
        Box::new(Fire::default()),
        Box::new(Volcano { sources: vec![VolcanoSource { x: 120, y: 160, rate: 1.0 }] }),
        Box::new(Particles),
        Box::new(Collisions),
        Box::new(Explosions::default()),
        Box::new(Chemistry),
        Box::new(Ocean { wind_speed: 3.0, sea_level: 195 }),
        Box::new(Lighting),
    ]
}

fn bench(c: &mut Criterion) {
    let mut w = default_scene();
    let mut s = systems();
    let mut rng = Rng::new(1);
    for _ in 0..300 {
        run_tick(&mut w, &mut s, &mut rng);
        w.events.clear();
    }
    c.bench_function("default_scene_full_tick", |b| {
        b.iter(|| {
            run_tick(&mut w, &mut s, &mut rng);
            w.events.clear();
        })
    });

    let mut w = default_scene();
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Granular::default()), Box::new(Liquid::default())];
    let mut rng = Rng::new(1);
    for _ in 0..300 {
        run_tick(&mut w, &mut s, &mut rng);
    }
    c.bench_function("granular_liquid_serial", |b| b.iter(|| run_tick(&mut w, &mut s, &mut rng)));

    let mut w = default_scene();
    let mut m = ParallelMovers::default();
    let mut rng = Rng::new(1);
    for _ in 0..300 {
        tick_parallel(&mut w, &mut m, &mut rng);
    }
    c.bench_function("granular_liquid_parallel", |b| b.iter(|| tick_parallel(&mut w, &mut m, &mut rng)));
}

criterion_group!(benches, bench);
criterion_main!(benches);
