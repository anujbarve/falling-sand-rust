fn material_slot(x:i32,y:i32,left:i32,top:i32)->Option<usize> {
    const SLOT:i32=32;
    const COUNT:i32=10;
    if x<left || y<top || y>=top+SLOT || x>=left+COUNT*SLOT {return None;}
    Some(((x-left)/SLOT) as usize)
}
fn main() {
    assert_eq!(material_slot(100,200,100,200),Some(0));
    assert_eq!(material_slot(132,200,100,200),Some(1));
    assert_eq!(material_slot(420,200,100,200),None);
    assert_eq!(material_slot(100,232,100,200),None);
    let clicked_slot=material_slot(140,210,100,200);
    let paint_world=clicked_slot.is_none();
    assert!(!paint_world);
    println!("UI click selects slot {:?}; world paint={paint_world}",clicked_slot);
}
