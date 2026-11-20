//! **Export to VirtualDJ…**: File › Export › VirtualDJ…, for the sidebar
//! row picked (the Collection, a list or a folder). It puts each track's
//! hot cues, saved loops, beat grid, sections and automix points into
//! VirtualDJ's `database.xml`, and the lists into `MyLists`, in the
//! VirtualDJ folder the DJ picks. The work is done by
//! `djsugar_plugins::virtualdj::export`, on the dialog's thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::Track;
use djsugar_plugins::traktor::nml_export::NmlList;
use djsugar_plugins::virtualdj::export::{self, VdjList};

use super::Browser;
use crate::LibraryEdit;

/// What the export said when it finished: the list it was for (None for
/// the collection), and a line for the DJ or why it failed.
type Done = (Option<i64>, Option<PathBuf>, Result<String, String>);

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
    pub(super) fn install_virtualdj_export(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_virtualdj(move |row| w(&|b| b.export_virtualdj(row as usize)));
    }

    /// Asks for VirtualDJ's folder, then writes the row's tracks' points
    /// into its database and the row's lists into its lists.
    fn export_virtualdj(&mut self, row: usize) {
        let Some(&node) = self.visible_nodes.get(row) else {
            return;
        };
        if !self.traktor_exportable(node) {
            return;
        }
        let (tracks, lists) = self.traktor_lists(node);
        let list = match self.nodes[node].contents {
            super::Contents::Playlist(id) => Some(id),
            _ => None,
        };
        if tracks.is_empty() {
            if let Some(id) = list {
                self.export_status(id, "Nothing to export: the list is empty.".into());
            }
            return;
        }
        let prompt = format!(
            "Close VirtualDJ, then choose its folder (the one with database.xml) for \"{}\"",
            self.node_title(node)
        );
        // The whole Collection: where it went is kept for Sync.
        let whole = matches!(self.nodes[node].contents, super::Contents::All);
        let tx = self.virtualdj_export.tx.clone();
        crate::prefs::choose_folder(&prompt, move |dir| {
            let result = run(&dir, &tracks, &lists);
            let _ = tx.send((list, whole.then_some(dir), result));
        });
    }

    /// Shows how exports the dialogs answered went.
    pub fn poll_virtualdj_export(&mut self) {
        while let Ok((list, target, result)) = self.virtualdj_export.rx.try_recv() {
            if let (Some(t), Ok(_)) = (&target, &result) {
                super::sync_export::remember("virtualdj", t);
            }
            let note = result.unwrap_or_else(|e| {
                eprintln!("VirtualDJ export failed: {e}");
                format!("Couldn't export to VirtualDJ: {e}")
            });
            if let Some(id) = list {
                self.export_status(id, note);
            }
        }
    }
}

/// Writes the database and lists; a line for the DJ saying what was done.
pub(super) fn run(home: &Path, tracks: &[Track], lists: &[NmlList]) -> Result<String, String> {
    let database = home.join("database.xml");
    if !database.is_file() {
        return Err("pick the VirtualDJ folder, the one with database.xml in it".into());
    }
    let w = export::write_database(&database, tracks)?;
    let out: Vec<VdjList> = lists
        .iter()
        .filter(|l| !l.folder)
        .map(|l| {
            let mut folders = Vec::new();
            let mut up = l.parent;
            while let Some(p) = up {
                folders.insert(0, lists[p].name.clone());
                up = lists[p].parent;
            }
            VdjList {
                folders,
                name: l.name.clone(),
                tracks: l.tracks.iter().filter_map(|&i| tracks.get(i)).collect(),
            }
        })
        .collect();
    let written = export::write_lists(home, &out)?;
    let plural = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    Ok(format!(
        "Sent {} to VirtualDJ ({} new) and {}; its old database is kept as database.xml.djsugar-backup",
        plural(w.updated + w.added, "track"),
        w.added,
        plural(written.len(), "list")
    ))
}
