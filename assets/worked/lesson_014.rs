fn main() {
    let period = 0.05_f64;
    let mut elapsed = 0.0_f64;
    let mut total_ticks = 0;
    for dt in [0.03, 0.03, 0.04] {
        elapsed += dt;
        let mut ticks_this_frame = 0;
        while elapsed + 1e-12 >= period && ticks_this_frame < 2 {
            total_ticks += 1;
            elapsed = (elapsed - period).max(0.0);
            ticks_this_frame += 1;
        }
        if elapsed >= period { elapsed %= period; }
        println!("ticks={total_ticks}, remainder={elapsed:.3}");
    }
}
