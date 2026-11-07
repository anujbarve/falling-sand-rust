fn plan(accumulated_ms:u64, period_ms:u64, cap:u64)->(u64,u64) {
    assert!(period_ms>0);
    let due=accumulated_ms/period_ms;
    (due.min(cap), accumulated_ms%period_ms)
}
#[test]
fn capped_work_does_not_bank_physics_backlog() {
    assert_eq!(plan(30,50,2),(0,30));
    assert_eq!(plan(60,50,2),(1,10));
    assert_eq!(plan(260,50,2),(2,10));
    assert_eq!(plan(260,50,0),(0,10));
}
