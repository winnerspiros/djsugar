//! The collection, or some of its lists, out as a rekordbox XML file: the
//! format rekordbox imports (File › Import, or Preferences › Advanced ›
//! rekordbox xml) and library tools such as Lexicon, Rekord Buddy and
//! DJ.Studio read and convert to Serato, Traktor, Engine DJ and VirtualDJ.
//! It mirrors what [`super::xml`] reads back.
//!
//! Each track carries its tags, rating, colour, dates and play count, its
//! beat grid as one `TEMPO` per grid marker (so a tempo that changes
//! survives), hot cues with their numbers and colours, memory cues, and
//! saved loops (`POSITION_MARK`). Lists and folders keep their tree.
//! rekordbox XML has no field for tag groups, so the DJ's tags go into the
//! comment as `/* Tag / Tag */`, as rekordbox's own "Add My Tag to
//! comments" writes them.

use std::fmt::Write as _;
use std::path::Path;

use djsugar_engine::import::StreamedTrack;
use djsugar_engine::key::Key;
use djsugar_engine::{Source, Track};

/// A list or folder to write. Folders come before what they hold.
#[derive(Debug, Clone, Default)]
pub struct XmlList {
    pub name: String,
    /// Index of its folder in the lists given, `None` at the top.
    pub parent: Option<usize>,
    pub folder: bool,
    /// Indices into the tracks given, in order.
    pub tracks: Vec<usize>,
}

/// rekordbox has hot cues A to H; later pads go out as memory cues.
const HOT_CUES: usize = 8;

/// The rekordbox XML file for `tracks` and `lists`. Tracks that aren't
/// files (streamed placeholders) are left out, and so are list entries
/// pointing at them.
pub fn write(tracks: &[Track], lists: &[XmlList], version: &str) -> String {
    // TrackIDs from 1, only for tracks that are files.
    let mut ids = vec![0usize; tracks.len()];
    let mut count = 0;
    for (i, t) in tracks.iter().enumerate() {
        if file_of(t).is_some() {
            count += 1;
            ids[i] = count;
        }
    }
    let mut x = String::with_capacity(1024 + tracks.len() * 1024);
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<DJ_PLAYLISTS Version=\"1.0.0\">\n");
    let _ = writeln!(
        x,
        "  <PRODUCT Name=\"DJ Sugar\" Version=\"{}\" Company=\"DJ Sugar\"/>",
        esc(version)
    );
    let _ = writeln!(x, "  <COLLECTION Entries=\"{count}\">");
    for (i, t) in tracks.iter().enumerate() {
        if let Some(path) = file_of(t) {
            track(&mut x, ids[i], t, path);
        }
    }
    x.push_str("  </COLLECTION>\n");
    x.push_str("  <PLAYLISTS>\n");
    let top: Vec<usize> = (0..lists.len())
        .filter(|&l| lists[l].parent.is_none())
        .collect();
    let _ = writeln!(
        x,
        "    <NODE Type=\"0\" Name=\"ROOT\" Count=\"{}\">",
        top.len()
    );
    for l in top {
        node(&mut x, lists, l, &ids, 3);
    }
    x.push_str("    </NODE>\n");
    x.push_str("  </PLAYLISTS>\n");
    x.push_str("</DJ_PLAYLISTS>\n");
    x
}

fn file_of(t: &Track) -> Option<&Path> {
    match &t.source {
        Source::File(p) if StreamedTrack::parse(&p.to_string_lossy()).is_none() => Some(p),
        _ => None,
    }
}

fn node(x: &mut String, lists: &[XmlList], l: usize, ids: &[usize], depth: usize) {
    let pad = "  ".repeat(depth);
    let list = &lists[l];
    if list.folder {
        let kids: Vec<usize> = (l + 1..lists.len())
            .filter(|&k| lists[k].parent == Some(l))
            .collect();
        let _ = writeln!(
            x,
            "{pad}<NODE Type=\"0\" Name=\"{}\" Count=\"{}\">",
            esc(&list.name),
            kids.len()
        );
        for k in kids {
            node(x, lists, k, ids, depth + 1);
        }
        let _ = writeln!(x, "{pad}</NODE>");
        return;
    }
    let keys: Vec<usize> = list
        .tracks
        .iter()
        .filter_map(|&t| ids.get(t).copied().filter(|&id| id > 0))
        .collect();
    let _ = writeln!(
        x,
        "{pad}<NODE Name=\"{}\" Type=\"1\" KeyType=\"0\" Entries=\"{}\">",
        esc(&list.name),
        keys.len()
    );
    for id in keys {
        let _ = writeln!(x, "{pad}  <TRACK Key=\"{id}\"/>");
    }
    let _ = writeln!(x, "{pad}</NODE>");
}

fn track(x: &mut String, id: usize, t: &Track, path: &Path) {
    let i = &t.info;
    let mut comment = i.comment.trim().to_string();
    let tags: Vec<&str> = i.tags.iter().map(|t| t.name.as_str()).collect();
    if !tags.is_empty() {
        if !comment.is_empty() {
            comment.push(' ');
        }
        let _ = write!(comment, "/* {} */", tags.join(" / "));
    }
    let attrs: [(&str, String); 21] = [
        ("TrackID", id.to_string()),
        ("Name", t.title.clone()),
        ("Artist", t.artist.clone()),
        ("Composer", i.composer.clone()),
        ("Album", t.album.clone()),
        ("Grouping", i.grouping.clone()),
        ("Genre", t.genre.clone()),
        ("Kind", kind(path)),
        (
            "TotalTime",
            format!("{}", t.duration.max(0.0).round() as u64),
        ),
        ("DiscNumber", i.disc_number.to_string()),
        ("TrackNumber", i.track_number.to_string()),
        ("Year", year(&i.year)),
        ("AverageBpm", format!("{:.2}", t.bpm.max(0.0))),
        ("DateAdded", date(i.added)),
        ("Comments", comment),
        ("PlayCount", i.play_count.to_string()),
        ("Rating", (u32::from(t.rating.min(5)) * 51).to_string()),
        ("Location", location(path)),
        ("Remixer", i.remixer.clone()),
        ("Tonality", tonality(&t.key)),
        ("Label", i.label.clone()),
    ];
    x.push_str("    <TRACK");
    for (name, value) in attrs.iter().filter(|(_, v)| !v.is_empty()) {
        let _ = write!(x, " {name}=\"{}\"", esc(value));
    }
    if !i.remix.is_empty() {
        let _ = write!(x, " Mix=\"{}\"", esc(&i.remix));
    }
    if let Some(c) = i.color {
        let _ = write!(x, " Colour=\"0x{:06X}\"", track_colour(c));
    }
    x.push_str(">\n");
    tempo(x, t);
    for h in &i.hotcues {
        let num = if h.number < HOT_CUES {
            h.number as i32
        } else {
            -1
        };
        let loop_end = h.end.filter(|e| *e > h.seconds);
        let _ = write!(
            x,
            "      <POSITION_MARK Name=\"{}\" Type=\"{}\" Start=\"{:.3}\"",
            esc(&h.name),
            if loop_end.is_some() { 4 } else { 0 },
            h.seconds.max(0.0)
        );
        if let Some(e) = loop_end {
            let _ = write!(x, " End=\"{e:.3}\"");
        }
        let _ = write!(x, " Num=\"{num}\"");
        if num >= 0 {
            // rekordbox gives every hot cue a colour; green when none was set.
            let c = h.color.unwrap_or(0x28E214);
            let _ = write!(
                x,
                " Red=\"{}\" Green=\"{}\" Blue=\"{}\"",
                c >> 16 & 0xFF,
                c >> 8 & 0xFF,
                c & 0xFF
            );
        }
        x.push_str("/>\n");
    }
    x.push_str("    </TRACK>\n");
}

/// One `TEMPO` per grid marker, with its place in the bar (bars are four
/// beats from the grid's beat 0). A grid starting before the track does
/// starts at its first beat inside it, as rekordbox needs.
fn tempo(x: &mut String, t: &Track) {
    let grid = t.beat_grid();
    for (k, m) in grid.markers().iter().enumerate() {
        let next = grid.markers().get(k + 1).map(|n| n.seconds);
        let mut seconds = m.seconds;
        if seconds < 0.0 {
            seconds = grid.seconds_at(grid.beat_at(0.0).ceil());
            if next.is_some_and(|n| seconds >= n) {
                continue;
            }
        }
        let beat = grid.beat_at(seconds).round().rem_euclid(4.0) as u8 + 1;
        let _ = writeln!(
            x,
            "      <TEMPO Inizio=\"{seconds:.3}\" Bpm=\"{:.2}\" Metro=\"4/4\" Battito=\"{beat}\"/>",
            m.bpm
        );
    }
}

/// The key as rekordbox writes it: "Am", "Db", "F#m".
fn tonality(key: &str) -> String {
    const MAJOR: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
    ];
    const MINOR: [&str; 12] = [
        "Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm",
    ];
    match Key::parse(key) {
        Some(k) if k.minor => MINOR[k.tonic as usize % 12].into(),
        Some(k) => MAJOR[k.tonic as usize % 12].into(),
        None => key.trim().to_string(),
    }
}

/// The nearest of the eight track colours rekordbox has, as its XML writes
/// them: pink, red, orange, yellow, green, aqua, blue, purple.
fn track_colour(c: u32) -> u32 {
    const COLOURS: [u32; 8] = [
        0xFF007F, 0xFF0000, 0xFFA500, 0xFFFF00, 0x00FF00, 0x25FDE9, 0x0000FF, 0x660099,
    ];
    let rgb = |c: u32| {
        [
            (c >> 16 & 0xFF) as i32,
            (c >> 8 & 0xFF) as i32,
            (c & 0xFF) as i32,
        ]
    };
    let a = rgb(c);
    *COLOURS
        .iter()
        .min_by_key(|&&k| {
            let b = rgb(k);
            (0..3).map(|i| (a[i] - b[i]).pow(2)).sum::<i32>()
        })
        .unwrap()
}

/// "2017" from a year or a full date; empty when not known.
fn year(y: &str) -> String {
    let digits: String = y.trim().chars().take_while(char::is_ascii_digit).collect();
    if digits.len() == 4 && digits != "0000" {
        digits
    } else {
        String::new()
    }
}

/// `YYYY-MM-DD` (UTC) for a Unix time; empty when not known.
fn date(unix: i64) -> String {
    if unix <= 0 {
        return String::new();
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
    format!("{y:04}-{m:02}-{d:02}")
}

/// rekordbox's name for the file type.
fn kind(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_uppercase();
    match ext.as_str() {
        "" => String::new(),
        "AIF" => "AIFF File".into(),
        e => format!("{e} File"),
    }
}

/// `file://localhost/…`, percent-encoded as rekordbox writes it (a Windows
/// drive goes in as `/C:/…`).
pub fn location(path: &Path) -> String {
    let p = path.to_string_lossy().replace('\\', "/");
    let p = if p.starts_with('/') {
        p
    } else {
        format!("/{p}")
    };
    let mut out = String::from("file://localhost");
    for (i, b) in p.bytes().enumerate() {
        let drive_colon = i == 2 && b == b':';
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) || drive_colon {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Text made safe for an XML attribute.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' | '\t' => out.push(' '),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rekordbox::PLAYLISTS;
    use djsugar_engine::beats::{BeatGrid, GridMarker};
    use djsugar_engine::{Hotcue, LibraryInfo, TrackTag, MEMORY_CUE};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn track(path: &str, title: &str) -> Track {
        Track {
            id: 0,
            artist: "Bicep".into(),
            title: title.into(),
            album: String::new(),
            genre: "Electronic".into(),
            rating: 4,
            bpm: 120.0,
            key: "4A".into(),
            duration: 269.4,
            first_beat: 0.025,
            grid: Default::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: LibraryInfo::default(),
        }
    }

    fn collection() -> (Vec<Track>, Vec<XmlList>) {
        let mut a = track("/Users/dj/Music/Bicep - Glue & Co.mp3", "Glue <live>");
        a.set_grid(BeatGrid::new(vec![
            GridMarker {
                seconds: 0.025,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 64.025,
                bpm: 121.0,
            },
        ]));
        a.info = LibraryInfo {
            comment: "nice".into(),
            color: Some(0xF0107A),
            play_count: 7,
            added: 1_649_462_400, // 2022-04-09
            year: "2017-01-20".into(),
            remix: "Original Mix".into(),
            label: "Ninja Tune".into(),
            tags: vec![
                TrackTag {
                    group: 1,
                    name: "Uplifting".into(),
                },
                TrackTag {
                    group: 2,
                    name: "Synth".into(),
                },
            ],
            hotcues: vec![
                Hotcue {
                    number: 0,
                    seconds: 32.025,
                    name: "Drop".into(),
                    end: None,
                    color: Some(0xE62828),
                },
                Hotcue {
                    number: 1,
                    seconds: 64.025,
                    name: String::new(),
                    end: Some(72.025),
                    color: None,
                },
                Hotcue {
                    number: 9,
                    seconds: 100.0,
                    name: "Late pad".into(),
                    end: None,
                    color: None,
                },
                Hotcue {
                    number: MEMORY_CUE,
                    seconds: 0.025,
                    name: String::new(),
                    end: None,
                    color: None,
                },
            ],
            ..Default::default()
        };
        let b = track("C:\\Music\\Ünïcode.flac", "Other");
        let streamed = track("spotify:track:4uLU6hMCjMI75M1A2tKUQC", "Streamed");
        let lists = vec![
            XmlList {
                name: "Sets".into(),
                parent: None,
                folder: true,
                tracks: vec![],
            },
            XmlList {
                name: "Friday".into(),
                parent: Some(0),
                folder: false,
                tracks: vec![0, 2, 1],
            },
            XmlList {
                name: "Deep".into(),
                parent: Some(0),
                folder: true,
                tracks: vec![],
            },
            XmlList {
                name: "Late \"night\"".into(),
                parent: Some(2),
                folder: false,
                tracks: vec![1],
            },
            XmlList {
                name: "Loose".into(),
                parent: None,
                folder: false,
                tracks: vec![0],
            },
        ];
        (vec![a, b, streamed], lists)
    }

    #[test]
    fn writes_what_rekordbox_reads() {
        let (tracks, lists) = collection();
        let text = write(&tracks, &lists, "0.6.0");
        assert!(roxmltree::Document::parse(&text).is_ok(), "{text}");
        assert!(text.contains("<COLLECTION Entries=\"2\">"));
        assert!(text.contains(
            "Location=\"file://localhost/Users/dj/Music/Bicep%20-%20Glue%20%26%20Co.mp3\""
        ));
        assert!(text.contains("Location=\"file://localhost/C:/Music/%C3%9Cn%C3%AFcode.flac\""));
        assert!(text.contains("Tonality=\"Fm\""));
        assert!(text.contains("Rating=\"204\""));
        assert!(text.contains("DateAdded=\"2022-04-09\""));
        assert!(text.contains("Year=\"2017\""));
        assert!(text.contains("Colour=\"0xFF007F\""));
        assert!(text.contains("Comments=\"nice /* Uplifting / Synth */\""));
        assert!(text.contains("Kind=\"MP3 File\""));
        assert!(text.contains("Inizio=\"64.025\" Bpm=\"121.00\" Metro=\"4/4\" Battito=\"1\""));
        // The late pad has no rekordbox hot cue to go on: a memory cue.
        assert!(text.contains("Name=\"Late pad\" Type=\"0\" Start=\"100.000\" Num=\"-1\"/>"));
        assert!(text.contains("<NODE Type=\"0\" Name=\"ROOT\" Count=\"2\">"));
        assert!(text.contains("<NODE Type=\"0\" Name=\"Sets\" Count=\"2\">"));
        assert!(text.contains("Name=\"Friday\" Type=\"1\" KeyType=\"0\" Entries=\"2\""));
    }

    /// Written, then read back by our own reader: the same tracks, grids,
    /// cues, loops and list tree.
    #[test]
    fn round_trips_through_the_reader() {
        let (tracks, lists) = collection();
        let text = write(&tracks, &lists, "0.6.0");
        let path = std::env::temp_dir().join(format!("djsugar-rbx-out-{}.xml", std::process::id()));
        std::fs::write(&path, &text).unwrap();
        let lib = super::super::xml::read(&path, &[PLAYLISTS], |_| true).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(lib.tracks.len(), 2);
        let a = &lib.tracks[0];
        let src = &tracks[0];
        assert!(matches!(&a.source, Source::File(p) if p == file_of(src).unwrap()));
        assert_eq!(a.title, "Glue <live>");
        assert_eq!((a.rating, a.info.play_count), (4, 7));
        assert_eq!(a.key, "Fm");
        assert_eq!(a.info.remix, "Original Mix");
        assert_eq!(a.grid.markers().len(), 2);
        for (m, n) in a.grid.markers().iter().zip(src.grid.markers()) {
            assert!((m.seconds - n.seconds).abs() < 0.001);
            assert!((m.bpm - n.bpm).abs() < 0.01);
        }
        let cues = &a.info.hotcues;
        assert_eq!(cues.len(), 4);
        assert_eq!((cues[0].number, cues[0].color), (0, Some(0xE62828)));
        assert_eq!(cues[1].end, Some(72.025));
        assert!(cues[2].is_memory() && cues[3].is_memory());
        assert!(
            matches!(&lib.tracks[1].source, Source::File(p) if p == Path::new("C:/Music/Ünïcode.flac"))
        );

        let names: Vec<(Vec<String>, String, usize)> = lib
            .lists
            .iter()
            .map(|l| (l.folders.clone(), l.name.clone(), l.entries.len()))
            .collect();
        assert_eq!(
            names,
            vec![
                (vec!["Sets".into()], "Friday".into(), 2),
                (
                    vec!["Sets".into(), "Deep".into()],
                    "Late \"night\"".into(),
                    1
                ),
                (vec![], "Loose".into(), 1),
            ]
        );
    }

    #[test]
    fn a_grid_starting_before_the_track_starts_on_its_first_beat() {
        let mut t = track("/a.mp3", "A");
        t.set_grid(BeatGrid::constant(120.0, -0.2));
        let mut x = String::new();
        tempo(&mut x, &t);
        // Beat 0 at -0.2 s; beat 1 at 0.3 s is the second of the bar.
        assert!(
            x.contains("Inizio=\"0.300\" Bpm=\"120.00\" Metro=\"4/4\" Battito=\"2\""),
            "{x}"
        );
    }

    #[test]
    fn dates_and_colours() {
        assert_eq!(date(0), "");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(track_colour(0x00E000), 0x00FF00);
        assert_eq!(track_colour(0x7020A0), 0x660099);
    }
}
