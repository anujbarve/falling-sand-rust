//! GPU granular + liquid passes must equal the CPU reference exactly.
//! Skips when the machine has no GPU adapter.

mod common;
use common::{scene, SEED, SIZE, TICKS};
use sandbox_v2_gpu::{CellRaw, Gpu, GranularPass, GridState, LiquidPass};

#[test]
fn gpu_matches_reference_after_300_ticks() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let mut r = scene();
    // Distinct payloads per cell so any lost or duplicated payload shows.
    let pay = |i: u32| {
        [
            293.0 + (i % 97) as f32,
            (i % 5) as f32,
            (i % 11) as f32 * 0.125,
            (i % 3) as f32,
        ]
    };
    for i in 0..SIZE * SIZE {
        r.set_payload(i % SIZE, i / SIZE, pay(i));
    }
    let (granular, liquid) = (GranularPass::new(&gpu), LiquidPass::new(&gpu));
    let grid = GridState::new(&gpu, SIZE, SIZE).unwrap();
    let cells: Vec<CellRaw> = r
        .cells()
        .iter()
        .zip(r.payload())
        .map(|(&mat, p)| CellRaw {
            mat,
            temp: p[0],
            fuel: p[1],
            vapor: p[2],
            pressure: p[3],
            ..Default::default()
        })
        .collect();
    grid.upload(&gpu, &cells);
    for t in 0..TICKS {
        r.tick(t, SEED);
        granular.dispatch(&gpu, &grid, t, SEED);
        liquid.dispatch(&gpu, &grid, t, SEED);
    }
    let got = grid.readback(&gpu);
    let mismatches = got
        .iter()
        .zip(r.cells())
        .filter(|(a, b)| a.mat != **b)
        .count();
    // Velocity state is integer-valued, so it must match bitwise too.
    let vel_mismatches = got
        .iter()
        .zip(r.vy().iter().zip(r.frac()))
        .filter(|(c, (vy, fr))| {
            c.vy.to_bits() != (**vy as f32).to_bits() || c.vx.to_bits() != (**fr as f32).to_bits()
        })
        .count();
    let payload_mismatches = got
        .iter()
        .zip(r.payload())
        .filter(|(c, p)| {
            [c.temp, c.fuel, c.vapor, c.pressure].map(f32::to_bits) != p.map(f32::to_bits)
        })
        .count();
    println!(
        "parity mismatching cells: {mismatches}; velocity-state mismatches: {vel_mismatches}; \
         payload mismatches: {payload_mismatches}"
    );
    assert_eq!(mismatches, 0);
    assert_eq!(vel_mismatches, 0);
    assert_eq!(payload_mismatches, 0);
}

/// Carried gate (0 K holes): a move swaps the whole cell with the empty cell
/// it enters, so no cell is left at 0 K and no vapor or heat is created or
/// deleted (the multiset of (temp, vapor) payloads is only permuted).
#[test]
fn moves_swap_full_cells_no_zero_kelvin_holes() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let r = scene();
    let (granular, liquid) = (GranularPass::new(&gpu), LiquidPass::new(&gpu));
    let grid = GridState::new(&gpu, SIZE, SIZE).unwrap();
    let cells: Vec<CellRaw> = r
        .cells()
        .iter()
        .enumerate()
        .map(|(i, &mat)| CellRaw {
            mat,
            temp: 293.0 + (i % 7) as f32,
            vapor: if mat == 0 { 0.25 } else { 0.0 },
            ..Default::default()
        })
        .collect();
    grid.upload(&gpu, &cells);
    for t in 0..TICKS {
        granular.dispatch(&gpu, &grid, t, SEED);
        liquid.dispatch(&gpu, &grid, t, SEED);
    }
    let got = grid.readback(&gpu);
    let zero_k = got.iter().filter(|c| c.temp == 0.0).count();
    let key = |c: &CellRaw| (c.temp.to_bits(), c.vapor.to_bits());
    let mut before: Vec<_> = cells.iter().map(key).collect();
    let mut after: Vec<_> = got.iter().map(key).collect();
    before.sort_unstable();
    after.sort_unstable();
    println!("cells at 0 K after {TICKS} ticks: {zero_k}");
    assert_eq!(zero_k, 0, "moves left 0 K holes");
    assert!(
        before == after,
        "moves created or deleted heat/vapor payloads"
    );
}
