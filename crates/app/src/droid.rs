//! The Android shell: the window, the GPU and every input the activity
//! sends, touch, mouse, wheel and keyboard, straight from the activity glue.

use std::sync::Arc;

use android_activity::input::{
    Axis, InputEvent, KeyAction, KeyMapChar, Keycode, MetaState, MotionAction, MotionEvent, ToolType,
};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    AndroidDisplayHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, WindowHandle,
};
use sugar_ui::Renderer;

use crate::{Key, Mods, Reader, Ui, fail};

const MOUSE: u64 = u64::MAX;
/// Lines a wheel notch scrolls, in points.
const NOTCH: f32 = 44.0;

/// The activity's window, as the GPU takes it.
struct Window(NativeWindow);

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        #[allow(unsafe_code, reason = "Android has one display and its handle carries nothing")]
        Ok(unsafe { DisplayHandle::borrow_raw(RawDisplayHandle::Android(AndroidDisplayHandle::new())) })
    }
}

struct Shell {
    app: AndroidApp,
    ui: Ui,
    gpu: Option<(Arc<Window>, Renderer<'static>)>,
    size: (u32, u32),
    cursor: (f32, f32),
    /// A right or middle click is down; its release is not a tap.
    side_click: bool,
    keyboard: bool,
    started: bool,
    drawn: bool,
    dirty: bool,
    /// When the last frame was drawn.
    last: std::time::Instant,
}

impl Shell {
    fn open(&mut self) {
        let Some(native) = self.app.native_window() else { return };
        if self.started {
            self.ui.rescan();
        }
        self.started = true;
        let window = Arc::new(Window(native));
        let size = (window.0.width().max(1) as u32, window.0.height().max(1) as u32);
        crate::stage("window");
        let renderer = Renderer::new(window.clone(), size.0, size.1).unwrap_or_else(|e| fail(&e));
        crate::stage("gpu ready");
        self.gpu = Some((window, renderer));
        self.size = (0, 0);
        self.keyboard = false;
        self.fit();
    }

    /// Matches the drawing to the window's size and density.
    fn fit(&mut self) {
        let Some((window, renderer)) = &mut self.gpu else { return };
        let size = (window.0.width().max(1) as u32, window.0.height().max(1) as u32);
        if size != self.size {
            self.size = size;
            renderer.resize(size.0, size.1);
        }
        let dpi = self.app.config().density().unwrap_or(160) as f32;
        self.ui.resize(size.0, size.1, dpi / 160.0 * crate::UI_SCALE);
        self.dirty = true;
    }

    fn draw(&mut self) {
        let typing = self.ui.typing();
        if typing != self.keyboard {
            self.keyboard = typing;
            if typing {
                self.app.show_soft_input(true);
            } else {
                self.app.hide_soft_input(false);
            }
        }
        let Some((_, renderer)) = &mut self.gpu else { return };
        self.ui.render(renderer);
        self.dirty = false;
        if !self.drawn {
            self.drawn = true;
            crate::stage("first frame");
            crate::android::test_mint();
        }
    }

    fn motion(&mut self, e: &MotionEvent<'_>) -> InputStatus {
        let index = e.pointer_index();
        let p = e.pointer_at_index(index);
        let mouse = matches!(p.tool_type(), ToolType::Mouse);
        let (x, y) = (p.x(), p.y());
        self.dirty = true;
        match e.action() {
            MotionAction::HoverMove | MotionAction::HoverEnter | MotionAction::HoverExit => {
                self.cursor = (x, y);
                self.ui.pointer_move(MOUSE, x, y);
            }
            MotionAction::Scroll => {
                let (v, h) = (p.axis_value(Axis::Vscroll), p.axis_value(Axis::Hscroll));
                self.ui.wheel(x, y, if v == 0.0 { h } else { v } * NOTCH);
            }
            MotionAction::ButtonPress if mouse => {
                let b = e.button_state();
                if b.secondary() && !b.primary() {
                    self.side_click = true;
                    self.ui.context(x, y);
                } else if b.back() {
                    self.side_click = true;
                    self.ui.back();
                } else if b.teriary() {
                    self.side_click = true;
                }
            }
            MotionAction::ButtonRelease if mouse => {}
            MotionAction::Down | MotionAction::PointerDown if mouse => {
                self.cursor = (x, y);
                if !self.side_click {
                    self.ui.pointer_down(MOUSE, x, y);
                }
            }
            MotionAction::Move if mouse => {
                self.cursor = (x, y);
                self.ui.pointer_move(MOUSE, x, y);
            }
            MotionAction::Up | MotionAction::PointerUp | MotionAction::Cancel if mouse => {
                if !std::mem::take(&mut self.side_click) {
                    self.ui.pointer_up(MOUSE, y);
                }
            }
            MotionAction::Down | MotionAction::PointerDown => self.ui.pointer_down(p.pointer_id() as u64, x, y),
            MotionAction::Up | MotionAction::PointerUp => self.ui.pointer_up(p.pointer_id() as u64, y),
            MotionAction::Move => {
                for p in e.pointers() {
                    self.ui.pointer_move(p.pointer_id() as u64, p.x(), p.y());
                }
            }
            MotionAction::Cancel => self.ui.release_all(),
            _ => {
                self.dirty = false;
                return InputStatus::Unhandled;
            }
        }
        if self.ui.quit() {
            crate::android::finish(&self.app);
        }
        InputStatus::Handled
    }

    fn key(&mut self, e: &android_activity::input::KeyEvent<'_>) -> InputStatus {
        let meta = e.meta_state();
        let mods = Mods { shift: meta.shift_on(), ctrl: meta.ctrl_on() || meta.meta_on(), alt: meta.alt_on() };
        let code = e.key_code();
        let key = match code {
            Keycode::Back => {
                if e.action() == KeyAction::Up {
                    return InputStatus::Handled;
                }
                // Back closes what is open; with nothing open it asks whether to leave.
                if !self.ui.back() {
                    self.ui.ask_quit();
                }
                self.dirty = true;
                if self.ui.quit() {
                    crate::android::finish(&self.app);
                }
                return InputStatus::Handled;
            }
            Keycode::Del => Key::Backspace,
            Keycode::ForwardDel => Key::Delete,
            Keycode::Escape => Key::Escape,
            Keycode::Enter | Keycode::NumpadEnter => Key::Enter,
            Keycode::Tab => Key::Tab,
            Keycode::Space => Key::Space,
            Keycode::DpadUp => Key::Up,
            Keycode::DpadDown => Key::Down,
            Keycode::DpadLeft => Key::Left,
            Keycode::DpadRight => Key::Right,
            Keycode::PageUp => Key::PageUp,
            Keycode::PageDown => Key::PageDown,
            Keycode::MoveHome => Key::Home,
            Keycode::MoveEnd => Key::End,
            Keycode::F1 => Key::F(1),
            Keycode::F2 => Key::F(2),
            Keycode::F3 => Key::F(3),
            Keycode::F4 => Key::F(4),
            Keycode::F5 => Key::F(5),
            Keycode::F6 => Key::F(6),
            Keycode::F7 => Key::F(7),
            Keycode::F8 => Key::F(8),
            Keycode::F9 => Key::F(9),
            Keycode::F10 => Key::F(10),
            Keycode::F11 => Key::F(11),
            Keycode::F12 => Key::F(12),
            _ => {
                // The layout's character with only Shift and Caps Lock, so Ctrl+O reads as o.
                let shift = MetaState(meta.0 & 0x0010_00c1);
                let map = self.app.device_key_character_map(e.device_id());
                match map.ok().and_then(|m| m.get(code, shift).ok()) {
                    Some(KeyMapChar::Unicode(c)) => Key::Char(c),
                    _ => return InputStatus::Unhandled,
                }
            }
        };
        self.dirty = true;
        match e.action() {
            KeyAction::Up => self.ui.key_up(key),
            KeyAction::Down => match key {
                Key::Char(c) if self.ui.typing() && !mods.ctrl => self.ui.text(c.encode_utf8(&mut [0; 4])),
                Key::F(_) | Key::Char(_) if e.repeat_count() > 0 => {}
                key => self.ui.key(key, mods),
            },
            _ => {}
        }
        if self.ui.quit() {
            crate::android::finish(&self.app);
        }
        InputStatus::Handled
    }
}

pub fn run(app: AndroidApp, read: Reader) {
    let ui = Ui::new(read);
    crate::stage("painter ready");
    let mut shell = Shell {
        app: app.clone(),
        ui,
        gpu: None,
        size: (0, 0),
        cursor: (0.0, 0.0),
        side_click: false,
        keyboard: false,
        started: false,
        drawn: false,
        dirty: true,
        last: std::time::Instant::now(),
    };
    crate::stage("waiting for window");
    loop {
        // A touch is drawn at once; what only moves by itself is drawn no more often than its frame gap.
        let due = shell.ui.frame_gap().saturating_sub(shell.last.elapsed());
        let wait = match (shell.gpu.is_some(), shell.dirty, shell.ui.animating()) {
            (true, true, _) => Some(std::time::Duration::ZERO),
            (true, false, true) => Some(due),
            _ => None,
        };
        let mut quit = false;
        app.poll_events(wait, |event| {
            let PollEvent::Main(event) = event else { return };
            match event {
                MainEvent::InitWindow { .. } => {
                    crate::stage("resumed");
                    crate::android::immersive(&app);
                    shell.open();
                }
                MainEvent::TerminateWindow { .. } => {
                    crate::stage("suspended");
                    shell.ui.release_all();
                    shell.gpu = None;
                    shell.drawn = false;
                }
                MainEvent::WindowResized { .. }
                | MainEvent::ConfigChanged { .. }
                | MainEvent::ContentRectChanged { .. } => {
                    shell.fit();
                }
                MainEvent::RedrawNeeded { .. } => shell.dirty = true,
                MainEvent::GainedFocus => {
                    crate::android::immersive(&app);
                    shell.ui.rescan_if_empty();
                    shell.dirty = true;
                }
                MainEvent::LostFocus => {
                    shell.ui.release_all();
                    shell.dirty = true;
                }
                MainEvent::Destroy => quit = true,
                _ => {}
            }
        });
        if quit {
            if crate::android::QUITTING.load(std::sync::atomic::Ordering::Relaxed) {
                // Nothing keeps playing after Quit: the audio and the download threads end with the process.
                std::process::exit(0);
            }
            return;
        }
        if let Ok(mut events) = app.input_events_iter() {
            while events.next(|e| match e {
                InputEvent::MotionEvent(m) => shell.motion(m),
                InputEvent::KeyEvent(k) => shell.key(k),
                InputEvent::TextEvent(_) => InputStatus::Handled,
                _ => InputStatus::Unhandled,
            }) {}
        }
        if shell.gpu.is_some()
            && (shell.dirty || (shell.ui.animating() && shell.last.elapsed() >= shell.ui.frame_gap()))
        {
            shell.draw();
            shell.last = std::time::Instant::now();
        }
    }
}
