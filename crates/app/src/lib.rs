mod library;
mod screen;
mod state;

use sugar_layout::{Id, Layout, Rect};
use sugar_ui::{Fonts, Painter};

use state::State;
pub use state::{Key, Reader};

static FONT: &[u8] = include_bytes!("../../../res/fonts/Inter.ttf");

#[derive(Debug)]
pub struct Ui {
    layout: Layout,
    portrait: Layout,
    placed: Vec<(Id, Rect)>,
    painter: Painter,
    state: State,
    window: Rect,
    scale: f32,
    _output: Option<sugar_io::Output>,
}

impl Ui {
    pub fn new(read: Reader) -> Self {
        let mut engine = None;
        let output = sugar_io::open(|f| {
            let (control, e) = sugar_engine::new(f.rate, f.burst.max(256) * 4);
            engine = Some(control);
            e
        });
        let (output, status) = match output {
            Ok(o) if o.silent => (Some(o), Some("No audio output on this platform yet: playback is silent".into())),
            Ok(o) => (Some(o), None),
            Err(e) => (None, Some(format!("Audio unavailable: {e}"))),
        };
        let mut state = State::new(engine, read, status);
        state.scan();
        Self {
            layout: Layout::new(&screen::tree()),
            portrait: Layout::new(&screen::portrait()),
            placed: Vec::with_capacity(sugar_layout::MAX_MODULES),
            painter: Painter::new(Fonts::new(FONT)),
            state,
            window: Rect::new(0.0, 0.0, 1.0, 1.0),
            scale: 1.0,
            _output: output,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.scale = scale.max(0.5);
        self.window = Rect::new(0.0, 0.0, width as f32 / self.scale, height as f32 / self.scale);
        let layout = if self.window.h > self.window.w * 1.1 { &mut self.portrait } else { &mut self.layout };
        layout.solve(screen::inset(self.window), &mut self.placed);
    }

    pub fn animating(&self) -> bool {
        self.state.animating()
    }

    pub fn render(&mut self, renderer: &mut sugar_ui::Renderer<'_>) {
        self.state.tick();
        self.painter.begin(self.scale);
        screen::paint(&mut self.painter, &mut self.state, self.window, &self.placed);
        renderer.render(&mut self.painter, screen::BACKGROUND);
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

    pub fn key(&mut self, key: Key) {
        self.state.key(key);
    }

    pub fn rescan(&mut self) {
        self.state.scan();
    }

    pub fn wheel(&mut self, x: f32, y: f32, dy: f32) {
        self.state.wheel(x / self.scale, y / self.scale, dy);
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
    use winit::event::{ElementState, MouseScrollDelta, TouchPhase, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::keyboard::{Key as WKey, NamedKey};
    use winit::window::{Window, WindowId};

    use crate::{Key, Reader, Ui, fail};

    const MOUSE: u64 = u64::MAX;

    struct Gpu {
        window: Arc<Window>,
        renderer: Renderer<'static>,
    }

    struct Native {
        ui: Ui,
        gpu: Option<Gpu>,
        cursor: (f32, f32),
        started: bool,
    }

    impl Native {
        fn resize(&mut self) {
            let Some(gpu) = &mut self.gpu else { return };
            let size = gpu.window.inner_size();
            gpu.renderer.resize(size.width, size.height);
            self.ui.resize(size.width, size.height, gpu.window.scale_factor() as f32);
            gpu.window.request_redraw();
        }

        fn redraw(&self) {
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
    }

    impl ApplicationHandler for Native {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.gpu.is_some() {
                return;
            }
            if self.started {
                self.ui.rescan();
            }
            self.started = true;
            let attrs = Window::default_attributes()
                .with_title("DJ Sugar")
                .with_min_inner_size(winit::dpi::LogicalSize::new(320.0, 320.0));
            let window = Arc::new(event_loop.create_window(attrs).unwrap_or_else(|e| fail(&e)));
            let size = window.inner_size();
            let renderer = Renderer::new(window.clone(), size.width, size.height).unwrap_or_else(|e| fail(&e));
            self.gpu = Some(Gpu { window, renderer });
            self.resize();
        }

        fn suspended(&mut self, _: &ActiveEventLoop) {
            if cfg!(target_os = "android") {
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
                WindowEvent::MouseInput { state, .. } => {
                    let (x, y) = self.cursor;
                    match state {
                        ElementState::Pressed => self.ui.pointer_down(MOUSE, x, y),
                        ElementState::Released => self.ui.pointer_up(MOUSE, y),
                    }
                    self.redraw();
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y * 44.0,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    self.ui.wheel(self.cursor.0, self.cursor.1, dy);
                    self.redraw();
                }
                WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                    match &event.logical_key {
                        WKey::Named(NamedKey::Backspace) => self.ui.key(Key::Backspace),
                        WKey::Named(NamedKey::Escape) => self.ui.key(Key::Escape),
                        WKey::Named(NamedKey::Enter) => self.ui.key(Key::Enter),
                        WKey::Named(NamedKey::F1) => self.ui.key(Key::Play(0)),
                        WKey::Named(NamedKey::F2) => self.ui.key(Key::Play(1)),
                        _ => {
                            if let Some(t) = &event.text {
                                self.ui.text(t);
                            }
                        }
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

    pub fn run(event_loop: EventLoop<()>, read: Reader) {
        let mut app = Native { ui: Ui::new(read), gpu: None, cursor: (0.0, 0.0), started: false };
        event_loop.run_app(&mut app).unwrap_or_else(|e| fail(&e));
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::io::Read;
    use std::sync::OnceLock;

    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;

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
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
        let mut env = vm.attach_current_thread()?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
        let sdk = env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?.i()?;
        let name =
            if sdk >= 33 { "android.permission.READ_MEDIA_AUDIO" } else { "android.permission.READ_EXTERNAL_STORAGE" };
        let perm = env.new_string(name)?;
        let state = env
            .call_method(&activity, "checkSelfPermission", "(Ljava/lang/String;)I", &[JValue::Object(&perm)])?
            .i()?;
        if state != 0 {
            let list = env.new_object_array(1, "java/lang/String", &perm)?;
            env.call_method(
                &activity,
                "requestPermissions",
                "([Ljava/lang/String;I)V",
                &[JValue::Object(&list), JValue::Int(1)],
            )?;
        }
        Ok(())
    }

    #[allow(unsafe_code, reason = "the activity glue looks this symbol up by name")]
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        let _ = APP.set(app.clone());
        let _ = request_audio(&app);
        let event_loop =
            winit::event_loop::EventLoop::builder().with_android_app(app).build().unwrap_or_else(|e| crate::fail(&e));
        crate::native::run(event_loop, read);
    }
}

pub fn read_dir_sample(name: &str) -> Option<Vec<u8>> {
    let root = std::env::var_os("DJSUGAR_RES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res"));
    std::fs::read(root.join("samples").join(name)).ok()
}
