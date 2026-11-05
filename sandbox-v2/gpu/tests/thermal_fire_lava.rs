//! Thermal, fire and lava tests. Every property runs on the CPU reference and
//! on the GPU (the GPU half prints SKIPPED without an adapter), plus a
//! reference/GPU parity test.

use sandbox_v2_gpu::materials::{Material, MATERIALS, WALL};
use sandbox_v2_gpu::reference_thermal::{
    edge_flux, heat_capacity, stability_number, ThermalReference, AMBIENT_K, LAVA_DELAY_S,
    LAVA_FREEZE_K,
};
use sandbox_v2_gpu::units::{ticks, DT};
use sandbox_v2_gpu::{CellRaw, FirePass, Gpu, GridState, LavaPass, ThermalPass};

const EMPTY: u32 = Material::Empty as u32;
const STONE: u32 = Material::Stone as u32;
const WOOD: u32 = Material::Wood as u32;
const WATER: u32 = Material::Water as u32;
const FIRE: u32 = Material::Fire as u32;
const ASH: u32 = Material::Ash as u32;
const LAVA: u32 = Material::Lava as u32;
const METAL: u32 = Material::Metal as u32;
const SEED: u32 = 0x5EED_0006;

/// A scene: grid size plus row-major cells.
#[derive(Clone)]
struct Scene {
    w: u32,
    h: u32,
    cells: Vec<CellRaw>,
}

impl Scene {
    fn new(w: u32, h: u32) -> Scene {
        let air = CellRaw {
            mat: EMPTY,
            temp: AMBIENT_K,
            ..Default::default()
        };
        Scene {
            w,
            h,
            cells: vec![air; (w * h) as usize],
        }
    }
    fn set(&mut self, x: u32, y: u32, mat: u32, temp: f32) {
        let i = (y * self.w + x) as usize;
        self.cells[i] = CellRaw {
            mat,
            temp,
            ..Default::default()
        };
    }
    fn at(&self, cells: &[CellRaw], x: u32, y: u32) -> CellRaw {
        cells[(y * self.w + x) as usize]
    }
}

/// Runs `ticks` full ticks (thermal, fire, lava) on the CPU reference, calling
/// `obs(tick_done, cells)` after each one.
fn run_cpu(s: &Scene, n: u32, mut obs: impl FnMut(u32, &[CellRaw])) -> Vec<CellRaw> {
    let mut r = ThermalReference::new(s.w, s.h);
    r.cells.clone_from(&s.cells);
    for t in 0..n {
        r.tick(t, SEED);
        obs(t + 1, &r.cells);
    }
    r.cells
}

/// Same on the GPU; `every` = readback period for `obs` (ticks).
fn run_gpu(
    gpu: &Gpu,
    s: &Scene,
    n: u32,
    every: u32,
    mut obs: impl FnMut(u32, &[CellRaw]),
) -> Vec<CellRaw> {
    let grid = GridState::new(gpu, s.w, s.h).unwrap();
    grid.upload(gpu, &s.cells);
    let (th, fi, la) = (
        ThermalPass::new(gpu),
        FirePass::new(gpu),
        LavaPass::new(gpu),
    );
    for t in 0..n {
        th.dispatch(gpu, &grid);
        fi.dispatch(gpu, &grid);
        la.dispatch(gpu, &grid, t, SEED);
        if (t + 1) % every == 0 {
            obs(t + 1, &grid.readback(gpu));
        }
    }
    grid.readback(gpu)
}

fn gpu_or_skip() -> Option<Gpu> {
    let g = Gpu::new_headless();
    if g.is_none() {
        println!("SKIPPED: no GPU adapter");
    }
    g
}

// ---------------------------------------------------------------- lava

/// 3x3 lava blob emitted at 1500 K above an 8-deep stone floor, in air.
fn lava_scene() -> Scene {
    let mut s = Scene::new(64, 40);
    for y in 32..40 {
        for x in 0..64 {
            s.set(x, y, STONE, AMBIENT_K);
        }
    }
    for y in 20..23 {
        for x in 31..34 {
            s.set(x, y, LAVA, 1500.0);
        }
    }
    s
}

/// Tracks (hottest lava temperature, lava count) and the hold time: the
/// first tick at which no lava cell is above 700 K.
struct LavaWatch {
    hold: Option<u32>,
    half: Option<u32>,
    landed: bool,
    hot_at_20: Option<usize>,
}

impl LavaWatch {
    fn new() -> LavaWatch {
        LavaWatch {
            hold: None,
            half: None,
            landed: false,
            hot_at_20: None,
        }
    }
    fn see(&mut self, t: u32, cells: &[CellRaw]) {
        let lava: Vec<&CellRaw> = cells.iter().filter(|c| c.mat == LAVA).collect();
        let hot = lava.iter().any(|c| c.temp > LAVA_FREEZE_K);
        if t == ticks(20.0) {
            self.hot_at_20 = Some(lava.iter().filter(|c| c.temp > LAVA_FREEZE_K).count());
        }
        if !hot && self.hold.is_none() {
            self.hold = Some(t);
        }
        if lava.len() < 5 && self.half.is_none() {
            self.half = Some(t);
        }
        // Lava resting on the floor (row 31) at some point.
        self.landed |= cells[(31 * 64)..(32 * 64)].iter().any(|c| c.mat == LAVA);
    }
    fn check(&self, who: &str, run_s: f32) {
        let fmt = |v: Option<u32>| match v {
            Some(t) => format!("{:.2} s", t as f32 * DT),
            None => format!("> {run_s} s"),
        };
        println!(
            "{who}: lava hold above 700 K {}, half the blob frozen {}, cells above 700 K at 20 s: {:?} of 9",
            fmt(self.hold),
            fmt(self.half),
            self.hot_at_20
        );
        assert!(self.landed, "{who}: lava never reached the stone floor");
        let need = ticks(20.0);
        assert!(
            self.hold.is_none_or(|t| t >= need),
            "{who}: lava froze below 700 K after {} s",
            fmt(self.hold)
        );
        let hot = self.hot_at_20.expect("run shorter than 20 s");
        assert!(
            hot >= 5,
            "{who}: only {hot} of 9 lava cells above 700 K at 20 s"
        );
    }
}

#[test]
fn cpu_lava_on_stone_stays_hot_for_20_s() {
    let s = lava_scene();
    let mut w = LavaWatch::new();
    let out = run_cpu(&s, ticks(60.0), |t, c| w.see(t, c));
    w.check("cpu", 60.0);
    let lava: Vec<usize> = (0..out.len()).filter(|&i| out[i].mat == LAVA).collect();
    let rock = out
        .iter()
        .filter(|c| c.mat == Material::Rock as u32)
        .count();
    let hottest = lava.iter().map(|&i| out[i].temp).fold(0.0, f32::max);
    let xs = lava.iter().map(|&i| i % 64);
    let (x0, x1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0));
    let floor = out[33 * 64 + 32].temp;
    println!(
        "cpu at 60 s: {} lava, {rock} rock, hottest lava {hottest:.0} K, lava spans x {x0}..={x1}, stone 1 cell under the blob {floor:.0} K",
        lava.len()
    );
}

#[test]
fn gpu_lava_on_stone_stays_hot_for_20_s() {
    let Some(gpu) = gpu_or_skip() else { return };
    let s = lava_scene();
    let mut w = LavaWatch::new();
    run_gpu(&gpu, &s, ticks(25.0), 1, |t, c| w.see(t, c));
    w.check("gpu", 25.0);
}

/// Informational: a single 1 cm lava cell on the same floor (not asserted:
/// a 1 cm drop on cold granite physically crusts in seconds).
#[test]
fn cpu_single_lava_cell_hold_time() {
    let mut s = lava_scene();
    for y in 20..23 {
        for x in 31..34 {
            s.set(x, y, EMPTY, AMBIENT_K);
        }
    }
    s.set(32, 31, LAVA, 1500.0);
    let mut hold = None;
    run_cpu(&s, ticks(40.0), |t, c| {
        if hold.is_none() && !c.iter().any(|c| c.mat == LAVA && c.temp > LAVA_FREEZE_K) {
            hold = Some(t);
        }
    });
    println!(
        "single lava cell on stone: hold {:?} s",
        hold.map(|t| t as f32 * DT)
    );
}

/// Lava on a 45-degree stone slope slides one cell diagonally per viscosity
/// delay: consecutive moves are exactly `ticks(LAVA_DELAY_S)` apart.
fn slope_scene() -> Scene {
    let mut s = Scene::new(32, 32);
    for y in 0..32u32 {
        for x in 0..32u32 {
            if y >= x + 5 {
                s.set(x, y, STONE, AMBIENT_K);
            }
        }
    }
    s.set(2, 6, LAVA, 1500.0);
    s
}

fn check_slide(who: &str, run: impl FnOnce(&mut dyn FnMut(u32, &[CellRaw]))) {
    let mut moves = Vec::new();
    let mut last = None;
    run(&mut |t, c| {
        let p = c.iter().position(|c| c.mat == LAVA);
        if p != last {
            moves.push(t);
            last = p;
        }
    });
    let gaps: Vec<u32> = moves.windows(2).map(|w| w[1] - w[0]).collect();
    let d = ticks(LAVA_DELAY_S);
    println!("{who}: lava slide moves at ticks {moves:?}; gaps {gaps:?} (delay {d} ticks = {LAVA_DELAY_S} s)");
    assert!(gaps.len() >= 8, "{who}: lava did not keep sliding");
    assert!(gaps.iter().all(|&g| g == d), "{who}: gaps {gaps:?}");
}

#[test]
fn cpu_lava_slides_once_per_viscosity_delay() {
    let s = slope_scene();
    check_slide("cpu", |o| {
        run_cpu(&s, 10 * ticks(LAVA_DELAY_S), o);
    });
}

#[test]
fn gpu_lava_slides_once_per_viscosity_delay() {
    let Some(gpu) = gpu_or_skip() else { return };
    let s = slope_scene();
    check_slide("gpu", |o| {
        run_gpu(&gpu, &s, 10 * ticks(LAVA_DELAY_S), 1, o);
    });
}

// ---------------------------------------------------------------- fire

/// One wood cell at 600 K (above ignition) on a stone floor, in air.
fn burn_scene() -> Scene {
    let mut s = Scene::new(16, 16);
    for x in 0..16 {
        s.set(x, 12, STONE, AMBIENT_K);
    }
    s.set(8, 11, WOOD, 600.0);
    s
}

fn check_burn(s: &Scene, who: &str, run: impl FnOnce(&mut dyn FnMut(u32, &[CellRaw]))) {
    let (mut lit, mut out) = (None, None);
    run(&mut |t, c| {
        let m = s.at(c, 8, 11).mat;
        if m == FIRE && lit.is_none() {
            lit = Some(t);
        }
        if m == ASH && out.is_none() {
            out = Some(t);
        }
    });
    let lit = lit.expect("wood never ignited");
    let out = out.expect("fire never produced ash");
    let secs = (out - lit) as f32 * DT;
    let expect = MATERIALS[Material::Wood as usize].burn_s;
    println!(
        "{who}: wood lit at tick {lit}, ash at tick {out}: burned {secs:.3} s (burn_s {expect})"
    );
    assert!((secs - expect).abs() <= 0.5, "{who}: burned {secs} s");
}

#[test]
fn cpu_wood_burns_for_burn_s_then_ash() {
    let s = burn_scene();
    check_burn(&s, "cpu", |obs| {
        run_cpu(&s, ticks(8.0), obs);
    });
}

#[test]
fn gpu_wood_burns_for_burn_s_then_ash() {
    let Some(gpu) = gpu_or_skip() else { return };
    let s = burn_scene();
    check_burn(&s, "gpu", |obs| {
        run_gpu(&gpu, &s, ticks(8.0), 1, obs);
    });
}

/// Wood at (8, 10) in a stone cup, a long-lived flame (12 s of fuel) right
/// above it; `side` fills (7, 10) and (9, 10).
fn torch_scene(side: u32) -> Scene {
    let mut s = Scene::new(16, 16);
    for x in 0..16 {
        s.set(x, 11, STONE, AMBIENT_K);
    }
    s.set(6, 10, STONE, AMBIENT_K);
    s.set(10, 10, STONE, AMBIENT_K);
    s.set(7, 10, side, AMBIENT_K);
    s.set(9, 10, side, AMBIENT_K);
    s.set(8, 10, WOOD, AMBIENT_K);
    s.set(8, 9, FIRE, 1200.0);
    s.cells[(9 * 16 + 8) as usize].fuel = ticks(12.0) as f32;
    s
}

/// First tick the wood cell at (8, 10) is no longer wood.
fn ignition(s: &Scene, run: impl FnOnce(&mut dyn FnMut(u32, &[CellRaw]))) -> Option<u32> {
    let mut lit = None;
    run(&mut |t, c| {
        if lit.is_none() && s.at(c, 8, 10).mat != WOOD {
            lit = Some(t);
        }
    });
    lit
}

/// Runs a scene, calling the observer after each tick.
type Runner<'a> = &'a dyn Fn(&Scene, &mut dyn FnMut(u32, &[CellRaw]));

fn check_wet_and_dry(who: &str, run: Runner) {
    let dry = torch_scene(EMPTY);
    let dry_lit = ignition(&dry, |o| run(&dry, o));
    assert!(
        dry_lit.is_some_and(|t| t <= ticks(10.0)),
        "{who}: dry wood did not ignite in 10 s"
    );

    // Wet: the cap moves the wood's excess heat into the two water cells, which
    // boil (held at 373.15 K for ticks(evap_s)) and turn to steam. The wood
    // must stay at or below 373.15 K while ANY water touches it.
    let wet = torch_scene(WATER);
    let (mut boil, mut gone) = (None, None);
    let mut capped = true;
    let lit = ignition(&wet, |o| {
        run(&wet, &mut |t, c| {
            let (l, r) = (wet.at(c, 7, 10), wet.at(c, 9, 10));
            let touching = l.mat == WATER || r.mat == WATER;
            if touching {
                capped &= wet.at(c, 8, 10).temp <= 373.15 && wet.at(c, 8, 10).mat == WOOD;
            }
            if boil.is_none() && (l.fuel > 0.0 || r.fuel > 0.0) {
                boil = Some(t);
            }
            if gone.is_none() && !touching {
                gone = Some(t);
            }
            o(t, c);
        })
    });
    let s_of = |v: Option<u32>| v.map(|t| t as f32 * DT);
    println!(
        "{who}: wet wood: water starts boiling {:?} s, last water gone {:?} s, wood ignites {:?} s (dry {:?} s)",
        s_of(boil),
        s_of(gone),
        s_of(lit),
        s_of(dry_lit)
    );
    assert!(
        capped,
        "{who}: wet wood exceeded 373.15 K or ignited while touching water"
    );
    let (lit, gone) = (
        lit.expect("wet wood never ignited"),
        gone.expect("water never left"),
    );
    assert!(lit > gone, "{who}: ignited before the water was gone");
    assert!(
        lit >= 3 * dry_lit.unwrap(),
        "{who}: wetness barely delayed ignition"
    );
}

#[test]
fn cpu_wet_wood_cannot_ignite_while_water_touches_it() {
    check_wet_and_dry("cpu", &|s, o| {
        run_cpu(s, ticks(15.0), o);
    });
}

#[test]
fn gpu_wet_wood_cannot_ignite_while_water_touches_it() {
    let Some(gpu) = gpu_or_skip() else { return };
    check_wet_and_dry("gpu", &|s, o| {
        run_gpu(&gpu, s, ticks(15.0), 1, o);
    });
}

// ---------------------------------------------------------------- conduction

#[test]
fn edge_flux_is_antisymmetric_and_explicit_step_is_stable() {
    let temps = [0.0f32, 1.5, 293.0, 700.25, 1500.0, 9999.0];
    for a in 0..=WALL {
        for b in 0..=WALL {
            for &ta in &temps {
                for &tb in &temps {
                    let ab = edge_flux(a, ta, b, tb);
                    let ba = edge_flux(b, tb, a, ta);
                    assert_eq!(ab, -ba, "mats {a},{b} temps {ta},{tb}");
                }
            }
        }
    }
    let s = stability_number();
    println!("explicit stability number (worst cell, 4 edges) = {s:.4} (must be <= 0.5)");
    assert!(s <= 0.5);
}

/// Random temperatures on materials with no phase change in range.
fn conduction_scene() -> Scene {
    let mats = [
        EMPTY,
        STONE,
        METAL,
        Material::Glass as u32,
        Material::Rock as u32,
        WALL,
    ];
    let mut s = Scene::new(64, 64);
    let mut h = 0x1234_5678u32;
    for i in 0..s.cells.len() {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        let mat = mats[(h % 6) as usize];
        let temp = 250.0 + (h >> 8) as f32 / (1u32 << 24) as f32 * 1000.0;
        s.cells[i] = CellRaw {
            mat,
            temp,
            ..Default::default()
        };
    }
    s
}

fn energy(c: &[CellRaw]) -> f64 {
    c.iter()
        .map(|c| f64::from(heat_capacity(c.mat)) * f64::from(c.temp))
        .sum()
}

/// Wet wood (380..500 K, above the 373 K moisture cap) mixed with water
/// (280..320 K), stone and air: exercises the cap's heat transfer into water.
fn wet_scene() -> Scene {
    let mut s = Scene::new(32, 32);
    let mut h = 0x0BAD_5EEDu32;
    for i in 0..s.cells.len() {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        let u = (h >> 8) as f32 / (1u32 << 24) as f32;
        let (mat, temp) = match h % 4 {
            0 => (WATER, 280.0 + 40.0 * u),
            1 => (WOOD, 380.0 + 120.0 * u),
            2 => (STONE, 280.0 + 100.0 * u),
            _ => (EMPTY, 293.0),
        };
        s.cells[i] = CellRaw {
            mat,
            temp,
            ..Default::default()
        };
    }
    s
}

fn wet_wood_cells(s: &Scene) -> usize {
    (0..s.cells.len())
        .filter(|&i| {
            let (x, y) = (i as u32 % s.w, i as u32 / s.w);
            s.cells[i].mat == WOOD
                && [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|&(dx, dy)| {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        nx >= 0
                            && ny >= 0
                            && nx < s.w as i32
                            && ny < s.h as i32
                            && s.at(&s.cells, nx as u32, ny as u32).mat == WATER
                    })
        })
        .count()
}

#[test]
fn cpu_moisture_cap_conserves_heat() {
    let s = wet_scene();
    println!("wet scene: {} wood cells touch water", wet_wood_cells(&s));
    let out = run_cpu(&s, 300, |_, _| {});
    check_conservation(&s, &out, "cpu wet", false);
}

#[test]
fn gpu_moisture_cap_conserves_heat() {
    let Some(gpu) = gpu_or_skip() else { return };
    let s = wet_scene();
    let out = run_gpu(&gpu, &s, 300, 300, |_, _| {});
    check_conservation(&s, &out, "gpu wet", false);
}

fn check_conservation(s: &Scene, out: &[CellRaw], who: &str, max_principle: bool) {
    let (e0, e1) = (energy(&s.cells), energy(out));
    let rel = ((e1 - e0) / e0).abs();
    let lo = s.cells.iter().map(|c| c.temp).fold(f32::MAX, f32::min);
    let hi = s.cells.iter().map(|c| c.temp).fold(f32::MIN, f32::max);
    let (olo, ohi) = out.iter().fold((f32::MAX, f32::MIN), |(a, b), c| {
        (a.min(c.temp), b.max(c.temp))
    });
    println!("{who}: heat {e0:.3} -> {e1:.3}, relative change {rel:e}; temp range [{lo}, {hi}] -> [{olo}, {ohi}]");
    assert!(rel < 1e-7, "{who}: heat not conserved ({rel})");
    assert!(
        out.iter().zip(&s.cells).all(|(a, b)| a.mat == b.mat),
        "{who}: phase change"
    );
    if max_principle {
        assert!(olo >= lo - 1e-3 && ohi <= hi + 1e-3, "{who}: new extremum");
    }
}

#[test]
fn cpu_conduction_conserves_heat() {
    let s = conduction_scene();
    let out = run_cpu(&s, 600, |_, _| {});
    check_conservation(&s, &out, "cpu", true);
}

#[test]
fn gpu_conduction_conserves_heat() {
    let Some(gpu) = gpu_or_skip() else { return };
    let s = conduction_scene();
    let out = run_gpu(&gpu, &s, 600, 600, |_, _| {});
    check_conservation(&s, &out, "gpu", true);
}

#[test]
fn gpu_nan_temperature_is_reset_and_counted() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut s = Scene::new(8, 8);
    s.cells[9].temp = f32::from_bits(0x7FC0_0000);
    let grid = GridState::new(&gpu, 8, 8).unwrap();
    grid.upload(&gpu, &s.cells);
    let th = ThermalPass::new(&gpu);
    th.dispatch(&gpu, &grid);
    let out = grid.readback(&gpu);
    assert!(out.iter().all(|c| c.temp.is_finite()));
    assert_eq!(th.sanitized_count(&gpu), 1);
    let mut r = ThermalReference::new(8, 8);
    r.cells.clone_from(&s.cells);
    r.thermal();
    assert_eq!(r.sanitized, 1);
}

// ---------------------------------------------------------------- parity

/// Mixed 64x64 scene: stone basin, lava blob, burning wood plank, water with
/// ice, hot wet wood in the water, hot metal bar, sand.
fn parity_scene() -> Scene {
    let mut s = Scene::new(64, 64);
    for x in 0..64 {
        for y in 56..64 {
            s.set(x, y, STONE, AMBIENT_K);
        }
    }
    for y in 40..56 {
        s.set(0, y, STONE, AMBIENT_K);
        s.set(63, y, STONE, AMBIENT_K);
        s.set(30, y, STONE, AMBIENT_K);
    }
    for y in 46..56 {
        for x in 31..63 {
            s.set(x, y, WATER, 300.0);
        }
    }
    for x in 40..46 {
        s.set(x, 45, Material::Ice as u32, 250.0);
    }
    // Hot wet wood in the pool: exercises the moisture heat transfer.
    for x in 34..40 {
        s.set(x, 50, WOOD, 520.0);
    }
    for y in 30..36 {
        for x in 8..14 {
            s.set(x, y, LAVA, 1500.0);
        }
    }
    for x in 2..28 {
        s.set(x, 50, WOOD, AMBIENT_K);
    }
    s.set(2, 49, FIRE, 1200.0);
    for x in 32..60 {
        s.set(x, 20, METAL, 900.0);
    }
    for x in 16..24 {
        s.set(x, 10, Material::Sand as u32, 2100.0);
    }
    s
}

#[test]
fn gpu_matches_reference() {
    let Some(gpu) = gpu_or_skip() else { return };
    let s = parity_scene();
    let n = 300;
    let cpu = run_cpu(&s, n, |_, _| {});
    let got = run_gpu(&gpu, &s, n, n, |_, _| {});
    let mat_diff = cpu.iter().zip(&got).filter(|(a, b)| a.mat != b.mat).count();
    let fuel_diff = cpu
        .iter()
        .zip(&got)
        .filter(|(a, b)| a.fuel != b.fuel)
        .count();
    let max_t = cpu.iter().map(|c| c.temp.abs()).fold(0.0, f32::max);
    let t_diff = cpu
        .iter()
        .zip(&got)
        .map(|(a, b)| (a.temp - b.temp).abs())
        .fold(0.0, f32::max);
    let rest_diff = cpu
        .iter()
        .zip(&got)
        .filter(|(a, b)| {
            (a.vx, a.vy, a.vapor, a.pressure, a.flags) != (b.vx, b.vy, b.vapor, b.pressure, b.flags)
        })
        .count();
    let count = |c: &[CellRaw], m: u32| c.iter().filter(|c| c.mat == m).count();
    println!(
        "parity after {n} ticks: material mismatches {mat_diff}, timer mismatches {fuel_diff}, \
         other-field mismatches {rest_diff}, max |dT| {t_diff:e} K (field max {max_t} K, ratio {:e}); \
         cpu lava {} fire {} ash {} glass {}",
        t_diff / max_t,
        count(&cpu, LAVA),
        count(&cpu, FIRE),
        count(&cpu, ASH),
        count(&cpu, Material::Glass as u32),
    );
    assert_eq!(mat_diff, 0);
    assert_eq!(fuel_diff, 0);
    assert_eq!(rest_diff, 0);
    assert!(t_diff <= 1e-4 * max_t, "temperature diff {t_diff}");
}
