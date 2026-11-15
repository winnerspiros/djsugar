//! Brings a Mixxx library into the collection at start, when the DJ turns it
//! on in Settings > Library: the tracks in `mixxxdb.sqlite` with their tags,
//! cues, saved loops and beat grids, and its playlists, crates and history.
//! It opens the database read-only and never writes it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::import::{ImportedEntry, ImportedList, ImportedListKind};
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, MEMORY_CUE};
use rusqlite::{Connection, OpenFlags};

use crate::{ImportedLibrary, LibraryImport, ListGroup};

pub const PLAYLISTS: &str = "playlists";
pub const CRATES: &str = "crates";
pub const HISTORY: &str = "history";

pub struct Mixxx;

/// Mixxx's database where Mixxx keeps it (or `DJSUGAR_MIXXX_DATABASE`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_MIXXX_DATABASE") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;
    [
        // The Mac App Store and newer builds keep it in the sandbox.
        "Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/mixxxdb.sqlite",
        "Library/Application Support/Mixxx/mixxxdb.sqlite",
        ".mixxx/mixxxdb.sqlite",
        ".local/share/mixxx/mixxxdb.sqlite",
        "AppData/Local/Mixxx/mixxxdb.sqlite",
    ]
    .iter()
    .map(|p| home.join(p))
    .find(|p| p.is_file())
}

fn open(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("can't open Mixxx's database: {e}"))
}

/// A SQLite database's path and the time it or its write-ahead log changed,
/// whichever was last.
pub(crate) fn sqlite_stamp(database: &Path) -> String {
    let modified = |p: &Path| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs())
    };
    let mut wal = database.as_os_str().to_owned();
    wal.push("-wal");
    let newest = modified(database).max(modified(Path::new(&wal)));
    format!("{}:{newest}", database.display())
}

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

impl LibraryImport for Mixxx {
    fn id(&self) -> &str {
        "mixxx"
    }

    fn title(&self) -> &str {
        "Mixxx"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: PLAYLISTS,
                title: "Playlists",
                detail: "Your Mixxx playlists.",
                on_by_default: true,
            },
            ListGroup {
                id: CRATES,
                title: "Crates",
                detail: "Your Mixxx crates, as lists.",
                on_by_default: true,
            },
            ListGroup {
                id: HISTORY,
                title: "History",
                detail: "Every session you played in Mixxx, in the History tab.",
                on_by_default: true,
            },
        ]
    }

    fn stamp(&self, database: &Path) -> String {
        sqlite_stamp(database)
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        let db = open(database)?;
        let count = |sql: &str| -> Result<usize, String> {
            db.query_row(sql, [], |r| r.get::<_, i64>(0))
                .map(|n| n as usize)
                .map_err(err)
        };
        let tracks = count("SELECT COUNT(*) FROM library WHERE mixxx_deleted = 0")?;
        let playlists = count("SELECT COUNT(*) FROM Playlists WHERE hidden = 0")?;
        let crates = count("SELECT COUNT(*) FROM crates")?;
        let history = count("SELECT COUNT(*) FROM Playlists WHERE hidden = 2")?;
        let n = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        Ok(format!(
            "{}, {}, {} and {}.",
            n(tracks, "track", "tracks"),
            n(playlists, "playlist", "playlists"),
            n(crates, "crate", "crates"),
            n(history, "session of history", "sessions of history"),
        ))
    }

    fn lists(&self, database: &Path, groups: &[&str]) -> Result<Vec<ImportedList>, String> {
        let db = open(database)?;
        lists(&db, groups, &locations(&db)?)
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = read(database, groups, Path::exists)?;
        eprintln!(
            "Mixxx library: {} tracks here, {} lists",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}

/// Each track's file, by its id.
fn locations(db: &Connection) -> Result<HashMap<i64, PathBuf>, String> {
    let mut q = db
        .prepare(
            "SELECT l.id, t.location FROM library l JOIN track_locations t ON t.id = l.location
             WHERE l.mixxx_deleted = 0",
        )
        .map_err(err)?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(err)?;
    rows.map(|r| r.map(|(id, p)| (id, PathBuf::from(p))))
        .collect::<Result<_, _>>()
        .map_err(err)
}

/// The tracks whose files `exists` says are here, and the lists in `groups`.
pub fn read(
    path: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let db = open(path)?;
    let files = locations(&db)?;
    let mut cues: HashMap<i64, Vec<Cue>> = HashMap::new();
    {
        let mut q = db
            .prepare("SELECT track_id, type, position, length, hotcue, label, color FROM cues")
            .map_err(err)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    Cue {
                        kind: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        position: r.get::<_, Option<f64>>(2)?.unwrap_or(-1.0),
                        length: r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                        hotcue: r.get::<_, Option<i64>>(4)?.unwrap_or(-1),
                        label: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                        color: r.get::<_, Option<i64>>(6)?,
                    },
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (track, cue) = row.map_err(err)?;
            cues.entry(track).or_default().push(cue);
        }
    }
    let mut q = db
        .prepare(
            "SELECT id, artist, title, album, genre, year, tracknumber, comment, duration,
                    samplerate, bpm, rating, key, timesplayed, color, composer, grouping,
                    datetime_added, last_played_at, beats, beats_version
             FROM library WHERE mixxx_deleted = 0",
        )
        .map_err(err)?;
    let text = |r: &rusqlite::Row, i: usize| -> rusqlite::Result<String> {
        Ok(r.get::<_, Option<String>>(i)?
            .unwrap_or_default()
            .trim()
            .to_string())
    };
    let rows = q
        .query_map([], |r| {
            Ok(Row {
                id: r.get(0)?,
                artist: text(r, 1)?,
                title: text(r, 2)?,
                album: text(r, 3)?,
                genre: text(r, 4)?,
                year: text(r, 5)?,
                track_number: text(r, 6)?,
                comment: text(r, 7)?,
                duration: r.get::<_, Option<f64>>(8)?.unwrap_or(0.0),
                sample_rate: r.get::<_, Option<f64>>(9)?.unwrap_or(0.0),
                bpm: r.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
                rating: r.get::<_, Option<i64>>(11)?.unwrap_or(0),
                key: text(r, 12)?,
                played: r.get::<_, Option<i64>>(13)?.unwrap_or(0),
                color: r.get::<_, Option<i64>>(14)?,
                composer: text(r, 15)?,
                grouping: text(r, 16)?,
                added: text(r, 17)?,
                last_played: text(r, 18)?,
                beats: r.get::<_, Option<Vec<u8>>>(19)?,
                beats_version: text(r, 20)?,
            })
        })
        .map_err(err)?;
    let mut tracks = Vec::new();
    for row in rows {
        let row = row.map_err(err)?;
        let Some(file) = files.get(&row.id) else {
            continue;
        };
        if !exists(file) {
            continue;
        }
        let track_cues = cues.remove(&row.id).unwrap_or_default();
        tracks.push(track(row, file.clone(), &track_cues));
    }
    let lists = lists(&db, groups, &files)?;
    Ok(ImportedLibrary { tracks, lists })
}

struct Row {
    id: i64,
    artist: String,
    title: String,
    album: String,
    genre: String,
    year: String,
    track_number: String,
    comment: String,
    duration: f64,
    sample_rate: f64,
    bpm: f64,
    rating: i64,
    key: String,
    played: i64,
    color: Option<i64>,
    composer: String,
    grouping: String,
    added: String,
    last_played: String,
    beats: Option<Vec<u8>>,
    beats_version: String,
}

/// A row of Mixxx's `cues`: positions count stereo samples (two a frame).
struct Cue {
    kind: i64,
    position: f64,
    length: f64,
    hotcue: i64,
    label: String,
    color: Option<i64>,
}

// Mixxx's cue types.
const HOT_CUE: i64 = 1;
const MAIN_CUE: i64 = 2;
const LOOP: i64 = 4;
const INTRO: i64 = 6;
const OUTRO: i64 = 7;

/// A date Mixxx writes ("2023-04-01T20:15:00.000Z" or "2023-04-01 20:15:00")
/// as a Unix time.
fn unix_time(text: &str) -> Option<i64> {
    let day = crate::virtualdj::lists::day_start(text)?;
    let time = text.get(11..19).unwrap_or("00:00:00");
    let mut parts = time.split(':').map(|p| p.parse::<i64>().unwrap_or(0));
    let (h, m, s) = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    Some(day + h * 3600 + m * 60 + s)
}

fn track(row: Row, file: PathBuf, cues: &[Cue]) -> Track {
    // Positions are in samples of both channels.
    let rate = if row.sample_rate > 0.0 {
        row.sample_rate
    } else {
        44_100.0
    };
    let seconds = |samples: f64| samples / (2.0 * rate);
    let mut hotcues = Vec::new();
    let mut markers = Vec::new();
    let mut memory = MEMORY_CUE;
    for c in cues.iter().filter(|c| c.position >= 0.0) {
        let at = seconds(c.position);
        let color = c.color.filter(|c| *c >= 0).map(|c| c as u32 & 0xFF_FFFF);
        match c.kind {
            HOT_CUE | LOOP if c.hotcue >= 0 => {
                let number = c.hotcue as usize;
                if hotcues.iter().any(|h: &Hotcue| h.number == number) {
                    continue;
                }
                hotcues.push(Hotcue {
                    number,
                    seconds: at,
                    name: c.label.clone(),
                    end: (c.kind == LOOP && c.length > 0.0).then(|| seconds(c.position + c.length)),
                    color,
                });
            }
            MAIN_CUE | LOOP => {
                hotcues.push(Hotcue {
                    number: memory,
                    seconds: at,
                    name: c.label.clone(),
                    end: (c.length > 0.0).then(|| seconds(c.position + c.length)),
                    color,
                });
                memory += 1;
            }
            INTRO | OUTRO => markers.push(Marker {
                kind: MarkerKind::Section,
                seconds: at,
                end: (c.length > 0.0).then(|| seconds(c.position + c.length)),
                name: if c.kind == INTRO { "Intro" } else { "Outro" }.into(),
                color: None,
                detected: false,
            }),
            _ => {}
        }
    }
    hotcues.sort_by_key(|h| h.number);
    let grid = row
        .beats
        .as_deref()
        .and_then(|b| beats(b, &row.beats_version, rate, row.bpm));
    let mut t = Track {
        id: 0,
        artist: row.artist,
        title: row.title,
        album: row.album,
        genre: row.genre,
        rating: row.rating.clamp(0, 5) as u8,
        bpm: row.bpm.max(0.0),
        key: row.key,
        duration: row.duration.max(0.0),
        first_beat: 0.0,
        grid: Default::default(),
        source: Source::File(file),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo {
            comment: row.comment,
            grouping: row.grouping,
            color: row.color.filter(|c| *c >= 0).map(|c| c as u32 & 0xFF_FFFF),
            play_count: row.played.max(0) as u32,
            hotcues,
            fixed_grid: grid.is_some(),
            year: row.year,
            track_number: row
                .track_number
                .split('/')
                .next()
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0),
            composer: row.composer,
            added: unix_time(&row.added).unwrap_or(0),
            last_played: unix_time(&row.last_played),
            markers,
            ..Default::default()
        },
    };
    if let Some(grid) = grid {
        t.set_grid(grid);
    }
    t
}

/// Mixxx's beats blob, a protocol buffer: `BeatGrid-2.0` holds the tempo
/// and the first beat's frame, `BeatMap-1.0` every beat's frame.
fn beats(blob: &[u8], version: &str, rate: f64, bpm: f64) -> Option<BeatGrid> {
    if version.starts_with("BeatGrid") {
        let mut grid_bpm = None;
        let mut first = None;
        for (field, value) in fields(blob)? {
            match (field, value) {
                (1, Value::Bytes(b)) => {
                    grid_bpm = fields(b)?.into_iter().find_map(|f| match f {
                        (1, Value::Fixed64(x)) => Some(f64::from_bits(x)),
                        _ => None,
                    })
                }
                (2, Value::Bytes(b)) => {
                    first = fields(b)?.into_iter().find_map(|f| match f {
                        (1, Value::Varint(x)) => Some(x as i64 as f64),
                        _ => None,
                    })
                }
                _ => {}
            }
        }
        let bpm = grid_bpm.filter(|b| *b > 0.0).unwrap_or(bpm);
        (bpm > 0.0).then(|| BeatGrid::constant(bpm, first.unwrap_or(0.0) / rate))
    } else if version.starts_with("BeatMap") {
        let mut frames: Vec<f64> = Vec::new();
        for (field, value) in fields(blob)? {
            if let (1, Value::Bytes(b)) = (field, value) {
                let mut frame = None;
                let mut enabled = true;
                for f in fields(b)? {
                    match f {
                        (1, Value::Varint(x)) => frame = Some(x as i64 as f64),
                        (2, Value::Varint(x)) => enabled = x != 0,
                        _ => {}
                    }
                }
                if let (Some(f), true) = (frame, enabled) {
                    frames.push(f / rate);
                }
            }
        }
        grid_from_beats(&frames)
    } else {
        None
    }
}

/// A grid from beat times: a marker where it starts and wherever the
/// spacing between beats changes by more than a little.
fn grid_from_beats(beats: &[f64]) -> Option<BeatGrid> {
    if beats.len() < 2 {
        return None;
    }
    let mut markers: Vec<GridMarker> = Vec::new();
    for w in beats.windows(2) {
        let bpm = 60.0 / (w[1] - w[0]);
        if !bpm.is_finite() {
            continue;
        }
        if markers.last().is_none_or(|m| (m.bpm - bpm).abs() > 0.5) {
            markers.push(GridMarker {
                seconds: w[0],
                bpm: (bpm * 100.0).round() / 100.0,
            });
        }
    }
    let grid = BeatGrid::new(markers);
    (!grid.is_empty()).then_some(grid)
}

enum Value<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32,
}

/// The fields of a protocol buffer message, in order. None when it isn't one.
fn fields(mut b: &[u8]) -> Option<Vec<(u64, Value<'_>)>> {
    fn varint(b: &mut &[u8]) -> Option<u64> {
        let mut out = 0u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = b.split_first()?;
            *b = rest;
            out |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(out);
            }
        }
        None
    }
    let mut out = Vec::new();
    while !b.is_empty() {
        let key = varint(&mut b)?;
        let value = match key & 7 {
            0 => Value::Varint(varint(&mut b)?),
            1 => {
                let (bytes, rest) = b.split_at_checked(8)?;
                b = rest;
                Value::Fixed64(u64::from_le_bytes(bytes.try_into().ok()?))
            }
            2 => {
                let len = varint(&mut b)? as usize;
                let (bytes, rest) = b.split_at_checked(len)?;
                b = rest;
                Value::Bytes(bytes)
            }
            5 => {
                b = b.get(4..)?;
                Value::Fixed32
            }
            _ => return None,
        };
        out.push((key >> 3, value));
    }
    Some(out)
}

/// Playlists (in a "Playlists" folder), crates (in "Crates") and history
/// sessions, with each track's file from `files`.
fn lists(
    db: &Connection,
    groups: &[&str],
    files: &HashMap<i64, PathBuf>,
) -> Result<Vec<ImportedList>, String> {
    let mut out = Vec::new();
    let entries = |rows: Vec<(i64, Option<String>)>| -> Vec<ImportedEntry> {
        rows.into_iter()
            .filter_map(|(id, at)| {
                Some(ImportedEntry::new(
                    files.get(&id)?.clone(),
                    at.as_deref().and_then(unix_time),
                ))
            })
            .collect()
    };
    let playlist_tracks = |id: i64| -> Result<Vec<(i64, Option<String>)>, String> {
        let mut q = db
            .prepare(
                "SELECT track_id, pl_datetime_added FROM PlaylistTracks
                 WHERE playlist_id = ?1 ORDER BY position",
            )
            .map_err(err)?;
        let rows = q
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?;
        rows.collect::<Result<_, _>>().map_err(err)
    };
    let playlists = |hidden: i64| -> Result<Vec<(i64, String, String)>, String> {
        let mut q = db
            .prepare(
                "SELECT id, name, COALESCE(date_created, '') FROM Playlists
                 WHERE hidden = ?1 ORDER BY position, id",
            )
            .map_err(err)?;
        let rows = q
            .query_map([hidden], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(err)?;
        rows.collect::<Result<_, _>>().map_err(err)
    };
    if groups.contains(&PLAYLISTS) {
        for (id, name, _) in playlists(0)? {
            out.push(ImportedList {
                folders: vec!["Playlists".into()],
                name,
                kind: ImportedListKind::List,
                entries: entries(playlist_tracks(id)?),
            });
        }
    }
    if groups.contains(&CRATES) {
        let mut q = db
            .prepare("SELECT id, name FROM crates ORDER BY name COLLATE NOCASE")
            .map_err(err)?;
        let crates: Vec<(i64, String)> = q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
        let mut members = db
            .prepare("SELECT track_id FROM crate_tracks WHERE crate_id = ?1")
            .map_err(err)?;
        for (id, name) in crates {
            let rows: Vec<(i64, Option<String>)> = members
                .query_map([id], |r| Ok((r.get(0)?, None)))
                .map_err(err)?
                .collect::<Result<_, _>>()
                .map_err(err)?;
            out.push(ImportedList {
                folders: vec!["Crates".into()],
                name,
                kind: ImportedListKind::List,
                entries: entries(rows),
            });
        }
    }
    if groups.contains(&HISTORY) {
        for (id, name, created) in playlists(2)? {
            let entries = entries(playlist_tracks(id)?);
            if entries.is_empty() {
                continue;
            }
            let started = entries
                .iter()
                .filter_map(|e| e.played_at)
                .min()
                .or_else(|| unix_time(&created))
                .or_else(|| unix_time(&name))
                .unwrap_or(0);
            out.push(ImportedList {
                folders: Vec::new(),
                name,
                kind: ImportedListKind::History { started },
                entries,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enough of Mixxx's schema for the importer.
    const SCHEMA: &str = "
        CREATE TABLE track_locations (id INTEGER PRIMARY KEY, location TEXT, filename TEXT,
            directory TEXT, filesize INTEGER, fs_deleted INTEGER, needs_verification INTEGER);
        CREATE TABLE library (id INTEGER PRIMARY KEY, artist TEXT, title TEXT, album TEXT,
            year TEXT, genre TEXT, tracknumber TEXT, location INTEGER, comment TEXT,
            duration INTEGER, samplerate INTEGER, bpm FLOAT, rating INTEGER, key TEXT,
            timesplayed INTEGER, color INTEGER, composer TEXT, grouping TEXT,
            datetime_added TEXT, last_played_at TEXT, beats BLOB, beats_version TEXT,
            mixxx_deleted INTEGER DEFAULT 0);
        CREATE TABLE cues (id INTEGER PRIMARY KEY, track_id INTEGER, type INTEGER,
            position INTEGER, length INTEGER, hotcue INTEGER, label TEXT, color INTEGER);
        CREATE TABLE Playlists (id INTEGER PRIMARY KEY, name TEXT, position INTEGER,
            hidden INTEGER, date_created TEXT, date_modified TEXT, locked INTEGER);
        CREATE TABLE PlaylistTracks (id INTEGER PRIMARY KEY, playlist_id INTEGER,
            track_id INTEGER, position INTEGER, pl_datetime_added TEXT);
        CREATE TABLE crates (id INTEGER PRIMARY KEY, name TEXT, count INTEGER, show INTEGER,
            locked INTEGER, autodj_source INTEGER);
        CREATE TABLE crate_tracks (crate_id INTEGER, track_id INTEGER);
    ";

    /// A `BeatGrid-2.0` blob: 128 BPM, first beat at frame 4410.
    fn grid_blob() -> Vec<u8> {
        let mut bpm = vec![0x09];
        bpm.extend(128.0f64.to_le_bytes());
        let first = [0x08, 0xba, 0x22]; // 4410
        let mut out = vec![0x0a, bpm.len() as u8];
        out.extend(bpm);
        out.extend([0x12, first.len() as u8]);
        out.extend(first);
        out
    }

    fn library() -> PathBuf {
        let path = std::env::temp_dir().join(format!("djsugar-mixxx-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = Connection::open(&path).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute_batch(
            "INSERT INTO track_locations (id, location) VALUES
                 (1, '/m/glue.mp3'), (2, '/m/gone.mp3'), (3, '/m/deleted.mp3');
             INSERT INTO library (id, artist, title, genre, tracknumber, location, duration,
                 samplerate, bpm, rating, key, timesplayed, color, datetime_added,
                 last_played_at, mixxx_deleted) VALUES
                 (1, 'Bicep', 'Glue', 'Electronic', '3/10', 1, 269, 44100, 128.0, 4, 'Fm', 7,
                  16711680, '2023-04-01T20:15:00.000Z', '2024-01-02 03:04:05', 0),
                 (2, 'X', 'Gone', '', '', 2, 100, 44100, 0, 0, '', 0, NULL, '', '', 0),
                 (3, 'Y', 'Deleted', '', '', 3, 100, 44100, 0, 0, '', 0, NULL, '', '', 1);
             INSERT INTO cues (track_id, type, position, length, hotcue, label, color) VALUES
                 (1, 1, 2822400, 0, 0, 'Drop', 255),
                 (1, 4, 5644800, 705600, 1, '', NULL),
                 (1, 2, 88200, 0, -1, '', NULL),
                 (1, 6, 88200, 1411200, -1, '', NULL);
             INSERT INTO Playlists (id, name, position, hidden, date_created) VALUES
                 (1, 'Friday', 1, 0, ''), (2, 'Auto DJ', 0, 1, ''),
                 (3, '2024-01-02', 2, 2, '2024-01-02 03:00:00');
             INSERT INTO PlaylistTracks (playlist_id, track_id, position, pl_datetime_added)
                 VALUES (1, 1, 1, ''), (1, 2, 2, ''), (2, 1, 1, ''),
                        (3, 1, 1, '2024-01-02 03:04:05');
             INSERT INTO crates (id, name) VALUES (1, 'Peak time');
             INSERT INTO crate_tracks (crate_id, track_id) VALUES (1, 1);",
        )
        .unwrap();
        db.execute(
            "UPDATE library SET beats = ?1, beats_version = 'BeatGrid-2.0' WHERE id = 1",
            [grid_blob()],
        )
        .unwrap();
        path
    }

    #[test]
    fn reads_a_mixxx_library() {
        let path = library();
        let lib = read(&path, &[PLAYLISTS, CRATES, HISTORY], |p| {
            p.ends_with("glue.mp3")
        })
        .unwrap();
        assert_eq!(lib.tracks.len(), 1);
        let t = &lib.tracks[0];
        assert_eq!(
            (t.artist.as_str(), t.rating, t.key.as_str()),
            ("Bicep", 4, "Fm")
        );
        assert_eq!((t.info.play_count, t.info.track_number), (7, 3));
        assert_eq!(t.info.color, Some(0xFF0000));
        assert_eq!(t.info.added, 1_680_380_100);
        assert_eq!(t.info.last_played, Some(1_704_164_645));
        assert!(t.info.fixed_grid);
        assert!((t.grid.first_beat() - 0.1).abs() < 1e-9);
        assert_eq!(t.bpm, 128.0);
        let cues: Vec<(usize, f64, Option<f64>)> = t
            .info
            .hotcues
            .iter()
            .map(|h| (h.number, h.seconds, h.end))
            .collect();
        assert_eq!(
            cues,
            [
                (0, 32.0, None),
                (1, 64.0, Some(72.0)),
                (MEMORY_CUE, 1.0, None)
            ]
        );
        assert_eq!(t.info.hotcues[0].color, Some(0x0000FF));
        assert_eq!(t.info.markers[0].name, "Intro");
        assert_eq!(t.info.markers[0].end, Some(17.0));
        let names: Vec<String> = lib
            .lists
            .iter()
            .map(|l| format!("{}/{}", l.folders.join("/"), l.name))
            .collect();
        assert_eq!(
            names,
            ["Playlists/Friday", "Crates/Peak time", "/2024-01-02"]
        );
        assert_eq!(lib.lists[0].entries.len(), 2);
        assert!(matches!(
            lib.lists[2].kind,
            ImportedListKind::History {
                started: 1_704_164_645
            }
        ));
        assert_eq!(
            Mixxx.preview(&path).unwrap(),
            "2 tracks, 1 playlist, 1 crate and 1 session of history."
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn a_beat_map_becomes_a_grid_with_its_tempo_changes() {
        // Frames at 120 BPM for 8 beats, then 124 BPM.
        let mut frames: Vec<f64> = (0..8).map(|i| i as f64 * 22_050.0).collect();
        let last = *frames.last().unwrap();
        frames.extend((1..8).map(|i| last + i as f64 * 44_100.0 * 60.0 / 124.0));
        let mut blob = Vec::new();
        for f in &frames {
            let mut beat = vec![0x08];
            let mut v = *f as u64;
            loop {
                let byte = (v & 0x7f) as u8;
                v >>= 7;
                if v == 0 {
                    beat.push(byte);
                    break;
                }
                beat.push(byte | 0x80);
            }
            blob.push(0x0a);
            blob.push(beat.len() as u8);
            blob.extend(beat);
        }
        let grid = beats(&blob, "BeatMap-1.0", 44_100.0, 0.0).unwrap();
        assert_eq!(grid.markers().len(), 2);
        assert_eq!(grid.markers()[0].bpm, 120.0);
        assert!((grid.markers()[1].bpm - 124.0).abs() < 0.01);
        assert!(beats(b"\xff", "BeatGrid-2.0", 44_100.0, 0.0).is_none());
    }
}
