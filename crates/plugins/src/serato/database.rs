//! Parses Serato's `database V2` binary format.
//!
//! The format is a TLV (Tag-Length-Value) structure with big-endian encoding:
//! - 4-byte tag (ASCII)
//! - 4-byte length (big-endian u32)
//! - Value bytes (length bytes)
//!
//! Top-level tags:
//! - `vrsn`: version string (UTF-16BE)
//! - `otrk`: track record (contains sub-tags)
//!
//! Track sub-tags (inside `otrk`):
//! - `pfil`: file path (UTF-16BE, no leading slash, volume-relative)
//! - `tsng`: track title (UTF-16BE)
//! - `tart`: artist (UTF-16BE)
//! - `talb`: album (UTF-16BE)
//! - `tgen`: genre (UTF-16BE)
//! - `tbpm`: BPM (big-endian f32)
//! - `tkey`: key (UTF-16BE, e.g. "8A")
//! - `tlen`: duration in seconds (big-endian f32)
//! - `trating`: rating 0-5 (big-endian u32)
//! - `tplay`: play count (big-endian u32)
//! - `tadd`: added date, seconds since 2001-01-01 (big-endian u32)
//! - `tcol`: color as packed RGB (big-endian u32, 0xRRGGBB)
//! - `tcom`: comment (UTF-16BE)
//! - `tgrp`: grouping (UTF-16BE)
//! - `tyer`: year (UTF-16BE)
//! - `tlab`: label (UTF-16BE)
//! - `tcmp`: composer (UTF-16BE)
//! - `trmx`: remixer (UTF-16BE)
//! - `tisrc`: ISRC (UTF-16BE)
//! - `tbit`: bitrate (big-endian u32)
//! - `tsiz`: file size (big-endian u32)
//! - `tfbe`: first beat offset in seconds (big-endian f32)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::serato::TrackRecordKey;

/// A track record from the database V2.
#[derive(Debug, Clone)]
pub struct TrackRecord {
    pub key: TrackRecordKey,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub bpm: f64,
    pub key: String,
    pub duration: f64,
    pub first_beat: f64,
    pub rating: u8,
    pub play_count: u32,
    pub added_date: i64,
    pub color: Option<u32>,
    pub comment: String,
    pub grouping: String,
    pub year: String,
    pub label: String,
    pub composer: String,
    pub remixer: String,
    pub isrc: String,
}

/// Unique key for a track record: the volume-relative path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrackRecordKey(pub String);

impl TrackRecord {
    /// Resolves the volume-relative path to an absolute path, trying the
    /// Serato root first, then /Volumes/<volume>/ on macOS.
    pub fn resolve_path(&self, serato_root: &Path) -> Option<PathBuf> {
        // Path is volume-relative, e.g. "Music/Track.mp3" or "VolumeName/Music/Track.mp3"
        let path = PathBuf::from(&self.path);

        // Try relative to the Serato root's parent (the volume root)
        let volume_root = serato_root.parent()?;
        let candidate = volume_root.join(&path);
        if candidate.exists() {
            return Some(candidate);
        }

        // On macOS, try /Volumes/<volume>/<path>
        #[cfg(target_os = "macos")]
        {
            let components: Vec<_> = path.components().collect();
            if !components.is_empty() {
                let volume_name = components[0].as_os_str();
                let rest = components[1..].iter().collect::<PathBuf>();
                let candidate = PathBuf::from("/Volumes").join(volume_name).join(rest);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }
}

/// Reads the `database V2` file and returns all track records.
pub fn read_database(
    path: &Path,
) -> Result<(HashMap<TrackRecordKey, TrackRecord>, String), String> {
    let data = std::fs::read(path).map_err(|e| format!("can't read {}: {}", path.display(), e))?;
    let mut cursor = Cursor::new(&data);

    let mut version = String::new();
    let mut tracks = HashMap::new();

    while cursor.remaining() >= 8 {
        let tag = cursor.read_tag()?;
        let len = cursor.read_u32()? as usize;
        if cursor.remaining() < len {
            return Err(format!("truncated tag {}", String::from_utf8_lossy(&tag)));
        }
        let value = cursor.read_bytes(len)?;

        match &tag {
            b"vrsn" => version = read_utf16be(&value),
            b"otrk" => {
                let record = parse_track_record(&value)?;
                tracks.insert(record.key.clone(), record);
            }
            _ => {
                // Skip unknown top-level tags
            }
        }
    }

    Ok((tracks, version))
}

/// Reads history sessions from the `History/` folder.
#[derive(Debug, Clone)]
pub struct HistorySession {
    pub name: String,
    pub start_date: i64,
    pub track_keys: Vec<TrackRecordKey>,
}

pub fn read_history(root: &Path) -> Result<Vec<HistorySession>, String> {
    let history_dir = root.join("History");
    if !history_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(&history_dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("session") {
            if let Ok(session) = parse_history_session(&path) {
                sessions.push(session);
            }
        }
    }
    sessions.sort_by_key(|s| s.start_date);
    Ok(sessions)
}

fn parse_history_session(path: &Path) -> Result<HistorySession, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut cursor = Cursor::new(&data);

    let mut name = String::new();
    let mut start_date = 0i64;
    let mut track_keys = Vec::new();

    while cursor.remaining() >= 8 {
        let tag = cursor.read_tag()?;
        let len = cursor.read_u32()? as usize;
        if cursor.remaining() < len {
            break;
        }
        let value = cursor.read_bytes(len)?;

        match &tag {
            b"snme" => name = read_utf16be(&value),
            b"sdat" => start_date = read_apple_timestamp(&value),
            b"strk" => {
                // Sub-tag with track key
                let mut sub = Cursor::new(&value);
                while sub.remaining() >= 8 {
                    let stag = sub.read_tag()?;
                    let slen = sub.read_u32()? as usize;
                    if sub.remaining() < slen {
                        break;
                    }
                    let sval = sub.read_bytes(slen)?;
                    if stag == *b"tkey" {
                        track_keys.push(TrackRecordKey(read_utf16be(&sval)));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(HistorySession {
        name,
        start_date,
        track_keys,
    })
}

/// Parses a track record from its `otrk` value bytes.
fn parse_track_record(data: &[u8]) -> Result<TrackRecord, String> {
    let mut cursor = Cursor::new(data);

    let mut path = String::new();
    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    let mut genre = String::new();
    let mut bpm = 0.0;
    let mut key = String::new();
    let mut duration = 0.0;
    let mut first_beat = 0.0;
    let mut rating = 0u8;
    let mut play_count = 0u32;
    let mut added_date = 0i64;
    let mut color = None;
    let mut comment = String::new();
    let mut grouping = String::new();
    let mut year = String::new();
    let mut label = String::new();
    let mut composer = String::new();
    let mut remixer = String::new();
    let mut isrc = String::new();

    while cursor.remaining() >= 8 {
        let tag = cursor.read_tag()?;
        let len = cursor.read_u32()? as usize;
        if cursor.remaining() < len {
            break;
        }
        let value = cursor.read_bytes(len)?;

        match &tag {
            b"pfil" => path = read_utf16be(&value),
            b"tsng" => title = read_utf16be(&value),
            b"tart" => artist = read_utf16be(&value),
            b"talb" => album = read_utf16be(&value),
            b"tgen" => genre = read_utf16be(&value),
            b"tbpm" => {
                if value.len() == 4 {
                    bpm = f32::from_be_bytes([value[0], value[1], value[2], value[3]]) as f64;
                }
            }
            b"tkey" => key = read_utf16be(&value),
            b"tlen" => {
                if value.len() == 4 {
                    duration = f32::from_be_bytes([value[0], value[1], value[2], value[3]]) as f64;
                }
            }
            b"tfbe" => {
                if value.len() == 4 {
                    first_beat =
                        f32::from_be_bytes([value[0], value[1], value[2], value[3]]) as f64;
                }
            }
            b"trating" => {
                if value.len() == 4 {
                    rating = u32::from_be_bytes([value[0], value[1], value[2], value[3]]) as u8;
                }
            }
            b"tplay" => {
                if value.len() == 4 {
                    play_count = u32::from_be_bytes([value[0], value[1], value[2], value[3]]);
                }
            }
            b"tadd" => {
                if value.len() == 4 {
                    // Seconds since 2001-01-01 (Apple epoch)
                    let secs = u32::from_be_bytes([value[0], value[1], value[2], value[3]]) as i64;
                    added_date = apple_epoch_to_unix(secs);
                }
            }
            b"tcol" => {
                if value.len() == 4 {
                    color = Some(u32::from_be_bytes([value[0], value[1], value[2], value[3]]));
                }
            }
            b"tcom" => comment = read_utf16be(&value),
            b"tgrp" => grouping = read_utf16be(&value),
            b"tyer" => year = read_utf16be(&value),
            b"tlab" => label = read_utf16be(&value),
            b"tcmp" => composer = read_utf16be(&value),
            b"trmx" => remixer = read_utf16be(&value),
            b"tisrc" => isrc = read_utf16be(&value),
            _ => {}
        }
    }

    let key = TrackRecordKey(path.clone());
    Ok(TrackRecord {
        key,
        path,
        title,
        artist,
        album,
        genre,
        bpm,
        key,
        duration,
        first_beat,
        rating,
        play_count,
        added_date,
        color,
        comment,
        grouping,
        year,
        label,
        composer,
        remixer,
        isrc,
    })
}

/// Apple epoch (2001-01-01) to Unix epoch (1970-01-01) conversion.
const APPLE_EPOCH_OFFSET: i64 = 978_307_200; // seconds between 1970 and 2001

fn apple_epoch_to_unix(apple_secs: i64) -> i64 {
    apple_secs + APPLE_EPOCH_OFFSET
}

fn read_apple_timestamp(data: &[u8]) -> i64 {
    if data.len() == 8 {
        // f64 seconds since 2001
        let bits = u64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);
        let secs = f64::from_bits(bits) as i64;
        apple_epoch_to_unix(secs)
    } else if data.len() == 4 {
        let secs = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as i64;
        apple_epoch_to_unix(secs)
    } else {
        0
    }
}

/// Reads a UTF-16BE string (no BOM).
fn read_utf16be(data: &[u8]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i + 1 < data.len() {
        let c = u16::from_be_bytes([data[i], data[i + 1]]);
        if c == 0 {
            break;
        }
        if let Some(ch) = char::from_u32(c as u32) {
            s.push(ch);
        }
        i += 2;
    }
    s
}

/// Simple cursor for reading big-endian values from a byte slice.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_tag(&mut self) -> Result<[u8; 4], String> {
        if self.remaining() < 4 {
            return Err("truncated tag".into());
        }
        let tag = [
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ];
        self.pos += 4;
        Ok(tag)
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        if self.remaining() < 4 {
            return Err("truncated u32".into());
        }
        let v = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], String> {
        if self.remaining() < len {
            return Err("truncated value".into());
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.data[start..self.pos])
    }
}

/// Converts history sessions to imported lists.
pub fn history_to_list(
    session: &HistorySession,
    track_by_path: &HashMap<PathBuf, usize>,
    track_by_key: &HashMap<TrackRecordKey, usize>,
) -> Result<Vec<ImportedList>, String> {
    use crate::{ImportedEntry, ImportedList, ImportedListKind};
    use std::path::PathBuf;

    let mut entries = Vec::new();
    for key in &session.track_keys {
        // Try to find the track by key first, then we'd need a path
        // For now, we create entries with the path from the key
        let path = PathBuf::from(&key.0);
        if let Some(&id) = track_by_path.get(&path) {
            entries.push(ImportedEntry::new(path, Some(session.start_date)));
        } else {
            // Track not found locally, add as placeholder
            entries.push(ImportedEntry::new(path, Some(session.start_date)));
        }
    }

    Ok(vec![ImportedList {
        folders: vec!["History".into()],
        name: session.name.clone(),
        kind: ImportedListKind::History {
            started: session.start_date,
        },
        entries,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_utf16be() {
        let data = "Hello"
            .encode_utf16()
            .flat_map(|c| c.to_be_bytes())
            .collect::<Vec<_>>();
        assert_eq!(read_utf16be(&data), "Hello");
    }

    #[test]
    fn test_apple_epoch() {
        assert_eq!(apple_epoch_to_unix(0), 978_307_200);
        assert_eq!(apple_epoch_to_unix(3600), 978_310_800);
    }
}
