//! Sandbox v2 application: a 1280x800 window cleared to a colour each frame.

use std::sync::Arc;

use sandbox_v2_gpu::{Gpu, WindowSurface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const CLEAR: wgpu::Color = wgpu::Color { r: 0.05, g: 0.08, b: 0.14, a: 1.0 };

struct State {
    window: Arc<Window>,
    gpu: Gpu,
    surface: WindowSurface,
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Sandbox v2")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();
        let Some((gpu, surface)) = Gpu::new_windowed(window.clone(), size.width, size.height)
        else {
            eprintln!("no GPU adapter available");
            event_loop.exit();
            return;
        };
        window.request_redraw();
        self.state = Some(State { window, gpu, surface });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed()
                    && matches!(event.logical_key, Key::Named(NamedKey::Escape)) =>
            {
                event_loop.exit()
            }
            WindowEvent::Resized(size) => s.surface.resize(&s.gpu, size.width, size.height),
            WindowEvent::RedrawRequested => {
                s.surface.clear_frame(&s.gpu, CLEAR);
                s.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::default()).expect("run app");
}
