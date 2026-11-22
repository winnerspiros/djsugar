mod screen;

use sugar_layout::{Id, Layout, Rect};
use sugar_ui::{Fonts, Painter};

static FONT: &[u8] = include_bytes!("../../../res/fonts/Inter.ttf");

#[derive(Debug)]
pub struct Ui {
    layout: Layout,
    placed: Vec<(Id, Rect)>,
    painter: Painter,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            layout: Layout::new(&screen::tree()),
            placed: Vec::with_capacity(sugar_layout::MAX_MODULES),
            painter: Painter::new(Fonts::new(FONT)),
        }
    }
}

impl Ui {
    pub fn relayout(&mut self, width: u32, height: u32, scale: f32) {
        let logical = Rect::new(0.0, 0.0, width as f32 / scale, height as f32 / scale);
        self.layout.solve(screen::inset(logical), &mut self.placed);
        self.painter.begin(scale);
        screen::paint(&mut self.painter, logical, &self.placed);
    }

    pub fn render(&mut self, renderer: &mut sugar_ui::Renderer<'_>) {
        renderer.render(&mut self.painter, screen::BACKGROUND);
    }
}

pub fn fail(e: &dyn std::fmt::Display) -> ! {
    eprintln!("djsugar: {e}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
pub mod native {
    use std::sync::Arc;

    use sugar_ui::Renderer;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::window::{Window, WindowId};

    use crate::{Ui, fail};

    struct Gpu {
        window: Arc<Window>,
        renderer: Renderer<'static>,
    }

    #[derive(Default)]
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

        fn suspended(&mut self, _: &ActiveEventLoop) {
            if cfg!(target_os = "android") {
                self.gpu = None;
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.relayout(),
                WindowEvent::RedrawRequested => {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.window.pre_present_notify();
                        self.ui.render(&mut gpu.renderer);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn run(event_loop: EventLoop<()>) {
        event_loop.run_app(&mut Native::default()).unwrap_or_else(|e| fail(&e));
    }
}

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    let event_loop = winit::event_loop::EventLoop::builder().with_android_app(app).build().unwrap_or_else(|e| fail(&e));
    native::run(event_loop);
}
