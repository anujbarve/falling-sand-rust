//! Golden-image tests for the renderer: a 64x64 scene (water basin, a lava
//! cell, a stone wall, one particle) rendered offscreen to a 256x256
//! `Rgba32Float` texture and checked by measured properties, not by pixels.
//! Skips with SKIPPED when no GPU adapter exists.

use sandbox_v2_gpu::materials::Material;
use sandbox_v2_gpu::{CellRaw, Gpu, GridState, ParticleRaw, Particles, Renderer};

const N: u32 = 64;
/// Output pixels per cell (target is N * PX square).
const PX: u32 = 4;
const OUT: u32 = N * PX;

const LAVA: (u32, u32) = (44, 30);
const OPEN_AIR: (u32, u32) = (36, 30);
const SHADOWED: (u32, u32) = (52, 30);
const WATER_DEEP: (u32, u32) = (14, 56);
const PARTICLE: [f32; 2] = [30.5, 12.5];

fn gpu_or_skip(name: &str) -> Option<Gpu> {
    let g = Gpu::new_headless();
    if g.is_none() {
        eprintln!("SKIPPED {name}: no GPU adapter available");
    }
    g
}

struct Scene {
    cells: Vec<CellRaw>,
}

impl Scene {
    fn empty() -> Scene {
        Scene {
            cells: vec![
                CellRaw {
                    temp: 293.0,
                    ..Default::default()
                };
                (N * N) as usize
            ],
        }
    }
    fn set(&mut self, x: u32, y: u32, m: Material, temp: f32) {
        let c = &mut self.cells[(y * N + x) as usize];
        c.mat = m as u32;
        c.temp = temp;
    }
}

/// Stone floor (rows 60..64), a stone basin (walls x = 4 and 24) filled with
/// water from row 48, one 1500 K lava cell at (44, 30) and a two-cell-thick
/// stone wall at x = 47..48 spanning rows 16..44.
fn golden_scene() -> Scene {
    let mut s = Scene::empty();
    for y in 60..N {
        for x in 0..N {
            s.set(x, y, Material::Stone, 293.0);
        }
    }
    for y in 44..60 {
        s.set(4, y, Material::Stone, 293.0);
        s.set(24, y, Material::Stone, 293.0);
    }
    for y in 48..60 {
        for x in 5..24 {
            s.set(x, y, Material::Water, 293.0);
        }
    }
    s.set(LAVA.0, LAVA.1, Material::Lava, 1500.0);
    for y in 16..45 {
        s.set(47, y, Material::Stone, 293.0);
        s.set(48, y, Material::Stone, 293.0);
    }
    s
}

fn particle() -> ParticleRaw {
    ParticleRaw {
        pos: PARTICLE,
        mat: Material::Sand as u32,
        temp: 293.0,
        life: f32::INFINITY,
        ..Default::default()
    }
}

/// Renders `scene` (+ `parts`) with `r` and reads the RGBA32F output back.
fn render(gpu: &Gpu, r: &mut Renderer, scene: &Scene, parts: &[ParticleRaw]) -> Vec<[f32; 4]> {
    let grid = GridState::new(gpu, N, N).expect("grid");
    grid.upload(gpu, &scene.cells);
    let mut p = Particles::new(gpu, 16);
    p.spawn(gpu, parts);
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("golden_target"),
        size: wgpu::Extent3d {
            width: OUT,
            height: OUT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    r.frame(gpu, &grid, &p, &view);
    let row = OUT * 16;
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("golden_readback"),
        size: u64::from(row * OUT),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(OUT),
            },
        },
        wgpu::Extent3d {
            width: OUT,
            height: OUT,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(Some(enc.finish()));
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("map golden"));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll golden");
    let data = staging
        .slice(..)
        .get_mapped_range()
        .expect("mapped")
        .to_vec();
    staging.unmap();
    bytemuck::pod_collect_to_vec(&data)
}

fn px(img: &[[f32; 4]], x: u32, y: u32) -> [f32; 4] {
    img[(y * OUT + x) as usize]
}

/// Output colour at the centre of cell (`x`, `y`).
fn cell(img: &[[f32; 4]], (x, y): (u32, u32)) -> [f32; 4] {
    px(img, x * PX + PX / 2, y * PX + PX / 2)
}

/// Rec. 709 relative luminance.
fn lum(c: [f32; 4]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Lava luminance must exceed the mean of the surrounding air (cells 4 away)
/// by at least this much.
const LAVA_MARGIN: f32 = 0.3;
/// Shadowed air must be at most this fraction of open air at equal distance.
const SHADOW_RATIO_MAX: f32 = 0.8;

#[test]
fn golden_scene_reads_as_materials() {
    let Some(gpu) = gpu_or_skip("golden_scene_reads_as_materials") else {
        return;
    };
    let mut r = Renderer::new(&gpu, wgpu::TextureFormat::Rgba32Float);
    let img = render(&gpu, &mut r, &golden_scene(), &[particle()]);

    let bad = img
        .iter()
        .flatten()
        .filter(|v| !v.is_finite() || **v < 0.0 || **v > 1.0)
        .count();
    eprintln!("non-finite or out of 0..1 components: {bad}");
    assert_eq!(bad, 0);

    let lava = lum(cell(&img, LAVA));
    let around = [(40, 30), (44, 26), (44, 34), (40, 26)];
    let air = around.iter().map(|&c| lum(cell(&img, c))).sum::<f32>() / around.len() as f32;
    eprintln!(
        "lava Y {lava:.4}, surrounding air Y {air:.4}, margin {:.4} (need {LAVA_MARGIN})",
        lava - air
    );
    assert!(lava - air > LAVA_MARGIN);

    let w = cell(&img, WATER_DEEP);
    eprintln!("water rgb {:.4} {:.4} {:.4}", w[0], w[1], w[2]);
    assert!(w[2] > w[0] && w[2] > w[1], "water must be blue-dominant");

    let open = lum(cell(&img, OPEN_AIR));
    let shadow = lum(cell(&img, SHADOWED));
    eprintln!(
        "open air Y {open:.4}, shadowed air Y {shadow:.4}, ratio {:.4} (need < {SHADOW_RATIO_MAX})",
        shadow / open
    );
    assert!(shadow < open * SHADOW_RATIO_MAX, "the wall casts a shadow");

    let part = lum(px(
        &img,
        (PARTICLE[0] * PX as f32) as u32,
        (PARTICLE[1] * PX as f32) as u32,
    ));
    let bg = lum(cell(&img, (30, 16)));
    eprintln!("particle Y {part:.4}, air below Y {bg:.4}");
    assert!(part > bg + 0.1, "particle is drawn");
}

#[test]
fn render_is_deterministic() {
    let Some(gpu) = gpu_or_skip("render_is_deterministic") else {
        return;
    };
    let scene = golden_scene();
    let mut r1 = Renderer::new(&gpu, wgpu::TextureFormat::Rgba32Float);
    let a = render(&gpu, &mut r1, &scene, &[particle()]);
    let b = render(&gpu, &mut r1, &scene, &[particle()]);
    let mut r2 = Renderer::new(&gpu, wgpu::TextureFormat::Rgba32Float);
    let c = render(&gpu, &mut r2, &scene, &[particle()]);
    let same = |x: &[[f32; 4]], y: &[[f32; 4]]| {
        bytemuck::cast_slice::<_, u8>(x) == bytemuck::cast_slice::<_, u8>(y)
    };
    eprintln!(
        "same renderer: identical {}, fresh renderer: identical {}",
        same(&a, &b),
        same(&a, &c)
    );
    assert!(same(&a, &b) && same(&a, &c));
}

// Blackbody ramp: below the glow threshold nothing glows; hotter lava is
// brighter and shifts from red toward yellow-white (g/r rises).
#[test]
fn emission_follows_blackbody_ramp() {
    let Some(gpu) = gpu_or_skip("emission_follows_blackbody_ramp") else {
        return;
    };
    let temps = [700.0f32, 1000.0, 1500.0, 3000.0];
    let mut r = Renderer::new(&gpu, wgpu::TextureFormat::Rgba32Float);
    let mut prev: Option<[f32; 4]> = None;
    for t in temps {
        let mut s = Scene::empty();
        s.set(32, 32, Material::Lava, t);
        let c = cell(&render(&gpu, &mut r, &s, &[]), (32, 32));
        eprintln!(
            "lava {t:>6} K: rgb {:.4} {:.4} {:.4}, Y {:.4}, g/r {:.4}",
            c[0],
            c[1],
            c[2],
            lum(c),
            c[1] / c[0].max(1e-6)
        );
        if let Some(p) = prev {
            assert!(lum(c) > lum(p) + 0.02, "{t} K brighter than cooler lava");
            if t > 1000.0 {
                assert!(c[1] / c[0] > p[1] / p[0], "{t} K yellower than cooler lava");
            }
        }
        prev = Some(c);
    }
}

// Cell edges are anti-aliased: across the water-surface boundary the colour
// changes over more than one pixel step (no hard pixel block).
#[test]
fn cell_edges_are_smooth() {
    let Some(gpu) = gpu_or_skip("cell_edges_are_smooth") else {
        return;
    };
    let mut r = Renderer::new(&gpu, wgpu::TextureFormat::Rgba32Float);
    let img = render(&gpu, &mut r, &golden_scene(), &[]);
    let x = 14 * PX + PX / 2;
    // Rows 46*PX .. 50*PX cross air (row 47) into the water surface (row 48).
    let col: Vec<f32> = (46 * PX..50 * PX).map(|y| px(&img, x, y)[2]).collect();
    let total = (col[col.len() - 1] - col[0]).abs();
    let max_step = col
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    eprintln!("edge blue profile {col:.3?}; max step {max_step:.4} of total {total:.4}");
    assert!(total > 0.02, "water differs from air");
    assert!(max_step < 0.75 * total, "transition spans several pixels");
}
