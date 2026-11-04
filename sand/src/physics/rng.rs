//! Part of the physics engine (see mod.rs).

use super::*;

// ---------------------------------------------------------------------------
// Randomness: a hash, not a running generator
// ---------------------------------------------------------------------------

/// PCG hash: scrambles a u32 into a random-looking u32. Pure function, so
/// the GPU (which has no shared random state) computes the same numbers.
pub fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

pub fn hash3(a: u32, b: u32, c: u32) -> u32 {
    pcg(a ^ pcg(b ^ pcg(c)))
}

