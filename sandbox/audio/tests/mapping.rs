use sandbox_audio::{map_event, AudioCmd};
use sandbox_core::event::{Event, SoundKind};

fn gain_of(e: Event) -> (SoundKind, f32) {
    match map_event(&e) {
        Some(AudioCmd::Play { kind, gain }) => (kind, gain),
        None => panic!("expected a command"),
    }
}

#[test]
fn explosion_maps_to_boom_with_scaled_gain() {
    let (k, g) = gain_of(Event::Explosion { x: 0.0, y: 0.0, radius: 3.0, energy: 50.0 });
    assert!(matches!(k, SoundKind::Boom));
    assert!(g > 0.0 && g <= 1.0);
    let (_, big) = gain_of(Event::Explosion { x: 0.0, y: 0.0, radius: 3.0, energy: 500.0 });
    assert!(big > g && big <= 1.0);
}

#[test]
fn ignite_maps_to_quiet_crackle() {
    let (k, g) = gain_of(Event::Ignite { x: 1, y: 2 });
    assert!(matches!(k, SoundKind::Crackle));
    assert_eq!(g, 0.2);
}

#[test]
fn silent_sound_maps_to_none() {
    let e = Event::Sound { kind: SoundKind::Flow, intensity: 0.0, x: 0.0, y: 0.0 };
    assert!(map_event(&e).is_none());
    let nan = Event::Sound { kind: SoundKind::Flow, intensity: f32::NAN, x: 0.0, y: 0.0 };
    assert!(map_event(&nan).is_none());
}

#[test]
fn sound_gain_is_clamped_and_keeps_kind() {
    let (k, g) = gain_of(Event::Sound { kind: SoundKind::Hiss, intensity: 7.0, x: 0.0, y: 0.0 });
    assert!(matches!(k, SoundKind::Hiss));
    assert_eq!(g, 1.0);
}
