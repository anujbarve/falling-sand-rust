#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Material { Air, Sand, Stone }
fn label(material: Material) -> &'static str {
    match material {
        Material::Air => "air",
        Material::Sand => "sand",
        Material::Stone => "stone",
    }
}
fn main() {
    for material in [Material::Air, Material::Sand, Material::Stone] {
        println!("{:?}: {}", material, label(material));
    }
}
