//! Write the audio to WAV files, to listen to it outside the game (the game
//! does not need these files: it builds the identical sounds at startup).
//!
//!     cargo run --release --bin bake             # one-shots -> assets/audio/*.wav
//!     cargo run --release --bin bake -- mix 1 10 # scene 1, 10 s of the full mix
//!
//! `mix` runs a scene on the CPU on a small grid and records exactly what
//! the game would play: analysis per 1/60 s, then the engine.

use sand::audio::analysis::Analyzer;
use sand::audio::bank::{write_wav, Bank, KINDS, KIND_NAMES, VARIANTS};
use sand::audio::engine::Engine;
use sand::physics::{cpu::CpuSim, scene, STEPS_PER_SECOND, SCENES};
use std::path::Path;

const SR: u32 = 48_000;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new("assets/audio");
    std::fs::create_dir_all(dir)?;
    if args.first().map(String::as_str) == Some("mix") {
        let index: usize = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(1).min(SCENES.len() - 1);
        let seconds: f32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(10.0);
        let path = dir.join(format!("mix_{}.wav", SCENES[index].0.replace([' ', '+'], "")));
        mix(index, seconds, &path)?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    let bank = Bank::new(SR as f32);
    for k in 0..KINDS {
        for v in 0..VARIANTS {
            write_wav(&dir.join(format!("{}_{v}.wav", KIND_NAMES[k])), &bank.sounds[k][v], SR, 1)?;
        }
    }
    println!("wrote {} sounds to {}", KINDS * VARIANTS, dir.display());
    Ok(())
}

fn mix(index: usize, seconds: f32, path: &Path) -> std::io::Result<()> {
    let mut world = scene(index, 320, 180);
    let mut sim = CpuSim::new(&world);
    let mut analyzer = Analyzer::default();
    let mut engine = Engine::new(SR as f32);
    let fps = 60;
    let steps = STEPS_PER_SECOND / fps;
    let per_frame = (SR / fps) as usize;
    let mut stereo = vec![0.0f32; per_frame * 2];
    let mut all = Vec::new();
    for n in 0..(seconds * fps as f32) as usize {
        for _ in 0..steps {
            sim.step_parallel(&mut world);
        }
        let frame = analyzer.analyze(&world, 1.0 / fps as f32, 1.0, 0.8);
        if std::env::var("BAKE_DEBUG").is_ok() && n % fps as usize == 0 {
            let beds: Vec<String> = frame.beds.iter().map(|b| format!("{:.2}", b.level)).collect();
            let rates: Vec<String> = frame.rates.iter().map(|r| format!("{:.0}", r.iter().sum::<f32>())).collect();
            println!("t={:>2}s beds {beds:?} rates/s {rates:?}", n / fps as usize);
        }
        engine.set_frame(frame);
        engine.process(&mut stereo, 2);
        all.extend_from_slice(&stereo);
    }
    let peak = all.iter().fold(0.0f32, |a, x| a.max(x.abs()));
    let rms = (all.iter().map(|x| x * x).sum::<f32>() / all.len() as f32).sqrt();
    println!("peak {peak:.3}  rms {rms:.4}  ({} active one-shots at the end)", engine.active_voices());
    write_wav(path, &all, SR, 2)
}
