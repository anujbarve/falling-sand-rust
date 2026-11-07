fn index(column: i32, row: i32, width: i32, height: i32) -> Option<usize> {
    if width <= 0 || height <= 0 || column < 0 || row < 0
        || column >= width || row >= height { return None; }
    Some(row as usize * width as usize + column as usize)
}
fn main() {
    assert_eq!(index(0, 0, 32, 18), Some(0));
    assert_eq!(index(31, 17, 32, 18), Some(575));
    for (x,y) in [(-1,0),(32,0),(0,-1),(0,18)] {
        assert_eq!(index(x,y,32,18), None);
    }
    println!("bounds and stride checks passed");
}
