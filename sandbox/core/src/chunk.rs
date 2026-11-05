//! Dirty-chunk map: which 16x16 blocks of the grid may still change.

/// Side of one chunk in cells.
pub const CHUNK_SIZE: usize = 16;

/// One flag per chunk. Invariant: `active.len() == w * h`, row-major by chunk.
pub struct Chunks {
    /// Chunk side in cells (always `CHUNK_SIZE`).
    pub size: usize,
    /// Activity flag per chunk, row-major.
    pub active: Vec<bool>,
    /// Number of chunk columns (partial edge chunks count).
    pub w: usize,
    /// Number of chunk rows (partial edge chunks count).
    pub h: usize,
}

impl Chunks {
    /// Creates an all-inactive map for a grid of `width` by `height` cells.
    pub fn new(width: usize, height: usize) -> Chunks {
        let size = CHUNK_SIZE;
        let (w, h) = (width.div_ceil(size), height.div_ceil(size));
        Chunks { size, active: vec![false; w * h], w, h }
    }

    /// Activates the chunk containing cell `(x, y)` (only that chunk); cells beyond the last chunk are ignored.
    pub fn mark(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let (cx, cy) = (x as usize / self.size, y as usize / self.size);
        if cx < self.w && cy < self.h {
            self.active[cy * self.w + cx] = true;
        }
    }

    /// True if chunk `(cx, cy)` is active; out-of-range chunks are inactive.
    pub fn is_active(&self, cx: usize, cy: usize) -> bool {
        cx < self.w && cy < self.h && self.active[cy * self.w + cx]
    }

    /// Deactivates every chunk.
    pub fn clear(&mut self) {
        self.active.fill(false);
    }

    /// Activates every chunk.
    pub fn mark_all(&mut self) {
        self.active.fill(true);
    }

    /// Replaces `self` with `src` grown by one chunk in all 8 directions. Both must have the same size.
    /// Allocation-free.
    pub fn spread_from(&mut self, src: &Chunks) {
        debug_assert_eq!((self.w, self.h), (src.w, src.h));
        for cy in 0..self.h {
            for cx in 0..self.w {
                let mut on = false;
                for ny in cy.saturating_sub(1)..=(cy + 1).min(self.h - 1) {
                    for nx in cx.saturating_sub(1)..=(cx + 1).min(self.w - 1) {
                        on |= src.active[ny * src.w + nx];
                    }
                }
                self.active[cy * self.w + cx] = on;
            }
        }
    }
}
