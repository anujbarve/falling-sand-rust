fn to_view(world:(i32,i32), left:i32, top:i32)->(i32,i32) {
    (world.0-left, top-world.1)
}
fn to_world(view:(i32,i32), left:i32, top:i32)->(i32,i32) {
    (view.0+left, top-view.1)
}
#[test]
fn world_view_conversion_round_trips() {
    let world=(-3,12);
    let view=to_view(world,-10,20);
    assert_eq!(view,(7,8));
    assert_eq!(to_world(view,-10,20),world);
    assert_eq!((-1_i32).div_euclid(64),-1);
    assert_eq!((-1_i32).rem_euclid(64),63);
}
