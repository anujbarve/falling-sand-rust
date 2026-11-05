//! Reusable RGBA pixel buffer that a world is drawn into each frame.

use crate::palette::cell_color;
use sandbox_core::world::World;

/// An RGBA8 pixel buffer, one pixel per world cell, row-major.
pub struct FrameBuffer {
    /// Width in pixels.
    pub w: usize,
    /// Height in pixels.
    pub h: usize,
    /// `w * h * 4` bytes of RGBA data.
    pub pixels: Vec<u8>,
}

impl FrameBuffer {
    /// Allocates a zeroed buffer; this is the only allocation, later draws reuse it.
    pub fn new(w: usize, h: usize) -> FrameBuffer {
        FrameBuffer { w, h, pixels: vec![0; w * h * 4] }
    }

    /// Writes every cell's colour into the buffer without allocating.
    /// Cells outside the buffer's size are ignored; empty cells get alpha 0.
    pub fn draw_world(&mut self, world: &World, seed: u32) {
        let w = self.w.min(world.cells.width());
        let h = self.h.min(world.cells.height());
        for y in 0..h {
            for x in 0..w {
                let cell = world.cells.as_slice()[y * world.cells.width() + x];
                let c = cell_color(cell.mat, x as i32, y as i32, seed);
                let i = (y * self.w + x) * 4;
                self.pixels[i..i + 4].copy_from_slice(&c);
            }
        }
    }
}
