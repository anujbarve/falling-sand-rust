#[derive(Clone, Copy, Debug, PartialEq)]
enum Material { Snow, Water }
#[derive(Clone, Copy)]
struct Cell { material: Material, temperature: i16 }
fn melt(cell: &mut Cell) {
    if cell.material == Material::Snow && cell.temperature > 0 {
        cell.material = Material::Water;
    }
}
#[test]
fn warm_snow_melts_but_cold_snow_does_not() {
    let mut warm = Cell { material: Material::Snow, temperature: 2 };
    let mut cold = Cell { material: Material::Snow, temperature: -2 };
    melt(&mut warm); melt(&mut cold);
    assert_eq!(warm.material, Material::Water);
    assert_eq!(cold.material, Material::Snow);
    assert_eq!(warm.temperature, 2);
}
