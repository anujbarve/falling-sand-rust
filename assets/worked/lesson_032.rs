#[derive(Clone,Copy)]
enum Material { Air, Sand, Water, Stone }
struct Properties { density:u16, conductivity:u8 }
fn properties(m:Material)->Properties {
    match m {
        Material::Air=>Properties{density:0,conductivity:0},
        Material::Sand=>Properties{density:1600,conductivity:2},
        Material::Water=>Properties{density:1000,conductivity:4},
        Material::Stone=>Properties{density:2600,conductivity:3},
    }
}
struct Cell { temperature:i16,material:u8,moisture:u8,timer:u8,progress:u8,flags:u8,variant:u8 }
fn main() {
    let p=properties(Material::Sand);
    println!("sand density={}, conductivity={}",p.density,p.conductivity);
    println!("cell size={}, alignment={}",std::mem::size_of::<Cell>(),std::mem::align_of::<Cell>());
}
