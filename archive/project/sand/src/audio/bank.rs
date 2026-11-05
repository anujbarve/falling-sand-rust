//! The one-shot sounds. Drips, bubbles, crackles and ignitions are cut from
//! recorded CC0 clips (embedded with `include_bytes!`, see
//! assets/audio/clips/README.md). Grains, plops, tinks and fry sizzles have no
//! CC0 recording yet, so they are synthesised once at startup (deterministic
//! seeds). The `bake` binary writes every one-shot to WAV files for listening.
//!
//! Each kind has several variants so repeated events never sound identical.

use super::dsp::{Rng, Svf, SvfCoef};
use std::f32::consts::TAU;

/// The one-shot kinds. Their order is the index into `Bank::sounds`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// One sand grain hitting others: a soft, woody micro-tick. Played in
    /// dense streams, these *are* the sound of pouring sand.
    Grain = 0,
    /// A small bubble in flowing water (Minnaert resonance, 0.8-2.7 kHz).
    /// Streams of these are the sound of running water.
    Drip,
    /// A boiling bubble: bigger, lower, rounder (450-1100 Hz).
    Bubble,
    /// Burning wood or oil popping: a soft pop with a woody body.
    Crackle,
    /// A thick lava bubble bursting: low, gloopy.
    Plop,
    /// A fuel catching fire: a soft breathy whoosh.
    Ignite,
    /// Lava setting into rock: a small, quiet cooling tink.
    Tink,
    /// One tiny pop of frying/sizzling; dense streams of these are a sizzle.
    Fry,
}

pub const KINDS: usize = 8;
pub const VARIANTS: usize = 8;
pub const KIND_NAMES: [&str; KINDS] = ["grain", "drip", "bubble", "crackle", "plop", "ignite", "tink", "fry"];

/// Recorded clips (CC0, see assets/audio/clips/README.md), embedded in the
/// binary so the game needs no files next to it.
const BUBBLE_1: &[u8] = include_bytes!("../../assets/audio/clips/bubbles/bubbles-single1.wav");
const BUBBLE_2: &[u8] = include_bytes!("../../assets/audio/clips/bubbles/bubbles-single2.wav");
const BUBBLE_3: &[u8] = include_bytes!("../../assets/audio/clips/bubbles/bubbles-single3.wav");
const WATER_LOOP: &[u8] = include_bytes!("../../assets/audio/clips/bubbles/bubbles-loop2-amp.wav");
const FIRE: &[u8] = include_bytes!("../../assets/audio/clips/fire/fire-1.wav");

pub struct Bank {
    pub sample_rate: f32,
    /// `sounds[kind][variant]`: mono samples, peak-normalised to 0.9.
    pub sounds: Vec<Vec<Vec<f32>>>,
    /// Seamless loops for the continuous beds: [bubbling water, fire].
    pub loops: [Vec<f32>; 2],
}

/// Decode a 16-bit PCM WAV into mono floats. Returns (sample rate, samples).
pub fn decode_wav(b: &[u8]) -> (f32, Vec<f32>) {
    let (mut channels, mut rate, mut data) = (1usize, 44_100f32, &b[..0]);
    let mut pos = 12;
    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let len = u32::from_le_bytes([b[pos + 4], b[pos + 5], b[pos + 6], b[pos + 7]]) as usize;
        let body = &b[pos + 8..(pos + 8 + len).min(b.len())];
        if id == b"fmt " {
            channels = u16::from_le_bytes([body[2], body[3]]).max(1) as usize;
            rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]) as f32;
        } else if id == b"data" {
            data = body;
        }
        pos += 8 + len + (len & 1);
    }
    let frames = data.len() / (2 * channels);
    let mono = (0..frames)
        .map(|f| {
            let sum: f32 = (0..channels)
                .map(|c| {
                    let i = (f * channels + c) * 2;
                    i16::from_le_bytes([data[i], data[i + 1]]) as f32 / 32768.0
                })
                .sum();
            sum / channels as f32
        })
        .collect();
    (rate, mono)
}

/// Linear resampling to the engine's rate.
fn resample(x: &[f32], from: f32, to: f32) -> Vec<f32> {
    if x.is_empty() {
        return Vec::new();
    }
    let n = (x.len() as f32 * to / from) as usize;
    (0..n)
        .map(|i| {
            let p = i as f32 * from / to;
            let j = p as usize;
            let f = p - j as f32;
            let a = x[j.min(x.len() - 1)];
            let b = x[(j + 1).min(x.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

/// A recording at the engine's rate.
fn clip(bytes: &[u8], sr: f32) -> Vec<f32> {
    let (rate, x) = decode_wav(bytes);
    resample(&x, rate, sr)
}

/// A piece of a recording: `start` and `len` in seconds.
fn piece(x: &[f32], sr: f32, start: f32, len: f32) -> Vec<f32> {
    let a = ((start * sr) as usize).min(x.len());
    let b = (a + (len * sr) as usize).min(x.len());
    x[a..b].to_vec()
}

/// A seamless loop from a recording: the last `xf` seconds are blended into
/// the first ones, so the wrap-around has no click.
fn seamless_loop(x: &[f32], sr: f32, xf: f32) -> Vec<f32> {
    let n = (xf * sr) as usize;
    if x.len() <= 2 * n {
        return x.to_vec();
    }
    let l = x.len();
    let mut body = x[..l - n].to_vec();
    for i in 0..n {
        let g = i as f32 / n as f32;
        body[i] = x[i] * g + x[l - n + i] * (1.0 - g);
    }
    body
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
                    1 => bubble(sr, &mut rng, 0.0012, 0.004, 0.009, 0.0005),
                    2 => bubble(sr, &mut rng, 0.003, 0.007, 0.035, 0.0015),
                    3 => crackle(sr, &mut rng),
                    4 => plop(sr, &mut rng),
                    5 => whoosh(sr, &mut rng),
                    6 => tink(sr, &mut rng),
                    _ => fry(sr, &mut rng),
                };
                // Everything gets the same warm top: nothing above ~7 kHz
                // matters for these sounds, and it is where harshness lives.
                lowpass(&mut s, sr, 7000.0);
                normalise(&mut s, 0.9);
                variants.push(s);
            }
            sounds.push(variants);
        }
        // Recorded sources. Each one-shot takes a different piece per variant.
        let bubbles = [clip(BUBBLE_1, sr), clip(BUBBLE_2, sr), clip(BUBBLE_3, sr)];
        let fire = clip(FIRE, sr);
        let loops = [seamless_loop(&clip(WATER_LOOP, sr), sr, 0.3), seamless_loop(&fire, sr, 0.25)];
        for (k, kind) in sounds.iter_mut().enumerate() {
            let variants: &mut Vec<Vec<f32>> = kind;
            for v in 0..VARIANTS {
                let recorded: Option<Vec<f32>> = match k {
                    // Drips: the short start of a bubble, higher up in pitch.
                    1 => Some(piece(&bubbles[v % 3], sr, 0.0, 0.12)),
                    // Boiling bubbles: the whole recorded bubble.
                    2 => Some(bubbles[v % 3].clone()),
                    // Crackles: short pops from the fire recording.
                    3 => Some(piece(&fire, sr, 0.15 + 0.25 * v as f32 % 2.2, 0.09)),
                    // Ignitions: a soft burst from the fire recording.
                    5 => Some(piece(&fire, sr, 0.05 + 0.3 * v as f32 % 2.0, 0.6)),
                    _ => None,
                };
                if let Some(mut r) = recorded {
                    fade_out(&mut r, sr, 0.004);
                    lowpass(&mut r, sr, 7000.0);
                    normalise(&mut r, 0.9);
                    variants[v] = r;
                }
            }
        }
        Bank { sample_rate, sounds, loops }
    }

    /// The sample `index` of a loop (wraps around).
    pub fn loop_sample(&self, which: usize, index: f32) -> f32 {
        let l = &self.loops[which];
        l[(index as usize) % l.len().max(1)]
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

fn lowpass(s: &mut [f32], sr: f32, cutoff: f32) {
    let c = SvfCoef::new(cutoff, 0.6, sr);
    let mut f = Svf::default();
    for x in s.iter_mut() {
        *x = f.process(*x, &c).low;
    }
}

/// Smooth envelope: a raised-cosine rise over `attack` seconds (no click),
/// then exponential decay with time constant `decay`.
fn env(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack {
        0.5 - 0.5 * (std::f32::consts::PI * t / attack).cos()
    } else {
        (-(t - attack) / decay).exp()
    }
}

/// Fade the last few ms to exactly zero, so a sound never ends with a step.
fn fade_out(s: &mut [f32], sr: f32, seconds: f32) {
    let n = samples(sr, seconds).max(1).min(s.len());
    let len = s.len();
    for i in 0..n {
        s[len - 1 - i] *= i as f32 / n as f32;
    }
}

/// A grain: a few ms of noise through two soft resonances (the grain and
/// what it hits), 0.7-2.6 kHz. Quiet and woody, not a "tsk".
fn grain(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let len = samples(sr, 0.018);
    let f = 900.0 + 1700.0 * rng.f();
    let (c1, c2) = (SvfCoef::new(f, 2.0 + 2.0 * rng.f(), sr), SvfCoef::new(f * (0.45 + 0.1 * rng.f()), 1.5, sr));
    let (attack, decay) = (0.0006 + 0.0008 * rng.f(), 0.0018 + 0.0025 * rng.f());
    let (mut a, mut b) = (Svf::default(), Svf::default());
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let x = rng.bi() * env(t, attack, decay);
            a.process(x, &c1).band + 0.7 * b.process(x, &c2).band
        })
        .collect();
    fade_out(&mut out, sr, 0.003);
    out
}

/// A bubble: Minnaert resonance f0 = 3.26 / r Hz (r in metres), with the
/// characteristic upward chirp as the bubble's radius settles.
fn bubble(sr: f32, rng: &mut Rng, r_min: f32, r_max: f32, decay: f32, attack: f32) -> Vec<f32> {
    let r = r_min + (r_max - r_min) * rng.f();
    let f0 = 3.26 / r;
    let d = decay * (0.7 + 0.6 * rng.f());
    let len = samples(sr, attack + d * 6.0);
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            // Pitch rises ~20% over the first part of the sound (the chirp).
            let f = f0 * (1.0 + 0.2 * (1.0 - (-t / (d * 0.6)).exp()));
            phase += TAU * f / sr;
            phase.sin() * env(t, attack, d)
        })
        .collect();
    fade_out(&mut out, sr, 0.002);
    out
}

/// A crackle: a short soft pop through a woody resonance, with a low body.
fn crackle(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let len = samples(sr, 0.05);
    let c = SvfCoef::new(700.0 + 1500.0 * rng.f(), 1.5 + 1.5 * rng.f(), sr);
    let body = SvfCoef::new(250.0 + 250.0 * rng.f(), 3.0, sr);
    let (attack, decay) = (0.0003 + 0.0004 * rng.f(), 0.002 + 0.004 * rng.f());
    let (mut a, mut b) = (Svf::default(), Svf::default());
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let x = rng.bi() * env(t, attack, decay);
            a.process(x, &c).band + 1.5 * b.process(x, &body).band
        })
        .collect();
    fade_out(&mut out, sr, 0.005);
    out
}

/// A lava plop: a big, low bubble (110-220 Hz) with a soft, dark burst.
fn plop(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let f0 = 110.0 + 110.0 * rng.f();
    let len = samples(sr, 0.4);
    let c = SvfCoef::new(400.0, 0.7, sr);
    let mut svf = Svf::default();
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let f = f0 * (1.0 + 0.4 * (1.0 - (-t / 0.05).exp()));
            phase += TAU * f / sr;
            let tone = phase.sin() * env(t, 0.006, 0.09);
            let noise = svf.process(rng.bi(), &c).low * env(t, 0.004, 0.03) * 0.8;
            tone + noise
        })
        .collect();
    fade_out(&mut out, sr, 0.02);
    out
}

/// A whoosh: noise through a band-pass sweeping up, with a slow swell.
fn whoosh(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let dur = 0.45 + 0.3 * rng.f();
    let len = samples(sr, dur);
    let (f_lo, f_hi) = (180.0 + 120.0 * rng.f(), 700.0 + 500.0 * rng.f());
    let mut svf = Svf::default();
    let mut pink = super::dsp::Pink::default();
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let u = i as f32 / len as f32;
        let f = f_lo * (f_hi / f_lo).powf(u.sqrt());
        let c = SvfCoef::new(f, 0.8, sr);
        let e = (u * std::f32::consts::PI).sin().powf(1.5);
        out.push(svf.process(pink.next(rng.bi()), &c).band * e);
    }
    out
}

/// A tink: a small falling sine with a faint overtone, like cooling glass.
fn tink(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let (f1, f2) = (1500.0 + 400.0 * rng.f(), 1100.0 + 200.0 * rng.f());
    let dur = 0.1 + 0.06 * rng.f();
    let len = samples(sr, dur * 3.0);
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let f = f2 + (f1 - f2) * (-t / dur).exp();
            phase += TAU * f / sr;
            (phase.sin() + 0.2 * (2.76 * phase).sin()) * env(t, 0.002, dur * 0.6)
        })
        .collect();
    fade_out(&mut out, sr, 0.01);
    out
}

/// One frying pop: very short, soft, 1.5-3.5 kHz.
fn fry(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let len = samples(sr, 0.012);
    let c = SvfCoef::new(1500.0 + 2000.0 * rng.f(), 1.2, sr);
    let (attack, decay) = (0.0003, 0.0008 + 0.0015 * rng.f());
    let mut svf = Svf::default();
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            svf.process(rng.bi() * env(t, attack, decay), &c).band
        })
        .collect();
    fade_out(&mut out, sr, 0.003);
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
