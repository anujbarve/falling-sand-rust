#[derive(Clone, Copy, Debug, PartialEq)]
enum Material { Water, Steam, Lava, Basalt }
fn eligible_contact(a: Material, b: Material) -> (Material, Material) {
    match (a,b) {
        (Material::Lava, Material::Water) => (Material::Basalt, Material::Steam),
        (Material::Water, Material::Lava) => (Material::Steam, Material::Basalt),
        _ => (a,b),
    }
}
#[test]
fn contact_is_symmetric_and_occupancy_preserving() {
    assert_eq!(eligible_contact(Material::Lava, Material::Water),
        (Material::Basalt, Material::Steam));
    assert_eq!(eligible_contact(Material::Water, Material::Lava),
        (Material::Steam, Material::Basalt));
}
