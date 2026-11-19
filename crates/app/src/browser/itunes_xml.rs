//! **Export as iTunes XML…**: File › Export › iTunes XML…, for the sidebar
//! row picked: the tracks and lists as an iTunes / Music Library XML, the
//! file rekordbox, Serato, Traktor, VirtualDJ and djay read as their iTunes
//! library. The file is written by `djsugar_plugins::itunes_library_export`,
//! on the save dialog's thread.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_plugins::itunes_library_export::{self, ItunesList};

use super::{Browser, Contents};
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
    pub(super) fn install_itunes_xml(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_itunes(move |row| w(&|b| b.export_itunes(row as usize)));
    }

    /// Asks for a folder, then writes the row's tracks and lists there.
    fn export_itunes(&mut self, row: usize) {
        let Some(&node) = self.visible_nodes.get(row) else {
            return;
        };
        if !self.traktor_exportable(node) {
            return;
        }
        let (tracks, lists) = self.traktor_lists(node);
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
            "iTunes Music Library.xml".to_string()
        } else {
            format!(
                "{}.xml",
                djsugar_library::playlist_files::file_name(&self.node_title(node))
            )
        };
        let prompt = format!(
            "Choose where to save \"{}\" as iTunes XML",
            self.node_title(node)
        );
        let tx = self.itunes_xml.tx.clone();
        crate::prefs::choose_folder(&prompt, move |dir| {
            let path = dir.join(file);
            let lists: Vec<ItunesList> = lists
                .into_iter()
                .map(|l| ItunesList {
                    name: l.name,
                    folder: l.folder,
                    parent: l.parent,
                    tracks: l.tracks,
                })
                .collect();
            let result = itunes_library_export::write(&path, &tracks, &lists).map(|_| path);
            let _ = tx.send((list, result));
        });
    }

    /// Shows how exports the dialogs answered went.
    pub fn poll_itunes_xml(&mut self) {
        while let Ok((list, result)) = self.itunes_xml.rx.try_recv() {
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
                    eprintln!("iTunes XML export failed: {e}");
                    format!("Couldn't save the iTunes XML: {e}")
                }
            };
            if let Some(id) = list {
                self.export_status(id, note);
            }
        }
    }
}
