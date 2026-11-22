mod screen;

use sugar_layout::{Id, Layout, Rect};
use sugar_ui::{Fonts, Painter};

static FONT: &[u8] = include_bytes!("../../../res/fonts/Inter.ttf");

struct Ui {
    layout: Layout,
    placed: Vec<(Id, Rect)>,
    painter: Painter,
}

impl Ui {
    fn new() -> Self {
        Self {
            layout: Layout::new(&screen::tree()),
            placed: Vec::with_capacity(sugar_layout::MAX_MODULES),
            painter: Painter::new(Fonts::new(FONT)),
        }
    }

    fn relayout(&mut self, width: u32, height: u32, scale: f32) {
        let logical = Rect::new(0.0, 0.0, width as f32 / scale, height as f32 / scale);
        self.layout.solve(screen::inset(logical), &mut self.placed);
        self.painter.begin(scale);
        screen::paint(&mut self.painter, logical, &self.placed);
    }
}

fn fail(e: &dyn std::fmt::Display) -> ! {
    eprintln!("djsugar: {e}");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
mod platform {
    use sugar_shell::{App, Event, Frame};
    use sugar_ui::Renderer;

    use crate::{Ui, fail, screen};

    struct Linux {
        ui: Ui,
        renderer: Renderer<'static>,
    }

    impl App for Linux {
        fn event(&mut self, event: Event) -> bool {
            match event {
                Event::Resize { width, height, scale } => {
                    self.renderer.resize(width, height);
                    self.ui.relayout(width, height, scale);
                    true
                }
                _ => false,
            }
        }

        fn draw(&mut self, present: &mut dyn FnMut(Frame<'_>)) {
            self.renderer.render(&mut self.ui.painter, screen::BACKGROUND);
            self.renderer
                .read(|p| present(Frame { bytes: p.bytes, stride: p.stride, width: p.width, height: p.height }));
        }
    }

    pub fn main() {
        let (width, height) = (1280, 800);
        let renderer = Renderer::offscreen(width, height).unwrap_or_else(|e| fail(&e));
        let mut app = Linux { ui: Ui::new(), renderer };
        if let Err(e) = sugar_shell::run("DJ Sugar", width, height, &mut app) {
            fail(&e);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use std::sync::Arc;

    use sugar_ui::Renderer;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::window::{Window, WindowId};

    use crate::{Ui, fail, screen};

    struct Gpu {
        window: Arc<Window>,
        renderer: Renderer<'static>,
    }

    struct Native {
        ui: Ui,
        gpu: Option<Gpu>,
    }

    impl Native {
        fn relayout(&mut self) {
            let Some(gpu) = &mut self.gpu else { return };
            let size = gpu.window.inner_size();
            gpu.renderer.resize(size.width, size.height);
            self.ui.relayout(size.width, size.height, gpu.window.scale_factor() as f32);
            gpu.window.request_redraw();
        }
    }

    impl ApplicationHandler for Native {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.gpu.is_some() {
                return;
            }
            let attrs = Window::default_attributes()
                .with_title("DJ Sugar")
                .with_min_inner_size(winit::dpi::LogicalSize::new(320.0, 320.0));
            let window = Arc::new(event_loop.create_window(attrs).unwrap_or_else(|e| fail(&e)));
            let size = window.inner_size();
            let renderer = Renderer::new(window.clone(), size.width, size.height).unwrap_or_else(|e| fail(&e));
            self.gpu = Some(Gpu { window, renderer });
            self.relayout();
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.relayout(),
                WindowEvent::RedrawRequested => {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.window.pre_present_notify();
                        gpu.renderer.render(&mut self.ui.painter, screen::BACKGROUND);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn main() {
        let event_loop = EventLoop::new().unwrap_or_else(|e| fail(&e));
        let mut app = Native { ui: Ui::new(), gpu: None };
        event_loop.run_app(&mut app).unwrap_or_else(|e| fail(&e));
    }
}

fn main() {
    platform::main();
}
