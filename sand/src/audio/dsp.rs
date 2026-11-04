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

/// A small, dark stereo room (Freeverb-style: 4 damped combs + 2 all-passes
/// per side). A little of it glues the close sounds together, the way a
/// real quiet room does. Buffers are allocated once in `new`.
pub struct Reverb {
    combs: [[Comb; 4]; 2],
    allpasses: [[Allpass; 2]; 2],
}

struct Comb {
    buf: Vec<f32>,
    i: usize,
    store: f32,
}

struct Allpass {
    buf: Vec<f32>,
    i: usize,
}

impl Comb {
    fn process(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buf[self.i];
        self.store = y * (1.0 - damp) + self.store * damp;
        self.buf[self.i] = x + self.store * feedback;
        self.i = (self.i + 1) % self.buf.len();
        y
    }
}

impl Allpass {
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.i];
        self.buf[self.i] = x + b * 0.5;
        self.i = (self.i + 1) % self.buf.len();
        b - x
    }
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Reverb {
        // Freeverb's tunings (at 44.1 kHz), scaled to a smaller room.
        let scale = sample_rate / 44_100.0 * 0.55;
        let len = |n: usize, side: usize| ((n + side * 23) as f32 * scale).max(1.0) as usize;
        let comb = |n, side| Comb { buf: vec![0.0; len(n, side)], i: 0, store: 0.0 };
        let ap = |n, side| Allpass { buf: vec![0.0; len(n, side)], i: 0 };
        let side = |s| ([comb(1116, s), comb(1188, s), comb(1277, s), comb(1356, s)], [ap(556, s), ap(441, s)]);
        let (l, r) = (side(0), side(1));
        Reverb { combs: [l.0, r.0], allpasses: [l.1, r.1] }
    }

    /// Wet signal only, for a mono-summed input.
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let mut out = [0.0f32; 2];
        for s in 0..2 {
            let mut y = 0.0;
            for c in self.combs[s].iter_mut() {
                y += c.process(x * 0.25, 0.72, 0.45);
            }
            for a in self.allpasses[s].iter_mut() {
                y = a.process(y);
            }
            out[s] = y;
        }
        (out[0], out[1])
    }
}

/// Slow random drift in [-1, 1]: a new random target now and then, glided
/// to. Sounds organic where a sine LFO sounds mechanical.
#[derive(Clone, Copy, Default)]
pub struct Wander {
    target: f32,
    value: f32,
}

impl Wander {
    /// Advance by one block. `rate` = new targets per second, `glide` = s.
    pub fn step(&mut self, rng: &mut Rng, rate: f32, glide: f32, block_s: f32) -> f32 {
        if rng.f() < rate * block_s {
            self.target = rng.bi();
        }
        self.value += (self.target - self.value) * smooth_coef(glide, block_s);
        self.value
    }
}
