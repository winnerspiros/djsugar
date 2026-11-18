//! The collection as a Traktor `collection.nml` (VERSION 19), for File >
//! Import Collection. It is what [`super::read`] reads back: each track's
//! tags, key (`MUSICAL_KEY`), rating, colour, beat grid (`CUE_V2` grid
//! markers), hot cues, memory cues, saved loops and fades, and the playlist
//! tree. Traktor has no tag groups, so the DJ's tags go into the comment as
//! `/* Tag / Tag */`. Traktor's cues have no colour.

use std::fmt::Write as _;

use djsugar_engine::key::Key;
use djsugar_engine::{MarkerKind, Source, Track, MEMORY_CUE};

/// A list or folder to write. Folders come before what they hold.
#[derive(Debug, Clone, Default)]
pub struct NmlList {
    pub name: String,
    /// Index of its folder in the lists given, `None` at the top.
    pub parent: Option<usize>,
    pub folder: bool,
    /// Indices into the tracks given, in order.
    pub tracks: Vec<usize>,
}

// CUE_V2 types.
const CUE: u8 = 0;
const FADE_IN: u8 = 1;
const FADE_OUT: u8 = 2;
const GRID: u8 = 4;
const LOOP: u8 = 5;

/// Traktor's track colours 1 to 7, as the reader shows them.
const COLOURS: [u32; 7] = [
    0xE5_3935, 0xFB_8C00, 0xFD_D835, 0x43_A047, 0x1E_88E5, 0x8E_24AA, 0xD8_1B60,
];

/// The `collection.nml` text for `tracks` and `lists`. Tracks that aren't
/// files are left out, and so are list entries for them.
pub fn write(tracks: &[Track], lists: &[NmlList], version: &str) -> String {
    let places: Vec<Option<Place>> = tracks.iter().map(place).collect();
    let mut x = String::with_capacity(512 + tracks.len() * 768);
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    let _ = writeln!(
        x,
        "<NML VERSION=\"19\"><HEAD COMPANY=\"DJ Sugar\" PROGRAM=\"DJ Sugar {}\"></HEAD>",
        esc(version)
    );
    x.push_str("<MUSICFOLDERS></MUSICFOLDERS>\n");
    let count = places.iter().flatten().count();
    let _ = writeln!(x, "<COLLECTION ENTRIES=\"{count}\">");
    for (t, p) in tracks.iter().zip(&places) {
        if let Some(p) = p {
            entry(&mut x, t, p);
        }
    }
    x.push_str("</COLLECTION>\n<SETS ENTRIES=\"0\"></SETS>\n<PLAYLISTS>\n");
    node(&mut x, lists, &places, None, "$ROOT");
    x.push_str("</PLAYLISTS>\n</NML>\n");
    x
}

/// Where Traktor finds a file: volume, folders as `/:a/:b/:`, and name.
struct Place {
    volume: String,
    dir: String,
    file: String,
}

impl Place {
    /// How playlists name the track.
    fn key(&self) -> String {
        format!("{}{}{}", self.volume, self.dir, self.file)
    }
}

/// On Windows the volume is the drive; on a Mac another disk is under
/// /Volumes. The startup disk's name isn't in the path, so it is Traktor's
/// usual "Macintosh HD" (the reader finds the file either way).
fn place(t: &Track) -> Option<Place> {
    let Source::File(path) = &t.source else {
        return None;
    };
    let text = path.to_str()?.replace('\\', "/");
    let (volume, rest) = match text.as_bytes() {
        [d, b':', b'/', ..] if d.is_ascii_alphabetic() => (text[..2].to_string(), &text[2..]),
        _ => match text.strip_prefix("/Volumes/") {
            Some(r) => {
                let (v, rest) = r.split_once('/')?;
                (v.to_string(), rest)
            }
            None => ("Macintosh HD".to_string(), text.as_str()),
        },
    };
    let (folders, file) = rest.rsplit_once('/').unwrap_or(("", rest));
    if file.is_empty() {
        return None;
    }
    let mut dir = String::from("/:");
    for f in folders.split('/').filter(|f| !f.is_empty()) {
        dir.push_str(f);
        dir.push_str("/:");
    }
    Some(Place {
        volume,
        dir,
        file: file.to_string(),
    })
}

fn entry(x: &mut String, t: &Track, p: &Place) {
    let i = &t.info;
    let mut comment = i.comment.trim().to_string();
    if !i.tags.is_empty() {
        let names: Vec<&str> = i.tags.iter().map(|g| g.name.as_str()).collect();
        if !comment.is_empty() {
            comment.push(' ');
        }
        let _ = write!(comment, "/* {} */", names.join(" / "));
    }
    let _ = writeln!(
        x,
        "<ENTRY TITLE=\"{}\" ARTIST=\"{}\">",
        esc(&t.title),
        esc(&t.artist)
    );
    let _ = writeln!(
        x,
        "<LOCATION DIR=\"{}\" FILE=\"{}\" VOLUME=\"{}\"></LOCATION>",
        esc(&p.dir),
        esc(&p.file),
        esc(&p.volume)
    );
    if !t.album.is_empty() || i.track_number > 0 {
        let _ = write!(x, "<ALBUM TITLE=\"{}\"", esc(&t.album));
        if i.track_number > 0 {
            let _ = write!(x, " TRACK=\"{}\"", i.track_number);
        }
        x.push_str("></ALBUM>\n");
    }
    x.push_str("<INFO");
    for (name, value) in [
        ("GENRE", &t.genre),
        ("LABEL", &i.label),
        ("REMIXER", &i.remixer),
        ("MIX", &i.remix),
        ("PRODUCER", &i.composer),
        ("COMMENT", &comment),
    ] {
        if !value.is_empty() {
            let _ = write!(x, " {name}=\"{}\"", esc(value));
        }
    }
    let _ = write!(
        x,
        " PLAYCOUNT=\"{}\" RANKING=\"{}\"",
        i.play_count,
        u32::from(t.rating.min(5)) * 51
    );
    if t.duration > 0.0 {
        let _ = write!(
            x,
            " PLAYTIME=\"{}\" PLAYTIME_FLOAT=\"{:.3}\"",
            t.duration.round() as i64,
            t.duration
        );
    }
    if let Some(c) = i.color.and_then(|c| COLOURS.iter().position(|&k| k == c)) {
        let _ = write!(x, " COLOR=\"{}\"", c + 1);
    }
    if !i.year.is_empty() {
        let _ = write!(x, " RELEASE_DATE=\"{}/1/1\"", esc(&i.year));
    }
    if let Some(d) = day(i.added) {
        let _ = write!(x, " IMPORT_DATE=\"{d}\"");
    }
    if let Some(d) = i.last_played.and_then(day) {
        let _ = write!(x, " LAST_PLAYED=\"{d}\"");
    }
    x.push_str("></INFO>\n");
    if t.bpm > 0.0 {
        let _ = writeln!(
            x,
            "<TEMPO BPM=\"{:.6}\" BPM_QUALITY=\"100.000000\"></TEMPO>",
            t.bpm
        );
    }
    if let Some(k) = Key::parse(&t.key) {
        let _ = writeln!(x, "<MUSICAL_KEY VALUE=\"{}\"></MUSICAL_KEY>", k.index());
    }
    for m in t.beat_grid().markers() {
        let _ = writeln!(
            x,
            "{}<GRID BPM=\"{:.6}\"></GRID></CUE_V2>",
            cue_open("Beat Marker", GRID, m.seconds, 0.0, -1),
            m.bpm
        );
    }
    for h in &i.hotcues {
        let len = h.end.map_or(0.0, |e| (e - h.seconds).max(0.0));
        let kind = if h.end.is_some() { LOOP } else { CUE };
        let slot = if h.number < MEMORY_CUE {
            h.number as i64
        } else {
            -1
        };
        let _ = writeln!(
            x,
            "{}</CUE_V2>",
            cue_open(&h.name, kind, h.seconds, len, slot)
        );
    }
    for m in i.markers.iter().filter(|m| m.kind == MarkerKind::Automix) {
        let kind = match m.name.as_str() {
            "realStart" => FADE_IN,
            "fadeStart" => FADE_OUT,
            _ => continue,
        };
        let _ = writeln!(x, "{}</CUE_V2>", cue_open("", kind, m.seconds, 0.0, -1));
    }
    x.push_str("</ENTRY>\n");
}

/// A `CUE_V2` start tag; times in milliseconds.
fn cue_open(name: &str, kind: u8, seconds: f64, len: f64, slot: i64) -> String {
    format!(
        "<CUE_V2 NAME=\"{}\" DISPL_ORDER=\"0\" TYPE=\"{kind}\" START=\"{:.3}\" LEN=\"{:.3}\" REPEATS=\"-1\" HOTCUE=\"{slot}\">",
        esc(name),
        seconds * 1000.0,
        len * 1000.0
    )
}

/// The folder or list `at` (the root when `None`) and what it holds.
fn node(
    x: &mut String,
    lists: &[NmlList],
    places: &[Option<Place>],
    at: Option<usize>,
    name: &str,
) {
    let Some(l) = at.filter(|&l| !lists[l].folder) else {
        let kids: Vec<usize> = (0..lists.len())
            .filter(|&k| lists[k].parent == at)
            .collect();
        let _ = writeln!(
            x,
            "<NODE TYPE=\"FOLDER\" NAME=\"{}\"><SUBNODES COUNT=\"{}\">",
            esc(name),
            kids.len()
        );
        for k in kids {
            node(x, lists, places, Some(k), &lists[k].name);
        }
        x.push_str("</SUBNODES></NODE>\n");
        return;
    };
    let keys: Vec<String> = lists[l]
        .tracks
        .iter()
        .filter_map(|&t| places.get(t)?.as_ref().map(Place::key))
        .collect();
    let _ = writeln!(
        x,
        "<NODE TYPE=\"PLAYLIST\" NAME=\"{}\"><PLAYLIST ENTRIES=\"{}\" TYPE=\"LIST\" UUID=\"djsugar{l}\">",
        esc(name),
        keys.len()
    );
    for k in keys {
        let _ = writeln!(
            x,
            "<ENTRY><PRIMARYKEY TYPE=\"TRACK\" KEY=\"{}\"></PRIMARYKEY></ENTRY>",
            esc(&k)
        );
    }
    x.push_str("</PLAYLIST></NODE>\n");
}

/// Traktor's day ("2022/4/9", UTC) for a Unix time; `None` when unknown.
fn day(unix: i64) -> Option<String> {
    if unix <= 0 {
        return None;
    }
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
    Some(format!("{y}/{m}/{d}"))
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use djsugar_engine::beats::{BeatGrid, GridMarker};
    use djsugar_engine::{Hotcue, LibraryInfo, Marker, TrackTag};

    use super::*;

    fn hotcue(number: usize, seconds: f64, end: Option<f64>, name: &str) -> Hotcue {
        Hotcue {
            number,
            seconds,
            name: name.into(),
            end,
            color: None,
        }
    }

    fn track(path: &str) -> Track {
        let mut t = Track {
            id: 0,
            title: "Glue & \"Co\"".into(),
            artist: "Bicep".into(),
            album: "Bicep".into(),
            genre: "Electronic".into(),
            rating: 4,
            bpm: 120.0,
            key: "Am".into(),
            duration: 268.9,
            first_beat: 0.0,
            grid: Default::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: LibraryInfo {
                comment: "nice".into(),
                color: Some(0x1E_88E5),
                play_count: 7,
                track_number: 3,
                year: "2017".into(),
                label: "Ninja Tune".into(),
                added: 1_649_462_400, // 2022-04-09
                tags: vec![TrackTag {
                    group: 1,
                    name: "Peak".into(),
                }],
                hotcues: vec![
                    hotcue(0, 1.5, None, "Drop"),
                    hotcue(2, 30.0, Some(32.0), ""),
                    hotcue(MEMORY_CUE, 60.25, None, "Break"),
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
                seconds: 0.025,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 100.025,
                bpm: 124.0,
            },
        ]));
        t
    }

    /// Written, then read back by our own Traktor reader: the same track,
    /// grid, cues, loops, fades and lists.
    #[test]
    fn reads_back_what_it_writes() {
        let tracks = vec![
            track("/Users/dj/Music/Bicep - Glue.mp3"),
            track("/Volumes/USB/Sets/b.flac"),
        ];
        let lists = vec![
            NmlList {
                name: "Sets".into(),
                folder: true,
                ..Default::default()
            },
            NmlList {
                name: "Friday".into(),
                parent: Some(0),
                tracks: vec![1, 0],
                ..Default::default()
            },
        ];
        let dir = std::env::temp_dir().join(format!("djs-nml-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("collection.nml");
        std::fs::write(&file, write(&tracks, &lists, "1.0")).unwrap();
        let here = |p: &Path| {
            p == Path::new("/Users/dj/Music/Bicep - Glue.mp3")
                || p == Path::new("/Volumes/USB/Sets/b.flac")
        };
        let lib = super::super::read(&file, &[super::super::PLAYLISTS], here).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(lib.tracks.len(), 2);
        let (a, src) = (&lib.tracks[0], &tracks[0]);
        let file = |t: &Track| match &t.source {
            Source::File(p) => p.clone(),
            Source::Synth { .. } => PathBuf::new(),
        };
        assert_eq!(file(a), file(src));
        assert_eq!(file(&lib.tracks[1]), file(&tracks[1]));
        assert_eq!(
            (a.title.as_str(), a.artist.as_str()),
            ("Glue & \"Co\"", "Bicep")
        );
        assert_eq!((a.rating, a.key.as_str()), (4, "Am"));
        assert_eq!(a.info.comment, "nice /* Peak */");
        assert_eq!(a.info.color, Some(0x1E_88E5));
        assert_eq!((a.info.play_count, a.info.track_number), (7, 3));
        assert_eq!(a.info.year, "2017");
        assert_eq!(a.info.label, "Ninja Tune");
        assert_eq!(a.info.added, 1_649_462_400);
        assert_eq!(a.grid.markers().len(), 2);
        for (m, n) in a.grid.markers().iter().zip(src.grid.markers()) {
            assert!((m.seconds - n.seconds).abs() < 1e-3, "{m:?} {n:?}");
            assert!((m.bpm - n.bpm).abs() < 1e-6, "{m:?} {n:?}");
        }
        let cues: Vec<(usize, f64, Option<f64>, &str)> = a
            .info
            .hotcues
            .iter()
            .map(|h| (h.number, h.seconds, h.end, h.name.as_str()))
            .collect();
        assert_eq!(
            cues,
            [
                (0, 1.5, None, "Drop"),
                (2, 30.0, Some(32.0), ""),
                (MEMORY_CUE, 60.25, None, "Break"),
            ]
        );
        assert!(a
            .info
            .markers
            .iter()
            .any(|m| m.name == "fadeStart" && m.seconds == 250.0));
        assert_eq!(lib.lists.len(), 1);
        let l = &lib.lists[0];
        assert_eq!(
            (l.folders.clone(), l.name.as_str()),
            (vec!["Sets".to_string()], "Friday")
        );
        let files: Vec<PathBuf> = l.entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(
            files,
            [
                PathBuf::from("/Volumes/USB/Sets/b.flac"),
                PathBuf::from("/Users/dj/Music/Bicep - Glue.mp3"),
            ]
        );
    }

    #[test]
    fn windows_paths_keep_their_drive() {
        let p = place(&track(r"C:\Music\House\a.mp3")).unwrap();
        assert_eq!(p.key(), "C:/:Music/:House/:a.mp3");
    }

    #[test]
    fn generated_tracks_are_left_out() {
        let mut t = track("/a.mp3");
        t.source = Source::Synth { seed: 1 };
        assert!(write(&[t], &[], "1.0").contains("<COLLECTION ENTRIES=\"0\">"));
    }
}
