use sandbox_render::buffer::FrameBuffer;
use sandbox_render::presenter::{fit_rect, Presenter};

struct Recorder {
    frames: Vec<(usize, usize, usize)>,
}

impl Presenter for Recorder {
    fn present(&mut self, fb: &FrameBuffer) {
        self.frames.push((fb.w, fb.h, fb.pixels.len()));
    }
}

#[test]
fn trait_is_object_safe_and_receives_buffer() {
    let mut r = Recorder { frames: vec![] };
    let fb = FrameBuffer::new(4, 3);
    let p: &mut dyn Presenter = &mut r;
    p.present(&fb);
    assert_eq!(r.frames, vec![(4, 3, 48)]);
}

#[test]
fn fit_rect_letterboxes_with_integer_free_scale() {
    // 160x120 into 800x800: scale 5, width fills, centred vertically.
    assert_eq!(fit_rect(160, 120, 800.0, 800.0), (0.0, 100.0, 800.0, 600.0));
}

#[test]
fn fit_rect_degenerate_sizes_return_empty() {
    assert_eq!(fit_rect(0, 120, 800.0, 600.0), (0.0, 0.0, 0.0, 0.0));
    assert_eq!(fit_rect(160, 120, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0));
}
