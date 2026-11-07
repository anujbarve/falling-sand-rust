fn condensation_distance(mut temperature: i16, threshold: i16) -> i32 {
    let mut height = 0;
    while temperature >= threshold {
        temperature -= 2;
        height += 2;
    }
    height
}
#[test]
fn cooling_sets_a_bounded_travel_distance() {
    assert_eq!(condensation_distance(130, 100), 32);
    assert_eq!(condensation_distance(150, 100), 52);
}
