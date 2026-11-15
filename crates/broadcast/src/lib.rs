//! Live broadcasting: streams the master mix to internet radio servers.
//!
//! The engine copies the master into a ring buffer while on air (a tap
//! like the recorder's). Here a pump thread empties it and hands each
//! station a copy through its own bounded queue; each station has a thread
//! that encodes and sends, and reconnects when the connection drops. A slow
//! or dead network only ever affects that station's thread: the audio
//! thread never waits.
//!
//! The controls use the standard names, so mappings with a broadcast button work:
//! `[Shoutcast],enabled` goes on and off air, `[Shoutcast],status` is 0 off,
//! 1 connecting, 2 connected to every station, 3 failed, 4 connected to
//! some. `[Broadcast],on_air_seconds` counts the time on air.

pub mod encode;
pub mod mixcloud;
pub mod resample;
pub mod server;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
use serde::{Deserialize, Serialize};

pub use server::ConnectError;

/// The kind of server a station is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerKind {
    #[default]
    Icecast,
    Shoutcast,
}

/// How the stream is encoded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// Opus in Ogg: the best sound for the bitrate.
    #[default]
    Opus,
    /// MP3: plays everywhere, and the only one Shoutcast takes here.
    Mp3,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Opus => "Opus",
            Format::Mp3 => "MP3",
        }
    }
}

/// A radio server to stream to. The password is kept apart (in the
/// system keychain), never in here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Station {
    /// Stays the same when the station is renamed; names its keychain entry.
    pub id: String,
    pub name: String,
    /// Streams when ON AIR is pressed.
    pub enabled: bool,
    pub server: ServerKind,
    pub host: String,
    pub port: u16,
    /// Icecast's mount point, such as `/live`.
    pub mount: String,
    /// Icecast's source user; `source` when empty.
    pub user: String,
    pub tls: bool,
    pub format: Format,
    /// kb/s.
    pub bitrate: u32,
    pub stream_name: String,
    pub description: String,
    pub genre: String,
    pub website: String,
    /// Listed in the Icecast or Shoutcast directory.
    pub public: bool,
    /// Sends each new title inside an Opus stream, by starting a new chained
    /// Ogg stream. Browsers and VLC follow; some players (those built on
    /// FFmpeg 6 and older) stop there, so it's off unless chosen.
    pub opus_titles: bool,
}

impl Default for Station {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "My station".into(),
            enabled: true,
            server: ServerKind::Icecast,
            host: String::new(),
            port: 8000,
            mount: "/live".into(),
            user: "source".into(),
            tls: false,
            format: Format::Mp3,
            bitrate: 128,
            stream_name: String::new(),
            description: String::new(),
            genre: String::new(),
            website: String::new(),
            public: false,
            opus_titles: false,
        }
    }
}

impl Station {
    /// Opus can't go to Shoutcast; MP3 instead.
    pub fn effective_format(&self) -> Format {
        match self.server {
            ServerKind::Shoutcast => Format::Mp3,
            ServerKind::Icecast => self.format,
        }
    }

    /// A short line for lists: "radio.example.org:8000/live · Opus 96".
    pub fn summary(&self) -> String {
        let mount = match self.server {
            ServerKind::Icecast => self.mount.trim().to_string(),
            ServerKind::Shoutcast => String::new(),
        };
        format!(
            "{}:{}{} · {} {}",
            self.host,
            self.port,
            mount,
            self.effective_format().name(),
            self.bitrate
        )
    }
}

/// Where a station is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Off,
    Connecting,
    Connected,
    /// Gave up: wrong password, or no luck for ten minutes.
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StationStatus {
    pub id: String,
    pub name: String,
    pub state: State,
    /// Why it isn't connected, or the last trouble.
    pub message: String,
    /// From Icecast's status page, when it reports them.
    pub listeners: Option<u32>,
    /// The upload fell behind; the station is reconnecting.
    pub behind: bool,
}

/// What the engine provides: the master, copied while switched on.
pub trait Source: Send + 'static {
    /// Moves the interleaved stereo waiting since the last call onto `out`.
    fn drain_into(&mut self, out: &mut Vec<f32>);
    /// Starts or stops the copying.
    fn set_active(&self, on: bool);
}

/// The audible track's artist and title, or None when nothing plays.
pub type NowPlaying = Arc<dyn Fn() -> Option<(String, String)> + Send + Sync>;

/// How the now-playing text is made: `{artist}`, `{title}` and `{dj}` are
/// filled in; text without them is sent as it is.
pub const DEFAULT_TITLE_TEMPLATE: &str = "{artist} - {title}";

/// Fills in the now-playing template.
pub fn title_text(template: &str, dj: &str, artist: &str, title: &str) -> String {
    let t = template.trim();
    let t = if t.is_empty() {
        DEFAULT_TITLE_TEMPLATE
    } else {
        t
    };
    let text = t
        .replace("{artist}", artist)
        .replace("{title}", title)
        .replace("{dj}", dj);
    // "- Title" when there's no artist, and so on.
    text.trim_matches(|c: char| c.is_whitespace() || c == '–' || c == '-')
        .to_string()
}

/// One station to stream to, with its password.
#[derive(Clone)]
pub struct Target {
    pub station: Station,
    pub password: String,
}

#[derive(Clone, Default)]
pub struct Settings {
    pub targets: Vec<Target>,
    pub title_template: String,
    pub dj_name: String,
}

/// How long the queue to a station holds before it counts as behind.
const QUEUE_SECONDS: f64 = 10.0;
/// How often the pump empties the engine's buffer.
const PUMP_EVERY: Duration = Duration::from_millis(20);
/// Reconnect delays: at once, then every 5 s, for up to 10 minutes.
const RETRY_EVERY: Duration = Duration::from_secs(5);
const GIVE_UP_AFTER: Duration = Duration::from_secs(600);
/// A new track's title goes out once it has been the audible one this long,
/// so a quick cut back and forth doesn't flicker.
const TITLE_SETTLE: Duration = Duration::from_secs(2);
/// How often listener counts are asked for.
const LISTENERS_EVERY: Duration = Duration::from_secs(15);

type Chunk = Arc<[f32]>;

/// The title every station should send, and a counter that moves when it
/// changes.
#[derive(Default)]
struct Title {
    text: Mutex<(String, String, String)>,
    version: AtomicU64,
}

struct Running {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    started: Instant,
}

struct Inner {
    rate: u32,
    source: Mutex<Option<Box<dyn Source>>>,
    now_playing: NowPlaying,
    settings: Mutex<Settings>,
    statuses: Arc<Mutex<Vec<StationStatus>>>,
    running: Mutex<Option<Running>>,
    /// Why the last press of ON AIR didn't go on air.
    error: Mutex<String>,
    enabled: ControlHandle,
    status: ControlHandle,
    seconds: ControlHandle,
}

/// Goes on and off air. Cheap to clone.
#[derive(Clone)]
pub struct Broadcaster(Arc<Inner>);

impl Broadcaster {
    /// Creates the controls and a thread that answers `[Shoutcast],enabled`.
    pub fn new(
        registry: &Arc<ControlRegistry>,
        rate: u32,
        source: Box<dyn Source>,
        now_playing: NowPlaying,
    ) -> Self {
        let enabled = registry.toggle("[Shoutcast]", "enabled");
        let inner = Arc::new(Inner {
            rate,
            source: Mutex::new(Some(source)),
            now_playing,
            settings: Mutex::default(),
            statuses: Arc::default(),
            running: Mutex::new(None),
            error: Mutex::default(),
            enabled: enabled.clone(),
            status: registry.indicator("[Shoutcast]", "status", 4.0),
            seconds: registry.indicator("[Broadcast]", "on_air_seconds", f64::MAX),
        });
        let weak = Arc::downgrade(&inner);
        std::thread::Builder::new()
            .name("broadcast-switch".into())
            .spawn(move || watch(weak, enabled))
            .expect("spawn broadcast switch");
        Self(inner)
    }

    /// The stations and the now-playing text; takes effect the next time
    /// ON AIR is pressed.
    pub fn configure(&self, settings: Settings) {
        *self.0.settings.lock().unwrap() = settings;
    }

    pub fn is_on_air(&self) -> bool {
        self.0.running.lock().unwrap().is_some()
    }

    /// Seconds since going on air.
    pub fn on_air_seconds(&self) -> f64 {
        self.0
            .running
            .lock()
            .unwrap()
            .as_ref()
            .map_or(0.0, |r| r.started.elapsed().as_secs_f64())
    }

    /// Why the last attempt to go on air didn't start, or "".
    pub fn last_error(&self) -> String {
        self.0.error.lock().unwrap().clone()
    }

    pub fn statuses(&self) -> Vec<StationStatus> {
        self.0.statuses.lock().unwrap().clone()
    }

    /// Goes on air to every enabled station. Err when none is enabled.
    pub fn start(&self) -> Result<(), String> {
        start(&self.0)
    }

    /// Goes off air and waits for the stations to close.
    pub fn stop(&self) {
        stop(&self.0);
    }

    /// Connects to `station` once and hangs up: for the Test button.
    pub fn test(station: &Station, password: &str) -> Result<(), String> {
        let format = station.effective_format();
        let content_type = match format {
            Format::Opus => "audio/ogg",
            Format::Mp3 => "audio/mpeg",
        };
        server::connect(station, password, content_type)
            .map(|_| ())
            .map_err(|e| e.message().to_string())
    }
}

/// Answers `[Shoutcast],enabled`, and keeps the status and time up to date.
fn watch(weak: Weak<Inner>, enabled: ControlHandle) {
    let mut was = false;
    while let Some(inner) = weak.upgrade() {
        let on = enabled.get_bool();
        let running = inner.running.lock().unwrap().is_some();
        if on && !was && !running {
            match start(&inner) {
                Ok(()) => inner.error.lock().unwrap().clear(),
                Err(e) => {
                    eprintln!("could not go on air: {e}");
                    *inner.error.lock().unwrap() = e;
                    enabled.set_bool(false);
                }
            }
        } else if !on && was && running {
            stop(&inner);
        }
        was = enabled.get_bool();
        let (status, seconds) = {
            let r = inner.running.lock().unwrap();
            match r.as_ref() {
                Some(r) => (
                    summary(&inner.statuses.lock().unwrap()),
                    r.started.elapsed().as_secs_f64(),
                ),
                None => (0.0, 0.0),
            }
        };
        inner.status.set(status);
        inner.seconds.set(seconds.floor());
        drop(inner);
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `[Shoutcast],status` from the stations' states.
fn summary(statuses: &[StationStatus]) -> f64 {
    let n = statuses.len();
    let connected = statuses
        .iter()
        .filter(|s| s.state == State::Connected)
        .count();
    let failed = statuses.iter().filter(|s| s.state == State::Failed).count();
    if n == 0 {
        0.0
    } else if connected == n {
        2.0
    } else if failed == n {
        3.0
    } else if connected > 0 {
        4.0
    } else {
        1.0
    }
}

fn start(inner: &Arc<Inner>) -> Result<(), String> {
    let mut running = inner.running.lock().unwrap();
    if running.is_some() {
        return Ok(());
    }
    let settings = inner.settings.lock().unwrap().clone();
    let targets: Vec<Target> = settings
        .targets
        .iter()
        .filter(|t| t.station.enabled)
        .cloned()
        .collect();
    if targets.is_empty() {
        return Err("No station is switched on in Settings › Broadcast.".into());
    }
    let Some(mut source) = inner.source.lock().unwrap().take() else {
        return Err("Broadcasting is busy.".into());
    };
    let stop = Arc::new(AtomicBool::new(false));
    let title = Arc::new(Title::default());
    // Set the first title before any station starts, so it leads every
    // stream (Opus carries it only in the headers each station writes
    // when it connects).
    let first = (inner.now_playing)();
    if let Some(track) = &first {
        let text = title_text(
            &settings.title_template,
            &settings.dj_name,
            &track.0,
            &track.1,
        );
        *title.text.lock().unwrap() = (track.0.clone(), track.1.clone(), text);
        title.version.fetch_add(1, Ordering::AcqRel);
    }
    *inner.statuses.lock().unwrap() = targets
        .iter()
        .map(|t| StationStatus {
            id: t.station.id.clone(),
            name: t.station.name.clone(),
            state: State::Connecting,
            ..StationStatus::default()
        })
        .collect();
    let mut threads = Vec::new();
    let mut queues = Vec::new();
    let capacity = ((QUEUE_SECONDS / PUMP_EVERY.as_secs_f64()) as usize).max(8);
    for (i, t) in targets.into_iter().enumerate() {
        let (tx, rx) = mpsc::sync_channel::<Chunk>(capacity);
        let behind = Arc::new(AtomicBool::new(false));
        queues.push((tx, behind.clone()));
        let job = StationJob {
            index: i,
            target: t,
            rate: inner.rate,
            rx,
            behind,
            stop: stop.clone(),
            title: title.clone(),
            statuses: inner.statuses.clone(),
        };
        threads.push(
            std::thread::Builder::new()
                .name("broadcast-station".into())
                .spawn(move || job.run())
                .map_err(|e| e.to_string())?,
        );
    }
    // Anything left over in the buffer from before.
    let mut scratch = Vec::new();
    source.drain_into(&mut scratch);
    source.set_active(true);
    {
        let (stop, inner2) = (stop.clone(), Arc::downgrade(inner));
        threads.push(
            std::thread::Builder::new()
                .name("broadcast-pump".into())
                .spawn(move || pump(source, queues, stop, inner2))
                .map_err(|e| e.to_string())?,
        );
    }
    {
        let stop = stop.clone();
        let now_playing = inner.now_playing.clone();
        let template = settings.title_template.clone();
        let dj = settings.dj_name.clone();
        threads.push(
            std::thread::Builder::new()
                .name("broadcast-title".into())
                .spawn(move || follow_title(now_playing, first, title, template, dj, stop))
                .map_err(|e| e.to_string())?,
        );
    }
    {
        let stop = stop.clone();
        let statuses = inner.statuses.clone();
        let stations: Vec<Station> = settings
            .targets
            .iter()
            .filter(|t| t.station.enabled)
            .map(|t| t.station.clone())
            .collect();
        threads.push(
            std::thread::Builder::new()
                .name("broadcast-listeners".into())
                .spawn(move || count_listeners(stations, statuses, stop))
                .map_err(|e| e.to_string())?,
        );
    }
    inner.enabled.set_bool(true);
    *running = Some(Running {
        stop,
        threads,
        started: Instant::now(),
    });
    Ok(())
}

fn stop(inner: &Arc<Inner>) {
    let Some(running) = inner.running.lock().unwrap().take() else {
        return;
    };
    running.stop.store(true, Ordering::Release);
    for t in running.threads {
        let _ = t.join();
    }
    for s in inner.statuses.lock().unwrap().iter_mut() {
        s.state = State::Off;
        s.listeners = None;
        s.behind = false;
    }
    inner.enabled.set_bool(false);
    inner.status.set(0.0);
    inner.seconds.set(0.0);
}

/// Empties the engine's buffer and hands each station a copy. Gives the
/// source back when it stops.
fn pump(
    mut source: Box<dyn Source>,
    queues: Vec<(SyncSender<Chunk>, Arc<AtomicBool>)>,
    stop: Arc<AtomicBool>,
    inner: Weak<Inner>,
) {
    let mut buf = Vec::new();
    while !stop.load(Ordering::Acquire) {
        buf.clear();
        source.drain_into(&mut buf);
        if !buf.is_empty() {
            let chunk: Chunk = Arc::from(&buf[..]);
            for (tx, behind) in &queues {
                match tx.try_send(chunk.clone()) {
                    Ok(()) => {}
                    // Ten seconds behind: the station drops its connection,
                    // empties the queue and starts again.
                    Err(TrySendError::Full(_)) => behind.store(true, Ordering::Release),
                    Err(TrySendError::Disconnected(_)) => {}
                }
            }
        }
        std::thread::sleep(PUMP_EVERY);
    }
    source.set_active(false);
    if let Some(inner) = inner.upgrade() {
        *inner.source.lock().unwrap() = Some(source);
    }
}

/// Watches which track is audible and sets the title once it settles.
fn follow_title(
    now_playing: NowPlaying,
    first: Option<(String, String)>,
    title: Arc<Title>,
    template: String,
    dj: String,
    stop: Arc<AtomicBool>,
) {
    let mut sent = first;
    let mut candidate: Option<((String, String), Instant)> = None;
    while !stop.load(Ordering::Acquire) {
        match now_playing() {
            Some(track) if sent.as_ref() != Some(&track) => {
                let since = match &candidate {
                    Some((c, since)) if *c == track => *since,
                    _ => Instant::now(),
                };
                // The first title goes out at once, so it can lead the
                // stream; later ones once the mix has settled on them.
                if sent.is_none() || since.elapsed() >= TITLE_SETTLE {
                    let text = title_text(&template, &dj, &track.0, &track.1);
                    *title.text.lock().unwrap() = (track.0.clone(), track.1.clone(), text);
                    title.version.fetch_add(1, Ordering::AcqRel);
                    sent = Some(track);
                    candidate = None;
                } else {
                    candidate = Some((track, since));
                }
            }
            _ => candidate = None,
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn count_listeners(
    stations: Vec<Station>,
    statuses: Arc<Mutex<Vec<StationStatus>>>,
    stop: Arc<AtomicBool>,
) {
    let mut last = Instant::now() - LISTENERS_EVERY + Duration::from_secs(3);
    while !stop.load(Ordering::Acquire) {
        if last.elapsed() >= LISTENERS_EVERY {
            last = Instant::now();
            for (i, st) in stations.iter().enumerate() {
                let connected = statuses
                    .lock()
                    .unwrap()
                    .get(i)
                    .is_some_and(|s| s.state == State::Connected);
                let n = if connected {
                    server::listeners(st)
                } else {
                    None
                };
                if let Some(s) = statuses.lock().unwrap().get_mut(i) {
                    s.listeners = n;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

struct StationJob {
    index: usize,
    target: Target,
    rate: u32,
    rx: Receiver<Chunk>,
    behind: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    title: Arc<Title>,
    statuses: Arc<Mutex<Vec<StationStatus>>>,
}

impl StationJob {
    fn set(&self, f: impl FnOnce(&mut StationStatus)) {
        if let Some(s) = self.statuses.lock().unwrap().get_mut(self.index) {
            f(s);
        }
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    /// Waits `d`, or less if going off air.
    fn wait(&self, d: Duration) {
        let until = Instant::now() + d;
        while Instant::now() < until && !self.stopping() {
            // Keep the queue from filling while not connected.
            while self.rx.try_recv().is_ok() {}
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn run(self) {
        let station = &self.target.station;
        let format = station.effective_format();
        let mut trying_since: Option<Instant> = None;
        while !self.stopping() {
            self.set(|s| s.state = State::Connecting);
            let since = *trying_since.get_or_insert_with(Instant::now);
            let mut encoder = match encode::encoder(format, station.bitrate, self.rate) {
                Ok(e) => e,
                Err(e) => {
                    self.set(|s| {
                        s.state = State::Failed;
                        s.message = format!("The encoder couldn't start: {e}");
                    });
                    return;
                }
            };
            let mut conn =
                match server::connect(station, &self.target.password, encoder.content_type()) {
                    Ok(c) => c,
                    Err(ConnectError::Fatal(m)) => {
                        self.set(|s| {
                            s.state = State::Failed;
                            s.message = m;
                        });
                        return;
                    }
                    Err(ConnectError::Retry(m)) => {
                        if since.elapsed() >= GIVE_UP_AFTER {
                            self.set(|s| {
                                s.state = State::Failed;
                                s.message = format!("{m}. Gave up after ten minutes.");
                            });
                            return;
                        }
                        self.set(|s| s.message = m);
                        self.wait(RETRY_EVERY);
                        continue;
                    }
                };
            // Connected: start from now, not from what queued up meanwhile.
            while self.rx.try_recv().is_ok() {}
            self.behind.store(false, Ordering::Release);
            self.set(|s| {
                s.state = State::Connected;
                s.message.clear();
                s.behind = false;
            });
            let trouble = self.stream(&mut *encoder, &mut *conn);
            if self.stopping() {
                break;
            }
            // Dropped: try again at once, then every few seconds.
            self.set(|s| {
                s.state = State::Connecting;
                s.behind = trouble.1;
                s.message = trouble.0;
                s.listeners = None;
            });
            trying_since = Some(Instant::now());
        }
    }

    /// Sends until the connection drops, the upload falls behind or the
    /// broadcast stops; returns why it ended and whether it fell behind.
    fn stream(
        &self,
        encoder: &mut dyn encode::Encoder,
        conn: &mut dyn server::Stream,
    ) -> (String, bool) {
        let station = &self.target.station;
        let format = station.effective_format();
        // A fresh connection sends the current title first.
        let mut title_seen = 0;
        let mut sent_audio = false;
        loop {
            if self.stopping() {
                let _ = conn.flush();
                return (String::new(), false);
            }
            if self.behind.swap(false, Ordering::AcqRel) {
                self.set(|s| s.behind = true);
                return ("The upload fell behind; reconnecting".into(), true);
            }
            let version = self.title.version.load(Ordering::Acquire);
            if version != title_seen {
                title_seen = version;
                let (artist, title, text) = self.title.text.lock().unwrap().clone();
                if format == Format::Opus {
                    // Opus carries titles in the stream. Before any audio
                    // that only fills in the headers; later it chains a new
                    // stream, which not every player follows, so only when
                    // chosen.
                    if !sent_audio || station.opus_titles {
                        encoder.set_title(&artist, &title);
                    }
                } else {
                    // MP3: the server takes it on the side.
                    let (st, pw) = (station.clone(), self.target.password.clone());
                    std::thread::spawn(move || {
                        if let Err(e) = server::update_title(&st, &pw, &text) {
                            eprintln!("could not update the title on {}: {e}", st.name);
                        }
                    });
                }
            }
            match self.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(chunk) => {
                    let bytes = match encoder.encode(&chunk) {
                        Ok(b) => b,
                        Err(e) => return (format!("The encoder failed: {e}"), false),
                    };
                    if !bytes.is_empty() {
                        sent_audio = true;
                        if let Err(e) = conn.write_all(&bytes).and_then(|_| conn.flush()) {
                            return (format!("The connection dropped: {e}"), false);
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return (String::new(), false),
            }
        }
    }
}

#[cfg(test)]
mod tests;
