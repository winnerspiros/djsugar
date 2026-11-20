//! **Export to Engine DJ…**: File › Export › Engine DJ…, for the sidebar
//! row picked (the Collection, a list or a folder). It gives the tracks
//! Engine DJ has on this computer their hot cues, saved loops and beat
//! grids, and makes the lists Engine playlists, in the `m.db` the DJ picks.
//! The work is done by `djsugar_export::engine::desktop`, on the dialog's
//! thread. (Sticks for Engine players come from Export to USB.)

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::Track;
use djsugar_export::engine::desktop::{self, EngineList};
use djsugar_plugins::traktor::nml_export::NmlList;

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
    pub(super) fn install_engine_export(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_engine(move |row| w(&|b| b.export_engine(row as usize)));
    }

    /// Asks for Engine DJ's database, then writes the row's tracks' cues
    /// and grids and its lists into it.
    fn export_engine(&mut self, row: usize) {
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
            "Close Engine DJ, then choose its library, m.db (Music › Engine Library › Database2), for \"{}\"",
            self.node_title(node)
        );
        // The whole Collection: where it went is kept for Sync.
        let whole = matches!(self.nodes[node].contents, super::Contents::All);
        let tx = self.engine_export.tx.clone();
        crate::prefs::choose_file(&prompt, move |dir| {
            let result = run(&dir, &tracks, &lists);
            let _ = tx.send((list, whole.then_some(dir), result));
        });
    }

    /// Shows how exports the dialogs answered went.
    pub fn poll_engine_export(&mut self) {
        while let Ok((list, target, result)) = self.engine_export.rx.try_recv() {
            if let (Some(t), Ok(_)) = (&target, &result) {
                super::sync_export::remember("engine", t);
            }
            let note = result.unwrap_or_else(|e| {
                eprintln!("Engine DJ export failed: {e}");
                format!("Couldn't export to Engine DJ: {e}")
            });
            if let Some(id) = list {
                self.export_status(id, note);
            }
        }
    }
}

/// Writes the cues, grids and playlists; a line for the DJ saying what was
/// done.
pub(super) fn run(database: &Path, tracks: &[Track], lists: &[NmlList]) -> Result<String, String> {
    if database.file_name().is_some_and(|n| n != "m.db") {
        return Err("pick Engine DJ's library file, m.db".into());
    }
    let out: Vec<EngineList> = lists
        .iter()
        .filter(|l| !l.folder)
        .map(|l| {
            // Lists go in at the top, their folders in the name.
            let mut name = l.name.clone();
            let mut up = l.parent;
            while let Some(p) = up {
                name = format!("{} / {name}", lists[p].name);
                up = lists[p].parent;
            }
            EngineList {
                name,
                tracks: l.tracks.iter().filter_map(|&i| tracks.get(i)).collect(),
            }
        })
        .collect();
    let w = desktop::write(database, tracks, &out)?;
    let plural = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    let mut note = format!(
        "Sent {} and {} to Engine DJ; its old library is kept as m.db.djsugar-backup",
        plural(w.tracks, "track"),
        plural(w.playlists, "playlist")
    );
    if w.missing > 0 {
        note += &format!(
            "; {} Engine DJ doesn't have yet (add their folder in Engine DJ, then export again)",
            plural(w.missing, "track")
        );
    }
    Ok(note)
}
