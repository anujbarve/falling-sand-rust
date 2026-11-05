//! Part of the physics engine (see mod.rs).

use super::*;

// ---------------------------------------------------------------------------
// Scenes. They are built from fractions of the size, so they work at any
// resolution (the benchmark runs them up to 4K).
// ---------------------------------------------------------------------------

pub const SCENES: [(&str, &str); 6] = [
    ("layers", "sand, water and oil sort themselves by density"),
    ("lava + water", "lava cools to stone, water boils to steam, steam rains back down"),
    ("fire", "fire heats wood until it ignites; a pool of oil goes up too"),
    ("ice", "hot steam and lava melt ice; meltwater refreezes on the cold blocks"),
    ("benchmark", "everything at once; what the benchmark runs"),
    ("jet engine", "four nozzles blow hot exhaust forever; a freezer ices the water; a jet lights the coal"),
];

/// A nozzle at (x, y) facing `dir` (0 up, 1 right, 2 down, 3 left).
fn jet(g: &mut World, x: i32, y: i32, dir: u32) {
    if let Some(i) = g.index(x, y) {
        g.cells[i] = pack(JET, dir, default_temp(JET));
    }
}

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
        4 => {
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
        _ => {
            // A jet rig: a metal body with a nozzle on each side; a freezer
            // under falling water; a jet blowing onto a coal pile.
            let (cx, cy) = (fx(0.5), fy(0.5));
            g.fill(cx - 6, cy - 4, cx + 6, cy + 4, METAL);
            jet(&mut g, cx, cy - 5, 0);
            jet(&mut g, cx + 7, cy, 1);
            jet(&mut g, cx, cy + 5, 2);
            jet(&mut g, cx - 7, cy, 3);
            g.fill(fx(0.12), fy(0.8), fx(0.12) + 8, fy(0.8) + 3, FREEZER);
            g.fill(fx(0.12), fy(0.55), fx(0.12) + 8, fy(0.7), WATER);
            g.fill(fx(0.7), floor - 6, fx(0.7) + 10, floor, COAL);
            jet(&mut g, fx(0.7) + 14, floor - 3, 3);
        }
    }
    g
}

