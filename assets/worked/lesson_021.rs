#[derive(Clone,Copy,PartialEq,Debug)]
enum Material { Air, Sand, Stone }
fn sand_count(cells:&[Material])->usize {
    cells.iter().filter(|material| **material==Material::Sand).count()
}
fn main() {
    let cells=vec![Material::Sand,Material::Air,Material::Sand,Material::Stone];
    println!("sand={}",sand_count(&cells));
    for (index,material) in cells.iter().enumerate() {
        println!("{index}: {material:?}");
    }
    println!("still owned, len={}",cells.len());
}
