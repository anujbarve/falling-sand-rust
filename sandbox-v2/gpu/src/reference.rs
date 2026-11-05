//! CPU reference for the parallel row-parity granular and liquid rules.
//!
//! This module DEFINES the GPU behaviour: the WGSL kernels must match it cell
//! for cell. It intentionally differs from the v1 CPU rules in `sandbox/`
//! (no sequential in-row sweep, no `updated` flags, no Rng stream).
//!
//! Rule summary (gravity points toward increasing `y`):
//! * A tick runs the granular pass (phase 0, phase 1) then the liquid pass
//!   (phase 0, phase 1). Phase `p` lets only cells with `y % 2 == p` act.
//! * Every decision reads only the state at the start of the phase.
//!
//! Velocity rule (all integers, fixed point with [`FP_ONE`] = 1 cell):
//! * Each mover keeps `vy` (fixed-point cells per tick, downward) in the `vy`
//!   field and a sub-cell remainder `frac` (0..FP_ONE) in the `vx` field.
//! * Each tick, the first time the mover is considered (a stamp
//!   `tick + 1` in `flags` prevents a second look after it changes row parity):
//!   `v = min(vy + GRAVITY_FP, terminal_fp)`, `acc = frac + v`,
//!   `n = acc / FP_ONE` whole cells to move, `frac = acc % FP_ONE`.
//! * If `n == 0` the cell stays and keeps `(v, frac)`.
//! * Else it targets the straight-down run of cells empty at phase start,
//!   `m = min(n, empty run length)`. With `m == n` it keeps `(v, frac)`; with
//!   `m < n` it stops and `vy = frac = 0`. If `m == 0` it tries the diagonal
//!   (water: then one cell sideways); any such move, and any blocked or
//!   conflict-losing attempt, resets `vy = frac = 0`.
//! * Several movers targeting one empty cell: winner in order the nearest
//!   mover straight above (path clear), above-left, above-right,
//!   sideways-left, sideways-right. Losers stay.
//! * Sand: down, else diagonal (hash bit 0 picks the first side). Water: also
//!   one sideways step (hash bit 1 picks the first side).
//! * Out-of-grid cells count as solid.
//! * Free fall from rest covers `g t^2 / 2`: 100 cells take 27 ticks (0.45 s).

use crate::materials::{Material, MATERIALS};
use crate::units::{cps_to_fp, FP_ONE, GRAVITY_FP, MAX_STEP};

/// Empty cell.
pub const EMPTY: u32 = 0;
/// Sand (granular).
pub const SAND: u32 = 1;
/// Water (liquid).
pub const WATER: u32 = 2;
/// Immovable solid.
pub const WALL: u32 = 3;

/// 32-bit integer finalizer (murmur3 `fmix32`). Mirrored in WGSL as `mix32`.
fn mix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^ (h >> 16)
}

/// Per-cell hash of `(tick, x, y, seed)` using u32 wrapping math.
///
/// Bit 0 chooses the diagonal side, bit 1 the sideways side. The WGSL
/// `cell_hash` is a line-for-line port.
pub fn cell_hash(tick: u32, x: u32, y: u32, seed: u32) -> u32 {
    let mut h = mix32(seed ^ tick);
    h = mix32(h ^ x);
    mix32(h ^ y)
}

/// Fixed-point terminal speed of the sand grains and water cells moved by `kind`.
fn terminal_fp(kind: Kind) -> i32 {
    let m = if kind == Kind::Granular { Material::Sand } else { Material::Water };
    cps_to_fp(MATERIALS[m as usize].terminal_cps)
}

/// Which family of movers a pass moves.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Granular,
    Liquid,
}

/// What a mover that is considered this phase wants to do.
struct Plan {
    /// Cell it moves to, if it wants to move.
    target: Option<(i64, i64)>,
    /// `(vy, frac)` after a successful move.
    moved: (i32, i32),
    /// `(vy, frac)` when it stays (no move wanted, or conflict lost).
    stay: (i32, i32),
}

/// A material grid plus per-cell fall state and the parallel row-parity rules.
#[derive(Clone)]
pub struct Reference {
    w: u32,
    h: u32,
    cells: Vec<u32>,
    vy: Vec<i32>,
    frac: Vec<i32>,
    stamp: Vec<u32>,
}

impl Reference {
    /// Creates an all-empty `w` x `h` grid.
    pub fn new(w: u32, h: u32) -> Reference {
        let n = (w * h) as usize;
        Reference {
            w,
            h,
            cells: vec![EMPTY; n],
            vy: vec![0; n],
            frac: vec![0; n],
            stamp: vec![0; n],
        }
    }

    /// Material at `(x, y)`; out-of-grid reads as [`WALL`].
    pub fn get(&self, x: u32, y: u32) -> u32 {
        self.at(i64::from(x), i64::from(y))
    }

    /// Sets the material at `(x, y)` and resets its fall state. Panics when out of the grid.
    pub fn set(&mut self, x: u32, y: u32, mat: u32) {
        assert!(x < self.w && y < self.h, "cell out of grid");
        let i = (y * self.w + x) as usize;
        self.cells[i] = mat;
        self.vy[i] = 0;
        self.frac[i] = 0;
        self.stamp[i] = 0;
    }

    /// Row-major materials.
    pub fn cells(&self) -> &[u32] {
        &self.cells
    }

    /// Row-major fixed-point fall velocity (the GPU `vy` field holds the same integers as `f32`).
    pub fn vy(&self) -> &[i32] {
        &self.vy
    }

    /// Row-major fixed-point sub-cell remainder (the GPU `vx` field).
    pub fn frac(&self) -> &[i32] {
        &self.frac
    }

    /// Number of cells holding `mat`.
    pub fn count(&self, mat: u32) -> usize {
        self.cells.iter().filter(|&&m| m == mat).count()
    }

    /// Runs one full tick: granular pass then liquid pass.
    pub fn tick(&mut self, tick: u32, seed: u32) {
        for kind in [Kind::Granular, Kind::Liquid] {
            for phase in 0..2 {
                self.phase(kind, phase, tick, seed);
            }
        }
    }

    fn at(&self, x: i64, y: i64) -> u32 {
        if x < 0 || y < 0 || x >= i64::from(self.w) || y >= i64::from(self.h) {
            WALL
        } else {
            self.cells[self.ix(x, y)]
        }
    }

    fn ix(&self, x: i64, y: i64) -> usize {
        (y as u32 * self.w + x as u32) as usize
    }

    fn is_empty(&self, x: i64, y: i64) -> bool {
        self.at(x, y) == EMPTY
    }

    /// Plan of the mover at `(x, y)`, or `None` when it is not considered this phase
    /// (wrong material, wrong row parity, or already considered this tick).
    fn plan(&self, kind: Kind, phase: u32, tick: u32, seed: u32, x: i64, y: i64) -> Option<Plan> {
        let mover = if kind == Kind::Granular { SAND } else { WATER };
        if y < 0 || y as u32 % 2 != phase || self.at(x, y) != mover {
            return None;
        }
        let i = self.ix(x, y);
        if self.stamp[i] == tick.wrapping_add(1) {
            return None;
        }
        let v = (self.vy[i] + GRAVITY_FP).clamp(0, terminal_fp(kind));
        let acc = self.frac[i].clamp(0, FP_ONE - 1) + v;
        let n = acc / FP_ONE;
        let kept = (v, acc % FP_ONE);
        if n == 0 {
            return Some(Plan { target: None, moved: kept, stay: kept });
        }
        let stop = (0, 0);
        let mut m = 0;
        while m < n && self.is_empty(x, y + 1 + i64::from(m)) {
            m += 1;
        }
        let (target, moved) = if m > 0 {
            (Some((x, y + i64::from(m))), if m == n { kept } else { stop })
        } else {
            (self.slide(kind, tick, seed, x, y), stop)
        };
        Some(Plan { target, moved, stay: stop })
    }

    /// Diagonal (then, for liquids, sideways) target of a blocked mover.
    fn slide(&self, kind: Kind, tick: u32, seed: u32, x: i64, y: i64) -> Option<(i64, i64)> {
        let hash = cell_hash(tick, x as u32, y as u32, seed);
        let d = if hash & 1 == 1 { 1 } else { -1 };
        for dx in [d, -d] {
            if self.is_empty(x + dx, y + 1) {
                return Some((x + dx, y + 1));
            }
        }
        if kind == Kind::Liquid {
            let s = if (hash >> 1) & 1 == 1 { 1 } else { -1 };
            for dx in [s, -s] {
                if self.is_empty(x + dx, y) {
                    return Some((x + dx, y));
                }
            }
        }
        None
    }

    /// Source (and its plan) that wins the empty cell `(x, y)`, by fixed priority.
    fn winner(&self, kind: Kind, phase: u32, tick: u32, seed: u32, x: i64, y: i64) -> Option<((i64, i64), Plan)> {
        let wins = |sx: i64, sy: i64| {
            self.plan(kind, phase, tick, seed, sx, sy).filter(|p| p.target == Some((x, y)))
        };
        // Only the nearest non-empty cell above can reach `(x, y)` straight down.
        for dy in 1..=i64::from(MAX_STEP) {
            if !self.is_empty(x, y - dy) {
                if let Some(p) = wins(x, y - dy) {
                    return Some(((x, y - dy), p));
                }
                break;
            }
        }
        [(x - 1, y - 1), (x + 1, y - 1), (x - 1, y), (x + 1, y)]
            .into_iter()
            .find_map(|(sx, sy)| wins(sx, sy).map(|p| ((sx, sy), p)))
    }

    /// One double-buffered phase: every output cell is computed from `self`.
    fn phase(&mut self, kind: Kind, phase: u32, tick: u32, seed: u32) {
        let mut out = self.clone();
        let stamp = tick.wrapping_add(1);
        for y in 0..i64::from(self.h) {
            for x in 0..i64::from(self.w) {
                let i = self.ix(x, y);
                if self.cells[i] == EMPTY {
                    if let Some(((sx, sy), p)) = self.winner(kind, phase, tick, seed, x, y) {
                        out.cells[i] = self.at(sx, sy);
                        (out.vy[i], out.frac[i]) = p.moved;
                        out.stamp[i] = stamp;
                    }
                } else if let Some(p) = self.plan(kind, phase, tick, seed, x, y) {
                    let wins = p.target.and_then(|(tx, ty)| self.winner(kind, phase, tick, seed, tx, ty));
                    if wins.is_some_and(|(src, _)| src == (x, y)) {
                        out.cells[i] = EMPTY;
                        out.vy[i] = 0;
                        out.frac[i] = 0;
                        out.stamp[i] = 0;
                    } else {
                        (out.vy[i], out.frac[i]) = p.stay;
                        out.stamp[i] = stamp;
                    }
                }
            }
        }
        *self = out;
    }
}
