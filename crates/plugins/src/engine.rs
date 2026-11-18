//! Brings an Engine DJ (Denon/Numark/Rane Engine OS) library into the
//! collection at start, when the DJ turns it on in Settings > Library:
//! the tracks in `Engine Library/Database2/m.db` with their tags, hot cues,
//! saved loops and beat grids, and its playlists, folders, smart lists
//! and history. It opens the database read-only and never writes it.
//!
//! The schema and blob formats follow EngineX-OS's `pkg/enginedb`
//! (<https://github.com/Cd1s/EngineX-OS>, MIT licence, Copyright (c) 2026
//! EngineX OS Contributors), measured on Engine OS 5.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::import::{ImportedEntry, ImportedList, ImportedListKind};
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, MEMORY_CUE};
use flate2::read::ZlibDecoder;
use rusqlite::{Connection, OpenFlags};

use crate::{ImportedLibrary, LibraryImport, ListGroup};

pub const PLAYLISTS: &str = "playlists";
pub const SMARTLISTS: &str = "smartlists";
pub const HISTORY: &str = "history";

/// Engine's key number (0..23): C=0, Am=1, G=2, Em=3, D=4, Bm=5, A=6, F#m=7,
/// E=8, C#m=9, B=10, G#m=11, F#=12, D#m=13, C#=14, A#m=15, Ab=16, Fm=17,
/// Eb=18, Cm=19, Bb=20, Gm=21, F=22, Dm=23.
fn engine_key_to_key(engine_key: i64) -> Option<djsugar_engine::Key> {
    if !(0..=23).contains(&engine_key) {
        return None;
    }
    let minor = engine_key % 2 == 1;
    let fifths = (engine_key / 2) as u8; // 0..11
                                         // Reverse of: fifths = (camelot_number + 4) % 12
                                         // camelot_number = (fifths * 7 + 7) % 12 + 1 (1..12)
                                         // major_tonic = (camelot_number - 8) * 7 mod 12
                                         // For minor: tonic = (major_tonic + 9) % 12
                                         // For major: tonic = major_tonic
    let camelot = (fifths as i32 * 7 + 7).rem_euclid(12) + 1;
    let major_tonic = (7 * (camelot - 8)).rem_euclid(12) as u8;
    let tonic = if minor {
        (major_tonic + 9) % 12
    } else {
        major_tonic
    };
    Some(djsugar_engine::Key { tonic, minor })
}

pub struct Engine;

/// Engine's desktop library (`~/Music/Engine Library/Database2/m.db`) or
/// a USB stick's (`/Volumes/.../Engine Library/Database2/m.db` on macOS,
/// `E:/Engine Library/Database2/m.db` on Windows). Also checks the older
/// 1.x layout (`Engine Library/m.db` and `Engine Library/p.db`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_ENGINE_DATABASE") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;

    // Desktop locations (macOS/Windows)
    let desktop_paths = [
        "Music/Engine Library/Database2/m.db",
        "Music/Engine Library/m.db", // 1.x
    ];

    // Also check mounted USB drives for Engine Library
    let mut candidates = desktop_paths
        .iter()
        .map(|p| home.join(p))
        .filter(|p| p.is_file())
        .collect::<Vec<_>>();

    // Check common USB mount points
    #[cfg(target_os = "macos")]
    let usb_roots = ["/Volumes"];
    #[cfg(target_os = "linux")]
    let usb_roots = ["/media", "/mnt", "/run/media"];
    #[cfg(target_os = "windows")]
    let usb_roots = ["E:\\", "F:\\", "G:\\", "H:\\", "I:\\", "J:\\"];

    for root in usb_roots {
        let root_path = Path::new(root);
        if root_path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(root_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let db2 = path.join("Engine Library/Database2/m.db");
                        if db2.is_file() {
                            candidates.push(db2);
                        }
                        let db1 = path.join("Engine Library/m.db");
                        if db1.is_file() {
                            candidates.push(db1);
                        }
                    }
                }
            }
        }
    }

    candidates.into_iter().next()
}

fn open(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("can't open Engine's database: {e}"))
}

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

impl LibraryImport for Engine {
    fn id(&self) -> &str {
        "engine"
    }

    fn title(&self) -> &str {
        "Engine DJ"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: PLAYLISTS,
                title: "Playlists",
                detail: "Your Engine DJ playlists, in their folders.",
                on_by_default: true,
            },
            ListGroup {
                id: SMARTLISTS,
                title: "Smart Lists",
                detail: "Your Engine DJ smart lists (saved searches).",
                on_by_default: false,
            },
            ListGroup {
                id: HISTORY,
                title: "History",
                detail: "Every session you played in Engine DJ, in the History tab.",
                on_by_default: true,
            },
        ]
    }

    fn stamp(&self, database: &Path) -> String {
        crate::mixxx::sqlite_stamp(database)
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        let db = open(database)?;
        let count = |sql: &str| -> Result<usize, String> {
            db.query_row(sql, [], |r| r.get::<_, i64>(0))
                .map(|n| n as usize)
                .map_err(err)
        };
        let tracks = count("SELECT COUNT(*) FROM Track WHERE isAvailable = 1")?;
        let playlists = count("SELECT COUNT(*) FROM Playlist WHERE isPersisted = 1")?;
        let smartlists = count("SELECT COUNT(*) FROM Smartlist")?;
        let history = count("SELECT COUNT(*) FROM Historylist")?;
        let n = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        Ok(format!(
            "{}, {}, {} and {}.",
            n(tracks, "track", "tracks"),
            n(playlists, "playlist", "playlists"),
            n(smartlists, "smart list", "smart lists"),
            n(history, "session of history", "sessions of history")
        ))
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = read(
            database,
            groups,
            database.parent().unwrap_or(Path::new(".")),
            Path::exists,
        )?;
        eprintln!(
            "Engine library: {} tracks here, {} lists",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}

/// A row of Engine's `Track` table.
struct TrackRow {
    id: i64,
    path: String,
    filename: String,
    title: String,
    artist: String,
    album: String,
    genre: String,
    comment: String,
    label: String,
    composer: String,
    remixer: String,
    year: i64,
    bpm: i64,
    bpm_analyzed: f64,
    key: i64,
    rating: i64,
    color: Option<i64>,
    length: i64, // milliseconds
    file_bytes: i64,
    bitrate: i64,
    file_type: String,
    date_added: String,
    date_created: String,
    time_last_played: Option<String>,
    is_played: bool,
    played_indicator: i64,
    album_art_id: Option<i64>,
    is_analyzed: bool,
}

/// A row of Engine's `PerformanceData` table.
struct PerfRow {
    track_id: i64,
    track_data: Option<Vec<u8>>,
    beat_data: Option<Vec<u8>>,
    quick_cues: Option<Vec<u8>>,
    loops: Option<Vec<u8>>,
}

/// Engine's cue types in quickCues/loops blobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum EngineCueType {
    HotCue = 0,
    Loop = 1,
    // Main cue is encoded separately in quickCues
}

/// Qt's qCompress: 4-byte big-endian length, then zlib.
fn q_decompress(blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < 4 {
        return Err("blob too short for qCompress header".into());
    }
    let len = u32::from_be_bytes(blob[..4].try_into().unwrap()) as usize;
    let mut decoder = ZlibDecoder::new(&blob[4..]);
    let mut out = Vec::with_capacity(len);
    decoder
        .read_to_end(&mut out)
        .map_err(|e| format!("zlib decompress failed: {e}"))?;
    if out.len() != len {
        return Err(format!(
            "decompressed length {} != expected {}",
            out.len(),
            len
        ));
    }
    Ok(out)
}

/// Read a u32 (big-endian) from a slice.
fn read_u32_be(b: &[u8], offset: &mut usize) -> Option<u32> {
    if *offset + 4 > b.len() {
        return None;
    }
    let val = u32::from_be_bytes(b[*offset..*offset + 4].try_into().ok()?);
    *offset += 4;
    Some(val)
}

/// Read a u64 (big-endian) from a slice.
fn read_u64_be(b: &[u8], offset: &mut usize) -> Option<u64> {
    if *offset + 8 > b.len() {
        return None;
    }
    let val = u64::from_be_bytes(b[*offset..*offset + 8].try_into().ok()?);
    *offset += 8;
    Some(val)
}

/// Read a f64 (big-endian) from a slice.
fn read_f64_be(b: &[u8], offset: &mut usize) -> Option<f64> {
    read_u64_be(b, offset).map(f64::from_bits)
}

/// Read a i32 (big-endian) from a slice.
fn read_i32_be(b: &[u8], offset: &mut usize) -> Option<i32> {
    read_u32_be(b, offset).map(i32::from_be)
}

/// Read a i64 (big-endian) from a slice.
fn read_i64_be(b: &[u8], offset: &mut usize) -> Option<i64> {
    read_u64_be(b, offset).map(i64::from_be)
}

/// Read a length-prefixed UTF-8 string (1-byte length).
fn read_label(b: &[u8], offset: &mut usize) -> Option<String> {
    if *offset >= b.len() {
        return None;
    }
    let len = b[*offset] as usize;
    *offset += 1;
    if *offset + len > b.len() {
        return None;
    }
    let s = String::from_utf8(b[*offset..*offset + len].to_vec()).ok()?;
    *offset += len;
    Some(s)
}

/// Parse Engine's beatData blob.
/// Format (per EngineX-OS):
/// - sample_rate (f64, BE)
/// - sample_count (f64, BE)
/// - grid_set (u8)
/// - default grid: count (u64, BE), then for each: sample (f64, LE), beat (i64, LE), beats_to_next (i32, LE), reserved (i32, LE)
/// - adjusted grid: same structure
/// - terminator (u8, 0)
fn parse_beat_data(blob: &[u8], track_duration: f64) -> Option<BeatGrid> {
    if blob.len() < 25 {
        return None;
    }
    let mut offset = 0;
    let _sample_rate = read_f64_be(blob, &mut offset)?;
    let _sample_count = read_f64_be(blob, &mut offset)?;
    let grid_set = blob.get(offset)?;
    offset += 1;
    if *grid_set == 0 {
        return None;
    }

    // Parse default grid
    let count = read_u64_be(blob, &mut offset)? as usize;
    let mut markers = Vec::with_capacity(count);
    for _ in 0..count {
        let sample = read_f64_le(blob, &mut offset)?;
        let beat = read_i64_le(blob, &mut offset)?;
        let beats_to_next = read_i32_le(blob, &mut offset)?;
        let _reserved = read_i32_le(blob, &mut offset)?;
        let prev_sample = markers.last().map(|m| m.seconds);
        let bpm = if beats_to_next > 0 && prev_sample.is_some() {
            let diff = sample - prev_sample.unwrap();
            if diff > 0.001 {
                (beats_to_next as f64 * 60.0) / diff
            } else {
                0.0
            }
        } else {
            0.0
        };
        markers.push(GridMarker {
            seconds: sample,
            bpm,
        });
    }

    // Skip adjusted grid (same count)
    let count2 = read_u64_be(blob, &mut offset)? as usize;
    offset += count2 * (8 + 8 + 4 + 4); // sample + beat + beats_to_next + reserved

    let terminator = blob.get(offset)?;
    if *terminator != 0 {
        return None;
    }

    if markers.len() < 2 {
        return None;
    }

    // Filter and build grid
    let mut filtered: Vec<GridMarker> = Vec::new();
    for m in markers {
        if m.seconds.is_finite()
            && m.bpm.is_finite()
            && m.bpm > 1.0
            && m.seconds >= 0.0
            && m.seconds <= track_duration
        {
            filtered.push(m);
        }
    }
    filtered.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    if filtered.len() < 2 {
        return None;
    }
    Some(BeatGrid::new(filtered))
}

/// Read a f64 (little-endian) from a slice.
fn read_f64_le(b: &[u8], offset: &mut usize) -> Option<f64> {
    if *offset + 8 > b.len() {
        return None;
    }
    let val = f64::from_le_bytes(b[*offset..*offset + 8].try_into().ok()?);
    *offset += 8;
    Some(val)
}

/// Read a i64 (little-endian) from a slice.
fn read_i64_le(b: &[u8], offset: &mut usize) -> Option<i64> {
    if *offset + 8 > b.len() {
        return None;
    }
    let val = i64::from_le_bytes(b[*offset..*offset + 8].try_into().ok()?);
    *offset += 8;
    Some(val)
}

/// Read a i32 (little-endian) from a slice.
fn read_i32_le(b: &[u8], offset: &mut usize) -> Option<i32> {
    if *offset + 4 > b.len() {
        return None;
    }
    let val = i32::from_le_bytes(b[*offset..*offset + 4].try_into().ok()?);
    *offset += 4;
    Some(val)
}

/// Parse Engine's quickCues blob.
/// Format (per EngineX-OS):
/// - count (u64, BE) = 8
/// - for each of 8 slots: label (1-byte len + UTF-8), position (f64, BE), color ARGB (4 bytes)
/// - main cue position (f64, BE)
/// - separator (u8, 0)
/// - main cue position again (f64, BE)
fn parse_quick_cues(blob: &[u8], sample_rate: f64) -> Vec<(usize, f64, String, Option<u32>)> {
    if blob.len() < 9 {
        return Vec::new();
    }
    let mut offset = 0;
    let count = read_u64_be(blob, &mut offset).unwrap_or(0) as usize;
    let mut cues = Vec::new();
    for i in 0..count.min(8) {
        let label = read_label(blob, &mut offset).unwrap_or_default();
        let pos_samples = read_f64_be(blob, &mut offset).unwrap_or(-1.0);
        let mut color = [0u8; 4];
        if offset + 4 <= blob.len() {
            color.copy_from_slice(&blob[offset..offset + 4]);
            offset += 4;
        }
        let pos_secs = if pos_samples >= 0.0 {
            pos_samples / sample_rate
        } else {
            -1.0
        };
        let color_rgb = if color[0] == 255 && (color[1] != 0 || color[2] != 0 || color[3] != 0) {
            Some(((color[1] as u32) << 16) | ((color[2] as u32) << 8) | (color[3] as u32))
        } else {
            None
        };
        cues.push((i, pos_secs, label, color_rgb));
    }
    cues
}

/// Parse Engine's loops blob (not qCompressed).
/// Format (per EngineX-OS):
/// - count (u64, LE) = 8
/// - for each of 8 slots: label (1-byte len + UTF-8), start (f64, LE), end (f64, LE), loop_type (u8), enabled (u8), color ARGB (4 bytes)
fn parse_loops(blob: &[u8], sample_rate: f64) -> Vec<(usize, f64, f64, String, Option<u32>)> {
    if blob.len() < 9 {
        return Vec::new();
    }
    let mut offset = 0;
    let count = read_u64_le(blob, &mut offset).unwrap_or(0) as usize;
    let mut loops = Vec::new();
    for i in 0..count.min(8) {
        let label = read_label(blob, &mut offset).unwrap_or_default();
        let start_samples = read_f64_le(blob, &mut offset).unwrap_or(-1.0);
        let end_samples = read_f64_le(blob, &mut offset).unwrap_or(-1.0);
        let _loop_type = blob.get(offset).copied().unwrap_or(0);
        offset += 1;
        let _enabled = blob.get(offset).copied().unwrap_or(0);
        offset += 1;
        let mut color = [0u8; 4];
        if offset + 4 <= blob.len() {
            color.copy_from_slice(&blob[offset..offset + 4]);
            offset += 4;
        }
        let start_secs = if start_samples >= 0.0 {
            start_samples / sample_rate
        } else {
            -1.0
        };
        let end_secs = if end_samples >= 0.0 {
            end_samples / sample_rate
        } else {
            -1.0
        };
        let color_rgb = if color[0] == 255 && (color[1] != 0 || color[2] != 0 || color[3] != 0) {
            Some(((color[1] as u32) << 16) | ((color[2] as u32) << 8) | (color[3] as u32))
        } else {
            None
        };
        loops.push((i, start_secs, end_secs, label, color_rgb));
    }
    loops
}

/// Read a u64 (little-endian) from a slice.
fn read_u64_le(b: &[u8], offset: &mut usize) -> Option<u64> {
    if *offset + 8 > b.len() {
        return None;
    }
    let val = u64::from_le_bytes(b[*offset..*offset + 8].try_into().ok()?);
    *offset += 8;
    Some(val)
}

/// Parse Engine's trackData blob.
/// Format (per EngineX-OS, 68 bytes):
/// - sample_rate (f64, BE)
/// - sample_count (u64, BE)
/// - key (u32, BE) - Engine's key number
/// - low_peak (f64, BE)
/// - mid_peak (f64, BE)
/// - high_peak (f64, BE)
/// - stem_low (f64, BE) - 0
/// - stem_mid (f64, BE) - 0
/// - stem_high (f64, BE) - 0
fn parse_track_data(blob: &[u8]) -> Option<(f64, u64, i64)> {
    if blob.len() < 68 {
        return None;
    }
    let mut offset = 0;
    let sample_rate = read_f64_be(blob, &mut offset)?;
    let sample_count = read_u64_be(blob, &mut offset)?;
    let key = read_u32_be(blob, &mut offset)? as i64;
    Some((sample_rate, sample_count, key))
}

/// A date Engine writes (Unix timestamp or "YYYY-MM-DD HH:MM:SS") as a Unix time.
fn unix_time(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // Try Unix timestamp first
    if let Ok(ts) = text.parse::<i64>() {
        if ts > 1000000000 {
            return Some(ts);
        }
    }
    // Try "YYYY-MM-DD HH:MM:SS" or "YYYY-MM-DDTHH:MM:SS"
    let day_part = text.get(..10)?;
    let time_part = text.get(11..19).or_else(|| text.get(11..19))?;
    let mut parts = day_part.split('-').map(|p| p.parse::<i64>().unwrap_or(0));
    let year = parts.next()?;
    let month = parts.next()?;
    let day = parts.next()?;
    let mut time_parts = time_part.split(':').map(|p| p.parse::<i64>().unwrap_or(0));
    let hour = time_parts.next()?;
    let minute = time_parts.next()?;
    let second = time_parts.next()?;
    // Days since epoch
    let mut days = 0i64;
    for y in 1970..year {
        days += if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
            366
        } else {
            365
        };
    }
    let month_days = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    for m in 1..month {
        days += month_days[m];
        if m == 2 && is_leap {
            days += 1;
        }
    }
    days += day - 1;
    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

/// The tracks whose files `exists` says are here, and the lists in `groups`.
fn read(
    path: &Path,
    groups: &[&str],
    db_root: &Path,
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let db = open(path)?;

    // Load all tracks
    let mut track_stmt = db
        .prepare(
            "SELECT id, path, filename, title, artist, album, genre, comment, label, composer,
                    remixer, year, bpm, bpmAnalyzed, key, rating, color, length, fileBytes,
                    bitrate, fileType, dateAdded, dateCreated, timeLastPlayed, isPlayed,
                    playedIndicator, albumArtId, isAnalyzed
             FROM Track WHERE isAvailable = 1",
        )
        .map_err(err)?;

    let track_rows = track_stmt
        .query_map([], |r| {
            Ok(TrackRow {
                id: r.get(0)?,
                path: r.get(1)?,
                filename: r.get(2)?,
                title: r.get(3)?,
                artist: r.get(4)?,
                album: r.get(5)?,
                genre: r.get(6)?,
                comment: r.get(7)?,
                label: r.get(8)?,
                composer: r.get(9)?,
                remixer: r.get(10)?,
                year: r.get(11)?,
                bpm: r.get(12)?,
                bpm_analyzed: r.get(13)?,
                key: r.get(14)?,
                rating: r.get(15)?,
                color: r.get(16)?,
                length: r.get(17)?,
                file_bytes: r.get(18)?,
                bitrate: r.get(19)?,
                file_type: r.get(20)?,
                date_added: r.get(21)?,
                date_created: r.get(22)?,
                time_last_played: r.get(23)?,
                is_played: r.get(24)?,
                played_indicator: r.get(25)?,
                album_art_id: r.get(26)?,
                is_analyzed: r.get(27)?,
            })
        })
        .map_err(err)?;

    // Load PerformanceData
    let mut perf_map: HashMap<i64, PerfRow> = HashMap::new();
    let mut perf_stmt = db
        .prepare(
            "SELECT trackId, trackData, beatData, quickCues, loops
             FROM PerformanceData",
        )
        .map_err(err)?;
    let perf_rows = perf_stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                PerfRow {
                    track_id: r.get(0)?,
                    track_data: r.get(1)?,
                    beat_data: r.get(2)?,
                    quick_cues: r.get(3)?,
                    loops: r.get(4)?,
                },
            ))
        })
        .map_err(err)?;
    for row in perf_rows {
        let (track_id, perf) = row.map_err(err)?;
        perf_map.insert(track_id, perf);
    }

    // Load album art
    let mut art_map: HashMap<i64, Vec<u8>> = HashMap::new();
    let mut art_stmt = db
        .prepare("SELECT id, albumArt FROM AlbumArt")
        .map_err(err)?;
    let art_rows = art_stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))
        .map_err(err)?;
    for row in art_rows {
        let (id, art) = row.map_err(err)?;
        art_map.insert(id, art);
    }

    // Build tracks
    let mut tracks = Vec::new();
    for row in track_rows {
        let row = row.map_err(err)?;
        let perf = perf_map.get(&row.id);

        // Resolve file path: Engine stores paths relative to the Engine Library folder
        // which is the parent of Database2/
        let db_dir = path.parent().unwrap_or(Path::new("."));
        let library_dir = db_dir.parent().unwrap_or(db_dir); // Engine Library/
        let file_path = library_dir.join(&row.path);
        if !exists(&file_path) {
            continue;
        }

        // Sample rate from trackData or default
        let (sample_rate, _sample_count, track_data_key) = perf
            .and_then(|p| p.track_data.as_deref())
            .and_then(|b| q_decompress(b).ok())
            .and_then(|b| parse_track_data(&b))
            .unwrap_or((44100.0, 0, 0));

        // Beat grid from beatData
        let duration = row.length as f64 / 1000.0;
        let grid = perf
            .and_then(|p| p.beat_data.as_deref())
            .and_then(|b| q_decompress(b).ok())
            .and_then(|b| parse_beat_data(&b, duration))
            .unwrap_or_default();

        // Hot cues from quickCues
        let mut hotcues = Vec::new();
        let mut memory = MEMORY_CUE;
        let mut main_cue_secs = None;

        if let Some(perf) = perf {
            if let Some(quick_cues) = perf.quick_cues.as_deref() {
                if let Ok(decompressed) = q_decompress(quick_cues) {
                    let cues = parse_quick_cues(&decompressed, sample_rate);
                    for (slot, pos_secs, label, color) in cues {
                        if pos_secs < 0.0 {
                            continue;
                        }
                        if slot < 8 {
                            // Hot cue slot 0-7
                            if hotcues.iter().any(|h: &Hotcue| h.number == slot) {
                                continue;
                            }
                            hotcues.push(Hotcue {
                                number: slot,
                                seconds: pos_secs,
                                name: label,
                                end: None,
                                color,
                            });
                        } else if main_cue_secs.is_none() {
                            // First main cue position
                            main_cue_secs = Some(pos_secs);
                        }
                    }
                }
            }

            // Saved loops from loops blob
            if let Some(loops_blob) = perf.loops.as_deref() {
                let loops = parse_loops(loops_blob, sample_rate);
                for (slot, start_secs, end_secs, label, color) in loops {
                    if start_secs < 0.0 || end_secs < 0.0 || end_secs <= start_secs {
                        continue;
                    }
                    if slot < 8 {
                        // Loop in hot cue slot
                        if hotcues.iter().any(|h: &Hotcue| h.number == slot) {
                            continue;
                        }
                        hotcues.push(Hotcue {
                            number: slot,
                            seconds: start_secs,
                            name: label,
                            end: Some(end_secs),
                            color,
                        });
                    } else {
                        // Memory loop
                        hotcues.push(Hotcue {
                            number: memory,
                            seconds: start_secs,
                            name: label,
                            end: Some(end_secs),
                            color,
                        });
                        memory += 1;
                    }
                }
            }
        }

        // Main cue (memory cue 0)
        if let Some(main) = main_cue_secs {
            hotcues.push(Hotcue {
                number: MEMORY_CUE,
                seconds: main,
                name: "Main".into(),
                end: None,
                color: None,
            });
        }

        hotcues.sort_by_key(|h| h.number);

        // Key: prefer trackData key, then Track.key
        let key = if track_data_key != 0 {
            engine_key_to_key(track_data_key)
        } else if row.key != 0 {
            engine_key_to_key(row.key)
        } else {
            None
        };

        // BPM: prefer analyzed, then stored
        let bpm = if row.bpm_analyzed > 0.0 {
            row.bpm_analyzed
        } else if row.bpm > 0 {
            row.bpm as f64
        } else {
            0.0
        };

        let mut t = Track {
            id: 0,
            artist: row.artist,
            title: row.title,
            album: row.album,
            genre: row.genre,
            rating: row.rating.clamp(0, 5) as u8,
            bpm: bpm.max(0.0),
            key: key.map(|k| k.name()).unwrap_or_default(),
            duration: duration.max(0.0),
            first_beat: 0.0,
            grid: Default::default(),
            source: Source::File(file_path),
            waveform: Arc::default(),
            audio: None,
            cover: art_map
                .get(&row.album_art_id.unwrap_or(0))
                .and_then(|a| djsugar_engine::cover::decode(a, djsugar_engine::cover::COVER_SIZE))
                .map(Arc::new),
            info: LibraryInfo {
                comment: row.comment,
                grouping: String::new(),
                color: row.color.filter(|c| *c >= 0).map(|c| c as u32 & 0xFF_FFFF),
                play_count: row.played_indicator.max(0) as u32,
                hotcues,
                fixed_grid: !grid.is_empty(),
                year: if row.year > 0 {
                    row.year.to_string()
                } else {
                    String::new()
                },
                track_number: 0,
                composer: row.composer,
                added: unix_time(&row.date_added).unwrap_or(0),
                last_played: row.time_last_played.and_then(|s| unix_time(&s)),
                markers: Vec::new(),
                label: row.label,
                remixer: row.remixer,
                ..Default::default()
            },
        };
        if !grid.is_empty() {
            t.set_grid(grid);
        }
        tracks.push(t);
    }

    // Build lists
    let library_dir = path
        .parent()
        .unwrap_or(Path::new("."))
        .parent()
        .unwrap_or(Path::new("."));
    let lists = lists(&db, groups, &tracks, library_dir)?;

    Ok(ImportedLibrary { tracks, lists })
}

/// Playlists, smart lists and history, with each track from `tracks`.
fn lists(
    db: &Connection,
    groups: &[&str],
    _tracks: &[Track],
    library_dir: &Path,
) -> Result<Vec<ImportedList>, String> {
    let mut out = Vec::new();

    // We need track_id -> path mapping.
    let mut track_id_to_path: HashMap<i64, PathBuf> = HashMap::new();
    let mut q = db
        .prepare("SELECT id, path FROM Track WHERE isAvailable = 1")
        .map_err(err)?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(err)?;
    for row in rows {
        let (id, path_str) = row.map_err(err)?;
        track_id_to_path.insert(id, library_dir.join(&path_str));
    }

    let make_entries =
        |track_ids: Vec<i64>, played_ats: Vec<Option<String>>| -> Vec<ImportedEntry> {
            track_ids
                .into_iter()
                .zip(played_ats)
                .filter_map(|(id, at)| {
                    let path = track_id_to_path.get(&id)?.clone();
                    Some(ImportedEntry::new(path, at.and_then(|s| unix_time(&s))))
                })
                .collect()
        };

    // Playlists (and folders via parentListId tree)
    if groups.contains(&PLAYLISTS) {
        // Build playlist tree
        let mut playlists: HashMap<i64, (String, Option<i64>, i64)> = HashMap::new(); // id -> (title, parentListId, nextListId)
        let mut q = db
            .prepare(
                "SELECT id, title, parentListId, nextListId FROM Playlist WHERE isPersisted = 1 ORDER BY position, id",
            )
            .map_err(err)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (id, title, parent, next) = row.map_err(err)?;
            playlists.insert(id, (title, parent, next.unwrap_or(0)));
        }

        // Build folder paths for each playlist
        let mut playlist_folders: HashMap<i64, Vec<String>> = HashMap::new();
        for &id in playlists.keys() {
            let mut folders = Vec::new();
            let mut current = playlists.get(&id);
            while let Some((_title, parent, _)) = current {
                if let Some(pid) = parent {
                    if let Some((ptitle, _, _)) = playlists.get(pid) {
                        folders.push(ptitle.clone());
                        current = playlists.get(pid);
                        continue;
                    }
                }
                break;
            }
            folders.reverse();
            playlist_folders.insert(id, folders);
        }

        // PlaylistEntity: linked list via nextEntityId
        let mut q = db
            .prepare(
                "SELECT pe.listId, pe.trackId, pe.nextEntityId
                 FROM PlaylistEntity pe
                 JOIN Playlist p ON p.id = pe.listId
                 WHERE p.isPersisted = 1
                 ORDER BY pe.listId, pe.nextEntityId",
            )
            .map_err(err)?;
        let mut current_list = -1i64;
        let mut track_ids = Vec::new();
        let mut played_ats = Vec::new();
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (list_id, track_id, _next) = row.map_err(err)?;
            if list_id != current_list {
                if current_list != -1 && !track_ids.is_empty() {
                    if let Some((title, _, _)) = playlists.get(&current_list) {
                        let folders = playlist_folders
                            .get(&current_list)
                            .cloned()
                            .unwrap_or_default();
                        out.push(ImportedList {
                            folders,
                            name: title.clone(),
                            kind: ImportedListKind::List,
                            entries: make_entries(track_ids.clone(), played_ats.clone()),
                        });
                    }
                }
                current_list = list_id;
                track_ids.clear();
                played_ats.clear();
            }
            track_ids.push(track_id);
            played_ats.push(None); // PlaylistEntity doesn't store date added in Engine
        }
        // Last playlist
        if current_list != -1 && !track_ids.is_empty() {
            if let Some((title, _, _)) = playlists.get(&current_list) {
                let folders = playlist_folders
                    .get(&current_list)
                    .cloned()
                    .unwrap_or_default();
                out.push(ImportedList {
                    folders,
                    name: title.clone(),
                    kind: ImportedListKind::List,
                    entries: make_entries(track_ids.clone(), played_ats.clone()),
                });
            }
        }
    }

    // Smart lists
    if groups.contains(&SMARTLISTS) {
        let mut q = db
            .prepare("SELECT listUuid, title, parentPlaylistPath, rules FROM Smartlist")
            .map_err(err)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (_uuid, title, parent_path, rules) = row.map_err(err)?;
            let folders: Vec<String> = parent_path
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            out.push(ImportedList {
                folders,
                name: title,
                kind: ImportedListKind::Smart { query: rules },
                entries: Vec::new(), // Smart lists are dynamic
            });
        }
    }

    // History
    if groups.contains(&HISTORY) {
        let mut q = db
            .prepare(
                "SELECT h.id, h.startTime, he.trackId, he.playTime
                 FROM Historylist h
                 JOIN HistorylistEntity he ON he.listId = h.id
                 ORDER BY h.startTime DESC, he.playTime",
            )
            .map_err(err)?;
        let mut current_session = -1i64;
        let mut track_ids = Vec::new();
        let mut played_ats = Vec::new();
        let mut session_start = 0i64;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (session_id, start_time, track_id, play_time) = row.map_err(err)?;
            if session_id != current_session {
                if current_session != -1 && !track_ids.is_empty() {
                    out.push(ImportedList {
                        folders: vec!["History".into()],
                        name: format!("Session {}", session_start),
                        kind: ImportedListKind::History {
                            started: session_start,
                        },
                        entries: make_entries(track_ids.clone(), played_ats.clone()),
                    });
                }
                current_session = session_id;
                session_start = unix_time(&start_time).unwrap_or(0);
                track_ids.clear();
                played_ats.clear();
            }
            track_ids.push(track_id);
            played_ats.push(Some(play_time));
        }
        if current_session != -1 && !track_ids.is_empty() {
            out.push(ImportedList {
                folders: vec!["History".into()],
                name: format!("Session {}", session_start),
                kind: ImportedListKind::History {
                    started: session_start,
                },
                entries: make_entries(track_ids.clone(), played_ats.clone()),
            });
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn q_compress(raw: &[u8]) -> Vec<u8> {
        let mut out = (raw.len() as u32).to_be_bytes().to_vec();
        let mut z = ZlibEncoder::new(out, Compression::default());
        z.write_all(raw).expect("writing to memory");
        z.finish().expect("writing to memory")
    }

    #[test]
    fn test_engine_key_mapping() {
        // Test a few known mappings
        // Engine key 0 = C major -> DJ Sugar C
        let k = engine_key_to_key(0).unwrap();
        assert_eq!(k.name(), "C");
        // Engine key 1 = A minor -> DJ Sugar Am
        let k = engine_key_to_key(1).unwrap();
        assert_eq!(k.name(), "Am");
        // Engine key 2 = G major -> DJ Sugar G
        let k = engine_key_to_key(2).unwrap();
        assert_eq!(k.name(), "G");
        // Engine key 22 = F major -> DJ Sugar F
        let k = engine_key_to_key(22).unwrap();
        assert_eq!(k.name(), "F");
        // Engine key 23 = D minor -> DJ Sugar Dm
        let k = engine_key_to_key(23).unwrap();
        assert_eq!(k.name(), "Dm");
    }

    #[test]
    fn test_q_decompress() {
        // Test qCompress round-trip
        let original = b"hello world test data";
        let compressed = q_compress(original);
        let decompressed = q_decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_parse_quick_cues() {
        // Build a minimal quickCues blob (8 slots, all empty)
        let mut b = Vec::new();
        b.extend((8u64).to_be_bytes());
        for _ in 0..8 {
            b.push(0); // empty label
            b.extend((-1f64).to_be_bytes());
            b.extend([0u8; 4]); // no color
        }
        b.extend((-1f64).to_be_bytes()); // main cue
        b.push(0); // separator
        b.extend((-1f64).to_be_bytes()); // main cue again
        let cues = parse_quick_cues(&b, 44100.0);
        assert_eq!(cues.len(), 8);
        for (slot, pos, label, color) in cues {
            assert_eq!(slot, 0); // all slot 0 since we're testing empty
            assert_eq!(pos, -1.0);
            assert!(label.is_empty());
            assert!(color.is_none());
        }
    }
}
