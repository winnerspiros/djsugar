//! The now-playing feed for streaming and overlay software: the track the
//! audience hears, as `now-playing.txt` (one line from a template) and
//! `now-playing.json` (with the last ten tracks) in a folder the DJ picks,
//! and as an overlay page for OBS (see `djsugar_remote::overlay`).
//!
//! Off until switched on in Settings › Broadcast. The track is the deck
//! heard most in the master mix (as for the radio title), once it has been
//! for the hold time; a deck cued in the headphones never counts. Runs on
//! the UI thread a few times a second; files are written on a thread of
//! their own, each to a temporary file that is then renamed over the old
//! one, so a reader never sees half a file.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use djsugar_control::ControlRegistry;
use djsugar_engine::{LoadedTracks, Track};
use djsugar_remote::overlay::{self, Overlay};
use serde_json::{json, Value};

use crate::{broadcast, settings};

/// The switch: off at first.
pub const ENABLED: &str = "now_playing";
/// Where the files go; empty is `~/Music/DJSugar/Now Playing`.
pub const FOLDER: &str = "now_playing_folder";
/// The text file's line.
pub const TEMPLATE: &str = "now_playing_template";
/// Seconds a deck must be the one heard before it counts.
pub const HOLD: &str = "now_playing_hold";
/// The overlay's look, an index into `overlay::STYLES`.
pub const STYLE: &str = "now_playing_style";
/// Whether other devices on the network may open the overlay.
pub const NETWORK: &str = "now_playing_network";

pub const DEFAULT_TEMPLATE: &str = "{artist} - {title}";
pub const DEFAULT_HOLD: f64 = 10.0;
/// Tracks kept in the JSON's `recent`.
const RECENT: usize = 10;

/// Set when a setting changes, so the next tick reads them again.
static CHANGED: AtomicBool = AtomicBool::new(true);
/// Whether the karaoke page is wanted (see [`set_karaoke_page`]).
static KARAOKE: AtomicBool = AtomicBool::new(false);
/// The karaoke page's address on the network while it is served.
static KARAOKE_URL: Mutex<String> = Mutex::new(String::new());
/// What Settings shows under the switch: the overlay's address, or why it
/// couldn't start.
static STATUS: Mutex<String> = Mutex::new(String::new());

/// The overlay's address while the feed runs, or what went wrong.
pub fn status() -> String {
    STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Tells the feed its settings changed.
pub fn settings_changed() {
    CHANGED.store(true, Ordering::Release);
}

/// Serves the karaoke page (`/karaoke`) on the overlay's server, open to
/// other devices on the network, or stops it. The server runs while the
/// feed or the page needs it.
pub fn set_karaoke_page(on: bool) {
    if KARAOKE.swap(on, Ordering::AcqRel) != on {
        CHANGED.store(true, Ordering::Release);
    }
    if !on {
        djsugar_remote::overlay::set_karaoke(r#"{"state":0}"#.into(), None);
    }
}

/// Where other devices open the karaoke page while it is served; empty
/// otherwise (or until the server has started).
pub fn karaoke_url() -> String {
    KARAOKE_URL.lock().map(|s| s.clone()).unwrap_or_default()
}

/// The folder the files are written to.
pub fn folder() -> PathBuf {
    settings::path_of(FOLDER).unwrap_or_else(|| {
        djsugar_engine::scan::default_music_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("DJ Sugar")
            .join("Now Playing")
    })
}

#[derive(Clone, PartialEq)]
struct Config {
    enabled: bool,
    folder: PathBuf,
    template: String,
    hold: Duration,
    style: usize,
    network: bool,
}

impl Config {
    fn read() -> Self {
        let template = settings::string(TEMPLATE);
        Config {
            enabled: settings::flag(ENABLED, false),
            folder: folder(),
            template: if template.trim().is_empty() {
                DEFAULT_TEMPLATE.into()
            } else {
                template
            },
            hold: Duration::from_secs_f64(settings::number(HOLD, DEFAULT_HOLD).clamp(0.0, 60.0)),
            style: settings::number(STYLE, 0.0).max(0.0) as usize,
            network: settings::flag(NETWORK, false),
        }
    }
}

/// What a track is, to tell a change from the same track.
type Key = (usize, usize, String, String);

/// The track now playing.
#[derive(Clone)]
struct Playing {
    key: Key,
    fields: Value,
    cover: Option<Arc<Vec<u8>>>,
}

pub struct NowPlaying {
    registry: Arc<ControlRegistry>,
    loaded: LoadedTracks,
    config: Option<Config>,
    /// What is heard now and since when, until it has lasted the hold time.
    candidate: Option<(Option<Key>, Instant)>,
    playing: Option<Playing>,
    recent: VecDeque<Value>,
    overlay: Option<Overlay>,
    writer: Sender<(PathBuf, String, String)>,
}

impl NowPlaying {
    pub fn new(registry: Arc<ControlRegistry>, loaded: LoadedTracks) -> Self {
        let (writer, jobs) = mpsc::channel::<(PathBuf, String, String)>();
        let _ = std::thread::Builder::new()
            .name("now playing files".into())
            .spawn(move || {
                while let Ok((dir, text, json)) = jobs.recv() {
                    if let Err(e) = write_files(&dir, &text, &json) {
                        eprintln!(
                            "can't write the now-playing files in {}: {e}",
                            dir.display()
                        );
                    }
                }
            });
        Self {
            registry,
            loaded,
            config: None,
            candidate: None,
            playing: None,
            recent: VecDeque::new(),
            overlay: None,
            writer,
        }
    }

    /// Follows the mix; call a few times a second.
    pub fn tick(&mut self) {
        if CHANGED.swap(false, Ordering::AcqRel) {
            self.reconfigure();
        }
        let Some(config) = self.config.clone().filter(|c| c.enabled) else {
            return;
        };
        let heard = broadcast::audible_deck(&self.registry, &self.loaded);
        let heard_key = heard.as_ref().map(|(d, t)| key(*d, t));
        if heard_key.as_ref() == self.playing.as_ref().map(|p| &p.key) {
            self.candidate = None;
            return;
        }
        let since = match &self.candidate {
            Some((k, since)) if *k == heard_key => *since,
            _ => Instant::now(),
        };
        // With nothing showing, a track counts at once, so the overlay isn't
        // empty when the feed is switched on mid-set.
        let first = self.playing.is_none() && heard.is_some();
        if !first && since.elapsed() < config.hold {
            self.candidate = Some((heard_key, since));
            return;
        }
        self.candidate = None;
        if self.playing.is_none() && heard.is_none() {
            return;
        }
        self.playing = heard.map(|(d, t)| playing(d, &t));
        if let Some(p) = &self.playing {
            self.recent.push_front(p.fields.clone());
            self.recent.truncate(RECENT + 1);
        }
        self.publish(&config);
    }

    fn reconfigure(&mut self) {
        let config = Config::read();
        self.config = Some(config.clone());
        let karaoke = KARAOKE.load(Ordering::Acquire);
        if !config.enabled {
            self.playing = None;
            self.candidate = None;
            set_status(String::new());
        }
        if !config.enabled && !karaoke {
            self.overlay = None;
            set_karaoke_url(String::new());
            return;
        }
        // The karaoke page is for a TV or another screen, so it needs the
        // network.
        let network = config.network || karaoke;
        let restart = match &self.overlay {
            None => true,
            Some(o) => o.network() != network,
        };
        if restart {
            self.overlay = None;
            match Overlay::start(overlay::DEFAULT_PORT, network) {
                Ok(o) => {
                    if config.enabled {
                        set_status(o.url());
                    }
                    self.overlay = Some(o);
                }
                Err(e) => {
                    if config.enabled {
                        set_status(format!("The overlay page can't start: {e}"));
                    }
                }
            }
        }
        set_karaoke_url(match (&self.overlay, karaoke) {
            (Some(o), true) => {
                let ip = djsugar_remote::local_addresses()
                    .first()
                    .map(|(_, ip)| ip.to_string())
                    .unwrap_or_else(|| "127.0.0.1".into());
                format!("http://{ip}:{}/karaoke", o.port())
            }
            _ => String::new(),
        });
        if !config.enabled {
            // Only the karaoke page: the overlay shows nothing.
            if let Some(o) = &self.overlay {
                o.set("{}".into(), None, config.style);
            }
            return;
        }
        // Everything is sent again: the folder, line or style may differ.
        self.publish(&config);
    }

    fn publish(&self, config: &Config) {
        let current = self.playing.as_ref().map(|p| p.fields.clone());
        // Recent tracks before this one, newest first.
        let skip = usize::from(self.playing.is_some());
        let recent: Vec<Value> = self
            .recent
            .iter()
            .skip(skip)
            .take(RECENT)
            .cloned()
            .collect();
        let json = json!({ "current": current, "recent": recent });
        let text = match &self.playing {
            Some(p) => fill(&config.template, &p.fields),
            None => String::new(),
        };
        let json_text = serde_json::to_string_pretty(&json).unwrap_or_default();
        let _ = self.writer.send((config.folder.clone(), text, json_text));
        if let Some(o) = &self.overlay {
            // The page follows a change of style without being reloaded.
            let mut page = json;
            page["style"] = json!(overlay::STYLES[config.style.min(overlay::STYLES.len() - 1)]);
            o.set(
                page.to_string(),
                self.playing.as_ref().and_then(|p| p.cover.clone()),
                config.style,
            );
        }
    }
}

fn set_karaoke_url(s: String) {
    if let Ok(mut url) = KARAOKE_URL.lock() {
        *url = s;
    }
}

fn set_status(s: String) {
    if let Ok(mut status) = STATUS.lock() {
        *status = s;
    }
}

fn key(deck: usize, t: &Track) -> Key {
    (deck, t.id, t.artist.clone(), t.title.clone())
}

fn playing(deck: usize, t: &Track) -> Playing {
    let started = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let cover = t.cover.as_ref().and_then(|c| jpeg(c));
    Playing {
        key: key(deck, t),
        fields: json!({
            "artist": t.artist,
            "title": t.title,
            "album": t.album,
            "label": t.info.label,
            "genre": t.genre,
            "bpm": if t.bpm > 0.0 { json!((t.bpm * 10.0).round() / 10.0) } else { Value::Null },
            "key": t.key,
            "deck": deck + 1,
            "started": started,
            "cover": cover.is_some(),
        }),
        cover: cover.map(Arc::new),
    }
}

/// The template with the track's fields filled in.
fn fill(template: &str, fields: &Value) -> String {
    let field = |k: &str| match &fields[k] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    let mut out = template.to_string();
    for k in ["artist", "title", "album", "bpm", "key", "label", "genre"] {
        out = out.replace(&format!("{{{k}}}"), &field(k));
    }
    out.trim_matches(|c: char| c.is_whitespace() || c == '-' || c == '–')
        .to_string()
}

fn jpeg(cover: &djsugar_engine::cover::Cover) -> Option<Vec<u8>> {
    let rgba = image::RgbaImage::from_raw(cover.width, cover.height, cover.rgba.clone())?;
    let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&rgb)
        .ok()?;
    Some(out)
}

/// Writes both files, each whole: to a temporary name, then renamed.
fn write_files(dir: &std::path::Path, text: &str, json: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, body) in [("now-playing.txt", text), ("now-playing.json", json)] {
        let tmp = dir.join(format!(".{name}.tmp"));
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, dir.join(name))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_is_filled_in() {
        let f = json!({"artist": "Band", "title": "Tune", "bpm": 124.0, "key": "8A", "label": ""});
        assert_eq!(fill(DEFAULT_TEMPLATE, &f), "Band - Tune");
        assert_eq!(fill("{title} ({bpm}, {key})", &f), "Tune (124.0, 8A)");
        let no_artist = json!({"artist": "", "title": "Tune"});
        assert_eq!(fill(DEFAULT_TEMPLATE, &no_artist), "Tune");
    }

    #[test]
    fn files_are_replaced_whole() {
        let dir = std::env::temp_dir().join(format!("djsugar-np-{}", std::process::id()));
        write_files(&dir, "A - B", "{}").unwrap();
        write_files(&dir, "C - D", "{\"x\":1}").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("now-playing.txt")).unwrap(),
            "C - D"
        );
        assert!(!dir.join(".now-playing.txt.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
