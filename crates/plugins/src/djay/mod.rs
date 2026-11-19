//! Brings a djay Pro library into the collection at start, when the DJ turns
//! it on in Settings › Collection › Import: each track's tags, BPM, key,
//! hot cues 1 to 8 with their names, saved loops, Automix points, rating,
//! play count, color and grid anchor; djay's playlists in their folders;
//! and its history. Streamed tracks (TIDAL, Spotify, Beatport, SoundCloud)
//! stand in djay's lists as placeholders. It reads a copy of djay's
//! database and never writes djay's own.
//!
//! djay keeps its library in `MediaLibrary.db`, a YapDatabase on SQLite:
//! table `database2` holds one row per record in each `collection`, keyed
//! by the record's uuid; a track's records share its key. Each row's `data`
//! is an object, in one of two formats: Apple's keyed archive (`bplist00`,
//! [`keyed`]) in records djay wrote before about 2024, TSAF ([`tsaf`]) in
//! newer ones. A library holds both.
//!
//! - `mediaItemTitleIDs`: `title`, `artist`, `duration`, `isrc`.
//! - `mediaItems`: `albumUUID`, `genreUUIDs` (names in `mediaAlbums` and
//!   `mediaGenres`), `year`, `comments`, `composer`, `grouping`,
//!   `addedDate`.
//! - `localMediaItemLocations`: `sourceURIs` (a `file://` URL) and
//!   `urlBookmarkData` (a macOS bookmark, [`bookmark`]);
//!   `globalMediaItemLocations`: a streamed track's `tidal:track:123`.
//! - `mediaItemAnalyzedData`: `bpm`, `keySignatureIndex`.
//! - `mediaItemUserData`: `cuePoints` (`ADCCuePoint`: `number` 1 to 8,
//!   `time`, `endTime` (-1 unless a loop), `comment`), `loopRegions`
//!   (`ADCLoopRegion`: `startTime`, `endTime`, `comment`),
//!   `automixStartPoint` and `automixEndPoint` (cue points), `rating`,
//!   `playCount`, `colorIndex`, `manualBPM`, `beatGridEdits`.
//! - `mediaItemPlaylists`: `name`, `type` (0 playlist, 1 folder, 2 smart),
//!   `childUUIDs`, `itemUUIDs`; the top is `mediaItemPlaylist-root`. Each
//!   item is a `mediaItemPlaylistItems` record naming its `mediaItemUUID`.
//! - `historySessions`: `startDate`, `itemUUIDs`; each a
//!   `historySessionItems` record with `startTime` and the track's
//!   `titleID`. Dates count seconds from 2001.
//!
//! Prior work: EngineX-OS's `pkg/djay` (MIT, Copyright (c) 2026 EngineX OS
//! Contributors) set out what to bring across; the TSAF layout follows
//! djay-pro-autohotcue (MIT, Copyright (c) 2026 payne0420) and tsaf-parser
//! (MIT, Copyright (c) 2026 Arlo O'Keeffe), and whats-now-playing's djay Pro
//! input (MIT, Copyright (c) 2021-2026 Allen Wittenauer) found the key
//! order. The rest was checked against a real djay Pro library.

pub mod bookmark;
pub mod keyed;
pub mod tsaf;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::import::{ImportedEntry, ImportedList, ImportedListKind};
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, MEMORY_CUE};
use rusqlite::{Connection, OpenFlags};

use crate::{ImportedLibrary, LibraryImport, ListGroup};
use keyed::APPLE_EPOCH;
use tsaf::{Obj, Value};

pub const PLAYLISTS: &str = "playlists";
pub const HISTORY: &str = "history";

/// The playlist tree's top, as djay keys it.
const ROOT: &str = "mediaItemPlaylist-root";

pub struct Djay;

/// djay Pro's library where djay keeps it (or `DJSUGAR_DJAY_LIBRARY`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_DJAY_LIBRARY") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;
    [
        "Music/djay/djay Media Library.djayMediaLibrary/MediaLibrary.db",
        "Music/djay/djay Media Library/MediaLibrary.db",
    ]
    .iter()
    .map(|p| home.join(p))
    .find(|p| p.is_file())
}

impl LibraryImport for Djay {
    fn id(&self) -> &str {
        "djay"
    }

    fn title(&self) -> &str {
        "djay Pro"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: PLAYLISTS,
                title: "Playlists",
                detail: "Your djay playlists, in their folders.",
                on_by_default: true,
            },
            ListGroup {
                id: HISTORY,
                title: "History",
                detail: "Every session you played in djay, in the History tab.",
                on_by_default: true,
            },
        ]
    }

    fn stamp(&self, database: &Path) -> String {
        crate::mixxx::sqlite_stamp(database)
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        let lib = Library::load(database)?;
        let tracks = lib
            .order
            .iter()
            .filter(|k| lib.items[*k].local.as_ref().and_then(location).is_some())
            .count();
        let streamed = lib
            .order
            .iter()
            .filter(|k| {
                let i = &lib.items[*k];
                i.local.as_ref().and_then(location).is_none()
                    && i.global.as_ref().and_then(stream_uri).is_some()
            })
            .count();
        let lists = lib.playlists(&HashMap::new()).len();
        let n = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        Ok(format!(
            "{} ({} streamed), {} and {}.",
            n(tracks + streamed, "track", "tracks"),
            streamed,
            n(lists, "playlist", "playlists"),
            n(
                lib.sessions.len(),
                "session of history",
                "sessions of history"
            ),
        ))
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = read(database, groups, Path::exists)?;
        eprintln!(
            "djay library: {} tracks here, {} lists",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}

/// A copy of djay's database in a folder of its own, removed when dropped.
/// djay keeps writing its library while it runs; reading a copy (with its
/// write-ahead log) never gets in its way or touches its files.
struct Copy {
    dir: PathBuf,
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn open(path: &Path) -> Result<(Connection, Copy), String> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "djsugar-djay-{}-{}",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let copy = Copy { dir };
    let to = copy.dir.join("MediaLibrary.db");
    let side = |p: &Path, s: &str| {
        let mut o = p.as_os_str().to_owned();
        o.push(s);
        PathBuf::from(o)
    };
    std::fs::copy(path, &to).map_err(|e| format!("can't read djay's library: {e}"))?;
    for s in ["-wal", "-shm"] {
        if side(path, s).is_file() {
            let _ = std::fs::copy(side(path, s), side(&to, s));
        }
    }
    let db = Connection::open_with_flags(&to, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .and_then(|db| {
            db.query_row("SELECT COUNT(*) FROM database2", [], |r| r.get::<_, i64>(0))?;
            Ok(db)
        })
        .map_err(|e| format!("can't read djay's library: {e}"))?;
    Ok((db, copy))
}

/// A record, in whichever format djay wrote it.
fn decode(data: &[u8]) -> Option<Obj> {
    let v = if data.starts_with(b"TSAF") {
        tsaf::parse(data).ok()?
    } else if data.starts_with(b"bplist") {
        keyed::parse(data).ok()?
    } else {
        return None;
    };
    match v {
        Value::Obj(o) => Some(o),
        _ => None,
    }
}

/// What djay holds about one track, by collection.
#[derive(Default)]
struct Item {
    title: Option<Obj>,
    media: Option<Obj>,
    local: Option<Obj>,
    global: Option<Obj>,
    analyzed: Option<Obj>,
    user: Option<Obj>,
}

const COLLECTIONS: [&str; 13] = [
    "mediaItemTitleIDs",
    "mediaItems",
    "localMediaItemLocations",
    "globalMediaItemLocations",
    "mediaItemAnalyzedData",
    "mediaItemUserData",
    "mediaAlbums",
    "mediaGenres",
    "mediaItemPlaylists",
    "mediaItemPlaylistItems",
    "historySessions",
    "historySessionItems",
    "mediaArtists",
];

/// The records the importer reads, by collection and key.
#[derive(Default)]
struct Library {
    items: HashMap<String, Item>,
    /// Track keys in the order djay added them.
    order: Vec<String>,
    names: HashMap<String, String>,
    playlists: HashMap<String, Obj>,
    playlist_items: HashMap<String, Obj>,
    sessions: Vec<Obj>,
    session_items: HashMap<String, Obj>,
}

impl Library {
    fn load(path: &Path) -> Result<Self, String> {
        let (db, _copy) = open(path)?;
        let mut lib = Library::default();
        let marks = (1..=COLLECTIONS.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = db
            .prepare(&format!(
                "SELECT collection, key, data FROM database2 \
                 WHERE collection IN ({marks}) ORDER BY rowid"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(COLLECTIONS, |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (collection, key, data) = row.map_err(|e| e.to_string())?;
            // A record this reader can't follow is left out rather than
            // stopping the import.
            let Some(obj) = data.as_deref().and_then(decode) else {
                continue;
            };
            let slot = match collection.as_str() {
                "mediaAlbums" | "mediaGenres" | "mediaArtists" => {
                    let name = obj.text("name");
                    if !name.is_empty() {
                        lib.names.insert(key, name);
                    }
                    continue;
                }
                "mediaItemPlaylists" => {
                    lib.playlists.insert(key, obj);
                    continue;
                }
                "mediaItemPlaylistItems" => {
                    lib.playlist_items.insert(key, obj);
                    continue;
                }
                "historySessions" => {
                    lib.sessions.push(obj);
                    continue;
                }
                "historySessionItems" => {
                    lib.session_items.insert(key, obj);
                    continue;
                }
                _ => {
                    if !lib.items.contains_key(&key) {
                        lib.order.push(key.clone());
                    }
                    lib.items.entry(key).or_default()
                }
            };
            match collection.as_str() {
                "mediaItemTitleIDs" => slot.title = Some(obj),
                "mediaItems" => slot.media = Some(obj),
                "localMediaItemLocations" => slot.local = Some(obj),
                "globalMediaItemLocations" => slot.global = Some(obj),
                "mediaItemAnalyzedData" => slot.analyzed = Some(obj),
                _ => slot.user = Some(obj),
            }
        }
        Ok(lib)
    }

    /// How a list names track `key`: its file, or the streamed track it
    /// stands for.
    fn entry(&self, key: &str, files: &HashMap<String, PathBuf>) -> Option<ImportedEntry> {
        if let Some(f) = files.get(key) {
            return Some(ImportedEntry::new(f.clone(), None));
        }
        let item = self.items.get(key)?;
        let uri = item.global.as_ref().and_then(stream_uri)?;
        let mut entry = ImportedEntry::new(uri, None);
        let title = title_id(item);
        if let (Some(s), Some(t)) = (entry.streamed.as_mut(), title) {
            s.artist = t.text("artist");
            s.title = t.text("title");
            s.duration = t.number("duration").unwrap_or(0.0).max(0.0);
            s.isrc = t.text("isrc");
        }
        entry.streamed.is_some().then_some(entry)
    }

    /// djay's playlists, folders within folders as djay shows them, each in
    /// djay's order. Smart playlists stay out: djay doesn't keep their rules
    /// in the library.
    fn playlists(&self, files: &HashMap<String, PathBuf>) -> Vec<ImportedList> {
        let by_uuid: HashMap<String, &Obj> = self
            .playlists
            .iter()
            .map(|(k, o)| {
                let uuid = Some(o.text("uuid")).filter(|u| !u.is_empty());
                (uuid.unwrap_or_else(|| k.clone()), o)
            })
            .collect();
        let mut out = Vec::new();
        match by_uuid.get(ROOT) {
            Some(root) => self.walk(root, &by_uuid, &mut Vec::new(), files, &mut out, 0),
            // No tree: every playlist at the top.
            None => {
                let mut lists: Vec<&&Obj> = by_uuid
                    .values()
                    .filter(|o| o.number("type").unwrap_or(0.0) == 0.0)
                    .collect();
                lists.sort_by_key(|o| o.text("name").to_lowercase());
                for o in lists {
                    out.push(self.list(o, Vec::new(), files));
                }
            }
        }
        out
    }

    fn walk(
        &self,
        folder: &Obj,
        by_uuid: &HashMap<String, &Obj>,
        path: &mut Vec<String>,
        files: &HashMap<String, PathBuf>,
        out: &mut Vec<ImportedList>,
        depth: usize,
    ) {
        if depth > 32 {
            return;
        }
        let children = folder
            .get("childUUIDs")
            .map(Value::items)
            .unwrap_or_default();
        for child in children.iter().filter_map(Value::as_str) {
            let Some(o) = by_uuid.get(child) else {
                continue;
            };
            match o.number("type").unwrap_or(0.0) as i64 {
                0 => out.push(self.list(o, path.clone(), files)),
                1 => {
                    path.push(o.text("name"));
                    self.walk(o, by_uuid, path, files, out, depth + 1);
                    path.pop();
                }
                _ => {}
            }
        }
    }

    fn list(
        &self,
        o: &Obj,
        folders: Vec<String>,
        files: &HashMap<String, PathBuf>,
    ) -> ImportedList {
        let entries = o
            .get("itemUUIDs")
            .map(Value::items)
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|i| self.playlist_items.get(i))
            .filter_map(|i| self.entry(&i.text("mediaItemUUID"), files))
            .collect();
        ImportedList {
            folders,
            name: o.text("name"),
            kind: ImportedListKind::List,
            entries,
        }
    }

    /// Each history session, oldest first, named by its day.
    fn history(&self, files: &HashMap<String, PathBuf>) -> Vec<ImportedList> {
        let mut out: Vec<ImportedList> = self
            .sessions
            .iter()
            .filter_map(|s| {
                let entries: Vec<ImportedEntry> = s
                    .get("itemUUIDs")
                    .map(Value::items)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(|i| self.session_items.get(i))
                    .filter_map(|i| {
                        let key = i
                            .get("titleID")
                            .and_then(Value::as_obj)
                            .map(|t| t.text("uuid"))?;
                        let mut e = self.entry(&key, files)?;
                        e.played_at = apple_time(i.number("startTime"));
                        Some(e)
                    })
                    .collect();
                if entries.is_empty() {
                    return None;
                }
                let started = apple_time(s.number("startDate"))
                    .or_else(|| entries.iter().filter_map(|e| e.played_at).min())?;
                let name = Some(s.text("name"))
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| day_name(started));
                Some(ImportedList {
                    folders: Vec::new(),
                    name,
                    kind: ImportedListKind::History { started },
                    entries,
                })
            })
            .collect();
        out.sort_by_key(|l| match l.kind {
            ImportedListKind::History { started } => started,
            _ => 0,
        });
        out
    }
}

/// A djay date (seconds from 2001) as a Unix time.
fn apple_time(seconds: Option<f64>) -> Option<i64> {
    let s = seconds.filter(|s| s.is_finite() && *s > 0.0)?;
    Some((s + APPLE_EPOCH) as i64)
}

/// "2026-11-10", in UTC.
fn day_name(unix: i64) -> String {
    // Howard Hinnant's days-to-civil.
    let z = unix.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The tracks whose files `exists` says are here, and the lists in `groups`.
pub fn read(
    path: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let lib = Library::load(path)?;
    let mut files: HashMap<String, PathBuf> = HashMap::new();
    let mut tracks = Vec::new();
    for key in &lib.order {
        let item = &lib.items[key];
        let Some(file) = item.local.as_ref().and_then(location) else {
            continue;
        };
        if exists(&file) {
            tracks.push(track(&lib, item, file.clone()));
        }
        files.insert(key.clone(), file);
    }
    let mut lists = Vec::new();
    if groups.contains(&PLAYLISTS) {
        lists.extend(lib.playlists(&files));
    }
    if groups.contains(&HISTORY) {
        lists.extend(lib.history(&files));
    }
    Ok(ImportedLibrary {
        tracks,
        lists,
        written: None,
    })
}

/// A streamed track's address in its location ("tidal:track:123").
fn stream_uri(obj: &Obj) -> Option<String> {
    fn find(v: &Value) -> Option<String> {
        match v {
            Value::Url(s) | Value::Str(s) => {
                djsugar_engine::import::StreamedTrack::parse(s).map(|_| s.clone())
            }
            Value::Arr(items) => items.iter().find_map(find),
            _ => None,
        }
    }
    obj.get("sourceURIs").and_then(find)
}

fn location(obj: &Obj) -> Option<PathBuf> {
    fn find(v: &Value) -> Option<PathBuf> {
        match v {
            Value::Url(s) | Value::Str(s) => file_url(s),
            Value::Data(d) => bookmark::path(d),
            Value::Arr(items) => items.iter().find_map(find),
            _ => None,
        }
    }
    ["sourceURIs", "urlBookmarkData"]
        .iter()
        .filter_map(|n| obj.get(n))
        .chain(obj.fields.iter().map(|(_, v)| v))
        .find_map(find)
}

/// The path in a `file://` URL, its %-escapes decoded.
fn file_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    // "file://localhost/Users/…" and "file:///Users/…".
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(b) = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let path = String::from_utf8(out).ok()?;
    // Windows: "/C:/Music/a.mp3".
    let path = match path.as_bytes() {
        [b'/', d, b':', ..] if d.is_ascii_alphabetic() => path[1..].to_string(),
        _ => path,
    };
    (path.len() > 1).then(|| PathBuf::from(path))
}

/// djay's `keySignatureIndex`: even numbers the major keys from D flat up
/// by semitones, each odd one the minor key of the major before it.
fn key_name(index: f64) -> Option<String> {
    const NOTES: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];
    if !(0.0..24.0).contains(&index) || index.fract() != 0.0 {
        return None;
    }
    let i = index as usize;
    let major = (1 + i / 2) % 12;
    Some(if i % 2 == 1 {
        format!("{}m", NOTES[(major + 9) % 12])
    } else {
        NOTES[major].to_string()
    })
}

/// djay's colors by `colorIndex`, for tracks and cues. Not yet checked
/// against a real library: the order follows djay's color menu.
fn color(index: f64) -> Option<u32> {
    match index as i64 {
        1 => Some(0xE5_3935), // red
        2 => Some(0xFB_8C00), // orange
        3 => Some(0xFD_D835), // yellow
        4 => Some(0x43_A047), // green
        5 => Some(0x00_ACC1), // cyan
        6 => Some(0x1E_88E5), // blue
        7 => Some(0x8E_24AA), // purple
        8 => Some(0xD8_1B60), // pink
        _ => None,
    }
}

/// The track's title ID: its own record, or the copy inside another.
fn title_id(item: &Item) -> Option<&Obj> {
    item.title.as_ref().or_else(|| {
        [&item.media, &item.local, &item.analyzed, &item.user]
            .into_iter()
            .flatten()
            .find_map(|o| match o.get("titleID").or_else(|| o.get("titleIDs"))? {
                Value::Obj(t) => Some(t),
                v => v.items().iter().find_map(Value::as_obj),
            })
    })
}

/// A cue point as a hot cue: its pad (from 0) when it has one, its time,
/// where its loop ends (if it is one) and its name. djay numbers pads 1 to
/// 8; a time of -1 is a pad with no place yet.
fn cue(c: &Obj, start: &str) -> Option<(Option<usize>, Hotcue)> {
    let time = c.number(start).filter(|t| t.is_finite() && *t >= 0.0)?;
    let end = c.number("endTime").filter(|e| e.is_finite() && *e > time);
    let pad = c
        .number("number")
        .filter(|n| (1.0..=8.0).contains(n))
        .map(|n| n as usize - 1);
    let hotcue = Hotcue {
        number: 0,
        seconds: time,
        name: c.text("comment"),
        end,
        color: c.number("colorIndex").and_then(color),
    };
    Some((pad, hotcue))
}

/// Whether the DJ set `field` in djay, rather than it being djay's default.
fn changed(user: &Obj, field: &str) -> bool {
    user.get("userChangedCloudKeys")
        .map(Value::items)
        .unwrap_or_default()
        .iter()
        .any(|k| k.as_str() == Some(field))
}

fn track(lib: &Library, item: &Item, file: PathBuf) -> Track {
    let empty = Obj::default();
    let title = title_id(item).unwrap_or(&empty);
    let media = item.media.as_ref().unwrap_or(&empty);
    let analyzed = item.analyzed.as_ref().unwrap_or(&empty);
    let user = item.user.as_ref().unwrap_or(&empty);

    let mut hotcues: Vec<Hotcue> = Vec::new();
    let mut memory = MEMORY_CUE;
    let mut add = |pad: Option<usize>, mut h: Hotcue, hotcues: &mut Vec<Hotcue>| {
        h.number = match pad {
            Some(p) if !hotcues.iter().any(|x| x.number == p) => p,
            _ => {
                memory += 1;
                memory - 1
            }
        };
        hotcues.push(h);
    };
    for c in user
        .get("cuePoints")
        .map(Value::items)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_obj)
    {
        if let Some((pad, h)) = cue(c, "time") {
            add(pad, h, &mut hotcues);
        }
    }
    // Saved loops have pads of their own in djay; here they are memory
    // cues with an end.
    for l in user
        .get("loopRegions")
        .map(Value::items)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_obj)
    {
        if let Some((_, h)) = cue(l, "startTime").filter(|(_, h)| h.end.is_some()) {
            add(None, h, &mut hotcues);
        }
    }
    hotcues.sort_by_key(|h| h.number);

    let mut markers = Vec::new();
    for (field, name) in [
        ("automixStartPoint", "realStart"),
        ("automixEndPoint", "fadeStart"),
    ] {
        let seconds = match user.get(field) {
            Some(Value::Obj(o)) => o.number("time"),
            Some(v) => v.as_f64(),
            None => None,
        };
        if let Some(seconds) = seconds.filter(|s| s.is_finite() && *s >= 0.0) {
            markers.push(Marker {
                kind: MarkerKind::Automix,
                seconds,
                end: None,
                name: name.into(),
                color: None,
                detected: false,
            });
        }
    }

    let positive = |v: Option<f64>| v.filter(|b| b.is_finite() && *b > 0.0);
    let bpm = positive(user.number("manualBPM"))
        .or_else(|| positive(analyzed.number("bpm")))
        .or_else(|| positive(media.number("bpm")))
        .unwrap_or(0.0);
    // A key index of 0 (D flat) is real, so a key is only djay's when the
    // field is there; the DJ's own only when they changed it.
    let key = changed(user, "manualKeySignatureIndex")
        .then(|| user.number("manualKeySignatureIndex"))
        .flatten()
        .or_else(|| analyzed.number("keySignatureIndex"))
        .or_else(|| media.number("musicalKeySignatureIndex"))
        .and_then(key_name)
        .unwrap_or_default();
    let anchor = user
        .get("beatGridEdits")
        .and_then(Value::as_obj)
        .and_then(|g| g.number("firstDownbeatPosition"))
        .filter(|s| s.is_finite() && *s >= 0.0);
    let name_of = |v: Option<&Value>| -> String {
        let id = match v {
            Some(Value::Arr(items)) => items.first().and_then(Value::as_str),
            Some(v) => v.as_str(),
            None => None,
        };
        id.and_then(|id| lib.names.get(id))
            .cloned()
            .unwrap_or_default()
    };
    let artist = Some(title.text("artist"))
        .filter(|a| !a.is_empty())
        .unwrap_or_else(|| name_of(media.get("artistUUIDs")));
    let mut t = Track {
        id: 0,
        artist,
        title: Some(title.text("title"))
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| media.text("title")),
        album: name_of(media.get("albumUUID")),
        genre: name_of(media.get("genreUUIDs")),
        rating: user.number("rating").unwrap_or(0.0).clamp(0.0, 5.0) as u8,
        bpm,
        key,
        duration: positive(title.number("duration"))
            .or_else(|| positive(media.number("duration")))
            .unwrap_or(0.0),
        first_beat: anchor.unwrap_or(0.0),
        grid: Default::default(),
        source: Source::File(file),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo {
            comment: media.text("comments"),
            grouping: media.text("grouping"),
            composer: media.text("composer"),
            year: positive(media.number("year"))
                .map(|y| format!("{}", y as i64))
                .unwrap_or_default(),
            added: apple_time(media.number("addedDate")).unwrap_or(0),
            color: user.number("colorIndex").and_then(color),
            play_count: user.number("playCount").unwrap_or(0.0).max(0.0) as u32,
            hotcues,
            isrc: title.text("isrc"),
            fixed_grid: anchor.is_some() && bpm > 0.0,
            markers,
            ..Default::default()
        },
    };
    if let (Some(seconds), true) = (anchor, bpm > 0.0) {
        t.set_grid(BeatGrid::new(vec![GridMarker { seconds, bpm }]));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::tsaf::write::{obj, Writer};
    use super::*;

    fn title_id(uuid: &str, title: &str, artist: &str, duration: f32) -> Value {
        obj(
            "ADCMediaItemTitleID",
            vec![
                ("uuid", Value::Str(uuid.into())),
                ("title", Value::Str(title.into())),
                ("artist", Value::Str(artist.into())),
                ("duration", Value::F32(duration)),
                ("isrc", Value::Str(format!("ISRC{title}"))),
            ],
        )
    }

    fn cue(number: i64, time: f32, comment: &str) -> Value {
        let mut f = vec![("number", Value::Int(number)), ("time", Value::F32(time))];
        f.push(("endTime", Value::F32(-1.0)));
        if !comment.is_empty() {
            f.push(("comment", Value::Str(comment.into())));
        }
        obj("ADCCuePoint", f)
    }

    fn strs(items: &[&str]) -> Value {
        Value::Arr(items.iter().map(|s| Value::Str(s.to_string())).collect())
    }

    /// A small djay library, its records in both formats: a local track
    /// with cues by file URL (keyed archive), one by bookmark (TSAF), a
    /// TIDAL track, playlists in a folder, a smart playlist and a session
    /// of history.
    fn library(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("djsugar-djay-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE database2 (rowid INTEGER PRIMARY KEY, collection CHAR NOT NULL, \
               key CHAR NOT NULL, data BLOB, metadata BLOB);",
        )
        .unwrap();
        let put = |collection: &str, key: &str, v: Value, old: bool| {
            let blob = if old {
                keyed::write(&v)
            } else {
                Writer::blob(&v)
            };
            db.execute(
                "INSERT INTO database2 (collection, key, data) VALUES (?1, ?2, ?3)",
                rusqlite::params![collection, key, blob],
            )
            .unwrap();
        };
        let a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let s = "cccccccccccccccccccccccccccccccc";
        put(
            "mediaItemTitleIDs",
            a,
            title_id(a, "Glue", "Bicep", 269.0),
            true,
        );
        put(
            "mediaItemTitleIDs",
            s,
            title_id(s, "Streamed", "X", 200.0),
            false,
        );
        put(
            "mediaItems",
            a,
            obj(
                "ADCMediaItem",
                vec![
                    ("title", Value::Str("Glue".into())),
                    ("albumUUID", Value::Str("alb".into())),
                    ("genreUUIDs", strs(&["gen"])),
                    ("year", Value::Int(2017)),
                    ("addedDate", Value::F64(600_000_000.0)),
                    ("comments", Value::Str("nice".into())),
                ],
            ),
            true,
        );
        put(
            "mediaAlbums",
            "alb",
            obj("ADCMediaAlbum", vec![("name", Value::Str("Bicep".into()))]),
            true,
        );
        put(
            "mediaGenres",
            "gen",
            obj(
                "ADCMediaGenre",
                vec![("name", Value::Str("Electronic".into()))],
            ),
            true,
        );
        put(
            "localMediaItemLocations",
            a,
            obj(
                "ADCMediaItemLocation",
                vec![
                    ("uuid", Value::Str(a.into())),
                    ("type", Value::Int(0)),
                    (
                        "sourceURIs",
                        Value::Arr(vec![Value::Url(
                            "file:///Users/dj/Music/Bicep%20-%20Glue.mp3".into(),
                        )]),
                    ),
                ],
            ),
            true,
        );
        put(
            "localMediaItemLocations",
            b,
            obj(
                "ADCMediaItemLocation",
                vec![
                    ("uuid", Value::Str(b.into())),
                    (
                        "titleIDs",
                        Value::Arr(vec![title_id(b, "Just", "Bicep", 372.5)]),
                    ),
                    (
                        "sourceURIs",
                        Value::Arr(vec![Value::Url("com.apple.iTunes:123".into())]),
                    ),
                    (
                        "urlBookmarkData",
                        Value::Data(bookmark::make(&["Volumes", "USB", "Just.mp3"])),
                    ),
                ],
            ),
            false,
        );
        put(
            "globalMediaItemLocations",
            s,
            obj(
                "ADCMediaItemLocation",
                vec![
                    ("type", Value::Int(1)),
                    (
                        "sourceURIs",
                        Value::Arr(vec![Value::Url("tidal:track:149747167".into())]),
                    ),
                ],
            ),
            true,
        );
        put(
            "mediaItemAnalyzedData",
            a,
            obj(
                "ADCMediaItemAnalyzedData",
                vec![
                    ("bpm", Value::F32(120.0)),
                    ("keySignatureIndex", Value::Int(15)),
                ],
            ),
            false,
        );
        // djay's own key 0 is D flat, not "no key".
        put(
            "mediaItemAnalyzedData",
            b,
            obj(
                "ADCMediaItemAnalyzedData",
                vec![
                    ("bpm", Value::F32(122.0)),
                    ("keySignatureIndex", Value::Int(0)),
                ],
            ),
            false,
        );
        let automix = |t: f32| cue(0, t, "");
        put(
            "mediaItemUserData",
            a,
            obj(
                "ADCMediaItemUserData",
                vec![
                    ("uuid", Value::Str(a.into())),
                    ("userChangedCloudKeys", strs(&["cuePoints", "playCount"])),
                    (
                        "cuePoints",
                        Value::Arr(vec![
                            cue(1, 0.5, "First Beat"),
                            cue(2, 96.0, "Drop"),
                            cue(4, -1.0, ""),
                            cue(6, 2.0, ""),
                        ]),
                    ),
                    ("automixStartPoint", automix(16.0)),
                    ("automixEndPoint", automix(240.0)),
                    ("playCount", Value::Int(7)),
                    ("rating", Value::Int(4)),
                    ("manualBPM", Value::F64(0.0)),
                    ("manualKeySignatureIndex", Value::Int(0)),
                ],
            ),
            true,
        );
        put(
            "mediaItemUserData",
            b,
            obj(
                "ADCMediaItemUserData",
                vec![
                    ("cuePoints", Value::Arr(vec![cue(1, 1.0, "")])),
                    (
                        "loopRegions",
                        Value::Arr(vec![obj(
                            "ADCLoopRegion",
                            vec![
                                ("startTime", Value::F32(14.0)),
                                ("endTime", Value::F32(16.0)),
                                ("comment", Value::Str("saved loop 1".into())),
                                ("number", Value::Int(1)),
                            ],
                        )]),
                    ),
                    (
                        "beatGridEdits",
                        obj(
                            "ADCBeatGridEdits",
                            vec![("firstDownbeatPosition", Value::F32(0.25))],
                        ),
                    ),
                ],
            ),
            false,
        );
        // A record this reader can't follow is skipped.
        db.execute(
            "INSERT INTO database2 (collection, key, data) VALUES ('mediaItemUserData', 'x', X'545341460000')",
            [],
        )
        .unwrap();
        // Playlists: root › Gigs › Friday (b, streamed, a), root › Warmup (a),
        // and a smart playlist.
        let item = |k: &str, track: &str| {
            put(
                "mediaItemPlaylistItems",
                k,
                obj(
                    "ADCMediaItemPlaylistItem",
                    vec![("mediaItemUUID", Value::Str(track.into()))],
                ),
                k.ends_with('1'),
            )
        };
        item("i1", b);
        item("i2", s);
        item("i3", a);
        item("i4", a);
        let list = |uuid: &str, name: &str, kind: i64, children: &[&str], items: &[&str]| {
            let mut f = vec![
                ("uuid", Value::Str(uuid.into())),
                ("name", Value::Str(name.into())),
                ("type", Value::Int(kind)),
            ];
            if !children.is_empty() {
                f.push(("childUUIDs", strs(children)));
            }
            if !items.is_empty() {
                f.push(("itemUUIDs", strs(items)));
            }
            obj("ADCMediaItemPlaylist", f)
        };
        put(
            "mediaItemPlaylists",
            ROOT,
            list(ROOT, "Playlists", 1, &["G", "W", "S"], &[]),
            false,
        );
        put(
            "mediaItemPlaylists",
            "G",
            list("G", "Gigs", 1, &["F"], &[]),
            true,
        );
        put(
            "mediaItemPlaylists",
            "F",
            list("F", "Friday", 0, &[], &["i1", "i2", "i3"]),
            true,
        );
        put(
            "mediaItemPlaylists",
            "W",
            list("W", "Warmup", 0, &[], &["i4"]),
            false,
        );
        put(
            "mediaItemPlaylists",
            "S",
            list("S", "Smart", 2, &[], &[]),
            true,
        );
        put(
            "historySessions",
            "h",
            obj(
                "ADCHistorySession",
                vec![
                    ("startDate", Value::F64(628_386_655.7)),
                    ("itemUUIDs", strs(&["h1", "h2"])),
                ],
            ),
            true,
        );
        for (k, track, at) in [("h1", a, 628_386_700.0), ("h2", s, 628_386_900.0)] {
            put(
                "historySessionItems",
                k,
                obj(
                    "ADCHistorySessionItem",
                    vec![
                        ("deckNumber", Value::Int(2)),
                        ("startTime", Value::F64(at)),
                        ("titleID", title_id(track, "t", "a", 1.0)),
                    ],
                ),
                true,
            );
        }
        path
    }

    #[test]
    fn reads_a_djay_library() {
        let path = library("read");
        let lib = read(&path, &[PLAYLISTS, HISTORY], |_| true).unwrap();
        assert_eq!(
            Djay.preview(&path).unwrap(),
            "3 tracks (1 streamed), 2 playlists and 1 session of history."
        );
        // djay's own files are left as they were.
        assert!(!Path::new(&format!("{}-wal", path.display())).exists());
        std::fs::remove_file(&path).ok();

        assert_eq!(lib.tracks.len(), 2);
        let t = &lib.tracks[0];
        assert!(
            matches!(&t.source, Source::File(p) if p == Path::new("/Users/dj/Music/Bicep - Glue.mp3"))
        );
        assert_eq!(
            (t.artist.as_str(), t.title.as_str(), t.album.as_str()),
            ("Bicep", "Glue", "Bicep")
        );
        assert_eq!(
            (
                t.genre.as_str(),
                t.info.year.as_str(),
                t.info.comment.as_str()
            ),
            ("Electronic", "2017", "nice")
        );
        assert_eq!(
            (t.bpm, t.key.as_str(), t.duration, t.rating),
            (120.0, "Fm", 269.0, 4)
        );
        assert_eq!(t.info.added, 600_000_000 + 978_307_200);
        assert_eq!((t.info.play_count, t.info.isrc.as_str()), (7, "ISRCGlue"));
        // Pads 1, 2 and 6; pad 4 had no place yet.
        let pads: Vec<(usize, &str)> = t
            .info
            .hotcues
            .iter()
            .map(|h| (h.number, h.name.as_str()))
            .collect();
        assert_eq!(pads, [(0, "First Beat"), (1, "Drop"), (5, "")]);
        let automix: Vec<(&str, f64)> = t
            .info
            .markers
            .iter()
            .map(|m| (m.name.as_str(), m.seconds))
            .collect();
        assert_eq!(automix, [("realStart", 16.0), ("fadeStart", 240.0)]);
        assert!(!t.info.fixed_grid);

        // Its file from a bookmark, its title from the location's copy.
        let t = &lib.tracks[1];
        assert_eq!(t.title, "Just");
        assert!(matches!(&t.source, Source::File(p) if p == Path::new("/Volumes/USB/Just.mp3")));
        assert_eq!(t.key, "Db");
        assert!(t.info.fixed_grid);
        assert_eq!(t.first_beat, 0.25);
        let pads: Vec<(usize, &str, Option<f64>)> = t
            .info
            .hotcues
            .iter()
            .map(|h| (h.number, h.name.as_str(), h.end))
            .collect();
        assert_eq!(
            pads,
            [(0, "", None), (MEMORY_CUE, "saved loop 1", Some(16.0))]
        );

        let names: Vec<String> = lib
            .lists
            .iter()
            .map(|l| {
                let mut p = l.folders.clone();
                p.push(l.name.clone());
                p.join("/")
            })
            .collect();
        assert_eq!(names, ["Gigs/Friday", "Warmup", "2020-11-29"]);
        let friday = &lib.lists[0].entries;
        assert_eq!(friday[0].path, Path::new("/Volumes/USB/Just.mp3"));
        let streamed = friday[1].streamed.as_ref().unwrap();
        assert_eq!(
            (streamed.service.as_str(), streamed.id.as_str()),
            ("tidal", "149747167")
        );
        assert_eq!(
            (streamed.title.as_str(), streamed.duration),
            ("Streamed", 200.0)
        );
        assert_eq!(
            friday[2].path,
            Path::new("/Users/dj/Music/Bicep - Glue.mp3")
        );
        let history = &lib.lists[2];
        assert!(matches!(
            history.kind,
            ImportedListKind::History {
                started: 1_606_693_855
            }
        ));
        assert_eq!(history.entries.len(), 2);
        assert_eq!(
            history.entries[0].played_at,
            Some(628_386_700 + 978_307_200)
        );
    }

    #[test]
    fn leaves_out_missing_files_and_lists() {
        let path = library("missing");
        let lib = read(&path, &[], |p| p.starts_with("/Volumes")).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(lib.tracks.len(), 1);
        assert!(lib.lists.is_empty());
    }

    #[test]
    fn keys_urls_and_days() {
        assert_eq!(key_name(0.0).as_deref(), Some("Db"));
        assert_eq!(key_name(1.0).as_deref(), Some("Bbm"));
        assert_eq!(key_name(22.0).as_deref(), Some("C"));
        assert_eq!(key_name(23.0).as_deref(), Some("Am"));
        assert_eq!(key_name(11.0).as_deref(), Some("Ebm"));
        assert_eq!(key_name(24.0), None);
        assert_eq!(
            file_url("file:///C:/Music/a%C3%A9.mp3"),
            Some(PathBuf::from("C:/Music/aé.mp3"))
        );
        assert_eq!(
            file_url("file://localhost/Users/x.mp3"),
            Some(PathBuf::from("/Users/x.mp3"))
        );
        assert_eq!(file_url("https://tidal.com/x"), None);
        assert_eq!(day_name(0), "1970-01-01");
        assert_eq!(day_name(1_606_693_855), "2020-11-29");
    }
}
