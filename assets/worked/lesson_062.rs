#[derive(Clone,Copy,Debug,PartialEq)]
enum Material { Wood, RottenWood, Soil }
fn decay(material:Material,progress:u16,wet:bool)->(Material,u16) {
    if !wet { return (material,progress); }
    let next=progress.saturating_add(10);
    if next<100 { return (material,next); }
    match material {
        Material::Wood=>(Material::RottenWood,0),
        Material::RottenWood=>(Material::Soil,0),
        Material::Soil=>(Material::Soil,0),
    }
}
#[test]
fn one_visit_crosses_only_one_stage() {
    assert_eq!(decay(Material::Wood,95,true),(Material::RottenWood,0));
    assert_eq!(decay(Material::Wood,95,false),(Material::Wood,95));
}
