#[derive(Default)]
struct Store { committed: u64, complete: Vec<u64> }
fn recover(store: &Store) -> Option<u64> {
    store.complete.iter().copied().find(|id| *id==store.committed)
}
fn save_model(crash_after: u8) -> Store {
    let mut s=Store {committed:1,complete:vec![1]};
    if crash_after==0 { return s; }
    // All payloads and manifest for generation 2 now validate.
    s.complete.push(2);
    if crash_after==1 { return s; }
    // Publish the commit reference only after completion.
    s.committed=2;
    s
}
fn main() {
    assert_eq!(recover(&save_model(0)),Some(1));
    assert_eq!(recover(&save_model(1)),Some(1));
    assert_eq!(recover(&save_model(2)),Some(2));
    println!("every interruption selects a complete generation");
}
