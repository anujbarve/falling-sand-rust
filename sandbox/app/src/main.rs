//! Sandbox application: opens a macroquad window and runs the main loop.

use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Sandbox".to_owned(),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        clear_background(DARKBLUE);
        next_frame().await;
    }
}
