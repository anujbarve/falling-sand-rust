//! Chunk skipping and banded parallel movement must be exact, not approximate.

use rayon::ThreadPoolBuilder;
use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::granular::Granular;
use sandbox_systems::liquid::Liquid;
use sandbox_systems::parallel::{tick_parallel, ParallelMovers};
use sandbox_systems::System;

const W: usize = 96;
const H: usize = 100; // not a multiple of the 32-row band: exercises the short last band

fn put(w: &mut World, x: i32, y: i32, m: Material) {
    w.cells.set(x, y, Cell { mat: m, temp: 293.0, fuel: 0, updated: false });
}

fn scene(seed: u32) -> World {
    let mut w = World::new(W, H);
    let mut rng = Rng::new(seed);
    for x in 0..W as i32 {
        put(&mut w, x, H as i32 - 1, Material::Stone);
    }
    for y in 0..H as i32 - 1 {
        for x in 0..W as i32 {
            let r = rng.next_f32();
            let m = if y < 70 && r < 0.25 {
                Material::Sand
            } else if y < 70 && r < 0.40 {
                Material::Water
            } else if y < 70 && r < 0.43 {
                Material::Oil
            } else if r < 0.44 {
                Material::Stone
            } else {
                continue;
            };
            put(&mut w, x, y, m);
        }
    }
    w
}

/// Pokes the world from outside the movers (as a brush, explosion or thermal would).
fn perturb(w: &mut World, tick: u32) {
    if tick % 25 == 7 {
        let x = (tick as usize * 13 % (W - 6)) as i32;
        for dx in 0..6 {
            for dy in 0..4 {
                put(w, x + dx, 90 - dy, Material::Empty);
                put(w, x + dx, 2 + dy, if dy % 2 == 0 { Material::Sand } else { Material::Water });
            }
        }
    }
}

fn same(a: &World, b: &World) -> bool {
    a.time == b.time
        && a.cells.as_slice().iter().zip(b.cells.as_slice()).all(|(p, q)| {
            p.mat == q.mat && p.temp == q.temp && p.fuel == q.fuel && p.updated == q.updated
        })
}

fn run_serial(use_chunks: bool, ticks: u32) -> World {
    let mut w = scene(5);
    let mut rng = Rng::new(5);
    let mut s: Vec<Box<dyn System>> = vec![
        Box::new(Granular::with_chunks(use_chunks)),
        Box::new(Liquid::with_chunks(use_chunks)),
    ];
    for t in 0..ticks {
        perturb(&mut w, t);
        run_tick(&mut w, &mut s, &mut rng);
    }
    w
}

fn run_par(threads: usize, ticks: u32) -> World {
    let pool = ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let mut w = scene(5);
    let mut rng = Rng::new(5);
    let mut m = ParallelMovers::default();
    pool.install(|| {
        for t in 0..ticks {
            perturb(&mut w, t);
            tick_parallel(&mut w, &mut m, &mut rng);
        }
    });
    w
}

#[test]
fn the_scene_actually_moves() {
    let a = scene(5);
    let b = run_serial(true, 300);
    assert!(!same(&a, &b));
}

#[test]
fn chunk_skipping_is_byte_identical_over_300_ticks() {
    assert!(same(&run_serial(true, 300), &run_serial(false, 300)));
}

#[test]
fn one_thread_and_eight_threads_are_identical_over_300_ticks() {
    assert!(same(&run_par(1, 300), &run_par(8, 300)));
}

#[test]
fn parallel_equals_serial_systems_over_300_ticks() {
    assert!(same(&run_par(4, 300), &run_serial(true, 300)));
}

#[test]
fn matter_is_conserved_across_band_seams() {
    let count = |w: &World, m| w.cells.as_slice().iter().filter(|c| c.mat == m).count();
    let a = scene(5);
    let mut w = scene(5);
    let mut rng = Rng::new(5);
    let mut m = ParallelMovers::default();
    for _ in 0..200 {
        tick_parallel(&mut w, &mut m, &mut rng);
    }
    for mat in [Material::Sand, Material::Water, Material::Oil, Material::Stone] {
        assert_eq!(count(&a, mat), count(&w, mat), "{mat:?}");
    }
}
