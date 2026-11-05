//! GPU granular + liquid passes must equal the CPU reference exactly.
//! Skips when the machine has no GPU adapter.

mod common;
use common::{scene, SEED, SIZE, TICKS};
use sandbox_v2_gpu::{CellRaw, GranularPass, Gpu, GridState, LiquidPass};

#[test]
fn gpu_matches_reference_after_300_ticks() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let mut r = scene();
    let (granular, liquid) = (GranularPass::new(&gpu), LiquidPass::new(&gpu));
    let grid = GridState::new(&gpu, SIZE, SIZE).unwrap();
    let cells: Vec<CellRaw> =
        r.cells().iter().map(|&mat| CellRaw { mat, ..Default::default() }).collect();
    grid.upload(&gpu, &cells);
    for t in 0..TICKS {
        r.tick(t, SEED);
        granular.dispatch(&gpu, &grid, t, SEED);
        liquid.dispatch(&gpu, &grid, t, SEED);
    }
    let got = grid.readback(&gpu);
    let mismatches = got.iter().zip(r.cells()).filter(|(a, b)| a.mat != **b).count();
    // Velocity state is integer-valued, so it must match bitwise too.
    let vel_mismatches = got
        .iter()
        .zip(r.vy().iter().zip(r.frac()))
        .filter(|(c, (vy, fr))| c.vy.to_bits() != (**vy as f32).to_bits() || c.vx.to_bits() != (**fr as f32).to_bits())
        .count();
    println!("parity mismatching cells: {mismatches}; velocity-state mismatches: {vel_mismatches}");
    assert_eq!(mismatches, 0);
    assert_eq!(vel_mismatches, 0);
}
