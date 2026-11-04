// A falling-sand simulation: the world is a grid of cells, and every frame
// each cell follows a few simple rules. Complex-looking behaviour (piles,
// splashing, flowing) emerges from those rules.

use macroquad::prelude::*;

// Grid size in cells, and how many screen pixels each cell takes up.
const W: usize = 640;
const H: usize = 360;
const SCALE: f32 = 4.0;
const BRUSH: i32 = 3;
const GRAVITY: f32 = 0.15; // added to a flying particle's downward speed each step
const MAX_PARTICLES: usize = 30_000;
const POWDER_BLAST: i32 = 7; // explosion radius of one gunpowder grain
const BOMB_BLAST: i32 = 26; // explosion radius of the E key bomb

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

/// What a single cell contains. `Copy` lets us pass cells around by value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Empty,
    Sand,
    Water,
    Stone,
    Wood,
    Gunpowder,
    // Variants can carry data. `life` counts down every step; at 0 the cell
    // turns into something else.
    Fire { life: u8 },
    Smoke { life: u8 },
}

impl Cell {
    /// Base colour as RGB bytes.
    fn color(self) -> [u8; 3] {
        match self {
            Cell::Empty => [20, 20, 28],
            Cell::Sand => [220, 190, 110],
            Cell::Water => [50, 110, 220],
            Cell::Stone => [110, 110, 115],
            Cell::Wood => [110, 70, 35],
            Cell::Gunpowder => [60, 60, 60],
            // Young fire is yellow, dying fire is deep red.
            Cell::Fire { life } => [255, 40 + life.min(70) * 3, 20],
            // Smoke fades into the background as it dies.
            Cell::Smoke { life } => {
                let v = 20 + life / 2;
                [v, v, v + 4]
            }
        }
    }

    /// Short name for the on-screen brush label.
    fn name(self) -> &'static str {
        match self {
            Cell::Empty => "erase",
            Cell::Sand => "sand",
            Cell::Water => "water",
            Cell::Stone => "stone",
            Cell::Wood => "wood",
            Cell::Gunpowder => "gunpowder",
            Cell::Fire { .. } => "fire", // `..` = ignore the fields
            Cell::Smoke { .. } => "smoke",
        }
    }

    /// Can a moving particle push into this cell?
    fn is_displaceable_by(self, mover: Cell) -> bool {
        match (mover, self) {
            (_, Cell::Empty) => true,
            // Powders sink through water.
            (Cell::Sand | Cell::Gunpowder, Cell::Water) => true,
            // Falling things push smoke out of the way.
            (Cell::Sand | Cell::Gunpowder | Cell::Water, Cell::Smoke { .. }) => true,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

/// A cell that has been blasted off the grid and is flying freely.
/// Unlike grid cells, its position is a float and it has a velocity.
#[derive(Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    cell: Cell, // what it turns back into when it lands
}

/// The whole simulation state.
struct Grid {
    cells: Vec<Cell>, // W * H cells, row by row, row 0 at the top
    moved: Vec<bool>, // did this cell already move this frame?
    rng: u32,         // state for our tiny random number generator
    frame: u32,
    particles: Vec<Particle>,
    pending: Vec<(i32, i32)>, // explosions to set off next step
}

impl Grid {
    fn new() -> Self {
        let mut grid = Grid {
            cells: vec![Cell::Empty; W * H],
            moved: vec![false; W * H],
            rng: 0x9E37_79B9,
            frame: 0,
            particles: Vec::new(),
            pending: Vec::new(),
        };
        // A stone floor and a little ledge so there is something to pile on.
        for x in 0..W {
            grid.set(x as i32, (H - 1) as i32, Cell::Stone);
        }
        for x in 80..160 {
            grid.set(x, 120, Cell::Stone);
        }
        grid
    }

    /// Convert (x, y) into an index into `cells`, or None if off the grid.
    fn index(x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            None
        } else {
            Some(y as usize * W + x as usize)
        }
    }

    fn get(&self, x: i32, y: i32) -> Cell {
        match Self::index(x, y) {
            Some(i) => self.cells[i],
            None => Cell::Stone, // treat the outside world as solid wall
        }
    }

    fn set(&mut self, x: i32, y: i32, cell: Cell) {
        if let Some(i) = Self::index(x, y) {
            self.cells[i] = cell;
        }
    }

    /// xorshift: a fast, simple pseudo-random generator.
    fn random(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn coin_flip(&mut self) -> bool {
        self.random() & 1 == 0
    }

    /// Try to move the cell at (x, y) to (x + dx, y + dy). Returns true on success.
    fn try_move(&mut self, x: i32, y: i32, dx: i32, dy: i32) -> bool {
        let (nx, ny) = (x + dx, y + dy);
        let mover = self.get(x, y);
        let target = self.get(nx, ny);
        if !target.is_displaceable_by(mover) {
            return false;
        }
        // Both indices are valid: get() returned a non-wall cell for the target.
        let (Some(a), Some(b)) = (Self::index(x, y), Self::index(nx, ny)) else {
            return false;
        };
        self.cells.swap(a, b);
        self.moved[b] = true;
        true
    }

    /// Advance the simulation by one step.
    fn step(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.moved.fill(false);

        // Explosions triggered last step. Taking the list out of `self` lets
        // explode() (which needs `&mut self`) push new ones for the next step.
        let pending = std::mem::take(&mut self.pending);
        for (x, y) in pending {
            self.explode(x, y, POWDER_BLAST);
        }
        self.update_particles();

        // Bottom to top, so a falling grain is not processed twice.
        // Flip the left/right scan direction every frame to avoid a bias.
        let left_to_right = self.frame % 2 == 0;
        for y in (0..H as i32).rev() {
            for i in 0..W as i32 {
                let x = if left_to_right { i } else { W as i32 - 1 - i };
                let idx = y as usize * W + x as usize;
                if self.moved[idx] {
                    continue;
                }
                match self.cells[idx] {
                    Cell::Sand | Cell::Gunpowder => self.update_sand(x, y),
                    Cell::Water => self.update_water(x, y),
                    Cell::Fire { life } => self.update_fire(x, y, life),
                    Cell::Smoke { life } => self.update_smoke(x, y, life),
                    Cell::Empty | Cell::Stone | Cell::Wood => {}
                }
            }
        }
    }

    fn update_sand(&mut self, x: i32, y: i32) {
        if self.try_move(x, y, 0, 1) {
            return; // fell straight down
        }
        // Blocked: slide down-left or down-right (random order).
        let dir = if self.coin_flip() { 1 } else { -1 };
        if !self.try_move(x, y, dir, 1) {
            self.try_move(x, y, -dir, 1);
        }
    }

    fn update_water(&mut self, x: i32, y: i32) {
        if self.try_move(x, y, 0, 1) {
            return;
        }
        let dir = if self.coin_flip() { 1 } else { -1 };
        if self.try_move(x, y, dir, 1) || self.try_move(x, y, -dir, 1) {
            return;
        }
        // Can't fall at all: flow sideways. This is what makes water level out.
        if !self.try_move(x, y, dir, 0) {
            self.try_move(x, y, -dir, 0);
        }
    }

    /// A random number in lo..hi (hi excluded).
    fn random_range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.random() % (hi - lo)
    }

    fn update_fire(&mut self, x: i32, y: i32, life: u8) {
        // 1. Burned out? Become smoke (sometimes nothing, so smoke isn't endless).
        if life == 0 {
            let next = if self.coin_flip() {
                Cell::Smoke {
                    life: self.random_range(60, 120) as u8,
                }
            } else {
                Cell::Empty
            };
            self.set(x, y, next);
            return;
        }

        // 2. Look at all 8 neighbours.
        for (dx, dy) in NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            match self.get(nx, ny) {
                // Water puts the fire out; both become steam-like smoke.
                Cell::Water => {
                    self.set(x, y, Cell::Smoke { life: 80 });
                    self.set(nx, ny, Cell::Smoke { life: 80 });
                    return;
                }
                // Gunpowder next to fire always goes off (next step).
                Cell::Gunpowder => {
                    self.set(nx, ny, Cell::Fire { life: 4 });
                    self.pending.push((nx, ny));
                }
                // Wood catches fire with a small chance each step.
                Cell::Wood if self.random() % 100 < 4 => {
                    let new_life = self.random_range(60, 140) as u8;
                    self.set(nx, ny, Cell::Fire { life: new_life });
                }
                _ => {}
            }
        }

        // 3. Age by one step, then flicker upward like a flame.
        self.set(x, y, Cell::Fire { life: life - 1 });
        if self.random() % 3 == 0 {
            let dir = self.random_range(0, 3) as i32 - 1; // -1, 0 or 1
            self.try_move(x, y, dir, -1);
        }
    }

    fn update_smoke(&mut self, x: i32, y: i32, life: u8) {
        if life == 0 {
            self.set(x, y, Cell::Empty);
            return;
        }
        self.set(x, y, Cell::Smoke { life: life - 1 });

        // Smoke is the opposite of sand: it rises, then drifts sideways.
        let dir = if self.coin_flip() { 1 } else { -1 };
        if self.try_move(x, y, 0, -1) || self.try_move(x, y, dir, -1) {
            return;
        }
        self.try_move(x, y, dir, 0);
    }

    /// Blow up everything within `radius` of (cx, cy).
    fn explode(&mut self, cx: i32, cy: i32, radius: i32) {
        let r = radius as f32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist > r {
                    continue; // outside the blast circle
                }
                let (x, y) = (cx + dx, cy + dy);
                let Some(i) = Self::index(x, y) else { continue };
                if y == H as i32 - 1 {
                    continue; // the bottom row is an indestructible floor
                }

                // 1.0 at the centre, 0.0 at the edge.
                let strength = 1.0 - dist / r;
                let core = strength > 0.5;

                match self.cells[i] {
                    // The middle of the blast fills with fire.
                    Cell::Empty => {
                        if core && self.random() % 3 == 0 {
                            let life = self.random_range(10, 30) as u8;
                            self.cells[i] = Cell::Fire { life };
                        }
                    }
                    // Stone only breaks in the core, into flying rubble.
                    Cell::Stone => {
                        if core {
                            self.fling(x, y, dx, dy, strength, Cell::Sand);
                        }
                    }
                    // Other gunpowder in range joins the chain reaction.
                    Cell::Gunpowder => {
                        self.cells[i] = Cell::Fire { life: 4 };
                        self.pending.push((x, y));
                    }
                    // Wood near the centre ignites as it flies.
                    Cell::Wood => {
                        let debris = if core {
                            Cell::Fire { life: 80 }
                        } else {
                            Cell::Wood
                        };
                        self.fling(x, y, dx, dy, strength, debris);
                    }
                    // Water near the centre boils into steam, the rest splashes.
                    Cell::Water if core => self.cells[i] = Cell::Smoke { life: 60 },
                    // Everything else gets thrown, more likely the closer it is.
                    other => {
                        if (self.random() % 100) as f32 / 100.0 < strength + 0.3 {
                            self.fling(x, y, dx, dy, strength, other);
                        }
                    }
                }
            }
        }
    }

    /// Remove the cell at (x, y) from the grid and launch it as a particle,
    /// flying away from the explosion centre.
    fn fling(&mut self, x: i32, y: i32, dx: i32, dy: i32, strength: f32, cell: Cell) {
        self.set(x, y, Cell::Empty);
        if self.particles.len() >= MAX_PARTICLES {
            return;
        }
        // Direction away from the centre (a unit vector), plus some randomness.
        let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
        let jitter = (self.random() % 100) as f32 / 100.0 - 0.5;
        let speed = 1.0 + strength * 4.0 + jitter;
        self.particles.push(Particle {
            x: x as f32 + 0.5,
            y: y as f32 + 0.5,
            vx: dx as f32 / len * speed + jitter,
            vy: dy as f32 / len * speed - 1.0, // slight upward kick
            cell,
        });
    }

    /// Move every flying particle; when one hits something, it lands back on the grid.
    fn update_particles(&mut self) {
        // We need to read and write the grid while changing the particle list.
        // Borrowing both through `self` at once isn't allowed, so take the list out first.
        let mut particles = std::mem::take(&mut self.particles);

        particles.retain_mut(|p| {
            p.vy += GRAVITY;
            // Move in small sub-steps so fast particles can't skip through thin walls.
            let steps = p.vx.abs().max(p.vy.abs()).ceil().max(1.0) as i32;
            let (sx, sy) = (p.vx / steps as f32, p.vy / steps as f32);
            for _ in 0..steps {
                let (nx, ny) = (p.x + sx, p.y + sy);
                let (ix, iy) = (nx.floor() as i32, ny.floor() as i32);
                if ix < 0 || ix >= W as i32 || iy >= H as i32 {
                    return false; // left the screen: remove it
                }
                // Above the top edge there is nothing to hit; keep flying.
                if iy >= 0 && self.get(ix, iy) != Cell::Empty {
                    // Hit something: land in the last free spot.
                    let (lx, ly) = (p.x.floor() as i32, p.y.floor() as i32);
                    if self.get(lx, ly) == Cell::Empty {
                        self.set(lx, ly, p.cell);
                    }
                    return false; // remove it from the flying list
                }
                p.x = nx;
                p.y = ny;
            }
            true // keep it: still flying
        });

        self.particles = particles;
    }
}

/// Offsets to the 8 cells surrounding a cell.
const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

// ---------------------------------------------------------------------------
// Window, input, drawing (macroquad)
// ---------------------------------------------------------------------------

fn window_conf() -> Conf {
    Conf {
        window_title: "sand".to_owned(),
        window_width: (W as f32 * SCALE) as i32,
        window_height: (H as f32 * SCALE) as i32,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut grid = Grid::new();
    let mut brush = Cell::Sand;

    // A W x H image on the CPU, and a matching texture on the GPU.
    let mut image = Image::gen_image_color(W as u16, H as u16, BLACK);
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest); // crisp square cells when scaled up

    loop {
        pick_brush(&mut brush);
        if is_key_pressed(KeyCode::E) {
            let (mx, my) = mouse_position();
            grid.explode((mx / SCALE) as i32, (my / SCALE) as i32, BOMB_BLAST);
        }
        paint(&mut grid, brush);
        grid.step();
        draw_grid(&grid, &mut image);

        texture.update(&image); // upload pixels to the GPU
        draw_texture_ex(
            &texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(W as f32 * SCALE, H as f32 * SCALE)),
                ..Default::default()
            },
        );
        draw_text(
            &format!(
                "1 sand 2 water 3 stone 4 erase 5 wood 6 fire 7 gunpowder | E bomb | brush: {} | {} fps",
                brush.name(),
                get_fps()
            ),
            10.0,
            24.0,
            24.0,
            WHITE,
        );

        next_frame().await; // hand control back to macroquad until the next frame
    }
}

/// Keys 1-4 choose what the left mouse button paints.
fn pick_brush(brush: &mut Cell) {
    let choices = [
        (KeyCode::Key1, Cell::Sand),
        (KeyCode::Key2, Cell::Water),
        (KeyCode::Key3, Cell::Stone),
        (KeyCode::Key4, Cell::Empty),
        (KeyCode::Key5, Cell::Wood),
        (KeyCode::Key6, Cell::Fire { life: 0 }), // life is randomised when painting
        (KeyCode::Key7, Cell::Gunpowder),
    ];
    for (key, cell) in choices {
        if is_key_pressed(key) {
            *brush = cell;
        }
    }
}

/// Left mouse paints the selected material, right mouse erases.
fn paint(grid: &mut Grid, brush: Cell) {
    let cell = if is_mouse_button_down(MouseButton::Left) {
        brush
    } else if is_mouse_button_down(MouseButton::Right) {
        Cell::Empty
    } else {
        return;
    };
    // Mouse position is in window pixels, (0, 0) at the top-left.
    let (mx, my) = mouse_position();
    let (cx, cy) = ((mx / SCALE) as i32, (my / SCALE) as i32);

    let sprinkle = matches!(
        cell,
        Cell::Sand | Cell::Water | Cell::Fire { .. } | Cell::Gunpowder
    );
    for dy in -BRUSH..=BRUSH {
        for dx in -BRUSH..=BRUSH {
            if dx * dx + dy * dy > BRUSH * BRUSH {
                continue; // round brush
            }
            // Sprinkle sand/water instead of filling solid blocks.
            if sprinkle && grid.random() % 3 != 0 {
                continue;
            }
            // Each fire particle gets its own random lifetime.
            let placed = match cell {
                Cell::Fire { .. } => Cell::Fire {
                    life: grid.random_range(40, 100) as u8,
                },
                other => other,
            };
            grid.set(cx + dx, cy + dy, placed);
        }
    }
}

/// Copy every cell's colour into the image's RGBA bytes.
fn draw_grid(grid: &Grid, image: &mut Image) {
    for (i, cell) in grid.cells.iter().enumerate() {
        let [r, g, b] = cell.color();
        // Small per-cell brightness variation so sand looks grainy.
        let shade = if *cell == Cell::Empty {
            0
        } else {
            ((i.wrapping_mul(2654435761)) >> 28) as u8 % 24
        };
        let p = i * 4;
        image.bytes[p] = r.saturating_sub(shade);
        image.bytes[p + 1] = g.saturating_sub(shade);
        image.bytes[p + 2] = b.saturating_sub(shade);
        image.bytes[p + 3] = 255;
    }

    // Flying particles are drawn on top of the grid.
    for particle in &grid.particles {
        let (x, y) = (particle.x as i32, particle.y as i32);
        if let Some(i) = Grid::index(x, y) {
            let [r, g, b] = particle.cell.color();
            image.bytes[i * 4..i * 4 + 3].copy_from_slice(&[r, g, b]);
        }
    }
}

// Run with `cargo test`. These run the simulation without opening a window.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gunpowder_chain_reaction_runs_without_crashing() {
        let mut grid = Grid::new();
        // A block of gunpowder on the ledge, lit from one corner.
        for y in 100..120 {
            for x in 90..150 {
                grid.set(x, y, Cell::Gunpowder);
            }
        }
        grid.set(89, 119, Cell::Fire { life: 50 });

        let mut max_particles = 0;
        for _ in 0..600 {
            grid.step();
            max_particles = max_particles.max(grid.particles.len());
        }
        let powder_left = grid.cells.iter().filter(|c| **c == Cell::Gunpowder).count();
        assert_eq!(powder_left, 0, "the whole block should have gone off");
        assert!(max_particles > 100, "explosions should throw debris");
    }

    #[test]
    fn bomb_at_edges_is_safe() {
        let mut grid = Grid::new();
        for (x, y) in [
            (0, 0),
            (W as i32 - 1, 0),
            (0, H as i32 - 1),
            (-50, -50),
            (160, 90),
        ] {
            grid.explode(x, y, BOMB_BLAST);
        }
        for _ in 0..300 {
            grid.step();
        }
    }
}
