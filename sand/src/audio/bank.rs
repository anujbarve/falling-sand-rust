//! The one-shot sounds: synthesised once, then played back like recordings.
//!
//! Every sound here is procedural (no sample files needed), generated once
//! at startup, deterministic (fixed seeds), and the same data the `bake`
//! binary writes to WAV files in `sand/assets/audio/`.
//!
//! Each kind has several variants so repeated events never sound identical.

use super::dsp::{Rng, Svf, SvfCoef};
use std::f32::consts::TAU;

/// The one-shot kinds. Their order is the index into `Bank::sounds`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A sand grain landing: a tiny resonant tick.
    Grain = 0,
    /// A small water drop landing: a high, short bubble (Minnaert resonance).
    Drip,
    /// A boiling bubble: a lower, rounder bubble.
    Bubble,
    /// Burning wood or oil popping.
    Crackle,
    /// A thick lava bubble bursting: low, gloopy.
    Plop,
    /// A fuel catching fire: a soft breathy whoosh.
    Ignite,
    /// Lava setting into rock: a small cooling tink.
    Tink,
}

pub const KINDS: usize = 7;
pub const VARIANTS: usize = 8;
pub const KIND_NAMES: [&str; KINDS] = ["grain", "drip", "bubble", "crackle", "plop", "ignite", "tink"];

pub struct Bank {
    pub sample_rate: f32,
    /// `sounds[kind][variant]`: mono samples, peak-normalised to 0.9.
    pub sounds: Vec<Vec<Vec<f32>>>,
}

impl Bank {
    pub fn new(sample_rate: f32) -> Bank {
        let sr = sample_rate;
        let mut sounds = Vec::with_capacity(KINDS);
        for k in 0..KINDS {
            let mut variants = Vec::with_capacity(VARIANTS);
            for v in 0..VARIANTS {
                let mut rng = Rng::new(0x5EED_0000 ^ ((k as u32) << 8) ^ v as u32 ^ 0x9E37_79B9);
                let mut s = match k {
                    0 => grain(sr, &mut rng),
                    1 => bubble(sr, &mut rng, 0.0006, 0.0012, 0.020),
                    2 => bubble(sr, &mut rng, 0.0015, 0.0035, 0.045),
                    3 => crackle(sr, &mut rng),
                    4 => plop(sr, &mut rng),
                    5 => whoosh(sr, &mut rng),
                    _ => tink(sr, &mut rng),
                };
                normalise(&mut s, 0.9);
                variants.push(s);
            }
            sounds.push(variants);
        }
        Bank { sample_rate, sounds }
    }

    pub fn get(&self, kind: Kind, variant: usize) -> &[f32] {
        &self.sounds[kind as usize][variant % VARIANTS]
    }
}

fn samples(sr: f32, seconds: f32) -> usize {
    (sr * seconds) as usize
}

fn normalise(s: &mut [f32], peak: f32) {
    let m = s.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
    if m > 0.0 {
        for x in s.iter_mut() {
            *x *= peak / m;
        }
    }
}

/// Short fade-in so no sound starts with a click.
fn fade_in(s: &mut [f32], sr: f32, seconds: f32) {
    let n = samples(sr, seconds).max(1).min(s.len());
    for (i, x) in s.iter_mut().take(n).enumerate() {
        *x *= i as f32 / n as f32;
    }
}

/// A grain landing: 2-5 ms of noise through a resonant band-pass at 3-7 kHz.
/// Soft and dry: the "tsk" in a sand pour.
fn grain(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let len = samples(sr, 0.012);
    let f = 3000.0 + 4000.0 * rng.f();
    let c = SvfCoef::new(f, 3.0 + 3.0 * rng.f(), sr);
    let decay = 0.0015 + 0.002 * rng.f();
    let mut svf = Svf::default();
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let env = (-t / decay).exp();
            svf.process(rng.bi() * env, &c).band
        })
        .collect();
    fade_in(&mut out, sr, 0.0003);
    out
}

/// A bubble: Minnaert resonance f0 = 3.26 / r Hz (r in metres), with the
/// characteristic upward chirp as the bubble's radius settles.
fn bubble(sr: f32, rng: &mut Rng, r_min: f32, r_max: f32, decay: f32) -> Vec<f32> {
    let r = r_min + (r_max - r_min) * rng.f();
    let f0 = 3.26 / r;
    let d = decay * (0.7 + 0.6 * rng.f());
    let len = samples(sr, d * 6.0);
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            // Pitch rises ~30% over the first ~12 ms (the Minnaert chirp).
            let f = f0 * (1.0 + 0.3 * (1.0 - (-t / 0.012).exp()));
            phase += TAU * f / sr;
            phase.sin() * (-t / d).exp()
        })
        .collect();
    fade_in(&mut out, sr, 0.0008);
    out
}

/// A crackle: a sharp 1-4 ms pop, band-limited, with a faint resonant tail.
fn crackle(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let len = samples(sr, 0.03);
    let c = SvfCoef::new(1500.0 + 2500.0 * rng.f(), 1.2 + 2.0 * rng.f(), sr);
    let burst = 0.001 + 0.003 * rng.f();
    let mut svf = Svf::default();
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let x = if t < burst { rng.bi() } else { 0.0 };
            svf.process(x, &c).band * (-t / 0.006).exp()
        })
        .collect();
    fade_in(&mut out, sr, 0.0002);
    out
}

/// A lava plop: a big, low bubble (120-250 Hz) with a soft noisy burst.
fn plop(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let f0 = 120.0 + 130.0 * rng.f();
    let len = samples(sr, 0.35);
    let c = SvfCoef::new(500.0, 0.7, sr);
    let mut svf = Svf::default();
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let f = f0 * (1.0 + 0.5 * (1.0 - (-t / 0.04).exp()));
            phase += TAU * f / sr;
            let tone = phase.sin() * (-t / 0.09).exp();
            let noise = svf.process(rng.bi(), &c).low * (-t / 0.03).exp() * 0.6;
            tone + noise
        })
        .collect();
    fade_in(&mut out, sr, 0.004);
    out
}

/// A whoosh: noise through a band-pass sweeping up, with a slow swell.
fn whoosh(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let dur = 0.35 + 0.25 * rng.f();
    let len = samples(sr, dur);
    let (f_lo, f_hi) = (300.0 + 200.0 * rng.f(), 1200.0 + 800.0 * rng.f());
    let mut svf = Svf::default();
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let u = i as f32 / len as f32;
        let f = f_lo * (f_hi / f_lo).powf(u);
        let c = SvfCoef::new(f, 0.9, sr);
        let env = (u * std::f32::consts::PI).sin().powf(1.5);
        out.push(svf.process(rng.bi(), &c).band * env);
    }
    out
}

/// A tink: a small falling sine with a faint overtone, like cooling glass.
fn tink(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let (f1, f2) = (2200.0 + 600.0 * rng.f(), 1400.0 + 300.0 * rng.f());
    let dur = 0.12 + 0.08 * rng.f();
    let len = samples(sr, dur * 3.0);
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let f = f2 + (f1 - f2) * (-t / dur).exp();
            phase += TAU * f / sr;
            (phase.sin() + 0.25 * (2.76 * phase).sin()) * (-t / (dur * 0.6)).exp()
        })
        .collect();
    fade_in(&mut out, sr, 0.0005);
    out
}

/// Write a 16-bit PCM WAV file. `samples` are interleaved, `channels` per frame.
pub fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32, channels: u16) -> std::io::Result<()> {
    use std::io::Write;
    let data_len = (samples.len() * 2) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&channels.to_le_bytes())?;
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&(sample_rate * 2 * channels as u32).to_le_bytes())?; // bytes per second
    f.write_all(&(2 * channels).to_le_bytes())?; // bytes per frame
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for &s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}
