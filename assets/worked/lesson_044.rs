#[derive(Clone, Copy, Debug, PartialEq)]
enum Material { Wood, Ash }
struct Cell { material: Material, burning: bool, fuel: u8 }
fn burn_tick(cell: &mut Cell) {
    if !cell.burning { return; }
    cell.fuel = cell.fuel.saturating_sub(1);
    if cell.fuel == 0 {
        cell.material = Material::Ash;
        cell.burning = false;
    }
}
#[test]
fn wood_remains_wood_until_consumed() {
    let mut beam = Cell { material: Material::Wood, burning: true, fuel: 2 };
    burn_tick(&mut beam);
    assert_eq!(beam.material, Material::Wood);
    burn_tick(&mut beam);
    assert_eq!(beam.material, Material::Ash);
    assert!(!beam.burning);
}
