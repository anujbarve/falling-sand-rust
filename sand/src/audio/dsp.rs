//! Small DSP building blocks. Everything here is allocation-free and cheap
//! enough to run per sample on the audio thread.

use std::f32::consts::PI;

/// Xorshift random numbers: fast and deterministic (same seed, same sound).
#[derive(Clone, Copy)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed | 1)
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    /// Uniform in [-1, 1).
    pub fn bi(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }
}

/// Pink noise (-3 dB/octave), Paul Kellet's economy filter on white noise.
/// Softer than white noise: the basis of most "warm" beds.
#[derive(Clone, Copy, Default)]
pub struct Pink {
    b0: f32,
    b1: f32,
    b2: f32,
}

impl Pink {
    pub fn next(&mut self, white: f32) -> f32 {
        self.b0 = 0.99765 * self.b0 + white * 0.099_046;
        self.b1 = 0.963 * self.b1 + white * 0.296_516_4;
        self.b2 = 0.57 * self.b2 + white * 1.052_691_3;
        (self.b0 + self.b1 + self.b2 + white * 0.1848) * 0.2
    }
}

/// Coefficients of a state-variable filter for a cutoff and resonance.
/// Computed once per block, not per sample (tan is costly).
#[derive(Clone, Copy)]
pub struct SvfCoef {
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl SvfCoef {
    pub fn new(cutoff_hz: f32, q: f32, sample_rate: f32) -> SvfCoef {
        let fc = cutoff_hz.clamp(10.0, sample_rate * 0.45);
        let g = (PI * fc / sample_rate).tan();
        let k = 1.0 / q.max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        SvfCoef { a1, a2, a3: g * a2, k }
    }
}

/// Topology-preserving state-variable filter (Simper/Cytomic): stable at any
/// cutoff, gives low, band and high pass from one structure.
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

pub struct SvfOut {
    pub low: f32,
    pub band: f32,
    pub high: f32,
}

impl Svf {
    pub fn process(&mut self, x: f32, c: &SvfCoef) -> SvfOut {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut { low: v2, band: v1, high: x - c.k * v1 - v2 }
    }
}

/// One-pole smoother: parameters glide instead of jumping (no zipper noise).
#[derive(Clone, Copy, Default)]
pub struct Smooth {
    pub value: f32,
}

impl Smooth {
    /// Move towards `target`; `coef` near 0 = slow, 1 = instant.
    pub fn step(&mut self, target: f32, coef: f32) -> f32 {
        self.value += (target - self.value) * coef;
        self.value
    }
}

/// Smoothing coefficient for a time constant, applied once per block.
pub fn smooth_coef(seconds: f32, block_seconds: f32) -> f32 {
    1.0 - (-block_seconds / seconds.max(1e-4)).exp()
}

/// Equal-power pan: total loudness stays the same across the stereo field.
/// `pan` in [-1, 1].
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * 0.25 * PI;
    (a.cos(), a.sin())
}
