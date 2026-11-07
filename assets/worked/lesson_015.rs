struct Cell { row: i32 }
fn tick_cell(cell: &mut Cell, floor_row: i32) {
    if cell.row < floor_row - 1 {
        cell.row = (cell.row + 3).min(floor_row - 1);
    }
}
fn main() {
    for (start, expected) in [(5, 8), (158, 159), (159, 159)] {
        let mut cell = Cell { row: start };
        tick_cell(&mut cell, 160);
        assert_eq!(cell.row, expected);
        println!("{start} -> {}", cell.row);
    }
}
