//! Hands the library to Serato DJ: each track's hot cues, saved loops and
//! beat grid written into its file the way Serato keeps them (see
//! [`super::tags`]), and the lists as crates in the `_Serato_` folder's
//! `Subcrates`. Serato picks both up the next time it starts.

use std::fs::File;
use std::path::{Component, Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use djsugar_engine::beats::GridMarker;
use djsugar_engine::{Hotcue, Source, Track};
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::AudioFile;
use lofty::id3::v2::{BinaryFrame, Frame, FrameId, Id3v2Tag};

use super::tags::{GRID, MARKERS};

/// Serato's pads.
const SLOTS: usize = 8;

/// `Serato Markers2`'s data for these cues and loops: pads 1 to 8 as cue
/// points, and saved loops (on a pad or not) in Serato's 8 loop slots.
pub fn markers(hotcues: &[Hotcue], color: Option<u32>, locked: bool) -> Vec<u8> {
    let mut raw = vec![1, 1];
    let mut entry = |name: &str, body: &[u8]| {
        raw.extend(name.as_bytes());
        raw.push(0);
        raw.extend((body.len() as u32).to_be_bytes());
        raw.extend(body);
    };
    let c = color.unwrap_or(0xFF_FFFF).to_be_bytes();
    entry("COLOR", &[0, c[1], c[2], c[3]]);
    let ms = |s: f64| (s.max(0.0) * 1000.0).round() as u32;
    let mut loops = 0;
    for h in hotcues {
        let name = h.name.replace('\0', "");
        let c = h.color.unwrap_or(0xCC_0000).to_be_bytes();
        match h.end {
            Some(end) if loops < SLOTS => {
                let mut body = vec![0, loops as u8];
                body.extend(ms(h.seconds).to_be_bytes());
                body.extend(ms(end).to_be_bytes());
                body.extend([0xff, 0xff, 0xff, 0xff, 0, 0x27, 0xaa, 0xe1, 0]);
                body.extend(name.as_bytes());
                body.push(0);
                entry("LOOP", &body);
                loops += 1;
            }
            None if h.number < SLOTS => {
                let mut body = vec![0, h.number as u8];
                body.extend(ms(h.seconds).to_be_bytes());
                body.extend([0, c[1], c[2], c[3], 0, 0]);
                body.extend(name.as_bytes());
                body.push(0);
                entry("CUE", &body);
            }
            _ => {}
        }
    }
    entry("BPMLOCK", &[u8::from(locked)]);
    raw.push(0);
    // Serato writes base64 in lines of 72 without padding, after `01 01`.
    let text = STANDARD.encode(&raw);
    let mut out = vec![1, 1];
    for (i, line) in text.trim_end_matches('=').as_bytes().chunks(72).enumerate() {
        if i > 0 {
            out.push(b'\n');
        }
        out.extend(line);
    }
    out.push(0);
    out
}

/// `Serato BeatGrid`'s data for a grid; `None` without one.
pub fn grid(markers: &[GridMarker]) -> Option<Vec<u8>> {
    let last = markers.last()?;
    let mut out = vec![1, 0];
    out.extend((markers.len() as u32).to_be_bytes());
    for pair in markers.windows(2) {
        out.extend((pair[0].seconds as f32).to_be_bytes());
        let beats = ((pair[1].seconds - pair[0].seconds) * pair[0].bpm / 60.0).round();
        out.extend((beats.max(1.0) as u32).to_be_bytes());
    }
    out.extend((last.seconds as f32).to_be_bytes());
    out.extend((last.bpm as f32).to_be_bytes());
    out.push(0);
    Some(out)
}

/// Writes the track's cues, loops and grid into its file for Serato.
/// Other tags in the file stay as they are.
pub fn write_tags(t: &Track) -> Result<(), String> {
    let Source::File(path) = &t.source else {
        return Err("not a file".into());
    };
    let grid_markers = t.beat_grid();
    let blobs = [
        (
            MARKERS,
            Some(markers(&t.info.hotcues, t.info.color, t.info.grid_locked)),
        ),
        (GRID, grid(grid_markers.markers())),
    ];
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let options = ParseOptions::new().read_properties(false);
    let write = WriteOptions::default();
    match ext.as_str() {
        "mp3" | "wav" | "aif" | "aiff" => {
            let put = |tag: &mut Id3v2Tag| {
                for (name, data) in &blobs {
                    tag.retain(|f| !is_geob(f, name));
                    if let Some(data) = data {
                        tag.insert(Frame::Binary(BinaryFrame::new(
                            FrameId::Valid("GEOB".into()),
                            geob(name, data),
                        )));
                    }
                }
            };
            match ext.as_str() {
                "mp3" => {
                    let mut file = lofty::mpeg::MpegFile::read_from(&mut f, options)
                        .map_err(|e| failed(path, e))?;
                    let mut tag = file.id3v2().cloned().unwrap_or_default();
                    put(&mut tag);
                    file.set_id3v2(tag);
                    file.save_to_path(path, write).map_err(|e| failed(path, e))
                }
                "wav" => {
                    let mut file = lofty::iff::wav::WavFile::read_from(&mut f, options)
                        .map_err(|e| failed(path, e))?;
                    let mut tag = file.id3v2().cloned().unwrap_or_default();
                    put(&mut tag);
                    file.set_id3v2(tag);
                    file.save_to_path(path, write).map_err(|e| failed(path, e))
                }
                _ => {
                    let mut file = lofty::iff::aiff::AiffFile::read_from(&mut f, options)
                        .map_err(|e| failed(path, e))?;
                    let mut tag = file.id3v2().cloned().unwrap_or_default();
                    put(&mut tag);
                    file.set_id3v2(tag);
                    file.save_to_path(path, write).map_err(|e| failed(path, e))
                }
            }
        }
        "flac" => {
            let mut file =
                lofty::flac::FlacFile::read_from(&mut f, options).map_err(|e| failed(path, e))?;
            let mut tag = file.vorbis_comments().cloned().unwrap_or_default();
            for (key, (name, data)) in ["SERATO_MARKERS_V2", "SERATO_BEATGRID"].iter().zip(&blobs) {
                let _ = tag.remove(key).count();
                if let Some(data) = data {
                    tag.insert((*key).to_string(), wrapped(name, data));
                }
            }
            file.set_vorbis_comments(tag);
            file.save_to_path(path, write).map_err(|e| failed(path, e))
        }
        "m4a" | "mp4" | "aac" | "alac" => {
            let mut file =
                lofty::mp4::Mp4File::read_from(&mut f, options).map_err(|e| failed(path, e))?;
            let mut ilst = file.ilst().cloned().unwrap_or_default();
            for (atom, (name, data)) in ["markersv2", "beatgrid"].iter().zip(&blobs) {
                let ident = lofty::mp4::AtomIdent::Freeform {
                    mean: "com.serato.dj".into(),
                    name: (*atom).into(),
                };
                let _ = ilst.remove(&ident).count();
                if let Some(data) = data {
                    ilst.insert(lofty::mp4::Atom::new(
                        ident,
                        lofty::mp4::AtomData::UTF8(wrapped(name, data)),
                    ));
                }
            }
            file.set_ilst(ilst);
            file.save_to_path(path, write).map_err(|e| failed(path, e))
        }
        _ => Err(format!("Serato can't keep cues in .{ext} files")),
    }
}

fn failed(path: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", path.display())
}

fn is_geob(frame: &Frame, name: &str) -> bool {
    match frame {
        Frame::Binary(b) if frame.id_str() == "GEOB" => {
            super::tags::geob(&b.data).is_some_and(|(n, _)| n == name)
        }
        _ => false,
    }
}

/// A `GEOB` frame's body: Latin-1, the MIME type, no file name, the name.
fn geob(name: &str, data: &[u8]) -> Vec<u8> {
    let mut out = vec![0];
    out.extend(b"application/octet-stream\0\0");
    out.extend(name.as_bytes());
    out.push(0);
    out.extend(data);
    out
}

/// FLAC's and MP4's form: base64 of the frame without its text encoding.
fn wrapped(name: &str, data: &[u8]) -> String {
    STANDARD.encode(&geob(name, data)[1..])
}

/// A crate to write: its folders, name and tracks' files in order.
pub struct SeratoCrate {
    pub folders: Vec<String>,
    pub name: String,
    pub files: Vec<PathBuf>,
}

/// Writes `crates` into `serato` (a `_Serato_` folder) as `.crate` files,
/// replacing crates of the same name. Paths are kept relative to the top
/// of the disk `serato` is on; files on other disks are left out, and how
/// many is returned.
pub fn write_crates(serato: &Path, crates: &[SeratoCrate]) -> Result<usize, String> {
    let dir = serato.join("Subcrates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let disk = super::disk_of(&serato.join("database V2"));
    let mut skipped = 0;
    for c in crates {
        let mut data = field(b"vrsn", &utf16("1.0/Serato ScratchLive Crate"));
        for column in ["song", "artist", "bpm", "key"] {
            let mut col = field(b"tvcn", &utf16(column));
            col.extend(field(b"tvcw", &utf16("0")));
            data.extend(field(b"ovct", &col));
        }
        for f in &c.files {
            match relative(f, &disk) {
                Some(rel) => data.extend(field(b"otrk", &field(b"ptrk", &utf16(&rel)))),
                None => skipped += 1,
            }
        }
        let mut parts: Vec<String> = c.folders.iter().map(|f| safe(f)).collect();
        parts.push(safe(&c.name));
        let file = dir.join(format!("{}.crate", parts.join("%%")));
        std::fs::write(&file, data).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    Ok(skipped)
}

/// `file` relative to the top of `disk`, with `/`s; `None` on another disk.
fn relative(file: &Path, disk: &Path) -> Option<String> {
    let rest = file.strip_prefix(disk).ok()?;
    let parts: Vec<String> = rest
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// A name that can be part of a file name and holds no `%%`.
fn safe(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let s = s.replace("%%", "%").trim().to_string();
    if s.is_empty() {
        "Untitled".into()
    } else {
        s
    }
}

pub(super) fn field(name: &[u8; 4], value: &[u8]) -> Vec<u8> {
    let mut out = name.to_vec();
    out.extend((value.len() as u32).to_be_bytes());
    out.extend(value);
    out
}

pub(super) fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use djsugar_engine::beats::BeatGrid;
    use djsugar_engine::import::ImportedListKind;
    use djsugar_engine::{LibraryInfo, MEMORY_CUE};

    use super::*;

    fn hotcue(number: usize, seconds: f64, end: Option<f64>, name: &str) -> Hotcue {
        Hotcue {
            number,
            seconds,
            name: name.into(),
            end,
            color: Some(0x00_88CC),
        }
    }

    /// A short silent 16-bit mono WAV.
    fn wav(path: &Path) {
        let samples = 4410u32;
        let mut w = b"RIFF".to_vec();
        w.extend((36 + samples * 2).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(16u32.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(44_100u32.to_le_bytes());
        w.extend(88_200u32.to_le_bytes());
        w.extend(2u16.to_le_bytes());
        w.extend(16u16.to_le_bytes());
        w.extend(b"data");
        w.extend((samples * 2).to_le_bytes());
        w.extend(vec![0; samples as usize * 2]);
        std::fs::write(path, w).unwrap();
    }

    fn track(path: &Path) -> Track {
        let mut t = Track {
            id: 0,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 120.0,
            key: String::new(),
            duration: 0.0,
            first_beat: 0.0,
            grid: Default::default(),
            source: Source::File(path.to_path_buf()),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: LibraryInfo {
                hotcues: vec![
                    hotcue(0, 1.5, None, "Drop"),
                    hotcue(3, 20.0, Some(24.0), "Build"),
                    hotcue(MEMORY_CUE, 40.0, None, "memory"),
                ],
                grid_locked: true,
                ..Default::default()
            },
        };
        t.set_grid(BeatGrid::new(vec![
            GridMarker {
                seconds: 0.05,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 32.05,
                bpm: 124.0,
            },
        ]));
        t
    }

    /// Written into a WAV, then read back by our own Serato reader; written
    /// twice, the file still holds one of each.
    #[test]
    fn cues_loops_and_grid_read_back() {
        let dir = std::env::temp_dir().join(format!("djs-serato-x-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.wav");
        wav(&file);
        let src = track(&file);
        write_tags(&src).unwrap();
        write_tags(&src).unwrap();

        let mut back = track(&file);
        back.info = LibraryInfo::default();
        back.grid = Default::default();
        super::super::tags::apply(&mut back);
        let geobs = {
            let mut f = File::open(&file).unwrap();
            let w = lofty::iff::wav::WavFile::read_from(&mut f, ParseOptions::new()).unwrap();
            w.id3v2()
                .unwrap()
                .into_iter()
                .filter(|f| f.id_str() == "GEOB")
                .count()
        };
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(geobs, 2);
        let cues: Vec<_> = back
            .info
            .hotcues
            .iter()
            .map(|h| (h.number, h.seconds, h.end, h.name.as_str()))
            .collect();
        assert_eq!(
            cues,
            [
                (0, 1.5, None, "Drop"),
                (MEMORY_CUE, 20.0, Some(24.0), "Build"),
            ]
        );
        assert!(back.info.grid_locked);
        let m = back.grid.markers();
        assert_eq!(m.len(), 2);
        assert!((m[0].bpm - 120.0).abs() < 1e-3 && (m[1].bpm - 124.0).abs() < 1e-3);
        assert!((m[1].seconds - 32.05).abs() < 1e-4);
    }

    #[test]
    fn crates_read_back() {
        let dir = std::env::temp_dir().join(format!("djs-serato-c-{}", std::process::id()));
        let serato = dir.join("_Serato_");
        let a = dir.join("Music").join("a.mp3");
        let crates = [SeratoCrate {
            folders: vec!["Gigs".into()],
            name: "Friday: late".into(),
            files: vec![a.clone(), PathBuf::from("/elsewhere/b.mp3")],
        }];
        let skipped = write_crates(&serato, &crates).unwrap();
        let file = serato.join("Subcrates").join("Gigs%%Friday- late.crate");
        let list = super::super::crate_list(&file, &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(skipped, 1);
        assert_eq!(list.folders, ["Gigs"]);
        assert_eq!(list.name, "Friday- late");
        assert!(matches!(list.kind, ImportedListKind::List));
        let files: Vec<PathBuf> = list.entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(files, [a]);
    }
}
