//! A recorded mix, ready for Mixcloud: its tracklist (read from the `.cue`
//! sheet the recorder writes), an MP3 with an ID3 tag that carries the
//! tracklist as chapters, and the fields of Mixcloud's upload form.
//!
//! Mixcloud's upload (`POST https://api.mixcloud.com/upload/`) takes the
//! audio as `mp3`, up to 4 GB, a `name`, and the tracklist as
//! `sections-N-artist`, `sections-N-song` and `sections-N-start_time` in
//! whole seconds. The audio goes up in full before the form is checked, so
//! [`Show::problems`] checks it first.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::encode::{Encoder, Mp3};

/// The largest file Mixcloud takes.
pub const MAX_UPLOAD: u64 = 4_000_000_000;
/// The most tags a show has.
pub const MAX_TAGS: usize = 5;
/// The longest description, in characters.
pub const MAX_DESCRIPTION: usize = 1000;
/// The bitrate of the MP3 made for Mixcloud. Mixcloud re-encodes what it
/// gets, so this is the most MP3 has.
pub const BITRATE: u32 = 320;

/// One track of the mix, from when it starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// Seconds from the start of the mix.
    pub start: f64,
    pub artist: String,
    pub title: String,
}

/// The tracks a `.cue` sheet lists under one `FILE`, timed within it.
#[derive(Clone, Debug, PartialEq)]
pub struct CueFile {
    pub name: String,
    pub tracks: Vec<Section>,
}

/// The files and tracks of a cue sheet, in order. Tracks without an
/// `INDEX 01` are left out.
pub fn parse_cue(text: &str) -> Vec<CueFile> {
    let mut files: Vec<CueFile> = Vec::new();
    let mut track: Option<Section> = None;
    let mut timed = false;
    let finish = |files: &mut Vec<CueFile>, track: &mut Option<Section>, timed: bool| {
        if let (Some(t), true) = (track.take(), timed) {
            if let Some(f) = files.last_mut() {
                f.tracks.push(t);
            }
        }
    };
    for line in text.lines() {
        let line = line.trim();
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        match word {
            "FILE" => {
                finish(&mut files, &mut track, timed);
                files.push(CueFile {
                    name: quoted(rest),
                    tracks: Vec::new(),
                });
            }
            "TRACK" => {
                finish(&mut files, &mut track, timed);
                timed = false;
                track = Some(Section {
                    start: 0.0,
                    artist: String::new(),
                    title: String::new(),
                });
            }
            "TITLE" => {
                if let Some(t) = &mut track {
                    t.title = quoted(rest);
                }
            }
            "PERFORMER" => {
                if let Some(t) = &mut track {
                    t.artist = quoted(rest);
                }
            }
            "INDEX" => {
                let mut parts = rest.split_whitespace();
                if let (Some("01"), Some(at), Some(t)) = (parts.next(), parts.next(), &mut track) {
                    if let Some(s) = cue_seconds(at) {
                        t.start = s;
                        timed = true;
                    }
                }
            }
            _ => {}
        }
    }
    finish(&mut files, &mut track, timed);
    files
}

/// `"text"` or a bare word.
fn quoted(s: &str) -> String {
    let s = s.trim();
    match s.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or("").to_string(),
        None => s.split_whitespace().next().unwrap_or("").to_string(),
    }
}

/// `mm:ss:ff`, ff in 75ths of a second.
fn cue_seconds(at: &str) -> Option<f64> {
    let mut p = at.split(':').map(|n| n.parse::<u64>().ok());
    let (m, s, f) = (p.next()??, p.next()??, p.next()??);
    Some((m * 60 + s) as f64 + f as f64 / 75.0)
}

/// The tracks of every file of a recording in one list, timed from the
/// start of the first file. `lengths` are the files' lengths in seconds,
/// in the cue sheet's order. Tracks in the same second as the one before
/// are dropped, as Mixcloud wants start times to go up.
pub fn sections(files: &[CueFile], lengths: &[f64]) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut offset = 0.0;
    for (i, f) in files.iter().enumerate() {
        for t in &f.tracks {
            let start = offset + t.start;
            if out.last().is_some_and(|l| start.floor() <= l.start.floor()) {
                continue;
            }
            out.push(Section { start, ..t.clone() });
        }
        offset += lengths.get(i).copied().unwrap_or(0.0);
    }
    out
}

/// `4:05`, or `1:04:05` from an hour up.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// "Artist - Title", or whichever of the two there is.
pub fn track_name(s: &Section) -> String {
    match (s.artist.trim(), s.title.trim()) {
        ("", t) => t.to_string(),
        (a, "") => a.to_string(),
        (a, t) => format!("{a} - {t}"),
    }
}

/// One line per track, `0:00 Artist - Title`, for a show's description or
/// a text file beside the mix.
pub fn tracklist_text(sections: &[Section]) -> String {
    sections
        .iter()
        .map(|s| format!("{} {}\n", clock(s.start), track_name(s)))
        .collect()
}

/// A show as it goes up to Mixcloud.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Show {
    pub name: String,
    pub description: String,
    /// Genres, mostly; the first [`MAX_TAGS`] are sent.
    pub tags: Vec<String>,
    /// Private: only those with the link can listen.
    pub unlisted: bool,
    pub sections: Vec<Section>,
}

impl Show {
    /// What Mixcloud would turn down, checked before the audio goes up.
    pub fn problems(&self, audio_bytes: u64) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("The show needs a name.".into());
        }
        if self.description.chars().count() > MAX_DESCRIPTION {
            out.push(format!(
                "The description is longer than {MAX_DESCRIPTION} characters."
            ));
        }
        if audio_bytes > MAX_UPLOAD {
            out.push("The audio is larger than Mixcloud's 4 GB.".into());
        }
        out
    }

    /// The text fields of Mixcloud's upload form, in order; the audio
    /// (`mp3`) and cover (`picture`) go with them as files.
    pub fn upload_fields(&self) -> Vec<(String, String)> {
        let mut f = vec![("name".to_string(), self.name.trim().to_string())];
        if !self.description.trim().is_empty() {
            let d: String = self
                .description
                .trim()
                .chars()
                .take(MAX_DESCRIPTION)
                .collect();
            f.push(("description".into(), d));
        }
        for (i, tag) in self
            .tags
            .iter()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .take(MAX_TAGS)
            .enumerate()
        {
            f.push((format!("tags-{i}-tag"), tag.to_string()));
        }
        for (i, s) in self.sections.iter().enumerate() {
            f.push((format!("sections-{i}-artist"), s.artist.trim().to_string()));
            f.push((format!("sections-{i}-song"), s.title.trim().to_string()));
            f.push((
                format!("sections-{i}-start_time"),
                (s.start.max(0.0).floor() as u64).to_string(),
            ));
        }
        if self.unlisted {
            f.push(("unlisted".into(), "1".into()));
        }
        f
    }
}

// ---------------------------------------------------------------- ID3

/// A cover for the ID3 tag.
pub struct Cover<'a> {
    /// `image/jpeg` or `image/png`.
    pub mime: &'a str,
    pub data: &'a [u8],
}

/// An ID3v2.4 tag naming the mix, with a chapter (`CHAP`, listed in a
/// `CTOC`) for each track, so players show and skip through the
/// tracklist. `length` is the mix's length in seconds.
pub fn id3_tag(
    title: &str,
    artist: &str,
    sections: &[Section],
    length: f64,
    cover: Option<Cover>,
) -> Vec<u8> {
    let mut frames = Vec::new();
    frames.extend(text_frame(b"TIT2", title));
    if !artist.is_empty() {
        frames.extend(text_frame(b"TPE1", artist));
    }
    frames.extend(text_frame(b"TLEN", &((length * 1000.0) as u64).to_string()));
    let comment = tracklist_text(sections);
    if !comment.is_empty() {
        // COMM: encoding, language, empty description, text.
        let mut body = vec![3];
        body.extend_from_slice(b"eng\0");
        body.extend_from_slice(comment.as_bytes());
        frames.extend(frame(b"COMM", &body));
    }
    if let Some(c) = cover {
        let mut body = vec![3];
        body.extend_from_slice(c.mime.as_bytes());
        body.push(0);
        body.push(3); // front cover
        body.push(0); // no description
        body.extend_from_slice(c.data);
        frames.extend(frame(b"APIC", &body));
    }
    // ID3 counts chapters in one byte.
    let chapters = &sections[..sections.len().min(255)];
    if !chapters.is_empty() {
        let mut toc = b"toc\0".to_vec();
        toc.push(0b11); // top level, ordered
        toc.push(chapters.len() as u8);
        for i in 0..chapters.len() {
            toc.extend_from_slice(format!("ch{i}\0").as_bytes());
        }
        toc.extend(text_frame(b"TIT2", title));
        frames.extend(frame(b"CTOC", &toc));
        for (i, s) in chapters.iter().enumerate() {
            let end = chapters.get(i + 1).map_or(length, |n| n.start);
            let ms = |t: f64| (t.max(0.0) * 1000.0).round() as u32;
            let mut body = format!("ch{i}\0").into_bytes();
            body.extend_from_slice(&ms(s.start).to_be_bytes());
            body.extend_from_slice(&ms(end.max(s.start)).to_be_bytes());
            body.extend_from_slice(&[0xFF; 8]); // no byte offsets
            body.extend(text_frame(b"TIT2", &track_name(s)));
            if !s.artist.is_empty() {
                body.extend(text_frame(b"TPE1", &s.artist));
            }
            frames.extend(frame(b"CHAP", &body));
        }
    }
    let mut tag = b"ID3\x04\x00\x00".to_vec();
    tag.extend_from_slice(&syncsafe(frames.len() as u32));
    tag.extend(frames);
    tag
}

fn syncsafe(n: u32) -> [u8; 4] {
    [
        (n >> 21 & 0x7F) as u8,
        (n >> 14 & 0x7F) as u8,
        (n >> 7 & 0x7F) as u8,
        (n & 0x7F) as u8,
    ]
}

fn frame(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut f = id.to_vec();
    f.extend_from_slice(&syncsafe(body.len() as u32));
    f.extend_from_slice(&[0, 0]);
    f.extend_from_slice(body);
    f
}

/// A text frame in UTF-8.
fn text_frame(id: &[u8; 4], text: &str) -> Vec<u8> {
    let mut body = vec![3];
    body.extend_from_slice(text.as_bytes());
    frame(id, &body)
}

// ---------------------------------------------------------------- the MP3

/// Writes an MP3 file: the tag first, then the audio as it comes.
pub struct Mp3File {
    out: BufWriter<File>,
    encoder: Mp3,
}

impl Mp3File {
    /// `rate` is the audio's sample rate; above 48 kHz it is halved.
    pub fn create(path: &Path, rate: u32, tag: &[u8]) -> io::Result<Self> {
        let encoder = Mp3::new(BITRATE, rate).map_err(io::Error::other)?;
        let mut out = BufWriter::new(File::create(path)?);
        out.write_all(tag)?;
        Ok(Self { out, encoder })
    }

    /// Interleaved stereo, -1..1.
    pub fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        let bytes = self.encoder.encode(samples).map_err(io::Error::other)?;
        self.out.write_all(&bytes)
    }

    pub fn finish(mut self) -> io::Result<()> {
        let rest = self.encoder.finish().map_err(io::Error::other)?;
        self.out.write_all(&rest)?;
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUE: &str = r#"REM DATE 2026-11-03 21:05
REM COMMENT "Recorded with DJ Sugar"
PERFORMER "DJ Sugar"
TITLE "DJ Sugar 2026-11-03 21.05"
FILE "DJ Sugar 2026-11-03 21.05.wav" WAVE
  TRACK 01 AUDIO
    TITLE "Glue"
    PERFORMER "Bicep"
    REM FILE "/m/glue.mp3"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Opal (Four Tet Remix)"
    PERFORMER "Bicep"
    INDEX 01 04:05:38
FILE "DJ Sugar 2026-11-03 21.05 (part 2).wav" WAVE
  TRACK 03 AUDIO
    TITLE "Atlas"
    PERFORMER "Bicep"
    INDEX 01 01:00:00
"#;

    #[test]
    fn reads_the_recorders_cue_sheet() {
        let files = parse_cue(CUE);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "DJ Sugar 2026-11-03 21.05.wav");
        assert_eq!(files[0].tracks.len(), 2);
        assert_eq!(files[0].tracks[1].title, "Opal (Four Tet Remix)");
        assert!((files[0].tracks[1].start - (245.0 + 38.0 / 75.0)).abs() < 1e-9);
        // The second part's tracks count on from the first part's length.
        let s = sections(&files, &[3600.0, 100.0]);
        assert_eq!(s.len(), 3);
        assert_eq!(s[2].start, 3660.0);
        assert_eq!(
            tracklist_text(&s),
            "0:00 Bicep - Glue\n4:05 Bicep - Opal (Four Tet Remix)\n1:01:00 Bicep - Atlas\n"
        );
    }

    #[test]
    fn tracks_in_the_same_second_keep_the_first() {
        let t = |start, title: &str| Section {
            start,
            artist: "A".into(),
            title: title.into(),
        };
        let files = [CueFile {
            name: "x.wav".into(),
            tracks: vec![t(10.2, "one"), t(10.8, "two"), t(11.0, "three")],
        }];
        let s = sections(&files, &[60.0]);
        let names: Vec<&str> = s.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(names, ["one", "three"]);
    }

    #[test]
    fn upload_fields_follow_mixclouds_form() {
        let show = Show {
            name: " Friday mix ".into(),
            description: "x".repeat(1200),
            tags: ["house", "", "techno", "a", "b", "c", "d"]
                .map(String::from)
                .to_vec(),
            unlisted: true,
            sections: parse_cue(CUE)[0].tracks.clone(),
        };
        let f = show.upload_fields();
        let get = |k: &str| f.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("name"), Some("Friday mix"));
        assert_eq!(get("description").unwrap().chars().count(), 1000);
        assert_eq!(get("tags-1-tag"), Some("techno"));
        assert_eq!(get("tags-4-tag"), Some("c"));
        assert_eq!(get("tags-5-tag"), None);
        assert_eq!(get("sections-1-artist"), Some("Bicep"));
        assert_eq!(get("sections-1-song"), Some("Opal (Four Tet Remix)"));
        assert_eq!(get("sections-1-start_time"), Some("245"));
        assert_eq!(get("unlisted"), Some("1"));
        assert_eq!(show.problems(10).len(), 1);
        assert_eq!(show.problems(MAX_UPLOAD + 1).len(), 2);
    }

    #[test]
    fn id3_tag_sizes_add_up() {
        let s = sections(&parse_cue(CUE), &[3600.0, 100.0]);
        let tag = id3_tag("Mix", "DJ", &s, 3700.0, None);
        assert_eq!(&tag[..5], b"ID3\x04\x00");
        let size = tag[6..10]
            .iter()
            .fold(0usize, |n, &b| (n << 7) | b as usize);
        assert_eq!(size, tag.len() - 10);
        // Walk the frames: each size lands on the next frame's id.
        let mut at = 10;
        let mut ids = Vec::new();
        while at < tag.len() {
            let id = std::str::from_utf8(&tag[at..at + 4]).unwrap().to_string();
            let n = tag[at + 4..at + 8]
                .iter()
                .fold(0usize, |n, &b| (n << 7) | b as usize);
            ids.push(id);
            at += 10 + n;
        }
        assert_eq!(at, tag.len());
        assert_eq!(ids.iter().filter(|i| *i == "CHAP").count(), 3);
        assert!(ids.contains(&"CTOC".to_string()));
    }

    /// An MP3 with chapters that FFmpeg reads, when it is installed.
    #[test]
    fn writes_an_mp3_with_chapters() {
        let dir = std::env::temp_dir().join(format!("djsugar-mixcloud-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mix.mp3");
        let rate = 48_000;
        let s = vec![
            Section {
                start: 0.0,
                artist: "A".into(),
                title: "One".into(),
            },
            Section {
                start: 1.0,
                artist: "B".into(),
                title: "Two".into(),
            },
        ];
        let tag = id3_tag("Mix", "DJ", &s, 2.0, None);
        let mut f = Mp3File::create(&path, rate, &tag).unwrap();
        let tone: Vec<f32> = (0..rate as usize * 2)
            .flat_map(|i| {
                let v = (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5;
                [v, v]
            })
            .collect();
        for chunk in tone.chunks(4096) {
            f.write(chunk).unwrap();
        }
        f.finish().unwrap();
        let len = std::fs::metadata(&path).unwrap().len();
        // 320 kbps for 2 s is 80 kB.
        assert!((70_000..95_000).contains(&len), "{len}");
        if let Ok(out) = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-show_chapters", "-of", "csv=p=0"])
            .arg(&path)
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(text.contains("A - One"), "{text}");
            assert!(text.contains("B - Two"), "{text}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
