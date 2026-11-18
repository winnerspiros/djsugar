//! Brings a Serato DJ library into the collection at start, when the DJ turns
//! it on in Settings › Collection › Import: each track's tags, BPM, key,
//! hot cues, saved loops, beatgrid, color, rating, play count, and added
//! date; Serato's crates in their folders; and its smart crates and history.
//! Streamed tracks (TIDAL, SoundCloud, Beatport, Beatsource) stand in Serato's
//! lists as placeholders. It reads Serato's files and never writes them.
//!
//! Serato keeps its library in the `_Serato_` folder (~/Music/_Serato_ on
//! macOS/Windows, plus _Serato_ at the root of external drives):
//! - `database V2`: binary TLV format with all tracks (see [`database`]).
//! - `Subcrates/*.crate`: regular crates, same TLV format with `ptrk` paths
//!   (see [`crates`]).
//! - `SmartCrates/*.scrate`: smart crates with search rules (see [`smart`]).
//! - Audio file tags: GEOB frames (MP3/AIFF/WAV), MP4 atoms, Vorbis comments
//!   (FLAC/Ogg) with `Serato Markers2`, `Serato BeatGrid`, `Serato Autotags`
//!   (see [`tags`]).
//!
//! Prior work: Holzhaus/serato-tags (MIT) documents the GEOB tag formats;
//! Mixxx's src/track/serato/*.cpp (GPL) was consulted for behaviour; sidecaramel
//! (MIT) reverse-engineered the database V2 and crate formats.

pub mod crates;
pub mod database;
pub mod smart;
pub mod tags;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{ImportedLibrary, ImportedList, ImportedListKind, LibraryImport, ListGroup};
use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::import::ImportedEntry;
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, MEMORY_CUE};

use database::{TrackRecord, TrackRecordKey};

/// The importer's stable id.
pub const ID: &str = "serato";

/// List group ids.
pub const CRATES: &str = "crates";
pub const SMART_CRATES: &str = "smart_crates";
pub const HISTORY: &str = "history";

/// Serato's library root where it keeps `database V2`, `Subcrates/`, `SmartCrates/`.
pub fn default_database_root() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_SERATO_ROOT") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;
    // macOS/Windows: ~/Music/_Serato_
    let music = home.join("Music").join("_Serato_");
    if music.is_dir() {
        return Some(music);
    }
    None
}

/// The `database V2` file inside the Serato root.
pub fn default_database() -> Option<PathBuf> {
    default_database_root()
        .map(|r| r.join("database V2"))
        .filter(|p| p.is_file())
}

/// Finds all Serato roots: ~/Music/_Serato_ plus _Serato_ at the root of
/// each mounted volume (macOS: /Volumes/*, Windows: drive letters).
pub fn all_serato_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = default_database_root() {
        roots.push(root);
    }
    // External drives: macOS /Volumes, Windows drive letters
    #[cfg(target_os = "macos")]
    {
        if let Ok(entries) = std::fs::read_dir("/Volumes") {
            for e in entries.flatten() {
                let serato = e.path().join("_Serato_");
                if serato.is_dir() && !roots.contains(&serato) {
                    roots.push(serato);
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        for drive in 'C'..='Z' {
            let serato = PathBuf::from(format!("{}:\\_Serato_", drive));
            if serato.is_dir() && !roots.contains(&serato) {
                roots.push(serato);
            }
        }
    }
    roots
}

/// Serato library importer.
pub struct Serato;

impl LibraryImport for Serato {
    fn id(&self) -> &str {
        ID
    }

    fn title(&self) -> &str {
        "Serato DJ"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn missing_hint(&self) -> &str {
        "Serato keeps its library in a folder named _Serato_ inside your Music folder, and on each external drive you've used with Serato. Point to the folder that contains 'database V2', 'Subcrates' and 'SmartCrates'."
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: CRATES,
                title: "Crates",
                detail: "Your Serato crates, in their folders.",
                on_by_default: true,
            },
            ListGroup {
                id: SMART_CRATES,
                title: "Smart Crates",
                detail: "Serato smart crates (saved searches). They become smart lists here.",
                on_by_default: false,
            },
            ListGroup {
                id: HISTORY,
                title: "History",
                detail: "Every session you played in Serato.",
                on_by_default: true,
            },
        ]
    }

    fn stamp(&self, database: &Path) -> String {
        let root = database.parent().unwrap_or(Path::new("."));
        let mut newest = modified(database);
        for dir in ["Subcrates", "SmartCrates", "History"] {
            newest = newest.max(modified(&root.join(dir)));
        }
        format!("{}:{}", database.display(), newest)
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        let lib = Library::load(database)?;
        let n = |count: usize, one: &str, many: &str| {
            format!("{} {}", count, if count == 1 { one } else { many })
        };
        Ok(format!(
            "{} and {} crates.",
            n(lib.tracks.len(), "track", "tracks"),
            n(lib.crates.len(), "crate", "crates")
        ))
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = Library::load(database)?;
        library.read(groups)
    }
}

/// Tracks loaded from the database, indexed by their database key.
struct Library {
    tracks: HashMap<TrackRecordKey, TrackRecord>,
    crates: Vec<crate::crates::Crate>,
    smart_crates: Vec<crate::smart::SmartCrate>,
    history: Vec<crate::database::HistorySession>,
    root: PathBuf,
}

impl Library {
    fn load(database: &Path) -> Result<Self, String> {
        let root = database.parent().unwrap_or(Path::new(".")).to_path_buf();
        let (tracks, _version) = database::read_database(database)?;
        let crates = crates::read_crates(&root)?;
        let smart_crates = smart::read_smart_crates(&root)?;
        let history = database::read_history(&root)?;
        Ok(Self {
            tracks,
            crates,
            smart_crates,
            history,
            root,
        })
    }

    fn read(self, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let mut tracks = Vec::new();
        let mut track_by_path = HashMap::new();
        let mut track_by_key = HashMap::new();

        // Read audio file tags for each track that exists on disk
        for (key, record) in &self.tracks {
            if let Some(path) = record.resolve_path(&self.root) {
                if path.exists() {
                    let mut track = track_from_record(record, &path)?;
                    // Read Serato tags from the audio file
                    tags::read_serato_tags(&mut track, &path);
                    track_by_path.insert(path.clone(), track.id);
                    track_by_key.insert(*key, track.id);
                    tracks.push(track);
                }
            }
        }

        let mut lists = Vec::new();

        // Regular crates
        if groups.contains(&CRATES) {
            for krate in &self.crates {
                lists.extend(crate::crates::crate_to_lists(
                    krate,
                    &track_by_path,
                    &self.root,
                )?);
            }
        }

        // Smart crates
        if groups.contains(&SMART_CRATES) {
            for sc in &self.smart_crates {
                lists.extend(crate::smart::smart_crate_to_list(sc, &track_by_path)?);
            }
        }

        // History
        if groups.contains(&HISTORY) {
            for session in &self.history {
                lists.extend(crate::database::history_to_list(
                    session,
                    &track_by_path,
                    &track_by_key,
                )?);
            }
        }

        Ok(ImportedLibrary { tracks, lists })
    }
}

/// Converts a database TrackRecord into a DJ Sugar Track, reading basic
/// metadata from the database. Audio file tags (cues, loops, beatgrid, color)
/// are added later by `tags::read_serato_tags`.
fn track_from_record(record: &TrackRecord, path: &Path) -> Result<Track, String> {
    let mut info = LibraryInfo::default();

    info.comment = record.comment.clone();
    info.rating = record.rating;
    info.play_count = record.play_count as u32;
    info.added = record.added_date;
    info.color = record.color.map(|c| c as u32);
    info.bpm = record.bpm;
    info.key = record.key.clone();
    info.duration = record.duration;
    info.genre = record.genre.clone();
    info.grouping = record.grouping.clone();
    info.year = record.year.clone();
    info.label = record.label.clone();
    info.composer = record.composer.clone();
    info.remixer = record.remixer.clone();
    info.isrc = record.isrc.clone();

    // Fixed grid from Serato
    info.fixed_grid = record.bpm > 0.0;
    info.grid_from = if record.bpm > 0.0 {
        "serato".into()
    } else {
        String::new()
    };

    Ok(Track {
        id: 0, // assigned by the library on import
        artist: record.artist.clone(),
        title: record.title.clone(),
        album: record.album.clone(),
        genre: record.genre.clone(),
        rating: record.rating,
        bpm: record.bpm,
        key: record.key.clone(),
        duration: record.duration,
        first_beat: record.first_beat,
        grid: BeatGrid::default(),
        source: Source::File(path.to_path_buf()),
        waveform: Default::default(),
        audio: None,
        cover: None,
        info,
    })
}

fn modified(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    /// Helper to write a big-endian u32.
    fn be_u32(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }

    /// Helper to write a big-endian u16.
    fn be_u16(v: u16) -> [u8; 2] {
        v.to_be_bytes()
    }

    /// Writes a TLV tag: 4-byte tag, 4-byte BE length, value.
    fn tlv(tag: &[u8; 4], value: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(tag);
        out.extend_from_slice(&be_u32(value.len() as u32));
        out.extend_from_slice(value);
        out
    }

    /// Writes a UTF-16BE string (no BOM, no null terminator in length).
    fn utf16be(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|c| c.to_be_bytes()).collect()
    }

    #[test]
    fn test_database_parsing() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("database V2");

        // Build a minimal database V2 with one track
        let mut data = Vec::new();
        // vrsn tag
        data.extend_from_slice(&tlv(b"vrsn", &utf16be("2.0")));
        // otrk with one track
        let mut track_data = Vec::new();
        track_data.extend_from_slice(&tlv(b"pfil", &utf16be("Music/Track.mp3")));
        track_data.extend_from_slice(&tlv(b"tsng", &utf16be("Test Track")));
        track_data.extend_from_slice(&tlv(b"tart", &utf16be("Test Artist")));
        track_data.extend_from_slice(&tlv(b"talb", &utf16be("Test Album")));
        track_data.extend_from_slice(&tlv(b"tgen", &utf16be("Electronic")));
        // tbpm: 4-byte float (big-endian IEEE 754)
        track_data.extend_from_slice(&tlv(b"tbpm", &128.0f32.to_be_bytes()));
        // tkey: key string
        track_data.extend_from_slice(&tlv(b"tkey", &utf16be("8A")));
        // tlen: duration in seconds (float)
        track_data.extend_from_slice(&tlv(b"tlen", &240.0f32.to_be_bytes()));
        // trating: 0-5
        track_data.extend_from_slice(&tlv(b"trating", &be_u32(4)));
        // tplay: play count
        track_data.extend_from_slice(&tlv(b"tplay", &be_u32(10)));
        // tadd: added date (seconds since 2001-01-01)
        track_data.extend_from_slice(&tlv(b"tadd", &be_u32(3600 * 24 * 365))); // ~2002
                                                                               // tcol: color (RGB packed)
        track_data.extend_from_slice(&tlv(b"tcol", &be_u32(0xFF0000)));
        // tcom: comment
        track_data.extend_from_slice(&tlv(b"tcom", &utf16be("Great track")));
        // tgrp: grouping
        track_data.extend_from_slice(&tlv(b"tgrp", &utf16be("My Group")));
        // tyer: year
        track_data.extend_from_slice(&tlv(b"tyer", &utf16be("2023")));
        // tlab: label
        track_data.extend_from_slice(&tlv(b"tlab", &utf16be("Test Label")));
        // tcmp: composer
        track_data.extend_from_slice(&tlv(b"tcmp", &utf16be("Test Composer")));
        // trmx: remixer
        track_data.extend_from_slice(&tlv(b"trmx", &utf16be("Test Remixer")));
        // tisrc: ISRC
        track_data.extend_from_slice(&tlv(b"tisrc", &utf16be("USRC12345678")));

        data.extend_from_slice(&tlv(b"otrk", &track_data));

        std::fs::write(&db_path, &data).unwrap();

        let (tracks, _version) = database::read_database(&db_path).unwrap();
        assert_eq!(tracks.len(), 1);
        let (key, record) = tracks.into_iter().next().unwrap();
        assert_eq!(record.title, "Test Track");
        assert_eq!(record.artist, "Test Artist");
        assert_eq!(record.album, "Test Album");
        assert_eq!(record.genre, "Electronic");
        assert!((record.bpm - 128.0).abs() < 0.01);
        assert_eq!(record.key, "8A");
        assert!((record.duration - 240.0).abs() < 0.01);
        assert_eq!(record.rating, 4);
        assert_eq!(record.play_count, 10);
        assert!(record.added_date > 0);
        assert_eq!(record.color, Some(0xFF0000));
        assert_eq!(record.comment, "Great track");
        assert_eq!(record.grouping, "My Group");
        assert_eq!(record.year, "2023");
        assert_eq!(record.label, "Test Label");
        assert_eq!(record.composer, "Test Composer");
        assert_eq!(record.remixer, "Test Remixer");
        assert_eq!(record.isrc, "USRC12345678");
        assert_eq!(record.path, "Music/Track.mp3");
    }

    #[test]
    fn test_crate_parsing() {
        let dir = tempdir().unwrap();
        let subcrate_dir = dir.path().join("Subcrates");
        std::fs::create_dir(&subcrate_dir).unwrap();
        let crate_path = subcrate_dir.join("Test Crate.crate");

        // Build a minimal crate file with one track reference
        let mut data = Vec::new();
        // vrsn
        data.extend_from_slice(&tlv(b"vrsn", &utf16be("2.0")));
        // osrt: sort order
        data.extend_from_slice(&tlv(b"osrt", &be_u32(0)));
        // ovct: column spec (simplified)
        data.extend_from_slice(&tlv(b"ovct", &[]));
        // otrk with ptrk
        let mut track_data = Vec::new();
        track_data.extend_from_slice(&tlv(b"ptrk", &utf16be("Music/Track.mp3")));
        data.extend_from_slice(&tlv(b"otrk", &track_data));

        std::fs::write(&crate_path, &data).unwrap();

        let crates = crates::read_crates(dir.path()).unwrap();
        assert_eq!(crates.len(), 1);
        assert_eq!(crates[0].name, "Test Crate");
        assert_eq!(crates[0].tracks.len(), 1);
        assert_eq!(crates[0].tracks[0], "Music/Track.mp3");
    }

    #[test]
    fn test_smart_crate_parsing() {
        let dir = tempdir().unwrap();
        let smart_dir = dir.path().join("SmartCrates");
        std::fs::create_dir(&smart_dir).unwrap();
        let sc_path = smart_dir.join("Smart Test.scrate");

        // Build a minimal smart crate
        let mut data = Vec::new();
        data.extend_from_slice(&tlv(b"vrsn", &utf16be("2.0")));
        data.extend_from_slice(&tlv(b"scnm", &utf16be("Smart Test")));
        // scrl: rule count
        data.extend_from_slice(&tlv(b"scrl", &be_u32(1)));
        // scr: one rule
        let mut rule_data = Vec::new();
        rule_data.extend_from_slice(&tlv(b"scrf", &be_u32(2))); // field: genre
        rule_data.extend_from_slice(&tlv(b"scro", &be_u32(0))); // op: contains
        rule_data.extend_from_slice(&tlv(b"scrv", &utf16be("House")));
        data.extend_from_slice(&tlv(b"scr", &rule_data));

        std::fs::write(&sc_path, &data).unwrap();

        let smart_crates = smart::read_smart_crates(dir.path()).unwrap();
        assert_eq!(smart_crates.len(), 1);
        assert_eq!(smart_crates[0].name, "Smart Test");
        assert_eq!(smart_crates[0].rules.len(), 1);
    }

    #[test]
    fn test_markers2_parsing() {
        // Test the Markers2 base64 decoding and parsing
        use crate::tags::parse_markers2;
        use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};

        // Build a simple Markers2 payload: COLOR + BPMLOCK
        let mut payload = Vec::new();
        payload.extend_from_slice(b"\x01\x01"); // header
                                                // COLOR entry
        payload.extend_from_slice(b"COLOR\0");
        payload.extend_from_slice(&[0, 0, 0, 4]); // length
        payload.extend_from_slice(&[0, 0xFF, 0x00, 0x00]); // RGB: red
                                                           // BPMLOCK entry
        payload.extend_from_slice(b"BPMLOCK\0");
        payload.extend_from_slice(&[0, 0, 0, 1]); // length
        payload.extend_from_slice(&[1]); // locked
        payload.push(0); // terminator

        let encoded = STANDARD_NO_PAD.encode(&payload);
        let geob_data = format!("application/octet-stream\0Serato Markers2\0{}", encoded);

        let (color, bpm_locked, cues, loops) = parse_markers2(&geob_data).unwrap();
        assert_eq!(color, Some(0xFF0000));
        assert_eq!(bpm_locked, true);
        assert!(cues.is_empty());
        assert!(loops.is_empty());
    }

    #[test]
    fn test_beatgrid_parsing() {
        use crate::tags::parse_beatgrid;
        use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};

        // Build a BeatGrid payload: header (2 bytes) + num_markers (4 bytes) + one terminal marker + footer
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0x01, 0x00]); // header
        payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // 1 marker
                                                              // Terminal marker: position (float32), bpm (float32)
        payload.extend_from_slice(&1.5f32.to_be_bytes()); // position 1.5s
        payload.extend_from_slice(&128.0f32.to_be_bytes()); // 128 BPM
        payload.push(0x37); // footer

        let encoded = STANDARD_NO_PAD.encode(&payload);
        let geob_data = format!("application/octet-stream\0Serato BeatGrid\0{}", encoded);

        let grid = parse_beatgrid(&geob_data).unwrap();
        assert_eq!(grid.markers.len(), 1);
        assert!((grid.markers[0].seconds - 1.5).abs() < 0.001);
        assert!((grid.markers[0].bpm - 128.0).abs() < 0.001);
    }

    #[test]
    fn test_autotags_parsing() {
        use crate::tags::parse_autotags;

        // Build an Autotags payload
        let mut payload = Vec::new();
        payload.extend_from_slice(b"\x01\x01"); // header
        payload.extend_from_slice(b"128.00\0"); // BPM
        payload.extend_from_slice(b"-3.257\0"); // Auto Gain
        payload.extend_from_slice(b"0.000\0"); // Gain dB

        let geob_data = format!(
            "application/octet-stream\0Serato Autotags\0{}",
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(&payload)
        );

        let (bpm, auto_gain, gain_db) = parse_autotags(&geob_data).unwrap();
        assert!((bpm - 128.0).abs() < 0.01);
        assert!((auto_gain - (-3.257)).abs() < 0.001);
        assert!((gain_db - 0.0).abs() < 0.001);
    }
}
