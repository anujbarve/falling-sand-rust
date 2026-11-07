fn main() {
    let mut row = 5;
    let cell_size = 4;
    println!("Before: row={row}");
    row += 1;
    println!("After: row={row}");
    println!("Drawing y={}", row * cell_size);
}
