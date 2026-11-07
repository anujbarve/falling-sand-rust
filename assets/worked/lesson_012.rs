struct Cell {
    row: i32,
}

fn consume_cell(cell: Cell) {
    println!("inside: {}", cell.row);
}

fn main() {
    let cell = Cell { row: 5 };
    consume_cell(cell);
    println!("outside: {}", cell.row);
}
