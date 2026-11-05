//! Per-pass GPU time of one step, for the busy benchmark scene.
//!
//!     cargo run --release --bin profile
//!
//! For each grid size of the benchmark it prints the GPU's own time for the
//! step pass, the wake pass (chunk sleeping), and their sum, per step. The
//! passes are timed with timestamp queries; without them it says so and exits.

use sand::physics::{gpu::{GpuSim, PassTimes}, scene};
use std::time::{Duration, Instant};

// Same grid sizes and busy scene as `bench` (bench.rs keeps its own copies).
const SIZES: [(usize, usize); 5] = [(640, 360), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)];
const BENCH_SCENE: usize = 4;

/// Steps per timed call, and how many calls to average.
const STEPS: u32 = 64;
const CALLS: usize = 20;

/// Poll until `gpu.pass_times_ms()` returns a value different from `last`
/// (a new timed call has come back), for up to 5 s.
fn next_pass_times(gpu: &mut GpuSim, last: Option<PassTimes>) -> Option<PassTimes> {
    let start = Instant::now();
    loop {
        match gpu.pass_times_ms() {
            Some(t) if Some(t) != last => return Some(t),
            _ if start.elapsed() > Duration::from_secs(5) => return None,
            _ => std::thread::sleep(Duration::from_millis(1)),
        }
    }
}

/// Mean per-step pass times over `CALLS` timed calls.
fn measure(gpu: &mut GpuSim) -> Option<PassTimes> {
    let (mut step, mut wake, mut n) = (0.0, 0.0, 0usize);
    let mut last = None;
    for _ in 0..CALLS {
        gpu.step(STEPS);
        gpu.wait();
        let t = next_pass_times(gpu, last)?;
        last = Some(t);
        step += t.step_ms;
        wake += t.wake_ms;
        n += 1;
    }
    Some(PassTimes { step_ms: step / n as f64, wake_ms: wake / n as f64 })
}

fn main() {
    println!("{:>11} | {:>9} | {:>9} | {:>9}", "grid", "step ms", "wake ms", "sum ms");
    println!("{}", "-".repeat(47));
    for (w, h) in SIZES {
        let world = scene(BENCH_SCENE, w, h);
        let mut gpu = match GpuSim::new(&world) {
            Ok(g) => g,
            Err(e) => {
                println!("no GPU: {e}");
                return;
            }
        };
        if !gpu.has_timestamps() {
            println!("{}: timestamp queries not supported on this adapter", gpu.adapter_name);
            return;
        }
        match measure(&mut gpu) {
            Some(t) => println!(
                "{:>11} | {:>9.3} | {:>9.3} | {:>9.3}",
                format!("{w}x{h}"),
                t.step_ms,
                t.wake_ms,
                t.step_ms + t.wake_ms
            ),
            None => println!("{:>11} | no per-pass timestamps came back", format!("{w}x{h}")),
        }
    }
}
