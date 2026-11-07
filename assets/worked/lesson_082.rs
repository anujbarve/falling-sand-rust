fn ramp(top:[u8;3],bottom:[u8;3],row:u32,span:u32)->[u8;3] {
    assert!(span>0);
    let t=(row as f32/span as f32).clamp(0.0,1.0);
    std::array::from_fn(|i| (top[i] as f32+(bottom[i] as f32-top[i] as f32)*t).round() as u8)
}
fn in_disc(x:i32,y:i32,cx:i32,cy:i32,r:i32)->bool {
    let dx=i64::from(x)-i64::from(cx); let dy=i64::from(y)-i64::from(cy);
    dx*dx+dy*dy<=i64::from(r)*i64::from(r)
}
fn main() {
    let top=[15,26,51]; let horizon=[51,80,122];
    assert_eq!(ramp(top,horizon,0,400),top);
    assert_eq!(ramp(top,horizon,400,400),horizon);
    assert_eq!(ramp(top,horizon,900,400),horizon);
    assert!(in_disc(3,4,0,0,5));
    assert!(!in_disc(4,4,0,0,5));
    println!("sky endpoints and hard disc boundary verified");
}
