#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Air, Water, Sand, Stone }
fn erode(cells: &mut [Kind], width: usize, from: usize, to: usize) -> bool {
    if width == 0 || from >= cells.len() || to >= cells.len() { return false; }
    let (x,y)=(from%width,from/width);
    let (tx,ty)=(to%width,to/width);
    if cells[from]!=Kind::Sand || cells[to]!=Kind::Air || ty!=y+1 || x.abs_diff(tx)>1 {
        return false;
    }
    let wet = (x>0 && cells[from-1]==Kind::Water)
        || (x+1<width && from+1<cells.len() && cells[from+1]==Kind::Water);
    if !wet { return false; }
    cells.swap(from,to);
    true
}
fn main() {
    let mut cells=[Kind::Air,Kind::Air,Kind::Air,
        Kind::Stone,Kind::Sand,Kind::Water,
        Kind::Stone,Kind::Air,Kind::Stone];
    let before=cells.iter().filter(|k|**k==Kind::Water).count();
    assert!(erode(&mut cells,3,4,7));
    assert_eq!(cells[4],Kind::Air);
    assert_eq!(cells[7],Kind::Sand);
    assert_eq!(cells.iter().filter(|k|**k==Kind::Water).count(),before);
    println!("sand moved downhill; water count unchanged");
}
