#[cfg(target_os = "linux")]
mod linux {
    use djsugar::{Ui, fail, read_dir_sample};
    use sugar_shell::{App, Event, Frame};
    use sugar_ui::Renderer;

    const MOUSE: u64 = u64::MAX;

    struct Linux {
        ui: Ui,
        renderer: Renderer<'static>,
        cursor: (f32, f32),
    }

    impl App for Linux {
        fn event(&mut self, event: Event) -> bool {
            match event {
                Event::Resize { width, height, scale } => {
                    self.renderer.resize(width, height);
                    self.ui.resize(width, height, scale);
                }
                Event::Pointer { x, y } => {
                    self.cursor = (x, y);
                    self.ui.pointer_move(MOUSE, x, y);
                }
                Event::Button { button: 4 | 5, pressed: true } => {
                    let dy = if matches!(event, Event::Button { button: 4, .. }) { 44.0 } else { -44.0 };
                    self.ui.wheel(self.cursor.0, self.cursor.1, dy);
                }
                Event::Button { button: 1, pressed } => {
                    let (x, y) = self.cursor;
                    if pressed {
                        self.ui.pointer_down(MOUSE, x, y);
                    } else {
                        self.ui.pointer_up(MOUSE, y);
                    }
                }
                _ => return false,
            }
            true
        }

        fn animating(&self) -> bool {
            self.ui.animating()
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
        let mut app = Linux { ui: Ui::new(read_dir_sample), renderer, cursor: (0.0, 0.0) };
        if let Err(e) = sugar_shell::run("DJ Sugar", width, height, &mut app) {
            fail(&e);
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    linux::main();
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    djsugar::native::run(
        winit::event_loop::EventLoop::new().unwrap_or_else(|e| djsugar::fail(&e)),
        djsugar::read_dir_sample,
    );
}
