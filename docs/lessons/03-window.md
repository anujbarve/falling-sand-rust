# 03 — Open a window and keep drawing

**Result:** a window with the approved night-sky base colour. **Prerequisites:** modules 01–02.

## Start with a probe

A variable begins at 5. A loop adds 1 to it on each iteration. After three iterations, what value does it hold?

1. 6.
2. 5.
3. 8.
0. I don't know.

Wait. If loops are new, trace a finite terminal example before introducing the window's continuous loop.

## Plan the lesson

We need a library to create the window. Then we need repeated drawing while the window is open. Learn dependencies, function calls and `loop` to solve those two problems.

## Work

Under the existing `[dependencies]` heading of `game/Cargo.toml`, add:

```toml
macroquad = "=0.4.16"
```

Do not add a second dependencies heading. The exact version matches the game design. Cargo resolves transitive dependencies in `Cargo.lock`; keep that lockfile with the eventual application.

Replace `src/main.rs`:

```rust
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Ecological sandbox — learning".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        window_resizable: false,
        sample_count: 1,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(Color::from_rgba(15, 26, 51, 255));
        next_frame().await;
    }
}
```

Run `cargo check`, then `cargo run`. Close the window before editing and rerunning.

## Build the understanding

First identify imports and function calls. Then explain `window_conf`: it returns configuration data, with fields overriding defaults. This is the first encounter with struct syntax; module 04 creates our own struct.

The attribute connects macroquad to our entry function. Keep the initial `async`/`.await` explanation narrow: macroquad's `next_frame().await` yields control so the framework can present the frame and process events. Full asynchronous programming is not a prerequisite for this module.

`clear_background` replaces the frame's background each iteration. Colour channels here are red, green, blue and opacity; these numbers represent `#0f1a33`. Ask the learner which call they would change to change colour and which field changes the title.

This is a flat background, not the final sky gradient. The non-resizable, low-DPI configuration is a temporary teaching restriction, not a change to the game design.

## Exit check

Change the title and one colour channel independently. Restore the night colour. Explain why we keep yielding to the next frame instead of running an endless loop with no yield. If the window fails, record the actual OS/error before guessing a fix.

Next: [04 — One cell and borrowing](04-cell-and-borrowing.md).

Sources: [macroquad 0.4.16](https://docs.rs/macroquad/0.4.16/macroquad/), [window configuration](https://docs.rs/macroquad/0.4.16/macroquad/conf/struct.Conf.html).
