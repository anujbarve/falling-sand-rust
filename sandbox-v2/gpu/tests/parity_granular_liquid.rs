//! GPU granular + liquid passes must equal the CPU reference exactly.
//! Skips when the machine has no GPU adapter.

mod common;
use common::{scene, SEED, SIZE, TICKS};
use sandbox_v2_gpu::{dispatch_granular, dispatch_liquid, CellRaw, Gpu, GridState};

#[test]
fn gpu_matches_reference_after_300_ticks() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let mut r = scene();
    let mut grid = GridState::new(&gpu, SIZE, SIZE).unwrap();
    let cells: Vec<CellRaw> =
        r.cells().iter().map(|&mat| CellRaw { mat, ..Default::default() }).collect();
    grid.upload(&gpu, &cells);
    for t in 0..TICKS {
        r.tick(t, SEED);
        dispatch_granular(&gpu, &mut grid, t, SEED);
        dispatch_liquid(&gpu, &mut grid, t, SEED);
    }
    let got: Vec<u32> = grid.readback(&gpu).iter().map(|c| c.mat).collect();
    let mismatches = got.iter().zip(r.cells()).filter(|(a, b)| a != b).count();
    println!("parity mismatching cells: {mismatches}");
    assert_eq!(mismatches, 0);
}
