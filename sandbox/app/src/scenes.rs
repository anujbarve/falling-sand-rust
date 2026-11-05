//! The four built-in scenes: their starting worlds and the system lists that run them.

use sandbox_core::event::{Event, SoundKind};
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use sandbox_systems::air::AirSystem;
use sandbox_systems::chemistry::Chemistry;
use sandbox_systems::collisions::Collisions;
use sandbox_systems::explosions::Explosions;
use sandbox_systems::fire::{Fire, DEFAULT_FUEL};
use sandbox_systems::granular::Granular;
use sandbox_systems::humidity::Humidity;
use sandbox_systems::lighting::Lighting;
use sandbox_systems::liquid::Liquid;
use sandbox_systems::ocean::Ocean;
use sandbox_systems::particles::Particles;
use sandbox_systems::thermal::Thermal;
use sandbox_systems::volcano::{Volcano, VolcanoSource, LAVA_TEMP};
use sandbox_systems::System;
use std::path::PathBuf;

/// Width of every scene's grid, in cells.
pub const WIDTH: usize = 160;
/// Height of every scene's grid, in cells.
pub const HEIGHT: usize = 120;
/// Row where the stone floor begins in every scene.
const FLOOR: i32 = 112;
/// Position of the volcano vent: lava appears one cell above it.
pub const VENT: (i32, i32) = (80, 58);

/// A built-in scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// A rock island with a lava vent in the sea. The default scene.
    VolcanoIsland,
    /// Wooden trees on sand beside a pond.
    Forest,
    /// Deep water under strong wind.
    OceanStorm,
    /// Gunpowder piles in a stone box with a timed fuse.
    ExplosionLab,
}

impl Scene {
    /// Every scene, in menu order.
    pub const ALL: [Scene; 4] = [Scene::VolcanoIsland, Scene::Forest, Scene::OceanStorm, Scene::ExplosionLab];

    /// Command-line name, also the stem of the scene's config file.
    pub fn name(self) -> &'static str {
        match self {
            Scene::VolcanoIsland => "volcano_island",
            Scene::Forest => "forest",
            Scene::OceanStorm => "ocean_storm",
            Scene::ExplosionLab => "explosion_lab",
        }
    }

    /// Finds a scene by [`Scene::name`].
    pub fn from_name(name: &str) -> Option<Scene> {
        Scene::ALL.into_iter().find(|s| s.name() == name)
    }

    /// Config file of this scene, relative to the sandbox root: `config/scenes/<name>.toml`.
    pub fn config_path(self) -> PathBuf {
        PathBuf::from("config").join("scenes").join(format!("{}.toml", self.name()))
    }

    /// `(wind, sea_level)` used by [`systems_for`]; the scene's TOML file repeats these values.
    pub fn ocean_params(self) -> (f32, i32) {
        match self {
            Scene::VolcanoIsland => (2.0, 95),
            Scene::Forest => (0.0, 103),
            Scene::OceanStorm => (6.0, 70),
            Scene::ExplosionLab => (0.0, 0),
        }
    }
}

fn fill(w: &mut World, x0: i32, y0: i32, x1: i32, y1: i32, mat: Material) {
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(c) = w.cells.get_mut(x, y) {
                c.mat = mat;
            }
        }
    }
}

fn set(w: &mut World, x: i32, y: i32, mat: Material, temp: f32, fuel: u8) {
    if let Some(c) = w.cells.get_mut(x, y) {
        c.mat = mat;
        c.temp = temp;
        c.fuel = fuel;
    }
}

/// Fills every empty cell in rows `y0..y1` with water.
fn flood(w: &mut World, y0: i32, y1: i32) {
    for y in y0..y1 {
        for x in 0..WIDTH as i32 {
            if w.cells.get(x, y).is_some_and(|c| c.mat == Material::Empty) {
                set(w, x, y, Material::Water, 293.0, 0);
            }
        }
    }
}

/// Builds the starting world of `scene`; `seed` only varies cosmetic details (island roughness, tree heights).
pub fn build(scene: Scene, seed: u32) -> World {
    let mut w = World::new(WIDTH, HEIGHT);
    let mut rng = Rng::new(seed);
    fill(&mut w, 0, FLOOR, WIDTH as i32, HEIGHT as i32, Material::Stone);
    match scene {
        Scene::VolcanoIsland => {
            for x in 0..WIDTH as i32 {
                let slope = 45.0 - (x - 80).abs() as f32 * 0.9;
                if slope > 0.0 {
                    let top = 95 - slope as i32 + rng.range_f32(0.0, 2.0) as i32;
                    fill(&mut w, x, top, x + 1, FLOOR, Material::Rock);
                }
            }
            // Crater: a pit above the vent, whose floor is the vent cell itself.
            fill(&mut w, 76, VENT.1 - 6, 85, VENT.1, Material::Empty);
            fill(&mut w, 76, VENT.1, 85, VENT.1 + 1, Material::Rock);
            // A pool of lava already in the crater.
            for y in VENT.1 - 3..VENT.1 {
                for x in 77..84 {
                    set(&mut w, x, y, Material::Lava, LAVA_TEMP, 0);
                }
            }
            flood(&mut w, 95, FLOOR);
        }
        Scene::Forest => {
            fill(&mut w, 0, 104, WIDTH as i32, FLOOR, Material::Sand);
            // Pond between stone walls.
            fill(&mut w, 117, 100, 118, 104, Material::Stone);
            fill(&mut w, 141, 100, 142, 104, Material::Stone);
            fill(&mut w, 118, 103, 141, 104, Material::Water);
            let mut x = 12;
            while x < 110 {
                let h = 14 + (rng.next_u32() % 9) as i32;
                fill(&mut w, x, 104 - h, x + 2, 104, Material::Wood);
                fill(&mut w, x - 4, 104 - h - 6, x + 6, 104 - h, Material::Wood);
                x += 16;
            }
        }
        Scene::OceanStorm => {
            fill(&mut w, 78, 56, 82, FLOOR, Material::Metal);
            fill(&mut w, 74, 54, 86, 56, Material::Metal);
            flood(&mut w, 70, FLOOR);
        }
        Scene::ExplosionLab => {
            fill(&mut w, 14, 60, 16, FLOOR, Material::Stone);
            fill(&mut w, 144, 60, 146, FLOOR, Material::Stone);
            fill(&mut w, 30, 100, 60, FLOOR, Material::Gunpowder);
            fill(&mut w, 100, 100, 130, FLOOR, Material::Gunpowder);
            fill(&mut w, 70, 96, 90, 98, Material::Metal);
            // A small flame on the left pile.
            set(&mut w, 45, 99, Material::Fire, 800.0, DEFAULT_FUEL);
        }
    }
    w
}

/// Explosion lab fuse: every `PERIOD` ticks after the first `FIRST`, detonates a charge
/// above the left pile, the metal plate (throws debris) and the right pile in turn. Emits an `Explosion` (consumed by `Explosions`)
/// and a `Boom` sound (survives to the app so the blast is heard).
struct LabFuse {
    tick: u64,
}

impl LabFuse {
    const FIRST: u64 = 120;
    const PERIOD: u64 = 300;
}

impl System for LabFuse {
    fn name(&self) -> &'static str {
        "lab_fuse"
    }

    fn step(&mut self, w: &mut World, _rng: &mut Rng, _dt: f32) {
        self.tick += 1;
        if self.tick < Self::FIRST || (self.tick - Self::FIRST) % Self::PERIOD != 0 {
            return;
        }
        let x = [45.0, 80.0, 115.0][(((self.tick - Self::FIRST) / Self::PERIOD) % 3) as usize];
        w.events.push(Event::Explosion { x, y: 98.0, radius: 10.0, energy: 80.0 });
        w.events.push(Event::Sound { kind: SoundKind::Boom, intensity: 0.8, x, y: 98.0 });
    }
}

/// Systems for `scene` using the scene's own wind and sea level. Same order as `sandbox_systems::registry`
/// (the explosion lab adds a fuse just before `explosions`).
pub fn systems_for(scene: Scene) -> Vec<Box<dyn System>> {
    let (wind, sea_level) = scene.ocean_params();
    systems_with(scene, wind, sea_level)
}

/// Like [`systems_for`] but with the wind and sea level taken from a loaded config.
pub fn systems_with(scene: Scene, wind: f32, sea_level: i32) -> Vec<Box<dyn System>> {
    let sources = match scene {
        Scene::VolcanoIsland => vec![VolcanoSource { x: VENT.0, y: VENT.1, rate: 0.5 }],
        _ => Vec::new(),
    };
    let air_wind = Vec2 { x: (wind * 0.2).clamp(-3.0, 3.0), y: 0.0 };
    let mut v: Vec<Box<dyn System>> = vec![
        Box::new(Granular::default()),
        Box::new(Liquid::default()),
        Box::new(AirSystem { wind: air_wind }),
        Box::new(Thermal),
        Box::new(Humidity),
        Box::new(Fire::default()),
        Box::new(Volcano { sources }),
        Box::new(Particles),
        Box::new(Collisions),
    ];
    if scene == Scene::ExplosionLab {
        v.push(Box::new(LabFuse { tick: 0 }));
    }
    v.push(Box::new(Explosions::default()));
    v.push(Box::new(Chemistry));
    v.push(Box::new(Ocean { wind_speed: wind, sea_level }));
    v.push(Box::new(Lighting));
    v
}

