//! Part of the physics engine (see mod.rs).

use super::*;

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u32>, // packed cells (material, aux, temperature), row by row, row 0 at the top
    pub frame: u32,      // steps run so far; picks the block offset and the randomness
}

impl World {
    /// An empty world at room temperature with a stone floor.
    pub fn new(width: usize, height: usize) -> Self {
        let mut world = World {
            width,
            height,
            cells: vec![pack(AIR, 0, AMBIENT); width * height],
            frame: 0,
        };
        world.fill(0, height as i32 - 1, width as i32, height as i32, STONE);
        world
    }

    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            None
        } else {
            Some(y as usize * self.width + x as usize)
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u32 {
        self.index(x, y).map_or(STONE, |i| material(self.cells[i]))
    }

    /// Place a fresh cell of material `m` (with its default temperature).
    pub fn set(&mut self, x: i32, y: i32, m: u32) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = pack(m, default_aux(m), default_temp(m));
        }
    }

    /// Fill the rectangle from (x0, y0) up to, but not including, (x1, y1).
    pub fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, m: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, m);
            }
        }
    }

    /// Fill a rectangle, but each cell only with chance `percent`/100.
    pub fn sprinkle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, m: u32, percent: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                if hash3(x as u32, y as u32, m) % 100 < percent {
                    self.set(x, y, m);
                }
            }
        }
    }

    pub fn count(&self, m: u32) -> usize {
        self.cells.iter().filter(|c| material(**c) == m).count()
    }
}

