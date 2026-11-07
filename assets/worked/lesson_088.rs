fn fit(window_w: u32, window_h: u32, budget: u64) -> (u32, u32) {
    if window_w < 4 || window_h < 4 { return (0, 0); }
    let max_w = window_w / 4;
    let max_h = window_h / 4;
    let ratio = window_h as f64 / window_w as f64;
    let mut best = (0, 0);
    for w in 1..=max_w {
        let h = ((w as f64 * ratio).round() as u32).clamp(1, max_h);
        let active = u64::from(w + 32) * u64::from(h + 32);
        if active <= budget && u64::from(w)*u64::from(h) >
            u64::from(best.0)*u64::from(best.1) { best = (w, h); }
    }
    best
}
fn main() {
    assert_eq!(fit(1280, 720, 74_624), (320, 180));
    let (w,h) = fit(1920,1080,74_624);
    assert_eq!((w,h),(320,180));
    assert!(u64::from(w+32)*u64::from(h+32) <= 74_624);
    println!("1920x1080 window: {}x{} physical world pixels",w*4,h*4);
}
