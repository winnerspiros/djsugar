mod bridge;
mod classic;
mod controllers;
mod hub;
mod library;
mod lyrics;
mod models;
mod open;
mod prefs;
mod record;
mod settings;
mod skips;
mod state;
mod store;
/// Where saved state lives, set before the app starts.
pub use store::set_dir as set_data_dir;
mod automix;
mod sugar;
mod sugardeck;
mod theme;

use sugar_ui::{Fonts, Painter};

use state::State;
pub use state::{Key, Mods, Reader};

static FONT: &[u8] = include_bytes!("../../../res/fonts/Inter.ttf");

#[derive(Debug)]
pub struct Ui {
    painter: Painter,
    state: State,
    size: (f32, f32),
    scale: f32,
    /// The window in pixels and the device's own scale, before the DJ's interface scale.
    pixels: (u32, u32),
    device: f32,
}

impl Ui {
    pub fn new(read: Reader) -> Self {
        stage("ui");
        let engine = crate::bridge::Bridge::start(read);
        let out_rate = engine.rate();
        let status = None;
        let engine = Some(engine);
        stage("audio open");
        crate::models::install();
        let mut state = State::new(engine, read, status, out_rate);
        state.scan();
        // The YouTube player script takes seconds to load; have it ready before the first download.
        std::thread::spawn(|| sugar_net::cipher::load().ok());
        stage("state ready");
        Self {
            painter: Painter::new(Fonts::new(FONT)),
            state,
            size: (1.0, 1.0),
            scale: 1.0,
            pixels: (1, 1),
            device: 1.0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.pixels = (width, height);
        self.device = scale;
        self.fit();
    }

    /// The scale drawn at: the device's times the DJ's interface scale.
    fn fit(&mut self) {
        self.scale = (self.device * self.state.settings.ui_scale()).max(0.25);
        self.size = (self.pixels.0 as f32 / self.scale, self.pixels.1 as f32 / self.scale);
    }

    pub fn animating(&self) -> bool {
        self.state.animating()
    }

    pub fn render(&mut self, renderer: &mut sugar_ui::Renderer<'_>) {
        self.state.tick();
        self.fit();
        self.painter.begin(self.scale);
        sugar::paint(&mut self.painter, &mut self.state, self.size.0, self.size.1);
        renderer.render(&mut self.painter, 0);
    }

    pub fn pointer_down(&mut self, id: u64, x: f32, y: f32) {
        self.state.pointer_down(id, x / self.scale, y / self.scale);
    }

    pub fn pointer_move(&mut self, id: u64, x: f32, y: f32) {
        self.state.pointer_move(id, x / self.scale, y / self.scale);
    }

    pub fn pointer_up(&mut self, id: u64, y: f32) {
        self.state.pointer_up(id, y / self.scale);
    }

    pub fn text(&mut self, text: &str) {
        if text.chars().all(|c| !c.is_control()) {
            self.state.type_text(text);
        }
    }

    pub fn key(&mut self, key: Key, mods: Mods) {
        self.state.key(key, mods);
    }

    pub fn key_up(&mut self, key: Key) {
        self.state.key_up(key);
    }

    /// A right click at a point, in pixels.
    pub fn context(&mut self, x: f32, y: f32) {
        self.state.context(x / self.scale, y / self.scale);
    }

    /// Closes the topmost menu or sheet, as Back does; false when none was open.
    pub fn back(&mut self) -> bool {
        self.state.dismiss()
    }

    /// Ctrl+Q was pressed.
    pub fn quit(&self) -> bool {
        self.state.quit
    }

    pub fn typing(&self) -> bool {
        self.state.typing()
    }

    /// Every control on screen as its target name and x, y, w, h, for scripted tests.
    pub fn targets(&self) -> Vec<(String, [f32; 4])> {
        self.state.hits.iter().map(|h| (format!("{:?}", h.target), [h.rect.x, h.rect.y, h.rect.w, h.rect.h])).collect()
    }

    /// Taps the topmost control named `name`; false when none is on screen.
    pub fn tap(&mut self, name: &str) -> bool {
        let Some(h) = self.state.hits.iter().rev().find(|h| format!("{:?}", h.target) == name) else { return false };
        let (x, y) = (h.rect.x + h.rect.w * 0.5, h.rect.y + h.rect.h * 0.5);
        self.state.pointer_down(0, x, y);
        self.state.pointer_up(0, y);
        true
    }

    /// The streaming search text.
    /// The message on screen, if one is.
    pub fn toast(&self) -> Option<String> {
        self.state.toast.as_ref().filter(|t| t.1.elapsed() < state::TOAST).map(|t| t.0.clone())
    }

    pub fn query(&self) -> &str {
        &self.state.hub.query
    }

    pub fn release_all(&mut self) {
        self.state.release_all();
    }

    pub fn rescan(&mut self) {
        self.state.scan();
    }

    /// Looks again when the last look found nothing: the DJ may just have let the app read the music.
    pub fn rescan_if_empty(&mut self) {
        if self.state.found == 0 {
            self.state.scan();
        }
    }

    pub fn wheel(&mut self, x: f32, y: f32, dy: f32) {
        self.state.wheel(x / self.scale, y / self.scale, dy);
    }
}

/// Startup and failure notes: Android's log on phones, stderr elsewhere.
pub fn diag(msg: &str) {
    #[cfg(target_os = "android")]
    {
        #[allow(unsafe_code, reason = "liblog is the platform log sink")]
        #[link(name = "log")]
        unsafe extern "C" {
            fn __android_log_write(prio: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
        }
        if let Ok(text) = std::ffi::CString::new(msg) {
            #[allow(unsafe_code, reason = "both strings are NUL terminated and live for the call")]
            unsafe {
                __android_log_write(4, c"djsugar".as_ptr(), text.as_ptr());
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("djsugar: {msg}");
}

/// Logs how long a startup stage took since the process began.
pub fn stage(name: &str) {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let t = START.get_or_init(std::time::Instant::now).elapsed();
    diag(&format!("{name} at {} ms", t.as_millis()));
}

pub fn fail(e: &dyn std::fmt::Display) -> ! {
    diag(&e.to_string());
    std::process::exit(1);
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub mod native {
    use std::sync::Arc;

    use sugar_ui::Renderer;
    use winit::application::ApplicationHandler;
    use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::keyboard::{Key as WKey, NamedKey};
    use winit::window::{Window, WindowId};

    use crate::{Key, Mods, Reader, Ui, fail};

    const MOUSE: u64 = u64::MAX;

    struct Gpu {
        window: Arc<Window>,
        renderer: Renderer<'static>,
    }

    struct Native {
        ui: Ui,
        gpu: Option<Gpu>,
        cursor: (f32, f32),
        mods: Mods,
        started: bool,
        ime: bool,
        drawn: bool,
    }

    impl Native {
        fn resize(&mut self) {
            let Some(gpu) = &mut self.gpu else { return };
            let size = gpu.window.inner_size();
            gpu.renderer.resize(size.width, size.height);
            self.ui.resize(size.width, size.height, gpu.window.scale_factor() as f32 * crate::UI_SCALE);
            gpu.window.request_redraw();
        }

        fn redraw(&mut self) {
            let Some(gpu) = &self.gpu else { return };
            let typing = self.ui.typing();
            if typing != self.ime {
                self.ime = typing;
                gpu.window.set_ime_allowed(typing);
            }
            gpu.window.request_redraw();
        }
    }

    impl ApplicationHandler for Native {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            crate::stage("resumed");
            if self.gpu.is_some() {
                return;
            }
            if self.started {
                self.ui.rescan();
            }
            self.started = true;
            let attrs = Window::default_attributes()
                .with_title("DJ Sugar")
                .with_min_inner_size(winit::dpi::LogicalSize::new(320.0, 320.0))
                .with_fullscreen(
                    std::env::args()
                        .any(|a| a == "-f" || a == "--full-screen")
                        .then_some(winit::window::Fullscreen::Borderless(None)),
                );
            let window = Arc::new(event_loop.create_window(attrs).unwrap_or_else(|e| fail(&e)));
            let size = window.inner_size();
            crate::stage("window");
            let renderer = Renderer::new(window.clone(), size.width, size.height).unwrap_or_else(|e| fail(&e));
            crate::stage("gpu ready");
            self.gpu = Some(Gpu { window, renderer });
            self.ime = false;
            self.resize();
            self.redraw();
        }

        fn suspended(&mut self, _: &ActiveEventLoop) {
            crate::stage("suspended");
            self.ui.release_all();
            if cfg!(target_os = "android") {
                self.drawn = false;
                self.gpu = None;
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.resize(),
                WindowEvent::RedrawRequested => {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.window.pre_present_notify();
                        self.ui.render(&mut gpu.renderer);
                        if !self.drawn {
                            self.drawn = true;
                            crate::stage("first frame");
                            #[cfg(target_os = "android")]
                            crate::android::test_mint();
                        }
                        if self.ui.animating() {
                            gpu.window.request_redraw();
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.cursor = (position.x as f32, position.y as f32);
                    self.ui.pointer_move(MOUSE, self.cursor.0, self.cursor.1);
                    self.redraw();
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let (x, y) = self.cursor;
                    match (button, state) {
                        (MouseButton::Left, ElementState::Pressed) => self.ui.pointer_down(MOUSE, x, y),
                        (MouseButton::Left, ElementState::Released) => self.ui.pointer_up(MOUSE, y),
                        (MouseButton::Right, ElementState::Pressed) => self.ui.context(x, y),
                        (MouseButton::Back, ElementState::Pressed) => {
                            self.ui.back();
                        }
                        _ => {}
                    }
                    if self.ui.quit() {
                        event_loop.exit();
                    }
                    self.redraw();
                }
                WindowEvent::Focused(false) => {
                    self.ui.release_all();
                    self.redraw();
                }
                WindowEvent::Ime(Ime::Commit(t)) => {
                    self.ui.text(&t);
                    self.redraw();
                }
                WindowEvent::ModifiersChanged(m) => {
                    let m = m.state();
                    self.mods = Mods { shift: m.shift_key(), ctrl: m.control_key() || m.super_key(), alt: m.alt_key() };
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (if y == 0.0 { x } else { y }) * 44.0,
                        MouseScrollDelta::PixelDelta(p) => (if p.y == 0.0 { p.x } else { p.y }) as f32,
                    };
                    self.ui.wheel(self.cursor.0, self.cursor.1, dy);
                    self.redraw();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    let Some(key) = key_of(&event.logical_key) else { return };
                    if event.state == ElementState::Released {
                        self.ui.key_up(key);
                        return;
                    }
                    match key {
                        Key::Char(_) if self.ui.typing() && !self.mods.ctrl => {
                            if let Some(t) = &event.text {
                                self.ui.text(t);
                            }
                        }
                        // Held keys repeat only where repeating means something.
                        Key::F(_) | Key::Char(_) if event.repeat => {}
                        key => self.ui.key(key, self.mods),
                    }
                    if self.ui.quit() {
                        event_loop.exit();
                    }
                    self.redraw();
                }
                WindowEvent::Touch(t) => {
                    let (x, y) = (t.location.x as f32, t.location.y as f32);
                    match t.phase {
                        TouchPhase::Started => self.ui.pointer_down(t.id, x, y),
                        TouchPhase::Moved => self.ui.pointer_move(t.id, x, y),
                        TouchPhase::Ended | TouchPhase::Cancelled => self.ui.pointer_up(t.id, y),
                    }
                    self.redraw();
                }
                _ => {}
            }
        }
    }

    /// A key as the app names it; None for modifiers and keys it has no use for.
    fn key_of(k: &WKey) -> Option<Key> {
        Some(match k {
            WKey::Named(n) => match n {
                NamedKey::Backspace => Key::Backspace,
                NamedKey::Delete => Key::Delete,
                NamedKey::Escape | NamedKey::BrowserBack | NamedKey::GoBack => Key::Escape,
                NamedKey::Enter => Key::Enter,
                NamedKey::Tab => Key::Tab,
                NamedKey::Space => Key::Space,
                NamedKey::ArrowUp => Key::Up,
                NamedKey::ArrowDown => Key::Down,
                NamedKey::ArrowLeft => Key::Left,
                NamedKey::ArrowRight => Key::Right,
                NamedKey::PageUp => Key::PageUp,
                NamedKey::PageDown => Key::PageDown,
                NamedKey::Home => Key::Home,
                NamedKey::End => Key::End,
                NamedKey::F1 => Key::F(1),
                NamedKey::F2 => Key::F(2),
                NamedKey::F3 => Key::F(3),
                NamedKey::F4 => Key::F(4),
                NamedKey::F5 => Key::F(5),
                NamedKey::F6 => Key::F(6),
                NamedKey::F7 => Key::F(7),
                NamedKey::F8 => Key::F(8),
                NamedKey::F9 => Key::F(9),
                NamedKey::F10 => Key::F(10),
                NamedKey::F11 => Key::F(11),
                NamedKey::F12 => Key::F(12),
                _ => return None,
            },
            WKey::Character(s) => Key::Char(s.chars().next()?),
            _ => return None,
        })
    }

    pub fn run(event_loop: EventLoop<()>, read: Reader) {
        let mut app = Native {
            ui: Ui::new(read),
            gpu: None,
            cursor: (0.0, 0.0),
            mods: Mods::default(),
            started: false,
            ime: false,
            drawn: false,
        };
        event_loop.run_app(&mut app).unwrap_or_else(|e| fail(&e));
    }
}

#[cfg(target_os = "android")]
mod potoken;

#[cfg(target_os = "android")]
mod droid;

#[cfg(target_os = "android")]
pub(crate) mod android {
    use std::io::Read;
    use std::sync::OnceLock;

    use android_activity::AndroidApp;

    static APP: OnceLock<AndroidApp> = OnceLock::new();

    fn read(name: &str) -> Option<Vec<u8>> {
        let path = std::ffi::CString::new(format!("samples/{name}")).ok()?;
        let mut asset = APP.get()?.asset_manager().open(&path)?;
        let mut bytes = Vec::with_capacity(asset.length());
        asset.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    #[allow(unsafe_code, reason = "JNI hands us raw VM and activity pointers")]
    fn request_audio(app: &AndroidApp) -> jni::errors::Result<()> {
        use jni::objects::{JObject, JValue};
        use jni::{jni_sig, jni_str};
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
        vm.attach_current_thread(|env| {
            let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
            let sdk =
                env.get_static_field(jni_str!("android/os/Build$VERSION"), jni_str!("SDK_INT"), jni_sig!("I"))?.i()?;
            let name = if sdk >= 33 {
                "android.permission.READ_MEDIA_AUDIO"
            } else {
                "android.permission.READ_EXTERNAL_STORAGE"
            };
            let perm = env.new_string(name)?;
            let state = env
                .call_method(
                    &activity,
                    jni_str!("checkSelfPermission"),
                    jni_sig!("(Ljava/lang/String;)I"),
                    &[JValue::Object(&perm)],
                )?
                .i()?;
            if state != 0 {
                let list = env.new_object_array(1, jni_str!("java/lang/String"), &perm)?;
                env.call_method(
                    &activity,
                    jni_str!("requestPermissions"),
                    jni_sig!("([Ljava/lang/String;I)V"),
                    &[JValue::Object(&list), JValue::Int(1)],
                )?;
            }
            Ok(())
        })
    }

    /// Hides the status and navigation bars; a swipe from the edge shows them for a moment.
    #[allow(unsafe_code, reason = "JNI hands us raw VM and activity pointers")]
    pub(crate) fn immersive(app: &AndroidApp) {
        use jni::objects::{JObject, JValue};
        use jni::{jni_sig, jni_str};
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
        let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
            let sdk =
                env.get_static_field(jni_str!("android/os/Build$VERSION"), jni_str!("SDK_INT"), jni_sig!("I"))?.i()?;
            let window =
                env.call_method(&activity, jni_str!("getWindow"), jni_sig!("()Landroid/view/Window;"), &[])?.l()?;
            let out = if sdk >= 30 {
                (|| {
                    let c = env
                        .call_method(
                            &window,
                            jni_str!("getInsetsController"),
                            jni_sig!("()Landroid/view/WindowInsetsController;"),
                            &[],
                        )?
                        .l()?;
                    let bars = env
                        .call_static_method(
                            jni_str!("android/view/WindowInsets$Type"),
                            jni_str!("systemBars"),
                            jni_sig!("()I"),
                            &[],
                        )?
                        .i()?;
                    // Bars shown by a swipe hide again by themselves.
                    env.call_method(&c, jni_str!("setSystemBarsBehavior"), jni_sig!("(I)V"), &[JValue::Int(2)])?;
                    env.call_method(&c, jni_str!("hide"), jni_sig!("(I)V"), &[JValue::Int(bars)])?;
                    Ok(())
                })()
            } else {
                (|| {
                    let decor = env
                        .call_method(&window, jni_str!("getDecorView"), jni_sig!("()Landroid/view/View;"), &[])?
                        .l()?;
                    // Immersive sticky, hide navigation and status, laid out under both.
                    env.call_method(
                        &decor,
                        jni_str!("setSystemUiVisibility"),
                        jni_sig!("(I)V"),
                        &[JValue::Int(0x1706)],
                    )?;
                    Ok(())
                })()
            };
            if out.is_err() {
                env.exception_clear();
            }
            out
        });
    }

    /// The DJ asked to quit, so the window's end is the app's end.
    pub(crate) static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    /// Closes the activity and its task, as Quit and Ctrl+Q ask; the process ends once it is gone.
    #[allow(unsafe_code, reason = "JNI hands us raw VM and activity pointers")]
    pub(crate) fn finish(app: &AndroidApp) {
        QUITTING.store(true, std::sync::atomic::Ordering::Relaxed);
        use jni::objects::JObject;
        use jni::{jni_sig, jni_str};
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
        let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
            env.call_method(&activity, jni_str!("finishAndRemoveTask"), jni_sig!("()V"), &[])?;
            Ok(())
        });
    }

    /// CI sets this property to a video ID to prove the token mint works on a
    /// real device; it starts after the first frame so its web view can't hold
    /// up the window.
    pub(crate) fn test_mint() {
        if let Some(video) = property(c"debug.djsugar.mint") {
            std::thread::spawn(move || crate::potoken::mint(&video, ""));
        }
    }

    fn property(name: &std::ffi::CStr) -> Option<String> {
        #[allow(unsafe_code, reason = "a libc call present on every Android version")]
        unsafe extern "C" {
            fn __system_property_get(name: *const std::ffi::c_char, value: *mut std::ffi::c_char) -> i32;
        }
        let mut buf = [0u8; 92];
        #[allow(unsafe_code, reason = "the buffer is the documented 92 byte maximum")]
        let n = unsafe { __system_property_get(name.as_ptr(), buf.as_mut_ptr().cast()) };
        let len = usize::try_from(n).ok().filter(|&n| n > 0)?;
        Some(String::from_utf8_lossy(&buf[..len]).into_owned())
    }

    pub fn app() -> Option<&'static AndroidApp> {
        APP.get()
    }

    #[allow(unsafe_code, reason = "the activity glue looks this symbol up by name")]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        crate::stage("start");
        std::panic::set_hook(Box::new(|info| crate::diag(&format!("panicked: {info}"))));
        let _ = APP.set(app.clone());
        if let Some(dir) = app.internal_data_path() {
            crate::store::set_dir(dir);
        }
        let _ = request_audio(&app);
        sugar_net::youtube::set_po_token_source(crate::potoken::mint);
        crate::stage("permissions");
        crate::droid::run(app, read);
    }
}

/// DJ Sugar draws its phone UI at 0.45 of Android's density; the same factor here
/// gives the same density of controls. Desktops draw at the system scale.
pub const UI_SCALE: f32 = if cfg!(target_os = "android") { 0.45 } else { 1.0 };

pub fn read_dir_sample(name: &str) -> Option<Vec<u8>> {
    let root = std::env::var_os("DJSUGAR_RES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res"));
    std::fs::read(root.join("samples").join(name)).ok()
}
