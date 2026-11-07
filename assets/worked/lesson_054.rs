fn try_use(now: u64, ready_at: &mut u64, cooldown: u64) -> bool {
    if now < *ready_at { return false; }
    *ready_at = now.saturating_add(cooldown);
    true
}
#[test]
fn cooldown_uses_a_world_tick_deadline() {
    let mut ready=0;
    assert!(try_use(10,&mut ready,24_000));
    assert_eq!(ready,24_010);
    assert!(!try_use(24_009,&mut ready,24_000));
    assert!(try_use(24_010,&mut ready,24_000));
}
