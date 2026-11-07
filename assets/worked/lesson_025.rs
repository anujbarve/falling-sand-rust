mod world {
    #[derive(Clone,Copy,Debug,PartialEq,Eq)]
    pub enum Material { Air, Sand, Stone }
    pub struct World { cells: Vec<Material> }
    impl World {
        pub fn new(size:usize)->Self {Self{cells:vec![Material::Air;size]}}
        pub fn len(&self)->usize {self.cells.len()}
    }
}
use world::World;
fn main() {
    let world=World::new(32);
    println!("world slots={}",world.len());
}
