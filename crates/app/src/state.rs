use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::bridge::Bridge as Control;
use sugar_engine::{Command, DECKS, FxSet, Telemetry, Track};
use sugar_layout::Rect;

use crate::library::{self, Entry, Source};
use sugar_engine::station::{Release, Scene, Set};

include!(concat!(env!("OUT_DIR"), "/samples.rs"));

pub type Reader = fn(&str) -> Option<Vec<u8>>;

const OVERVIEW: usize = 1024;
pub const DETAIL_FRAMES: usize = 256;
/// From one row of Automix's list to the next, in points.
pub const QUEUE_ROW: f32 = 38.0;

/// The row of Automix's list at height `y`, of `shown` rows from `top` down.
pub fn queue_slot(top: f32, shown: usize, y: f32) -> usize {
    (((y - top) / QUEUE_ROW).floor().max(0.0) as usize).min(shown.saturating_sub(1))
}
/// The view switch fade, as long as the reference's reflow transition.
pub const VIEW_FADE: std::time::Duration = std::time::Duration::from_millis(150);
const TAP: f32 = 8.0;
/// Seconds of track one turn of a platter is: a record at 33 1/3.
const TURN_SECONDS: f64 = 1.8;
/// A finger on a platter that has not moved for this long, in seconds, holds the record still.
const JOG_STILL: f64 = 0.04;
/// Two presses of a knob or fader within this are a double tap.
const DOUBLE_TAP: std::time::Duration = std::time::Duration::from_millis(350);

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
    /// Times a deck's untimed lyrics from its vocals.
    TimeLyrics(usize),
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
    /// Opens the guide (0), the problem report (1) or the folder the app keeps its files in (2).
    Open(u8),
    /// Starts the first-mix tour, or ends it.
    Tour,
    /// Shrinks the window to the mini player, or brings the whole app back.
    Mini,
    /// Moves the tour on without doing its step.
    TourSkip,
    /// The tour's card: taps on it go no further.
    TourCard,
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
    /// Waits for the key to put an action on, by the key it shipped on; and every key back as shipped.
    Bind(char),
    BindReset,
    PickerClose,
    /// Starts a take from the microphone for the pad being picked, or ends the one running.
    SoundRec,
    /// Opens the picked pad's own sound for cutting.
    CutOpen,
    /// The outline of the sound being cut: a drag moves the nearer end.
    Cut,
    CutPlay,
    CutSave,
    CutClose,
    Scene(usize),
    Xy,
    XyHold,
    /// The pad's next aim: the master, then each deck.
    XyAim,
    /// The pad's deck back as it was, or flat.
    XyReset,
    StationTime,
    Iso(usize),
    Release(usize),
    Record,
    SearchFocus,
    HubFocus,
    HubSearch,
    HubList,
    HubLoad(usize, usize),
    /// Signs in to the shown service, or gives up waiting for its page.
    HubSignIn,
    HubSignOut,
    /// One of the account's lists: liked songs, then the playlists.
    HubShow(u8),
    /// One of the past searches, and forgetting them all.
    HubRecent(u8),
    HubForget,
    /// The menu of the YouTube account's channels, and one of them.
    HubChannels,
    HubChannel(u8),
    /// Opens the playlist among the hits.
    HubOpen(usize),
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
    ToggleWaves,
    ToggleExpand,
    ToggleSide,
    SideTab(u8),
    /// Puts the menu's track on Automix's list.
    MenuQueue,
    QueueRemove(usize),
    /// The answers to the question whether to quit.
    QuitNow,
    QuitCancel,
    /// Puts the track of the open menu on the Sidelist, or links it to the selected one (and back).
    MenuSide,
    MenuLink,
    /// A row of the Sidelist: onto a deck, or off the list; then the whole list onto Automix's, or emptied.
    SideLoad(usize, usize),
    SideRemove(usize),
    SideToQueue,
    SideClear,
    /// A row of the Related tab onto a deck, and a linked one unlinked.
    RelatedLoad(usize, usize),
    Unlink(usize),
    QueueClear,
    MixNow,
    MixSkip,
    OrderMenu,
    /// A track on Automix's list, to drag to another place.
    QueueRow(usize),
    /// Opens the choices of one of Automix's settings, and picks one.
    MixMenu(crate::settings::Opt),
    MixPick(crate::settings::Opt, u8),
    /// Puts Automix's list in an order, by its place in `automix::ORDERS`.
    MixOrder(u8),
    /// Steps a setting to its next choice, round to the first.
    Cycle(crate::settings::Opt),
    /// Folds or opens a sidebar section.
    Fold(u8),
    /// A choice in a chip's editor: chip, then the value.
    ChipSet(u8, u8),
    /// Sets the menu's track to a preparation stage; the lit stage clears it.
    MenuStage(u8),
    /// The field a playlist's new name is typed in.
    NameField,
    /// Puts the menu's track's tag on or takes it off.
    MenuTag(usize),
    /// A new tag on the menu's track, named next.
    NewTag,
    /// Turns an effect slot on or off; holding it lists the effects.
    FxOn(usize, usize),
    /// Opens a slot's list of effects.
    FxName(usize, usize),
    /// Loads an effect in a slot from its list.
    FxPick(usize, usize, u8),
    FxMeta(usize, usize),
    FxMix(usize),
    /// Opens a slot's parameters.
    FxEdit(usize, usize),
    /// One parameter of the open slot, as a slider.
    FxParam(usize),
    /// Deletes a tag from every track.
    DeleteTag(usize),
    RenameTag(usize),
    ToggleColumn(crate::library::Column),
    ResetColumns,
    /// A hot cue row in the side panel: jumps there, or sets it at the play position when empty.
    SideCue(usize, usize),
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
    /// Starts typing a new name in the sidebar.
    Rename,
    /// Writes the list as a file other players read.
    Export,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shelf {
    #[default]
    Tracks,
    History,
    /// The mixes recorded here.
    Recordings,
    Playlist(usize),
    /// The tracks in one folder, by its place in `State::folders`.
    Folder(usize),
    /// The tracks at one preparation stage, 1 to 3.
    Stage(u8),
    /// The tracks wearing one tag.
    Tag(usize),
}

/// The preparation stages, in order, with their colours.
pub const STAGES: [(&str, u32); 3] = [("Inbox", 0x8a8a96), ("In Prep", 0xf5a02a), ("Play Ready", 0x3ccf6e)];

/// The library's filter chips, in order, each a bit of `State::filters`.
pub const FILTERS: [&str; 4] = ["Near BPM", "Mixes in key", "Rated", "Colored"];
/// How far a tempo may be from the lead deck's for the Near BPM chip.
/// The longest name a playlist may have, in characters.
const NAME_MAX: usize = 48;
/// An effect unit as the screen shows it: three slots (effect, on, knob), then the dry/wet mix.
pub type FxUnit = ([(u8, bool, f32); 3], f32);
/// The parameters of one effect, each 0 to 1.
pub type FxParams = [f32; djsugar_engine::MAX_PARAMETERS];

pub const NEAR_BPM: [u8; 5] = [2, 4, 6, 8, 10];

/// The streamed tracks whose files are still there, as library entries.
fn streamed(saved: &[(String, Vec<String>)]) -> impl Iterator<Item = Entry> + '_ {
    let field = |t: &'_ [String], n: usize| t.get(n).cloned().unwrap_or_default();
    saved
        .iter()
        .filter(|(path, _)| std::path::Path::new(path).is_file())
        .map(move |(path, t)| library::file(path.into(), &field(t, 0), &field(t, 1), &field(t, 2)))
}

pub fn entry_key(e: &Entry) -> String {
    match &e.source {
        Source::File(p) => p.to_string_lossy().into_owned(),
        Source::Sample(n) => format!("sample:{n}"),
    }
}

/// How a deck was before the pad played it.
#[derive(Clone, Copy, Debug)]
pub struct Before {
    deck: usize,
    filter: f32,
    mix: f32,
    /// The pad turned the first effect on, none being on.
    lit: bool,
}

/// FX station settings shown on its page.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub scene: Scene,
    pub on: bool,
    pub xy: (f32, f32),
    /// The pad keeps the effect on after the finger lifts.
    pub hold: bool,
    /// What the pad plays: 0 the scene on the master, else that deck's filter across and its effects up.
    pub aim: usize,
    /// How the pad's deck was before the pad was touched.
    pub before: Option<Before>,
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
            hold: false,
            aim: 0,
            before: None,
            time: 0.5,
            iso: [0.0; 3],
            release: Release::Brake,
            bpm: 0.0,
        }
    }
}

pub const LONG_PRESS_MS: u128 = 450;
/// What the status line says once lyrics have their times.
const TIMED: &str = "The lyrics follow the vocals now";
/// How far a list at its top is pulled down, in pixels, for letting go to refresh it.
pub const PULL: f32 = 72.0;
/// The pointer a right click stands in for.
const CONTEXT: u64 = u64::MAX - 1;

/// What a finger held on a list has picked up: a track of the library or a hit of a streaming search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carried {
    Track(u32),
    Hit(usize),
}
/// How long a message stays on screen.
pub const TOAST: std::time::Duration = std::time::Duration::from_secs(6);

pub const JUMP_BEATS: [f64; 10] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0];

fn short(name: &str) -> &str {
    name.split_once(' ').map_or(name, |(n, rest)| if n.chars().all(|c| c.is_ascii_digit()) { rest } else { name })
}

pub fn sample_name(s: &State, i: usize) -> String {
    match i.checked_sub(SAMPLES.len()) {
        Some(n) => s.sounds.get(n).map_or_else(String::new, |(name, _)| name.clone()),
        None => s.entries.get(i).map_or_else(String::new, |e| short(&e.title).to_string()),
    }
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
    Stems,
}

/// The stems in the engine's order.
pub const STEMS: [&str; 4] = ["DRUMS", "BASS", "INST", "VOCAL"];

pub const ECHO_BEATS: [f64; 5] = [0.25, 0.5, 0.75, 1.0, 2.0];
/// The engine's Echo and Echo Out, as `loaded_effect` values.
const PAD_ECHO: u8 = 1;
const PAD_ECHO_OUT: u8 = 9;
pub const FX_PADS: [&str; 8] = ["1/4", "1/2", "3/4", "1", "2", "OUT", "BRK", "REV"];

pub const LOOP_BEATS: [f64; 8] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];

#[derive(Debug)]
pub struct Hit {
    pub rect: Rect,
    pub target: Target,
}

/// A hand on a platter: how far it turned since the last frame, when it last moved, and the speed it gave.
#[derive(Clone, Copy, Debug, Default)]
struct Jog {
    turn: f32,
    moved: Option<std::time::Instant>,
    speed: f64,
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
    /// The album art, if the file or its folder has any.
    pub art: Option<Art>,
    /// The parts that are not music, in seconds.
    pub skips: Vec<(f64, f64)>,
    /// Its phrases, for Automix's Phrases length; empty when none were found.
    pub phrases: Vec<djsugar_engine::phrases::Phrase>,
}

/// A square picture ready to draw: a cover or a thumbnail.
#[derive(Clone)]
pub struct Art {
    /// What the painter keeps it as.
    pub key: u64,
    pub side: usize,
    pub rgba: Arc<[u8]>,
}

impl std::fmt::Debug for Art {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Art({:x}, {})", self.key, self.side)
    }
}

impl Art {
    /// The middle square of `c`, averaged down to at most `side` pixels, kept under a key made from `name`.
    pub fn square(name: &str, c: &djsugar_engine::cover::Cover, side: usize) -> Option<Self> {
        use std::hash::{Hash, Hasher};
        let (w, h) = (c.width as usize, c.height as usize);
        let from = w.min(h);
        if from == 0 || c.rgba.len() < w * h * 4 {
            return None;
        }
        let side = side.min(from).min(sugar_ui::CELL);
        let (ox, oy) = ((w - from) / 2, (h - from) / 2);
        let mut rgba = vec![255u8; side * side * 4];
        for (i, cell) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x0, y0) = (ox + i % side * from / side, oy + i / side * from / side);
            let (x1, y1) = (ox + (i % side + 1) * from / side, oy + (i / side + 1) * from / side);
            let mut sum = [0u32; 3];
            let mut n = 0;
            for y in y0..y1.max(y0 + 1) {
                for x in x0..x1.max(x0 + 1) {
                    let p = &c.rgba[(y * w + x) * 4..][..3];
                    for k in 0..3 {
                        sum[k] += u32::from(p[k]);
                    }
                    n += 1;
                }
            }
            for k in 0..3 {
                cell[k] = (sum[k] / n) as u8;
            }
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (name, side).hash(&mut hash);
        Some(Self { key: hash.finish() | 1, side, rgba: rgba.into() })
    }

    /// Draws it over the rectangle, handing it to the painter first if it has let go of it.
    pub fn draw(&self, p: &mut sugar_ui::Painter, r: sugar_layout::Rect, radius: f32) {
        if !p.pictures.has(self.key) {
            p.pictures.set(self.key, self.side, self.side, &self.rgba);
        }
        p.picture(self.key, r.x, r.y, r.w, r.h, radius, sugar_ui::rgb(0xffffff));
    }
}

/// The album art of the file at `path`.
fn art(path: &std::path::Path) -> Option<Art> {
    let c = djsugar_engine::cover::find_sized(path, "", 2 * sugar_ui::CELL as u32)?;
    Art::square(&path.to_string_lossy(), &c, sugar_ui::CELL)
}

#[derive(Debug, Default)]
pub struct DeckUi {
    pub loaded: Option<Loaded>,
    pub loading: bool,
    pub rate: f32,
    pub volume: f32,
    pub knobs: [f32; 4],
    /// Low, mid and high kills.
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
    /// Stems silenced on the STEMS pads.
    pub muted: [bool; 4],
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
    /// Lyrics found online for the track now on a deck, by its file.
    Lyrics(usize, std::sync::Arc<std::path::Path>, crate::lyrics::Lyrics),
}

struct Drag {
    id: u64,
    target: Option<Target>,
    start: (f32, f32),
    last: (f32, f32),
    moved: bool,
    at: std::time::Instant,
}

pub struct State {
    control: Option<Control>,
    /// The controllers plugged in.
    pub desk: Option<crate::controllers::Desk>,
    pub telemetry: Telemetry,
    pub decks: [DeckUi; DECKS],
    pub crossfader: f32,
    pub samples: Vec<Option<Arc<Track>>>,
    pub pads: [[usize; 8 * BANKS]; 2],
    /// The user's own sounds, which the pads count on from where the built-in samples end.
    pub sounds: Vec<(String, std::path::PathBuf)>,
    pub bank: [usize; 2],
    /// Sample picker open for (deck, pad), its scroll and grid geometry.
    pub picker: Option<(usize, usize)>,
    pub picker_scroll: f32,
    /// The take running from the microphone.
    pub take: Option<djsugar_engine::capture::Capture>,
    /// The sound open for cutting, and how many times its part has been played.
    pub cut: Option<crate::sounds::Cut>,
    cut_plays: usize,
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
    jog: [Jog; DECKS],
    /// The last control pressed and when, to tell a double tap.
    last_press: Option<(Target, std::time::Instant)>,
    read: Reader,
    jobs: (Sender<Done>, Receiver<Done>),
    pub status: Option<String>,
    /// The message on screen, and when it came.
    pub toast: Option<(String, std::time::Instant)>,
    /// Only the mini player is shown.
    pub mini: bool,
    /// The step the first-mix tour is on, while it runs.
    pub tour: Option<usize>,
    /// What is carried, the pointer carrying it and where that is.
    pub carry: Option<(Carried, u64, (f32, f32))>,
    /// How far the list under the finger is pulled down past its top.
    pub pull: f32,
    /// The app's and the streaming page's last messages, to show each new one once.
    last_msgs: [String; 2],
    pub out_rate: u32,
    pub auto: Option<Auto>,
    pub view: View,
    /// When the view last changed, for its fade in.
    pub view_at: Option<std::time::Instant>,
    pub show_fx: bool,
    pub show_samplers: bool,
    /// The large waveforms in the wide Perform mode.
    pub show_waves: bool,
    /// The library over the whole wide Perform mode.
    pub lib_expanded: bool,
    /// The side panel right of the library.
    pub side_panel: bool,
    pub side_tab: u8,
    /// Folded sidebar sections, one bit each.
    pub folded: u8,
    /// The Near BPM chip's tolerance, a place in `NEAR_BPM`.
    pub near_bpm: u8,
    /// The fewest stars the Rated chip lets through.
    pub min_stars: u8,
    /// The chip whose editor is open.
    pub chip_menu: Option<u8>,
    /// The playlist being renamed, and the name so far.
    pub naming: Option<Shelf>,
    /// Each deck's effect unit: three slots (effect, on, knob), then the dry/wet mix.
    pub fx: [FxUnit; 2],
    /// The slot whose effect list is open.
    pub fx_menu: Option<(usize, usize)>,
    /// The slot whose parameters are open, and where they stand.
    pub fx_edit: Option<(usize, usize)>,
    pub fx_params: FxParams,
    /// The held tag whose menu is open.
    pub tag_menu: Option<usize>,
    pub name_buf: String,
    /// The large waveforms' zoom, each step twice as close; 0 is the default.
    pub wave_zoom: i8,
    /// The large waveforms' height, each step a fifth taller; 0 is the default.
    pub wave_height: i8,
    /// The library's filter chips that are on, one bit per `FILTERS` entry.
    pub filters: u8,
    pub search_focus: bool,
    /// The top bar drew the search box this frame, so the library leaves out its own.
    pub search_in_bar: bool,
    pub hub: crate::hub::Hub,
    pub hub_area: Rect,
    pub shelf: Shelf,
    pub playlists: Vec<(String, Vec<String>)>,
    /// Tags: each a name and the tracks wearing it.
    pub tags: Vec<(String, Vec<String>)>,
    pub history: Vec<u32>,
    /// Tracks put aside for later: each its key, then its title and artist.
    pub sidelist: Vec<(String, Vec<String>)>,
    /// Tracks linked by hand, and those played one after another.
    pub links: crate::related::Pairs,
    pub played: crate::related::Pairs,
    /// Automix's list: each track's key, then its title and artist.
    pub queue: Vec<(String, Vec<String>)>,
    /// Tracks got from the streaming services: the file, then its title, artist and service.
    pub streamed: Vec<(String, Vec<String>)>,
    /// How many tracks the last look through the music folders found.
    pub found: usize,
    /// How many of the entries are mixes recorded here.
    pub mixes: usize,
    /// The collection's health, counted after each look through the folders and when the settings open.
    pub health: library::Health,
    /// Keys moved from where they shipped, and the shipped key of the action now waiting for its new key.
    pub binds: Vec<(char, char)>,
    pub binding: Option<char>,
    /// The list track waiting or playing on each deck; it stays on the list until it has played.
    pub mix_held: [Option<String>; 2],
    pub order_menu: bool,
    /// The menu of the YouTube account's channels is open.
    pub channel_menu: bool,
    /// Which of Automix's settings has its choices open.
    pub mix_menu: Option<crate::settings::Opt>,
    /// The row of Automix's list being dragged, and where the finger is.
    pub queue_drag: Option<(usize, f32)>,
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
    /// The question whether to quit is on screen.
    pub quit_ask: bool,
    /// The held playlist whose menu is open.
    pub list_menu: Option<usize>,
    /// The deck to start once its track has loaded.
    play_on_load: Option<usize>,
    /// Where the track menu was opened.
    pub menu_at: (f32, f32),
    /// The column chooser, from a held or right-clicked column heading.
    pub column_menu: bool,
    /// Columns the DJ hid, one bit per column.
    pub hidden_cols: u8,
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
    /// Started with tracks on its list, so it plays that and not the list on screen.
    pub listed: bool,
    /// Asked to mix now, without waiting for the track's end.
    pub now: bool,
    /// Which deck the bass is cut on through this mix: neither yet, the incoming, the outgoing.
    bass: u8,
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
        let fx = std::array::from_fn(|d| control.as_ref().map_or(([(0, false, 0.5); 3], 1.0), |c| c.fx(d)));
        let mut pads = [[0; 8 * BANKS]; 2];
        for (i, p) in pads.iter_mut().flatten().enumerate() {
            *p = i.min(SAMPLES.len().saturating_sub(1));
        }
        let desk = control.as_ref().map(Control::desk);
        let sounds = crate::store::sounds();
        let mut s = Self {
            desk,
            control,
            telemetry: Telemetry::default(),
            decks: std::array::from_fn(|_| DeckUi::new()),
            crossfader: 0.0,
            samples: vec![None; SAMPLES.len()],
            pads: crate::store::load_pads(SAMPLES.len(), &sounds).unwrap_or(pads),
            sounds,
            bank: [0; 2],
            picker: None,
            take: None,
            cut: None,
            cut_plays: 0,
            picker_scroll: 0.0,
            picker_grid: Picker::default(),
            entries: library::samples().into_iter().chain(streamed(&crate::store::load_streamed())).collect(),
            shown: Vec::new(),
            query: String::new(),
            focus: 0,
            browser: None,
            scroll: 0.0,
            row_h: 22.0,
            library: Rect::new(0.0, 0.0, 0.0, 0.0),
            hits: Vec::with_capacity(128),
            drags: Vec::new(),
            carry: None,
            pull: 0.0,
            jog: [Jog::default(); DECKS],
            last_press: None,
            read,
            jobs: channel(),
            status,
            toast: None,
            tour: None,
            mini: false,
            last_msgs: Default::default(),
            out_rate,
            auto: None,
            view: View::Dj,
            view_at: None,
            show_fx: false,
            show_samplers: false,
            show_waves: true,
            lib_expanded: false,
            wave_zoom: 0,
            wave_height: 0,
            filters: 0,
            search_focus: false,
            search_in_bar: false,
            hub_area: Rect::default(),
            hub: crate::hub::Hub::new(crate::store::dir().unwrap_or_else(std::env::temp_dir).join("youtube_cache")),
            shelf: Shelf::Tracks,
            playlists: crate::store::load_playlists(),
            tags: crate::store::load_tags(),
            history: Vec::new(),
            sidelist: crate::store::load_sidelist(),
            links: crate::store::load_links(),
            played: crate::store::load_played(),
            queue: crate::store::load_queue(),
            streamed: crate::store::load_streamed(),
            found: 0,
            mixes: 0,
            health: library::Health::default(),
            binds: crate::store::load_keys(),
            binding: None,
            mix_held: [None, None],
            order_menu: false,
            channel_menu: false,
            mix_menu: None,
            queue_drag: None,
            track_menu: None,
            settings: crate::settings::Settings::load(),
            fx,
            fx_menu: None,
            fx_edit: None,
            fx_params: [0.5; djsugar_engine::MAX_PARAMETERS],
            hidden_cols: 0,
            side_panel: false,
            side_tab: 0,
            folded: 0,
            near_bpm: 2,
            min_stars: 1,
            chip_menu: None,
            naming: None,
            tag_menu: None,
            name_buf: String::new(),
            prefs: None,
            pref_menu: None,
            app_menu: false,
            prefs_scroll: 0.0,
            prefs_max: 0.0,
            quit: false,
            quit_ask: false,
            list_menu: None,
            column_menu: false,
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
        s.hidden_cols = s.settings.get(crate::settings::Opt::HiddenColumns);
        let panel = s.settings.get(crate::settings::Opt::SidePanel);
        s.side_panel = panel > 0;
        s.side_tab = panel.saturating_sub(1).min(crate::sugar::SIDE_TABS.len() as u8 - 1);
        s.send_curve();
        s.send_phones();
        s.send(Command::VinylStop(s.settings.on(crate::settings::Opt::VinylStop)));
        djsugar_engine::separation::set_on_load(s.settings.on(crate::settings::Opt::Stems));
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
        if self.take.as_ref().is_some_and(|t| self.picker.is_none() || t.seconds() >= djsugar_engine::capture::LONGEST)
        {
            self.sound_rec();
        }
        if self.picker.is_none() && self.cut.is_some() {
            self.cut_close();
        }
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
        for (path, title, artist, deck, service) in self.hub.poll() {
            let key = path.to_string_lossy().into_owned();
            if !self.streamed.iter().any(|t| t.0 == key) {
                self.streamed.push((key, vec![title.clone(), artist.clone(), service.to_string()]));
                crate::store::save_streamed(&self.streamed);
            }
            let at =
                self.entries.iter().position(|e| matches!(&e.source, Source::File(p) if **p == *path)).unwrap_or_else(
                    || {
                        self.entries.push(library::file(path, &title, &artist, service));
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
                    self.decks[d].muted = [false; 4];
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
                (Job::Lyrics(d, path, ly), _) => {
                    if let Some(l) = &mut self.decks[d].loaded
                        && matches!(&l.entry.source, Source::File(p) if *p == path)
                    {
                        l.lyrics = ly;
                    }
                }
                (Job::Scan(found), _) => {
                    self.entries.truncate(SAMPLES.len());
                    self.found = found.len();
                    self.entries.extend(found);
                    // Streamed tracks live outside the music folders, so a scan never finds them.
                    let kept: Vec<_> = streamed(&self.streamed)
                        .filter(|t| !self.entries.iter().any(|e| entry_key(e) == entry_key(t)))
                        .collect();
                    self.entries.extend(kept);
                    self.mixes = self.recorded().len();
                    self.check_health();
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
            if let Some((d, slot)) = self.fx_edit {
                self.fx_params = c.fx_params(d, slot);
            }
        }
        if let Some(at) = self.tour {
            self.tour = Some(crate::tour::advance(self, at));
        }
        for ask in self.desk.as_ref().map(|d| d.poll()).unwrap_or_default() {
            match ask {
                crate::controllers::Ask::Move(by) => self.select_by(by),
                crate::controllers::Ask::Load(d) => self.load_pick(d),
            }
        }
        self.jog_step();
        self.skip_step();
        self.auto_step();
    }

    /// Scratching: a platter under a finger turns its record as the finger turned since the last frame, and is
    /// held still when the finger is. Letting go gives the record back to the deck.
    fn jog_step(&mut self) {
        for d in 0..DECKS {
            let held = self.drags.iter().any(|g| g.target == Some(Target::Jog(d)));
            let mut j = self.jog[d];
            let now = std::time::Instant::now();
            let since = j.moved.map_or(0.0, |at| (now - at).as_secs_f64());
            if !held {
                if j.moved.is_some() {
                    self.send(Command::Scratch(d, None));
                    self.jog[d] = Jog::default();
                }
                continue;
            }
            if j.moved.is_none() {
                j = Jog { turn: 0.0, moved: Some(now), speed: 0.0 };
                self.send(Command::Scratch(d, Some(0.0)));
            } else if j.turn != 0.0 {
                // Over the time since the finger last moved, so touches slower than frames do not stutter.
                j.speed = f64::from(j.turn / std::f32::consts::TAU) * TURN_SECONDS / since.clamp(0.002, 0.05);
                j.turn = 0.0;
                j.moved = Some(now);
                self.send(Command::Scratch(d, Some(j.speed)));
            } else if j.speed != 0.0 && since > JOG_STILL {
                j.speed = 0.0;
                self.send(Command::Scratch(d, Some(0.0)));
            }
            self.jog[d] = j;
        }
    }

    fn toggle_auto(&mut self) {
        if let Some(a) = self.auto.take() {
            if let Some((from, ..)) = a.fade {
                self.mix_done(from);
            }
            self.mix_held = [None, None];
            return;
        }
        let mut a = Auto { next: 0, fade: None, listed: !self.queue.is_empty(), now: false, bass: 0 };
        for d in 0..2 {
            if self.decks[d].loaded.is_none()
                && !self.decks[d].loading
                && let Some(i) = self.auto_next(&mut a, d)
            {
                self.load_deck(d, i);
            }
        }
        self.auto = Some(a);
    }

    /// The track Automix loads next: the head of its list, or the next of the list on screen
    /// when it was started with nothing on its own.
    fn auto_next(&mut self, a: &mut Auto, deck: usize) -> Option<usize> {
        if !a.listed {
            let i = *self.shown.get(a.next % self.shown.len().max(1))?;
            a.next += 1;
            return Some(i as usize);
        }
        self.mix_held[deck] = None;
        // The other deck's track is still on the list, so it is passed over once.
        let mut other = self.mix_held[1 - deck].as_deref();
        for (key, _) in &self.queue {
            if other == Some(key.as_str()) {
                other = None;
            } else if let Some(i) = self.entries.iter().position(|e| entry_key(e) == *key) {
                self.mix_held[deck] = Some(key.clone());
                return Some(i);
            }
        }
        None
    }

    /// Takes a deck's track off Automix's list: it has played, or been skipped. Repeat sends it to the end.
    fn mix_off_list(&mut self, deck: usize) {
        let Some(key) = self.mix_held[deck].take() else { return };
        if let Some(at) = self.queue.iter().position(|t| t.0 == key) {
            let track = self.queue.remove(at);
            if self.settings.on(crate::settings::Opt::MixRepeat) {
                self.queue.push(track);
            }
            crate::store::save_queue(&self.queue);
        }
    }

    /// Puts Automix's list in an order. The first track and the ones on the decks stay where they are.
    fn mix_order(&mut self, order: u8) {
        let fact = |s: &Self, key: &str| {
            let meta =
                s.entries.iter().position(|e| entry_key(e) == key).and_then(|i| s.meta.get(i).copied().flatten());
            meta.map_or((0.0, None), |m| (m.bpm, m.key))
        };
        let free: Vec<usize> =
            (1..self.queue.len()).filter(|&n| !self.mix_held.iter().flatten().any(|k| *k == self.queue[n].0)).collect();
        let Some(first) = self.queue.first() else { return };
        let facts: Vec<_> = free.iter().map(|&n| fact(self, &self.queue[n].0)).collect();
        let seed =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64 | 1);
        let places = crate::automix::arrange(order, fact(self, &first.0), &facts, seed);
        let moved: Vec<_> = places.into_iter().map(|k| self.queue[free[k]].clone()).collect();
        for (slot, track) in free.into_iter().zip(moved) {
            self.queue[slot] = track;
        }
        crate::store::save_queue(&self.queue);
    }

    /// Where Automix's mix runs on a deck's track, as parts of its length: the mix to come on the
    /// playing deck, the one under way on the deck coming in.
    pub fn mix_span(&self, d: usize) -> Option<(f32, f32)> {
        let a = self.auto?;
        let len = self.decks[d].length();
        if len <= 0.0 {
            return None;
        }
        match a.fade {
            Some((from, start, mix)) if from != d => {
                Some(((start / len) as f32, ((start + mix) / len).min(1.0) as f32))
            }
            Some(_) => None,
            None if self.telemetry.decks[d].playing => {
                let (start, mix) = self.mix_plan(d)?;
                Some(((start / len) as f32, ((start + mix) / len).min(1.0) as f32))
            }
            None => None,
        }
    }

    /// Where the mix out of a deck's track starts and how long it runs, in its frames. A mix by phrases may end
    /// before the track does; the others run to its end.
    fn mix_plan(&self, d: usize) -> Option<(f64, f64)> {
        use crate::settings::Opt;
        let l = self.decks[d].loaded.as_ref()?;
        let (len, rate) = (l.track.frames.len() as f64, f64::from(l.track.rate));
        let beat = l.grid.map_or(0.5 * rate, |g| g.beat_frames(l.track.rate));
        let (length, style) = (self.settings.get(Opt::MixLength), self.settings.get(Opt::MixStyle));
        if length == crate::automix::PHRASES && style != crate::automix::CUT && l.grid.is_some() {
            let bar = beat * 4.0 / rate;
            if let Some((start, mix)) = crate::automix::phrase_mix(&l.phrases, bar, len / rate, self.mix_intro(1 - d)) {
                return Some((start * rate, mix * rate));
            }
        }
        let mix = crate::automix::frames(length, style, beat, rate).min(len * 0.4);
        Some((len - mix, mix))
    }

    /// The bars from where a deck's track waits to its first phrase change after that.
    fn mix_intro(&self, d: usize) -> Option<f64> {
        let l = self.decks[d].loaded.as_ref()?;
        let at = self.decks[d].seconds(self.telemetry.decks[d].position);
        let change = l.phrases.iter().map(|p| p.start).find(|&s| s > at + 1e-3)?;
        let bar = l.grid?.beat_frames(l.track.rate) * 4.0 / f64::from(l.track.rate);
        Some((change - at) / bar)
    }

    /// What goes with the selected track: the tracks linked to it, then the others it was played with.
    pub fn related(&self) -> (Vec<u32>, Vec<u32>) {
        let Some(e) = self.selected.and_then(|i| self.entries.get(i as usize)) else { return Default::default() };
        let key = entry_key(e);
        let find = |keys: &[String]| -> Vec<u32> {
            keys.iter().filter_map(|k| self.entries.iter().position(|e| entry_key(e) == *k)).map(|i| i as u32).collect()
        };
        let linked = find(crate::related::partners(&self.links, &key));
        let mut played = find(crate::related::partners(&self.played, &key));
        played.retain(|i| !linked.contains(i));
        (linked, played)
    }

    /// Whether the track of the open menu is linked to the selected one; None when there is nothing to link.
    pub fn menu_linked(&self) -> Option<bool> {
        let (i, to) = (self.track_menu?, self.selected?);
        let key = |i: u32| self.entries.get(i as usize).map(entry_key);
        (i != to)
            .then(|| crate::related::paired(&self.links, &key(to).unwrap_or_default(), &key(i).unwrap_or_default()))
    }

    /// The place on Automix's list a row let go at height `y` lands on.
    pub fn queue_slot(&self, y: f32) -> Option<usize> {
        let top = self.rect_of(Target::QueueRow(0))?.y;
        let shown = self.hits.iter().filter(|h| matches!(h.target, Target::QueueRow(_))).count();
        Some(queue_slot(top, shown, y))
    }

    /// What Automix is doing, for its panel: the mix under way, or how long until the next.
    pub fn mix_status(&self) -> String {
        let Some(a) = self.auto else { return String::new() };
        let title = |d: usize| self.decks[d].loaded.as_ref().map_or_else(String::new, |l| l.entry.title.to_string());
        if let Some((from, ..)) = a.fade {
            return format!("Mixing into {}", title(1 - from));
        }
        let Some(live) = (0..2).find(|&d| self.telemetry.decks[d].playing) else { return "Starting".into() };
        if self.decks[1 - live].loading {
            return "Loading the next track".into();
        }
        if self.decks[1 - live].loaded.is_none() {
            return "The last track plays out".into();
        }
        let Some((start, _)) = self.mix_plan(live) else { return String::new() };
        let wait = self.decks[live].seconds((start - self.telemetry.decks[live].position).max(0.0)) as u32;
        format!("Mixes into {} in {}:{:02}", title(1 - live), wait / 60, wait % 60)
    }

    /// Automix holds the crossfader while it is on; says so and returns true when it does.
    fn crossfader_held(&mut self) -> bool {
        if self.auto.is_some() {
            self.status = Some("Automix has the crossfader: stop Automix to take over".into());
        }
        self.auto.is_some()
    }

    /// Hands the mixer back as the DJ left it, once a mix is over or called off.
    fn mix_done(&mut self, from: usize) {
        for d in 0..2 {
            self.send(Command::Kill(d, self.decks[d].kills));
        }
        self.send(Command::Filter(from, self.decks[from].knobs[3] * 2.0 - 1.0));
    }

    /// Loads the next track over the one Automix has waiting.
    fn mix_skip(&mut self) {
        let Some(mut a) = self.auto else { return };
        if a.fade.is_some() {
            return;
        }
        let waiting = (0..2).rev().find(|&d| !self.telemetry.decks[d].playing).unwrap_or(1);
        self.mix_off_list(waiting);
        if let Some(i) = self.auto_next(&mut a, waiting) {
            self.load_deck(waiting, i);
        }
        self.auto = Some(a);
    }

    /// Jumps each playing deck past a part that is not music. A paused deck is left where the DJ put it.
    fn skip_step(&mut self) {
        if !self.settings.on(crate::settings::Opt::SkipNonMusic) {
            return;
        }
        for d in 0..2 {
            let t = self.telemetry.decks[d];
            let Some(l) = &self.decks[d].loaded else { continue };
            let rate = f64::from(l.track.rate);
            if t.playing
                && !self.scrubbing(d)
                && let Some(to) = crate::skips::past(&l.skips, t.position / rate)
            {
                self.send(Command::Seek(d, (to * rate).min(self.decks[d].length())));
            }
        }
    }

    /// Whether the DJ has a hand on deck `d`'s platter or its overview.
    fn scrubbing(&self, d: usize) -> bool {
        self.drags.iter().any(|g| matches!(g.target, Some(Target::Jog(n) | Target::Overview(n)) if n == d))
    }

    fn auto_step(&mut self) {
        let Some(mut a) = self.auto else { return };
        for d in 0..2 {
            let t = &self.telemetry.decks[d];
            if t.playing && self.decks[d].seconds(t.position) >= crate::automix::PLAYED_AFTER {
                self.mix_off_list(d);
            }
        }
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
                && let Some(i) = self.auto_next(&mut a, d)
            {
                self.load_deck(d, i);
            }
            self.auto = Some(a);
            return;
        };
        let other = 1 - live;
        if let Some((from, start, len)) = a.fade {
            let pos = self.telemetry.decks[1 - from].position;
            let t = if len > 0.0 { ((pos - start) / len).clamp(0.0, 1.0) as f32 } else { 1.0 };
            let dir = if from == 0 { 1.0 } else { -1.0 };
            self.crossfade(-dir + 2.0 * dir * t);
            let moves = crate::automix::moves(self.settings.get(crate::settings::Opt::MixStyle), t);
            let bass = moves.bass.iter().position(|&cut| cut).map_or(0, |n| 2 - n as u8);
            if bass != a.bass {
                a.bass = bass;
                for (d, cut) in [from, 1 - from].into_iter().zip(moves.bass) {
                    let mut kills = self.decks[d].kills;
                    kills[0] |= cut;
                    self.send(Command::Kill(d, kills));
                }
            }
            if moves.filter > 0.0 {
                self.send(Command::Filter(from, moves.filter));
            }
            if t >= 1.0 {
                self.send(Command::Play(from, false));
                self.telemetry.decks[from].playing = false;
                a.fade = None;
                a.bass = 0;
                self.mix_done(from);
                if let Some(i) = self.auto_next(&mut a, from) {
                    self.load_deck(from, i);
                }
            }
            self.auto = Some(a);
            return;
        }
        let (Some(l), Some(o), Some((from, mix))) =
            (&self.decks[live].loaded, &self.decks[other].loaded, self.mix_plan(live))
        else {
            return;
        };
        let rate = f64::from(l.track.rate);
        let left = l.track.frames.len() as f64 - self.telemetry.decks[live].position;
        // A mix of no length still has to start the next deck before this one runs out.
        let wait = self.telemetry.decks[live].position < from && left > rate * 0.05;
        if !a.now && wait {
            return;
        }
        let mix = mix.min(left);
        a.now = false;
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
        let online = self.settings.on(crate::settings::Opt::LyricsOnline);
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
                let art = match &entry.source {
                    Source::File(p) if !detail.is_empty() => art(p),
                    _ => None,
                };
                let secs = t.frames.len() as f64 / f64::from(t.rate.max(1));
                let lyrics = match &entry.source {
                    Source::File(p) if !detail.is_empty() => {
                        let found = crate::lyrics::find(p, &entry.artist, &entry.title, secs, false);
                        if found.lines.is_empty() && online {
                            let (tx, p, e) = (tx.clone(), p.clone(), entry.clone());
                            let d = if let Job::Deck(d) = job { d } else { 0 };
                            std::thread::spawn(move || {
                                let ly = crate::lyrics::find(&p, &e.artist, &e.title, secs, true);
                                if !ly.lines.is_empty() {
                                    let _ = tx.send((Job::Lyrics(d, p, ly), None));
                                }
                            });
                        }
                        found
                    }
                    _ => crate::lyrics::Lyrics::default(),
                };
                let skips = match &entry.source {
                    Source::File(path) if !detail.is_empty() => crate::skips::find(path, &t),
                    _ => Vec::new(),
                };
                let phrases =
                    grid.map_or_else(Vec::new, |g| crate::automix::phrases(&detail, DETAIL_FRAMES, &g, t.rate));
                Loaded { entry, track: Arc::new(t), overview, detail, grid, key, lyrics, art, skips, phrases }
            });
            let _ = tx.send((job, loaded));
        });
    }

    pub fn scan(&mut self) {
        let roots = library::scan_roots();
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
            ListOp::Export => {
                let (name, keys) = &self.playlists[n];
                let tracks: Vec<_> = keys
                    .iter()
                    .filter_map(|k| self.entries.iter().find(|e| entry_key(e) == *k))
                    .filter_map(|e| match &e.source {
                        Source::File(p) => Some((e.artist.as_ref(), e.title.as_ref(), p.as_ref())),
                        Source::Sample(_) => None,
                    })
                    .collect();
                self.status = Some(match crate::store::export(name, &tracks) {
                    Some(file) => {
                        if let Some(folder) = file.parent() {
                            crate::open::open(&folder.to_string_lossy());
                        }
                        format!("Written to {}", file.display())
                    }
                    None => "The playlist could not be written".into(),
                });
                return;
            }
            ListOp::Up if n > 0 => self.playlists.swap(n, n - 1),
            ListOp::Down if n + 1 < self.playlists.len() => self.playlists.swap(n, n + 1),
            ListOp::Clear => self.playlists[n].1.clear(),
            ListOp::Rename => {
                self.name_buf.clone_from(&self.playlists[n].0);
                self.naming = Some(Shelf::Playlist(n));
                self.folded &= !(1 << 1);
            }
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

    /// How many tracks sit at each preparation stage.
    pub fn stage_counts(&self) -> [usize; 3] {
        let mut c = [0; 3];
        for e in &self.entries[SAMPLES.len().min(self.entries.len())..] {
            if let Some(v) = self.store.get(&entry_key(e))
                && (1..=3).contains(&v.stage)
            {
                c[usize::from(v.stage) - 1] += 1;
            }
        }
        c
    }

    pub fn search(&mut self, query: String) {
        self.query = query;
        self.scroll = 0.0;
        let q = &self.query;
        let (n, s) = (self.entries.len() as u32, SAMPLES.len() as u32);
        let order: Vec<u32> = match self.shelf {
            Shelf::Tracks => (s..n).collect(),
            Shelf::History => self.history.clone(),
            Shelf::Recordings => self.recorded(),
            Shelf::Tag(t) => {
                let keys = self.tags.get(t).map(|(_, k)| k.as_slice()).unwrap_or_default();
                (s..n).filter(|&i| keys.contains(&entry_key(&self.entries[i as usize]))).collect()
            }
            Shelf::Stage(st) => (s..n)
                .filter(|&i| self.store.get(&entry_key(&self.entries[i as usize])).is_some_and(|v| v.stage == st))
                .collect(),
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
        let near = f32::from(NEAR_BPM[usize::from(self.near_bpm).min(NEAR_BPM.len() - 1)]) / 100.0;
        let min_stars = self.min_stars.max(1);
        let (meta, store, entries) = (&self.meta, &self.store, &self.entries);
        let keep = |i: u32| {
            let m = meta.get(i as usize).copied().flatten();
            let saved = || store.get(&entry_key(&entries[i as usize])).unwrap_or_default();
            (f & 1 == 0 || lead.0.zip(m.map(|m| m.bpm)).is_some_and(|(b, t)| t > 0.0 && (t / b - 1.0).abs() <= near))
                && (f & 2 == 0 || lead.1.zip(m.and_then(|m| m.key)).is_some_and(|(a, b)| a.mixes(b)))
                && (f & 4 == 0 || saved().rating >= min_stars)
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
    fn check_health(&mut self) {
        let tracks = &self.entries[SAMPLES.len().min(self.entries.len())..];
        let keys = self.playlists.iter().chain(&self.tags).flat_map(|(_, k)| k.iter().map(String::as_str));
        self.health = library::health(tracks, keys, entry_key);
    }

    /// The entries that are mixes recorded here.
    fn recorded(&self) -> Vec<u32> {
        let Some(dir) = library::recordings_dir() else { return Vec::new() };
        let from = SAMPLES.len().min(self.entries.len());
        let mix = |e: &library::Entry| matches!(&e.source, Source::File(p) if p.starts_with(&dir));
        (from..self.entries.len()).filter(|&i| mix(&self.entries[i])).map(|i| i as u32).collect()
    }

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
        self.naming.is_some() || self.search_focus || self.browser.is_some() || self.hub_typing()
    }

    fn hub_typing(&self) -> bool {
        self.view == View::Stream && self.hub.focus
    }

    pub fn type_text(&mut self, text: &str) {
        if self.naming.is_some() {
            let room = NAME_MAX.saturating_sub(self.name_buf.chars().count());
            self.name_buf.extend(text.chars().filter(|c| !c.is_control()).take(room));
            return;
        }
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
                let k = crate::settings::resolve(&self.binds, unshift(c).to_ascii_lowercase());
                if let Some(Target::Cue(d)) = k.and_then(keymap) {
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

    /// Quits, after asking where a slip would cost a set: on a phone, where Back is a swipe from the edge, and
    /// anywhere while a deck plays.
    pub fn ask_quit(&mut self) {
        let playing = self.telemetry.decks.iter().any(|d| d.playing);
        if quit_asks(cfg!(any(target_os = "android", target_os = "ios")), playing) {
            self.quit_ask = true;
        } else {
            self.quit = true;
        }
    }

    /// Closes the topmost menu or sheet; false when nothing was open.
    pub fn dismiss(&mut self) -> bool {
        if std::mem::take(&mut self.quit_ask) {
            return true;
        }
        if self.pref_menu.take().is_some() {
            return true;
        }
        if self.prefs.is_some() || self.app_menu {
            self.binding = None;
            self.prefs = None;
            self.app_menu = false;
            self.submenu = None;
            return true;
        }
        let open = self.track_menu.is_some()
            || self.column_menu
            || self.order_menu
            || self.channel_menu
            || self.mix_menu.is_some()
            || self.chip_menu.is_some()
            || self.tag_menu.is_some()
            || self.fx_menu.is_some()
            || self.fx_edit.is_some()
            || self.list_menu.is_some()
            || self.cue_menu.is_some()
            || self.grid_menu.is_some()
            || self.picker.is_some();
        self.track_menu = None;
        self.column_menu = false;
        self.order_menu = false;
        self.channel_menu = false;
        self.mix_menu = None;
        self.chip_menu = None;
        self.tag_menu = None;
        self.fx_menu = None;
        self.fx_edit = None;
        self.list_menu = None;
        self.cue_menu = None;
        self.grid_menu = None;
        self.picker = None;
        self.submenu = None;
        open
    }

    /// Keeps the side panel and its tab for the next start: 0 is closed, else the tab plus one.
    fn save_panels(&mut self) {
        let v = if self.side_panel { self.side_tab + 1 } else { 0 };
        self.settings.set(crate::settings::Opt::SidePanel, v);
    }

    /// A right click: what holding there would open.
    pub fn context(&mut self, x: f32, y: f32) {
        let target = self.hit(x, y);
        let now = std::time::Instant::now();
        let at = now.checked_sub(std::time::Duration::from_millis(LONG_PRESS_MS as u64)).unwrap_or(now);
        self.drags.push(Drag { id: CONTEXT, target, start: (x, y), last: (x, y), moved: false, at });
        self.holds();
        self.drags.retain(|d| d.id != CONTEXT);
        if let Some((what, CONTEXT, at)) = self.carry {
            self.carry = None;
            self.put_down(what, false, at);
        }
    }

    /// Ends typing a playlist's name, keeping it unless cancelled or left empty.
    fn end_naming(&mut self, keep: bool) {
        let Some(what) = self.naming.take() else { return };
        let name = std::mem::take(&mut self.name_buf);
        let name = name.trim();
        if !keep || name.is_empty() {
            return;
        }
        match what {
            Shelf::Playlist(n) if n < self.playlists.len() => {
                self.playlists[n].0 = name.to_string();
                crate::store::save_playlists(&self.playlists);
            }
            Shelf::Tag(n) if n < self.tags.len() => {
                self.tags[n].0 = name.to_string();
                crate::store::save_tags(&self.tags);
            }
            _ => {}
        }
    }

    fn key_press(&mut self, key: Key, m: Mods) {
        if self.naming.is_some() {
            match key {
                Key::Enter => self.end_naming(true),
                Key::Escape => self.end_naming(false),
                Key::Backspace => {
                    self.name_buf.pop();
                }
                _ => {}
            }
            return;
        }
        if let Some(action) = self.binding.take() {
            if let (Key::Char(c), false) = (key, m.ctrl) {
                crate::settings::rebind(&mut self.binds, action, unshift(c).to_ascii_lowercase());
                crate::store::save_keys(&self.binds);
            }
            return;
        }
        if key == Key::Escape && self.dismiss() {
            return;
        }
        // On a phone Escape is Back, and Back with nothing left to close is the way out.
        if key == Key::Escape && cfg!(target_os = "android") {
            self.ask_quit();
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
            n @ '1'..='6' if m.alt => {
                const VIEWS: [View; 6] = [View::Dj, View::Wave, View::Lib, View::Fx, View::Stream, View::Ctl];
                self.press(Target::View(VIEWS[n as usize - '1' as usize]), 0.0, 0.0);
            }
            '5' => self.press(Target::ToggleFx, 0.0, 0.0),
            'q' => self.ask_quit(),
            _ => {}
        }
    }

    /// The deck keys, each with its Shift action.
    fn deck_key(&mut self, c: char, m: Mods) {
        let shift = m.shift || unshift(c) != c;
        let Some(k) = crate::settings::resolve(&self.binds, unshift(c).to_ascii_lowercase()) else { return };
        if let 'b' | 'n' = k {
            let d = usize::from(k == 'n');
            self.decks[d].kills[0] = !self.decks[d].kills[0];
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
            Target::XfadeStep(by) if !self.crossfader_held() => {
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

    /// A deck's filter knob and how much of its effects is heard.
    fn xy_set(&mut self, d: usize, filter: f32, mix: f32) {
        self.decks[d].knobs[3] = filter;
        self.send(Command::Filter(d, filter * 2.0 - 1.0));
        self.fx[d].1 = mix;
        self.send(Command::Fx(d, FxSet::Mix(mix)));
    }

    /// The pad on deck `d`. What it changes is noted first, and an effect turned on if none is, so the pad is heard.
    fn xy_deck(&mut self, d: usize, xy: (f32, f32)) {
        if self.station.before.is_none() {
            let lit = self.fx[d].0.iter().all(|slot| !slot.1);
            if lit {
                self.fx[d].0[0].1 = true;
                self.send(Command::Fx(d, FxSet::On(0, true)));
            }
            self.station.before = Some(Before { deck: d, filter: self.decks[d].knobs[3], mix: self.fx[d].1, lit });
        }
        self.xy_set(d, xy.0, xy.1);
    }

    /// The pad let go: the scene stops, and a deck the pad played goes back to how it was.
    fn xy_let_go(&mut self) {
        if self.station.on {
            self.station.on = false;
            self.send(Command::Station(Set::On(false)));
        }
        if let Some(b) = self.station.before.take() {
            self.xy_set(b.deck, b.filter, b.mix);
            if b.lit {
                self.fx[b.deck].0[0].1 = false;
                self.send(Command::Fx(b.deck, FxSet::On(0, false)));
            }
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
        // A track loaded while the other deck plays is taken to be played with that one.
        if self.telemetry.decks[1 - deck].playing
            && let Some(with) = self.decks[1 - deck].loaded.as_ref().map(|l| entry_key(&l.entry))
            && crate::related::pair(&mut self.played, &with, &entry_key(&self.entries[index]))
        {
            crate::store::save_played(&self.played);
        }
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
        if self.carry.is_none()
            && self.track_menu.is_none()
            && let Some(g) = self.drags.iter().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
        {
            let what = match g.target {
                Some(Target::Library) if g.start.1 > self.library.y + 30.0 => {
                    let row = (g.start.1 - self.library.y - 30.0 + self.scroll) / self.row_h;
                    self.shown.get(row as usize).copied().map(Carried::Track)
                }
                Some(Target::HubList) => {
                    let row = ((g.start.1 - self.hub_area.y + self.hub.scroll) / crate::sugar::HUB_ROW) as usize;
                    (row < self.hub.hits.len()).then_some(Carried::Hit(row))
                }
                _ => None,
            };
            self.carry = what.map(|w| (w, g.id, g.start));
        }
        if self.fx_menu.is_none()
            && let Some(g) = self.drags.iter_mut().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::FxOn(d, slot)) = g.target
        {
            g.moved = true;
            self.fx_menu = Some((d, slot));
            self.menu_at = g.start;
        }
        if self.tag_menu.is_none()
            && let Some(g) = self.drags.iter_mut().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::Shelf(Shelf::Tag(n))) = g.target
        {
            g.moved = true;
            self.tag_menu = Some(n);
            self.menu_at = g.start;
        }
        if self.chip_menu.is_none()
            && let Some(g) = self.drags.iter_mut().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::Filter(n @ (0 | 2))) = g.target
        {
            g.moved = true;
            self.chip_menu = Some(n);
            self.menu_at = g.start;
        }
        if !self.column_menu
            && let Some(g) = self.drags.iter_mut().find(|g| !g.moved && g.at.elapsed().as_millis() >= LONG_PRESS_MS)
            && let Some(Target::Sort(_)) = g.target
        {
            g.moved = true;
            self.column_menu = true;
            self.menu_at = g.start;
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
            self.find_sounds();
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
    fn send_phones(&mut self) {
        let (mix, split) = self.settings.phones();
        let (mode, depth) = self.settings.ducking();
        self.send(Command::Phones(mix, split));
        self.send(Command::Ducking(mode, depth));
    }

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
            Opt::CueMix | Opt::SplitCue | Opt::Ducking | Opt::DuckDepth => self.send_phones(),
            Opt::VinylStop => self.send(Command::VinylStop(v != 0)),
            Opt::Stems => djsugar_engine::separation::set_on_load(v != 0),
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

    /// Looks in the folder again, keeping each pad on its sound though the list has moved under it.
    fn find_sounds(&mut self) {
        let found = crate::store::sounds();
        if found == self.sounds {
            return;
        }
        let built = SAMPLES.len();
        for pad in self.pads.iter_mut().flatten() {
            if let Some(old) = pad.checked_sub(built).and_then(|n| self.sounds.get(n)) {
                *pad = found.iter().position(|new| new.1 == old.1).map_or(0, |n| built + n);
            }
        }
        self.sounds = found;
    }

    /// The sound on the pad being picked, when it is one of the user's own.
    pub fn picked_sound(&self) -> Option<&(String, std::path::PathBuf)> {
        let (d, p) = self.picker?;
        self.pads[d][self.bank[d] * 8 + p].checked_sub(SAMPLES.len()).and_then(|n| self.sounds.get(n))
    }

    /// Starts a take from the microphone, or ends the one running and keeps it as a sound of the user's own.
    fn sound_rec(&mut self) {
        let Some(take) = self.take.take() else {
            #[cfg(target_os = "android")]
            if !crate::android::mic() {
                self.status = Some("Allow the microphone, then record again".into());
                return;
            }
            match djsugar_engine::capture::Capture::start() {
                Ok(take) => self.take = Some(take),
                Err(why) => self.status = Some(why),
            }
            return;
        };
        let rate = take.rate;
        let mut heard = take.stop();
        crate::sounds::tighten(&mut heard, rate);
        if heard.is_empty() {
            self.status = Some("Nothing was heard".into());
        } else {
            self.keep_sound("take", 1, rate, &heard);
        }
    }

    /// Writes a sound among the user's own, puts it on the pad being picked and plays it.
    fn keep_sound(&mut self, name: &str, channels: u16, rate: u32, samples: &[f32]) {
        let Some(dir) = crate::store::dir().map(|d| d.join("samples")) else { return };
        let _ = std::fs::create_dir_all(&dir);
        let path = crate::sounds::free(&dir, name);
        if let Err(e) = std::fs::write(&path, crate::sounds::wav(channels, rate, samples)) {
            self.status = Some(format!("Could not keep the sound: {e}"));
            return;
        }
        self.find_sounds();
        let Some(i) = self.sounds.iter().position(|s| s.1 == path).map(|n| SAMPLES.len() + n) else { return };
        if let Some((d, p)) = self.picker {
            self.pads[d][self.bank[d] * 8 + p] = i;
            crate::store::save_pads(&self.pads, SAMPLES.len(), &self.sounds);
        }
        self.sample(i);
    }

    fn cut_open(&mut self) {
        let Some((_, path)) = self.picked_sound().cloned() else { return };
        match crate::sounds::Cut::open(&path) {
            Ok(cut) => self.cut = Some(cut),
            Err(why) => self.status = Some(why),
        }
    }

    /// Where the chosen part is written to be heard before it is kept.
    fn cut_dir() -> Option<std::path::PathBuf> {
        crate::store::dir().map(|d| d.join("cut"))
    }

    fn cut_play(&mut self) {
        let (Some(cut), Some(dir)) = (&self.cut, Self::cut_dir()) else { return };
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        // A name of its own each time, so the part as it is now is read and not the one before.
        self.cut_plays += 1;
        let name = format!("cut-{}", self.cut_plays);
        let path = dir.join(format!("{name}.wav"));
        if std::fs::write(&path, crate::sounds::wav(2, cut.rate, &cut.part())).is_ok()
            && let Some(c) = &mut self.control
        {
            c.sample(&name, path, 0.9);
        }
    }

    fn cut_save(&mut self) {
        if let Some(cut) = self.cut.take() {
            self.keep_sound(&cut.name, 2, cut.rate, &cut.part());
        }
        self.cut_close();
    }

    fn cut_close(&mut self) {
        self.cut = None;
        if let Some(dir) = Self::cut_dir() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    fn pad(&mut self, deck: usize, pad: usize) {
        self.sample(self.pads[deck % 2][self.bank[deck % 2] * 8 + pad]);
    }

    fn sample(&mut self, i: usize) {
        if let Some((name, path)) = i.checked_sub(SAMPLES.len()).and_then(|n| self.sounds.get(n)).cloned() {
            if let Some(c) = &mut self.control {
                c.sample(&name, path, 0.9);
            }
            return;
        }
        match self.samples.get(i).cloned().flatten() {
            Some(_) => self.play_sample(i),
            None => self.load(Job::Sample(i, true), i),
        }
    }

    /// A STEMS pad: the top row silences a stem or brings it back, the bottom row leaves one playing alone.
    fn stem(&mut self, d: usize, p: usize) {
        let s = p % 4;
        let m = &mut self.decks[d].muted;
        if p < 4 {
            m[s] = !m[s];
        } else {
            let alone = (0..4).all(|i| m[i] == (i != s));
            *m = std::array::from_fn(|i| !alone && i != s);
        }
        for (i, off) in self.decks[d].muted.into_iter().enumerate() {
            self.send(Command::Stem(d, i, !off));
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
            0..6 if self.decks[d].echo == Some(p) => {
                self.decks[d].echo = None;
                self.send(Command::Echo(d, 0.0, out_beat));
                self.send(Command::Fx(d, FxSet::Pad(0, 0.0, false)));
            }
            0..5 => {
                self.decks[d].echo = Some(p);
                self.send(Command::Echo(d, 0.6, out_beat * ECHO_BEATS[p]));
                self.send(Command::Fx(d, FxSet::Pad(PAD_ECHO, ECHO_BEATS[p] as f32, true)));
            }
            5 => {
                // Echo Out: the echo rings on while the dry track drops away.
                self.decks[d].echo = Some(5);
                self.send(Command::Echo(d, 0.75, out_beat));
                self.send(Command::Fx(d, FxSet::Pad(PAD_ECHO_OUT, 0.0, true)));
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

    /// Where the open slot's parameters start.
    fn fx_defaults(&self) -> FxParams {
        let kind = self.fx_edit.map_or(0, |(d, slot)| self.fx[d].0[slot].0);
        let start = djsugar_engine::parameter_defaults(f64::from(kind));
        std::array::from_fn(|k| start.get(k).copied().unwrap_or(0.5) as f32)
    }

    fn fx_param(&mut self, k: usize, v: f32) {
        let Some((d, slot)) = self.fx_edit else { return };
        self.fx_params[k] = v;
        self.send(Command::Fx(d, FxSet::Param(slot, k, v)));
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
        self.drags.push(Drag { id, target, start: (x, y), last: (x, y), moved: false, at: std::time::Instant::now() });
        if self.naming.is_some() && !matches!(target, Some(Target::NameField)) {
            self.end_naming(true);
        }
        self.search_focus = target == Some(Target::SearchFocus);
        self.hub.focus = target == Some(Target::HubFocus);
        if let Some(t) = target {
            let again = self.last_press.is_some_and(|(last, at)| last == t && at.elapsed() < DOUBLE_TAP);
            self.last_press = (!again).then(|| (t, std::time::Instant::now()));
            if again {
                self.reset(t);
            }
            self.press(t, x, y);
        }
    }

    /// Puts a knob or fader back where it starts out, as a double tap on it asks.
    fn reset(&mut self, t: Target) {
        match t {
            Target::Knob(d, k) => {
                self.decks[d].knobs[k] = 0.5;
                if k == 3 {
                    self.send(Command::Filter(d, 0.0));
                } else {
                    let eq = std::array::from_fn(|b| knob_gain(self.decks[d].knobs[2 - b]));
                    self.send(Command::Eq(d, eq));
                }
            }
            Target::Gain(d) => {
                self.decks[d].gain = 0.5;
                self.send_gain(d);
            }
            Target::Rate(d) => {
                self.decks[d].rate = 0.0;
                self.send(Command::Rate(d, 0.0));
            }
            Target::Crossfader if !self.crossfader_held() => {
                self.crossfader = 0.0;
                let x = self.crossfader_out();
                self.send(Command::Crossfader(x));
            }
            Target::FxMeta(d, slot) => {
                self.fx[d].0[slot].2 = 0.5;
                self.send(Command::Fx(d, FxSet::Meta(slot, 0.5)));
            }
            Target::FxMix(d) => {
                self.fx[d].1 = 1.0;
                self.send(Command::Fx(d, FxSet::Mix(1.0)));
            }
            Target::FxParam(k) => {
                let v = self.fx_defaults()[k];
                self.fx_param(k, v);
            }
            Target::StationTime => {
                self.station.time = 0.5;
                self.send(Command::Station(Set::Params(0.5, self.station.xy.1)));
            }
            Target::Iso(k) => {
                self.station.iso[k] = 0.0;
                self.send(Command::Station(Set::Isolate(self.station.iso)));
            }
            _ => {}
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
                    Mode::Stems => self.stem(d, p),
                }
            }
            Target::Mode(d, m) => {
                if m == Mode::Samples && self.decks[d].mode == m {
                    self.bank[d] = (self.bank[d] + 1) % BANKS;
                }
                self.decks[d].mode = m;
            }
            Target::PickerClose => self.picker = None,
            Target::SoundRec => self.sound_rec(),
            Target::CutOpen => self.cut_open(),
            Target::CutPlay => self.cut_play(),
            Target::CutSave => self.cut_save(),
            Target::CutClose => self.cut_close(),
            Target::AutoDj => self.toggle_auto(),
            Target::TimeLyrics(d) => {
                let Some(l) = &mut self.decks[d].loaded else { return };
                let Source::File(path) = l.entry.source.clone() else { return };
                let lyrics = std::mem::take(&mut l.lyrics);
                let tx = self.jobs.0.clone();
                self.status = Some("Listening for the vocals to time the lyrics".into());
                std::thread::spawn(move || {
                    let back = crate::lyrics::Lyrics { lines: lyrics.lines.clone() };
                    let (ly, said) = match crate::lyrics::time(&path, lyrics) {
                        Ok(ly) => (ly, TIMED.to_string()),
                        Err(why) => (back, why),
                    };
                    let _ = tx.send((Job::Lyrics(d, path, ly), None));
                    let _ = tx.send((Job::Failed(said), None));
                });
            }
            Target::MenuQueue => {
                if let Some(i) = self.track_menu.take() {
                    let e = &self.entries[i as usize];
                    self.queue.push((entry_key(e), vec![e.title.to_string(), e.artist.to_string()]));
                    crate::store::save_queue(&self.queue);
                }
            }
            Target::MenuSide => {
                if let Some(i) = self.track_menu.take() {
                    let e = &self.entries[i as usize];
                    let key = entry_key(e);
                    if !self.sidelist.iter().any(|(k, _)| *k == key) {
                        self.sidelist.push((key, vec![e.title.to_string(), e.artist.to_string()]));
                        crate::store::save_sidelist(&self.sidelist);
                    }
                }
            }
            Target::MenuLink => {
                if let (Some(i), Some(to)) = (self.track_menu.take(), self.selected) {
                    let (a, b) = (entry_key(&self.entries[to as usize]), entry_key(&self.entries[i as usize]));
                    if !crate::related::pair(&mut self.links, &a, &b) {
                        crate::related::unpair(&mut self.links, &a, &b);
                    }
                    crate::store::save_links(&self.links);
                }
            }
            Target::SideLoad(n, d) => {
                let at = self.sidelist.get(n).and_then(|(k, _)| self.entries.iter().position(|e| entry_key(e) == *k));
                match at {
                    Some(i) => self.load_deck(d, i),
                    None => self.status = Some("That track is no longer in the collection".into()),
                }
            }
            Target::SideRemove(n) if n < self.sidelist.len() => {
                self.sidelist.remove(n);
                crate::store::save_sidelist(&self.sidelist);
            }
            Target::SideToQueue => {
                self.queue.append(&mut self.sidelist);
                crate::store::save_queue(&self.queue);
                crate::store::save_sidelist(&self.sidelist);
            }
            Target::SideClear => {
                self.sidelist.clear();
                crate::store::save_sidelist(&self.sidelist);
            }
            Target::RelatedLoad(n, d) => {
                let (linked, played) = self.related();
                if let Some(&i) = linked.iter().chain(&played).nth(n) {
                    self.load_deck(d, i as usize);
                }
            }
            Target::Unlink(n) => {
                if let (Some(to), Some(&i)) = (self.selected, self.related().0.get(n)) {
                    let (a, b) = (entry_key(&self.entries[to as usize]), entry_key(&self.entries[i as usize]));
                    crate::related::unpair(&mut self.links, &a, &b);
                    crate::store::save_links(&self.links);
                }
            }
            Target::QueueRemove(n) if n < self.queue.len() => {
                self.queue.remove(n);
                crate::store::save_queue(&self.queue);
            }
            Target::QueueClear => {
                self.order_menu = false;
                self.queue.clear();
                crate::store::save_queue(&self.queue);
            }
            Target::MixNow => {
                if let Some(a) = &mut self.auto {
                    a.now = true;
                }
            }
            Target::MixSkip => self.mix_skip(),
            Target::OrderMenu => {
                self.order_menu = true;
                // The menu hangs under its button, clear of the row it belongs to.
                self.menu_at = self.rect_of(t).map_or((x, y), |r| (r.x, r.y + r.h + 2.0));
            }
            Target::MixOrder(order) => {
                self.order_menu = false;
                self.mix_order(order);
            }
            Target::MixMenu(o) => {
                self.mix_menu = Some(o);
                self.menu_at = self.rect_of(t).map_or((x, y), |r| (r.x, r.y + r.h + 2.0));
            }
            Target::MixPick(o, v) => {
                self.mix_menu = None;
                self.settings.set(o, v);
            }
            Target::Cycle(o) => {
                let choices = match crate::settings::spec(o).kind {
                    crate::settings::Kind::Switch => 2,
                    crate::settings::Kind::Segmented(c) | crate::settings::Kind::Menu(c) => c.len() as u8,
                };
                self.settings.set(o, (self.settings.get(o) + 1) % choices);
            }
            Target::Quit => {
                self.dismiss();
                self.ask_quit();
            }
            Target::QuitNow => self.quit = true,
            Target::QuitCancel => self.quit_ask = false,
            Target::Tour => {
                self.app_menu = false;
                self.submenu = None;
                self.tour = if self.tour.is_some() { None } else { Some(0) };
            }
            Target::Mini => {
                self.app_menu = false;
                self.submenu = None;
                self.mini = !self.mini;
            }
            Target::TourSkip => self.tour = self.tour.map(|at| (at + 1).min(crate::tour::STEPS.len() - 1)),
            Target::Open(n) => {
                self.app_menu = false;
                let what = match n {
                    0 => Some(crate::open::HOME.to_string()),
                    1 => Some(crate::open::ISSUES.to_string()),
                    // The folder the user's own pad sounds go in, made on first asking.
                    3 => crate::store::dir().map(|d| d.join("samples")).and_then(|d| {
                        std::fs::create_dir_all(&d).ok()?;
                        Some(d.to_string_lossy().into_owned())
                    }),
                    _ => crate::store::dir().map(|d| d.to_string_lossy().into_owned()),
                };
                if !what.is_some_and(|w| crate::open::open(&w)) {
                    self.status = Some("Nothing on this system opens that".into());
                }
            }
            Target::Find => {
                self.app_menu = false;
                self.view = View::Lib;
                self.search_focus = true;
            }
            Target::LoadPick(d) => {
                self.app_menu = false;
                self.load_pick(d);
            }
            Target::Jog(d) => self.focus = d,
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
                self.check_health();
                self.prefs = Some(usize::from(n));
                self.prefs_scroll = 0.0;
                self.app_menu = false;
                self.pref_menu = None;
                self.submenu = None;
            }
            Target::Bind(action) => self.binding = (self.binding != Some(action)).then_some(action),
            Target::BindReset => {
                self.binds.clear();
                self.binding = None;
                crate::store::save_keys(&self.binds);
            }
            Target::PrefsClose => {
                self.binding = None;
                if self.pref_menu.take().is_none() {
                    self.prefs = None;
                }
            }
            // Settings rows act on release, so a drag over them scrolls instead.
            Target::Pref(_) | Target::PrefPick(..) | Target::PrefsScroll => {}
            Target::XyHold => {
                self.station.hold = !self.station.hold;
                if !self.station.hold {
                    self.xy_let_go();
                }
            }
            Target::XyAim => {
                self.xy_let_go();
                self.station.aim = (self.station.aim + 1) % (DECKS + 1);
            }
            Target::XyReset => {
                if self.station.before.is_none()
                    && let Some(d) = self.station.aim.checked_sub(1)
                {
                    self.xy_set(d, 0.5, 0.0);
                }
                self.xy_let_go();
                self.station.xy = (0.5, 0.5);
            }
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
            Target::HubRecent(n) => {
                self.hub.focus = false;
                self.hub.again(usize::from(n));
            }
            Target::HubForget => self.hub.forget(),
            Target::HubLoad(hit, deck) => self.hub.fetch(hit, deck),
            Target::HubSignIn => self.hub.sign_in(),
            Target::HubSignOut => self.hub.sign_out(),
            Target::HubShow(n) => self.hub.show([crate::spotify::LIKED, crate::spotify::LISTS][usize::from(n.min(1))]),
            Target::HubOpen(hit) => self.hub.open(hit),
            Target::HubChannels => {
                self.channel_menu = true;
                self.menu_at = self.rect_of(t).map_or((x, y), |r| (r.x, r.y + r.h + 2.0));
            }
            Target::HubChannel(n) => {
                self.channel_menu = false;
                self.hub.choose(usize::from(n));
            }
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
                self.list_op(self.playlists.len() - 1, ListOp::Rename);
            }
            Target::MenuDeck(d) => {
                if let Some(i) = self.track_menu.take() {
                    self.load_deck(d, i as usize);
                }
            }
            Target::FxOn(d, slot) => {
                let on = !self.fx[d].0[slot].1;
                self.fx[d].0[slot].1 = on;
                self.send(Command::Fx(d, FxSet::On(slot, on)));
            }
            Target::FxName(d, slot) => {
                self.fx_menu = Some((d, slot));
                self.menu_at = (x, y);
            }
            Target::FxEdit(d, slot) => {
                self.fx_menu = None;
                self.submenu = None;
                self.fx_edit = Some((d, slot));
                self.fx_params = self.fx_defaults();
                self.menu_at = (x, y);
            }
            Target::FxPick(d, slot, k) => {
                self.fx_menu = None;
                self.submenu = None;
                self.fx[d].0[slot].0 = k;
                self.send(Command::Fx(d, FxSet::Kind(slot, k)));
                if !self.fx[d].0[slot].1 && k > 0 {
                    self.fx[d].0[slot].1 = true;
                    self.send(Command::Fx(d, FxSet::On(slot, true)));
                }
            }
            Target::MenuTag(t) => {
                if let (Some(i), Some((_, keys))) = (self.track_menu, self.tags.get_mut(t)) {
                    let k = entry_key(&self.entries[i as usize]);
                    match keys.iter().position(|x| *x == k) {
                        Some(at) => {
                            keys.remove(at);
                        }
                        None => keys.push(k),
                    }
                    crate::store::save_tags(&self.tags);
                    if self.shelf == Shelf::Tag(t) {
                        let q = std::mem::take(&mut self.query);
                        self.search(q);
                    }
                }
            }
            Target::NewTag => {
                let keys = self.track_menu.map(|i| vec![entry_key(&self.entries[i as usize])]).unwrap_or_default();
                self.tags.push((format!("Tag {}", self.tags.len() + 1), keys));
                crate::store::save_tags(&self.tags);
                self.track_menu = None;
                self.submenu = None;
                self.name_buf.clone_from(&self.tags[self.tags.len() - 1].0);
                self.naming = Some(Shelf::Tag(self.tags.len() - 1));
                self.folded &= !(1 << 5);
            }
            Target::RenameTag(t) if t < self.tags.len() => {
                self.tag_menu = None;
                self.name_buf.clone_from(&self.tags[t].0);
                self.naming = Some(Shelf::Tag(t));
            }
            Target::DeleteTag(t) if t < self.tags.len() => {
                self.tags.remove(t);
                crate::store::save_tags(&self.tags);
                self.tag_menu = None;
                if let Shelf::Tag(x) = self.shelf {
                    self.shelf = match x.cmp(&t) {
                        std::cmp::Ordering::Equal => Shelf::Tracks,
                        std::cmp::Ordering::Greater => Shelf::Tag(x - 1),
                        std::cmp::Ordering::Less => self.shelf,
                    };
                    let q = std::mem::take(&mut self.query);
                    self.search(q);
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
                self.column_menu = false;
                self.order_menu = false;
                self.channel_menu = false;
                self.mix_menu = None;
                self.chip_menu = None;
                self.tag_menu = None;
                self.fx_menu = None;
                self.fx_edit = None;
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
                    self.scan();
                    if let Some(c) = &mut self.control {
                        let mut rest = Vec::new();
                        c.drain_recording(&mut rest);
                    }
                } else if let Some(dir) = library::recordings_dir()
                    && let Ok(rec) = crate::record::Recorder::start(dir, self.out_rate)
                {
                    self.recorder = Some(rec);
                    self.send(Command::Record(true));
                }
            }
            Target::XfadeStep(by) if !self.crossfader_held() => {
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
            Target::ToggleWaves => self.show_waves = !self.show_waves,
            Target::ToggleExpand => self.lib_expanded = !self.lib_expanded,
            Target::ToggleSide => {
                self.side_panel = !self.side_panel;
                self.save_panels();
            }
            Target::SideTab(n) => {
                self.side_tab = n;
                self.save_panels();
            }
            Target::Fold(b) => self.folded ^= 1 << b,
            Target::MenuStage(n) => {
                if let Some(i) = self.track_menu {
                    let key = entry_key(&self.entries[i as usize]);
                    let mut saved = self.store.get(&key).unwrap_or_default();
                    saved.stage = if saved.stage == n { 0 } else { n };
                    self.store.put(&key, saved);
                    if matches!(self.shelf, Shelf::Stage(_)) {
                        let q = std::mem::take(&mut self.query);
                        self.search(q);
                    }
                }
            }
            Target::ChipSet(chip, v) => {
                if chip == 0 {
                    self.near_bpm = v;
                } else {
                    self.min_stars = v;
                }
                self.filters |= 1 << if chip == 0 { 0 } else { 2 };
                self.chip_menu = None;
                let q = std::mem::take(&mut self.query);
                self.search(q);
            }
            Target::ToggleColumn(c) => {
                self.hidden_cols ^= 1 << c as u8;
                self.settings.set(crate::settings::Opt::HiddenColumns, self.hidden_cols);
            }
            Target::ResetColumns => {
                self.hidden_cols = 0;
                self.settings.set(crate::settings::Opt::HiddenColumns, 0);
            }
            Target::SideCue(d, n) => {
                self.focus = d;
                self.hotcue(d, n);
            }
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
        if let Some(c) = &mut self.carry
            && c.1 == id
        {
            c.2 = (x, y);
            return;
        }
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
        if let Some((what, by, _)) = self.carry
            && by == id
        {
            self.carry = None;
            self.put_down(what, d.moved, if d.moved { d.last } else { d.start });
            return;
        }
        if std::mem::take(&mut self.pull) >= PULL {
            match d.target {
                Some(Target::Library) => self.press(Target::Rescan, 0.0, 0.0),
                Some(Target::HubList) => self.hub.search(),
                _ => {}
            }
        }
        if let Some(Target::Cue(deck)) = d.target {
            self.end_preview(deck);
        }
        if let Some((from, at)) = self.queue_drag.take()
            && d.moved
            && let Some(to) = self.queue_slot(at)
            && from < self.queue.len()
            && to != from
        {
            let row = self.queue.remove(from);
            self.queue.insert(to, row);
            crate::store::save_queue(&self.queue);
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
            Some(Target::Picker) if !d.moved && self.cut.is_none() => {
                let g = self.picker_grid;
                // The list is the last of the picker's areas; the first is the card around it.
                let list = self.hits.iter().rev().find(|h| h.target == Target::Picker).map(|h| h.rect);
                if let (Some((deck, pad)), Some(r)) = (self.picker, list) {
                    let col = ((d.last.0 - r.x) / g.cell.0) as usize;
                    let row = ((y - r.y + self.picker_scroll) / g.cell.1) as usize;
                    let i = row * g.cols + col;
                    if d.last.0 >= r.x && y >= r.y && col < g.cols && i < SAMPLES.len() + self.sounds.len() {
                        self.pads[deck][self.bank[deck] * 8 + pad] = i;
                        self.sample(i);
                        crate::store::save_pads(&self.pads, SAMPLES.len(), &self.sounds);
                    }
                }
            }
            Some(Target::Xy) if !self.station.hold => self.xy_let_go(),
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

    /// The deck whose controls are around a point, for dropping a track on it.
    pub(crate) fn deck_at(&self, x: f32, y: f32) -> Option<usize> {
        self.deck_zones().into_iter().enumerate().find_map(|(d, r)| r.filter(|r| r.hit(x, y)).map(|_| d))
    }

    /// The box around each deck's controls on screen now.
    pub(crate) fn deck_zones(&self) -> [Option<Rect>; 2] {
        let mut zones = [None; 2];
        for h in &self.hits {
            let d = match h.target {
                Target::Jog(d)
                | Target::Overview(d)
                | Target::Play(d)
                | Target::Cue(d)
                | Target::Sync(d)
                | Target::Browse(d)
                | Target::Pad(d, _) => d,
                _ => continue,
            };
            let Some(zone) = zones.get_mut(d) else { continue };
            let r = h.rect;
            *zone = Some(zone.map_or(r, |z: Rect| {
                let (x, y) = (z.x.min(r.x), z.y.min(r.y));
                Rect::new(x, y, (z.x + z.w).max(r.x + r.w) - x, (z.y + z.h).max(r.y + r.h) - y)
            }));
        }
        zones
    }

    /// Lets go of what was carried: on a deck it loads there, on a playlist a track joins it. Never having
    /// moved, a held track opens its menu instead.
    fn put_down(&mut self, what: Carried, moved: bool, (x, y): (f32, f32)) {
        let deck = self.deck_at(x, y);
        match (what, deck) {
            (Carried::Track(i), _) if !moved => {
                self.track_menu = Some(i);
                self.menu_at = (x, y);
                self.submenu = None;
            }
            (Carried::Track(i), Some(d)) => self.load_deck(d, i as usize),
            (Carried::Hit(i), Some(d)) if moved => self.hub.fetch(i, d),
            (Carried::Track(i), None) => {
                let list = self.hits.iter().rev().find_map(|h| match h.target {
                    Target::Shelf(Shelf::Playlist(n)) if h.rect.hit(x, y) => Some(n),
                    _ => None,
                });
                if let Some(n) = list
                    && let Some((name, keys)) = self.playlists.get_mut(n)
                {
                    let k = entry_key(&self.entries[i as usize]);
                    if !keys.contains(&k) {
                        keys.push(k);
                    }
                    self.toast = Some((format!("Added to {name}"), std::time::Instant::now()));
                    crate::store::save_playlists(&self.playlists);
                }
            }
            (Carried::Hit(_), _) => {}
        }
    }

    pub(crate) fn rect_of(&self, t: Target) -> Option<Rect> {
        self.hits.iter().find(|h| h.target == t).map(|h| h.rect)
    }

    fn drag(&mut self, t: Target, x: f32, y: f32, delta: (f32, f32)) {
        let Some(r) = self.rect_of(t) else { return };
        let vertical = |r: Rect| (1.0 - (y - r.y) / r.h).clamp(0.0, 1.0);
        match t {
            Target::QueueRow(n) => self.queue_drag = Some((n, y)),
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
            Target::Crossfader if !self.crossfader_held() => {
                let v = ((x - r.x) / r.w * 2.0 - 1.0).clamp(-1.0, 1.0);
                let v = if v.abs() < 0.04 { 0.0 } else { v };
                self.crossfader = v;
                let x = self.crossfader_out();
                self.send(Command::Crossfader(x));
            }
            Target::FxMeta(d, slot) => {
                let v = (self.fx[d].0[slot].2 - delta.1 / 160.0).clamp(0.0, 1.0);
                self.fx[d].0[slot].2 = v;
                self.send(Command::Fx(d, FxSet::Meta(slot, v)));
            }
            Target::FxParam(k) => self.fx_param(k, ((x - r.x) / r.w).clamp(0.0, 1.0)),
            Target::Cut => {
                let v = ((x - r.x) / r.w).clamp(0.0, 1.0);
                if let Some(c) = &mut self.cut {
                    if (v - c.from).abs() <= (v - c.to).abs() { c.set_from(v) } else { c.set_to(v) }
                }
            }
            Target::FxMix(d) => {
                let v = (self.fx[d].1 - delta.1 / 160.0).clamp(0.0, 1.0);
                self.fx[d].1 = v;
                self.send(Command::Fx(d, FxSet::Mix(v)));
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
                // Near the spindle a small wobble of the finger is a large angle: the motion comes in over the inner
                // third of the platter, so the record never jumps.
                let grip = ((x - cx).hypot(y - cy) / (r.w.min(r.h) / 6.0)).min(1.0);
                self.jog[d].turn += grip * turn;
            }
            Target::Gain(d) => {
                let v = (self.decks[d].gain - delta.1 / 160.0).clamp(0.0, 1.0);
                let v = if (v - 0.5).abs() < 0.02 { 0.5 } else { v };
                self.decks[d].gain = v;
                self.send_gain(d);
            }
            Target::Library if self.scroll <= 0.0 && (delta.1 > 0.0 || self.pull > 0.0) => {
                self.pull = (self.pull + delta.1).max(0.0);
            }
            Target::Library => self.scroll_by(-delta.1),
            Target::Pref(_) | Target::PrefsScroll => self.prefs_scroll_by(-delta.1),
            Target::PrefPick(..) if self.pref_menu.is_none() => self.prefs_scroll_by(-delta.1),
            Target::HubList if self.hub.scroll <= 0.0 && (delta.1 > 0.0 || self.pull > 0.0) => {
                self.pull = (self.pull + delta.1).max(0.0);
            }
            Target::HubList => self.hub_scroll(-delta.1),
            Target::Picker => {
                let g = self.picker_grid;
                let rows = (SAMPLES.len() + self.sounds.len()).div_ceil(g.cols.max(1)) as f32;
                // The list is the last of the picker's areas; the first is the card around it.
                let list = self.hits.iter().rev().find(|h| h.target == t).map_or(r.h, |h| h.rect.h);
                let max = (rows * g.cell.1 - list).max(0.0);
                self.picker_scroll = (self.picker_scroll - delta.1).clamp(0.0, max);
            }
            Target::Xy => {
                let xy = (((x - r.x) / r.w).clamp(0.0, 1.0), (1.0 - (y - r.y) / r.h).clamp(0.0, 1.0));
                self.station.xy = xy;
                if let Some(d) = self.station.aim.checked_sub(1) {
                    self.xy_deck(d, xy);
                    return;
                }
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

/// Whether quitting asks first: always on a phone, elsewhere only while a deck plays.
fn quit_asks(phone: bool, playing: bool) -> bool {
    phone || playing
}

#[cfg(test)]
mod tests {
    use super::quit_asks;

    #[test]
    fn quitting_asks_on_a_phone_and_while_a_deck_plays() {
        assert!(quit_asks(true, false), "a phone with silent decks");
        assert!(quit_asks(true, true));
        assert!(quit_asks(false, true), "a desk with a deck playing");
        assert!(!quit_asks(false, false), "a silent desk quits at once");
    }
}
