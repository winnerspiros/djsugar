#[cfg(target_os = "linux")]
mod linux {
    use djsugar::{Ui, fail};
    use sugar_shell::{App, Event, Frame};
    use sugar_ui::Renderer;

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
            self.ui.render(&mut self.renderer);
            self.renderer
                .read(|p| present(Frame { bytes: p.bytes, stride: p.stride, width: p.width, height: p.height }));
        }
    }

    pub fn main() {
        let (width, height) = (1280, 800);
        let renderer = Renderer::offscreen(width, height).unwrap_or_else(|e| fail(&e));
        let mut app = Linux { ui: Ui::default(), renderer };
        if let Err(e) = sugar_shell::run("DJ Sugar", width, height, &mut app) {
            fail(&e);
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    linux::main();
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    djsugar::native::run(winit::event_loop::EventLoop::new().unwrap_or_else(|e| djsugar::fail(&e)));
}
