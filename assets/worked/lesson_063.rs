fn downhill(left_height:i32, current_height:i32, right_height:i32)->Option<i32> {
    if left_height < current_height && left_height <= right_height { Some(-1) }
    else if right_height < current_height { Some(1) }
    else { None }
}
#[test]
fn direction_requires_a_lower_neighbour() {
    assert_eq!(downhill(4,5,7),Some(-1));
    assert_eq!(downhill(7,5,3),Some(1));
    assert_eq!(downhill(5,5,5),None);
}
