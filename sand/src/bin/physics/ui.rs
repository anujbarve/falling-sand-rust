//! The toolbar (always on screen) and the settings window (Esc).
//!
//! The toolbar is drawn by hand with macroquad shapes: it is cheap and lets
//! the material buttons show their colour. The settings window uses
//! macroquad's immediate-mode `root_ui()` widgets with a dark skin.
//! Neither keeps any state of its own: they read and write `Settings` and
//! return what the user asked for as `Action`s, which `main` carries out.

use macroquad::prelude::*;
use macroquad::ui::{hash, root_ui, Skin};

use crate::settings::{Graphics, Quality, Settings, FPS_CAP_NAMES, GRID_NAMES, QUALITY_NAMES, WINDOW_NAMES};
use sand::physics::{clock::Clock, *};

pub const BAR_HEIGHT: f32 = 44.0;

/// Things the UI asks `main` to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    SetGrid(usize),
    SetWindow(usize),
    SetFullscreen(bool),
    SetSleeping(bool),
    NextScene,
    PreviousScene,
    Reset,
    Clear,
    TogglePause,
    ToggleMenu,
}

/// The brushes, in toolbar order, with their key and swatch colour.
pub const BRUSHES: [(u32, KeyCode, [u8; 3]); 10] = [
    (SAND, KeyCode::Key1, [219, 189, 115]),
    (WATER, KeyCode::Key2, [33, 92, 199]),
    (STONE, KeyCode::Key3, [110, 110, 117]),
    (AIR, KeyCode::Key4, [16, 18, 28]),
    (WOOD, KeyCode::Key5, [107, 66, 33]),
    (FIRE, KeyCode::Key6, [255, 140, 40]),
    (OIL, KeyCode::Key7, [102, 74, 26]),
    (LAVA, KeyCode::Key8, [240, 90, 20]),
    (ICE, KeyCode::Key9, [184, 219, 250]),
    (STEAM, KeyCode::Key0, [190, 196, 210]),
];

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

/// A flat button. Returns true when clicked this frame.
fn button(r: Rect, label: &str, active: bool, swatch: Option<Color>) -> bool {
    let (mx, my) = mouse_position();
    let hover = r.contains(vec2(mx, my));
    let bg = match (active, hover) {
        (true, _) => Color::from_rgba(70, 110, 200, 255),
        (false, true) => Color::from_rgba(52, 58, 76, 255),
        _ => Color::from_rgba(34, 38, 50, 255),
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    let mut text_x = r.x + 8.0;
    if let Some(c) = swatch {
        draw_rectangle(r.x + 6.0, r.y + 7.0, r.h - 14.0, r.h - 14.0, c);
        draw_rectangle_lines(r.x + 6.0, r.y + 7.0, r.h - 14.0, r.h - 14.0, 1.0, Color::from_rgba(0, 0, 0, 120));
        text_x = r.x + r.h - 2.0;
    }
    draw_text(label, text_x, r.y + r.h * 0.5 + 5.0, 17.0, Color::from_rgba(232, 234, 240, 255));
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// The top bar. Returns the actions clicked.
pub fn toolbar(brush: &mut u32, clock: &mut Clock, paused: bool, out: &mut Vec<Action>) {
    let w = screen_width();
    draw_rectangle(0.0, 0.0, w, BAR_HEIGHT, Color::from_rgba(12, 14, 20, 225));
    draw_line(0.0, BAR_HEIGHT, w, BAR_HEIGHT, 1.0, Color::from_rgba(70, 80, 110, 255));
    let (y, h) = (6.0, BAR_HEIGHT - 12.0);
    let mut x = 8.0;
    for (m, _, c) in BRUSHES {
        let label = if m == AIR { "erase" } else { NAMES[m as usize] };
        let bw = 26.0 + measure_text(label, None, 17, 1.0).width + 10.0;
        if button(Rect::new(x, y, bw, h), label, *brush == m, Some(rgb(c))) {
            *brush = m;
        }
        x += bw + 4.0;
    }

    // Right side, laid out from the right edge.
    let mut rx = w - 8.0;
    let mut right = |width: f32, label: &str, active: bool| {
        rx -= width;
        let clicked = button(Rect::new(rx, y, width, h), label, active, None);
        rx -= 4.0;
        clicked
    };
    if right(96.0, "Settings", false) {
        out.push(Action::ToggleMenu);
    }
    if right(30.0, ">", false) {
        clock.faster();
    }
    let speed = format!("{}x", clock.speed());
    if right(62.0, &speed, clock.speed_index == sand::physics::clock::REAL_TIME) {
        clock.real_time();
    }
    if right(30.0, "<", false) {
        clock.slower();
    }
    if right(74.0, if paused { "Play" } else { "Pause" }, paused) {
        out.push(Action::TogglePause);
    }
}

fn skin() -> Skin {
    let ui = root_ui();
    let text = Color::from_rgba(228, 230, 238, 255);
    let label_style = ui.style_builder().text_color(text).font_size(16).build();
    let window_style = ui
        .style_builder()
        .color(Color::from_rgba(16, 18, 26, 240))
        .color_inactive(Color::from_rgba(16, 18, 26, 225))
        .build();
    let titlebar = ui
        .style_builder()
        .color(Color::from_rgba(30, 36, 52, 255))
        .color_inactive(Color::from_rgba(30, 36, 52, 255))
        .text_color(text)
        .font_size(16)
        .build();
    // Build every style from this one borrow of the UI (asking `root_ui()`
    // again while `ui` is alive would be a second RefCell borrow: a panic).
    let widget = |base: Color| {
        ui.style_builder()
            .color(base)
            .color_hovered(Color::from_rgba(58, 66, 88, 255))
            .color_clicked(Color::from_rgba(70, 110, 200, 255))
            .color_selected(Color::from_rgba(70, 110, 200, 255))
            .color_selected_hovered(Color::from_rgba(90, 130, 220, 255))
            .text_color(text)
            .font_size(16)
            .build()
    };
    let base = Color::from_rgba(38, 43, 58, 255);
    Skin {
        label_style,
        window_style,
        window_titlebar_style: titlebar,
        button_style: widget(base),
        combobox_style: widget(base),
        checkbox_style: widget(base),
        group_style: widget(Color::from_rgba(22, 25, 34, 255)),
        ..ui.default_skin()
    }
}

pub struct Menu {
    skin: Option<Skin>,
}

impl Menu {
    pub fn new() -> Menu {
        Menu { skin: None }
    }

    /// The settings window. Edits `settings` in place; changes that need
    /// more than a new value (new grid, window size...) become `Action`s.
    pub fn draw(&mut self, settings: &mut Settings, out: &mut Vec<Action>) {
        let skin = self.skin.get_or_insert_with(skin);
        let mut ui = root_ui();
        ui.push_skin(skin);
        let size = vec2(380.0, 680.0);
        let pos = vec2(screen_width() - size.x - 16.0, BAR_HEIGHT + 12.0);
        let before = settings.clone();
        ui.window(hash!(), pos, size, |ui| {
            ui.label(None, "GRAPHICS");
            let mut q = settings.quality.index();
            ui.combo_box(hash!(), "Quality", &QUALITY_NAMES, &mut q);
            if q != settings.quality.index() && Quality::from_index(q) != Quality::Custom {
                settings.quality = Quality::from_index(q);
                settings.graphics = Graphics::preset(settings.quality);
            }
            let g = &mut settings.graphics;
            ui.checkbox(hash!(), "Bloom (glow around hot things)", &mut g.bloom);
            let mut bloom_size = match g.bloom_divisor {
                2 => 0,
                4 => 1,
                _ => 2,
            };
            ui.combo_box(hash!(), "Bloom resolution", &["1/2", "1/4", "1/8"], &mut bloom_size);
            g.bloom_divisor = [2, 4, 8][bloom_size];
            ui.slider(hash!(), "Bloom strength", 0.0..2.0, &mut g.bloom_strength);
            ui.checkbox(hash!(), "Surface lighting + shadows", &mut g.shading);
            ui.checkbox(hash!(), "Liquid depth", &mut g.depth);
            ui.checkbox(hash!(), "Grain + shimmer", &mut g.grain);
            ui.checkbox(hash!(), "Glow of hot solids", &mut g.incandescence);
            ui.checkbox(hash!(), "Vignette", &mut g.vignette);
            ui.slider(hash!(), "Exposure", 0.5..2.0, &mut g.exposure);

            ui.separator();
            ui.label(None, "DISPLAY");
            ui.combo_box(hash!(), "Window", &WINDOW_NAMES, &mut settings.window);
            ui.checkbox(hash!(), "Fullscreen (F11)", &mut settings.fullscreen);
            ui.combo_box(hash!(), "Frame cap", &FPS_CAP_NAMES, &mut settings.fps_cap);
            ui.checkbox(hash!(), "Heat view (H)", &mut settings.heat_view);
            ui.checkbox(hash!(), "Performance stats", &mut settings.show_stats);

            ui.separator();
            ui.label(None, "AUDIO");
            ui.checkbox(hash!(), "Sound (M)", &mut settings.audio);
            ui.slider(hash!(), "Volume", 0.0..1.0, &mut settings.volume);

            ui.separator();
            ui.label(None, "SIMULATION");
            ui.combo_box(hash!(), "Grid", &GRID_NAMES, &mut settings.grid);
            ui.slider(hash!(), "GPU budget ms/frame", 2.0..16.0, &mut settings.gpu_budget_ms);
            ui.checkbox(hash!(), "Skip still regions", &mut settings.sleeping);

            ui.separator();
            if ui.button(None, "Reset scene (R)") {
                out.push(Action::Reset);
            }
            ui.same_line(0.0);
            if ui.button(None, "Clear (C)") {
                out.push(Action::Clear);
            }
            ui.same_line(0.0);
            if ui.button(None, "Close (Esc)") {
                out.push(Action::ToggleMenu);
            }
        });
        ui.pop_skin();
        drop(ui);

        if settings.graphics != before.graphics {
            settings.refresh_quality();
        }
        if settings.grid != before.grid {
            out.push(Action::SetGrid(settings.grid));
        }
        if settings.window != before.window {
            out.push(Action::SetWindow(settings.window));
        }
        if settings.fullscreen != before.fullscreen {
            out.push(Action::SetFullscreen(settings.fullscreen));
        }
        if settings.sleeping != before.sleeping {
            out.push(Action::SetSleeping(settings.sleeping));
        }
    }

    /// Is the mouse over the settings window? (Then it must not paint.)
    pub fn wants_mouse(&self) -> bool {
        root_ui().is_mouse_over(Vec2::from(mouse_position()))
    }
}

/// Small text panel with what the frame costs.
pub fn stats(lines: &[String]) {
    let h = 8.0 + 20.0 * lines.len() as f32;
    let y = screen_height() - h - 8.0;
    draw_rectangle(8.0, y, 430.0, h, Color::from_rgba(10, 12, 18, 200));
    for (i, l) in lines.iter().enumerate() {
        draw_text(l, 16.0, y + 20.0 + 20.0 * i as f32, 17.0, Color::from_rgba(200, 210, 225, 255));
    }
}
