//! Stage changes and the file moves that go with them (see [`crate::stages`]).
//!
//! A stage change is saved at once; its file move is queued with it, and
//! made when the DJ has file moves on ([`Library::apply_moves`]). Each
//! change is logged, so the last batch can be taken back.

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use crate::stages::{Folders, Stage};
use crate::{now, path_text, row_id, track_id, Error, Library};

/// A logged stage change and the file move that goes with it (`from` and
/// `to` are the same when the file stays).
#[derive(Clone, Debug, PartialEq)]
pub struct FileMove {
    pub id: i64,
    pub batch: i64,
    pub track: usize,
    pub from: PathBuf,
    pub to: PathBuf,
    pub from_stage: String,
    pub to_stage: String,
}

/// What [`Library::apply_moves`] or [`Library::undo_batch`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoveReport {
    pub moved: usize,
    /// Files that couldn't be moved, and why. Their moves stay queued.
    pub failed: Vec<(PathBuf, String)>,
}

/// A stage change asked for: the track, and where its file goes (`None`
/// when it stays where it is).
pub struct Change {
    pub track: usize,
    pub to: Option<PathBuf>,
}

impl Library {
    /// Gives tracks without a stage the one of the folder they are in.
    /// Returns how many got one.
    pub fn assign_stages(&mut self, folders: &Folders) -> Result<usize, Error> {
        if !folders.is_set() {
            return Ok(0);
        }
        let tx = self.db.transaction()?;
        let mut count = 0;
        {
            let mut q = tx.prepare("SELECT id, path FROM tracks WHERE stage = ''")?;
            let rows: Vec<(i64, String)> = q
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
            let mut set = tx.prepare("UPDATE tracks SET stage = ?2 WHERE id = ?1")?;
            for (id, path) in rows {
                if let Some(stage) = folders.stage_of(Path::new(&path)) {
                    set.execute(params![id, stage.name()])?;
                    count += 1;
                }
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Puts a track that has no stage yet in the Inbox: a download, which
    /// lands outside the stage folders.
    pub fn put_in_inbox(&mut self, track: usize) -> Result<(), Error> {
        self.db.execute(
            "UPDATE tracks SET stage = ?2 WHERE id = ?1 AND stage = ''",
            params![row_id(track), Stage::Inbox.name()],
        )?;
        Ok(())
    }

    /// Moves tracks to `stage`, as one batch that [`Library::undo_batch`]
    /// can take back. Their files' moves are queued; a move still queued
    /// from an earlier change of the same track is replaced. Returns the
    /// batch.
    pub fn change_stage(&mut self, changes: &[Change], stage: Stage) -> Result<i64, Error> {
        let tx = self.db.transaction()?;
        let batch: i64 = tx.query_row(
            "SELECT COALESCE(MAX(batch), 0) + 1 FROM file_moves",
            [],
            |r| r.get(0),
        )?;
        {
            let mut find = tx.prepare("SELECT path, stage FROM tracks WHERE id = ?1")?;
            let mut drop_queued = tx.prepare(
                "DELETE FROM file_moves WHERE track_id = ?1 AND done IS NULL AND undone IS NULL",
            )?;
            let mut set = tx.prepare("UPDATE tracks SET stage = ?2 WHERE id = ?1")?;
            let mut log = tx.prepare(
                "INSERT INTO file_moves (batch, track_id, from_path, to_path, from_stage, to_stage, queued)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for c in changes {
                let Some(row) = row_id(c.track) else { continue };
                let Some((path, old)): Option<(String, String)> = find
                    .query_row([row], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?
                else {
                    continue;
                };
                drop_queued.execute([row])?;
                set.execute(params![row, stage.name()])?;
                let to = c.to.as_deref().map_or_else(|| path.clone(), path_text);
                log.execute(params![batch, row, path, to, old, stage.name(), now()])?;
            }
        }
        tx.commit()?;
        Ok(batch)
    }

    /// The file moves waiting to be made, oldest first.
    pub fn pending_moves(&self) -> Result<Vec<FileMove>, Error> {
        self.moves("done IS NULL AND undone IS NULL AND from_path != to_path ORDER BY id")
    }

    fn moves(&self, filter: &str) -> Result<Vec<FileMove>, Error> {
        let mut q = self.db.prepare(&format!(
            "SELECT id, batch, track_id, from_path, to_path, from_stage, to_stage
             FROM file_moves WHERE {filter}"
        ))?;
        let rows = q.query_map([], |r| {
            Ok(FileMove {
                id: r.get(0)?,
                batch: r.get(1)?,
                track: track_id(r.get(2)?),
                from: PathBuf::from(r.get::<_, String>(3)?),
                to: PathBuf::from(r.get::<_, String>(4)?),
                from_stage: r.get(5)?,
                to_stage: r.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// Makes the queued file moves, keeping each track's analysis with it.
    /// A file already at its new place (moved by hand) just has its move
    /// marked done; one that can't move keeps its move queued.
    pub fn apply_moves(&mut self) -> Result<MoveReport, Error> {
        self.apply(None)
    }

    /// [`Library::apply_moves`] for one batch only, whether or not the DJ
    /// has file moves on: for a move asked for there and then (Move to
    /// Albums), leaving other queued moves waiting.
    pub fn apply_batch(&mut self, batch: i64) -> Result<MoveReport, Error> {
        self.apply(Some(batch))
    }

    fn apply(&mut self, batch: Option<i64>) -> Result<MoveReport, Error> {
        let mut report = MoveReport::default();
        // Stage changes whose files stay need nothing more.
        self.db.execute(
            "UPDATE file_moves SET done = ?1
             WHERE done IS NULL AND undone IS NULL AND from_path = to_path
             AND (?2 IS NULL OR batch = ?2)",
            params![now(), batch],
        )?;
        let pending = self.pending_moves()?;
        for m in pending
            .into_iter()
            .filter(|m| batch.is_none_or(|b| m.batch == b))
        {
            match self.move_track_file(m.track, &m.from, &m.to) {
                Ok(()) => {
                    self.db.execute(
                        "UPDATE file_moves SET done = ?2 WHERE id = ?1",
                        params![m.id, now()],
                    )?;
                    report.moved += 1;
                }
                Err(e) => report.failed.push((m.from.clone(), e.to_string())),
            }
        }
        Ok(report)
    }

    /// Moves a track's file from `from` to `to` and its library entry with
    /// it. A file already moved by hand only has its entry follow.
    fn move_track_file(&self, track: usize, from: &Path, to: &Path) -> io::Result<()> {
        let taken: Option<i64> = self
            .db
            .query_row(
                "SELECT id FROM tracks WHERE path = ?1",
                [path_text(to)],
                |r| r.get(0),
            )
            .optional()
            .map_err(io::Error::other)?;
        if taken.is_some_and(|id| Some(id) != row_id(track)) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "another track in your collection is there",
            ));
        }
        if from.exists() || !to.exists() {
            move_file(from, to)?;
        }
        self.db
            .execute(
                "UPDATE tracks SET path = ?2 WHERE id = ?1",
                params![row_id(track), path_text(to)],
            )
            .map_err(io::Error::other)?;
        Ok(())
    }

    /// The newest batch of stage changes not yet taken back, and how many
    /// tracks it changed.
    pub fn last_batch(&self) -> Result<Option<(i64, usize)>, Error> {
        self.db
            .query_row(
                "SELECT batch, COUNT(*) FROM file_moves WHERE undone IS NULL
                 GROUP BY batch ORDER BY batch DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize)),
            )
            .optional()
    }

    /// Takes back a batch of stage changes: each track gets its old stage,
    /// and files that moved go back where they were.
    pub fn undo_batch(&mut self, batch: i64) -> Result<MoveReport, Error> {
        let mut report = MoveReport::default();
        let rows = self.moves(&format!(
            "batch = {batch} AND undone IS NULL ORDER BY id DESC"
        ))?;
        for m in rows {
            let done: Option<i64> =
                self.db
                    .query_row("SELECT done FROM file_moves WHERE id = ?1", [m.id], |r| {
                        r.get(0)
                    })?;
            if done.is_some() && m.from != m.to {
                if let Err(e) = self.move_track_file(m.track, &m.to, &m.from) {
                    report.failed.push((m.to.clone(), e.to_string()));
                    continue;
                }
                report.moved += 1;
            }
            self.db.execute(
                "UPDATE tracks SET stage = ?2 WHERE id = ?1",
                params![row_id(m.track), m.from_stage],
            )?;
            self.db.execute(
                "UPDATE file_moves SET undone = ?2 WHERE id = ?1",
                params![m.id, now()],
            )?;
        }
        Ok(report)
    }
}

/// What an album's folder keeps beside its tracks: cover pictures, a cue
/// sheet, notes from the ripper.
const ALBUM_EXTRAS: [&str; 13] = [
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "cue", "nfo", "log", "m3u", "m3u8", "txt", "sfv",
];

/// Moves what is left in `from` once its tracks have gone (cover
/// pictures, a .cue or .nfo, a log) into `to`, then removes `from` if
/// that empties it. Other files, files `to` already has, and subfolders
/// stay. Returns the files moved.
pub fn move_leftovers(from: &Path, to: &Path) -> io::Result<Vec<PathBuf>> {
    let mut moved = Vec::new();
    for e in std::fs::read_dir(from)?.flatten() {
        let path = e.path();
        let name = e.file_name();
        let hidden = name.to_string_lossy().starts_with('.');
        let extra = path
            .extension()
            .is_some_and(|x| ALBUM_EXTRAS.contains(&x.to_string_lossy().to_lowercase().as_str()));
        if hidden || !extra || !e.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let dest = to.join(&name);
        if dest.exists() {
            continue;
        }
        std::fs::create_dir_all(to)?;
        if std::fs::rename(&path, &dest).is_err() {
            std::fs::copy(&path, &dest)?;
            std::fs::remove_file(&path)?;
        }
        moved.push(dest);
    }
    // Only an empty folder goes, so nothing the DJ kept there is lost;
    // a .DS_Store left behind is no reason to keep it.
    let left: Vec<_> = std::fs::read_dir(from)?.flatten().collect();
    if left.iter().all(|e| e.file_name() == ".DS_Store") {
        for e in left {
            let _ = std::fs::remove_file(e.path());
        }
        let _ = std::fs::remove_dir(from);
    }
    Ok(moved)
}

/// Moves a file, into a new folder if needed, and its cached analysis and
/// stems with it. Never replaces a file that is already at `to`.
fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a file is already there",
        ));
    }
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let cached = djsugar_engine::cache::before_move(from);
    if std::fs::rename(from, to).is_err() {
        // Another disk: copy, then remove the original once the copy is whole.
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)?;
    }
    djsugar_engine::cache::file_moved(to, cached);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::file_track;

    fn library_with(paths: &[&Path]) -> (Library, Vec<usize>) {
        let mut lib = Library::in_memory().unwrap();
        let tracks: Vec<_> = paths
            .iter()
            .map(|p| file_track(&p.to_string_lossy(), "T"))
            .collect();
        lib.sync(&tracks).unwrap();
        let ids = lib
            .all_tracks(|_| true)
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect();
        (lib, ids)
    }

    fn stages(lib: &Library) -> Vec<(String, PathBuf)> {
        lib.all_tracks(|_| true)
            .unwrap()
            .into_iter()
            .map(|t| match t.source {
                djsugar_engine::Source::File(p) => (t.info.stage, p),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn stages_come_from_the_folders() {
        let (mut lib, _) = library_with(&[
            Path::new("/u/Downloads/a.mp3"),
            Path::new("/u/DB/_Staging/Yoga STG/b.mp3"),
            Path::new("/u/DB/Library/c.mp3"),
            Path::new("/u/Books/d.m4b"),
        ]);
        let folders = Folders {
            inbox: vec!["/u/Downloads".into()],
            triage: Some("/u/DB/_Staging".into()),
            collection: Some("/u/DB/Library".into()),
            albums: None,
        };
        assert_eq!(lib.assign_stages(&folders).unwrap(), 3);
        let s: Vec<String> = stages(&lib).into_iter().map(|(s, _)| s).collect();
        assert_eq!(s, ["inbox", "triage", "collection", ""]);
        // A stage the DJ set isn't changed by where the file is.
        assert_eq!(lib.assign_stages(&folders).unwrap(), 0);
    }

    #[test]
    fn tracks_once_hidden_from_the_inbox_come_back_to_it() {
        let dir = std::env::temp_dir().join(format!("djsugar-hidden-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.db");
        {
            let mut lib = Library::open(&path).unwrap();
            lib.sync(&[file_track("/u/Downloads/a.mp3", "A")]).unwrap();
            lib.db
                .execute_batch("UPDATE tracks SET stage = 'dismissed'")
                .unwrap();
        }
        let lib = Library::open(&path).unwrap();
        let s: Vec<String> = stages(&lib).into_iter().map(|(s, _)| s).collect();
        assert_eq!(s, ["inbox"]);
        drop(lib);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn moves_wait_until_applied_and_can_be_undone() {
        let dir = std::env::temp_dir().join(format!("djsugar-moves-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let inbox = dir.join("Downloads");
        std::fs::create_dir_all(&inbox).unwrap();
        let a = inbox.join("01_a.mp3");
        std::fs::write(&a, b"audio").unwrap();
        let (mut lib, ids) = library_with(&[&a]);
        let to = dir.join("_Staging/Yoga STG/Artist - A.mp3");
        let batch = lib
            .change_stage(
                &[Change {
                    track: ids[0],
                    to: Some(to.clone()),
                }],
                Stage::Triage,
            )
            .unwrap();
        // The stage changes at once; the file waits.
        assert_eq!(stages(&lib), [("triage".to_string(), a.clone())]);
        assert!(a.exists());
        assert_eq!(lib.pending_moves().unwrap().len(), 1);

        let report = lib.apply_moves().unwrap();
        assert_eq!((report.moved, report.failed.len()), (1, 0));
        assert!(!a.exists() && to.exists());
        assert_eq!(stages(&lib), [("triage".to_string(), to.clone())]);
        assert!(lib.pending_moves().unwrap().is_empty());

        assert_eq!(lib.last_batch().unwrap(), Some((batch, 1)));
        let undone = lib.undo_batch(batch).unwrap();
        assert_eq!(undone.moved, 1);
        assert!(a.exists() && !to.exists());
        assert_eq!(stages(&lib), [(String::new(), a.clone())]);
        assert_eq!(lib.last_batch().unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_batch_applies_alone_and_takes_the_folder_along() {
        let dir = std::env::temp_dir().join(format!("djsugar-album-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let album = dir.join("Downloaded/Soulseek/Air/Moon Safari");
        std::fs::create_dir_all(&album).unwrap();
        let a = album.join("01 La femme d'argent.mp3");
        let b = album.join("02 Sexy boy.mp3");
        let other = dir.join("Downloads/c.mp3");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        for f in [&a, &b, &other] {
            std::fs::write(f, b"audio").unwrap();
        }
        std::fs::write(album.join("cover.jpg"), b"jpeg").unwrap();
        std::fs::write(album.join(".DS_Store"), b"").unwrap();
        let (mut lib, ids) = library_with(&[&a, &b, &other]);
        lib.put_in_inbox(ids[0]).unwrap();
        assert_eq!(stages(&lib)[0].0, "inbox");
        // A move queued earlier, waiting for file moves to be turned on.
        lib.change_stage(
            &[Change {
                track: ids[2],
                to: Some(dir.join("T/c.mp3")),
            }],
            Stage::Triage,
        )
        .unwrap();
        let to = dir.join("Albums/Air/Moon Safari");
        let changes: Vec<Change> = [(ids[0], &a), (ids[1], &b)]
            .into_iter()
            .map(|(id, p)| Change {
                track: id,
                to: Some(to.join(p.file_name().unwrap())),
            })
            .collect();
        let batch = lib.change_stage(&changes, Stage::Collection).unwrap();
        let report = lib.apply_batch(batch).unwrap();
        assert_eq!((report.moved, report.failed.len()), (2, 0));
        assert!(other.exists(), "the other queued move still waits");
        assert_eq!(lib.pending_moves().unwrap().len(), 1);
        let left = move_leftovers(&album, &to).unwrap();
        assert_eq!(left, [to.join("cover.jpg")]);
        assert!(!album.exists());
        assert!(to.join("02 Sexy boy.mp3").exists());
        // Undo brings the tracks back to the Inbox.
        lib.undo_batch(batch).unwrap();
        assert!(a.exists() && b.exists());
        assert_eq!(stages(&lib)[0], ("inbox".to_string(), a.clone()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_change_replaces_a_queued_move() {
        let (mut lib, ids) = library_with(&[Path::new("/u/Downloads/a.mp3")]);
        let change = |to: &str| {
            [Change {
                track: ids[0],
                to: Some(PathBuf::from(to)),
            }]
        };
        lib.change_stage(&change("/u/T/a.mp3"), Stage::Triage)
            .unwrap();
        lib.change_stage(&change("/u/C/a.mp3"), Stage::Collection)
            .unwrap();
        let pending = lib.pending_moves().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].to, PathBuf::from("/u/C/a.mp3"));
        assert_eq!(pending[0].from, PathBuf::from("/u/Downloads/a.mp3"));
        // A file that can't be found keeps its move queued.
        let report = lib.apply_moves().unwrap();
        assert_eq!((report.moved, report.failed.len()), (0, 1));
        assert_eq!(lib.pending_moves().unwrap().len(), 1);
    }
}
