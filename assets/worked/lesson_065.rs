fn grow_one(occupied:&mut [bool], target:usize)->bool {
    let Some(slot)=occupied.get_mut(target) else { return false; };
    if *slot { return false; }
    *slot=true; true
}
#[test]
fn growth_does_not_overwrite_new_obstacles() {
    let mut world=[false,false,true,false];
    assert!(grow_one(&mut world,1));
    assert!(!grow_one(&mut world,2));
    assert!(!grow_one(&mut world,9));
    assert_eq!(world,[false,true,true,false]);
}
