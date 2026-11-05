use sandbox_audio::AudioHandle;
use sandbox_core::event::{Event, SoundKind};
use std::time::{Duration, Instant};

#[test]
fn dead_channel_never_blocks_or_panics() {
    let h = AudioHandle::dead();
    let events = batch();
    let t = Instant::now();
    h.send_events(&events);
    assert!(t.elapsed() < Duration::from_millis(100), "took {:?}", t.elapsed());
}

#[test]
fn started_handle_never_blocks_or_panics() {
    let h = AudioHandle::start();
    let events = batch();
    let t = Instant::now();
    h.send_events(&events);
    assert!(t.elapsed() < Duration::from_millis(100), "took {:?}", t.elapsed());
    std::thread::sleep(Duration::from_millis(200));
    let t = Instant::now();
    h.send_events(&events);
    assert!(t.elapsed() < Duration::from_millis(100));
}

fn batch() -> Vec<Event> {
    (0..1000)
        .map(|i| {
            if i % 2 == 0 {
                Event::Ignite { x: i, y: 0 }
            } else {
                Event::Sound { kind: SoundKind::Splash, intensity: 0.5, x: 0.0, y: 0.0 }
            }
        })
        .collect()
}
