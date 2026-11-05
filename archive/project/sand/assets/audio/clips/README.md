# Recorded clips used by the game

All clips are CC0 (public domain dedication): no attribution is required,
and they are listed here for the record. They are embedded in the binary
with `include_bytes!` (see `src/audio/bank.rs`).

| File | Source | Used for |
|---|---|---|
| bubbles/bubbles-single1.wav, bubbles-single2.wav, bubbles-single3.wav | https://opengameart.org/content/bubble-sound-effects (CC0; credit to Brian MacIntosh is appreciated, not required) | boiling bubbles, drips |
| bubbles/bubbles-loop2-amp.wav | same | bubbling-water bed (loop) |
| fire/fire-1.wav | https://opengameart.org/content/fire-crackling (CC0) | fire bed (loop), crackles, ignition |
| water/splash1.wav, water/drowning.wav | https://opengameart.org/content/water-splash-and-sand-footsteps (CC0) | not used yet |

Not found under CC0 (yet): sand pouring, frying/sizzle. Those sounds are
still synthesised (`src/audio/bank.rs`).
