//! Engine DJ's library on a stick, for Denon DJ (Prime, SC), Numark and
//! Rane players running Engine OS: `Engine Library/Database2/m.db`, with
//! album art beside it in `Engine Library/Artwork`.
//!
//! The audio is the same `/Contents` the Pioneer export writes, so one
//! stick serves both: Engine's paths are relative to `Engine Library`, so
//! they read `../Contents/...`.
//!
//! The schema and the way rows are filled follow EngineX-OS's
//! `pkg/enginedb` (<https://github.com/Cd1s/EngineX-OS>, MIT licence,
//! Copyright (c) 2026 EngineX OS Contributors), measured on Engine OS 5.

pub mod perf;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use base64::Engine as _;
use rusqlite::{params, Connection};
use sha1::{Digest, Sha1};

use crate::model::Model;
use crate::Error;

pub use perf::Performance;

const SCHEMA: &str = include_str!("schema.sql");
pub const LIBRARY_DIR: &str = "Engine Library";
/// Left beside the library so a later export knows the library is
/// DJ Sugar's to replace. Engine drops tables it doesn't know from `m.db`,
/// so this can't live in there.
pub const MARK: &str = "Engine Library/Database2/.djsugar-export";

/// Album art, each picture once, by its SHA-1.
#[derive(Debug, Clone, Default)]
pub struct Art {
    pub pictures: Vec<([u8; 20], Vec<u8>)>,
    by_hash: HashMap<[u8; 20], usize>,
}

impl Art {
    /// Keeps `jpeg` and returns its id in `AlbumArt` (from 1).
    pub fn add(&mut self, jpeg: Vec<u8>) -> i64 {
        let hash: [u8; 20] = Sha1::digest(&jpeg).into();
        let n = *self.by_hash.entry(hash).or_insert_with(|| {
            self.pictures.push((hash, jpeg));
            self.pictures.len() - 1
        });
        n as i64 + 1
    }
}

/// Refuses a stick whose Engine Library someone else wrote (Engine DJ
/// itself, say): replacing it would lose that library.
pub fn check(root: &Path) -> Result<(), Error> {
    let db = root.join(LIBRARY_DIR).join("Database2/m.db");
    if db.exists() && !root.join(MARK).exists() {
        return Err(Error::Other(
            "the stick already has an Engine DJ library that DJ Sugar didn't write; \
             use another stick, or move its \"Engine Library\" folder off it first"
                .into(),
        ));
    }
    Ok(())
}

/// Engine's path for a stick path (`/Contents/...`).
fn engine_path(stick_path: &str) -> String {
    format!("../{}", stick_path.trim_start_matches('/'))
}

fn file_name(stick_path: &str) -> &str {
    stick_path.rsplit('/').next().unwrap_or(stick_path)
}

fn extension(stick_path: &str) -> String {
    let name = file_name(stick_path);
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// A random version 4 UUID, as Engine names its libraries.
fn uuid() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("random bytes");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// Builds `m.db` at `path` (which mustn't exist yet).
pub fn write_db(path: &Path, model: &Model) -> Result<(), Error> {
    let sql = |e: rusqlite::Error| Error::Other(format!("m.db: {e}"));
    let mut db = Connection::open(path).map_err(sql)?;
    db.execute_batch("PRAGMA journal_mode = DELETE; PRAGMA foreign_keys = ON;")
        .map_err(sql)?;
    let tx = db.transaction().map_err(sql)?;
    tx.execute_batch(SCHEMA).map_err(sql)?;
    let library = uuid();
    // A positive random 63-bit number, as Engine starts it.
    let mut r = [0u8; 8];
    getrandom::fill(&mut r).expect("random bytes");
    let played = (i64::from_be_bytes(r) >> 1).abs() | 1;
    tx.execute(
        "INSERT INTO Information(id, uuid, schemaVersionMajor, schemaVersionMinor, \
         schemaVersionPatch, currentPlayedIndiciator, lastRekordBoxLibraryImportReadCounter) \
         VALUES (1, ?1, 3, 0, 2, ?2, 0)",
        params![library, played],
    )
    .map_err(sql)?;

    for (i, (hash, jpeg)) in model.art.pictures.iter().enumerate() {
        tx.execute(
            "INSERT INTO AlbumArt(id, hash, albumArt) VALUES (?1, ?2, ?3)",
            params![i as i64 + 1, &hash[..], jpeg],
        )
        .map_err(sql)?;
    }

    for (i, t) in model.tracks.iter().enumerate() {
        let id = i as i64 + 1;
        let perf = t.engine.clone().unwrap_or_default();
        let bpm = t.bpmx100 as f64 / 100.0;
        let added = t.added_unix;
        tx.execute(
            "INSERT INTO Track(id, length, bpm, year, path, filename, bitrate, bpmAnalyzed, \
             albumArtId, fileBytes, title, artist, album, genre, comment, label, composer, \
             remixer, key, rating, fileType, isAnalyzed, isAvailable, isMetadataImported, \
             isBeatGridLocked, isPlayed, explicitLyrics, isMetadataOfPackedTrackChanged, \
             isPerfomanceDataOfPackedTrackChanged, dateCreated, dateAdded, lastEditTime) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
             ?17, ?18, ?19, ?20, ?21, ?22, 1, 1, ?23, 0, 0, 0, 0, ?24, ?24, strftime('%s'))",
            params![
                id,
                t.length,
                bpm.round() as i64,
                (t.year > 0).then_some(t.year),
                engine_path(&t.path),
                file_name(&t.path),
                t.bitrate,
                bpm,
                (t.art_id > 0).then_some(t.art_id),
                t.file_size,
                t.title,
                t.artist,
                t.album,
                t.genre,
                t.comment,
                t.label,
                t.composer,
                t.remixer,
                t.engine_key,
                t.rating * 20,
                extension(&t.path),
                perf.complete,
                t.grid_locked,
                added,
            ],
        )
        .map_err(sql)?;
        // Inserting the track made its PerformanceData row.
        tx.execute(
            "UPDATE PerformanceData SET trackData = ?1, overviewWaveFormData = ?2, \
             beatData = ?3, quickCues = ?4, loops = ?5, thirdPartySourceId = 1, \
             activeOnLoadLoops = 0 WHERE trackId = ?6",
            params![
                perf.track_data,
                perf.overview,
                perf.beats,
                perf.quick_cues,
                perf.loops,
                id
            ],
        )
        .map_err(sql)?;
    }

    // Lists in the order given; inserting each with nextListId 0 puts it
    // after the others in its folder (Engine's triggers chain them).
    let mut ids: Vec<i64> = Vec::with_capacity(model.lists.len());
    let mut taken: HashMap<(i64, String), ()> = HashMap::new();
    for list in &model.lists {
        let parent = list.parent.and_then(|p| ids.get(p).copied()).unwrap_or(0);
        // Names are unique within a folder.
        let mut title = list.name.clone();
        let mut n = 2;
        while taken.contains_key(&(parent, title.clone())) {
            title = format!("{} ({n})", list.name);
            n += 1;
        }
        taken.insert((parent, title.clone()), ());
        tx.execute(
            "INSERT INTO Playlist(title, parentListId, isPersisted, nextListId, \
             lastEditTime, isExplicitlyExported) VALUES (?1, ?2, 1, 0, strftime('%s'), 1)",
            params![title, parent],
        )
        .map_err(sql)?;
        let list_id = tx.last_insert_rowid();
        ids.push(list_id);
        let mut previous: Option<i64> = None;
        for &track in &list.tracks {
            tx.execute(
                "INSERT INTO PlaylistEntity(listId, trackId, databaseUuid, nextEntityId, \
                 membershipReference) VALUES (?1, ?2, ?3, 0, 0)",
                params![list_id, track, library],
            )
            .map_err(sql)?;
            let entity = tx.last_insert_rowid();
            if let Some(p) = previous {
                tx.execute(
                    "UPDATE PlaylistEntity SET nextEntityId = ?1 WHERE id = ?2",
                    params![entity, p],
                )
                .map_err(sql)?;
            }
            previous = Some(entity);
        }
    }
    tx.commit().map_err(sql)?;
    db.close().map_err(|(_, e)| sql(e))?;
    Ok(())
}

/// Builds the library here, then puts it and its album art on the stick
/// in place of the library an earlier export left.
pub fn write(root: &Path, model: &Model) -> Result<(), Error> {
    let work = tempfile::tempdir()?;
    let built = work.path().join("m.db");
    write_db(&built, model)?;

    let library = root.join(LIBRARY_DIR);
    let art = library.join("Artwork");
    fs::create_dir_all(&art)?;
    for (hash, jpeg) in &model.art.pictures {
        // Engine Link serves covers to other decks from these files.
        let name = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);
        let path = art.join(format!("{name}.jpg"));
        if !path.exists() {
            fs::write(&path, jpeg)?;
        }
    }
    let dir = library.join("Database2");
    fs::create_dir_all(&dir)?;
    for name in ["m.db", "m.db-journal", "m.db-wal", "m.db-shm"] {
        let path = dir.join(name);
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }
    fs::write(
        root.join(MARK),
        "This Engine Library was written by DJ Sugar.\n",
    )?;
    fs::copy(&built, dir.join("m.db"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_relative_to_the_engine_library() {
        assert_eq!(engine_path("/Contents/A/B/x.mp3"), "../Contents/A/B/x.mp3");
        assert_eq!(file_name("/Contents/A/B/x.MP3"), "x.MP3");
        assert_eq!(extension("/Contents/A/B/x.MP3"), "mp3");
    }

    #[test]
    fn a_uuid_looks_like_one() {
        let u = uuid();
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "4");
    }

    #[test]
    fn art_is_kept_once() {
        let mut art = Art::default();
        assert_eq!(art.add(vec![1, 2, 3]), 1);
        assert_eq!(art.add(vec![4]), 2);
        assert_eq!(art.add(vec![1, 2, 3]), 1);
        assert_eq!(art.pictures.len(), 2);
    }

    #[test]
    fn someone_elses_library_is_left_alone() {
        let stick = tempfile::tempdir().unwrap();
        assert!(check(stick.path()).is_ok());
        let db = stick.path().join("Engine Library/Database2");
        fs::create_dir_all(&db).unwrap();
        fs::write(db.join("m.db"), b"x").unwrap();
        assert!(check(stick.path()).is_err());
        fs::write(stick.path().join(MARK), b"").unwrap();
        assert!(check(stick.path()).is_ok());
    }
}
