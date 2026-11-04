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
//! - gpu (timestamps) the same run, timed by the GPU's own clock (timestamp
//!                  queries): no CPU-side submit/wait overhead. n/a if the
//!                  adapter has no `TIMESTAMP_QUERY`.
//! - gpu + readback one step, then copy the grid back to the CPU, every time.
//!                  This is what a game that draws on the CPU pays per frame.
//!
//! Before timing, it checks that all backends produce the identical world.
//!
//! A second table measures chunk sleeping on the GPU: the busy benchmark
//! scene, and a "settled" world (scene 0 run until most of it has come to
//! rest), each with sleeping on and off, plus how many chunks are awake.

use sand::physics::{STEPS_PER_SECOND, cpu::CpuSim, gpu::GpuSim, scene, World};
use std::time::{Duration, Instant};

const SIZES: [(usize, usize); 5] = [(640, 360), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)];
const BENCH_SCENE: usize = 4;
/// Scene that mostly comes to rest, and how long to let it settle first.
const SETTLED_SCENE: usize = 0;
const SETTLE_STEPS: u32 = 6000;

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
        Ok(g) => format!(
            "{}, timestamp queries {}",
            g.adapter_name,
            if g.has_timestamps() { "supported" } else { "NOT supported" }
        ),
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
        "{:>11} | {:>9} | {:>11} | {:>11} | {:>11} | {:>16} | {:>15} | {:>12} | {:>11} | {:>13}",
        "grid", "cells", "cpu 1 thr", "cpu all", "gpu", "gpu (timestamps)", "gpu + readback", "gpu vs 1 thr", "gpu vs all", "gpu max speed"
    );
    println!("{}", "-".repeat(148));
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

        let (gpu, gpu_ts, readback) = match GpuSim::new(&start) {
            Ok(mut g) => {
                // Queue 32 steps, then wait for them: measures GPU throughput.
                // Also collect the GPU's own timestamps for each batch.
                let mut stamps = Vec::new();
                let gpu = time_it(budget, || {
                    g.step(32);
                    g.wait();
                    stamps.extend(g.gpu_time_ms());
                    32
                });
                let skip = stamps.len().min(3); // warm-up calls
                let gpu_ts = (stamps.len() > skip)
                    .then(|| stamps[skip..].iter().sum::<f64>() / (stamps.len() - skip) as f64);
                let mut world = start.clone();
                let readback = time_it(budget, || {
                    g.step(1);
                    g.download(&mut world);
                    1
                });
                (Some(gpu), gpu_ts, Some(readback))
            }
            Err(_) => (None, None, None),
        };

        let ms = |v: Option<f64>| v.map_or("n/a".to_string(), |v| format!("{v:.3} ms"));
        let x = |v: Option<f64>, base: f64| v.map_or("n/a".to_string(), |v| format!("{:.1}x", base / v));
        println!(
            "{:>11} | {:>8.2}M | {:>11} | {:>11} | {:>11} | {:>16} | {:>15} | {:>12} | {:>11} | {:>13}",
            format!("{w}x{h}"),
            (w * h) as f64 / 1e6,
            ms(Some(single)),
            ms(Some(multi)),
            ms(gpu),
            ms(gpu_ts),
            ms(readback),
            x(gpu, single),
            x(gpu, multi),
            // World seconds per wall second if the GPU did nothing else.
            gpu.map_or("n/a".to_string(), |v| format!("{:.2}x", 1000.0 / (STEPS_PER_SECOND as f64 * v))),
        );
    }
    println!("\nms = milliseconds per simulation step (lower is better). 60 fps leaves 16.7 ms per frame.");
    println!(
        "One step = 1/{STEPS_PER_SECOND} s of world time, so real time needs {STEPS_PER_SECOND} steps per second.\n\
         gpu max speed = how many times faster than real time the GPU alone could run (busy scene)."
    );

    if GpuSim::new(&World::new(8, 8)).is_err() {
        return;
    }
    println!("\nGPU chunk sleeping (32x32 chunks). settled = scene {SETTLED_SCENE} after {SETTLE_STEPS} steps.");
    println!(
        "{:>11} | {:>12} | {:>12} | {:>14} | {:>14} | {:>15}",
        "grid", "busy sleep", "busy no-slp", "settled sleep", "settled no-slp", "settled awake"
    );
    println!("{}", "-".repeat(94));
    let gpu_ms = |g: &mut GpuSim| {
        time_it(budget, || {
            g.step(32);
            g.wait();
            32
        })
    };
    for (w, h) in SIZES {
        // Each measurement starts from the same world, so sleeping on/off
        // time the same work (the results are identical either way).
        let mut busy = GpuSim::new(&scene(BENCH_SCENE, w, h)).expect("gpu");
        let busy_sleep = gpu_ms(&mut busy);
        let mut busy = GpuSim::new(&scene(BENCH_SCENE, w, h)).expect("gpu");
        busy.set_sleeping(false);
        let busy_plain = gpu_ms(&mut busy);

        let mut g = GpuSim::new(&scene(SETTLED_SCENE, w, h)).expect("gpu");
        g.step(SETTLE_STEPS);
        let mut settled = World::new(w, h);
        g.download(&mut settled);
        let awake = g.awake_chunks();
        let settled_sleep = gpu_ms(&mut g);
        // The sleep state is not in `World`: re-upload and let it sleep again
        // before timing would mix in the wake-up. Off: every chunk runs.
        g.upload(&settled);
        g.set_sleeping(false);
        let settled_plain = gpu_ms(&mut g);
        println!(
            "{:>11} | {:>9.3} ms | {:>9.3} ms | {:>11.3} ms | {:>11.3} ms | {:>15}",
            format!("{w}x{h}"),
            busy_sleep,
            busy_plain,
            settled_sleep,
            settled_plain,
            format!("{awake}/{}", g.chunk_count()),
        );
    }
}
