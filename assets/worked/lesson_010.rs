use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Rusty — learning".to_owned(),
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
    let column = 12;
    let row = 5;
    loop {
        clear_background(Color::from_rgba(15, 26, 51, 255));
        draw_rectangle(column as f32 * 4.0, row as f32 * 4.0,
            4.0, 4.0, Color::from_rgba(226, 182, 121, 255));
        next_frame().await;
    }
}
