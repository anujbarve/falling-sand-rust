fn increment(values: &mut [u32]) {
    for value in values { *value += 1; }
}
#[test]
fn disjoint_jobs_match_sequential_reference() {
    let mut expected = vec![0_u32; 128];
    increment(&mut expected);
    let mut actual = vec![0_u32; 128];
    let (left, right) = actual.split_at_mut(64);
    std::thread::scope(|scope| {
        scope.spawn(|| increment(left));
        scope.spawn(|| increment(right));
    });
    assert_eq!(actual, expected);
}
