//! Air: semi-Lagrangian advection, buoyancy, wind and pressure relaxation.
//!
//! Velocity units are "simulation units"; one unit moves a parcel
//! `ADVECT_SCALE` cells per second.

use crate::engine::System;
use sandbox_core::rng::Rng;
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;

/// Gauss-Seidel sweeps of the pressure solve per tick.
const PRESSURE_ITERS: usize = 20;
/// Maximum air speed (velocity magnitude).
const MAX_VEL: f32 = 4.0;
/// Cells travelled per second by a velocity of 1.0.
const ADVECT_SCALE: f32 = 20.0;
/// Ambient temperature in kelvin; buoyancy is proportional to the excess over it.
const AMBIENT: f32 = 293.0;
/// Upward acceleration per kelvin of excess, per second.
const BUOYANCY: f32 = 0.05;
/// Fraction of the gap to `wind` closed each tick (also acts as drag).
const WIND_RELAX: f32 = 0.5;
/// Diffusion strength of the pressure solve (neighbour weight).
const PRESSURE_DIFFUSION: f32 = 0.2;
/// Velocity change per unit pressure gradient, per second.
const PRESSURE_FORCE: f32 = 0.5;

/// Moves air velocity and vapor, heats rise, wind blows, pressure evens out.
#[derive(Default)]
pub struct AirSystem {
    /// Background wind the velocity field relaxes toward every tick.
    pub wind: Vec2,
}

/// Bilinear sample of `f` at continuous cell coordinates, clamped to the grid.
fn sample<T: Copy>(f: &[T], w: usize, h: usize, x: f32, y: f32, lerp: impl Fn(T, T, f32) -> T) -> T {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let top = lerp(f[y0 * w + x0], f[y0 * w + x1], tx);
    let bot = lerp(f[y1 * w + x0], f[y1 * w + x1], tx);
    lerp(top, bot, ty)
}

fn lerp_f(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_v(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    a + (b - a) * t
}

/// Clamps a vector's length to `MAX_VEL`; non-finite input becomes zero.
fn clamp_vel(v: Vec2) -> Vec2 {
    if !v.x.is_finite() || !v.y.is_finite() {
        return Vec2::default();
    }
    let l = v.length();
    if l > MAX_VEL { v * (MAX_VEL / l) } else { v }
}

impl System for AirSystem {
    fn name(&self) -> &'static str {
        "air"
    }

    fn step(&mut self, w: &mut World, _rng: &mut Rng, dt: f32) {
        let (gw, gh) = (w.air.w, w.air.h);
        if gw == 0 || gh == 0 {
            return;
        }
        let n = gw * gh;

        // Advect velocity and vapor by tracing each cell back along the velocity.
        let old_vel = w.air.vel.clone();
        let old_vapor = w.air.vapor.clone();
        for y in 0..gh {
            for x in 0..gw {
                let i = y * gw + x;
                let back = old_vel[i] * (dt * ADVECT_SCALE);
                let (sx, sy) = (x as f32 - back.x, y as f32 - back.y);
                w.air.vel[i] = sample(&old_vel, gw, gh, sx, sy, lerp_v);
                w.air.vapor[i] = sample(&old_vapor, gw, gh, sx, sy, lerp_f).max(0.0);
            }
        }

        // Buoyancy (up is -y) and relaxation toward the background wind.
        for i in 0..n {
            let temp = w.cells.as_slice()[i].temp;
            let mut v = w.air.vel[i];
            v.y -= BUOYANCY * (temp - AMBIENT) * dt;
            v = v + (self.wind - v) * WIND_RELAX;
            w.air.vel[i] = v;
        }

        // A field of exact zeros stays exactly zero under diffusion, rescaling and the
        // gradient force, so the 20-sweep solve is skipped (identical result, no work).
        if w.air.pressure.iter().all(|&p| p == 0.0) {
            for v in &mut w.air.vel {
                *v = clamp_vel(*v);
            }
            return;
        }

        // Pressure: diffuse with Gauss-Seidel (walls reflect), then rescale so the
        // total is exactly conserved.
        let before: f32 = w.air.pressure.iter().sum();
        let rhs = w.air.pressure.clone();
        let a = PRESSURE_DIFFUSION;
        for _ in 0..PRESSURE_ITERS {
            for y in 0..gh {
                for x in 0..gw {
                    let i = y * gw + x;
                    let p = &w.air.pressure;
                    let l = p[if x > 0 { i - 1 } else { i }];
                    let r = p[if x + 1 < gw { i + 1 } else { i }];
                    let u = p[if y > 0 { i - gw } else { i }];
                    let d = p[if y + 1 < gh { i + gw } else { i }];
                    w.air.pressure[i] = (rhs[i] + a * (l + r + u + d)) / (1.0 + 4.0 * a);
                }
            }
        }
        let after: f32 = w.air.pressure.iter().sum();
        if after.abs() > f32::EPSILON && before.is_finite() {
            let k = before / after;
            for p in &mut w.air.pressure {
                *p *= k;
            }
        }

        // Pressure gradient pushes air from high to low pressure.
        for y in 0..gh {
            for x in 0..gw {
                let i = y * gw + x;
                let p = &w.air.pressure;
                let gx = p[if x + 1 < gw { i + 1 } else { i }] - p[if x > 0 { i - 1 } else { i }];
                let gy = p[if y + 1 < gh { i + gw } else { i }] - p[if y > 0 { i - gw } else { i }];
                let v = w.air.vel[i] - Vec2 { x: gx, y: gy } * (0.5 * PRESSURE_FORCE * dt);
                w.air.vel[i] = clamp_vel(v);
            }
        }
    }
}
