use sand::physics::{cpu::CpuSim, *};
fn main() {
    let mut w = World::new(120, 90);
    w.fill(0, 20, 120, 89, WATER);
    w.fill(40, 66, 80, 89, LAVA);
    w.fill(48, 58, 72, 66, LAVA);
    let mut sim = CpuSim::new(&w);
    for s in 0..=3 * STEPS_PER_SECOND {
        if s % 360 == 0 {
            let plateau = w.cells.iter().filter(|&&c| material(c) == WATER && temp(c) > celsius(100)).count();
            let hotw = w.cells.iter().filter(|&&c| material(c) == WATER && sens(c) >= celsius(99)).count();
            println!("t={:.1} steam={} lava={} stone={} sand={} water_at_100={} water_storing_latent={}", s as f64 / 1200.0, w.count(STEAM), w.count(LAVA), w.count(STONE), w.count(SAND), hotw, plateau);
        }
        sim.step(&mut w);
    }
    for y in 50..62 { let c = w.cells[y*120+60]; print!("{}:{}{}/{} ", y, &NAMES[material(c) as usize][..2], sens(c)/256, temp(c)/256); }
    println!();
}
