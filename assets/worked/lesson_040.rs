fn add_one(values: &mut [i32]) {
    for value in values { *value += 1; }
}
#[test]
fn split_regions_change_the_original() {
    let mut values = [0, 10, 20, 30];
    let (left, right) = values.split_at_mut(2);
    add_one(left);
    add_one(right);
    assert_eq!(values, [1, 11, 21, 31]);
}
#[test]
fn edge_splits_are_valid() {
    let mut values = [5, 6];
    let (left, right) = values.split_at_mut(0);
    assert!(left.is_empty());
    right[0] = 9;
    assert_eq!(values, [9, 6]);
}
