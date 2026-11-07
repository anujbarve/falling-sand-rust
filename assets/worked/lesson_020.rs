#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Material { Air, Sand, Stone }
fn move_into_air(cells:&mut [Material],from:usize,to:usize)->bool {
    if from==to || from>=cells.len() || to>=cells.len() { return false; }
    if cells[from]==Material::Sand && cells[to]==Material::Air {
        cells.swap(from,to); true
    } else { false }
}
fn main() {
    let mut cells=vec![Material::Sand,Material::Air,Material::Stone];
    let before=cells.iter().filter(|m| **m==Material::Sand).count();
    assert!(move_into_air(&mut cells,0,1));
    assert_eq!(cells,[Material::Air,Material::Sand,Material::Stone]);
    assert!(!move_into_air(&mut cells,1,2));
    assert_eq!(before,cells.iter().filter(|m| **m==Material::Sand).count());
    println!("one grain moved; count preserved");
}
