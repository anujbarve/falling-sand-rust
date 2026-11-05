//! CPU reference for the particle system. It DEFINES the GPU behaviour
//! (`shaders/particles.wgsl`); the GPU must match it bit for bit.
//!
//! Storage is a ring buffer ([`Ring`]) of up to `cap` slots: spawning writes
//! at `head` and wraps, so beyond `cap` the oldest particles are overwritten
//! and the count stays at `cap`.
//!
//! Per step, for every occupied slot (motion in integer fixed point, 1 cell =
//! [`FP_ONE`] = 1024 units, so CPU and GPU agree exactly):
//! 1. Sanitize: a non-finite `pos`, `vel` or `temp`, or a NaN `life`, zeroes
//!    the particle (dead) and counts it. Dead particles (`life <= 0`) are
//!    skipped.
//! 2. A particle outside the grid or inside a solid cell ([`is_solid`]) is
//!    killed (buried). Otherwise it is quantized: `fp = floor(x * 1024)` (exact, so
//!    the fixed-point cell is exactly `floor(x)`);
//!    velocity is clamped to +-[`MAX_SPEED_CPS`] first.
//! 3. Resting contact: a particle SUPPORTED by a floor (its fixed-point `y`
//!    is the last unit of its cell, the cell below is solid or outside, and
//!    `|vy| <= REST_VY_CPS`) gets `vy = 0` (the floor's normal force cancels
//!    gravity) and Coulomb friction: `vx` moves toward 0 by
//!    `round(FRICTION_MU * g * 1024 / hz)` per step, never past 0. So a slide
//!    stops after about `v^2 / (2 mu g)` and the horizontal move is the full
//!    `vx / hz` (no crawl). Otherwise kick: `vy += round(g * 1024 / hz)`
//!    (g = 981 cells/s^2, `hz = 1/dt`), clamped again.
//!    Drift: `d = v / hz` (truncating integer division).
//! 4. Collision: the segment is walked cell by cell (grid DDA, x first on a
//!    tie). On entering a solid cell or leaving the grid, the velocity
//!    component on the crossed axis becomes `-(v * 3) / 5` (restitution 0.6),
//!    the particle stops at the crossing point clamped inside the last free
//!    cell, and the rest of the move is dropped. A particle therefore never
//!    ends a step inside a solid cell and never tunnels through a one-cell wall.
//! 5. `life -= dt` in f32 seconds; `<= 0` becomes exactly 0 (dead).
//!
//! Positions and velocities are stored back as `fp / 1024` (exact in f32).

use crate::layout::{CellRaw, ParticleRaw};
use crate::units::GRAVITY_CELLS;

pub use crate::air::is_solid;

/// One cell in particle fixed point (10 fractional bits).
pub const FP_ONE: i32 = 1 << FP_BITS;
/// Fractional bits of the particle fixed point.
pub const FP_BITS: u32 = 10;
/// Speed cap per axis in cells/s (18 m/s). Keeps one step at 1/60 s within
/// 30 cells (30 720 units), so DDA products stay below 2^31.
pub const MAX_SPEED_CPS: f32 = 1800.0;
/// [`MAX_SPEED_CPS`] in fixed point.
pub const MAX_SPEED_FP: i32 = MAX_SPEED_CPS as i32 * FP_ONE;
/// Restitution numerator (restitution = 3/5 = 0.6).
pub const REST_NUM: i32 = 3;
/// Restitution denominator.
pub const REST_DEN: i32 = 5;
/// Coulomb friction coefficient for a particle resting on a floor (dry
/// granular or stone on stone, typically 0.4 to 0.7; Engineering ToolBox
/// "Friction and Friction Coefficients"). Deceleration `mu * g` = 490.5
/// cells/s^2.
pub const FRICTION_MU: f32 = 0.5;
/// Largest vertical speed (cells/s) at which a supported particle stops
/// bouncing and rests. A 30 cells/s bounce would rise `v^2 / 2g` = 0.46 cell,
/// less than one cell, so resting it is invisible.
pub const REST_VY_CPS: i32 = 30;
/// [`REST_VY_CPS`] in fixed point.
pub const REST_VY_FP: i32 = REST_VY_CPS * FP_ONE;
/// Largest grid side (cells) for which fixed-point positions are exact in f32.
pub const MAX_SIDE: u32 = 1 << (24 - FP_BITS);

/// Per-step constants derived from `dt`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepConsts {
    /// Steps per second (`1 / dt`).
    pub hz: i32,
    /// Gravity gained per step in fixed-point cells/s.
    pub grav_fp: i32,
    /// Step length in seconds (life countdown).
    pub dt: f32,
    /// Friction speed loss per step for a resting particle, fixed-point cells/s.
    pub fric_fp: i32,
}

impl StepConsts {
    /// Derives the constants for a step of `dt` seconds.
    ///
    /// # Panics
    /// Unless `dt = 1/n` s for an integer `n >= 60` (the speed cap assumes at
    /// most 30 cells per step).
    pub fn new(dt: f32) -> StepConsts {
        let hz = (1.0 / f64::from(dt)).round();
        assert!(
            hz >= 60.0 && (f64::from(dt) * hz - 1.0).abs() < 1e-4,
            "particle dt must be 1/n s with n >= 60, got {dt}"
        );
        let hz = hz as i32;
        StepConsts {
            hz,
            grav_fp: (f64::from(GRAVITY_CELLS) * f64::from(FP_ONE) / f64::from(hz)).round() as i32,
            dt,
            fric_fp: (f64::from(FRICTION_MU) * f64::from(GRAVITY_CELLS) * f64::from(FP_ONE)
                / f64::from(hz))
            .round() as i32,
        }
    }
}

/// Ring-buffer cursor shared by the reference and the GPU system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ring {
    cap: u32,
    head: u32,
    count: u32,
}

/// One contiguous write of a spawn batch: `len` entries of the batch starting
/// at `src` go to slots `dst..dst + len`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// First destination slot.
    pub dst: u32,
    /// First batch index.
    pub src: usize,
    /// Number of entries.
    pub len: usize,
}

impl Ring {
    /// An empty ring of `cap` slots.
    ///
    /// # Panics
    /// If `cap` is 0.
    pub fn new(cap: u32) -> Ring {
        assert!(cap > 0, "particle cap must be non-zero");
        Ring {
            cap,
            head: 0,
            count: 0,
        }
    }

    /// Slots in the ring.
    pub fn cap(&self) -> u32 {
        self.cap
    }

    /// Next slot to write; once full, the slot of the oldest particle.
    pub fn head(&self) -> u32 {
        self.head
    }

    /// Occupied slots (`0..count`), at most `cap`.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Reserves slots for a batch of `n` and returns where each part goes
    /// (at most two chunks). Only the newest `cap` entries are kept.
    pub fn push(&mut self, n: usize) -> Vec<Chunk> {
        let cap = self.cap as usize;
        let keep = n.min(cap);
        let mut src = n - keep;
        let mut left = keep;
        let mut out = Vec::with_capacity(2);
        while left > 0 {
            let len = left.min(cap - self.head as usize);
            out.push(Chunk {
                dst: self.head,
                src,
                len,
            });
            src += len;
            left -= len;
            self.head = ((self.head as usize + len) % cap) as u32;
        }
        self.count = (self.count as usize + keep).min(cap) as u32;
        out
    }
}

fn to_fp(x: f32) -> i32 {
    (x * FP_ONE as f32).floor() as i32
}

fn from_fp(i: i32) -> f32 {
    i as f32 * (1.0 / FP_ONE as f32)
}

fn vel_fp(v: f32) -> i32 {
    to_fp(v.clamp(-MAX_SPEED_CPS, MAX_SPEED_CPS))
}

/// Material at cell (`x`, `y`); outside the grid is [`crate::materials::WALL`].
fn mat(cells: &[CellRaw], w: u32, h: u32, x: i32, y: i32) -> u32 {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        crate::materials::WALL
    } else {
        cells[(y as u32 * w + x as u32) as usize].mat
    }
}

/// Advances one particle by one step. Returns true if it was sanitized.
pub fn step_particle(
    p: &mut ParticleRaw,
    cells: &[CellRaw],
    w: u32,
    h: u32,
    k: StepConsts,
) -> bool {
    let ok =
        p.pos.iter().chain(&p.vel).all(|v| v.is_finite()) && p.temp.is_finite() && !p.life.is_nan();
    if !ok {
        *p = ParticleRaw {
            mat: p.mat,
            ..Default::default()
        };
        return true;
    }
    if p.life <= 0.0 {
        return false;
    }
    let inside = p.pos[0] >= 0.0 && p.pos[1] >= 0.0 && p.pos[0] < w as f32 && p.pos[1] < h as f32;
    let (mut px, mut py) = if inside {
        (to_fp(p.pos[0]), to_fp(p.pos[1]))
    } else {
        (-1, -1)
    };
    let (mut cx, mut cy) = (px >> FP_BITS, py >> FP_BITS);
    if !inside || is_solid(mat(cells, w, h, cx, cy)) {
        p.life = 0.0;
        return false;
    }
    let mut vx = vel_fp(p.vel[0]);
    let vy0 = vel_fp(p.vel[1]);
    let supported = (py & (FP_ONE - 1)) == FP_ONE - 1
        && is_solid(mat(cells, w, h, cx, cy + 1))
        && vy0.abs() <= REST_VY_FP;
    let mut vy = if supported {
        vx = if vx > 0 {
            (vx - k.fric_fp).max(0)
        } else {
            (vx + k.fric_fp).min(0)
        };
        0
    } else {
        (vy0 + k.grav_fp).clamp(-MAX_SPEED_FP, MAX_SPEED_FP)
    };
    let (dx, dy) = (vx / k.hz, vy / k.hz);
    let (ex, ey) = ((px + dx) >> FP_BITS, (py + dy) >> FP_BITS);
    let (ax, ay) = (dx.abs(), dy.abs());
    let (sx, sy) = (dx.signum(), dy.signum());
    // 0 = no hit, 1 = hit crossing x, 2 = hit crossing y.
    let mut hit = 0;
    let (mut distx, mut disty) = (0, 0);
    loop {
        let (needx, needy) = (cx != ex, cy != ey);
        if !needx && !needy {
            break;
        }
        // Distance from the start to entering the next cell on each axis.
        distx = if sx > 0 {
            ((cx + 1) << FP_BITS) - px
        } else {
            px - (cx << FP_BITS) + 1
        };
        disty = if sy > 0 {
            ((cy + 1) << FP_BITS) - py
        } else {
            py - (cy << FP_BITS) + 1
        };
        let stepx = if needx && needy {
            distx * ay <= disty * ax
        } else {
            needx
        };
        let (nx, ny) = if stepx { (cx + sx, cy) } else { (cx, cy + sy) };
        if is_solid(mat(cells, w, h, nx, ny)) {
            hit = if stepx { 1 } else { 2 };
            break;
        }
        cx = nx;
        cy = ny;
    }
    let (lo_x, lo_y) = (cx << FP_BITS, cy << FP_BITS);
    let top = FP_ONE - 1;
    if hit == 1 {
        py = (py + dy * distx / ax).clamp(lo_y, lo_y + top);
        px = if sx > 0 { lo_x + top } else { lo_x };
        vx = -(vx * REST_NUM) / REST_DEN;
    } else if hit == 2 {
        px = (px + dx * disty / ay).clamp(lo_x, lo_x + top);
        py = if sy > 0 { lo_y + top } else { lo_y };
        vy = -(vy * REST_NUM) / REST_DEN;
    } else {
        px += dx;
        py += dy;
    }
    p.pos = [from_fp(px), from_fp(py)];
    p.vel = [from_fp(vx), from_fp(vy)];
    p.life -= k.dt;
    if p.life <= 0.0 {
        p.life = 0.0;
    }
    false
}

/// CPU model of the particle system: a ring of particles plus the step rule.
pub struct ParticleReference {
    ring: Ring,
    parts: Vec<ParticleRaw>,
    sanitized: u32,
}

impl ParticleReference {
    /// An empty system of `cap` slots.
    pub fn new(cap: u32) -> ParticleReference {
        ParticleReference {
            ring: Ring::new(cap),
            parts: Vec::new(),
            sanitized: 0,
        }
    }

    /// Adds `batch`, overwriting the oldest particles beyond the cap.
    pub fn spawn(&mut self, batch: &[ParticleRaw]) {
        for c in self.ring.push(batch.len()) {
            let dst = c.dst as usize;
            if self.parts.len() < dst + c.len {
                self.parts.resize(dst + c.len, ParticleRaw::default());
            }
            self.parts[dst..dst + c.len].copy_from_slice(&batch[c.src..c.src + c.len]);
        }
    }

    /// Advances every particle by `dt` against the `w` x `h` row-major `cells`.
    pub fn step(&mut self, cells: &[CellRaw], w: u32, h: u32, dt: f32) {
        assert_eq!(cells.len() as u64, u64::from(w) * u64::from(h));
        let k = StepConsts::new(dt);
        for p in &mut self.parts {
            self.sanitized += u32::from(step_particle(p, cells, w, h, k));
        }
    }

    /// Occupied slots in slot order (`count` entries).
    pub fn particles(&self) -> &[ParticleRaw] {
        &self.parts
    }

    /// Occupied slots.
    pub fn count(&self) -> u32 {
        self.ring.count()
    }

    /// The ring cursor.
    pub fn ring(&self) -> Ring {
        self.ring
    }

    /// Particles sanitized (non-finite fields) so far.
    pub fn sanitized(&self) -> u32 {
        self.sanitized
    }
}
