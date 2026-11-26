#[cfg(target_os = "linux")]
mod linux {
    use djsugar::{Key, Mods, Ui, fail, read_dir_sample};
    use sugar_shell::{App, Event, Frame, keysym};
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
                Event::Key { sym, pressed, mods } => {
                    let key = match sym {
                        keysym::BACKSPACE => Key::Backspace,
                        keysym::TAB => Key::Tab,
                        keysym::RETURN => Key::Enter,
                        keysym::ESCAPE => Key::Escape,
                        keysym::HOME => Key::Home,
                        keysym::LEFT => Key::Left,
                        keysym::UP => Key::Up,
                        keysym::RIGHT => Key::Right,
                        keysym::DOWN => Key::Down,
                        keysym::PAGE_UP => Key::PageUp,
                        keysym::PAGE_DOWN => Key::PageDown,
                        keysym::END => Key::End,
                        keysym::DELETE => Key::Delete,
                        f if (keysym::F1..keysym::F1 + 12).contains(&f) => Key::F((f - keysym::F1) as u8 + 1),
                        _ => match keysym::char(sym) {
                            Some(' ') => Key::Space,
                            Some(c) => Key::Char(c),
                            None => return false,
                        },
                    };
                    let mods = Mods { shift: mods & 1 != 0, ctrl: mods & 4 != 0, alt: mods & 8 != 0 };
                    match key {
                        _ if !pressed => self.ui.key_up(key),
                        Key::Char(c) if self.ui.typing() && !mods.ctrl => self.ui.text(c.encode_utf8(&mut [0; 4])),
                        key => self.ui.key(key, mods),
                    }
                }
                Event::Button { button: 3, pressed: true } => self.ui.context(self.cursor.0, self.cursor.1),
                Event::Button { button: 8, pressed: true } => {
                    self.ui.back();
                }
                _ => return false,
            }
            true
        }

        fn animating(&self) -> bool {
            self.ui.animating()
        }

        fn closing(&self) -> bool {
            self.ui.quit()
        }

        fn wanted_size(&mut self) -> Option<(u32, u32)> {
            self.ui.wanted_size()
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
