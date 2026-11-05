use sandbox_core::material::Material;
use sandbox_render::palette::{base_color, cell_color};

#[test]
fn base_colors() {
    assert_eq!(base_color(Material::Sand), [194, 178, 128, 255]);
    let w = base_color(Material::Water);
    assert!(w[2] > w[0]);
    assert_eq!(base_color(Material::Empty)[3], 0);
}

#[test]
fn cell_color_deterministic_and_varied() {
    assert_eq!(cell_color(Material::Sand, 3, 5, 42), cell_color(Material::Sand, 3, 5, 42));
    let base = base_color(Material::Sand);
    let mut distinct = std::collections::HashSet::new();
    for y in 0..16 {
        for x in 0..16 {
            let c = cell_color(Material::Sand, x, y, 42);
            for i in 0..4 {
                assert!((c[i] as i32 - base[i] as i32).abs() <= 15);
            }
            distinct.insert(c);
        }
    }
    assert!(distinct.len() > 1);
}

#[test]
fn base_colors_distinct() {
    let all = [
        Material::Empty, Material::Sand, Material::Water, Material::Oil, Material::Stone,
        Material::Rock, Material::Wood, Material::Ash, Material::Metal, Material::Glass,
        Material::Lava, Material::Acid, Material::Gunpowder, Material::Ice, Material::Fire,
        Material::Steam, Material::Smoke, Material::Ember,
    ];
    let set: std::collections::HashSet<_> = all.iter().map(|m| base_color(*m)).collect();
    assert_eq!(set.len(), 18);
}
