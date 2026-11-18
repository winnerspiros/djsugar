//! Reads VirtualDJ's library, `database.xml`, so a collection built in
//! VirtualDJ comes across with its tags, ratings, colours, beat grids, keys,
//! hot cues, saved loops, named sections and automix points. What VirtualDJ
//! leaves empty is filled in from the file's own tags. Reading never
//! changes the file (<https://virtualdj.com/wiki/VDJ_database.html>).
//!
//! Each `<Song FilePath=…>` holds `<Tags>` (Author, Title, Remix, Album,
//! Genre, Stars, Grouping, User1, User2…), `<Infos>` (SongLength,
//! PlayCount, UserColor…), `<Scan>` (Bpm as seconds per beat, Key, Phase)
//! and `<Poi>` points: `beatgrid` anchors, numbered `cue`s, saved `loop`s,
//! `remix` sections and `automix` points. Streaming entries
//! (`netsearch://…`) and VirtualDJ's protected offline copies
//! (`.vdjcache`) are counted but not imported: they can't be played here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Source, Track};

/// Where VirtualDJ keeps its main database: `DJSUGAR_VDJ_DATABASE`, else
/// the first of VirtualDJ's home folders that has one.
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_VDJ_DATABASE") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;
    [
        "Library/Application Support/VirtualDJ/database.xml",
        "Documents/VirtualDJ/database.xml",
        // Windows with Documents backed up to OneDrive.
        "OneDrive/Documents/VirtualDJ/database.xml",
        ".vdj/database.xml",
    ]
    .iter()
    .map(|p| home.join(p))
    .find(|p| p.is_file())
}

/// What an import found.
#[derive(Debug, Default)]
pub struct Import {
    pub tracks: Vec<Arc<Track>>,
    /// Streaming entries (TIDAL, SoundCloud, Deezer…), not imported.
    pub streaming: usize,
    /// What the database knows of each streamed track, by its address
    /// (`netsearch://td…`): artist, title and length in seconds, for lists
    /// that didn't save them.
    pub streamed: HashMap<String, (String, String, f64)>,
    /// VirtualDJ's offline copies of streamed tracks, not imported.
    pub cached: usize,
    /// Local files that are no longer where the database says.
    pub missing: usize,
}

/// Reads the database at `path`. Tracks get ids from `first_id`; `exists`
/// says whether a file is there, and `file_tags` reads a file's own tags
/// (so tests need no real files).
pub fn import(
    path: &Path,
    first_id: usize,
    exists: impl Fn(&Path) -> bool,
    file_tags: impl Fn(&Path) -> Option<Track>,
) -> Result<Import, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text, first_id, exists, file_tags)
}

/// The tags of a file on this computer, unless reading them would download
/// it from iCloud first.
pub fn read_file_tags(path: &Path) -> Option<Track> {
    (!djsugar_engine::scan::is_cloud_only(path)).then(|| djsugar_engine::scan::read_track(0, path))
}

pub fn parse(
    text: &str,
    first_id: usize,
    exists: impl Fn(&Path) -> bool,
    file_tags: impl Fn(&Path) -> Option<Track>,
) -> Result<Import, String> {
    let mut out = Import::default();
    super::parse_xml(text, |doc| {
        songs(doc, first_id, exists, file_tags, &mut out)
    })
    .map_err(|e| e.to_string())?;
    Ok(out)
}

fn songs(
    doc: &roxmltree::Document,
    first_id: usize,
    exists: impl Fn(&Path) -> bool,
    file_tags: impl Fn(&Path) -> Option<Track>,
    out: &mut Import,
) {
    for song in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("Song"))
    {
        let Some(file) = song.attribute("FilePath") else {
            continue;
        };
        if file.contains("://") {
            out.streaming += 1;
            let tags = song.children().find(|n| n.has_tag_name("Tags"));
            let infos = song.children().find(|n| n.has_tag_name("Infos"));
            let attr = |n: Option<roxmltree::Node>, name: &str| {
                n.and_then(|n| n.attribute(name))
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            };
            out.streamed.insert(
                file.to_string(),
                (
                    attr(tags, "Author"),
                    attr(tags, "Title"),
                    attr(infos, "SongLength").parse().unwrap_or(0.0),
                ),
            );
            continue;
        }
        let lower = file.to_ascii_lowercase();
        if lower.ends_with(".vdjcache") || lower.ends_with(".vdjedit") {
            out.cached += 1;
            continue;
        }
        let path = PathBuf::from(file);
        if !exists(&path) {
            out.missing += 1;
            continue;
        }
        let id = first_id + out.tracks.len();
        let mut track = song_track(id, path, song);
        if let Some(tags) = file_tags(&path_of(&track)) {
            fill_from_tags(&mut track, &tags);
        }
        out.tracks.push(Arc::new(track));
    }
}

fn path_of(t: &Track) -> PathBuf {
    match &t.source {
        Source::File(p) => p.clone(),
        Source::Synth { .. } => PathBuf::new(),
    }
}

/// Fills what VirtualDJ left empty from the file's tags. VirtualDJ's own
/// values win: they are what the DJ sees and edits there.
fn fill_from_tags(t: &mut Track, tags: &Track) {
    let fill = |mine: &mut String, theirs: &str| {
        if mine.is_empty() {
            *mine = theirs.to_string();
        }
    };
    fill(&mut t.artist, &tags.artist);
    fill(&mut t.album, &tags.album);
    fill(&mut t.genre, &tags.genre);
    if t.key.is_empty() && !tags.key.is_empty() {
        t.key = tags.key.clone();
    }
    if t.duration <= 0.0 {
        t.duration = tags.duration;
    }
    let (i, f) = (&mut t.info, &tags.info);
    fill(&mut i.label, &f.label);
    fill(&mut i.composer, &f.composer);
    fill(&mut i.remixer, &f.remixer);
    fill(&mut i.remix, &f.remix);
    fill(&mut i.isrc, &f.isrc);
    fill(&mut i.year, &f.year);
    if i.track_number == 0 {
        i.track_number = f.track_number;
    }
    if i.disc_number == 0 {
        i.disc_number = f.disc_number;
    }
}

/// A colour VirtualDJ keeps as an ARGB number, as 0xRRGGBB.
fn color(value: f64) -> u32 {
    (value as u64 & 0xff_ffff) as u32
}

fn song_track(id: usize, path: PathBuf, song: roxmltree::Node) -> Track {
    let child = |name: &str| song.children().find(|n| n.has_tag_name(name));
    let attr = |node: Option<roxmltree::Node>, name: &str| -> String {
        node.and_then(|n| n.attribute(name))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let number = |node: Option<roxmltree::Node>, name: &str| -> Option<f64> {
        node.and_then(|n| n.attribute(name))
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    let tags = child("Tags");
    let infos = child("Infos");
    let scan = child("Scan");

    let mut title = attr(tags, "Title");
    let remix = attr(tags, "Remix");
    if !remix.is_empty() && !title.to_lowercase().contains(&remix.to_lowercase()) {
        title = format!("{title} ({remix})");
    }
    if title.is_empty() {
        title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
    }

    // Scan's Bpm is seconds per beat; a Bpm typed into the tags is too.
    let seconds_per_beat = number(scan, "Bpm").or_else(|| number(tags, "Bpm"));
    let points: Vec<roxmltree::Node> = song.children().filter(|n| n.has_tag_name("Poi")).collect();
    let grid_points: Vec<&roxmltree::Node> = points
        .iter()
        .filter(|p| p.attribute("Type") == Some("beatgrid"))
        .collect();
    let grid = grid_points.first().copied();
    // The first grid point anchors the beats; its own Bpm (in beats per
    // minute) wins where the tempo was set by hand.
    let bpm = grid
        .and_then(|g| number(Some(*g), "Bpm"))
        .or_else(|| seconds_per_beat.filter(|s| *s > 0.0).map(|s| 60.0 / s))
        .filter(|b| (20.0..=400.0).contains(b))
        .unwrap_or(0.0);
    let first_beat = grid
        .and_then(|g| number(Some(*g), "Pos"))
        .or_else(|| number(scan, "Phase"))
        .unwrap_or(0.0)
        .max(0.0);
    let key = Some(attr(scan, "Key"))
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| attr(tags, "Key"));

    let hotcues = points
        .iter()
        .filter(|p| p.attribute("Type") == Some("cue"))
        .filter_map(|p| {
            let n = number(Some(*p), "Num")? as i64;
            let seconds = number(Some(*p), "Pos")?;
            (n >= 1).then(|| Hotcue {
                number: n as usize - 1,
                seconds,
                name: p.attribute("Name").unwrap_or_default().to_string(),
                end: None,
                color: number(Some(*p), "Color").map(color),
            })
        })
        .collect::<Vec<_>>();
    // VirtualDJ's two user fields, as hashtags: each word a tag, put in
    // the DJ's groups by the library.
    let imported_tags = [("User1", "User 1"), ("User2", "User 2")]
        .iter()
        .flat_map(|(key, field)| {
            djsugar_engine::hashtags(&attr(tags, key))
                .into_iter()
                .map(|t| (field.to_string(), t))
        })
        .collect();
    let track_color = number(infos, "UserColor").map(color);
    let markers: Vec<Marker> = points
        .iter()
        .filter_map(|p| {
            let (kind, name) = match p.attribute("Type")? {
                "remix" => (MarkerKind::Section, p.attribute("Name").unwrap_or_default()),
                "automix" => (MarkerKind::Automix, p.attribute("Point")?),
                _ => return None,
            };
            Some(Marker {
                kind,
                seconds: number(Some(*p), "Pos")?,
                end: None,
                name: name.to_string(),
                color: number(Some(*p), "Color").map(color),
                detected: false,
            })
        })
        .collect();
    // Saved loops (Size in beats) go on the pads after the hot cues, as
    // DJ Sugar keeps loops on the same pads; in VirtualDJ's order.
    let mut loops: Vec<(f64, f64, f64)> = points
        .iter()
        .filter(|p| p.attribute("Type") == Some("loop"))
        .filter_map(|p| {
            Some((
                number(Some(*p), "Num").unwrap_or(0.0),
                number(Some(*p), "Pos")?,
                number(Some(*p), "Size").filter(|s| *s > 0.0)?,
            ))
        })
        .collect();
    loops.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Each further grid point starts a section at its own tempo (VirtualDJ
    // writes them for tracks whose tempo drifts).
    let mut grid_markers = Vec::new();
    let mut tempo = bpm;
    for p in &grid_points {
        let Some(pos) = number(Some(**p), "Pos") else {
            continue;
        };
        tempo = number(Some(**p), "Bpm")
            .filter(|b| (20.0..=400.0).contains(b))
            .unwrap_or(tempo);
        grid_markers.push(GridMarker {
            seconds: pos,
            bpm: tempo,
        });
    }
    let mut track = Track {
        id,
        artist: attr(tags, "Author"),
        title,
        album: attr(tags, "Album"),
        genre: attr(tags, "Genre"),
        rating: number(tags, "Stars").unwrap_or(0.0).clamp(0.0, 5.0) as u8,
        bpm,
        key,
        duration: number(infos, "SongLength").unwrap_or(0.0),
        first_beat,
        source: Source::File(path),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        grid: Default::default(),
        info: LibraryInfo {
            comment: song
                .children()
                .find(|n| n.has_tag_name("Comment"))
                .and_then(|n| n.text())
                .unwrap_or_default()
                .trim()
                .to_string(),
            grouping: attr(tags, "Grouping"),
            imported_tags,
            color: track_color,
            play_count: number(infos, "PlayCount").unwrap_or(0.0) as u32,
            hotcues,
            fixed_grid: bpm > 0.0,
            year: Some(attr(tags, "Year"))
                .filter(|y| y != "0")
                .unwrap_or_default(),
            track_number: number(tags, "TrackNumber")
                .filter(|n| *n > 0.0)
                .unwrap_or(0.0) as u32,
            label: attr(tags, "Label"),
            composer: attr(tags, "Composer"),
            remixer: attr(tags, "Remixer"),
            remix,
            added: number(infos, "FirstSeen").unwrap_or(0.0) as i64,
            last_played: number(infos, "LastPlay")
                .filter(|p| *p > 0.0)
                .map(|p| p as i64),
            // A level VirtualDJ keeps as a factor; DJ Sugar keeps decibels.
            gain_db: number(infos, "Gain")
                .filter(|g| *g > 0.0)
                .map(|g| (20.0 * g.log10()) as f32),
            markers,
            ..Default::default()
        },
    };
    if grid_markers.len() > 1 && bpm > 0.0 {
        track.set_grid(BeatGrid::new(grid_markers));
    }
    for (pos, size) in loops.into_iter().map(|(_, pos, size)| (pos, size)) {
        let Some(number) = (0..).find(|n| !track.info.hotcues.iter().any(|c| c.number == *n))
        else {
            break;
        };
        let end = pos + size * track.beat_length_at(pos);
        track.info.hotcues.push(Hotcue {
            number,
            seconds: pos,
            name: String::new(),
            end: Some(end),
            color: None,
        });
    }
    track.info.hotcues.sort_by_key(|c| c.number);
    track
}

#[cfg(test)]
mod tests {
    use super::*;

    const DB: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<VirtualDJ_Database Version="2026">
 <Song FilePath="/music/a.flac" FileSize="1">
  <Tags Author="Artist A" Title="Song A" Remix="Club Mix" Genre="House" Album="LP" Stars="4" Grouping="11 YOGA" User1="#uplifting" User2="#synth #vox" Year="1999" TrackNumber="3" Label="Warp" Composer="C" Flag="1" />
  <Infos SongLength="300.5" PlayCount="7" UserColor="4294959104" FirstSeen="1648875124" LastPlay="1714769511" Gain="2.0" />
  <Comment>nice one</Comment>
  <Scan Version="801" Bpm="0.468750" Phase="0.25" Key="Am" Flag="32768" />
  <Poi Pos="0.30" Type="beatgrid" />
  <Poi Pos="37.5" Num="1" Type="cue" Name="Drop" Color="4291559424" />
  <Poi Pos="60" Num="2" Type="loop" Size="8.0" Slot="1" />
  <Poi Num="-1" Type="loop" Size="8.0" Slot="1" />
  <Poi Pos="71.9" Num="3" Type="cue" />
  <Poi Name="Break 1" Pos="100" Type="remix" />
  <Poi Pos="10" Type="automix" Point="fadeStart" />
 </Song>
 <Song FilePath="/music/b.mp3" FileSize="1">
  <Tags Author="B" Title="Song B" Bpm="0.5" />
  <Infos SongLength="200" />
  <Poi Pos="1.0" Type="beatgrid" Bpm="73.7" />
  <Poi Pos="40.0" Type="beatgrid" Bpm="80.0" Phrase="1" />
 </Song>
 <Song FilePath="netsearch://td123" FileSize="1"><Tags Title="Streamed" /></Song>
 <Song FilePath="/Users/x/Library/Application Support/VirtualDJ/Cache/c.vdjcache" FileSize="1"><Tags Title="Cached" /></Song>
 <Song FilePath="/music/gone.mp3" FileSize="1"><Tags Title="Gone" /></Song>
</VirtualDJ_Database>"##;

    #[test]
    fn reads_a_song_with_an_attribute_written_twice() {
        let db = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- a <Song FilePath="x" FilePath="y"> in a comment stays -->
<VirtualDJ_Database Version="2026">
 <Song FilePath="/music/a.flac" FileSize="1" FilePath='/music/b.flac'><Tags Title="A &gt; B" Title="C" /></Song>
 <Song FilePath="/music/c.flac" FileSize="1"><Tags Title="C" /></Song>
</VirtualDJ_Database>"#;
        let out = parse(db, 0, |_| true, |_| None).unwrap();
        let paths: Vec<_> = out.tracks.iter().map(|t| path_of(t)).collect();
        assert_eq!(
            paths,
            [
                PathBuf::from("/music/a.flac"),
                PathBuf::from("/music/c.flac")
            ]
        );
        assert_eq!(out.tracks[0].title, "A > B");
    }

    #[test]
    fn imports_local_tracks_with_their_grid_cues_and_tags() {
        // The file's own tags fill in what VirtualDJ leaves empty.
        let file_tags = |p: &Path| {
            // A file that isn't there reads as empty tags.
            let mut t = djsugar_engine::scan::read_track(0, p);
            t.album = "From the file".into();
            t.info.isrc = "GBAAA9900001".into();
            t.info.year = "2001".into();
            Some(t)
        };
        let out = parse(DB, 100, |p| !p.ends_with("gone.mp3"), file_tags).unwrap();
        assert_eq!(
            (out.tracks.len(), out.streaming, out.cached, out.missing),
            (2, 1, 1, 1)
        );
        let a = &out.tracks[0];
        assert_eq!(a.id, 100);
        assert_eq!(a.title, "Song A (Club Mix)");
        assert_eq!(
            (a.artist.as_str(), a.genre.as_str(), a.rating),
            ("Artist A", "House", 4)
        );
        assert!((a.bpm - 128.0).abs() < 1e-9);
        assert_eq!(a.first_beat, 0.30);
        assert_eq!(a.key, "Am");
        assert_eq!(a.duration, 300.5);
        assert!(a.info.fixed_grid);
        assert_eq!(a.info.grouping, "11 YOGA");
        let tags: Vec<(&str, &str)> = a
            .info
            .imported_tags
            .iter()
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        assert_eq!(
            tags,
            [
                ("User 1", "uplifting"),
                ("User 2", "synth"),
                ("User 2", "vox")
            ]
        );
        assert_eq!(a.info.color, Some(0xffe000));
        assert_eq!(a.info.play_count, 7);
        assert_eq!(a.info.comment, "nice one");
        assert_eq!(
            a.info.hotcues,
            [
                Hotcue {
                    number: 0,
                    seconds: 37.5,
                    name: "Drop".into(),
                    end: None,
                    color: Some(0xcc0000),
                },
                // The saved loop takes the first free pad: 8 beats at 128.
                Hotcue {
                    number: 1,
                    seconds: 60.0,
                    name: String::new(),
                    end: Some(63.75),
                    color: None,
                },
                Hotcue {
                    number: 2,
                    seconds: 71.9,
                    name: String::new(),
                    end: None,
                    color: None,
                },
            ]
        );
        let names: Vec<(&str, MarkerKind)> = a
            .info
            .markers
            .iter()
            .map(|m| (m.name.as_str(), m.kind))
            .collect();
        assert_eq!(
            names,
            [
                ("Break 1", MarkerKind::Section),
                ("fadeStart", MarkerKind::Automix)
            ]
        );
        let i = &a.info;
        assert_eq!((i.year.as_str(), i.track_number), ("1999", 3));
        assert_eq!((i.label.as_str(), i.composer.as_str()), ("Warp", "C"));
        assert_eq!(i.remix, "Club Mix");
        assert_eq!(
            (i.added, i.last_played),
            (1_648_875_124, Some(1_714_769_511))
        );
        assert!((i.gain_db.unwrap() - 6.0206).abs() < 1e-3);
        assert_eq!(a.album, "LP", "VirtualDJ's own value wins");
        assert_eq!(i.isrc, "GBAAA9900001", "the file fills in the rest");
        assert_eq!(out.tracks[1].album, "From the file");
        assert_eq!(out.tracks[1].info.year, "2001");
        // A grid point's own Bpm wins over the tag's, and further points
        // start sections at their own tempo.
        let b = &out.tracks[1];
        assert!(b.grid.is_dynamic());
        assert!((b.bpm_at(10.0) - 73.7).abs() < 0.2);
        assert!((b.bpm_at(50.0) - 80.0).abs() < 1e-9);
        assert!((b.bpm - 80.0).abs() < 1e-9, "the longer section's tempo");
        assert!((b.beat_at(40.0) - b.beat_at(40.0).round()).abs() < 1e-9);
        assert!(a.grid.is_empty(), "one grid point is a steady tempo");
    }

    #[test]
    fn reads_a_real_database_when_given_one() {
        // Set DJSUGAR_VDJ_TEST_DB to check against a real VirtualDJ library.
        let Some(path) = std::env::var_os("DJSUGAR_VDJ_TEST_DB") else {
            return;
        };
        let out = import(Path::new(&path), 0, |_| true, |_| None).unwrap();
        let cues: usize = out.tracks.iter().map(|t| t.info.hotcues.len()).sum();
        let gridded = out.tracks.iter().filter(|t| t.bpm > 0.0).count();
        eprintln!(
            "{} tracks ({} with a grid, {} hot cues), {} streaming, {} cached, {} missing",
            out.tracks.len(),
            gridded,
            cues,
            out.streaming,
            out.cached,
            out.missing
        );
        assert!(!out.tracks.is_empty());
    }
}
