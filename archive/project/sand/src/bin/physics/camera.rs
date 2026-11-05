//! Pan and zoom for the viewer: which part of the grid is on screen, and how
//! screen pixels map to cells.
//!
//! Coordinates: `screen` is the drawable area under the toolbar, i.e.
//! `(screen_width(), screen_height() - BAR_HEIGHT)`. Points are window pixels,
//! so the grid's top-left is at `(x, BAR_HEIGHT + ...)`. `center` is in cells.

use crate::ui::BAR_HEIGHT;
use macroquad::prelude::*;

pub const MIN_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub zoom: f32,
    pub center: Vec2, // in cells
}

/// Pixels per cell at zoom 1: the largest scale at which the whole grid fits.
fn fit_scale(grid: (usize, usize), screen: (f32, f32)) -> f32 {
    let (gw, gh) = (grid.0.max(1) as f32, grid.1.max(1) as f32);
    (screen.0 / gw).min(screen.1 / gh).max(1e-6)
}

/// Does `cells` at `px_per_cell` fit in `view_px`? (A half-pixel of slack absorbs rounding.)
fn fits(cells: usize, view_px: f32, px_per_cell: f32) -> bool {
    cells.max(1) as f32 * px_per_cell <= view_px + 0.5
}

/// Clamp one axis of the centre: centred when the whole axis is in view, else keep the view inside.
fn clamp_axis(c: f32, cells: usize, view_px: f32, px_per_cell: f32) -> f32 {
    let len = cells.max(1) as f32;
    if fits(cells, view_px, px_per_cell) {
        return len * 0.5;
    }
    let half = view_px / px_per_cell * 0.5;
    c.clamp(half, len - half)
}

impl Camera {
    /// The whole grid, letterboxed: zoom 1 is exactly the old `world_rect` view.
    pub fn fit(grid: (usize, usize), _screen: (f32, f32)) -> Camera {
        Camera {
            zoom: MIN_ZOOM,
            center: vec2(grid.0.max(1) as f32 * 0.5, grid.1.max(1) as f32 * 0.5),
        }
    }

    /// Pixels per cell at the current zoom.
    fn scale(&self, grid: (usize, usize), screen: (f32, f32)) -> f32 {
        fit_scale(grid, screen) * self.zoom
    }

    /// Where the grid is drawn, in window pixels.
    pub fn dest_rect(&self, grid: (usize, usize), screen: (f32, f32)) -> Rect {
        let (sw, sh) = screen;
        let s = self.scale(grid, screen);
        let (w, h) = (grid.0 as f32 * s, grid.1 as f32 * s);
        let x = if fits(grid.0, sw, s) { (sw - w) * 0.5 } else { sw * 0.5 - self.center.x * s };
        let y = if fits(grid.1, sh, s) {
            BAR_HEIGHT + (sh - h) * 0.5
        } else {
            BAR_HEIGHT + sh * 0.5 - self.center.y * s
        };
        Rect::new(x, y, w, h)
    }

    /// The cell under window point `p`, or `None` if `p` is off the grid.
    pub fn screen_to_cell(&self, p: Vec2, grid: (usize, usize), screen: (f32, f32)) -> Option<(i32, i32)> {
        let s = self.scale(grid, screen);
        let d = self.dest_rect(grid, screen);
        let cx = ((p.x - d.x) / s).floor();
        let cy = ((p.y - d.y) / s).floor();
        if cx < 0.0 || cy < 0.0 || cx >= grid.0 as f32 || cy >= grid.1 as f32 {
            return None;
        }
        Some((cx as i32, cy as i32))
    }

    /// Centre of `cell` in window pixels.
    pub fn cell_to_screen(&self, cell: (i32, i32), grid: (usize, usize), screen: (f32, f32)) -> Vec2 {
        let s = self.scale(grid, screen);
        let d = self.dest_rect(grid, screen);
        vec2(d.x + (cell.0 as f32 + 0.5) * s, d.y + (cell.1 as f32 + 0.5) * s)
    }

    /// Cells that are at least partly on screen: `x0, y0, x1, y1`, `x1`/`y1` exclusive, inside the grid.
    pub fn visible_cells(&self, grid: (usize, usize), screen: (f32, f32)) -> (i32, i32, i32, i32) {
        let s = self.scale(grid, screen);
        let d = self.dest_rect(grid, screen);
        let (gw, gh) = (grid.0 as f32, grid.1 as f32);
        let (sw, sh) = screen;
        let x0 = ((0.0 - d.x) / s).floor().clamp(0.0, gw);
        let x1 = ((sw - d.x) / s).ceil().clamp(x0, gw);
        let y0 = ((BAR_HEIGHT - d.y) / s).floor().clamp(0.0, gh);
        let y1 = ((BAR_HEIGHT + sh - d.y) / s).ceil().clamp(y0, gh);
        (x0 as i32, y0 as i32, x1 as i32, y1 as i32)
    }

    /// Move the view by `delta` window pixels (the grid follows the pointer).
    pub fn pan(&mut self, delta: Vec2, grid: (usize, usize), screen: (f32, f32)) {
        let s = self.scale(grid, screen);
        self.center -= delta / s;
        self.clamp(grid, screen);
    }

    /// Keep the zoom in range and the view inside the grid (centred on an axis the view is wider than).
    pub fn clamp(&mut self, grid: (usize, usize), screen: (f32, f32)) {
        self.zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let s = self.scale(grid, screen);
        self.center.x = clamp_axis(self.center.x, grid.0, screen.0, s);
        self.center.y = clamp_axis(self.center.y, grid.1, screen.1, s);
    }

    /// Zoom by `factor` keeping the cell under window point `p` fixed.
    pub fn zoom_at(&mut self, p: Vec2, factor: f32, grid: (usize, usize), screen: (f32, f32)) {
        // Screen centre of the view (below the toolbar); the view centre is `center`.
        let view_c = vec2(screen.0 * 0.5, BAR_HEIGHT + screen.1 * 0.5);
        let anchor = self.center + (p - view_c) / self.scale(grid, screen);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = anchor - (p - view_c) / self.scale(grid, screen);
        self.clamp(grid, screen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_at_keeps_the_cell_under_the_pointer() {
        let grid = (640, 360);
        let screen = (1280.0, 760.0);
        let mut cam = Camera::fit(grid, screen);
        let p = vec2(400.0, 300.0);
        let before = cam.screen_to_cell(p, grid, screen);
        cam.zoom_at(p, 4.0, grid, screen);
        assert_eq!(cam.screen_to_cell(p, grid, screen), before);
    }

    #[test]
    fn clamp_keeps_view_inside_grid() {
        let grid = (640, 360);
        let screen = (1280.0, 760.0);
        let mut cam = Camera { zoom: 8.0, center: vec2(0.0, 0.0) };
        cam.clamp(grid, screen);
        let (x0, y0, x1, y1) = cam.visible_cells(grid, screen);
        assert!(x0 >= 0 && y0 >= 0 && x1 <= 640 && y1 <= 360);
    }

    #[test]
    fn fit_matches_the_old_letterboxed_rect() {
        let grid = (640, 360);
        let screen = (1280.0, 760.0);
        let r = Camera::fit(grid, screen).dest_rect(grid, screen);
        assert_eq!(r.w, 1280.0);
        assert_eq!(r.h, 720.0);
    }

    #[test]
    fn screen_to_cell_is_none_outside_the_grid() {
        let grid = (640, 360);
        let screen = (1280.0, 760.0);
        assert_eq!(Camera::fit(grid, screen).screen_to_cell(vec2(5.0, 5.0), grid, screen), None);
    }

    /// Deterministic generator (no crates): a linear congruential generator.
    struct Lcg(u64);

    impl Lcg {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0
        }

        /// Uniform in `[0, 1)`.
        fn unit(&mut self) -> f32 {
            (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
        }
    }

    #[test]
    fn camera_round_trip() {
        // Screens are the area under the toolbar, as the viewer passes them.
        let grids = [(640usize, 360usize), (301, 157), (1, 1), (64, 900)];
        let screens = [(1280.0f32, 720.0f32), (1000.0, 500.0), (333.0, 777.0)];
        let mut rng = Lcg(0x5eed_cafe);
        for _ in 0..200 {
            let grid = grids[(rng.next_u64() % grids.len() as u64) as usize];
            let screen = screens[(rng.next_u64() % screens.len() as u64) as usize];
            let mut cam = Camera {
                zoom: MIN_ZOOM + rng.unit() * (MAX_ZOOM - MIN_ZOOM),
                center: vec2(rng.unit() * grid.0 as f32, rng.unit() * grid.1 as f32),
            };
            cam.clamp(grid, screen);
            let (x0, y0, x1, y1) = cam.visible_cells(grid, screen);
            assert!(x0 < x1 && y0 < y1, "nothing visible for {cam:?} on {grid:?}");
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = cam.cell_to_screen((x, y), grid, screen);
                    assert_eq!(
                        cam.screen_to_cell(p, grid, screen),
                        Some((x, y)),
                        "cell ({x},{y}) at {p:?} maps back wrongly; camera {cam:?}, grid {grid:?}, screen {screen:?}"
                    );
                }
            }
        }
    }
}
