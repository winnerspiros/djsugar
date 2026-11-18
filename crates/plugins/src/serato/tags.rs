//! What Serato keeps in the audio file itself: hot cues, saved loops and
//! the grid's lock in `Serato Markers2`, and the beat grid in `Serato
//! BeatGrid`. MP3, AIFF and WAV keep them in ID3 `GEOB` frames; FLAC in
//! Vorbis comments (`SERATO_MARKERS_V2`, `SERATO_BEATGRID`) and MP4 in
//! `com.serato.dj` atoms (`markersv2`, `beatgrid`), as base64 of the same
//! frame: a MIME type, the frame's name, then the data.
//!
//! Markers2 is `01 01` and base64 of `01 01` and entries: a name ending in
//! NUL, a big-endian `u32` length, and the entry:
//! - `CUE`: 0, slot, start (ms, `u32`), 0, colour (RGB), 0 0, name, NUL.
//! - `LOOP`: 0, slot, start, end (ms), `ff ff ff ff`, 0, colour, locked,
//!   name, NUL.
//! - `BPMLOCK`: 1 when the grid is locked.
//!
//! BeatGrid is `01 00`, a `u32` count of markers, each but the last a start
//! (seconds, `f32`) and the beats to the next (`u32`), the last a start and
//! its tempo (`f32`); all big-endian.

use std::fs::File;
use std::path::Path;

use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine as _;
use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::{Hotcue, Source, Track, MEMORY_CUE};
use lofty::config::ParseOptions;
use lofty::file::AudioFile;

pub(super) const MARKERS: &str = "Serato Markers2";
pub(super) const GRID: &str = "Serato BeatGrid";

/// Adds what the track's file holds for Serato. A file without it, or one
/// that can't be read, is left as it is.
pub fn apply(t: &mut Track) {
    let Source::File(path) = &t.source else {
        return;
    };
    let Some(blobs) = read(path) else {
        return;
    };
    if let Some(m) = blobs.markers.as_deref().and_then(markers) {
        t.info.hotcues = m.hotcues;
        t.info.grid_locked = m.locked;
    }
    if let Some(grid) = blobs.grid.as_deref().and_then(grid) {
        t.first_beat = grid[0].seconds;
        t.set_grid(BeatGrid::new(grid));
        t.info.fixed_grid = true;
    }
}

#[derive(Default)]
struct Blobs {
    markers: Option<Vec<u8>>,
    grid: Option<Vec<u8>>,
}

fn read(path: &Path) -> Option<Blobs> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let mut f = File::open(path).ok()?;
    let options = ParseOptions::new().read_properties(false);
    let mut blobs = Blobs::default();
    match ext.as_str() {
        "mp3" | "aif" | "aiff" | "wav" => {
            let tag = match ext.as_str() {
                "mp3" => lofty::mpeg::MpegFile::read_from(&mut f, options)
                    .ok()?
                    .id3v2()
                    .cloned(),
                "wav" => lofty::iff::wav::WavFile::read_from(&mut f, options)
                    .ok()?
                    .id3v2()
                    .cloned(),
                _ => lofty::iff::aiff::AiffFile::read_from(&mut f, options)
                    .ok()?
                    .id3v2()
                    .cloned(),
            }?;
            for frame in &tag {
                if let lofty::id3::v2::Frame::Binary(b) = frame {
                    if frame.id_str() == "GEOB" {
                        if let Some((name, data)) = geob(&b.data) {
                            blobs.put(&name, data.to_vec());
                        }
                    }
                }
            }
        }
        "flac" => {
            let file = lofty::flac::FlacFile::read_from(&mut f, options).ok()?;
            let comments = file.vorbis_comments()?;
            for key in ["SERATO_MARKERS_V2", "SERATO_BEATGRID"] {
                if let Some(v) = comments.get(key) {
                    blobs.put_wrapped(v);
                }
            }
        }
        "m4a" | "mp4" | "aac" | "alac" => {
            let file = lofty::mp4::Mp4File::read_from(&mut f, options).ok()?;
            let ilst = file.ilst()?;
            for name in ["markersv2", "beatgrid"] {
                let ident = lofty::mp4::AtomIdent::Freeform {
                    mean: "com.serato.dj".into(),
                    name: name.into(),
                };
                for d in ilst.get(&ident).into_iter().flat_map(|a| a.data()) {
                    if let lofty::mp4::AtomData::UTF8(v) = d {
                        blobs.put_wrapped(v);
                    }
                }
            }
        }
        _ => return None,
    }
    Some(blobs)
}

impl Blobs {
    fn put(&mut self, name: &str, data: Vec<u8>) {
        match name {
            MARKERS => self.markers = Some(data),
            GRID => self.grid = Some(data),
            _ => {}
        }
    }

    /// FLAC's and MP4's base64 of a MIME type, name and data.
    fn put_wrapped(&mut self, text: &str) {
        let Some(raw) = base64(text) else {
            return;
        };
        let mut parts = raw.splitn(3, |&b| b == 0);
        let (_mime, _empty) = (parts.next(), parts.next());
        // What's left: the name, NUL, the data.
        let Some(rest) = parts.next() else {
            return;
        };
        let rest = rest.strip_prefix(&[0][..]).unwrap_or(rest);
        if let Some(end) = rest.iter().position(|&b| b == 0) {
            let name = String::from_utf8_lossy(&rest[..end]).into_owned();
            self.put(&name, rest[end + 1..].to_vec());
        }
    }
}

/// Base64 as Serato writes it: lines of 72, the padding often left off.
fn base64(text: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = text
        .bytes()
        .filter(|b| b.is_ascii_alphanumeric() || *b == b'+' || *b == b'/')
        .collect();
    // A single character over a multiple of four carries no whole byte.
    let usable = clean.len() - usize::from(clean.len() % 4 == 1);
    STANDARD_NO_PAD.decode(&clean[..usable]).ok()
}

/// A `GEOB` frame's name and data: after its text encoding come the MIME
/// type (Latin-1), a file name and the name, each ending in NUL (two for
/// UTF-16).
pub(super) fn geob(body: &[u8]) -> Option<(String, &[u8])> {
    let (&encoding, rest) = body.split_first()?;
    let wide = matches!(encoding, 1 | 2);
    let mime_end = rest.iter().position(|&b| b == 0)?;
    let mut rest = &rest[mime_end + 1..];
    let take = |rest: &mut &[u8]| -> Option<String> {
        let end = if wide {
            rest.as_chunks::<2>().0.iter().position(|c| *c == [0, 0])? * 2
        } else {
            rest.iter().position(|&b| b == 0)?
        };
        let text = if wide {
            let units: Vec<u16> = rest[..end]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .collect();
            String::from_utf16_lossy(&units)
                .trim_start_matches('\u{feff}')
                .to_string()
        } else {
            String::from_utf8_lossy(&rest[..end]).into_owned()
        };
        *rest = &rest[end + if wide { 2 } else { 1 }..];
        Some(text)
    };
    let _file_name = take(&mut rest)?;
    let name = take(&mut rest)?;
    Some((name, rest))
}

struct Markers {
    hotcues: Vec<Hotcue>,
    locked: bool,
}

fn markers(data: &[u8]) -> Option<Markers> {
    let encoded = data.strip_prefix(&[1, 1][..])?;
    let end = encoded
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(encoded.len());
    let raw = base64(std::str::from_utf8(&encoded[..end]).ok()?)?;
    let mut rest = raw.strip_prefix(&[1, 1][..])?;
    let mut out = Markers {
        hotcues: Vec::new(),
        locked: false,
    };
    loop {
        let name_end = rest.iter().position(|&b| b == 0)?;
        if name_end == 0 {
            break;
        }
        let name = &rest[..name_end];
        let len_at = name_end + 1;
        let len = u32::from_be_bytes(rest.get(len_at..len_at + 4)?.try_into().ok()?) as usize;
        let entry = rest.get(len_at + 4..len_at + 4 + len)?;
        rest = &rest[len_at + 4 + len..];
        match name {
            b"CUE" if entry.len() >= 13 => {
                let ms = u32::from_be_bytes(entry[2..6].try_into().ok()?);
                out.hotcues.push(Hotcue {
                    number: usize::from(entry[1]),
                    seconds: f64::from(ms) / 1000.0,
                    name: label(&entry[12..]),
                    end: None,
                    color: Some(rgb(&entry[7..10])),
                });
            }
            b"LOOP" if entry.len() >= 19 => {
                let start = u32::from_be_bytes(entry[2..6].try_into().ok()?);
                let end = u32::from_be_bytes(entry[6..10].try_into().ok()?);
                // Saved loops have their own slots, so they go with the
                // memory cues.
                out.hotcues.push(Hotcue {
                    number: MEMORY_CUE + usize::from(entry[1]),
                    seconds: f64::from(start) / 1000.0,
                    name: label(&entry[19..]),
                    end: Some(f64::from(end) / 1000.0),
                    color: None,
                });
            }
            b"BPMLOCK" => out.locked = entry.first() == Some(&1),
            _ => {}
        }
    }
    out.hotcues.sort_by_key(|h| h.number);
    out.hotcues.dedup_by_key(|h| h.number);
    Some(out)
}

fn rgb(b: &[u8]) -> u32 {
    u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2])
}

fn label(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

/// The grid's markers with their tempos; `None` for an empty or broken one.
fn grid(data: &[u8]) -> Option<Vec<GridMarker>> {
    let rest = data.strip_prefix(&[1, 0][..])?;
    let count = u32::from_be_bytes(rest.get(..4)?.try_into().ok()?) as usize;
    if count == 0 || rest.len() < 4 + count * 8 {
        return None;
    }
    let at = |i: usize| -> (f32, [u8; 4]) {
        let b = &rest[4 + i * 8..4 + i * 8 + 8];
        (
            f32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            [b[4], b[5], b[6], b[7]],
        )
    };
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let (start, tail) = at(i);
        let bpm = if i + 1 == count {
            f64::from(f32::from_be_bytes(tail))
        } else {
            let beats = f64::from(u32::from_be_bytes(tail));
            let span = f64::from(at(i + 1).0) - f64::from(start);
            beats * 60.0 / span
        };
        if !(bpm.is_finite() && bpm > 0.0) {
            return None;
        }
        out.push(GridMarker {
            seconds: f64::from(start),
            bpm,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::STANDARD;

    use super::*;

    fn entry(name: &str, body: &[u8]) -> Vec<u8> {
        let mut out = name.as_bytes().to_vec();
        out.push(0);
        out.extend((body.len() as u32).to_be_bytes());
        out.extend(body);
        out
    }

    /// Markers2 as Serato writes it.
    fn markers2() -> Vec<u8> {
        let mut raw = vec![1, 1];
        raw.extend(entry("COLOR", &[0, 0xff, 0xff, 0xff]));
        let mut cue = vec![0, 2];
        cue.extend(1500u32.to_be_bytes());
        cue.extend([0, 0xcc, 0, 0, 0, 0]);
        cue.extend(b"Drop\0");
        raw.extend(entry("CUE", &cue));
        let mut lp = vec![0, 0];
        lp.extend(30_000u32.to_be_bytes());
        lp.extend(32_000u32.to_be_bytes());
        lp.extend([0xff, 0xff, 0xff, 0xff, 0, 0x27, 0xaa, 0xe1, 0]);
        lp.extend(b"Build\0");
        raw.extend(entry("LOOP", &lp));
        raw.extend(entry("BPMLOCK", &[1]));
        raw.push(0);
        let text = STANDARD.encode(&raw);
        let text = text.trim_end_matches('=');
        let mut out = vec![1, 1];
        for (i, c) in text.as_bytes().chunks(72).enumerate() {
            if i > 0 {
                out.push(b'\n');
            }
            out.extend(c);
        }
        out.extend([0; 16]);
        out
    }

    #[test]
    fn reads_cues_loops_and_lock() {
        let m = markers(&markers2()).unwrap();
        assert!(m.locked);
        let cues: Vec<_> = m
            .hotcues
            .iter()
            .map(|h| (h.number, h.seconds, h.end, h.name.as_str(), h.color))
            .collect();
        assert_eq!(
            cues,
            [
                (2, 1.5, None, "Drop", Some(0xcc_0000)),
                (MEMORY_CUE, 30.0, Some(32.0), "Build", None),
            ]
        );
    }

    #[test]
    fn reads_a_grid_with_a_tempo_change() {
        let mut g = vec![1, 0];
        g.extend(2u32.to_be_bytes());
        g.extend(0.05f32.to_be_bytes());
        g.extend(64u32.to_be_bytes());
        g.extend(32.05f32.to_be_bytes());
        g.extend(124f32.to_be_bytes());
        g.push(0);
        let m = grid(&g).unwrap();
        assert_eq!(m.len(), 2);
        assert!((m[0].bpm - 120.0).abs() < 1e-3);
        assert!((m[1].seconds - 32.05).abs() < 1e-5);
        assert_eq!(m[1].bpm, 124.0);
        assert!(grid(&[1, 0, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn reads_geob_frames_and_wrapped_blobs() {
        let mut body = vec![0];
        body.extend(b"application/octet-stream\0\0Serato Markers2\0");
        body.extend([1, 1, b'A']);
        let (name, data) = geob(&body).unwrap();
        assert_eq!((name.as_str(), data), (MARKERS, &[1, 1, b'A'][..]));

        let mut wrapped = b"application/octet-stream\0\0Serato BeatGrid\0".to_vec();
        wrapped.extend([1, 0, 9]);
        let mut blobs = Blobs::default();
        blobs.put_wrapped(&STANDARD.encode(&wrapped));
        assert_eq!(blobs.grid.as_deref(), Some(&[1, 0, 9][..]));
    }
}
