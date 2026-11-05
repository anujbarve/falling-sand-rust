//! Built-in scenes: per-scene config (grid size, timers) and the starting
//! grid. Only [`Scene::VolcanoIsland`] (the default) is built here; the
//! other scenes have their config but build a placeholder (stone floor)
//! until Task 10 adds them.

use sandbox_v2_gpu::materials::Material;
use sandbox_v2_gpu::reference_thermal::AMBIENT_K;
use sandbox_v2_gpu::CellRaw;

/// Lava temperature at the vent and from the brush, kelvin (v1 `LAVA_TEMP`).
pub const LAVA_K: f32 = 1500.0;

/// A built-in scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// A rock island with a lava vent in the sea. The default scene.
    VolcanoIsland,
    /// Trees beside a pond (config only; placeholder build).
    Forest,
    /// Deep water under strong wind (config only; placeholder build).
    OceanStorm,
    /// Gunpowder in a stone box (config only; placeholder build).
    ExplosionLab,
}

/// Size and timers of a scene. Times are seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneConfig {
    /// Grid width in cells.
    pub w: u32,
    /// Grid height in cells.
    pub h: u32,
    /// Ambient wind, cells/s (x right, y down).
    pub wind: [f32; 2],
    /// Seconds between two lava emissions at the vent (`None`: no vent).
    pub vent_period_s: Option<f32>,
    /// Seconds an ember particle thrown from the vent lives.
    pub ember_life_s: f32,
    /// Particle ring capacity.
    pub particle_cap: u32,
}

/// A built scene: row-major cells plus the vent cell, if any.
pub struct Built {
    /// `w * h` cells, row-major.
    pub cells: Vec<CellRaw>,
    /// Cell where the vent emits lava (above the crater mouth).
    pub vent: Option<(i32, i32)>,
}

impl Scene {
    /// Every scene, in menu order.
    pub const ALL: [Scene; 4] = [
        Scene::VolcanoIsland,
        Scene::Forest,
        Scene::OceanStorm,
        Scene::ExplosionLab,
    ];

    /// Command-line name (same names as v1).
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

    /// This scene's config (default grid 512 x 512 per the plan).
    pub fn config(self) -> SceneConfig {
        let base = SceneConfig {
            w: 512,
            h: 512,
            wind: [0.0, 0.0],
            vent_period_s: None,
            ember_life_s: 1.5,
            particle_cap: 200_000,
        };
        match self {
            Scene::VolcanoIsland => SceneConfig {
                wind: [5.0, 0.0],
                vent_period_s: Some(1.0 / 30.0),
                ..base
            },
            Scene::Forest => base,
            Scene::OceanStorm => SceneConfig {
                wind: [40.0, 0.0],
                ..base
            },
            Scene::ExplosionLab => base,
        }
    }
}

fn cell(mat: Material, temp: f32) -> CellRaw {
    CellRaw {
        mat: mat as u32,
        temp,
        ..Default::default()
    }
}

/// Small deterministic hash for cosmetic roughness.
fn rough(seed: u32, x: u32) -> u32 {
    sandbox_v2_gpu::reference::cell_hash(0, x, 0, seed)
}

/// Builds `scene` on a `w` x `h` grid. The Volcano layout is v1's 160 x 120
/// island scaled to the grid: stone floor, rock cone with a crater holding a
/// lava pool over the vent, a few trees on the slopes, sea around it.
pub fn build(scene: Scene, w: u32, h: u32, seed: u32) -> Built {
    let air = cell(Material::Empty, AMBIENT_K);
    let mut cells = vec![air; (w * h) as usize];
    // v1 coordinates (u in 0..160, v in 0..120) to this grid.
    let gx = |u: f32| (u * w as f32 / 160.0).round() as i32;
    let gy = |v: f32| (v * h as f32 / 120.0).round() as i32;
    let mut put = |x: i32, y: i32, c: CellRaw| {
        if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
            cells[(y as u32 * w + x as u32) as usize] = c;
        }
    };
    let floor = gy(112.0);
    for y in floor..h as i32 {
        for x in 0..w as i32 {
            put(x, y, cell(Material::Stone, AMBIENT_K));
        }
    }
    if scene != Scene::VolcanoIsland {
        return Built { cells, vent: None };
    }
    let top_of = |x: i32| {
        let u = x as f32 * 160.0 / w as f32;
        let slope = 45.0 - (u - 80.0).abs() * 0.9;
        (slope > 0.0).then(|| gy(95.0 - slope) + (rough(seed, x as u32) % 2) as i32)
    };
    for x in 0..w as i32 {
        if let Some(top) = top_of(x) {
            for y in top..floor {
                put(x, y, cell(Material::Rock, AMBIENT_K));
            }
        }
    }
    let vent_row = gy(58.0);
    for y in gy(52.0)..vent_row {
        for x in gx(76.0)..gx(85.0) {
            put(x, y, air);
        }
    }
    for y in gy(55.0)..vent_row {
        for x in gx(77.0)..gx(84.0) {
            put(x, y, cell(Material::Lava, LAVA_K));
        }
    }
    // Trees: 2-cell wood trunks on the crater rim (where the rising lava
    // mound touches them) and further down the slopes.
    let (rim_l, rim_r) = (gx(76.0) - 2, gx(85.0));
    for x in [rim_l, rim_r, gx(64.0), gx(96.0)] {
        if let Some(top) = top_of(x) {
            for y in top - gy(8.0)..top {
                for dx in 0..2 {
                    put(x + dx, y, cell(Material::Wood, AMBIENT_K));
                }
            }
        }
    }
    // Sea: water in every empty cell between sea level and the floor.
    for y in gy(95.0)..floor {
        for x in 0..w as i32 {
            let i = (y as u32 * w + x as u32) as usize;
            if cells[i].mat == Material::Empty as u32 {
                cells[i] = cell(Material::Water, AMBIENT_K);
            }
        }
    }
    // The vent pours from just above the crater mouth, so the pool fills and
    // overflows down the slopes; it stops by itself while lava covers it.
    Built {
        cells,
        vent: Some(((w / 2) as i32, gy(46.0))),
    }
}
