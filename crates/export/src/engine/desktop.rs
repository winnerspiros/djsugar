//! Hands the library to Engine DJ on this computer: each track Engine
//! already has gets its hot cues, saved loops and beat grid from here, and
//! the lists become Engine playlists (one of the same name is refilled).
//! Unlike a stick, the DJ's own Engine library is kept: nothing else in it
//! changes, and tracks Engine doesn't have yet are left out and counted
//! (Engine adds them when it scans their folder).
//!
//! The old `m.db` is kept as `m.db.djsugar-backup`, and everything is
//! written in one transaction. Engine DJ should be closed while this runs.
//!
//! Track paths in `m.db` are relative to the `Engine Library` folder.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use djsugar_engine::key::Key;
use djsugar_engine::{Source, Track};
use rusqlite::{params, Connection, OptionalExtension};

use super::perf;

/// What the export did.
#[derive(Debug, Default, PartialEq)]
pub struct Written {
    pub tracks: usize,
    /// Tracks Engine doesn't have yet.
    pub missing: usize,
    pub playlists: usize,
}

/// A list to write: its name and tracks in order.
pub struct EngineList<'a> {
    pub name: String,
    pub tracks: Vec<&'a Track>,
}

/// Writes into the `m.db` at `database` (in `Engine Library/Database2`).
pub fn write(database: &Path, tracks: &[Track], lists: &[EngineList]) -> Result<Written, String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", database.display());
    if !database.is_file() {
        return Err(fail(&"not found"));
    }
    let library = database
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| fail(&"not in an Engine Library folder"))?;
    let mut db = Connection::open(database).map_err(|e| fail(&e))?;
    let backup = database.with_file_name("m.db.djsugar-backup");
    let _ = std::fs::remove_file(&backup);
    db.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
        .map_err(|e| fail(&e))?;
    let tx = db.transaction().map_err(|e| fail(&e))?;
    let written = write_into(&tx, library, tracks, lists).map_err(|e| fail(&e))?;
    tx.commit().map_err(|e| fail(&e))?;
    Ok(written)
}

/// `path` with `.` and `..` worked out, without touching the disk.
fn tidy(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// A path as the same text however it was written: on Windows and macOS
/// letters' case doesn't matter.
fn key(path: &Path) -> String {
    let s = tidy(path).to_string_lossy().replace('\\', "/");
    if cfg!(any(windows, target_os = "macos")) {
        s.to_lowercase()
    } else {
        s
    }
}

fn write_into(
    db: &Connection,
    library: &Path,
    tracks: &[Track],
    lists: &[EngineList],
) -> rusqlite::Result<Written> {
    // Engine's tracks by file, with the sample rate its analysis used.
    let mut ids: HashMap<String, (i64, f64)> = HashMap::new();
    {
        let mut q = db.prepare(
            "SELECT t.id, t.path, p.trackData FROM Track t
             LEFT JOIN PerformanceData p ON p.trackId = t.id WHERE t.path IS NOT NULL",
        )?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })?;
        for row in rows {
            let (id, path, data) = row?;
            let rate = data.as_deref().and_then(sample_rate).unwrap_or(44_100.0);
            ids.insert(key(&library.join(path)), (id, rate));
        }
    }
    let find = |t: &Track| match &t.source {
        Source::File(p) => ids.get(&key(p)).copied(),
        Source::Synth { .. } => None,
    };
    let mut out = Written::default();
    for t in tracks {
        let Some((id, rate)) = find(t) else {
            out.missing += 1;
            continue;
        };
        let grid = t.beat_grid();
        let built = perf::build(&perf::Input {
            sample_rate: rate,
            duration: t.duration,
            grid: &grid,
            waveform: &t.waveform,
            cues: &t.info.hotcues,
            key: Key::parse(&t.key),
        });
        let exists: Option<i64> = db
            .query_row(
                "SELECT trackId FROM PerformanceData WHERE trackId = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            db.execute("INSERT INTO PerformanceData (trackId) VALUES (?1)", [id])?;
        }
        db.execute(
            "UPDATE PerformanceData SET quickCues = ?2, loops = ?3,
             beatData = COALESCE(?4, beatData) WHERE trackId = ?1",
            params![id, built.quick_cues, built.loops, built.beats],
        )?;
        if t.bpm > 0.0 {
            db.execute(
                "UPDATE Track SET bpm = ?2, bpmAnalyzed = ?3, isBeatGridLocked = ?4,
                 lastEditTime = strftime('%s') WHERE id = ?1",
                params![id, t.bpm.round() as i64, t.bpm, t.info.grid_locked],
            )?;
        }
        out.tracks += 1;
    }
    let database: String =
        db.query_row("SELECT uuid FROM Information LIMIT 1", [], |r| r.get(0))?;
    for l in lists {
        let old: Option<i64> = db
            .query_row(
                "SELECT id FROM Playlist WHERE title = ?1 AND parentListId = 0",
                [&l.name],
                |r| r.get(0),
            )
            .optional()?;
        let list = match old {
            Some(id) => {
                db.execute("DELETE FROM PlaylistEntity WHERE listId = ?1", [id])?;
                id
            }
            None => {
                // With nextListId 0, Engine's triggers put it last.
                db.execute(
                    "INSERT INTO Playlist (title, parentListId, isPersisted, nextListId,
                     lastEditTime, isExplicitlyExported) VALUES (?1, 0, 1, 0, strftime('%s'), 1)",
                    [&l.name],
                )?;
                db.last_insert_rowid()
            }
        };
        let mut previous: Option<i64> = None;
        for t in &l.tracks {
            let Some((track, _)) = find(t) else {
                continue;
            };
            db.execute(
                "INSERT INTO PlaylistEntity (listId, trackId, databaseUuid, nextEntityId,
                 membershipReference) VALUES (?1, ?2, ?3, 0, 0)",
                params![list, track, database],
            )?;
            let entity = db.last_insert_rowid();
            if let Some(p) = previous {
                db.execute(
                    "UPDATE PlaylistEntity SET nextEntityId = ?1 WHERE id = ?2",
                    params![entity, p],
                )?;
            }
            previous = Some(entity);
        }
        out.playlists += 1;
    }
    Ok(out)
}

/// The sample rate at the start of Engine's `trackData` (Qt-compressed).
fn sample_rate(blob: &[u8]) -> Option<f64> {
    use std::io::Read;
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(blob.get(4..)?)
        .read_to_end(&mut raw)
        .ok()?;
    let rate = f64::from_be_bytes(raw.get(..8)?.try_into().ok()?);
    (rate.is_finite() && rate > 0.0).then_some(rate)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use djsugar_engine::beats::{BeatGrid, GridMarker};
    use djsugar_engine::{Hotcue, LibraryInfo};

    use super::*;

    fn track(path: &Path) -> Track {
        let mut t = Track {
            id: 0,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 124.0,
            key: "Am".into(),
            duration: 120.0,
            first_beat: 0.2,
            grid: Default::default(),
            source: Source::File(path.to_path_buf()),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: LibraryInfo {
                hotcues: vec![
                    Hotcue {
                        number: 0,
                        seconds: 1.5,
                        name: "Drop".into(),
                        end: None,
                        color: Some(0xCC_0000),
                    },
                    Hotcue {
                        number: 1,
                        seconds: 30.0,
                        name: String::new(),
                        end: Some(34.0),
                        color: None,
                    },
                ],
                ..Default::default()
            },
        };
        t.set_grid(BeatGrid::new(vec![GridMarker {
            seconds: 0.2,
            bpm: 124.0,
        }]));
        t
    }

    #[test]
    fn merges_into_an_existing_library() {
        let dir = std::env::temp_dir().join(format!("djs-engine-d-{}", std::process::id()));
        let lib = dir.join("Engine Library");
        std::fs::create_dir_all(lib.join("Database2")).unwrap();
        let path = lib.join("Database2").join("m.db");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(super::super::SCHEMA).unwrap();
            db.execute_batch(
                "INSERT INTO Information (id, uuid, schemaVersionMajor, schemaVersionMinor,
                   schemaVersionPatch, currentPlayedIndiciator, lastRekordBoxLibraryImportReadCounter)
                   VALUES (1, 'lib-uuid', 3, 0, 2, 1, 0);
                 INSERT INTO Track (id, path, title, bpm) VALUES (1, '../Music/a.mp3', 'A', 100);
                 INSERT INTO Track (id, path, title) VALUES (2, '../Music/keep.mp3', 'Keep');
                 UPDATE PerformanceData SET overviewWaveFormData = x'0102' WHERE trackId = 1;",
            )
            .unwrap();
        }
        let a = track(&dir.join("Music").join("a.mp3"));
        let other = track(&dir.join("Elsewhere").join("b.mp3"));
        let tracks = [a, other];
        let lists = [EngineList {
            name: "Friday".into(),
            tracks: vec![&tracks[1], &tracks[0]],
        }];
        let w = write(&path, &tracks, &lists).unwrap();
        let again = write(&path, &tracks, &lists).unwrap();
        let db = Connection::open(&path).unwrap();
        let (bpm, cues, overview): (i64, Vec<u8>, Vec<u8>) = db
            .query_row(
                "SELECT t.bpm, p.quickCues, p.overviewWaveFormData FROM Track t
                 JOIN PerformanceData p ON p.trackId = t.id WHERE t.id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        let kept: Option<Vec<u8>> = db
            .query_row(
                "SELECT quickCues FROM PerformanceData WHERE trackId = 2",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let lists: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM Playlist WHERE title = 'Friday'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let entries: Vec<(i64, String)> = db
            .prepare("SELECT trackId, databaseUuid FROM PlaylistEntity")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let backup = path.with_file_name("m.db.djsugar-backup").is_file();
        drop(db);
        std::fs::remove_dir_all(&dir).ok();

        assert!(backup);
        assert_eq!(
            w,
            Written {
                tracks: 1,
                missing: 1,
                playlists: 1
            }
        );
        assert_eq!(again, w);
        assert_eq!(bpm, 124);
        let expected = perf::build(&perf::Input {
            sample_rate: 44_100.0,
            duration: 120.0,
            grid: &tracks[0].beat_grid(),
            waveform: &tracks[0].waveform,
            cues: &tracks[0].info.hotcues,
            key: Key::parse("Am"),
        });
        assert_eq!(cues, expected.quick_cues);
        assert_eq!(overview, [1, 2]);
        assert_eq!(kept, None);
        assert_eq!(lists, 1);
        assert_eq!(entries, [(1, "lib-uuid".to_string())]);
    }
}
