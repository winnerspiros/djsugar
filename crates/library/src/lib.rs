//! DJ Sugar's own music library, kept in SQLite: every track with its tags, rating, colour and
//! comment, its hot cues and beat grid, the DJ's playlists and folders, and
//! a history of what was played.
//!
//! The database is SQLite (`library.sqlite` in the settings folder), opened
//! in WAL mode so the background scan and the UI can both use it. It is the
//! source of truth once a track is in it:
//!
//! - A folder scan adds new files, and re-reads a file's tags only when the
//!   file has changed since.
//! - Analysis results are stored once, so a track loads with its grid at once.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use djsugar_engine::beats::BeatGrid;
use djsugar_engine::import::{ImportedLibrary, ImportedList, ImportedListKind, StreamedTrack};
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, MEMORY_CUE};
use rusqlite::{params, Connection, OptionalExtension};

pub use rusqlite::Error;

pub mod checklist;
pub mod cleanup;
pub mod dupes;
pub mod fix_names;
pub mod health;
mod moves;
pub mod playlist_files;
pub mod query;
pub mod relocate;
pub mod sets;
pub mod stages;
pub mod tags;

pub use moves::{move_leftovers, Change, FileMove, MoveReport};

/// Library track ids start here, clear of the built-in sample tracks.
pub const ID_BASE: usize = 1_000_000;

/// The schema version this code writes (`PRAGMA user_version`).
/// 2 added smart lists: `playlists.query`, kind 3.
/// 3 added purchases: `tracks.store` and `store_url`, `wanted`, `inbox_log`.
/// 4 added saved loops: `cues.end_seconds`.
/// 5 added `tracks.key_source`.
/// 6 added the track info panel's fields: `tracks.remix`, `year`,
/// `track_number`, `user1`, `user2` and `gain`.
/// 7 added `tracks.disc_number`, `label`, `composer`, `remixer` and `isrc`,
/// `markers`, and `playlists.source` for lists brought in from another app.
/// 8 added `tracks.user_more`: tag fields from Tag 3 up.
/// 9 added `tracks.stage` (see [`stages`]).
/// 10 added `tracks.grid_locked` and `grid_unsure`; imported grids start
/// locked.
/// 12 takes off the wanted list the streamed tracks imports used to put
/// there: only the DJ puts tracks on it now.
const VERSION: i32 = 13;

/// The `source` of grids, keys and markers found by this app's analysis.
const ANALYSIS: &str = "analysis";

/// The smart lists a new library starts with: name and search.
const STARTER_SMART_LISTS: [(&str, &str); 4] = [
    ("Mixes with deck", "~deck"),
    ("Recently added", "added:<30d"),
    ("Top rated", "rating:>=4"),
    ("Never played", "played:0"),
];

/// Starter smart lists Collection Health took over: taken out of the tree
/// when the DJ left them as they came.
const RETIRED_SMART_LISTS: [(&str, &str); 2] = [
    ("Duplicates", "is:duplicate"),
    ("Missing files", "is:missing"),
];

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    artist TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    genre TEXT NOT NULL DEFAULT '',
    duration REAL NOT NULL DEFAULT 0,
    bpm REAL NOT NULL DEFAULT 0,
    key TEXT NOT NULL DEFAULT '',
    rating INTEGER NOT NULL DEFAULT 0,
    color INTEGER,
    comment TEXT NOT NULL DEFAULT '',
    grouping TEXT NOT NULL DEFAULT '',
    labels TEXT NOT NULL DEFAULT '',
    play_count INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER,
    -- Beat grid as BeatGrid::to_text, and where it came from:
    -- 'analysis', 'user', or another app's name for an imported grid.
    grid TEXT,
    grid_source TEXT,
    -- The file's modification time when its tags were last read.
    file_mtime INTEGER NOT NULL DEFAULT 0,
    -- 'scan', 'file' (dropped in), 'purchase' (from the inbox), or another
    -- app's name for imported tracks, whose tags a scan leaves alone.
    source TEXT NOT NULL DEFAULT 'scan',
    added INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS cues (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    seconds REAL NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    color INTEGER,
    -- Where a saved loop ends; NULL for a plain cue.
    end_seconds REAL,
    -- 'user', or another app's name for imported cues.
    source TEXT NOT NULL,
    PRIMARY KEY (track_id, number)
);
-- Lists and folders in one tree. kind: 0 list, 1 folder, 2 history session,
-- 3 smart list (its tracks are those its search finds, in `query`), 4 set
-- (a folder made for one gig).
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY,
    parent INTEGER REFERENCES playlists(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    created INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    played_at INTEGER,
    PRIMARY KEY (playlist_id, position)
);
CREATE INDEX IF NOT EXISTS playlist_tracks_track ON playlist_tracks(track_id);
-- Named sections, phrases and mix points. kind: 'section', 'phrase' or
-- 'automix'; source as for cues.
CREATE TABLE IF NOT EXISTS markers (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    seconds REAL NOT NULL,
    end_seconds REAL,
    name TEXT NOT NULL DEFAULT '',
    color INTEGER,
    source TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS markers_track ON markers(track_id);
-- Tracks the DJ went to buy (clicked Buy on a store result), until a file
-- that matches arrives in the purchase inbox.
CREATE TABLE IF NOT EXISTS wanted (
    id INTEGER PRIMARY KEY,
    store TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    page_url TEXT NOT NULL UNIQUE,
    added INTEGER NOT NULL DEFAULT 0,
    track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL
);
-- Every file the purchase inbox has dealt with, so none is taken twice.
-- outcome: 'imported', 'duplicate' or 'failed: <why>'.
CREATE TABLE IF NOT EXISTS inbox_log (
    path TEXT PRIMARY KEY,
    outcome TEXT NOT NULL,
    track_id INTEGER,
    at INTEGER NOT NULL
);
";

/// Columns added to `tracks` after version 1: name and definition.
const ADDED_COLUMNS: [(&str, &str); 18] = [
    ("store", "store TEXT NOT NULL DEFAULT ''"),
    ("store_url", "store_url TEXT"),
    // Where the key came from: 'analysis', 'tags', or another app's name;
    // NULL when not known (keys from before version 4).
    ("key_source", "key_source TEXT"),
    ("remix", "remix TEXT NOT NULL DEFAULT ''"),
    ("year", "year TEXT NOT NULL DEFAULT ''"),
    ("track_number", "track_number INTEGER NOT NULL DEFAULT 0"),
    // VirtualDJ's User 1 and User 2, from before tag groups: read once
    // into them (see `tags`), no longer written.
    ("user1", "user1 TEXT NOT NULL DEFAULT ''"),
    ("user2", "user2 TEXT NOT NULL DEFAULT ''"),
    // The DJ's gain for the track in dB; NULL for none.
    ("gain", "gain REAL"),
    // What other DJ apps keep beyond these, so nothing they hold is lost
    // on the way in (version 7).
    ("disc_number", "disc_number INTEGER NOT NULL DEFAULT 0"),
    ("label", "label TEXT NOT NULL DEFAULT ''"),
    ("composer", "composer TEXT NOT NULL DEFAULT ''"),
    ("remixer", "remixer TEXT NOT NULL DEFAULT ''"),
    ("isrc", "isrc TEXT NOT NULL DEFAULT ''"),
    // The tag fields after the first two, one per line (version 8); read
    // once into tag groups, as `user1` is.
    ("user_more", "user_more TEXT NOT NULL DEFAULT ''"),
    // Inbox, triage or collection, as `Stage::name`; empty for none (version 9).
    ("stage", "stage TEXT NOT NULL DEFAULT ''"),
    // The DJ locked the beat grid against edits and analysis (version 10).
    ("grid_locked", "grid_locked INTEGER NOT NULL DEFAULT 0"),
    // The analysed grid fits the beats poorly: worth a check (version 10).
    ("grid_unsure", "grid_unsure INTEGER NOT NULL DEFAULT 0"),
];

/// Tracks the DJ linked as going well together: `track` links to `linked`. Created on every open rather than by
/// a version bump, so it doesn't clash with other schema changes.
const LINKS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS track_links (
    track INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    linked INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    added INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track, linked)
);
CREATE INDEX IF NOT EXISTS track_links_linked ON track_links(linked);
";

/// Another app's copy of a song the library already has, found by artist,
/// title and duration: its lists point at `track_id` instead of adding the
/// song twice. Made again on each import of `source`. Created on every
/// open, as [`LINKS_SCHEMA`] is.
const ALIASES_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS track_aliases (
    path TEXT PRIMARY KEY,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    source TEXT NOT NULL
);
";

/// Files moved between the stage folders, and the stage changes that go
/// with them. A move waits (`done` NULL) while the DJ keeps file moves
/// off; `undone` is set when it was taken back. Changes made together
/// share a `batch`, which is undone as one. Created on every open, as
/// [`LINKS_SCHEMA`] is.
const MOVES_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS file_moves (
    id INTEGER PRIMARY KEY,
    batch INTEGER NOT NULL,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    from_path TEXT NOT NULL,
    to_path TEXT NOT NULL,
    from_stage TEXT NOT NULL,
    to_stage TEXT NOT NULL,
    queued INTEGER NOT NULL,
    done INTEGER,
    undone INTEGER
);
CREATE INDEX IF NOT EXISTS file_moves_track ON file_moves(track_id);
";

/// Which collection track a streaming service's track is, as the DJ set it
/// by hand: `track_id` NULL means it is none of them. Created on every
/// open, as [`LINKS_SCHEMA`] is.
const STREAM_MATCHES_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS stream_matches (
    service TEXT NOT NULL,
    key TEXT NOT NULL,
    track_id INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    PRIMARY KEY (service, key)
);
";

/// Streamed tracks in lists brought in from another app (TIDAL, Spotify…),
/// standing in their place until the collection has the song: `position`
/// shares the list's order with `playlist_tracks`. `stream_id` is the
/// service's id for it, `uri` as the app wrote it. Created on every open,
/// as [`LINKS_SCHEMA`] is.
const LIST_STREAMS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS list_streams (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    service TEXT NOT NULL,
    stream_id TEXT NOT NULL DEFAULT '',
    uri TEXT NOT NULL,
    artist TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    duration REAL NOT NULL DEFAULT 0,
    isrc TEXT NOT NULL DEFAULT '',
    played_at INTEGER,
    PRIMARY KEY (playlist_id, position)
);
CREATE INDEX IF NOT EXISTS list_streams_id ON list_streams(service, stream_id);
";

/// Pages the DJ took off the wanted list, so an import or a playlist kept
/// in sync with it doesn't put them back. Adding one by hand forgets it.
const WANTED_DISMISSED_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS wanted_dismissed (
    page_url TEXT PRIMARY KEY
);
";

/// Stores the tags a scan reads beyond the basic ones (`?1` is the row),
/// for tracks whose tags come from their files. A rating in the tags fills
/// in one the library doesn't have.
const TAG_EXTRAS: &str = "
UPDATE tracks SET remix = ?2, year = ?3, track_number = ?4, grouping = ?5, comment = ?6,
       rating = CASE WHEN ?7 > 0 THEN ?7 ELSE rating END
WHERE id = ?1 AND source IN ('scan', 'file', 'purchase')";

fn tag_extras(row: i64, t: &Track) -> impl rusqlite::Params + '_ {
    (
        row,
        t.info.remix.as_str(),
        t.info.year.as_str(),
        t.info.track_number as i64,
        t.info.grouping.as_str(),
        t.info.comment.as_str(),
        t.rating as i64,
    )
}

/// A list, folder or history session.
#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    pub id: i64,
    pub parent: Option<i64>,
    pub name: String,
    pub kind: PlaylistKind,
    /// A smart list's search (see [`query`]).
    pub query: Option<String>,
    /// When it was made, or for a history session when it started (Unix
    /// time).
    pub created: i64,
    /// The app it was brought in from, for lists an import keeps up to date;
    /// None for the DJ's own.
    pub source: Option<String>,
    /// The colour the DJ gave it (0xRRGGBB), drawn on its icon.
    pub color: Option<u32>,
    /// The icon the DJ picked for it by name (see [`LIST_ICONS`]); None for
    /// its kind's own.
    pub icon: Option<String>,
}

/// Icons a crate can wear in the sidebar, by the name the library keeps.
pub const LIST_ICONS: [&str; 8] = [
    "crate", "star", "heart", "flame", "bolt", "moon", "sun", "leaf",
];

/// A track the DJ went to buy and hasn't received yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    pub id: i64,
    pub store: String,
    pub artist: String,
    pub title: String,
    pub page_url: String,
    /// The recording's ISRC, when the service it came from knew it.
    pub isrc: String,
    /// Seconds; 0 when unknown.
    pub duration: f64,
    /// The store the DJ opened it on with Buy, while a file is awaited
    /// (see [`PENDING_DAYS`]); empty when not.
    pub pending_store: String,
}

/// How long a Wanted track opened with Buy waits for its file before it's
/// no longer counted as being bought.
pub const PENDING_DAYS: i64 = 7;

/// Lowercase letters and digits only, for comparing names loosely.
fn simplify(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistKind {
    List,
    Folder,
    /// The tracks played in one session, oldest first.
    History,
    /// A saved search.
    Smart,
    /// A folder made for one gig, holding its lists (see [`sets`]).
    Set,
    /// A list kept for sorting music (Yoga, Peak time): no order of its
    /// own, each track once (see [`sets`]).
    Crate,
}

impl PlaylistKind {
    fn from_i64(k: i64) -> Self {
        match k {
            1 => Self::Folder,
            2 => Self::History,
            3 => Self::Smart,
            4 => Self::Set,
            5 => Self::Crate,
            _ => Self::List,
        }
    }

    fn to_i64(self) -> i64 {
        match self {
            Self::List => 0,
            Self::Folder => 1,
            Self::History => 2,
            Self::Smart => 3,
            Self::Set => 4,
            Self::Crate => 5,
        }
    }
}

/// What a sync changed.
#[derive(Debug, Default, PartialEq)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    /// Lists, folders and history sessions brought in.
    pub lists: usize,
    /// List entries left out because the library doesn't have their file
    /// (streamed tracks, files that have gone).
    pub unmatched: usize,
    /// Tracks found to be another copy of a song the library has, whose
    /// lists now point at that one.
    pub matched: usize,
    /// Lists left out as none of their tracks are here (files that have
    /// all gone).
    pub left_out: usize,
    /// Streamed tracks standing in their lists until the collection has
    /// the song.
    pub streamed: usize,
    /// Streamed tracks the collection already has, which their lists now
    /// point at.
    pub filled: usize,
    /// Where the other app and the library disagree, and which side won.
    pub conflicts: Vec<Conflict>,
}

/// A field a track has here and in the other app, set differently.
#[derive(Clone, Debug, PartialEq)]
pub struct Conflict {
    pub path: String,
    /// "artist", "title", "album", "genre", "rating", "colour", "comment",
    /// "grouping"; or "grid" and "cues" for ones the DJ edited here, which
    /// an import never replaces.
    pub field: &'static str,
    pub ours: String,
    pub theirs: String,
    /// The library's value stays (else the other app's replaces it).
    pub kept_ours: bool,
}

/// An entry of a list: a collection track, or a streamed track standing in
/// for one (see [`Library::playlist_entries`]).
#[derive(Clone, Debug, PartialEq)]
pub enum ListEntry {
    Track(usize),
    Streamed(StreamedTrack),
}

/// What [`Library::remove_import`] took out.
#[derive(Debug, Default, PartialEq)]
pub struct RemovedImport {
    /// Lists and history sessions.
    pub lists: usize,
    /// Tracks only the other app had.
    pub tracks: usize,
    /// Tracks kept, as the DJ has them too.
    pub kept: usize,
}

pub struct Library {
    db: Connection,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The database id of a library track, or None for a track from elsewhere
/// (the built-in samples).
pub fn row_id(track_id: usize) -> Option<i64> {
    track_id.checked_sub(ID_BASE).map(|i| i as i64)
}

fn track_id(row: i64) -> usize {
    ID_BASE + row as usize
}

/// Where the library lives by default: `library.sqlite` in `dir`.
pub fn default_path(dir: &Path) -> PathBuf {
    dir.join("library.sqlite")
}

/// Writes a consistent copy of the library at `from` to `to` (which must
/// not exist yet), while other connections may be using it: WAL mode lets
/// this read a snapshot. The copy is a single file, without a WAL.
pub fn snapshot(from: &Path, to: &Path) -> Result<(), Error> {
    let db = Connection::open_with_flags(
        from,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.execute("VACUUM INTO ?1", [to.to_string_lossy()])?;
    Ok(())
}

impl Library {
    /// Opens (creating if needed) the library at `path`.
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::setup(Connection::open(path)?)
    }

    /// A library that lives only in memory, for tests.
    pub fn in_memory() -> Result<Self, Error> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(mut db: Connection) -> Result<Self, Error> {
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        // The app and the startup sync each have a connection. A deferred
        // transaction that reads and then writes fails at once with "database
        // is locked" when the other one wrote in between; an immediate one
        // takes the write lock first and waits its turn.
        db.set_transaction_behavior(rusqlite::TransactionBehavior::Immediate);
        // In-memory databases stay in "memory" journal mode; that is fine.
        let _: String = db.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        db.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
        let version: i32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < VERSION {
            db.execute_batch(SCHEMA)?;
            let has_query = db
                .prepare("SELECT 1 FROM pragma_table_info('playlists') WHERE name = 'query'")?
                .exists([])?;
            if !has_query {
                db.execute_batch("ALTER TABLE playlists ADD COLUMN query TEXT")?;
            }
            let has_loop_ends = db
                .prepare("SELECT 1 FROM pragma_table_info('cues') WHERE name = 'end_seconds'")?
                .exists([])?;
            if !has_loop_ends {
                db.execute_batch("ALTER TABLE cues ADD COLUMN end_seconds REAL")?;
            }
            // The app a list was brought in from; NULL for the DJ's own.
            let has_list_source = db
                .prepare("SELECT 1 FROM pragma_table_info('playlists') WHERE name = 'source'")?
                .exists([])?;
            if !has_list_source {
                db.execute_batch("ALTER TABLE playlists ADD COLUMN source TEXT")?;
            }
            for (name, definition) in ADDED_COLUMNS {
                let has = db
                    .prepare("SELECT 1 FROM pragma_table_info('tracks') WHERE name = ?1")?
                    .exists([name])?;
                if !has {
                    db.execute_batch(&format!("ALTER TABLE tracks ADD COLUMN {definition}"))?;
                }
            }
            if version < 10 {
                // Grids brought in from another app are kept as they came.
                db.execute_batch(
                    "UPDATE tracks SET grid_locked = 1
                     WHERE grid IS NOT NULL AND grid_source NOT IN ('analysis', 'user')",
                )?;
            }
            if (2..11).contains(&version) {
                for (name, query) in RETIRED_SMART_LISTS {
                    db.execute(
                        "DELETE FROM playlists
                         WHERE kind = 3 AND parent IS NULL AND name = ?1 AND query = ?2",
                        params![name, query],
                    )?;
                }
            }
            if version < 5 {
                // Keys brought in from another app came from there.
                db.execute_batch(
                    "UPDATE tracks SET key_source = source
                     WHERE key != '' AND key_source IS NULL
                       AND source NOT IN ('scan', 'file', 'purchase')",
                )?;
            }
            db.execute_batch(&format!("PRAGMA user_version = {VERSION}"))?;
        }
        // Tracks once hidden from the Inbox (a stage since retired) go back
        // to it; quarantine is the way to put a track aside now.
        db.execute_batch("UPDATE tracks SET stage = 'inbox' WHERE stage = 'dismissed'")?;
        // A list's colour and icon, which the DJ picks in the sidebar.
        for (name, definition) in [("color", "color INTEGER"), ("icon", "icon TEXT")] {
            let has = db
                .prepare("SELECT 1 FROM pragma_table_info('playlists') WHERE name = ?1")?
                .exists([name])?;
            if !has {
                db.execute_batch(&format!("ALTER TABLE playlists ADD COLUMN {definition}"))?;
            }
        }
        db.execute_batch(LINKS_SCHEMA)?;
        db.execute_batch(MOVES_SCHEMA)?;
        db.execute_batch(ALIASES_SCHEMA)?;
        db.execute_batch(STREAM_MATCHES_SCHEMA)?;
        db.execute_batch(LIST_STREAMS_SCHEMA)?;
        db.execute_batch(WANTED_DISMISSED_SCHEMA)?;
        // The import that put a wanted track there (its streamed tracks),
        // NULL for tracks the DJ went to buy.
        let has_wanted_source = db
            .prepare("SELECT 1 FROM pragma_table_info('wanted') WHERE name = 'source'")?
            .exists([])?;
        if !has_wanted_source {
            db.execute_batch("ALTER TABLE wanted ADD COLUMN source TEXT")?;
        }
        // What matches a wanted track to the file that arrives for it, and
        // the store it's being bought on (Buy).
        for (name, definition) in [
            ("isrc", "isrc TEXT NOT NULL DEFAULT ''"),
            ("duration", "duration REAL NOT NULL DEFAULT 0"),
            ("pending_store", "pending_store TEXT"),
            ("pending_since", "pending_since INTEGER"),
        ] {
            let has = db
                .prepare("SELECT 1 FROM pragma_table_info('wanted') WHERE name = ?1")?
                .exists([name])?;
            if !has {
                db.execute_batch(&format!("ALTER TABLE wanted ADD COLUMN {definition}"))?;
            }
        }
        if version < 13 {
            // Crates were lists at the top with no folder or source (other
            // than the Inbox list); now they're a kind of their own.
            db.execute(
                "UPDATE playlists SET kind = 5
                  WHERE kind = 0 AND parent IS NULL AND source IS NULL
                    AND id IS NOT (SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'inbox_list')",
                [],
            )?;
        }
        if version < 12 {
            // Imports put every streamed track of every list (history
            // sessions too) on the wanted list; the DJ picks them now.
            db.execute(
                "DELETE FROM wanted WHERE source IS NOT NULL AND track_id IS NULL",
                [],
            )?;
        }
        db.execute_batch(health::HEALTH_SCHEMA)?;
        // Bitrates came after covers: files looked at before get read again.
        let has_bitrate = db
            .prepare("SELECT 1 FROM pragma_table_info('covers') WHERE name = 'bitrate'")?
            .exists([])?;
        if !has_bitrate {
            db.execute_batch("ALTER TABLE covers ADD COLUMN bitrate INTEGER")?;
        }
        db.execute_batch(cleanup::CHANGES_SCHEMA)?;
        db.execute_batch(checklist::CHECKLIST_SCHEMA)?;
        tags::setup(&db)?;
        let lib = Library { db };
        if version < 2 {
            lib.add_starter_smart_lists()?;
        }
        Ok(lib)
    }

    /// A few useful smart lists, so the DJ sees what they can do. They sit
    /// at the top level, which the browser shows under "Smart Lists".
    fn add_starter_smart_lists(&self) -> Result<(), Error> {
        for (name, query) in STARTER_SMART_LISTS {
            self.create_smart_list(name, None, query)?;
        }
        Ok(())
    }

    fn meta(&self, key: &str) -> Option<String> {
        self.db
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<(), Error> {
        self.db.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Every track whose file `exists`, with its cues and grid, in id order.
    pub fn tracks(&self, exists: impl Fn(&Path) -> bool) -> Result<Vec<Track>, Error> {
        let mut all = self.all_tracks(exists)?;
        all.retain(|t| !t.info.missing);
        Ok(all)
    }

    /// Every track, with those whose file doesn't `exist` marked missing.
    pub fn all_tracks(&self, exists: impl Fn(&Path) -> bool) -> Result<Vec<Track>, Error> {
        let mut cues: HashMap<i64, Vec<Hotcue>> = HashMap::new();
        {
            let mut q = self.db.prepare(
                "SELECT track_id, number, seconds, name, end_seconds, color FROM cues ORDER BY number",
            )?;
            let rows = q.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    Hotcue {
                        number: r.get::<_, i64>(1)? as usize,
                        seconds: r.get(2)?,
                        name: r.get(3)?,
                        end: r.get(4)?,
                        color: r.get::<_, Option<i64>>(5)?.map(|c| c as u32),
                    },
                ))
            })?;
            for row in rows {
                let (id, cue) = row?;
                cues.entry(id).or_default().push(cue);
            }
        }
        let mut markers: HashMap<i64, Vec<Marker>> = HashMap::new();
        {
            let mut q = self.db.prepare(
                "SELECT track_id, kind, seconds, end_seconds, name, color, source
                 FROM markers ORDER BY track_id, seconds",
            )?;
            let rows = q.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                    r.get::<_, Option<f64>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (id, kind, seconds, end, name, color, source) = row?;
                let Some(kind) = MarkerKind::from_name(&kind) else {
                    continue;
                };
                markers.entry(id).or_default().push(Marker {
                    kind,
                    seconds,
                    end,
                    name,
                    color: color.map(|c| c as u32),
                    detected: source == ANALYSIS,
                });
            }
        }
        let mut tags = tags::track_tags(&self.db)?;
        let mut q = self.db.prepare(
            "SELECT id, path, artist, title, album, genre, duration, bpm, key, rating, color,
                    comment, grouping, labels, play_count, grid, added, key_source,
                    remix, year, track_number, user1, user2, gain, last_played,
                    disc_number, label, composer, remixer, isrc, user_more, stage, grid_source,
                    grid_locked, grid_unsure
             FROM tracks WHERE id NOT IN (SELECT track_id FROM removed) ORDER BY id",
        )?;
        let rows = q.query_map([], |r| {
            let id: i64 = r.get(0)?;
            let path: String = r.get(1)?;
            let grid: Option<String> = r.get(15)?;
            let key_source: Option<String> = r.get(17)?;
            let grid_source: Option<String> = r.get(32)?;
            Ok((
                id,
                PathBuf::from(path),
                grid,
                Track {
                    id: track_id(id),
                    artist: r.get(2)?,
                    title: r.get(3)?,
                    album: r.get(4)?,
                    genre: r.get(5)?,
                    duration: r.get(6)?,
                    bpm: r.get(7)?,
                    key: r.get(8)?,
                    rating: r.get::<_, i64>(9)?.clamp(0, 5) as u8,
                    first_beat: 0.0,
                    grid: BeatGrid::default(),
                    source: Source::File(PathBuf::new()),
                    waveform: Default::default(),
                    audio: None,
                    cover: None,
                    info: LibraryInfo {
                        color: r.get::<_, Option<i64>>(10)?.map(|c| c as u32),
                        comment: r.get(11)?,
                        grouping: r.get(12)?,
                        tags: Vec::new(),
                        imported_tags: Vec::new(),
                        play_count: r.get::<_, i64>(14)?.max(0) as u32,
                        hotcues: Vec::new(),
                        fixed_grid: false,
                        grid_analysed: grid_source.as_deref() == Some(ANALYSIS),
                        grid_from: grid_source.unwrap_or_default(),
                        grid_locked: r.get(33)?,
                        grid_unsure: r.get(34)?,
                        added: r.get(16)?,
                        missing: false,
                        key_detected: key_source.map(|s| s == "analysis"),
                        remix: r.get(18)?,
                        year: r.get(19)?,
                        track_number: r.get::<_, i64>(20)?.max(0) as u32,
                        gain_db: r.get::<_, Option<f64>>(23)?.map(|g| g as f32),
                        last_played: r.get(24)?,
                        disc_number: r.get::<_, i64>(25)?.max(0) as u32,
                        label: r.get(26)?,
                        composer: r.get(27)?,
                        remixer: r.get(28)?,
                        isrc: r.get(29)?,
                        markers: Vec::new(),
                        stage: r.get(31)?,
                    },
                },
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, path, grid, mut t) = row?;
            t.info.missing = !exists(&path);
            t.source = Source::File(path);
            t.info.hotcues = cues.remove(&id).unwrap_or_default();
            t.info.markers = markers.remove(&id).unwrap_or_default();
            t.info.tags = tags.remove(&id).unwrap_or_default();
            if let Some(g) = grid.map(|g| BeatGrid::from_text(&g)) {
                if !g.is_empty() {
                    t.set_grid(g);
                    t.info.fixed_grid = true;
                }
            }
            out.push(t);
        }
        Ok(out)
    }

    /// Brings the library up to date with a folder scan. Returns what
    /// changed.
    pub fn sync(&mut self, scanned: &[Track]) -> Result<SyncReport, Error> {
        let mut report = SyncReport::default();
        let tx = self.db.transaction()?;
        {
            let mut find = tx.prepare("SELECT id, file_mtime FROM tracks WHERE path = ?1")?;
            let mut insert = tx.prepare(
                "INSERT INTO tracks (path, artist, title, album, genre, duration, bpm, key,
                                     rating, file_mtime, source, added, key_source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                         CASE WHEN ?8 != '' THEN 'tags' END)",
            )?;
            let mut retag = tx.prepare(
                "UPDATE tracks SET artist = ?2, title = ?3, album = ?4, genre = ?5,
                        duration = ?6, file_mtime = ?7
                 WHERE id = ?1 AND source IN ('scan', 'file', 'purchase')",
            )?;
            let mut extras = tx.prepare(TAG_EXTRAS)?;
            for t in scanned {
                let Source::File(path) = &t.source else {
                    continue;
                };
                let p = path_text(path);
                let stamp = mtime(path);
                let found: Option<(i64, i64)> = find
                    .query_row([&p], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?;
                match found {
                    None => {
                        insert.execute(params![
                            p,
                            t.artist,
                            t.title,
                            t.album,
                            t.genre,
                            t.duration,
                            t.bpm,
                            t.key,
                            t.rating as i64,
                            stamp,
                            "scan",
                            now()
                        ])?;
                        extras.execute(tag_extras(tx.last_insert_rowid(), t))?;
                        set_details(&tx, tx.last_insert_rowid(), &t.info, true)?;
                        report.added += 1;
                    }
                    Some((id, old)) if old != stamp => {
                        if retag.execute(params![
                            id, t.artist, t.title, t.album, t.genre, t.duration, stamp
                        ])? > 0
                        {
                            extras.execute(tag_extras(id, t))?;
                            set_details(&tx, id, &t.info, true)?;
                            report.updated += 1;
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }

    /// Brings in another app's library (`source` names the app, `title` is
    /// what the DJ calls it), unless `stamp` is the same as last time: the
    /// importer makes it from its files' modification times and the DJ's
    /// choices, so it changes whenever there is something new. Its tags,
    /// ratings and colours win, as the other app is where they were set; its
    /// grids and cues fill in what the DJ hasn't edited here. Its lists are
    /// made again in a folder named `title`, replacing the ones brought in
    /// last time. `read` is only called when something has changed. Returns
    /// what changed, or `None` when it was left alone.
    pub fn import(
        &mut self,
        source: &str,
        title: &str,
        stamp: &str,
        read: impl FnOnce() -> Result<ImportedLibrary, String>,
    ) -> Result<Option<SyncReport>, String> {
        self.import_with(source, title, stamp, true, false, read)
    }

    /// [`Library::import`], where `theirs_win` says whether the other app's
    /// tags, ratings and colours replace what the library has; without it
    /// they only fill in what is empty here. With `want_streamed`, streamed
    /// tracks in its playlists (not history or smart lists) that the
    /// collection hasn't got go on the wanted list, as the DJ's own; ones
    /// the DJ took off stay off.
    pub fn import_with(
        &mut self,
        source: &str,
        title: &str,
        stamp: &str,
        theirs_win: bool,
        want_streamed: bool,
        read: impl FnOnce() -> Result<ImportedLibrary, String>,
    ) -> Result<Option<SyncReport>, String> {
        let key = format!("import_stamp:{source}");
        if self.meta(&key).as_deref() == Some(stamp) {
            return Ok(None);
        }
        let report = self.merge(source, title, theirs_win, want_streamed, &read()?, true)?;
        self.set_meta(&key, stamp).map_err(|e| e.to_string())?;
        Ok(Some(report))
    }

    /// What [`Library::import_with`] would do with `imported`, conflicts
    /// and all, without changing anything: the import is made and then
    /// rolled back.
    pub fn preview_import(
        &mut self,
        source: &str,
        title: &str,
        theirs_win: bool,
        want_streamed: bool,
        imported: &ImportedLibrary,
    ) -> Result<SyncReport, String> {
        self.merge(source, title, theirs_win, want_streamed, imported, false)
    }

    /// Brings `imported` in, in one transaction, kept only when `commit`.
    fn merge(
        &mut self,
        source: &str,
        title: &str,
        theirs_win: bool,
        want_streamed: bool,
        imported: &ImportedLibrary,
        commit: bool,
    ) -> Result<SyncReport, String> {
        let mut report = SyncReport::default();
        let tx = self.db.transaction().map_err(|e| e.to_string())?;
        import_tracks(&tx, source, &imported.tracks, theirs_win, &mut report)
            .map_err(|e| e.to_string())?;
        import_lists(
            &tx,
            source,
            title,
            &imported.lists,
            want_streamed,
            &mut report,
        )
        .map_err(|e| e.to_string())?;
        if commit {
            tx.commit().map_err(|e| e.to_string())?;
        }
        Ok(report)
    }

    /// Takes out what another app's import brought in: its lists, folders and
    /// history, its cues, grids and markers, and the tracks only it had.
    /// Tracks the DJ keeps (in their own lists, or for which `keep` says
    /// yes, such as files in the music folder) stay, and get their tags read
    /// from the file again at the next scan. Turning the import on again
    /// brings everything back in.
    pub fn remove_import(
        &mut self,
        source: &str,
        keep: impl Fn(&Path) -> bool,
    ) -> Result<RemovedImport, Error> {
        let tx = self.db.transaction()?;
        sets::rescue_own_lists(&tx, source)?;
        let mut out = RemovedImport {
            lists: tx.execute(
                "DELETE FROM playlists WHERE source = ?1 AND kind != 1",
                [source],
            )?,
            ..Default::default()
        };
        tx.execute("DELETE FROM playlists WHERE source = ?1", [source])?;
        tx.execute("DELETE FROM markers WHERE source = ?1", [source])?;
        tx.execute("DELETE FROM cues WHERE source = ?1", [source])?;
        tx.execute("DELETE FROM track_aliases WHERE source = ?1", [source])?;
        tx.execute(
            "DELETE FROM wanted WHERE source = ?1 AND track_id IS NULL",
            [source],
        )?;
        tx.execute(
            "UPDATE tracks SET grid = NULL, grid_source = NULL WHERE grid_source = ?1",
            [source],
        )?;
        let tracks: Vec<(i64, String, bool)> = tx
            .prepare(
                "SELECT id, path, EXISTS (SELECT 1 FROM playlist_tracks p WHERE p.track_id = tracks.id)
                 FROM tracks WHERE source = ?1",
            )?
            .query_map([source], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (id, path, listed) in tracks {
            if listed || keep(Path::new(&path)) {
                tx.execute(
                    "UPDATE tracks SET source = 'scan', file_mtime = 0,
                            key_source = CASE WHEN key_source = ?2 THEN NULL ELSE key_source END
                     WHERE id = ?1",
                    params![id, source],
                )?;
                out.kept += 1;
            } else {
                tx.execute("DELETE FROM tracks WHERE id = ?1", [id])?;
                out.tracks += 1;
            }
        }
        tx.execute(
            "DELETE FROM meta WHERE key = ?1",
            [format!("import_stamp:{source}")],
        )?;
        tx.commit()?;
        Ok(out)
    }

    /// Adds one file (dropped in from outside the collection), returning the
    /// library's copy of it.
    pub fn add_file(&mut self, track: &Track) -> Result<Track, Error> {
        Ok(self.add_files(std::slice::from_ref(track))?.remove(0))
    }

    /// Adds files in one go (a folder from the Files source), returning the
    /// library's copies in the same order. Files it has already keep their
    /// rows.
    pub fn add_files(&mut self, tracks: &[Track]) -> Result<Vec<Track>, Error> {
        let tx = self.db.transaction()?;
        let mut out = Vec::with_capacity(tracks.len());
        {
            let mut insert = tx.prepare(
                "INSERT INTO tracks (path, artist, title, album, genre, duration, bpm, key,
                                     rating, file_mtime, source, added, key_source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'file', ?11,
                         CASE WHEN ?8 != '' THEN 'tags' END)
                 ON CONFLICT(path) DO NOTHING",
            )?;
            let mut find = tx.prepare("SELECT id FROM tracks WHERE path = ?1")?;
            let mut extras = tx.prepare(TAG_EXTRAS)?;
            for track in tracks {
                let Source::File(path) = &track.source else {
                    out.push(track.clone());
                    continue;
                };
                let p = path_text(path);
                let added = insert.execute(params![
                    p,
                    track.artist,
                    track.title,
                    track.album,
                    track.genre,
                    track.duration,
                    track.bpm,
                    track.key,
                    track.rating as i64,
                    mtime(path),
                    now()
                ])?;
                let id: i64 = find.query_row([&p], |r| r.get(0))?;
                // Added again by hand: a track taken out comes back.
                tx.execute("DELETE FROM removed WHERE track_id = ?1", [id])?;
                if added > 0 {
                    extras.execute(tag_extras(id, track))?;
                    set_details(&tx, id, &track.info, true)?;
                }
                out.push(Track {
                    id: track_id(id),
                    ..track.clone()
                });
            }
        }
        tx.commit()?;
        Ok(out)
    }

    /// Stores what analysing a track found, unless its grid came from
    /// elsewhere (an import, or the DJ's own edits). The key is marked as
    /// found by analysis when [`LibraryInfo::key_detected`] says so.
    pub fn save_analysis(&self, id: usize, track: &Track) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let grid = track.beat_grid();
        let key_source = track
            .info
            .key_detected
            .map(|d| if d { "analysis" } else { "tags" });
        self.db.execute(
            "UPDATE tracks SET key = ?3, duration = ?4,
                    key_source = CASE ?6 WHEN 'analysis' THEN 'analysis'
                                         WHEN 'tags' THEN COALESCE(key_source, 'tags')
                                         ELSE key_source END,
                    bpm = CASE WHEN grid_source IS NULL OR grid_source = 'analysis'
                               THEN ?2 ELSE bpm END,
                    grid = CASE WHEN grid_source IS NULL OR grid_source = 'analysis'
                                THEN ?5 ELSE grid END,
                    grid_unsure = CASE WHEN grid_source IS NULL OR grid_source = 'analysis'
                                       THEN ?7 ELSE grid_unsure END,
                    grid_source = COALESCE(grid_source, 'analysis')
             WHERE id = ?1",
            params![
                row,
                track.bpm,
                track.key,
                track.duration,
                (!grid.is_empty()).then(|| grid.to_text()),
                key_source,
                track.info.grid_unsure
            ],
        )?;
        Ok(())
    }

    /// Replaces the phrases analysis found for a track (markers of kind
    /// phrase from [`ANALYSIS`]); markers from imports are left alone.
    pub fn save_phrases(&self, id: usize, phrases: &[Marker]) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let tx = self.db.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM markers WHERE track_id = ?1 AND kind = 'phrase' AND source = ?2",
            params![row, ANALYSIS],
        )?;
        {
            let mut add = tx.prepare(
                "INSERT INTO markers (track_id, kind, seconds, end_seconds, name, color, source)
                 VALUES (?1, 'phrase', ?2, ?3, ?4, ?5, ?6)",
            )?;
            for m in phrases {
                add.execute(params![
                    row,
                    m.seconds,
                    m.end,
                    m.name,
                    m.color.map(|c| c as i64),
                    ANALYSIS
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Locks or unlocks a track's beat grid against edits and analysis.
    pub fn set_grid_locked(&self, id: usize, locked: bool) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            "UPDATE tracks SET grid_locked = ?2 WHERE id = ?1",
            params![row, locked],
        )?;
        Ok(())
    }

    /// Forgets a track's beat grid, edited or imported, so the next analysis
    /// (see [`Library::save_analysis`]) gives it one.
    pub fn clear_grid(&self, id: usize) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            "UPDATE tracks SET grid = NULL, grid_source = NULL, grid_unsure = 0 WHERE id = ?1",
            [row],
        )?;
        Ok(())
    }

    /// Stores a beat grid the DJ edited.
    pub fn save_grid(&self, id: usize, grid: &BeatGrid, bpm: f64) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            "UPDATE tracks SET grid = ?2, bpm = ?3, grid_source = 'user' WHERE id = ?1",
            params![row, grid.to_text(), bpm],
        )?;
        Ok(())
    }

    /// Replaces a track's hot cues and saved loops with the DJ's.
    pub fn save_cues(&mut self, id: usize, cues: &[Hotcue]) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM cues WHERE track_id = ?1", [row])?;
        for c in cues {
            tx.execute(
                "INSERT INTO cues (track_id, number, seconds, name, end_seconds, color, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'user')",
                params![
                    row,
                    c.number as i64,
                    c.seconds,
                    c.name,
                    c.end,
                    c.color.map(|c| c as i64)
                ],
            )?;
        }
        tx.commit()
    }

    pub fn set_rating(&self, id: usize, rating: u8) -> Result<(), Error> {
        self.update(id, "rating", rating.min(5) as i64)
    }

    /// Sets a track's colour (0xRRGGBB), or clears it.
    pub fn set_color(&self, id: usize, color: Option<u32>) -> Result<(), Error> {
        self.update(id, "color", color.map(|c| c as i64))
    }

    pub fn set_comment(&self, id: usize, comment: &str) -> Result<(), Error> {
        self.update(id, "comment", comment)
    }

    /// Saves what the DJ edited in the track info panel: the tags, rating,
    /// the User 1 and 2 tags and the gain. A key typed in counts as the
    /// DJ's own, so analysis leaves it alone.
    pub fn save_info(&self, id: usize, t: &Track) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            "UPDATE tracks SET title = ?2, artist = ?3, remix = ?4, album = ?5, year = ?6,
                    genre = ?7, track_number = ?8, grouping = ?9, key = ?10,
                    key_source = CASE WHEN key = ?10 THEN key_source ELSE 'user' END,
                    rating = ?11, comment = ?12, gain = ?13, file_mtime = ?14
             WHERE id = ?1",
            params![
                row,
                t.title,
                t.artist,
                t.info.remix,
                t.album,
                t.info.year,
                t.genre,
                t.info.track_number as i64,
                t.info.grouping,
                t.key,
                t.rating.min(5) as i64,
                t.info.comment,
                t.info.gain_db.map(f64::from),
                match &t.source {
                    // The tags were just written, so a scan needn't read them again.
                    Source::File(path) => mtime(path),
                    Source::Synth { .. } => 0,
                },
            ],
        )?;
        tags::write_track_tags(&self.db, row, &t.info.tags, None)
    }

    fn update(&self, id: usize, column: &str, value: impl rusqlite::ToSql) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            &format!("UPDATE tracks SET {column} = ?2 WHERE id = ?1"),
            params![row, value],
        )?;
        Ok(())
    }

    /// When each list was last touched: the latest time a track was added
    /// to it, or when it was made if nothing was. For the track menu's
    /// "Add to playlist", most recent first.
    pub fn playlists_touched(&self) -> Result<HashMap<i64, i64>, Error> {
        let mut q = self.db.prepare(
            "SELECT p.id, MAX(COALESCE(p.created, 0),
                              COALESCE((SELECT MAX(played_at) FROM playlist_tracks WHERE playlist_id = p.id), 0))
               FROM playlists p",
        )?;
        let out = q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(out)
    }

    /// The lists a track is in (not history sessions' own: those are
    /// lists too, so the caller leaves out what it doesn't show).
    pub fn playlists_with(&self, track: usize) -> Result<Vec<i64>, Error> {
        let Some(row) = row_id(track) else {
            return Ok(Vec::new());
        };
        let mut q = self
            .db
            .prepare("SELECT DISTINCT playlist_id FROM playlist_tracks WHERE track_id = ?1")?;
        let out = q
            .query_map([row], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(out)
    }

    /// The tree of lists, folders and history sessions, parents before
    /// children, siblings in order.
    pub fn playlists(&self) -> Result<Vec<Playlist>, Error> {
        let mut q = self.db.prepare(
            "SELECT id, parent, name, kind, query, created, source, color, icon
                 FROM playlists ORDER BY position, id",
        )?;
        let all: Vec<Playlist> = q
            .query_map([], |r| {
                Ok(Playlist {
                    id: r.get(0)?,
                    parent: r.get(1)?,
                    name: r.get(2)?,
                    kind: PlaylistKind::from_i64(r.get(3)?),
                    query: r.get(4)?,
                    created: r.get(5)?,
                    source: r.get(6)?,
                    color: r.get::<_, Option<i64>>(7)?.map(|c| c as u32 & 0xff_ffff),
                    icon: r.get(8)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        // Depth-first, so each folder is followed by what is in it.
        fn walk(all: &[Playlist], parent: Option<i64>, out: &mut Vec<Playlist>) {
            for p in all.iter().filter(|p| p.parent == parent) {
                out.push(p.clone());
                walk(all, Some(p.id), out);
            }
        }
        let mut out = Vec::with_capacity(all.len());
        walk(&all, None, &mut out);
        Ok(out)
    }

    /// Makes a list or folder at the end of `parent`.
    pub fn create_playlist(
        &self,
        name: &str,
        parent: Option<i64>,
        kind: PlaylistKind,
    ) -> Result<i64, Error> {
        let position: i64 = self.db.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists WHERE parent IS ?1",
            [parent],
            |r| r.get(0),
        )?;
        self.db.execute(
            "INSERT INTO playlists (parent, name, kind, position, created)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![parent, name, kind.to_i64(), position, now()],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Makes a smart list at the end of `parent`.
    pub fn create_smart_list(
        &self,
        name: &str,
        parent: Option<i64>,
        query: &str,
    ) -> Result<i64, Error> {
        let id = self.create_playlist(name, parent, PlaylistKind::Smart)?;
        self.set_query(id, query)?;
        Ok(id)
    }

    /// Changes a smart list's search.
    pub fn set_query(&self, id: i64, query: &str) -> Result<(), Error> {
        self.db.execute(
            "UPDATE playlists SET query = ?2 WHERE id = ?1",
            params![id, query.trim()],
        )?;
        Ok(())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<(), Error> {
        self.db.execute(
            "UPDATE playlists SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        Ok(())
    }

    /// Gives a list a colour (0xRRGGBB, or None for none).
    pub fn set_playlist_color(&self, id: i64, color: Option<u32>) -> Result<(), Error> {
        self.db.execute(
            "UPDATE playlists SET color = ?2 WHERE id = ?1",
            params![id, color.map(|c| i64::from(c & 0xff_ffff))],
        )?;
        Ok(())
    }

    /// Gives a list one of [`LIST_ICONS`] by name, or None for its kind's own.
    pub fn set_playlist_icon(&self, id: i64, icon: Option<&str>) -> Result<(), Error> {
        self.db.execute(
            "UPDATE playlists SET icon = ?2 WHERE id = ?1",
            params![id, icon],
        )?;
        Ok(())
    }

    /// Deletes a list, or a folder with everything in it.
    pub fn delete_playlist(&self, id: i64) -> Result<(), Error> {
        self.db
            .execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        Ok(())
    }

    /// The tracks in a list, in order (as track ids).
    pub fn playlist_tracks(&self, id: i64) -> Result<Vec<usize>, Error> {
        let mut q = self.db.prepare(
            "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let ids = q
            .query_map([id], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(track_id))
            .collect();
        ids
    }

    /// How many entries each list holds, streamed ones included, by list
    /// id; empty lists are left out.
    pub fn list_sizes(&self) -> Result<HashMap<i64, usize>, Error> {
        let mut q = self.db.prepare(
            "SELECT playlist_id, COUNT(*) FROM (
                 SELECT playlist_id FROM playlist_tracks
                 UNION ALL SELECT playlist_id FROM list_streams)
             GROUP BY playlist_id",
        )?;
        let sizes = q
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as usize))
            })?
            .collect();
        sizes
    }

    /// A list in order, streamed tracks that stand in it included.
    pub fn playlist_entries(&self, id: i64) -> Result<Vec<ListEntry>, Error> {
        let mut q = self.db.prepare(
            "SELECT position, track_id, NULL, NULL, NULL, NULL, NULL, NULL, NULL
               FROM playlist_tracks WHERE playlist_id = ?1
             UNION ALL
             SELECT position, NULL, service, stream_id, uri, artist, title, duration, isrc
               FROM list_streams WHERE playlist_id = ?1
             ORDER BY 1",
        )?;
        let rows = q.query_map([id], |r| {
            Ok(match r.get::<_, Option<i64>>(1)? {
                Some(track) => ListEntry::Track(track_id(track)),
                None => ListEntry::Streamed(StreamedTrack {
                    service: r.get(2)?,
                    id: r.get(3)?,
                    uri: r.get(4)?,
                    artist: r.get(5)?,
                    title: r.get(6)?,
                    duration: r.get(7)?,
                    isrc: r.get(8)?,
                }),
            })
        })?;
        rows.collect()
    }

    /// When each entry of a list was played (a history session) or added
    /// to it, in list order with its streamed tracks, as
    /// [`Library::playlist_entries`] lists them: Unix seconds, or None when
    /// not known.
    pub fn playlist_times(&self, id: i64) -> Result<Vec<Option<i64>>, Error> {
        let mut q = self.db.prepare(
            "SELECT position, played_at FROM playlist_tracks WHERE playlist_id = ?1
             UNION ALL
             SELECT position, played_at FROM list_streams WHERE playlist_id = ?1
             ORDER BY 1",
        )?;
        let times = q.query_map([id], |r| r.get(1))?.collect();
        times
    }

    /// Whether any list has streamed tracks standing in it.
    pub fn has_streamed_entries(&self) -> Result<bool, Error> {
        self.db
            .prepare("SELECT 1 FROM list_streams LIMIT 1")?
            .exists([])
    }

    /// The ids of the lists' streamed tracks on `service` whose ISRC isn't
    /// known yet, to ask the service.
    pub fn streamed_without_isrc(&self, service: &str) -> Result<Vec<String>, Error> {
        let mut q = self.db.prepare(
            "SELECT DISTINCT stream_id FROM list_streams
             WHERE service = ?1 AND isrc = '' AND stream_id != ''",
        )?;
        let ids = q.query_map([service], |r| r.get(0))?.collect();
        ids
    }

    /// Fills in what the lists' streamed tracks with this id don't say yet,
    /// from what their service told: its ISRC, artist, title and length.
    pub fn describe_streamed(&self, found: &StreamedTrack) -> Result<(), Error> {
        self.db.execute(
            "UPDATE list_streams SET
                 isrc = CASE WHEN isrc = '' THEN ?3 ELSE isrc END,
                 artist = CASE WHEN artist = '' THEN ?4 ELSE artist END,
                 title = CASE WHEN title = '' THEN ?5 ELSE title END,
                 duration = CASE WHEN duration <= 0 THEN ?6 ELSE duration END
             WHERE service = ?1 AND stream_id = ?2",
            params![
                found.service,
                found.id,
                found.isrc,
                found.artist,
                found.title,
                found.duration
            ],
        )?;
        Ok(())
    }

    /// Appends tracks to a list. Tracks not in the library are skipped.
    pub fn add_to_playlist(&mut self, id: i64, tracks: &[usize]) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        let start: i64 = tx.query_row(
            "SELECT MAX(COALESCE((SELECT MAX(position) FROM playlist_tracks WHERE playlist_id = ?1), -1),
                        COALESCE((SELECT MAX(position) FROM list_streams WHERE playlist_id = ?1), -1)) + 1",
            [id],
            |r| r.get(0),
        )?;
        // A crate holds each track once.
        let is_crate: bool = tx
            .prepare("SELECT 1 FROM playlists WHERE id = ?1 AND kind = 5")?
            .exists([id])?;
        let mut rows: Vec<i64> = tracks.iter().filter_map(|&t| row_id(t)).collect();
        if is_crate {
            let mut seen: HashSet<i64> = tx
                .prepare("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1")?
                .query_map([id], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            rows.retain(|r| seen.insert(*r));
        }
        for (position, row) in (start..).zip(rows) {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, position, track_id, played_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, position, row, now()],
            )?;
        }
        tx.commit()
    }

    /// Removes the track at `index` (0-based, in list order) from a list.
    pub fn remove_from_playlist(&mut self, id: i64, index: usize) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        let position: Option<i64> = tx
            .query_row(
                "SELECT position FROM playlist_tracks WHERE playlist_id = ?1
                 ORDER BY position LIMIT 1 OFFSET ?2",
                params![id, index as i64],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(p) = position {
            tx.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
                params![id, p],
            )?;
        }
        tx.commit()
    }

    /// The folders the last library scan looked through; None before the
    /// first scan that noted them.
    pub fn scanned_folders(&self) -> Option<Vec<PathBuf>> {
        self.meta("scanned_folders").map(|v| {
            v.lines()
                .filter(|l| !l.is_empty())
                .map(PathBuf::from)
                .collect()
        })
    }

    pub fn set_scanned_folders(&self, dirs: &[PathBuf]) -> Result<(), Error> {
        let v: Vec<String> = dirs
            .iter()
            .map(|d| d.to_string_lossy().into_owned())
            .collect();
        self.set_meta("scanned_folders", &v.join("\n"))
    }

    /// Takes out tracks a scan found under `dir`, a folder no longer
    /// scanned, unless the DJ has done something with them: put them in a
    /// list or a stage, rated or played them, or set cues. Tracks under any
    /// of `still` (folders still scanned, such as a music folder inside
    /// `~/Music`) stay too. Returns how many went. The files stay.
    pub fn forget_scanned_under(&mut self, dir: &Path, still: &[PathBuf]) -> Result<usize, Error> {
        let tx = self.db.transaction()?;
        let found: Vec<(i64, String)> = tx
            .prepare(
                "SELECT id, path FROM tracks
                  WHERE source = 'scan' AND stage = '' AND rating = 0 AND play_count = 0
                    AND NOT EXISTS (SELECT 1 FROM playlist_tracks p WHERE p.track_id = tracks.id)
                    AND NOT EXISTS (SELECT 1 FROM cues c WHERE c.track_id = tracks.id
                                      AND c.source = 'user')",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut gone = 0;
        for (id, path) in found {
            let path = Path::new(&path);
            if path.starts_with(dir) && !still.iter().any(|d| path.starts_with(d)) {
                gone += tx.execute("DELETE FROM tracks WHERE id = ?1", [id])?;
            }
        }
        tx.commit()?;
        Ok(gone)
    }

    /// Starts a history session named `name`; tracks played are added to it
    /// with [`Library::record_play`].
    pub fn start_session(&self, name: &str) -> Result<i64, Error> {
        let folder = match self
            .db
            .query_row(
                "SELECT id FROM playlists WHERE parent IS NULL AND kind = 2 AND name = 'History'",
                [],
                |r| r.get(0),
            )
            .optional()?
        {
            Some(id) => id,
            None => {
                // The history folder lives at the top and holds the sessions.
                self.db.execute(
                    "INSERT INTO playlists (parent, name, kind, position, created)
                     VALUES (NULL, 'History', 2, -1, ?1)",
                    [now()],
                )?;
                self.db.last_insert_rowid()
            }
        };
        self.create_playlist(name, Some(folder), PlaylistKind::History)
    }

    /// Records that a track was played: one more play, when, and a line in
    /// the session's history.
    pub fn record_play(&mut self, session: Option<i64>, id: usize) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        self.db.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = ?2 WHERE id = ?1",
            params![row, now()],
        )?;
        if let Some(s) = session {
            self.add_to_playlist(s, &[id])?;
        }
        Ok(())
    }

    /// Links `linked` to `track`, as one that goes well with it.
    pub fn link(&self, track: usize, linked: usize) -> Result<(), Error> {
        let (Some(a), Some(b)) = (row_id(track), row_id(linked)) else {
            return Ok(());
        };
        if a != b {
            self.db.execute(
                "INSERT OR IGNORE INTO track_links (track, linked, added) VALUES (?1, ?2, ?3)",
                params![a, b, now()],
            )?;
        }
        Ok(())
    }

    /// The matches the DJ set by hand for a streaming service's tracks, by
    /// the service's key: the collection track, or None for none of them.
    pub fn stream_matches(&self, service: &str) -> Result<HashMap<String, Option<usize>>, Error> {
        let mut q = self
            .db
            .prepare("SELECT key, track_id FROM stream_matches WHERE service = ?1")?;
        let rows = q.query_map([service], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?.map(track_id),
            ))
        })?;
        rows.collect()
    }

    /// Sets which collection track a streaming service's track is (None:
    /// none of them), or with `forget` goes back to matching it by its tags.
    pub fn set_stream_match(
        &self,
        service: &str,
        key: &str,
        track: Option<usize>,
        forget: bool,
    ) -> Result<(), Error> {
        if forget {
            self.db.execute(
                "DELETE FROM stream_matches WHERE service = ?1 AND key = ?2",
                params![service, key],
            )?;
        } else {
            self.db.execute(
                "INSERT INTO stream_matches (service, key, track_id) VALUES (?1, ?2, ?3)
                 ON CONFLICT(service, key) DO UPDATE SET track_id = excluded.track_id",
                params![service, key, track.and_then(row_id)],
            )?;
        }
        Ok(())
    }

    /// Removes a link between two tracks, whichever way it was made.
    pub fn unlink(&self, track: usize, linked: usize) -> Result<(), Error> {
        let (Some(a), Some(b)) = (row_id(track), row_id(linked)) else {
            return Ok(());
        };
        self.db.execute(
            "DELETE FROM track_links WHERE (track = ?1 AND linked = ?2) OR (track = ?2 AND linked = ?1)",
            params![a, b],
        )?;
        Ok(())
    }

    /// The tracks linked to `track`, and those that link to it, newest
    /// link first.
    pub fn linked(&self, track: usize) -> Result<Vec<usize>, Error> {
        let Some(a) = row_id(track) else {
            return Ok(Vec::new());
        };
        let mut q = self.db.prepare(
            "SELECT other FROM (
                 SELECT linked AS other, added FROM track_links WHERE track = ?1
                 UNION ALL
                 SELECT track AS other, added FROM track_links WHERE linked = ?1
             ) GROUP BY other ORDER BY MAX(added) DESC, other",
        )?;
        let ids = q
            .query_map([a], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(track_id))
            .collect();
        ids
    }

    /// Every link between two tracks, both ways round.
    pub fn all_links(&self) -> Result<HashSet<(usize, usize)>, Error> {
        let mut q = self.db.prepare("SELECT track, linked FROM track_links")?;
        let mut out = HashSet::new();
        for pair in q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (a, b) = pair?;
            out.insert((track_id(a), track_id(b)));
            out.insert((track_id(b), track_id(a)));
        }
        Ok(out)
    }

    /// Tracks played right before or after `track` in the history, most
    /// often first, then most recently.
    pub fn played_with(&self, track: usize) -> Result<Vec<usize>, Error> {
        let Some(a) = row_id(track) else {
            return Ok(Vec::new());
        };
        let mut q = self.db.prepare(
            "SELECT n.track_id FROM playlist_tracks t
             JOIN playlists p ON p.id = t.playlist_id AND p.kind = 2
             JOIN playlist_tracks n ON n.playlist_id = t.playlist_id
                 AND (n.position = t.position - 1 OR n.position = t.position + 1)
             WHERE t.track_id = ?1 AND n.track_id != ?1
             GROUP BY n.track_id
             ORDER BY COUNT(*) DESC, MAX(COALESCE(n.played_at, 0)) DESC, n.track_id",
        )?;
        let ids = q
            .query_map([a], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(track_id))
            .collect();
        ids
    }

    /// Deletes history sessions in which nothing was played.
    pub fn drop_empty_sessions(&self) -> Result<(), Error> {
        self.db.execute(
            "DELETE FROM playlists WHERE kind = 2 AND parent IS NOT NULL
             AND id NOT IN (SELECT playlist_id FROM playlist_tracks)",
            [],
        )?;
        Ok(())
    }

    /// A track already in the library with this artist, title and (within
    /// two seconds) length, so the inbox doesn't add it again.
    pub fn find_same(
        &self,
        artist: &str,
        title: &str,
        duration: f64,
    ) -> Result<Option<usize>, Error> {
        let mut q = self
            .db
            .prepare("SELECT id, artist, title, duration FROM tracks")?;
        let (artist, title) = (simplify(artist), simplify(title));
        if title.is_empty() {
            return Ok(None);
        }
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })?;
        for row in rows {
            let (id, a, t, d) = row?;
            let close = duration <= 0.0 || d <= 0.0 || (d - duration).abs() < 2.0;
            if simplify(&a) == artist && simplify(&t) == title && close {
                return Ok(Some(track_id(id)));
            }
        }
        Ok(None)
    }

    /// Adds a file that arrived in the purchase inbox, from `store` (empty
    /// when unknown), and ties it to the matching wanted track if there is
    /// one. Returns the library's copy of it.
    pub fn add_purchase(&mut self, track: &Track, store: &str) -> Result<Track, Error> {
        let Source::File(path) = &track.source else {
            return Ok(track.clone());
        };
        let p = path_text(path);
        self.db.execute(
            "INSERT INTO tracks (path, artist, title, album, genre, duration, bpm, key, rating,
                                 file_mtime, source, store, added, key_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, 'purchase', ?10, ?11,
                     CASE WHEN ?8 != '' THEN 'tags' END)
             ON CONFLICT(path) DO NOTHING",
            params![
                p,
                track.artist,
                track.title,
                track.album,
                track.genre,
                track.duration,
                track.bpm,
                track.key,
                mtime(path),
                store,
                now()
            ],
        )?;
        let row: i64 = self
            .db
            .query_row("SELECT id FROM tracks WHERE path = ?1", [&p], |r| r.get(0))?;
        self.db.execute(TAG_EXTRAS, tag_extras(row, track))?;
        if let Some(w) = self.wanted_for(
            &track.artist,
            &track.title,
            track.duration,
            &track.info.isrc,
            false,
        )? {
            self.db.execute(
                "UPDATE wanted SET track_id = ?2, pending_store = NULL, pending_since = NULL
                 WHERE id = ?1",
                params![w.id, row],
            )?;
            self.db.execute(
                "UPDATE tracks SET store_url = ?2 WHERE id = ?1",
                params![row, w.page_url],
            )?;
        }
        Ok(Track {
            id: track_id(row),
            ..track.clone()
        })
    }

    /// The Inbox list's id, if it has been made: the browser leaves it out
    /// of the tree once the Inbox stage shows the same tracks.
    pub fn inbox_list_id(&self) -> Option<i64> {
        self.meta("inbox_list").and_then(|v| v.parse::<i64>().ok())
    }

    /// The list new purchases go into for the DJ to look through, made the
    /// first time it's needed (and again if the DJ deleted it).
    pub fn inbox_list(&self) -> Result<i64, Error> {
        if let Some(id) = self.meta("inbox_list").and_then(|v| v.parse::<i64>().ok()) {
            let exists = self
                .db
                .prepare("SELECT 1 FROM playlists WHERE id = ?1 AND kind IN (0, 5)")?
                .exists([id])?;
            if exists {
                return Ok(id);
            }
        }
        let id = self.create_playlist("Inbox", None, PlaylistKind::List)?;
        self.set_meta("inbox_list", &id.to_string())?;
        Ok(id)
    }

    /// Remembers that the DJ wants a track: went to buy it, or added it
    /// from a streaming service. The same page twice is kept once, as the
    /// DJ's own (no import takes it off again). Returns whether it's new.
    pub fn add_wanted(
        &self,
        store: &str,
        artist: &str,
        title: &str,
        page_url: &str,
    ) -> Result<bool, Error> {
        let known = self
            .db
            .prepare("SELECT 1 FROM wanted WHERE page_url = ?1 AND track_id IS NULL")?
            .exists([page_url])?;
        self.db.execute(
            "INSERT INTO wanted (store, artist, title, page_url, added) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(page_url) DO UPDATE SET source = NULL",
            params![store, artist, title, page_url, now()],
        )?;
        self.db.execute(
            "DELETE FROM wanted_dismissed WHERE page_url = ?1",
            [page_url],
        )?;
        Ok(!known)
    }

    /// Puts a track on the wanted list for a playlist kept in sync with it,
    /// unless the DJ took it off before or it's there already. Returns
    /// whether it went on.
    pub fn add_wanted_unless_dismissed(
        &self,
        store: &str,
        artist: &str,
        title: &str,
        page_url: &str,
    ) -> Result<bool, Error> {
        let n = self.db.execute(
            "INSERT INTO wanted (store, artist, title, page_url, added)
             SELECT ?1, ?2, ?3, ?4, ?5
             WHERE NOT EXISTS (SELECT 1 FROM wanted_dismissed WHERE page_url = ?4)
             ON CONFLICT(page_url) DO NOTHING",
            params![store, artist, title, page_url, now()],
        )?;
        Ok(n > 0)
    }

    /// Tracks the DJ went to buy that haven't arrived yet, newest first.
    /// Ones opened with Buy more than [`PENDING_DAYS`] ago stop counting
    /// as being bought.
    pub fn wanted(&self) -> Result<Vec<Wanted>, Error> {
        self.db.execute(
            "UPDATE wanted SET pending_store = NULL, pending_since = NULL
             WHERE pending_since IS NOT NULL AND pending_since < ?1",
            [now() - PENDING_DAYS * 86_400],
        )?;
        let mut q = self.db.prepare(
            "SELECT id, store, artist, title, page_url, isrc, duration,
                    COALESCE(pending_store, '')
             FROM wanted WHERE track_id IS NULL ORDER BY added DESC, id DESC",
        )?;
        let rows = q.query_map([], |r| {
            Ok(Wanted {
                id: r.get(0)?,
                store: r.get(1)?,
                artist: r.get(2)?,
                title: r.get(3)?,
                page_url: r.get(4)?,
                isrc: r.get(5)?,
                duration: r.get(6)?,
                pending_store: r.get(7)?,
            })
        })?;
        rows.collect()
    }

    /// Keeps what the service said of a wanted track: its ISRC and length,
    /// which match it to the file that comes. Known values aren't wiped.
    pub fn set_wanted_details(
        &self,
        page_url: &str,
        isrc: &str,
        duration: f64,
    ) -> Result<(), Error> {
        self.db.execute(
            "UPDATE wanted SET
                isrc = CASE WHEN ?2 != '' THEN ?2 ELSE isrc END,
                duration = CASE WHEN ?3 > 0 THEN ?3 ELSE duration END
             WHERE page_url = ?1",
            params![page_url, isrc, duration],
        )?;
        Ok(())
    }

    /// Marks wanted tracks as being bought on `store` (Buy opened them
    /// there), from now.
    pub fn mark_pending(&self, ids: &[i64], store: &str) -> Result<(), Error> {
        let mut q = self.db.prepare(
            "UPDATE wanted SET pending_store = ?2, pending_since = ?3
             WHERE id = ?1 AND track_id IS NULL",
        )?;
        let at = now();
        for id in ids {
            q.execute(params![id, store, at])?;
        }
        Ok(())
    }

    /// The wanted track a file is: by ISRC, else by title, artist and
    /// length (within 2 seconds when both are known), tracks being bought
    /// first. With `pending_only`, only tracks being bought count.
    pub fn wanted_for(
        &self,
        artist: &str,
        title: &str,
        duration: f64,
        isrc: &str,
        pending_only: bool,
    ) -> Result<Option<Wanted>, Error> {
        let mut all = self.wanted()?;
        if pending_only {
            all.retain(|w| !w.pending_store.is_empty());
        }
        // Being bought first, then the rest, newest first within each.
        all.sort_by_key(|w| w.pending_store.is_empty());
        let key = isrc_key(isrc);
        if !key.is_empty() {
            if let Some(w) = all.iter().find(|w| isrc_key(&w.isrc) == key) {
                return Ok(Some(w.clone()));
            }
        }
        // An untagged file's name, "Artist - Title" (or "01 Artist - Title"),
        // stands in for its tags.
        let mut names = vec![(simplify(artist), simplify(title))];
        if artist.trim().is_empty() {
            if let Some((a, t)) = title.split_once(" - ") {
                let a = a.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ');
                names.push((simplify(a), simplify(t)));
            }
        }
        Ok(all.into_iter().find(|w| {
            let same_length =
                w.duration <= 0.0 || duration <= 0.0 || (w.duration - duration).abs() <= 2.0;
            same_length
                && names.iter().any(|(artist, title)| {
                    !title.is_empty()
                        && simplify(&w.title) == *title
                        && (w.artist.is_empty()
                            || simplify(&w.artist) == *artist
                            || (!artist.is_empty() && simplify(&w.artist).contains(artist)))
                })
        }))
    }

    pub fn remove_wanted(&self, id: i64) -> Result<(), Error> {
        self.db.execute("DELETE FROM wanted WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Takes these tracks off the wanted list, at once, for good: no import
    /// or synced playlist puts them back. Returns how many went.
    pub fn remove_wanted_many(&mut self, ids: &[i64]) -> Result<usize, Error> {
        let tx = self.db.transaction()?;
        let mut n = 0;
        {
            let mut dismiss = tx.prepare(
                "INSERT OR IGNORE INTO wanted_dismissed (page_url)
                 SELECT page_url FROM wanted WHERE id = ?1",
            )?;
            let mut q = tx.prepare("DELETE FROM wanted WHERE id = ?1")?;
            for id in ids {
                dismiss.execute([id])?;
                n += q.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// Whether the inbox has already dealt with this file.
    pub fn inbox_seen(&self, path: &Path) -> Result<bool, Error> {
        self.db
            .prepare("SELECT 1 FROM inbox_log WHERE path = ?1")?
            .exists([path_text(path)])
    }

    /// Records what the inbox did with a file.
    pub fn log_inbox(&self, path: &Path, outcome: &str, track: Option<usize>) -> Result<(), Error> {
        self.db.execute(
            "INSERT OR REPLACE INTO inbox_log (path, outcome, track_id, at) VALUES (?1, ?2, ?3, ?4)",
            params![path_text(path), outcome, track.and_then(row_id), now()],
        )?;
        Ok(())
    }
}

/// Brings in another app's tracks (`source` names the app): their tags,
/// rating, colour and play count, and their grid and cues where the DJ
/// hasn't edited those here.
///
/// A track that is another import's (by path, or a different copy of a
/// song the library has, matched by artist, title and duration) is that
/// app's first: this one only fills in what is empty, its grid only goes
/// where there is none from another app, and its hot cues become memory
/// cues when the track has hot cues already.
fn import_tracks(
    tx: &rusqlite::Transaction,
    source: &str,
    tracks: &[Track],
    theirs_win: bool,
    report: &mut SyncReport,
) -> Result<(), Error> {
    // The other apps brought in before this one.
    let others: HashSet<String> = tx
        .prepare("SELECT substr(key, 14) FROM meta WHERE key LIKE 'import_stamp:%'")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<HashSet<_>, _>>()?
        .into_iter()
        .filter(|s| s != source)
        .collect();
    tx.execute("DELETE FROM track_aliases WHERE source = ?1", [source])?;
    let mut songs = SongIndex::new(tx, source)?;
    let mut find = tx.prepare("SELECT id, source FROM tracks WHERE path = ?1")?;
    let mut insert =
        tx.prepare("INSERT INTO tracks (path, file_mtime, source, added) VALUES (?1, ?2, ?4, ?3)")?;
    let mut alias = tx.prepare(
        "INSERT OR REPLACE INTO track_aliases (path, track_id, source) VALUES (?1, ?2, ?3)",
    )?;
    let mut set = tx.prepare(
        "UPDATE tracks SET
                artist = CASE WHEN ?18 OR artist = '' THEN ?2 ELSE artist END,
                title = CASE WHEN ?18 OR title = '' THEN ?3 ELSE title END,
                album = CASE WHEN ?18 OR album = '' THEN ?4 ELSE album END,
                genre = CASE WHEN ?18 OR genre = '' THEN ?5 ELSE genre END,
                rating = CASE WHEN ?18 OR rating = 0 THEN ?6 ELSE rating END,
                color = CASE WHEN ?18 OR color IS NULL THEN ?7 ELSE color END,
                comment = CASE WHEN ?18 OR comment = '' THEN ?8 ELSE comment END,
                grouping = CASE WHEN ?18 OR grouping = '' THEN ?9 ELSE grouping END,
                play_count = MAX(play_count, ?11),
                source = CASE WHEN ?20 THEN source ELSE ?14 END,
                duration = CASE WHEN ?12 > 0 AND (NOT ?20 OR duration <= 0) THEN ?12 ELSE duration END,
                key_source = CASE WHEN ?13 != '' AND (NOT ?20 OR key = '') THEN ?14 ELSE key_source END,
                key = CASE WHEN ?13 != '' AND (NOT ?20 OR key = '') THEN ?13 ELSE key END,
                remix = CASE WHEN ?15 != '' AND (NOT ?20 OR remix = '') THEN ?15 ELSE remix END
         WHERE id = ?1",
    )?;
    let mut set_grid = tx.prepare(
        "UPDATE tracks SET grid = ?2, bpm = ?3, grid_source = ?4, grid_locked = 1
         WHERE id = ?1 AND (grid_source IS NULL OR grid_source != 'user')
           AND (grid_locked = 0 OR grid_source = ?4)",
    )?;
    let mut grid_source = tx.prepare("SELECT grid_source FROM tracks WHERE id = ?1")?;
    let mut current = tx.prepare(
        "SELECT artist, title, album, genre, rating, color, comment, grouping FROM tracks WHERE id = ?1",
    )?;
    let mut user_cues =
        tx.prepare("SELECT COUNT(*) FROM cues WHERE track_id = ?1 AND source = 'user'")?;
    // Hot cues from somewhere other than this app.
    let mut other_hot_cues = tx.prepare(
        "SELECT COUNT(*) FROM cues WHERE track_id = ?1 AND source != ?2 AND number < ?3",
    )?;
    let mut next_memory = tx.prepare(
        "SELECT MAX(COALESCE(MAX(number) + 1, ?2), ?2) FROM cues WHERE track_id = ?1 AND number >= ?2",
    )?;
    let mut set_dates = tx.prepare(
        "UPDATE tracks SET added = CASE WHEN ?5 THEN MIN(added, COALESCE(?2, added)) ELSE COALESCE(?2, added) END,
                last_played = NULLIF(MAX(COALESCE(last_played, 0), COALESCE(?3, 0)), 0),
                gain = CASE WHEN ?5 THEN COALESCE(gain, ?4) ELSE COALESCE(?4, gain) END
         WHERE id = ?1",
    )?;
    let mut clear_markers =
        tx.prepare("DELETE FROM markers WHERE track_id = ?1 AND source = ?2")?;
    let mut other_markers =
        tx.prepare("SELECT COUNT(*) FROM markers WHERE track_id = ?1 AND source != ?2")?;
    let mut add_marker = tx.prepare(
        "INSERT INTO markers (track_id, kind, seconds, end_seconds, name, color, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    // Cues other apps brought in stay: they are memory cues of theirs.
    let mut clear_own_cues = tx.prepare("DELETE FROM cues WHERE track_id = ?1 AND source = ?2")?;
    let mut clear_cues = tx.prepare("DELETE FROM cues WHERE track_id = ?1 AND source != 'user'")?;
    let mut add_cue = tx.prepare(
        "INSERT OR REPLACE INTO cues (track_id, number, seconds, name, color, end_seconds, source)
         VALUES (?1, ?2, ?3, ?4, ?7, ?5, ?6)",
    )?;
    for t in tracks {
        let Source::File(path) = &t.source else {
            continue;
        };
        let p = path_text(path);
        let found: Option<(i64, String)> = find
            .query_row([&p], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        // Whether the track came from another app first, and whether it's
        // new here.
        let (id, guest, fresh) = match found {
            Some((id, owner)) => {
                report.updated += 1;
                (id, others.contains(&owner), false)
            }
            None => match songs.find(t) {
                Some(id) => {
                    alias.execute(params![p, id, source])?;
                    report.matched += 1;
                    (id, true, false)
                }
                None => {
                    insert.execute(params![p, mtime(path), now(), source])?;
                    report.added += 1;
                    (tx.last_insert_rowid(), false, true)
                }
            },
        };
        let theirs_replace = theirs_win && !guest;
        let mut conflict = |field, ours: String, theirs: String, kept_ours| {
            report.conflicts.push(Conflict {
                path: p.clone(),
                field,
                ours,
                theirs,
                kept_ours,
            });
        };
        if !fresh {
            let ours: [String; 8] = current.query_row([id], |r| {
                let text = |i| r.get::<_, String>(i);
                let int = |i| {
                    r.get::<_, Option<i64>>(i).map(|v| {
                        v.filter(|&v| v != 0)
                            .map_or(String::new(), |v| v.to_string())
                    })
                };
                Ok([
                    text(0)?,
                    text(1)?,
                    text(2)?,
                    text(3)?,
                    int(4)?,
                    int(5)?,
                    text(6)?,
                    text(7)?,
                ])
            })?;
            let int = |v: Option<i64>| {
                v.filter(|&v| v != 0)
                    .map_or(String::new(), |v| v.to_string())
            };
            let theirs = [
                t.artist.clone(),
                t.title.clone(),
                t.album.clone(),
                t.genre.clone(),
                int(Some(t.rating as i64)),
                int(t.info.color.map(i64::from)),
                t.info.comment.clone(),
                t.info.grouping.clone(),
            ];
            const FIELDS: [&str; 8] = [
                "artist", "title", "album", "genre", "rating", "colour", "comment", "grouping",
            ];
            for ((field, ours), theirs) in FIELDS.into_iter().zip(ours).zip(theirs) {
                if !ours.is_empty() && !theirs.is_empty() && ours != theirs {
                    conflict(field, ours, theirs, !theirs_replace);
                }
            }
        }
        set.execute(params![
            id,
            t.artist,
            t.title,
            t.album,
            t.genre,
            t.rating as i64,
            t.info.color.map(|c| c as i64),
            t.info.comment,
            t.info.grouping,
            "",
            t.info.play_count as i64,
            t.duration,
            t.key,
            source,
            t.info.remix,
            "",
            "",
            theirs_replace,
            "",
            guest
        ])?;
        tags::import_track_tags(tx, source, id, &t.info, theirs_win && !guest)?;
        let grid = t.beat_grid();
        if t.info.fixed_grid && !grid.is_empty() {
            let from: Option<String> = grid_source.query_row([id], |r| r.get(0))?;
            if from.as_deref() == Some("user") {
                conflict(
                    "grid",
                    "edited here".into(),
                    format!("{:.2} BPM", t.bpm),
                    true,
                );
            }
            let taken = from.is_some_and(|f| f != source && others.contains(&f));
            if !(guest && taken) {
                set_grid.execute(params![id, grid.to_text(), t.bpm, source])?;
            }
        }
        let mine: i64 = user_cues.query_row([id], |r| r.get(0))?;
        if guest {
            clear_own_cues.execute(params![id, source])?;
            let theirs: i64 =
                other_hot_cues.query_row(params![id, source, MEMORY_CUE as i64], |r| r.get(0))?;
            // Hot cues only when the track has none; otherwise all of them
            // come in as memory cues, after any it has.
            let mut next: i64 =
                next_memory.query_row(params![id, MEMORY_CUE as i64], |r| r.get(0))?;
            for c in &t.info.hotcues {
                let number = if theirs == 0 && !c.is_memory() {
                    c.number as i64
                } else {
                    next += 1;
                    next - 1
                };
                add_cue.execute(params![
                    id,
                    number,
                    c.seconds,
                    c.name,
                    c.end,
                    source,
                    c.color.map(|c| c as i64)
                ])?;
            }
        } else if mine > 0 {
            if !t.info.hotcues.is_empty() {
                conflict(
                    "cues",
                    format!("{mine} edited here"),
                    t.info.hotcues.len().to_string(),
                    true,
                );
            }
        } else {
            clear_cues.execute([id])?;
            for c in &t.info.hotcues {
                add_cue.execute(params![
                    id,
                    c.number as i64,
                    c.seconds,
                    c.name,
                    c.end,
                    source,
                    c.color.map(|c| c as i64)
                ])?;
            }
        }
        set_details(tx, id, &t.info, false)?;
        // When it joined the other app's library, and when it was last
        // played there.
        set_dates.execute(params![
            id,
            (t.info.added > 0).then_some(t.info.added),
            t.info.last_played,
            t.info.gain_db.map(f64::from),
            guest
        ])?;
        clear_markers.execute(params![id, source])?;
        // A guest's phrases and sections would repeat the first app's.
        let taken: i64 = if guest {
            other_markers.query_row(params![id, source], |r| r.get(0))?
        } else {
            0
        };
        if taken == 0 {
            for m in &t.info.markers {
                add_marker.execute(params![
                    id,
                    m.kind.name(),
                    m.seconds,
                    m.end,
                    m.name,
                    m.color.map(|c| c as i64),
                    source
                ])?;
            }
        }
    }
    Ok(())
}

/// The library's songs by artist and title, to find another app's copy of
/// one: same artist and title (ignoring case, punctuation and spacing) and
/// a duration within [`SAME_SONG_SECONDS`]. Leaves out `source`'s own
/// tracks, as two copies in one app are that app's to sort out.
struct SongIndex {
    songs: HashMap<(String, String), Vec<(i64, f64)>>,
    /// Paths by lower-case file name, for copies (on a USB stick, say)
    /// whose tags say too little: same name and size is the same file.
    files: HashMap<String, Vec<(i64, PathBuf)>>,
}

/// How far apart two copies' durations can be and still be one song.
const SAME_SONG_SECONDS: f64 = 2.0;

impl SongIndex {
    fn new(tx: &Connection, source: &str) -> Result<Self, Error> {
        let mut songs: HashMap<(String, String), Vec<(i64, f64)>> = HashMap::new();
        let mut files: HashMap<String, Vec<(i64, PathBuf)>> = HashMap::new();
        let mut q = tx.prepare(
            "SELECT id, artist, title, duration, path FROM tracks WHERE source != ?1 ORDER BY id",
        )?;
        let rows = q.query_map([source], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (id, artist, title, duration, path) = row?;
            if let Some(key) = song_key(&artist, &title) {
                songs.entry(key).or_default().push((id, duration));
            }
            let path = PathBuf::from(path);
            if let Some(name) = file_key(&path) {
                files.entry(name).or_default().push((id, path));
            }
        }
        Ok(SongIndex { songs, files })
    }

    /// The library's copy of `t`, the first added when there are several:
    /// by its tags, or else a file of the same name and size.
    fn find(&mut self, t: &Track) -> Option<i64> {
        self.find_song(&t.artist, &t.title, t.duration)
            .or_else(|| self.find_file(&t.source))
    }

    fn find_file(&self, source: &Source) -> Option<i64> {
        let Source::File(path) = source else {
            return None;
        };
        let size = std::fs::metadata(path).ok()?.len();
        self.files
            .get(&file_key(path)?)?
            .iter()
            .find(|(_, p)| p != path && std::fs::metadata(p).is_ok_and(|m| m.len() == size))
            .map(|(id, _)| *id)
    }

    fn find_song(&self, artist: &str, title: &str, duration: f64) -> Option<i64> {
        if duration <= 0.0 {
            return None;
        }
        self.songs
            .get(&song_key(artist, title)?)?
            .iter()
            .find(|(_, d)| *d > 0.0 && (d - duration).abs() <= SAME_SONG_SECONDS)
            .map(|(id, _)| *id)
    }
}

/// A file name as compared, in lower case.
fn file_key(path: &Path) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().to_lowercase())
}

/// An ISRC as compared: letters and digits in upper case.
fn isrc_key(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Artist and title as they are compared: lower case, letters and digits
/// only, one space between words. `None` when either is empty.
fn song_key(artist: &str, title: &str) -> Option<(String, String)> {
    let norm = |s: &str| {
        s.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let (a, t) = (norm(artist), norm(title));
    (!a.is_empty() && !t.is_empty()).then_some((a, t))
}

/// Stores the details beyond artist, title, album and genre. With
/// `overwrite` off, empty values leave what is there.
fn set_details(tx: &Connection, id: i64, info: &LibraryInfo, overwrite: bool) -> Result<(), Error> {
    let sql = if overwrite {
        "UPDATE tracks SET disc_number = ?4, label = ?5, composer = ?6, remixer = ?7, isrc = ?8
         WHERE id = ?1"
    } else {
        "UPDATE tracks SET
                year = CASE WHEN ?2 != '' THEN ?2 ELSE year END,
                track_number = CASE WHEN ?3 > 0 THEN ?3 ELSE track_number END,
                disc_number = CASE WHEN ?4 > 0 THEN ?4 ELSE disc_number END,
                label = CASE WHEN ?5 != '' THEN ?5 ELSE label END,
                composer = CASE WHEN ?6 != '' THEN ?6 ELSE composer END,
                remixer = CASE WHEN ?7 != '' THEN ?7 ELSE remixer END,
                isrc = CASE WHEN ?8 != '' THEN ?8 ELSE isrc END
         WHERE id = ?1"
    };
    tx.prepare_cached(sql)?.execute(params![
        id,
        info.year,
        info.track_number as i64,
        info.disc_number as i64,
        info.label,
        info.composer,
        info.remixer,
        info.isrc
    ])?;
    Ok(())
}

/// Makes another app's lists again, in a folder named `title` at the top,
/// in place of those brought in last time. History sessions go to the
/// history tab, as the DJ's own do.
fn import_lists(
    tx: &rusqlite::Transaction,
    source: &str,
    title: &str,
    lists: &[ImportedList],
    want_streamed: bool,
    report: &mut SyncReport,
) -> Result<(), Error> {
    // Deleting the folder takes everything in it along, so the DJ's own
    // lists in it move out first.
    sets::rescue_own_lists(tx, source)?;
    tx.execute("DELETE FROM playlists WHERE source = ?1", [source])?;
    if lists.is_empty() {
        return Ok(());
    }
    let mut insert = tx.prepare(
        "INSERT INTO playlists (parent, name, kind, position, created, query, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut make = |parent: Option<i64>,
                    name: &str,
                    kind: PlaylistKind,
                    position: i64,
                    created: i64,
                    query: Option<&str>|
     -> Result<i64, Error> {
        insert.execute(params![
            parent,
            name,
            kind.to_i64(),
            position,
            created,
            query,
            source
        ])?;
        Ok(tx.last_insert_rowid())
    };
    // After the DJ's own lists.
    let last: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists WHERE parent IS NULL",
        [],
        |r| r.get(0),
    )?;
    let top = make(None, title, PlaylistKind::Folder, last, now(), None)?;
    report.lists += 1;
    let mut folders: HashMap<Vec<String>, i64> = HashMap::new();
    // The library's copy, or the song another copy stands for.
    let mut find = tx.prepare(
        "SELECT id FROM tracks WHERE path = ?1
         UNION ALL SELECT track_id FROM track_aliases WHERE path = ?1 LIMIT 1",
    )?;
    let mut add = tx.prepare(
        "INSERT INTO playlist_tracks (playlist_id, position, track_id, played_at)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut stand_in = tx.prepare(
        "INSERT INTO list_streams
             (playlist_id, position, service, stream_id, uri, artist, title, duration, isrc, played_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;
    // Streamed tracks stay in their lists; they go on the wanted list only
    // when the DJ adds them (Add to Wanted), or from playlists when the DJ
    // turned that on for this import. Never from history: what was played
    // isn't always what's wanted.
    let mut want = tx.prepare(
        "INSERT INTO wanted (store, artist, title, page_url, added)
         SELECT ?1, ?2, ?3, ?4, ?5
         WHERE NOT EXISTS (SELECT 1 FROM wanted_dismissed WHERE page_url = ?4)
         ON CONFLICT(page_url) DO NOTHING",
    )?;
    // The collection's copy of a streamed track: the same ISRC, else the
    // same artist and title at the same length.
    let songs = SongIndex::new(tx, "")?;
    let mut by_isrc: HashMap<String, i64> = HashMap::new();
    {
        let mut q = tx.prepare("SELECT id, isrc FROM tracks WHERE isrc != '' ORDER BY id")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, isrc) = row?;
            by_isrc.entry(isrc_key(&isrc)).or_insert(id);
        }
    }
    let have = |s: &StreamedTrack| {
        let isrc = isrc_key(&s.isrc);
        (!isrc.is_empty())
            .then(|| by_isrc.get(&isrc).copied())
            .flatten()
            .or_else(|| songs.find_song(&s.artist, &s.title, s.duration))
    };
    enum Kept<'a> {
        Track(i64),
        Streamed(&'a StreamedTrack),
    }
    for (i, list) in lists.iter().enumerate() {
        // The library's track for each entry, or the streamed track
        // standing in for one; files that aren't here are left out.
        let mut rows = Vec::with_capacity(list.entries.len());
        for e in &list.entries {
            let row: Option<i64> = find
                .query_row([path_text(&e.path)], |r| r.get(0))
                .optional()?;
            match (row, &e.streamed) {
                (Some(row), _) => rows.push((Kept::Track(row), e.played_at)),
                (None, Some(s)) => match have(s) {
                    Some(row) => {
                        report.filled += 1;
                        rows.push((Kept::Track(row), e.played_at));
                    }
                    None => {
                        report.streamed += 1;
                        rows.push((Kept::Streamed(s), e.played_at));
                    }
                },
                (None, None) => report.unmatched += 1,
            }
        }
        // A list whose files have all gone would come in empty: it stays out.
        if rows.is_empty() && !list.entries.is_empty() {
            report.left_out += 1;
            continue;
        }
        let mut parent = top;
        for depth in 1..=list.folders.len() {
            let path = list.folders[..depth].to_vec();
            parent = match folders.get(&path) {
                Some(&id) => id,
                None => {
                    let id = make(
                        Some(parent),
                        &path[depth - 1],
                        PlaylistKind::Folder,
                        i as i64,
                        now(),
                        None,
                    )?;
                    report.lists += 1;
                    folders.insert(path, id);
                    id
                }
            };
        }
        let (kind, created, query) = match &list.kind {
            ImportedListKind::List => (PlaylistKind::List, now(), None),
            ImportedListKind::History { started } => (PlaylistKind::History, *started, None),
            ImportedListKind::Smart { query } => (PlaylistKind::Smart, now(), Some(query.as_str())),
        };
        let id = make(Some(parent), &list.name, kind, i as i64, created, query)?;
        report.lists += 1;
        let wanted_here = want_streamed && matches!(list.kind, ImportedListKind::List);
        for (position, (row, played_at)) in rows.into_iter().enumerate() {
            let played_at = played_at.unwrap_or(created);
            match row {
                Kept::Track(row) => {
                    add.execute(params![id, position as i64, row, played_at])?;
                }
                Kept::Streamed(s) => {
                    stand_in.execute(params![
                        id,
                        position as i64,
                        s.service,
                        s.id,
                        s.uri,
                        s.artist,
                        s.title,
                        s.duration,
                        s.isrc,
                        played_at
                    ])?;
                    if let Some(url) = s.page_url().filter(|_| wanted_here) {
                        want.execute(params![s.service_name(), s.artist, s.title, url, now()])?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::beats::GridMarker;

    pub(crate) fn file_track(path: &str, title: &str) -> Track {
        Track {
            id: 0,
            artist: "Artist".into(),
            title: title.into(),
            album: String::new(),
            genre: "House".into(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 200.0,
            first_beat: 0.0,
            grid: BeatGrid::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Default::default(),
            audio: None,
            cover: None,
            info: Default::default(),
        }
    }

    #[test]
    fn crates_keep_their_colour_and_icon() {
        let lib = Library::in_memory().unwrap();
        let krate = lib
            .create_playlist("Yoga", None, PlaylistKind::List)
            .unwrap();
        let get = |lib: &Library| {
            lib.playlists()
                .unwrap()
                .into_iter()
                .find(|p| p.id == krate)
                .unwrap()
        };
        assert_eq!((get(&lib).color, get(&lib).icon), (None, None));
        lib.set_playlist_color(krate, Some(0x34c759)).unwrap();
        lib.set_playlist_icon(krate, Some("leaf")).unwrap();
        assert_eq!(get(&lib).color, Some(0x34c759));
        assert_eq!(get(&lib).icon.as_deref(), Some("leaf"));
        lib.set_playlist_color(krate, None).unwrap();
        lib.set_playlist_icon(krate, None).unwrap();
        assert_eq!((get(&lib).color, get(&lib).icon), (None, None));
    }

    #[test]
    fn scans_add_new_files_once() {
        let mut lib = Library::in_memory().unwrap();
        let scanned = [file_track("/m/a.mp3", "A"), file_track("/m/b.mp3", "B")];
        let report = lib.sync(&scanned).unwrap();
        assert_eq!(report.added, 2);
        // Scanning again changes nothing.
        assert_eq!(lib.sync(&scanned).unwrap(), SyncReport::default());
        let tracks = lib.tracks(|_| true).unwrap();
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().all(|t| t.id >= ID_BASE));
        // Files that are gone are left out.
        assert_eq!(lib.tracks(|p| p.ends_with("a.mp3")).unwrap().len(), 1);
    }

    #[test]
    fn keeps_the_info_panels_fields() {
        let mut lib = Library::in_memory().unwrap();
        let mut scanned = file_track("/m/a.mp3", "A");
        scanned.info.remix = "Club Mix".into();
        scanned.info.year = "1998".into();
        scanned.info.track_number = 4;
        scanned.info.grouping = "3 JNY".into();
        scanned.rating = 3;
        lib.sync(std::slice::from_ref(&scanned)).unwrap();
        let mut t = lib.tracks(|_| true).unwrap().remove(0);
        assert_eq!(t.info.remix, "Club Mix");
        assert_eq!(t.info.year, "1998");
        assert_eq!(t.info.track_number, 4);
        assert_eq!(t.info.grouping, "3 JNY");
        assert_eq!(t.rating, 3);
        assert_eq!(t.info.gain_db, None);

        t.title = "Edited".into();
        t.key = "8A".into();
        let groups = lib.tag_groups().unwrap();
        let (mood, instruments) = (groups[0].id, groups[1].id);
        t.info.set_tags_in(mood, ["Uplifting".into()]);
        t.info
            .set_tags_in(instruments, ["Synth".into(), "Vocals".into()]);
        t.info.gain_db = Some(-2.5);
        lib.save_info(t.id, &t).unwrap();
        let back = lib.tracks(|_| true).unwrap().remove(0);
        assert_eq!(back.title, "Edited");
        assert_eq!(back.info.tag_text(mood), "Uplifting");
        assert_eq!(back.info.tag_text(instruments), "Synth, Vocals");
        assert_eq!(back.info.gain_db, Some(-2.5));
        // A key the DJ typed in is theirs, not analysis's.
        assert_eq!(back.info.key_detected, Some(false));
    }

    fn imported_track(path: &str) -> Track {
        let mut t = file_track(path, "Imported");
        t.rating = 4;
        t.bpm = 128.0;
        t.first_beat = 0.25;
        t.info = LibraryInfo {
            comment: "nice".into(),
            grouping: "11 YOGA".into(),
            imported_tags: vec![
                ("User 2".into(), "#synth".into()),
                ("User 2".into(), "#vox".into()),
            ],
            color: Some(0xff8800),
            play_count: 3,
            hotcues: vec![Hotcue {
                number: 0,
                seconds: 32.0,
                name: "Drop".into(),
                end: None,
                color: None,
            }],
            fixed_grid: true,
            ..Default::default()
        };
        t
    }

    #[test]
    fn imports_and_keeps_the_djs_edits() {
        let mut lib = Library::in_memory().unwrap();
        let scanned = [file_track("/m/a.mp3", "A"), file_track("/m/b.mp3", "B")];
        lib.sync(&scanned).unwrap();
        let other = vec![imported_track("/m/b.mp3"), imported_track("/m/c.mp3")];
        let read = || {
            Ok(ImportedLibrary {
                tracks: other.clone(),
                lists: Vec::new(),
            })
        };
        let report = lib.import("other", "Other", "1", read).unwrap().unwrap();
        assert_eq!((report.added, report.updated), (1, 1));
        let tracks = lib.tracks(|_| true).unwrap();
        assert_eq!(tracks.len(), 3);
        let b = tracks.iter().find(|t| t.title == "Imported").unwrap();
        assert!(b.id >= ID_BASE);
        assert_eq!(b.rating, 4);
        assert_eq!(b.info.color, Some(0xff8800));
        // A place for tags seen first gets a group of its name.
        let group = lib.tag_groups().unwrap().last().unwrap().clone();
        assert_eq!(group.name, "User 2");
        assert_eq!(b.info.tag_text(group.id), "synth, vox");
        assert_eq!(b.info.grouping, "11 YOGA");
        assert_eq!(b.info.hotcues.len(), 1);
        assert!(b.info.fixed_grid);
        assert!((b.bpm - 128.0).abs() < 1e-9);
        assert!((b.first_beat - 0.25).abs() < 1e-9);
        // Imported grids come in locked.
        assert!(b.info.grid_locked);
        assert_eq!(b.info.grid_from, "other");

        // The DJ edits the grid and the cues here; a later import keeps them.
        let grid = BeatGrid::new(vec![
            GridMarker {
                seconds: 0.5,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 64.5,
                bpm: 124.0,
            },
        ]);
        lib.save_grid(b.id, &grid, 124.0).unwrap();
        lib.save_cues(
            b.id,
            &[
                Hotcue {
                    number: 2,
                    seconds: 10.0,
                    name: String::new(),
                    end: None,
                    color: None,
                },
                Hotcue {
                    number: 3,
                    seconds: 20.0,
                    name: "Break loop".into(),
                    end: Some(24.0),
                    color: None,
                },
            ],
        )
        .unwrap();
        lib.db.execute("DELETE FROM meta", []).unwrap();
        assert!(lib.import("other", "Other", "1", read).unwrap().is_some());
        let b = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| t.id == b.id)
            .unwrap();
        assert_eq!(b.grid, grid);
        assert_eq!(b.info.hotcues[0].number, 2);
        assert_eq!(b.info.hotcues[0].end, None);
        // A saved loop keeps its end.
        assert_eq!(b.info.hotcues[1].end, Some(24.0));
        // An unchanged file isn't read again.
        assert!(lib
            .import("other", "Other", "1", || panic!("read again"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn removes_an_import_and_keeps_the_djs_tracks() {
        let mut lib = Library::in_memory().unwrap();
        let read = || {
            Ok(ImportedLibrary {
                tracks: vec![
                    imported_track("/m/kept.mp3"),
                    imported_track("/m/listed.mp3"),
                    imported_track("/vdj/only.mp3"),
                ],
                lists: vec![ImportedList {
                    folders: Vec::new(),
                    name: "Friday".into(),
                    kind: ImportedListKind::List,
                    entries: Vec::new(),
                }],
            })
        };
        lib.import("vdj", "VirtualDJ", "1", read).unwrap().unwrap();
        let listed = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| matches!(&t.source, Source::File(p) if p == Path::new("/m/listed.mp3")))
            .unwrap();
        let mine = lib
            .create_playlist("Mine", None, PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(mine, &[listed.id]).unwrap();

        let removed = lib
            .remove_import("vdj", |p| p == Path::new("/m/kept.mp3"))
            .unwrap();
        assert_eq!(
            removed,
            RemovedImport {
                lists: 1,
                tracks: 1,
                kept: 2
            }
        );
        let names: Vec<String> = lib
            .playlists()
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert!(names.contains(&"Mine".to_string()));
        assert!(!names.iter().any(|n| n == "VirtualDJ" || n == "Friday"));
        let tracks = lib.tracks(|_| true).unwrap();
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().all(|t| t.info.hotcues.is_empty()));
        // The import can come back in.
        assert!(lib.import("vdj", "VirtualDJ", "1", read).unwrap().is_some());
    }

    #[test]
    fn own_lists_in_an_imported_folder_survive_the_next_import() {
        let mut lib = Library::in_memory().unwrap();
        let read = || {
            Ok(ImportedLibrary {
                tracks: Vec::new(),
                lists: vec![ImportedList {
                    folders: vec!["Sets".into()],
                    name: "Friday".into(),
                    kind: ImportedListKind::List,
                    entries: Vec::new(),
                }],
            })
        };
        lib.import("vdj", "VirtualDJ", "1", read).unwrap().unwrap();
        let find = |lib: &Library, name: &str| {
            lib.playlists()
                .unwrap()
                .into_iter()
                .find(|p| p.name == name)
        };
        let sets = find(&lib, "Sets").unwrap();
        assert!(sets::is_imported(&sets));
        let mine = lib
            .create_playlist("Mine", Some(sets.id), PlaylistKind::List)
            .unwrap();
        lib.create_playlist("Inside", Some(mine), PlaylistKind::List)
            .ok();
        lib.import("vdj", "VirtualDJ", "2", read).unwrap().unwrap();
        // Out at the top of Playlists, not taken for a crate, with what's in it.
        let mine = find(&lib, "Mine").unwrap();
        assert_eq!(mine.parent, None);
        assert!(!sets::is_crate(&mine) && !sets::is_imported(&mine));
        assert!(find(&lib, "Friday").is_some());
    }

    #[test]
    fn adopted_imports_are_the_djs_own() {
        let mut lib = Library::in_memory().unwrap();
        let read = || {
            Ok(ImportedLibrary {
                tracks: Vec::new(),
                lists: vec![ImportedList {
                    folders: Vec::new(),
                    name: "Friday".into(),
                    kind: ImportedListKind::List,
                    entries: Vec::new(),
                }],
            })
        };
        lib.import("vdj", "VirtualDJ", "1", read).unwrap().unwrap();
        assert_eq!(lib.adopt_import("vdj").unwrap(), 2);
        let lists = lib.playlists().unwrap();
        assert!(lists.iter().all(|p| !sets::is_imported(p)));
        // An import with no lists (as for an adopted app) leaves them.
        lib.import("vdj", "VirtualDJ", "2", || {
            Ok(ImportedLibrary {
                tracks: Vec::new(),
                lists: Vec::new(),
            })
        })
        .unwrap();
        let kept = lib.playlists().unwrap();
        assert!(["VirtualDJ", "Friday"]
            .iter()
            .all(|n| kept.iter().any(|p| p.name == *n)));
    }

    #[test]
    fn an_import_can_leave_the_djs_tags_alone() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "Mine")]).unwrap();
        let mut theirs = imported_track("/m/a.mp3");
        theirs.title = "Theirs".into();
        let read = || {
            Ok(ImportedLibrary {
                tracks: vec![theirs.clone()],
                lists: Vec::new(),
            })
        };
        lib.import_with("vdj", "VirtualDJ", "1", false, false, read)
            .unwrap()
            .unwrap();
        let t = &lib.tracks(|_| true).unwrap()[0];
        // The title stays; the rating, which was empty here, comes in.
        assert_eq!((t.title.as_str(), t.rating), ("Mine", 4));
        lib.import_with("vdj", "VirtualDJ", "2", true, false, read)
            .unwrap()
            .unwrap();
        assert_eq!(lib.tracks(|_| true).unwrap()[0].title, "Theirs");
    }

    #[test]
    fn a_stick_copy_joins_the_file_of_the_same_name_and_size() {
        let dir = std::env::temp_dir().join(format!("djs-stick-{}", std::process::id()));
        let (music, stick) = (dir.join("music"), dir.join("stick"));
        std::fs::create_dir_all(&music).unwrap();
        std::fs::create_dir_all(&stick).unwrap();
        for (folder, name, bytes) in [
            (&music, "a.mp3", 10),
            (&stick, "A.MP3", 10),
            (&music, "b.mp3", 10),
            (&stick, "b.mp3", 11),
        ] {
            std::fs::write(folder.join(name), vec![0u8; bytes]).unwrap();
        }
        let at = |p: PathBuf| p.to_string_lossy().into_owned();
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[
            file_track(&at(music.join("a.mp3")), ""),
            file_track(&at(music.join("b.mp3")), ""),
        ])
        .unwrap();
        let untagged = |p: PathBuf| {
            let mut t = imported_track(&at(p));
            t.artist.clear();
            t.title.clear();
            t
        };
        let stick_lib = ImportedLibrary {
            tracks: vec![untagged(stick.join("A.MP3")), untagged(stick.join("b.mp3"))],
            lists: Vec::new(),
        };
        let report = lib
            .preview_import("rekordbox", "rekordbox", false, false, &stick_lib)
            .unwrap();
        assert_eq!((report.matched, report.added), (1, 1));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_preview_reports_conflicts_and_changes_nothing() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "Mine")]).unwrap();
        let mut theirs = imported_track("/m/a.mp3");
        theirs.title = "Theirs".into();
        let other = ImportedLibrary {
            tracks: vec![theirs.clone(), imported_track("/m/new.mp3")],
            lists: Vec::new(),
        };
        let titles = |lib: &Library| -> Vec<String> {
            lib.tracks(|_| true)
                .unwrap()
                .iter()
                .map(|t| t.title.clone())
                .collect()
        };
        let before = titles(&lib);
        let report = lib
            .preview_import("vdj", "VirtualDJ", false, false, &other)
            .unwrap();
        assert_eq!((report.added, report.updated), (1, 1));
        assert_eq!(
            report.conflicts,
            [Conflict {
                path: "/m/a.mp3".into(),
                field: "title",
                ours: "Mine".into(),
                theirs: "Theirs".into(),
                kept_ours: true,
            }]
        );
        assert_eq!(titles(&lib), before);

        // Imported, then edited here: the next import keeps the DJ's grid
        // and cues and says so.
        lib.import_with("vdj", "VirtualDJ", "1", true, false, || Ok(other.clone()))
            .unwrap();
        let id = lib.tracks(|_| true).unwrap()[0].id;
        let grid = BeatGrid::new(vec![GridMarker {
            seconds: 0.5,
            bpm: 120.0,
        }]);
        lib.save_grid(id, &grid, 120.0).unwrap();
        lib.save_cues(
            id,
            &[Hotcue {
                number: 1,
                seconds: 8.0,
                name: String::new(),
                end: None,
                color: None,
            }],
        )
        .unwrap();
        let report = lib
            .preview_import("vdj", "VirtualDJ", true, false, &other)
            .unwrap();
        let fields: Vec<(&str, bool)> = report
            .conflicts
            .iter()
            .map(|c| (c.field, c.kept_ours))
            .collect();
        assert_eq!(fields, [("grid", true), ("cues", true)]);
    }

    #[test]
    fn a_second_apps_copy_joins_the_song_already_here() {
        use djsugar_engine::import::ImportedEntry;
        let mut lib = Library::in_memory().unwrap();
        let mut vdj = imported_track("/vdj/a.mp3");
        vdj.artist = "Bicep".into();
        vdj.title = "Glue".into();
        lib.import("vdj", "VirtualDJ", "1", || {
            Ok(ImportedLibrary {
                tracks: vec![vdj.clone()],
                lists: Vec::new(),
            })
        })
        .unwrap();
        // rekordbox's copy is another file, a second shorter, tagged a little
        // differently, with its own cue and a comment VirtualDJ didn't have.
        let mut rb = imported_track("/rb/Bicep - Glue.mp3");
        rb.artist = "BICEP".into();
        rb.title = "Glue!".into();
        rb.duration = 199.0;
        rb.rating = 2;
        rb.info.comment = String::new();
        rb.info.hotcues[0].seconds = 64.0;
        let mut other = imported_track("/rb/other.mp3");
        other.title = "Other".into();
        let list = ImportedList {
            folders: Vec::new(),
            name: "Friday".into(),
            kind: ImportedListKind::List,
            entries: vec![ImportedEntry::new("/rb/Bicep - Glue.mp3", None)],
        };
        let read = || {
            Ok(ImportedLibrary {
                tracks: vec![rb.clone(), other.clone()],
                lists: vec![list.clone()],
            })
        };
        let report = lib.import("rb", "rekordbox", "1", read).unwrap().unwrap();
        assert_eq!((report.added, report.matched, report.unmatched), (1, 1, 0));
        let tracks = lib.tracks(|_| true).unwrap();
        assert_eq!(tracks.len(), 2);
        let glue = tracks.iter().find(|t| t.title == "Glue").unwrap();
        // VirtualDJ's copy and tags stay; rekordbox's cue is a memory cue.
        assert!(matches!(&glue.source, Source::File(p) if p == Path::new("/vdj/a.mp3")));
        assert_eq!(glue.rating, 4);
        assert_eq!(glue.info.hotcues.len(), 2);
        assert_eq!(glue.info.hotcues[0].seconds, 32.0);
        assert!(glue.info.hotcues[1].is_memory());
        assert_eq!(glue.info.hotcues[1].seconds, 64.0);
        // The list points at the one copy.
        let friday = lib
            .playlists()
            .unwrap()
            .into_iter()
            .find(|p| p.name == "Friday")
            .unwrap();
        assert_eq!(lib.playlist_tracks(friday.id).unwrap(), vec![glue.id]);
        // Bringing rekordbox in again doesn't add the cue twice.
        lib.import("rb", "rekordbox", "2", read).unwrap();
        let glue = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| t.title == "Glue")
            .unwrap();
        assert_eq!(glue.info.hotcues.len(), 2);
        // Removing the rekordbox import leaves VirtualDJ's track as it was.
        lib.remove_import("rb", |_| false).unwrap();
        let tracks = lib.tracks(|_| true).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].info.hotcues.len(), 1);
    }

    #[test]
    fn streamed_tracks_stand_in_their_lists() {
        use djsugar_engine::import::ImportedEntry;
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "Have It")]).unwrap();
        let streamed = |id: &str, title: &str| {
            let mut e = ImportedEntry::new(format!("netsearch://td{id}"), None);
            let s = e.streamed.as_mut().unwrap();
            s.artist = "Artist".into();
            s.title = title.into();
            s.duration = 201.0;
            e
        };
        let list = |name: &str, entries: Vec<ImportedEntry>| ImportedList {
            folders: vec!["Online".into()],
            name: name.into(),
            kind: ImportedListKind::List,
            entries,
        };
        let read = || {
            Ok(ImportedLibrary {
                tracks: vec![imported_track("/m/b.mp3")],
                lists: vec![
                    list(
                        "Mixed",
                        vec![
                            streamed("1", "Want It"),
                            ImportedEntry::new("/m/b.mp3", None),
                            ImportedEntry::new("/m/gone.mp3", None),
                            // The collection has this one: the list gets it.
                            streamed("2", "Have It"),
                        ],
                    ),
                    list("Streams", vec![streamed("3", "Online Only")]),
                    list("Gone", vec![ImportedEntry::new("/m/gone.mp3", None)]),
                    list("Empty on purpose", Vec::new()),
                ],
            })
        };
        let report = lib.import("vdj", "VirtualDJ", "1", read).unwrap().unwrap();
        assert_eq!(
            (
                report.left_out,
                report.unmatched,
                report.streamed,
                report.filled
            ),
            (1, 2, 2, 1)
        );
        let lists = lib.playlists().unwrap();
        let id = |name: &str| lists.iter().find(|p| p.name == name).map(|p| p.id);
        assert!(id("Gone").is_none());
        assert!(id("Streams").is_some() && id("Empty on purpose").is_some());
        let tracks = lib.tracks(|_| true).unwrap();
        let title = |id: usize| tracks.iter().find(|t| t.id == id).unwrap().title.clone();
        let entries = lib.playlist_entries(id("Mixed").unwrap()).unwrap();
        let shown: Vec<String> = entries
            .iter()
            .map(|e| match e {
                ListEntry::Track(t) => title(*t),
                ListEntry::Streamed(s) => format!("{}:{} {}", s.service, s.id, s.title),
            })
            .collect();
        assert_eq!(shown, ["tidal:1 Want It", "Imported", "Have It"]);
        // Appending goes after the placeholders.
        let a = tracks.iter().find(|t| t.title == "Have It").unwrap().id;
        lib.add_to_playlist(id("Mixed").unwrap(), &[a]).unwrap();
        assert_eq!(lib.playlist_entries(id("Mixed").unwrap()).unwrap().len(), 4);
        // Streamed tracks stay in their lists, off the wanted list.
        assert!(lib.wanted().unwrap().is_empty());
        // What the service says fills in what the list didn't.
        lib.describe_streamed(&StreamedTrack {
            service: "tidal".into(),
            id: "3".into(),
            isrc: "GBARL9600049".into(),
            ..Default::default()
        })
        .unwrap();
        let Some(ListEntry::Streamed(s)) =
            lib.playlist_entries(id("Streams").unwrap()).unwrap().pop()
        else {
            panic!("no placeholder")
        };
        assert_eq!(
            (s.isrc.as_str(), s.title.as_str()),
            ("GBARL9600049", "Online Only")
        );
        assert_eq!(lib.streamed_without_isrc("tidal").unwrap(), ["1"]);
        lib.remove_import("vdj", |_| false).unwrap();
        assert!(lib.wanted().unwrap().is_empty());
        assert!(!lib.has_streamed_entries().unwrap());
    }

    #[test]
    fn imports_details_markers_and_lists() {
        use djsugar_engine::import::ImportedEntry;
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let mine = lib
            .create_playlist("Mine", None, PlaylistKind::List)
            .unwrap();
        let mut b = imported_track("/m/b.mp3");
        b.info.year = "1999".into();
        b.info.label = "Warp".into();
        b.info.added = 1_600_000_000;
        b.info.last_played = Some(1_700_000_000);
        b.info.hotcues[0].color = Some(0x00ff00);
        b.info.hotcues.push(Hotcue {
            number: djsugar_engine::MEMORY_CUE,
            seconds: 5.0,
            ..Default::default()
        });
        b.info.markers = vec![Marker {
            kind: MarkerKind::Section,
            seconds: 100.0,
            end: None,
            name: "Break 1".into(),
            color: None,
            detected: false,
        }];
        let entry = |p: &str, at| ImportedEntry::new(p, at);
        let lists = vec![
            ImportedList {
                folders: vec!["Sets".into(), "2023".into()],
                name: "Friday".into(),
                kind: ImportedListKind::List,
                entries: vec![
                    entry("/m/b.mp3", None),
                    entry("netsearch://td1", None),
                    entry("/m/a.mp3", None),
                ],
            },
            ImportedList {
                folders: Vec::new(),
                name: "2023-11-13".into(),
                kind: ImportedListKind::History {
                    started: 1_699_900_000,
                },
                entries: vec![entry("/m/b.mp3", Some(1_699_928_555))],
            },
            ImportedList {
                folders: vec!["Filters".into()],
                name: "Old ones".into(),
                kind: ImportedListKind::Smart {
                    query: "year:<2000".into(),
                },
                entries: Vec::new(),
            },
        ];
        let read = || {
            Ok(ImportedLibrary {
                tracks: vec![b.clone()],
                lists: lists.clone(),
            })
        };
        let report = lib.import("vdj", "VirtualDJ", "1", read).unwrap().unwrap();
        // VirtualDJ, Sets, 2023, Friday, the session, Filters, Old ones.
        assert_eq!((report.lists, report.unmatched, report.streamed), (7, 0, 1));
        let t = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| t.title == "Imported")
            .unwrap();
        assert_eq!(
            (t.info.year.as_str(), t.info.label.as_str()),
            ("1999", "Warp")
        );
        assert_eq!(
            (t.info.added, t.info.last_played),
            (1_600_000_000, Some(1_700_000_000))
        );
        assert_eq!(t.info.hotcues.len(), 2);
        assert_eq!(t.info.hotcues[0].color, Some(0x00ff00));
        assert!(t.info.hotcues[1].is_memory());
        assert_eq!(t.info.markers[0].name, "Break 1");

        let tree = lib.playlists().unwrap();
        let names: Vec<&str> = tree.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names[names.len() - 8..],
            [
                "Mine",
                "VirtualDJ",
                "Sets",
                "2023",
                "Friday",
                "2023-11-13",
                "Filters",
                "Old ones"
            ]
        );
        let friday = tree.iter().find(|p| p.name == "Friday").unwrap();
        assert_eq!(friday.source.as_deref(), Some("vdj"));
        assert_eq!(lib.playlist_tracks(friday.id).unwrap(), [t.id, ID_BASE + 1]);
        let session = tree
            .iter()
            .find(|p| p.kind == PlaylistKind::History)
            .unwrap();
        assert_eq!(session.created, 1_699_900_000);
        let smart = tree
            .iter()
            .find(|p| p.kind == PlaylistKind::Smart && p.name == "Old ones");
        assert_eq!(smart.unwrap().query.as_deref(), Some("year:<2000"));

        // Importing again replaces the lists brought in last time, and
        // leaves the DJ's own alone.
        let fewer = || {
            Ok(ImportedLibrary {
                tracks: vec![b.clone()],
                lists: lists[..1].to_vec(),
            })
        };
        lib.import("vdj", "VirtualDJ", "2", fewer).unwrap().unwrap();
        let tree = lib.playlists().unwrap();
        assert!(tree.iter().any(|p| p.id == mine));
        assert_eq!(tree.iter().filter(|p| p.name == "Friday").count(), 1);
        assert!(!tree.iter().any(|p| p.name == "Old ones"));
    }

    #[test]
    fn detected_phrases_replace_their_own_and_leave_imported_markers() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let id = lib.tracks(|_| true).unwrap()[0].id;
        let row = row_id(id).unwrap();
        lib.db
            .execute(
                "INSERT INTO markers (track_id, kind, seconds, name, source)
                 VALUES (?1, 'section', 30.0, 'Break 1', 'vdj')",
                [row],
            )
            .unwrap();
        let phrase = |seconds: f64, name: &str| Marker {
            kind: MarkerKind::Phrase,
            seconds,
            end: Some(seconds + 30.0),
            name: name.into(),
            color: None,
            detected: true,
        };
        lib.save_phrases(id, &[phrase(0.0, "Intro"), phrase(30.0, "Drop")])
            .unwrap();
        lib.save_phrases(id, &[phrase(0.0, "Intro"), phrase(60.0, "Drop")])
            .unwrap();
        let t = lib.tracks(|_| true).unwrap().remove(0);
        let found: Vec<_> = t
            .info
            .markers
            .iter()
            .map(|m| (m.name.as_str(), m.seconds, m.detected))
            .collect();
        assert_eq!(
            found,
            [
                ("Intro", 0.0, true),
                ("Break 1", 30.0, false),
                ("Drop", 60.0, true)
            ]
        );
    }

    #[test]
    fn grids_lock_and_go_back_to_the_analysed_one() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let mut a = lib.tracks(|_| true).unwrap().remove(0);
        a.set_grid(BeatGrid::constant(126.0, 0.1));
        a.info.grid_unsure = true;
        lib.save_analysis(a.id, &a).unwrap();
        let t = lib.tracks(|_| true).unwrap().remove(0);
        assert_eq!(t.info.grid_from, "analysis");
        assert!(t.info.grid_unsure && !t.info.grid_locked);

        lib.set_grid_locked(a.id, true).unwrap();
        assert!(lib.tracks(|_| true).unwrap()[0].info.grid_locked);

        lib.save_grid(a.id, &BeatGrid::constant(125.0, 0.0), 125.0)
            .unwrap();
        assert_eq!(lib.tracks(|_| true).unwrap()[0].info.grid_from, "user");
        // Back to the analysed grid: forgotten, then analysis stores it.
        lib.clear_grid(a.id).unwrap();
        a.info.grid_unsure = false;
        lib.save_analysis(a.id, &a).unwrap();
        let t = lib.tracks(|_| true).unwrap().remove(0);
        assert_eq!(t.info.grid_from, "analysis");
        assert_eq!(t.grid, BeatGrid::constant(126.0, 0.1));
        assert!(!t.info.grid_unsure);
        assert!(t.info.grid_locked, "the lock stays");
    }

    #[test]
    fn analysis_is_stored_once_and_ratings_and_colours_are_kept() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let mut a = lib.tracks(|_| true).unwrap().remove(0);
        assert!(!a.info.fixed_grid);
        a.set_grid(BeatGrid::constant(126.0, 0.1));
        a.key = "8A".into();
        a.info.key_detected = Some(true);
        lib.save_analysis(a.id, &a).unwrap();
        lib.set_rating(a.id, 5).unwrap();
        lib.set_color(a.id, Some(0x00ff00)).unwrap();
        lib.set_comment(a.id, "opener").unwrap();
        let a2 = lib.tracks(|_| true).unwrap().remove(0);
        assert!(a2.info.fixed_grid);
        assert!(a2.info.grid_analysed);
        assert_eq!((a2.bpm, a2.key.as_str(), a2.rating), (126.0, "8A", 5));
        assert_eq!(a2.info.key_detected, Some(true));
        assert_eq!(a2.info.color, Some(0x00ff00));
        assert_eq!(a2.info.comment, "opener");
        // A user grid isn't replaced by a later analysis.
        lib.save_grid(a.id, &BeatGrid::constant(125.0, 0.0), 125.0)
            .unwrap();
        lib.save_analysis(a.id, &a).unwrap();
        assert!(!lib.tracks(|_| true).unwrap()[0].info.grid_analysed);
        assert_eq!(lib.tracks(|_| true).unwrap()[0].bpm, 125.0);
        assert_eq!(
            lib.tracks(|_| true).unwrap()[0].grid,
            BeatGrid::constant(125.0, 0.0)
        );
    }

    #[test]
    fn keys_remember_where_they_came_from() {
        let mut lib = Library::in_memory().unwrap();
        let mut tagged = file_track("/m/tagged.mp3", "Tagged");
        tagged.key = "5A".into();
        lib.sync(&[tagged, file_track("/m/bare.mp3", "Bare")])
            .unwrap();
        let found = |lib: &Library| -> Vec<Option<bool>> {
            let tracks = lib.tracks(|_| true).unwrap();
            tracks.iter().map(|t| t.info.key_detected).collect()
        };
        assert_eq!(found(&lib), [Some(false), None]);

        // Analysis keeps the tagged key as the tags' own.
        let tracks = lib.tracks(|_| true).unwrap();
        let mut t = tracks[0].clone();
        t.info.key_detected = Some(false);
        lib.save_analysis(t.id, &t).unwrap();
        let mut b = tracks[1].clone();
        b.key = "9B".into();
        b.info.key_detected = Some(true);
        lib.save_analysis(b.id, &b).unwrap();
        assert_eq!(found(&lib), [Some(false), Some(true)]);

        // Another app's key is that app's.
        let mut imported = imported_track("/m/bare.mp3");
        imported.key = "1A".into();
        let tx = lib.db.transaction().unwrap();
        import_tracks(
            &tx,
            "virtualdj",
            &[imported],
            true,
            &mut SyncReport::default(),
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(found(&lib), [Some(false), Some(false)]);
    }

    #[test]
    fn a_write_after_a_read_waits_for_another_connection() {
        let path = std::env::temp_dir().join(format!("djsugar-locked-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut a = Library::open(&path).unwrap();
        a.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        // The startup sync reads first, then writes, while the app writes too.
        let tx = a.db.transaction().unwrap();
        let n: i64 = tx
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        let other = {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut b = Library::open(&path).unwrap();
                b.sync(&[file_track("/m/b.mp3", "B")]).map(|_| ())
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(300));
        tx.execute("UPDATE tracks SET rating = ?1", [n]).unwrap();
        tx.commit().unwrap();
        other.join().unwrap().unwrap();
        assert_eq!(a.tracks(|_| true).unwrap().len(), 2);
        drop(a);
        for end in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{end}", path.display()));
        }
    }

    #[test]
    fn a_version_4_library_marks_imported_keys() {
        let path = std::env::temp_dir().join(format!("djsugar-lib-v4-{}.db", std::process::id()));
        {
            let mut lib = Library::open(&path).unwrap();
            let mut t = file_track("/m/a.mp3", "A");
            t.key = "4B".into();
            lib.sync(&[t, file_track("/m/b.mp3", "B")]).unwrap();
            lib.db
                .execute_batch(
                    "UPDATE tracks SET key_source = NULL;
                     UPDATE tracks SET source = 'virtualdj', key = '7A' WHERE path = '/m/b.mp3';
                     PRAGMA user_version = 4",
                )
                .unwrap();
        }
        let lib = Library::open(&path).unwrap();
        let keys: Vec<Option<bool>> = lib
            .tracks(|_| true)
            .unwrap()
            .iter()
            .map(|t| t.info.key_detected)
            .collect();
        // A scanned key could be from tags or analysis; that's checked on load.
        assert_eq!(keys, [None, Some(false)]);
        drop(lib);
        for end in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{end}", path.display()));
        }
    }

    #[test]
    fn smart_lists_and_missing_files() {
        let mut lib = Library::in_memory().unwrap();
        let starters: Vec<(String, Option<String>)> = lib
            .playlists()
            .unwrap()
            .into_iter()
            .map(|p| (p.name, p.query))
            .collect();
        assert_eq!(starters.len(), STARTER_SMART_LISTS.len());
        assert_eq!(
            starters[0],
            ("Mixes with deck".into(), Some("~deck".into()))
        );
        let id = lib
            .create_smart_list("Peak", None, " bpm:126-130 ")
            .unwrap();
        lib.set_query(id, "bpm:127-131").unwrap();
        let peak = lib
            .playlists()
            .unwrap()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap();
        assert_eq!(peak.kind, PlaylistKind::Smart);
        assert_eq!(peak.query.as_deref(), Some("bpm:127-131"));

        lib.sync(&[file_track("/m/a.mp3", "A"), file_track("/m/b.mp3", "B")])
            .unwrap();
        let all = lib.all_tracks(|p| p.ends_with("a.mp3")).unwrap();
        assert_eq!(all.len(), 2);
        assert!(!all[0].info.missing && all[1].info.missing);
        assert!(all[0].info.added > 0, "the date it was added is kept");
        assert_eq!(lib.tracks(|p| p.ends_with("a.mp3")).unwrap().len(), 1);
    }

    #[test]
    fn a_version_3_library_gains_saved_loops() {
        let path = std::env::temp_dir().join(format!("djsugar-v3-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Connection::open(&path).unwrap();
            // The cues table before saved loops; markers came later still.
            let schema = SCHEMA.replacen("    end_seconds REAL,\n", "", 1);
            db.execute_batch(&schema[..schema.find("-- Named sections").unwrap()])
                .unwrap();
            db.execute_batch("PRAGMA user_version = 3").unwrap();
        }
        let mut lib = Library::open(&path).unwrap();
        lib.sync(&[file_track("/m/a.mp3", "A")]).unwrap();
        let id = lib.tracks(|_| true).unwrap()[0].id;
        let saved = Hotcue {
            number: 0,
            seconds: 8.0,
            name: String::new(),
            end: Some(16.0),
            color: None,
        };
        lib.save_cues(id, std::slice::from_ref(&saved)).unwrap();
        assert_eq!(lib.tracks(|_| true).unwrap()[0].info.hotcues, [saved]);
        drop(lib);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_version_1_library_gains_smart_lists() {
        let path = std::env::temp_dir().join(format!("djsugar-v1-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(SCHEMA).unwrap();
            db.execute_batch("PRAGMA user_version = 1").unwrap();
            db.execute(
                "INSERT INTO playlists (name, kind) VALUES ('Old list', 0)",
                [],
            )
            .unwrap();
        }
        let lib = Library::open(&path).unwrap();
        let lists = lib.playlists().unwrap();
        assert_eq!(lists[0].name, "Old list");
        assert_eq!(lists.len(), 1 + STARTER_SMART_LISTS.len());
        drop(lib);
        // Opening again doesn't add them twice.
        assert_eq!(
            Library::open(&path).unwrap().playlists().unwrap().len(),
            lists.len()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn streaming_matches_set_by_hand() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[file_track("/m/a.mp3", "a")]).unwrap();
        let a = lib.tracks(|_| true).unwrap()[0].id;
        lib.set_stream_match("spotify", "t1", Some(a), false)
            .unwrap();
        lib.set_stream_match("spotify", "t2", None, false).unwrap();
        lib.set_stream_match("tidal", "t1", None, false).unwrap();
        let m = lib.stream_matches("spotify").unwrap();
        assert_eq!((m["t1"], m["t2"], m.len()), (Some(a), None, 2));
        // Set again, and forgotten.
        lib.set_stream_match("spotify", "t2", Some(a), false)
            .unwrap();
        lib.set_stream_match("spotify", "t1", None, true).unwrap();
        let m = lib.stream_matches("spotify").unwrap();
        assert_eq!((m.get("t1"), m["t2"]), (None, Some(a)));
    }

    #[test]
    fn links_and_tracks_played_together() {
        let mut lib = Library::in_memory().unwrap();
        let files: Vec<Track> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| file_track(&format!("/m/{n}.mp3"), n))
            .collect();
        lib.sync(&files).unwrap();
        let ids: Vec<usize> = lib.tracks(|_| true).unwrap().iter().map(|t| t.id).collect();
        let (a, b, c, d) = (ids[0], ids[1], ids[2], ids[3]);

        lib.link(a, b).unwrap();
        lib.link(c, a).unwrap();
        lib.link(a, a).unwrap();
        // Forward and backward links both show, each once.
        let mut linked = lib.linked(a).unwrap();
        linked.sort();
        assert_eq!(linked, [b, c]);
        assert_eq!(lib.linked(b).unwrap(), [a]);
        lib.unlink(b, a).unwrap();
        assert_eq!(lib.linked(a).unwrap(), [c]);

        // Two sessions: b follows a twice, d precedes a once.
        let s1 = lib.start_session("one").unwrap();
        for t in [d, a, b] {
            lib.record_play(Some(s1), t).unwrap();
        }
        let s2 = lib.start_session("two").unwrap();
        for t in [a, b, c] {
            lib.record_play(Some(s2), t).unwrap();
        }
        // An ordinary list doesn't count as history.
        let list = lib
            .create_playlist("List", None, PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(list, &[a, c]).unwrap();
        assert_eq!(lib.played_with(a).unwrap(), [b, d]);
        assert_eq!(lib.played_with(c).unwrap(), [b]);
    }

    #[test]
    fn forgets_tracks_of_a_folder_no_longer_scanned() {
        let mut lib = Library::in_memory().unwrap();
        assert_eq!(lib.scanned_folders(), None);
        lib.sync(&[
            file_track("/home/s/Music/Ableton/kick.wav", "Kick"),
            file_track("/home/s/Music/Ableton/snare.wav", "Snare"),
            file_track("/home/s/Music/Old/liked.mp3", "Liked"),
            file_track("/home/s/Music/DJ/b.mp3", "B"),
            file_track("/home/s/DB/Library/a.mp3", "A"),
        ])
        .unwrap();
        let ids: HashMap<String, usize> = lib
            .tracks(|_| true)
            .unwrap()
            .iter()
            .map(|t| (t.title.clone(), t.id))
            .collect();
        let crate_ = lib
            .create_playlist("Yoga", None, PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(crate_, &[ids["Liked"]]).unwrap();
        lib.set_scanned_folders(&[PathBuf::from("/home/s/Music"), PathBuf::from("/home/s/DB")])
            .unwrap();
        assert_eq!(
            lib.scanned_folders().unwrap(),
            [PathBuf::from("/home/s/Music"), PathBuf::from("/home/s/DB")]
        );
        assert_eq!(
            lib.forget_scanned_under(
                Path::new("/home/s/Music"),
                &[PathBuf::from("/home/s/Music/DJ")]
            )
            .unwrap(),
            2
        );
        let mut left: Vec<String> = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["A", "B", "Liked"],
            "a track in a list, or in a folder still scanned, stays"
        );
    }

    #[test]
    fn playlists_folders_and_history() {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[
            file_track("/m/a.mp3", "A"),
            file_track("/m/b.mp3", "B"),
            file_track("/m/c.mp3", "C"),
        ])
        .unwrap();
        let ids: Vec<usize> = lib.tracks(|_| true).unwrap().iter().map(|t| t.id).collect();
        let sets = lib
            .create_playlist("Sets", None, PlaylistKind::Folder)
            .unwrap();
        let friday = lib
            .create_playlist("Friday", Some(sets), PlaylistKind::List)
            .unwrap();
        let warmup = lib
            .create_playlist("Warm-up", None, PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(friday, &[ids[2], ids[0], ids[2]])
            .unwrap();
        assert_eq!(
            lib.playlist_tracks(friday).unwrap(),
            [ids[2], ids[0], ids[2]]
        );
        lib.remove_from_playlist(friday, 1).unwrap();
        assert_eq!(lib.playlist_tracks(friday).unwrap(), [ids[2], ids[2]]);
        lib.rename_playlist(warmup, "Openers").unwrap();
        let names: Vec<String> = lib
            .playlists()
            .unwrap()
            .into_iter()
            .filter(|p| p.kind != PlaylistKind::Smart)
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Sets", "Friday", "Openers"]);

        let session = lib.start_session("2026-10-13 21:00").unwrap();
        lib.record_play(Some(session), ids[1]).unwrap();
        lib.record_play(Some(session), ids[1]).unwrap();
        let empty = lib.start_session("2026-10-14 21:00").unwrap();
        lib.drop_empty_sessions().unwrap();
        let tree = lib.playlists().unwrap();
        assert!(tree
            .iter()
            .any(|p| p.name == "History" && p.parent.is_none()));
        assert!(tree.iter().any(|p| p.id == session));
        assert!(!tree.iter().any(|p| p.id == empty));
        assert_eq!(lib.playlist_tracks(session).unwrap(), [ids[1], ids[1]]);
        // Each play keeps when it was, in the order played.
        let times = lib.playlist_times(session).unwrap();
        assert_eq!(times.len(), 2);
        assert!(times.iter().all(|t| t.is_some_and(|t| t > 0)));
        assert!(times[0] <= times[1]);
        let b = lib
            .tracks(|_| true)
            .unwrap()
            .into_iter()
            .find(|t| t.id == ids[1])
            .unwrap();
        assert_eq!(b.info.play_count, 2);

        // Deleting a folder deletes what is in it.
        lib.delete_playlist(sets).unwrap();
        assert!(lib.playlist_tracks(friday).unwrap().is_empty());
        assert!(!lib.playlists().unwrap().iter().any(|p| p.id == friday));
    }

    #[test]
    fn an_import_can_want_its_playlists_streamed_tracks() {
        use djsugar_engine::import::ImportedEntry;
        let mut lib = Library::in_memory().unwrap();
        let streamed = |id: &str| {
            let mut e = ImportedEntry::new(format!("netsearch://td{id}"), None);
            let s = e.streamed.as_mut().unwrap();
            s.artist = "Artist".into();
            s.title = format!("Track {id}");
            e
        };
        let read = |n: usize| {
            move || {
                Ok(ImportedLibrary {
                    tracks: Vec::new(),
                    lists: vec![
                        ImportedList {
                            folders: Vec::new(),
                            name: "To buy".into(),
                            kind: ImportedListKind::List,
                            entries: (1..=n).map(|i| streamed(&i.to_string())).collect(),
                        },
                        ImportedList {
                            folders: Vec::new(),
                            name: "Friday".into(),
                            kind: ImportedListKind::History { started: 0 },
                            entries: vec![streamed("99")],
                        },
                    ],
                })
            }
        };
        lib.import_with("vdj", "VirtualDJ", "1", true, true, read(2))
            .unwrap();
        let titles = |lib: &Library| {
            let mut t: Vec<String> = lib.wanted().unwrap().into_iter().map(|w| w.title).collect();
            t.sort();
            t
        };
        // History stays off the list.
        assert_eq!(titles(&lib), ["Track 1", "Track 2"]);
        // Taken off by hand, it stays off; new ones come on; none go.
        let one = lib
            .wanted()
            .unwrap()
            .into_iter()
            .find(|w| w.title == "Track 1");
        lib.remove_wanted_many(&[one.unwrap().id]).unwrap();
        lib.import_with("vdj", "VirtualDJ", "2", true, true, read(3))
            .unwrap();
        assert_eq!(titles(&lib), ["Track 2", "Track 3"]);
        lib.import_with("vdj", "VirtualDJ", "3", true, true, read(0))
            .unwrap();
        assert_eq!(titles(&lib), ["Track 2", "Track 3"]);
        lib.remove_import("vdj", |_| false).unwrap();
        assert_eq!(titles(&lib), ["Track 2", "Track 3"]);
        // Adding it by hand brings it back.
        assert!(lib
            .add_wanted(
                "TIDAL",
                "Artist",
                "Track 1",
                "https://tidal.com/browse/track/1"
            )
            .unwrap());
        assert!(!lib
            .add_wanted_unless_dismissed(
                "TIDAL",
                "Artist",
                "Track 1",
                "https://tidal.com/browse/track/1"
            )
            .unwrap());
    }

    #[test]
    fn the_wanted_list_is_the_djs_own() {
        let mut lib = Library::in_memory().unwrap();
        // A wanted track from before, put there by an import.
        lib.db
            .execute(
                "INSERT INTO wanted (store, artist, title, page_url, source)
                 VALUES ('TIDAL', 'A', 'Old', 'https://tidal.com/browse/track/9', 'vdj')",
                [],
            )
            .unwrap();
        // Adding it by hand makes it the DJ's: removing the import keeps it.
        assert!(!lib
            .add_wanted("TIDAL", "A", "Old", "https://tidal.com/browse/track/9")
            .unwrap());
        assert!(lib
            .add_wanted("TIDAL", "B", "New", "https://tidal.com/browse/track/8")
            .unwrap());
        lib.remove_import("vdj", |_| false).unwrap();
        let ids: Vec<i64> = lib.wanted().unwrap().iter().map(|w| w.id).collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(lib.remove_wanted_many(&ids).unwrap(), 2);
        assert!(lib.wanted().unwrap().is_empty());
    }

    #[test]
    fn opening_drops_what_imports_put_on_the_wanted_list() {
        let dir = std::env::temp_dir().join(format!("djsugar-wanted-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("library.db");
        let _ = std::fs::remove_file(&path);
        {
            let lib = Library::open(&path).unwrap();
            lib.add_wanted("Bandcamp", "A", "Bought", "https://a.bandcamp.com/track/x")
                .unwrap();
            lib.db
                .execute_batch(
                    "INSERT INTO wanted (store, artist, title, page_url, source)
                     VALUES ('TIDAL', 'B', 'Streamed', 'https://tidal.com/browse/track/1', 'vdj');
                     PRAGMA user_version = 11;",
                )
                .unwrap();
        }
        let lib = Library::open(&path).unwrap();
        let left: Vec<String> = lib.wanted().unwrap().into_iter().map(|w| w.title).collect();
        assert_eq!(left, ["Bought"]);
        drop(lib);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn purchases_match_by_isrc_and_tracks_being_bought_first() {
        let mut lib = Library::in_memory().unwrap();
        lib.add_wanted(
            "Spotify",
            "Satorio",
            "Crystal Bowls",
            "https://open.spotify.com/track/1",
        )
        .unwrap();
        lib.set_wanted_details("https://open.spotify.com/track/1", "GB-ABC-25-00001", 412.0)
            .unwrap();
        lib.add_wanted(
            "TIDAL",
            "Satorio",
            "Crystal Bowls",
            "https://tidal.com/browse/track/2",
        )
        .unwrap();
        let ids: Vec<i64> = lib.wanted().unwrap().iter().map(|w| w.id).collect();
        // The TIDAL one is newest; mark the Spotify one as being bought.
        let spotify = *ids.last().unwrap();
        lib.mark_pending(&[spotify], "bandcamp").unwrap();
        let w = lib.wanted().unwrap();
        assert_eq!(
            w.iter().find(|w| w.id == spotify).unwrap().pending_store,
            "bandcamp"
        );
        assert_eq!(
            w.iter().find(|w| w.id == spotify).unwrap().isrc,
            "GB-ABC-25-00001"
        );

        // A file of another length doesn't match by name…
        let found = lib
            .wanted_for("Satorio", "Crystal Bowls", 300.0, "", true)
            .unwrap();
        assert!(found.is_none());
        // …but its ISRC does, whatever its name says.
        let found = lib
            .wanted_for(
                "Satorio",
                "Crystal Bowls (Original Mix)",
                300.0,
                "GBABC2500001",
                true,
            )
            .unwrap();
        assert_eq!(found.map(|w| w.id), Some(spotify));
        // An untagged file named "Artist - Title".
        let found = lib
            .wanted_for("", "01 Satorio - Crystal Bowls", 412.5, "", true)
            .unwrap();
        assert_eq!(found.map(|w| w.id), Some(spotify));
        // By name, the track being bought comes first.
        let found = lib
            .wanted_for("SATORIO", "Crystal Bowls", 0.0, "", false)
            .unwrap();
        assert_eq!(found.map(|w| w.id), Some(spotify));

        let mut t = file_track(
            "/m/Purchased/Bandcamp/Satorio/Crystal Bowls.flac",
            "Crystal Bowls",
        );
        t.artist = "Satorio".into();
        t.duration = 411.0;
        lib.add_purchase(&t, "Bandcamp").unwrap();
        let left = lib.wanted().unwrap();
        assert_eq!(left.len(), 1);
        assert_ne!(left[0].id, spotify);
        assert!(left[0].pending_store.is_empty());
    }

    #[test]
    fn being_bought_runs_out() {
        let lib = Library::in_memory().unwrap();
        lib.add_wanted("Spotify", "A", "B", "https://open.spotify.com/track/1")
            .unwrap();
        let id = lib.wanted().unwrap()[0].id;
        lib.mark_pending(&[id], "juno").unwrap();
        lib.db
            .execute(
                "UPDATE wanted SET pending_since = ?1",
                [now() - (PENDING_DAYS + 1) * 86_400],
            )
            .unwrap();
        let w = lib.wanted().unwrap();
        assert_eq!(w.len(), 1);
        assert!(w[0].pending_store.is_empty());
    }

    #[test]
    fn purchases_match_wanted_and_dedupe() {
        let mut lib = Library::in_memory().unwrap();
        lib.add_wanted(
            "Bandcamp",
            "Bonobo",
            "Kerala",
            "https://b.bandcamp.com/track/kerala",
        )
        .unwrap();
        lib.add_wanted(
            "Bandcamp",
            "Bonobo",
            "Kerala",
            "https://b.bandcamp.com/track/kerala",
        )
        .unwrap();
        lib.add_wanted(
            "Bandcamp",
            "Bicep",
            "Glue",
            "https://bicep.bandcamp.com/track/glue",
        )
        .unwrap();
        assert_eq!(lib.wanted().unwrap().len(), 2);

        let mut t = file_track(
            "/m/Purchased/Bandcamp/Bonobo/Migration/03 Kerala.flac",
            "Kerala",
        );
        t.artist = "BONOBO".into();
        t.duration = 300.0;
        let added = lib.add_purchase(&t, "Bandcamp").unwrap();
        assert!(added.id >= ID_BASE);
        let left = lib.wanted().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].title, "Glue");
        let url: Option<String> = lib
            .db
            .query_row(
                "SELECT store_url FROM tracks WHERE id = ?1",
                [row_id(added.id)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(url.as_deref(), Some("https://b.bandcamp.com/track/kerala"));

        assert_eq!(
            lib.find_same("Bonobo", "kerala", 301.0).unwrap(),
            Some(added.id)
        );
        assert_eq!(lib.find_same("Bonobo", "Kerala", 200.0).unwrap(), None);
        assert_eq!(lib.find_same("Other", "Kerala", 300.0).unwrap(), None);

        let inbox = lib.inbox_list().unwrap();
        assert_eq!(lib.inbox_list().unwrap(), inbox);
        lib.delete_playlist(inbox).unwrap();
        let again = lib.inbox_list().unwrap();
        assert!(lib
            .playlists()
            .unwrap()
            .iter()
            .any(|p| p.id == again && p.name == "Inbox"));

        let p = Path::new("/dl/a.flac");
        assert!(!lib.inbox_seen(p).unwrap());
        lib.log_inbox(p, "imported", Some(added.id)).unwrap();
        assert!(lib.inbox_seen(p).unwrap());
    }
}
