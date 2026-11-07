#[derive(Clone, Copy, Debug, PartialEq)]
enum Material { Water, Ice }
fn phase(material: Material, temperature: i16) -> Material {
    match (material, temperature) {
        (Material::Water, t) if t < 0 => Material::Ice,
        (Material::Ice, t) if t > 0 => Material::Water,
        _ => material,
    }
}
#[test]
fn freezing_is_reversible_and_zero_is_stable() {
    assert_eq!(phase(Material::Water, -1), Material::Ice);
    assert_eq!(phase(Material::Ice, 1), Material::Water);
    assert_eq!(phase(Material::Water, 0), Material::Water);
    assert_eq!(phase(Material::Ice, 0), Material::Ice);
}
