//! Reproduces the three reported bugs and prints their metrics every 0.1 s
//! of world time. Used by docs/plans/physics-fixes.md, step 1.
use sand::physics::{cpu::CpuSim, *};

fn metrics(w: &World, s: f64) -> String {
    let water_max = (0..w.cells.len()).filter(|&i| material(w.cells[i]) == WATER).map(|i| temp(w.cells[i])).max().unwrap_or(AMBIENT) / T_SCALE;
    let count = |m| w.count(m);
    format!(
        "t={s:5.2}s water_max={water_max:4}C steam={:4} lava={:4} stone={:4} wood={:4} fire={:4} oil={:4} fire_total={:4}",
        count(STEAM), count(LAVA), count(STONE), count(WOOD), count(FIRE), count(OIL), count(FIRE)
    )
}

fn run(name: &str, mut w: World, secs: f64) {
    println!("== {name}");
    let mut sim = CpuSim::new(&w);
    let steps_per_print = (STEPS_PER_SECOND / 10) as usize;
    for s in 0..=(secs * 10.0) as usize {
        if s % 10 == 0 || s < 3 {
            println!("{}", metrics(&w, s as f64 / 10.0));
        }
        for _ in 0..steps_per_print { sim.step(&mut w); }
    }
}

fn main() {
    let (wd, ht) = (200usize, 120usize);
    let floor = ht as i32 - 1;

    // 1. Lava poured on a water bed.
    let mut w = World::new(wd, ht);
    w.fill(0, floor - 30, wd as i32, floor, WATER);
    w.fill(60, floor - 45, 100, floor - 30, LAVA);
    run("lava_on_water", w, 10.0);

    // 2. Lava column against a wood wall.
    let mut w = World::new(wd, ht);
    w.fill(90, floor - 40, 110, floor, WOOD);
    w.fill(60, floor - 40, 90, floor, LAVA);
    run("lava_wood_wall", w, 10.0);

    // 3. Oil tub with one fire cell at one end.
    let mut w = World::new(wd, ht);
    w.fill(40, floor - 20, 160, floor, OIL);
    w.set(40, floor - 19, FIRE);
    run("fire_oil_tub", w, 10.0);
}
