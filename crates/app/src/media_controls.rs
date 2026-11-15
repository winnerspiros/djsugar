//! The system's media keys and Now Playing: macOS Control Center and the
//! lock screen, the Windows media overlay, and MPRIS on Linux, through
//! souvlaki. They show the track the Player bar shows, and their play,
//! pause, next, previous and position act as the bar's buttons do (see
//! [`crate::transport`]).
//!
//! Only Player mode takes them, so a headphone button can't stop a deck in
//! the middle of a set; Settings › Player › Media keys in every layout
//! lets the DJ layouts have them too. Outside those, DJ Sugar leaves the
//! system's Now Playing.
//!
//! A timer on the UI thread keeps the system in step, calling it only when
//! something changed. The system's events arrive on its own thread and are
//! handed to the UI thread.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use djsugar_engine::LoadedTracks;
use slint::{ComponentHandle, Model, Timer, TimerMode};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};

use crate::{settings, transport, AppWindow};

/// The setting: media keys in the DJ layouts too. Off at first.
pub const EVERYWHERE: &str = "media_keys_everywhere";

/// The Player layout's index.
const PLAYER: i32 = 1;

/// How far the seek keys move when the system doesn't say.
const SEEK_STEP: Duration = Duration::from_secs(10);

/// Longest side of the cover handed to the system.
const COVER_SIDE: u32 = 512;

/// Starts keeping the system's controls in step with the Player bar. Keep
/// the timer for as long as the app runs.
pub fn install(app: &AppWindow, loaded: LoadedTracks) -> Timer {
    let mut state = State::default();
    let weak = app.as_weak();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        if let Some(app) = weak.upgrade() {
            state.tick(&app, &loaded);
        }
    });
    timer
}

/// What the system was last told about the track.
#[derive(Clone, PartialEq, Default)]
struct Shown {
    title: String,
    artist: String,
    album: String,
    /// Milliseconds.
    length: u64,
    cover: Option<String>,
}

#[derive(Default)]
struct State {
    controls: Option<MediaControls>,
    /// The system has no media controls (or no session bus): don't try
    /// again.
    unavailable: bool,
    attached: bool,
    ticks: u32,
    everywhere: bool,
    shown: Option<Shown>,
    /// Playing, and the position (seconds) last told, with when.
    playback: Option<(bool, f64, Instant)>,
    /// The track whose cover is written, and where.
    cover_for: String,
    cover_file: Option<PathBuf>,
}

impl State {
    fn tick(&mut self, app: &AppWindow, loaded: &LoadedTracks) {
        if self.unavailable {
            return;
        }
        if self.controls.is_none() {
            match create(app) {
                Some(Ok(c)) => self.controls = Some(c),
                Some(Err(e)) => {
                    eprintln!("media keys unavailable: {e}");
                    self.unavailable = true;
                    return;
                }
                // The window isn't up yet: try on the next tick.
                None => return,
            }
        }
        // The setting, every two seconds.
        if self.ticks.is_multiple_of(8) {
            self.everywhere = settings::flag(EVERYWHERE, false);
        }
        self.ticks = self.ticks.wrapping_add(1);
        let active = app.get_layout() == PLAYER || self.everywhere;
        let Some(controls) = self.controls.as_mut() else {
            return;
        };
        if active && !self.attached {
            let weak = app.as_weak();
            let attached = controls.attach(move |event| {
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = weak.upgrade() {
                        act(&app, event);
                    }
                });
            });
            if let Err(e) = attached {
                eprintln!("media keys unavailable: {e:?}");
                self.unavailable = true;
                return;
            }
            self.attached = true;
            self.shown = None;
            self.playback = None;
        } else if !active && self.attached {
            let _ = controls.set_playback(MediaPlayback::Stopped);
            let _ = controls.set_metadata(MediaMetadata::default());
            let _ = controls.detach();
            self.attached = false;
            return;
        }
        if !self.attached {
            return;
        }
        self.update(app, loaded);
    }

    /// Tells the system what changed since the last tick.
    fn update(&mut self, app: &AppWindow, loaded: &LoadedTracks) {
        let (d, _) = transport::deck(app);
        let info = app.get_decks().row_data(d).filter(|i| i.loaded);
        let (shown, playing, position) = match &info {
            Some(info) => {
                let album = loaded
                    .read()
                    .ok()
                    .and_then(|t| t.get(d).cloned().flatten())
                    .map(|t| t.album.clone())
                    .unwrap_or_default();
                let mut shown = Shown {
                    title: info.title.to_string(),
                    artist: info.artist.to_string(),
                    album,
                    length: (info.duration.max(0.0) as f64 * 1000.0) as u64,
                    cover: None,
                };
                if info.has_cover {
                    shown.cover = self.cover_url(&shown, &info.cover);
                }
                let position = (info.progress.clamp(0.0, 1.0) * info.duration.max(0.0)) as f64;
                (shown, transport::playing(app), position)
            }
            None => (Shown::default(), false, 0.0),
        };
        let Some(controls) = self.controls.as_mut() else {
            return;
        };
        let new_track = self.shown.as_ref() != Some(&shown);
        if new_track {
            let metadata = MediaMetadata {
                title: Some(shown.title.as_str()).filter(|s| !s.is_empty()),
                artist: Some(shown.artist.as_str()).filter(|s| !s.is_empty()),
                album: Some(shown.album.as_str()).filter(|s| !s.is_empty()),
                cover_url: shown.cover.as_deref(),
                duration: (shown.length > 0).then(|| Duration::from_millis(shown.length)),
            };
            let _ = controls.set_metadata(metadata);
            self.shown = Some(shown);
        }
        // The position is told when playing starts or stops, and when it
        // jumps (a seek, a new track): the system runs the clock between.
        let now = Instant::now();
        let jumped = match self.playback {
            Some((was, at, when)) => {
                let expected = if was {
                    at + now.duration_since(when).as_secs_f64()
                } else {
                    at
                };
                was != playing || (position - expected).abs() > 1.5
            }
            None => true,
        };
        if new_track || jumped {
            let progress = Some(MediaPosition(Duration::from_secs_f64(position.max(0.0))));
            let playback = if info.is_none() {
                MediaPlayback::Stopped
            } else if playing {
                MediaPlayback::Playing { progress }
            } else {
                MediaPlayback::Paused { progress }
            };
            let _ = controls.set_playback(playback);
            self.playback = Some((playing, position, now));
        }
    }

    /// The cover as a `file://` URL, written to the cache the first time
    /// this track asks.
    fn cover_url(&mut self, shown: &Shown, cover: &slint::Image) -> Option<String> {
        let key = format!("{}\u{1}{}\u{1}{}", shown.title, shown.artist, shown.album);
        if key != self.cover_for {
            self.cover_for = key.clone();
            if let Some(old) = self.cover_file.take() {
                let _ = std::fs::remove_file(old);
            }
            self.cover_file = write_cover(&key, cover);
        }
        self.cover_file
            .as_ref()
            .map(|p| format!("file://{}", p.display()))
    }
}

/// Saves `cover` as a PNG in the cache, named by the track so the system
/// doesn't show an older one it kept.
fn write_cover(key: &str, cover: &slint::Image) -> Option<PathBuf> {
    let pixels = cover.to_rgba8()?;
    let image =
        image::RgbaImage::from_raw(pixels.width(), pixels.height(), pixels.as_bytes().to_vec())?;
    let image = if image.width().max(image.height()) > COVER_SIDE {
        image::imageops::thumbnail(
            &image,
            COVER_SIDE.min(image.width() * COVER_SIDE / image.height().max(1)),
            COVER_SIDE.min(image.height() * COVER_SIDE / image.width().max(1)),
        )
    } else {
        image
    };
    let dir = djsugar_engine::cache::cache_dir()?.join("now-playing");
    std::fs::create_dir_all(&dir).ok()?;
    let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    });
    let path = dir.join(format!("cover-{hash:016x}.png"));
    image.save(&path).ok()?;
    Some(path)
}

/// The system's controls, once the main window is up; `None` while it
/// isn't (Windows needs its handle).
fn create(app: &AppWindow) -> Option<Result<MediaControls, String>> {
    #[cfg(target_os = "windows")]
    let hwnd = {
        use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use slint::winit_030::WinitWindowAccessor;
        let hwnd = app
            .window()
            .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
                RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut std::ffi::c_void),
                _ => None,
            })
            .flatten();
        Some(hwnd?)
    };
    #[cfg(not(target_os = "windows"))]
    let hwnd = {
        let _ = app;
        None
    };
    // MPRIS lives on the session bus; without one there is nothing to talk
    // to.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let bus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
            || std::env::var_os("XDG_RUNTIME_DIR")
                .is_some_and(|d| std::path::Path::new(&d).join("bus").exists());
        if !bus {
            return Some(Err("no D-Bus session bus".into()));
        }
    }
    let config = PlatformConfig {
        display_name: "DJ Sugar",
        dbus_name: "djsugar",
        hwnd,
    };
    Some(MediaControls::new(config).map_err(|e| format!("{e:?}")))
}

/// Acts on an event from the system as the Player bar's buttons do.
fn act(app: &AppWindow, event: MediaControlEvent) {
    let (d, _) = transport::deck(app);
    let length = app
        .get_decks()
        .row_data(d)
        .filter(|i| i.loaded)
        .map_or(0.0, |i| i.duration.max(0.0) as f64);
    let progress = app
        .get_decks()
        .row_data(d)
        .map_or(0.0, |i| i.progress as f64);
    let seek_by = |seconds: f64| {
        if length > 0.0 {
            transport::seek(app, ((progress * length + seconds) / length) as f32);
        }
    };
    match event {
        MediaControlEvent::Play => transport::play(app),
        MediaControlEvent::Pause | MediaControlEvent::Stop => transport::pause(app),
        MediaControlEvent::Toggle => transport::play_pause(app),
        MediaControlEvent::Next => transport::next(app),
        MediaControlEvent::Previous => transport::previous(app),
        MediaControlEvent::Seek(direction) => seek_by(signed(direction, SEEK_STEP)),
        MediaControlEvent::SeekBy(direction, by) => seek_by(signed(direction, by)),
        MediaControlEvent::SetPosition(MediaPosition(at)) => {
            if length > 0.0 {
                transport::seek(app, (at.as_secs_f64() / length) as f32);
            }
        }
        MediaControlEvent::Raise => {
            let _ = app.show();
        }
        _ => {}
    }
}

fn signed(direction: SeekDirection, by: Duration) -> f64 {
    match direction {
        SeekDirection::Forward => by.as_secs_f64(),
        SeekDirection::Backward => -by.as_secs_f64(),
    }
}
