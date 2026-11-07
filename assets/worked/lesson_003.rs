fn main() {
    let column = 12;
    let row = 5;
    let cell_size = 4;
    let screen_x = column * cell_size;
    let screen_y = row * cell_size;
    println!("Cell: {column}, {row}");
    println!("Drawing position: {screen_x}, {screen_y}");
}
