//! GridState tests. The round-trip is byte-for-byte for every field value
//! including negative floats and infinities; NaN bit patterns are NOT required
//! to round-trip (the generator never produces NaN). Skips without a GPU.

use sandbox_v2_gpu::{CellRaw, Gpu, GridState, MAX_CELLS};

fn rng(seed: &mut u64) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 16) as u32
}

fn f(seed: &mut u64) -> f32 {
    (rng(seed) as i32 as f32) / 1000.0 // includes negatives
}

fn random_cells(n: usize) -> Vec<CellRaw> {
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    (0..n)
        .map(|_| CellRaw {
            mat: rng(&mut s),
            temp: f(&mut s),
            fuel: f(&mut s),
            vx: f(&mut s),
            vy: f(&mut s),
            vapor: f32::NEG_INFINITY,
            pressure: -0.0,
            flags: rng(&mut s),
        })
        .collect()
}

#[test]
fn cell_raw_is_32_bytes() {
    assert_eq!(std::mem::size_of::<CellRaw>(), 32);
}

#[test]
fn round_trip_64x64_byte_for_byte() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let mut g = GridState::new(&gpu, 64, 64).unwrap();
    let cells = random_cells(64 * 64);
    g.upload(&gpu, &cells);
    let back = g.readback(&gpu);
    assert_eq!(bytemuck::cast_slice::<_, u8>(&cells), bytemuck::cast_slice::<_, u8>(&back));
    // swap flips the current buffer: the other buffer is still zeroed.
    g.swap();
    assert!(g.readback(&gpu).iter().all(|c| *c == CellRaw::default()));
    g.swap();
    assert_eq!(g.readback(&gpu), cells);
}

#[test]
fn oversized_grid_rejected() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    assert!(GridState::new(&gpu, 1025, 1024).is_err());
    assert!(GridState::new(&gpu, u32::MAX, u32::MAX).is_err());
    assert!(GridState::new(&gpu, 0, 5).is_err());
    assert!(GridState::new(&gpu, 1024, 1024).is_ok());
    assert_eq!(MAX_CELLS, 1 << 20);
}
