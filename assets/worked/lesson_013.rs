struct Cell { row: i32 }

fn inspect_cell(cell: &Cell) {
    println!("row={}", cell.row);
}
fn reset_cell(cell: &mut Cell) {
    cell.row = 5;
}
fn main() {
    let mut cell = Cell { row: 30 };
    inspect_cell(&cell);
    reset_cell(&mut cell);
    inspect_cell(&cell);
}
