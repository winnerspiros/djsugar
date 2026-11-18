//! Hands the library to VirtualDJ: each track's hot cues, saved loops, beat
//! grid, named sections and automix points go into VirtualDJ's
//! `database.xml`, and the lists go into `MyLists` as `.vdjfolder` files,
//! in their folders.
//!
//! VirtualDJ allows other apps to write its database but doesn't recommend
//! it, so this is careful with it. A song VirtualDJ already has keeps
//! everything else it holds: only its `cue`, `loop`, `beatgrid`, `remix` and
//! `automix` points are replaced, and the rest of the file is left byte for
//! byte. Songs it doesn't have yet are added at the end. The old file is
//! kept as `database.xml.djsugar-backup`, and the new one replaces it only
//! once it is fully written. VirtualDJ should be closed while this runs, as
//! it writes the file back when it quits.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use djsugar_engine::{MarkerKind, Source, Track, MEMORY_CUE};

/// The `Poi` types written here, and so replaced in songs VirtualDJ has.
const OURS: [&str; 5] = ["cue", "loop", "beatgrid", "remix", "automix"];

/// What writing the database did.
#[derive(Debug, Default, PartialEq)]
pub struct Written {
    /// Songs VirtualDJ had, their points replaced.
    pub updated: usize,
    /// Songs added.
    pub added: usize,
}

/// Writes the tracks' points into the `database.xml` at `path` (made if
/// there is none).
pub fn write_database(path: &Path, tracks: &[Track]) -> Result<Written, String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let old = match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(fail(&e)),
    };
    let (text, written) = merge(old.as_deref(), tracks)?;
    if let Some(old) = &old {
        let backup = path.with_file_name("database.xml.djsugar-backup");
        std::fs::write(&backup, old).map_err(|e| fail(&e))?;
    } else if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
    }
    let part = path.with_file_name("database.xml.djsugar-part");
    std::fs::write(&part, text).map_err(|e| fail(&e))?;
    std::fs::rename(&part, path).map_err(|e| fail(&e))?;
    Ok(written)
}

/// The database `old` (or a new one) with the tracks' points in it.
pub fn merge(old: Option<&str>, tracks: &[Track]) -> Result<(String, Written), String> {
    let old = old.unwrap_or(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<VirtualDJ_Database Version=\"8.1\">\r\n</VirtualDJ_Database>\r\n",
    );
    let base = match roxmltree::Document::parse(old) {
        Err(roxmltree::Error::DuplicatedAttribute(..)) => super::without_repeated_attributes(old),
        Err(e) => return Err(format!("not a VirtualDJ database: {e}")),
        Ok(_) => old.to_string(),
    };
    let doc = roxmltree::Document::parse(&base).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if !root.has_tag_name("VirtualDJ_Database") {
        return Err("not a VirtualDJ database".into());
    }
    let files: Vec<(&Track, String)> = tracks
        .iter()
        .filter_map(|t| match &t.source {
            Source::File(p) => Some((t, p.to_string_lossy().into_owned())),
            Source::Synth { .. } => None,
        })
        .collect();
    let mut done = vec![false; files.len()];
    // Edits as (byte range in `base`, replacement), in order.
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    let mut written = Written::default();
    for song in root.children().filter(|n| n.has_tag_name("Song")) {
        let Some(file) = song.attribute("FilePath") else {
            continue;
        };
        let Some(i) = files.iter().position(|(_, f)| same_file(f, file)) else {
            continue;
        };
        if std::mem::replace(&mut done[i], true) {
            continue;
        }
        let range = song.range();
        let mut kept = String::new();
        let mut at = range.start;
        let indent = indent_of(&base, range.start);
        let mut inner = None;
        for child in song.children() {
            let r = child.range();
            if inner.is_none() && child.is_element() {
                inner = Some(r.start);
            }
            let ours = child.has_tag_name("Poi")
                && child.attribute("Type").is_some_and(|t| OURS.contains(&t));
            if ours {
                kept.push_str(&base[at..line_start(&base, r.start, at)]);
                at = line_end(&base, r.end);
            }
        }
        let points = points(files[i].0, &format!("{indent} "));
        let replacement = if let Some(close) = base[..range.end].rfind("</Song>") {
            kept.push_str(&base[at..line_start(&base, close, at)]);
            format!("{kept}{points}{indent}</Song>")
        } else {
            // `<Song …/>`: opened up to hold the points.
            let open = base[range.start..range.end]
                .trim_end_matches('>')
                .trim_end_matches('/')
                .trim_end();
            format!("{open}>\r\n{points}{indent}</Song>")
        };
        edits.push((range.start..range.end, replacement));
        written.updated += 1;
    }
    let mut added = String::new();
    for ((t, file), _) in files.iter().zip(&done).filter(|(_, d)| !**d) {
        added.push_str(&song(t, file));
        written.added += 1;
    }
    if !added.is_empty() {
        let end = root.range().end;
        let close = base[..end]
            .rfind("</VirtualDJ_Database>")
            .ok_or("the database has no end")?;
        let at = line_start(&base, close, 0);
        edits.push((at..at, added));
    }
    let mut out = String::with_capacity(base.len() + 4096);
    let mut at = 0;
    for (r, text) in edits {
        out.push_str(&base[at..r.start]);
        out.push_str(&text);
        at = r.end;
    }
    out.push_str(&base[at..]);
    Ok((out, written))
}

/// Whether two of VirtualDJ's paths are the same file: on Windows and
/// macOS, letters' case doesn't matter.
fn same_file(a: &str, b: &str) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        a.len() == b.len() && a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// The start of the line `at` is on, when only spaces come before it on
/// that line (and not before `floor`); else `at`.
fn line_start(text: &str, at: usize, floor: usize) -> usize {
    let start = text[floor..at].rfind('\n').map_or(floor, |i| floor + i + 1);
    if text[start..at].trim().is_empty() {
        start
    } else {
        at
    }
}

/// Past the line break after `at`, when only spaces come between.
fn line_end(text: &str, at: usize) -> usize {
    let rest = &text[at..];
    let gap = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let after = &rest[gap..];
    if after.starts_with("\r\n") {
        at + gap + 2
    } else if after.starts_with('\n') {
        at + gap + 1
    } else {
        at
    }
}

fn indent_of(text: &str, at: usize) -> String {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let lead = &text[start..at];
    if lead.trim().is_empty() {
        lead.to_string()
    } else {
        " ".into()
    }
}

/// A new `<Song>` for VirtualDJ, with its tags and points.
fn song(t: &Track, file: &str) -> String {
    let size = std::fs::metadata(file).map_or(0, |m| m.len());
    let mut s = format!(
        " <Song FilePath=\"{}\" FileSize=\"{size}\">\r\n",
        escape(file)
    );
    s.push_str("  <Tags");
    for (name, value) in [
        ("Author", t.artist.as_str()),
        ("Title", t.title.as_str()),
        ("Album", t.album.as_str()),
        ("Genre", t.genre.as_str()),
        ("Year", t.info.year.as_str()),
        ("Label", t.info.label.as_str()),
        ("Composer", t.info.composer.as_str()),
        ("Grouping", t.info.grouping.as_str()),
    ] {
        if !value.is_empty() {
            let _ = write!(s, " {name}=\"{}\"", escape(value));
        }
    }
    if t.rating > 0 {
        let _ = write!(s, " Stars=\"{}\"", t.rating.min(5));
    }
    s.push_str(" Flag=\"1\" />\r\n");
    s.push_str("  <Infos");
    if t.duration > 0.0 {
        let _ = write!(s, " SongLength=\"{}\"", num(t.duration));
    }
    if let Some(c) = t.info.color {
        let _ = write!(s, " UserColor=\"{}\"", 0xFF00_0000u32 | c);
    }
    if t.info.play_count > 0 {
        let _ = write!(s, " PlayCount=\"{}\"", t.info.play_count);
    }
    s.push_str(" />\r\n");
    if t.bpm > 0.0 || !t.key.is_empty() {
        s.push_str("  <Scan Version=\"801\" Flag=\"0\"");
        if t.bpm > 0.0 {
            let _ = write!(s, " Bpm=\"{}\"", num(60.0 / t.bpm));
        }
        if !t.key.is_empty() {
            let _ = write!(s, " Key=\"{}\"", escape(&t.key));
        }
        s.push_str(" />\r\n");
    }
    s.push_str(&points(t, "  "));
    if !t.info.comment.is_empty() {
        let _ = write!(s, "  <Comment>{}</Comment>\r\n", escape(&t.info.comment));
    }
    s.push_str(" </Song>\r\n");
    s
}

/// The track's `<Poi>`s: the grid, cues on pads 1 to 8 and on, saved
/// loops (their length in beats), sections and automix points.
fn points(t: &Track, indent: &str) -> String {
    let mut s = String::new();
    let grid = t.beat_grid();
    let markers = grid.markers();
    if markers.is_empty() && t.bpm > 0.0 {
        let _ = write!(
            s,
            "{indent}<Poi Pos=\"{}\" Type=\"beatgrid\" Bpm=\"{}\" />\r\n",
            num(t.first_beat),
            num(t.bpm)
        );
    }
    for m in markers {
        let _ = write!(
            s,
            "{indent}<Poi Pos=\"{}\" Type=\"beatgrid\" Bpm=\"{}\" />\r\n",
            num(m.seconds),
            num(m.bpm)
        );
    }
    let mut loops = 0;
    for h in &t.info.hotcues {
        let name = if h.name.is_empty() {
            String::new()
        } else {
            format!(" Name=\"{}\"", escape(&h.name))
        };
        match h.end {
            Some(end) if end > h.seconds => {
                loops += 1;
                let beat = t.beat_length_at(h.seconds);
                let beats = if beat > 0.0 {
                    (end - h.seconds) / beat
                } else {
                    end - h.seconds
                };
                let _ = write!(
                    s,
                    "{indent}<Poi Pos=\"{}\" Num=\"{loops}\" Type=\"loop\" Size=\"{}\"{name} />\r\n",
                    num(h.seconds),
                    num(beats)
                );
            }
            None if h.number < MEMORY_CUE => {
                let color = h
                    .color
                    .map(|c| format!(" Color=\"{}\"", 0xFF00_0000u32 | c))
                    .unwrap_or_default();
                let _ = write!(
                    s,
                    "{indent}<Poi Pos=\"{}\" Num=\"{}\" Type=\"cue\"{name}{color} />\r\n",
                    num(h.seconds),
                    h.number + 1
                );
            }
            _ => {}
        }
    }
    for m in t.info.markers.iter().filter(|m| !m.detected) {
        let _ = match m.kind {
            MarkerKind::Section => write!(
                s,
                "{indent}<Poi Name=\"{}\" Pos=\"{}\" Type=\"remix\" />\r\n",
                escape(&m.name),
                num(m.seconds)
            ),
            MarkerKind::Automix => write!(
                s,
                "{indent}<Poi Pos=\"{}\" Type=\"automix\" Point=\"{}\" />\r\n",
                num(m.seconds),
                escape(&m.name)
            ),
            MarkerKind::Phrase => Ok(()),
        };
    }
    s
}

/// A number as VirtualDJ writes them: up to six decimals, no trailing
/// zeros.
fn num(v: f64) -> String {
    let s = format!("{:.6}", v.max(0.0));
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' | '\n' | '\t' => {
                let _ = write!(out, "&#{};", c as u32);
            }
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// A list to write: its folders, name and tracks in order.
pub struct VdjList<'a> {
    pub folders: Vec<String>,
    pub name: String,
    pub tracks: Vec<&'a Track>,
}

/// Writes each list as `MyLists/<folders>/<name>.vdjfolder` under
/// VirtualDJ's folder `home`, replacing lists of the same name. Returns
/// the files written.
pub fn write_lists(home: &Path, lists: &[VdjList]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for l in lists {
        let mut dir = home.join("MyLists");
        for f in &l.folders {
            dir.push(safe(f));
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut text = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<VirtualFolder noDuplicates=\"no\">\r\n",
        );
        let songs = l.tracks.iter().filter_map(|t| match &t.source {
            Source::File(p) => Some((t, p)),
            Source::Synth { .. } => None,
        });
        for (idx, (t, p)) in songs.enumerate() {
            let size = std::fs::metadata(p).map_or(0, |m| m.len());
            let _ = write!(
                text,
                " <song path=\"{}\" size=\"{size}\" songlength=\"{}\" bpm=\"{}\" key=\"{}\" artist=\"{}\" title=\"{}\" idx=\"{idx}\" />\r\n",
                escape(&p.to_string_lossy()),
                num(t.duration),
                num(t.bpm),
                escape(&t.key),
                escape(&t.artist),
                escape(&t.title),
            );
        }
        text.push_str("</VirtualFolder>\r\n");
        let file = dir.join(format!("{}.vdjfolder", safe(&l.name)));
        std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
        out.push(file);
    }
    Ok(out)
}

/// A name that can be a file's.
fn safe(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() {
        "Untitled".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use djsugar_engine::beats::{BeatGrid, GridMarker};
    use djsugar_engine::{Hotcue, LibraryInfo, Marker};

    use super::super::database;
    use super::*;

    fn track(path: &str) -> Track {
        let mut t = Track {
            id: 0,
            artist: "Bicep".into(),
            title: "Glue & Co".into(),
            album: String::new(),
            genre: "Electronic".into(),
            rating: 4,
            bpm: 120.0,
            key: "Am".into(),
            duration: 268.5,
            first_beat: 0.05,
            grid: Default::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: LibraryInfo {
                hotcues: vec![
                    Hotcue {
                        number: 0,
                        seconds: 1.55,
                        name: "Drop".into(),
                        end: None,
                        color: Some(0xCC_0000),
                    },
                    Hotcue {
                        number: 2,
                        seconds: 30.05,
                        name: String::new(),
                        end: Some(34.05),
                        color: None,
                    },
                ],
                markers: vec![Marker {
                    kind: MarkerKind::Automix,
                    seconds: 250.0,
                    end: None,
                    name: "fadeStart".into(),
                    color: None,
                    detected: false,
                }],
                ..Default::default()
            },
        };
        t.set_grid(BeatGrid::new(vec![
            GridMarker {
                seconds: 0.05,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 200.05,
                bpm: 124.0,
            },
        ]));
        t
    }

    fn read_back(text: &str) -> Vec<Arc<Track>> {
        database::parse(text, 0, |_: &Path| true, |_: &Path| None)
            .unwrap()
            .tracks
    }

    fn check(t: &Track) {
        assert_eq!(t.bpm, 120.0);
        assert_eq!(t.first_beat, 0.05);
        assert_eq!(t.grid.markers().len(), 2);
        assert_eq!(t.grid.markers()[1].bpm, 124.0);
        let cues: Vec<_> = t
            .info
            .hotcues
            .iter()
            .map(|h| (h.number, h.seconds, h.end, h.name.as_str(), h.color))
            .collect();
        assert_eq!(
            cues,
            [
                (0, 1.55, None, "Drop", Some(0xCC_0000)),
                (1, 30.05, Some(34.05), "", None),
            ]
        );
        assert_eq!(t.info.markers.len(), 1);
        assert_eq!(t.info.markers[0].name, "fadeStart");
    }

    #[test]
    fn new_database_reads_back() {
        let (text, w) = merge(None, &[track("/m/glue.mp3")]).unwrap();
        assert_eq!(
            w,
            Written {
                updated: 0,
                added: 1
            }
        );
        let back = read_back(&text);
        assert_eq!(back.len(), 1);
        let t = &back[0];
        assert_eq!(
            (t.artist.as_str(), t.title.as_str()),
            ("Bicep", "Glue & Co")
        );
        assert_eq!((t.rating, t.key.as_str()), (4, "Am"));
        check(t);
    }

    /// A song VirtualDJ has keeps all it had but its points; other songs
    /// and the rest of the file stay byte for byte.
    #[test]
    fn existing_songs_keep_the_rest() {
        let old = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<VirtualDJ_Database Version=\"2026\">\r\n \
<Song FilePath=\"/m/other.mp3\" FileSize=\"9\">\r\n  \
<Tags Author=\"X\" Title=\"Other\" />\r\n  \
<Poi Pos=\"5\" Num=\"1\" Type=\"cue\" />\r\n \
</Song>\r\n \
<Song FilePath=\"/m/glue.mp3\" FileSize=\"7\">\r\n  \
<Tags Author=\"Bicep\" Title=\"Glue\" User1=\"#warmup\" />\r\n  \
<Infos SongLength=\"268.5\" />\r\n  \
<Scan Version=\"801\" Bpm=\"0.5\" Key=\"Am\" />\r\n  \
<Poi Pos=\"9\" Num=\"4\" Type=\"cue\" />\r\n  \
<Poi Pos=\"0.3\" Type=\"beatgrid\" />\r\n  \
<Poi Pos=\"12\" Type=\"action\" Action=\"goto_cue 1\" />\r\n  \
<Comment>keep me</Comment>\r\n \
</Song>\r\n\
</VirtualDJ_Database>\r\n";
        let (text, w) = merge(Some(old), &[track("/m/glue.mp3")]).unwrap();
        assert_eq!(
            w,
            Written {
                updated: 1,
                added: 0
            }
        );
        let other_end = old.find("</Song>").unwrap() + 7;
        assert_eq!(text[..other_end], old[..other_end]);
        assert!(text.contains("Action=\"goto_cue 1\""));
        assert!(text.contains("User1=\"#warmup\""));
        assert!(!text.contains("Num=\"4\""));
        assert!(text.ends_with("</VirtualDJ_Database>\r\n"));
        let back = read_back(&text);
        assert_eq!(back.len(), 2);
        let glue = back
            .iter()
            .find(|t| matches!(&t.source, Source::File(p) if p == Path::new("/m/glue.mp3")))
            .unwrap();
        assert_eq!(glue.title, "Glue");
        assert_eq!(glue.info.comment, "keep me");
        check(glue);

        // Again: the same file.
        let (again, _) = merge(Some(&text), &[track("/m/glue.mp3")]).unwrap();
        assert_eq!(again, text);
    }

    #[test]
    fn song_without_points_gets_them() {
        let old = "<VirtualDJ_Database Version=\"8\">\n <Song FilePath=\"/m/glue.mp3\" FileSize=\"7\"><Infos SongLength=\"268.5\" /></Song>\n</VirtualDJ_Database>\n";
        let (text, w) = merge(Some(old), &[track("/m/glue.mp3")]).unwrap();
        assert_eq!(w.updated, 1);
        check(&read_back(&text)[0]);
    }

    #[test]
    fn self_closed_song_opens_up() {
        let old = "<VirtualDJ_Database Version=\"8\">\n <Song FilePath=\"/m/glue.mp3\" FileSize=\"7\" />\n</VirtualDJ_Database>\n";
        let (text, w) = merge(Some(old), &[track("/m/glue.mp3")]).unwrap();
        assert_eq!(w.updated, 1);
        let back = read_back(&text);
        assert_eq!(back[0].info.hotcues[0].name, "Drop");
    }

    #[test]
    fn lists_read_back() {
        let home = std::env::temp_dir().join(format!("djs-vdj-x-{}", std::process::id()));
        let a = track("/m/a.mp3");
        let b = track("/m/b \"live\".mp3");
        let files = write_lists(
            &home,
            &[VdjList {
                folders: vec!["Gigs".into()],
                name: "Friday: late".into(),
                tracks: vec![&b, &a],
            }],
        )
        .unwrap();
        let mut skipped = super::super::lists::Skipped::default();
        let lists =
            super::super::lists::read(&home, &[super::super::lists::PLAYLISTS], &mut skipped);
        std::fs::remove_dir_all(&home).ok();

        assert_eq!(
            files,
            [home
                .join("MyLists")
                .join("Gigs")
                .join("Friday- late.vdjfolder")]
        );
        assert_eq!(lists.len(), 1);
        let l = &lists[0];
        assert_eq!(l.name, "Friday- late");
        assert_eq!(l.folders, ["Playlists", "Gigs"]);
        let paths: Vec<PathBuf> = l.entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(
            paths,
            [
                PathBuf::from("/m/b \"live\".mp3"),
                PathBuf::from("/m/a.mp3")
            ]
        );
    }
}
