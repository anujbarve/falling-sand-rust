//! Dense row-major 2D grid with signed, bounds-checked access.

/// Row-major grid. Invariant: `data.len() == w * h`.
pub struct Grid<T> {
    w: usize,
    h: usize,
    data: Vec<T>,
}

impl<T: Clone> Grid<T> {
    /// Creates a `w` by `h` grid with every cell set to `fill`.
    pub fn new(w: usize, h: usize, fill: T) -> Grid<T> {
        Grid { w, h, data: vec![fill; w * h] }
    }

    /// Width in cells.
    pub fn width(&self) -> usize {
        self.w
    }

    /// Height in cells.
    pub fn height(&self) -> usize {
        self.h
    }

    /// Flat index of `(x, y)`, or `None` if out of bounds.
    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            None
        } else {
            Some(y as usize * self.w + x as usize)
        }
    }

    /// Cell at `(x, y)`, or `None` if out of bounds.
    pub fn get(&self, x: i32, y: i32) -> Option<&T> {
        self.index(x, y).map(|i| &self.data[i])
    }

    /// Mutable cell at `(x, y)`, or `None` if out of bounds.
    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut T> {
        self.index(x, y).map(move |i| &mut self.data[i])
    }

    /// Writes `v` at `(x, y)`; returns false (and writes nothing) if out of bounds.
    pub fn set(&mut self, x: i32, y: i32, v: T) -> bool {
        match self.get_mut(x, y) {
            Some(c) => {
                *c = v;
                true
            }
            None => false,
        }
    }

    /// All cells in row-major order.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    /// All cells in row-major order, mutable.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }
}
