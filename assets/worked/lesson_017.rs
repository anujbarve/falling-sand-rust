#[derive(Clone, Copy, PartialEq)]
enum Material { Air, Sand, Stone }
struct World { width: usize, height: usize, cells: Vec<Material> }
fn main() {
    let mut world = World { width: 8, height: 4,
        cells: vec![Material::Air; 8 * 4] };
    let floor = (world.height - 1) * world.width;
    for column in 0..world.width { world.cells[floor + column] = Material::Stone; }
    for row in 0..world.height {
        for column in 0..world.width {
            let mark = match world.cells[row * world.width + column] {
                Material::Air => '.', Material::Sand => 's', Material::Stone => '#',
            };
            print!("{mark}");
        }
        println!();
    }
}
