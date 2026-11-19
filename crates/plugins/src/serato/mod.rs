//! Brings a Serato DJ library in: every track in `database V2` with its
//! tags, and the hot cues, saved loops, beat grid and colour Serato keeps
//! in each audio file (see [`tags`]); and Serato's crates, in their folders.
//! It reads Serato's files and never writes them.
//!
//! Serato keeps its library in a `_Serato_` folder: in the Music folder for
//! the system disk, and at the top of each other disk it has seen. Paths in
//! it are relative to the top of that disk.
//!
//! `database V2` and `Subcrates/*.crate` are runs of fields: a four-letter
//! name, a big-endian `u32` length, then the value. The name's first letter
//! says what the value is: `o` fields hold more fields (`otrk` a track),
//! `t` and `p` are UTF-16BE text and paths, `u` a `u32`, `s` a `u16`, `b` a
//! flag. A crate named `House%%Deep.crate` is the crate Deep in House.
//! Smart crates are in `SmartCrates` (see [`smart`]), and the sessions
//! played in `History/Sessions` (see [`history`]).

pub mod export;
pub mod history;
pub mod smart;
pub mod tags;

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::import::{ImportedEntry, ImportedListKind};
use djsugar_engine::{LibraryInfo, Source, Track};

use crate::{ImportedLibrary, ImportedList, LibraryImport, ListGroup};

pub const CRATES: &str = "crates";
pub const HISTORY: &str = "history";

pub struct Serato;

/// The `database V2` in the system disk's `_Serato_` folder (or
/// `DJSUGAR_SERATO_DATABASE`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_SERATO_DATABASE") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;
    let file = home.join("Music").join("_Serato_").join("database V2");
    file.is_file().then_some(file)
}

impl LibraryImport for Serato {
    fn id(&self) -> &str {
        "serato"
    }

    fn title(&self) -> &str {
        "Serato DJ"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn missing_hint(&self) -> &str {
        "Serato keeps its library in the _Serato_ folder in your Music folder \
         (and at the top of each other drive it has used). Choose the \
         \"database V2\" file in it."
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: CRATES,
                title: "Crates",
                detail: "Your Serato crates and smart crates, in their folders.",
                on_by_default: true,
            },
            ListGroup {
                id: HISTORY,
                title: "History",
                detail: "The sessions you played, with when each track started.",
                on_by_default: true,
            },
        ]
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        let tracks = records(&load(database)?, b"otrk").count();
        let crates = crate_files(database).len() + smart::files(database).len();
        let n = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        let sessions = history::files(database).len();
        Ok(format!(
            "{}, {} and {}.",
            n(tracks, "track", "tracks"),
            n(crates, "crate", "crates"),
            n(sessions, "history session", "history sessions")
        ))
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = read(database, groups, Path::exists)?;
        eprintln!(
            "Serato library: {} tracks here, {} crates",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}

/// The tracks whose files `exists` says are here, with what their files'
/// tags hold, and the crates if `groups` asks for them.
pub fn read(
    database: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let data = load(database)?;
    let disk = disk_of(database);
    let mut tracks = Vec::new();
    for rec in records(&data, b"otrk") {
        let Some(t) = track(rec, &disk) else {
            continue;
        };
        let Source::File(file) = &t.source else {
            continue;
        };
        if exists(file) {
            let mut t = t;
            tags::apply(&mut t);
            tracks.push(t);
        }
    }
    let mut lists: Vec<ImportedList> = if groups.contains(&CRATES) {
        let crates = crate_files(database);
        let smart = smart::files(database);
        crates
            .iter()
            .filter_map(|f| crate_list(f, &disk))
            .chain(smart.iter().filter_map(|f| smart::list(f, &disk)))
            .collect()
    } else {
        Vec::new()
    };
    if groups.contains(&HISTORY) {
        lists.extend(
            history::files(database)
                .iter()
                .filter_map(|f| history::session(f, &disk)),
        );
    }
    Ok(ImportedLibrary {
        tracks,
        lists,
        written: None,
    })
}

fn load(path: &Path) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    if fields(&data).next().is_none_or(|(name, _)| name != b"vrsn") {
        return Err("not a Serato library (database V2)".into());
    }
    Ok(data)
}

/// The top of the disk the `_Serato_` folder holding `database` is on: its
/// paths are relative to that.
fn disk_of(database: &Path) -> PathBuf {
    let folder = database.parent().unwrap_or(Path::new(""));
    let top = folder.parent().unwrap_or(Path::new(""));
    // On another disk the folder is at the disk's top; on the system disk
    // it is in the Music folder.
    if top.parent().is_some() && folder.parent().and_then(Path::file_name) == Some("Music".as_ref())
    {
        let mut root = PathBuf::new();
        for c in database.components() {
            match c {
                Component::Prefix(_) | Component::RootDir => root.push(c),
                _ => break,
            }
        }
        root
    } else {
        top.to_path_buf()
    }
}

/// The fields in `data`, in order. A field running past the end ends it.
fn fields(data: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let head = data.get(at..at + 8)?;
        let len = u32::from_be_bytes([head[4], head[5], head[6], head[7]]) as usize;
        let value = data.get(at + 8..(at + 8).checked_add(len)?)?;
        at += 8 + len;
        Some((&head[..4], value))
    })
}

/// The values of the fields called `name` at the top of `data`.
fn records<'a>(data: &'a [u8], name: &'a [u8]) -> impl Iterator<Item = &'a [u8]> {
    fields(data)
        .filter(move |(n, _)| *n == name)
        .map(|(_, v)| v)
}

fn text(value: &[u8]) -> String {
    let units: Vec<u16> = value
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_be_bytes(c))
        .collect();
    String::from_utf16_lossy(&units)
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

fn number(value: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(value.try_into().ok()?))
}

/// A path as Serato keeps it, relative to the top of `disk`.
fn file(path: &str, disk: &Path) -> PathBuf {
    let rel = path.trim_start_matches(['/', '\\']);
    disk.join(rel.replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR))
}

/// "4:23.12" or "263.12" as seconds.
fn length(text: &str) -> f64 {
    let mut seconds = 0.0;
    for part in text.split(':') {
        seconds = seconds * 60.0 + part.trim().parse::<f64>().unwrap_or(0.0);
    }
    seconds.max(0.0)
}

fn track(rec: &[u8], disk: &Path) -> Option<Track> {
    let mut t = Track {
        id: 0,
        artist: String::new(),
        title: String::new(),
        album: String::new(),
        genre: String::new(),
        rating: 0,
        bpm: 0.0,
        key: String::new(),
        duration: 0.0,
        first_beat: 0.0,
        grid: Default::default(),
        source: Source::Synth { seed: 0 },
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo::default(),
    };
    let mut path = None;
    for (name, v) in fields(rec) {
        match name {
            b"pfil" => path = Some(file(&text(v), disk)),
            b"tsng" => t.title = text(v),
            b"tart" => t.artist = text(v),
            b"talb" => t.album = text(v),
            b"tgen" => t.genre = text(v),
            b"tbpm" => t.bpm = text(v).parse::<f64>().unwrap_or(0.0).max(0.0),
            b"tkey" => t.key = text(v),
            b"tlen" => t.duration = length(&text(v)),
            b"tcom" => t.info.comment = text(v),
            b"tlbl" => t.info.label = text(v),
            b"tcmp" => t.info.composer = text(v),
            b"tgrp" => t.info.grouping = text(v),
            b"ttyr" => {
                let y = text(v);
                if y.len() >= 4 && y[..4].bytes().all(|b| b.is_ascii_digit()) {
                    t.info.year = y[..4].to_string();
                }
            }
            b"uadd" => t.info.added = number(v).map_or(0, i64::from),
            // Serato's default is white: no colour.
            b"ulbl" => t.info.color = number(v).map(|c| c & 0xFF_FFFF).filter(|&c| c != 0xFF_FFFF),
            _ => {}
        }
    }
    t.source = Source::File(path?);
    Some(t)
}

/// The `.crate` files in the `Subcrates` folder next to `database`.
fn crate_files(database: &Path) -> Vec<PathBuf> {
    let dir = database.parent().unwrap_or(Path::new("")).join("Subcrates");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "crate"))
        .collect();
    files.sort();
    files
}

/// A crate: its folders and name from the file's name, its tracks in order.
fn crate_list(path: &Path, disk: &Path) -> Option<ImportedList> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let mut parts: Vec<String> = stem.split("%%").map(str::to_string).collect();
    let name = parts.pop()?;
    let data = std::fs::read(path).ok()?;
    let entries = records(&data, b"otrk")
        .filter_map(|r| {
            let (_, v) = fields(r).find(|(n, _)| *n == b"ptrk")?;
            Some(ImportedEntry::new(file(&text(v), disk), None))
        })
        .collect();
    Some(ImportedList {
        folders: parts,
        name,
        kind: ImportedListKind::List,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::export::{field, utf16};
    use super::*;

    fn library(dir: &Path) -> PathBuf {
        let serato = dir.join("_Serato_");
        std::fs::create_dir_all(serato.join("Subcrates")).unwrap();
        let mut db = field(b"vrsn", &utf16("2.0/Serato Scratch LIVE Database"));
        let mut rec = Vec::new();
        for (n, v) in [
            (b"pfil", "Sets/Glue.mp3"),
            (b"tsng", "Glue"),
            (b"tart", "Bicep"),
            (b"tgen", "Electronic"),
            (b"tbpm", "130.00"),
            (b"tkey", "Am"),
            (b"tlen", "04:28.90"),
            (b"ttyr", "2017"),
        ] {
            rec.extend(field(n, &utf16(v)));
        }
        rec.extend(field(b"uadd", &1_649_462_400u32.to_be_bytes()));
        rec.extend(field(b"ulbl", &0x99_FF99u32.to_be_bytes()));
        db.extend(field(b"otrk", &rec));
        db.extend(field(b"otrk", &field(b"pfil", &utf16("Sets/Gone.mp3"))));
        // A broken last field is left out, not an error.
        db.extend(b"otrk\0\0\x10\0");
        std::fs::write(serato.join("database V2"), db).unwrap();

        let mut c = field(b"vrsn", &utf16("1.0/Serato ScratchLive Crate"));
        for p in ["Sets/Gone.mp3", "Sets/Glue.mp3"] {
            c.extend(field(b"otrk", &field(b"ptrk", &utf16(p))));
        }
        std::fs::write(serato.join("Subcrates").join("House%%Deep.crate"), c).unwrap();
        serato.join("database V2")
    }

    #[test]
    fn reads_tracks_and_crates() {
        let dir = std::env::temp_dir().join(format!("djs-serato-{}", std::process::id()));
        let db = library(&dir);
        let glue = dir.join("Sets").join("Glue.mp3");
        let lib = read(&db, &[CRATES], |p: &Path| p == glue).unwrap();
        let preview = Serato.preview(&db).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(preview, "2 tracks, 1 crate and 0 history sessions.");
        assert_eq!(lib.tracks.len(), 1);
        let t = &lib.tracks[0];
        assert!(matches!(&t.source, Source::File(p) if *p == glue));
        assert_eq!((t.title.as_str(), t.artist.as_str()), ("Glue", "Bicep"));
        assert_eq!((t.genre.as_str(), t.key.as_str()), ("Electronic", "Am"));
        assert_eq!(t.bpm, 130.0);
        assert!((t.duration - 268.9).abs() < 1e-9);
        assert_eq!(t.info.year, "2017");
        assert_eq!(t.info.added, 1_649_462_400);
        assert_eq!(t.info.color, Some(0x99_FF99));

        assert_eq!(lib.lists.len(), 1);
        let l = &lib.lists[0];
        assert_eq!(
            (l.folders.clone(), l.name.as_str()),
            (vec!["House".to_string()], "Deep")
        );
        let files: Vec<PathBuf> = l.entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(files, [dir.join("Sets").join("Gone.mp3"), glue]);
    }

    #[test]
    fn paths_are_from_the_top_of_the_disk() {
        let sys = Path::new("/Users/dj/Music/_Serato_/database V2");
        assert_eq!(disk_of(sys), Path::new("/"));
        let usb = Path::new("/Volumes/USB/_Serato_/database V2");
        assert_eq!(disk_of(usb), Path::new("/Volumes/USB"));
    }

    #[test]
    fn lengths() {
        assert_eq!(length("04:28.50"), 268.5);
        assert_eq!(length("1:02:03"), 3723.0);
        assert_eq!(length(""), 0.0);
    }
}
