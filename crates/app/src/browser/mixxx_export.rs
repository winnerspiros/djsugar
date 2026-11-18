//! **Export to Mixxx…**: File › Export › Mixxx…, for the sidebar row
//! picked (the Collection, a list or a folder). It gives the tracks Mixxx
//! has their hot cues, saved loops and beat grids, and makes the lists
//! Mixxx playlists, in the `mixxxdb.sqlite` the DJ picks. The work is done
//! by `djsugar_plugins::mixxx_export`, on the dialog's thread.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::Track;
use djsugar_plugins::mixxx_export::{self, MixxxList};
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
    pub(super) fn install_mixxx_export(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_mixxx(move |row| w(&|b| b.export_mixxx(row as usize)));
    }

    /// Asks for Mixxx's database, then writes the row's tracks' cues and
    /// grids and its lists into it.
    fn export_mixxx(&mut self, row: usize) {
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
            "Close Mixxx, then choose its library, mixxxdb.sqlite, for \"{}\"",
            self.node_title(node)
        );
        let tx = self.mixxx_export.tx.clone();
        crate::prefs::choose_file(&prompt, move |dir| {
            let _ = tx.send((list, run(&dir, &tracks, &lists)));
        });
    }

    /// Shows how exports the dialogs answered went.
    pub fn poll_mixxx_export(&mut self) {
        while let Ok((list, result)) = self.mixxx_export.rx.try_recv() {
            let note = result.unwrap_or_else(|e| {
                eprintln!("Mixxx export failed: {e}");
                format!("Couldn't export to Mixxx: {e}")
            });
            if let Some(id) = list {
                self.export_status(id, note);
            }
        }
    }
}

/// Writes the cues, grids and playlists; a line for the DJ saying what was
/// done.
fn run(database: &Path, tracks: &[Track], lists: &[NmlList]) -> Result<String, String> {
    if database.file_name().is_some_and(|n| n != "mixxxdb.sqlite") {
        return Err("pick Mixxx's library file, mixxxdb.sqlite".into());
    }
    let out: Vec<MixxxList> = lists
        .iter()
        .filter(|l| !l.folder)
        .map(|l| {
            // Mixxx has no folders: they go into the name.
            let mut name = l.name.clone();
            let mut up = l.parent;
            while let Some(p) = up {
                name = format!("{} / {name}", lists[p].name);
                up = lists[p].parent;
            }
            MixxxList {
                name,
                tracks: l.tracks.iter().filter_map(|&i| tracks.get(i)).collect(),
            }
        })
        .collect();
    let w = mixxx_export::write(database, tracks, &out)?;
    let plural = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    let mut note = format!(
        "Sent {} and {} to Mixxx; its old library is kept as mixxxdb.sqlite.djsugar-backup",
        plural(w.tracks, "track"),
        plural(w.playlists, "playlist")
    );
    if w.missing > 0 {
        note += &format!(
            "; {} Mixxx doesn't have yet (add their folder in Mixxx, then export again)",
            plural(w.missing, "track")
        );
    }
    Ok(note)
}
