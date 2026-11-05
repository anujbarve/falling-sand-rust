//! Air rules and their CPU reference. The WGSL in `shaders/air.wgsl` must match.
//!
//! Per tick, for every non-solid cell (solid cells hold zero velocity, zero
//! vapor and zero pressure):
//! 1. force: `vy -= BUOYANCY_K * (temp - AMBIENT_K) * dt` (hot air rises, +y is
//!    down), drag `v *= max(0, 1 - AIR_DRAG * dt)`, clamp `|v|` to [`MAX_AIR_CPS`].
//! 2. advect velocity (Stam semi-Lagrangian, bilinear) by `v + wind`.
//! 3. divergence of the advected field, then [`AIR_SWEEPS`] Jacobi sweeps of
//!    the pressure Poisson equation from `p = 0` (cost independent of content).
//! 4. project: `v -= grad p`.
//! 5. advect vapor by `v + wind`; store vapor and pressure in the cells.
//!
//! Neighbours that are solid or outside the grid use the centre cell's
//! pressure (zero normal gradient), so pressure never pushes air into solids.
//! Both the force and the advection step clamp speeds to [`MAX_AIR_CPS`] =
//! 60 cells/s = 1 cell per tick, so a backtrace never reaches past a one-cell
//! wall and vapor cannot tunnel through it (for `dt <= DT`).
//!
//! Air velocity does NOT live in `CellRaw.vx/vy` (those are granular state);
//! it is a separate buffer owned by [`crate::AirPass`]. `wind` is an ambient
//! uniform velocity added to the transport velocity only; it is not part of
//! the stored field and it does not flow around obstacles, but it is blocked
//! by them because solid cells are never sources of vapor.

use crate::materials::Material;
use crate::materials::WALL;

/// Jacobi sweeps per tick, always exactly this many.
pub const AIR_SWEEPS: u32 = 20;
/// Largest air speed in cells/s (1 cell per tick at 60 Hz).
pub const MAX_AIR_CPS: f32 = 60.0;
/// Ambient temperature in kelvin; buoyancy is zero here.
pub const AMBIENT_K: f32 = 293.0;
/// Buoyant acceleration per kelvin above ambient, in cells/s^2/K
/// (Boussinesq: `g / T_ambient`, so 300 K of excess gives about 1 g).
pub const BUOYANCY_K: f32 = 981.0 / 293.0;
/// Linear drag in 1/s.
pub const AIR_DRAG: f32 = 2.0;
/// Temperature excesses beyond this (or NaN) are ignored by buoyancy.
pub const MAX_TEMP_EXCESS: f32 = 1.0e4;

/// True for materials (and [`WALL`]) that block air: Stone, Rock, Wood, Ash,
/// Metal, Glass, Ice and the boundary wall.
pub fn is_solid(mat: u32) -> bool {
    use Material::*;
    const SOLIDS: [u32; 7] = [
        Stone as u32,
        Rock as u32,
        Wood as u32,
        Ash as u32,
        Metal as u32,
        Glass as u32,
        Ice as u32,
    ];
    mat == WALL || SOLIDS.contains(&mat)
}

fn clamp_len(v: [f32; 2]) -> [f32; 2] {
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if len > MAX_AIR_CPS {
        let s = MAX_AIR_CPS / len;
        [v[0] * s, v[1] * s]
    } else {
        v
    }
}

/// CPU model of the air rule. Fields are public so tests can set up scenes.
pub struct AirReference {
    w: u32,
    h: u32,
    /// Material id per cell (row-major).
    pub mat: Vec<u32>,
    /// Temperature in kelvin per cell.
    pub temp: Vec<f32>,
    /// Vapor amount per cell.
    pub vapor: Vec<f32>,
    /// Pressure per cell (Lagrange multiplier of the projection, cells^2/s).
    pub pressure: Vec<f32>,
    /// Air velocity per cell in cells/s (excludes wind).
    pub vel: Vec<[f32; 2]>,
}

impl AirReference {
    /// An empty `w` x `h` field at ambient temperature with no vapor.
    pub fn new(w: u32, h: u32) -> AirReference {
        let n = (w * h) as usize;
        AirReference {
            w,
            h,
            mat: vec![0; n],
            temp: vec![AMBIENT_K; n],
            vapor: vec![0.0; n],
            pressure: vec![0.0; n],
            vel: vec![[0.0; 2]; n],
        }
    }

    fn idx(&self, x: i32, y: i32) -> usize {
        let cx = x.clamp(0, self.w as i32 - 1) as usize;
        let cy = y.clamp(0, self.h as i32 - 1) as usize;
        cy * self.w as usize + cx
    }

    fn solid(&self, i: usize) -> bool {
        is_solid(self.mat[i])
    }

    /// Bilinear sample of `f` at fractional cell position (`px`, `py`).
    fn sample<T: Copy + Default>(
        &self,
        f: impl Fn(usize) -> T,
        lerp: impl Fn(T, T, f32) -> T,
        px: f32,
        py: f32,
    ) -> T {
        let px = px.max(0.0).min(self.w as f32 - 1.0);
        let py = py.max(0.0).min(self.h as f32 - 1.0);
        let (x0, y0) = (px.floor() as i32, py.floor() as i32);
        let (fx, fy) = (px - x0 as f32, py - y0 as f32);
        let (x1, y1) = (
            (x0 + 1).min(self.w as i32 - 1),
            (y0 + 1).min(self.h as i32 - 1),
        );
        let top = lerp(f(self.idx(x0, y0)), f(self.idx(x1, y0)), fx);
        let bot = lerp(f(self.idx(x0, y1)), f(self.idx(x1, y1)), fx);
        lerp(top, bot, fy)
    }

    /// Pressure of the neighbour at (`x`, `y`) of centre `c`; solid or outside -> `c`.
    fn pn(&self, p: &[f32], x: i32, y: i32, c: f32) -> f32 {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return c;
        }
        let i = self.idx(x, y);
        if self.solid(i) {
            c
        } else {
            p[i]
        }
    }

    /// Advances one tick of `dt` seconds with ambient `wind` (cells/s).
    pub fn tick(&mut self, dt: f32, wind: [f32; 2]) {
        let (w, h) = (self.w as i32, self.h as i32);
        let n = (w * h) as usize;
        let wind = clamp_len(wind);
        let vlerp = |a: [f32; 2], b: [f32; 2], t: f32| {
            [a[0] * (1.0 - t) + b[0] * t, a[1] * (1.0 - t) + b[1] * t]
        };
        // 1. force
        for i in 0..n {
            if self.solid(i) {
                self.vel[i] = [0.0; 2];
                continue;
            }
            let d = self.temp[i] - AMBIENT_K;
            let d = if d.abs() <= MAX_TEMP_EXCESS { d } else { 0.0 };
            let mut v = self.vel[i];
            v[1] -= BUOYANCY_K * d * dt;
            let k = (1.0 - AIR_DRAG * dt).max(0.0);
            self.vel[i] = clamp_len([v[0] * k, v[1] * k]);
        }
        // 2. advect velocity
        let mut vb = vec![[0.0f32; 2]; n];
        for y in 0..h {
            for x in 0..w {
                let i = self.idx(x, y);
                if self.solid(i) {
                    continue;
                }
                let t = clamp_len([self.vel[i][0] + wind[0], self.vel[i][1] + wind[1]]);
                vb[i] = self.sample(
                    |j| self.vel[j],
                    vlerp,
                    x as f32 - t[0] * dt,
                    y as f32 - t[1] * dt,
                );
            }
        }
        // 3. divergence and Jacobi
        let mut div = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = self.idx(x, y);
                if !self.solid(i) {
                    let (r, l) = (vb[self.idx(x + 1, y)], vb[self.idx(x - 1, y)]);
                    let (d, u) = (vb[self.idx(x, y + 1)], vb[self.idx(x, y - 1)]);
                    div[i] = 0.5 * ((r[0] - l[0]) + (d[1] - u[1]));
                }
            }
        }
        let mut pa = vec![0.0f32; n];
        let mut pb = vec![0.0f32; n];
        for _ in 0..AIR_SWEEPS {
            for y in 0..h {
                for x in 0..w {
                    let i = self.idx(x, y);
                    pb[i] = if self.solid(i) {
                        0.0
                    } else {
                        let c = pa[i];
                        let s = self.pn(&pa, x + 1, y, c)
                            + self.pn(&pa, x - 1, y, c)
                            + self.pn(&pa, x, y - 1, c)
                            + self.pn(&pa, x, y + 1, c);
                        (s - div[i]) * 0.25
                    };
                }
            }
            std::mem::swap(&mut pa, &mut pb);
        }
        // 4. project
        for y in 0..h {
            for x in 0..w {
                let i = self.idx(x, y);
                self.vel[i] = if self.solid(i) {
                    [0.0; 2]
                } else {
                    let c = pa[i];
                    let gx = (self.pn(&pa, x + 1, y, c) - self.pn(&pa, x - 1, y, c)) * 0.5;
                    let gy = (self.pn(&pa, x, y + 1, c) - self.pn(&pa, x, y - 1, c)) * 0.5;
                    [vb[i][0] - gx, vb[i][1] - gy]
                };
            }
        }
        // 5. advect vapor, commit
        let mut vt = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = self.idx(x, y);
                if self.solid(i) {
                    continue;
                }
                let t = clamp_len([self.vel[i][0] + wind[0], self.vel[i][1] + wind[1]]);
                let fl = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
                vt[i] = self.sample(
                    |j| if self.solid(j) { 0.0 } else { self.vapor[j] },
                    fl,
                    x as f32 - t[0] * dt,
                    y as f32 - t[1] * dt,
                );
            }
        }
        self.vapor = vt;
        self.pressure = pa;
    }
}
