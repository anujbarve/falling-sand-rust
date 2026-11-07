fn main() {
    let column: i32 = 12;
    let row: i32 = 5;
    let cell_size: f32 = 4.0;
    let screen_x: f32 = column as f32 * cell_size;
    let screen_y: f32 = row as f32 * cell_size;
    let tick_seconds: f64 = 0.05;
    println!("Drawing: {screen_x}, {screen_y}");
    println!("Tick duration: {tick_seconds} seconds");
}
