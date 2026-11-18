//! Hands the library to Mixxx: each track Mixxx already has gets its hot
//! cues, saved loops, beat grid and tempo from here, and the lists become
//! Mixxx playlists. Tracks Mixxx doesn't have yet are left out (Mixxx adds
//! them itself when it scans their folder) and counted.
//!
//! Mixxx's database is changed in one transaction, after a copy of it is
//! kept as `mixxxdb.sqlite.djsugar-backup`. Only cues of the kinds written
//! here (hot cues and loops) are replaced; the main cue, intro and outro
//! stay. Mixxx should be closed while this runs.
//!
//! Positions in `cues` count samples of both channels (two a frame); the
//! grid is a protocol buffer: `BeatGrid-2.0` (tempo and first beat's frame)
//! for a steady tempo, `BeatMap-1.0` (each beat's frame) when it changes.

use std::collections::HashMap;
use std::path::Path;

use djsugar_engine::{Source, Track, MEMORY_CUE};
use rusqlite::{params, Connection, OptionalExtension};

const HOT_CUE: i64 = 1;
const LOOP: i64 = 4;

/// What the export did.
#[derive(Debug, Default, PartialEq)]
pub struct Written {
    pub tracks: usize,
    /// Tracks Mixxx doesn't have yet.
    pub missing: usize,
    pub playlists: usize,
}

/// A list to write: its name (folders joined in, as Mixxx has none) and
/// tracks in order.
pub struct MixxxList<'a> {
    pub name: String,
    pub tracks: Vec<&'a Track>,
}

pub fn write(database: &Path, tracks: &[Track], lists: &[MixxxList]) -> Result<Written, String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", database.display());
    if !database.is_file() {
        return Err(fail(&"not found"));
    }
    let mut db = Connection::open(database).map_err(|e| fail(&e))?;
    let backup = database.with_file_name("mixxxdb.sqlite.djsugar-backup");
    let _ = std::fs::remove_file(&backup);
    db.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
        .map_err(|e| fail(&e))?;
    let tx = db.transaction().map_err(|e| fail(&e))?;
    let written = write_into(&tx, tracks, lists).map_err(|e| fail(&e))?;
    tx.commit().map_err(|e| fail(&e))?;
    Ok(written)
}

fn write_into(db: &Connection, tracks: &[Track], lists: &[MixxxList]) -> rusqlite::Result<Written> {
    let mut ids: HashMap<String, (i64, f64)> = HashMap::new();
    {
        let mut q = db.prepare(
            "SELECT l.id, t.location, l.samplerate FROM library l
             JOIN track_locations t ON t.id = l.location WHERE l.mixxx_deleted = 0",
        )?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                (
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                ),
            ))
        })?;
        for row in rows {
            let (location, id) = row?;
            ids.insert(key(&location), id);
        }
    }
    let find = |t: &Track| match &t.source {
        Source::File(p) => ids.get(&key(&p.to_string_lossy())).copied(),
        Source::Synth { .. } => None,
    };
    let mut out = Written::default();
    for t in tracks {
        let Some((id, rate)) = find(t) else {
            out.missing += 1;
            continue;
        };
        let rate = if rate > 0.0 { rate } else { 44_100.0 };
        let samples = |s: f64| (s.max(0.0) * rate).round() as i64 * 2;
        db.execute(
            "DELETE FROM cues WHERE track_id = ?1 AND type IN (?2, ?3)",
            params![id, HOT_CUE, LOOP],
        )?;
        for h in &t.info.hotcues {
            let pad = (h.number < MEMORY_CUE).then_some(h.number as i64);
            let (kind, length) = match h.end {
                Some(end) if end > h.seconds => (LOOP, samples(end) - samples(h.seconds)),
                None if pad.is_some() => (HOT_CUE, 0),
                _ => continue,
            };
            db.execute(
                "INSERT INTO cues (track_id, type, position, length, hotcue, label, color)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    kind,
                    samples(h.seconds),
                    length,
                    pad.unwrap_or(-1),
                    h.name,
                    i64::from(h.color.unwrap_or(0xFF_0000) & 0xFF_FFFF)
                ],
            )?;
        }
        if let Some((version, blob)) = beats(t, rate) {
            db.execute(
                "UPDATE library SET bpm = ?2, beats = ?3, beats_version = ?4 WHERE id = ?1",
                params![id, t.bpm, blob, version],
            )?;
        }
        out.tracks += 1;
    }
    for l in lists {
        let old: Option<i64> = db
            .query_row(
                "SELECT id FROM Playlists WHERE name = ?1 AND hidden = 0",
                [&l.name],
                |r| r.get(0),
            )
            .optional()?;
        let id = match old {
            Some(id) => {
                db.execute("DELETE FROM PlaylistTracks WHERE playlist_id = ?1", [id])?;
                id
            }
            None => {
                let position: i64 = db.query_row(
                    "SELECT COALESCE(MAX(position), 0) + 1 FROM Playlists",
                    [],
                    |r| r.get(0),
                )?;
                db.execute(
                    "INSERT INTO Playlists (name, position, hidden, date_created, date_modified, locked)
                     VALUES (?1, ?2, 0, datetime('now'), datetime('now'), 0)",
                    params![l.name, position],
                )?;
                db.last_insert_rowid()
            }
        };
        let mut position = 1;
        for t in &l.tracks {
            if let Some((track, _)) = find(t) {
                db.execute(
                    "INSERT INTO PlaylistTracks (playlist_id, track_id, position, pl_datetime_added)
                     VALUES (?1, ?2, ?3, datetime('now'))",
                    params![id, track, position],
                )?;
                position += 1;
            }
        }
        out.playlists += 1;
    }
    Ok(out)
}

/// Mixxx compares paths as written; on Windows and macOS case doesn't
/// matter, and Windows paths may use either slash.
fn key(path: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        path.replace('\\', "/").to_lowercase()
    } else {
        path.to_string()
    }
}

/// The grid as Mixxx's beats blob, with its version.
fn beats(t: &Track, rate: f64) -> Option<(&'static str, Vec<u8>)> {
    let grid = t.beat_grid();
    let markers = grid.markers();
    let first = markers.first()?;
    if markers.len() == 1 {
        let mut bpm = vec![0x09];
        bpm.extend(first.bpm.to_le_bytes());
        let mut beat = vec![0x08];
        varint(&mut beat, (first.seconds.max(0.0) * rate).round() as u64);
        let mut out = vec![0x0a, bpm.len() as u8];
        out.extend(bpm);
        out.push(0x12);
        varint(&mut out, beat.len() as u64);
        out.extend(beat);
        return Some(("BeatGrid-2.0", out));
    }
    let end = if t.duration > 0.0 {
        t.duration
    } else {
        markers.last()?.seconds + 600.0
    };
    let mut out = Vec::new();
    for (i, m) in markers.iter().enumerate() {
        let until = markers.get(i + 1).map_or(end, |n| n.seconds);
        let step = 60.0 / m.bpm;
        if !(step.is_finite() && step > 0.0) {
            return None;
        }
        let mut at = m.seconds;
        while at < until - step * 0.01 {
            if at >= 0.0 {
                let mut beat = vec![0x08];
                varint(&mut beat, (at * rate).round() as u64);
                out.push(0x0a);
                varint(&mut out, beat.len() as u64);
                out.extend(beat);
            }
            at += step;
        }
    }
    Some(("BeatMap-1.0", out))
}

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use djsugar_engine::beats::{BeatGrid, GridMarker};
    use djsugar_engine::{Hotcue, LibraryInfo};

    use super::*;

    fn track(path: &str, markers: Vec<GridMarker>) -> Track {
        let mut t = Track {
            id: 0,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: markers[0].bpm,
            key: String::new(),
            duration: 60.0,
            first_beat: markers[0].seconds,
            grid: Default::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: LibraryInfo {
                hotcues: vec![
                    Hotcue {
                        number: 1,
                        seconds: 1.5,
                        name: "Drop".into(),
                        end: None,
                        color: Some(0x00_88CC),
                    },
                    Hotcue {
                        number: 3,
                        seconds: 10.0,
                        name: String::new(),
                        end: Some(12.0),
                        color: None,
                    },
                ],
                ..Default::default()
            },
        };
        t.set_grid(BeatGrid::new(markers));
        t
    }

    fn marker(seconds: f64, bpm: f64) -> GridMarker {
        GridMarker { seconds, bpm }
    }

    /// Written into a Mixxx database, then read back by our Mixxx importer.
    #[test]
    fn reads_back() {
        let dir = std::env::temp_dir().join(format!("djs-mixxx-x-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mixxxdb.sqlite");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(crate::mixxx::tests::SCHEMA).unwrap();
            db.execute_batch(
                "INSERT INTO track_locations (id, location) VALUES (1, '/m/a.mp3'), (2, '/m/b.mp3');
                 INSERT INTO library (id, location, samplerate, bpm) VALUES (10, 1, 44100, 0), (11, 2, 48000, 0);
                 INSERT INTO cues (track_id, type, position, length, hotcue) VALUES (10, 2, 4410, 0, -1);
                 INSERT INTO cues (track_id, type, position, length, hotcue) VALUES (10, 1, 999, 0, 0);",
            )
            .unwrap();
        }
        let a = track("/m/a.mp3", vec![marker(0.1, 128.0)]);
        let b = track("/m/b.mp3", vec![marker(0.5, 120.0), marker(30.5, 124.0)]);
        let c = track("/m/elsewhere.mp3", vec![marker(0.0, 100.0)]);
        let tracks = [a, b, c];
        let lists = [MixxxList {
            name: "Gigs / Friday".into(),
            tracks: vec![&tracks[1], &tracks[2], &tracks[0]],
        }];
        let w = write(&path, &tracks, &lists).unwrap();
        // Again: replaces, doesn't add.
        let again = write(&path, &tracks, &lists).unwrap();
        let back = crate::mixxx::read(&path, &[crate::mixxx::PLAYLISTS], |_: &Path| true).unwrap();
        let backup = dir.join("mixxxdb.sqlite.djsugar-backup").is_file();
        std::fs::remove_dir_all(&dir).ok();

        assert!(backup);
        assert_eq!(
            w,
            Written {
                tracks: 2,
                missing: 1,
                playlists: 1
            }
        );
        assert_eq!(again, w);
        let get = |p: &str| {
            back.tracks
                .iter()
                .find(|t| matches!(&t.source, Source::File(f) if f == Path::new(p)))
                .unwrap()
        };
        let a = get("/m/a.mp3");
        assert_eq!(a.bpm, 128.0);
        assert!((a.grid.markers()[0].seconds - 0.1).abs() < 1e-4);
        let cues: Vec<_> = a
            .info
            .hotcues
            .iter()
            .map(|h| {
                (
                    h.number,
                    (h.seconds * 1000.0).round(),
                    h.end.map(|e| (e * 1000.0).round()),
                    h.name.as_str(),
                )
            })
            .collect();
        // The main cue Mixxx had stays; the old pad 1 is replaced.
        assert_eq!(
            cues,
            [
                (1, 1500.0, None, "Drop"),
                (3, 10000.0, Some(12000.0), ""),
                (MEMORY_CUE, 50.0, None, ""),
            ]
        );
        let m = get("/m/b.mp3").grid.markers().to_vec();
        assert_eq!(m.len(), 2);
        assert!((m[0].bpm - 120.0).abs() < 0.1 && (m[1].bpm - 124.0).abs() < 0.1);
        assert!((m[1].seconds - 30.5).abs() < 1e-3);
        assert_eq!(back.lists.len(), 1);
        assert_eq!(back.lists[0].name, "Gigs / Friday");
        let order: Vec<PathBuf> = back.lists[0]
            .entries
            .iter()
            .map(|e| e.path.clone())
            .collect();
        assert_eq!(
            order,
            [PathBuf::from("/m/b.mp3"), PathBuf::from("/m/a.mp3")]
        );
    }
}
