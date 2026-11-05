//! Shared fixed scene for the reference and parity tests.

use sandbox_v2_gpu::reference::{Reference, SAND, WALL, WATER};

/// Scene width and height in cells.
pub const SIZE: u32 = 128;
/// Fixed hash seed.
pub const SEED: u32 = 0xC0FF_EE11;
/// Number of ticks simulated.
pub const TICKS: u32 = 300;

/// 128x128: walled box, a sand column at the left, a water basin at the right
/// with extra water dropped in from above.
pub fn scene() -> Reference {
    let mut r = Reference::new(SIZE, SIZE);
    for x in 0..SIZE {
        r.set(x, SIZE - 1, WALL);
    }
    for y in 0..SIZE {
        r.set(0, y, WALL);
        r.set(SIZE - 1, y, WALL);
    }
    for y in 5..40 {
        for x in 20..30 {
            r.set(x, y, SAND);
        }
    }
    for y in 90..SIZE - 1 {
        r.set(60, y, WALL);
        r.set(100, y, WALL);
    }
    for y in 100..SIZE - 1 {
        for x in 61..100 {
            r.set(x, y, WATER);
        }
    }
    for y in 60..80 {
        for x in 70..90 {
            r.set(x, y, WATER);
        }
    }
    r
}
