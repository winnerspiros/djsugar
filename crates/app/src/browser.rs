//! The browser: folder tree, track list and Automix / Sidelist panel.
//!
//! The collection is the saved library (see `djsugar_library`): the scanned
//! music folder, or the built-in sample
//! library when there is none. The DJ's lists and folders, ratings,
//! colors and play history are kept in the library database. With the Spotify source
//! the tree lists the user's Spotify playlists, and their tracks play when the
//! collection has them (see [`crate::spotify`]).

use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::key::{self, Key};
use djsugar_engine::scan::{is_audio, read_track};
use djsugar_engine::{deck_group, AudioEngine, Hotcue, Track, NUM_DECKS, NUM_HOTCUES};
use djsugar_library::dupes::{Case, Group};
use djsugar_library::health::{Area, Check, Facts};
use djsugar_library::query::{self, Context, Indexed, Query};
use djsugar_library::sets::{is_crate, is_imported, CRATE_FOLDER};
use djsugar_library::stages::{Folders, Stage};
use djsugar_library::{Library, PlaylistKind};
use slint::{ComponentHandle, DataTransfer, Model, ModelRc, SharedString, VecModel};

use djsugar_library::Wanted;
use djsugar_plugins::inbox::Mode;
use djsugar_plugins::{ItemKind, StoreItem, StoreQuery};

mod albums;
mod analysis;
mod buy;
mod checklist;
mod cleanup;
pub(crate) mod columns;
mod cues;
mod favorites;
mod filters;
mod health;
mod history;
mod home;
mod inbox_albums;
mod info;
mod keep_going;
mod lyrics;
mod mix_export;
mod pages;
mod picks;
mod playlist_files;
mod prep;
mod rail_order;
mod rekordbox_xml;
mod sets;
mod side_tabs;
mod spotify_export;
mod staging;
mod streamed;
mod tag_chips;
mod track_menu;
mod up_next;
mod usb;
mod views;
mod wanted;

pub use filters::{with_crates, DEFAULT_PINNED};
pub use lyrics::wave::set_shown as set_wave_lyrics;

use crate::analysis_queue::AnalysisQueue;
use crate::art::Art;
use crate::cover_scan::CoverScan;
use crate::files::{self, Found, Place};
use crate::inbox::Inbox;
use crate::settings::show_key;
use crate::spotify::{self, open_in_browser, Event, Request, Service, StreamTrack};
use crate::stores::{self, Stores};
use crate::waves::Waves;
use crate::{
    fmt_time, AppWindow, ColumnUi, FilesUi, Folder, HealthUi, KeyNotation, LibraryEdit, SideRow,
    StoreUi, StreamMatch, TrackDrag, TrackInfo, TrackRow, WantedUi,
};

/// Sources in the rail that have a tree of their own.
const COLLECTION: i32 = 0;
/// Folders on the computer (see [`crate::files`]).
const FILES: i32 = 1;
const SPOTIFY: i32 = 2;
const TIDAL: i32 = 3;
const APPLE_MUSIC: i32 = 4;
const SOUNDCLOUD: i32 = 8;
const MIXCLOUD: i32 = 11;
/// Bandcamp in the rail, with the other stores in its tree.
const STORES: i32 = 5;
/// The albums folder and the mixed sets folder (see [`albums`]); drawn
/// in the rail after Files.
const ALBUMS: i32 = 9;
const MIXED_SETS: i32 = 10;
/// The Groove Box, instrument plugins and inputs (see [`crate::instruments`]).
const INSTRUMENTS: i32 = 12;

/// Whether `source` shows store results.
fn is_stores(source: i32) -> bool {
    source == STORES
}

/// Matches the DJ set by hand for a service's tracks, by
/// [`StreamTrack::match_key`]: the collection track's id, or None.
type Manual = HashMap<String, Option<usize>>;

#[derive(Clone)]
enum Contents {
    All,
    /// Track ids in playlist order.
    List(Vec<usize>),
    /// A smart list: the tracks a search finds (see `djsugar_library::query`).
    Query(String),
    /// Tracks with this grouping.
    Grouping(String),
    /// Tracks tagged with this word (a hashtag).
    Label(String),
    /// A folder shows everything in the lists below it.
    Folder,
    /// A streaming service's playlist, by its key.
    Stream(String),
    /// One of the DJ's lists (or a history session) in the library database.
    Playlist(i64),
    /// A day of history: its sessions' tracks one after another, the
    /// sessions oldest first.
    HistoryDay(Vec<i64>),
    /// Search results from every store but Bandcamp.
    OtherStores,
    /// Search results from one store, by its index in `Stores`, or all.
    Store(Option<usize>),
    /// Tracks the DJ went to buy that haven't arrived yet.
    Wanted,
    /// A folder on the computer, in the Files source: its audio files.
    Dir(PathBuf),
    /// The collection's tracks in a folder and below it, in the Albums
    /// and Mixed Sets sources.
    Under(PathBuf),
    /// An artist's albums in the Albums source: the tracks right in each
    /// of these album folders, album after album.
    Albums(Vec<PathBuf>),
    /// An album downloaded into the Inbox, by its folder (see
    /// `inbox_albums`).
    InboxAlbum(PathBuf),
    /// The tracks at a stage (see [`djsugar_library::stages`]), or only those
    /// in one subfolder of its folder (a triage crate).
    Stage(Stage, Option<PathBuf>),
    /// Collection Health's page, or the tracks one of its checks finds (see
    /// [`djsugar_library::health`]).
    Health(Option<Check>),
    /// The groups of copies of one case, under Collection Health's
    /// Duplicates (see [`djsugar_library::dupes`]).
    Dupes(Case),
    /// One of Collection Health's areas: the tracks of the check (or case
    /// of copies) picked on its chips.
    Area(Area),
    /// The tracks a Collection Health check was told to leave alone.
    Ignored,
}

/// Whether the streaming services that are built (Spotify, TIDAL) can be
/// browsed.
/// The others in the rail say "Coming soon".
pub const STREAMING_ENABLED: bool = true;

/// Whether the stores (Bandcamp, the other stores) and the purchase inbox
/// can be used. While false, their source says "Coming soon".
pub const STORES_ENABLED: bool = false;

/// How many collection tracks the match menu offers.
const MATCH_CANDIDATES: usize = 8;

/// The sources in the rail, in order, as saved when hidden. The rail's
/// `names` and `source-names` in `library.slint` must list the same.
pub const SOURCE_NAMES: [&str; 13] = [
    "Collection",
    "Files",
    "Spotify",
    "TIDAL",
    "Apple Music",
    "Bandcamp",
    "Beatport",
    "OneLibrary",
    "SoundCloud",
    "Albums",
    "Recordings",
    "Mixcloud",
    "Jam",
];

/// Every source in the rail: the built-in ones, then add-ons' own (see
/// [`crate::sources`]).
/// The name Collection Health's rail button is hidden under, beside the
/// hidden sources' names.
const HEALTH_RAIL_NAME: &str = "Collection Health";

pub(crate) fn all_source_names() -> Vec<String> {
    SOURCE_NAMES
        .iter()
        .map(|n| n.to_string())
        .chain(
            crate::sources::source_plugins()
                .into_iter()
                .map(|(_, info)| info.name.to_string()),
        )
        .collect()
}

/// The rail sources hidden on a fresh install: the streaming services not
/// signed in, the stores (v0.6), OneLibrary, and add-on sources that still
/// need setting up. Settings › Sources switches any of them on.
fn default_hidden_sources() -> Vec<String> {
    let mut hidden: Vec<String> = ["Bandcamp", "Beatport", "OneLibrary"]
        .iter()
        .map(|n| n.to_string())
        .collect();
    for &(_, name, id) in STREAMS.iter() {
        let signed_in = crate::spotify::Saved::load(&format!("{id}.json"))
            .is_some_and(|s| !s.refresh_token.is_empty());
        if !signed_in && id != "spotify" {
            hidden.push(name.to_string());
        }
    }
    for (plugin, info) in crate::sources::source_plugins() {
        let config = crate::settings::plugin_config(plugin.manifest().id);
        if crate::sources::needs_setup(&plugin.settings(), &config) {
            hidden.push(info.name.to_string());
        }
    }
    hidden
}

/// A source's tree while another source is shown.
/// What the track menu's undo line takes back.
struct Undo {
    /// The batch of stage changes and file moves in the library.
    batch: Option<i64>,
    /// What the line says.
    text: String,
    /// Tracks to bring back into the library: merged copies.
    restore: Vec<usize>,
    /// The batch of changes to tags to take back (see
    /// [`djsugar_library::cleanup`]).
    changes: Option<i64>,
    /// Missing tracks that were found elsewhere, by id, with the place
    /// they pointed at before.
    relinked: Vec<(usize, PathBuf)>,
}

struct Parked {
    nodes: Vec<Node>,
    current: usize,
    /// Its sort column and direction.
    sort: (i32, bool),
}

/// What the stores have found for the current search.
struct StoreState {
    stores: Stores,
    /// The search the answers are for; older answers are dropped.
    generation: u64,
    query: String,
    /// Each store's answer: results, why there are none, and a link to its
    /// own search page.
    answers: Vec<Option<Answer>>,
    wanted: Vec<Wanted>,
    /// What each row of the list is, while the stores are shown.
    rows: Vec<StoreRow>,
    /// The last download started or failed.
    download: String,
}

/// A store's results, why there are none, and a link to its own search.
type Answer = (Vec<StoreItem>, Option<String>, Option<String>);

#[derive(Clone)]
enum StoreRow {
    Item(usize, StoreItem),
    /// A store's own search page.
    Link(usize, String),
    Wanted(Wanted),
}

#[derive(Clone)]
struct Node {
    name: String,
    depth: i32,
    kind: i32,
    contents: Contents,
    expanded: bool,
    /// The DJ's own list or folder in the library database.
    playlist: Option<i64>,
    /// Shown on the history tab rather than the library tab.
    history: bool,
    /// A folder in Files whose subfolders haven't been read yet.
    lazy: bool,
    /// Drawn with a line above it, setting it apart from what's before.
    separator: bool,
    /// A crate's color, an index into `COLOR_NAMES`, and its icon, an
    /// index into `LIST_ICONS` (0 the crate's own).
    color: Option<usize>,
    icon: usize,
    /// What the list's header calls it, when not its name (a history day
    /// or set in full).
    title: Option<String>,
    /// How many tracks it holds, shown after its name.
    count: Option<usize>,
    /// Shown dimmed: a history set of a single song.
    dim: bool,
    /// Brought in by another app's import, or inside such a folder: shown
    /// under Imported and read-only, since the next import replaces it.
    locked: bool,
    /// The top folder of an import, with the importer's id: its menu
    /// offers to make its lists the DJ's own.
    import_root: Option<String>,
}

impl Node {
    fn new(name: impl Into<String>, depth: i32, kind: i32, contents: Contents) -> Self {
        Node {
            name: name.into(),
            depth,
            kind,
            contents,
            expanded: true,
            playlist: None,
            history: false,
            lazy: false,
            separator: false,
            color: None,
            icon: 0,
            title: None,
            count: None,
            dim: false,
            locked: false,
            import_root: None,
        }
    }

    /// A folder in Files, closed until it's opened.
    fn dir(name: impl Into<String>, path: PathBuf, depth: i32, kind: i32) -> Self {
        Node {
            expanded: false,
            lazy: true,
            ..Node::new(name, depth, kind, Contents::Dir(path))
        }
    }

    fn path(&self) -> Option<&Path> {
        match &self.contents {
            Contents::Dir(p) => Some(p),
            _ => None,
        }
    }
}

/// Reads a Files folder's subfolders into the tree below it and opens it.
fn open_dir(nodes: &mut Vec<Node>, i: usize) {
    if nodes[i].lazy {
        let depth = nodes[i].depth + 1;
        let children: Vec<Node> = nodes[i]
            .path()
            .map(files::subfolders)
            .unwrap_or_default()
            .into_iter()
            .map(|p| Node::dir(files::folder_name(&p), p, depth, KIND_FOLDER))
            .collect();
        nodes.splice(i + 1..i + 1, children);
        nodes[i].lazy = false;
    }
    nodes[i].expanded = true;
}

/// The Files tree: its starting folders, with the folders in `open` opened.
fn files_tree(open: &HashSet<PathBuf>) -> Vec<Node> {
    let mut nodes: Vec<Node> = files::roots()
        .into_iter()
        .map(|r| {
            let kind = match r.place {
                Place::Folder => KIND_FOLDER,
                Place::Drive => KIND_DRIVE,
                Place::Pinned => KIND_PINNED,
                Place::Cloud => KIND_CLOUD,
            };
            Node::dir(r.name, r.path, 0, kind)
        })
        .collect();
    let mut i = 0;
    while i < nodes.len() {
        if nodes[i].path().is_some_and(|p| open.contains(p)) {
            open_dir(&mut nodes, i);
        }
        i += 1;
    }
    nodes
}

/// Opens the Files tree down to `path` and returns its node: the deepest
/// folder on the way there that the tree shows.
fn reveal_dir(nodes: &mut Vec<Node>, path: &Path) -> Option<usize> {
    let mut at = (0..nodes.len())
        .filter(|&i| nodes[i].depth == 0)
        .filter(|&i| nodes[i].path().is_some_and(|r| path.starts_with(r)))
        .max_by_key(|&i| nodes[i].path().map_or(0, |r| r.components().count()))?;
    loop {
        if nodes[at].path() == Some(path) {
            return Some(at);
        }
        open_dir(nodes, at);
        let depth = nodes[at].depth;
        let next = nodes[at + 1..]
            .iter()
            .take_while(|n| n.depth > depth)
            .position(|n| n.depth == depth + 1 && n.path().is_some_and(|p| path.starts_with(p)));
        match next {
            Some(j) => at += 1 + j,
            None => return Some(at),
        }
    }
}

/// Files shown in the Files source that the collection doesn't have get ids
/// from here up (below the library's own ids), and stay out of the
/// collection until they're loaded or added.
const BROWSED_BASE: usize = 800_000;

fn is_browsed(t: &Track) -> bool {
    (BROWSED_BASE..djsugar_library::ID_BASE).contains(&t.id)
}

/// A file picked for a missing track, and the track's id.
type Located = (usize, PathBuf);

fn file_path(t: &Track) -> Option<&Path> {
    match &t.source {
        djsugar_engine::Source::File(p) => Some(p),
        _ => None,
    }
}

/// A tree node for one of the DJ's smart lists.
/// A crate's row, in its color and with its icon.
fn crate_node(p: &djsugar_library::Playlist, depth: i32) -> Node {
    let mut n = Node::new(p.name.clone(), depth, KIND_CRATE, Contents::Playlist(p.id));
    n.playlist = Some(p.id);
    n.color = p.color.and_then(|rgb| {
        djsugar_library::query::COLOR_NAMES
            .iter()
            .position(|&(_, c)| c == rgb)
    });
    n.icon = p
        .icon
        .as_deref()
        .and_then(|name| djsugar_library::LIST_ICONS.iter().position(|&i| i == name))
        .unwrap_or(0);
    n
}

fn smart_node(p: &djsugar_library::Playlist, depth: i32) -> Node {
    let mut n = Node::new(
        p.name.clone(),
        depth,
        KIND_SMART,
        Contents::Query(p.query.clone().unwrap_or_default()),
    );
    n.playlist = Some(p.id);
    n
}

/// Folder kinds the tree draws (see `Folder` in library.slint).
const KIND_FOLDER: i32 = 1;
const KIND_LIST: i32 = 2;
const KIND_SMART: i32 = 3;
const KIND_WANTED: i32 = 4;
const KIND_HISTORY: i32 = 5;
/// A day on the history tab, its sets under it. Its own number: 19 is
/// Play Ready.
const KIND_HISTORY_DAY: i32 = 23;
/// Bandcamp in the stores list, shown with its logo.
const KIND_BANDCAMP: i32 = 6;
/// A drive, and a folder pinned to the top, in Files.
const KIND_DRIVE: i32 = 7;
const KIND_PINNED: i32 = 8;
/// iCloud Drive, Google Drive, Dropbox and the like, in Files.
const KIND_CLOUD: i32 = 9;
/// The inbox and triage stages; the collection stage is drawn as kind 0.
const KIND_INBOX: i32 = 10;
const KIND_TRIAGE: i32 = 11;
/// Collection Health, and one of its checks.
const KIND_HEALTH: i32 = 12;
/// One of Collection Health's areas, drawn as a dot in its state's colour.
const KIND_AREA: i32 = 21;
/// A set: a folder made for one gig, drawn in lilac.
/// One of the DJ's crates, and the Crates and Playlists headings.
const KIND_CRATE: i32 = 17;
const KIND_SECTION: i32 = 18;
/// The Play Ready stage, green in the sidebar.
const KIND_READY: i32 = 19;
/// An album's folder, and a folder of mixed sets.
const KIND_ALBUM: i32 = 15;
const KIND_MIX: i32 = 16;
/// An artist in the Albums source, with their albums under them.
const KIND_ARTIST: i32 = 20;
/// Collection Health's Ignored node.
const KIND_IGNORED: i32 = 22;

/// Seconds a track must play on a deck to count as played, for the history.
const PLAYED_AFTER: f64 = 30.0;

/// Watches one deck's hot cues so changes the DJ makes are saved.
#[derive(Default)]
struct CueWatch {
    /// The track on the deck, by id, and when it arrived.
    track: Option<(usize, std::time::Instant)>,
    /// The cues as last saved or loaded.
    saved: Vec<Hotcue>,
    /// Seconds played since it was loaded, and whether that was recorded.
    played: f64,
    recorded: bool,
}

pub struct Browser {
    app: slint::Weak<AppWindow>,
    library: Vec<Arc<Track>>,
    nodes: Vec<Node>,
    /// Indices into `nodes` of the rows currently shown in the tree.
    visible_nodes: Vec<usize>,
    current_node: usize,
    search: String,
    sort_column: i32,
    ascending: bool,
    /// Indices into `library` shown in the list, in display order; `None`
    /// for a Spotify track the collection doesn't have.
    rows: Vec<Option<usize>>,
    /// For each row of a Spotify playlist, where its track is in the
    /// playlist; of an imported list, which of its streamed tracks it is.
    stream_rows: Vec<Option<usize>>,
    /// Streamed tracks standing in the imported list shown.
    placed: streamed::Placeholders,
    /// What the match menu offers, by index into the library.
    match_candidates: Vec<usize>,
    /// Ids (not indices) of the tracks on each deck.
    loaded: [Option<usize>; NUM_DECKS],
    /// Prep's Play Ready, next track waiting for the deck to lock the
    /// finished track's grid: that track's id, the library index of the
    /// track to load next, and when it was asked for.
    prep_pending: Option<(usize, Option<usize>, std::time::Instant)>,
    /// The decks the layout shows: 4 in 4-Deck, else 2. "Load on a free
    /// deck" picks among these.
    decks_shown: usize,
    side: [Vec<usize>; 2],
    /// The Automix list before the last Order pick, for Undo.
    automix_before_order: Option<Vec<usize>>,
    shown_side_tab: usize,
    /// The Linked tab: the track selected in the list (an index into
    /// `library`) and what it shows, tracks linked to it and then those
    /// played next to it in the history. `None` marks a section heading.
    linked_to: Option<usize>,
    linked: Vec<Option<usize>>,
    /// How many of `linked`'s rows (heading included) are linked tracks.
    link_rows: usize,
    engine: Rc<AudioEngine>,
    play: [ControlHandle; NUM_DECKS],
    /// The key each deck plays in, with its shift.
    deck_key: [ControlHandle; NUM_DECKS],
    /// How far, in semitones, tracks are shifted to match keys on load.
    key_match_range: ControlHandle,
    /// The key mixing rule and the reach of the key marks the rows were
    /// made with.
    key_rules: (key::Mixing, i32),
    lib: LibraryControls,
    /// Moving the selection with a controller goes through folders instead of tracks.
    folders_focused: bool,
    /// Tracks loaded with LoadSelectedTrackAndPlay, to start once they arrive.
    play_when_loaded: [Option<usize>; NUM_DECKS],
    /// The streaming services, in the order of [`STREAMS`].
    streams: Vec<Stream>,
    store: StoreState,
    inbox: Inbox,
    /// The Buy sheet, while open.
    buy: Option<buy::BuyState>,
    /// Tracks arrived from the inbox while another source was shown, so
    /// the collection's tree (its Inbox list) needs reading again.
    collection_stale: bool,
    /// The source whose tree is in `nodes`: the collection, Spotify or the stores.
    tree: i32,
    /// The other sources' trees, selections and sort orders while not shown.
    parked: HashMap<i32, Parked>,
    /// Where each list was left, the list shown now, and the place a list
    /// just picked goes back to once its tracks are in.
    views: HashMap<views::ViewKey, views::View>,
    shown_view: Option<views::ViewKey>,
    waiting_view: Option<views::View>,
    /// Keys in Camelot notation rather than notes.
    camelot: bool,
    /// The key notation the rows were written with, to redo them on a change.
    notation: djsugar_engine::key::Notation,
    /// Album art thumbnails for the list.
    art: Art,
    /// Mini waveforms for the list's Waveform column.
    waves: Waves,
    /// The list rows given a mini waveform, and the rows in view then.
    wave_rows: Vec<usize>,
    shown_rows: (i32, i32),
    /// The list as shown, kept so arriving art can fill in single rows.
    list: Rc<VecModel<TrackRow>>,
    /// The rows picked in the list.
    picks: picks::Picks,
    /// The saved library, when it could be opened.
    db: Option<Library>,
    /// Where each track id is in `library`.
    index_of: HashMap<usize, usize>,
    /// Where each file is in `library`, for Files.
    path_index: HashMap<PathBuf, usize>,
    files: FilesState,
    /// Each track's text lowercased for searching, as `library` is ordered.
    indexed: Vec<Indexed>,
    /// Ids of tracks that are in the library more than once.
    duplicates: HashSet<usize>,
    /// The copies grouped by case, worked out (reading their files) the
    /// first time they're needed after the library changes.
    dupe_groups: OnceCell<Vec<Group>>,
    /// Pairs of linked tracks, both ways round.
    links: HashSet<(usize, usize)>,
    /// Tempo and key of the track playing now, for "mixes with deck", and its id.
    deck_ref: Option<(f64, Option<Key>)>,
    deck_track: Option<usize>,
    /// The sidebar shows the history sessions instead of the library tree.
    history_tab: bool,
    /// The collection's tree shows Collection Health's checks, picked in
    /// the rail.
    health_view: bool,
    /// The chip picked on each Collection Health area (an index into
    /// `health::area_chips`); unpicked, the first with tracks.
    area_chip: HashMap<Area, usize>,
    /// Tracks each area found when first opened, for its progress bar.
    area_start: std::cell::RefCell<HashMap<Area, usize>>,
    /// The review card's place among the listed tracks, while it's open.
    review_at: Option<usize>,
    /// Audio files in the stage and music folders, and what the collection
    /// knows of them, to find missing files in; read when first needed.
    relink_files: OnceCell<djsugar_library::relocate::Index>,
    /// Files picked with "Find file…" for a missing track (by id).
    located: (Sender<Located>, Receiver<Located>),
    /// The node selected on the other tab, to go back to.
    other_tab_node: usize,
    /// The list or history session the sort was last set up for: each
    /// opens in its own order (a session as played), and the collection's
    /// sort (kept here) comes back after.
    sorted_list: Option<i64>,
    view_sort: (i32, bool),
    /// The Wanted list's sort column and direction; -1 is newest first.
    wanted_sort: (i32, bool),
    /// What exporting a list to Spotify did last, for that list.
    export_status: Option<(i64, String)>,
    /// This run's history session, made when the first track is played.
    session: Option<i64>,
    /// When the last track counted as played (Unix seconds): an hour
    /// later, or on a new day, the next play starts a new set.
    last_play: Option<i64>,
    /// Lists tracks can be added to, as offered in the track menu.
    menu_lists: Vec<track_menu::MenuList>,
    /// The crates each track is in, by track id.
    crates: HashMap<usize, Vec<String>>,
    /// Empty crates and entries for removed tracks.
    list_health: djsugar_library::sets::ListHealth,
    /// Hot cue controls per deck: enabled and position.
    /// Each deck's hot cues: enabled, position and a saved loop's end.
    hotcues: [Vec<(ControlHandle, ControlHandle, ControlHandle)>; NUM_DECKS],
    watch: [CueWatch; NUM_DECKS],
    cues: cues::CueState,
    /// The Prep checklist's ticks and scores, by track id.
    marks: HashMap<usize, djsugar_library::checklist::Marks>,
    /// The checklist's lines, as last read from the settings.
    checklist_setup: RefCell<Option<(std::time::Instant, checklist::Setup)>>,
    /// What the Checklist tab and Prep's count show.
    checklist_shown: Option<checklist::Shown>,
    review: checklist::Review,
    /// The side panel's Info tab.
    info: info::InfoState,
    /// The side panel's Lyrics tab.
    lyrics: lyrics::LyricsState,
    /// The filter bar.
    filter: filters::FilterState,
    /// The track list's columns.
    columns: columns::ColumnState,
    /// The stage folders, as last read from the settings.
    folders: Folders,
    /// The mixed sets folder: its tracks are kept out of the collection.
    mixed_sets: Option<PathBuf>,
    /// A mixed set being made into a Mixcloud MP3.
    mix_export: Option<std::sync::mpsc::Receiver<mix_export::Progress>>,
    /// Play Album was picked, for Automix to play the list gaplessly.
    album_request: bool,
    keep_going: keep_going::KeepGoing,
    pages: pages::Pages,
    /// Up Next's own mixes, by the id of the track mixed into.
    mixes: HashMap<usize, up_next::Picks>,
    mixes_changed: bool,
    /// The Albums source's covers, bigger than the list's thumbnails.
    album_art: Art,
    /// The album folders the covers view shows, in its order, and the
    /// node to go back to from an album opened there.
    album_tiles: Vec<PathBuf>,
    album_back: Option<usize>,
    /// Tempo and key for tracks in triage, found in the background.
    analysis: AnalysisQueue,
    analysis_shown: analysis::Shown,
    /// Export to USB's dialog and export.
    usb: usb::State,
    playlist_files: playlist_files::State,
    inbox_albums: inbox_albums::InboxAlbums,
    rekordbox_xml: rekordbox_xml::State,
    phrases_seen: analysis::SeenDecks,
    /// The last stage change or merge of copies, which the track menu can
    /// take back.
    undo: Option<Undo>,
    /// Where files whose moves are waiting (file moves off) will go, by
    /// track id, so they show in the stage subfolder they're headed for.
    pending_moves: HashMap<usize, PathBuf>,
    /// What Collection Health's checks go by.
    health: Facts,
    /// Looks for covers in the background, for the "No cover" check.
    cover_scan: CoverScan,
    /// What the last Collection Health fix did.
    health_status: String,
    /// How many tracks have been taken out of the library.
    removed: usize,
    /// Merged copies' files in quarantine, or on their way there.
    quarantined: usize,
    /// The clean-up's review table, genres page and cover batch.
    cleanup: cleanup::CleanupState,
}

/// The Files source: the folder listed and the thread reading it.
struct FilesState {
    reader: files::Reader,
    /// The folder listed, its audio files in order, and the listing's
    /// generation (older answers are for folders since left).
    dir: Option<PathBuf>,
    paths: Vec<PathBuf>,
    generation: u64,
    /// Tags are still being read.
    reading: bool,
    /// Listings include the files in subfolders.
    recursive: bool,
    /// The next id for a file the collection doesn't have.
    next_id: usize,
    /// Tracks added so far from the folder being added to the collection.
    added: usize,
}

/// The rail sources that are streaming services: their source, name, and
/// id, which names how matches set by hand are saved for them and the file
/// their sign-in is kept in (`<id>.json` in the config folder).
pub(crate) const STREAMS: [(i32, &str, &str); 5] = [
    (SPOTIFY, "Spotify", "spotify"),
    (TIDAL, "TIDAL", "tidal"),
    (APPLE_MUSIC, "Apple Music", "applemusic"),
    (SOUNDCLOUD, "SoundCloud", "soundcloud"),
    (MIXCLOUD, "Mixcloud", "mixcloud"),
];

/// A streaming service: its worker, and what it has sent.
struct Stream {
    source: i32,
    name: &'static str,
    id: &'static str,
    service: Service,
    state: StreamState,
    user: String,
    /// Progress or an error to show while signing in, or what copying a
    /// playlist did.
    status: String,
    /// Playlists whose tracks go on the Wanted list once they come (Add to
    /// Wanted before they were fetched).
    wanted_later: HashSet<String>,
}

#[derive(Default)]
struct StreamState {
    connected: bool,
    playlists: Vec<spotify::Playlist>,
    tracks: HashMap<String, Vec<StreamTrack>>,
    /// The [`spotify::Playlist::version`] each playlist's tracks were
    /// fetched at.
    versions: HashMap<String, String>,
    /// Playlists whose tracks are up to date: fetched since DJ Sugar started
    /// or since the last refresh, or kept from last time at the version the
    /// service gives now. Others are fetched again in the background when
    /// opened, showing the tracks kept from last time meanwhile.
    fresh: HashSet<String>,
    loading: HashSet<String>,
    errors: HashMap<String, String>,
    /// Matches the DJ set by hand, by [`StreamTrack::match_key`]: the
    /// collection track's id, or None for none of them.
    manual: HashMap<String, Option<usize>>,
}

impl StreamState {
    /// The state after the service listed `playlists` again: tracks kept
    /// for playlists still there, up to date if their version is the same.
    fn merged(old: StreamState, playlists: Vec<spotify::Playlist>) -> Self {
        let keys: HashSet<&str> = playlists.iter().map(|p| p.key.as_str()).collect();
        let mut tracks = old.tracks;
        tracks.retain(|k, _| keys.contains(k.as_str()));
        let mut versions = old.versions;
        versions.retain(|k, _| tracks.contains_key(k));
        let mut fresh = old.fresh;
        for p in &playlists {
            let same = versions.get(&p.key) == Some(&p.version);
            if p.version.is_empty() {
                continue;
            } else if same && tracks.contains_key(&p.key) {
                fresh.insert(p.key.clone());
            } else {
                fresh.remove(&p.key);
            }
        }
        fresh.retain(|k| keys.contains(k.as_str()));
        Self {
            connected: old.connected,
            playlists,
            tracks,
            versions,
            fresh,
            loading: old.loading,
            errors: HashMap::new(),
            manual: old.manual,
        }
    }
}

impl Stream {
    /// Keeps the service's playlists for next time.
    fn keep(&self) {
        crate::stream_cache::store(
            self.id,
            &crate::stream_cache::Kept {
                user: self.user.clone(),
                playlists: self.state.playlists.clone(),
                tracks: self.state.tracks.clone(),
                versions: self.state.versions.clone(),
            },
        );
    }

    /// Shows the playlists kept from last time while the service signs in.
    fn restore(&mut self, manual: HashMap<String, Option<usize>>) {
        let Some(kept) = crate::stream_cache::load(self.id) else {
            return;
        };
        self.user = kept.user;
        self.state = StreamState {
            connected: true,
            playlists: kept.playlists,
            tracks: kept.tracks,
            versions: kept.versions,
            manual,
            ..Default::default()
        };
    }
}

/// What the list shows of a track, from the collection or Spotify.
struct RowData<'a> {
    title: &'a str,
    artist: String,
    album: &'a str,
    genre: &'a str,
    bpm: f64,
    key: &'a str,
    duration: f64,
    rating: u8,
    local: Option<usize>,
    /// A Spotify track: where it is in its playlist.
    stream: Option<usize>,
    /// Where it is in the list shown, and when it was played.
    list: columns::InList,
}

/// Mixxx's library controls, which controller mappings use to browse and load.
pub(crate) struct LibraryControls {
    /// Steps up (negative) or down (positive) through the focused list.
    steps: Vec<(ControlHandle, f64)>,
    focus: [ControlHandle; 3],
    go_to_item: ControlHandle,
    load_first_stopped: ControlHandle,
    load: [ControlHandle; NUM_DECKS],
    load_and_play: [ControlHandle; NUM_DECKS],
    add_to_automix: [ControlHandle; 2],
    /// Opens the Linked tab (the DDJ-FLX4's SHIFT + left LOAD).
    show_linked: ControlHandle,
    folder_steps: [(ControlHandle, f64); 3],
    /// Opens or closes the folder picked in the folders pane.
    toggle_folder: ControlHandle,
    /// Which pane the browse knob moves in, as Mixxx's `focused_widget`:
    /// 2 the folders, 3 the track list.
    focused_widget: ControlHandle,
}

impl LibraryControls {
    pub(crate) fn create(reg: &ControlRegistry) -> Self {
        let lib = |k: &str| reg.free("[Library]", k);
        let playlist = |k: &str| reg.free("[Playlist]", k);
        Self {
            steps: vec![
                (lib("MoveVertical"), 1.0),
                (lib("ScrollVertical"), 1.0),
                (lib("MoveDown"), 1.0),
                (lib("MoveUp"), -1.0),
                (playlist("SelectTrackKnob"), 1.0),
                (playlist("SelectNextTrack"), 1.0),
                (playlist("SelectPrevTrack"), -1.0),
            ],
            focus: [
                lib("MoveFocusForward"),
                lib("MoveFocusBackward"),
                lib("MoveFocus"),
            ],
            go_to_item: lib("GoToItem"),
            load_first_stopped: playlist("LoadSelectedIntoFirstStopped"),
            load: std::array::from_fn(|d| reg.free(&deck_group(d), "LoadSelectedTrack")),
            load_and_play: std::array::from_fn(|d| {
                reg.free(&deck_group(d), "LoadSelectedTrackAndPlay")
            }),
            add_to_automix: [lib("AutoDjAddBottom"), lib("AutoDjAddTop")],
            show_linked: lib("ShowLinked"),
            folder_steps: [
                (playlist("SelectNextPlaylist"), 1.0),
                (playlist("SelectPrevPlaylist"), -1.0),
                (playlist("SelectPlaylist"), 1.0),
            ],
            toggle_folder: playlist("ToggleSelectedSidebarItem"),
            focused_widget: lib("focused_widget"),
        }
    }
}

/// Takes the presses or steps a controller has accumulated in `c`.
fn take(c: &ControlHandle) -> f64 {
    let v = c.get();
    if v != 0.0 {
        c.set(c.get() - v);
    }
    v
}

/// Automix's view of the browser: its Automix list, and the engine to load
/// tracks with.
pub struct AutomixHost<'a> {
    pub browser: &'a mut Browser,
    pub engine: &'a AudioEngine,
}

impl crate::automix::Host for AutomixHost<'_> {
    fn track_at(&self, index: usize) -> Option<Arc<Track>> {
        self.browser.automix_at(index)
    }

    fn remove(&mut self, id: usize) {
        self.browser.remove_from_automix(id);
    }

    fn requeue(&mut self, id: usize) {
        self.browser.requeue_in_automix(id);
    }

    fn queued(&self) -> usize {
        self.browser.automix_len()
    }

    fn load(&mut self, deck: usize, track: Arc<Track>) {
        self.engine.load_track(deck, track);
    }
}

const ROW_PAYLOAD: &str = "djsugar-row:";
/// Rows of a side panel tab: `djsugar-side:<tab>:<rows>`.
const SIDE_PAYLOAD: &str = "djsugar-side:";
/// The track on a deck, picked up by its cover: `djsugar-deck:<deck>`.
const DECK_PAYLOAD: &str = "djsugar-deck:";
/// One of the DJ's lists or folders dragged in the tree: `djsugar-list:<id>`.
const LIST_PAYLOAD: &str = "djsugar-list:";

/// The list or folder a tree drag carries.
fn dragged_list(data: &DataTransfer) -> Option<i64> {
    data.plain_text()
        .ok()?
        .strip_prefix(LIST_PAYLOAD)?
        .parse()
        .ok()
}

/// Where a drag from inside the app comes from.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Dragged {
    /// Rows of the track list, first to last.
    Rows(usize, usize),
    /// Rows of a side panel tab, first to last.
    Side(usize, usize, usize),
    /// The track loaded on a deck.
    Deck(usize),
}

/// One row, or a shift-picked range written as `first-last`.
fn parse_rows(rows: &str) -> Option<(usize, usize)> {
    match rows.split_once('-') {
        Some((first, last)) => {
            let (first, last) = (first.parse().ok()?, last.parse().ok()?);
            (first <= last).then_some((first, last))
        }
        None => rows.parse().ok().map(|row| (row, row)),
    }
}

fn parse_dragged(text: &str) -> Option<Dragged> {
    if let Some(rows) = text.strip_prefix(ROW_PAYLOAD) {
        let (first, last) = parse_rows(rows)?;
        Some(Dragged::Rows(first, last))
    } else if let Some(rest) = text.strip_prefix(SIDE_PAYLOAD) {
        let (tab, rows) = rest.split_once(':')?;
        let (first, last) = parse_rows(rows)?;
        Some(Dragged::Side(tab.parse().ok()?, first, last))
    } else {
        let deck = text.strip_prefix(DECK_PAYLOAD)?.parse().ok()?;
        (deck < NUM_DECKS).then_some(Dragged::Deck(deck))
    }
}

/// What a drag from inside the app carries, if it is one.
fn dragged(data: &DataTransfer) -> Option<Dragged> {
    parse_dragged(&data.plain_text().ok()?)
}

/// Moves `list[first..=last]` to go in before `at` (an index into the list
/// as it was), keeping their order.
/// `base`, or `base 2`, `base 3`… when the DJ already has a list by that name.
fn untaken_name(base: &str, taken: &[&str]) -> String {
    let free = |name: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(name));
    if free(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| free(name))
        .unwrap_or_default()
}

fn move_block<T>(list: &mut Vec<T>, first: usize, last: usize, at: usize) {
    let len = list.len();
    if first >= len {
        return;
    }
    let last = last.min(len - 1);
    let at = at.min(len);
    if (first..=last + 1).contains(&at) {
        return;
    }
    let block: Vec<T> = list.drain(first..=last).collect();
    let at = if at > last { at - block.len() } else { at };
    list.splice(at..at, block);
}

/// A file's name without its extension.
pub fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn dropped_file(data: &DataTransfer) -> Option<PathBuf> {
    data.file_paths()
        .ok()?
        .find(|p| is_audio(p))
        .map(Path::to_path_buf)
}

fn pick(library_len: usize, seed: usize, count: usize) -> Vec<usize> {
    (0..count)
        .map(|i| (seed * 31 + i * 17 + i * i * 7) % library_len)
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut v, x| {
            if !v.contains(&x) {
                v.push(x);
            }
            v
        })
}

/// Lists of the library's groupings and tags: one per grouping and one per
/// hashtag, each a smart list of the tracks that carry it.
fn imported_tree(library: &[Arc<Track>]) -> Vec<Node> {
    let mut groupings: Vec<&str> = Vec::new();
    let mut labels: HashMap<&str, usize> = HashMap::new();
    for t in library {
        let g = t.info.grouping.as_str();
        if !g.is_empty() && !groupings.contains(&g) {
            groupings.push(g);
        }
        for l in &t.info.tags {
            *labels.entry(l.name.as_str()).or_default() += 1;
        }
    }
    if groupings.is_empty() && labels.is_empty() {
        return Vec::new();
    }
    // "11 YOGA" sorts after "3 JNY": by the leading number, then by name.
    groupings.sort_by_key(|g| {
        let n: u32 = g
            .split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(u32::MAX);
        (n, g.to_string())
    });
    let mut labels: Vec<(&str, usize)> = labels.into_iter().collect();
    labels.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let node = |name: String, depth, kind, contents| Node::new(name, depth, kind, contents);
    let mut v = vec![node("Groupings and Tags".into(), 0, 1, Contents::Folder)];
    if !groupings.is_empty() {
        v.push(node("Groupings".into(), 1, 1, Contents::Folder));
        v.extend(
            groupings
                .iter()
                .map(|g| node(g.to_string(), 2, 3, Contents::Grouping(g.to_string()))),
        );
    }
    if !labels.is_empty() {
        v.push(node("Tags".into(), 1, 1, Contents::Folder));
        v.extend(
            labels
                .iter()
                .map(|(l, _)| node(l.to_string(), 2, 3, Contents::Label(l.to_string()))),
        );
    }
    // Tags start folded: there can be many.
    if let Some(tags) = v.iter_mut().find(|n| n.name == "Tags" && n.depth == 1) {
        tags.expanded = false;
    }
    v
}

fn sample_tree(n: usize) -> Vec<Node> {
    let node = |name: &str, depth, kind, contents| Node::new(name, depth, kind, contents);
    let list = |name: &str, depth, seed| {
        node(name, depth, 2, Contents::List(pick(n, seed, 12 + seed % 9)))
    };
    let mut v = vec![
        node("Collection", 0, 0, Contents::All),
        node("My Lists", 0, 1, Contents::Folder),
        list("Warm-up", 1, 1),
        list("Peak Time", 1, 2),
        list("Closing", 1, 3),
        node("Sets", 1, 1, Contents::Folder),
        list("Friday 10-09-26", 2, 4),
        list("Saturday 10-10-26", 2, 5),
        list("Sunday Sessions", 2, 6),
        node("Backing", 1, 1, Contents::Folder),
        list("Ambient", 2, 7),
        list("Edits", 2, 8),
        node("Smart Lists", 0, 1, Contents::Folder),
        node("Mixes with deck", 1, 3, Contents::Query("~deck".into())),
        node("120-124 BPM", 1, 3, Contents::Query("bpm:120-124".into())),
        node("125-128 BPM", 1, 3, Contents::Query("bpm:125-128".into())),
        node("Top rated", 1, 3, Contents::Query("rating:>=4".into())),
    ];
    // Start with "Backing" collapsed so the tree shows both states.
    v[9].expanded = false;
    v
}

impl Browser {
    pub fn install(
        app: &AppWindow,
        library: Vec<Arc<Track>>,
        db: Option<Library>,
        library_path: Option<PathBuf>,
        engine: Rc<AudioEngine>,
        registry: &ControlRegistry,
    ) -> Rc<RefCell<Self>> {
        let inbox_settings = crate::settings::inbox();
        let (inbox_on, inbox_mode) = (inbox_settings.on, inbox_settings.mode);
        let stores = Stores::start(stores::configured());
        let store_count = stores.stores.len();
        let nodes = sample_tree(library.len());
        let b = Rc::new(RefCell::new(Self {
            app: app.as_weak(),
            library: Vec::new(),
            nodes,
            visible_nodes: Vec::new(),
            current_node: 0,
            search: String::new(),
            sort_column: 1,
            ascending: true,
            rows: Vec::new(),
            stream_rows: Vec::new(),
            placed: Default::default(),
            match_candidates: Vec::new(),
            loaded: [None; NUM_DECKS],
            prep_pending: None,
            decks_shown: 2,
            side: [Vec::new(), Vec::new()],
            automix_before_order: None,
            shown_side_tab: 0,
            linked_to: None,
            linked: Vec::new(),
            link_rows: 0,
            engine,
            play: std::array::from_fn(|d| {
                registry.get(&deck_group(d), "play").expect("play control")
            }),
            deck_key: std::array::from_fn(|d| {
                registry.get(&deck_group(d), "key").expect("key control")
            }),
            key_match_range: registry
                .get("[Controls]", "key_match_on_load")
                .expect("key matching control"),
            key_rules: (key::Mixing::Harmonic, 1),
            lib: LibraryControls::create(registry),
            folders_focused: false,
            play_when_loaded: [None; NUM_DECKS],
            streams: STREAMS
                .iter()
                .map(|&(source, name, id)| Stream {
                    source,
                    name,
                    id,
                    service: match source {
                        TIDAL => crate::tidal::start(),
                        MIXCLOUD => crate::mixcloud::start(),
                        SOUNDCLOUD => crate::soundcloud::start(),
                        APPLE_MUSIC => crate::apple_music::start(),
                        _ => Service::spotify(),
                    },
                    state: StreamState::default(),
                    user: String::new(),
                    status: String::new(),
                    wanted_later: HashSet::new(),
                })
                .collect(),
            store: StoreState {
                stores,
                generation: 0,
                query: String::new(),
                answers: vec![None; store_count],
                wanted: Vec::new(),
                rows: Vec::new(),
                download: String::new(),
            },
            inbox: Inbox::start(
                inbox_settings,
                library_path,
                djsugar_engine::scan::default_music_dir(),
                stores::configured(),
            ),
            buy: None,
            collection_stale: false,
            tree: COLLECTION,
            parked: HashMap::new(),
            views: HashMap::new(),
            shown_view: None,
            waiting_view: None,
            camelot: crate::settings::camelot_keys(),
            notation: djsugar_engine::key::notation(),
            art: Art::start(),
            waves: Waves::start(),
            wave_rows: Vec::new(),
            shown_rows: (0, 0),
            list: Rc::new(VecModel::default()),
            picks: Default::default(),
            db,
            index_of: HashMap::new(),
            path_index: HashMap::new(),
            files: FilesState {
                reader: files::Reader::start(),
                dir: None,
                paths: Vec::new(),
                generation: 0,
                reading: false,
                recursive: files::recursive(),
                next_id: BROWSED_BASE,
                added: 0,
            },
            indexed: Vec::new(),
            duplicates: HashSet::new(),
            dupe_groups: OnceCell::new(),
            links: HashSet::new(),
            deck_ref: None,
            deck_track: None,
            history_tab: false,
            health_view: false,
            area_chip: HashMap::new(),
            area_start: Default::default(),
            review_at: None,
            relink_files: OnceCell::new(),
            located: std::sync::mpsc::channel(),
            other_tab_node: 0,
            sorted_list: None,
            view_sort: (1, true),
            wanted_sort: (-1, true),
            export_status: None,
            session: None,
            last_play: None,
            menu_lists: Vec::new(),
            crates: HashMap::new(),
            list_health: Default::default(),
            hotcues: std::array::from_fn(|d| {
                (1..=NUM_HOTCUES)
                    .map(|n| {
                        let c = |k: &str| {
                            registry
                                .get(&deck_group(d), &format!("hotcue_{n}_{k}"))
                                .expect("hotcue control")
                        };
                        (c("enabled"), c("position"), c("endposition"))
                    })
                    .collect()
            }),
            watch: Default::default(),
            cues: cues::CueState::new(registry),
            marks: HashMap::new(),
            checklist_setup: RefCell::new(None),
            checklist_shown: None,
            review: Default::default(),
            info: info::InfoState::new(registry),
            lyrics: lyrics::LyricsState::new(registry),
            filter: filters::FilterState::new(),
            columns: columns::ColumnState::new(&crate::settings::field_names()),
            folders: crate::settings::stage_folders(),
            mixed_sets: crate::settings::mixed_sets_folder(),
            mix_export: None,
            album_request: false,
            keep_going: Default::default(),
            pages: Default::default(),
            mixes: HashMap::new(),
            mixes_changed: false,
            album_art: Art::sized(albums::COVER_SIZE),
            album_tiles: Vec::new(),
            album_back: None,
            analysis: AnalysisQueue::start(),
            analysis_shown: Default::default(),
            usb: Default::default(),
            playlist_files: Default::default(),
            inbox_albums: inbox_albums::InboxAlbums::new(),
            rekordbox_xml: Default::default(),
            phrases_seen: Default::default(),
            undo: None,
            pending_moves: HashMap::new(),
            health: Facts::default(),
            cover_scan: CoverScan::start(),
            health_status: String::new(),
            removed: 0,
            quarantined: 0,
            cleanup: Default::default(),
        }));
        b.borrow_mut().set_library(library);
        if STREAMING_ENABLED {
            b.borrow_mut().restore_streams();
        }
        app.global::<KeyNotation>().set_camelot(b.borrow().camelot);
        {
            let ui = app.global::<StoreUi>();
            ui.set_streaming(STREAMING_ENABLED);
            ui.set_stores(STORES_ENABLED);
            ui.set_inbox_on(inbox_on);
            ui.set_inbox_mode(match inbox_mode {
                Mode::Move => 0,
                Mode::Copy => 1,
                Mode::Leave => 2,
            });
            if b.borrow().inbox.dir.is_none() {
                ui.set_inbox_status("No folder to watch.".into());
            }
        }
        b.borrow_mut().refresh_tree();
        b.borrow_mut().refresh_rows();
        b.borrow_mut().refresh_side();

        let weak = Rc::downgrade(&b);
        let with = move |f: Box<dyn Fn(&mut Browser)>| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        app.on_select_folder(move |i| {
            w(Box::new(move |b| {
                if let Some(&n) = b.visible_nodes.get(i as usize) {
                    b.park_view();
                    b.current_node = n;
                    // An artist opens to show their albums too.
                    if b.nodes[n].kind == KIND_ARTIST {
                        b.nodes[n].expanded = true;
                    }
                    if let Some(app) = b.app.upgrade() {
                        app.global::<LibraryEdit>().set_making_smart(false);
                    }
                    b.health_status.clear();
                    b.close_cleanup();
                    b.close_page();
                    if let Some(app) = b.app.upgrade() {
                        app.set_current_track(-1);
                    }
                    b.refresh_tree();
                    b.refresh_rows();
                    b.fetch_current_playlist();
                }
            }))
        });
        let w = with.clone();
        app.on_toggle_folder(move |i| {
            w(Box::new(move |b| {
                if let Some(&n) = b.visible_nodes.get(i as usize) {
                    if b.nodes[n].lazy {
                        open_dir(&mut b.nodes, n);
                    } else {
                        b.nodes[n].expanded = !b.nodes[n].expanded;
                        b.save_open_folders();
                    }
                    b.refresh_tree();
                }
            }))
        });
        let w = with.clone();
        app.on_sort(move |c| {
            w(Box::new(move |b| {
                if b.shows_wanted() {
                    if wanted::sorts_by(c) {
                        b.wanted_sort = match b.wanted_sort {
                            (col, up) if col == c => (c, !up),
                            _ => (c, true),
                        };
                        b.refresh_rows_keeping_current();
                    }
                    return;
                }
                if b.sort_column == c {
                    b.ascending = !b.ascending;
                } else {
                    b.sort_column = c;
                    b.ascending = true;
                }
                b.refresh_rows_keeping_current();
            }))
        });
        let w = with.clone();
        app.on_search(move |text| {
            let text = text.to_string();
            w(Box::new(move |b| {
                b.search = text.clone();
                if is_stores(b.tree) {
                    b.search_stores(StoreQuery::text(&text));
                }
                b.refresh_rows();
            }))
        });
        let w = with.clone();
        app.on_activate_track(move |row| {
            w(Box::new(move |b| {
                if b.shows_store_rows() {
                    b.activate_store_row(row as usize);
                } else if let Some(&Some(id)) = b.rows.get(row as usize) {
                    b.adopt(&[id]);
                    b.load_to_free_deck(id);
                } else if let Some(t) = b.stream_track(row as usize) {
                    // Not in the collection: it plays on the service's page.
                    if !t.page.is_empty() {
                        open_in_browser(&t.page);
                    }
                }
            }))
        });
        let w = with.clone();
        app.on_load_track(move |deck, row| {
            w(Box::new(move |b| {
                if let Some(&Some(id)) = b.rows.get(row as usize) {
                    if b.playable(id) {
                        b.adopt(&[id]);
                        b.engine.load_track(deck as usize, b.library[id].clone());
                    }
                }
            }))
        });
        // Drag and drop onto a deck. The payload only names the list row, so
        // building it never touches the browser while the list is refreshing.
        let drag = app.global::<TrackDrag>();
        drag.on_payload(|first, last| {
            let rows = if last > first {
                format!("{first}-{last}")
            } else {
                first.to_string()
            };
            DataTransfer::from(SharedString::from(format!("{ROW_PAYLOAD}{rows}")))
        });
        drag.on_side_payload(|tab, first, last| {
            let rows = if last > first {
                format!("{first}-{last}")
            } else {
                first.to_string()
            };
            DataTransfer::from(SharedString::from(format!("{SIDE_PAYLOAD}{tab}:{rows}")))
        });
        drag.on_deck_payload(|deck| {
            DataTransfer::from(SharedString::from(format!("{DECK_PAYLOAD}{deck}")))
        });
        drag.on_accepts(|data| dragged(&data).is_some() || dropped_file(&data).is_some());
        let w = with.clone();
        drag.on_drop(move |deck, data| w(Box::new(move |b| b.load_dropped(deck as usize, &data))));
        let w = with.clone();
        drag.on_drop_side(move |index, data| {
            w(Box::new(move |b| {
                let tab = b.side_tab();
                let at = index.max(0) as usize;
                // Rows dragged within their own list move there.
                if let Some(Dragged::Side(from, first, last)) = dragged(&data) {
                    if from == tab {
                        if tab != LINKED_TAB {
                            move_block(&mut b.side[tab], first, last, at);
                            b.refresh_side();
                        }
                        return;
                    }
                }
                let ids = b.dropped_tracks(&data);
                if ids.is_empty() {
                    return;
                }
                if tab == LINKED_TAB {
                    for id in ids {
                        b.link(id);
                    }
                    return;
                }
                let list = &mut b.side[tab];
                let at = at.min(list.len());
                list.splice(at..at, ids);
                b.refresh_side();
            }))
        });
        let w = with.clone();
        app.global::<LibraryEdit>()
            .on_add_to_automix(move |first, last| {
                w(Box::new(move |b| {
                    let ids = b.playable_rows(first.max(0) as usize, last.max(0) as usize);
                    if !ids.is_empty() {
                        b.side[0].extend(ids);
                        b.refresh_side();
                    }
                }))
            });
        let w = with.clone();
        app.on_side_activate(move |row| {
            w(Box::new(move |b| {
                if let Some(id) = b.side_id(row as usize) {
                    b.load_to_free_deck(id);
                }
            }))
        });
        let w = with.clone();
        app.on_side_remove(move |first, last| {
            w(Box::new(move |b| {
                let tab = b.side_tab();
                let first = first.max(0) as usize;
                let last = last.max(0) as usize;
                if tab == LINKED_TAB {
                    // From the bottom up, so the rows above stay put.
                    for row in (first..=last).rev() {
                        b.unlink_row(row);
                    }
                } else if first < b.side[tab].len() {
                    let end = (last + 1).min(b.side[tab].len());
                    b.side[tab].drain(first..end);
                    b.refresh_side();
                }
            }))
        });
        let w = with.clone();
        app.on_select_source(move |s| w(Box::new(move |b| b.select_source(s))));
        // The sources the rail shows, by name so new ones appear by default.
        // Before any choice is saved, the ones that need an account or a
        // server stay out until they have one.
        let names = all_source_names();
        let hidden = crate::settings::hidden_sources().unwrap_or_else(|| {
            let hidden = default_hidden_sources();
            crate::settings::set_hidden_sources(hidden.clone());
            hidden
        });
        let shown: Vec<bool> = names
            .iter()
            .map(|n| !hidden.iter().any(|h| h == n))
            .collect();
        app.set_sources_shown(std::rc::Rc::new(slint::VecModel::from(shown)).into());
        side_tabs::install(app);
        rail_order::install(app);
        // Collection Health's rail button hides the same way.
        app.global::<HealthUi>()
            .set_rail_shown(!hidden.iter().any(|h| h == HEALTH_RAIL_NAME));
        let weak = app.as_weak();
        app.on_source_shown_changed(move |_, _| {
            let Some(app) = weak.upgrade() else { return };
            let shown = app.get_sources_shown();
            let mut hidden: Vec<String> = names
                .iter()
                .zip(shown.iter())
                .filter(|(_, on)| !on)
                .map(|(n, _)| n.to_string())
                .collect();
            if !app.global::<HealthUi>().get_rail_shown() {
                hidden.push(HEALTH_RAIL_NAME.to_string());
            }
            crate::settings::set_hidden_sources(hidden);
        });
        let ui = app.global::<StoreUi>();
        let w = with.clone();
        ui.on_find(move |row| w(Box::new(move |b| b.find_in_stores(row as usize))));
        let w = with.clone();
        ui.on_set_inbox(move |on| w(Box::new(move |b| b.set_inbox_on(on))));
        let w = with.clone();
        ui.on_set_inbox_mode(move |m| {
            w(Box::new(move |b| {
                let mode = match m {
                    1 => Mode::Copy,
                    2 => Mode::Leave,
                    _ => Mode::Move,
                };
                crate::settings::set_inbox_mode(mode);
                b.inbox.send(crate::inbox::Request::SetMode(mode));
                if let Some(app) = b.app.upgrade() {
                    app.global::<StoreUi>().set_inbox_mode(m);
                }
            }))
        });
        let w = with.clone();
        app.on_spotify_connect(move |id| {
            let id = id.to_string();
            w(Box::new(move |b| {
                let Some(st) = b.shown_stream_mut() else {
                    return;
                };
                st.status = format!("Opening {} in your browser…", st.name);
                // SoundCloud's and Apple Music's forms send more after the
                // client ID (or Team ID); only that stays in the box.
                st.service.client_id = id.split_whitespace().next().unwrap_or_default().into();
                st.service.send(Request::Connect(id.clone()));
                b.show_stream();
            }))
        });
        let w = with.clone();
        app.on_spotify_disconnect(move || {
            w(Box::new(|b| {
                if let Some(st) = b.shown_stream_mut() {
                    st.service.send(Request::Disconnect);
                }
            }))
        });
        // Sign out on Settings › Sources: the service named, shown or not.
        let w = with.clone();
        app.on_stream_sign_out(move |source| {
            w(Box::new(move |b| {
                if let Some(st) = b.streams.iter().find(|s| s.source == source) {
                    st.service.send(Request::Disconnect);
                }
            }))
        });
        let w = with.clone();
        app.on_spotify_copy(move || w(Box::new(|b| b.copy_stream_playlist())));
        let w = with.clone();
        app.on_spotify_refresh(move || w(Box::new(|b| b.refresh_stream())));
        {
            let ui = app.global::<StreamMatch>();
            let w = with.clone();
            ui.on_open(move |row| w(Box::new(move |b| b.open_match(row as usize))));
            let w = with.clone();
            ui.on_search(move |text| {
                let text = text.to_string();
                w(Box::new(move |b| b.search_match(&text)))
            });
            let w = with.clone();
            ui.on_pick(move |row, c| {
                w(Box::new(move |b| {
                    let pick = b.match_candidates.get(c as usize).copied();
                    if pick.is_some() {
                        b.set_match(row as usize, pick, false);
                    }
                }))
            });
            let w = with.clone();
            ui.on_none(move |row| w(Box::new(move |b| b.set_match(row as usize, None, false))));
            let w = with.clone();
            ui.on_auto(move |row| w(Box::new(move |b| b.set_match(row as usize, None, true))));
        }
        Self::install_editing(app, &b);
        Self::install_files(app, &b);
        Self::install_info(app, &b);
        Self::install_picks(app, &b);
        Self::install_wanted(app, &b);
        Self::install_buy(app, &b);
        Self::install_filters(app, &b);
        Self::install_columns(app, &b);
        b
    }

    /// Hooks up the Files source's menus and its subfolders switch.
    fn install_files(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<FilesUi>();
        ui.set_recursive(b.borrow().files.recursive);
        ui.set_file_manager(files::file_manager().into());
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        ui.on_set_recursive(move |on| {
            w(&|b| {
                b.files.recursive = on;
                files::set_recursive(on);
                b.files.dir = None;
                b.list_current_dir();
            })
        });
        let w = with.clone();
        ui.on_pin(move |i| w(&|b| b.toggle_pin(i as usize)));
        let w = with.clone();
        ui.on_add_folder(move |i| {
            w(&|b| {
                if let Some(path) = b.dir_at(i as usize) {
                    b.set_files_status(format!(
                        "Adding {} to the collection…",
                        files::folder_name(&path)
                    ));
                    b.files.reader.add(&path);
                }
            })
        });
        let w = with.clone();
        ui.on_reveal_folder(move |i| {
            w(&|b| {
                if let Some(path) = b.dir_at(i as usize) {
                    files::reveal(&path);
                }
            })
        });
        let w = with.clone();
        ui.on_add_track(move |row| {
            w(&|b| {
                if let Some(&Some(id)) = b.rows.get(row as usize) {
                    b.adopt(&[id]);
                    b.refresh_rows();
                }
            })
        });
        let w = with.clone();
        ui.on_reveal_track(move |row| {
            w(&|b| {
                if let Some(path) = b
                    .rows
                    .get(row as usize)
                    .copied()
                    .flatten()
                    .and_then(|id| file_path(&b.library[id]))
                {
                    files::reveal(path);
                }
            })
        });
        let w = with.clone();
        ui.on_in_collection(move |row| {
            let found = std::cell::Cell::new(false);
            w(&|b| {
                found.set(
                    b.rows
                        .get(row as usize)
                        .copied()
                        .flatten()
                        .is_some_and(|id| !is_browsed(&b.library[id])),
                )
            });
            found.get()
        });
        let w = with.clone();
        ui.on_has_file(move |row| {
            let found = std::cell::Cell::new(false);
            w(&|b| {
                found.set(
                    b.rows
                        .get(row as usize)
                        .copied()
                        .flatten()
                        .is_some_and(|id| file_path(&b.library[id]).is_some()),
                )
            });
            found.get()
        });
    }

    /// Hooks up making, naming and deleting lists, adding tracks to them,
    /// ratings and colors, and the history tab.
    fn install_editing(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let edit = app.global::<LibraryEdit>();
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        edit.on_create(move |what, at| {
            let row = std::cell::Cell::new(-1);
            w(&|b| row.set(b.create_list(what, at)));
            row.get()
        });
        let w = with.clone();
        edit.on_rename(move |i, name| w(&|b| b.rename_node(i as usize, &name)));
        let w = with.clone();
        edit.on_adopt_import(move |i| w(&|b| b.adopt_import_at(i.max(0) as usize)));
        let w = with.clone();
        edit.on_set_crate(move |i, krate| w(&|b| b.set_crate_at(i.max(0) as usize, krate)));
        let w = with.clone();
        edit.on_delete(move |i| w(&|b| b.delete_node(i as usize)));
        let w = with.clone();
        edit.on_set_list_color(move |i, c| {
            w(&|b| b.color_node(i.max(0) as usize, usize::try_from(c).ok()))
        });
        let w = with.clone();
        edit.on_set_list_icon(move |i, n| {
            w(&|b| b.icon_node(i.max(0) as usize, n.max(0) as usize))
        });
        // The open list's own rows are already in it, so only the side
        // panel's rows and the decks' tracks go in by dropping on it.
        edit.on_accepts_list_drop(|data| {
            matches!(dragged(&data), Some(Dragged::Side(..) | Dragged::Deck(_)))
        });
        let w = with.clone();
        edit.on_drop_on_list(move |data| {
            w(&|b| {
                if !matches!(dragged(&data), Some(Dragged::Side(..) | Dragged::Deck(_))) {
                    return;
                }
                let Some(n) = b.nodes.get(b.current_node) else {
                    return;
                };
                if !(n.kind == KIND_LIST || n.kind == KIND_CRATE) || n.locked {
                    return;
                }
                if let Some(list) = n.playlist {
                    let ids = b.dropped_tracks(&data);
                    b.add_to_playlist(&ids, list);
                }
            })
        });
        edit.on_accepts_drop(|_, data| dragged(&data).is_some());
        // Asked while dragging, so it never waits on a busy browser.
        let weak = Rc::downgrade(b);
        let w = weak.clone();
        edit.on_list_payload(move |row| {
            let id = w
                .upgrade()
                .and_then(|b| {
                    b.try_borrow()
                        .ok()
                        .and_then(|b| b.own_list_at(row.max(0) as usize))
                })
                .unwrap_or(-1);
            DataTransfer::from(SharedString::from(format!("{LIST_PAYLOAD}{id}")))
        });
        edit.on_accepts_list_move(move |i, data| {
            let Some(list) = dragged_list(&data) else {
                return false;
            };
            weak.upgrade()
                .and_then(|b| {
                    b.try_borrow()
                        .ok()
                        .map(|b| b.list_move(list, i as usize).is_some())
                })
                .unwrap_or(false)
        });
        let w = with.clone();
        edit.on_drop(move |i, data| {
            w(&|b| {
                if let Some(list) = dragged_list(&data) {
                    b.move_list(list, i as usize);
                } else if dragged(&data).is_some() {
                    let ids = b.dropped_tracks(&data);
                    if let Some(list) = b.own_list_at(i as usize) {
                        b.add_to_playlist(&ids, list);
                    } else if let Some((stage, dir)) = b.stage_at(i as usize) {
                        b.change_stage(&ids, stage, dir.as_deref());
                    }
                }
            })
        });
        let w = with.clone();
        edit.on_set_color(move |row, n| {
            w(&|b| {
                let row = row.max(0) as usize;
                let color = usize::try_from(n)
                    .ok()
                    .and_then(|i| djsugar_library::query::COLOR_NAMES.get(i))
                    .map(|&(_, rgb)| rgb);
                b.color_rows(row, row, color);
            })
        });
        let w = with.clone();
        edit.on_set_rating(move |row, n| {
            w(&|b| {
                let row = row.max(0) as usize;
                b.rate_rows(row, row, n.clamp(0, 5) as u8);
            })
        });
        let w = with.clone();
        edit.on_add_to_list(move |first, last, list| {
            w(&|b| {
                if let Some(id) = b.menu_lists.get(list as usize).map(|l| l.id) {
                    b.add_rows_to_playlist(first.max(0) as usize, last.max(0) as usize, id);
                }
            })
        });
        let w = with.clone();
        edit.on_remove_from_list(move |row| {
            w(&|b| {
                let row = row.max(0) as usize;
                // From the bottom up, so the rows above stay where they are.
                for r in b.chosen_rows(row, row).into_iter().rev() {
                    b.remove_row_from_list(r);
                }
            })
        });
        let w = with.clone();
        edit.on_show_history(move |on| w(&|b| b.show_history(on)));
        let w = with.clone();
        edit.on_save_search(move |text| {
            let row = std::cell::Cell::new(-1);
            w(&|b| row.set(b.save_search(&text)));
            row.get()
        });
        let w = with.clone();
        edit.on_start_smart_list(move || w(&|b| b.start_smart_list()));
        let w = with.clone();
        edit.on_rule_at(move |i| {
            let rule = RefCell::new(String::new());
            w(&|b| *rule.borrow_mut() = b.rule_at(i as usize));
            rule.into_inner().into()
        });
        let w = with.clone();
        edit.on_set_rule(move |i, text| w(&|b| b.set_rule(i as usize, &text)));
        Self::install_staging(&edit, &with);
        Self::install_inbox_albums(&edit, &with);
        Self::install_prep(app, &with);
        Self::install_cues(app, &with);
        Self::install_checklist(app, &with);
        Self::install_lyrics(app, &with);
        Self::install_track_menu(&edit, &with);
        Self::install_keep_going(&app.global::<crate::KeepGoingUi>(), &with);
        Self::install_up_next(&app.global::<crate::UpNextUi>(), &with);
        Self::install_pages(&app.global::<crate::PageUi>(), &with);
        Self::install_mix_export(&app.global::<FilesUi>(), &with);
        Self::install_health(&app.global::<HealthUi>(), &with);
        Self::install_cleanup(&app.global::<crate::CleanupUi>(), &with);
        Self::install_fix_names(app, &with);
        Self::install_analysis(&edit, &with);
        Self::install_albums(&edit, &with);
        Self::install_album_covers(&app.global::<crate::AlbumsUi>(), &with);
        Self::install_usb_export(app, &edit, &with);
        Self::install_spotify_export(&edit, &with);
        Self::install_playlist_files(&edit, &with);
        Self::install_rekordbox_xml(&edit, &with);
    }

    /// Saves a search as a smart list at the top level (under "Smart
    /// Lists"), named after the search, and selects it. Returns its row.
    fn save_search(&mut self, text: &str) -> i32 {
        let text = query::and_text(text.trim(), &self.filter.saved_text());
        if text.is_empty() || !self.saved() || self.tree != COLLECTION || self.history_tab {
            return -1;
        }
        let Some(db) = &self.db else { return -1 };
        let name: String = text.chars().take(40).collect();
        match db.create_smart_list(&name, None, &text) {
            Ok(id) => {
                self.search.clear();
                self.filter.clear_all();
                self.refresh_filter_ui();
                self.reload_tree(Some(id));
                self.visible_nodes
                    .iter()
                    .position(|&n| n == self.current_node)
                    .map_or(-1, |p| p as i32)
            }
            Err(e) => {
                eprintln!("can't save the smart list: {e}");
                -1
            }
        }
    }

    /// Starts a smart playlist from the + on Playlists: shows the whole
    /// collection with the search and filters cleared and the filter bar
    /// on, so the DJ picks filters and saves them as the smart list.
    fn start_smart_list(&mut self) {
        if !self.saved() || self.tree != COLLECTION || self.history_tab {
            return;
        }
        if let Some(n) = self
            .nodes
            .iter()
            .position(|n| matches!(n.contents, Contents::All))
        {
            self.current_node = n;
        }
        self.health_status.clear();
        self.close_cleanup();
        self.search.clear();
        self.filter.clear_all();
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
            app.global::<crate::StoreUi>()
                .set_search_text(SharedString::new());
            app.global::<crate::FilterUi>().set_shown(true);
        }
        self.refresh_tree();
        self.refresh_filter_ui();
        self.refresh_rows();
    }

    /// The search of the smart list at `row` of the tree.
    fn rule_at(&self, row: usize) -> String {
        match self.visible_nodes.get(row).and_then(|&n| self.nodes.get(n)) {
            Some(Node {
                contents: Contents::Query(q),
                ..
            }) => q.clone(),
            _ => String::new(),
        }
    }

    fn set_rule(&mut self, row: usize, text: &str) {
        let (Some(id), Some(db)) = (self.playlist_at(row), &self.db) else {
            return;
        };
        if let Err(e) = db.set_query(id, text) {
            eprintln!("can't change the smart list: {e}");
        }
        self.reload_tree(None);
    }

    /// Shows the tree of the source picked in the rail. Sources without a
    /// tree of their own keep the current one underneath their placeholder.
    fn select_source(&mut self, source: i32) {
        let usable = match source {
            COLLECTION | FILES | ALBUMS | MIXED_SETS | INSTRUMENTS => true,
            SPOTIFY | TIDAL | APPLE_MUSIC | SOUNDCLOUD | MIXCLOUD => STREAMING_ENABLED,
            STORES => STORES_ENABLED,
            _ => false,
        };
        if source == COLLECTION && self.tree == COLLECTION && self.health_view {
            return self.show_health(false);
        }
        if !usable || source == self.tree {
            return;
        }
        self.park_view();
        // Health's checks aren't kept: the collection comes back as itself.
        if self.tree == COLLECTION && self.health_view {
            self.health_view = false;
            self.collection_stale = true;
            self.nodes.clear();
            self.current_node = 0;
        }
        let leaving = Parked {
            nodes: std::mem::take(&mut self.nodes),
            current: self.current_node,
            sort: (self.sort_column, self.ascending),
        };
        self.parked.insert(self.tree, leaving);
        // Spotify playlists and store results start in their own order.
        let back = self.parked.remove(&source).unwrap_or(Parked {
            nodes: Vec::new(),
            current: 0,
            sort: (-1, true),
        });
        self.tree = source;
        (self.nodes, self.current_node) = (back.nodes, back.current);
        (self.sort_column, self.ascending) = back.sort;
        match source {
            FILES => self.rebuild_files_tree(),
            ALBUMS | MIXED_SETS => self.rebuild_folder_tree(source),
            SPOTIFY | TIDAL | APPLE_MUSIC | SOUNDCLOUD | MIXCLOUD => {
                self.rebuild_stream_tree(source)
            }
            STORES => self.rebuild_stores_tree(),
            _ => {}
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
            app.global::<StoreUi>().set_source(source);
        }
        self.show_stream();
        if source == COLLECTION && self.collection_stale {
            self.collection_stale = false;
            self.reload_tree(None);
        }
        self.album_back = None;
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_album_ui();
        self.fetch_current_playlist();
        if is_stores(source) && self.store.query != self.search {
            self.search_stores(StoreQuery::text(&self.search.clone()));
        }
    }

    /// Shows Collection Health's checks in the collection's place (`on`),
    /// or the collection again.
    fn show_health(&mut self, on: bool) {
        if self.tree != COLLECTION {
            if !on {
                return;
            }
            self.select_source(COLLECTION);
        }
        if self.history_tab {
            self.show_history(false);
        }
        if self.health_view == on {
            return;
        }
        self.park_view();
        self.health_view = on;
        self.nodes.clear();
        self.current_node = 0;
        self.close_cleanup();
        self.health_status.clear();
        self.reload_tree(None);
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.fetch_current_playlist();
    }

    /// The stores tree: Bandcamp, the other stores (together and each),
    /// then the wanted list.
    fn rebuild_stores_tree(&mut self) {
        let stores = &self.store.stores.stores;
        let node = |i: usize| {
            let s = &stores[i];
            let kind = if s.name == "Bandcamp" {
                KIND_BANDCAMP
            } else {
                KIND_LIST
            };
            Node::new(s.name.clone(), 0, kind, Contents::Store(Some(i)))
        };
        let (bandcamp, others): (Vec<usize>, Vec<usize>) =
            (0..stores.len()).partition(|&i| stores[i].name == "Bandcamp");
        let mut nodes: Vec<Node> = bandcamp.into_iter().map(node).collect();
        if !others.is_empty() {
            nodes.push(Node::new("All other stores", 0, 0, Contents::OtherStores));
            nodes.extend(others.into_iter().map(node));
        }
        nodes.push(Node::new("Wanted", 0, KIND_WANTED, Contents::Wanted));
        self.current_node = self.current_node.min(nodes.len() - 1);
        self.nodes = nodes;
        self.store.wanted = self
            .db
            .as_ref()
            .and_then(|db| db.wanted().ok())
            .unwrap_or_default();
    }

    /// Asks the stores, unless they're already answering this.
    fn search_stores(&mut self, query: StoreQuery) {
        if query.text == self.store.query && self.store.generation > 0 {
            return;
        }
        self.store.query = query.text.clone();
        self.store.download.clear();
        self.store.answers = vec![None; self.store.stores.stores.len()];
        self.store.generation = self.store.stores.search(query);
    }

    /// "Find in stores" on a track: show the stores, searching for it.
    fn find_in_stores(&mut self, row: usize) {
        let Some(&Some(id)) = self.rows.get(row) else {
            return;
        };
        let t = self.library[id].clone();
        let query = StoreQuery::track(&t.artist, &t.title);
        self.search = query.text.clone();
        if let Some(app) = self.app.upgrade() {
            app.global::<StoreUi>()
                .set_search_text(query.text.clone().into());
        }
        self.store.query.clear();
        self.store.generation = 0;
        if is_stores(self.tree) {
            self.search_stores(query);
            self.refresh_rows();
        } else {
            self.search_stores(query);
            self.select_source(STORES);
        }
    }

    /// Whether the list shows store rows: the stores' results, or the
    /// wanted list wherever it is.
    fn shows_store_rows(&self) -> bool {
        is_stores(self.tree) || self.shows_wanted()
    }

    /// A store row's button: open its page (and remember the track as
    /// wanted), open a store's search page, or download into the inbox.
    fn activate_store_row(&mut self, row: usize) {
        let Some(r) = self.store.rows.get(row).cloned() else {
            return;
        };
        match r {
            StoreRow::Item(store, item) => {
                if let Some(url) = &item.page_url {
                    buy::open_page(url);
                    if item.kind == ItemKind::Track {
                        if let Some(db) = &self.db {
                            let _ = db.add_wanted(&item.store, &item.artist, &item.title, url);
                            self.store.wanted = db.wanted().unwrap_or_default();
                        }
                    }
                } else if item.download.is_some() {
                    match &self.inbox.dir {
                        Some(dir) => self.store.stores.download(store, item, dir.clone()),
                        None => {
                            self.store.download = "There's no inbox folder to download into.".into()
                        }
                    }
                }
            }
            StoreRow::Link(_, url) => {
                open_in_browser(&url);
            }
            StoreRow::Wanted(w) => {
                open_in_browser(&w.page_url);
            }
        }
        self.refresh_rows();
    }

    /// Takes in what the stores and the inbox have done. Called every frame.
    pub fn poll_stores(&mut self, app: &AppWindow) {
        let mut changed = false;
        for event in self.store.stores.poll() {
            match event {
                stores::Event::Results {
                    generation,
                    store,
                    items,
                    error,
                    link,
                } if generation == self.store.generation => {
                    if let Some(a) = self.store.answers.get_mut(store) {
                        *a = Some((items, error, link));
                        changed = true;
                    }
                }
                stores::Event::Results { .. } => {}
                stores::Event::Download(message) => {
                    self.store.download = message;
                    changed = true;
                }
            }
        }
        self.poll_buy();
        for event in self.inbox.poll() {
            match event {
                crate::inbox::Event::Status(s) => {
                    app.global::<StoreUi>().set_inbox_status(s.into())
                }
                crate::inbox::Event::Imported(track) => self.add_imported(*track),
            }
        }
        if changed && self.shows_store_rows() {
            self.refresh_rows();
        }
    }

    /// A track the inbox added to the library joins the list in memory.
    pub fn add_imported(&mut self, track: Track) {
        if self.index_of.contains_key(&track.id) {
            return;
        }
        // A file shown in Files makes way for the collection's copy.
        if let Some(i) = file_path(&track).and_then(|p| self.path_index.get(p).copied()) {
            if is_browsed(&self.library[i]) {
                self.index_of.remove(&self.library[i].id);
                self.index_of.insert(track.id, i);
                self.indexed[i] = Indexed::new(&track);
                self.library[i] = Arc::new(track);
                self.collection_stale = true;
                return;
            }
        }
        // Read its row back so it has everything the library knows.
        self.push_track(track);
        if let Some(db) = &self.db {
            self.store.wanted = db.wanted().unwrap_or_default();
        }
        if self.tree == COLLECTION {
            self.reload_tree(None);
        } else {
            self.collection_stale = true;
            if is_stores(self.tree) {
                self.refresh_rows();
            }
        }
    }

    /// The list while the stores are shown: the selected store's results
    /// (or all of them), with links to each store's own search.
    fn refresh_store_rows(&mut self) {
        let ids = self.columns.ids();
        let selected = self
            .nodes
            .get(self.current_node)
            .map(|n| n.contents.clone());
        let mut rows = Vec::new();
        let mut notice = String::new();
        let mut summary = String::new();
        match selected {
            Some(Contents::Wanted) => {
                if let Some(db) = &self.db {
                    self.store.wanted = db.wanted().unwrap_or_default();
                }
                // A streamed track whose file has since come into the
                // collection some other way isn't wanted any more.
                // Under Collection, the search box narrows the list.
                let needle = if self.tree == COLLECTION {
                    query::fold(&self.search)
                } else {
                    String::new()
                };
                let have: HashSet<(String, String)> = self
                    .library
                    .iter()
                    .filter(|t| !is_browsed(t))
                    .map(|t| (spotify::normalize(&t.artist), spotify::normalize(&t.title)))
                    .collect();
                let mut shown: Vec<_> = self
                    .store
                    .wanted
                    .iter()
                    .filter(|w| {
                        let (artist, title) =
                            (spotify::normalize(&w.artist), spotify::normalize(&w.title));
                        // Untagged files named "Artist - Title" count too.
                        !have.contains(&(artist.clone(), title.clone()))
                            && !have.contains(&(String::new(), artist + &title))
                    })
                    .filter(|w| {
                        needle.is_empty()
                            || [&w.artist, &w.title, &w.store]
                                .iter()
                                .any(|t| query::fold(t).contains(&needle))
                    })
                    .cloned()
                    .collect();
                let (column, ascending) = self.wanted_sort;
                wanted::sort(&mut shown, column, ascending);
                rows.extend(shown.into_iter().map(StoreRow::Wanted));
                if rows.is_empty() && !needle.is_empty() {
                    notice = "Nothing on the Wanted list matches.".into();
                } else if rows.is_empty() {
                    notice = "Tracks you click Buy on, or Add to Wanted on a streaming service, wait here until your collection has them.".into();
                }
            }
            Some(which @ (Contents::Store(_) | Contents::OtherStores)) => {
                let bandcamp = self
                    .store
                    .stores
                    .stores
                    .iter()
                    .position(|s| s.name == "Bandcamp");
                let shown = |i: usize| match &which {
                    Contents::Store(w) => w.is_none_or(|w| w == i),
                    _ => Some(i) != bandcamp,
                };
                let mut errors = Vec::new();
                let mut waiting = false;
                for (i, a) in self
                    .store
                    .answers
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| shown(*i))
                {
                    match a {
                        Some((items, error, _)) => {
                            rows.extend(items.iter().cloned().map(|it| StoreRow::Item(i, it)));
                            if let Some(e) = error {
                                errors.push(e.clone());
                            }
                        }
                        None => waiting = true,
                    }
                }
                // Each store's own search page, after the results.
                for (i, a) in self
                    .store
                    .answers
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| shown(*i))
                {
                    if let Some((_, _, Some(link))) = a {
                        rows.push(StoreRow::Link(i, link.clone()));
                    }
                }
                if self.store.query.is_empty() {
                    notice = "Search the stores above, or right-click a track in your collection and choose Find in stores.".into();
                } else if rows.is_empty() && waiting {
                    notice = "Searching…".into();
                } else if rows.is_empty() {
                    notice = errors
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Nothing found.".into());
                }
                summary = match (waiting && !rows.is_empty(), errors.first()) {
                    (true, _) => "Searching…".into(),
                    (false, Some(e)) if !rows.is_empty() => e.clone(),
                    _ => String::new(),
                };
            }
            _ => {}
        }
        if !self.store.download.is_empty() {
            summary = self.store.download.clone();
        }
        let names: Vec<String> = self
            .store
            .stores
            .stores
            .iter()
            .map(|s| s.name.clone())
            .collect();
        let list: Vec<TrackRow> = rows
            .iter()
            .map(|r| {
                let (title, artist, detail, duration, action) = match r {
                    StoreRow::Item(i, it) => (
                        it.title.clone(),
                        it.artist.clone(),
                        [names[*i].as_str(), &it.release, &it.price]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" · "),
                        it.duration,
                        if it.download.is_some() {
                            "Get"
                        } else if it.kind == ItemKind::Track {
                            "Buy"
                        } else {
                            "Open"
                        },
                    ),
                    StoreRow::Link(i, _) => (
                        format!("Search {} for “{}”", names[*i], self.store.query),
                        String::new(),
                        names[*i].clone(),
                        0.0,
                        "Search",
                    ),
                    StoreRow::Wanted(w) => (
                        w.title.clone(),
                        w.artist.clone(),
                        wanted::detail(w),
                        w.duration,
                        "Buy",
                    ),
                };
                let row = TrackRow {
                    title: title.into(),
                    artist: artist.into(),
                    genre: detail.into(),
                    length: if duration > 0.0 {
                        short_time(duration)
                    } else {
                        SharedString::new()
                    },
                    deck: -1,
                    available: false,
                    action: action.into(),
                    ..Default::default()
                };
                TrackRow {
                    cells: columns::row_cells(
                        &ids,
                        &row,
                        None,
                        None,
                        Default::default(),
                        Default::default(),
                    ),
                    ..row
                }
            })
            .collect();
        self.store.rows = rows;
        self.rows = vec![None; self.store.rows.len()];
        self.clear_picks();
        if let Some(app) = self.app.upgrade() {
            let current = app.get_current_track();
            app.set_folder_duration(summary.into());
            app.set_list_notice(notice.into());
            self.list.set_vec(list);
            app.set_tracks(ModelRc::from(self.list.clone()));
            if self.shows_wanted() {
                app.set_sort_column(self.wanted_sort.0);
                app.set_sort_ascending(self.wanted_sort.1);
            } else {
                app.set_sort_column(-1);
            }
            if current >= self.rows.len() as i32 {
                app.set_current_track(-1);
            }
        }
    }

    /// Lists the Spotify playlists as tree nodes, keeping the selection.
    /// Builds the Files tree afresh (drives come and go, pins change),
    /// keeping the folders that were open and the one selected. The first
    /// time, it opens the folder used last, or the music folder.
    fn rebuild_files_tree(&mut self) {
        let open: HashSet<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| n.expanded && !n.lazy)
            .filter_map(|n| n.path().map(Path::to_path_buf))
            .collect();
        let selected = match self.nodes.get(self.current_node).and_then(Node::path) {
            Some(p) => Some(p.to_path_buf()),
            None if self.nodes.is_empty() => files::last_folder(),
            None => None,
        };
        let mut nodes = files_tree(&open);
        let at = selected
            .and_then(|p| {
                nodes
                    .iter()
                    .position(|n| n.path() == Some(&p))
                    .or_else(|| reveal_dir(&mut nodes, &p))
            })
            .or_else(|| {
                nodes
                    .iter()
                    .position(|n| n.name == "Music" && n.kind == KIND_FOLDER)
            })
            .unwrap_or(0);
        self.nodes = nodes;
        self.current_node = at;
        // Opening the tree down to it shows the folder on screen.
        let mut i = at;
        while let Some(parent) = (0..i)
            .rev()
            .find(|&j| self.nodes[j].depth < self.nodes[i].depth)
        {
            self.nodes[parent].expanded = true;
            i = parent;
        }
    }

    /// The folder at a row of the Files tree.
    fn dir_at(&self, row: usize) -> Option<PathBuf> {
        let n = self.nodes.get(*self.visible_nodes.get(row)?)?;
        n.path().map(Path::to_path_buf)
    }

    /// Pins a folder to the top of the Files tree, or unpins it.
    fn toggle_pin(&mut self, row: usize) {
        let Some(&n) = self.visible_nodes.get(row) else {
            return;
        };
        let Some(path) = self.nodes[n].path().map(Path::to_path_buf) else {
            return;
        };
        let mut pinned = files::pinned();
        if self.nodes[n].kind == KIND_PINNED {
            pinned.retain(|p| *p != path);
        } else if !pinned.contains(&path) {
            pinned.push(path);
        }
        files::set_pinned(&pinned);
        self.rebuild_files_tree();
        self.refresh_tree();
    }

    fn set_files_status(&self, text: String) {
        if let Some(app) = self.app.upgrade() {
            app.global::<FilesUi>().set_status(text.into());
        }
    }

    /// Starts listing the folder selected in Files, unless it's listed.
    fn list_current_dir(&mut self) {
        if self.tree != FILES {
            return;
        }
        let Some(dir) = self.nodes.get(self.current_node).and_then(Node::path) else {
            return;
        };
        if self.files.dir.as_deref() == Some(dir) {
            return;
        }
        let dir = dir.to_path_buf();
        files::set_last_folder(&dir);
        self.files.generation = self.files.reader.list(&dir, self.files.recursive);
        self.files.dir = Some(dir);
        self.files.paths.clear();
        self.files.reading = true;
        self.refresh_rows();
    }

    /// Takes in folder listings and tags read for Files, and folders being
    /// added to the collection. Called every frame.
    pub fn poll_files(&mut self) {
        let mut changed = false;
        for found in self.files.reader.poll() {
            match found {
                Found::Listed { generation, paths } => {
                    if generation == self.files.generation {
                        self.files.paths = paths;
                        changed = true;
                    }
                }
                Found::Read {
                    generation,
                    tracks,
                    done,
                } => {
                    for t in tracks {
                        self.add_browsed(t);
                    }
                    if generation == self.files.generation {
                        changed = true;
                        if done {
                            self.files.reading = false;
                        }
                    }
                }
                Found::ToAdd {
                    known,
                    tracks,
                    done,
                } => {
                    let mut adding: Vec<usize> = known
                        .iter()
                        .filter_map(|p| self.path_index.get(p).copied())
                        .collect();
                    adding.extend(tracks.into_iter().filter_map(|t| self.add_browsed(t)));
                    let added = self.adopt(&adding);
                    self.files.added += added;
                    if done {
                        let n = std::mem::take(&mut self.files.added);
                        self.set_files_status(match n {
                            0 => "Everything there is in the collection already.".into(),
                            1 => "Added 1 track to the collection.".into(),
                            n => format!("Added {n} tracks to the collection."),
                        });
                    }
                    changed = true;
                }
            }
        }
        if changed && self.tree == FILES {
            self.refresh_rows();
        }
    }

    /// Plays a file in the preview player while a source downloads it, or
    /// stops it.
    pub fn preview_download(&self, action: crate::sources::PreviewAction) {
        match action {
            crate::sources::PreviewAction::Play(path, size) => {
                self.engine.preview_download(path, size)
            }
            crate::sources::PreviewAction::Stop => self.engine.preview(None, false),
        }
    }

    /// Loads a file from a music server's cache onto a deck that isn't
    /// playing (deck A first), without it joining the collection: it stays
    /// a file shown, as in Files, until the DJ adds it.
    pub fn load_cached(&mut self, path: &Path) {
        let index = match self.path_index.get(path) {
            Some(&i) => Some(i),
            None => self.add_browsed(read_track(0, path)),
        };
        if let Some(i) = index {
            self.load_to_free_deck(i);
        }
    }

    /// Keeps a file read for Files that the collection doesn't have, with
    /// an id of its own. Returns where it is in `library`.
    fn add_browsed(&mut self, mut t: Track) -> Option<usize> {
        let path = file_path(&t)?.to_path_buf();
        if let Some(&i) = self.path_index.get(&path) {
            return Some(i);
        }
        t.id = self.files.next_id;
        self.files.next_id += 1;
        Some(self.push_track(t))
    }

    /// Appends a track to `library` and its indexes; returns its index.
    fn push_track(&mut self, t: Track) -> usize {
        let index = self.library.len();
        self.index_of.insert(t.id, index);
        if let Some(p) = file_path(&t) {
            self.path_index.insert(p.to_path_buf(), index);
        }
        self.indexed.push(Indexed::new(&t));
        self.library.push(Arc::new(t));
        index
    }

    /// Adds files shown in Files (by index into `library`) to the
    /// collection, as loading, rating or listing them does. Returns how
    /// many joined it.
    fn adopt(&mut self, indices: &[usize]) -> usize {
        let mut pick: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| self.library.get(i).is_some_and(|t| is_browsed(t)))
            .collect();
        pick.sort_unstable();
        pick.dedup();
        let Some(db) = &mut self.db else { return 0 };
        if pick.is_empty() {
            return 0;
        }
        let tracks: Vec<Track> = pick.iter().map(|&i| (*self.library[i]).clone()).collect();
        let added = match db.add_files(&tracks) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("can't add to the library: {e}");
                return 0;
            }
        };
        let mut known = self.files.reader.collection.write().ok();
        for (&i, t) in pick.iter().zip(added) {
            self.index_of.remove(&self.library[i].id);
            self.index_of.insert(t.id, i);
            if let (Some(k), Some(p)) = (known.as_mut(), file_path(&t)) {
                k.insert(p.to_path_buf());
            }
            self.library[i] = Arc::new(t);
        }
        drop(known);
        self.collection_stale = true;
        pick.len()
    }

    /// Lists a streaming service's playlists as tree nodes, keeping the
    /// selection.
    fn rebuild_stream_tree(&mut self, source: i32) {
        let Some(st) = self.streams.iter().find(|s| s.source == source) else {
            return;
        };
        let nodes: Vec<Node> = st
            .state
            .playlists
            .iter()
            .map(|p| {
                Node::new(
                    p.name.clone(),
                    0,
                    if p.key == spotify::LIKED { 4 } else { 2 },
                    Contents::Stream(p.key.clone()),
                )
            })
            .collect();
        let parked = self.parked.entry(source).or_insert(Parked {
            nodes: Vec::new(),
            current: 0,
            sort: (-1, true),
        });
        let (nodes_ref, current) = if self.tree == source {
            (&mut self.nodes, &mut self.current_node)
        } else {
            (&mut parked.nodes, &mut parked.current)
        };
        let selected = match nodes_ref.get(*current).map(|n| &n.contents) {
            Some(Contents::Stream(k)) => Some(k.clone()),
            _ => None,
        };
        *current = selected
            .and_then(|k| {
                nodes
                    .iter()
                    .position(|n| matches!(&n.contents, Contents::Stream(x) if *x == k))
            })
            .unwrap_or(0);
        *nodes_ref = nodes;
    }

    /// Shows each service's playlists kept from last time, before it has
    /// signed in again.
    fn restore_streams(&mut self) {
        for i in 0..self.streams.len() {
            let manual = self
                .db
                .as_ref()
                .and_then(|db| db.stream_matches(self.streams[i].id).ok())
                .unwrap_or_default();
            self.streams[i].restore(manual);
            if self.streams[i].state.connected {
                self.rebuild_stream_tree(self.streams[i].source);
            }
        }
    }

    /// Fetches the shown service's playlists again, and the open playlist's
    /// tracks; other playlists are fetched again when next opened.
    fn refresh_stream(&mut self) {
        let Some(st) = self.shown_stream_mut() else {
            return;
        };
        if !st.state.connected {
            return;
        }
        st.state.fresh.clear();
        st.status = "Refreshing playlists…".into();
        st.service.send(Request::Playlists);
        self.show_stream();
    }

    /// The streaming service shown in the tree, if one is.
    fn shown_stream(&self) -> Option<&Stream> {
        self.streams.iter().find(|s| s.source == self.tree)
    }

    fn shown_stream_mut(&mut self) -> Option<&mut Stream> {
        let tree = self.tree;
        self.streams.iter_mut().find(|s| s.source == tree)
    }

    /// Shows the shown service's sign-in, name and status in the sidebar.
    fn show_stream(&self) {
        let (Some(app), Some(st)) = (self.app.upgrade(), self.shown_stream()) else {
            return;
        };
        app.set_spotify_connected(st.state.connected);
        app.set_spotify_user(st.user.as_str().into());
        app.set_spotify_status(st.status.as_str().into());
        app.set_spotify_client_id(st.service.client_id.as_str().into());
    }

    /// The streaming playlist selected in the tree, if a service is shown.
    fn current_playlist(&self) -> Option<&str> {
        self.shown_stream()?;
        match &self.nodes.get(self.current_node)?.contents {
            Contents::Stream(k) => Some(k),
            _ => None,
        }
    }

    fn fetch_current_playlist(&mut self) {
        self.list_current_dir();
        let Some(key) = self.current_playlist().map(str::to_string) else {
            return;
        };
        let Some(stream) = self.shown_stream_mut() else {
            return;
        };
        // What copying the last playlist did is for that playlist only.
        stream.status.clear();
        let st = &mut stream.state;
        if !st.fresh.contains(&key) && !st.loading.contains(&key) {
            st.loading.insert(key.clone());
            st.errors.remove(&key);
            stream.service.send(Request::Tracks(key));
        }
        self.show_stream();
    }

    /// Follows the key notation picked by clicking a deck's key, and keeps it
    /// for next time. Called every frame.
    pub fn set_camelot(&mut self, camelot: bool) {
        let notation = djsugar_engine::key::notation();
        if camelot != self.camelot || notation != self.notation {
            self.camelot = camelot;
            self.notation = notation;
            crate::settings::set_camelot_keys(camelot);
            self.refresh_rows();
            self.refresh_side();
            if let Some(app) = self.app.upgrade() {
                self.refresh_info(&app);
            }
        }
    }

    /// Takes in what the Spotify worker has done. Called every frame.
    /// Puts newly found album art into the rows showing it.
    pub fn poll_art(&mut self) {
        if self.album_art.poll() {
            self.refresh_album_ui();
            if self.pages_open() {
                self.refresh_page();
            }
        }
        if !self.art.poll() {
            return;
        }
        // In place, so the side panel keeps its selection.
        if let Some(app) = self.app.upgrade() {
            let side = app.get_side_rows();
            for (i, id) in self.side_ids().into_iter().enumerate() {
                let Some(id) = id else { continue };
                let Some(mut row) = side.row_data(i) else {
                    continue;
                };
                if !row.has_cover {
                    if let Some(image) = self.art.get(&self.library[id]) {
                        row.cover = image;
                        row.has_cover = true;
                        side.set_row_data(i, row);
                    }
                }
            }
        }
        for (i, local) in self.rows.iter().enumerate() {
            let Some(id) = *local else { continue };
            let Some(mut row) = self.list.row_data(i) else {
                continue;
            };
            if row.has_cover {
                continue;
            }
            if let Some(image) = self.art.get(&self.library[id]) {
                row.cover = image;
                row.has_cover = true;
                self.list.set_row_data(i, row);
            }
        }
    }

    /// Gives the rows in view (and a screenful either side) their mini
    /// waveforms, and takes them from rows scrolled well away, so only a
    /// few small images are ever kept. Overviews not read yet are asked
    /// for; [`Browser::poll_waves`] fills them in as they arrive.
    fn fill_waves(&mut self) {
        let shown = self.columns.layout.has(columns::WAVE);
        let (first, count) = self.shown_rows;
        let count = count.max(1) as usize;
        let from = (first.max(0) as usize).saturating_sub(count);
        let to = (first.max(0) as usize + 2 * count).min(self.rows.len());
        let list = &self.list;
        self.wave_rows.retain(|&i| {
            let keep = shown && (from..to).contains(&i);
            if !keep {
                if let Some(mut row) = list.row_data(i) {
                    row.wave = Default::default();
                    row.has_wave = false;
                    list.set_row_data(i, row);
                }
            }
            keep
        });
        if !shown {
            return;
        }
        for i in from..to {
            let Some(id) = self.rows[i] else { continue };
            if self.wave_rows.contains(&i) {
                continue;
            }
            let Some(image) = self.waves.get(&self.library[id]) else {
                continue;
            };
            if let Some(mut row) = list.row_data(i) {
                row.wave = image;
                row.has_wave = true;
                list.set_row_data(i, row);
                self.wave_rows.push(i);
            }
        }
    }

    /// Follows the list's scrolling and puts newly read mini waveforms
    /// into the rows showing them. Called every frame; does nothing while
    /// the list is still and nothing has arrived.
    pub fn poll_waves(&mut self) {
        let restyled = crate::waves::take_restyled();
        if restyled {
            // Drawn again in the new style.
            for i in std::mem::take(&mut self.wave_rows) {
                if let Some(mut row) = self.list.row_data(i) {
                    row.wave = Default::default();
                    row.has_wave = false;
                    self.list.set_row_data(i, row);
                }
            }
        }
        let arrived = self.waves.poll() || restyled;
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<ColumnUi>();
        let shown = (ui.get_first_shown(), ui.get_shown_count());
        if arrived || shown != self.shown_rows {
            self.shown_rows = shown;
            self.fill_waves();
        }
        if restyled || arrived && !app.global::<TrackInfo>().get_has_wave() {
            self.fill_info_wave(&app);
        }
    }

    /// Takes in what the streaming services' workers have done. Called
    /// every frame.
    pub fn poll_streams(&mut self) {
        for i in 0..self.streams.len() {
            let source = self.streams[i].source;
            for event in self.streams[i].service.poll() {
                let shown = self.tree == source;
                let st = &mut self.streams[i];
                match event {
                    Event::Status(s) => st.status = s,
                    Event::Connected { user, playlists } => {
                        let old = std::mem::take(&mut st.state);
                        st.state = StreamState {
                            connected: true,
                            manual: self
                                .db
                                .as_ref()
                                .and_then(|db| db.stream_matches(st.id).ok())
                                .unwrap_or_default(),
                            ..StreamState::merged(old, playlists)
                        };
                        st.user = user;
                        st.status.clear();
                        st.keep();
                        self.look_up_placed(i);
                        self.fetch_synced(i);
                        self.rebuild_stream_tree(source);
                        if shown {
                            self.refresh_tree();
                            self.refresh_rows();
                            self.fetch_current_playlist();
                        }
                    }
                    Event::SignInFailed(error) => {
                        st.state.loading.clear();
                        st.status = if st.state.connected {
                            format!("{error} Showing your playlists from last time.")
                        } else {
                            error
                        };
                    }
                    Event::Tracks { key, tracks } => {
                        st.state.loading.remove(&key);
                        st.state.errors.remove(&key);
                        let version = st
                            .state
                            .playlists
                            .iter()
                            .find(|p| p.key == key)
                            .map(|p| p.version.clone())
                            .unwrap_or_default();
                        st.state.versions.insert(key.clone(), version);
                        st.state.fresh.insert(key.clone());
                        st.state.tracks.insert(key.clone(), tracks);
                        st.keep();
                        let want = st.wanted_later.remove(&key);
                        if shown && self.current_playlist() == Some(key.as_str()) {
                            self.refresh_rows();
                        }
                        if want {
                            self.want_arrived(i, &key);
                        }
                        self.synced_arrived(i, &key);
                    }
                    Event::TracksFailed { key, error } => {
                        st.state.loading.remove(&key);
                        if st.state.tracks.contains_key(&key) {
                            // The tracks from last time stay.
                            st.status = format!("Couldn't refresh this playlist: {error}");
                        } else {
                            st.state.errors.insert(key.clone(), error);
                        }
                        if shown && self.current_playlist() == Some(key.as_str()) {
                            self.refresh_rows();
                        }
                    }
                    Event::Found(found) => self.found_placed(i, found),
                    Event::Exported {
                        list,
                        url,
                        added,
                        missing,
                    } => self.exported(list, &url, added, &missing),
                    Event::ExportFailed { list, error } => {
                        self.export_status(list, format!("Couldn't export to Spotify: {error}"))
                    }
                    Event::Disconnected => {
                        st.state = StreamState::default();
                        crate::stream_cache::remove(st.id);
                        st.user.clear();
                        st.status.clear();
                        self.rebuild_stream_tree(source);
                        if shown {
                            self.refresh_tree();
                            self.refresh_rows();
                        }
                    }
                }
                if shown {
                    self.show_stream();
                }
            }
        }
    }

    /// Replaces the collection, e.g. when the library has been brought up
    /// to date with the music folder.
    pub fn set_library(&mut self, mut library: Vec<Arc<Track>>) {
        let paths: HashSet<PathBuf> = library
            .iter()
            .filter_map(|t| file_path(t).map(Path::to_path_buf))
            .collect();
        // Files shown in Files stay, unless the collection has them now.
        library.extend(
            self.library
                .iter()
                .filter(|t| is_browsed(t) && file_path(t).is_some_and(|p| !paths.contains(p)))
                .cloned(),
        );
        if let Ok(mut known) = self.files.reader.collection.write() {
            *known = paths;
        }
        self.path_index = library
            .iter()
            .enumerate()
            .filter_map(|(i, t)| Some((file_path(t)?.to_path_buf(), i)))
            .collect();
        self.index_of = library.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
        self.indexed = library.iter().map(|t| Indexed::new(t)).collect();
        self.library = library;
        self.side = [Vec::new(), Vec::new()];
        self.folders = crate::settings::stage_folders();
        self.mixed_sets = crate::settings::mixed_sets_folder();
        self.read_pending_moves();
        self.queue_triage_analysis();
        self.refresh_health();
        self.refresh_dupes();
        if let Some(db) = &self.db {
            self.store.wanted = db.wanted().unwrap_or_default();
        }
        self.load_marks();
        self.show_review_average();
        let tree = self.collection_tree();
        if self.tree == COLLECTION {
            self.nodes = tree;
            self.current_node = self.first_node(self.history_tab);
            self.other_tab_node = self.first_node(!self.history_tab);
        } else {
            let sort = self.parked.get(&COLLECTION).map_or((1, true), |p| p.sort);
            self.parked.insert(
                COLLECTION,
                Parked {
                    nodes: tree,
                    current: 0,
                    sort,
                },
            );
        }
        if matches!(self.tree, ALBUMS | MIXED_SETS) {
            self.rebuild_folder_tree(self.tree);
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_side();
        // A page's tracks are library indices, so it is made again.
        if self.pages_open() {
            self.refresh_page();
        }
    }

    /// The track at `index` on the Automix list.
    pub fn automix_at(&self, index: usize) -> Option<Arc<Track>> {
        let i = *self.side[0].get(index)?;
        self.library.get(i).cloned()
    }

    /// Takes a track off the Automix list once Automix has played it.
    pub fn remove_from_automix(&mut self, id: usize) {
        let Some(&index) = self.index_of.get(&id) else {
            return;
        };
        if let Some(i) = self.side[0].iter().position(|&t| t == index) {
            self.side[0].remove(i);
            self.refresh_side();
        }
    }

    /// Moves the track with id `id` to the end of the Automix list.
    pub fn requeue_in_automix(&mut self, id: usize) {
        let Some(&index) = self.index_of.get(&id) else {
            return;
        };
        if let Some(i) = self.side[0].iter().position(|&t| t == index) {
            let t = self.side[0].remove(i);
            self.side[0].push(t);
            self.refresh_side();
        }
    }

    /// Puts the Automix list in `order`, leaving the first `fixed` tracks
    /// where they are. Returns the note the panel shows, with Undo.
    pub fn order_automix(&mut self, order: crate::automix_order::Order, fixed: usize) -> String {
        use crate::automix_order::{self as o, Item};
        let list = &self.side[0];
        let items: Vec<Item> = list
            .iter()
            .map(|&i| {
                let t = &self.library[i];
                Item {
                    bpm: t.bpm,
                    key: Key::parse(&t.key),
                }
            })
            .collect();
        let mut seed = [0u8; 8];
        let _ = getrandom::fill(&mut seed);
        let mut state = u64::from_le_bytes(seed) | 1;
        let mut random = move || {
            // xorshift64*
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11
        };
        let picked = o::order(&items, fixed, order, self.key_rules.0, &mut random);
        let ordered: Vec<usize> = picked.iter().map(|&i| list[i]).collect();
        self.automix_before_order = Some(std::mem::replace(&mut self.side[0], ordered));
        self.refresh_side();
        format!("Put in order: {}", order.name())
    }

    /// Puts the Automix list back as it was before the last Order pick, if
    /// it still holds the same tracks.
    pub fn undo_automix_order(&mut self) {
        let Some(before) = self.automix_before_order.take() else {
            return;
        };
        let (mut a, mut b) = (before.clone(), self.side[0].clone());
        a.sort_unstable();
        b.sort_unstable();
        if a == b {
            self.side[0] = before;
            self.refresh_side();
        }
    }

    pub fn automix_len(&self) -> usize {
        self.side[0].len()
    }

    /// Moves the track with id `id` to place `to` on the Automix list.
    /// Returns whether it was on the list.
    pub fn move_in_automix(&mut self, id: usize, to: usize) -> bool {
        let Some(&index) = self.index_of.get(&id) else {
            return false;
        };
        let list = &mut self.side[0];
        let Some(from) = list.iter().position(|&t| t == index) else {
            return false;
        };
        let t = list.remove(from);
        list.insert(to.min(list.len()), t);
        self.refresh_side();
        true
    }

    /// Puts the Automix list in `order`, leaving the tracks Automix holds
    /// (playing and waiting) at the top. Returns the note with Undo.
    pub fn order_automix_for(
        &mut self,
        automix: &crate::automix::AutoMix,
        order: crate::automix_order::Order,
    ) -> String {
        let top: Vec<usize> = (0..3)
            .filter_map(|i| self.automix_at(i))
            .map(|t| t.id)
            .collect();
        let fixed = automix.fixed(&top);
        self.order_automix(order, fixed)
    }

    /// Collection tracks that `text` finds, with the browser's search
    /// syntax (`bpm:120-124`, `key:8A`, `crate:Warm-up`, ...), sorted by
    /// title, at most `limit` from place `offset`, with how many there were
    /// in all. Empty text finds the whole collection.
    pub fn find_tracks(&self, text: &str, offset: usize, limit: usize) -> (Vec<Arc<Track>>, usize) {
        let q = Query::parse(text.trim());
        let mut found = self.search(&q, None);
        let all = found.len();
        found.sort_by_cached_key(|&i| {
            let t = &self.library[i];
            (t.title.to_lowercase(), t.artist.to_lowercase())
        });
        let tracks = found
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|i| self.library[i].clone())
            .collect();
        (tracks, all)
    }

    /// Puts the track with id `id` on the Automix list at place `at`, or at
    /// the end. Returns whether there is such a track.
    pub fn add_to_automix_at(&mut self, id: usize, at: Option<usize>) -> bool {
        let Some(&index) = self.index_of.get(&id) else {
            return false;
        };
        self.adopt(&[index]);
        let list = &mut self.side[0];
        match at {
            Some(at) => list.insert(at.min(list.len()), index),
            None => list.push(index),
        }
        self.refresh_side();
        true
    }

    /// Whether Undo can put back the Automix list's last Order pick.
    pub fn can_undo_automix_order(&self) -> bool {
        self.automix_before_order.is_some()
    }

    /// The collection track with id `id`.
    pub fn track_by_id(&self, id: usize) -> Option<&Arc<Track>> {
        self.index_of.get(&id).map(|&i| &self.library[i])
    }

    /// A track's album art thumbnail, once found (as in the list).
    pub fn cover_of(&self, track: &Track) -> Option<slint::Image> {
        self.art.get(track)
    }

    /// Whether keys are shown in Camelot notation.
    pub fn camelot(&self) -> bool {
        self.camelot
    }

    /// Whether the collection is the saved library rather than the samples.
    fn saved(&self) -> bool {
        self.db.is_some()
            && self
                .library
                .first()
                .is_some_and(|t| djsugar_library::row_id(t.id).is_some())
    }

    /// The collection's tree: the collection with Wanted and the stages
    /// under it; the crates; the playlists in their folders, with
    /// groupings and tags and smart lists; and (on the history tab) the
    /// history sessions. Collection Health shows its checks instead.
    /// Folders start closed, and open as the DJ left them.
    fn collection_tree(&self) -> Vec<Node> {
        let mut nodes = self.collection_nodes();
        if self.saved() {
            let open: HashSet<String> = crate::settings::open_folders().into_iter().collect();
            for n in nodes.iter_mut().filter(|n| !n.history) {
                n.expanded = open.contains(&Self::node_key(n));
            }
        }
        nodes
    }

    fn collection_nodes(&self) -> Vec<Node> {
        if !self.saved() {
            let mut tree = sample_tree(self.library.len());
            let imported = imported_tree(&self.library);
            if !imported.is_empty() {
                let smart = tree
                    .iter()
                    .position(|n| n.name == "Smart Lists")
                    .unwrap_or(1);
                tree.splice(1..smart, imported);
            }
            return tree;
        }
        if self.health_view {
            return health::health_nodes();
        }
        let playlists = self
            .db
            .as_ref()
            .and_then(|db| db.playlists().ok())
            .unwrap_or_default();
        let inbox = self.db.as_ref().and_then(|db| db.inbox_list_id());
        let stages = self.folders.is_set();
        let mut tree = vec![
            Node::new("Collection", 0, 0, Contents::All),
            Node::new("Wanted", 1, KIND_WANTED, Contents::Wanted),
        ];
        if stages {
            for mut n in staging::stage_nodes(&self.folders) {
                n.depth += 1;
                let inbox = matches!(n.contents, Contents::Stage(Stage::Inbox, None));
                tree.push(n);
                // Albums downloaded whole, under the Inbox.
                if inbox {
                    tree.extend(self.inbox_album_nodes(2));
                }
            }
        }
        // Without stages, the Inbox list downloads go into stands in for one.
        let mut sections = Vec::new();
        for p in playlists.iter().filter(|p| Some(p.id) == inbox) {
            if !stages {
                let mut n = Node::new(p.name.clone(), 1, KIND_INBOX, Contents::Playlist(p.id));
                n.playlist = Some(p.id);
                tree.push(n);
                tree.extend(self.inbox_album_nodes(2));
            }
        }
        let mut crates = Node::new("Crates", 0, KIND_SECTION, Contents::Folder);
        crates.separator = true;
        sections.push(crates);
        let mut depth: HashMap<i64, i32> = HashMap::new();
        // Each imported list's import, by the source of its top folder.
        let mut locked: HashMap<i64, Option<String>> = HashMap::new();
        let mut in_crates: HashSet<i64> = HashSet::new();
        let mut sessions = Vec::new();
        let mut smart = Vec::new();
        // The DJ's crates and crate folders, their playlists and folders,
        // and other apps' lists (with the DJ's own made inside them before
        // they were read-only), each under its heading.
        let mut crate_rows = Vec::new();
        let mut playlist_rows = Vec::new();
        let mut imported = Vec::new();
        for p in &playlists {
            let d = p.parent.and_then(|id| depth.get(&id)).map_or(1, |d| d + 1);
            depth.insert(p.id, d);
            if Some(p.id) == inbox {
                continue;
            }
            let root = match p.parent.and_then(|id| locked.get(&id)) {
                Some(root) => Some(root.clone()),
                None => is_imported(p).then(|| p.source.clone()),
            };
            let lock = root.is_some();
            if let Some(root) = &root {
                locked.insert(p.id, root.clone());
            }
            let crate_side = match p.parent {
                None => is_crate(p) || p.source.as_deref() == Some(CRATE_FOLDER),
                Some(id) => in_crates.contains(&id),
            };
            if crate_side && !lock {
                in_crates.insert(p.id);
            }
            let node = match p.kind {
                // Smart lists at the top level go under "Smart Lists".
                PlaylistKind::Smart if p.parent.is_none() && !lock => {
                    smart.push(p);
                    continue;
                }
                PlaylistKind::Smart => smart_node(p, d),
                PlaylistKind::History if p.parent.is_some() => {
                    sessions.push(p);
                    continue;
                }
                PlaylistKind::History => continue,
                PlaylistKind::Crate => crate_node(p, d),
                // Sets made for a gig are folders like any other now.
                PlaylistKind::Folder | PlaylistKind::Set | PlaylistKind::List => {
                    let folder = p.kind != PlaylistKind::List;
                    let mut n = Node::new(
                        p.name.clone(),
                        d,
                        if folder { KIND_FOLDER } else { KIND_LIST },
                        if folder {
                            Contents::Folder
                        } else {
                            Contents::Playlist(p.id)
                        },
                    );
                    n.playlist = Some(p.id);
                    n
                }
            };
            let node = Node {
                locked: lock,
                // Every row of an import offers to make the whole import
                // the DJ's own, and reads dimmer than the DJ's own lists.
                import_root: root.flatten(),
                dim: node.dim || lock,
                ..node
            };
            if lock {
                imported.push(node);
            } else if crate_side {
                crate_rows.push(node);
            } else {
                playlist_rows.push(node);
            }
        }
        sections.extend(crate_rows);
        sections.push(Node::new("Playlists", 0, KIND_SECTION, Contents::Folder));
        sections.extend(playlist_rows);
        if !smart.is_empty() {
            sections.push(Node::new("Smart Lists", 1, KIND_FOLDER, Contents::Folder));
            sections.extend(smart.into_iter().map(|p| smart_node(p, 2)));
        }
        // What other apps and the files' own tags bring, kept apart from the
        // DJ's lists: read-only, as the next import makes it again.
        let generated = imported_tree(&self.library);
        if !imported.is_empty() || !generated.is_empty() {
            sections.push(Node {
                locked: true,
                ..Node::new("Imported", 0, KIND_SECTION, Contents::Folder)
            });
            sections.extend(imported);
            sections.extend(generated.into_iter().map(|mut n| {
                n.depth += 1;
                n
            }));
        }
        tree.extend(sections);
        tree.extend(self.history_nodes(&playlists, &sessions));
        tree
    }

    /// The history tab: a row per day, newest first, holding the day's
    /// sets, newest first. Sessions brought in from other apps go among
    /// the DJ's own. The newest day starts open.
    fn history_nodes(
        &self,
        playlists: &[djsugar_library::Playlist],
        sessions: &[&djsugar_library::Playlist],
    ) -> Vec<Node> {
        let sizes = self
            .db
            .as_ref()
            .and_then(|db| db.list_sizes().ok())
            .unwrap_or_default();
        let names: HashMap<i64, &str> = playlists.iter().map(|p| (p.id, p.name.as_str())).collect();
        let found: Vec<history::Session> = sessions
            .iter()
            .map(|p| history::Session {
                id: p.id,
                name: &p.name,
                started: p.created,
                from: p
                    .source
                    .as_ref()
                    .and(p.parent)
                    .and_then(|parent| names.get(&parent).copied()),
                songs: sizes.get(&p.id).copied().unwrap_or(0),
            })
            .collect();
        let full: HashMap<i64, &str> = sessions.iter().map(|p| (p.id, p.name.as_str())).collect();
        let today = history::day_of(history::now());
        let mut nodes = Vec::new();
        for (d, day) in history::by_day(&found).into_iter().enumerate() {
            let ids = day.sets.iter().map(|s| s.id).collect();
            let mut n = Node::new(
                history::day_label(day.date, today),
                0,
                KIND_HISTORY_DAY,
                Contents::HistoryDay(ids),
            );
            n.history = true;
            n.expanded = d == 0;
            n.title = Some(history::day_title(day.date));
            n.count = Some(day.songs());
            nodes.push(n);
            for set in day.sets.iter().rev() {
                let mut n = Node::new(
                    set.label.clone(),
                    1,
                    KIND_HISTORY,
                    Contents::Playlist(set.id),
                );
                n.history = true;
                n.title = full.get(&set.id).map(|s| s.to_string());
                n.count = Some(set.songs);
                n.dim = set.songs <= 1;
                nodes.push(n);
            }
        }
        nodes
    }

    /// The first node shown on the library tab (or the history tab).
    fn first_node(&self, history: bool) -> usize {
        self.nodes
            .iter()
            .position(|n| n.history == history)
            .unwrap_or(0)
    }

    /// What the tree remembers a node's being open by: its list's id, or its
    /// name and depth.
    fn node_key(n: &Node) -> String {
        match n.playlist {
            Some(id) => format!("list:{id}"),
            None => format!("{}:{}", n.depth, n.name),
        }
    }

    /// Remembers which of the collection's folders are open, for next time.
    fn save_open_folders(&self) {
        if self.tree != COLLECTION || !self.saved() {
            return;
        }
        let mut open: Vec<String> = crate::settings::open_folders();
        for (i, n) in self.nodes.iter().enumerate() {
            if n.history || !self.has_children(i) {
                continue;
            }
            let key = Self::node_key(n);
            open.retain(|k| *k != key);
            if n.expanded {
                open.push(key);
            }
        }
        crate::settings::set_open_folders(&open);
    }

    /// Rebuilds the tree from the database, keeping the selection where it can.
    fn reload_tree(&mut self, select: Option<i64>) {
        if self.tree != COLLECTION {
            return;
        }
        self.folders = crate::settings::stage_folders();
        self.mixed_sets = crate::settings::mixed_sets_folder();
        self.refresh_crates();
        if let Some(db) = &self.db {
            self.store.wanted = db.wanted().unwrap_or_default();
        }
        let selected = select.or_else(|| self.nodes.get(self.current_node)?.playlist);
        let name = self.nodes.get(self.current_node).map(|n| n.name.clone());
        self.nodes = self.collection_tree();
        self.current_node = selected
            .and_then(|id| self.nodes.iter().position(|n| n.playlist == Some(id)))
            .or_else(|| {
                self.nodes
                    .iter()
                    .position(|n| Some(&n.name) == name.as_ref() && n.history == self.history_tab)
            })
            .unwrap_or_else(|| self.first_node(self.history_tab));
        // Open the folders above the selection.
        let mut depth = self.nodes.get(self.current_node).map_or(0, |n| n.depth);
        for i in (0..self.current_node).rev() {
            if self.nodes[i].depth < depth {
                self.nodes[i].expanded = true;
                depth = self.nodes[i].depth;
            }
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Makes a new crate (`what` 0), folder (1), playlist (2) or folder at
    /// the top of Crates (3), and selects it. A crate, folder or playlist
    /// goes in the DJ's folder at tree row `at`, or (-1) at the top of
    /// Crates (a crate) or Playlists. Returns its row in the tree.
    fn create_list(&mut self, what: i32, at: i32) -> i32 {
        let Some(db) = &self.db else { return -1 };
        if !self.saved() || self.tree != COLLECTION || self.history_tab || self.health_view {
            return -1;
        }
        let parent = usize::try_from(at)
            .ok()
            .and_then(|row| self.visible_nodes.get(row))
            .map(|&n| &self.nodes[n])
            .filter(|n| n.kind == KIND_FOLDER && !n.locked)
            .and_then(|n| n.playlist);
        // Numbered past the DJ's other lists, so each new one can be told
        // apart even when it keeps the name it was given.
        let taken: Vec<&str> = self
            .nodes
            .iter()
            .filter(|n| n.playlist.is_some())
            .map(|n| n.name.as_str())
            .collect();
        let name = |base: &str| untaken_name(base, &taken);
        let made = match (what, parent) {
            (0, parent) => db.create_crate(&name("New crate"), parent),
            (1, parent) => db.create_playlist(&name("New folder"), parent, PlaylistKind::Folder),
            (3, _) => db.create_crate_folder(&name("New folder")),
            (_, Some(parent)) => {
                db.create_playlist(&name("New playlist"), Some(parent), PlaylistKind::List)
            }
            (_, None) => db.create_top_playlist(&name("New playlist")),
        };
        match made {
            Ok(id) => {
                self.reload_tree(Some(id));
                self.visible_nodes
                    .iter()
                    .position(|&n| n == self.current_node)
                    .map_or(-1, |p| p as i32)
            }
            Err(e) => {
                eprintln!("can't make a list: {e}");
                -1
            }
        }
    }

    /// Where the DJ's list or folder `id` would go, dropped on tree row
    /// `row`: into the folder there, or to the top of the Crates or
    /// Playlists heading there. None when it can't: imported, the list
    /// itself or a folder inside it, a crate's top-level place in
    /// Playlists or a playlist's in Crates.
    fn list_move(&self, id: i64, row: usize) -> Option<(Option<i64>, bool)> {
        if self.tree != COLLECTION || self.history_tab {
            return None;
        }
        let moved = self
            .nodes
            .iter()
            .find(|n| n.playlist == Some(id) && !n.history && !n.locked)?;
        let target = self.nodes.get(*self.visible_nodes.get(row)?)?;
        if target.locked {
            return None;
        }
        if target.kind == KIND_SECTION {
            let crates = target.name == "Crates";
            let fits = match moved.kind {
                KIND_FOLDER => true,
                KIND_CRATE => crates,
                _ => !crates,
            };
            return fits.then_some((None, crates));
        }
        if target.kind != KIND_FOLDER {
            return None;
        }
        let folder = target.playlist?;
        // Not into itself or a folder inside it.
        let lists = self.db.as_ref()?.playlists().ok()?;
        let mut at = Some(folder);
        while let Some(f) = at {
            if f == id {
                return None;
            }
            at = lists.iter().find(|p| p.id == f).and_then(|p| p.parent);
        }
        Some((Some(folder), false))
    }

    /// Moves the DJ's list or folder `id` to where dropping it on tree row
    /// `row` puts it.
    fn move_list(&mut self, id: i64, row: usize) {
        let Some((folder, crates)) = self.list_move(id, row) else {
            return;
        };
        let Some(db) = &self.db else { return };
        if let Err(e) = db.move_playlist(id, folder, crates) {
            eprintln!("can't move the list: {e}");
        }
        self.reload_tree(Some(id));
    }

    /// Turns the list at `row` into a crate, or a crate into a playlist.
    fn set_crate_at(&mut self, row: usize, krate: bool) {
        let Some(id) = self.own_list_at(row) else {
            return;
        };
        let Some(db) = &mut self.db else { return };
        if let Err(e) = db.set_crate(id, krate) {
            eprintln!("can't change the list's kind: {e}");
        }
        self.refresh_crates();
        self.reload_tree(Some(id));
    }

    /// The DJ's list or folder shown at `row` of the tree.
    fn playlist_at(&self, row: usize) -> Option<i64> {
        self.nodes.get(*self.visible_nodes.get(row)?)?.playlist
    }

    /// Like [`Browser::playlist_at`], leaving out imported (read-only) lists.
    fn own_list_at(&self, row: usize) -> Option<i64> {
        let n = self.nodes.get(*self.visible_nodes.get(row)?)?;
        n.playlist.filter(|_| !n.locked)
    }

    /// Makes the import with a list or folder at `row` the DJ's own: its
    /// lists stay, become editable, and its next import leaves them be.
    fn adopt_import_at(&mut self, row: usize) {
        let Some(source) = self
            .visible_nodes
            .get(row)
            .and_then(|&i| self.nodes[i].import_root.clone())
        else {
            return;
        };
        let Some(db) = &self.db else { return };
        match db.adopt_import(&source) {
            Ok(_) => {
                crate::settings::set(
                    &crate::settings::import_lists_mine_key(&source),
                    serde_json::json!(true),
                );
            }
            Err(e) => eprintln!("can't make the {source} lists your own: {e}"),
        }
        self.reload_tree(None);
    }

    fn rename_node(&mut self, row: usize, name: &str) {
        let name = name.trim();
        let (Some(id), Some(db), false) = (self.own_list_at(row), &self.db, name.is_empty()) else {
            return;
        };
        if let Err(e) = db.rename_playlist(id, name) {
            eprintln!("can't rename the list: {e}");
        }
        self.reload_tree(None);
    }

    /// Gives the crate at a sidebar row a color (an index into
    /// `COLOR_NAMES`, None for none).
    fn color_node(&mut self, row: usize, color: Option<usize>) {
        let (Some(id), Some(db)) = (self.own_list_at(row), &self.db) else {
            return;
        };
        let rgb = color
            .and_then(|i| djsugar_library::query::COLOR_NAMES.get(i))
            .map(|&(_, rgb)| rgb);
        if let Err(e) = db.set_playlist_color(id, rgb) {
            eprintln!("can't color the list: {e}");
        }
        self.reload_tree(None);
    }

    /// Gives the crate at a sidebar row one of `LIST_ICONS` (0, the crate,
    /// is kept as none).
    fn icon_node(&mut self, row: usize, icon: usize) {
        let (Some(id), Some(db)) = (self.own_list_at(row), &self.db) else {
            return;
        };
        let name = djsugar_library::LIST_ICONS
            .get(icon)
            .filter(|_| icon > 0)
            .copied();
        if let Err(e) = db.set_playlist_icon(id, name) {
            eprintln!("can't change the list's icon: {e}");
        }
        self.reload_tree(None);
    }

    fn delete_node(&mut self, row: usize) {
        let (Some(id), Some(db)) = (self.own_list_at(row), &self.db) else {
            return;
        };
        if let Err(e) = db.delete_playlist(id) {
            eprintln!("can't delete the list: {e}");
        }
        let deleted_selected =
            self.nodes.get(self.current_node).and_then(|n| n.playlist) == Some(id);
        if deleted_selected {
            self.current_node = 0;
        }
        self.reload_tree(None);
    }

    /// Adds the tracks of rows `first..=last`, in order, to a playlist.
    fn add_rows_to_playlist(&mut self, first: usize, last: usize, playlist: i64) {
        let indices = self.chosen_tracks(first, last);
        self.add_to_playlist(&indices, playlist);
    }

    /// Adds library tracks (indices into `library`), in order, to a playlist.
    fn add_to_playlist(&mut self, indices: &[usize], playlist: i64) {
        if indices.is_empty() {
            return;
        }
        self.adopt(indices);
        let ids: Vec<_> = indices.iter().map(|&i| self.library[i].id).collect();
        if let Some(db) = &mut self.db {
            if let Err(e) = db.add_to_playlist(playlist, &ids) {
                eprintln!("can't add to the list: {e}");
            }
        }
        self.refresh_crates();
        if self.nodes.get(self.current_node).and_then(|n| n.playlist) == Some(playlist) {
            self.refresh_rows();
        }
    }

    /// Takes the track at `row` out of the list being shown.
    fn remove_row_from_list(&mut self, row: usize) {
        let Some(Contents::Playlist(list)) = self
            .nodes
            .get(self.current_node)
            .map(|n| n.contents.clone())
        else {
            return;
        };
        let Some(&Some(index)) = self.rows.get(row) else {
            return;
        };
        // Rows may be sorted or filtered: find which entry of the list this
        // row is (the nth time the track appears).
        let id = self.library[index].id;
        let nth = self.rows[..row]
            .iter()
            .filter(|r| r.is_some_and(|i| self.library[i].id == id))
            .count();
        let Some(db) = &mut self.db else { return };
        let entries = db.playlist_tracks(list).unwrap_or_default();
        let Some(position) = entries
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == id)
            .nth(nth)
            .map(|(p, _)| p)
        else {
            return;
        };
        if let Err(e) = db.remove_from_playlist(list, position) {
            eprintln!("can't remove from the list: {e}");
        }
        self.refresh_crates();
        self.refresh_rows();
    }

    /// Changes the library tracks `indices`, then lists the rows once.
    fn change_tracks(
        &mut self,
        indices: &[usize],
        mut change: impl FnMut(&mut Track, Option<&Library>),
    ) {
        if indices.is_empty() {
            return;
        }
        self.adopt(indices);
        for &index in indices {
            let mut t = (*self.library[index]).clone();
            change(&mut t, self.db.as_ref());
            self.library[index] = Arc::new(t);
        }
        self.refresh_rows();
    }

    /// Rates the rows picked in the track list (all of them when `first`
    /// is one of several picked).
    pub fn rate_rows(&mut self, first: usize, last: usize, rating: u8) {
        let tracks = self.chosen_tracks(first, last);
        self.change_tracks(&tracks, |t, db| {
            t.rating = rating;
            if let Some(Err(e)) = db.map(|db| db.set_rating(t.id, rating)) {
                eprintln!("can't save the rating: {e}");
            }
        });
    }

    /// Gives the rows picked in the track list (all of them when `first`
    /// is one of several picked) a color, or none.
    pub fn color_rows(&mut self, first: usize, last: usize, color: Option<u32>) {
        let tracks = self.chosen_tracks(first, last);
        self.change_tracks(&tracks, |t, db| {
            t.info.color = color;
            if let Some(Err(e)) = db.map(|db| db.set_color(t.id, color)) {
                eprintln!("can't save the color: {e}");
            }
        });
    }

    /// Rates the track loaded on `deck`; false when there's none.
    pub fn rate_deck(&mut self, deck: usize, rating: u8) -> bool {
        let Some(index) = self.deck_track(deck) else {
            return false;
        };
        self.adopt(&[index]);
        let mut t = (*self.library[index]).clone();
        t.rating = rating;
        if let Some(Err(e)) = self.db.as_ref().map(|db| db.set_rating(t.id, rating)) {
            eprintln!("can't save the rating: {e}");
        }
        self.library[index] = Arc::new(t);
        self.refresh_rows();
        true
    }

    /// Switches the sidebar between the library tree and the history.
    fn show_history(&mut self, on: bool) {
        if on == self.history_tab {
            return;
        }
        self.park_view();
        self.history_tab = on;
        std::mem::swap(&mut self.current_node, &mut self.other_tab_node);
        if self
            .nodes
            .get(self.current_node)
            .is_none_or(|n| n.history != on)
        {
            self.current_node = self.first_node(on);
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Saves what changed on the decks: hot cues the DJ set or cleared, beat
    /// grids found by analysis or edited, and tracks played long enough to
    /// count. `dt` is the time since the last call. Called every frame.
    pub fn keep(&mut self, loaded: &[Option<Arc<Track>>], dt: f64) {
        if !self.saved() {
            return;
        }
        let mut changed = false;
        let mut cues_changed = false;
        for (d, t) in loaded.iter().enumerate().take(NUM_DECKS) {
            let Some(t) = t.as_ref() else {
                self.watch[d] = CueWatch::default();
                continue;
            };
            if djsugar_library::row_id(t.id).is_none() {
                continue;
            }
            let w = &mut self.watch[d];
            if w.track.map(|(id, _)| id) != Some(t.id) {
                *w = CueWatch {
                    track: Some((t.id, std::time::Instant::now())),
                    saved: t.info.hotcues.clone(),
                    ..Default::default()
                };
            }
            // The engine sets the new track's cues as it loads; wait for that.
            let settled = w
                .track
                .is_some_and(|(_, at)| at.elapsed() > std::time::Duration::from_millis(500));
            if settled {
                let pads = self.hotcues[d].len();
                let mut now: Vec<Hotcue> = self.hotcues[d]
                    .iter()
                    .enumerate()
                    .filter(|(_, (enabled, _, _))| enabled.get_bool())
                    .map(|(n, (_, pos, end))| {
                        let seconds = pos.get();
                        let end = Some(end.get()).filter(|&e| e > seconds);
                        // Keep a cue's name and color while it stays where
                        // it was.
                        let kept = w
                            .saved
                            .iter()
                            .find(|c| c.number == n && (c.seconds - seconds).abs() < 1e-6);
                        Hotcue {
                            number: n,
                            seconds,
                            name: kept.map(|c| c.name.clone()).unwrap_or_default(),
                            end,
                            color: kept.and_then(|c| c.color),
                        }
                    })
                    .collect();
                // Cues the deck has no pad for (memory cues, and hot cues
                // past the last pad) stay as they were.
                now.extend(w.saved.iter().filter(|c| c.number >= pads).cloned());
                if now != w.saved {
                    w.saved = now.clone();
                    if let Some(db) = &mut self.db {
                        if let Err(e) = db.save_cues(t.id, &now) {
                            eprintln!("can't save the cues: {e}");
                        }
                    }
                    if let Some(&i) = self.index_of.get(&t.id) {
                        let mut e = (*self.library[i]).clone();
                        e.info.hotcues = now;
                        self.library[i] = Arc::new(e);
                        // The Cues column counts them.
                        cues_changed = true;
                    }
                }
            }
            let w = &mut self.watch[d];
            if self.play[d].get_bool() && !w.recorded {
                w.played += dt;
                if w.played >= PLAYED_AFTER {
                    w.recorded = true;
                    self.record_play(t.id);
                    changed = true;
                }
            }
        }
        if (changed && self.history_tab) || cues_changed {
            self.refresh_rows();
        }
    }

    /// Adds a played track to this session's history.
    fn record_play(&mut self, id: usize) {
        let Some(db) = &mut self.db else { return };
        let now = history::now();
        if self
            .last_play
            .is_some_and(|last| history::new_set(last, now))
        {
            self.session = None;
        }
        self.last_play = Some(now);
        if self.session.is_none() {
            let name = chrono::Local::now()
                .format("%a %e %b %Y, %H:%M")
                .to_string();
            match db.start_session(&name) {
                Ok(s) => self.session = Some(s),
                Err(e) => eprintln!("can't start the history: {e}"),
            }
            let selected = self.nodes.get(self.current_node).and_then(|n| n.playlist);
            self.reload_tree(selected);
        }
        let Some(db) = &mut self.db else { return };
        if let Err(e) = db.record_play(self.session, id) {
            eprintln!("can't record the play: {e}");
        }
        if let Some(&i) = self.index_of.get(&id) {
            let mut e = (*self.library[i]).clone();
            e.info.play_count += 1;
            self.library[i] = Arc::new(e);
        }
        let showing_session =
            self.nodes
                .get(self.current_node)
                .is_some_and(|n| match (&n.contents, self.session) {
                    (Contents::Playlist(p), Some(s)) => *p == s,
                    (Contents::HistoryDay(sets), Some(s)) => sets.contains(&s),
                    _ => false,
                });
        if showing_session {
            self.refresh_rows();
        }
    }

    fn side_tab(&self) -> usize {
        self.app
            .upgrade()
            .map(|a| a.get_side_tab())
            .unwrap_or(0)
            .clamp(0, info::INFO_TAB as i32) as usize
    }

    /// Loads onto a deck that is not playing, preferring deck A.
    fn load_dropped(&mut self, deck: usize, data: &DataTransfer) {
        if deck >= NUM_DECKS {
            return;
        }
        // A deck's own cover dropped back on it changes nothing.
        if dragged(data) == Some(Dragged::Deck(deck)) {
            return;
        }
        if let Some(id) = self.dropped_track(data) {
            self.engine.load_track(deck, self.library[id].clone());
        }
    }

    /// The playable tracks of list rows `first..=last`, in order, taken into
    /// the library.
    fn playable_rows(&mut self, first: usize, last: usize) -> Vec<usize> {
        let ids: Vec<usize> = self
            .chosen_tracks(first, last)
            .into_iter()
            .filter(|&id| self.playable(id))
            .collect();
        self.adopt(&ids);
        ids
    }

    /// The library tracks a drop carries: the rows of the list it names, or
    /// an audio file from outside.
    pub(crate) fn dropped_tracks(&mut self, data: &DataTransfer) -> Vec<usize> {
        match dragged(data) {
            Some(Dragged::Rows(first, last)) => self.playable_rows(first, last),
            Some(Dragged::Side(tab, first, last)) => {
                let ids: Vec<usize> = self
                    .side_ids_of(tab)
                    .into_iter()
                    .skip(first)
                    .take(last + 1 - first)
                    .flatten()
                    .collect();
                self.adopt(&ids);
                ids
            }
            Some(Dragged::Deck(deck)) => self.deck_track(deck).into_iter().collect(),
            None => self.dropped_track(data).into_iter().collect(),
        }
    }

    /// The library track loaded on `deck`.
    fn deck_track(&self, deck: usize) -> Option<usize> {
        self.index_of
            .get(&self.loaded.get(deck).copied().flatten()?)
            .copied()
    }

    /// The library track a drop carries: the first of the rows it names, a
    /// deck's track, or an audio file from outside, which joins the library.
    fn dropped_track(&mut self, data: &DataTransfer) -> Option<usize> {
        if let Some(Dragged::Rows(row, _)) = dragged(data) {
            match self.rows.get(row) {
                Some(&Some(id)) if self.playable(id) => {
                    self.adopt(&[id]);
                    Some(id)
                }
                _ => None,
            }
        } else if dragged(data).is_some() {
            self.dropped_tracks(data).first().copied()
        } else if let Some(path) = dropped_file(data) {
            // A file from outside the collection joins the library so the
            // deck markers and reloading work as for any other track.
            if let Some(&i) = self.path_index.get(&path) {
                self.adopt(&[i]);
                return Some(i);
            }
            let index = self.library.len();
            let mut track = read_track(index, &path);
            if self.saved() {
                if let Some(db) = &mut self.db {
                    match db.add_file(&track) {
                        Ok(t) => track = t,
                        Err(e) => eprintln!("can't add {} to the library: {e}", path.display()),
                    }
                }
            }
            Some(if let Some(&i) = self.index_of.get(&track.id) {
                i
            } else {
                self.push_track(track)
            })
        } else {
            None
        }
    }

    /// Acts on library controls set by controller mappings: browsing,
    /// switching between folders and tracks, loading. Called every frame.
    pub fn poll_controllers(&mut self, app: &AppWindow) {
        if take(&self.lib.show_linked) > 0.0 {
            app.set_side_panel_visible(true);
            app.set_side_tab(LINKED_TAB as i32);
        }
        let mut steps: f64 = self.lib.steps.iter().map(|(c, sign)| take(c) * sign).sum();
        // Clicking a folder or a track moves the focus too.
        self.folders_focused = app.get_folders_focused();
        if self.lib.focus.iter().map(take).sum::<f64>() != 0.0 {
            self.folders_focused = !self.folders_focused;
            app.set_folders_focused(self.folders_focused);
        }
        let folder_steps: f64 = self.lib.folder_steps.iter().map(|(c, s)| take(c) * s).sum();
        let mut folder_steps = folder_steps.round() as i64;
        if self.folders_focused {
            folder_steps += steps.round() as i64;
            steps = 0.0;
        }
        if folder_steps != 0 && !self.visible_nodes.is_empty() {
            let at = self
                .visible_nodes
                .iter()
                .position(|&n| n == self.current_node)
                .unwrap_or(0) as i64;
            let to = (at + folder_steps).clamp(0, self.visible_nodes.len() as i64 - 1) as usize;
            self.park_view();
            self.current_node = self.visible_nodes[to];
            app.set_current_track(-1);
            self.refresh_tree();
            self.refresh_rows();
            self.fetch_current_playlist();
        }
        let steps = steps.round() as i64;
        if steps != 0 && !self.rows.is_empty() {
            // Moving the selection picks one track again.
            self.clear_picks();
            let current = app.get_current_track() as i64;
            let to = if current < 0 {
                if steps > 0 {
                    steps - 1
                } else {
                    self.rows.len() as i64 + steps
                }
            } else {
                current + steps
            };
            app.set_current_track(to.clamp(0, self.rows.len() as i64 - 1) as i32);
        }

        let selected = usize::try_from(app.get_current_track())
            .ok()
            .and_then(|r| self.rows.get(r).copied().flatten());
        let widget = if self.folders_focused { 2.0 } else { 3.0 };
        if self.lib.focused_widget.get() != widget {
            self.lib.focused_widget.set(widget);
        }
        let go = take(&self.lib.go_to_item) > 0.0;
        if take(&self.lib.toggle_folder) > 0.0 {
            if let Some(n) = self.nodes.get_mut(self.current_node) {
                n.expanded = !n.expanded;
                self.save_open_folders();
                self.refresh_tree();
            }
        }
        if go && self.folders_focused {
            // Like pressing the browse knob on a folder: open or close it.
            if let Some(n) = self.nodes.get_mut(self.current_node) {
                n.expanded = !n.expanded;
                self.save_open_folders();
                self.refresh_tree();
            }
        } else if (go || take(&self.lib.load_first_stopped) > 0.0) && selected.is_some() {
            self.adopt(&[selected.unwrap()]);
            self.load_to_free_deck(selected.unwrap());
        }
        for d in 0..NUM_DECKS {
            let load = take(&self.lib.load[d]) > 0.0;
            let and_play = take(&self.lib.load_and_play[d]) > 0.0;
            if let (Some(id), true) = (selected.filter(|&i| self.playable(i)), load || and_play) {
                self.adopt(&[id]);
                if self.engine.load_track(d, self.library[id].clone()) {
                    self.play_when_loaded[d] = and_play.then_some(id);
                }
            }
        }
        let pressed: Vec<bool> = self
            .lib
            .add_to_automix
            .iter()
            .map(|c| take(c) > 0.0)
            .collect();
        for (i, pressed) in pressed.into_iter().enumerate() {
            if let (true, Some(id)) = (pressed, selected) {
                self.adopt(&[id]);
                if i == 0 {
                    self.side[0].push(id);
                } else {
                    self.side[0].insert(0, id);
                }
                self.refresh_side();
            }
        }
    }

    /// Whether a drag carries something a sampler pad takes: a track or an
    /// audio file.
    pub fn accepts_sample(data: &DataTransfer) -> bool {
        dragged(data).is_some() || dropped_file(data).is_some()
    }

    /// The audio file a drop on a sampler pad carries, with a name for the
    /// pad: a track's title, or a file's name. Tracks stay where they are in
    /// the library and files from outside don't join it.
    pub fn dropped_sample(&self, data: &DataTransfer) -> Option<(PathBuf, String)> {
        let id = match dragged(data) {
            Some(Dragged::Rows(row, _)) => self.rows.get(row).copied().flatten(),
            Some(Dragged::Side(tab, first, _)) => {
                self.side_ids_of(tab).get(first).copied().flatten()
            }
            Some(Dragged::Deck(deck)) => self.deck_track(deck),
            None => {
                let path = dropped_file(data)?;
                let name = file_stem(&path);
                return Some((path, name));
            }
        }?;
        self.sample_of(id)
    }

    /// The track selected in the list, as a sampler sample.
    pub fn selected_sample(&self, app: &AppWindow) -> Option<(PathBuf, String)> {
        let id = usize::try_from(app.get_current_track())
            .ok()
            .and_then(|r| self.rows.get(r).copied().flatten())?;
        self.sample_of(id)
    }

    fn sample_of(&self, id: usize) -> Option<(PathBuf, String)> {
        let t = self.library.get(id).filter(|_| self.playable(id))?;
        let djsugar_engine::Source::File(path) = &t.source else {
            return None;
        };
        let name = if t.title.is_empty() {
            file_stem(path)
        } else {
            t.title.clone()
        };
        Some((path.clone(), name))
    }

    /// Whether the track at `index` of the library can load: its file is there.
    fn playable(&self, index: usize) -> bool {
        self.library.get(index).is_some_and(|t| !t.info.missing)
    }

    fn load_to_free_deck(&self, id: usize) {
        if !self.playable(id) {
            return;
        }
        if let Some(deck) = (0..self.decks_shown).find(|&d| !self.play[d].get_bool()) {
            self.engine.load_track(deck, self.library[id].clone());
        }
    }

    pub fn set_decks_shown(&mut self, decks: usize) {
        self.decks_shown = decks.clamp(2, NUM_DECKS);
        if let Some(app) = self.app.upgrade() {
            app.global::<LibraryEdit>()
                .set_decks(self.decks_shown as i32);
        }
    }

    /// Called every frame with what is on the decks; refreshes the A/B markers
    /// and the side panel when they change.
    pub fn set_loaded(&mut self, app: &AppWindow, loaded: [Option<usize>; NUM_DECKS]) {
        let shown = app.get_side_tab().max(0) as usize;
        self.poll_info(app, shown);
        self.poll_lyrics(app, shown);
        self.poll_wave_lyrics(app);
        let tab = shown.min(info::INFO_TAB);
        // The Linked tab follows the track selected in the list.
        let selected = usize::try_from(app.get_current_track())
            .ok()
            .and_then(|r| self.rows.get(r).copied().flatten());
        // Not while a track is being dragged: picking it up selects it, and
        // it is about to be linked to the track shown.
        let dragging = app.global::<TrackDrag>().get_active();
        let follow = tab == LINKED_TAB && !dragging && selected != self.linked_to;
        if follow {
            self.linked_to = selected;
        }
        if tab != self.shown_side_tab || follow {
            self.shown_side_tab = tab;
            self.refresh_side();
        }
        if loaded != self.loaded {
            self.loaded = loaded;
            self.show_favorites();
            self.update_reference();
            self.refresh_rows();
            self.refresh_side();
            self.refresh_prep();
        } else if self.update_reference() {
            // Keys light up when they mix with the track playing now.
            self.refresh_rows();
            self.refresh_side();
            if self.follows_deck() {
                self.refresh_filter_ui();
            }
        }
        // Grouping or the tag groups changed in Settings or by an import.
        let names = query::field_names();
        if names != self.filter.names {
            self.field_names_changed(names);
        }
        for ((pending, now), play) in self.play_when_loaded.iter_mut().zip(loaded).zip(&self.play) {
            if pending.is_some() && *pending == now {
                *pending = None;
                play.set_bool(true);
            }
        }
        self.refresh_cues();
        self.refresh_checklist();
        self.tick_review();
    }

    /// Copies the tempo, key and beat grid found by analysing a loaded
    /// track, or a grid the DJ edited, into the library and saves them, as
    /// Mixxx stores analysis results. The list shows them too.
    pub fn learn_analysis(&mut self, loaded: &[Option<Arc<Track>>]) {
        let mut changed = HashSet::new();
        let saved = self.saved();
        for t in loaded.iter().flatten() {
            // Its mini waveform, filling in as the deck analyses it, in its
            // row and in Info.
            if self.waves.learn(t) {
                let i = self.index_of.get(&t.id).copied();
                if let Some(row) = self.rows.iter().position(|r| i.is_some() && *r == i) {
                    self.wave_rows.retain(|&w| w != row);
                    self.fill_waves();
                }
                if let Some(app) = self.app.upgrade() {
                    self.fill_info_wave(&app);
                }
            }
            if t.bpm <= 0.0 {
                continue;
            }
            let Some(&i) = self.index_of.get(&t.id) else {
                continue;
            };
            let entry = &self.library[i];
            // An analysed grid arrives with the analysis; a fixed grid that
            // differs from the library's was edited on the deck.
            let analysed = !t.info.fixed_grid && !t.grid.is_empty() && t.grid != entry.grid;
            let edited = t.info.fixed_grid && t.grid != entry.grid && !t.grid.is_empty();
            let key_changed = entry.key != t.key || entry.info.key_detected != t.info.key_detected;
            let lock_changed = entry.info.grid_locked != t.info.grid_locked;
            if !(analysed || edited || lock_changed || entry.bpm != t.bpm || key_changed) {
                continue;
            }
            // An analysed grid replacing one the DJ edited or an import
            // brought: the DJ went back to the analysed grid.
            let reset = analysed && !matches!(entry.info.grid_from.as_str(), "" | "analysis");
            let mut e = (**entry).clone();
            e.bpm = t.bpm;
            e.key = t.key.clone();
            e.info.key_detected = t.info.key_detected;
            e.info.grid_locked = t.info.grid_locked;
            if analysed || edited {
                e.set_grid(t.grid.clone());
                e.info.fixed_grid = true;
                e.info.grid_analysed = analysed;
                e.info.grid_from = if analysed { "analysis" } else { "user" }.into();
                e.info.grid_unsure = t.info.grid_unsure;
            }
            if let (true, Some(db)) = (saved, &self.db) {
                let mut result = if lock_changed {
                    db.set_grid_locked(t.id, t.info.grid_locked)
                } else {
                    Ok(())
                };
                if reset {
                    result = result.and_then(|()| db.clear_grid(t.id));
                }
                result = result.and_then(|()| {
                    if edited {
                        db.save_grid(t.id, &t.grid, t.bpm)
                    } else {
                        db.save_analysis(t.id, t)
                    }
                });
                if let Err(err) = result {
                    eprintln!("can't save the analysis: {err}");
                }
            }
            self.library[i] = Arc::new(e);
            changed.insert(i);
        }
        if changed.is_empty() {
            return;
        }
        if self.rows_stay_put() {
            self.refresh_track_rows(&changed);
        } else {
            self.refresh_rows();
        }
    }

    /// Whether the list shown keeps the same rows, in the same order, when
    /// tracks get a new tempo, key or grid: nothing searched or filtered,
    /// sorted by something analysis doesn't change, and a view that
    /// doesn't pick its tracks by what analysis finds.
    fn rows_stay_put(&self) -> bool {
        use columns::*;
        !self.shows_store_rows()
            && Query::parse(&self.full_search()).is_empty()
            && (self.sort_column < 0
                || [
                    ARTIST, TITLE_ID, ALBUM, GENRE, LENGTH, RATING_ID, ORDER, PLAYED_AT, CRATES,
                ]
                .contains(&self.sort_column))
            && !matches!(
                self.current_shown(),
                Some(
                    Contents::Query(_)
                        | Contents::Health(_)
                        | Contents::Dupes(_)
                        | Contents::Area(_)
                        | Contents::Ignored
                )
            )
    }

    /// Shows the new tempo, key and other analysed cells of these library
    /// tracks (indices into `library`) in the rows that list them, leaving
    /// every other row as it is. Rebuilding the whole list (see
    /// [`Self::refresh_rows`]) re-matches streamed tracks and lays every
    /// row out again, which a batch of analysed tracks shouldn't pay for
    /// each time one finishes.
    fn refresh_track_rows(&mut self, changed: &HashSet<usize>) {
        let ids = self.columns.ids();
        let kept = |id: &i32| matches!(*id, columns::ORDER | columns::PLAYED_AT);
        for (row, i) in self.rows.iter().enumerate() {
            let Some(i) = i.filter(|i| changed.contains(i)) else {
                continue;
            };
            let Some(mut r) = self.list.row_data(row) else {
                continue;
            };
            let t = &self.library[i];
            r.bpm = if t.bpm > 0.0 {
                let sections = if t.grid.is_dynamic() { "~" } else { "" };
                format!("{sections}{:.1}", t.bpm).into()
            } else {
                SharedString::new()
            };
            r.key = show_key(&t.key, self.camelot).into();
            r.key_match = self.mixes_with_deck(&t.key);
            r.key_shift = self.key_shift_to_deck(&t.key);
            r.length = short_time(t.duration);
            let old = r.cells.clone();
            let crates = self.crates.get(&t.id);
            let cells = columns::row_cells(
                &ids,
                &r,
                Some(t),
                crates.map(Vec::as_slice),
                Default::default(),
                self.prep_facts(t, &ids),
            );
            // The place in the list and the time played stay as they were.
            for (c, id) in ids.iter().enumerate() {
                if kept(id) {
                    if let Some(cell) = old.row_data(c) {
                        cells.set_row_data(c, cell);
                    }
                }
            }
            r.cells = cells;
            self.list.set_row_data(row, r);
        }
    }

    fn refresh_tree(&mut self) {
        self.visible_nodes.clear();
        let mut hidden_below: Option<i32> = None;
        for (i, n) in self.nodes.iter().enumerate() {
            if n.history != self.history_tab {
                continue;
            }
            if let Some(d) = hidden_below {
                if n.depth > d {
                    continue;
                }
                hidden_below = None;
            }
            self.visible_nodes.push(i);
            if !n.expanded && self.has_children(i) {
                hidden_below = Some(n.depth);
            }
        }
        let folders: Vec<Folder> = self
            .visible_nodes
            .iter()
            .map(|&i| {
                let n = &self.nodes[i];
                Folder {
                    name: n.name.clone().into(),
                    depth: n.depth,
                    children: if n.lazy {
                        1
                    } else if !self.has_children(i) {
                        0
                    } else if n.expanded {
                        2
                    } else {
                        1
                    },
                    kind: n.kind,
                    editable: n.playlist.is_some() && !n.history && !n.locked,
                    locked: n.locked,
                    adoptable: n.import_root.is_some(),
                    analysable: !n.history && Self::analysable(&n.contents),
                    stage: matches!(n.contents, Contents::Stage(..)),
                    bundle: matches!(n.contents, Contents::InboxAlbum(_)),
                    separator: n.separator,
                    color: n.color.map_or(-1, |c| c as i32),
                    icon: match n.contents {
                        Contents::Area(a) => self.area_state(a) as i32,
                        _ => n.icon as i32,
                    },
                    count: match &n.contents {
                        Contents::All if self.saved() && self.tree == COLLECTION => {
                            self.library
                                .iter()
                                .filter(|t| !t.info.missing && self.in_collection(t))
                                .count() as i32
                        }
                        Contents::Wanted if self.tree == COLLECTION => {
                            self.store.wanted.len() as i32
                        }
                        Contents::Stage(stage, dir) => self
                            .library
                            .iter()
                            .filter(|t| !t.info.missing && self.at_stage(t, *stage, dir.as_deref()))
                            .count() as i32,
                        Contents::Health(Some(check)) => self.health_count(*check) as i32,
                        Contents::Dupes(case) => self.dupe_tracks(Some(*case)).len() as i32,
                        Contents::Area(a) => self.area_count(*a) as i32,
                        Contents::Ignored => self.ignored_tracks().len() as i32,
                        _ => n.count.map_or(-1, |c| c as i32),
                    },
                    dim: n.dim,
                }
            })
            .collect();
        // The track menu offers the DJ's lists, with the folders they're in.
        let mut lists = Vec::new();
        let mut path: Vec<&str> = Vec::new();
        for n in self
            .nodes
            .iter()
            .filter(|n| n.playlist.is_some() && !n.history)
        {
            path.truncate(n.depth as usize);
            if (n.kind == KIND_LIST || n.kind == KIND_CRATE) && !n.locked {
                // Without the Playlists heading.
                lists.push(track_menu::MenuList {
                    id: n.playlist.unwrap(),
                    name: n.name.clone(),
                    folder: path[path.len().min(1)..].join(" › "),
                    is_crate: n.kind == KIND_CRATE,
                });
            }
            path.push(&n.name);
        }
        self.menu_lists = lists;
        let in_list = self.nodes.get(self.current_node).is_some_and(|n| {
            (n.kind == KIND_LIST || n.kind == KIND_CRATE) && n.playlist.is_some() && !n.locked
        });
        if let Some(app) = self.app.upgrade() {
            let edit = app.global::<LibraryEdit>();
            edit.set_has_lists(!self.menu_lists.is_empty());
            edit.set_in_list(in_list);
            edit.set_can_save_search(self.saved() && self.tree == COLLECTION && !self.history_tab);
            let rule = match self.nodes.get(self.current_node).map(|n| &n.contents) {
                Some(Contents::Query(q)) if self.tree == COLLECTION => q.clone(),
                _ => String::new(),
            };
            edit.set_rule(rule.into());
            self.refresh_stage_ui(&edit);
            self.refresh_health_ui();
            app.set_folders(ModelRc::new(VecModel::from(folders)));
            let current = self
                .visible_nodes
                .iter()
                .position(|&n| n == self.current_node)
                .map(|p| p as i32)
                .unwrap_or(-1);
            app.set_current_folder(current);
            // File › Export works on the list picked here, as its menu does.
            let node = self.nodes.get(self.current_node).filter(|_| current >= 0);
            let kind = node.map_or(-1, |n| n.kind);
            let analysable = node.is_some_and(|n| !n.history && Self::analysable(&n.contents))
                && matches!(self.tree, COLLECTION | ALBUMS | MIXED_SETS);
            crate::app_menu::set_exports(crate::app_menu::Exports {
                m3u: analysable || (self.tree == COLLECTION && kind == KIND_HISTORY),
                usb: analysable,
                spotify: self.tree == COLLECTION
                    && STREAMING_ENABLED
                    && matches!(kind, KIND_LIST | KIND_HISTORY | KIND_CRATE),
                mixcloud: self.tree == MIXED_SETS,
                rekordbox: node.is_some() && self.rekordbox_exportable(self.current_node),
            });
            let name = match self.nodes.get(self.current_node) {
                Some(n) => n.title.clone().unwrap_or_else(|| n.name.clone()),
                None if self.tree == FILES => "Files".into(),
                None if self.tree == ALBUMS => "Albums".into(),
                None if self.tree == MIXED_SETS => "Recordings".into(),
                None if self.shown_stream().is_some() => {
                    self.shown_stream().map_or("", |st| st.name).into()
                }
                None if self.tree == STORES => "Bandcamp".into(),
                None => String::new(),
            };
            app.set_folder_name(name.into());
        }
    }

    /// What a node is called in full: a history set's own name ("Mon  2
    /// Nov 2026, 21:20") rather than its time on the tree.
    fn node_title(&self, i: usize) -> String {
        let n = &self.nodes[i];
        n.title.clone().unwrap_or_else(|| n.name.clone())
    }

    fn has_children(&self, i: usize) -> bool {
        self.nodes.get(i + 1).is_some_and(|next| {
            next.depth > self.nodes[i].depth && next.history == self.nodes[i].history
        })
    }

    /// What searches compare tracks against: now, the track playing, and
    /// the duplicates.
    fn context(&self) -> Context<'_> {
        Context {
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
            deck: self.deck_ref,
            deck_track: self.deck_track,
            duplicates: Some(&self.duplicates),
            health: Some(&self.health),
            crates: Some(&self.crates),
        }
    }

    /// The tracks (indices into `library`) that `q` finds, among `within`
    /// or the whole library. Missing files are left out unless asked for.
    fn search(&self, q: &Query, within: Option<&[usize]>) -> Vec<usize> {
        let cx = self.context();
        let missing_ok = q.wants_missing();
        let test = |&t: &usize| {
            let track = &self.library[t];
            (missing_ok || !track.info.missing) && q.matches(track, &self.indexed[t], &cx)
        };
        match within {
            Some(rows) => rows.iter().copied().filter(test).collect(),
            None => (0..self.library.len())
                .filter(|&t| self.in_collection(&self.library[t]))
                .filter(test)
                .collect(),
        }
    }

    /// The tempo and key of the track playing now (the first playing deck,
    /// else the first loaded one), for "mixes with deck".
    fn playing_reference(&self) -> Option<(usize, f64, Option<Key>)> {
        let decks = (0..NUM_DECKS).filter(|&d| self.play[d].get_bool());
        let deck = decks
            .chain(0..NUM_DECKS)
            .find(|&d| self.loaded[d].is_some())?;
        let t = &self.library[*self.index_of.get(&self.loaded[deck]?)?];
        // The key it plays in, shifted or sped up, once the deck has it.
        let key = Key::from_mixxx_number(self.deck_key[deck].get()).or_else(|| Key::parse(&t.key));
        Some((t.id, t.bpm, key))
    }

    /// The rule keys mix by, and how far the marks reach: the shift tracks
    /// get on load, and at least one semitone.
    fn current_key_rules(&self) -> (key::Mixing, i32) {
        (
            key::mixing(),
            (self.key_match_range.get().round() as i32).clamp(1, 2),
        )
    }

    /// Notes the track playing now; returns whether it changed.
    fn update_reference(&mut self) -> bool {
        let r = self.playing_reference();
        let track = r.map(|(id, ..)| id);
        let reference = r.map(|(_, bpm, key)| (bpm, key));
        let rules = self.current_key_rules();
        let changed =
            track != self.deck_track || reference != self.deck_ref || rules != self.key_rules;
        self.deck_track = track;
        self.deck_ref = reference;
        self.key_rules = rules;
        changed
    }

    /// Whether a key mixes harmonically with the track playing now's.
    fn mixes_with_deck(&self, key: &str) -> bool {
        self.deck_ref
            .and_then(|(_, k)| k)
            .zip(Key::parse(key))
            .is_some_and(|(a, b)| a.compatible(b))
    }

    /// The semitones a track in `key` would be shifted to mix with the
    /// track playing now, within the marks' reach; 0 when it mixes as it is
    /// or nothing in reach does.
    fn key_shift_to_deck(&self, key: &str) -> i32 {
        let (mixing, reach) = self.key_rules;
        match self.deck_ref.and_then(|(_, k)| k).zip(Key::parse(key)) {
            Some((theirs, mine)) if !mine.mixes(theirs, mixing) => {
                mine.shift_to_mix(theirs, reach, mixing).unwrap_or(0)
            }
            _ => 0,
        }
    }

    /// Whether the list shown depends on the track playing now.
    fn follows_deck(&self) -> bool {
        self.filter.filters.follows_deck()
            || Query::parse(&self.search).uses_deck()
            || self.nodes.get(self.current_node).is_some_and(
                |n| matches!(&n.contents, Contents::Query(q) if Query::parse(q).uses_deck()),
            )
    }

    fn node_tracks(&self, i: usize) -> Vec<usize> {
        // Missing files only show in their own lists, mixed sets only in
        // theirs.
        let all = || {
            (0..self.library.len())
                .filter(|&t| !self.library[t].info.missing && self.in_collection(&self.library[t]))
                .collect::<Vec<_>>()
        };
        let Some(node) = self.nodes.get(i) else {
            return Vec::new();
        };
        match &self.resolved(&node.contents) {
            Contents::Stream(_) | Contents::Store(_) | Contents::OtherStores | Contents::Wanted => {
                Vec::new()
            }
            Contents::Playlist(id) => self
                .db
                .as_ref()
                .and_then(|db| db.playlist_tracks(*id).ok())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|t| self.index_of.get(&t).copied())
                .collect(),
            Contents::HistoryDay(sets) => sets
                .iter()
                .flat_map(|id| {
                    self.db
                        .as_ref()
                        .and_then(|db| db.playlist_tracks(*id).ok())
                        .unwrap_or_default()
                })
                .filter_map(|t| self.index_of.get(&t).copied())
                .collect(),
            Contents::All => all(),
            Contents::Dir(dir) if self.files.dir.as_ref() == Some(dir) => self
                .files
                .paths
                .iter()
                .filter_map(|p| self.path_index.get(p).copied())
                .collect(),
            Contents::Dir(_) => Vec::new(),
            Contents::Under(dir) => self.tracks_under(dir),
            Contents::InboxAlbum(dir) => self.inbox_album_tracks(dir),
            Contents::Albums(dirs) => dirs.iter().flat_map(|d| self.album_tracks(d)).collect(),
            Contents::List(ids) => ids.clone(),
            Contents::Stage(stage, dir) => all()
                .into_iter()
                .filter(|&t| self.at_stage(&self.library[t], *stage, dir.as_deref()))
                .collect(),
            Contents::Query(q) => self.search(&Query::parse(q), None),
            Contents::Health(Some(Check::Duplicate)) => self.dupe_tracks(None),
            Contents::Dupes(case) => self.dupe_tracks(Some(*case)),
            Contents::Health(Some(check)) => self.health_tracks(*check),
            Contents::Health(None) | Contents::Area(_) => Vec::new(),
            Contents::Ignored => self.ignored_tracks(),
            Contents::Grouping(g) => all()
                .into_iter()
                .filter(|&t| self.library[t].info.grouping == *g)
                .collect(),
            Contents::Label(l) => all()
                .into_iter()
                .filter(|&t| {
                    self.library[t]
                        .info
                        .tags
                        .iter()
                        .any(|x| djsugar_engine::same_tag(&x.name, l))
                })
                .collect(),
            Contents::Folder => {
                let depth = self.nodes[i].depth;
                let mut ids = Vec::new();
                for j in i + 1..self.nodes.len() {
                    if self.nodes[j].depth <= depth || self.nodes[j].history != node.history {
                        break;
                    }
                    for t in self.node_tracks(j) {
                        if !ids.contains(&t) {
                            ids.push(t);
                        }
                    }
                }
                ids
            }
        }
    }

    /// The tracks of the selected node, with Spotify tracks matched to the
    /// collection.
    fn row_data(&self) -> Vec<RowData<'_>> {
        let local = |id: usize| {
            let t = &self.library[id];
            RowData {
                title: &t.title,
                artist: t.artist.clone(),
                album: &t.album,
                genre: &t.genre,
                bpm: t.bpm,
                key: &t.key,
                duration: t.duration,
                rating: t.rating,
                local: Some(id),
                stream: None,
                list: Default::default(),
            }
        };
        // An imported list with streamed tracks standing in it: those the
        // collection has now show as its tracks.
        if let Some(placed) = &self.placed.entries {
            let found = self.placed_matches();
            let lists = &self.placed.lists;
            return placed
                .iter()
                .zip(lists)
                .map(|(p, &list)| RowData {
                    list,
                    ..match *p {
                        streamed::Placed::Track(i) => local(i),
                        streamed::Placed::Streamed(n) => match found[n] {
                            Some(id) => RowData {
                                stream: Some(n),
                                ..local(id)
                            },
                            None => {
                                let t = &self.placed.tracks[n].1;
                                RowData {
                                    title: &t.title,
                                    artist: t.artist(),
                                    album: &t.album,
                                    genre: "",
                                    bpm: 0.0,
                                    key: "",
                                    duration: t.duration,
                                    rating: 0,
                                    local: None,
                                    stream: Some(n),
                                    list: Default::default(),
                                }
                            }
                        },
                    }
                })
                .collect();
        }
        // One of the DJ's lists or a history session: each track with its
        // place, and in a session when it was played.
        if let (Some(Contents::Playlist(id)), Some(db)) = (
            self.nodes.get(self.current_node).map(|n| &n.contents),
            &self.db,
        ) {
            let history = self.nodes[self.current_node].history;
            let tracks = db.playlist_tracks(*id).unwrap_or_default();
            let times = if history {
                db.playlist_times(*id).unwrap_or_default()
            } else {
                Vec::new()
            };
            return tracks
                .into_iter()
                .enumerate()
                .filter_map(|(n, t)| {
                    let i = *self.index_of.get(&t)?;
                    Some(RowData {
                        list: columns::InList {
                            order: n + 1,
                            played_at: times.get(n).copied().flatten(),
                            set_start: false,
                        },
                        ..local(i)
                    })
                })
                .collect();
        }
        // A day of history: its sets one after another, numbered through,
        // each set after the first marked where it starts.
        if let (Some(Contents::HistoryDay(sets)), Some(db)) = (
            self.nodes.get(self.current_node).map(|n| &n.contents),
            &self.db,
        ) {
            let mut rows = Vec::new();
            for (s, id) in sets.iter().enumerate() {
                let tracks = db.playlist_tracks(*id).unwrap_or_default();
                let times = db
                    .playlist_times(*id)
                    .ok()
                    .filter(|t| t.len() == tracks.len())
                    .unwrap_or_default();
                let mut first = s > 0;
                for (n, t) in tracks.into_iter().enumerate() {
                    let Some(&i) = self.index_of.get(&t) else {
                        continue;
                    };
                    rows.push(RowData {
                        list: columns::InList {
                            order: rows.len() + 1,
                            played_at: times.get(n).copied().flatten(),
                            set_start: std::mem::take(&mut first),
                        },
                        ..local(i)
                    });
                }
            }
            return rows;
        }
        let Some(key) = self.current_playlist() else {
            return self
                .node_tracks(self.current_node)
                .into_iter()
                .map(local)
                .collect();
        };
        let Some(tracks) = self.shown_stream().and_then(|st| st.state.tracks.get(key)) else {
            return Vec::new();
        };
        tracks
            .iter()
            .zip(self.spotify_matches(tracks))
            .enumerate()
            .map(|(n, (sp, found))| {
                match found {
                    // Show what Spotify calls it, with what the collection knows.
                    Some(id) => RowData {
                        title: &sp.title,
                        artist: sp.artist(),
                        album: &sp.album,
                        duration: sp.duration,
                        stream: Some(n),
                        ..local(id)
                    },
                    None => RowData {
                        title: &sp.title,
                        artist: sp.artist(),
                        album: &sp.album,
                        genre: "",
                        bpm: 0.0,
                        key: "",
                        duration: sp.duration,
                        rating: 0,
                        local: None,
                        stream: Some(n),
                        list: Default::default(),
                    },
                }
            })
            .collect()
    }

    /// The collection track (by index into `library`) that each Spotify
    /// track is, if any: the same ISRC first, else the same title and artist.
    fn spotify_matches(&self, tracks: &[StreamTrack]) -> Vec<Option<usize>> {
        let none = HashMap::new();
        let manual = self.shown_stream().map_or(&none, |st| &st.state.manual);
        let tracks: Vec<_> = tracks.iter().map(|t| (t, Some(manual))).collect();
        self.match_streamed(&tracks, false)
    }

    /// The collection track (by index into `library`) that each streaming
    /// service's track is, if any: the one the DJ set by hand (in its
    /// `manual` matches), else the same ISRC, else the same title and
    /// artist, with `same_length` at the same length too where both are
    /// known.
    fn match_streamed(
        &self,
        tracks: &[(&StreamTrack, Option<&Manual>)],
        same_length: bool,
    ) -> Vec<Option<usize>> {
        // Files only shown in Files don't count as in the collection.
        let normalized: Vec<(String, String)> = self
            .library
            .iter()
            .map(|t| match is_browsed(t) {
                true => Default::default(),
                false => (spotify::normalize(&t.title), spotify::normalize(&t.artist)),
            })
            .collect();
        let mut by_isrc = HashMap::new();
        for (i, t) in self.library.iter().enumerate() {
            let isrc = spotify::normalize_isrc(&t.info.isrc);
            if !isrc.is_empty() && !is_browsed(t) {
                by_isrc.entry(isrc).or_insert(i);
            }
        }
        let length_fits = |sp: &StreamTrack, i: usize| {
            let d = self.library[i].duration;
            !same_length || sp.duration <= 0.0 || d <= 0.0 || (sp.duration - d).abs() <= 2.0
        };
        tracks
            .iter()
            .map(|(sp, manual)| {
                // A match the DJ set by hand wins.
                if let Some(set) = manual.and_then(|m| m.get(&sp.match_key())) {
                    return (*set).and_then(|id| self.index_of.get(&id).copied());
                }
                by_isrc
                    .get(&spotify::normalize_isrc(&sp.isrc))
                    .copied()
                    .or_else(|| {
                        normalized
                            .iter()
                            .enumerate()
                            .position(|(i, (title, artist))| {
                                spotify::matches(sp, title, artist) && length_fits(sp, i)
                            })
                    })
            })
            .collect()
    }

    /// The page of Spotify playlist `index` in the tree (Liked Songs: the
    /// DJ's liked songs), when Spotify is shown.
    pub fn spotify_playlist_link(&self, index: usize) -> Option<String> {
        if self.tree != SPOTIFY {
            return None;
        }
        match &self.nodes.get(index)?.contents {
            Contents::Stream(k) if k == spotify::LIKED => {
                Some("https://open.spotify.com/collection/tracks".into())
            }
            Contents::Stream(k) => Some(format!("https://open.spotify.com/playlist/{k}")),
            _ => None,
        }
    }

    /// A line under the shown service's playlists, e.g. where downloads went.
    pub fn set_stream_status(&mut self, text: String) {
        if let Some(st) = self.shown_stream_mut() {
            st.status = text;
        }
        self.show_stream();
    }

    /// The Spotify track shown at `row`, if a playlist is shown, or the
    /// streamed track standing in an imported list there.
    fn stream_track(&self, row: usize) -> Option<&StreamTrack> {
        if self.placed.entries.is_some() {
            return self.placed_at(row).map(|(_, t)| t);
        }
        let n = (*self.stream_rows.get(row)?)?;
        self.shown_stream()?
            .state
            .tracks
            .get(self.current_playlist()?)?
            .get(n)
    }

    /// Fills the match menu for the track at `row`: its name as the query,
    /// and the collection tracks that fit it.
    fn open_match(&mut self, row: usize) {
        let Some(sp) = self.stream_track(row) else {
            return;
        };
        let key = sp.match_key();
        let query = format!("{} {}", sp.artist(), sp.title);
        let manual = match self.placed_at(row) {
            Some((service, _)) => self
                .placed
                .manual
                .get(service)
                .is_some_and(|m| m.contains_key(&key)),
            None => self
                .shown_stream()
                .is_some_and(|st| st.state.manual.contains_key(&key)),
        };
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<StreamMatch>();
            ui.set_query(query.clone().into());
            ui.set_manual(manual);
        }
        self.search_match(&query);
    }

    /// The collection tracks whose artist and title hold every word typed,
    /// best known first.
    fn search_match(&mut self, text: &str) {
        let words: Vec<String> = text
            .split_whitespace()
            .map(spotify::normalize)
            .filter(|w| !w.is_empty())
            .collect();
        let mut found: Vec<usize> = self
            .library
            .iter()
            .enumerate()
            .filter(|(_, t)| !is_browsed(t))
            .filter(|(_, t)| {
                let hay = spotify::normalize(&format!("{} {}", t.artist, t.title));
                !words.is_empty() && words.iter().all(|w| hay.contains(w.as_str()))
            })
            .map(|(i, _)| i)
            .collect();
        found.sort_by_key(|&i| std::cmp::Reverse(self.library[i].info.play_count));
        found.truncate(MATCH_CANDIDATES);
        let lines: Vec<SharedString> = found
            .iter()
            .map(|&i| {
                let t = &self.library[i];
                format!("{} – {}", t.artist, t.title).into()
            })
            .collect();
        self.match_candidates = found;
        if let Some(app) = self.app.upgrade() {
            app.global::<StreamMatch>()
                .set_candidates(ModelRc::new(VecModel::from(lines)));
        }
    }

    /// Saves which collection track (None: none of them) the track at `row`
    /// is, or with `forget` goes back to matching it automatically.
    fn set_match(&mut self, row: usize, track: Option<usize>, forget: bool) {
        let Some(key) = self.stream_track(row).map(StreamTrack::match_key) else {
            return;
        };
        let id = track.map(|i| self.library[i].id);
        let placed = self.placed_at(row).map(|(service, _)| service.clone());
        let Some(service) = placed
            .clone()
            .or_else(|| self.shown_stream().map(|st| st.id.to_string()))
        else {
            return;
        };
        if let Some(db) = &self.db {
            if let Err(e) = db.set_stream_match(&service, &key, id, forget) {
                eprintln!("can't save the match: {e}");
                return;
            }
        }
        let manual = match placed {
            Some(service) => self.placed.manual.entry(service).or_default(),
            None => match self.shown_stream_mut() {
                Some(st) => &mut st.state.manual,
                None => return,
            },
        };
        if forget {
            manual.remove(&key);
        } else {
            manual.insert(key, id);
        }
        self.refresh_rows();
    }

    /// Copies the streaming playlist being shown into a new list of the
    /// DJ's own, once: the tracks the collection has go in the list, the
    /// rest on the wanted list.
    fn copy_stream_playlist(&mut self) {
        let status = |b: &mut Self, s: &str| {
            if let Some(st) = b.shown_stream_mut() {
                st.status = s.to_string();
            }
            b.show_stream();
        };
        let Some(key) = self.current_playlist().map(str::to_string) else {
            return status(self, "Pick a playlist to copy.");
        };
        let Some((service, tracks, playlist)) = self.shown_stream().map(|st| {
            (
                st.name,
                st.state.tracks.get(&key).cloned(),
                st.state
                    .playlists
                    .iter()
                    .find(|p| p.key == key)
                    .map(|p| p.name.clone()),
            )
        }) else {
            return;
        };
        let Some(tracks) = tracks else {
            return status(self, "Wait for the playlist to load, then copy it.");
        };
        if !self.saved() {
            return status(self, "There's no collection to copy into.");
        }
        let name = playlist.unwrap_or_else(|| format!("{service} playlist"));
        let found = self.spotify_matches(&tracks);
        let mut have: Vec<usize> = Vec::new();
        let mut missing = 0;
        let Some(db) = &self.db else { return };
        let list = match db.create_top_playlist(&name) {
            Ok(id) => id,
            Err(e) => return status(self, &format!("Couldn't make the list: {e}")),
        };
        for (sp, f) in tracks.iter().zip(found) {
            match f {
                // The same track twice in a playlist goes in once.
                Some(i) if !have.contains(&i) => have.push(i),
                Some(_) => {}
                None => {
                    missing += 1;
                    let _ = db.add_wanted(service, &sp.artist(), &sp.title, &sp.page);
                }
            }
        }
        self.store.wanted = db.wanted().unwrap_or_default();
        self.add_to_playlist(&have, list);
        // The collection's tree shows the new list when it's next opened.
        self.collection_stale = true;
        status(
            self,
            &match missing {
                0 => format!("Copied all {} tracks to \"{name}\".", have.len()),
                n => format!(
                    "Copied {} {} to \"{name}\". {n} not in your collection went on the wanted list.",
                    have.len(),
                    if have.len() == 1 { "track" } else { "tracks" },
                ),
            },
        );
    }

    /// What the list says instead of rows while a playlist loads or fails.
    fn list_notice(&self) -> String {
        if matches!(self.tree, ALBUMS | MIXED_SETS) && self.nodes.is_empty() {
            return if self.tree == ALBUMS {
                "Choose your albums folder in Settings › Collection."
            } else {
                "Choose your Recordings folder in Settings › Collection. It holds your own recordings and other DJ mixes."
            }
            .into();
        }
        if self.tree == FILES {
            let f = &self.files;
            return if self.nodes.is_empty() {
                "No folders to show.".into()
            } else if f.dir.is_none() {
                "Pick a folder on the left.".into()
            } else if f.paths.is_empty() && f.reading {
                "Reading…".into()
            } else if f.paths.is_empty() && f.recursive {
                "No music in this folder or its subfolders.".into()
            } else if f.paths.is_empty() {
                "No music in this folder. Turn on Include subfolders to see what's below it.".into()
            } else {
                String::new()
            };
        }
        let Some(key) = self.current_playlist() else {
            return String::new();
        };
        let Some(st) = self.shown_stream().map(|st| &st.state) else {
            return String::new();
        };
        if let Some(e) = st.errors.get(key) {
            e.clone()
        } else if st.loading.contains(key) && !st.tracks.contains_key(key) {
            "Loading…".into()
        } else if st.tracks.get(key).is_some_and(|t| t.is_empty()) {
            "This playlist is empty.".into()
        } else {
            String::new()
        }
    }

    /// Whether the list shows near misses because nothing matched the
    /// search as typed.
    fn set_close_matches(&self, close: bool) {
        if let Some(app) = self.app.upgrade() {
            app.global::<LibraryEdit>().set_close_matches(close);
        }
    }

    fn refresh_rows(&mut self) {
        self.fill_rows();
        self.follow_view();
    }

    fn fill_rows(&mut self) {
        if let Some(app) = self.app.upgrade() {
            app.global::<WantedUi>().set_shown(self.shows_wanted());
        }
        if self.shows_store_rows() {
            self.set_close_matches(false);
            self.refresh_store_rows();
            return;
        }
        self.load_placed();
        self.follow_list_sort();
        // Collection tracks are searched with the full syntax, filters
        // included; Spotify tracks the collection doesn't have, by their
        // names, and only while nothing is filtered.
        let q = Query::parse(&self.full_search());
        let cx = self.context();
        let needle = query::fold(&self.search);
        let filtering = self.filtering();
        let all = self.row_data();
        let keep = |q: &Query, r: &RowData| match r.local {
            _ if q.is_empty() => true,
            Some(id) => q.matches(&self.library[id], &self.indexed[id], &cx),
            None if filtering => false,
            None => [r.title, &r.artist, r.album]
                .iter()
                .any(|t| query::fold(t).contains(&needle)),
        };
        let mut kept: Vec<bool> = all.iter().map(|r| keep(&q, r)).collect();
        // Nothing as typed: a letter off, and the count says so.
        let mut close = false;
        if !self.search.trim().is_empty() && !kept.contains(&true) {
            if let Some(loose) = q.loosened() {
                kept = all.iter().map(|r| keep(&loose, r)).collect();
                close = kept.contains(&true);
            }
        }
        self.set_close_matches(close);
        let mut data: Vec<RowData> = all
            .into_iter()
            .zip(kept)
            .filter_map(|(r, k)| k.then_some(r))
            .collect();
        // A Spotify playlist keeps its own order until a column is clicked.
        // So does a folder in Files, in file name order, and an album.
        // Copies stay in their groups, the one kept first.
        let dupes = self.dupe_view();
        let keep_order = ((self.current_playlist().is_some()
            || self.placed.entries.is_some()
            || matches!(self.tree, FILES | ALBUMS | MIXED_SETS))
            && self.sort_column < 0)
            || dupes.is_some();
        let fade = dupes.map(|case| self.dupe_faded(case)).unwrap_or_default();
        if !keep_order {
            let library = &self.library;
            data.sort_by(|a, b| {
                use std::cmp::Ordering;
                let o = match self.sort_column {
                    columns::ARTIST => a.artist.cmp(&b.artist),
                    columns::TITLE_ID => a.title.cmp(b.title),
                    columns::ALBUM => a.album.cmp(b.album),
                    columns::GENRE => a.genre.cmp(b.genre),
                    columns::BPM => a.bpm.total_cmp(&b.bpm),
                    columns::KEY_ID => columns::key_order(a.key, self.camelot)
                        .cmp(&columns::key_order(b.key, self.camelot)),
                    columns::LENGTH => a.duration.total_cmp(&b.duration),
                    columns::RATING_ID => a.rating.cmp(&b.rating),
                    columns::ORDER => a.list.order.cmp(&b.list.order),
                    columns::PLAYED_AT => a.list.played_at.cmp(&b.list.played_at),
                    // What only the collection knows: Spotify tracks it
                    // doesn't have go last.
                    columns::CRATES => {
                        let names = |r: &RowData| {
                            r.local
                                .and_then(|i| self.crates.get(&library[i].id))
                                .map(|c| c.join(", "))
                                .unwrap_or_default()
                        };
                        let (x, y) = (names(a), names(b));
                        // Tracks in no crate last.
                        (x.is_empty(), x.to_lowercase()).cmp(&(y.is_empty(), y.to_lowercase()))
                    }
                    id => match (a.local, b.local) {
                        (Some(x), Some(y)) => columns::compare(id, &library[x], &library[y]),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    },
                };
                if self.ascending {
                    o
                } else {
                    o.reverse()
                }
            });
        }
        let art = &self.art;
        let library = &self.library;
        let ids = self.columns.ids();
        let rows: Vec<TrackRow> = data
            .iter()
            .map(|r| {
                let cover = r.local.and_then(|id| art.get(&library[id]));
                let color = r.local.and_then(|id| library[id].info.color);
                let row = TrackRow {
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                    title: r.title.into(),
                    artist: r.artist.clone().into(),
                    album: r.album.into(),
                    genre: r.genre.into(),
                    length: short_time(r.duration),
                    // ~ marks a grid in tempo sections, as VirtualDJ does.
                    bpm: if r.bpm > 0.0 {
                        let sections = r.local.is_some_and(|id| library[id].grid.is_dynamic());
                        format!("{}{:.1}", if sections { "~" } else { "" }, r.bpm).into()
                    } else {
                        SharedString::new()
                    },
                    key: show_key(r.key, self.camelot).into(),
                    key_match: self.mixes_with_deck(r.key),
                    key_shift: self.key_shift_to_deck(r.key),
                    rating: r.rating as i32,
                    deck: r
                        .local
                        .and_then(|id| {
                            let id = self.library[id].id;
                            self.loaded.iter().position(|l| *l == Some(id))
                        })
                        .map(|d| d as i32)
                        .unwrap_or(-1),
                    available: r.local.is_some(),
                    missing: r.local.is_some_and(|id| library[id].info.missing),
                    has_color: color.is_some(),
                    color: color
                        .map(|c| {
                            slint::Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
                        })
                        .unwrap_or_default(),
                    action: SharedString::new(),
                    cells: Default::default(),
                    wave: Default::default(),
                    has_wave: false,
                    analysing: 0,
                    fade: r.local.is_some_and(|id| fade.contains(&library[id].id)),
                    streamed: r.stream.is_some(),
                    // Only while the day is in play order.
                    set_start: r.list.set_start && self.sort_column == columns::ORDER,
                };
                let crates = r.local.and_then(|id| self.crates.get(&library[id].id));
                TrackRow {
                    cells: columns::row_cells(
                        &ids,
                        &row,
                        r.local.map(|id| &*library[id]),
                        crates.map(Vec::as_slice),
                        r.list,
                        r.local.map_or_else(Default::default, |id| {
                            self.prep_facts(&library[id], &ids)
                        }),
                    ),
                    ..row
                }
            })
            .collect();
        let total: f64 = data.iter().map(|r| r.duration).sum();
        let mut summary = total_time(total);
        if (self.current_playlist().is_some() || self.placed.entries.is_some()) && !data.is_empty()
        {
            let have = data.iter().filter(|r| r.local.is_some()).count();
            summary = format!("{summary} · {have} in your collection");
        }
        if let Some(note) = self.export_note() {
            summary = format!("{summary} · {note}");
        }
        if self.tree == FILES && self.files.reading && !data.is_empty() {
            summary = format!(
                "{summary} · reading {} of {}…",
                data.len(),
                self.files.paths.len()
            );
        }
        let notice = self.list_notice();
        let streams: Vec<Option<usize>> = data.iter().map(|r| r.stream).collect();
        let picked = self.picked_tracks();
        self.rows = data.iter().map(|r| r.local).collect();
        self.stream_rows = streams;
        // The same tracks stay picked, where the list still shows them.
        self.repick(&picked);
        if let Some(app) = self.app.upgrade() {
            let current = app.get_current_track();
            app.set_folder_duration(summary.into());
            app.set_list_notice(notice.into());
            self.list.set_vec(rows);
            self.wave_rows.clear();
            self.fill_waves();
            self.analysis_shown.marks.clear();
            self.mark_analysis_rows();
            app.set_tracks(ModelRc::from(self.list.clone()));
            // Copies are listed in their groups, whatever the sort.
            app.set_sort_column(if dupes.is_some() {
                -1
            } else {
                self.sort_column
            });
            app.set_sort_ascending(self.ascending);
            if current >= self.rows.len() as i32 {
                app.set_current_track(-1);
            }
        }
        self.refresh_album_ui();
    }

    /// Sets the sort and columns up for the node shown: one of the DJ's
    /// lists or a history session opens in its own order, and a session
    /// shows its own columns (the play order and times first).
    fn follow_list_sort(&mut self) {
        let node = self.nodes.get(self.current_node);
        let list = node
            .and_then(|n| match &n.contents {
                // A crate has no order of its own: it keeps the collection's sort.
                Contents::Playlist(_) if n.kind == KIND_CRATE => None,
                Contents::Playlist(id) => Some(*id),
                // Keyed apart from the lists by its first set.
                Contents::HistoryDay(sets) => sets.first().map(|id| -id),
                _ => None,
            })
            .filter(|_| self.tree == COLLECTION);
        let history = list.is_some() && node.is_some_and(|n| n.history);
        let prep = node.is_some_and(|n| matches!(n.contents, Contents::Stage(Stage::Triage, _)));
        let area = self.health_view
            && node.is_some_and(|n| matches!(n.contents, Contents::Area(_) | Contents::Ignored));
        let kind = if history {
            columns::Columns::History
        } else if area {
            columns::Columns::Health
        } else if prep {
            columns::Columns::Prep
        } else {
            columns::Columns::Lists
        };
        if self.columns.show(kind) {
            self.refresh_column_ui();
        }
        if self.tree != COLLECTION || list == self.sorted_list {
            return;
        }
        match (self.sorted_list, list) {
            (None, Some(_)) => {
                self.view_sort = (self.sort_column, self.ascending);
                (self.sort_column, self.ascending) = (columns::ORDER, true);
            }
            (Some(_), Some(_)) => (self.sort_column, self.ascending) = (columns::ORDER, true),
            (Some(_), None) => (self.sort_column, self.ascending) = self.view_sort,
            (None, None) => {}
        }
        self.sorted_list = list;
    }

    /// Lists the rows again, re-sorted, with the selected track still
    /// selected at its new place (the list then scrolls to it).
    fn refresh_rows_keeping_current(&mut self) {
        let app = match self.app.upgrade() {
            Some(app) if self.shows_wanted() => return self.resort_wanted(&app),
            Some(app) if !self.shows_store_rows() => app,
            _ => return self.refresh_rows(),
        };
        let current = usize::try_from(app.get_current_track()).ok();
        // A collection track by its entry; any other row by its names.
        let names = |r: &TrackRow| (r.title.clone(), r.artist.clone(), r.album.clone());
        let was = current.and_then(|i| Some((*self.rows.get(i)?, self.list.row_data(i)?)));
        self.refresh_rows();
        let Some((local, row)) = was else {
            return;
        };
        let at = match local {
            Some(id) => self.rows.iter().position(|r| *r == Some(id)),
            None => (0..self.list.row_count()).find(|&i| {
                self.rows[i].is_none()
                    && self
                        .list
                        .row_data(i)
                        .is_some_and(|r| names(&r) == names(&row))
            }),
        };
        app.set_current_track(at.map_or(-1, |i| i as i32));
    }

    fn refresh_side(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let tab = self.side_tab();
        if tab == LINKED_TAB {
            self.refresh_linked(&app);
            return;
        }
        if tab == info::INFO_TAB {
            return;
        }
        let ids = &self.side[tab];
        let row = |id: usize, suggested: bool| {
            let t = &self.library[id];
            let cover = self.art.get(t);
            let picks = if tab == 0 && !suggested {
                self.mix_into(id)
            } else {
                (None, None)
            };
            let pick = |p: Option<usize>| p.map_or(0, |p| p as i32 + 1);
            SideRow {
                heading: SharedString::new(),
                title: t.title.clone().into(),
                artist: t.artist.clone().into(),
                bpm: if t.bpm > 0.0 {
                    format!("{:.0}", t.bpm).into()
                } else {
                    SharedString::new()
                },
                key: show_key(&t.key, self.camelot).into(),
                key_match: self.mixes_with_deck(&t.key),
                key_shift: self.key_shift_to_deck(&t.key),
                length: short_time(t.duration),
                has_cover: cover.is_some(),
                cover: cover.unwrap_or_default(),
                suggested,
                mix: up_next::label(picks).into(),
                mix_length: pick(picks.0),
                mix_style: pick(picks.1),
            }
        };
        let mut rows: Vec<SideRow> = ids.iter().map(|&id| row(id, false)).collect();
        if tab == 0 && self.keeps_going() && !self.keep_going.picks.is_empty() {
            rows.push(SideRow {
                heading: self.keep_going_heading().into(),
                suggested: true,
                ..Default::default()
            });
            rows.extend(self.keep_going.picks.iter().map(|&id| row(id, true)));
        }
        let total: f64 = ids.iter().map(|&id| self.library[id].duration).sum();
        app.set_side_total(
            format!(
                "{} · {} song{}",
                short_time(total),
                ids.len(),
                if ids.len() == 1 { "" } else { "s" }
            )
            .into(),
        );
        app.set_side_rows(ModelRc::new(VecModel::from(rows)));
    }

    /// The side panel's rows as library indices, `None` for a heading.
    fn side_ids(&self) -> Vec<Option<usize>> {
        self.side_ids_of(self.side_tab())
    }

    /// The tracks of a side panel tab, `None` for the Linked tab's headings.
    fn side_ids_of(&self, tab: usize) -> Vec<Option<usize>> {
        match tab {
            LINKED_TAB => self.linked.clone(),
            tab => self
                .side
                .get(tab)
                .map(|l| l.iter().map(|&i| Some(i)).collect())
                .unwrap_or_default(),
        }
    }

    fn side_id(&self, row: usize) -> Option<usize> {
        self.side_ids().get(row).copied().flatten()
    }

    /// Fills the Linked tab for the selected track: tracks linked to it,
    /// then those played right before or after it in the history.
    fn refresh_linked(&mut self, app: &AppWindow) {
        self.linked.clear();
        let subject = self.linked_to.map(|i| &self.library[i]);
        let (mut linked, mut history) = (Vec::new(), Vec::new());
        if let (Some(t), Some(db)) = (subject, &self.db) {
            let index = |ids: Vec<usize>| -> Vec<usize> {
                ids.into_iter()
                    .filter_map(|id| self.index_of.get(&id).copied())
                    .collect()
            };
            linked = index(db.linked(t.id).unwrap_or_default());
            history = index(db.played_with(t.id).unwrap_or_default());
            history.retain(|i| !linked.contains(i));
        }
        let mut rows = Vec::new();
        for (heading, ids) in [("Linked", &linked), ("Played with", &history)] {
            if ids.is_empty() {
                continue;
            }
            self.linked.push(None);
            rows.push(SideRow {
                heading: heading.into(),
                ..Default::default()
            });
            for &i in ids {
                self.linked.push(Some(i));
                let t = &self.library[i];
                let cover = self.art.get(t);
                rows.push(SideRow {
                    heading: SharedString::new(),
                    title: t.title.clone().into(),
                    artist: t.artist.clone().into(),
                    bpm: if t.bpm > 0.0 {
                        format!("{:.0}", t.bpm).into()
                    } else {
                        SharedString::new()
                    },
                    key: show_key(&t.key, self.camelot).into(),
                    key_match: self.mixes_with_deck(&t.key),
                    key_shift: self.key_shift_to_deck(&t.key),
                    length: short_time(t.duration),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                    suggested: false,
                    ..Default::default()
                });
            }
        }
        self.link_rows = if linked.is_empty() {
            0
        } else {
            linked.len() + 1
        };
        app.set_side_total(match subject {
            Some(t) if t.artist.is_empty() => t.title.clone().into(),
            Some(t) => format!("{} - {}", t.artist, t.title).into(),
            None => SharedString::new(),
        });
        app.set_side_rows(ModelRc::new(VecModel::from(rows)));
    }

    /// Links a track dropped on the Linked tab to the selected one.
    fn link(&mut self, index: usize) {
        if let Some(subject) = self.linked_to {
            self.adopt(&[subject, index]);
        }
        let (Some(subject), Some(db)) = (self.linked_to, &self.db) else {
            return;
        };
        let _ = db.link(self.library[subject].id, self.library[index].id);
        // Picking the dragged track up selected it; go back to the one the
        // tab is showing.
        if let (Some(app), Some(row)) = (
            self.app.upgrade(),
            self.rows.iter().position(|r| *r == Some(subject)),
        ) {
            app.set_current_track(row as i32);
        }
        self.refresh_side();
    }

    /// Unlinks a row of the Linked tab; history rows stay, as they are
    /// what was played.
    fn unlink_row(&mut self, row: usize) {
        let is_link = row < self.link_rows;
        let (Some(subject), Some(Some(other)), Some(db)) =
            (self.linked_to, self.linked.get(row).copied(), &self.db)
        else {
            return;
        };
        if is_link {
            let _ = db.unlink(self.library[subject].id, self.library[other].id);
            self.refresh_side();
        }
    }
}

/// The side panel's third tab, after Automix and Sidelist.
const LINKED_TAB: usize = 2;

fn short_time(seconds: f64) -> SharedString {
    fmt_time(seconds)
        .split('.')
        .next()
        .unwrap_or_default()
        .to_string()
        .into()
}

/// A list's total length as the header shows it: "3h 56m", or "42m".
fn total_time(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as u64;
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod total_time_tests {
    use super::total_time;

    #[test]
    fn formats_times_and_sizes() {
        assert_eq!(total_time(14_160.0), "3h 56m");
        assert_eq!(total_time(2_520.0), "42m");
        assert_eq!(total_time(3_600.0), "1h 0m");
        assert_eq!(total_time(0.0), "0m");
    }
}

#[cfg(test)]
mod drag_tests {
    use super::{move_block, parse_dragged, Dragged};

    #[test]
    fn reads_what_a_drag_carries() {
        assert_eq!(parse_dragged("djsugar-row:3"), Some(Dragged::Rows(3, 3)));
        assert_eq!(parse_dragged("djsugar-row:2-5"), Some(Dragged::Rows(2, 5)));
        assert_eq!(
            parse_dragged("djsugar-side:0:1-2"),
            Some(Dragged::Side(0, 1, 2))
        );
        assert_eq!(
            parse_dragged("djsugar-side:1:4"),
            Some(Dragged::Side(1, 4, 4))
        );
        assert_eq!(parse_dragged("djsugar-deck:1"), Some(Dragged::Deck(1)));
        assert_eq!(parse_dragged("djsugar-deck:9"), None);
        assert_eq!(parse_dragged("djsugar-row:5-2"), None);
        assert_eq!(parse_dragged("hello"), None);
    }

    #[test]
    fn moves_rows_within_a_list() {
        let moved = |first, last, at| {
            let mut v = vec![0, 1, 2, 3, 4];
            move_block(&mut v, first, last, at);
            v
        };
        assert_eq!(moved(3, 3, 0), [3, 0, 1, 2, 4], "up to the top");
        assert_eq!(moved(0, 0, 5), [1, 2, 3, 4, 0], "down to the end");
        assert_eq!(
            moved(1, 2, 4),
            [0, 3, 1, 2, 4],
            "a picked range moves together"
        );
        assert_eq!(moved(1, 2, 2), [0, 1, 2, 3, 4], "dropped on itself");
        assert_eq!(moved(1, 2, 3), [0, 1, 2, 3, 4], "just below itself");
        assert_eq!(
            moved(4, 9, 0),
            [4, 0, 1, 2, 3],
            "range past the end is clipped"
        );
    }
}

#[cfg(test)]
mod stream_state_tests {
    use super::{spotify::Playlist, StreamState};

    fn playlist(key: &str, version: &str) -> Playlist {
        Playlist {
            key: key.into(),
            name: key.into(),
            total: 0,
            version: version.into(),
        }
    }

    #[test]
    fn keeps_tracks_and_marks_unchanged_playlists_fresh() {
        let mut old = StreamState {
            connected: true,
            ..Default::default()
        };
        for (key, version) in [
            ("same", "v1"),
            ("changed", "v1"),
            ("liked", ""),
            ("gone", "v1"),
        ] {
            old.tracks.insert(key.into(), Vec::new());
            old.versions.insert(key.into(), version.into());
        }
        let new = StreamState::merged(
            old,
            vec![
                playlist("same", "v1"),
                playlist("changed", "v2"),
                playlist("liked", ""),
            ],
        );
        assert!(new.connected);
        assert!(new.fresh.contains("same"));
        assert!(!new.fresh.contains("changed"), "fetched again when opened");
        assert!(!new.fresh.contains("liked"), "no version: fetched again");
        assert!(new.tracks.contains_key("changed"), "shown until then");
        assert!(!new.tracks.contains_key("gone"));
    }
}

#[cfg(test)]
mod naming_tests {
    use super::untaken_name;

    #[test]
    fn new_lists_are_numbered_past_taken_names() {
        assert_eq!(untaken_name("New playlist", &["Warm-up"]), "New playlist");
        assert_eq!(
            untaken_name("New playlist", &["new playlist"]),
            "New playlist 2"
        );
        assert_eq!(
            untaken_name("New crate", &["New crate", "New crate 2", "New crate 4"]),
            "New crate 3"
        );
    }
}
