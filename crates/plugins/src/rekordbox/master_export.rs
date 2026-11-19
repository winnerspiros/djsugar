//! Puts lists into rekordbox 6 and 7's own library, `master.db`, as
//! rekordbox playlists in their folders (a playlist of the same name in
//! the same folder is refilled). Only tracks rekordbox already has can go
//! in; the rest are counted. Cues, loops and grids go to rekordbox through
//! the rekordbox XML export, which rekordbox imports itself.
//!
//! The writes go through rbox, which keeps rekordbox's change counters,
//! and it refuses while rekordbox is running. The old file is kept as
//! `master.db.djsugar-backup`.

use std::collections::HashMap;
use std::path::Path;

use djsugar_engine::{Source, Track};
use rbox_masterdb::{MasterDb, PlaylistType};

use super::dates;

/// What the export did.
#[derive(Debug, Default, PartialEq)]
pub struct Written {
    pub playlists: usize,
    pub entries: usize,
    /// Tracks rekordbox doesn't have yet.
    pub missing: usize,
}

/// A list to write: the folders it's in, its name and its tracks in order.
pub struct RekordboxList<'a> {
    pub folders: Vec<String>,
    pub name: String,
    pub tracks: Vec<&'a Track>,
}

/// rekordbox writes `/` on every system; Windows and macOS don't mind
/// letters' case.
fn key(path: &str) -> String {
    let p = path.replace('\\', "/");
    if cfg!(any(windows, target_os = "macos")) {
        p.to_lowercase()
    } else {
        p
    }
}

pub fn write(database: &Path, lists: &[RekordboxList]) -> Result<Written, String> {
    let fail = |e: &dyn std::fmt::Display| format!("rekordbox's library: {e}");
    if !database.is_file() {
        return Err(fail(&"master.db not found"));
    }
    if rbox_core::is_rekordbox_running() {
        return Err("close rekordbox first".into());
    }
    if dates::needs_fixing(database).unwrap_or(false) {
        return Err(fail(
            &"it has dates in a form we can only read, so it isn't changed; use the rekordbox XML export",
        ));
    }
    let backup = database.with_file_name("master.db.djsugar-backup");
    std::fs::copy(database, &backup).map_err(|e| fail(&e))?;
    let mut db = MasterDb::new(database).map_err(|e| fail(&e))?;
    let mut ids: HashMap<String, String> = HashMap::new();
    for c in db.get_contents().map_err(|e| fail(&e))? {
        if c.rb_local_deleted != 0 {
            continue;
        }
        if let Some(p) = &c.folder_path {
            ids.insert(key(p), c.id.clone());
        }
    }
    let mut out = Written::default();
    for l in lists {
        let mut parent: Option<String> = None;
        let mut path: Vec<&str> = Vec::new();
        for f in &l.folders {
            path.push(f);
            let found = db
                .get_playlist_by_path(path.clone())
                .map_err(|e| fail(&e))?;
            parent = Some(match found {
                Some(p) if p.attribute == PlaylistType::Folder => p.id,
                Some(_) => return Err(fail(&format!("\"{f}\" is a playlist, not a folder"))),
                None => {
                    db.create_playlist_folder(f.clone(), parent.clone(), None)
                        .map_err(|e| fail(&e))?
                        .id
                }
            });
        }
        path.push(&l.name);
        let list = match db.get_playlist_by_path(path).map_err(|e| fail(&e))? {
            Some(p) if p.attribute == PlaylistType::List => {
                let old: Vec<String> = db
                    .get_playlist_songs(&p.id)
                    .map_err(|e| fail(&e))?
                    .into_iter()
                    .map(|s| s.id)
                    .collect();
                if !old.is_empty() {
                    db.delete_playlist_songs(old.iter().map(String::as_str).collect())
                        .map_err(|e| fail(&e))?;
                }
                p.id
            }
            Some(_) => {
                return Err(fail(&format!(
                    "\"{}\" is a folder or an intelligent playlist there",
                    l.name
                )))
            }
            None => {
                db.create_playlist(l.name.clone(), parent, None, None, None)
                    .map_err(|e| fail(&e))?
                    .id
            }
        };
        for t in &l.tracks {
            let found = match &t.source {
                Source::File(p) => ids.get(&key(&p.to_string_lossy())),
                Source::Synth { .. } => None,
            };
            let Some(content) = found else {
                out.missing += 1;
                continue;
            };
            db.create_playlist_song(&list, content, None)
                .map_err(|e| fail(&e))?;
            out.entries += 1;
        }
        out.playlists += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;

    fn track(path: &str) -> Track {
        Track {
            id: 0,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 120.0,
            key: String::new(),
            duration: 60.0,
            first_beat: 0.0,
            grid: Default::default(),
            source: Source::File(PathBuf::from(path)),
            waveform: Arc::default(),
            audio: None,
            cover: None,
            info: Default::default(),
        }
    }

    /// Into a copy of rbox's demo library (`RBOX_TESTDATA`), then read back
    /// by our importer.
    #[test]
    fn writes_playlists_into_the_demo_library() {
        let Some(root) = std::env::var_os("RBOX_TESTDATA") else {
            eprintln!("RBOX_TESTDATA not set; skipping");
            return;
        };
        for version in [6, 7] {
            let demo = PathBuf::from(&root).join(format!("RBv{version}/demo/rekordbox"));
            let dir =
                std::env::temp_dir().join(format!("djs-rb-x-{version}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            for f in ["master.db", "masterPlaylists6.xml"] {
                std::fs::copy(demo.join(f), dir.join(f)).unwrap();
            }
            let path = dir.join("master.db");
            let have: Vec<String> = MasterDb::new(&path)
                .unwrap()
                .get_contents()
                .unwrap()
                .into_iter()
                .filter_map(|c| c.folder_path)
                .take(2)
                .collect();
            assert_eq!(have.len(), 2);
            let tracks = [track(&have[1]), track("/nowhere/x.mp3"), track(&have[0])];
            let lists = [RekordboxList {
                folders: vec!["DJ Sugar".into()],
                name: "Friday".into(),
                tracks: tracks.iter().collect(),
            }];
            let w = write(&path, &lists).unwrap();
            let again = write(&path, &lists).unwrap();
            let back =
                super::super::masterdb::read(&path, &[super::super::PLAYLISTS], |_| true).unwrap();
            let backup = dir.join("master.db.djsugar-backup").is_file();
            std::fs::remove_dir_all(&dir).ok();

            assert!(backup);
            let expect = Written {
                playlists: 1,
                entries: 2,
                missing: 1,
            };
            assert_eq!(w, expect);
            assert_eq!(again, expect);
            let friday: Vec<_> = back
                .lists
                .iter()
                .filter(|l| l.name == "Friday" && l.folders == ["DJ Sugar"])
                .collect();
            assert_eq!(friday.len(), 1, "rekordbox {version}");
            let order: Vec<String> = friday[0]
                .entries
                .iter()
                .map(|e| key(&e.path.to_string_lossy()))
                .collect();
            assert_eq!(order, [key(&have[1]), key(&have[0])], "rekordbox {version}");
        }
    }
}
