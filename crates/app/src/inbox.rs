//! The purchase inbox, on a thread of its own: watches the inbox folder,
//! files what arrives into the music folder and adds it to the library
//! (see [`djsugar_plugins::inbox`]).
//!
//! Files a store plugin recognises (or that match a track the DJ went to
//! buy) are filed under `Purchased/<Store>/…` as the settings say. Other
//! audio is added where it is, and zips nobody recognises are left alone,
//! so a sample pack in Downloads doesn't flood the library.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, UNIX_EPOCH};

use djsugar_engine::scan::{read_comment, read_track};
use djsugar_engine::{Source, Track};
use djsugar_library::Library;
use djsugar_plugins::inbox::{self, Mode, Watcher};
use djsugar_plugins::{InboxFile, Plugin};

use crate::settings::InboxSettings;

const POLL: Duration = Duration::from_secs(1);

pub enum Event {
    /// A track joined the library (and its Inbox list).
    Imported(Box<Track>),
    /// What the inbox is doing, for the Stores sidebar.
    Status(String),
}

pub enum Request {
    /// On (taking files that arrive from the given time, in seconds since
    /// 1970) or off.
    SetOn(bool, u64),
    SetMode(Mode),
    /// Watch the Music app's folder from the given time, or stop.
    WatchMusicApp(Option<(PathBuf, u64)>),
}

/// How many folders down the Music app files its tracks: Artist/Album/.
const MUSIC_APP_DEPTH: usize = 2;

fn since(secs: u64) -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn music_app_watcher(dir: PathBuf, from: u64) -> Watcher {
    Watcher::new(dir, since(from)).deep(MUSIC_APP_DEPTH)
}

pub struct Inbox {
    pub dir: Option<PathBuf>,
    requests: Sender<Request>,
    events: Receiver<Event>,
}

impl Inbox {
    /// Starts watching, if there's a library to add to and a folder to watch.
    pub fn start(
        settings: InboxSettings,
        library: Option<PathBuf>,
        music: Option<PathBuf>,
        plugins: Vec<Box<dyn Plugin>>,
    ) -> Self {
        let (requests, rx) = channel();
        let (tx, events) = channel();
        let dir = settings.dir.clone();
        if let (Some(dir), Some(library), Some(music)) = (dir.clone(), library, music) {
            std::thread::Builder::new()
                .name("inbox".into())
                .spawn(move || {
                    let worker = Worker {
                        watcher: Watcher::new(dir, since(settings.since)),
                        music_app: settings
                            .music_app
                            .map(|(dir, from)| music_app_watcher(dir, from)),
                        on: settings.on,
                        mode: settings.mode,
                        library,
                        music,
                        plugins,
                        tx,
                        imported: 0,
                    };
                    worker.run(rx)
                })
                .expect("spawn inbox thread");
        }
        Self {
            dir,
            requests,
            events,
        }
    }

    pub fn send(&self, r: Request) {
        let _ = self.requests.send(r);
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

struct Worker {
    watcher: Watcher,
    /// The Music app's folder: only files that are Wanted tracks being
    /// bought are taken from it, and left where they are.
    music_app: Option<Watcher>,
    on: bool,
    mode: Mode,
    library: PathBuf,
    music: PathBuf,
    plugins: Vec<Box<dyn Plugin>>,
    tx: Sender<Event>,
    imported: usize,
}

impl Worker {
    fn run(mut self, rx: Receiver<Request>) {
        self.status();
        loop {
            match rx.recv_timeout(POLL) {
                Ok(Request::SetOn(on, from)) => {
                    if on && !self.on {
                        self.watcher = Watcher::new(self.watcher.dir.clone(), since(from));
                    }
                    self.on = on;
                    self.status();
                }
                Ok(Request::SetMode(mode)) => self.mode = mode,
                Ok(Request::WatchMusicApp(folder)) => {
                    self.music_app = folder.map(|(dir, from)| music_app_watcher(dir, from));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return,
            }
            if !self.on {
                continue;
            }
            let now = Instant::now();
            let ready = self.watcher.poll(now);
            let from_app = self
                .music_app
                .as_mut()
                .map(|w| w.poll(now))
                .unwrap_or_default();
            if ready.is_empty() && from_app.is_empty() {
                continue;
            }
            let mut db = match Library::open(&self.library) {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("inbox: can't open the library: {e}");
                    continue;
                }
            };
            for path in ready {
                if db.inbox_seen(&path).unwrap_or(false) {
                    continue;
                }
                let outcome = match self.take(&mut db, &path) {
                    Ok(o) => o,
                    Err(e) => format!("failed: {e}"),
                };
                let _ = db.log_inbox(&path, &outcome, None);
            }
            for path in from_app {
                if db.inbox_seen(&path).unwrap_or(false) {
                    continue;
                }
                let outcome = match self.take_bought(&mut db, &path) {
                    Ok(Some(o)) => o,
                    // Not a track being bought: the Music app's own music.
                    Ok(None) => continue,
                    Err(e) => format!("failed: {e}"),
                };
                let _ = db.log_inbox(&path, &outcome, None);
            }
            self.status();
        }
    }

    fn status(&self) {
        let dir = self.watcher.dir.display();
        let text = match (self.on, self.imported) {
            (false, _) => format!("Off. Turn it on to import from {dir}."),
            (true, 0) => format!("Watching {dir}"),
            (true, 1) => format!("Watching {dir} · 1 track imported"),
            (true, n) => format!("Watching {dir} · {n} tracks imported"),
        };
        let _ = self.tx.send(Event::Status(text));
    }

    /// Deals with one finished download, returning what happened to it.
    fn take(&mut self, db: &mut Library, path: &Path) -> Result<String, String> {
        let is_zip = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        if !is_zip {
            return self.take_file(db, path, None, self.mode);
        }
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let staging = self
            .watcher
            .dir
            .join(".djsugar-unpacked")
            .join(inbox::safe_name(&name, "zip"));
        let audio = inbox::unpack(path, &staging).map_err(|e| e.to_string())?;
        let archive = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let files: Vec<InboxFile> = audio
            .iter()
            .map(|f| describe(f, archive.clone()).0)
            .collect();
        let known = files.iter().any(|f| self.recognised(db, f).is_some());
        if !known {
            let _ = std::fs::remove_dir_all(&staging);
            return Ok("skipped: no store recognised this zip".into());
        }
        let mut outcomes = Vec::new();
        let mut album_dir = None;
        for f in &audio {
            // What was unpacked always moves out of the staging folder.
            match self.take_file(db, f, archive.clone(), Mode::Move) {
                Ok(o) => {
                    if album_dir.is_none() && o.starts_with("imported") {
                        album_dir = db_path_dir(&o);
                    }
                    outcomes.push(o)
                }
                Err(e) => outcomes.push(format!("failed: {e}")),
            }
        }
        // The cover goes with the tracks, where the art lookup finds it.
        if let (Some(dir), Ok(entries)) = (album_dir, std::fs::read_dir(&staging)) {
            for e in entries.flatten() {
                let to = dir.join(e.file_name());
                if !to.exists() {
                    let _ = inbox::file_into(&e.path(), &to, Mode::Move);
                }
            }
        }
        let _ = std::fs::remove_dir_all(&staging);
        let all_in = outcomes.iter().all(|o| !o.starts_with("failed"));
        if all_in && self.mode == Mode::Move {
            let _ = std::fs::remove_file(path);
        }
        Ok(outcomes.join("; "))
    }

    /// A file in the Music app's folder: added where it is when it's a
    /// Wanted track being bought, else left alone (None).
    fn take_bought(&mut self, db: &mut Library, path: &Path) -> Result<Option<String>, String> {
        let (_, track) = describe(path, None);
        let pending = db
            .wanted_for(
                &track.artist,
                &track.title,
                track.duration,
                &track.info.isrc,
                true,
            )
            .map_err(|e| e.to_string())?;
        if pending.is_none() {
            return Ok(None);
        }
        self.take_file(db, path, None, Mode::Leave).map(Some)
    }

    /// The store a file came from: the surest plugin, or the store of a
    /// track the DJ went to buy. `Some("")` when only the wanted list knows.
    fn recognised(&self, db: &Library, f: &InboxFile) -> Option<(String, bool)> {
        if let Some(m) = inbox::recognise(f, &self.plugins) {
            return Some((m.name.to_string(), m.downloads));
        }
        // A Wanted track: the store it's being bought on. One that went to
        // the Wanted list from a streaming service and wasn't opened with
        // Buy came from a store DJ Sugar doesn't know.
        let w = db
            .wanted_for(&f.artist, &f.title, f.duration, &f.isrc, false)
            .ok()??;
        let store = djsugar_plugins::buy::store_traits(&w.pending_store)
            .or_else(|| djsugar_plugins::buy::store_traits(&w.store.to_lowercase()))
            .map_or("Other", |t| t.name);
        Some((store.to_string(), false))
    }

    fn take_file(
        &mut self,
        db: &mut Library,
        path: &Path,
        archive: Option<String>,
        mode: Mode,
    ) -> Result<String, String> {
        let (mut file, mut track) = describe(path, archive);
        // An untagged file of a Wanted track takes its artist and title
        // from the Wanted list, for its folder and the library.
        if track.artist.trim().is_empty() {
            if let Ok(Some(w)) =
                db.wanted_for("", &track.title, track.duration, &track.info.isrc, false)
            {
                track.artist = w.artist.clone();
                track.title = w.title.clone();
                file.artist = w.artist;
                file.title = w.title;
            }
        }
        if let Ok(Some(_)) = db.find_same(&track.artist, &track.title, track.duration) {
            if file.archive.is_some() {
                let _ = std::fs::remove_file(path);
            }
            return Ok("duplicate: already in your collection".into());
        }
        let store = self.recognised(db, &file);
        let (dest, mode) = match &store {
            Some((name, downloads)) => (
                inbox::destination(&self.music, Some(name), *downloads, &file),
                mode,
            ),
            // Nobody knows where it's from: add it where it is.
            None => (path.to_path_buf(), Mode::Leave),
        };
        let now_at = inbox::file_into(path, &dest, mode).map_err(|e| e.to_string())?;
        track.source = Source::File(now_at.clone());
        let store_name = store.map(|(n, _)| n).unwrap_or_default();
        let track = db
            .add_purchase(&track, &store_name)
            .map_err(|e| e.to_string())?;
        if let Ok(list) = db.inbox_list() {
            let _ = db.add_to_playlist(list, &[track.id]);
        }
        let _ = db.log_inbox(&now_at, "imported", Some(track.id));
        self.imported += 1;
        let _ = self.tx.send(Event::Imported(Box::new(track)));
        Ok(format!("imported: {}", now_at.display()))
    }
}

/// Adds a file a source add-on downloaded (already filed where it belongs)
/// to the library and its Inbox list, as from `store`.
pub fn add_download(library: &Path, path: &Path, store: &str) -> Result<Track, String> {
    let mut db = Library::open(library).map_err(|e| e.to_string())?;
    let (_, mut track) = describe(path, None);
    if let Ok(Some(same)) = db.find_same(&track.artist, &track.title, track.duration) {
        return Err(format!("already in your collection as track {same}"));
    }
    track.source = Source::File(path.to_path_buf());
    let mut track = db.add_purchase(&track, store).map_err(|e| e.to_string())?;
    if let Ok(list) = db.inbox_list() {
        let _ = db.add_to_playlist(list, &[track.id]);
    }
    // Downloads land outside the stage folders, so say where they are.
    if track.info.stage.is_empty() && db.put_in_inbox(track.id).is_ok() {
        track.info.stage = djsugar_library::stages::Stage::Inbox.name().into();
    }
    let _ = db.log_inbox(path, "imported", Some(track.id));
    Ok(track)
}

/// The folder an "imported: <path>" outcome put its file in.
fn db_path_dir(outcome: &str) -> Option<PathBuf> {
    let path = outcome.strip_prefix("imported: ")?;
    Path::new(path).parent().map(Path::to_path_buf)
}

/// What the inbox and the plugins look at: the file's tags, and the track
/// as the library will have it.
fn describe(path: &Path, archive: Option<String>) -> (InboxFile, Track) {
    let t = read_track(0, path);
    let file = InboxFile {
        path: path.to_path_buf(),
        artist: t.artist.clone(),
        title: t.title.clone(),
        album: t.album.clone(),
        comment: read_comment(path),
        archive,
        duration: t.duration,
        isrc: t.info.isrc.clone(),
    };
    (file, t)
}
