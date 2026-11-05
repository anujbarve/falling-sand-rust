//! Deterministic xorshift32 random number generator.

/// xorshift32 generator. Invariant: `state` is never 0 (xorshift would stick at 0).
pub struct Rng {
    state: u32,
}

impl Rng {
    /// Creates a generator from `seed`; seed 0 is mapped to 1. Same seed, same sequence.
    pub fn new(seed: u32) -> Rng {
        Rng { state: if seed == 0 { 1 } else { seed } }
    }

    /// Next raw 32-bit value (xorshift32).
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Uniform float in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // 24 bits fit exactly in an f32 mantissa, so the result is strictly below 1.0.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform float from `lo` toward `hi`; float rounding may rarely land on `hi` itself.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// True with probability `p`: never for `p <= 0`, always for `p >= 1`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }
}
