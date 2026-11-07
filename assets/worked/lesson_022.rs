#[derive(Clone,Copy,Default)]
struct Cell { sand: bool, moved: bool }
fn tick(column:&mut [Cell]) {
    for cell in column.iter_mut() { cell.moved=false; }
    for row in (0..column.len().saturating_sub(1)).rev() {
        if column[row].sand && !column[row].moved && !column[row+1].sand {
            column.swap(row,row+1);
            column[row+1].moved=true;
        }
    }
}
fn main() {
    let mut column=[Cell::default();6];
    column[0].sand=true;
    tick(&mut column);
    assert!(column[1].sand);
    assert_eq!(column.iter().filter(|c|c.sand).count(),1);
    println!("one tick, one row");
}
