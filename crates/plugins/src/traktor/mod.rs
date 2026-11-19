//! Brings a Traktor library into the collection at start, when the DJ turns
//! it on in Settings > Library: the tracks in `collection.nml` with their
//! tags, hot cues, saved loops and beat grid, and the playlist tree, folders
//! within folders as Traktor has them. It only reads the file, never writes
//! it.
//!
//! `collection.nml` is XML. Each `COLLECTION/ENTRY` is a track: `LOCATION`
//! names its file (`VOLUME`, `DIR` with `/:` between folders, `FILE`),
//! `INFO` its tags, `TEMPO` its tempo, `MUSICAL_KEY` its key (0 to 23), and
//! each `CUE_V2` a cue, loop or grid marker, its `START` and `LEN` in
//! milliseconds. `PLAYLISTS` holds a tree of `NODE`s: `TYPE="FOLDER"` with
//! `SUBNODES`, `TYPE="PLAYLIST"` whose `PRIMARYKEY`s name tracks by
//! volume, folder and file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::import::{ImportedEntry, ImportedList, ImportedListKind};
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track, MEMORY_CUE};
use roxmltree::{Document, Node};

use crate::{ImportedLibrary, LibraryImport, ListGroup};

pub const PLAYLISTS: &str = "playlists";
pub const HISTORY: &str = "history";

pub mod history;
pub mod nml_export;

pub struct Traktor;

/// Traktor's collection where Traktor keeps it, the newest version's (or
/// `DJSUGAR_TRAKTOR_COLLECTION`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_TRAKTOR_COLLECTION") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;
    newest_collection(&home.join("Documents").join("Native Instruments"))
}

/// The `collection.nml` of the newest "Traktor x.y.z" folder in `dir`.
fn newest_collection(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<(Vec<u32>, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let version = name.strip_prefix("Traktor")?.trim();
            let file = e.path().join("collection.nml");
            file.is_file().then(|| {
                let v = version
                    .split('.')
                    .map(|n| n.trim().parse().unwrap_or(0))
                    .collect();
                (v, file)
            })
        })
        .collect();
    found.sort();
    found.pop().map(|(_, f)| f)
}

impl LibraryImport for Traktor {
    fn id(&self) -> &str {
        "traktor"
    }

    fn title(&self) -> &str {
        "Traktor"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: PLAYLISTS,
                title: "Playlists",
                detail: "Your Traktor playlists, in their folders.",
                on_by_default: true,
            },
            ListGroup {
                id: HISTORY,
                title: "History",
                detail: "The sessions you played, from Traktor's History folder.",
                on_by_default: true,
            },
        ]
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        preview(database)
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = read(database, groups, Path::exists)?;
        eprintln!(
            "Traktor library: {} tracks here, {} lists",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}

fn load(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))
}

fn document(text: &str) -> Result<Document<'_>, String> {
    let doc = Document::parse(text).map_err(|e| e.to_string())?;
    if !doc.root_element().has_tag_name("NML") {
        return Err("not a Traktor collection (collection.nml)".into());
    }
    Ok(doc)
}

/// "1,204 tracks, 37 playlists and 12 history sessions."
pub fn preview(path: &Path) -> Result<String, String> {
    let text = load(path)?;
    let doc = document(&text)?;
    let tracks = collection(&doc).count();
    let lists = playlists(&doc, &HashMap::new()).len();
    let n = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    let sessions = history::files(path).len();
    Ok(format!(
        "{}, {} and {}.",
        n(tracks, "track", "tracks"),
        n(lists, "playlist", "playlists"),
        n(sessions, "history session", "history sessions")
    ))
}

/// The tracks whose files `exists` says are here, and the playlists in
/// `groups`.
pub fn read(
    path: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let text = load(path)?;
    let doc = document(&text)?;
    // Playlists name a track by its key: volume, folders and file.
    let mut files: HashMap<String, PathBuf> = HashMap::new();
    let mut tracks = Vec::new();
    for n in collection(&doc) {
        let Some((key, file)) = location(n, &exists) else {
            continue;
        };
        if exists(&file) {
            tracks.push(track(n, file.clone()));
        }
        files.insert(key, file);
    }
    let mut lists = if groups.contains(&PLAYLISTS) {
        playlists(&doc, &files)
    } else {
        Vec::new()
    };
    if groups.contains(&HISTORY) {
        lists.extend(
            history::files(path)
                .iter()
                .filter_map(|f| history::session(f, &exists)),
        );
    }
    Ok(ImportedLibrary { tracks, lists })
}

fn collection<'a>(doc: &'a Document) -> impl Iterator<Item = Node<'a, 'a>> {
    doc.root_element()
        .children()
        .filter(|n| n.has_tag_name("COLLECTION"))
        .flat_map(|c| c.children().filter(|n| n.has_tag_name("ENTRY")))
}

fn child<'a>(n: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    n.children().find(|c| c.has_tag_name(name))
}

fn text(n: Option<Node>, name: &str) -> String {
    n.and_then(|n| n.attribute(name))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn number(n: Option<Node>, name: &str) -> Option<f64> {
    n?.attribute(name)?.trim().parse().ok()
}

/// A track's key as playlists name it ("Macintosh HD/:Users/:dj/:a.mp3"),
/// and its file. On a Mac the volume is the disk's name: the startup disk's
/// files are at the path itself, another disk's under /Volumes. On Windows
/// it is the drive ("C:").
fn location(n: Node, exists: &impl Fn(&Path) -> bool) -> Option<(String, PathBuf)> {
    let l = child(n, "LOCATION")?;
    let (volume, dir, file) = (
        l.attribute("VOLUME").unwrap_or_default(),
        l.attribute("DIR").unwrap_or_default(),
        l.attribute("FILE").unwrap_or_default(),
    );
    if file.is_empty() {
        return None;
    }
    let key = format!("{volume}{dir}{file}");
    let path = format!("{}{file}", dir.replace("/:", "/"));
    let drive =
        volume.len() == 2 && volume.ends_with(':') && volume.as_bytes()[0].is_ascii_alphabetic();
    let file = if drive {
        PathBuf::from(format!("{volume}{path}"))
    } else {
        let plain = PathBuf::from(&path);
        let other = Path::new("/Volumes")
            .join(volume)
            .join(path.trim_start_matches('/'));
        if !volume.is_empty() && !exists(&plain) && exists(&other) {
            other
        } else {
            plain
        }
    };
    Some((key, file))
}

/// Traktor's day ("2022/4/9") as a Unix time.
fn day(text: &str) -> Option<i64> {
    let mut parts = text.trim().splitn(3, '/');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    let iso = format!(
        "{:04}-{:02}-{:02}",
        y.parse::<u32>().ok()?,
        m.parse::<u32>().ok()?,
        d.parse::<u32>().ok()?
    );
    crate::virtualdj::lists::day_start(&iso)
}

/// `MUSICAL_KEY`'s value: 0 to 11 the major keys from C, 12 to 23 the
/// minor keys from C minor.
fn key_name(value: f64) -> Option<String> {
    const NOTES: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];
    let v = value as i64;
    if !(0..24).contains(&v) || value.fract() != 0.0 {
        return None;
    }
    let note = NOTES[(v % 12) as usize];
    Some(if v >= 12 {
        format!("{note}m")
    } else {
        note.to_string()
    })
}

/// Traktor's track colours, 1 to 7.
fn color(index: f64) -> Option<u32> {
    match index as i64 {
        1 => Some(0xE5_3935), // red
        2 => Some(0xFB_8C00), // orange
        3 => Some(0xFD_D835), // yellow
        4 => Some(0x43_A047), // green
        5 => Some(0x1E_88E5), // blue
        6 => Some(0x8E_24AA), // violet
        7 => Some(0xD8_1B60), // magenta
        _ => None,
    }
}

// CUE_V2 types.
const CUE: i64 = 0;
const FADE_IN: i64 = 1;
const FADE_OUT: i64 = 2;
const LOAD: i64 = 3;
const GRID: i64 = 4;
const LOOP: i64 = 5;

fn track(n: Node, file: PathBuf) -> Track {
    let info = child(n, "INFO");
    let album = child(n, "ALBUM");
    let bpm = number(child(n, "TEMPO"), "BPM").unwrap_or(0.0).max(0.0);
    let mut hotcues: Vec<Hotcue> = Vec::new();
    let mut markers = Vec::new();
    let mut grid = Vec::new();
    let mut memory = MEMORY_CUE;
    for c in n.children().filter(|c| c.has_tag_name("CUE_V2")) {
        let Some(start) = number(Some(c), "START") else {
            continue;
        };
        let seconds = start / 1000.0;
        let kind = number(Some(c), "TYPE").unwrap_or(0.0) as i64;
        let len = number(Some(c), "LEN").unwrap_or(0.0);
        let end = (kind == LOOP && len > 0.0).then(|| (start + len) / 1000.0);
        let name = text(Some(c), "NAME");
        // Traktor names cues it set itself "n.n.", grids "AutoGrid".
        let name = if name == "n.n." || name == "AutoGrid" {
            String::new()
        } else {
            name
        };
        if kind == GRID {
            grid.push(GridMarker {
                seconds,
                bpm: number(child(c, "GRID"), "BPM").unwrap_or(bpm),
            });
        }
        let slot = number(Some(c), "HOTCUE").unwrap_or(-1.0);
        let number = if slot >= 0.0 {
            slot as usize
        } else {
            match kind {
                CUE | LOAD | LOOP => {
                    memory += 1;
                    memory - 1
                }
                FADE_IN | FADE_OUT => {
                    markers.push(Marker {
                        kind: MarkerKind::Automix,
                        seconds,
                        end: None,
                        name: if kind == FADE_IN {
                            "realStart"
                        } else {
                            "fadeStart"
                        }
                        .into(),
                        color: None,
                        detected: false,
                    });
                    continue;
                }
                _ => continue,
            }
        };
        if !hotcues.iter().any(|h| h.number == number) {
            hotcues.push(Hotcue {
                number,
                seconds,
                name,
                end,
                color: None,
            });
        }
    }
    hotcues.sort_by_key(|h| h.number);
    grid.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    let key = number(child(n, "MUSICAL_KEY"), "VALUE")
        .and_then(key_name)
        .unwrap_or_else(|| text(info, "KEY"));
    let duration = number(info, "PLAYTIME_FLOAT")
        .or_else(|| number(info, "PLAYTIME"))
        .unwrap_or(0.0)
        .max(0.0);
    let year = text(info, "RELEASE_DATE")
        .split('/')
        .next()
        .filter(|y| y.len() == 4 && *y != "0000")
        .unwrap_or_default()
        .to_string();
    let mut t = Track {
        id: 0,
        artist: text(Some(n), "ARTIST"),
        title: text(Some(n), "TITLE"),
        album: text(album, "TITLE"),
        genre: text(info, "GENRE"),
        // 0 to 255, 51 a star.
        rating: (number(info, "RANKING").unwrap_or(0.0) / 51.0)
            .round()
            .clamp(0.0, 5.0) as u8,
        bpm,
        key,
        duration,
        first_beat: grid.first().map_or(0.0, |m| m.seconds),
        grid: Default::default(),
        source: Source::File(file),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo {
            comment: text(info, "COMMENT"),
            color: number(info, "COLOR").and_then(color),
            play_count: number(info, "PLAYCOUNT").unwrap_or(0.0).max(0.0) as u32,
            hotcues,
            fixed_grid: !grid.is_empty(),
            year,
            track_number: number(album, "TRACK").unwrap_or(0.0).max(0.0) as u32,
            label: text(info, "LABEL"),
            remixer: text(info, "REMIXER"),
            remix: text(info, "MIX"),
            composer: text(info, "PRODUCER"),
            added: info
                .and_then(|i| i.attribute("IMPORT_DATE"))
                .and_then(day)
                .unwrap_or(0),
            last_played: info.and_then(|i| i.attribute("LAST_PLAYED")).and_then(day),
            markers,
            ..Default::default()
        },
    };
    if !grid.is_empty() {
        t.set_grid(BeatGrid::new(grid));
    }
    t
}

/// The playlist tree under `$ROOT`, folders within folders, in Traktor's
/// order. Traktor's own lists (`_LOOPS`, `_RECORDINGS`) stay out, and so do
/// smart lists, whose searches DJ Sugar can't say yet.
fn playlists(doc: &Document, files: &HashMap<String, PathBuf>) -> Vec<ImportedList> {
    let mut out = Vec::new();
    let root = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("PLAYLISTS"))
        .and_then(|p| p.children().find(|n| n.has_tag_name("NODE")));
    if let Some(root) = root {
        walk(root, &mut Vec::new(), files, &mut out);
    }
    out
}

fn walk(
    folder: Node,
    path: &mut Vec<String>,
    files: &HashMap<String, PathBuf>,
    out: &mut Vec<ImportedList>,
) {
    let nodes = folder
        .children()
        .filter(|n| n.has_tag_name("SUBNODES"))
        .flat_map(|s| s.children().filter(|n| n.has_tag_name("NODE")));
    for n in nodes {
        let name = text(Some(n), "NAME");
        match n.attribute("TYPE") {
            Some("FOLDER") => {
                path.push(name);
                walk(n, path, files, out);
                path.pop();
            }
            Some("PLAYLIST") if !name.starts_with('_') => {
                let entries = child(n, "PLAYLIST")
                    .into_iter()
                    .flat_map(|p| p.children().filter(|e| e.has_tag_name("ENTRY")))
                    .filter_map(|e| {
                        let key = child(e, "PRIMARYKEY")?.attribute("KEY")?;
                        let file = files.get(key).cloned().unwrap_or_else(|| {
                            // A track that isn't in the collection: its key
                            // still names the file.
                            let dir = key.find("/:").map_or(key, |i| &key[i..]);
                            PathBuf::from(dir.replace("/:", "/"))
                        });
                        Some(ImportedEntry::new(file, None))
                    })
                    .collect();
                out.push(ImportedList {
                    folders: path.clone(),
                    name,
                    kind: ImportedListKind::List,
                    entries,
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<NML VERSION="19"><HEAD COMPANY="www.native-instruments.com" PROGRAM="Traktor"></HEAD>
<MUSICFOLDERS></MUSICFOLDERS>
<COLLECTION ENTRIES="3">
<ENTRY MODIFIED_DATE="2023/1/5" TITLE="Glue" ARTIST="Bicep">
<LOCATION DIR="/:Users/:dj/:Music/:" FILE="Bicep - Glue.mp3" VOLUME="Macintosh HD" VOLUMEID="abc"></LOCATION>
<ALBUM TRACK="3" TITLE="Bicep"></ALBUM>
<INFO BITRATE="320000" GENRE="Electronic" LABEL="Ninja Tune" COMMENT="nice" KEY="5m" PLAYCOUNT="7" PLAYTIME="269" PLAYTIME_FLOAT="268.900" RANKING="204" IMPORT_DATE="2022/4/9" LAST_PLAYED="2023/1/5" RELEASE_DATE="2017/1/1" COLOR="5" MIX="Original Mix"></INFO>
<TEMPO BPM="120.000000" BPM_QUALITY="100.000000"></TEMPO>
<MUSICAL_KEY VALUE="17"></MUSICAL_KEY>
<CUE_V2 NAME="AutoGrid" DISPL_ORDER="0" TYPE="4" START="25.000" LEN="0.000000" REPEATS="-1" HOTCUE="0"><GRID BPM="120.000000"></GRID></CUE_V2>
<CUE_V2 NAME="Drop" DISPL_ORDER="0" TYPE="0" START="32025.000" LEN="0.000000" REPEATS="-1" HOTCUE="2"></CUE_V2>
<CUE_V2 NAME="n.n." DISPL_ORDER="0" TYPE="5" START="64025.000" LEN="8000.000" REPEATS="-1" HOTCUE="3"></CUE_V2>
<CUE_V2 NAME="Out" DISPL_ORDER="0" TYPE="2" START="200000.000" LEN="0.000000" REPEATS="-1" HOTCUE="-1"></CUE_V2>
<CUE_V2 NAME="Mem" DISPL_ORDER="0" TYPE="0" START="1000.000" LEN="0.000000" REPEATS="-1" HOTCUE="-1"></CUE_V2>
</ENTRY>
<ENTRY TITLE="Other" ARTIST="X">
<LOCATION DIR="/:Music/:" FILE="other.mp3" VOLUME="C:"></LOCATION>
<INFO PLAYTIME="200"></INFO>
</ENTRY>
<ENTRY TITLE="Away" ARTIST="Y">
<LOCATION DIR="/:Music/:" FILE="away.mp3" VOLUME="USB"></LOCATION>
</ENTRY>
</COLLECTION>
<SETS ENTRIES="0"></SETS>
<PLAYLISTS>
<NODE TYPE="FOLDER" NAME="$ROOT"><SUBNODES COUNT="4">
<NODE TYPE="PLAYLIST" NAME="_LOOPS"><PLAYLIST ENTRIES="0" TYPE="LIST" UUID="1"></PLAYLIST></NODE>
<NODE TYPE="PLAYLIST" NAME="Preparation"><PLAYLIST ENTRIES="1" TYPE="LIST" UUID="2">
<ENTRY><PRIMARYKEY TYPE="TRACK" KEY="C:/:Music/:other.mp3"></PRIMARYKEY></ENTRY>
</PLAYLIST></NODE>
<NODE TYPE="FOLDER" NAME="Gigs"><SUBNODES COUNT="2">
<NODE TYPE="FOLDER" NAME="2023"><SUBNODES COUNT="1">
<NODE TYPE="PLAYLIST" NAME="Friday"><PLAYLIST ENTRIES="2" TYPE="LIST" UUID="3">
<ENTRY><PRIMARYKEY TYPE="TRACK" KEY="Macintosh HD/:Users/:dj/:Music/:Bicep - Glue.mp3"></PRIMARYKEY></ENTRY>
<ENTRY><PRIMARYKEY TYPE="TRACK" KEY="USB/:Music/:away.mp3"></PRIMARYKEY></ENTRY>
</PLAYLIST></NODE>
</SUBNODES></NODE>
<NODE TYPE="FOLDER" NAME="Empty"><SUBNODES COUNT="0"></SUBNODES></NODE>
</SUBNODES></NODE>
<NODE TYPE="SMARTLIST" NAME="Techno"><SMARTLIST UUID="4"><SEARCH_EXPRESSION VERSION="1" QUERY="$GENRE % &quot;Techno&quot;"></SEARCH_EXPRESSION></SMARTLIST></NODE>
</SUBNODES></NODE>
</PLAYLISTS>
</NML>"#;

    fn write(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("djsugar-traktor-{name}-{}.nml", std::process::id()));
        std::fs::write(&path, NML).unwrap();
        path
    }

    #[test]
    fn reads_a_traktor_collection() {
        let path = write("read");
        let here = |p: &Path| p.starts_with("/Users") || p.starts_with("/Volumes/USB");
        let lib = read(&path, &[PLAYLISTS], here).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(lib.tracks.len(), 2);
        let t = &lib.tracks[0];
        assert!(
            matches!(&t.source, Source::File(p) if p == Path::new("/Users/dj/Music/Bicep - Glue.mp3"))
        );
        assert_eq!((t.artist.as_str(), t.title.as_str()), ("Bicep", "Glue"));
        assert_eq!((t.rating, t.key.as_str(), t.bpm), (4, "Fm", 120.0));
        assert_eq!(t.duration, 268.9);
        let i = &t.info;
        assert_eq!((i.play_count, i.track_number), (7, 3));
        assert_eq!(
            (i.year.as_str(), i.remix.as_str()),
            ("2017", "Original Mix")
        );
        assert_eq!(i.label, "Ninja Tune");
        assert_eq!(i.color, Some(0x1E_88E5));
        assert!(i.added > 0 && i.last_played.is_some());
        assert_eq!(t.grid.markers().len(), 1);
        assert_eq!(t.first_beat, 0.025);
        // The grid marker on pad 1, the cue on pad 3, the loop on pad 4,
        // then a memory cue; the fade out is an Automix point.
        let pads: Vec<(usize, &str, Option<f64>)> = i
            .hotcues
            .iter()
            .map(|h| (h.number, h.name.as_str(), h.end))
            .collect();
        assert_eq!(
            pads,
            [
                (0, "", None),
                (2, "Drop", None),
                (3, "", Some(72.025)),
                (MEMORY_CUE, "Mem", None)
            ]
        );
        assert_eq!(i.markers.len(), 1);
        assert_eq!(i.markers[0].kind, MarkerKind::Automix);
        // A disk other than the startup disk is under /Volumes.
        assert!(
            matches!(&lib.tracks[1].source, Source::File(p) if p == Path::new("/Volumes/USB/Music/away.mp3"))
        );

        // Folders within folders, Traktor's own lists and smart lists left out.
        let lists: Vec<String> = lib
            .lists
            .iter()
            .map(|l| {
                let mut p = l.folders.clone();
                p.push(l.name.clone());
                p.join("/")
            })
            .collect();
        assert_eq!(lists, ["Preparation", "Gigs/2023/Friday"]);
        assert_eq!(
            lib.lists[0].entries[0].path,
            PathBuf::from("C:/Music/other.mp3")
        );
        let friday: Vec<&Path> = lib.lists[1]
            .entries
            .iter()
            .map(|e| e.path.as_path())
            .collect();
        assert_eq!(
            friday,
            [
                Path::new("/Users/dj/Music/Bicep - Glue.mp3"),
                Path::new("/Volumes/USB/Music/away.mp3")
            ]
        );
    }

    #[test]
    fn previews_and_rejects_other_files() {
        let path = write("preview");
        assert_eq!(
            preview(&path).unwrap(),
            "3 tracks, 2 playlists and 0 history sessions."
        );
        std::fs::remove_file(&path).ok();
        assert!(document("<DJ_PLAYLISTS/>").is_err());
    }

    #[test]
    fn keys_and_days() {
        assert_eq!(key_name(0.0).as_deref(), Some("C"));
        assert_eq!(key_name(21.0).as_deref(), Some("Am"));
        assert_eq!(key_name(24.0), None);
        assert_eq!(day("1970/1/2"), Some(86_400));
        assert_eq!(day("junk"), None);
    }

    #[test]
    fn finds_the_newest_traktor_folder() {
        let dir = std::env::temp_dir().join(format!("djsugar-ni-{}", std::process::id()));
        for v in ["Traktor 3.4.0", "Traktor 3.10.1", "Traktor Kontrol S2"] {
            std::fs::create_dir_all(dir.join(v)).unwrap();
        }
        for v in ["Traktor 3.4.0", "Traktor 3.10.1"] {
            std::fs::write(dir.join(v).join("collection.nml"), "").unwrap();
        }
        assert_eq!(
            newest_collection(&dir),
            Some(dir.join("Traktor 3.10.1").join("collection.nml"))
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
