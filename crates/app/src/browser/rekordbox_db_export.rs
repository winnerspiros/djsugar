//! **Export to rekordbox library…**: File › Export › rekordbox library…,
//! for the sidebar row picked (the Collection, a list or a folder). It puts
//! its lists into rekordbox's own `master.db` as playlists in their
//! folders. The work is done by `djsugar_plugins::rekordbox::master_export`,
//! on the dialog's thread.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::Track;
use djsugar_plugins::rekordbox::master_export::{self, RekordboxList};
use djsugar_plugins::traktor::nml_export::NmlList;

use super::Browser;
use crate::LibraryEdit;

/// What the export said when it finished: the list it was for (None for
/// the collection), and a line for the DJ or why it failed.
type Done = (Option<i64>, Result<String, String>);

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
    pub(super) fn install_rekordbox_db_export(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_rekordbox_db(move |row| w(&|b| b.export_rekordbox_db(row as usize)));
    }

    /// Asks for rekordbox's database, then writes the row's lists into it.
    fn export_rekordbox_db(&mut self, row: usize) {
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
            "Close rekordbox, then choose its library, master.db (in the Pioneer › rekordbox folder), for \"{}\"",
            self.node_title(node)
        );
        let tx = self.rekordbox_db_export.tx.clone();
        crate::prefs::choose_file(&prompt, move |dir| {
            let _ = tx.send((list, run(&dir, &tracks, &lists)));
        });
    }

    /// Shows how exports the dialogs answered went.
    pub fn poll_rekordbox_db_export(&mut self) {
        while let Ok((list, result)) = self.rekordbox_db_export.rx.try_recv() {
            let note = result.unwrap_or_else(|e| {
                eprintln!("rekordbox export failed: {e}");
                format!("Couldn't export to rekordbox: {e}")
            });
            if let Some(id) = list {
                self.export_status(id, note);
            }
        }
    }
}

/// Writes the playlists; a line for the DJ saying what was done.
fn run(database: &Path, tracks: &[Track], lists: &[NmlList]) -> Result<String, String> {
    if database.file_name().is_some_and(|n| n != "master.db") {
        return Err("pick rekordbox's library file, master.db".into());
    }
    let folders = |mut up: Option<usize>| {
        let mut out = Vec::new();
        while let Some(p) = up {
            out.insert(0, lists[p].name.clone());
            up = lists[p].parent;
        }
        out
    };
    let mut out: Vec<RekordboxList> = lists
        .iter()
        .filter(|l| !l.folder)
        .map(|l| RekordboxList {
            folders: folders(l.parent),
            name: l.name.clone(),
            tracks: l.tracks.iter().filter_map(|&i| tracks.get(i)).collect(),
        })
        .collect();
    // The Collection has no list of its own: it goes in as one.
    if out.is_empty() {
        out.push(RekordboxList {
            folders: Vec::new(),
            name: "DJ Sugar".into(),
            tracks: tracks.iter().collect(),
        });
    }
    let w = master_export::write(database, &out)?;
    let plural = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    let mut note = format!(
        "Sent {} to rekordbox; its old library is kept as master.db.djsugar-backup. Cues and grids go in with the rekordbox XML export",
        plural(w.playlists, "playlist")
    );
    if w.missing > 0 {
        note += &format!(
            "; {} rekordbox doesn't have yet were left out (add them in rekordbox, then export again)",
            plural(w.missing, "track")
        );
    }
    Ok(note)
}
