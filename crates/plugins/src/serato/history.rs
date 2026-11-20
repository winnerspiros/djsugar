//! Serato's history: one `History/Sessions/*.session` file per session,
//! next to `database V2`. After `vrsn`, an `oses` holds the session's own
//! details and each `oent` a track played in it. Both keep them in an
//! `adat`: numbered fields, each a big-endian `u32` number, a `u32`
//! length, then the value (UTF-16BE text or a `u32`).
//!
//! In a session: 41 its date as Serato shows it, 43 when it started. In a
//! track: 2 its file's full path, 28 when it started playing (Unix time),
//! 50 whether it was played (0 for one only loaded onto a deck).

use std::path::{Path, PathBuf};

use djsugar_engine::import::{ImportedEntry, ImportedListKind};

use super::{fields, file, number, records, text};
use crate::ImportedList;

/// The session files, oldest first by name (Serato numbers them).
pub fn files(database: &Path) -> Vec<PathBuf> {
    let dir = database
        .parent()
        .unwrap_or(Path::new(""))
        .join("History")
        .join("Sessions");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "session"))
        .collect();
    files.sort();
    files
}

/// The numbered fields in an `adat`.
fn adat(data: &[u8]) -> impl Iterator<Item = (u32, &[u8])> {
    fields(data).filter_map(|(id, v)| Some((u32::from_be_bytes(id.try_into().ok()?), v)))
}

/// A session, if any track in it was played.
pub fn session(path: &Path, disk: &Path) -> Option<ImportedList> {
    let data = std::fs::read(path).ok()?;
    let mut name = String::new();
    let mut started = None;
    if let Some(info) = records(&data, b"oses")
        .next()
        .and_then(|o| records(o, b"adat").next())
    {
        for (id, v) in adat(info) {
            match id {
                41 => name = text(v),
                43 => started = number(v).map(i64::from),
                _ => {}
            }
        }
    }
    let mut entries = Vec::new();
    for entry in records(&data, b"oent") {
        let Some(row) = records(entry, b"adat").next() else {
            continue;
        };
        let mut path = None;
        let mut at = None;
        let mut played = true;
        for (id, v) in adat(row) {
            match id {
                2 => path = Some(text(v)),
                28 => at = number(v).map(i64::from),
                50 => played = number(v) != Some(0),
                _ => {}
            }
        }
        let Some(path) = path.filter(|p| !p.is_empty() && played) else {
            continue;
        };
        let full = if Path::new(&path).is_absolute() {
            PathBuf::from(&path)
        } else {
            file(&path, disk)
        };
        entries.push(ImportedEntry::new(full, at));
    }
    if entries.is_empty() {
        return None;
    }
    let started = entries
        .iter()
        .filter_map(|e| e.played_at)
        .min()
        .or(started)
        .unwrap_or(0);
    if name.is_empty() {
        name = path.file_stem()?.to_string_lossy().into_owned();
    }
    Some(ImportedList {
        folders: Vec::new(),
        name,
        kind: ImportedListKind::History { started },
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::super::export::{field, utf16};
    use super::*;

    fn num(id: u32, value: &[u8]) -> Vec<u8> {
        field(&id.to_be_bytes(), value)
    }

    fn track(path: &str, at: u32, played: u32) -> Vec<u8> {
        let mut a = num(1, &7u32.to_be_bytes());
        a.extend(num(2, &utf16(path)));
        a.extend(num(6, &utf16("Title")));
        a.extend(num(28, &at.to_be_bytes()));
        a.extend(num(29, &(at + 200).to_be_bytes()));
        a.extend(num(50, &played.to_be_bytes()));
        field(b"oent", &field(b"adat", &a))
    }

    #[test]
    fn broken_sessions_are_turned_down() {
        let mut info = num(41, &utf16("13/05/2020"));
        info.extend(num(43, &1_589_400_000u32.to_be_bytes()));
        let mut d = field(b"oses", &field(b"adat", &info));
        d.extend(track("Music/a.mp3", 1_589_400_100, 1));
        let file = std::env::temp_dir().join(format!("djs-garble-{}.session", std::process::id()));
        crate::garble::check(&[d], |b| {
            std::fs::write(&file, b).unwrap();
            let _ = session(&file, Path::new("/"));
        });
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn reads_a_session() {
        let dir = std::env::temp_dir().join(format!("djs-session-{}", std::process::id()));
        let sessions = dir.join("_Serato_/History/Sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let mut d = field(b"vrsn", &utf16("1.0/Serato Scratch LIVE Review"));
        let mut info = num(1, &5u32.to_be_bytes());
        info.extend(num(41, &utf16("13/05/2020")));
        info.extend(num(43, &1_589_400_000u32.to_be_bytes()));
        d.extend(field(b"oses", &field(b"adat", &info)));
        // A full path (on another disk), then one from the top of this one.
        let elsewhere = dir.join("Elsewhere").join("a.mp3");
        d.extend(track(elsewhere.to_str().unwrap(), 1_589_400_100, 1));
        d.extend(track("/Music/loaded.mp3", 1_589_400_200, 0));
        d.extend(track("Music/b.mp3", 1_589_400_300, 1));
        std::fs::write(sessions.join("5.session"), &d).unwrap();
        // Nothing played: no session.
        let mut empty = field(b"vrsn", &utf16("1.0/Serato Scratch LIVE Review"));
        empty.extend(track("/Music/c.mp3", 1, 0));
        std::fs::write(sessions.join("6.session"), &empty).unwrap();
        let database = dir.join("_Serato_/database V2");
        let found = files(&database);
        let lists: Vec<ImportedList> = found.iter().filter_map(|f| session(f, &dir)).collect();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(found.len(), 2);
        assert_eq!(lists.len(), 1);
        let s = &lists[0];
        assert_eq!(s.name, "13/05/2020");
        assert_eq!(
            s.kind,
            ImportedListKind::History {
                started: 1_589_400_100
            }
        );
        let got: Vec<(PathBuf, Option<i64>)> = s
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.played_at))
            .collect();
        assert_eq!(
            got,
            [
                (elsewhere, Some(1_589_400_100)),
                (dir.join("Music").join("b.mp3"), Some(1_589_400_300)),
            ]
        );
    }
}
