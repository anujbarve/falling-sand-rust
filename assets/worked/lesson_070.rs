#[derive(Debug,PartialEq)]
enum Surface { Soil, Stone }
fn surface_for_slope(left:i32, here:i32, right:i32, max_step:i32)->Surface {
    let steep=(i64::from(here)-i64::from(left)).abs()
        .max((i64::from(right)-i64::from(here)).abs());
    if steep>i64::from(max_step) { Surface::Stone } else { Surface::Soil }
}
#[test]
fn loose_surface_obeys_the_declared_rest_limit() {
    assert_eq!(surface_for_slope(10,11,12,1),Surface::Soil);
    assert_eq!(surface_for_slope(10,13,14,1),Surface::Stone);
    assert_eq!(surface_for_slope(10,10,10,1),Surface::Soil);
}
