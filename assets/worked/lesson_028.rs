const CHUNK:i32=64;
fn split_coordinate(world:i32)->(i32,i32) {
    (world.div_euclid(CHUNK),world.rem_euclid(CHUNK))
}
fn main() {
    for world in [-65,-64,-1,0,63,64] {
        let (chunk,local)=split_coordinate(world);
        assert!((0..CHUNK).contains(&local));
        assert_eq!(chunk*CHUNK+local,world);
        println!("{world}: chunk={chunk}, local={local}");
    }
}
