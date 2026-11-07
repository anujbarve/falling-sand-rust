fn inside_blast(dx: i32, dy: i32, radius: i32) -> bool {
    let dx=i64::from(dx); let dy=i64::from(dy); let r=i64::from(radius);
    radius >= 0 && dx*dx + dy*dy <= r*r
}
fn consume_trigger(armed: &mut bool) -> bool {
    if !*armed { return false; }
    *armed=false;
    true
}
#[test]
fn selection_is_bounded_and_trigger_is_single_use() {
    assert!(inside_blast(3,4,5));
    assert!(!inside_blast(4,4,5));
    let mut armed=true;
    assert!(consume_trigger(&mut armed));
    assert!(!consume_trigger(&mut armed));
}
