mod screen;

use std::sync::Arc;

use sugar_layout::{Id, Layout, Rect};
use sugar_ui::{Painter, Renderer};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct Gpu {
    window: Arc<Window>,
    renderer: Renderer<'static>,
}

struct App {
    gpu: Option<Gpu>,
    layout: Layout,
    placed: Vec<(Id, Rect)>,
    painter: Painter,
}

impl App {
    fn relayout(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let size = gpu.window.inner_size();
        let scale = gpu.window.scale_factor() as f32;
        gpu.renderer.resize(size.width, size.height);
        let logical = Rect::new(
            0.0,
            0.0,
            size.width as f32 / scale,
            size.height as f32 / scale,
        );
        let body = screen::inset(logical);
        self.layout.solve(body, &mut self.placed);
        self.painter.begin(scale);
        screen::paint(&mut self.painter, logical, &self.placed);
        gpu.window.request_redraw();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("DJ Sugar")
            .with_min_inner_size(winit::dpi::LogicalSize::new(320.0, 320.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => return fail(event_loop, &e),
        };
        let size = window.inner_size();
        match Renderer::new(window.clone(), size.width, size.height) {
            Ok(renderer) => {
                self.gpu = Some(Gpu { window, renderer });
                self.relayout();
            }
            Err(e) => fail(event_loop, &e),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.relayout(),
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.window.pre_present_notify();
                    gpu.renderer
                        .render(self.painter.quads(), screen::BACKGROUND);
                }
            }
            _ => {}
        }
    }
}

fn fail(event_loop: &ActiveEventLoop, e: &dyn std::fmt::Display) {
    eprintln!("djsugar: {e}");
    event_loop.exit();
}

fn main() {
    let event_loop = match EventLoop::new() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("djsugar: {e}");
            std::process::exit(1);
        }
    };
    let mut app = App {
        gpu: None,
        layout: Layout::new(&screen::tree()),
        placed: Vec::with_capacity(sugar_layout::MAX_MODULES),
        painter: Painter::default(),
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("djsugar: {e}");
        std::process::exit(1);
    }
}
