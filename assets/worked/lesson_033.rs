#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Material { Air, Oil, Water, Sand, Stone }
fn density(m:Material)->u16 {match m {Material::Air=>0,Material::Oil=>800,
    Material::Water=>1000,Material::Sand=>1600,Material::Stone=>2600}}
fn may_sink(a:Material,b:Material)->bool {
    let moving=matches!(a,Material::Oil|Material::Water|Material::Sand);
    let displaced=matches!(b,Material::Air|Material::Oil|Material::Water);
    moving && displaced && density(a)>density(b)
}
fn main() {
    assert!(may_sink(Material::Sand,Material::Water));
    assert!(!may_sink(Material::Oil,Material::Water));
    assert!(may_sink(Material::Water,Material::Oil));
    assert!(!may_sink(Material::Sand,Material::Stone));
    let mut pair=[Material::Sand,Material::Water];
    if may_sink(pair[0],pair[1]) {pair.swap(0,1);}
    println!("{pair:?}");
}
