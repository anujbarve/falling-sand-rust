//! A second falling-sand engine, built so the same rules can run on the CPU
//! and on the GPU (wgpu compute shaders) and be compared fairly.
//!
//! What is more "physical" than the original engine in `main.rs`:
//!
//! - **Density and buoyancy.** Every material has a density. A heavier fluid
//!   sinks through a lighter one, and the bigger the density gap, the faster
//!   it sinks: sand falls fast through air but slowly through water, oil
//!   rises slowly through water, steam shoots up through air.
//! - **Friction and viscosity.** Powders slide down slopes only some of the
//!   time (friction sets the pile angle). Liquids spread at different speeds:
//!   water fast, oil slower, lava very slowly.
//! - **Heat.** Every cell has a temperature. Heat flows between neighbours
//!   by conduction, at a rate set by how conductive both materials are
//!   (air insulates, ice and water conduct well). Gases also lose heat to the
//!   surroundings.
//! - **Phase changes driven by temperature.** Water boils to steam at 100°C,
//!   steam condenses back into rain when it cools, water freezes, ice melts,
//!   lava cools into stone, very hot stone melts back into lava, and wood and
//!   oil catch fire at their ignition points. Fire burns hot and heats its
//!   surroundings, which is how it spreads.
//!
//! How one step works, and why it parallelises:
//!
//! 1. **Heat pass.** Every cell reads its 4 neighbours from the *old* grid and
//!    writes its new temperature and material into a *new* grid. No cell
//!    writes anywhere another cell writes, so all cells can run at once.
//! 2. **Move pass (Margolus blocks).** The grid is cut into 2x2 blocks. Each
//!    block rearranges only its own 4 cells, so all blocks can run at once.
//!    Every other step the block grid is shifted by one cell diagonally, so
//!    material can cross block borders.
//!
//! The original engine scans cells one by one and lets each move anywhere
//! nearby. That only works in order, so it cannot run on a GPU.
//!
//! All the maths is integer maths and the randomness is a hash of
//! (position, step number), not a running generator. So the CPU and the GPU
//! give exactly the same result, bit for bit, which the tests check.

pub mod cpu;
pub mod gpu;

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

// A cell is one packed u32, so a step moves half the bytes it would with
// a separate temperature array (the GPU is limited by memory traffic):
//
//   bits 0-3    material (16 possible)
//   bits 4-11   aux: remaining life for fire and smoke
//   bits 12-31  temperature, signed 20 bits (see T_SCALE): about ±2047°C
//
// The GPU shader `sim.wgsl` repeats these numbers: keep the two in sync.
pub const AIR: u32 = 0;
pub const STONE: u32 = 1;
pub const SAND: u32 = 2;
pub const WATER: u32 = 3;
pub const OIL: u32 = 4;
pub const LAVA: u32 = 5;
pub const STEAM: u32 = 6;
pub const WOOD: u32 = 7;
pub const FIRE: u32 = 8;
pub const ICE: u32 = 9;
pub const SMOKE: u32 = 10;
pub const MATERIAL_COUNT: usize = 11;

// What kind of matter each material is.
pub const GAS: u32 = 0;
pub const LIQUID: u32 = 1;
pub const POWDER: u32 = 2;
pub const SOLID: u32 = 3;

pub const NAMES: [&str; MATERIAL_COUNT] = [
    "air", "stone", "sand", "water", "oil", "lava", "steam", "wood", "fire", "ice", "smoke",
];

// Indexed by material id.         air stone sand water oil lava steam wood fire ice smoke
pub const KIND: [u32; MATERIAL_COUNT] = [GAS, SOLID, POWDER, LIQUID, LIQUID, LIQUID, GAS, SOLID, GAS, SOLID, GAS];
/// Relative density. Only differences matter. Solids never move.
pub const DENSITY: [i32; MATERIAL_COUNT] = [10, 255, 150, 100, 80, 250, 3, 255, 9, 255, 6];
/// Chance out of 256 to flow sideways (liquids, gases) or slide down a
/// slope (powders) in one step. Low = viscous / high friction.
pub const FLOW: [u32; MATERIAL_COUNT] = [256, 0, 200, 230, 150, 40, 200, 0, 120, 0, 150];
/// Thermal conductivity, out of 1024 per neighbour per step. Must stay
/// at most 256 so that 4 neighbours can never move more than 100% of
/// the temperature gap in one step (which would make the maths unstable).
pub const COND: [i32; MATERIAL_COUNT] = [15, 100, 40, 120, 40, 80, 20, 40, 80, 150, 15];

/// Temperatures are fixed point: 256 units per degree Celsius. Fine steps
/// matter: conduction rounds toward zero, so a gap smaller than one unit
/// times 1024 / COND would never flow.
pub const T_SCALE: i32 = 256;
pub const fn celsius(c: i32) -> i32 {
    c * T_SCALE
}
pub const AMBIENT: i32 = celsius(20);

/// Temperature range that fits in 20 signed bits.
pub const T_MIN: i32 = -(1 << 19);
pub const T_MAX: i32 = (1 << 19) - 1;

pub const fn material(cell: u32) -> u32 {
    cell & 0xF
}
pub const fn aux(cell: u32) -> u32 {
    (cell >> 4) & 0xFF
}
/// `as i32` reinterprets the bits; `>>` on an i32 copies the sign bit
/// down, so negative temperatures come back out negative.
pub const fn temp(cell: u32) -> i32 {
    (cell as i32) >> 12
}
pub fn pack(material: u32, aux: u32, temp: i32) -> u32 {
    material | (aux << 4) | ((temp.clamp(T_MIN, T_MAX) as u32) << 12)
}

/// Starting temperature of a freshly placed cell.
pub fn default_temp(m: u32) -> i32 {
    match m {
        LAVA => celsius(1600),
        FIRE => celsius(900),
        ICE => celsius(-40),
        STEAM => celsius(120),
        _ => AMBIENT,
    }
}

/// Starting `aux` (life) of a freshly placed cell.
pub fn default_aux(m: u32) -> u32 {
    match m {
        FIRE => 80,
        SMOKE => 60,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Randomness: a hash, not a running generator
// ---------------------------------------------------------------------------

/// PCG hash: scrambles a u32 into a random-looking u32. Pure function, so
/// the GPU (which has no shared random state) computes the same numbers.
pub fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

pub fn hash3(a: u32, b: u32, c: u32) -> u32 {
    pcg(a ^ pcg(b ^ pcg(c)))
}

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

// ---------------------------------------------------------------------------
// The rules. cpu.rs calls these; sim.wgsl is a line-by-line copy of them.
// ---------------------------------------------------------------------------

/// Heat pass for one cell: conduction from the 4 neighbours, then phase
/// changes. Reads only the old grid, returns the cell's new packed value.
pub fn heat_cell(cells: &[u32], w: usize, h: usize, x: usize, y: usize, frame: u32) -> u32 {
    let i = y * w + x;
    let m = material(cells[i]);
    let mut life = aux(cells[i]);
    let t0 = temp(cells[i]);
    let k_self = COND[m as usize];

    // Conduction. The heat that flows from j to i is exactly minus the heat
    // that flows from i to j (integer division rounds toward zero, the same
    // for +x and -x), so conduction alone never creates or destroys heat.
    let mut flux = 0;
    let mut touches_water = false;
    let mut neighbour = |j: usize| {
        let mn = material(cells[j]);
        let k = k_self.min(COND[mn as usize]);
        flux += (temp(cells[j]) - t0) * k / 1024;
        touches_water |= mn == WATER;
    };
    if x > 0 {
        neighbour(i - 1);
    }
    if x + 1 < w {
        neighbour(i + 1);
    }
    if y > 0 {
        neighbour(i - w);
    }
    if y + 1 < h {
        neighbour(i + w);
    }
    let mut t = t0 + flux;
    // Is something solid holding this cell up? (The world's bottom edge counts.)
    let supported = y + 1 >= h || KIND[material(cells[i + w]) as usize] >= POWDER;

    // Gases mix with the wider atmosphere and drift back to room temperature.
    if KIND[m as usize] == GAS && m != FIRE {
        t += (AMBIENT - t) / 64;
    }

    let r = hash3(x as u32, y as u32, frame ^ 0xA511_E9B3);
    let mut next = m;
    match m {
        WATER if t >= celsius(100) => next = STEAM,
        WATER if t <= celsius(-2) => next = ICE,
        ICE if t >= celsius(1) => next = WATER,
        // Condenses well below boiling, so steam rises a while before raining.
        STEAM if t < celsius(50) => next = WATER,
        // Lava resting on something sets into stone. Lava that cools while
        // falling or sinking through water shatters into gravel instead
        // (real lava does this in water), so no stone is left hanging in mid-air.
        LAVA if t < celsius(800) => next = if supported { STONE } else { SAND },
        STONE if t >= celsius(1100) => next = LAVA,
        WOOD if t >= celsius(300) => {
            next = FIRE;
            life = 40 + (r & 63);
        }
        OIL if t >= celsius(250) => {
            next = FIRE;
            life = 20 + (r & 31);
        }
        FIRE => {
            if touches_water || life == 0 {
                next = SMOKE;
                life = 30 + (r & 31);
            } else {
                // Burning releases heat: the flame stays at least this hot.
                life -= 1;
                t = t.max(celsius(900));
            }
        }
        SMOKE => {
            if life == 0 {
                next = AIR;
            } else {
                life -= 1;
            }
        }
        _ => {}
    }
    pack(next, if next == AIR { 0 } else { life }, t)
}

/// Density of a cell at temperature `t`. Gases expand when heated, so hot
/// air is lighter than cold air and rises: that is convection. (Flames keep
/// a fixed density, or they would shoot away from what they are burning.)
fn density(cell: u32) -> i32 {
    let (m, t) = (material(cell), temp(cell));
    let d = DENSITY[m as usize];
    if KIND[m as usize] == GAS && m != FIRE {
        (d - (t - AMBIENT) / celsius(40)).max(1)
    } else {
        d
    }
}

/// Would the cell `top` sink into the cell `bottom` right below it?
/// The bigger the density gap, the more likely: that is buoyancy.
fn sinks(top: u32, bottom: u32, roll: u32) -> bool {
    let (mt, mb) = (material(top) as usize, material(bottom) as usize);
    // Solids never move; a pile of powder is not pushed aside by liquid.
    if KIND[mt] == SOLID || KIND[mb] == SOLID || KIND[mb] == POWDER {
        return false;
    }
    let (dt, db) = (density(top), density(bottom));
    if dt <= db {
        return false;
    }
    roll < ((dt - db) * 256 / dt) as u32
}

/// Would two side-by-side fluid cells trade places (spreading, mixing)?
fn spreads(left: u32, right: u32, roll: u32) -> bool {
    let (ml, mr) = (material(left) as usize, material(right) as usize);
    let fluid = |k: u32| k == GAS || k == LIQUID;
    ml != mr && fluid(KIND[ml]) && fluid(KIND[mr]) && roll < FLOW[ml].min(FLOW[mr])
}

/// Move pass for one 2x2 block. `cells` is in the order top-left,
/// top-right, bottom-left, bottom-right. The temperature is packed into
/// the cell, so it travels with the cell for free.
pub fn move_block(cells: &mut [u32; 4], r: u32) {
    let r2 = pcg(r);
    let mut fell = [false; 2];

    // 1. Gravity and buoyancy, in each column: heavy above light trades places.
    for col in 0..2 {
        let (top, bottom) = (col, col + 2);
        if sinks(cells[top], cells[bottom], (r >> (col * 8)) & 255) {
            cells.swap(top, bottom);
            fell[col] = true;
        }
    }

    // 2. Could not fall straight down: powders and liquids slide diagonally,
    //    with a chance set by their friction / viscosity.
    let mut slid = false;
    for col in 0..2 {
        if fell[col] {
            continue;
        }
        let (top, diagonal) = (col, 3 - col);
        let m = material(cells[top]) as usize;
        if KIND[m] != POWDER && KIND[m] != LIQUID {
            continue;
        }
        let roll = (r >> (16 + col * 8)) & 255;
        if roll < FLOW[m] && sinks(cells[top], cells[diagonal], 0) {
            cells.swap(top, diagonal);
            slid = true;
        }
    }

    // 3. Nothing fell: fluids spread sideways. This levels out liquids.
    if !fell[0] && !fell[1] && !slid {
        for row in 0..2 {
            let (left, right) = (row * 2, row * 2 + 1);
            if spreads(cells[left], cells[right], (r2 >> (row * 8)) & 255) {
                cells.swap(left, right);
            }
        }
    }
}

/// Which corner the block grid starts at in this step: (0,0) or (1,1).
pub fn block_offset(frame: u32) -> usize {
    (frame & 1) as usize
}

// ---------------------------------------------------------------------------
// Scenes. They are built from fractions of the size, so they work at any
// resolution (the benchmark runs them up to 4K).
// ---------------------------------------------------------------------------

pub const SCENES: [(&str, &str); 5] = [
    ("layers", "sand, water and oil sort themselves by density"),
    ("lava + water", "lava cools to stone, water boils to steam, steam rains back down"),
    ("fire", "fire heats wood until it ignites; a pool of oil goes up too"),
    ("ice", "hot steam and lava melt ice; meltwater refreezes on the cold blocks"),
    ("benchmark", "everything at once; what the benchmark runs"),
];

pub fn scene(index: usize, w: usize, h: usize) -> World {
    let mut g = World::new(w, h);
    let (wi, hi) = (w as i32, h as i32);
    let fx = |f: f32| (f * w as f32) as i32; // fraction of the width, in cells
    let fy = |f: f32| (f * h as f32) as i32;
    let floor = hi - 1;
    match index {
        0 => {
            // A tank holding a jumble of all three; they sort into layers.
            g.fill(fx(0.2), fy(0.3), fx(0.2) + 3, floor, STONE);
            g.fill(fx(0.8), fy(0.3), fx(0.8) + 3, floor, STONE);
            g.sprinkle(fx(0.2) + 3, fy(0.35), fx(0.8), floor, OIL, 30);
            g.sprinkle(fx(0.2) + 3, fy(0.35), fx(0.8), floor, WATER, 35);
            g.sprinkle(fx(0.2) + 3, fy(0.35), fx(0.8), floor, SAND, 25);
        }
        1 => {
            g.fill(fx(0.1), fy(0.55), fx(0.1) + 3, floor, STONE);
            g.fill(fx(0.9), fy(0.55), fx(0.9) + 3, floor, STONE);
            g.fill(fx(0.1) + 3, fy(0.7), fx(0.9), floor, WATER);
            g.fill(fx(0.4), fy(0.05), fx(0.6), fy(0.3), LAVA);
        }
        2 => {
            for i in 0..5 {
                let x = fx(0.08 + i as f32 * 0.12);
                g.fill(x - 2, fy(0.65), x + 2, floor, WOOD); // trunk
                g.fill(x - fx(0.05), fy(0.5), x + fx(0.05), fy(0.65), WOOD); // crown
            }
            g.fill(fx(0.03), floor - 6, fx(0.05), floor, FIRE);
            // An open tub of oil, lit from above.
            g.fill(fx(0.7), fy(0.75), fx(0.7) + 3, floor, STONE);
            g.fill(fx(0.95), fy(0.75), fx(0.95) + 3, floor, STONE);
            g.fill(fx(0.7) + 3, fy(0.8), fx(0.95), floor, OIL);
            g.fill(fx(0.8), fy(0.7), fx(0.82), fy(0.72), FIRE);
        }
        3 => {
            g.fill(fx(0.1), fy(0.6), fx(0.3), floor, ICE);
            g.fill(fx(0.4), fy(0.6), fx(0.6), floor, ICE);
            g.fill(fx(0.7), fy(0.6), fx(0.9), floor, ICE);
            g.fill(fx(0.12), fy(0.1), fx(0.28), fy(0.3), STEAM);
            g.fill(fx(0.45), fy(0.2), fx(0.55), fy(0.3), LAVA);
            g.sprinkle(fx(0.7), fy(0.1), fx(0.9), fy(0.3), WATER, 40);
        }
        _ => {
            // Busy everywhere, so no part of the grid is idle.
            for i in 0..8 {
                let y = fy(0.15 + i as f32 * 0.1);
                let x = if i % 2 == 0 { fx(0.05) } else { fx(0.55) };
                g.fill(x, y, x + fx(0.4), y + 2, STONE); // ledges
            }
            g.sprinkle(0, 0, wi, fy(0.25), SAND, 25);
            g.sprinkle(0, 0, wi, fy(0.25), WATER, 25);
            g.sprinkle(0, fy(0.25), wi, fy(0.5), OIL, 20);
            g.fill(fx(0.45), fy(0.55), fx(0.55), fy(0.65), LAVA);
            g.fill(fx(0.1), fy(0.8), fx(0.2), floor, ICE);
            g.fill(fx(0.8), fy(0.75), fx(0.85), floor, WOOD);
            g.fill(fx(0.82), fy(0.73), fx(0.83), fy(0.74), FIRE);
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpu::CpuSim;

    fn run(world: &mut World, steps: usize) {
        let mut sim = CpuSim::new(world);
        for _ in 0..steps {
            sim.step(world);
        }
    }

    #[test]
    fn single_thread_and_rayon_give_identical_worlds() {
        let mut a = scene(4, 256, 160);
        let mut b = a.clone();
        let mut sim_a = CpuSim::new(&a);
        let mut sim_b = CpuSim::new(&b);
        for _ in 0..200 {
            sim_a.step(&mut a);
            sim_b.step_parallel(&mut b);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn gpu_gives_identical_world_to_cpu() {
        let mut cpu_world = scene(4, 256, 160);
        let mut gpu_world = cpu_world.clone();
        let mut gpu = match gpu::GpuSim::new(&gpu_world) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                return;
            }
        };
        run(&mut cpu_world, 200);
        gpu.step(200);
        gpu.download(&mut gpu_world);
        assert_eq!(cpu_world.frame, gpu_world.frame);
        let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
        assert_eq!(diff, 0, "{diff} cells differ between CPU and GPU");
    }

    #[test]
    fn conduction_conserves_heat() {
        // Only stone (no phase change below 1100°C, no gas cooling).
        let mut world = World::new(64, 64);
        world.fill(0, 0, 64, 64, STONE);
        for y in 20..30 {
            for x in 20..30 {
                world.cells[y * 64 + x] = pack(STONE, 0, celsius(1000));
            }
        }
        let total = |w: &World| w.cells.iter().map(|&c| temp(c) as i64).sum::<i64>();
        let before = total(&world);
        run(&mut world, 300);
        let after = total(&world);
        assert_eq!(before, after);
        assert!(temp(world.cells[64 * 25 + 15]) > AMBIENT, "heat should have spread outward");
    }

    #[test]
    fn oil_floats_on_water() {
        // Start upside down: water on top of oil.
        let mut world = World::new(32, 64);
        world.fill(0, 20, 32, 43, WATER);
        world.fill(0, 43, 32, 63, OIL);
        run(&mut world, 3000);
        let mean_y = |m: u32| {
            let ys: Vec<usize> = (0..world.cells.len()).filter(|&i| material(world.cells[i]) == m).map(|i| i / 32).collect();
            ys.iter().sum::<usize>() as f64 / ys.len() as f64
        };
        assert!(mean_y(OIL) < mean_y(WATER), "oil should end up above water");
    }

    #[test]
    fn lava_boils_water_and_turns_to_stone() {
        let mut world = scene(1, 200, 120);
        let mut sim = CpuSim::new(&world);
        let mut most_steam = 0;
        for _ in 0..600 {
            sim.step(&mut world);
            most_steam = most_steam.max(world.count(STEAM));
        }
        assert!(most_steam > 20, "boiling makes steam (saw at most {most_steam})");
        assert!(world.count(STONE) > World::new(200, 120).count(STONE), "lava cools into stone");
    }

    #[test]
    fn hot_fire_ignites_wood() {
        let mut world = World::new(64, 64);
        world.fill(10, 40, 50, 63, WOOD);
        world.fill(28, 38, 32, 40, FIRE);
        let wood_before = world.count(WOOD);
        run(&mut world, 600);
        assert!(world.count(WOOD) < wood_before, "the wood should burn");
    }
}
