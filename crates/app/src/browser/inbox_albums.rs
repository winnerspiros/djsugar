//! Albums in the Inbox: a download of a whole album (a folder of tracks
//! that share one album tag) shows under Inbox as one item, and Move to
//! Albums takes it into the albums folder as `Artist/Album` in one go,
//! straight to Play Ready. The files move there and then, whether or not
//! the DJ has file moves on: a fresh download isn't in another app yet.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use djsugar_engine::Source;
use djsugar_library::query::Indexed;
use djsugar_library::stages::{self, Stage};
use djsugar_library::Change;
use serde_json::json;

use super::albums::{album_title, most};
use super::{file_path, is_browsed, Browser, Contents, Node, Undo, KIND_ALBUM};
use crate::{settings, LibraryEdit};

/// The Albums folder the DJ picks when none is set yet, and the album
/// that was waiting for it.
pub(super) struct InboxAlbums {
    tx: Sender<PathBuf>,
    rx: Receiver<PathBuf>,
    waiting: Option<PathBuf>,
}

impl InboxAlbums {
    pub(super) fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        InboxAlbums {
            tx,
            rx,
            waiting: None,
        }
    }
}

/// An album in the Inbox: its folder, name and artist, and its tracks
/// (indices into the library) in album order.
pub(super) struct InboxAlbum {
    pub dir: PathBuf,
    pub name: String,
    pub artist: String,
    pub tracks: Vec<usize>,
}

impl InboxAlbum {
    /// What the tree calls it.
    fn title(&self) -> String {
        format!("{} - {}", self.artist, self.name)
    }
}

/// Whether the tracks of a folder, by their album tags, are one album: at
/// least two, every album tag the same (ignoring case), and most of them
/// tagged. A Downloads folder with tracks from several albums isn't one.
fn one_album(albums: &[&str]) -> Option<String> {
    if albums.len() < 2 {
        return None;
    }
    let tagged: HashSet<String> = albums
        .iter()
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
        .collect();
    if tagged.len() != 1 {
        return None;
    }
    most(albums)
}

impl Browser {
    pub(super) fn install_inbox_albums(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_move_node_album(move |row| {
            w(&|b| {
                let dir = b.visible_nodes.get(row.max(0) as usize).and_then(|&n| {
                    match &b.nodes[n].contents {
                        Contents::InboxAlbum(dir) => Some(dir.clone()),
                        _ => None,
                    }
                });
                if let Some(dir) = dir {
                    b.move_to_albums(&dir);
                }
            })
        });
        let w = with.clone();
        edit.on_in_inbox_album(move |row| {
            let found = std::cell::Cell::new(false);
            w(&|b| found.set(b.inbox_album_of_row(row).is_some()));
            found.get()
        });
        let w = with.clone();
        edit.on_move_row_album(move |row| {
            w(&|b| {
                if let Some(dir) = b.inbox_album_of_row(row) {
                    b.move_to_albums(&dir);
                }
            })
        });
    }

    /// The tracks in the Inbox: those at the Inbox stage, and downloads in
    /// the Inbox list that have no stage.
    fn inbox_tracks(&self) -> Vec<usize> {
        let listed: HashSet<usize> = self
            .db
            .as_ref()
            .and_then(|db| db.playlist_tracks(db.inbox_list_id()?).ok())
            .unwrap_or_default()
            .into_iter()
            .collect();
        (0..self.library.len())
            .filter(|&i| {
                let t = &self.library[i];
                !is_browsed(t)
                    && !t.info.missing
                    && (t.info.stage == Stage::Inbox.name()
                        || (t.info.stage.is_empty() && listed.contains(&t.id)))
            })
            .collect()
    }

    /// The albums in the Inbox, by artist then name. Folders the DJ set
    /// as an inbox, and Downloads itself, are never one: their files come
    /// from all over.
    pub(super) fn inbox_albums(&self) -> Vec<InboxAlbum> {
        let mut by_dir: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
        for i in self.inbox_tracks() {
            if let Some(dir) = file_path(&self.library[i]).and_then(Path::parent) {
                by_dir.entry(dir.to_path_buf()).or_default().push(i);
            }
        }
        let downloads = djsugar_engine::paths::home_dir().map(|h| h.join("Downloads"));
        let mut found: Vec<InboxAlbum> = by_dir
            .into_iter()
            .filter(|(dir, _)| !self.folders.inbox.contains(dir) && downloads.as_ref() != Some(dir))
            .filter_map(|(dir, mut tracks)| {
                let albums: Vec<&str> = tracks
                    .iter()
                    .map(|&i| self.library[i].album.as_str())
                    .collect();
                one_album(&albums)?;
                let artists: Vec<&str> = tracks
                    .iter()
                    .map(|&i| self.library[i].artist.as_str())
                    .collect();
                let root = dir.parent().and_then(Path::parent).unwrap_or(&dir);
                let (name, artist) = album_title(&dir, root, &albums, &artists);
                tracks.sort_by_cached_key(|&i| {
                    let t = &self.library[i];
                    let number = match t.info.track_number {
                        0 => u32::MAX,
                        n => n,
                    };
                    let file = file_path(t)
                        .and_then(Path::file_name)
                        .map(|n| n.to_string_lossy().to_lowercase());
                    (number, file)
                });
                Some(InboxAlbum {
                    dir,
                    name,
                    artist,
                    tracks,
                })
            })
            .collect();
        found.sort_by_cached_key(|a| (a.artist.to_lowercase(), a.name.to_lowercase()));
        found
    }

    /// The Inbox albums as tree nodes, one level under the Inbox at `depth`.
    pub(super) fn inbox_album_nodes(&self, depth: i32) -> Vec<Node> {
        self.inbox_albums()
            .into_iter()
            .map(|a| {
                let mut n = Node::new(
                    a.title(),
                    depth,
                    KIND_ALBUM,
                    Contents::InboxAlbum(a.dir.clone()),
                );
                n.count = Some(a.tracks.len());
                n
            })
            .collect()
    }

    /// The tracks of the Inbox album in `dir`, in album order.
    pub(super) fn inbox_album_tracks(&self, dir: &Path) -> Vec<usize> {
        self.inbox_albums()
            .into_iter()
            .find(|a| a.dir == dir)
            .map(|a| a.tracks)
            .unwrap_or_default()
    }

    /// The Inbox album the track at a row of the list is in.
    fn inbox_album_of_row(&self, row: i32) -> Option<PathBuf> {
        let t = self
            .rows
            .get(usize::try_from(row).ok()?)
            .copied()
            .flatten()
            .and_then(|i| self.library.get(i))?;
        let dir = file_path(t)?.parent()?;
        self.inbox_albums()
            .into_iter()
            .find(|a| a.dir == dir)
            .map(|a| a.dir)
    }

    /// Moves the Inbox album in `dir` into the albums folder, as
    /// `Artist/Album`, its cover and cue sheet with it; its tracks go to
    /// Play Ready. Without an albums folder, asks for one first.
    pub(super) fn move_to_albums(&mut self, dir: &Path) {
        if !self.saved() {
            return;
        }
        let Some(root) = self.folders.albums.clone() else {
            self.inbox_albums.waiting = Some(dir.to_path_buf());
            let tx = self.inbox_albums.tx.clone();
            crate::prefs::choose_folder("Choose your Albums folder", move |picked| {
                let _ = tx.send(picked);
            });
            return;
        };
        let Some(album) = self.inbox_albums().into_iter().find(|a| a.dir == dir) else {
            return;
        };
        let Some(to_dir) =
            stages::album_destination(&root, &album.artist, &album.name, |p| p.exists())
        else {
            return;
        };
        let changes: Vec<Change> = album
            .tracks
            .iter()
            .filter_map(|&i| {
                let t = &self.library[i];
                let name = file_path(t)?.file_name()?;
                Some(Change {
                    track: t.id,
                    to: Some(to_dir.join(name)),
                })
            })
            .collect();
        let Some(db) = &mut self.db else { return };
        let batch = match db.change_stage(&changes, Stage::Collection) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("can't move the album: {e}");
                return;
            }
        };
        let failed: HashSet<PathBuf> = match db.apply_batch(batch) {
            Ok(r) => r
                .failed
                .into_iter()
                .map(|(path, why)| {
                    eprintln!("can't move {}: {why}", path.display());
                    path
                })
                .collect(),
            Err(e) => {
                eprintln!("can't move the album: {e}");
                return;
            }
        };
        if failed.is_empty() {
            if let Err(e) = djsugar_library::move_leftovers(dir, &to_dir) {
                eprintln!("can't move the rest of {}: {e}", dir.display());
            }
        }
        // Out of the Inbox list too, when the downloads went into one.
        if let Some(list) = db.inbox_list_id() {
            let ids: HashSet<usize> = album.tracks.iter().map(|&i| self.library[i].id).collect();
            let listed = db.playlist_tracks(list).unwrap_or_default();
            for (at, id) in listed.iter().enumerate().rev() {
                if ids.contains(id) {
                    let _ = db.remove_from_playlist(list, at);
                }
            }
        }
        for (&i, change) in album.tracks.iter().zip(&changes) {
            let mut t = (*self.library[i]).clone();
            t.info.stage = Stage::Collection.name().into();
            if let (Some(to), Some(from)) = (&change.to, file_path(&t).map(Path::to_path_buf)) {
                if !failed.contains(&from) {
                    self.path_index.remove(&from);
                    self.path_index.insert(to.clone(), i);
                    t.source = Source::File(to.clone());
                }
            }
            self.indexed[i] = Indexed::new(&t);
            self.library[i] = Arc::new(t);
        }
        self.read_pending_moves();
        let n = album.tracks.len();
        self.undo = Some(Undo {
            batch: Some(batch),
            text: format!(
                "Undo Move to Albums ({n} track{})",
                if n == 1 { "" } else { "s" }
            ),
            restore: Vec::new(),
            changes: None,
            relinked: Vec::new(),
        });
        self.reload_tree(None);
        self.refresh_rows();
    }

    /// Moves the album that was waiting once the DJ has picked an Albums
    /// folder, which is kept as the setting.
    pub fn poll_inbox_albums(&mut self) {
        while let Ok(root) = self.inbox_albums.rx.try_recv() {
            settings::set(settings::ALBUMS_FOLDER, json!(root.to_string_lossy()));
            self.folders.albums = Some(root);
            if let Some(dir) = self.inbox_albums.waiting.take() {
                self.move_to_albums(&dir);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::one_album;

    #[test]
    fn an_album_is_one_album_tag() {
        assert_eq!(
            one_album(&["Moon Safari", "moon safari", ""]).as_deref(),
            Some("Moon Safari")
        );
        assert_eq!(one_album(&["Moon Safari"]), None);
        assert_eq!(one_album(&["Moon Safari", "Talkie Walkie"]), None);
        assert_eq!(one_album(&["", "", "Moon Safari"]), None);
        assert_eq!(one_album(&["", ""]), None);
    }
}
