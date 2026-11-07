#[derive(Debug, PartialEq)]
enum Kind { Air, Water, Stone }
fn sample_lake(x: i32, y: i32, centre: i32, half: i32, level: i32) -> Kind {
    assert!(half > 0);
    let distance = (x - centre).abs();
    if distance > half { return Kind::Air; }
    let floor = level - (half - distance);
    if y <= floor { Kind::Stone }
    else if y <= level { Kind::Water }
    else { Kind::Air }
}
fn main() {
    assert_eq!(sample_lake(64, 8, 64, 12, 10), Kind::Water);
    assert_eq!(sample_lake(52, 10, 64, 12, 10), Kind::Stone);
    assert_eq!(sample_lake(64, 11, 64, 12, 10), Kind::Air);
    println!("bowl centre, rim and open sky agree");
}
