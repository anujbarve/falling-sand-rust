//! Zero heap allocations per tick in steady state for the granular + liquid path.
//! Scoped to those two systems; air, humidity and the rest may allocate and are not asserted here.

use std::alloc::{GlobalAlloc, Layout, System as SysAlloc};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use sandbox_core::cell::Cell;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::engine::run_tick;
use sandbox_systems::granular::Granular;
use sandbox_systems::liquid::Liquid;
use sandbox_systems::System;

struct Counting;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        SysAlloc.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        SysAlloc.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        SysAlloc.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Feeds sand and water at the top and drains the floor so the sim never goes idle.
fn feed(w: &mut World, t: u32) {
    let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
    for x in 0..width {
        w.cells.set(x, height - 1, Cell { mat: Material::Empty, temp: 293.0, fuel: 0, updated: false });
    }
    let m = if t % 2 == 0 { Material::Sand } else { Material::Water };
    for x in (t as i32 % 5..width).step_by(5) {
        w.cells.set(x, 0, Cell { mat: m, temp: 293.0, fuel: 0, updated: false });
    }
}

#[test]
fn granular_and_liquid_allocate_nothing_per_tick_after_warm_up() {
    let mut w = World::new(128, 96);
    let mut rng = Rng::new(3);
    let mut s: Vec<Box<dyn System>> = vec![Box::new(Granular::default()), Box::new(Liquid::default())];
    for t in 0..200 {
        feed(&mut w, t);
        run_tick(&mut w, &mut s, &mut rng);
    }
    ALLOCS.store(0, Ordering::SeqCst);
    COUNTING.store(true, Ordering::SeqCst);
    for t in 0..1000 {
        feed(&mut w, t);
        run_tick(&mut w, &mut s, &mut rng);
    }
    COUNTING.store(false, Ordering::SeqCst);
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "heap allocations during 1000 steady-state ticks");
    let moving = w.cells.as_slice().iter().filter(|c| matches!(c.mat, Material::Sand | Material::Water)).count();
    assert!(moving > 100, "scene went idle ({moving} movers), test would prove nothing");
}
