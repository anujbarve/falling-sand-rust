struct Cell {
    column: i32,
    row: i32,
}

fn main() {
    let mut cell = Cell { column: 12, row: 5 };
    println!("Before: {}, {}", cell.column, cell.row);
    cell.row += 1;
    println!("After: {}, {}", cell.column, cell.row);
}
