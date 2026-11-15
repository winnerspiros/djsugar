//! **Export as rekordbox XML…**: File › Export › rekordbox XML…, for the
//! sidebar row picked. On the Collection node it writes every track and the
//! whole list tree (Crates and Playlists as folders); on a list or folder
//! just that list, or the folder with everything in it. The file itself is
//! written by `djsugar_plugins::rekordbox::xml_export`, on the save
//! dialog's thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::Track;
use djsugar_plugins::rekordbox::xml_export::{self, XmlList};

use super::{Browser, Contents, COLLECTION, KIND_SECTION};
use crate::LibraryEdit;

/// What the export said when it finished: the list it was for (None for
/// the collection) and the file written, or why it failed.
type Done = (Option<i64>, Result<PathBuf, String>);

pub(super) struct State {
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Default for State {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }
}

impl Browser {
    pub(super) fn install_rekordbox_xml(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_rekordbox(move |row| w(&|b| b.export_rekordbox(row as usize)));
    }

    /// Whether the sidebar row `node` can go out as rekordbox XML: the
    /// Collection node, or one of the DJ's lists or folders in it.
    pub(super) fn rekordbox_exportable(&self, node: usize) -> bool {
        self.tree == COLLECTION
            && self.nodes.get(node).is_some_and(|n| {
                !n.history
                    && n.kind != KIND_SECTION
                    && match n.contents {
                        Contents::All | Contents::Folder => true,
                        _ => n.playlist.is_some() || matches!(n.contents, Contents::List(_)),
                    }
            })
    }

    /// Asks for a folder, then writes the row's tracks and lists there.
    fn export_rekordbox(&mut self, row: usize) {
        let Some(&node) = self.visible_nodes.get(row) else {
            return;
        };
        if !self.rekordbox_exportable(node) {
            return;
        }
        let (tracks, lists) = self.rekordbox_lists(node);
        let list = match self.nodes[node].contents {
            Contents::Playlist(id) => Some(id),
            _ => None,
        };
        if tracks.is_empty() {
            if let Some(id) = list {
                self.export_status(id, "Nothing to export: the list is empty.".into());
            }
            return;
        }
        let whole = matches!(self.nodes[node].contents, Contents::All);
        let file = if whole {
            "rekordbox.xml".to_string()
        } else {
            format!(
                "{}.xml",
                djsugar_library::playlist_files::file_name(&self.node_title(node))
            )
        };
        let prompt = format!(
            "Choose where to save \"{}\" as rekordbox XML",
            self.node_title(node)
        );
        let tx = self.rekordbox_xml.tx.clone();
        crate::prefs::choose_folder(&prompt, move |dir| {
            let path = dir.join(file);
            let text = xml_export::write(&tracks, &lists, env!("CARGO_PKG_VERSION"));
            let result = std::fs::write(&path, text)
                .map(|_| path)
                .map_err(|e| e.to_string());
            let _ = tx.send((list, result));
        });
    }

    /// The tracks under a node, and its list tree. The Collection node
    /// gives every track and every list outside History.
    fn rekordbox_lists(&self, node: usize) -> (Vec<Track>, Vec<XmlList>) {
        let mut tracks: Vec<Track> = Vec::new();
        let mut index: HashMap<usize, usize> = HashMap::new();
        let mut add = |b: &Browser, t: usize| -> usize {
            *index.entry(t).or_insert_with(|| {
                tracks.push((*b.library[t]).clone());
                tracks.len() - 1
            })
        };
        let whole = matches!(self.nodes[node].contents, Contents::All);
        let mut lists: Vec<XmlList> = Vec::new();
        // The folders open above each depth, by their place in `lists`.
        let mut open: Vec<(i32, usize)> = Vec::new();
        let (first, top_depth) = if whole {
            for t in self.node_tracks(node) {
                add(self, t);
            }
            let first = self
                .nodes
                .iter()
                .position(|n| n.kind == KIND_SECTION)
                .unwrap_or(self.nodes.len());
            (first, -1)
        } else {
            (node, self.nodes[node].depth - 1)
        };
        for n in first..self.nodes.len() {
            let m = &self.nodes[n];
            if n > node && !whole && m.depth <= self.nodes[node].depth {
                break;
            }
            if m.history || m.depth <= top_depth {
                if whole {
                    continue;
                }
                break;
            }
            let folder = matches!(m.contents, Contents::Folder);
            if !folder && !(m.playlist.is_some() || matches!(m.contents, Contents::List(_))) {
                continue;
            }
            while open.last().is_some_and(|&(d, _)| d >= m.depth) {
                open.pop();
            }
            let mut list = XmlList {
                name: self.node_title(n),
                parent: open.last().map(|&(_, i)| i),
                folder,
                tracks: Vec::new(),
            };
            if folder {
                open.push((m.depth, lists.len()));
            } else {
                list.tracks = self
                    .node_tracks(n)
                    .into_iter()
                    .map(|t| add(self, t))
                    .collect();
            }
            lists.push(list);
        }
        (tracks, lists)
    }

    /// Shows how exports the dialogs answered went.
    pub fn poll_rekordbox_xml(&mut self) {
        while let Ok((list, result)) = self.rekordbox_xml.rx.try_recv() {
            let note = match result {
                Ok(path) => {
                    crate::files::reveal(&path);
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Saved {name}")
                }
                Err(e) => {
                    eprintln!("rekordbox XML export failed: {e}");
                    format!("Couldn't save the rekordbox XML: {e}")
                }
            };
            if let Some(id) = list {
                self.export_status(id, note);
            }
        }
    }
}
