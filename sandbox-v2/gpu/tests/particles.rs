//! Particle system tests: ring-buffer cap, projectile range, collision and
//! GPU/reference parity. CPU halves always run; GPU halves print SKIPPED
//! without an adapter.

use sandbox_v2_gpu::materials::{Material, WALL};
use sandbox_v2_gpu::reference_particles::{
    is_solid, ParticleReference, Ring, FP_ONE, FRICTION_MU, MAX_SPEED_CPS, REST_VY_CPS,
};
use sandbox_v2_gpu::units::{DT, GRAVITY_CELLS};
use sandbox_v2_gpu::{CellRaw, Gpu, GridState, ParticleRaw, Particles};

const EMPTY: u32 = Material::Empty as u32;
const STONE: u32 = Material::Stone as u32;
const WOOD: u32 = Material::Wood as u32;
const WATER: u32 = Material::Water as u32;
const METAL: u32 = Material::Metal as u32;

/// Deterministic xorshift for test scenes.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform in [lo, hi).
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

fn empty_cells(w: u32, h: u32) -> Vec<CellRaw> {
    vec![CellRaw::default(); (w * h) as usize]
}

fn mat_at(cells: &[CellRaw], w: u32, h: u32, x: f32, y: f32) -> u32 {
    let (cx, cy) = (x.floor() as i64, y.floor() as i64);
    if cx < 0 || cy < 0 || cx >= i64::from(w) || cy >= i64::from(h) {
        return WALL;
    }
    cells[(cy as u32 * w + cx as u32) as usize].mat
}

/// 128x128 obstacle course: border of stone, scattered solid blocks of every
/// solid kind, a one-cell metal wall, and non-solid water pockets.
fn course() -> (u32, u32, Vec<CellRaw>) {
    let (w, h) = (128u32, 128u32);
    let mut cells = empty_cells(w, h);
    let mut set = |x: u32, y: u32, m: u32| cells[(y * w + x) as usize].mat = m;
    for i in 0..w {
        set(i, h - 1, STONE);
        set(i, 0, STONE);
        set(0, i, STONE);
        set(w - 1, i, STONE);
    }
    for y in 20..110 {
        set(64, y, METAL); // one cell thick
    }
    let solids = [
        Material::Stone,
        Material::Rock,
        Material::Wood,
        Material::Ash,
        Material::Metal,
        Material::Glass,
        Material::Ice,
    ];
    let mut rng = Rng(0xB10C_0007);
    for k in 0..60 {
        let (bx, by) = (rng.next() % 120 + 2, rng.next() % 120 + 2);
        let m = solids[k % solids.len()] as u32;
        for y in by..(by + 1 + rng.next() % 5).min(h - 1) {
            for x in bx..(bx + 1 + rng.next() % 5).min(w - 1) {
                set(x, y, m);
            }
        }
    }
    for y in 100..110 {
        for x in 10..30 {
            set(x, y, WATER);
        }
    }
    (w, h, cells)
}

/// `n` particles at random free positions with random velocities up to the
/// speed cap; some short-lived, one NaN and one spawned inside a solid.
fn swarm(cells: &[CellRaw], w: u32, h: u32, n: usize, seed: u32) -> Vec<ParticleRaw> {
    let mut rng = Rng(seed);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let (x, y) = (rng.f(0.0, w as f32), rng.f(0.0, h as f32));
        if is_solid(mat_at(cells, w, h, x, y)) {
            continue;
        }
        let s = MAX_SPEED_CPS;
        let i = out.len();
        out.push(ParticleRaw {
            pos: [x, y],
            vel: [rng.f(-s, s), rng.f(-s, s)],
            mat: WATER,
            temp: i as f32,
            life: if i % 7 == 0 { rng.f(0.0, 2.0) } else { 100.0 },
            _pad: 0.0,
        });
    }
    out[1].vel[0] = f32::NAN;
    out[2].pos = [1.5, 0.5]; // inside the stone border
    out
}

fn tagged(n: usize, offset: usize) -> Vec<ParticleRaw> {
    (0..n)
        .map(|i| ParticleRaw {
            pos: [4.5, 4.5],
            temp: (i + offset) as f32,
            life: 10.0,
            ..Default::default()
        })
        .collect()
}

fn gpu_or_skip(name: &str) -> Option<Gpu> {
    let g = Gpu::new_headless();
    if g.is_none() {
        eprintln!("SKIPPED {name}: no GPU adapter available");
    }
    g
}

fn grid_with(gpu: &Gpu, w: u32, h: u32, cells: &[CellRaw]) -> GridState {
    let grid = GridState::new(gpu, w, h).expect("grid");
    grid.upload(gpu, cells);
    grid
}

fn assert_same(a: &[ParticleRaw], b: &[ParticleRaw]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .filter(|(p, q)| bytemuck::bytes_of(*p) != bytemuck::bytes_of(*q))
        .count()
}

#[test]
fn layout_is_32_bytes() {
    assert_eq!(std::mem::size_of::<ParticleRaw>(), 32);
    assert_eq!(Particles::MAX, 1_000_000);
}

// (b) Ring buffer: beyond cap the oldest slot is overwritten, count == cap.
#[test]
fn ring_recycles_oldest_cpu() {
    let mut r = ParticleReference::new(1000);
    r.spawn(&tagged(1000, 0));
    r.spawn(&tagged(250, 1000));
    assert_eq!(r.count(), 1000);
    let p = r.particles();
    for (slot, q) in p.iter().enumerate() {
        let want = if slot < 250 { slot + 1000 } else { slot };
        assert_eq!(q.temp, want as f32, "slot {slot}");
    }
    assert_eq!(r.ring().head(), 250, "next overwrite = oldest live slot");
    // One batch larger than cap keeps only its newest `cap` entries.
    r.spawn(&tagged(2500, 5000));
    assert_eq!(r.count(), 1000);
    let mut temps: Vec<f32> = r.particles().iter().map(|q| q.temp).collect();
    temps.sort_by(f32::total_cmp);
    assert_eq!(temps.first().copied(), Some(6500.0));
    assert_eq!(temps.last().copied(), Some(7499.0));

    let mut ring = Ring::new(Particles::MAX);
    ring.push(Particles::MAX as usize + 7);
    assert_eq!(ring.count(), Particles::MAX);
}

#[test]
fn ring_recycles_oldest_gpu() {
    let Some(gpu) = gpu_or_skip("ring_recycles_oldest_gpu") else {
        return;
    };
    let mut parts = Particles::new(&gpu, 1000);
    parts.spawn(&gpu, &tagged(1000, 0));
    parts.spawn(&gpu, &tagged(250, 1000));
    assert_eq!(parts.count(), 1000);
    let p = parts.readback(&gpu);
    for (slot, q) in p.iter().enumerate() {
        let want = if slot < 250 { slot + 1000 } else { slot };
        assert_eq!(q.temp, want as f32, "slot {slot}");
    }
    parts.spawn(&gpu, &tagged(2500, 5000));
    assert_eq!(parts.count(), 1000);
    let mut r = ParticleReference::new(1000);
    r.spawn(&tagged(1000, 0));
    r.spawn(&tagged(250, 1000));
    r.spawn(&tagged(2500, 5000));
    assert_eq!(assert_same(&parts.readback(&gpu), r.particles()), 0);
}

// (a) One million particles spawn and step; then the cap holds.
#[test]
fn million_particles_step_gpu() {
    let Some(gpu) = gpu_or_skip("million_particles_step_gpu") else {
        return;
    };
    let (w, h) = (1024u32, 1024u32);
    let mut cells = empty_cells(w, h);
    for x in 0..w {
        cells[((h - 1) * w + x) as usize].mat = STONE;
    }
    let grid = grid_with(&gpu, w, h, &cells);
    let mut rng = Rng(0x0001_0000);
    let batch: Vec<ParticleRaw> = (0..Particles::MAX)
        .map(|i| ParticleRaw {
            pos: [rng.f(0.0, w as f32), rng.f(0.0, (h - 1) as f32)],
            vel: [rng.f(-300.0, 300.0), rng.f(-300.0, 300.0)],
            mat: WATER,
            temp: i as f32,
            life: 5.0,
            _pad: 0.0,
        })
        .collect();
    let mut parts = Particles::new(&gpu, Particles::MAX);
    parts.spawn(&gpu, &batch);
    assert_eq!(parts.count(), Particles::MAX);
    let t0 = std::time::Instant::now();
    parts.step(&gpu, &grid, DT);
    let got = parts.readback(&gpu);
    eprintln!(
        "1M step + 32 MB readback: {:.1} ms",
        t0.elapsed().as_secs_f64() * 1e3
    );
    let mut r = ParticleReference::new(Particles::MAX);
    r.spawn(&batch);
    r.step(&cells, w, h, DT);
    let bad = assert_same(&got, r.particles());
    eprintln!("1M parity after one step: {bad} mismatches");
    assert_eq!(bad, 0);
    assert!(got.iter().all(|p| p.life == 5.0 - DT));

    parts.spawn(&gpu, &tagged(1, 0xABCD));
    assert_eq!(parts.count(), Particles::MAX);
    assert_eq!(parts.readback(&gpu)[0].temp, 0xABCD as f32);
}

/// Steps one launched particle over a stone floor until it bounces; returns
/// the landing x minus the launch x.
fn range_cpu(p: ParticleRaw) -> f32 {
    let (w, h) = (256u32, 128u32);
    let mut cells = empty_cells(w, h);
    for y in 101..h {
        for x in 0..w {
            cells[(y * w + x) as usize].mat = STONE;
        }
    }
    let mut r = ParticleReference::new(1);
    r.spawn(&[p]);
    for _ in 0..200 {
        let before = r.particles()[0].vel[1];
        r.step(&cells, w, h, DT);
        let now = r.particles()[0];
        if before > 0.0 && now.vel[1] < 0.0 {
            return now.pos[0] - p.pos[0];
        }
    }
    panic!("never landed");
}

// (c) 45 degrees at 300 cells/s (3 m/s): range v^2/g = 91.74 cells.
#[test]
fn projectile_range_matches_v2_over_g() {
    let v = 300.0f32;
    let c = v * std::f32::consts::FRAC_1_SQRT_2;
    let ideal = v * v / GRAVITY_CELLS;
    let p = ParticleRaw::launched([20.0, 100.999], [c, -c], EMPTY, 0.0, 10.0);
    let range = range_cpu(p);
    let err = (range - ideal).abs() / ideal;
    eprintln!(
        "range {range:.4} cells, ideal {ideal:.4}, error {:.4}%",
        err * 100.0
    );
    assert!(err < 0.01, "range error {:.3}%", err * 100.0);

    // Without the half-kick start, semi-implicit Euler flies a parabola
    // launched at vy - g*DT/2: range short by g*DT/(sqrt(2) v) = 3.85%.
    let naive = ParticleRaw {
        pos: [20.0, 100.999],
        vel: [c, -c],
        life: 10.0,
        ..Default::default()
    };
    let nerr = (ideal - range_cpu(naive)) / ideal;
    let bias = GRAVITY_CELLS * DT / (std::f32::consts::SQRT_2 * v);
    eprintln!(
        "naive start error {:.3}% (predicted bias {:.3}%)",
        nerr * 100.0,
        bias * 100.0
    );
    assert!((nerr - bias).abs() < 0.005);
}

#[test]
fn reflects_with_restitution_and_never_tunnels() {
    let (w, h) = (64u32, 8u32);
    let mut cells = empty_cells(w, h);
    for y in 0..h {
        cells[(y * w + 32) as usize].mat = METAL; // one-cell wall at x = 32
    }
    let mut r = ParticleReference::new(2);
    r.spawn(&[
        ParticleRaw {
            pos: [30.5, 4.0],
            vel: [600.0, 0.0],
            life: 10.0,
            ..Default::default()
        },
        ParticleRaw {
            pos: [31.9, 4.0],
            vel: [MAX_SPEED_CPS, 0.0],
            life: 10.0,
            ..Default::default()
        },
    ]);
    r.step(&cells, w, h, DT);
    let [a, b] = [r.particles()[0], r.particles()[1]];
    assert_eq!(a.vel[0], -360.0, "600 cells/s * -0.6");
    assert_eq!(b.vel[0], -0.6 * MAX_SPEED_CPS);
    for p in [a, b] {
        assert!(
            p.pos[0] < 32.0 && p.pos[0] >= 31.0,
            "stops in previous cell"
        );
    }
    // A max-speed particle (30 cells/tick) never crosses the one-cell wall.
    let mut r = ParticleReference::new(1);
    r.spawn(&[ParticleRaw {
        pos: [2.5, 4.0],
        vel: [MAX_SPEED_CPS, 0.0],
        life: 10.0,
        ..Default::default()
    }]);
    for _ in 0..120 {
        r.step(&cells, w, h, DT);
        assert!(r.particles()[0].pos[0] < 32.0);
    }
}

fn check_no_solid(parts: &[ParticleRaw], cells: &[CellRaw], w: u32, h: u32) -> usize {
    parts
        .iter()
        .filter(|p| p.life > 0.0 && is_solid(mat_at(cells, w, h, p.pos[0], p.pos[1])))
        .count()
}

// (d) No live particle ends a step inside a solid cell (or outside the grid).
#[test]
fn never_ends_in_solid_cpu() {
    let (w, h, cells) = course();
    let mut r = ParticleReference::new(20_000);
    r.spawn(&swarm(&cells, w, h, 20_000, 0xD00D));
    let mut hits = 0;
    for _ in 0..300 {
        r.step(&cells, w, h, DT);
        hits += check_no_solid(r.particles(), &cells, w, h);
    }
    let live = r.particles().iter().filter(|p| p.life > 0.0).count();
    eprintln!("CPU: {live} live after 300 ticks, {hits} in-solid endings");
    assert_eq!(hits, 0);
    assert!(live > 10_000);
    assert_eq!(r.particles()[2].life, 0.0, "spawned inside a solid: killed");
}

#[test]
fn never_ends_in_solid_gpu() {
    let Some(gpu) = gpu_or_skip("never_ends_in_solid_gpu") else {
        return;
    };
    let (w, h, cells) = course();
    let grid = grid_with(&gpu, w, h, &cells);
    let mut parts = Particles::new(&gpu, 20_000);
    parts.spawn(&gpu, &swarm(&cells, w, h, 20_000, 0xD00D));
    let mut hits = 0;
    for t in 0..300 {
        parts.step(&gpu, &grid, DT);
        if t % 10 == 9 {
            hits += check_no_solid(&parts.readback(&gpu), &cells, w, h);
        }
    }
    eprintln!("GPU: {hits} in-solid endings (checked every 10 ticks)");
    assert_eq!(hits, 0);
}

// (e) GPU matches the reference bit for bit over 300 ticks, including a
// mid-run spawn that wraps the ring, NaN sanitizing and life expiry.
#[test]
fn parity_300_ticks() {
    let Some(gpu) = gpu_or_skip("parity_300_ticks") else {
        return;
    };
    let (w, h, mut cells) = course();
    cells[(50 * w + 50) as usize].mat = WOOD;
    let grid = grid_with(&gpu, w, h, &cells);
    let first = swarm(&cells, w, h, 30_000, 0xFACE);
    let second = swarm(&cells, w, h, 20_000, 0xBEEF);
    let mut parts = Particles::new(&gpu, 40_000);
    let mut r = ParticleReference::new(40_000);
    parts.spawn(&gpu, &first);
    r.spawn(&first);
    let mut worst = 0;
    for t in 0..300 {
        if t == 100 {
            parts.spawn(&gpu, &second);
            r.spawn(&second);
        }
        parts.step(&gpu, &grid, DT);
        r.step(&cells, w, h, DT);
        if t % 50 == 49 {
            worst = worst.max(assert_same(&parts.readback(&gpu), r.particles()));
        }
    }
    let dead = r.particles().iter().filter(|p| p.life <= 0.0).count();
    eprintln!(
        "parity: worst {worst} mismatches of {}; {dead} dead; sanitized gpu {} ref {}",
        r.count(),
        parts.sanitized_count(&gpu),
        r.sanitized()
    );
    assert_eq!(worst, 0);
    assert_eq!(parts.sanitized_count(&gpu), r.sanitized());
    assert_eq!(r.sanitized(), 2, "one NaN per swarm");
    assert_eq!(FP_ONE, 1024);
}

/// A 256x16 strip with a stone floor at row 10.
fn floor_strip() -> (u32, u32, Vec<CellRaw>) {
    let (w, h) = (256u32, 16u32);
    let mut cells = empty_cells(w, h);
    for y in 10..h {
        for x in 0..w {
            cells[(y * w + x) as usize].mat = STONE;
        }
    }
    (w, h, cells)
}

// Resting contact + Coulomb friction: a particle sliding on a floor at
// 300 cells/s travels v^2 / (2 mu g) and stops (no endless crawl).
#[test]
fn floor_slide_stops_after_coulomb_distance() {
    let (w, h, cells) = floor_strip();
    let v = 300.0f32;
    let mut r = ParticleReference::new(1);
    r.spawn(&[ParticleRaw {
        pos: [20.0, 10.0 - 1.0 / FP_ONE as f32],
        vel: [v, 0.0],
        life: f32::INFINITY,
        ..Default::default()
    }]);
    let mut stop_tick = None;
    for t in 0..120 {
        r.step(&cells, w, h, DT);
        if stop_tick.is_none() && r.particles()[0].vel[0] == 0.0 {
            stop_tick = Some(t + 1);
        }
    }
    let p = r.particles()[0];
    let slid = p.pos[0] - 20.0;
    let ideal = v * v / (2.0 * FRICTION_MU * GRAVITY_CELLS);
    let err = (slid - ideal).abs() / ideal;
    eprintln!(
        "slide {slid:.3} cells (ideal {ideal:.3}, error {:.3}%), stopped after {:?} ticks, vy {}",
        err * 100.0,
        stop_tick,
        p.vel[1]
    );
    assert!(err < 0.04, "slide distance error {:.3}%", err * 100.0);
    assert_eq!(p.vel, [0.0, 0.0], "at rest");
    assert!(p.pos[1] < 10.0 && p.pos[1] >= 9.0, "on the floor");
}

// A dropped particle bounces, settles within 2 s and then stays put.
#[test]
fn dropped_particle_comes_to_rest() {
    let (w, h, cells) = floor_strip();
    let mut r = ParticleReference::new(1);
    r.spawn(&[ParticleRaw {
        pos: [100.5, 2.5],
        vel: [40.0, 0.0],
        life: f32::INFINITY,
        ..Default::default()
    }]);
    for _ in 0..120 {
        r.step(&cells, w, h, DT);
    }
    let rest = r.particles()[0];
    for _ in 0..60 {
        r.step(&cells, w, h, DT);
    }
    let later = r.particles()[0];
    eprintln!(
        "rest at {:?}, vel {:?}, rest bounce cap {REST_VY_CPS} cells/s",
        rest.pos, rest.vel
    );
    assert_eq!(rest.vel, [0.0, 0.0]);
    assert_eq!(rest.pos, later.pos, "no crawl, no micro-bounce");
}

// GPU parity of the resting rule: sliders, droppers and a ledge.
#[test]
fn friction_parity_gpu() {
    let Some(gpu) = gpu_or_skip("friction_parity_gpu") else {
        return;
    };
    let (w, h, mut cells) = floor_strip();
    for x in 150..256 {
        cells[(9 * w + x) as usize].mat = STONE; // a one-cell step up
    }
    let batch: Vec<ParticleRaw> = (0..2000)
        .map(|i| ParticleRaw {
            pos: [1.0 + (i % 140) as f32, 1.0 + (i % 7) as f32],
            vel: [
                (i as f32 * 7.3) % 600.0 - 300.0,
                (i as f32 * 3.1) % 100.0 - 50.0,
            ],
            life: f32::INFINITY,
            ..Default::default()
        })
        .collect();
    let grid = grid_with(&gpu, w, h, &cells);
    let mut parts = Particles::new(&gpu, 2000);
    let mut r = ParticleReference::new(2000);
    parts.spawn(&gpu, &batch);
    r.spawn(&batch);
    let mut worst = 0;
    for t in 0..240 {
        parts.step(&gpu, &grid, DT);
        r.step(&cells, w, h, DT);
        if t % 40 == 39 {
            worst = worst.max(assert_same(&parts.readback(&gpu), r.particles()));
        }
    }
    let resting = r.particles().iter().filter(|p| p.vel == [0.0, 0.0]).count();
    eprintln!("friction parity: worst {worst} mismatches of 2000; {resting} at rest");
    assert_eq!(worst, 0);
    assert!(resting > 1000);
}
