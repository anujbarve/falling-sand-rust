fn allowed_drop<F: Fn(i32) -> bool>(maximum: i32, blocked: F) -> i32 {
    let mut distance = 0;
    for step in 1..=maximum {
        if blocked(step) { break; }
        distance = step;
    }
    distance
}
#[test]
fn the_first_obstacle_limits_the_whole_move() {
    assert_eq!(allowed_drop(3, |step| step == 2), 1);
    assert_eq!(allowed_drop(3, |_| false), 3);
    assert_eq!(allowed_drop(3, |step| step == 1), 0);
}
