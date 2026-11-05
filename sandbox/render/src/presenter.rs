//! Presenting a [`FrameBuffer`] to the screen: the [`Presenter`] trait and the default
//! macroquad texture presenter.

use crate::buffer::FrameBuffer;
use macroquad::prelude::*;

/// Something that can show a finished [`FrameBuffer`] on screen.
pub trait Presenter {
    /// Displays `fb`. Must not panic on a zero-sized target; it skips the frame instead.
    fn present(&mut self, fb: &FrameBuffer);
}

/// Largest rectangle `(x, y, w, h)` with the buffer's aspect ratio that fits a
/// `screen_w` by `screen_h` target, centred. Returns all zeros for any empty size.
pub fn fit_rect(fb_w: usize, fb_h: usize, screen_w: f32, screen_h: f32) -> (f32, f32, f32, f32) {
    if fb_w == 0 || fb_h == 0 || !(screen_w > 0.0) || !(screen_h > 0.0) {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let scale = (screen_w / fb_w as f32).min(screen_h / fb_h as f32);
    let (w, h) = (fb_w as f32 * scale, fb_h as f32 * scale);
    ((screen_w - w) / 2.0, (screen_h - h) / 2.0, w, h)
}

/// Default presenter: uploads the buffer to a macroquad texture and draws it letterboxed.
/// Must be created and used on the macroquad thread after the window exists.
pub struct MacroquadPresenter {
    texture: Option<Texture2D>,
    size: (usize, usize),
}

impl MacroquadPresenter {
    /// Creates a presenter; the texture is allocated on the first frame.
    pub fn new() -> MacroquadPresenter {
        MacroquadPresenter { texture: None, size: (0, 0) }
    }
}

impl Default for MacroquadPresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Presenter for MacroquadPresenter {
    fn present(&mut self, fb: &FrameBuffer) {
        if fb.w == 0 || fb.h == 0 {
            return;
        }
        let (x, y, w, h) = fit_rect(fb.w, fb.h, screen_width(), screen_height());
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        if self.texture.is_none() || self.size != (fb.w, fb.h) {
            let tex = Texture2D::from_rgba8(fb.w as u16, fb.h as u16, &fb.pixels);
            tex.set_filter(FilterMode::Nearest);
            self.texture = Some(tex);
            self.size = (fb.w, fb.h);
        } else if let Some(tex) = &self.texture {
            tex.update_from_bytes(fb.w as u32, fb.h as u32, &fb.pixels);
        }
        if let Some(tex) = &self.texture {
            draw_texture_ex(
                tex,
                x,
                y,
                WHITE,
                DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() },
            );
        }
    }
}
