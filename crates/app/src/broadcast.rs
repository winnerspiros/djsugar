//! Broadcasting from the app: the stations in settings.json, their
//! passwords in the system keychain, the broadcaster fed from the engine's
//! master, the audible track for now-playing, and the ON AIR panel's rows.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_broadcast::{
    Broadcaster, NowPlaying, Settings, Source, State, Station, StationStatus, Target,
};
use djsugar_control::ControlRegistry;
use djsugar_engine::{deck_group, AudioEngine, LoadedTracks, MasterFeed, NUM_DECKS};
use serde_json::json;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{settings, AppWindow, BroadcastState, BroadcastStation};

/// Settings keys.
const STATIONS: &str = "broadcast_stations";
pub const TITLE_TEMPLATE: &str = "broadcast_title_template";
pub const DJ_NAME: &str = "broadcast_dj_name";
/// Broadcasting switched on in Settings › Broadcast: ON AIR shows in the
/// top bar. Off by default, for DJs who never broadcast.
pub const ENABLED: &str = "broadcast_enabled";

/// The keychain's name for DJ Sugar's broadcast passwords; each station's
/// entry is under its id.
const KEYCHAIN_SERVICE: &str = "DJ Sugar broadcast";
/// A password is written to the keychain once typing has stopped this long.
const SAVE_AFTER: Duration = Duration::from_secs(1);
/// A deck counts as heard from this channel fader position up, as for the
/// recording's cue sheet.
const AUDIBLE: f64 = 0.05;

/// The engine's master, for the broadcaster.
struct Master(MasterFeed);

impl Source for Master {
    fn drain_into(&mut self, out: &mut Vec<f32>) {
        self.0.drain_into(out);
    }

    fn set_active(&self, on: bool) {
        self.0.set_active(on);
    }
}

/// Nothing to stream: for listing the controls without an engine.
struct Silence;

impl Source for Silence {
    fn drain_into(&mut self, _: &mut Vec<f32>) {}
    fn set_active(&self, _: bool) {}
}

/// Creates the broadcast controls, for the list of supported controls.
pub fn create_controls(registry: &Arc<ControlRegistry>) {
    let b = Broadcaster::new(registry, 48_000, Box::new(Silence), Arc::new(|| None));
    // The controls stay in the registry; the broadcaster isn't needed.
    drop(b);
}

/// The stations as saved, in their order.
pub fn stations() -> Vec<Station> {
    serde_json::from_value(settings::load()[STATIONS].clone()).unwrap_or_default()
}

fn save_stations(stations: &[Station]) {
    settings::set(STATIONS, json!(stations));
}

/// A new station's id: random, so it never names another's keychain entry.
pub fn new_id() -> String {
    let mut b = [0u8; 8];
    let _ = getrandom::fill(&mut b);
    format!("{:016x}", u64::from_le_bytes(b))
}

/// Passwords: read from the keychain once, kept here while the app runs,
/// written back after typing stops. Where there is no keychain they last
/// for the session only.
#[derive(Default)]
struct Secrets {
    known: HashMap<String, String>,
    /// Typed but not yet in the keychain, and when.
    unsaved: HashMap<String, Instant>,
    /// The keychain couldn't be used; why.
    trouble: Option<String>,
}

impl Secrets {
    fn get(&mut self, id: &str) -> String {
        if let Some(p) = self.known.get(id) {
            return p.clone();
        }
        let p = match keychain(id).and_then(|e| e.get_password().map_err(|e| e.to_string())) {
            Ok(p) => p,
            Err(e) => {
                if !e.contains("No matching entry") && !e.contains("NoEntry") {
                    self.trouble.get_or_insert(e);
                }
                String::new()
            }
        };
        self.known.insert(id.to_string(), p.clone());
        p
    }

    fn set(&mut self, id: &str, password: &str) {
        self.known.insert(id.to_string(), password.to_string());
        self.unsaved.insert(id.to_string(), Instant::now());
    }

    /// Writes the passwords typing has stopped on; all of them if `now`.
    fn save(&mut self, now: bool) {
        let due: Vec<String> = self
            .unsaved
            .iter()
            .filter(|(_, t)| now || t.elapsed() >= SAVE_AFTER)
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            self.unsaved.remove(&id);
            let password = self.known.get(&id).cloned().unwrap_or_default();
            let result = keychain(&id).and_then(|e| {
                if password.is_empty() {
                    match e.delete_credential() {
                        Err(keyring::Error::NoEntry) => Ok(()),
                        r => r.map_err(|e| e.to_string()),
                    }
                } else {
                    e.set_password(&password).map_err(|e| e.to_string())
                }
            });
            match result {
                Ok(()) => self.trouble = None,
                Err(e) => self.trouble = Some(e),
            }
        }
    }

    fn forget(&mut self, id: &str) {
        self.known.remove(id);
        self.unsaved.remove(id);
        if let Ok(e) = keychain(id) {
            let _ = e.delete_credential();
        }
    }
}

fn keychain(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, id).map_err(|e| e.to_string())
}

/// The track heard most: playing, its channel fader up, weighted by the
/// crossfader's side. None when nothing is heard.
fn audible(registry: &ControlRegistry, loaded: &LoadedTracks) -> Option<(String, String)> {
    let (_, track) = audible_deck(registry, loaded)?;
    Some((track.artist.clone(), track.title.clone()))
}

/// The deck heard most in the master mix and its track: playing, with a
/// track, its channel fader up, weighted by the crossfader's side. A deck
/// heard only in the headphones (fader down) never counts.
pub fn audible_deck(
    registry: &ControlRegistry,
    loaded: &LoadedTracks,
) -> Option<(usize, std::sync::Arc<djsugar_engine::Track>)> {
    let get = |g: &str, k: &str, d: f64| registry.get(g, k).map_or(d, |c| c.get());
    let xfader = if get("[Mixer]", "crossfader_enabled", 1.0) > 0.5 {
        get("[Master]", "crossfader", 0.0)
    } else {
        0.0
    };
    let tracks = loaded.read().ok()?;
    let levels: Vec<Option<f64>> = (0..NUM_DECKS)
        .map(|d| {
            let g = deck_group(d);
            if tracks[d].is_none() || get(&g, "play", 0.0) < 0.5 {
                return None;
            }
            let orientation = get(&g, "orientation", 1.0).round() as i32;
            Some(get(&g, "volume", 0.0) * crossfader_side(orientation, xfader))
        })
        .collect();
    let d = loudest(&levels)?;
    Some((d, tracks[d].clone()?))
}

/// How much of a deck the crossfader lets through: left decks (0) fade as
/// it moves right, right decks (2) as it moves left, others never.
fn crossfader_side(orientation: i32, xfader: f64) -> f64 {
    match orientation {
        0 => (1.0 - xfader).clamp(0.0, 1.0),
        2 => (1.0 + xfader).clamp(0.0, 1.0),
        _ => 1.0,
    }
}

/// The deck with the highest level above [`AUDIBLE`]; the first on a tie.
fn loudest(levels: &[Option<f64>]) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (d, level) in levels.iter().enumerate() {
        if let Some(level) = *level {
            if level > AUDIBLE && best.is_none_or(|(b, _)| level > b) {
                best = Some((level, d));
            }
        }
    }
    best.map(|(_, d)| d)
}

/// Broadcasting in the app.
pub struct Broadcast {
    broadcaster: Broadcaster,
    secrets: RefCell<Secrets>,
    /// What the panel shows, so the model is only replaced on a change.
    shown: RefCell<(Vec<StationStatus>, String, bool, bool)>,
}

impl Broadcast {
    /// Starts the broadcaster on the engine's master, or None if the engine
    /// has already given its master away.
    pub fn start(registry: &Arc<ControlRegistry>, engine: &AudioEngine) -> Option<Rc<Self>> {
        let feed = engine.take_master_feed()?;
        let loaded = engine.loaded_tracks();
        let reg = registry.clone();
        let now_playing: NowPlaying = Arc::new(move || audible(&reg, &loaded));
        let broadcaster = Broadcaster::new(
            registry,
            engine.sample_rate().round() as u32,
            Box::new(Master(feed)),
            now_playing,
        );
        let b = Rc::new(Self {
            broadcaster,
            secrets: RefCell::default(),
            shown: RefCell::default(),
        });
        b.configure();
        Some(b)
    }

    /// Hands the broadcaster the saved stations and their passwords.
    /// Passwords are only read from the keychain for stations switched on.
    pub fn configure(&self) {
        let mut secrets = self.secrets.borrow_mut();
        let targets = stations()
            .into_iter()
            .map(|station| Target {
                password: if station.enabled {
                    secrets.get(&station.id)
                } else {
                    String::new()
                },
                station,
            })
            .collect();
        self.broadcaster.configure(Settings {
            targets,
            title_template: settings::string(TITLE_TEMPLATE),
            dj_name: settings::string(DJ_NAME),
        });
    }

    pub fn password(&self, id: &str) -> String {
        self.secrets.borrow_mut().get(id)
    }

    pub fn set_password(&self, id: &str, password: &str) {
        self.secrets.borrow_mut().set(id, password);
        self.configure();
    }

    /// Why passwords can't be kept in the keychain, if they can't.
    pub fn keychain_trouble(&self) -> Option<String> {
        self.secrets.borrow().trouble.clone()
    }

    /// Changes station `index` with `f` and saves.
    pub fn edit(&self, index: usize, f: impl FnOnce(&mut Station)) {
        let mut all = stations();
        if let Some(s) = all.get_mut(index) {
            f(s);
            save_stations(&all);
            self.configure();
        }
    }

    /// Adds a station; returns its place.
    pub fn add(&self) -> usize {
        let mut all = stations();
        all.push(Station {
            id: new_id(),
            name: format!("Station {}", all.len() + 1),
            ..Station::default()
        });
        save_stations(&all);
        self.configure();
        all.len() - 1
    }

    pub fn remove(&self, index: usize) {
        let mut all = stations();
        if index < all.len() {
            let s = all.remove(index);
            self.secrets.borrow_mut().forget(&s.id);
            save_stations(&all);
            self.configure();
        }
    }

    /// Connects to station `index` once and hangs up, for its Test button.
    /// Runs in the background and calls `done` on the UI thread.
    pub fn test(&self, index: usize, done: impl FnOnce(Result<(), String>) + Send + 'static) {
        let Some(station) = stations().into_iter().nth(index) else {
            return;
        };
        let password = self.password(&station.id);
        std::thread::spawn(move || {
            let result = if station.host.trim().is_empty() {
                Err("Fill in the server first.".to_string())
            } else {
                Broadcaster::test(&station, &password)
            };
            let _ = slint::invoke_from_event_loop(move || done(result));
        });
    }

    /// Called a few times a second: saves typed passwords and updates the
    /// ON AIR panel.
    pub fn poll(&self, app: &AppWindow) {
        self.secrets.borrow_mut().save(false);
        let statuses = if self.broadcaster.is_on_air() {
            self.broadcaster.statuses()
        } else {
            Vec::new()
        };
        let error = self.broadcaster.last_error();
        let all = stations();
        let any = all.iter().any(|s| s.enabled);
        // ON AIR shows once broadcasting is on and a station is set up.
        let shown = settings::flag(ENABLED, false)
            && all.iter().any(|s| s.enabled && !s.host.trim().is_empty());
        let now = (statuses, error, any, shown);
        if *self.shown.borrow() == now {
            return;
        }
        let g = app.global::<BroadcastState>();
        let rows: Vec<BroadcastStation> = now
            .0
            .iter()
            .map(|s| BroadcastStation {
                name: s.name.clone().into(),
                state: match s.state {
                    State::Off => 0,
                    State::Connecting => 1,
                    State::Connected => 2,
                    State::Failed => 3,
                },
                message: if s.behind && s.message.is_empty() {
                    "The upload fell behind".into()
                } else {
                    s.message.clone().into()
                },
                listeners: s.listeners.map_or(-1, |n| n as i32),
            })
            .collect();
        g.set_listeners(
            now.0
                .iter()
                .filter_map(|s| s.listeners)
                .map(|n| n as i32)
                .reduce(|a, b| a + b)
                .unwrap_or(-1),
        );
        g.set_stations(ModelRc::new(VecModel::from(rows)));
        g.set_error(now.1.clone().into());
        g.set_ready(now.2);
        g.set_shown(now.3);
        *self.shown.borrow_mut() = now;
    }

    /// Off air and passwords saved, at quit.
    pub fn shut_down(&self) {
        self.broadcaster.stop();
        self.secrets.borrow_mut().save(true);
    }
}

#[cfg(test)]
mod audible_tests {
    use super::*;

    #[test]
    fn the_loudest_deck_in_the_mix_is_heard() {
        // A blend: deck 2 passes deck 1 halfway through.
        assert_eq!(loudest(&[Some(0.8), Some(0.5), None, None]), Some(0));
        assert_eq!(loudest(&[Some(0.4), Some(0.9), None, None]), Some(1));
        // A deck cued in the headphones with its fader down isn't heard.
        assert_eq!(loudest(&[None, None, Some(0.0), None]), None);
        // A cut with the crossfader: full right silences a left deck.
        let left = 1.0 * crossfader_side(0, 1.0);
        let right = 1.0 * crossfader_side(2, 1.0);
        assert_eq!(loudest(&[Some(left), Some(right), None, None]), Some(1));
    }
}
