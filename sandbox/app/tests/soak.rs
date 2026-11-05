//! Headless soak: every scene runs 10 000 ticks with seed 1.

use app::scenes::{build, systems_for, Scene};
use sandbox_core::event::{Event, SoundKind};
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::run_tick;
use std::time::Instant;

const TICKS: usize = 10_000;

/// Structural solids that nothing in the sim should create or destroy (lava cools to Rock and wood burns, so those are excluded).
fn solids(w: &World) -> usize {
    w.cells
        .as_slice()
        .iter()
        .filter(|c| matches!(c.mat, Material::Stone | Material::Metal | Material::Glass))
        .count()
}

fn count(w: &World, m: Material) -> usize {
    w.cells.as_slice().iter().filter(|c| c.mat == m).count()
}

fn assert_finite(w: &World, scene: Scene) {
    for c in w.cells.as_slice() {
        assert!(c.temp.is_finite(), "{scene:?}: cell temp {}", c.temp);
    }
    for v in &w.air.vel {
        assert!(v.x.is_finite() && v.y.is_finite(), "{scene:?}: air vel");
    }
    assert!(w.air.pressure.iter().all(|v| v.is_finite()), "{scene:?}: pressure");
    assert!(w.air.vapor.iter().all(|v| v.is_finite()), "{scene:?}: vapor");
    assert!(w.waves.iter().all(|v| v.is_finite()), "{scene:?}: waves");
    assert!(w.light.iter().all(|v| v.is_finite()), "{scene:?}: light");
    assert!(w.time.is_finite());
    for p in &w.particles {
        assert!(
            p.pos.x.is_finite() && p.pos.y.is_finite() && p.vel.x.is_finite() && p.vel.y.is_finite()
                && p.temp.is_finite() && p.life.is_finite(),
            "{scene:?}: particle"
        );
    }
}

fn soak(scene: Scene) {
    let mut w = build(scene, 1);
    let mut systems = systems_for(scene);
    let mut rng = Rng::new(1);
    let start = solids(&w);
    let start_all = w.cells.as_slice().iter().filter(|c| c.mat != Material::Empty).count();
    let mut booms = 0;
    let start_sand = count(&w, Material::Sand);
    let start_gp = count(&w, Material::Gunpowder);
    let t0 = Instant::now();
    for i in 0..TICKS {
        run_tick(&mut w, &mut systems, &mut rng);
        booms += w.events.iter().filter(|e| matches!(e, Event::Sound { kind: SoundKind::Boom, .. })).count();
        w.events.clear();
        if i % 1000 == 999 {
            assert_finite(&w, scene);
        }
    }
    let mean_ms = t0.elapsed().as_secs_f64() * 1000.0 / TICKS as f64;
    assert_finite(&w, scene);
    let end = solids(&w);
    let end_all = w.cells.as_slice().iter().filter(|c| c.mat != Material::Empty).count();
    let drift = (end as f64 - start as f64).abs() / start.max(1) as f64;
    let gp = count(&w, Material::Gunpowder);
    println!("SOAK {scene:?} detail: booms {booms}, gunpowder {start_gp} -> {gp}");
    println!(
        "SOAK {scene:?}: mean tick {mean_ms:.3} ms, solids {start} -> {end} (drift {drift:.2e}), non-empty {start_all} -> {end_all}"
    );
    if scene == Scene::ExplosionLab {
        // Loss attribution is tested in `explosions_cause_extra_lab_loss`.
        return;
    }
    let sand = count(&w, Material::Sand);
    let sand_drift = (sand as f64 - start_sand as f64).abs() / start_sand.max(1) as f64;
    println!("SOAK {scene:?}: sand {start_sand} -> {sand} (drift {sand_drift:.2e})");
    assert!(sand_drift <= 1e-3, "{scene:?}: sand drift {sand_drift}");
    assert!(drift <= 1e-3, "{scene:?}: solid drift {drift}");
}

#[test]
fn volcano_island_soak() {
    soak(Scene::VolcanoIsland);
}

#[test]
fn forest_soak() {
    soak(Scene::Forest);
}

#[test]
fn ocean_storm_soak() {
    soak(Scene::OceanStorm);
}

#[test]
fn explosion_lab_soak() {
    soak(Scene::ExplosionLab);
}

/// Result of one headless lab run.
struct LabRun {
    booms: usize,
    gunpowder_lost: usize,
    non_empty_start: usize,
    non_empty_end: usize,
    max_particles: usize,
    ash: usize,
    metal_lost: usize,
}

fn non_empty(w: &World) -> usize {
    w.cells.as_slice().iter().filter(|c| c.mat != Material::Empty).count()
}

fn run_lab(fused: bool) -> LabRun {
    let mut w = build(Scene::ExplosionLab, 1);
    let mut systems: Vec<_> = systems_for(Scene::ExplosionLab)
        .into_iter()
        .filter(|s| fused || s.name() != "lab_fuse")
        .collect();
    let mut rng = Rng::new(1);
    let metal0 = count(&w, Material::Metal);
    let (gp0, ne0) = (count(&w, Material::Gunpowder), non_empty(&w));
    let (mut booms, mut max_particles) = (0, 0);
    for _ in 0..TICKS {
        run_tick(&mut w, &mut systems, &mut rng);
        booms += w.events.iter().filter(|e| matches!(e, Event::Sound { kind: SoundKind::Boom, .. })).count();
        w.events.clear();
        max_particles = max_particles.max(w.particles.len());
    }
    LabRun {
        booms,
        gunpowder_lost: gp0 - count(&w, Material::Gunpowder).min(gp0),
        non_empty_start: ne0,
        non_empty_end: non_empty(&w),
        max_particles,
        ash: count(&w, Material::Ash),
        metal_lost: metal0 - count(&w, Material::Metal).min(metal0),
    }
}

/// Blasts must be heard, throw debris particles, leave extra ash, destroy metal, and must destroy more
/// gunpowder than the same world with only its starting flame (control run, fuse removed).
#[test]
fn explosions_cause_extra_lab_loss() {
    let fused = run_lab(true);
    let control = run_lab(false);
    println!(
        "LAB fused: booms {}, gunpowder lost {}, non-empty {} -> {}, max particles {}; control: booms {}, gunpowder lost {}, non-empty {} -> {}, max particles {}",
        fused.booms, fused.gunpowder_lost, fused.non_empty_start, fused.non_empty_end, fused.max_particles,
        control.booms, control.gunpowder_lost, control.non_empty_start, control.non_empty_end, control.max_particles,
    );
    println!("LAB ash fused {} control {}; metal lost fused {} control {}", fused.ash, control.ash, fused.metal_lost, control.metal_lost);
    assert!(fused.booms > 0);
    assert_eq!(control.booms, 0);
    assert!(fused.max_particles > control.max_particles, "no debris particles from blasts");
    assert!(fused.ash > control.ash, "blasts left no extra ash");
    assert!(fused.metal_lost > 0 && control.metal_lost == 0, "blasts did not destroy metal");
    assert!(fused.gunpowder_lost > control.gunpowder_lost, "blasts did not add gunpowder loss");
}
