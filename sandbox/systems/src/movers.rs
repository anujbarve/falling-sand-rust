//! Shared engine for the granular and liquid rules: banded row sweeps, hashed randomness
//! and dirty-chunk skipping. Serial and parallel runs share this code, so they match exactly.
//!
//! A tick sweeps horizontal bands of `BAND_ROWS` rows. Phase 1 processes every row of a
//! band except its last (the grid's last row is the exception); bands share no cells, so
//! they can run on separate threads. Phase 2 then processes each band's last row on its
//! own, together with the first row of the band below, in top-to-bottom seam order.
//! Randomness is a hash of `(tick seed, x, y)`, never a stream, so skipping cells or
//! running bands in any order cannot change it.

use rayon::prelude::*;
use sandbox_core::cell::Cell;
use sandbox_core::chunk::Chunks;
use sandbox_core::material::Material;
use sandbox_core::world::World;

use crate::granular::density;

/// Rows per band: two chunk rows, so every band spans at least two chunk rows (except a short last band).
pub(crate) const BAND_ROWS: usize = 32;

/// Maximum number of cells a liquid travels sideways in one tick.
const FLOW: i32 = 4;

/// Which rule set a sweep applies.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Sand: fall, slide diagonally.
    Granular,
    /// Water, oil, acid: fall, slide, flow sideways.
    Liquid,
}

/// Remembers last tick's materials so changed regions (and their neighbours) wake up.
/// A chunk is active if any cell in it or a neighbouring chunk changed since this system last ran.
pub(crate) struct Tracker {
    snapshot: Vec<Material>,
    changed: Chunks,
    active: Chunks,
}

impl Tracker {
    /// Creates an empty tracker; the first `begin` activates everything.
    pub(crate) fn new() -> Tracker {
        Tracker { snapshot: Vec::new(), changed: Chunks::new(0, 0), active: Chunks::new(0, 0) }
    }

    /// Computes the active set: cells moved by the last sweep (marked in `end`) plus cells whose
    /// material was changed from outside since then (diff against the snapshot).
    /// Allocates only on first use or resize.
    fn begin(&mut self, cells: &[Cell], width: usize, height: usize) {
        if self.snapshot.len() != cells.len() {
            self.snapshot = cells.iter().map(|c| c.mat).collect();
            self.changed = Chunks::new(width, height);
            self.active = Chunks::new(width, height);
            self.active.mark_all();
            return;
        }
        for (y, (row, old)) in cells.chunks(width).zip(self.snapshot.chunks(width)).enumerate() {
            for (x, (c, o)) in row.iter().zip(old).enumerate() {
                if c.mat != *o {
                    self.changed.mark(x as i32, y as i32);
                }
            }
        }
        self.active.spread_from(&self.changed);
        self.changed.clear();
    }

    /// Marks every cell the sweep swapped (its `updated` flag is set) and records the
    /// post-sweep materials for the next `begin`.
    fn end(&mut self, cells: &[Cell], width: usize) {
        for (y, (row, old)) in cells.chunks(width).zip(self.snapshot.chunks_mut(width)).enumerate() {
            for (x, (c, o)) in row.iter().zip(old).enumerate() {
                if c.updated {
                    self.changed.mark(x as i32, y as i32);
                }
                *o = c.mat;
            }
        }
    }

    /// Forgets everything so the next `begin` activates all chunks.
    fn reset(&mut self) {
        self.snapshot.clear();
    }
}

/// Mixes the tick seed with two coordinates into 32 well-spread bits.
fn hash(seed: u32, a: u32, b: u32) -> u32 {
    let mut h = seed ^ a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77).rotate_left(13);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^ (h >> 16)
}

/// A fair coin from the hash's top bit.
fn coin(seed: u32, a: u32, b: u32) -> bool {
    hash(seed, a, b) >> 31 == 1
}

/// A contiguous block of whole rows. Cells outside the band do not exist for movement rules.
struct Band<'a> {
    cells: &'a mut [Cell],
    width: usize,
    /// Absolute row of the band's first row.
    y0: usize,
}

impl Band<'_> {
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        let ly = y - self.y0 as i32;
        if x < 0 || x as usize >= self.width || ly < 0 || ly as usize >= self.cells.len() / self.width {
            None
        } else {
            Some(ly as usize * self.width + x as usize)
        }
    }

    /// True if `mover` may enter `(x, y)`: in band, not updated, empty or strictly lighter.
    fn can_enter(&self, mover: Material, x: i32, y: i32) -> bool {
        match (self.idx(x, y), density(mover)) {
            (Some(i), Some(dm)) => {
                let c = &self.cells[i];
                !c.updated && density(c.mat).is_some_and(|dt| dt < dm)
            }
            _ => false,
        }
    }

    fn swap(&mut self, a: (i32, i32), b: (i32, i32)) {
        let (ia, ib) = (self.idx(a.0, a.1).unwrap(), self.idx(b.0, b.1).unwrap());
        self.cells.swap(ia, ib);
        self.cells[ia].updated = true;
        self.cells[ib].updated = true;
    }
}

fn is_liquid(m: Material) -> bool {
    matches!(m, Material::Water | Material::Oil | Material::Acid)
}

/// Sweeps local rows `rows` of `band` bottom to top. `active` of `None` visits every chunk.
fn sweep(band: &mut Band, rows: std::ops::Range<usize>, kind: Kind, seed: u32, active: Option<&Chunks>) {
    let width = band.width as i32;
    let chunk_cols = band.width.div_ceil(16);
    for ly in rows.rev() {
        let y = (band.y0 + ly) as i32;
        let left_first = coin(seed, y as u32, 0);
        for ci in 0..chunk_cols {
            let cx = if left_first { ci } else { chunk_cols - 1 - ci };
            if active.is_some_and(|a| !a.is_active(cx, y as usize / 16)) {
                continue;
            }
            for i in 0..16 {
                let off = if left_first { i } else { 15 - i };
                let x = (cx * 16 + off) as i32;
                if x >= width {
                    continue;
                }
                let c = band.cells[band.idx(x, y).unwrap()];
                let wanted = match kind {
                    Kind::Granular => c.mat == Material::Sand,
                    Kind::Liquid => is_liquid(c.mat),
                };
                if c.updated || !wanted {
                    continue;
                }
                if band.can_enter(c.mat, x, y + 1) {
                    band.swap((x, y), (x, y + 1));
                    continue;
                }
                let dir = if coin(seed, x as u32, y as u32 + 1) { -1 } else { 1 };
                if let Some(dx) = [dir, -dir].into_iter().find(|&dx| band.can_enter(c.mat, x + dx, y + 1)) {
                    band.swap((x, y), (x + dx, y + 1));
                    continue;
                }
                if kind == Kind::Liquid {
                    for dx in [dir, -dir] {
                        let mut reach = 0;
                        while reach < FLOW
                            && band
                                .idx(x + dx * (reach + 1), y)
                                .is_some_and(|j| band.cells[j].mat == Material::Empty && !band.cells[j].updated)
                        {
                            reach += 1;
                        }
                        if reach > 0 {
                            band.swap((x, y), (x + dx * reach, y));
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Runs one granular or liquid sweep over the whole world; bands run on rayon when `parallel`.
/// With `use_chunks` false every chunk is visited (the reference path).
pub(crate) fn run(w: &mut World, kind: Kind, seed: u32, tracker: &mut Tracker, use_chunks: bool, parallel: bool) {
    let (width, height) = (w.cells.width(), w.cells.height());
    if width == 0 || height == 0 {
        return;
    }
    let cells = w.cells.as_mut_slice();
    if use_chunks {
        tracker.begin(cells, width, height);
    } else {
        tracker.reset();
    }
    let active = if use_chunks { Some(&tracker.active) } else { None };
    let bands = height.div_ceil(BAND_ROWS);
    let phase1 = |(k, s): (usize, &mut [Cell])| {
        let rows = s.len() / width;
        // Every band but the last leaves its bottom row for the seam pass.
        let end = if k + 1 == bands { rows } else { rows - 1 };
        sweep(&mut Band { cells: s, width, y0: k * BAND_ROWS }, 0..end, kind, seed, active);
    };
    if parallel {
        cells.par_chunks_mut(BAND_ROWS * width).enumerate().for_each(phase1);
    } else {
        cells.chunks_mut(BAND_ROWS * width).enumerate().for_each(phase1);
    }
    for k in 0..bands - 1 {
        let y = (k + 1) * BAND_ROWS - 1;
        let mut band = Band { cells: &mut cells[y * width..(y + 2) * width], width, y0: y };
        sweep(&mut band, 0..1, kind, seed, active);
    }
    if use_chunks {
        tracker.end(cells, width);
    }
}
