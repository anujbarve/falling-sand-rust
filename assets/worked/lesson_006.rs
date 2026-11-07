fn screen_position(cell: i32) -> f32 {
    cell as f32 * 4.0
}

fn main() {
    let column = 12;
    let row = 5;
    println!("{}, {}", screen_position(column), screen_position(row));
}
