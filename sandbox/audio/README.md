# sandbox_audio

Maps simulation events to sounds and plays them on a dedicated audio thread.

## Exposes

- `AudioCmd::Play { kind, gain }`: a sound request for the audio thread.
- `map_event(&Event) -> Option<AudioCmd>`: pure mapping. Explosions become `Boom` with gain rising toward 1 with energy, ignitions become `Crackle` at 0.2, `Sound` events keep their kind with gain clamped to 1. Zero, negative or non-finite intensity maps to `None`.
- `AudioHandle::start()` and `send_events(&[Event])`: the sim-side API.

## Behaviour

- Sending never blocks or panics. The queue holds 64 commands; extras are dropped.
- With no output device (or after one is lost) the handle silently discards commands and the sim keeps running.
- Sounds are synthesised noise and sine bursts via `rodio`.

## Tests

```bash
cargo test -p sandbox_audio
```
