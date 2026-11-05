//! Sim loop, events, carried gates, save/load and device loss. GPU tests
//! skip with a message when the machine has no adapter.

use sandbox_v2_app::controls::{cell_at, map_key, stroke, Action, KEY_MATERIALS};
use sandbox_v2_app::frame::{FixedStep, Sim, EVENT_PERIOD_TICKS, MAX_TICKS_PER_FRAME};
use sandbox_v2_app::save::fuel_byte;
use sandbox_v2_app::scenes::Scene;
use sandbox_v2_app::sounds::sounds;
use sandbox_v2_gpu::materials::Material;
use sandbox_v2_gpu::{drawable, CellRaw, EventKind, Gpu};
use winit::keyboard::{Key, NamedKey};

const SEED: u32 = 1;

fn gpu_or_skip() -> Option<Gpu> {
    let g = Gpu::new_headless();
    if g.is_none() {
        eprintln!("SKIPPED: no GPU adapter available");
    }
    g
}

fn floats(c: &CellRaw) -> [f32; 6] {
    [c.temp, c.fuel, c.vx, c.vy, c.vapor, c.pressure]
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sandbox_v2_loop_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// (a) 10 s of Volcano Island at 256x256: flow and crackle heard, no NaN,
/// no 0 K hole, no GPU error.
#[test]
fn volcano_600_ticks_events_no_nan() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::VolcanoIsland, 256, 256, SEED).unwrap();
    sim.run(600);
    sim.flush_events();
    let t = sim.event_totals();
    let cells = sim.readback().unwrap();
    let nan = cells
        .iter()
        .filter(|c| floats(c).iter().any(|v| !v.is_finite()))
        .count();
    let zero_k = cells.iter().filter(|c| c.temp == 0.0).count();
    println!(
        "600 ticks: flow {} impact {} crackle {} boom {}; NaN cells {nan}; 0 K cells {zero_k}; \
         particles {}; gpu errors {}; gpu ms {:?}",
        t[EventKind::Flow as usize],
        t[EventKind::Impact as usize],
        t[EventKind::Crackle as usize],
        t[EventKind::Boom as usize],
        sim.parts().count(),
        sim.gpu_errors(),
        sim.gpu_ms(),
    );
    assert!(t[EventKind::Flow as usize] > 0, "no flow events");
    assert!(t[EventKind::Crackle as usize] > 0, "no crackle events");
    assert_eq!(nan, 0);
    assert_eq!(zero_k, 0, "0 K holes");
    assert_eq!(sim.gpu_errors(), 0);
    assert!(sim.parts().count() > 0, "vent threw no embers");
}

/// (b) The accumulator never yields more than 4 ticks per frame, and a
/// steady 60 Hz frame gives exactly one tick.
#[test]
fn at_most_four_ticks_per_frame() {
    let mut c = FixedStep::new();
    assert_eq!(c.push(1.0), MAX_TICKS_PER_FRAME);
    assert!(c.push(0.0) <= 1, "backlog beyond the cap is dropped");
    let mut c = FixedStep::new();
    let mut total = 0;
    for _ in 0..600 {
        let n = c.push(1.0 / 60.0);
        assert_eq!(n, 1);
        total += n;
    }
    assert_eq!(total, 600);
    let mut c = FixedStep::new();
    let mut h = 12345u32;
    for _ in 0..10_000 {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        let dt = (h % 1000) as f32 / 1000.0 * 0.3;
        assert!(c.push(dt) <= MAX_TICKS_PER_FRAME);
    }
    assert_eq!(c.push(f32::NAN), 0);
    assert_eq!(c.push(-1.0), 0);
}

/// (b) Through the sim: a long frame runs 4 ticks, not 60.
#[test]
fn sim_advance_caps_ticks() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::VolcanoIsland, 64, 64, SEED).unwrap();
    let mut clock = FixedStep::new();
    assert_eq!(sim.advance(&mut clock, 1.0), MAX_TICKS_PER_FRAME);
    assert_eq!(sim.tick_count(), MAX_TICKS_PER_FRAME);
}

/// (c) Carried gate: after granular and liquid moves (and everything else),
/// no cell with a material sits at 0 K.
#[test]
fn no_zero_kelvin_cells_after_moves() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::VolcanoIsland, 128, 128, SEED).unwrap();
    // Drop sand and water into the sky so they fall through air.
    for x in (10..118).step_by(6) {
        sim.paint(stroke(Material::Sand, x, 8, 2));
        sim.paint(stroke(Material::Water, x + 3, 14, 2));
    }
    sim.run(240);
    let cells = sim.readback().unwrap();
    let zero_k_mat = cells
        .iter()
        .filter(|c| c.mat != Material::Empty as u32 && c.temp == 0.0)
        .count();
    // Nothing in this scene is colder than ambient, so a cell well below it
    // is a hole that conduction has only partly refilled.
    let cold = cells.iter().filter(|c| c.temp < 250.0).count();
    let min_t = cells.iter().map(|c| c.temp).fold(f32::INFINITY, f32::min);
    println!("0 K cells with a material: {zero_k_mat}; cells below 250 K: {cold}; min {min_t} K");
    assert_eq!(zero_k_mat, 0);
    assert_eq!(cold, 0);
}

/// (d) Carried gate: one submit per tick for all passes.
#[test]
fn one_submit_per_tick() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::VolcanoIsland, 64, 64, SEED).unwrap();
    let before = sim.submits();
    let n = 10 * EVENT_PERIOD_TICKS;
    for _ in 0..n {
        sim.tick();
    }
    let per_tick = (sim.submits() - before) as f64 / f64::from(n);
    println!("submits per tick: {per_tick}");
    assert_eq!(sim.submits() - before, u64::from(n));
}

/// (e) Device loss: ticking a lost device never panics; recovery rebuilds
/// on a new device and reloads the last save.
#[test]
fn device_loss_recovers_and_reloads_save() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::VolcanoIsland, 64, 64, SEED).unwrap();
    sim.run(30);
    let path = tmp("loss.txt");
    sim.save(&path).unwrap();
    let saved = sim.readback().unwrap();
    sim.run(30);
    sim.gpu().device.destroy();
    for _ in 0..5 {
        sim.tick();
        sim.poll();
    }
    // wgpu reports a destroyed device once its queue has drained.
    let _ = sim.gpu().device.poll(wgpu::PollType::wait_indefinitely());
    assert!(sim.is_lost(), "lost-device callback did not fire");
    assert!(sim.readback().is_none());
    assert!(sim.save(&tmp("never.txt")).is_err());
    let msg = sim.recover(Gpu::new_headless().unwrap(), Some(&path));
    println!("recovery message: {msg}");
    assert!(!sim.is_lost());
    assert!(msg.contains("reloaded"));
    let now = sim.readback().unwrap();
    let diff = now
        .iter()
        .zip(&saved)
        .filter(|(a, b)| {
            a.mat != b.mat
                || a.temp.to_bits() != b.temp.to_bits()
                || a.fuel != f32::from(fuel_byte(b.fuel))
                || a.vapor.to_bits() != b.vapor.to_bits()
        })
        .count();
    println!("cells differing from the save after recovery: {diff}");
    assert_eq!(diff, 0);
    sim.run(10);
    assert_eq!(sim.gpu_errors(), 0);
    // No save: the scene restarts.
    let gpu2 = Gpu::new_headless().unwrap();
    sim.gpu().device.destroy();
    sim.tick();
    let _ = sim.gpu().device.poll(wgpu::PollType::wait_indefinitely());
    assert!(sim.is_lost());
    let msg = sim.recover(gpu2, None);
    assert!(msg.contains("restarted"), "{msg}");
}

/// (f) Save/load round trip through GPU readback and the v1 text format;
/// a corrupt file leaves the world untouched.
#[test]
fn save_load_round_trip() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::VolcanoIsland, 96, 64, SEED).unwrap();
    sim.run(90);
    let path = tmp("round.txt");
    sim.save(&path).unwrap();
    let a = sim.snapshot().unwrap();
    sim.run(60);
    sim.load(&path).unwrap();
    let b = sim.snapshot().unwrap();
    assert_eq!((b.w, b.h), (96, 64));
    let cell_diff = a
        .cells
        .iter()
        .zip(&b.cells)
        .filter(|(x, y)| {
            x.mat != y.mat
                || x.temp.to_bits() != y.temp.to_bits()
                || f32::from(fuel_byte(x.fuel)) != y.fuel
                || x.vapor.to_bits() != y.vapor.to_bits()
                || x.pressure.to_bits() != y.pressure.to_bits()
        })
        .count();
    let vel_diff = a
        .air_vel
        .iter()
        .zip(&b.air_vel)
        .filter(|(x, y)| x.map(f32::to_bits) != y.map(f32::to_bits))
        .count();
    println!("round trip: {cell_diff} cell diffs, {vel_diff} air velocity diffs");
    assert_eq!(cell_diff, 0);
    assert_eq!(vel_diff, 0);
    // v1 can read it: it is the v1 format.
    let w = sandbox_core::save::load_world(&path).unwrap();
    assert_eq!((w.cells.width(), w.cells.height()), (96, 64));
    // Corrupt file: error, world untouched.
    let bad = tmp("bad.txt");
    std::fs::write(&bad, "version 1\nsize 96 64\n1 nan 0\n").unwrap();
    let before = sim.readback().unwrap();
    let err = sim.load(&bad).unwrap_err();
    println!("corrupt load: {err}");
    assert_eq!(sim.readback().unwrap(), before);
    std::fs::write(&bad, "version 9\n").unwrap();
    assert!(sim.load(&bad).unwrap_err().contains("version"));
}

/// Review Focus 8: NaN/inf in uploaded float state is replaced and counted.
#[test]
fn upload_sanitises_non_finite() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::VolcanoIsland, 32, 32, SEED).unwrap();
    let base = sim.sanitized();
    let mut cells = sim.readback().unwrap();
    cells[5].temp = f32::NAN;
    cells[6].vapor = f32::INFINITY;
    cells[7].vy = f32::NEG_INFINITY;
    sim.load_cells(&cells);
    let mut s = stroke(Material::Sand, 3, 3, 1);
    s.temp = f32::NAN;
    sim.paint(s);
    sim.run(5);
    assert_eq!(sim.sanitized() - base, 4);
    let out = sim.readback().unwrap();
    assert!(out.iter().all(|c| floats(c).iter().all(|v| v.is_finite())));
}

/// Review Focus 2: strokes at and beyond the grid edge never write out of
/// bounds and paint only the in-grid part.
#[test]
fn brush_at_edges_is_bounded() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut sim = Sim::new(gpu, Scene::Forest, 32, 32, SEED).unwrap();
    let before = sim.readback().unwrap();
    for (x, y) in [(0, 0), (31, 0), (-5, 10), (40, 40), (31, 31)] {
        sim.paint(stroke(Material::Wood, x, y, 3));
    }
    sim.apply_strokes();
    let after = sim.readback().unwrap();
    let painted = after
        .iter()
        .zip(&before)
        .filter(|(a, b)| a.mat != b.mat)
        .count();
    println!("edge strokes painted {painted} cells");
    assert!(painted > 0);
    assert_eq!(sim.gpu_errors(), 0);
}

/// Controls keep the v1 mapping.
#[test]
fn key_mapping_matches_v1() {
    for (i, m) in KEY_MATERIALS.iter().enumerate() {
        let k = Key::Character((i + 1).to_string().into());
        assert_eq!(map_key(&k), Some(Action::SelectMaterial(*m)));
    }
    assert_eq!(KEY_MATERIALS[6], Material::Fire);
    assert_eq!(map_key(&Key::Character("9".into())), None);
    assert_eq!(
        map_key(&Key::Character("[".into())),
        Some(Action::BrushSmaller)
    );
    assert_eq!(
        map_key(&Key::Character("]".into())),
        Some(Action::BrushLarger)
    );
    assert_eq!(map_key(&Key::Character("s".into())), Some(Action::Save));
    assert_eq!(map_key(&Key::Character("L".into())), Some(Action::Load));
    assert_eq!(
        map_key(&Key::Named(NamedKey::Space)),
        Some(Action::TogglePause)
    );
    assert_eq!(map_key(&Key::Named(NamedKey::Escape)), Some(Action::Quit));
}

/// Review Focus 1: a 0x0 (minimised) window is not drawable and maps no
/// mouse position; the sim keeps stepping regardless (the app ticks
/// outside the redraw handler).
#[test]
fn minimised_window_skips_render_only() {
    assert!(!drawable(0, 0));
    assert!(!drawable(0, 800));
    assert!(drawable(1, 1));
    assert_eq!(cell_at(10.0, 10.0, (0, 0), (256, 256)), None);
    assert_eq!(
        cell_at(640.0, 400.0, (1280, 800), (256, 256)),
        Some((128, 128))
    );
    assert_eq!(cell_at(1280.0, 0.0, (1280, 800), (256, 256)), None);
}

/// Event counts become bounded audio intensities.
#[test]
fn event_counts_map_to_sounds() {
    assert!(sounds([0; 4]).is_empty());
    let s = sounds([1000, 0, 3, 1]);
    assert_eq!(s.len(), 3);
    for e in s {
        let sandbox_core::event::Event::Sound { intensity, .. } = e else {
            panic!("not a sound")
        };
        assert!(intensity > 0.0 && intensity < 1.0);
    }
    // The audio handle never blocks, even with no device.
    let dead = sandbox_audio::AudioHandle::dead();
    for _ in 0..1000 {
        dead.send_events(&sounds([5, 5, 5, 5]));
    }
}
