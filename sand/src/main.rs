// A falling-sand simulation: the world is a grid of cells, and every frame
// each cell follows a few simple rules. Complex-looking behaviour (piles,
// splashing, flowing) emerges from those rules.

use macroquad::prelude::*;

// Grid size in cells, and how many screen pixels each cell takes up.
const W: usize = 640;
const H: usize = 360;
const SCALE: f32 = 2.0; // 2 screen pixels per cell: small grains, 1280x720 window
const GRAVITY: f32 = 0.15; // added to a flying particle's downward speed each step
const MAX_PARTICLES: usize = 30_000;
const POWDER_BLAST: i32 = 8; // explosion radius of one gunpowder grain
const BOMB_BLAST: i32 = 40; // explosion radius of the E key bomb

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
    Lava,
    Acid,
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
            Cell::Lava => [255, 95, 20],
            Cell::Acid => [130, 240, 50],
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
            Cell::Lava => "lava",
            Cell::Acid => "acid",
            Cell::Fire { .. } => "fire", // `..` = ignore the fields
            Cell::Smoke { .. } => "smoke",
        }
    }

    /// Can a moving particle push into this cell?
    fn is_displaceable_by(self, mover: Cell) -> bool {
        match (mover, self) {
            (_, Cell::Empty) => true,
            // Powders sink through water.
            (Cell::Sand | Cell::Gunpowder, Cell::Water | Cell::Acid | Cell::Lava) => true,
            // Falling things push smoke out of the way.
            (
                Cell::Sand | Cell::Gunpowder | Cell::Water | Cell::Lava | Cell::Acid,
                Cell::Smoke { .. },
            ) => true,
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
    emitters: Vec<Emitter>,   // taps that keep pouring material in
}

/// A tap: every step, drops `cell` into a `w`-wide strip at (x, y),
/// each spot with a 1-in-`rate` chance. Runs out after `steps_left` steps.
#[derive(Clone, Copy)]
struct Emitter {
    x: i32,
    y: i32,
    w: i32,
    cell: Cell,
    rate: u32,
    steps_left: u32,
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
            emitters: Vec::new(),
        };
        // A stone floor along the bottom row.
        grid.fill(0, H as i32 - 1, W as i32, H as i32, Cell::Stone);
        grid
    }

    /// Fill the rectangle from (x0, y0) up to, but not including, (x1, y1).
    fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, cell: Cell) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, cell);
            }
        }
    }

    /// A straight line of `thick` x `thick` squares from (x0, y0) to (x1, y1).
    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, thick: i32, cell: Cell) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for s in 0..=steps {
            let x = x0 + (x1 - x0) * s / steps;
            let y = y0 + (y1 - y0) * s / steps;
            self.fill(x, y, x + thick, y + thick, cell);
        }
    }

    fn emit(&mut self, x: i32, y: i32, w: i32, cell: Cell, rate: u32, steps: u32) {
        self.emitters.push(Emitter { x, y, w, cell, rate, steps_left: steps });
    }

    /// Run every tap for one step, and drop the ones that ran dry.
    fn run_emitters(&mut self) {
        // Indexing instead of `for e in &mut self.emitters`: the loop body
        // calls self.random() and self.set(), which need all of `self`.
        for i in 0..self.emitters.len() {
            let e = self.emitters[i]; // a copy (Emitter is Copy)
            for dx in 0..e.w {
                if self.random() % e.rate == 0 && self.get(e.x + dx, e.y) == Cell::Empty {
                    self.set(e.x + dx, e.y, e.cell);
                }
            }
            self.emitters[i].steps_left -= 1;
        }
        self.emitters.retain(|e| e.steps_left > 0);
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
        self.run_emitters();

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
                    Cell::Water => self.update_liquid(x, y, false),
                    Cell::Lava => self.update_lava(x, y),
                    Cell::Acid => self.update_acid(x, y),
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

    /// Movement shared by every liquid. `thick` liquids (lava) spread slowly.
    fn update_liquid(&mut self, x: i32, y: i32, thick: bool) {
        if self.try_move(x, y, 0, 1) {
            return;
        }
        let dir = if self.coin_flip() { 1 } else { -1 };
        if self.try_move(x, y, dir, 1) || self.try_move(x, y, -dir, 1) {
            return;
        }
        // Thick liquids only flow sideways 1 step in 4.
        if thick && self.random() % 4 != 0 {
            return;
        }
        // Can't fall at all: flow sideways. This is what makes liquids level out.
        if !self.try_move(x, y, dir, 0) {
            self.try_move(x, y, -dir, 0);
        }
    }

    fn update_lava(&mut self, x: i32, y: i32) {
        for (dx, dy) in NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            match self.get(nx, ny) {
                // Lava + water: the lava cools into stone, the water boils off.
                Cell::Water => {
                    self.set(x, y, Cell::Stone);
                    self.set(nx, ny, Cell::Smoke { life: 70 });
                    return; // we are stone now: stop, don't flow
                }
                // Lava sets wood alight much faster than fire does.
                Cell::Wood if self.random() % 100 < 15 => {
                    self.set(nx, ny, Cell::Fire { life: 100 });
                }
                Cell::Gunpowder => {
                    self.set(nx, ny, Cell::Fire { life: 4 });
                    self.pending.push((nx, ny));
                }
                // Occasionally a flame licks up from the surface.
                Cell::Empty if dy == -1 && self.random() % 400 == 0 => {
                    self.set(nx, ny, Cell::Fire { life: 20 });
                }
                _ => {}
            }
        }
        self.update_liquid(x, y, true);
    }

    fn update_acid(&mut self, x: i32, y: i32) {
        for (dx, dy) in NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            if ny == H as i32 - 1 {
                continue; // the floor is acid-proof
            }
            match self.get(nx, ny) {
                // Acid eats solids and powders, a little at a time...
                Cell::Sand | Cell::Stone | Cell::Wood | Cell::Gunpowder
                    if self.random() % 100 < 6 =>
                {
                    self.set(nx, ny, Cell::Empty);
                    // ...and gets used up doing it, leaving a puff of fumes.
                    if self.coin_flip() {
                        self.set(x, y, Cell::Smoke { life: 30 });
                        return;
                    }
                }
                // Acid boils away on contact with lava.
                Cell::Lava => {
                    self.set(x, y, Cell::Smoke { life: 40 });
                    return;
                }
                _ => {}
            }
        }
        self.update_liquid(x, y, false);
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
// Test bench: preset scenes that show off each material
// ---------------------------------------------------------------------------

/// A named starting layout. `build` is a plain function pointer:
/// `fn(&mut Grid)` is the type of "any function that takes a &mut Grid".
struct Scene {
    name: &'static str,
    about: &'static str,
    build: fn(&mut Grid),
}

const SCENES: [Scene; 6] = [
    Scene {
        name: "sandbox",
        about: "empty world with a ledge: paint anything",
        build: sandbox,
    },
    Scene {
        name: "sand + water",
        about: "sand piles on a ramp; water levels out; sand sinks through water",
        build: sand_and_water,
    },
    Scene {
        name: "fire",
        about: "fire spreads through a wood forest; the hut burns through its water tank and floods itself out",
        build: fire,
    },
    Scene {
        name: "gunpowder",
        about: "a fuse sets off piles one by one, then the stone bunker blows apart",
        build: gunpowder,
    },
    Scene {
        name: "lava",
        about: "left: lava into water makes stone + steam. right: lava burns wood, finds powder",
        build: lava,
    },
    Scene {
        name: "acid",
        about: "acid rain eats stone, sand, wood and powder; the floor is acid-proof",
        build: acid,
    },
];

fn sandbox(g: &mut Grid) {
    g.fill(160, 240, 320, 243, Cell::Stone);
}

fn sand_and_water(g: &mut Grid) {
    // Left: a ramp with sand pouring onto it.
    g.line(40, 100, 200, 180, 3, Cell::Stone);
    g.emit(50, 10, 20, Cell::Sand, 3, 900);
    // Right: a basin filled by water, with sand dropped into it.
    g.fill(260, 200, 264, H as i32, Cell::Stone);
    g.fill(600, 200, 604, H as i32, Cell::Stone);
    g.emit(280, 10, 60, Cell::Water, 2, 900);
    g.emit(480, 10, 20, Cell::Sand, 3, 900);
}

fn fire(g: &mut Grid) {
    // A row of trees whose crowns touch, so fire can walk along them.
    for x in [60, 120, 180, 240, 300] {
        g.fill(x - 3, 260, x + 3, H as i32 - 1, Cell::Wood); // trunk
        g.fill(x - 30, 215, x + 30, 260, Cell::Wood); // crown
    }
    g.fill(50, 340, 54, 358, Cell::Fire { life: 120 });
    // A hut with a water tank on its roof. The tank's floor is wood, so when
    // the fire burns through it, the water floods down and puts the fire out.
    g.fill(440, 300, 560, H as i32 - 1, Cell::Wood); // hut
    g.fill(430, 180, 434, 300, Cell::Stone); // tank walls
    g.fill(566, 180, 570, 300, Cell::Stone);
    g.fill(434, 294, 566, 300, Cell::Wood); // tank floor
    g.fill(434, 190, 566, 294, Cell::Water);
    g.fill(440, 300, 446, 306, Cell::Fire { life: 120 }); // fire under the tank
}

fn gunpowder(g: &mut Grid) {
    let floor = H as i32 - 1;
    // A thin fuse along the floor, with bigger piles every 100 cells.
    g.fill(40, floor - 2, 440, floor, Cell::Gunpowder);
    for x in [100, 200, 300] {
        g.fill(x, floor - 16, x + 30, floor, Cell::Gunpowder);
    }
    g.fill(150, floor - 30, 180, floor, Cell::Wood); // a crate to send flying
    g.fill(250, floor - 60, 256, floor, Cell::Stone); // a pillar to break
    // A stone bunker packed with powder at the end of the fuse.
    g.fill(440, 250, 600, floor, Cell::Stone);
    g.fill(444, 254, 596, floor, Cell::Gunpowder);
    // Light the fuse.
    g.fill(34, floor - 3, 40, floor, Cell::Fire { life: 120 });
}

fn lava(g: &mut Grid) {
    // Left: a basin of water with lava pouring in from above.
    g.fill(60, 240, 64, H as i32, Cell::Stone);
    g.fill(300, 240, 304, H as i32, Cell::Stone);
    g.fill(64, 260, 300, H as i32 - 1, Cell::Water);
    g.emit(160, 10, 30, Cell::Lava, 3, 600);
    // Right: a block of wood with a powder pocket hidden inside.
    g.fill(400, 290, 620, H as i32 - 1, Cell::Wood);
    g.fill(540, 315, 580, 335, Cell::Gunpowder);
    g.emit(470, 10, 20, Cell::Lava, 3, 600);
}

fn acid(g: &mut Grid) {
    let floor = H as i32 - 1;
    g.fill(60, 260, 160, floor, Cell::Stone);
    g.fill(200, 300, 300, floor, Cell::Sand);
    g.fill(340, 260, 440, floor, Cell::Wood);
    g.fill(480, 300, 580, floor, Cell::Gunpowder);
    g.emit(40, 10, 560, Cell::Acid, 20, 1200);
}

fn load_scene(index: usize) -> Grid {
    let mut grid = Grid::new();
    (SCENES[index].build)(&mut grid); // call the function stored in the struct
    grid
}

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
    let mut scene = 1;
    let mut grid = load_scene(scene);
    let mut brush = Cell::Sand;
    let mut brush_size = 6;
    let mut paused = false;

    // A W x H image on the CPU, and a matching texture on the GPU.
    let mut image = Image::gen_image_color(W as u16, H as u16, BLACK);
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest); // crisp square cells when scaled up

    loop {
        // --- scene controls ---
        let mut reload = is_key_pressed(KeyCode::R);
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Right) {
            scene = (scene + 1) % SCENES.len();
            reload = true;
        }
        if is_key_pressed(KeyCode::Left) {
            scene = (scene + SCENES.len() - 1) % SCENES.len();
            reload = true;
        }
        if reload {
            grid = load_scene(scene); // the old Grid is dropped (freed) here
        }
        if is_key_pressed(KeyCode::C) {
            grid = Grid::new();
        }
        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
        }

        // --- brush ---
        pick_brush(&mut brush);
        let wheel = mouse_wheel().1;
        if is_key_pressed(KeyCode::RightBracket) || wheel > 0.0 {
            brush_size = (brush_size + 1).min(40);
        }
        if is_key_pressed(KeyCode::LeftBracket) || wheel < 0.0 {
            brush_size = (brush_size - 1).max(1);
        }
        if is_key_pressed(KeyCode::E) {
            let (mx, my) = mouse_position();
            grid.explode((mx / SCALE) as i32, (my / SCALE) as i32, BOMB_BLAST);
        }
        paint(&mut grid, brush, brush_size);

        // --- simulate: N steps one frame while paused ---
        if !paused || is_key_pressed(KeyCode::N) {
            grid.step();
        }

        // --- draw ---
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
        draw_hud(scene, brush, brush_size, paused, &grid);

        next_frame().await; // hand control back to macroquad until the next frame
    }
}

fn draw_hud(scene: usize, brush: Cell, brush_size: i32, paused: bool, grid: &Grid) {
    draw_rectangle(0.0, 0.0, W as f32 * SCALE, 78.0, Color::new(0.0, 0.0, 0.0, 0.6));
    let s = &SCENES[scene];
    let state = if paused { "PAUSED (N = step)" } else { "running" };
    let lines = [
        format!(
            "scene {}/{}: {} - {}",
            scene + 1,
            SCENES.len(),
            s.name,
            s.about
        ),
        format!(
            "brush: {} size {} | {} | flying {} | {} fps",
            brush.name(),
            brush_size,
            state,
            grid.particles.len(),
            get_fps()
        ),
        "1 sand 2 water 3 stone 4 erase 5 wood 6 fire 7 powder 8 lava 9 acid | \
         E bomb | [ ] / wheel size | Tab/arrows scene | R reset | C clear | Space pause"
            .to_string(),
    ];
    for (i, text) in lines.iter().enumerate() {
        draw_text(text, 10.0, 22.0 + i as f32 * 24.0, 22.0, WHITE);
    }
}

/// Keys 1-9 choose what the left mouse button paints.
fn pick_brush(brush: &mut Cell) {
    let choices = [
        (KeyCode::Key1, Cell::Sand),
        (KeyCode::Key2, Cell::Water),
        (KeyCode::Key3, Cell::Stone),
        (KeyCode::Key4, Cell::Empty),
        (KeyCode::Key5, Cell::Wood),
        (KeyCode::Key6, Cell::Fire { life: 0 }), // life is randomised when painting
        (KeyCode::Key7, Cell::Gunpowder),
        (KeyCode::Key8, Cell::Lava),
        (KeyCode::Key9, Cell::Acid),
    ];
    for (key, cell) in choices {
        if is_key_pressed(key) {
            *brush = cell;
        }
    }
}

/// Left mouse paints the selected material, right mouse erases.
fn paint(grid: &mut Grid, brush: Cell, size: i32) {
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
        Cell::Sand | Cell::Water | Cell::Fire { .. } | Cell::Gunpowder | Cell::Lava | Cell::Acid
    );
    for dy in -size..=size {
        for dx in -size..=size {
            if dx * dx + dy * dy > size * size {
                continue; // round brush
            }
            // Sprinkle loose materials instead of filling solid blocks.
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
        grid.fill(80, 120, 160, 121, Cell::Stone); // a ledge
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
    fn lava_and_water_make_stone() {
        let mut grid = Grid::new();
        grid.set(100, 50, Cell::Lava);
        grid.set(101, 50, Cell::Water);
        grid.step();
        assert_eq!(grid.get(100, 50), Cell::Stone);
    }

    #[test]
    fn acid_eats_through_stone() {
        let mut grid = Grid::new();
        grid.fill(80, 120, 160, 121, Cell::Stone); // a ledge
        // Acid sitting on the stone ledge at y = 120.
        for x in 100..110 {
            grid.set(x, 119, Cell::Acid);
        }
        let stone_before = (80..160).filter(|&x| grid.get(x, 120) == Cell::Stone).count();
        for _ in 0..300 {
            grid.step();
        }
        let stone_after = (80..160).filter(|&x| grid.get(x, 120) == Cell::Stone).count();
        assert!(stone_after < stone_before, "acid should dissolve part of the ledge");
    }

    #[test]
    fn every_scene_runs_without_crashing() {
        for i in 0..SCENES.len() {
            let mut grid = load_scene(i);
            for _ in 0..400 {
                grid.step();
            }
        }
    }

    #[test]
    fn emitters_run_dry() {
        let mut grid = Grid::new();
        grid.emit(10, 10, 5, Cell::Sand, 1, 3);
        for _ in 0..3 {
            grid.step();
        }
        assert!(grid.emitters.is_empty());
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

