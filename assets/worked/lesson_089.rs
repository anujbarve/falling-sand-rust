#[derive(Clone, Copy, Debug, PartialEq)]
struct Event { at_ms: u64, action: &'static str }
fn crossed(events: &[Event], before: u64, now: u64) -> Vec<Event> {
    events.iter().copied().filter(|e| before < e.at_ms && e.at_ms <= now).collect()
}
fn main() {
    let events = [Event { at_ms: 8_000, action: "rain fixture begins" },
        Event { at_ms: 18_000, action: "clear fixture resumes" },
        Event { at_ms: 28_000, action: "ignite dry exposed grass" },
        Event { at_ms: 36_000, action: "rain suppresses the fire" }];
    assert_eq!(crossed(&events, 7_900, 8_100).len(), 1);
    assert!(crossed(&events, 8_100, 8_200).is_empty());
    for event in crossed(&events, 0, 60_000) {
        println!("{:>2}s  {}", event.at_ms / 1000, event.action);
    }
}
