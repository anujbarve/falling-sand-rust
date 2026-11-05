use sandbox_core::material::Material;
use sandbox_core::world::World;
use sandbox_render::buffer::FrameBuffer;
use sandbox_render::palette::cell_color;

#[test]
fn draws_cells_into_pixels() {
    let mut world = World::new(4, 4);
    world.cells.get_mut(1, 2).unwrap().mat = Material::Stone;
    let mut fb = FrameBuffer::new(4, 4);
    fb.pixels.iter_mut().for_each(|b| *b = 99);
    fb.draw_world(&world, 7);
    let i = (2 * 4 + 1) * 4;
    assert_eq!(&fb.pixels[i..i + 4], &cell_color(Material::Stone, 1, 2, 7));
    assert_eq!(fb.pixels[3], 0);
    assert_eq!(fb.pixels.len(), 4 * 4 * 4);
}
