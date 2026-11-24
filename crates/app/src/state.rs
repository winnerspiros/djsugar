use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::bridge::Bridge as Control;
use sugar_engine::{Command, DECKS, Telemetry, Track};
use sugar_layout::Rect;

use crate::library::{self, Entry, Source};
use sugar_engine::station::{Release, Scene, Set};

include!(concat!(env!("OUT_DIR"), "/samples.rs"));

pub type Reader = fn(&str) -> Option<Vec<u8>>;

const OVERVIEW: usize = 1024;
pub const DETAIL_FRAMES: usize = 256;
/// The view switch fade, as long as the reference's reflow transition.
pub const VIEW_FADE: std::time::Duration = std::time::Duration::from_millis(150);
const TAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Play(usize),
    Cue(usize),
    KeyLock(usize),
    Rate(usize),
    Volume(usize),
    Knob(usize, usize),
    Crossfader,
    Pad(usize, usize),
    Mode(usize, Mode),
    Sync(usize),
    AutoDj,
    Jog(usize),
    Overview(usize),
    Library,
    Browse(usize),
    CloseBrowser,
    View(View),
    ToggleFx,
    ToggleSamplers,
    Sampler(usize),
    Reverse(usize),
    Slip(usize),
    Eject(usize),
    Pfl(usize),
    Quantize(usize),
    Pitch(usize, i8),
    Jump(usize, i8),
    LoopIn(usize),
    LoopOut(usize),
    LoopSize(usize, i8),
    LoopToggle(usize),
    CueColour(u8),
    CueClear,
    CueMenuClose,
    Grid(usize),
    Sort(library::Column),
    XfadeStep(i8),
    Shelf(Shelf),
    NewPlaylist,
    MenuDeck(usize),
    MenuToggle(usize),
    /// Loads the menu's track on a deck and plays it.
    MenuPlay(usize),
    /// Measures the menu's track again.
    MenuAnalyze,
    /// Takes the menu's track out of the open playlist.
    MenuRemove,
    /// A playlist's menu action.
    ListOp(usize, ListOp),
    MenuClose,
    Picker,
    /// Opens a submenu of the open menu, by its number in that menu.
    Submenu(u8),
    /// A menu's own area, so a press there neither closes it nor reaches what is under it.
    MenuBody,
    /// The app menu button.
    AppMenu,
    /// Opens settings at a page.
    Prefs(u8),
    PrefsClose,
    HubService(crate::hub::Service),
    Quit,
    Find,
    LoadPick(usize),
    /// The track menu's stars, 1 to 5; the lit one again clears them.
    MenuRate(u8),
    /// The track menu's colours, 0 for none.
    MenuColour(u8),
    /// Shows the menu's track's artist's tracks, or its folder's.
    MenuArtist,
    MenuFolder,
    /// The settings page's body, dragged to scroll.
    PrefsScroll,
    /// A settings switch, or a choice's menu.
    Pref(crate::settings::Opt),
    /// A choice picked for a setting.
    PrefPick(crate::settings::Opt, u8),
    Rescan,
    PickerClose,
    Scene(usize),
    Xy,
    StationTime,
    Iso(usize),
    Release(usize),
    Record,
    SearchFocus,
    HubFocus,
    HubSearch,
    HubList,
    HubLoad(usize, usize),
    LoadSelected(usize),
    GridOp(GridOp),
    GridClose,
    /// Zooms the large waveforms in (1) or out (-1); 0 resets.
    WaveZoom(i8),
    /// Makes the large waveforms taller (1) or shorter (-1); 0 resets.
    WaveHeight(i8),
    /// A streaming service in the library's rail: its page in the hub.
    StreamFrom(crate::hub::Service),
    /// Turns a library filter chip on or off, by its place in `FILTERS`.
    Filter(u8),
    Gain(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridOp {
    Half,
    Here,
    Earlier,
    Later,
    Halve,
    Double,
    Match,
    Undo,
    Lock,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    Ctl,
    #[default]
    Dj,
    Lib,
    Wave,
    Fx,
    Stream,
}

/// Layout of the sample picker grid: columns and cell size.
#[derive(Clone, Copy, Debug, Default)]
pub struct Picker {
    pub cols: usize,
    pub cell: (f32, f32),
}

/// What the library lists, picked in its sidebar.
/// What a playlist's menu does to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListOp {
    Load(usize),
    Up,
    Down,
    Clear,
    Delete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shelf {
    #[default]
    Tracks,
    History,
    Playlist(usize),
    /// The tracks in one folder, by its place in `State::folders`.
    Folder(usize),
}

/// The library's filter chips, in order, each a bit of `State::filters`.
pub const FILTERS: [&str; 4] = ["Near BPM", "Mixes in key", "Rated", "Colored"];
/// How far a tempo may be from the lead deck's for the Near BPM chip.
const NEAR_BPM: f32 = 0.06;

pub fn entry_key(e: &Entry) -> String {
    match &e.source {
        Source::File(p) => p.to_string_lossy().into_owned(),
        Source::Sample(n) => format!("sample:{n}"),
    }
}

/// FX station settings shown on its page.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub scene: Scene,
    pub on: bool,
    pub xy: (f32, f32),
    pub time: f32,
    pub iso: [f32; 3],
    pub release: Release,
    pub bpm: f64,
}

impl Default for Station {
    fn default() -> Self {
        Self {
            scene: Scene::Echo,
            on: false,
            xy: (0.5, 0.5),
            time: 0.5,
            iso: [0.0; 3],
            release: Release::Brake,
            bpm: 0.0,
        }
    }
}

fn recordings_dir() -> Option<std::path::PathBuf> {
    let music = std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join("Music"));
    match music {
        Some(m) if m.is_dir() => Some(m.join("DJ Sugar Recordings")),
        _ => crate::store::dir().map(|d| d.join("recordings")),
    }
}

pub const LONG_PRESS_MS: u128 = 450;
/// How long a message stays on screen.
pub const TOAST: std::time::Duration = std::time::Duration::from_secs(6);

pub const JUMP_BEATS: [f64; 10] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0];

fn short(name: &str) -> &str {
    name.split_once(' ').map_or(name, |(n, rest)| if n.chars().all(|c| c.is_ascii_digit()) { rest } else { name })
}

pub fn sample_name(s: &State, i: usize) -> String {
    s.entries.get(i).map_or_else(String::new, |e| short(&e.title).to_string())
}

pub const BANKS: usize = 4;

pub fn pad_name(s: &State, d: usize, i: usize) -> String {
    sample_name(s, s.pads[d % 2][s.bank[d % 2] * 8 + i])
}

pub fn beats_label(b: f64) -> String {
    match b {
        0.25 => "1/4".into(),
        0.5 => "1/2".into(),
        _ => format!("{b}"),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Cues,
    Loops,
    Samples,
    Fx,
}

pub const ECHO_BEATS: [f64; 5] = [0.25, 0.5, 0.75, 1.0, 2.0];
pub const FX_PADS: [&str; 8] = ["1/4", "1/2", "3/4", "1", "2", "OUT", "BRK", "REV"];

pub const LOOP_BEATS: [f64; 8] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];

#[derive(Debug)]
pub struct Hit {
    pub rect: Rect,
    pub target: Target,
}

#[derive(Debug)]
pub struct Loaded {
    pub entry: Entry,
    pub track: Arc<Track>,
    pub overview: Box<[u8]>,
    pub detail: Box<[[u8; 4]]>,
    pub grid: Option<sugar_analysis::Grid>,
    pub key: Option<sugar_analysis::Key>,
    pub lyrics: crate::lyrics::Lyrics,
}

#[derive(Debug, Default)]
pub struct DeckUi {
    pub loaded: Option<Loaded>,
    pub loading: bool,
    pub rate: f32,
    pub volume: f32,
    pub knobs: [f32; 4],
    /// High, mid and low kills.
    pub kills: [bool; 3],
    pub key_lock: bool,
    pub cue: f64,
    pub mode: Mode,
    pub cues: [Option<f64>; 8],
    pub cue_colours: [u8; 8],
    pub grid_undo: Vec<sugar_analysis::Grid>,
    pub grid_locked: bool,
    pub looping: Option<usize>,
    pub loop_range: Option<(f64, f64)>,
    pub echo: Option<usize>,
    pub gain: f32,
    pub jump: usize,
    pub loop_size: usize,
    pub loop_in: Option<f64>,
    pub quantize: bool,
    pub reverse: bool,
    pub slip: bool,
    pub pfl: bool,
    /// The CUE button is held from the cue, in Hold mode.
    pub previewing: bool,
    /// Auto gain's level change for the loaded track, as a factor.
    pub auto_gain: f32,
    pub semitones: i8,
}

impl DeckUi {
    fn new() -> Self {
        Self {
            volume: 0.85,
            knobs: [0.5; 4],
            gain: 0.5,
            auto_gain: 1.0,
            jump: 4,
            loop_size: 4,
            quantize: true,
            ..Self::default()
        }
    }

    pub fn seconds(&self, frames: f64) -> f64 {
        self.loaded.as_ref().map_or(0.0, |l| frames / f64::from(l.track.rate))
    }

    pub fn length(&self) -> f64 {
        self.loaded.as_ref().map_or(0.0, |l| l.track.frames.len() as f64)
    }

    pub fn beat(&self) -> Option<(sugar_analysis::Grid, f64)> {
        let l = self.loaded.as_ref()?;
        let g = l.grid?;
        Some((g, g.beat_frames(l.track.rate)))
    }

    pub fn snap(&self, pos: f64) -> f64 {
        match self.beat() {
            Some((g, b)) if self.quantize => (g.first_beat + ((pos - g.first_beat) / b).round() * b).max(0.0),
            _ => pos,
        }
    }
}

/// A key, by what it does rather than where it sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Delete,
    Escape,
    Enter,
    Tab,
    Space,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    /// F1 to F12.
    F(u8),
    /// A typed character, after the layout and Shift.
    Char(char),
}

/// The modifier keys held with a key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// A shifted symbol back to its key on a US layout, as the keyboard map names keys.
pub fn unshift(c: char) -> char {
    match c {
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        ':' => ';',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        '~' => '`',
        c => c.to_ascii_lowercase(),
    }
}

/// Permanent tempo steps, normal and small, as fractions of the speed.
const RATE_STEP: [f64; 2] = [0.005, 0.0005];
/// Held tempo changes, normal and small.
const NUDGE: [f64; 2] = [0.04, 0.02];

/// Default keyboard map, same keys as the DJ Sugar desktop mapping (en_US).
pub fn keymap(c: char) -> Option<Target> {
    const DECK: [[char; 15]; 2] = [
        ['d', 'f', 'a', 's', '1', 't', 'z', 'x', 'c', 'v', '2', '3', '4', 'w', 'e'],
        ['l', ';', 'j', 'k', '6', 'y', 'm', ',', '.', '/', '7', '8', '9', 'i', 'o'],
    ];
    let c = c.to_ascii_lowercase();
    match c {
        'q' => return Some(Target::LoopToggle(0)),
        'u' => return Some(Target::LoopToggle(1)),
        'g' => return Some(Target::XfadeStep(-1)),
        'h' => return Some(Target::XfadeStep(1)),
        _ => {}
    }
    let (d, n) = DECK.iter().enumerate().find_map(|(d, row)| row.iter().position(|&k| k == c).map(|n| (d, n)))?;
    Some(match n {
        0 => Target::Play(d),
        1 => Target::Cue(d),
        2 => Target::Jump(d, -1),
        3 => Target::Jump(d, 1),
        4 => Target::Sync(d),
        5 => Target::Pfl(d),
        6..=9 => Target::Pad(d, n - 6),
        10 => Target::LoopIn(d),
        11 => Target::LoopOut(d),
        12 => Target::LoopToggle(d),
        13 => Target::LoopSize(d, -1),
        _ => Target::LoopSize(d, 1),
    })
}

type Done = (Job, Option<Loaded>);

enum Job {
    Deck(usize),
    Sample(usize, bool),
    Scan(Vec<Entry>),
    Meta(u32, u32, library::Meta),
    /// A track that would not load, and why.
    Failed(String),
}

struct Drag {
    id: u64,
    target: Option<Target>,
    start: (f32, f32),
    last: (f32, f32),
    moved: bool,
    resume: bool,
    at: std::time::Instant,
}

pub struct State {
    control: Option<Control>,
    pub telemetry: Telemetry,
    pub decks: [DeckUi; DECKS],
    pub crossfader: f32,
    pub samples: Vec<Option<Arc<Track>>>,
    pub pads: [[usize; 8 * BANKS]; 2],
    pub bank: [usize; 2],
    /// Sample picker open for (deck, pad), its scroll and grid geometry.
    pub picker: Option<(usize, usize)>,
    pub picker_scroll: f32,
    pub picker_grid: Picker,
    pub entries: Vec<Entry>,
    pub shown: Vec<u32>,
    pub query: String,
    pub focus: usize,
    pub browser: Option<usize>,
    pub scroll: f32,
    pub row_h: f32,
    pub library: Rect,
    pub hits: Vec<Hit>,
    drags: Vec<Drag>,
    read: Reader,
    jobs: (Sender<Done>, Receiver<Done>),
    pub status: Option<String>,
    /// The message on screen, and when it came.
    pub toast: Option<(String, std::time::Instant)>,
    /// The app's and the streaming page's last messages, to show each new one once.
    last_msgs: [String; 2],
    pub out_rate: u32,
    pub auto: Option<Auto>,
    pub view: View,
    /// When the view last changed, for its fade in.
    pub view_at: Option<std::time::Instant>,
    pub show_fx: bool,
    pub show_samplers: bool,
    /// The large waveforms' zoom, each step twice as close; 0 is the default.
    pub wave_zoom: i8,
    /// The large waveforms' height, each step a fifth taller; 0 is the default.
    pub wave_height: i8,
    /// The library's filter chips that are on, one bit per `FILTERS` entry.
    pub filters: u8,
    pub search_focus: bool,
    pub hub: crate::hub::Hub,
    pub hub_area: Rect,
    pub shelf: Shelf,
    pub playlists: Vec<(String, Vec<String>)>,
    pub history: Vec<u32>,
    pub track_menu: Option<u32>,
    pub settings: crate::settings::Settings,
    /// The open settings page.
    pub prefs: Option<usize>,
    /// The setting whose choices are open, and where.
    pub pref_menu: Option<(crate::settings::Opt, f32, f32)>,
    pub app_menu: bool,
    /// How far the settings page is scrolled, and how far it can go.
    pub prefs_scroll: f32,
    pub prefs_max: f32,
    /// Ctrl+Q asked the app to close.
    pub quit: bool,
    /// The held playlist whose menu is open.
    pub list_menu: Option<usize>,
    /// The deck to start once its track has loaded.
    play_on_load: Option<usize>,
    /// Where the track menu was opened.
    pub menu_at: (f32, f32),
    /// The open menu's open submenu.
    pub submenu: Option<u8>,
    pub station: Station,
    pub cue_menu: Option<(usize, usize, f32, f32)>,
    pub grid_menu: Option<usize>,
    pub meta: Vec<Option<library::Meta>>,
    pub sort: (library::Column, bool),
    pub selected: Option<u32>,
    analysis: Arc<std::sync::atomic::AtomicU32>,
    pub store: crate::store::Store,
    pub recorder: Option<crate::record::Recorder>,
}

#[derive(Debug, Clone, Copy)]
pub struct Auto {
    pub next: usize,
    pub fade: Option<(usize, f64, f64)>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State").field("focus", &self.focus).finish()
    }
}

fn bands(t: &Track, per: usize) -> Box<[[u8; 4]]> {
    let k = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz / t.rate as f32).exp();
    let (kl, kh) = (k(220.0), k(2500.0));
    let (mut lo, mut lohi) = (0.0f32, 0.0f32);
    t.frames
        .chunks(per)
        .map(|c| {
            let mut m = [0.0f32; 4];
            for s in c {
                let x = (s[0] + s[1]) * 0.5;
                lo += kl * (x - lo);
                lohi += kh * (x - lohi);
                let (l, mid, h) = (lo, lohi - lo, x - lohi);
                m[0] = m[0].max(x.abs());
                m[1] = m[1].max(l.abs());
                m[2] = m[2].max(mid.abs());
                m[3] = m[3].max(h.abs());
            }
            m.map(|v| (v.min(1.0) * 255.0) as u8)
        })
        .collect()
}

fn peaks(t: &Track, bins: usize, per: usize) -> Box<[u8]> {
    let f = &t.frames;
    let per = if per == 0 { f.len().div_ceil(bins).max(1) } else { per };
    f.chunks(per)
        .map(|c| {
            let m = c.iter().fold(0.0f32, |m, s| m.max(s[0].abs()).max(s[1].abs()));
            (m.min(1.0) * 255.0) as u8
        })
        .collect()
}

impl State {
    pub fn new(control: Option<Control>, read: Reader, status: Option<String>, out_rate: u32) -> Self {
        let mut pads = [[0; 8 * BANKS]; 2];
        for (i, p) in pads.iter_mut().flatten().enumerate() {
            *p = i.min(SAMPLES.len().saturating_sub(1));
        }
        let mut s = Self {
            control,
            telemetry: Telemetry::default(),
            decks: std::array::from_fn(|_| DeckUi::new()),
            crossfader: 0.0,
            samples: vec![None; SAMPLES.len()],
            pads: crate::store::load_pads().unwrap_or(pads),
            bank: [0; 2],
            picker: None,
            picker_scroll: 0.0,
            picker_grid: Picker::default(),
            entries: library::samples(),
            shown: Vec::new(),
            query: String::new(),
            focus: 0,
            browser: None,
            scroll: 0.0,
            row_h: 22.0,
            library: Rect::new(0.0, 0.0, 0.0, 0.0),
            hits: Vec::with_capacity(128),
            drags: Vec::new(),
            read,
            jobs: channel(),
            status,
            toast: None,
            last_msgs: Default::default(),
            out_rate,
            auto: None,
            view: View::Dj,
            view_at: None,
            show_fx: false,
            show_samplers: false,
            wave_zoom: 0,
            wave_height: 0,
            filters: 0,
            search_focus: false,
            hub_area: Rect::default(),
            hub: crate::hub::Hub::new(crate::store::dir().unwrap_or_else(std::env::temp_dir).join("youtube_cache")),
            shelf: Shelf::Tracks,
            playlists: crate::store::load_playlists(),
            history: Vec::new(),
            track_menu: None,
            settings: crate::settings::Settings::load(),
            prefs: None,
            pref_menu: None,
            app_menu: false,
            prefs_scroll: 0.0,
            prefs_max: 0.0,
            quit: false,
            list_menu: None,
            play_on_load: None,
            menu_at: (0.0, 0.0),
            submenu: None,
            station: Station::default(),
            cue_menu: None,
            grid_menu: None,
            meta: Vec::new(),
            sort: (library::Column::Index, true),
            selected: None,
            analysis: Arc::default(),
            store: crate::store::Store::open(),
            recorder: None,
        };
        s.send_curve();
        s
    }

    /// A file the engine can open: library files as they are, built-in
    /// samples written once to the cache.
    fn on_disk(&self, source: &Source) -> Option<std::path::PathBuf> {
        match source {
            Source::File(p) => Some(p.to_path_buf()),
            Source::Sample(name) => {
                let path = std::env::temp_dir().join("djsugar-samples").join(name);
                if !path.is_file() {
                    std::fs::create_dir_all(path.parent()?).ok()?;
                    std::fs::write(&path, (self.read)(name)?).ok()?;
                }
                Some(path)
            }
        }
    }

    fn play_sample(&mut self, i: usize) {
        let Some(name) = SAMPLES.get(i) else { return };
        let Some(path) = self.on_disk(&Source::Sample(name)) else { return };
        if let Some(c) = &mut self.control {
            c.sample(name, path, 0.9);
        }
    }

    fn send(&mut self, c: Command) {
        if let Some(ctl) = &mut self.control {
            ctl.send(c);
        }
    }

    pub fn animating(&self) -> bool {
        self.recorder.is_some()
            || self.telemetry.decks.iter().any(|d| d.playing)
            || !self.drags.is_empty()
            || self.decks.iter().any(|d| d.loading)
            || self.auto.is_some()
            || self.view_at.is_some_and(|t| t.elapsed() < VIEW_FADE)
            || self.toast.as_ref().is_some_and(|t| t.1.elapsed() < TOAST)
    }

    pub fn tick(&mut self) {
        let now = [self.status.clone().unwrap_or_default(), self.hub.status.clone()];
        for (msg, last) in now.into_iter().zip(&mut self.last_msgs) {
            if msg != *last {
                if !msg.is_empty() {
                    crate::diag(&msg);
                    self.toast = Some((msg.clone(), std::time::Instant::now()));
                }
                *last = msg;
            }
        }
        for (path, title, artist, deck) in self.hub.poll() {
            let at =
                self.entries.iter().position(|e| matches!(&e.source, Source::File(p) if **p == *path)).unwrap_or_else(
                    || {
                        self.entries.push(library::file(path, &title, &artist, "YouTube"));
                        self.entries.len() - 1
                    },
                );
            self.load_deck(deck, at);
        }
        let bpm = self.master_bpm();
        if (bpm - self.station.bpm).abs() > 0.01 {
            self.station.bpm = bpm;
            self.send(Command::Station(Set::Bpm(bpm)));
        }
        self.holds();
        while let Ok((job, loaded)) = self.jobs.1.try_recv() {
            match (job, loaded) {
                (Job::Deck(d), Some(l)) => {
                    if let Some(path) = self.on_disk(&l.entry.source)
                        && let Some(c) = &mut self.control
                    {
                        c.load(d, &path, l.track.rate);
                    }
                    self.decks[d].loaded = Some(l);
                    self.decks[d].loading = false;
                    self.decks[d].cue = 0.0;
                    self.decks[d].cues = [None; 8];
                    self.decks[d].grid_undo.clear();
                    self.decks[d].grid_locked = false;
                    self.decks[d].cue_colours = [0; 8];
                    self.restore(d);
                    self.decks[d].looping = None;
                    self.decks[d].loop_range = None;
                    self.on_load(d);
                    if self.play_on_load == Some(d) {
                        self.play_on_load = None;
                        self.send(Command::Play(d, true));
                    }
                }
                (Job::Deck(d), None) => {
                    self.decks[d].loading = false;
                    self.play_on_load = None;
                }
                (Job::Sample(i, play), Some(l)) => {
                    if play {
                        self.play_sample(i);
                    }
                    self.samples[i] = Some(l.track);
                }
                (Job::Sample(..), None) => {}
                (Job::Failed(why), _) => self.status = Some(why),
                (Job::Scan(found), _) => {
                    self.entries.truncate(SAMPLES.len());
                    self.entries.extend(found);
                    self.selected = None;
                    let q = std::mem::take(&mut self.query);
                    self.search(q);
                    self.analyze();
                }
                (Job::Meta(generation, i, m), _) => {
                    use std::sync::atomic::Ordering::Relaxed;
                    if generation == self.analysis.load(Relaxed)
                        && let Some(slot) = self.meta.get_mut(i as usize)
                    {
                        *slot = Some(m);
                        if !matches!(
                            self.sort.0,
                            library::Column::Index | library::Column::Title | library::Column::Artist
                        ) {
                            self.resort();
                        }
                    }
                }
            }
        }
        if let Some(c) = &mut self.control {
            if let Some(rec) = &mut self.recorder {
                let mut block = Vec::new();
                if c.drain_recording(&mut block) > 0 {
                    rec.push(block);
                }
            }
            self.telemetry = c.poll();
        }
        self.auto_step();
    }

    fn toggle_auto(&mut self) {
        if self.auto.take().is_some() {
            return;
        }
        let mut next = 0;
        if self.decks[0].loaded.is_none()
            && !self.decks[0].loading
            && let Some(&i) = self.shown.first()
        {
            self.load_deck(0, i as usize);
            next = 1;
        }
        self.auto = Some(Auto { next, fade: None });
        if let Some(&i) = self.shown.get(next)
            && self.decks[1].loaded.is_none()
        {
            self.load_deck(1, i as usize);
        }
        if let Some(a) = &mut self.auto {
            a.next = next + 1;
        }
    }

    fn auto_step(&mut self) {
        let Some(mut a) = self.auto else { return };
        let live = (0..2).find(|&d| self.telemetry.decks[d].playing);
        let Some(live) = live else {
            a.fade = None;
            let fresh = |d: usize| {
                self.decks[d]
                    .loaded
                    .as_ref()
                    .is_some_and(|l| self.telemetry.decks[d].position + 1.0 < l.track.frames.len() as f64)
            };
            if let Some(d) = (0..2).find(|&d| fresh(d)) {
                self.send(Command::Play(d, true));
                self.telemetry.decks[d].playing = true;
                self.crossfade(if d == 0 { -1.0 } else { 1.0 });
            } else if let Some(d) = (0..2).find(|&d| self.decks[d].loaded.is_some() && !self.decks[d].loading)
                && let Some(&i) = self.shown.get(a.next % self.shown.len().max(1))
            {
                self.load_deck(d, i as usize);
                a.next += 1;
            }
            self.auto = Some(a);
            return;
        };
        let other = 1 - live;
        if let Some((from, start, len)) = a.fade {
            let pos = self.telemetry.decks[1 - from].position;
            let t = ((pos - start) / len).clamp(0.0, 1.0) as f32;
            let dir = if from == 0 { 1.0 } else { -1.0 };
            self.crossfade(-dir + 2.0 * dir * t);
            if t >= 1.0 {
                self.send(Command::Play(from, false));
                self.telemetry.decks[from].playing = false;
                a.fade = None;
                if let Some(&i) = self.shown.get(a.next % self.shown.len().max(1)) {
                    self.load_deck(from, i as usize);
                    a.next += 1;
                }
            }
            self.auto = Some(a);
            return;
        }
        let (Some(l), Some(o)) = (&self.decks[live].loaded, &self.decks[other].loaded) else { return };
        let len = l.track.frames.len() as f64;
        let rate = f64::from(l.track.rate);
        let beat = l.grid.map_or(0.5 * rate, |g| g.beat_frames(l.track.rate));
        let mix = (beat * 16.0).min(len * 0.4);
        if len - self.telemetry.decks[live].position > mix {
            return;
        }
        let other_mix = (mix * f64::from(o.track.rate) / rate).min(o.track.frames.len() as f64 * 0.5);
        self.sync(other);
        self.send(Command::Play(other, true));
        self.telemetry.decks[other].playing = true;
        let start = self.telemetry.decks[other].position;
        a.fade = Some((live, start, other_mix));
        self.auto = Some(a);
    }

    fn crossfade(&mut self, v: f32) {
        self.crossfader = v.clamp(-1.0, 1.0);
        self.send(Command::Crossfader(self.crossfader));
    }

    fn load(&mut self, job: Job, index: usize) {
        let Some(entry) = self.entries.get(index).cloned() else { return };
        let (tx, read) = (self.jobs.0.clone(), self.read);
        let detail = matches!(job, Job::Deck(_));
        std::thread::spawn(move || {
            let bytes = match &entry.source {
                Source::Sample(name) => read(name),
                Source::File(path) => std::fs::read(path).ok(),
            };
            let name = entry.title.to_string();
            let decoded = match bytes {
                Some(b) => sugar_decode::bytes(b, entry.extension()).map_err(|e| e.to_string()),
                None => Err("the file could not be read".to_string()),
            };
            if let Err(why) = &decoded {
                let _ = tx.send((Job::Failed(format!("Could not load \"{name}\": {why}")), None));
            }
            let loaded = decoded.ok().map(|t| {
                let overview = if detail { peaks(&t, OVERVIEW, 0) } else { Box::default() };
                let detail = if detail { bands(&t, DETAIL_FRAMES) } else { Box::default() };
                let grid = if detail.is_empty() {
                    None
                } else {
                    sugar_analysis::grid(t.rate, &t.frames).map(crate::settings::fold)
                };
                let key = if detail.is_empty() { None } else { sugar_analysis::key(t.rate, &t.frames) };
                let lyrics = match &entry.source {
                    Source::File(p) => std::fs::read_to_string(p.with_extension("lrc"))
                        .map(|t| crate::lyrics::Lyrics::parse(&t))
                        .unwrap_or_default(),
                    Source::Sample(_) => crate::lyrics::Lyrics::default(),
                };
                Loaded { entry, track: Arc::new(t), overview, detail, grid, key, lyrics }
            });
            let _ = tx.send((job, loaded));
        });
    }

    pub fn scan(&mut self) {
        let roots = library::roots();
        if roots.is_empty() {
            return;
        }
        let tx = self.jobs.0.clone();
        std::thread::spawn(move || {
            let _ = tx.send((Job::Scan(library::scan(&roots)), None));
        });
    }

    fn track_key(d: &DeckUi) -> Option<String> {
        match &d.loaded.as_ref()?.entry.source {
            Source::File(p) => Some(p.to_string_lossy().into_owned()),
            Source::Sample(n) => Some(format!("sample:{n}")),
        }
    }

    fn restore(&mut self, d: usize) {
        let Some(key) = Self::track_key(&self.decks[d]) else { return };
        let Some(saved) = self.store.get(&key) else { return };
        let deck = &mut self.decks[d];
        deck.cues = saved.cues;
        deck.cue_colours = saved.colours;
        deck.grid_locked = saved.locked;
        if let (Some((bpm, first_beat)), Some(l)) = (saved.grid, &mut deck.loaded) {
            l.grid = Some(sugar_analysis::Grid { bpm, first_beat });
        }
    }

    fn remember(&mut self) {
        for d in 0..2 {
            let deck = &self.decks[d];
            let Some(key) = Self::track_key(deck) else { continue };
            let grid = deck.loaded.as_ref().and_then(|l| l.grid).map(|g| (g.bpm, g.first_beat));
            let old = self.store.get(&key).unwrap_or_default();
            let saved = crate::store::Saved {
                grid,
                locked: deck.grid_locked,
                cues: deck.cues,
                colours: deck.cue_colours,
                ..old
            };
            self.store.put(&key, saved);
        }
    }

    pub fn analyze(&mut self) {
        use std::sync::atomic::Ordering::Relaxed;
        self.meta = vec![None; self.entries.len()];
        let generation = self.analysis.fetch_add(1, Relaxed) + 1;
        let (live, tx, read) = (self.analysis.clone(), self.jobs.0.clone(), self.read);
        let entries: Vec<(u32, Entry)> = self
            .entries
            .iter()
            .enumerate()
            .skip(SAMPLES.len())
            .chain(self.entries.iter().enumerate().take(SAMPLES.len()))
            .map(|(i, e)| (i as u32, e.clone()))
            .collect();
        std::thread::spawn(move || {
            for (i, entry) in entries {
                if live.load(Relaxed) != generation {
                    return;
                }
                let bytes = match &entry.source {
                    Source::Sample(name) => read(name),
                    Source::File(path) => std::fs::read(path).ok(),
                };
                let Some(t) = bytes.and_then(|b| sugar_decode::bytes(b, entry.extension()).ok()) else { continue };
                if tx.send((Job::Meta(generation, i, library::Meta::of(&t)), None)).is_err() {
                    return;
                }
            }
        });
    }

    /// Measures one track again, as the library's pass does.
    fn analyze_one(&mut self, i: u32) {
        use std::sync::atomic::Ordering::Relaxed;
        let Some(entry) = self.entries.get(i as usize).cloned() else { return };
        let (generation, tx, read) = (self.analysis.load(Relaxed), self.jobs.0.clone(), self.read);
        self.status = Some(format!("Analyzing \"{}\"", entry.title));
        std::thread::spawn(move || {
            let bytes = match &entry.source {
                Source::Sample(name) => read(name),
                Source::File(path) => std::fs::read(path).ok(),
            };
            if let Some(t) = bytes.and_then(|b| sugar_decode::bytes(b, entry.extension()).ok()) {
                let _ = tx.send((Job::Meta(generation, i, library::Meta::of(&t)), None));
            }
        });
    }

    fn list_op(&mut self, n: usize, op: ListOp) {
        if n >= self.playlists.len() {
            return;
        }
        match op {
            ListOp::Load(d) => {
                let first =
                    self.playlists[n].1.first().and_then(|k| self.entries.iter().position(|e| entry_key(e) == *k));
                match first {
                    Some(i) => self.load_deck(d, i),
                    None => self.status = Some("That playlist is empty".into()),
                }
                return;
            }
            ListOp::Up if n > 0 => self.playlists.swap(n, n - 1),
            ListOp::Down if n + 1 < self.playlists.len() => self.playlists.swap(n, n + 1),
            ListOp::Clear => self.playlists[n].1.clear(),
            ListOp::Delete => {
                self.playlists.remove(n);
                if let Shelf::Playlist(p) = self.shelf {
                    self.shelf = match p.cmp(&n) {
                        std::cmp::Ordering::Equal => Shelf::Tracks,
                        std::cmp::Ordering::Greater => Shelf::Playlist(p - 1),
                        std::cmp::Ordering::Less => self.shelf,
                    };
                }
            }
            _ => return,
        }
        if let (ListOp::Up | ListOp::Down, Shelf::Playlist(p)) = (op, self.shelf) {
            let to = if op == ListOp::Up { n - 1 } else { n + 1 };
            self.shelf = if p == n {
                Shelf::Playlist(to)
            } else if p == to {
                Shelf::Playlist(n)
            } else {
                self.shelf
            };
        }
        crate::store::save_playlists(&self.playlists);
        let q = std::mem::take(&mut self.query);
        self.search(q);
    }

    pub fn resort(&mut self) {
        use library::Column as C;
        let (col, asc) = self.sort;
        if col == C::Index {
            if !asc {
                self.shown.reverse();
            }
            return;
        }
        let (entries, meta, store) = (&self.entries, &self.meta, &self.store);
        let m = |i: u32| meta.get(i as usize).copied().flatten();
        let num = |i: u32| match col {
            C::Bpm => m(i).map_or(f32::MAX, |m| if m.bpm > 0.0 { m.bpm } else { f32::MAX }),
            C::Time => m(i).map_or(f32::MAX, |m| m.secs),
            C::Key => m(i).and_then(|m| m.key).map_or(f32::MAX, |k| k.camelot_order() as f32),
            // Best first when going down, as the arrow reads.
            C::Rating => -f32::from(store.get(&entry_key(&entries[i as usize])).map_or(0, |v| v.rating)),
            _ => 0.0,
        };
        self.shown.sort_by(|&a, &b| {
            let o = match col {
                C::Title => entries[a as usize].title.to_lowercase().cmp(&entries[b as usize].title.to_lowercase()),
                C::Artist => entries[a as usize].artist.to_lowercase().cmp(&entries[b as usize].artist.to_lowercase()),
                _ => num(a).total_cmp(&num(b)),
            };
            if asc { o } else { o.reverse() }
        });
    }

    pub fn search(&mut self, query: String) {
        self.query = query;
        self.scroll = 0.0;
        let q = &self.query;
        let (n, s) = (self.entries.len() as u32, SAMPLES.len() as u32);
        let order: Vec<u32> = match self.shelf {
            Shelf::Tracks => (s..n).collect(),
            Shelf::History => self.history.clone(),
            Shelf::Playlist(p) => {
                let keys = self.playlists.get(p).map(|(_, k)| k.as_slice()).unwrap_or_default();
                keys.iter()
                    .filter_map(|k| self.entries.iter().position(|e| entry_key(e) == *k))
                    .map(|i| i as u32)
                    .collect()
            }
            Shelf::Folder(f) => {
                let name = self.folders().get(f).map(|n| n.to_string()).unwrap_or_default();
                (s..n).filter(|&i| *self.entries[i as usize].folder == *name).collect()
            }
        };
        let lead = self.lead();
        let f = self.filters;
        let (meta, store, entries) = (&self.meta, &self.store, &self.entries);
        let keep = |i: u32| {
            let m = meta.get(i as usize).copied().flatten();
            let saved = || store.get(&entry_key(&entries[i as usize])).unwrap_or_default();
            (f & 1 == 0
                || lead.0.zip(m.map(|m| m.bpm)).is_some_and(|(b, t)| t > 0.0 && (t / b - 1.0).abs() <= NEAR_BPM))
                && (f & 2 == 0 || lead.1.zip(m.and_then(|m| m.key)).is_some_and(|(a, b)| a.mixes(b)))
                && (f & 4 == 0 || saved().rating > 0)
                && (f & 8 == 0 || saved().colour > 0)
        };
        self.shown = order.into_iter().filter(|&i| keep(i) && library::matches(&self.entries[i as usize], q)).collect();
        if !matches!(self.shelf, Shelf::History | Shelf::Playlist(_)) || self.sort.0 != library::Column::Index {
            self.resort();
        }
    }

    /// The tempo and key the library's chips match against: the playing deck's, else the first loaded one's.
    fn lead(&self) -> (Option<f32>, Option<sugar_analysis::Key>) {
        let playing =
            (0..self.decks.len()).find(|&d| self.telemetry.decks[d].playing && self.decks[d].loaded.is_some());
        let d = playing.or_else(|| self.decks.iter().position(|d| d.loaded.is_some()));
        let Some(l) = d.and_then(|d| self.decks[d].loaded.as_ref()) else { return (None, None) };
        (l.grid.map(|g| g.bpm as f32).filter(|&b| b > 0.0), l.key)
    }

    /// The library's folders, sorted, each once.
    pub fn folders(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.entries[SAMPLES.len().min(self.entries.len())..]
            .iter()
            .map(|e| &*e.folder)
            .filter(|f| !f.is_empty())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    pub fn typing(&self) -> bool {
        self.search_focus || self.browser.is_some() || self.hub_typing()
    }

    fn hub_typing(&self) -> bool {
        self.view == View::Stream && self.hub.focus
    }

    pub fn type_text(&mut self, text: &str) {
        if self.hub_typing() {
            self.hub.query.push_str(text);
            return;
        }
        let mut q = std::mem::take(&mut self.query);
        q.push_str(text);
        self.search(q);
    }

    pub fn key(&mut self, key: Key, mods: Mods) {
        self.key_press(key, mods);
        self.remember();
    }

    /// A key let go: ends a held temporary tempo change.
    pub fn key_up(&mut self, key: Key) {
        match key {
            Key::F(n @ (3 | 4 | 7 | 8)) => self.send(Command::Nudge(usize::from(n > 4), 0.0)),
            Key::Char(c) => {
                if let Some(Target::Cue(d)) = keymap(unshift(c)) {
                    self.end_preview(d);
                }
            }
            _ => {}
        }
    }

    /// Lets go of a CUE held in Hold mode: back to the cue, stopped.
    fn end_preview(&mut self, d: usize) {
        if std::mem::take(&mut self.decks[d].previewing) {
            self.send(Command::Play(d, false));
            self.send(Command::Seek(d, self.decks[d].cue));
        }
    }

    /// Closes the topmost menu or sheet; false when nothing was open.
    pub fn dismiss(&mut self) -> bool {
        if self.pref_menu.take().is_some() {
            return true;
        }
        if self.prefs.is_some() || self.app_menu {
            self.prefs = None;
            self.app_menu = false;
            self.submenu = None;
            return true;
        }
        let open = self.track_menu.is_some()
            || self.list_menu.is_some()
            || self.cue_menu.is_some()
            || self.grid_menu.is_some()
            || self.picker.is_some();
        self.track_menu = None;
        self.list_menu = None;
        self.cue_menu = None;
        self.grid_menu = None;
        self.picker = None;
        self.submenu = None;
        open
    }

    /// A right click: what holding there would open.
    pub fn context(&mut self, x: f32, y: f32) {
        const CONTEXT: u64 = u64::MAX - 1;
        let target = self.hit(x, y);
        let now = std::time::Instant::now();
        let at = now.checked_sub(std::time::Duration::from_millis(LONG_PRESS_MS as u64)).unwrap_or(now);
        self.drags.push(Drag { id: CONTEXT, target, start: (x, y), last: (x, y), moved: false, resume: false, at });
        self.holds();
        self.drags.retain(|d| d.id != CONTEXT);
    }

    fn key_press(&mut self, key: Key, m: Mods) {
        if key == Key::Escape && self.dismiss() {
            return;
        }
        if m.ctrl {
            self.shortcut(key, m);
            return;
        }
        if self.hub_typing() {
            match key {
                Key::Backspace => {
                    self.hub.query.pop();
                }
                Key::Escape => self.hub.focus = false,
                Key::Enter => {
                    self.hub.focus = false;
                    self.hub.search();
                }
                _ => {}
            }
            return;
        }
        match key {
            Key::Backspace => {
                let mut q = std::mem::take(&mut self.query);
                q.pop();
                self.search(q);
            }
            Key::Escape => {
                self.search_focus = false;
                self.browser = None;
                self.search(String::new());
            }
            Key::Enter => {
                let deck = self.browser.take().unwrap_or(self.focus);
                self.load_pick(deck);
            }
            Key::Up => self.select_by(-1),
            Key::Down => self.select_by(1),
            Key::PageUp => self.select_by(-10),
            Key::PageDown => self.select_by(10),
            Key::Home => self.select_by(isize::MIN / 2),
            Key::End => self.select_by(isize::MAX / 2),
            Key::Left | Key::Right if m.shift => {
                let d = usize::from(key == Key::Right);
                if m.alt {
                    self.press(Target::Eject(d), 0.0, 0.0);
                } else {
                    self.load_pick(d);
                }
            }
            Key::Tab => self.next_view(if m.shift { -1 } else { 1 }),
            Key::Space => {
                let v = if self.view == View::Lib { View::Dj } else { View::Lib };
                self.press(Target::View(v), 0.0, 0.0);
            }
            Key::F(n) => self.function_key(n, m),
            Key::Char(c) => self.deck_key(c, m),
            Key::Delete | Key::Left | Key::Right => {}
        }
    }

    /// Ctrl shortcuts, as the desktop menus name them.
    fn shortcut(&mut self, key: Key, m: Mods) {
        let Key::Char(c) = key else {
            if key == Key::Tab {
                self.next_view(if m.shift { -1 } else { 1 });
            }
            return;
        };
        match unshift(c) {
            'f' => {
                self.press(Target::View(View::Lib), 0.0, 0.0);
                self.search_focus = true;
            }
            'n' => self.press(Target::NewPlaylist, 0.0, 0.0),
            'r' => self.press(Target::Record, 0.0, 0.0),
            'o' => self.load_pick(usize::from(m.shift)),
            ',' => self.press(Target::Prefs(0), 0.0, 0.0),
            '=' | '+' => self.press(Target::WaveZoom(1), 0.0, 0.0),
            '-' => self.press(Target::WaveZoom(-1), 0.0, 0.0),
            '0' => self.press(Target::WaveZoom(0), 0.0, 0.0),
            '5' => self.press(Target::ToggleFx, 0.0, 0.0),
            'q' => self.quit = true,
            n @ '1'..='6' if m.alt => {
                const VIEWS: [View; 6] = [View::Dj, View::Wave, View::Lib, View::Fx, View::Stream, View::Ctl];
                self.press(Target::View(VIEWS[n as usize - '1' as usize]), 0.0, 0.0);
            }
            _ => {}
        }
    }

    /// The deck keys, each with its Shift action.
    fn deck_key(&mut self, c: char, m: Mods) {
        let k = unshift(c);
        let shift = m.shift || k != c;
        if let 'b' | 'n' = k {
            let d = usize::from(k == 'n');
            self.decks[d].kills[2] = !self.decks[d].kills[2];
            self.send(Command::Kill(d, self.decks[d].kills));
            return;
        }
        let Some(t) = keymap(k) else { return };
        if !shift {
            self.pointer_target(t);
            return;
        }
        match t {
            Target::Play(d) => {
                let at = self.telemetry.decks[d].position;
                self.decks[d].cue = self.decks[d].snap(at);
            }
            Target::Cue(d) => {
                self.send(Command::Play(d, false));
                self.send(Command::Seek(d, self.decks[d].cue));
            }
            Target::Jump(d, dir) => self.beat_jump(d, f64::from(dir)),
            Target::Pad(d, p) => self.decks[d].cues[p] = None,
            Target::LoopIn(d) | Target::LoopOut(d) => {
                if let Some((a, b)) = self.decks[d].loop_range {
                    self.send(Command::Seek(d, if matches!(t, Target::LoopIn(_)) { a } else { b }));
                }
            }
            Target::LoopToggle(d) if matches!(k, '4' | '9') => {
                self.pointer_target(t);
                self.send(Command::Play(d, false));
            }
            Target::XfadeStep(by) => {
                self.crossfader = (self.crossfader + f32::from(by) * 0.02).clamp(-1.0, 1.0);
                let x = self.crossfader_out();
                self.send(Command::Crossfader(x));
            }
            t => self.pointer_target(t),
        }
    }

    /// F1 to F4 change deck 1's tempo, F5 to F8 deck 2's: down and up for good,
    /// then down and up while held; Shift makes the steps small. Shift+F12
    /// starts or stops the auto mix.
    fn function_key(&mut self, n: u8, m: Mods) {
        if n == 12 && m.shift {
            self.press(Target::AutoDj, 0.0, 0.0);
            return;
        }
        if !(1..=8).contains(&n) {
            return;
        }
        let (d, i) = (usize::from(n > 4), (n - 1) % 4);
        let small = usize::from(m.shift);
        let dir = if i % 2 == 0 { -1.0 } else { 1.0 };
        if i < 2 {
            let range = self.settings.rate_range();
            let r = (f64::from(self.decks[d].rate) + dir * RATE_STEP[small] / range).clamp(-1.0, 1.0);
            self.decks[d].rate = r as f32;
            self.send(Command::Rate(d, r * range));
        } else {
            self.send(Command::Nudge(d, dir * NUDGE[small]));
        }
    }

    /// Moves the library's selection, keeping it in view.
    fn select_by(&mut self, by: isize) {
        if self.shown.is_empty() {
            return;
        }
        let now = self.selected.and_then(|i| self.shown.iter().position(|&s| s == i));
        let at = match now {
            Some(n) => n.saturating_add_signed(by).min(self.shown.len() - 1),
            None if by < 0 => self.shown.len() - 1,
            None => 0,
        };
        self.selected = Some(self.shown[at]);
        let top = at as f32 * self.row_h;
        let room = (self.library.h - 30.0).max(self.row_h);
        if top < self.scroll {
            self.scroll = top;
        } else if top + self.row_h > self.scroll + room {
            self.scroll = top + self.row_h - room;
        }
    }

    /// Loads the selected track, or the first one shown.
    fn load_pick(&mut self, deck: usize) {
        if let Some(i) = self.selected.or_else(|| self.shown.first().copied()) {
            self.load_deck(deck, i as usize);
        }
    }

    fn next_view(&mut self, by: isize) {
        const ORDER: [View; 6] = [View::Dj, View::Wave, View::Lib, View::Fx, View::Stream, View::Ctl];
        let at = ORDER.iter().position(|v| *v == self.view).unwrap_or(0);
        let next = ORDER[(at as isize + by).rem_euclid(ORDER.len() as isize) as usize];
        self.press(Target::View(next), 0.0, 0.0);
    }

    pub fn load_deck(&mut self, deck: usize, index: usize) {
        use crate::settings::Opt;
        if self.settings.on(Opt::Protect) && self.telemetry.decks[deck].playing {
            self.status = Some(format!("Deck {} is playing; stop it to load another track", deck + 1));
            return;
        }
        self.history.retain(|&i| i as usize != index);
        self.history.insert(0, index as u32);
        self.status = None;
        self.decks[deck].loading = true;
        self.load(Job::Deck(deck), index);
    }

    /// What a finger held still, or a right click, opens.
    fn holds(&mut self) {
        if self.cue_menu.is_none()
            && let Some(g) = self.drags.iter().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::Pad(d, p)) = g.target
            && self.decks[d].mode == Mode::Cues
            && self.decks[d].cues[p].is_some()
        {
            self.cue_menu = Some((d, p, g.start.0, g.start.1));
            self.submenu = None;
        }
        if let Some(g) = self.drags.iter_mut().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::Sync(d)) = g.target
        {
            g.moved = true;
            let t = self.telemetry.decks[d];
            let c = if t.sync_lock { Command::SyncLeader(d, t.sync_leader < 2) } else { Command::SyncLock(d, true) };
            self.send(c);
        }
        if self.track_menu.is_none()
            && let Some(g) = self.drags.iter_mut().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && g.target == Some(Target::Library)
        {
            g.moved = true;
            let row = ((g.start.1 - self.library.y - 30.0 + self.scroll) / self.row_h).floor();
            if row >= 0.0 && g.start.1 > self.library.y + 30.0 {
                self.track_menu = self.shown.get(row as usize).copied();
                self.menu_at = g.start;
                self.submenu = None;
            }
        }
        if self.list_menu.is_none()
            && let Some(g) = self.drags.iter_mut().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::Shelf(Shelf::Playlist(n))) = g.target
        {
            g.moved = true;
            self.list_menu = Some(n);
            self.menu_at = g.start;
        }
        if self.picker.is_none()
            && let Some(g) = self.drags.iter().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::Pad(d, p)) = g.target
            && self.decks[d].mode == Mode::Samples
        {
            self.picker = Some((d, p));
            let at = self.pads[d][self.bank[d] * 8 + p];
            self.picker_scroll = (at / self.picker_grid.cols.max(1)) as f32 * self.picker_grid.cell.1;
        }
    }

    /// What the settings ask of a deck that has just loaded a track.
    fn on_load(&mut self, d: usize) {
        use crate::settings::Opt;
        let s = self.settings.clone();
        let lock = s.on(Opt::KeyLock);
        self.decks[d].key_lock = lock;
        self.send(Command::KeyLock(d, lock));
        let q = s.on(Opt::Quantize);
        self.decks[d].quantize = q;
        self.send(Command::Quantize(d, q));
        if s.on(Opt::ResetTempo) {
            self.decks[d].rate = 0.0;
            self.send(Command::Rate(d, 0.0));
        }
        if s.on(Opt::ResetEq) {
            self.decks[d].knobs[..3].fill(0.5);
            self.send(Command::Eq(d, [knob_gain(0.5); 3]));
        }
        if s.on(Opt::ResetGain) {
            self.decks[d].gain = 0.5;
        }
        self.decks[d].auto_gain = if s.on(Opt::AutoGain) {
            self.decks[d].loaded.as_ref().map_or(1.0, |l| auto_gain(&l.track.frames, s.gain_target()))
        } else {
            1.0
        };
        self.send_gain(d);
        if s.on(Opt::TempoMatch) && self.telemetry.decks[1 - d.min(1)].playing {
            self.match_tempo(d);
        }
        if s.on(Opt::KeyMatch) {
            self.match_key(d);
        }
        if s.on(Opt::PflOnLoad) {
            for o in 0..self.decks.len() {
                let on = o == d;
                self.decks[o].pfl = on;
                self.send(Command::Pfl(o, on));
            }
        }
    }

    /// The device playing, or why nothing is.
    pub fn audio_device(&self) -> String {
        self.control.as_ref().map(Control::status).unwrap_or_default()
    }

    fn send_gain(&mut self, d: usize) {
        let g = knob_gain(self.decks[d].gain) * self.decks[d].auto_gain;
        self.send(Command::Gain(d, g));
    }

    /// Shifts deck `d` by the fewest semitones to the key of the deck playing.
    fn match_key(&mut self, d: usize) {
        let key = |i: usize| self.decks[i].loaded.as_ref().and_then(|l| l.key);
        let Some(mine) = key(d) else { return };
        let Some(theirs) = (0..self.decks.len())
            .filter(|&o| o != d && self.telemetry.decks[o].playing)
            .find_map(|o| key(o).map(|k| (k, self.decks[o].semitones)))
        else {
            return;
        };
        // A relative minor or major counts as the same key.
        let tonic = |k: sugar_analysis::Key| i16::from(k.tonic) + if k.minor { 3 } else { 0 };
        let diff = (tonic(theirs.0) + i16::from(theirs.1) - tonic(mine)).rem_euclid(12);
        let st = if diff > 6 { diff - 12 } else { diff } as i8;
        self.decks[d].semitones = st;
        self.send(Command::Key(d, f32::from(st)));
    }

    /// Sends the crossfader's curve as the settings have it.
    fn send_curve(&mut self) {
        let c = self.settings.curve();
        self.send(Command::Curve(c));
    }

    fn pref_tap(&mut self, t: Target) {
        match t {
            Target::Pref(o) => match crate::settings::spec(o).kind {
                crate::settings::Kind::Switch => {
                    let v = u8::from(!self.settings.on(o));
                    self.set_pref(o, v);
                }
                _ => {
                    let at = self.rect_of(Target::Pref(o)).map_or((0.0, 0.0), |r| (r.x, r.y + r.h + 2.0));
                    self.pref_menu = Some((o, at.0, at.1));
                }
            },
            Target::PrefPick(o, v) => {
                self.pref_menu = None;
                self.set_pref(o, v);
            }
            _ => {}
        }
    }

    fn prefs_scroll_by(&mut self, dy: f32) {
        self.prefs_scroll = (self.prefs_scroll + dy).clamp(0.0, self.prefs_max);
    }

    /// Sets a setting and applies what changes at once.
    fn set_pref(&mut self, o: crate::settings::Opt, v: u8) {
        use crate::settings::Opt;
        self.settings.set(o, v);
        match o {
            Opt::RateRange => {
                for d in 0..self.decks.len() {
                    let r = f64::from(self.decks[d].rate) * self.settings.rate_range();
                    self.send(Command::Rate(d, r));
                }
            }
            Opt::ReverseFader => {
                let x = self.crossfader_out();
                self.send(Command::Crossfader(x));
            }
            Opt::FaderOn | Opt::FaderCurve | Opt::CutIn => self.send_curve(),
            Opt::AutoGain | Opt::GainTarget => {
                for d in 0..self.decks.len() {
                    let target = self.settings.gain_target();
                    let on = self.settings.on(Opt::AutoGain);
                    self.decks[d].auto_gain = match &self.decks[d].loaded {
                        Some(l) if on => auto_gain(&l.track.frames, target),
                        _ => 1.0,
                    };
                    self.send_gain(d);
                }
            }
            _ => {}
        }
    }

    /// The crossfader as the mixer takes it: flipped when the DJ reversed it.
    fn crossfader_out(&self) -> f32 {
        if self.settings.on(crate::settings::Opt::ReverseFader) { -self.crossfader } else { self.crossfader }
    }

    /// The tempo fader position to its value, -1 to 1, as the DJ set its direction.
    pub fn rate_from(&self, up: f32) -> f32 {
        if self.settings.on(crate::settings::Opt::TempoUp) { up } else { -up }
    }

    fn pad(&mut self, deck: usize, pad: usize) {
        self.sample(self.pads[deck % 2][self.bank[deck % 2] * 8 + pad]);
    }

    fn sample(&mut self, i: usize) {
        match self.samples.get(i).cloned().flatten() {
            Some(_) => self.play_sample(i),
            None => self.load(Job::Sample(i, true), i),
        }
    }

    fn hotcue(&mut self, d: usize, p: usize) {
        let Some(l) = &self.decks[d].loaded else { return };
        let rate = l.track.rate;
        let grid = l.grid;
        match self.decks[d].cues[p] {
            Some(at) => {
                self.send(Command::Seek(d, at));
                self.send(Command::Play(d, true));
                self.telemetry.decks[d].position = at;
            }
            None => {
                let pos = self.telemetry.decks[d].position;
                let at = if self.decks[d].quantize { grid.map_or(pos, |g| g.snap(rate, pos)).max(0.0) } else { pos };
                self.decks[d].cues[p] = Some(at);
                self.decks[d].cue_colours[p] = p as u8;
            }
        }
    }

    fn fx(&mut self, d: usize, p: usize) {
        let Some(l) = &self.decks[d].loaded else { return };
        let rate = l.track.rate;
        let speed = 1.0 + f64::from(self.decks[d].rate) * self.settings.rate_range();
        let beat = l.grid.map_or(0.5 * f64::from(rate), |g| g.beat_frames(rate)) / speed;
        let out_beat = beat * f64::from(self.out_rate) / f64::from(rate);
        match p {
            0..5 if self.decks[d].echo == Some(p) => {
                self.decks[d].echo = None;
                self.send(Command::Echo(d, 0.0, out_beat));
            }
            0..5 => {
                self.decks[d].echo = Some(p);
                self.send(Command::Echo(d, 0.6, out_beat * ECHO_BEATS[p]));
            }
            5 => {
                self.decks[d].echo = Some(3);
                self.send(Command::Echo(d, 0.75, out_beat));
                self.send(Command::Play(d, false));
                self.telemetry.decks[d].playing = false;
            }
            6 => self.send(Command::Brake(d, 1.2)),
            _ => self.send(Command::Spin(d, 0.8)),
        }
    }

    /// Sets deck `d`'s tempo fader so it plays at the other deck's tempo, folded to the nearest octave.
    /// Returns false when either deck has no beat grid.
    fn match_tempo(&mut self, d: usize) -> bool {
        let o = 1 - d.min(1);
        let (Some(me), Some(other)) = (&self.decks[d].loaded, &self.decks[o].loaded) else { return false };
        let (Some(g), Some(og)) = (me.grid, other.grid) else { return false };
        let other_speed = 1.0 + f64::from(self.decks[o].rate) * self.settings.rate_range();
        let mut target = og.bpm * other_speed;
        while target / g.bpm > 1.5 {
            target /= 2.0;
        }
        while target / g.bpm < 0.67 {
            target *= 2.0;
        }
        let r = ((target / g.bpm - 1.0) / self.settings.rate_range()).clamp(-1.0, 1.0);
        self.decks[d].rate = r as f32;
        self.send(Command::Rate(d, r * self.settings.rate_range()));
        true
    }

    fn sync(&mut self, d: usize) {
        if !self.match_tempo(d) {
            return;
        }
        let o = 1 - d.min(1);
        let (Some(me), Some(other)) = (&self.decks[d].loaded, &self.decks[o].loaded) else { return };
        let (Some(g), Some(og)) = (me.grid, other.grid) else { return };
        let (rate, orate) = (me.track.rate, other.track.rate);
        if self.telemetry.decks[o].playing {
            let op = self.telemetry.decks[o].position;
            let phase = ((op - og.first_beat) / og.beat_frames(orate)).rem_euclid(1.0);
            let beat = g.beat_frames(rate);
            let pos = self.telemetry.decks[d].position;
            let base = g.first_beat + ((pos - g.first_beat) / beat).floor() * beat;
            let to = (base + phase * beat).max(0.0);
            self.send(Command::Seek(d, to));
            self.telemetry.decks[d].position = to;
        }
    }

    fn auto_loop(&mut self, d: usize, p: usize) {
        if self.decks[d].looping == Some(p) {
            self.decks[d].looping = None;
            self.decks[d].loop_range = None;
            self.send(Command::Loop(d, None));
            return;
        }
        self.decks[d].loop_size = p;
        self.beat_loop(d, LOOP_BEATS[p], Some(p));
    }

    fn beat_loop(&mut self, d: usize, beats: f64, pad: Option<usize>) {
        let Some(l) = &self.decks[d].loaded else { return };
        let g = l.grid.unwrap_or(sugar_analysis::Grid { bpm: 120.0, first_beat: 0.0 });
        let beat = g.beat_frames(l.track.rate);
        let pos = self.telemetry.decks[d].position;
        let start = (g.first_beat + ((pos - g.first_beat) / beat).floor() * beat).max(0.0);
        let range = (start, start + beat * beats);
        self.decks[d].looping = pad;
        self.decks[d].loop_range = Some(range);
        self.send(Command::Loop(d, Some(range)));
    }

    fn grid_edit(&mut self, d: usize, op: GridOp) {
        let pos = self.telemetry.decks[d].position;
        let other = self.decks[1 - d].beat().map(|(g, b)| (g, b, self.telemetry.decks[1 - d].position));
        let deck = &mut self.decks[d];
        match op {
            GridOp::Lock => return deck.grid_locked = !deck.grid_locked,
            GridOp::Undo => {
                if let (false, Some(g), Some(l)) = (deck.grid_locked, deck.grid_undo.pop(), &mut deck.loaded) {
                    l.grid = Some(g);
                }
                return;
            }
            _ if deck.grid_locked => return,
            _ => {}
        }
        let Some(l) = &mut deck.loaded else { return };
        let rate = l.track.rate;
        let g = l.grid.unwrap_or(sugar_analysis::Grid { bpm: 120.0, first_beat: 0.0 });
        let beat = g.beat_frames(rate);
        let nudge = f64::from(rate) * 0.01;
        let mut n = g;
        match op {
            GridOp::Half => n.first_beat += beat * 0.5,
            GridOp::Here => n.first_beat = pos,
            GridOp::Earlier => n.first_beat -= nudge,
            GridOp::Later => n.first_beat += nudge,
            GridOp::Halve => n.bpm *= 0.5,
            GridOp::Double => n.bpm *= 2.0,
            GridOp::Match => {
                let Some((og, ob, opos)) = other else { return };
                let phase = ((opos - og.first_beat) / ob).rem_euclid(1.0);
                n.first_beat = pos - phase * beat;
            }
            GridOp::Undo | GridOp::Lock => unreachable!(),
        }
        n.first_beat = n.first_beat.rem_euclid(n.beat_frames(rate));
        deck.grid_undo.push(g);
        l.grid = Some(n);
    }

    fn beat_jump(&mut self, d: usize, beats: f64) {
        let Some((_, beat)) = self.decks[d].beat() else { return };
        let by = beat * beats;
        let to = (self.telemetry.decks[d].position + by).clamp(0.0, self.decks[d].length());
        if let Some((a, b)) = self.decks[d].loop_range {
            let range = (a + by, b + by);
            self.decks[d].loop_range = Some(range);
            self.send(Command::Loop(d, Some(range)));
        }
        self.send(Command::Seek(d, to));
        self.telemetry.decks[d].position = to;
    }

    pub fn pressed(&self, t: Target) -> bool {
        self.drags.iter().any(|d| d.target == Some(t))
    }

    fn hit(&self, x: f32, y: f32) -> Option<Target> {
        self.hits.iter().rev().find(|h| h.rect.hit(x, y)).map(|h| h.target)
    }

    pub fn pointer_down(&mut self, id: u64, x: f32, y: f32) {
        let target = self.hit(x, y);
        self.drags.retain(|d| d.id != id);
        let resume = matches!(target, Some(Target::Jog(d)) if self.telemetry.decks[d].playing);
        self.drags.push(Drag {
            id,
            target,
            start: (x, y),
            last: (x, y),
            moved: false,
            resume,
            at: std::time::Instant::now(),
        });
        self.search_focus = target == Some(Target::SearchFocus);
        self.hub.focus = target == Some(Target::HubFocus);
        if let Some(t) = target {
            self.press(t, x, y);
        }
    }

    /// Lets go of every finger and the mouse, as when the window loses focus.
    pub fn release_all(&mut self) {
        for d in &mut self.drags {
            d.moved = true;
        }
        while let Some(d) = self.drags.last() {
            let (id, y) = (d.id, d.last.1);
            self.release(id, y);
        }
        self.remember();
    }

    pub fn pointer_target(&mut self, t: Target) {
        self.press(t, 0.0, 0.0);
    }

    fn press(&mut self, t: Target, x: f32, y: f32) {
        match t {
            Target::Play(d) if self.decks[d].loaded.is_some() => {
                let on = !self.telemetry.decks[d].playing;
                self.focus = d;
                if on
                    && self.settings.on(crate::settings::Opt::SyncOnPlay)
                    && self.telemetry.decks[1 - d.min(1)].playing
                {
                    self.sync(d);
                }
                self.send(Command::Play(d, on));
                self.telemetry.decks[d].playing = on;
            }
            Target::Cue(d) => {
                self.focus = d;
                let cue = self.decks[d].cue;
                if self.telemetry.decks[d].playing {
                    self.send(Command::Play(d, false));
                    self.send(Command::Seek(d, cue));
                } else if (self.telemetry.decks[d].position - cue).abs() < 1.0 {
                    self.send(Command::Play(d, true));
                    self.decks[d].previewing = self.settings.get(crate::settings::Opt::CueMode) == 1;
                } else {
                    self.decks[d].cue = self.telemetry.decks[d].position;
                }
            }
            Target::KeyLock(d) => {
                let on = !self.decks[d].key_lock;
                self.decks[d].key_lock = on;
                self.send(Command::KeyLock(d, on));
            }
            Target::Pad(d, p) => {
                self.focus = d;
                match self.decks[d].mode {
                    Mode::Cues => self.hotcue(d, p),
                    Mode::Loops => self.auto_loop(d, p),
                    Mode::Samples => self.pad(d, p),
                    Mode::Fx => self.fx(d, p),
                }
            }
            Target::Mode(d, m) => {
                if m == Mode::Samples && self.decks[d].mode == m {
                    self.bank[d] = (self.bank[d] + 1) % BANKS;
                }
                self.decks[d].mode = m;
            }
            Target::PickerClose => self.picker = None,
            Target::AutoDj => self.toggle_auto(),
            Target::Quit => self.quit = true,
            Target::Find => {
                self.app_menu = false;
                self.view = View::Lib;
                self.search_focus = true;
            }
            Target::LoadPick(d) => {
                self.app_menu = false;
                self.load_pick(d);
            }
            Target::Jog(d) => {
                self.focus = d;
                self.send(Command::Play(d, false));
            }
            Target::Browse(d) => {
                self.focus = d;
                self.scroll = 0.0;
                self.browser = if self.browser == Some(d) { None } else { Some(d) };
            }
            Target::CloseBrowser => self.browser = None,
            Target::CueMenuClose => {
                self.cue_menu = None;
                self.submenu = None;
            }
            Target::Submenu(n) => self.submenu = Some(n),
            Target::AppMenu => {
                self.app_menu = !self.app_menu;
                self.submenu = None;
            }
            Target::Prefs(n) => {
                self.prefs = Some(usize::from(n));
                self.prefs_scroll = 0.0;
                self.app_menu = false;
                self.pref_menu = None;
                self.submenu = None;
            }
            Target::PrefsClose => {
                if self.pref_menu.take().is_none() {
                    self.prefs = None;
                }
            }
            // Settings rows act on release, so a drag over them scrolls instead.
            Target::Pref(_) | Target::PrefPick(..) | Target::PrefsScroll => {}
            Target::Rescan => {
                self.app_menu = false;
                self.scan();
                self.status = Some("Looking for new tracks".into());
            }
            Target::MenuBody => {}
            Target::Grid(d) if self.decks[d].loaded.is_some() => {
                self.grid_menu = Some(d);
                self.submenu = None;
            }
            Target::GridClose => {
                self.grid_menu = None;
                self.submenu = None;
            }
            Target::SearchFocus => {}
            Target::HubFocus => {}
            Target::HubService(sv) => self.hub.switch(sv),
            Target::Filter(n) => {
                self.filters ^= 1 << n;
                let q = std::mem::take(&mut self.query);
                self.search(q);
            }
            Target::StreamFrom(sv) => {
                self.hub.switch(sv);
                self.view = View::Stream;
            }
            Target::HubSearch => {
                self.hub.focus = false;
                self.hub.search();
            }
            Target::HubList => {}
            Target::HubLoad(hit, deck) => self.hub.fetch(hit, deck),
            Target::Shelf(sh) => {
                if self.view == View::Stream {
                    self.view = View::Lib;
                }
                self.shelf = sh;
                self.selected = None;
                let q = std::mem::take(&mut self.query);
                self.search(q);
            }
            Target::NewPlaylist => {
                let name = format!("Playlist {}", self.playlists.len() + 1);
                self.playlists.push((name, Vec::new()));
                crate::store::save_playlists(&self.playlists);
            }
            Target::MenuDeck(d) => {
                if let Some(i) = self.track_menu.take() {
                    self.load_deck(d, i as usize);
                }
            }
            Target::MenuToggle(p) => {
                if let (Some(i), Some((_, keys))) = (self.track_menu, self.playlists.get_mut(p)) {
                    let k = entry_key(&self.entries[i as usize]);
                    match keys.iter().position(|x| *x == k) {
                        Some(at) => {
                            keys.remove(at);
                        }
                        None => keys.push(k),
                    }
                    crate::store::save_playlists(&self.playlists);
                    if self.shelf == Shelf::Playlist(p) {
                        let q = std::mem::take(&mut self.query);
                        self.search(q);
                    }
                }
            }
            Target::MenuPlay(d) => {
                if let Some(i) = self.track_menu.take() {
                    self.load_deck(d, i as usize);
                    self.play_on_load = Some(d);
                }
            }
            Target::MenuRate(n) | Target::MenuColour(n) => {
                if let Some(i) = self.track_menu {
                    let key = entry_key(&self.entries[i as usize]);
                    let mut saved = self.store.get(&key).unwrap_or_default();
                    if let Target::MenuRate(_) = t {
                        saved.rating = if saved.rating == n { 0 } else { n };
                    } else {
                        saved.colour = n;
                    }
                    self.store.put(&key, saved);
                }
            }
            Target::MenuArtist | Target::MenuFolder => {
                if let Some(i) = self.track_menu.take() {
                    let e = &self.entries[i as usize];
                    let q = if t == Target::MenuArtist { e.artist.to_string() } else { e.folder.to_string() };
                    self.shelf = Shelf::Tracks;
                    self.view = View::Lib;
                    self.search(q);
                }
            }
            Target::MenuAnalyze => {
                if let Some(i) = self.track_menu.take() {
                    self.analyze_one(i);
                }
            }
            Target::MenuRemove => {
                if let (Some(i), Shelf::Playlist(p)) = (self.track_menu.take(), self.shelf) {
                    let k = entry_key(&self.entries[i as usize]);
                    if let Some((_, keys)) = self.playlists.get_mut(p) {
                        keys.retain(|x| *x != k);
                    }
                    crate::store::save_playlists(&self.playlists);
                    let q = std::mem::take(&mut self.query);
                    self.search(q);
                }
            }
            Target::ListOp(n, op) => {
                self.list_menu = None;
                self.list_op(n, op);
            }
            Target::MenuClose => {
                self.list_menu = None;
                self.track_menu = None;
                self.submenu = None;
            }
            Target::Scene(i) => {
                self.station.scene = Scene::ALL[i.min(Scene::ALL.len() - 1)];
                self.send(Command::Station(Set::Scene(self.station.scene)));
            }
            Target::Release(i) => {
                self.station.release = [Release::Brake, Release::Echo, Release::BackSpin][i.min(2)];
                self.station.iso = [0.0; 3];
                self.send(Command::Station(Set::Release(self.station.release)));
            }
            Target::Record => {
                if let Some(rec) = self.recorder.take() {
                    self.status = Some(format!("Recording saved: {}", rec.path.display()));
                    self.send(Command::Record(false));
                    if let Some(c) = &mut self.control {
                        let mut rest = Vec::new();
                        c.drain_recording(&mut rest);
                    }
                } else if let Some(dir) = recordings_dir()
                    && let Ok(rec) = crate::record::Recorder::start(dir, self.out_rate)
                {
                    self.recorder = Some(rec);
                    self.send(Command::Record(true));
                }
            }
            Target::XfadeStep(by) => {
                self.crossfader = (self.crossfader + f32::from(by) * 0.1).clamp(-1.0, 1.0);
                let x = self.crossfader_out();
                self.send(Command::Crossfader(x));
            }
            Target::Sort(c) => {
                self.sort = if self.sort.0 == c { (c, !self.sort.1) } else { (c, true) };
                let q = std::mem::take(&mut self.query);
                self.search(q);
            }
            Target::LoadSelected(d) => {
                if let Some(i) = self.selected {
                    self.load_deck(d, i as usize);
                }
            }
            Target::GridOp(op) => {
                if let Some(d) = self.grid_menu {
                    self.grid_edit(d, op);
                }
            }
            Target::CueColour(c) => {
                if let Some((d, p, ..)) = self.cue_menu.take() {
                    self.decks[d].cue_colours[p] = c;
                }
            }
            Target::CueClear => {
                if let Some((d, p, ..)) = self.cue_menu.take() {
                    self.decks[d].cues[p] = None;
                }
            }
            Target::View(v) => {
                if self.view != v {
                    self.view_at = Some(std::time::Instant::now());
                }
                self.view = v;
            }
            Target::ToggleFx => self.show_fx = !self.show_fx,
            Target::ToggleSamplers => self.show_samplers = !self.show_samplers,
            Target::WaveZoom(z) => self.wave_zoom = if z == 0 { 0 } else { (self.wave_zoom + z).clamp(-3, 3) },
            Target::WaveHeight(z) => self.wave_height = if z == 0 { 0 } else { (self.wave_height + z).clamp(-3, 3) },
            Target::Sampler(i) => self.sample(i),
            Target::Reverse(d) => {
                let on = !self.decks[d].reverse;
                self.decks[d].reverse = on;
                self.send(Command::Reverse(d, on));
            }
            Target::Slip(d) => {
                let on = !self.decks[d].slip;
                self.decks[d].slip = on;
                self.send(Command::Slip(d, on));
            }
            Target::Eject(d) if !self.telemetry.decks[d].playing => {
                self.decks[d] = DeckUi { volume: self.decks[d].volume, knobs: self.decks[d].knobs, ..DeckUi::new() };
                self.send(Command::Unload(d));
            }
            Target::Pfl(d) => {
                let on = !self.decks[d].pfl;
                self.decks[d].pfl = on;
                self.send(Command::Pfl(d, on));
            }
            Target::Quantize(d) => {
                let on = !self.decks[d].quantize;
                self.decks[d].quantize = on;
                self.send(Command::Quantize(d, on));
            }
            Target::Pitch(d, by) => {
                let st = if by == 0 { 0 } else { (self.decks[d].semitones + by).clamp(-12, 12) };
                self.decks[d].semitones = st;
                self.send(Command::Key(d, f32::from(st)));
            }
            Target::Jump(d, dir) => self.beat_jump(d, JUMP_BEATS[self.decks[d].jump] * f64::from(dir)),
            Target::LoopSize(d, by) => {
                let i = self.decks[d].loop_size.saturating_add_signed(by.into()).min(LOOP_BEATS.len() - 1);
                self.decks[d].loop_size = i;
                if let Some((a, _)) = self.decks[d].loop_range
                    && let Some((_, beat)) = self.decks[d].beat()
                {
                    let range = (a, a + beat * LOOP_BEATS[i]);
                    self.decks[d].loop_range = Some(range);
                    self.send(Command::Loop(d, Some(range)));
                }
            }
            Target::LoopToggle(d) => {
                if self.decks[d].loop_range.is_some() {
                    self.decks[d].looping = None;
                    self.decks[d].loop_range = None;
                    self.send(Command::Loop(d, None));
                } else {
                    self.beat_loop(d, LOOP_BEATS[self.decks[d].loop_size], None);
                }
            }
            Target::LoopIn(d) => {
                let at = self.decks[d].snap(self.telemetry.decks[d].position);
                self.decks[d].loop_in = Some(at);
            }
            Target::LoopOut(d) => {
                let out = self.decks[d].snap(self.telemetry.decks[d].position);
                if let Some(a) = self.decks[d].loop_in
                    && out > a + 64.0
                {
                    self.decks[d].loop_range = Some((a, out));
                    self.decks[d].looping = None;
                    self.send(Command::Loop(d, Some((a, out))));
                }
            }
            _ => self.drag(t, x, y, (0.0, 0.0)),
        }
    }

    pub fn pointer_move(&mut self, id: u64, x: f32, y: f32) {
        let Some(i) = self.drags.iter().position(|d| d.id == id) else { return };
        let d = &mut self.drags[i];
        let delta = (x - d.last.0, y - d.last.1);
        d.last = (x, y);
        d.moved |= (x - d.start.0).hypot(y - d.start.1) > TAP;
        if let Some(t) = d.target {
            self.drag(t, x, y, delta);
        }
    }

    pub fn pointer_up(&mut self, id: u64, y: f32) {
        self.release(id, y);
        self.remember();
    }

    fn release(&mut self, id: u64, y: f32) {
        let Some(i) = self.drags.iter().position(|d| d.id == id) else { return };
        let d = self.drags.swap_remove(i);
        if let Some(Target::Cue(deck)) = d.target {
            self.end_preview(deck);
        }
        if let Some(t @ (Target::Pref(_) | Target::PrefPick(..))) = d.target
            && !d.moved
        {
            self.pref_tap(t);
        }
        match d.target {
            Some(Target::Library) if !d.moved => {
                let row = ((y - self.library.y - 30.0 + self.scroll) / self.row_h).floor();
                if let Some(&index) = self.shown.get(row.max(0.0) as usize)
                    && row >= 0.0
                    && y > self.library.y + 30.0
                {
                    match self.browser.take() {
                        Some(d) => self.load_deck(d, index as usize),
                        None if self.selected == Some(index) => {
                            let d = if self.telemetry.decks[self.focus].playing {
                                1 - self.focus.min(1)
                            } else {
                                self.focus
                            };
                            self.load_deck(d, index as usize);
                        }
                        None => self.selected = Some(index),
                    }
                }
            }
            Some(Target::Picker) if !d.moved => {
                let g = self.picker_grid;
                if let (Some((deck, pad)), Some(r)) = (self.picker, self.rect_of(Target::Picker)) {
                    let col = ((d.last.0 - r.x) / g.cell.0) as usize;
                    let row = ((y - r.y + self.picker_scroll) / g.cell.1) as usize;
                    let i = row * g.cols + col;
                    if col < g.cols && i < SAMPLES.len() {
                        self.pads[deck][self.bank[deck] * 8 + pad] = i;
                        self.sample(i);
                        crate::store::save_pads(&self.pads);
                    }
                }
            }
            Some(Target::Xy) => {
                self.station.on = false;
                self.send(Command::Station(Set::On(false)));
            }
            // A tap matches tempo and phase once, so a stray touch never locks the
            // deck, or unlocks a locked one; holding locks it (see `tick`).
            Some(Target::Sync(deck)) if !d.moved => {
                let c = if self.telemetry.decks[deck].sync_lock {
                    Command::SyncLock(deck, false)
                } else {
                    Command::BeatSync(deck)
                };
                self.send(c);
            }
            Some(Target::Jog(deck)) if d.resume => {
                self.send(Command::Play(deck, true));
                self.telemetry.decks[deck].playing = true;
            }
            _ => {}
        }
    }

    pub fn wheel(&mut self, x: f32, y: f32, dy: f32) {
        if self.picker.is_some() {
            self.drag(Target::Picker, x, y, (0.0, dy));
            return;
        }
        if self.prefs.is_some() {
            self.prefs_scroll_by(-dy);
        } else if self.view == View::Stream {
            self.hub_scroll(-dy);
        } else if self.library.hit(x, y) {
            self.scroll_by(-dy);
        }
    }

    fn hub_scroll(&mut self, dy: f32) {
        let max = (self.hub.hits.len() as f32 * crate::sugar::HUB_ROW - (self.hub_area.h - 8.0)).max(0.0);
        self.hub.scroll = (self.hub.scroll + dy).clamp(0.0, max);
        if self.hub.scroll >= max - crate::sugar::HUB_ROW * 3.0 {
            self.hub.more();
        }
    }

    fn scroll_by(&mut self, dy: f32) {
        let max = (self.shown.len() as f32 * self.row_h - (self.library.h - 30.0)).max(0.0);
        self.scroll = (self.scroll + dy).clamp(0.0, max);
    }

    pub(crate) fn rect_of(&self, t: Target) -> Option<Rect> {
        self.hits.iter().find(|h| h.target == t).map(|h| h.rect)
    }

    fn drag(&mut self, t: Target, x: f32, y: f32, delta: (f32, f32)) {
        let Some(r) = self.rect_of(t) else { return };
        let vertical = |r: Rect| (1.0 - (y - r.y) / r.h).clamp(0.0, 1.0);
        match t {
            Target::Volume(d) => {
                let v = vertical(r);
                self.decks[d].volume = v;
                self.send(Command::Volume(d, v * v));
            }
            Target::Rate(d) => {
                let v = vertical(r) * 2.0 - 1.0;
                let v = if v.abs() < 0.04 { 0.0 } else { v };
                let v = self.rate_from(v);
                self.decks[d].rate = v;
                self.send(Command::Rate(d, f64::from(v) * self.settings.rate_range()));
            }
            Target::Crossfader => {
                let v = ((x - r.x) / r.w * 2.0 - 1.0).clamp(-1.0, 1.0);
                let v = if v.abs() < 0.04 { 0.0 } else { v };
                self.crossfader = v;
                let x = self.crossfader_out();
                self.send(Command::Crossfader(x));
            }
            Target::Knob(d, k) => {
                let v = (self.decks[d].knobs[k] - delta.1 / 160.0).clamp(0.0, 1.0);
                let v = if (v - 0.5).abs() < 0.02 { 0.5 } else { v };
                self.decks[d].knobs[k] = v;
                if k == 3 {
                    self.send(Command::Filter(d, v * 2.0 - 1.0));
                } else {
                    let eq = std::array::from_fn(|b| knob_gain(self.decks[d].knobs[2 - b]));
                    self.send(Command::Eq(d, eq));
                }
            }
            Target::Overview(d) => {
                let len = self.decks[d].length();
                let pos = f64::from(((x - r.x) / r.w).clamp(0.0, 1.0)) * len;
                self.send(Command::Seek(d, pos));
                self.telemetry.decks[d].position = pos;
            }
            Target::Jog(d) => {
                let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
                let (px, py) = (x - delta.0 - cx, y - delta.1 - cy);
                let turn = (px * (y - cy) - py * (x - cx)).atan2(px * (x - cx) + py * (y - cy));
                if let Some(l) = &self.decks[d].loaded {
                    let frames = f64::from(turn / std::f32::consts::TAU) * 1.8 * f64::from(l.track.rate);
                    let pos = (self.telemetry.decks[d].position + frames).clamp(0.0, self.decks[d].length());
                    self.telemetry.decks[d].position = pos;
                    self.send(Command::Seek(d, pos));
                }
            }
            Target::Gain(d) => {
                let v = (self.decks[d].gain - delta.1 / 160.0).clamp(0.0, 1.0);
                let v = if (v - 0.5).abs() < 0.02 { 0.5 } else { v };
                self.decks[d].gain = v;
                self.send_gain(d);
            }
            Target::Library => self.scroll_by(-delta.1),
            Target::Pref(_) | Target::PrefsScroll => self.prefs_scroll_by(-delta.1),
            Target::PrefPick(..) if self.pref_menu.is_none() => self.prefs_scroll_by(-delta.1),
            Target::HubList => self.hub_scroll(-delta.1),
            Target::Picker => {
                let g = self.picker_grid;
                let rows = SAMPLES.len().div_ceil(g.cols.max(1)) as f32;
                let max = (rows * g.cell.1 - r.h).max(0.0);
                self.picker_scroll = (self.picker_scroll - delta.1).clamp(0.0, max);
            }
            Target::Xy => {
                let xy = (((x - r.x) / r.w).clamp(0.0, 1.0), (1.0 - (y - r.y) / r.h).clamp(0.0, 1.0));
                self.station.xy = xy;
                self.send(Command::Station(Set::Dial(xy.0)));
                self.send(Command::Station(Set::Params(self.station.time, xy.1)));
                if !self.station.on {
                    self.station.on = true;
                    self.send(Command::Station(Set::On(true)));
                }
            }
            Target::StationTime => {
                self.station.time = (self.station.time - delta.1 / 160.0).clamp(0.0, 1.0);
                self.send(Command::Station(Set::Params(self.station.time, self.station.xy.1)));
            }
            Target::Iso(k) => {
                let v = (self.station.iso[k] - delta.1 / 80.0).clamp(-1.0, 1.0);
                self.station.iso[k] = if v.abs() < 0.04 { 0.0 } else { v };
                self.send(Command::Station(Set::Isolate(self.station.iso)));
            }
            _ => {}
        }
    }

    /// Tempo the FX station follows: the playing deck, the focused one otherwise.
    fn master_bpm(&self) -> f64 {
        let pick = (0..2).find(|&d| self.telemetry.decks[d].playing).unwrap_or(self.focus.min(1));
        let deck = &self.decks[pick];
        deck.loaded
            .as_ref()
            .and_then(|l| l.grid)
            .map_or(120.0, |g| g.bpm * (1.0 + f64::from(deck.rate) * self.settings.rate_range()))
    }
}

/// The level change that brings a track's loudness to `target` dB, within 12 dB either way.
fn auto_gain(frames: &[[f32; 2]], target: f32) -> f32 {
    // Every 16th frame is plenty to measure a whole track.
    let (sum, n) = frames.iter().step_by(16).fold((0.0f64, 0u32), |(s, n), f| {
        let m = f64::from(f[0] + f[1]) * 0.5;
        (s + m * m, n + 1)
    });
    if n == 0 || sum == 0.0 {
        return 1.0;
    }
    let db = 10.0 * (sum / f64::from(n)).log10() as f32;
    10f32.powf((target - db) / 20.0).clamp(0.25, 4.0)
}

pub fn knob_gain(v: f32) -> f32 {
    if v < 0.5 { (v * 2.0).powi(2) } else { 1.0 + (v - 0.5) * 2.0 }
}
