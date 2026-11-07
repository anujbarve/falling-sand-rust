fn within_twenty_percent(start:u64,end:u64)->bool {
    if start==0 { return end==0; }
    let difference=start.abs_diff(end);
    u128::from(difference)*100 <= u128::from(start)*20
}
#[test]
fn water_tolerance_has_explicit_boundaries() {
    assert!(within_twenty_percent(100,80));
    assert!(within_twenty_percent(100,120));
    assert!(!within_twenty_percent(100,79));
    assert!(!within_twenty_percent(100,121));
}
