//! Sources in the rail that come from add-ons (see
//! [`djsugar_plugins::source`]): saved searches on the left, what they found
//! in the list, and downloads that join the collection when they finish.
//!
//! Each source runs on a thread of its own, which also asks how searches
//! and downloads are getting on about once a second, so a slow network
//! never holds up the UI or the audio.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use djsugar_engine::Track;
use djsugar_plugins::inbox::safe_name;
use djsugar_plugins::source::{
    megabytes, DownloadUpdate, Found, Prefer, Progress, QueryKind, SearchSource, Snapshot,
    SourceInfo, SourceQuery, StatusPage, DOWNLOAD_FOLDER,
};
use djsugar_plugins::{Plugin, PluginConfig, PluginError, Setting, SettingKind};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::{
    AppWindow, FoundRow, QueueLine, SearchItem, SetupField, SourceTab, SourceUi, StatusLine,
    StoreUi,
};

/// Where plugin sources start in the rail, after the built-in ones.
pub const FIRST_SOURCE: i32 = crate::browser::SOURCE_NAMES.len() as i32;

/// How often searches and downloads are asked how they're getting on.
const TICK: Duration = Duration::from_millis(900);

/// How often a source's own page (see [`SearchSource::status`]) is asked
/// for.
/// How often a preview waiting for its first bytes asks for them.
const PREVIEW_TICK: Duration = Duration::from_millis(400);
/// How long a preview waits for the other user before giving up.
const PREVIEW_WAIT: Duration = Duration::from_secs(45);
const STATUS_TICK: Duration = Duration::from_secs(3);

/// A square picture's side length and RGBA pixels.
type Thumbnail = (u32, Vec<u8>);

/// How big a result's cover is fetched, in pixels (shown at 40).
const COVER_SIDE: u32 = 96;

/// Looks in a row (about a second apart) a playlist entry's results must
/// stay the same before its best copy is downloaded by itself.
const SETTLED: u32 = 3;

/// Formats that count as lossless for "Lossless only".
const LOSSLESS: [&str; 5] = ["FLAC", "WAV", "AIFF", "ALAC", "APE"];

/// The plugins with a source of their own, and what each is called.
pub fn source_plugins() -> Vec<(Box<dyn Plugin>, SourceInfo)> {
    crate::stores::configured()
        .into_iter()
        .filter_map(|mut p| {
            let info = p.source()?.info();
            Some((p, info))
        })
        .collect()
}

enum Request {
    Start(u64, SourceQuery),
    Stop(u64),
    OpenFolder(u64, Box<Found>),
    Download(u64, Vec<Found>, PathBuf),
    /// Downloads into the source's cache (see [`SearchSource::plays`]),
    /// first making room so it stays under the size given, in bytes.
    Cache(u64, Vec<Found>, PathBuf, u64),
    /// Moves a cached file into the collection: its key, from, to.
    Keep(String, PathBuf, PathBuf),
    Retry(String),
    Cancel(String),
    /// Starts hearing a file of search `local` before it's downloaded (see
    /// [`SearchSource::preview`]).
    Preview(u64, Box<Found>),
    StopPreview,
    /// The DJ changed the plugin's settings.
    Configure(PluginConfig),
}

enum Event {
    /// What search `local` has found so far.
    Found(usize, u64, Snapshot),
    /// Search `local` couldn't start or went wrong.
    SearchFailed(usize, u64, String),
    /// A folder's files, read when the DJ opened it.
    Folder(usize, u64, String, Vec<Found>),
    Download(usize, DownloadUpdate),
    /// A download into the cache finished, with its files.
    Cached(usize, String, Vec<PathBuf>),
    /// A finished download joined the library.
    Imported(Box<Track>),
    /// The source's own page, when it changed.
    Status(usize, Option<StatusPage>),
    /// The lines under downloads still going, when they changed.
    Notes(usize, Vec<(String, String)>),
    /// The preview's bytes started landing in this file.
    PreviewFile(usize, PathBuf),
    /// The preview couldn't start, and why.
    PreviewFailed(usize, String),
}

/// One source's thread: searches by the app's own numbers, mapped to the
/// add-on's ids once it has started them.
fn run(
    mut plugin: Box<dyn Plugin>,
    index: usize,
    library: Option<PathBuf>,
    rx: Receiver<Request>,
    tx: Sender<Event>,
) {
    let Some(name) = plugin.source().map(|s| s.info().name) else {
        return;
    };
    let mut ids: HashMap<u64, String> = HashMap::new();
    let mut active: HashSet<u64> = HashSet::new();
    // Downloads going to the cache, by key: they don't join the library.
    let mut caching: HashSet<String> = HashSet::new();
    let mut last = Instant::now();
    // Asked for at the first tick.
    let mut last_status: Option<Instant> = None;
    let mut status: Option<StatusPage> = None;
    let mut notes: Vec<(String, String)> = Vec::new();
    // A preview waiting for its first bytes, and since when.
    let mut preview: Option<Instant> = None;
    loop {
        let wait = if preview.is_some() {
            PREVIEW_TICK
        } else {
            TICK
        };
        match rx.recv_timeout(wait) {
            Ok(Request::Configure(config)) => plugin.configure(&config),
            Ok(Request::Keep(key, from, to)) => {
                let progress = match keep(&from, &to) {
                    Ok(()) => {
                        if let Some(library) = &library {
                            match crate::inbox::add_download(library, &to, name) {
                                Ok(track) => {
                                    let _ = tx.send(Event::Imported(Box::new(track)));
                                }
                                Err(e) => eprintln!("{name}: {}: {e}", to.display()),
                            }
                        }
                        Progress::Done(vec![to])
                    }
                    Err(e) => Progress::Failed(e),
                };
                let update = DownloadUpdate { key, progress };
                if tx.send(Event::Download(index, update)).is_err() {
                    return;
                }
            }
            Ok(request) => {
                match &request {
                    Request::Download(_, items, _) => {
                        for f in items {
                            caching.remove(&f.key);
                        }
                    }
                    Request::Cache(_, items, _, room) => {
                        // One limit for every source's cache together.
                        if let Some(root) = caches_root() {
                            make_room(&root, *room);
                        }
                        caching.extend(items.iter().map(|f| f.key.clone()));
                    }
                    _ => {}
                }
                let source = plugin.source().expect("a source");
                match request {
                    Request::Preview(local, item) => {
                        let id = ids.get(&local).cloned().unwrap_or_default();
                        preview = None;
                        match source.preview(&id, &item) {
                            Ok(()) => preview = Some(Instant::now()),
                            Err(e) => {
                                let why = match e {
                                    PluginError::NotSupported => "Can't preview this.".into(),
                                    e => e.to_string(),
                                };
                                if tx.send(Event::PreviewFailed(index, why)).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Request::StopPreview => {
                        preview = None;
                        source.stop_preview();
                    }
                    request => {
                        if !handle(source, index, request, &mut ids, &mut active, &tx) {
                            return;
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if let Some(since) = preview {
            let source = plugin.source().expect("a source");
            let event = match source.preview_file() {
                Ok(Some(path)) => Some(Event::PreviewFile(index, path)),
                Ok(None) if since.elapsed() >= PREVIEW_WAIT => Some(Event::PreviewFailed(
                    index,
                    "Nobody sent it in time.".into(),
                )),
                Ok(None) => None,
                Err(e) => Some(Event::PreviewFailed(index, e.to_string())),
            };
            if let Some(event) = event {
                preview = None;
                if tx.send(event).is_err() {
                    return;
                }
            }
        }
        if last.elapsed() < TICK {
            continue;
        }
        last = Instant::now();
        let source = plugin.source().expect("a source");
        if last_status.is_none_or(|t| t.elapsed() >= STATUS_TICK) {
            last_status = Some(Instant::now());
            let now = source.status();
            if now != status {
                status = now.clone();
                if tx.send(Event::Status(index, now)).is_err() {
                    return;
                }
            }
        }
        for local in active.clone() {
            let Some(id) = ids.get(&local) else { continue };
            let event = match source.results(id) {
                Ok(snap) => {
                    if snap.done {
                        active.remove(&local);
                    }
                    Event::Found(index, local, snap)
                }
                Err(e) => {
                    active.remove(&local);
                    Event::SearchFailed(index, local, e.to_string())
                }
            };
            if tx.send(event).is_err() {
                return;
            }
        }
        for mut update in source.progress() {
            if let Progress::Done(paths) = &update.progress {
                if caching.remove(&update.key) {
                    let cached = Event::Cached(index, update.key.clone(), paths.clone());
                    if tx.send(cached).is_err() {
                        return;
                    }
                    if tx.send(Event::Download(index, update)).is_err() {
                        return;
                    }
                    continue;
                }
                for path in paths {
                    let Some(library) = &library else { break };
                    match crate::inbox::add_download(library, path, name) {
                        Ok(track) => {
                            let _ = tx.send(Event::Imported(Box::new(track)));
                        }
                        Err(e) => eprintln!("{name}: {}: {e}", path.display()),
                    }
                }
                update.progress = Progress::Done(paths.clone());
            }
            if tx.send(Event::Download(index, update)).is_err() {
                return;
            }
        }
        let now = source.notes();
        if now != notes {
            notes = now.clone();
            if tx.send(Event::Notes(index, now)).is_err() {
                return;
            }
        }
    }
}

/// Does one request; false when the app has gone.
fn handle(
    source: &mut dyn SearchSource,
    index: usize,
    request: Request,
    ids: &mut HashMap<u64, String>,
    active: &mut HashSet<u64>,
    tx: &Sender<Event>,
) -> bool {
    let event = match request {
        Request::Start(local, query) => match source.start(&query) {
            Ok(id) => {
                ids.insert(local, id);
                active.insert(local);
                None
            }
            Err(e) => Some(Event::SearchFailed(index, local, e.to_string())),
        },
        Request::Stop(local) => {
            active.remove(&local);
            if let Some(id) = ids.remove(&local) {
                source.stop(&id);
            }
            None
        }
        Request::OpenFolder(local, folder) => {
            let folder = *folder;
            let id = ids.get(&local).cloned().unwrap_or_default();
            match source.open_folder(&id, &folder) {
                Ok(tracks) => Some(Event::Folder(index, local, folder.key, tracks)),
                Err(e) => {
                    eprintln!("can't open {}: {e}", folder.title);
                    None
                }
            }
        }
        Request::Download(local, items, dir) | Request::Cache(local, items, dir, _) => {
            let id = ids.get(&local).cloned().unwrap_or_default();
            let _ = std::fs::create_dir_all(&dir);
            match source.download(&id, &items, &dir) {
                Ok(()) => None,
                Err(e) => {
                    for item in items {
                        let failed = DownloadUpdate {
                            key: item.key,
                            progress: Progress::Failed(e.to_string()),
                        };
                        if tx.send(Event::Download(index, failed)).is_err() {
                            return false;
                        }
                    }
                    None
                }
            }
        }
        Request::Configure(_) | Request::Keep(..) => None,
        Request::Preview(..) | Request::StopPreview => None,
        Request::Cancel(key) => {
            if let Err(e) = source.cancel(&key) {
                eprintln!("can't cancel {key}: {e}");
            }
            None
        }
        Request::Retry(key) => match source.retry(&key) {
            Ok(()) => None,
            Err(e) => Some(Event::Download(
                index,
                DownloadUpdate {
                    key,
                    progress: Progress::Failed(match e {
                        PluginError::NotSupported => "Can't try again.".into(),
                        e => e.to_string(),
                    }),
                },
            )),
        },
    };
    event.is_none_or(|e| tx.send(e).is_ok())
}

/// A saved search and what it has found.
struct Search {
    local: u64,
    query: SourceQuery,
    /// When the DJ added it (Unix milliseconds); the newest is listed first.
    added: u64,
    items: Vec<Found>,
    done: bool,
    note: String,
    /// What the source calls it, e.g. a playlist's name; empty for none.
    title: String,
    error: Option<String>,
    /// Folders the DJ has opened, by key, and playlist entries showing all
    /// their copies, by [`song_key`].
    open: HashSet<String>,
    /// Download each playlist entry's best copy once its results settle,
    /// and its next copy when one fails. For this run only, not saved.
    auto: bool,
    /// Each entry's results at the last looks, and how many looks in a
    /// row they've stayed the same.
    settling: HashMap<String, (Vec<String>, u32)>,
    /// Playlist entries whose download the DJ took off the list: not
    /// downloaded by themselves again. For this run only, like `auto`.
    dropped: HashSet<String>,
    /// Entries kept from before "Search again", which isn't looking for
    /// them.
    kept: Vec<Found>,
}

impl Search {
    /// What the search is listed by: the source's name for it, else the
    /// query.
    fn name(&self) -> &str {
        if self.title.is_empty() {
            &self.query.text
        } else {
            &self.title
        }
    }

    fn find(&self, key: &str) -> Option<&Found> {
        self.items.iter().find_map(|f| {
            if f.key == key {
                Some(f)
            } else {
                f.tracks.iter().find(|t| t.key == key)
            }
        })
    }

    /// The folder a file is in, if it's in one.
    fn parent(&self, key: &str) -> Option<&Found> {
        self.items
            .iter()
            .find(|f| f.folder && f.tracks.iter().any(|t| t.key == key))
    }
}

struct Download {
    key: String,
    /// The search it came from, by the app's number.
    local: u64,
    label: String,
    progress: Progress,
    /// Into the cache rather than the collection.
    cache: bool,
    /// Where it is in the other user's queue, or how fast it comes
    /// ([`SearchSource::notes`]).
    note: String,
}

/// Now, in Unix milliseconds.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// One plugin source: its searches and downloads.
struct Source {
    /// The plugin's id, as its settings are saved under.
    id: &'static str,
    info: SourceInfo,
    /// Sites whose links it searches (see [`SearchSource::sites`]).
    sites: &'static [&'static str],
    requests: Sender<Request>,
    searches: Vec<Search>,
    downloads: Vec<Download>,
    /// Its files can play from a cache (see [`SearchSource::plays`]): the
    /// cache's folder.
    cache: Option<PathBuf>,
    /// Its settings, for the set-up form shown until it is set up.
    settings: Vec<Setting>,
    /// Its own page in the panel, e.g. Sharing (see
    /// [`SearchSource::status`]).
    status: Option<StatusPage>,
    /// Its files can be heard before they're downloaded (see
    /// [`SearchSource::previews`]).
    previews: bool,
}

impl Source {
    /// The latest download of `key` into the collection.
    fn download(&self, key: &str) -> Option<&Download> {
        self.downloads
            .iter()
            .rev()
            .find(|d| d.key == key && !d.cache)
    }

    /// The latest download of `key` into the cache.
    fn caching(&self, key: &str) -> Option<&Download> {
        self.downloads
            .iter()
            .rev()
            .find(|d| d.key == key && d.cache)
    }

    /// Where a file from it is in the cache, if it's there.
    fn cached(&self, f: &Found) -> Option<PathBuf> {
        let path = self.cache.as_ref()?.join(relative(&f.file)?);
        path.is_file().then_some(path)
    }

    fn save(&self) {
        let saved: Vec<_> = self
            .searches
            .iter()
            .map(|s| {
                (
                    s.query.text.clone(),
                    s.query.kind.name().to_string(),
                    s.query.prefer.name().to_string(),
                    s.added,
                    s.query.songs.clone(),
                )
            })
            .collect();
        crate::settings::set_source_searches(self.info.name, &saved);
        self.save_results();
    }

    /// Keeps finished searches' results, so they show again after a
    /// restart without searching again. Not for a source that plays: its
    /// files download through what its own search found.
    fn save_results(&self) {
        if self.cache.is_some() {
            return;
        }
        let Some(path) = results_path(self.info.name) else {
            return;
        };
        let mut all = serde_json::Map::new();
        for s in self
            .searches
            .iter()
            .filter(|s| s.done && s.error.is_none() && !s.items.is_empty())
        {
            all.insert(
                s.added.to_string(),
                serde_json::json!({
                    "items": s.items.iter().map(djsugar_plugins::remote::found_json).collect::<Vec<_>>(),
                    "note": s.note,
                    "title": s.title,
                }),
            );
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, serde_json::Value::Object(all).to_string());
    }
}

/// Where a source's finished searches' results are kept.
fn results_path(source: &str) -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| {
        d.join("source-results")
            .join(format!("{}.json", safe_name(source, "Source")))
    })
}

/// A finished search's kept results: what it found, its note and title.
type Kept = (Vec<Found>, String, String);

/// The results kept by [`Source::save_results`], by when each search was
/// added.
fn kept_results(source: &str) -> HashMap<u64, Kept> {
    let Some(text) = results_path(source).and_then(|p| std::fs::read_to_string(p).ok()) else {
        return HashMap::new();
    };
    read_kept(&text)
}

fn read_kept(text: &str) -> HashMap<u64, Kept> {
    let Ok(serde_json::Value::Object(all)) = serde_json::from_str(text) else {
        return HashMap::new();
    };
    all.into_iter()
        .filter_map(|(added, v)| {
            let items: Vec<Found> = v["items"]
                .as_array()?
                .iter()
                .map(djsugar_plugins::remote::found_from)
                .collect();
            let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
            Some((added.parse().ok()?, (items, text("note"), text("title"))))
        })
        .filter(|(_, (items, _, _))| !items.is_empty())
        .collect()
}

/// What a row of the list is.
#[derive(Clone, PartialEq)]
enum Row {
    /// "Albums", "Tracks", or a playlist entry (by its group), whose other
    /// copies show under it when it's open.
    Heading(Option<String>),
    /// A result, by its key.
    Item(String),
    /// A download in the Downloads list, by its index.
    Download(usize),
}

pub struct Sources {
    app: slint::Weak<AppWindow>,
    /// Results' covers fetched so far, by address, and those asked for.
    covers: HashMap<String, slint::Image>,
    asked: HashSet<String>,
    cover_requests: Sender<String>,
    cover_events: Receiver<(String, Option<Thumbnail>)>,
    sources: Vec<Source>,
    events: Receiver<Event>,
    next_local: u64,
    /// The source shown, if a plugin source is picked in the rail.
    shown: Option<usize>,
    /// The search shown, or the Downloads list.
    search: Option<usize>,
    /// The source's own page is shown instead (see [`Source::status`]).
    page: bool,
    /// 0 = everything, 1 = single tracks, 2 = albums.
    filter: i32,
    lossless_only: bool,
    /// Ticked results, by key.
    checked: HashSet<String>,
    /// The row ticked last, for shift-click.
    anchor: Option<usize>,
    rows: Vec<Row>,
    /// The music folder; downloads go to `Downloaded/<source>/` in it
    /// unless the source's settings name a folder.
    music: Option<PathBuf>,
    list: Rc<VecModel<FoundRow>>,
    /// What the DJ typed in the shown source's set-up form, by setting.
    setup_values: HashMap<String, String>,
    /// What the last Connect on the set-up form said.
    setup_status: String,
    /// Files to load onto a deck, once the frame loop takes them.
    loads: Vec<PathBuf>,
    /// Files to load once they're in the cache, by source and key.
    play_after: HashSet<(usize, String)>,
    /// The file being heard before it's downloaded, if any.
    preview: Option<Preview>,
    /// What the frame loop should tell the preview player.
    preview_actions: Vec<PreviewAction>,
}

/// A file of a search being heard before it's downloaded.
struct Preview {
    source: usize,
    key: String,
    size: u64,
    state: PreviewState,
}

#[derive(Clone, Debug, PartialEq)]
enum PreviewState {
    /// Waiting for the other user to start sending.
    Waiting,
    Playing,
    Failed(String),
}

/// What the preview player should do, for the frame loop.
#[derive(Debug, PartialEq)]
pub enum PreviewAction {
    /// Play this file while it downloads, which will hold this many bytes.
    Play(PathBuf, u64),
    Stop,
}

impl Sources {
    /// Starts each source with its saved searches, and wires up the UI.
    pub fn install(app: &AppWindow, library: Option<PathBuf>) -> Rc<std::cell::RefCell<Self>> {
        let (tx, events) = channel();
        let sources: Vec<Source> = source_plugins()
            .into_iter()
            .enumerate()
            .map(|(index, (mut plugin, info))| {
                let id = plugin.manifest().id;
                let sites = plugin.source().map(|s| s.sites()).unwrap_or_default();
                let plays = plugin.source().is_some_and(|s| s.plays());
                let previews = plugin.source().is_some_and(|s| s.previews());
                let cache = plays.then(|| cache_dir(info.name)).flatten();
                let settings = plugin.settings();
                let (requests, rx) = channel();
                let (tx, library) = (tx.clone(), library.clone());
                std::thread::Builder::new()
                    .name(format!("source {}", info.name))
                    .spawn(move || run(plugin, index, library, rx, tx))
                    .expect("spawn source thread");
                Source {
                    id,
                    info,
                    sites,
                    requests,
                    searches: Vec::new(),
                    downloads: Vec::new(),
                    cache,
                    settings,
                    status: None,
                    previews,
                }
            })
            .collect();
        let list = Rc::new(VecModel::default());
        let (cover_requests, wanted) = channel::<String>();
        let (covers_tx, cover_events) = channel();
        std::thread::Builder::new()
            .name("source covers".into())
            .spawn(move || {
                for url in wanted {
                    let thumb = crate::coverart::thumbnail(&url, COVER_SIDE);
                    if covers_tx.send((url, thumb)).is_err() {
                        return;
                    }
                }
            })
            .expect("spawn source covers thread");
        let mut me = Self {
            app: app.as_weak(),
            covers: HashMap::new(),
            asked: HashSet::new(),
            cover_requests,
            cover_events,
            sources,
            events,
            next_local: 0,
            shown: None,
            search: None,
            page: false,
            filter: 0,
            lossless_only: false,
            checked: HashSet::new(),
            anchor: None,
            rows: Vec::new(),
            music: djsugar_engine::scan::default_music_dir(),
            list: list.clone(),
            setup_values: HashMap::new(),
            setup_status: String::new(),
            loads: Vec::new(),
            play_after: HashSet::new(),
            preview: None,
            preview_actions: Vec::new(),
        };
        // Saved searches show what they found last time; those that hadn't
        // finished, and a playing source's, run again.
        for s in 0..me.sources.len() {
            let saved = crate::settings::source_searches(me.sources[s].info.name);
            let mut kept = if me.sources[s].cache.is_none() {
                kept_results(me.sources[s].info.name)
            } else {
                HashMap::new()
            };
            for (text, kind, prefer, added, songs) in saved {
                let query = SourceQuery {
                    text,
                    kind: QueryKind::from_name(&kind),
                    prefer: Prefer::from_name(&prefer),
                    skip: Vec::new(),
                    songs,
                    restored: true,
                };
                match kept.remove(&added) {
                    Some(found) => me.restore(s, query, added, found),
                    None => {
                        me.start(s, query, added, false);
                    }
                }
            }
        }
        let ui = app.global::<SourceUi>();
        let tabs: Vec<SourceTab> = me
            .sources
            .iter()
            .map(|s| SourceTab {
                name: s.info.name.into(),
                icon: slint::Image::load_from_svg_data(s.info.icon_svg).unwrap_or_default(),
                hint: s.info.hint.into(),
            })
            .collect();
        ui.set_sources(ModelRc::new(VecModel::from(tabs)));
        ui.set_rows(list.into());
        let me = Rc::new(std::cell::RefCell::new(me));
        let weak = Rc::downgrade(&me);
        let with = move |f: Box<dyn Fn(&mut Sources)>| {
            if let Some(s) = weak.upgrade() {
                f(&mut s.borrow_mut());
            }
        };
        let w = with.clone();
        ui.on_search(move |text, kind, prefer, auto| {
            let query = query(&text, kind, prefer);
            w(Box::new(move |s| s.add_search(query.clone(), auto)))
        });
        let w = with.clone();
        ui.on_setup_typed(move |key, text| {
            let (key, text) = (key.to_string(), text.to_string());
            w(Box::new(move |s| {
                s.setup_values.insert(key.clone(), text.clone());
            }))
        });
        let weak = app.as_weak();
        ui.on_open_settings(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<crate::Preferences>()
                    .invoke_show(crate::prefs::SOURCES_PAGE);
            }
        });
        let w = with.clone();
        ui.on_setup_connect(move || w(Box::new(|s| s.setup_connect())));
        let w = with.clone();
        ui.on_select_search(move |i| w(Box::new(move |s| s.select(i))));
        let w = with.clone();
        ui.on_remove_search(move |i| w(Box::new(move |s| s.remove(i))));
        let w = with.clone();
        ui.on_clear_searches(move || w(Box::new(|s| s.clear_searches())));
        let w = with.clone();
        ui.on_search_again(move |i| w(Box::new(move |s| s.again(i))));
        let w = with.clone();
        ui.on_check(move |row, shift| w(Box::new(move |s| s.check(row as usize, shift))));
        let w = with.clone();
        ui.on_toggle_open(move |row| w(Box::new(move |s| s.toggle_open(row as usize))));
        let w = with.clone();
        ui.on_download(move |row| w(Box::new(move |s| s.download_row(row as usize))));
        let w = with.clone();
        ui.on_remove_download(move |row| w(Box::new(move |s| s.remove_download(row as usize))));
        let w = with.clone();
        ui.on_retry_failed(move || w(Box::new(|s| s.retry_failed())));
        let w = with.clone();
        ui.on_clear_finished(move || w(Box::new(|s| s.clear_finished())));
        let w = with.clone();
        ui.on_cancel_downloads(move || w(Box::new(|s| s.cancel_downloads())));
        let w = with.clone();
        ui.on_download_selected(move || w(Box::new(|s| s.download_selected())));
        let w = with.clone();
        ui.on_play(move |row| w(Box::new(move |s| s.play_row(row as usize))));
        let w = with.clone();
        ui.on_preview_row(move |row| w(Box::new(move |s| s.preview_row(row as usize))));
        let w = with.clone();
        ui.on_cache(move |row| w(Box::new(move |s| s.cache_row(row as usize))));
        let w = with.clone();
        ui.on_cache_selected(move || w(Box::new(|s| s.cache_selected())));
        let w = with.clone();
        ui.on_pick_best(move || w(Box::new(|s| s.pick_best())));
        let w = with.clone();
        ui.on_clear_selection(move || {
            w(Box::new(|s| {
                s.checked.clear();
                s.refresh();
            }))
        });
        let w = with.clone();
        ui.on_set_filter(move |f| {
            w(Box::new(move |s| {
                s.filter = f;
                s.refresh();
            }))
        });
        let w = with.clone();
        ui.on_set_lossless_only(move |on| {
            w(Box::new(move |s| {
                s.lossless_only = on;
                s.refresh();
            }))
        });
        me
    }

    /// Takes in what the sources have done, and follows the rail. Called
    /// every frame; returns tracks that joined the library.
    /// The DJ changed plugin `id`'s settings: its source hears of it.
    /// Connect on the shown source's set-up form: what was typed is saved
    /// as its settings, and the source told; the form goes once every field
    /// it needs is filled.
    fn setup_connect(&mut self) {
        let Some(s) = self.shown else {
            return;
        };
        let id = self.sources[s].id;
        for (key, value) in self.setup_values.drain() {
            let value = value.trim();
            if !value.is_empty() {
                crate::settings::set_plugin_value(id, &key, value);
            }
        }
        self.configure(id);
        let config = crate::settings::plugin_config(id);
        self.setup_status = if needs_setup(&self.sources[s].settings, &config) {
            "Fill in every field first.".into()
        } else {
            String::new()
        };
        self.refresh();
    }

    pub fn configure(&mut self, id: &str) {
        let config = crate::settings::plugin_config(id);
        for source in self.sources.iter().filter(|s| s.id == id) {
            let _ = source.requests.send(Request::Configure(config.clone()));
        }
    }

    pub fn poll(&mut self) -> Vec<Track> {
        let mut imported = Vec::new();
        let mut changed = false;
        for (url, thumb) in self.cover_events.try_iter().collect::<Vec<_>>() {
            if let Some((side, rgba)) = thumb {
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &rgba, side, side,
                );
                self.covers.insert(url, slint::Image::from_rgba8(pixels));
                changed = true;
            }
        }
        // Sources with a search that just finished, to keep its results.
        let mut finished: HashSet<usize> = HashSet::new();
        for event in self.events.try_iter().collect::<Vec<_>>() {
            changed = true;
            match event {
                Event::Found(s, local, snap) => {
                    if let Some(search) = self.search_mut(s, local) {
                        // Results mean it's working, whatever went wrong
                        // before.
                        search.error = None;
                        // Opened folders keep the files read for them.
                        let mut items = snap.items;
                        for item in items.iter_mut().filter(|i| i.folder) {
                            if let Some(old) = search.items.iter().find(|o| o.key == item.key) {
                                if old.tracks.len() > item.tracks.len() {
                                    item.tracks = old.tracks.clone();
                                }
                            }
                        }
                        if !search.kept.is_empty() {
                            items = with_kept(&search.kept, items);
                        }
                        if search.auto {
                            let wanted = wanted(&items, search.query.kind);
                            settle(&mut search.settling, &items, &wanted);
                        }
                        search.items = items;
                        if snap.done && !search.done {
                            finished.insert(s);
                        }
                        search.done = snap.done;
                        search.note = snap.note;
                        if !snap.title.is_empty() {
                            search.title = snap.title;
                        }
                    }
                }
                Event::SearchFailed(s, local, error) => {
                    if let Some(search) = self.search_mut(s, local) {
                        search.done = true;
                        search.error = Some(error);
                    }
                }
                Event::Folder(s, local, key, tracks) => {
                    if let Some(search) = self.search_mut(s, local) {
                        if let Some(f) = search.items.iter_mut().find(|f| f.key == key) {
                            f.tracks = tracks;
                        }
                    }
                }
                Event::Download(s, update) => {
                    if let Some(d) = self.sources[s]
                        .downloads
                        .iter_mut()
                        .rev()
                        .find(|d| d.key == update.key)
                    {
                        d.progress = update.progress;
                    }
                }
                Event::Cached(s, key, paths) => {
                    if self.play_after.remove(&(s, key)) {
                        if let Some(path) = paths.into_iter().next() {
                            self.loads.push(path);
                        }
                    }
                }
                Event::Imported(track) => imported.push(*track),
                Event::Status(s, status) => {
                    if status.is_none() && self.shown == Some(s) {
                        self.page = false;
                    }
                    self.sources[s].status = status;
                }
                Event::PreviewFile(s, path) => {
                    if let Some(p) = self.preview.as_mut().filter(|p| p.source == s) {
                        if p.state == PreviewState::Waiting {
                            p.state = PreviewState::Playing;
                            self.preview_actions.push(PreviewAction::Play(path, p.size));
                        }
                    }
                }
                Event::PreviewFailed(s, why) => {
                    if let Some(p) = self.preview.as_mut().filter(|p| p.source == s) {
                        p.state = PreviewState::Failed(why);
                    }
                }
                Event::Notes(s, notes) => {
                    for d in &mut self.sources[s].downloads {
                        d.note = notes
                            .iter()
                            .find(|(k, _)| *k == d.key)
                            .map(|(_, n)| n.clone())
                            .unwrap_or_default();
                    }
                }
            }
        }
        for s in finished {
            self.sources[s].save_results();
        }
        if changed {
            for s in 0..self.sources.len() {
                for i in 0..self.sources[s].searches.len() {
                    self.auto_download(s, i);
                }
            }
        }
        if let Some(app) = self.app.upgrade() {
            let picked = app.global::<StoreUi>().get_source();
            let shown = (picked >= FIRST_SOURCE)
                .then(|| (picked - FIRST_SOURCE) as usize)
                .filter(|&s| s < self.sources.len());
            if shown != self.shown {
                self.shown = shown;
                self.setup_values.clear();
                self.setup_status.clear();
                self.search = shown
                    .filter(|&s| !self.sources[s].searches.is_empty())
                    .map(|_| 0);
                self.page = false;
                self.checked.clear();
                changed = true;
            }
        }
        if changed {
            self.refresh();
        }
        imported
    }

    /// What the preview player should do since last asked.
    pub fn take_preview_actions(&mut self) -> Vec<PreviewAction> {
        std::mem::take(&mut self.preview_actions)
    }

    /// A file row's preview button: starts hearing it, or stops if it's
    /// the one being heard.
    fn preview_row(&mut self, row: usize) {
        let (Some(s), Some(i)) = (self.shown, self.search) else {
            return;
        };
        let Some(Row::Item(key)) = self.rows.get(row).cloned() else {
            return;
        };
        let same = self
            .preview
            .as_ref()
            .is_some_and(|p| p.source == s && p.key == key);
        self.stop_preview();
        if same {
            self.refresh();
            return;
        }
        let search = &self.sources[s].searches[i];
        let Some(found) = search.find(&key).filter(|f| !f.folder).cloned() else {
            return;
        };
        let _ = self.sources[s]
            .requests
            .send(Request::Preview(search.local, Box::new(found.clone())));
        self.preview = Some(Preview {
            source: s,
            key,
            size: found.size,
            state: PreviewState::Waiting,
        });
        self.refresh();
    }

    /// Stops any preview, in the player and at its source.
    fn stop_preview(&mut self) {
        if let Some(p) = self.preview.take() {
            let _ = self.sources[p.source].requests.send(Request::StopPreview);
            if p.state == PreviewState::Playing {
                self.preview_actions.push(PreviewAction::Stop);
            }
        }
    }

    /// Files from a source's cache to load onto a deck.
    pub fn take_loads(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.loads)
    }

    /// Double-clicking a file of a source that plays: loads it from the
    /// cache, fetching it there first if it isn't yet.
    fn play_row(&mut self, row: usize) {
        let (Some(s), Some(i)) = (self.shown, self.search) else {
            return;
        };
        let Some(Row::Item(key)) = self.rows.get(row).cloned() else {
            return;
        };
        let source = &self.sources[s];
        let Some(found) = source.searches[i].find(&key) else {
            return;
        };
        if found.folder {
            return;
        }
        if let Some(path) = source.cached(found) {
            self.loads.push(path);
            return;
        }
        self.play_after.insert((s, key.clone()));
        self.cache_in(s, i, vec![key]);
    }

    /// A row's cloud button: keeps it in the cache.
    fn cache_row(&mut self, row: usize) {
        let (Some(s), Some(i)) = (self.shown, self.search) else {
            return;
        };
        if let Some(Row::Item(key)) = self.rows.get(row).cloned() {
            self.cache_in(s, i, vec![key]);
        }
    }

    fn cache_selected(&mut self) {
        let (Some(s), Some(i)) = (self.shown, self.search) else {
            return;
        };
        let keys = self.checked.drain().collect();
        self.cache_in(s, i, keys);
    }

    /// Downloads results of search `i` of source `s` into its cache:
    /// folders whole, and files not there yet.
    fn cache_in(&mut self, s: usize, i: usize, keys: Vec<String>) {
        let source = &mut self.sources[s];
        let Some(dir) = source.cache.clone() else {
            return;
        };
        let search = &source.searches[i];
        let mut items: Vec<Found> = Vec::new();
        for key in &keys {
            let Some(found) = search.find(key) else {
                continue;
            };
            let busy = source
                .caching(key)
                .is_some_and(|d| !matches!(d.progress, Progress::Failed(_)));
            let there = !found.folder && source.cached(found).is_some();
            if !busy && !there {
                items.push(found.clone());
            }
        }
        if items.is_empty() {
            self.refresh();
            return;
        }
        for f in &items {
            source.downloads.push(Download {
                key: f.key.clone(),
                local: search.local,
                label: label(f),
                progress: Progress::Queued,
                cache: true,
                note: String::new(),
            });
        }
        let room = cache_bytes();
        let _ = source
            .requests
            .send(Request::Cache(search.local, items, dir, room));
        self.refresh();
    }

    fn search_mut(&mut self, s: usize, local: u64) -> Option<&mut Search> {
        self.sources
            .get_mut(s)?
            .searches
            .iter_mut()
            .find(|x| x.local == local)
    }

    /// Starts a search and lists it by when it was added, newest first;
    /// returns its place in the list.
    fn start(&mut self, s: usize, query: SourceQuery, added: u64, auto: bool) -> usize {
        let at = self.start_listed(s, query.clone(), added, auto);
        let local = self.sources[s].searches[at].local;
        let _ = self.sources[s].requests.send(Request::Start(local, query));
        at
    }

    /// Lists a new search, not started yet; returns its place in the list.
    fn start_listed(&mut self, s: usize, query: SourceQuery, added: u64, auto: bool) -> usize {
        self.next_local += 1;
        let local = self.next_local;
        let searches = &mut self.sources[s].searches;
        let at = place(searches, added);
        searches.insert(
            at,
            Search {
                local,
                query,
                added,
                items: Vec::new(),
                done: false,
                note: String::new(),
                title: String::new(),
                error: None,
                open: HashSet::new(),
                auto,
                settling: HashMap::new(),
                dropped: HashSet::new(),
                kept: Vec::new(),
            },
        );
        at
    }

    /// Lists a search with the results it found before the app last
    /// closed, finished, without searching again ("Search again" does).
    fn restore(&mut self, s: usize, query: SourceQuery, added: u64, (items, note, title): Kept) {
        let at = self.start_listed(s, query, added, false);
        let search = &mut self.sources[s].searches[at];
        (search.items, search.note, search.title, search.done) = (items, note, title, true);
    }

    fn add_search(&mut self, query: SourceQuery, auto: bool) {
        let Some(s) = self.shown else { return };
        if query.text.trim().is_empty() {
            return;
        }
        let at = self.start(s, query, now(), auto);
        self.sources[s].save();
        self.search = Some(at);
        self.checked.clear();
        self.refresh();
    }

    /// The source that searches links from `site`, by its name, if one does.
    pub fn takes_links(&self, site: &str) -> Option<&'static str> {
        self.sources
            .iter()
            .find(|s| s.sites.contains(&site))
            .map(|s| s.info.name)
    }

    /// "Add to Downloads" from elsewhere in the app: a search on the source
    /// taking links from `site`, downloading right away.
    pub fn add_download(&mut self, site: &str, query: SourceQuery) {
        let Some(s) = self.sources.iter().position(|x| x.sites.contains(&site)) else {
            return;
        };
        let at = self.start(s, query, now(), true);
        self.sources[s].save();
        if self.shown == Some(s) {
            self.search = Some(at);
        }
        self.refresh();
    }

    /// The search at a row of the panel; -1 is Downloads.
    /// Shows search `i`, the Downloads list (-1) or the source's own page
    /// (-2).
    fn select(&mut self, i: i32) {
        self.search = usize::try_from(i).ok();
        self.page = i == -2;
        self.checked.clear();
        self.anchor = None;
        self.refresh();
    }

    fn remove(&mut self, i: i32) {
        let (Some(s), Ok(i)) = (self.shown, usize::try_from(i)) else {
            return;
        };
        let source = &mut self.sources[s];
        if i >= source.searches.len() {
            return;
        }
        let gone = source.searches.remove(i);
        let _ = source.requests.send(Request::Stop(gone.local));
        source.save();
        self.search = match self.search {
            Some(x) if x == i => (!source.searches.is_empty()).then_some(0),
            Some(x) if x > i => Some(x - 1),
            other => other,
        };
        self.refresh();
    }

    /// Removes every search of the source shown. Downloads stay.
    fn clear_searches(&mut self) {
        let Some(s) = self.shown else { return };
        let source = &mut self.sources[s];
        for gone in source.searches.drain(..) {
            let _ = source.requests.send(Request::Stop(gone.local));
        }
        source.save();
        self.search = None;
        self.checked.clear();
        self.refresh();
    }

    /// Runs a search again from scratch, in its place in the list.
    fn again(&mut self, i: i32) {
        let (Some(s), Ok(i)) = (self.shown, usize::try_from(i)) else {
            return;
        };
        if i >= self.sources[s].searches.len() {
            return;
        }
        self.next_local += 1;
        let local = self.next_local;
        let source = &mut self.sources[s];
        // A playlist's entries already found or downloaded stay as they
        // are; only the rest are searched for.
        let skip = found_entries(source, &source.searches[i]);
        let old = std::mem::replace(&mut source.searches[i].local, local);
        let _ = source.requests.send(Request::Stop(old));
        let search = &mut source.searches[i];
        search.kept = std::mem::take(&mut search.items)
            .into_iter()
            .filter(|f| skip.contains(&f.group))
            .collect();
        search.items = search.kept.clone();
        (search.done, search.note, search.error) = (false, String::new(), None);
        search.settling.clear();
        // The DJ asked for this one, so a source may ask them to sign in.
        search.query.restored = false;
        let query = SourceQuery {
            skip: skip.into_iter().collect(),
            ..search.query.clone()
        };
        let _ = source.requests.send(Request::Start(local, query));
        // Downloads from before stay with the new search's results.
        for d in source.downloads.iter_mut().filter(|d| d.local == old) {
            d.local = local;
        }
        self.refresh();
    }

    fn current(&self) -> Option<(usize, &Search)> {
        let s = self.shown?;
        Some((s, self.sources[s].searches.get(self.search?)?))
    }

    fn check(&mut self, row: usize, shift: bool) {
        let Some(Row::Item(key)) = self.rows.get(row).cloned() else {
            return;
        };
        let on = !self.checked.contains(&key);
        let range = match (shift, self.anchor) {
            (true, Some(a)) => a.min(row)..=a.max(row),
            _ => row..=row,
        };
        for r in range {
            if let Some(Row::Item(k)) = self.rows.get(r) {
                if on {
                    self.checked.insert(k.clone());
                } else {
                    self.checked.remove(k);
                }
            }
        }
        self.anchor = Some(row);
        self.refresh();
    }

    fn toggle_open(&mut self, row: usize) {
        let key = match self.rows.get(row).cloned() {
            Some(Row::Item(key)) => key,
            Some(Row::Heading(Some(group))) => song_key(&group),
            _ => return,
        };
        let Some(s) = self.shown else { return };
        let Some(i) = self.search else { return };
        let source = &mut self.sources[s];
        let search = &mut source.searches[i];
        if !search.open.remove(&key) {
            search.open.insert(key.clone());
            if let Some(f) = search.find(&key) {
                if f.folder && f.tracks.len() < f.track_count {
                    let _ = source
                        .requests
                        .send(Request::OpenFolder(search.local, Box::new(f.clone())));
                }
            }
        }
        self.refresh();
    }

    /// The button on a row: download it, or try a failed one again.
    fn download_row(&mut self, row: usize) {
        let Some(s) = self.shown else { return };
        let key = match self.rows.get(row) {
            Some(Row::Item(k)) => k.clone(),
            Some(Row::Download(d)) => self.sources[s].downloads[*d].key.clone(),
            _ => return,
        };
        match self.sources[s].download(&key).map(|d| &d.progress) {
            Some(Progress::Failed(_)) => {
                let source = &mut self.sources[s];
                if let Some(d) = source.downloads.iter_mut().rev().find(|d| d.key == key) {
                    d.progress = Progress::Queued;
                }
                let _ = source.requests.send(Request::Retry(key));
                self.refresh();
            }
            Some(_) => {}
            None => self.download_keys(vec![key]),
        }
    }

    /// Takes a row off the Downloads list, stopping it if it's still going.
    /// Files already downloaded stay.
    fn remove_download(&mut self, row: usize) {
        let (Some(s), Some(Row::Download(d))) = (self.shown, self.rows.get(row).cloned()) else {
            return;
        };
        let source = &mut self.sources[s];
        if d >= source.downloads.len() {
            return;
        }
        let gone = source.downloads.remove(d);
        drop_entry(source, gone.local, &gone.key);
        if is_active(&gone.progress) {
            let _ = source.requests.send(Request::Cancel(gone.key));
        }
        self.refresh();
    }

    /// Tries every failed download of the source shown again.
    fn retry_failed(&mut self) {
        let Some(s) = self.shown else { return };
        let source = &mut self.sources[s];
        for d in &mut source.downloads {
            if matches!(d.progress, Progress::Failed(_)) {
                d.progress = Progress::Queued;
                let _ = source.requests.send(Request::Retry(d.key.clone()));
            }
        }
        self.refresh();
    }

    /// Takes finished and failed downloads off the list.
    fn clear_finished(&mut self) {
        let Some(s) = self.shown else { return };
        self.sources[s].downloads.retain(|d| is_active(&d.progress));
        self.refresh();
    }

    /// Stops every download still going, and takes them off the list.
    fn cancel_downloads(&mut self) {
        let Some(s) = self.shown else { return };
        let source = &mut self.sources[s];
        let going: Vec<(u64, String)> = source
            .downloads
            .iter()
            .filter(|d| is_active(&d.progress))
            .map(|d| (d.local, d.key.clone()))
            .collect();
        for (local, key) in going {
            drop_entry(source, local, &key);
            let _ = source.requests.send(Request::Cancel(key));
        }
        source.downloads.retain(|d| !is_active(&d.progress));
        self.refresh();
    }

    /// Ticks the first (best) copy found for each song of a playlist.
    fn pick_best(&mut self) {
        let Some((_, search)) = self.current() else {
            return;
        };
        let mut seen = HashSet::new();
        let best: Vec<String> = search
            .items
            .iter()
            .filter(|f| !f.group.is_empty() && seen.insert(f.group.clone()))
            .map(|f| f.key.clone())
            .collect();
        self.checked.extend(best);
        self.refresh();
    }

    /// For a search set to download by itself: see [`auto_picks`].
    fn auto_download(&mut self, s: usize, i: usize) {
        let source = &self.sources[s];
        let search = &source.searches[i];
        if !search.auto {
            return;
        }
        let keys = auto_picks(source, search);
        if !keys.is_empty() {
            self.download_in(s, i, keys);
        }
    }

    fn download_selected(&mut self) {
        let keys = self.checked.drain().collect();
        self.download_keys(keys);
    }

    /// Downloads results of the shown search: folders whole, and files
    /// that aren't in a folder also being downloaded.
    fn download_keys(&mut self, keys: Vec<String>) {
        if let (Some(s), Some(i)) = (self.shown, self.search) {
            self.download_in(s, i, keys);
        }
    }

    /// [`Self::download_keys`] for search `i` of source `s`, shown or not.
    fn download_in(&mut self, s: usize, i: usize, keys: Vec<String>) {
        let Some(dir) = download_dir(
            self.sources[s].id,
            self.sources[s].info.name,
            self.music.as_deref(),
        ) else {
            return;
        };
        let source = &mut self.sources[s];
        let search = &source.searches[i];
        let wanted: HashSet<&String> = keys.iter().collect();
        let mut items: Vec<Found> = Vec::new();
        for key in &keys {
            let Some(found) = search.find(key) else {
                continue;
            };
            let in_folder = search
                .parent(key)
                .is_some_and(|p| wanted.contains(&p.key) || source.download(&p.key).is_some());
            let busy = source
                .download(key)
                .is_some_and(|d| !matches!(d.progress, Progress::Failed(_)));
            if !in_folder && !busy {
                items.push(found.clone());
            }
        }
        if items.is_empty() {
            return;
        }
        // Keep the order they're listed in (when they're listed).
        let order: Vec<String> = self
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Item(k) => Some(k.clone()),
                _ => None,
            })
            .collect();
        items.sort_by_key(|f| order.iter().position(|k| *k == f.key));
        for f in &items {
            source.downloads.push(Download {
                key: f.key.clone(),
                local: search.local,
                label: label(f),
                progress: Progress::Queued,
                cache: false,
                note: String::new(),
            });
        }
        // Files already in the cache move into the collection from there.
        let (kept, items): (Vec<Found>, Vec<Found>) = items
            .into_iter()
            .partition(|f| !f.folder && source.cached(f).is_some() && relative(&f.file).is_some());
        for f in kept {
            if let (Some(from), Some(rel)) = (source.cached(&f), relative(&f.file)) {
                let _ = source
                    .requests
                    .send(Request::Keep(f.key, from, dir.join(rel)));
            }
        }
        if items.is_empty() {
            self.refresh();
            return;
        }
        let _ = source
            .requests
            .send(Request::Download(search.local, items, dir));
        self.refresh();
    }

    /// Puts the panel, the list and the queue on screen.
    fn refresh(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<SourceUi>();
        let Some(s) = self.shown else {
            self.rows.clear();
            self.list.set_vec(Vec::new());
            return;
        };
        let source = &self.sources[s];
        let items: Vec<SearchItem> = source
            .searches
            .iter()
            .map(|search| SearchItem {
                name: search.name().into(),
                sub: search_line(search).into(),
                kind: if search.items.iter().any(|f| !f.group.is_empty()) {
                    QueryKind::ALL
                        .iter()
                        .position(|k| *k == QueryKind::Playlist)
                        .unwrap_or(0) as i32
                } else {
                    QueryKind::ALL
                        .iter()
                        .position(|k| *k == search.query.kind)
                        .unwrap_or(0) as i32
                },
                busy: !search.done,
            })
            .collect();
        ui.set_searches(ModelRc::new(VecModel::from(items)));
        ui.set_current_search(if self.page {
            -2
        } else {
            self.search.map_or(-1, |i| i as i32)
        });
        let status = source.status.clone().unwrap_or_default();
        ui.set_status_shown(source.status.is_some());
        ui.set_status_title(status.title.as_str().into());
        ui.set_status_line(status.line.as_str().into());
        ui.set_status_note(status.note.as_str().into());
        ui.set_status_lines(ModelRc::new(VecModel::from(status_lines(&status))));
        ui.set_plays(source.cache.is_some());
        ui.set_previews(source.previews);
        let config = crate::settings::plugin_config(source.id);
        let fields: Vec<SetupField> = if needs_setup(&source.settings, &config) {
            source
                .settings
                .iter()
                .filter(|f| matches!(f.kind, SettingKind::Text | SettingKind::Secret))
                .filter(|f| f.key != DOWNLOAD_FOLDER)
                .map(|f| SetupField {
                    key: f.key.into(),
                    title: f.title.as_str().into(),
                    detail: f.detail.as_str().into(),
                    secret: f.kind == SettingKind::Secret,
                    value: self
                        .setup_values
                        .get(f.key)
                        .map(String::as_str)
                        .unwrap_or_default()
                        .into(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let intro = source
            .settings
            .iter()
            .find(|f| f.kind == SettingKind::Note)
            .map(|f| f.detail.clone())
            .unwrap_or_default();
        // Set anew only when it changes, so a field being typed in keeps
        // its focus.
        let old: Vec<SetupField> = ui.get_setup().iter().collect();
        if old.len() != fields.len() || old.iter().zip(&fields).any(|(a, b)| a.key != b.key) {
            ui.set_setup(ModelRc::new(VecModel::from(fields)));
        }
        ui.set_setup_intro(intro.into());
        ui.set_setup_status(self.setup_status.as_str().into());
        ui.set_grouped(
            self.current()
                .is_some_and(|(_, search)| search.items.iter().any(|f| !f.group.is_empty())),
        );
        ui.set_filter(self.filter);
        ui.set_lossless_only(self.lossless_only);
        let count =
            |f: fn(&Progress) -> bool| source.downloads.iter().filter(|d| f(&d.progress)).count();
        let active = count(is_active);
        let done = count(|p| matches!(p, Progress::Done(_)));
        let failed = count(|p| matches!(p, Progress::Failed(_)));
        ui.set_active_count(active as i32);
        ui.set_finished_count((done + failed) as i32);
        ui.set_failed_count(failed as i32);
        ui.set_downloads_line(
            match (active, failed) {
                (0, 0) if done == 0 => "Nothing yet".into(),
                (0, 0) => format!("{done} done"),
                (0, f) => format!("{f} failed"),
                (n, _) => format!("{n} active"),
            }
            .into(),
        );
        let queue: Vec<QueueLine> = source
            .downloads
            .iter()
            .filter(|d| matches!(d.progress, Progress::Running(_) | Progress::Queued))
            .take(3)
            .map(|d| QueueLine {
                text: d.label.as_str().into(),
                progress: match d.progress {
                    Progress::Running(p) => p,
                    _ => 0.0,
                },
            })
            .collect();
        ui.set_queue(ModelRc::new(VecModel::from(queue)));

        let (rows, list) = match self.current() {
            Some((_, search)) => self.result_rows(source, search),
            None if self.page => (Vec::new(), Vec::new()),
            None => self.download_rows(source),
        };
        // Covers of what's listed are fetched once each, in the background.
        let wanted: Vec<String> = match self.current() {
            Some((_, search)) => search.items.iter().map(|f| f.cover_url.clone()).collect(),
            None => Vec::new(),
        };
        for url in wanted {
            if !url.is_empty() && self.asked.insert(url.clone()) {
                let _ = self.cover_requests.send(url);
            }
        }
        let (title, summary, notice) = match self.current() {
            Some((_, search)) => (
                search.name().to_string(),
                search_line(search),
                match (&search.error, search.done, search.items.is_empty()) {
                    (Some(e), _, _) => e.clone(),
                    (None, false, true) => "Searching…".into(),
                    (None, true, true) => "Nothing found.".into(),
                    _ if list.is_empty() => "Nothing matches the filters.".into(),
                    _ => String::new(),
                },
            ),
            None if self.page => (status.title.clone(), status.line.clone(), String::new()),
            None => (
                "Downloads".into(),
                if source.downloads.is_empty() {
                    String::new()
                } else {
                    ui.get_downloads_line().to_string()
                },
                if list.is_empty() {
                    "Nothing downloaded yet.".into()
                } else {
                    String::new()
                },
            ),
        };
        ui.set_title(title.into());
        ui.set_summary(summary.into());
        ui.set_notice(notice.into());
        let picked: Vec<&Found> = self
            .current()
            .map(|(_, search)| self.checked.iter().filter_map(|k| search.find(k)).collect())
            .unwrap_or_default();
        ui.set_selected_count(picked.len() as i32);
        let bytes: u64 = picked.iter().map(|f| f.size).sum();
        ui.set_selected_summary(megabytes(bytes).into());
        self.rows = rows;
        // Rows update in place so the list keeps its scroll position.
        if self.list.row_count() == list.len() {
            for (i, r) in list.into_iter().enumerate() {
                if self.list.row_data(i).as_ref() != Some(&r) {
                    self.list.set_row_data(i, r);
                }
            }
        } else {
            self.list.set_vec(list);
        }
    }

    fn result_rows(&self, source: &Source, search: &Search) -> (Vec<Row>, Vec<FoundRow>) {
        let mut rows = Vec::new();
        let mut list = Vec::new();
        let keep = |f: &Found| {
            !self.lossless_only
                || LOSSLESS.contains(&f.format.to_uppercase().as_str())
                || f.format.is_empty()
        };
        let push = |rows: &mut Vec<Row>, list: &mut Vec<FoundRow>, f: &Found, depth: i32| {
            let parent_checked = search
                .parent(&f.key)
                .is_some_and(|p| self.checked.contains(&p.key));
            let own = source.download(&f.key);
            let inherited = || search.parent(&f.key).and_then(|p| source.download(&p.key));
            rows.push(Row::Item(f.key.clone()));
            let mut row = found_row(
                f,
                self.covers.get(&f.cover_url),
                depth,
                self.checked.contains(&f.key) || parent_checked,
                search.open.contains(&f.key),
                own.or_else(inherited).map(|d| &d.progress),
            );
            if let Some(p) = self.preview.as_ref().filter(|p| p.key == f.key) {
                if self.shown == Some(p.source) {
                    row.preview = match &p.state {
                        PreviewState::Waiting => 1,
                        PreviewState::Playing => 2,
                        PreviewState::Failed(why) => {
                            row.from = why.as_str().into();
                            3
                        }
                    };
                }
            }
            if source.cache.is_some() {
                row.cached = !f.folder && source.cached(f).is_some();
                let caching = source
                    .caching(&f.key)
                    .or_else(|| search.parent(&f.key).and_then(|p| source.caching(&p.key)));
                match caching.map(|d| &d.progress) {
                    Some(Progress::Queued) => row.caching = true,
                    Some(Progress::Running(p)) => {
                        row.caching = true;
                        row.cache_progress = *p;
                    }
                    Some(Progress::Done(_)) if f.folder => row.cached = true,
                    _ => {}
                }
            }
            list.push(row);
        };
        let heading = |rows: &mut Vec<Row>, list: &mut Vec<FoundRow>, text: String| {
            rows.push(Row::Heading(None));
            list.push(FoundRow {
                kind: 0,
                title: text.into(),
                ..Default::default()
            });
        };
        let grouped = search.items.iter().any(|f| !f.group.is_empty());
        if grouped {
            // A playlist: each entry shows one copy, the one downloading or
            // downloaded if there is one, with the others under it when open.
            for group in entries(&search.items) {
                let copies: Vec<&Found> = search
                    .items
                    .iter()
                    .filter(|f| f.group == group && keep(f))
                    .collect();
                if copies.is_empty() {
                    continue;
                }
                let tried = |f: &&Found| source.download(&f.key).map(|d| &d.progress);
                let lead = copies
                    .iter()
                    .position(|f| tried(f).is_some_and(|p| !matches!(p, Progress::Failed(_))))
                    .or_else(|| copies.iter().position(|f| tried(f).is_none()))
                    .unwrap_or(0);
                let open = search.open.contains(&song_key(group));
                let more = copies.len() - 1;
                rows.push(Row::Heading(Some(group.to_string())));
                list.push(FoundRow {
                    kind: 0,
                    title: group.into(),
                    open,
                    note: match more {
                        0 => String::new(),
                        n => plural(n, "other copy", "other copies"),
                    }
                    .into(),
                    ..Default::default()
                });
                push(&mut rows, &mut list, copies[lead], 0);
                if open {
                    for (_, f) in copies.iter().enumerate().filter(|(n, _)| *n != lead) {
                        push(&mut rows, &mut list, f, 1);
                    }
                }
            }
            return (rows, list);
        }
        let folders: Vec<&Found> = search
            .items
            .iter()
            .filter(|f| f.folder && keep(f))
            .collect();
        let files: Vec<&Found> = search
            .items
            .iter()
            .filter(|f| !f.folder && keep(f))
            .collect();
        if self.filter != 1 && !folders.is_empty() {
            if self.filter == 0 {
                heading(&mut rows, &mut list, "Albums".into());
            }
            for f in folders {
                push(&mut rows, &mut list, f, 0);
                if search.open.contains(&f.key) {
                    for t in &f.tracks {
                        push(&mut rows, &mut list, t, 1);
                    }
                }
            }
        }
        if self.filter != 2 && !files.is_empty() {
            if self.filter == 0 && !rows.is_empty() {
                heading(&mut rows, &mut list, "Tracks".into());
            }
            for f in files {
                push(&mut rows, &mut list, f, 0);
            }
        }
        (rows, list)
    }

    fn download_rows(&self, source: &Source) -> (Vec<Row>, Vec<FoundRow>) {
        let mut rows = Vec::new();
        let mut list = Vec::new();
        for (i, d) in source.downloads.iter().enumerate().rev() {
            let found = source
                .searches
                .iter()
                .find(|s| s.local == d.local)
                .and_then(|s| s.find(&d.key))
                .cloned()
                .unwrap_or_else(|| Found {
                    key: d.key.clone(),
                    title: d.label.clone(),
                    ..Default::default()
                });
            rows.push(Row::Download(i));
            let mut row = found_row(
                &found,
                self.covers.get(&found.cover_url),
                0,
                false,
                false,
                Some(&d.progress),
            );
            row.to_cache = d.cache;
            if is_active(&d.progress) {
                row.going = d.note.clone().into();
            }
            list.push(row);
        }
        (rows, list)
    }
}

/// Where a search added at `added` goes in a list kept newest first: after
/// any added at the same time, so a list read back keeps its order.
/// The playlist entries among results, in the order they're listed.
/// A source's own page as the list on the right shows it: each section's
/// heading, then its figures.
fn status_lines(page: &StatusPage) -> Vec<StatusLine> {
    let mut lines = Vec::new();
    for section in &page.sections {
        if !section.heading.is_empty() {
            lines.push(StatusLine {
                heading: true,
                label: section.heading.as_str().into(),
                value: SharedString::new(),
            });
        }
        lines.extend(section.rows.iter().map(|(label, value)| StatusLine {
            heading: false,
            label: label.as_str().into(),
            value: value.as_str().into(),
        }));
    }
    lines
}

fn entries(items: &[Found]) -> Vec<&str> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|f| !f.group.is_empty() && seen.insert(f.group.as_str()))
        .map(|f| f.group.as_str())
        .collect()
}

/// How an open playlist entry is kept in [`Search::open`], apart from keys.
fn song_key(group: &str) -> String {
    format!("song:{group}")
}

/// What a search set to download by itself gets one copy of: each
/// playlist entry, or for a track search the track (results with no group).
/// What a search set to download by itself downloads now: each playlist
/// entry whose results have settled and has no copy downloading or
/// downloaded gets its best copy not yet tried, so a failed copy makes way
/// for the next. Entries whose download the DJ took off the list are left.
fn auto_picks(source: &Source, search: &Search) -> Vec<String> {
    let mut keys = Vec::new();
    for group in wanted(&search.items, search.query.kind) {
        if search.dropped.contains(&group) {
            continue;
        }
        let group = group.as_str();
        let settled = search.done
            || search
                .settling
                .get(group)
                .is_some_and(|(_, looks)| *looks >= SETTLED);
        let copies: Vec<&Found> = search
            .items
            .iter()
            .filter(|f| f.group == group && !f.folder)
            .collect();
        let going = copies.iter().any(|f| {
            source
                .download(&f.key)
                .is_some_and(|d| !matches!(d.progress, Progress::Failed(_)))
        });
        if !settled || going {
            continue;
        }
        if let Some(next) = copies.iter().find(|f| source.download(&f.key).is_none()) {
            keys.push(next.key.clone());
        }
    }
    keys
}

/// The DJ took the download of `key` (from search `local`) off the list:
/// its playlist entry isn't downloaded by itself again ("Download each song
/// right away").
fn drop_entry(source: &mut Source, local: u64, key: &str) {
    let Some(search) = source.searches.iter_mut().find(|x| x.local == local) else {
        return;
    };
    if let Some(group) = search
        .find(key)
        .map(|f| f.group.clone())
        .filter(|g| !g.is_empty())
    {
        search.dropped.insert(group);
    }
}

fn wanted(items: &[Found], kind: QueryKind) -> Vec<String> {
    let groups = entries(items);
    if !groups.is_empty() {
        return groups.into_iter().map(str::to_string).collect();
    }
    if kind == QueryKind::Track && items.iter().any(|f| !f.folder) {
        return vec![String::new()];
    }
    Vec::new()
}

/// Counts how many looks in a row each wanted entry's results stayed the
/// same.
fn settle(settling: &mut HashMap<String, (Vec<String>, u32)>, items: &[Found], wanted: &[String]) {
    for group in wanted {
        let group = group.as_str();
        let keys: Vec<String> = items
            .iter()
            .filter(|f| f.group == group && !f.folder)
            .map(|f| f.key.clone())
            .collect();
        let entry = settling
            .entry(group.to_string())
            .or_insert_with(|| (Vec::new(), 0));
        if entry.0 == keys {
            entry.1 += 1;
        } else {
            *entry = (keys, 1);
        }
    }
}

/// A playlist's entries that needn't be searched for again: one of their
/// copies was downloaded, or they were found and none failed.
fn found_entries(source: &Source, search: &Search) -> HashSet<String> {
    entries(&search.items)
        .into_iter()
        .filter(|group| {
            let progress: Vec<Option<&Progress>> = search
                .items
                .iter()
                .filter(|f| f.group == *group)
                .map(|f| source.download(&f.key).map(|d| &d.progress))
                .collect();
            progress
                .iter()
                .any(|p| matches!(p, Some(Progress::Done(_))))
                || !progress
                    .iter()
                    .any(|p| matches!(p, Some(Progress::Failed(_))))
        })
        .map(str::to_string)
        .collect()
}

/// Results of a search again with the entries kept from before, listed by
/// their number in the playlist ("3. Bonobo – Kerala").
fn with_kept(kept: &[Found], items: Vec<Found>) -> Vec<Found> {
    let number = |f: &Found| -> u32 {
        let digits: String = f.group.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().unwrap_or(u32::MAX)
    };
    let mut all: Vec<Found> = kept
        .iter()
        .filter(|k| !items.iter().any(|f| f.group == k.group))
        .cloned()
        .collect();
    all.extend(items);
    all.sort_by_key(number);
    all
}

fn place(searches: &[Search], added: u64) -> usize {
    searches
        .iter()
        .position(|x| x.added < added)
        .unwrap_or(searches.len())
}

/// Whether a source with these settings still needs some filled in: a
/// text or password field (not its download folder) left empty.
pub fn needs_setup(settings: &[Setting], config: &PluginConfig) -> bool {
    settings.iter().any(|s| {
        matches!(s.kind, SettingKind::Text | SettingKind::Secret)
            && s.key != DOWNLOAD_FOLDER
            && config.get(s.key).trim().is_empty()
    })
}

/// Where playing sources keep their caches: `sources` in the app's cache
/// folder, one folder per source.
fn caches_root() -> Option<PathBuf> {
    djsugar_engine::cache::cache_dir().map(|d| d.join("sources"))
}

/// Where a playing source keeps its cache: `sources/<name>` in the app's
/// cache folder.
fn cache_dir(name: &str) -> Option<PathBuf> {
    caches_root().map(|d| d.join(safe_name(name, "Source")))
}

/// The setting for how big all sources' caches may grow together, in GB.
pub const CACHE_GB: &str = "sources_cache_gb";

/// How big the caches grow when the DJ hasn't picked a size.
pub const DEFAULT_CACHE_GB: f64 = 10.0;

/// How big all sources' caches may grow together, in bytes (Settings ›
/// Sources).
fn cache_bytes() -> u64 {
    let gb = crate::settings::number(CACHE_GB, DEFAULT_CACHE_GB);
    let gb = if gb.is_finite() && gb >= 0.0 {
        gb
    } else {
        DEFAULT_CACHE_GB
    };
    (gb * 1e9) as u64
}

/// The bytes in the sources' caches together, and in each source's by its
/// folder name.
pub fn cache_usage() -> (u64, HashMap<String, u64>) {
    let mut each = HashMap::new();
    let Some(root) = caches_root() else {
        return (0, each);
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return (0, each);
    };
    for e in entries.flatten() {
        if e.path().is_dir() {
            each.insert(
                e.file_name().to_string_lossy().into_owned(),
                folder_size(&e.path()),
            );
        }
    }
    (each.values().sum(), each)
}

/// The key [`cache_usage`] lists source `name`'s cache under.
pub fn cache_key(name: &str) -> String {
    safe_name(name, "Source")
}

fn folder_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

/// Empties every source's cache; returns the bytes freed. Tracks kept in
/// the collection aren't in it, so they stay.
pub fn clear_cache() -> u64 {
    let (used, _) = cache_usage();
    if let Some(root) = caches_root() {
        make_room(&root, 0);
    }
    used
}

/// A source's path for a file, if it stays below the folder it's joined
/// to: no `..`, no root.
fn relative(file: &str) -> Option<&Path> {
    let path = Path::new(file);
    let plain = !file.is_empty()
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
    plain.then_some(path)
}

/// Deletes the oldest files in `dir` (and folders left empty) until what's
/// left is at most `room` bytes, leaving room for what comes next.
fn make_room(dir: &Path, room: u64) {
    let mut files = Vec::new();
    let mut folders = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                folders.push(e.path());
                stack.push(e.path());
            } else {
                let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                files.push((modified, meta.len(), e.path()));
            }
        }
    }
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, size, path) in files {
        if total <= room {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
    // Deepest first, so a folder emptied by its subfolders goes too.
    folders.sort_by_key(|f| std::cmp::Reverse(f.components().count()));
    for f in folders {
        let _ = std::fs::remove_dir(&f);
    }
}

/// Moves a cached file to `to` in the collection: renamed when it's on the
/// same disk, else copied and the cached one removed.
fn keep(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if to.exists() {
        let _ = std::fs::remove_file(from);
        return Ok(());
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    let part = to.with_extension("part");
    std::fs::copy(from, &part).map_err(|e| e.to_string())?;
    std::fs::rename(&part, to).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

/// Where a source's downloads go: its download folder setting, else
/// `<music>/Downloaded/<source>`.
fn download_dir(id: &str, name: &str, music: Option<&Path>) -> Option<PathBuf> {
    let config = crate::settings::plugin_config(id);
    folder_or_default(config.get(DOWNLOAD_FOLDER), name, music)
}

fn folder_or_default(set: &str, name: &str, music: Option<&Path>) -> Option<PathBuf> {
    let set = set.trim();
    if set.is_empty() {
        return music.map(|m| m.join("Downloaded").join(safe_name(name, "Downloads")));
    }
    djsugar_engine::paths::expand_tilde(set)
}

/// Queued or running.
fn is_active(p: &Progress) -> bool {
    matches!(p, Progress::Queued | Progress::Running(_))
}

/// A search typed into the search bar, with the kind and copy picked under
/// its cog.
fn query(text: &str, kind: i32, prefer: i32) -> SourceQuery {
    SourceQuery {
        text: text.trim().to_string(),
        kind: QueryKind::ALL
            .get(kind as usize)
            .copied()
            .unwrap_or_default(),
        prefer: Prefer::ALL
            .get(prefer as usize)
            .copied()
            .unwrap_or_default(),
        ..Default::default()
    }
}

/// "Artist – Title", or a folder's album.
fn label(f: &Found) -> String {
    let name = if f.folder && !f.album.is_empty() {
        &f.album
    } else {
        &f.title
    };
    if f.artist.is_empty() {
        name.clone()
    } else {
        format!("{} – {name}", f.artist)
    }
}

/// The line under a search in the panel.
fn search_line(search: &Search) -> String {
    if let Some(e) = &search.error {
        return e.lines().next().unwrap_or_default().to_string();
    }
    let folders = search.items.iter().filter(|f| f.folder).count();
    let files = search.items.len() - folders;
    let mut parts = Vec::new();
    if files > 0 {
        parts.push(plural(files, "track", "tracks"));
    }
    if folders > 0 {
        parts.push(plural(folders, "album", "albums"));
    }
    let found = parts.join(" · ");
    match (search.done, found.is_empty(), search.note.is_empty()) {
        (_, _, false) if search.done => search.note.clone(),
        (false, true, _) => "Searching…".into(),
        (false, false, _) => format!("Searching… {found} so far"),
        (true, true, _) => "Nothing found".into(),
        (true, false, _) => found,
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Who a download comes from, with how it's going: "dj_k · 4th in line".
fn found_row(
    f: &Found,
    cover: Option<&slint::Image>,
    depth: i32,
    checked: bool,
    open: bool,
    progress: Option<&Progress>,
) -> FoundRow {
    let (state, fraction, note) = match progress {
        None => (0, 0.0, String::new()),
        Some(Progress::Queued) => (1, 0.0, String::new()),
        Some(Progress::Running(p)) => (2, *p, String::new()),
        Some(Progress::Done(_)) => (3, 1.0, String::new()),
        Some(Progress::Failed(e)) => (4, 0.0, e.clone()),
    };
    let length = if f.duration > 0.0 {
        let s = f.duration.round() as u64;
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        String::new()
    };
    let detail = if f.folder {
        let n = f.track_count.max(f.tracks.len());
        [
            f.artist.clone(),
            f.year.clone(),
            if n > 0 {
                plural(n, "track", "tracks")
            } else {
                String::new()
            },
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
    } else {
        String::new()
    };
    FoundRow {
        kind: if f.folder { 1 } else { 2 },
        depth,
        checked,
        open,
        title: if f.folder && !f.album.is_empty() {
            f.album.as_str().into()
        } else {
            f.title.as_str().into()
        },
        artist: if f.folder {
            detail.into()
        } else {
            f.artist.as_str().into()
        },
        number: f.number.as_str().into(),
        length: length.into(),
        format: f.format.as_str().into(),
        quality: f.quality.as_str().into(),
        size: megabytes(f.size).into(),
        from: f.from.as_str().into(),
        state,
        progress: fraction,
        note: note.into(),
        cover: cover.cloned().unwrap_or_default(),
        has_cover: cover.is_some(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_paths_stay_inside() {
        assert_eq!(
            relative("Bonobo/Migration/03 Kerala.flac"),
            Some(Path::new("Bonobo/Migration/03 Kerala.flac"))
        );
        assert_eq!(relative("../escape.flac"), None);
        assert_eq!(relative("/etc/passwd"), None);
        assert_eq!(relative(""), None);
    }

    #[test]
    fn makes_room_oldest_first() {
        let dir = std::env::temp_dir().join(format!("djsugar-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let old = dir.join("A/Old");
        let new = dir.join("B/New");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        let first = old.join("1.flac");
        std::fs::write(&first, vec![0u8; 600]).unwrap();
        let earlier = std::time::SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&first)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
        std::fs::write(new.join("2.flac"), vec![0u8; 600]).unwrap();
        make_room(&dir, 1000);
        assert!(!first.exists());
        assert!(!old.exists(), "an emptied folder goes too");
        assert!(new.join("2.flac").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn needs_setup_until_every_field_is_filled() {
        let settings = vec![
            Setting::new("", SettingKind::Note, "About", "A note"),
            Setting::new("server", SettingKind::Text, "Server", ""),
            Setting::new("token", SettingKind::Secret, "Token", ""),
            Setting::new(DOWNLOAD_FOLDER, SettingKind::Folder, "Folder", ""),
        ];
        let mut config = PluginConfig::default();
        assert!(needs_setup(&settings, &config));
        config
            .values
            .insert("server".into(), "http://x:32400".into());
        config.values.insert("token".into(), "  ".into());
        assert!(needs_setup(&settings, &config), "blank isn't filled in");
        config.values.insert("token".into(), "abc".into());
        assert!(
            !needs_setup(&settings, &config),
            "the folder may stay empty"
        );
        assert!(!needs_setup(&[], &config), "nothing to fill in");
    }

    #[test]
    fn keeps_a_cached_file() {
        let dir = std::env::temp_dir().join(format!("djsugar-keep-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let from = dir.join("cached.flac");
        std::fs::write(&from, b"audio").unwrap();
        let to = dir.join("Collection/Artist/Album/cached.flac");
        keep(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(std::fs::read(&to).unwrap(), b"audio");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn file(key: &str, title: &str) -> Found {
        Found {
            key: key.into(),
            title: title.into(),
            artist: "Bonobo".into(),
            format: "FLAC".into(),
            ..Default::default()
        }
    }

    fn search(items: Vec<Found>, done: bool) -> Search {
        Search {
            local: 1,
            query: SourceQuery::default(),
            added: 0,
            items,
            done,
            note: String::new(),
            title: String::new(),
            error: None,
            open: HashSet::new(),
            auto: false,
            settling: HashMap::new(),
            dropped: HashSet::new(),
            kept: Vec::new(),
        }
    }

    /// The rail numbers add-on sources on from its own list of built-in
    /// sources. If that list and ours differ, picking an add-on's source
    /// shows its panel but its searches are neither listed nor started.
    #[test]
    fn rail_lists_the_built_in_sources() {
        let ui = include_str!("../ui/library.slint");
        let lists: Vec<Vec<&str>> = [
            "property <[string]> names: [",
            "property <[string]> source-names: [",
        ]
        .iter()
        .map(|start| {
            let from = ui.find(start).expect(start) + start.len();
            let len = ui[from..].find(']').expect("a closing ]");
            ui[from..from + len]
                .split(',')
                .map(|n| n.trim().trim_matches('"'))
                .collect()
        })
        .collect();
        for list in lists {
            assert_eq!(list, crate::browser::SOURCE_NAMES);
        }
    }

    fn copy(key: &str, group: &str) -> Found {
        Found {
            group: group.into(),
            ..file(key, "Kerala")
        }
    }

    #[test]
    fn entries_settle_when_their_results_stop_changing() {
        let mut settling = HashMap::new();
        let kerala = ["1. Kerala".to_string()];
        let first = vec![copy("a", "1. Kerala")];
        settle(&mut settling, &first, &kerala);
        settle(&mut settling, &first, &kerala);
        assert_eq!(settling["1. Kerala"].1, 2);
        let more = vec![copy("a", "1. Kerala"), copy("b", "1. Kerala")];
        settle(&mut settling, &more, &kerala);
        assert_eq!(settling["1. Kerala"].1, 1);
        // A track search downloads one copy of the track; an album search
        // nothing by itself.
        assert_eq!(wanted(&first, QueryKind::Track), ["1. Kerala"]);
        assert_eq!(wanted(&[file("a", "Kerala")], QueryKind::Track), [""]);
        assert!(wanted(&[file("a", "Kerala")], QueryKind::Album).is_empty());
    }

    #[test]
    fn search_again_keeps_entries_in_playlist_order() {
        let kept = vec![copy("a", "1. Kerala"), copy("c", "10. Kong")];
        let new = vec![copy("b", "2. Cirrus"), copy("d", "11. Grains")];
        let all: Vec<String> = with_kept(&kept, new).into_iter().map(|f| f.key).collect();
        assert_eq!(all, ["a", "b", "c", "d"]);
        assert_eq!(
            entries(&[
                copy("a", "1. Kerala"),
                copy("b", "1. Kerala"),
                copy("c", "2. Kong")
            ]),
            ["1. Kerala", "2. Kong"]
        );
    }

    #[test]
    fn found_or_downloaded_entries_are_not_searched_again() {
        let (requests, _rx) = channel();
        let mut source = Source {
            id: "x",
            info: SourceInfo {
                name: "X",
                icon_svg: b"",
                hint: "",
            },
            sites: &[],
            requests,
            searches: Vec::new(),
            downloads: Vec::new(),
            cache: None,
            settings: Vec::new(),
            status: None,
            previews: false,
        };
        let progress = |key: &str, progress| Download {
            key: key.into(),
            local: 1,
            label: String::new(),
            progress,
            cache: false,
            note: String::new(),
        };
        source.downloads = vec![
            progress("a", Progress::Done(Vec::new())),
            progress("b", Progress::Failed("gone".into())),
            progress("c", Progress::Failed("gone".into())),
            progress("d", Progress::Done(Vec::new())),
        ];
        let s = search(
            vec![
                copy("a", "1. Kerala"),
                copy("b", "2. Kong"),
                copy("c", "3. Cirrus"),
                copy("d", "3. Cirrus"),
                copy("e", "4. Grains"),
            ],
            true,
        );
        let mut skip: Vec<String> = found_entries(&source, &s).into_iter().collect();
        skip.sort();
        // Kong's only copy failed, so it's searched for again.
        assert_eq!(skip, ["1. Kerala", "3. Cirrus", "4. Grains"]);
    }

    #[test]
    fn removed_downloads_are_not_picked_again() {
        let (requests, _rx) = channel();
        let mut source = Source {
            id: "x",
            info: SourceInfo {
                name: "X",
                icon_svg: b"",
                hint: "",
            },
            sites: &[],
            requests,
            searches: vec![search(
                vec![
                    copy("a", "1. Kerala"),
                    copy("b", "1. Kerala"),
                    copy("c", "2. Kong"),
                ],
                true,
            )],
            downloads: Vec::new(),
            cache: None,
            settings: Vec::new(),
            status: None,
            previews: false,
        };
        assert_eq!(auto_picks(&source, &source.searches[0]), ["a", "c"]);
        // The DJ took Kerala's download off the list: it stays off, and
        // its other copy isn't tried either.
        drop_entry(&mut source, 1, "a");
        assert_eq!(auto_picks(&source, &source.searches[0]), ["c"]);
    }

    #[test]
    fn download_folders() {
        let music = Path::new("/m");
        assert_eq!(
            folder_or_default("", "Soulseek", Some(music)),
            Some(PathBuf::from("/m/Downloaded/Soulseek"))
        );
        assert_eq!(folder_or_default(" ", "Soulseek", None), None);
        assert_eq!(
            folder_or_default("/data/slsk", "Soulseek", None),
            Some(PathBuf::from("/data/slsk"))
        );
        let home = djsugar_engine::paths::home_dir().unwrap();
        assert_eq!(
            folder_or_default("~/Music/In", "Soulseek", None),
            Some(home.join("Music/In"))
        );
    }

    #[test]
    fn listed_by_the_sources_name() {
        let mut s = search(vec![], true);
        s.query.text = "https://open.spotify.com/playlist/x".into();
        assert_eq!(s.name(), "https://open.spotify.com/playlist/x");
        s.title = "Psy chill".into();
        assert_eq!(s.name(), "Psy chill");
    }

    #[test]
    fn newest_searches_first() {
        let list: Vec<Search> = [30, 20, 20, 10]
            .into_iter()
            .map(|added| Search {
                added,
                ..search(vec![], true)
            })
            .collect();
        assert_eq!(place(&list, 40), 0);
        assert_eq!(place(&list, 20), 3);
        assert_eq!(place(&list, 5), 4);
        assert_eq!(place(&[], 5), 0);
    }

    #[test]
    fn lines_under_searches() {
        let album = Found {
            folder: true,
            tracks: vec![file("a1", "Migration")],
            ..file("a", "")
        };
        assert_eq!(search_line(&search(vec![], false)), "Searching…");
        assert_eq!(
            search_line(&search(vec![file("k", "Kerala")], false)),
            "Searching… 1 track so far"
        );
        assert_eq!(
            search_line(&search(vec![file("k", "Kerala"), album], true)),
            "1 track · 1 album"
        );
        assert_eq!(search_line(&search(vec![], true)), "Nothing found");
        let mut noted = search(vec![file("k", "Kerala")], true);
        noted.note = "18 of 22 found".into();
        assert_eq!(search_line(&noted), "18 of 22 found");
    }

    #[test]
    fn files_are_found_in_folders() {
        let album = Found {
            folder: true,
            album: "Migration".into(),
            tracks: vec![file("a1", "Migration"), file("a3", "Kerala")],
            ..file("a", "")
        };
        let s = search(vec![album, file("k", "Kong")], true);
        assert_eq!(s.find("a3").unwrap().title, "Kerala");
        assert_eq!(s.parent("a3").unwrap().key, "a");
        assert!(s.parent("k").is_none());
        assert_eq!(label(s.find("a").unwrap()), "Bonobo – Migration");
    }

    #[test]
    fn rows_show_download_state() {
        let r = found_row(&file("k", "Kong"), None, 0, true, false, None);
        assert_eq!((r.kind, r.state, r.checked), (2, 0, true));
        let r = found_row(
            &Found {
                duration: 302.0,
                ..file("k", "Kong")
            },
            None,
            0,
            false,
            false,
            Some(&Progress::Running(0.5)),
        );
        assert_eq!((r.state, r.progress, r.length.as_str()), (2, 0.5, "5:02"));
        let r = found_row(
            &file("k", "Kong"),
            None,
            0,
            false,
            false,
            Some(&Progress::Failed("gone".into())),
        );
        assert_eq!((r.state, r.note.as_str()), (4, "gone"));
    }

    #[test]
    fn queries_from_the_search_bar() {
        let q = query("  Bonobo - Kerala ", 2, 1);
        assert_eq!(q.text, "Bonobo - Kerala");
        assert_eq!(q.kind, QueryKind::Album);
        assert_eq!(q.prefer, Prefer::Mp3);
        assert_eq!(query("x", 99, -1).kind, QueryKind::Auto);
    }

    /// A source whose previews start sending after a few asks.
    struct Previewer {
        asks: usize,
    }

    impl Plugin for Previewer {
        fn manifest(&self) -> &djsugar_plugins::Manifest {
            static M: djsugar_plugins::Manifest = djsugar_plugins::Manifest {
                id: "previewer",
                name: "Previewer",
                hosts: &[],
                downloads: true,
            };
            &M
        }
        fn source(&mut self) -> Option<&mut dyn SearchSource> {
            Some(self)
        }
    }

    impl SearchSource for Previewer {
        fn info(&self) -> SourceInfo {
            SourceInfo {
                name: "Previewer",
                icon_svg: b"",
                hint: "",
            }
        }
        fn describe(&self, _: &SourceQuery) -> Vec<String> {
            Vec::new()
        }
        fn start(&mut self, _: &SourceQuery) -> Result<String, PluginError> {
            Ok("1".into())
        }
        fn results(&mut self, _: &str) -> Result<Snapshot, PluginError> {
            Ok(Snapshot::default())
        }
        fn download(&mut self, _: &str, _: &[Found], _: &Path) -> Result<(), PluginError> {
            Ok(())
        }
        fn progress(&mut self) -> Vec<DownloadUpdate> {
            Vec::new()
        }
        fn previews(&self) -> bool {
            true
        }
        fn preview(&mut self, _: &str, item: &Found) -> Result<(), PluginError> {
            if item.key == "bad" {
                return Err(PluginError::NotSupported);
            }
            self.asks = 0;
            Ok(())
        }
        fn preview_file(&mut self) -> Result<Option<PathBuf>, PluginError> {
            self.asks += 1;
            Ok((self.asks >= 2).then(|| PathBuf::from("/incoming/kerala.mp3")))
        }
    }

    #[test]
    fn previews_wait_for_the_first_bytes() {
        let (requests, rx) = channel();
        let (tx, events) = channel();
        std::thread::spawn(move || run(Box::new(Previewer { asks: 0 }), 0, None, rx, tx));
        let file = |key: &str| {
            Box::new(Found {
                key: key.into(),
                size: 1000,
                ..Default::default()
            })
        };
        let next = || loop {
            let e = events
                .recv_timeout(Duration::from_secs(5))
                .expect("an event");
            if matches!(e, Event::PreviewFile(..) | Event::PreviewFailed(..)) {
                return e;
            }
        };
        requests.send(Request::Preview(1, file("bad"))).unwrap();
        assert!(matches!(next(), Event::PreviewFailed(0, why) if why == "Can't preview this."));
        requests.send(Request::Preview(1, file("good"))).unwrap();
        assert!(matches!(
            next(),
            Event::PreviewFile(0, path) if path == Path::new("/incoming/kerala.mp3")
        ));
    }

    #[test]
    fn kept_results_read_back() {
        let found = Found {
            key: "k1".into(),
            artist: "Bonobo".into(),
            title: "Kerala".into(),
            size: 5,
            ..Default::default()
        };
        let text = serde_json::json!({
            "1700": {
                "items": [djsugar_plugins::remote::found_json(&found)],
                "note": "1 found",
                "title": "",
            },
            "1800": { "items": [], "note": "", "title": "" },
            "x": { "items": [], "note": "", "title": "" },
        })
        .to_string();
        let kept = read_kept(&text);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[&1700], (vec![found], "1 found".into(), String::new()));
        assert!(read_kept("not json").is_empty());
    }
}
