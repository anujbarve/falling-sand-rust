//! Air tests: advection, conservation, stability, solids blocking air, and
//! GPU/CPU parity. Property checks run on both the CPU reference and the GPU
//! (the GPU half skips without an adapter).

use sandbox_v2_gpu::air::{is_solid, AirReference, AIR_SWEEPS};
use sandbox_v2_gpu::materials::Material;
use sandbox_v2_gpu::units::DT;
use sandbox_v2_gpu::{AirPass, CellRaw, Gpu, GridState};

const STONE: u32 = Material::Stone as u32;

/// Initial scene: per-cell material, temperature and vapor, plus wind.
#[derive(Clone)]
struct Scene {
    w: u32,
    h: u32,
    mat: Vec<u32>,
    temp: Vec<f32>,
    vapor: Vec<f32>,
    wind: [f32; 2],
}

/// Final state after a run.
struct Out {
    mat: Vec<u32>,
    vapor: Vec<f32>,
    pressure: Vec<f32>,
    vel: Vec<[f32; 2]>,
}

impl Scene {
    fn new(w: u32, h: u32, wind: [f32; 2]) -> Scene {
        let n = (w * h) as usize;
        Scene {
            w,
            h,
            mat: vec![0; n],
            temp: vec![293.0; n],
            vapor: vec![0.0; n],
            wind,
        }
    }
    fn i(&self, x: u32, y: u32) -> usize {
        (y * self.w + x) as usize
    }
    fn puff(&mut self, cx: u32, cy: u32, r: u32) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let i = self.i(x, y);
                self.vapor[i] = 1.0;
            }
        }
    }
}

fn run_cpu(s: &Scene, ticks: u32) -> Out {
    let mut r = AirReference::new(s.w, s.h);
    r.mat.clone_from(&s.mat);
    r.temp.clone_from(&s.temp);
    r.vapor.clone_from(&s.vapor);
    for _ in 0..ticks {
        r.tick(DT, s.wind);
    }
    Out {
        mat: r.mat,
        vapor: r.vapor,
        pressure: r.pressure,
        vel: r.vel,
    }
}

fn run_gpu(gpu: &Gpu, s: &Scene, ticks: u32) -> Out {
    let grid = GridState::new(gpu, s.w, s.h).unwrap();
    let cells: Vec<CellRaw> = (0..s.mat.len())
        .map(|i| CellRaw {
            mat: s.mat[i],
            temp: s.temp[i],
            vapor: s.vapor[i],
            ..Default::default()
        })
        .collect();
    grid.upload(gpu, &cells);
    let air = AirPass::new(gpu);
    for t in 0..ticks {
        air.dispatch(gpu, &grid, DT, s.wind);
        if t % 64 == 63 {
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
        }
    }
    let got = grid.readback(gpu);
    Out {
        mat: got.iter().map(|c| c.mat).collect(),
        vapor: got.iter().map(|c| c.vapor).collect(),
        pressure: got.iter().map(|c| c.pressure).collect(),
        vel: air.read_velocity(gpu, &grid),
    }
}

fn com_x(s: &Scene, vapor: &[f32]) -> f32 {
    let (mut m, mut mx) = (0.0f64, 0.0f64);
    for (i, &v) in vapor.iter().enumerate() {
        m += f64::from(v);
        mx += f64::from(v) * (i as u32 % s.w) as f64;
    }
    (mx / m) as f32
}

fn total(v: &[f32]) -> f64 {
    v.iter().map(|&x| f64::from(x)).sum()
}

fn wind_scene() -> Scene {
    let mut s = Scene::new(256, 128, [30.0, 0.0]);
    s.puff(64, 64, 2);
    s
}

fn check_wind_and_conservation(s: &Scene, o: &Out, who: &str) {
    let t = 120.0 * DT;
    let moved = com_x(s, &o.vapor) - com_x(s, &s.vapor);
    let expect = 30.0 * t;
    println!("{who}: centre moved {moved:.3} cells, expected {expect}");
    assert!(
        (moved - expect).abs() <= 0.1 * expect,
        "{who}: moved {moved}"
    );
    let (a, b) = (total(&s.vapor), total(&o.vapor));
    let leak = ((a - b) / a).abs();
    println!("{who}: vapor {a} -> {b}, relative leak {leak:e}");
    assert!(leak < 1e-3, "{who}: leak {leak}");
}

fn wall_scene() -> Scene {
    let mut s = Scene::new(128, 128, [30.0, 0.0]);
    for y in 20..108 {
        let i = s.i(80, y);
        s.mat[i] = STONE;
    }
    // Hot gas left of the wall, vapor puff left of the wall.
    for y in 60..100 {
        for x in 40..60 {
            let i = s.i(x, y);
            s.temp[i] = 700.0;
        }
    }
    s.puff(70, 64, 3);
    s
}

fn check_solids_block(s: &Scene, o: &Out, who: &str) {
    let mut solids = 0;
    for i in 0..o.mat.len() {
        if is_solid(o.mat[i]) {
            solids += 1;
            assert_eq!(o.vel[i], [0.0, 0.0], "{who}: air moves inside solid {i}");
            assert_eq!(o.vapor[i], 0.0, "{who}: vapor inside solid {i}");
            assert_eq!(o.pressure[i], 0.0, "{who}: pressure inside solid {i}");
        }
    }
    assert!(solids > 0);
    // Nothing crossed the wall at x = 80 (rows 20..108; air may flow around its ends).
    let right: f32 = (20..108)
        .flat_map(|y| (81..s.w).map(move |x| (x, y)))
        .map(|(x, y)| o.vapor[s.i(x, y)])
        .sum();
    // Diffusion tails around the wall ends are ~1e-30; a real leak is orders larger.
    println!("{who}: vapor right of wall {right:e}");
    assert!(right < 1e-9, "{who}: vapor leaked through the wall");
}

#[test]
fn cpu_puff_follows_wind_and_conserves_vapor() {
    let s = wind_scene();
    check_wind_and_conservation(&s, &run_cpu(&s, 120), "cpu");
}

#[test]
fn cpu_solids_block_air() {
    let s = wall_scene();
    check_solids_block(&s, &run_cpu(&s, 200), "cpu");
}

#[test]
fn pressure_uses_a_fixed_sweep_count() {
    assert_eq!(AIR_SWEEPS, 20);
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let air = AirPass::new(&gpu);
    let mut counts = Vec::new();
    for random in [false, true] {
        let grid = GridState::new(&gpu, 64, 64).unwrap();
        let cells: Vec<CellRaw> = (0..64 * 64)
            .map(|i| CellRaw {
                temp: if random {
                    200.0 + (i * 7919 % 500) as f32
                } else {
                    293.0
                },
                pressure: if random { (i * 31 % 17) as f32 } else { 0.0 },
                vapor: if random { (i % 5) as f32 / 5.0 } else { 0.0 },
                ..Default::default()
            })
            .collect();
        grid.upload(&gpu, &cells);
        let before = air.dispatch_count();
        air.dispatch(&gpu, &grid, DT, [0.0, 0.0]);
        counts.push(air.dispatch_count() - before);
    }
    println!("dispatches per tick: {counts:?}");
    assert_eq!(counts[0], counts[1]);
    assert_eq!(counts[0], u64::from(AIR_SWEEPS) + 6);
}

#[test]
fn gpu_puff_follows_wind_and_conserves_vapor() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let s = wind_scene();
    check_wind_and_conservation(&s, &run_gpu(&gpu, &s, 120), "gpu");
}

#[test]
fn gpu_solids_block_air() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let s = wall_scene();
    check_solids_block(&s, &run_gpu(&gpu, &s, 200), "gpu");
}

#[test]
fn gpu_no_nan_after_10000_ticks() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let mut s = wall_scene();
    s.wind = [-20.0, 10.0];
    for y in 100..128 {
        for x in 0..128 {
            let i = s.i(x, y);
            s.temp[i] = 1500.0;
            s.vapor[i] = 1.0;
        }
    }
    let o = run_gpu(&gpu, &s, 10_000);
    assert!(
        o.vapor
            .iter()
            .all(|v| v.is_finite() && (-1e-3..=1.001).contains(v)),
        "bad vapor"
    );
    assert!(o.pressure.iter().all(|v| v.is_finite()), "bad pressure");
    assert!(
        o.vel.iter().all(|v| v[0].is_finite() && v[1].is_finite()),
        "bad velocity"
    );
    println!("10000 ticks: all finite");
}

#[test]
fn gpu_matches_reference() {
    let Some(gpu) = Gpu::new_headless() else {
        eprintln!("SKIPPED: no GPU adapter available");
        return;
    };
    let mut s = wall_scene();
    s.wind = [20.0, -5.0];
    let (c, g) = (run_cpu(&s, 60), run_gpu(&gpu, &s, 60));
    let diff = |a: &[f32], b: &[f32]| {
        let scale = a.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
        let d = a
            .iter()
            .zip(b)
            .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()));
        (d, scale)
    };
    let (dv, sv) = diff(&c.vapor, &g.vapor);
    let (dp, sp) = diff(&c.pressure, &g.pressure);
    let cv: Vec<f32> = c.vel.iter().flatten().copied().collect();
    let gv: Vec<f32> = g.vel.iter().flatten().copied().collect();
    let (du, su) = diff(&cv, &gv);
    println!(
        "parity max abs diff / scale: vapor {dv:e}/{sv}, pressure {dp:e}/{sp}, vel {du:e}/{su}"
    );
    assert!(dv <= 1e-4 * sv && dp <= 1e-3 * sp && du <= 1e-3 * su);
}
