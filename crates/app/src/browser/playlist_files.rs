//! Playlist files from the tree: File › Export › M3U8… on a list, folder,
//! history session, album or mixed set, and **Import Playlist File…** in
//! the Playlists + menu (M3U, M3U8, PLS and XSPF). The file reading and
//! writing is in `djsugar_library::playlist_files`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_library::playlist_files::{self as pf, OutEntry};
use djsugar_library::ListEntry;

use super::{file_path, is_browsed, Browser, Contents};
use crate::LibraryEdit;

/// A list (or folder) ready to write, with its place in the tree.
struct OutList {
    name: String,
    /// Index of its folder in the lists, None at the top.
    parent: Option<usize>,
    folder: bool,
    entries: Vec<OutEntry>,
}

/// Exports waiting for the DJ to pick a folder, and files picked to import.
pub(super) struct State {
    export_tx: Sender<(u64, PathBuf)>,
    export_rx: Receiver<(u64, PathBuf)>,
    import_tx: Sender<PathBuf>,
    import_rx: Receiver<PathBuf>,
    /// What the dialog open now is for: its number, the lists, and the
    /// DJ's list (for the note under its heading).
    waiting: Option<(u64, Vec<OutList>, Option<i64>)>,
    next: u64,
}

impl Default for State {
    fn default() -> Self {
        let (export_tx, export_rx) = mpsc::channel();
        let (import_tx, import_rx) = mpsc::channel();
        Self {
            export_tx,
            export_rx,
            import_tx,
            import_rx,
            waiting: None,
            next: 0,
        }
    }
}

impl Browser {
    pub(super) fn install_playlist_files(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_m3u(move |row| w(&|b| b.export_m3u(row as usize)));
        let w = with.clone();
        edit.on_import_playlist_file(move || w(&|b| b.pick_playlist_file()));
    }

    /// Asks where to save the tree's row `row` as M3U8.
    fn export_m3u(&mut self, row: usize) {
        let Some(&node) = self.visible_nodes.get(row) else {
            return;
        };
        let lists = self.m3u_lists(node);
        if lists.iter().all(|l| l.entries.is_empty()) {
            if let Contents::Playlist(id) = self.nodes[node].contents {
                self.export_status(id, "Nothing to export: the list is empty.".into());
            }
            return;
        }
        let list = match self.nodes[node].contents {
            Contents::Playlist(id) => Some(id),
            _ => None,
        };
        self.playlist_files.next += 1;
        let ticket = self.playlist_files.next;
        self.playlist_files.waiting = Some((ticket, lists, list));
        let tx = self.playlist_files.export_tx.clone();
        let prompt = format!("Choose where to save \"{}\"", self.node_title(node));
        crate::prefs::choose_folder(&prompt, move |dir| {
            let _ = tx.send((ticket, dir));
        });
    }

    /// The node and, for a folder, everything under it, as lists to write.
    fn m3u_lists(&self, node: usize) -> Vec<OutList> {
        let list_of = |n: usize, parent: Option<usize>| -> OutList {
            let folder = matches!(self.nodes[n].contents, Contents::Folder);
            OutList {
                name: self.node_title(n),
                parent,
                folder,
                entries: if folder {
                    Vec::new()
                } else {
                    self.m3u_entries(n)
                },
            }
        };
        let mut lists = vec![list_of(node, None)];
        if lists[0].folder {
            let top = &self.nodes[node];
            let mut open: Vec<(i32, usize)> = vec![(top.depth, 0)];
            for (n, m) in self.nodes.iter().enumerate().skip(node + 1) {
                if m.depth <= top.depth || m.history != top.history {
                    break;
                }
                while open.last().is_some_and(|&(d, _)| d >= m.depth) {
                    open.pop();
                }
                let list = list_of(n, open.last().map(|&(_, i)| i));
                if list.folder {
                    open.push((m.depth, lists.len()));
                }
                lists.push(list);
            }
        }
        lists
    }

    /// A node's tracks in order; a list's streamed tracks as comments.
    fn m3u_entries(&self, n: usize) -> Vec<OutEntry> {
        let file = |i: usize| {
            let t = &self.library[i];
            file_path(t).map(|p| OutEntry::File {
                path: p.to_path_buf(),
                artist: t.artist.clone(),
                title: t.title.clone(),
                duration: t.duration,
            })
        };
        if let (Contents::Playlist(id), Some(db)) = (&self.nodes[n].contents, &self.db) {
            if let Ok(entries) = db.playlist_entries(*id) {
                return entries
                    .into_iter()
                    .filter_map(|e| match e {
                        ListEntry::Track(id) => file(*self.index_of.get(&id)?),
                        ListEntry::Streamed(s) => Some(OutEntry::Missing {
                            artist: s.artist,
                            title: s.title,
                            service: service_name(&s.service),
                        }),
                    })
                    .collect();
            }
        }
        self.node_tracks(n).into_iter().filter_map(file).collect()
    }

    fn pick_playlist_file(&mut self) {
        let tx = self.playlist_files.import_tx.clone();
        crate::prefs::choose_file("Choose a playlist file (M3U, PLS or XSPF)", move |path| {
            let _ = tx.send(path);
        });
    }

    /// Writes exports and reads imports the dialogs have answered.
    pub fn poll_playlist_files(&mut self) {
        while let Ok((ticket, dir)) = self.playlist_files.export_rx.try_recv() {
            let Some((t, lists, list)) = self.playlist_files.waiting.take() else {
                continue;
            };
            if t != ticket {
                self.playlist_files.waiting = Some((t, lists, list));
                continue;
            }
            let note = match write_lists(&dir, &lists) {
                Ok(first) => {
                    crate::files::reveal(&first);
                    let name = first
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Saved {name}")
                }
                Err(e) => format!("Couldn't save the playlist file: {e}"),
            };
            if let Some(id) = list {
                self.export_status(id, note);
            } else {
                eprintln!("{note}");
            }
        }
        while let Ok(path) = self.playlist_files.import_rx.try_recv() {
            self.import_playlist_file(&path);
        }
    }

    /// Makes a playlist from the file at `path`: tracks the collection has
    /// are found by path, by path from the file's folder, then by file name;
    /// files found on disk that it hasn't got join it.
    pub(super) fn import_playlist_file(&mut self, path: &Path) {
        let read = match pf::read(path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("can't read {}: {e}", path.display());
                return;
            }
        };
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let mut by_name: HashMap<String, usize> = HashMap::new();
        for (i, t) in self.library.iter().enumerate() {
            if is_browsed(t) || t.info.missing {
                continue;
            }
            if let Some(name) = file_path(t).and_then(|p| p.file_name()) {
                by_name
                    .entry(name.to_string_lossy().to_lowercase())
                    .or_insert(i);
            }
        }
        let mut found: Vec<usize> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        for entry in &read.entries {
            let mut hit = None;
            for c in pf::candidates(entry, &dir) {
                if let Some(&i) = self.path_index.get(&c) {
                    hit = Some(i);
                    break;
                }
                if c.is_file() {
                    let t = djsugar_engine::scan::read_track(0, &c);
                    hit = self.add_browsed(t);
                    break;
                }
            }
            if hit.is_none() {
                hit = pf::entry_file_name(entry)
                    .and_then(|n| by_name.get(&n.to_lowercase()).copied());
            }
            match hit {
                Some(i) => found.push(i),
                None => missing.push(if entry.title.is_empty() {
                    entry.location.clone()
                } else {
                    entry.title.clone()
                }),
            }
        }
        // Files from outside the collection join it, as dropped files do.
        let browsed: Vec<usize> = found
            .iter()
            .copied()
            .filter(|&i| is_browsed(&self.library[i]))
            .collect();
        self.adopt(&browsed);
        let ids: Vec<usize> = found.iter().map(|&i| self.library[i].id).collect();
        let Some(db) = &mut self.db else { return };
        let made = db
            .create_top_playlist(&read.name)
            .and_then(|id| db.add_to_playlist(id, &ids).map(|()| id));
        let id = match made {
            Ok(id) => id,
            Err(e) => {
                eprintln!("can't make the playlist: {e}");
                return;
            }
        };
        self.reload_tree(Some(id));
        self.refresh_tree();
        let mut note = format!("Imported {} tracks", ids.len());
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().take(3).map(String::as_str).collect();
            let more = missing.len() - names.len();
            note = format!(
                "{note}. Not found ({}): {}{}",
                missing.len(),
                names.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            );
            eprintln!("Not found from {}: {}", path.display(), missing.join("; "));
        }
        self.export_status(id, note);
        self.refresh_rows();
    }
}

fn service_name(id: &str) -> String {
    match id {
        "spotify" => "Spotify",
        "tidal" => "TIDAL",
        "apple" | "apple_music" => "Apple Music",
        "soundcloud" => "SoundCloud",
        "mixcloud" => "Mixcloud",
        "beatport" => "Beatport",
        other => other,
    }
    .to_string()
}

/// Writes `lists` under `dir`: a list as `<name>.m3u8`, a folder as a
/// folder of them. Paths of tracks in a file's own folder or below are
/// written relative to it (a USB stick with the music beside the list),
/// the rest absolute. Returns the first file or folder written.
fn write_lists(dir: &Path, lists: &[OutList]) -> Result<PathBuf, String> {
    let mut places: Vec<PathBuf> = Vec::with_capacity(lists.len());
    for l in lists {
        let parent = l.parent.map_or(dir, |p| places[p].as_path());
        let place = if l.folder {
            let p = parent.join(pf::file_name(&l.name));
            std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
            p
        } else {
            let p = parent.join(format!("{}.m3u8", pf::file_name(&l.name)));
            let text = pf::write_m3u8(&l.entries, Some(parent));
            std::fs::write(&p, text).map_err(|e| e.to_string())?;
            p
        };
        places.push(place);
    }
    places
        .into_iter()
        .next()
        .ok_or_else(|| "nothing to write".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_becomes_a_folder_of_files() {
        let dir = std::env::temp_dir().join(format!("djsugar-m3u-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let track = |p: &Path| OutEntry::File {
            path: p.to_path_buf(),
            artist: "A".into(),
            title: "T".into(),
            duration: 100.0,
        };
        let lists = vec![
            OutList {
                name: "Weddings".into(),
                parent: None,
                folder: true,
                entries: vec![],
            },
            OutList {
                name: "Dinner".into(),
                parent: Some(0),
                folder: false,
                entries: vec![track(&dir.join("Weddings/music/a.mp3"))],
            },
            OutList {
                name: "Party/Late".into(),
                parent: Some(0),
                folder: false,
                entries: vec![track(Path::new("/elsewhere/b.mp3"))],
            },
        ];
        let first = write_lists(&dir, &lists).unwrap();
        assert_eq!(first, dir.join("Weddings"));
        let dinner = std::fs::read_to_string(dir.join("Weddings/Dinner.m3u8")).unwrap();
        assert!(dinner.ends_with("\nmusic/a.mp3\n"), "{dinner}");
        let late = std::fs::read_to_string(dir.join("Weddings/Party-Late.m3u8")).unwrap();
        assert!(late.ends_with("\n/elsewhere/b.mp3\n"), "{late}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
