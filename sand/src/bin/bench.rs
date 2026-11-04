//! CPU vs GPU benchmark for the physics engine.
//!
//!     cargo run --release --bin bench            # default: about 1 s per measurement
//!     cargo run --release --bin bench -- 3       # 3 s per measurement (steadier numbers)
//!
//! For each grid size it times the same step four ways:
//!
//! - cpu 1 thread   `CpuSim::step`
//! - cpu all cores  `CpuSim::step_parallel` (rayon)
//! - gpu            many steps queued back to back, the GPU never waits for us
//! - gpu + readback one step, then copy the grid back to the CPU, every time.
//!                  This is what a game that draws on the CPU pays per frame.
//!
//! Before timing, it checks that all backends produce the identical world.

use sand::physics::{cpu::CpuSim, gpu::GpuSim, scene, World};
use std::time::{Duration, Instant};

const SIZES: [(usize, usize); 5] = [(640, 360), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)];
const BENCH_SCENE: usize = 4;

/// Run `step` repeatedly for at least `budget`; return the mean ms per call.
/// `step` returns how many steps it ran, so batched GPU calls count right.
fn time_it(budget: Duration, mut step: impl FnMut() -> u32) -> f64 {
    for _ in 0..3 {
        step(); // warm up: caches, thread pool, GPU clocks
    }
    let start = Instant::now();
    let mut steps = 0u64;
    while start.elapsed() < budget || steps < 5 {
        steps += step() as u64;
    }
    start.elapsed().as_secs_f64() * 1000.0 / steps as f64
}

fn check_identical(w: usize, h: usize, steps: u32) -> Result<(), String> {
    let start = scene(BENCH_SCENE, w, h);
    let (mut single, mut multi, mut gpu_world) = (start.clone(), start.clone(), start.clone());
    let mut sim = CpuSim::new(&start);
    for _ in 0..steps {
        sim.step(&mut single);
        sim.step_parallel(&mut multi);
    }
    let mut gpu = GpuSim::new(&start)?;
    gpu.step(steps);
    gpu.download(&mut gpu_world);
    let differ = |a: &World, b: &World| (0..a.cells.len()).filter(|&i| a.cells[i] != b.cells[i]).count();
    match (differ(&single, &multi), differ(&single, &gpu_world)) {
        (0, 0) => Ok(()),
        (m, g) => Err(format!("{m} cells differ (rayon), {g} cells differ (gpu)")),
    }
}

fn main() {
    let seconds: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let budget = Duration::from_secs_f64(seconds);

    let probe = GpuSim::new(&World::new(8, 8));
    let gpu_name = match &probe {
        Ok(g) => g.adapter_name.clone(),
        Err(e) => format!("none ({e})"),
    };
    drop(probe);
    println!("cpu threads: {}", rayon::current_num_threads());
    println!("gpu:         {gpu_name}");
    match check_identical(640, 360, 300) {
        Ok(()) => println!("check:       300 steps at 640x360, cpu 1 thread == cpu all cores == gpu, every cell\n"),
        Err(e) => println!("check:       MISMATCH: {e}\n"),
    }

    println!(
        "{:>11} | {:>9} | {:>11} | {:>11} | {:>11} | {:>15} | {:>12} | {:>11}",
        "grid", "cells", "cpu 1 thr", "cpu all", "gpu", "gpu + readback", "gpu vs 1 thr", "gpu vs all"
    );
    println!("{}", "-".repeat(113));
    for (w, h) in SIZES {
        let start = scene(BENCH_SCENE, w, h);
        let mut sim = CpuSim::new(&start);

        let mut world = start.clone();
        let single = time_it(budget, || {
            sim.step(&mut world);
            1
        });
        let mut world = start.clone();
        let multi = time_it(budget, || {
            sim.step_parallel(&mut world);
            1
        });

        let (gpu, readback) = match GpuSim::new(&start) {
            Ok(mut g) => {
                // Queue 32 steps, then wait for them: measures GPU throughput.
                let gpu = time_it(budget, || {
                    g.step(32);
                    g.wait();
                    32
                });
                let mut world = start.clone();
                let readback = time_it(budget, || {
                    g.step(1);
                    g.download(&mut world);
                    1
                });
                (Some(gpu), Some(readback))
            }
            Err(_) => (None, None),
        };

        let ms = |v: Option<f64>| v.map_or("n/a".to_string(), |v| format!("{v:.3} ms"));
        let x = |v: Option<f64>, base: f64| v.map_or("n/a".to_string(), |v| format!("{:.1}x", base / v));
        println!(
            "{:>11} | {:>8.2}M | {:>11} | {:>11} | {:>11} | {:>15} | {:>12} | {:>11}",
            format!("{w}x{h}"),
            (w * h) as f64 / 1e6,
            ms(Some(single)),
            ms(Some(multi)),
            ms(gpu),
            ms(readback),
            x(gpu, single),
            x(gpu, multi),
        );
    }
    println!("\nms = milliseconds per simulation step (lower is better). 60 fps leaves 16.7 ms per frame.");
}
