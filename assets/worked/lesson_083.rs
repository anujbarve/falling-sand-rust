fn soil_colour(moisture:u8)->[u8;3] {
    let dry=[121_i32,100,81]; let wet=[76_i32,69,59];
    let m=i32::from(moisture.min(100));
    std::array::from_fn(|i| (dry[i]+(wet[i]-dry[i])*m/100) as u8)
}
fn main() {
    assert_eq!(soil_colour(0),[121,100,81]);
    assert_eq!(soil_colour(100),[76,69,59]);
    for m in [0,25,50,75,100] {println!("moisture {m}: {:?}",soil_colour(m));}
    assert_eq!(soil_colour(200),soil_colour(100));
}
