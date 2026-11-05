//! Scene, controls, HUD and determinism tests.

use app::controls::{map_key, paint, Action, Controls, MAX_BRUSH};
use app::hud::format_line;
use app::scenes::{build, systems_for, Scene};
use macroquad::prelude::KeyCode;
use sandbox_core::config::load_config;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::save::save_world;
use sandbox_core::world::World;
use sandbox_systems::{registry, run_tick};

fn count(w: &World, m: Material) -> usize {
    w.cells.as_slice().iter().filter(|c| c.mat == m).count()
}

#[test]
fn each_scene_has_its_theme_material() {
    for (scene, m) in [
        (Scene::VolcanoIsland, Material::Lava),
        (Scene::Forest, Material::Wood),
        (Scene::OceanStorm, Material::Water),
        (Scene::ExplosionLab, Material::Gunpowder),
    ] {
        let w = build(scene, 1);
        assert!(count(&w, m) > 0, "{scene:?} has no {m:?}");
    }
}

#[test]
fn systems_for_keeps_registry_names_and_order() {
    let reg: Vec<_> = registry().iter().map(|s| s.name()).collect();
    for scene in Scene::ALL {
        let got: Vec<_> = systems_for(scene).iter().map(|s| s.name()).filter(|n| *n != "lab_fuse").collect();
        assert_eq!(got, reg, "{scene:?}");
    }
}

#[test]
fn scene_files_match_scene_parameters() {
    for scene in Scene::ALL {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(scene.config_path());
        let (cfg, warn) = load_config(&path);
        assert!(warn.is_none(), "{scene:?}: {warn:?}");
        let (wind, sea) = scene.ocean_params();
        assert_eq!(cfg.wind, wind, "{scene:?}");
        assert_eq!(cfg.sea_level, sea, "{scene:?}");
    }
}

#[test]
fn scene_names_round_trip() {
    for scene in Scene::ALL {
        assert_eq!(Scene::from_name(scene.name()), Some(scene));
    }
    assert_eq!(Scene::from_name("nope"), None);
}

fn save_bytes(scene: Scene, seed: u32, ticks: usize, tag: &str) -> Vec<u8> {
    let mut w = build(scene, seed);
    let mut systems = systems_for(scene);
    let mut rng = Rng::new(seed);
    for _ in 0..ticks {
        run_tick(&mut w, &mut systems, &mut rng);
        w.events.clear();
    }
    let path = std::env::temp_dir().join(format!("sandbox_det_{}_{tag}.txt", std::process::id()));
    save_world(&w, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    bytes
}

#[test]
fn same_scene_and_seed_give_identical_save_bytes() {
    for scene in Scene::ALL {
        let a = save_bytes(scene, 1, 300, "a");
        let b = save_bytes(scene, 1, 300, "b");
        assert_eq!(a, b, "{scene:?}");
    }
}

#[test]
fn map_key_covers_every_control() {
    let mats = [
        Material::Sand, Material::Water, Material::Oil, Material::Stone,
        Material::Wood, Material::Gunpowder, Material::Fire, Material::Lava,
    ];
    let keys = [
        KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4,
        KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8,
    ];
    for (k, m) in keys.into_iter().zip(mats) {
        assert_eq!(map_key(k), Some(Action::SelectMaterial(m)));
    }
    assert_eq!(map_key(KeyCode::LeftBracket), Some(Action::BrushSmaller));
    assert_eq!(map_key(KeyCode::RightBracket), Some(Action::BrushLarger));
    assert_eq!(map_key(KeyCode::S), Some(Action::Save));
    assert_eq!(map_key(KeyCode::L), Some(Action::Load));
    assert_eq!(map_key(KeyCode::Space), Some(Action::TogglePause));
    assert_eq!(map_key(KeyCode::Escape), Some(Action::Quit));
    assert_eq!(map_key(KeyCode::Key9), None);
    assert_eq!(map_key(KeyCode::A), None);
}

#[test]
fn controls_state_changes() {
    let mut c = Controls::new(3);
    assert_eq!(c.brush, 3);
    assert_eq!(c.apply(Action::SelectMaterial(Material::Oil)), None);
    assert_eq!(c.material, Material::Oil);
    c.apply(Action::BrushLarger);
    assert_eq!(c.brush, 4);
    for _ in 0..100 {
        c.apply(Action::BrushLarger);
    }
    assert_eq!(c.brush, MAX_BRUSH);
    for _ in 0..100 {
        c.apply(Action::BrushSmaller);
    }
    assert_eq!(c.brush, 1);
    c.apply(Action::TogglePause);
    assert!(c.paused);
    c.apply(Action::TogglePause);
    assert!(!c.paused);
    c.apply(Action::Quit);
    assert!(c.quit);
    assert_eq!(c.apply(Action::Save), Some(Action::Save));
    assert_eq!(c.apply(Action::Load), Some(Action::Load));
}

#[test]
fn paint_fills_disc_only_into_empty_cells_and_clips_at_edges() {
    let mut w = World::new(20, 20);
    w.cells.get_mut(10, 10).unwrap().mat = Material::Stone;
    paint(&mut w, 10, 10, Material::Sand, 2);
    assert_eq!(w.cells.get(10, 10).unwrap().mat, Material::Stone);
    assert_eq!(w.cells.get(11, 10).unwrap().mat, Material::Sand);
    assert_eq!(w.cells.get(13, 10).unwrap().mat, Material::Empty);
    paint(&mut w, 0, 0, Material::Lava, 3); // clipped, no panic
    assert_eq!(w.cells.get(0, 0).unwrap().mat, Material::Lava);
    assert_eq!(w.cells.get(0, 0).unwrap().temp, 1500.0);
    paint(&mut w, -50, 99, Material::Sand, 3);
}

#[test]
fn hud_line_format() {
    assert_eq!(format_line(60.0, 3.2), "FPS 60  tick 3.2 ms");
}
