fn first_surface(column_top_down: &[bool]) -> Option<usize> {
    column_top_down.iter().position(|occupied| *occupied)
}
#[test]
fn the_roof_intercepts_the_strike() {
    let column=[false,false,true,false,false,true];
    assert_eq!(first_surface(&column),Some(2));
    assert_eq!(first_surface(&[false,false,false]),None);
}
