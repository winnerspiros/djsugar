//! Writes the library as an iTunes / Music Library XML: the tracks with
//! their tags, ratings and play counts, and the lists in their folders.
//! rekordbox, Serato, Traktor, VirtualDJ and djay all read this file (it's
//! their "iTunes" library), and the Music app imports it with File ›
//! Library › Import Playlist….
//!
//! The layout is the one [`crate::itunes_library`] reads: an Apple
//! property list with `Tracks` (a dictionary by `Track ID`) and
//! `Playlists` (the hidden master list first, then folders before what
//! they hold, each naming its tracks by `Track ID`). It has no place for
//! cues or beat grids.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use djsugar_engine::{Source, Track};
use plist::{Dictionary, Value};

use crate::rekordbox::xml_export::location;

/// A playlist or folder, in the order they're shown, with its folder.
pub struct ItunesList {
    pub name: String,
    pub folder: bool,
    /// Index into the lists of the folder it's in.
    pub parent: Option<usize>,
    /// Indexes into the tracks.
    pub tracks: Vec<usize>,
}

/// A persistent ID: 16 hex digits, the same for the same text every time,
/// so writing the file again names everything as before.
fn persistent_id(kind: &str, text: &str) -> String {
    // FNV-1a, 64 bits.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in kind.bytes().chain([0]).chain(text.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016X}")
}

fn date(seconds: i64) -> Value {
    let t = UNIX_EPOCH + Duration::from_secs(seconds.max(0) as u64);
    Value::Date(t.into())
}

fn int(n: impl Into<i64>) -> Value {
    Value::Integer(n.into().into())
}

/// The file kind iTunes shows for an extension.
fn kind(path: &Path) -> String {
    match path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("mp3") => "MPEG audio file".into(),
        Some("m4a" | "mp4" | "aac") => "AAC audio file".into(),
        Some("aif" | "aiff") => "AIFF audio file".into(),
        Some("wav") => "WAV audio file".into(),
        Some(e) => format!("{} audio file", e.to_ascii_uppercase()),
        None => "Audio file".into(),
    }
}

fn track(id: i64, t: &Track, file: &Path) -> Dictionary {
    let mut d = Dictionary::new();
    let mut text = |key: &str, value: &str| {
        if !value.is_empty() {
            d.insert(key.into(), Value::String(value.into()));
        }
    };
    text("Name", &t.title);
    text("Artist", &t.artist);
    text("Album", &t.album);
    text("Genre", &t.genre);
    text("Comments", &t.info.comment);
    text("Grouping", &t.info.grouping);
    text("Composer", &t.info.composer);
    d.insert("Track ID".into(), int(id));
    d.insert("Kind".into(), Value::String(kind(file)));
    if t.duration > 0.0 {
        d.insert(
            "Total Time".into(),
            int((t.duration * 1000.0).round() as i64),
        );
    }
    if let Ok(year) = t.info.year.trim().parse::<i64>() {
        d.insert("Year".into(), int(year));
    }
    for (key, n) in [
        ("Track Number", t.info.track_number),
        ("Disc Number", t.info.disc_number),
        ("Play Count", t.info.play_count),
    ] {
        if n > 0 {
            d.insert(key.into(), int(n));
        }
    }
    if t.bpm > 0.0 {
        d.insert("BPM".into(), int(t.bpm.round() as i64));
    }
    if t.rating > 0 {
        d.insert("Rating".into(), int(i64::from(t.rating.min(5)) * 20));
    }
    if t.info.added > 0 {
        d.insert("Date Added".into(), date(t.info.added));
    }
    if let Some(played) = t.info.last_played.filter(|&p| p > 0) {
        d.insert("Play Date UTC".into(), date(played));
    }
    d.insert(
        "Persistent ID".into(),
        Value::String(persistent_id("track", &file.to_string_lossy())),
    );
    d.insert("Track Type".into(), Value::String("File".into()));
    d.insert("Location".into(), Value::String(location(file)));
    d
}

/// The library as Library XML text.
pub fn render(tracks: &[Track], lists: &[ItunesList]) -> Result<Vec<u8>, String> {
    // Track IDs as iTunes numbers them: from 1, files only.
    let mut ids: Vec<Option<i64>> = vec![None; tracks.len()];
    let mut all = Dictionary::new();
    let mut next = 1;
    for (i, t) in tracks.iter().enumerate() {
        let Source::File(file) = &t.source else {
            continue;
        };
        ids[i] = Some(next);
        all.insert(next.to_string(), Value::Dictionary(track(next, t, file)));
        next += 1;
    }
    let items = |indexes: &mut dyn Iterator<Item = usize>| {
        Value::Array(
            indexes
                .filter_map(|i| *ids.get(i)?)
                .map(|id| {
                    let mut d = Dictionary::new();
                    d.insert("Track ID".into(), int(id));
                    Value::Dictionary(d)
                })
                .collect(),
        )
    };
    let list_id = |i: usize| {
        let mut path = lists[i].name.clone();
        let mut up = lists[i].parent;
        while let Some(p) = up {
            path = format!("{}\u{1f}{path}", lists[p].name);
            up = lists[p].parent;
        }
        persistent_id(if lists[i].folder { "folder" } else { "list" }, &path)
    };
    let mut playlists = Vec::with_capacity(lists.len() + 1);
    let mut master = Dictionary::new();
    master.insert("Name".into(), Value::String("Library".into()));
    master.insert("Master".into(), Value::Boolean(true));
    master.insert("Visible".into(), Value::Boolean(false));
    master.insert("All Items".into(), Value::Boolean(true));
    master.insert(
        "Playlist Persistent ID".into(),
        Value::String(persistent_id("master", "")),
    );
    master.insert("Playlist Items".into(), items(&mut (0..tracks.len())));
    playlists.push(Value::Dictionary(master));
    // Each folder, then what's in it, in the DJ's order (iTunes wants a
    // folder before what it holds).
    let mut order = Vec::with_capacity(lists.len());
    let mut stack: Vec<usize> = (0..lists.len())
        .rev()
        .filter(|&i| lists[i].parent.is_none_or(|p| p >= lists.len()))
        .collect();
    while let Some(i) = stack.pop() {
        order.push(i);
        stack.extend(
            (0..lists.len())
                .rev()
                .filter(|&c| lists[c].parent == Some(i)),
        );
    }
    for i in order {
        let l = &lists[i];
        let mut d = Dictionary::new();
        d.insert("Name".into(), Value::String(l.name.clone()));
        d.insert("Playlist ID".into(), int(next + i as i64));
        d.insert("Playlist Persistent ID".into(), Value::String(list_id(i)));
        if let Some(p) = l.parent {
            d.insert("Parent Persistent ID".into(), Value::String(list_id(p)));
        }
        d.insert("All Items".into(), Value::Boolean(true));
        if l.folder {
            d.insert("Folder".into(), Value::Boolean(true));
        } else {
            d.insert(
                "Playlist Items".into(),
                items(&mut l.tracks.iter().copied()),
            );
        }
        playlists.push(Value::Dictionary(d));
    }
    let mut root = Dictionary::new();
    root.insert("Major Version".into(), int(1));
    root.insert("Minor Version".into(), int(1));
    root.insert("Date".into(), Value::Date(SystemTime::now().into()));
    root.insert("Application Version".into(), Value::String("12.0".into()));
    root.insert("Features".into(), int(5));
    root.insert("Show Content Ratings".into(), Value::Boolean(true));
    root.insert(
        "Library Persistent ID".into(),
        Value::String(persistent_id("library", "DJ Sugar")),
    );
    root.insert("Tracks".into(), Value::Dictionary(all));
    root.insert("Playlists".into(), Value::Array(playlists));
    let mut out = Vec::new();
    Value::Dictionary(root)
        .to_writer_xml(&mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Writes the file, replacing one there only once it's whole.
pub fn write(path: &Path, tracks: &[Track], lists: &[ItunesList]) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let text = render(tracks, lists).map_err(|e| fail(&e))?;
    let part = path.with_extension("xml.part");
    std::fs::write(&part, text).map_err(|e| fail(&e))?;
    std::fs::rename(&part, path).map_err(|e| fail(&e))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use djsugar_engine::LibraryInfo;

    use super::*;
    use crate::itunes_library::{read, PLAYLISTS};

    fn track(path: &str, title: &str) -> Track {
        Track {
            id: 0,
            artist: "Artist & Co".into(),
            title: title.into(),
            album: "Album".into(),
            genre: "House".into(),
            rating: 4,
            bpm: 124.4,
            key: String::new(),
            duration: 201.5,
            first_beat: 0.0,
            grid: Default::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: LibraryInfo {
                comment: "<great>".into(),
                play_count: 3,
                year: "2021".into(),
                track_number: 7,
                added: 1_600_000_000,
                ..Default::default()
            },
        }
    }

    /// Written, then read back by our iTunes importer.
    #[test]
    fn reads_back() {
        let tracks = [
            track("/music/a b.mp3", "Á"),
            track("/music/c#.flac", "C"),
            track("/music/d.wav", "D"),
        ];
        let lists = [
            ItunesList {
                name: "Friday".into(),
                folder: false,
                parent: Some(1),
                tracks: vec![2, 0],
            },
            ItunesList {
                name: "Gigs".into(),
                folder: true,
                parent: None,
                tracks: Vec::new(),
            },
            ItunesList {
                name: "Warm up".into(),
                folder: false,
                parent: None,
                tracks: vec![1],
            },
        ];
        let dir = std::env::temp_dir().join(format!("djs-itunes-x-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Library.xml");
        write(&path, &tracks, &lists).unwrap();
        let first = std::fs::read(&path).unwrap();
        let lib = read(&path, &[PLAYLISTS], |_| true).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(lib.tracks.len(), 3);
        let a = &lib.tracks[0];
        assert!(matches!(&a.source, Source::File(f) if f == Path::new("/music/a b.mp3")));
        assert_eq!((a.title.as_str(), a.artist.as_str()), ("Á", "Artist & Co"));
        assert_eq!((a.rating, a.bpm, a.duration), (4, 124.0, 201.5));
        assert_eq!(a.info.comment, "<great>");
        assert_eq!((a.info.play_count, a.info.track_number), (3, 7));
        assert_eq!(
            (a.info.year.as_str(), a.info.added),
            ("2021", 1_600_000_000)
        );
        let got: Vec<(Vec<String>, String, Vec<PathBuf>)> = lib
            .lists
            .iter()
            .map(|l| {
                (
                    l.folders.clone(),
                    l.name.clone(),
                    l.entries.iter().map(|e| e.path.clone()).collect(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (
                    vec!["Gigs".to_string()],
                    "Friday".to_string(),
                    vec![
                        PathBuf::from("/music/d.wav"),
                        PathBuf::from("/music/a b.mp3")
                    ]
                ),
                (
                    Vec::new(),
                    "Warm up".to_string(),
                    vec![PathBuf::from("/music/c#.flac")]
                ),
            ]
        );
        // The same names every time, except the file's own date.
        let again = render(&tracks, &lists).unwrap();
        let strip = |b: &[u8]| {
            let s = String::from_utf8(b.to_vec()).unwrap();
            let at = s.find("<key>Date</key>").unwrap();
            let end = at + s[at..].find("</date>").unwrap();
            format!("{}{}", &s[..at], &s[end..])
        };
        assert_eq!(strip(&first), strip(&again));
    }
}
