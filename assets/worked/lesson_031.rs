use std::time::Instant;
fn main() {
    let mut values=vec![0_u64;74_624];
    let start=Instant::now();
    for tick in 0..600_u64 {
        for value in &mut values { *value=value.wrapping_add(tick); }
    }
    let elapsed=start.elapsed();
    println!("synthetic total={elapsed:?}");
    println!("synthetic mean per iteration={:?}",elapsed/600);
    std::hint::black_box(values);
}
