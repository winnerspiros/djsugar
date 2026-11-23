//! The helper's window for a plugin's own editor, on the helper's main thread. DJ Sugar asks for the editor through the shared block (`editor`); the helper's main loop calls [`EditorWindow::step`]
//! between the plugin's idle calls. The window (and the platform's event loop under it) is only made the first time an editor is asked for, so helpers whose editor is never opened
//! stay headless. The window floats above DJ Sugar, is titled after the plugin and where it plays, and closing it only hides the editor: the plugin keeps playing.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize, Size};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

use crate::effects::{EditorSize, MainThread, ParentWindow};

/// `editor_shown` values.
pub const CLOSED: u32 = 0;
pub const OPEN: u32 = 1;
pub const NONE: u32 = 2;

/// Editor sizes are in points on macOS and in pixels elsewhere.
const LOGICAL: bool = cfg!(target_os = "macos");

#[derive(Default)]
struct Events {
    /// Make a window at the next chance (only possible inside a callback).
    create: Option<String>,
    window: Option<Window>,
    close_asked: bool,
    resized: Option<PhysicalSize<u32>>,
    scale: Option<f64>,
}

impl ApplicationHandler for Events {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.make_window(el);
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.make_window(el);
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_asked = true,
            WindowEvent::Resized(size) => self.resized = Some(size),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.scale = Some(scale_factor),
            _ => {}
        }
    }
}

impl Events {
    fn make_window(&mut self, el: &ActiveEventLoop) {
        let Some(title) = self.create.take() else {
            return;
        };
        let attrs = WindowAttributes::default()
            .with_title(title)
            .with_visible(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(400.0, 300.0));
        self.window = el.create_window(attrs).ok();
    }
}

/// The editor window of one helper.
pub struct EditorWindow {
    title: String,
    event_loop: Option<EventLoop<()>>,
    events: Events,
    /// The editor is in the window.
    open: bool,
    /// The last ask failed: don't try again until DJ Sugar asks anew.
    refused: bool,
}

impl EditorWindow {
    pub fn new(title: String) -> Self {
        Self { title, event_loop: None, events: Events::default(), open: false, refused: false }
    }

    /// Whether the platform's events need pumping (a window was made once).
    pub fn active(&self) -> bool {
        self.event_loop.is_some()
    }

    /// Follows DJ Sugar's ask in `editor`, reporting in `shown`, and handles
    /// the window's events; waits up to `wait` for them. Without a window
    /// this only sleeps.
    pub fn step(&mut self, main: &mut dyn MainThread, editor: &AtomicU32, shown: &AtomicU32, wait: Duration) {
        self.follow(main, editor, shown, None);
        if self.wants_run() {
            return;
        }
        let Some(el) = self.event_loop.as_mut() else {
            std::thread::sleep(wait);
            return;
        };
        let _ = el.pump_app_events(Some(wait), &mut self.events);
        self.after_events(main, editor, shown);
    }

    /// On macOS, once a window was made the platform's own loop takes over
    /// (see [`EditorWindow::run`]): pumping it in slices stops a plugin's
    /// menus in the middle of their own loop, and they freeze.
    pub fn wants_run(&self) -> bool {
        cfg!(target_os = "macos") && self.event_loop.is_some()
    }

    /// Hands the main thread to the platform's loop for good, calling `tick`
    /// whenever it is idle, and again after the time `tick` returns.
    pub fn run(mut self, tick: impl FnMut(&mut EditorWindow, &ActiveEventLoop) -> Duration) -> ! {
        struct Runner<F> {
            win: EditorWindow,
            tick: F,
        }
        impl<F: FnMut(&mut EditorWindow, &ActiveEventLoop) -> Duration> ApplicationHandler for Runner<F> {
            fn resumed(&mut self, _el: &ActiveEventLoop) {}
            fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
                self.win.events.window_event(el, id, event);
            }
            fn about_to_wait(&mut self, el: &ActiveEventLoop) {
                let wait = (self.tick)(&mut self.win, el);
                el.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + wait));
            }
        }
        let Some(el) = self.event_loop.take() else {
            std::process::exit(0);
        };
        let mut runner = Runner { win: self, tick };
        let _ = el.run_app(&mut runner);
        std::process::exit(0);
    }

    /// [`EditorWindow::step`] inside the platform's loop: follows DJ Sugar's
    /// ask and handles what the window's events left.
    pub fn step_in(&mut self, main: &mut dyn MainThread, editor: &AtomicU32, shown: &AtomicU32, el: &ActiveEventLoop) {
        self.follow(main, editor, shown, Some(el));
        self.after_events(main, editor, shown);
    }

    /// Opens or closes the editor as DJ Sugar asks.
    fn follow(
        &mut self,
        main: &mut dyn MainThread,
        editor: &AtomicU32,
        shown: &AtomicU32,
        el: Option<&ActiveEventLoop>,
    ) {
        let want = editor.load(Ordering::Acquire) == OPEN;
        if !want {
            self.refused = false;
            if self.open {
                self.close(main, shown);
            }
        } else if !self.open && !self.refused {
            // macOS can't pump its loop and then hand it over: make the loop and
            // open the editor inside it, on the next turn (see `wants_run`).
            if cfg!(target_os = "macos") && el.is_none() {
                if self.event_loop.is_none() {
                    match new_event_loop() {
                        Ok(l) => self.event_loop = Some(l),
                        Err(e) => {
                            eprintln!("editor: {e}");
                            self.refused = true;
                            editor.store(CLOSED, Ordering::Release);
                            shown.store(NONE, Ordering::Release);
                        }
                    }
                }
                return;
            }
            match self.open_editor(main, el) {
                Ok(()) => shown.store(OPEN, Ordering::Release),
                Err(e) => {
                    eprintln!("editor: {e}");
                    self.refused = true;
                    editor.store(CLOSED, Ordering::Release);
                    shown.store(NONE, Ordering::Release);
                }
            }
        }
    }

    /// The window was closed, or resized by the DJ or the plugin.
    fn after_events(&mut self, main: &mut dyn MainThread, editor: &AtomicU32, shown: &AtomicU32) {
        if !self.open {
            return;
        }
        if std::mem::take(&mut self.events.close_asked) || main.take_editor_closed() {
            self.close(main, shown);
            editor.store(CLOSED, Ordering::Release);
            return;
        }
        let Some(window) = self.events.window.as_ref() else {
            return;
        };
        let scale = self.events.scale.take().unwrap_or_else(|| window.scale_factor());
        let mut moved = false;
        if let Some(size) = main.take_editor_resize() {
            let _ = window.request_inner_size(window_size(size));
            moved = true;
        } else if let Some(size) = self.events.resized.take() {
            let asked = editor_size(size, scale);
            let took = main.editor_resized(asked);
            if took != asked {
                let _ = window.request_inner_size(window_size(took));
            }
            moved = true;
        }
        if moved {
            if let Ok(parent) = parent_of(window) {
                pin_top_left(parent);
            }
        }
    }

    fn open_editor(&mut self, main: &mut dyn MainThread, active: Option<&ActiveEventLoop>) -> Result<(), String> {
        if self.events.window.is_none() {
            self.events.create = Some(self.title.clone());
            match active {
                Some(el) => self.events.make_window(el),
                None => {
                    if self.event_loop.is_none() {
                        self.event_loop = Some(new_event_loop()?);
                    }
                    if let Some(el) = self.event_loop.as_mut() {
                        let _ = el.pump_app_events(Some(Duration::ZERO), &mut self.events);
                    }
                }
            }
        }
        let window = self.events.window.as_ref().ok_or("the window could not be made")?;
        let parent = parent_of(window)?;
        let size = main.open_editor(parent, window.scale_factor())?;
        window.set_resizable(main.editor_can_resize());
        let _ = window.request_inner_size(window_size(size));
        main.editor_placed();
        pin_top_left(parent);
        window.set_visible(true);
        window.focus_window();
        self.events.resized = None;
        self.events.close_asked = false;
        self.open = true;
        keep_awake(true);
        Ok(())
    }

    fn close(&mut self, main: &mut dyn MainThread, shown: &AtomicU32) {
        main.close_editor();
        // A new window next time: some plugins can't be put back into a
        // window they were taken out of.
        self.events.window = None;
        if let Some(el) = self.event_loop.as_mut() {
            // In the platform's own loop (macOS), it goes at the next turn.
            let _ = el.pump_app_events(Some(Duration::ZERO), &mut self.events);
        }
        self.open = false;
        keep_awake(false);
        shown.store(CLOSED, Ordering::Release);
    }
}

fn window_size(size: EditorSize) -> Size {
    if LOGICAL {
        LogicalSize::new(size.width as f64, size.height as f64).into()
    } else {
        PhysicalSize::new(size.width, size.height).into()
    }
}

fn editor_size(size: PhysicalSize<u32>, scale: f64) -> EditorSize {
    if LOGICAL {
        let l = size.to_logical::<f64>(scale);
        EditorSize { width: l.width.round() as u32, height: l.height.round() as u32 }
    } else {
        EditorSize { width: size.width, height: size.height }
    }
}

fn new_event_loop() -> Result<EventLoop<()>, String> {
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        // No Dock icon or menu bar of its own: the window belongs to DJ Sugar.
        builder.with_activation_policy(ActivationPolicy::Accessory).with_default_menu(false);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Plugins draw with X11 (under XWayland on a Wayland desktop).
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11();
    }
    builder.build().map_err(|e| e.to_string())
}

// An X11 window id is a C `unsigned long`: 64 bits here, 32 on Windows.
#[allow(clippy::useless_conversion)]
fn parent_of(window: &Window) -> Result<ParentWindow, String> {
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::AppKit(h) => Ok(ParentWindow::Cocoa(h.ns_view.as_ptr())),
        RawWindowHandle::Win32(h) => Ok(ParentWindow::Win32(h.hwnd.get() as *mut _)),
        RawWindowHandle::Xlib(h) => Ok(ParentWindow::X11(u64::from(h.window))),
        RawWindowHandle::Xcb(h) => Ok(ParentWindow::X11(u64::from(h.window.get()))),
        _ => Err("plugin windows need X11, Windows or macOS".into()),
    }
}

/// Keeps the editor at the window's top left on macOS. AppKit counts from the bottom,
/// so an editor attached while the window was smaller (or that placed itself by the
/// window's old height) shows only its bottom part, at the top, until it is moved.
#[cfg(target_os = "macos")]
fn pin_top_left(parent: ParentWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSPoint, NSRect};
    let ParentWindow::Cocoa(view) = parent else {
        return;
    };
    let view = view as *mut AnyObject;
    // SAFETY: the window's live content view and its subviews, on the main
    // thread.
    unsafe {
        let bounds: NSRect = msg_send![view, bounds];
        let flipped: bool = msg_send![view, isFlipped];
        let subviews: *mut AnyObject = msg_send![view, subviews];
        if subviews.is_null() {
            return;
        }
        let n: usize = msg_send![subviews, count];
        for i in 0..n {
            let sub: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
            let frame: NSRect = msg_send![sub, frame];
            let y = if flipped { 0.0 } else { bounds.size.height - frame.size.height };
            if frame.origin.x != 0.0 || frame.origin.y != y {
                let _: () = msg_send![sub, setFrameOrigin: NSPoint::new(0.0, y)];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn pin_top_left(_parent: ParentWindow) {}

/// While an editor is open, macOS would slow a helper whose window is
/// hidden behind others (App Nap), and the plugin's audio with it.
#[cfg(target_os = "macos")]
fn keep_awake(on: bool) {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use std::cell::RefCell;
    thread_local! {
        static ACTIVITY: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
    }
    ACTIVITY.with(|a| {
        let mut a = a.borrow_mut();
        // SAFETY: NSProcessInfo's documented methods, on the main thread.
        unsafe {
            let info: *mut AnyObject = msg_send![class!(NSProcessInfo), processInfo];
            if let Some(old) = a.take() {
                let _: () = msg_send![info, endActivity: &*old];
            }
            if on {
                // NSActivityUserInitiatedAllowingIdleSystemSleep |
                // NSActivityLatencyCritical
                let options: u64 = 0x00EF_FFFF | 0xFF_0000_0000;
                let reason = objc2_foundation::NSString::from_str("Plugin playing in DJ Sugar");
                let activity: Option<Retained<AnyObject>> =
                    msg_send![info, beginActivityWithOptions: options, reason: &*reason];
                *a = activity;
            }
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn keep_awake(_on: bool) {}
