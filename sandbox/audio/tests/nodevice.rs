use sandbox_audio::AudioHandle;
use sandbox_core::event::{Event, SoundKind};
use std::time::{Duration, Instant};

#[test]
fn send_events_never_blocks_or_panics() {
    let h = AudioHandle::start();
    let events: Vec<Event> = (0..1000)
        .map(|i| {
            if i % 2 == 0 {
                Event::Ignite { x: i, y: 0 }
            } else {
                Event::Sound { kind: SoundKind::Splash, intensity: 0.5, x: 0.0, y: 0.0 }
            }
        })
        .collect();
    let t = Instant::now();
    h.send_events(&events);
    assert!(t.elapsed() < Duration::from_millis(100), "took {:?}", t.elapsed());
    // Still fine after the audio thread has had time to give up or start.
    std::thread::sleep(Duration::from_millis(200));
    let t = Instant::now();
    h.send_events(&events);
    assert!(t.elapsed() < Duration::from_millis(100));
}
