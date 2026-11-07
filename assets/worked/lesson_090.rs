#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Air, Sand, Stone }
fn wind_right(row: &mut [Kind]) {
    let mut moved = vec![false; row.len()];
    for x in 0..row.len().saturating_sub(1) {
        if !moved[x] && row[x] == Kind::Sand && row[x+1] == Kind::Air {
            row.swap(x, x+1);
            moved[x+1] = true;
        }
    }
}
fn main() {
    let mut row = [Kind::Sand, Kind::Air, Kind::Air, Kind::Stone];
    wind_right(&mut row);
    assert_eq!(row, [Kind::Air, Kind::Sand, Kind::Air, Kind::Stone]);
    assert_eq!(row.iter().filter(|k| **k == Kind::Sand).count(), 1);
    println!("one tick: one eligible grain, one bounded move");
}
