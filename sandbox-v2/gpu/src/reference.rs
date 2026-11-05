//! CPU reference for the parallel row-parity granular and liquid rules.
//!
//! This module DEFINES the GPU behaviour: the WGSL kernels must match it cell
//! for cell. It intentionally differs from the v1 CPU rules in `sandbox/`
//! (no sequential in-row sweep, no `updated` flags, no Rng stream).
//!
//! Rule summary (gravity points toward increasing `y`):
//! * A tick runs the granular pass (phase 0, phase 1) then the liquid pass
//!   (phase 0, phase 1). Phase `p` lets only cells with `y % 2 == p` move.
//! * Every decision reads only the state at the start of the phase.
//! * A mover targets one cell that is empty at phase start. If several movers
//!   target the same cell the winner is, in order: the mover directly above,
//!   the one above-left, above-right, then sideways-left, sideways-right.
//!   Losers stay put (no retry).
//! * Sand: down, else diagonal down (hash bit 0 picks the first side, the
//!   other side is the fallback).
//! * Water: down, else diagonal (as sand), else one step sideways (hash bit 1
//!   picks the first side, the other is the fallback).
//! * Out-of-grid cells count as solid.

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

/// Which family of movers a pass moves.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Granular,
    Liquid,
}

/// A material grid plus the parallel row-parity step rules.
#[derive(Clone)]
pub struct Reference {
    w: u32,
    h: u32,
    cells: Vec<u32>,
}

impl Reference {
    /// Creates an all-empty `w` x `h` grid.
    pub fn new(w: u32, h: u32) -> Reference {
        Reference { w, h, cells: vec![EMPTY; (w * h) as usize] }
    }

    /// Material at `(x, y)`; out-of-grid reads as [`WALL`].
    pub fn get(&self, x: u32, y: u32) -> u32 {
        self.at(i64::from(x), i64::from(y))
    }

    /// Sets the material at `(x, y)`. Panics when out of the grid.
    pub fn set(&mut self, x: u32, y: u32, mat: u32) {
        assert!(x < self.w && y < self.h, "cell out of grid");
        self.cells[(y * self.w + x) as usize] = mat;
    }

    /// Row-major materials.
    pub fn cells(&self) -> &[u32] {
        &self.cells
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
            self.cells[(y as u32 * self.w + x as u32) as usize]
        }
    }

    fn is_empty(&self, x: i64, y: i64) -> bool {
        self.at(x, y) == EMPTY
    }

    /// Target cell of the mover at `(x, y)` this phase, if any.
    fn intent(&self, kind: Kind, phase: u32, tick: u32, seed: u32, x: i64, y: i64) -> Option<(i64, i64)> {
        let mover = if kind == Kind::Granular { SAND } else { WATER };
        if y < 0 || y as u32 % 2 != phase || self.at(x, y) != mover {
            return None;
        }
        if self.is_empty(x, y + 1) {
            return Some((x, y + 1));
        }
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

    /// Source cell that wins the empty cell `(x, y)`, by fixed priority.
    fn winner(&self, kind: Kind, phase: u32, tick: u32, seed: u32, x: i64, y: i64) -> Option<(i64, i64)> {
        [(x, y - 1), (x - 1, y - 1), (x + 1, y - 1), (x - 1, y), (x + 1, y)]
            .into_iter()
            .find(|&(sx, sy)| self.intent(kind, phase, tick, seed, sx, sy) == Some((x, y)))
    }

    /// One double-buffered phase: every output cell is computed from `self`.
    fn phase(&mut self, kind: Kind, phase: u32, tick: u32, seed: u32) {
        let mut out = self.cells.clone();
        for y in 0..i64::from(self.h) {
            for x in 0..i64::from(self.w) {
                let i = (y as u32 * self.w + x as u32) as usize;
                if self.cells[i] == EMPTY {
                    if let Some((sx, sy)) = self.winner(kind, phase, tick, seed, x, y) {
                        out[i] = self.at(sx, sy);
                    }
                } else if let Some((tx, ty)) = self.intent(kind, phase, tick, seed, x, y) {
                    if self.winner(kind, phase, tick, seed, tx, ty) == Some((x, y)) {
                        out[i] = EMPTY;
                    }
                }
            }
        }
        self.cells = out;
    }
}
