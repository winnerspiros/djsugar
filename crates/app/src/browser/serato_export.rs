//! **Export to Serato DJ…**: File › Export › Serato DJ…, for the sidebar
//! row picked (the Collection, a list or a folder). It writes each track's
//! hot cues, saved loops and beat grid into its file the way Serato keeps
//! them, and the lists as crates, in their folders, in the `_Serato_`
//! folder the DJ picks. The work is done by
//! `djsugar_plugins::serato::export`, on the dialog's thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::{Source, Track};
use djsugar_plugins::serato::export::{self, SeratoCrate};
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
    pub(super) fn install_serato_export(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_serato(move |row| w(&|b| b.export_serato(row as usize)));
    }

    /// Asks for the `_Serato_` folder, then writes the row's tracks' cues
    /// into their files and its lists as crates there.
    fn export_serato(&mut self, row: usize) {
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
            "Choose your _Serato_ folder (in Music) for \"{}\": cues, loops and grids go into the files",
            self.node_title(node)
        );
        let tx = self.serato_export.tx.clone();
        crate::prefs::choose_folder(&prompt, move |dir| {
            let _ = tx.send((list, run(&dir, &tracks, &lists)));
        });
    }

    /// Shows how exports the dialogs answered went.
    pub fn poll_serato_export(&mut self) {
        while let Ok((list, result)) = self.serato_export.rx.try_recv() {
            let note = result.unwrap_or_else(|e| {
                eprintln!("Serato export failed: {e}");
                format!("Couldn't export to Serato: {e}")
            });
            if let Some(id) = list {
                self.export_status(id, note);
            }
        }
    }
}

/// Writes the tags and crates; a line for the DJ saying what was done.
fn run(serato: &Path, tracks: &[Track], lists: &[NmlList]) -> Result<String, String> {
    if serato.file_name().is_some_and(|n| n != "_Serato_") {
        return Err("pick the folder named _Serato_".into());
    }
    let mut written = 0;
    let mut failed = 0;
    for t in tracks {
        match export::write_tags(t) {
            Ok(()) => written += 1,
            Err(e) => {
                eprintln!("Serato export: {e}");
                failed += 1;
            }
        }
    }
    let files = |l: &NmlList| -> Vec<PathBuf> {
        l.tracks
            .iter()
            .filter_map(|&i| match &tracks.get(i)?.source {
                Source::File(p) => Some(p.clone()),
                Source::Synth { .. } => None,
            })
            .collect()
    };
    let crates: Vec<SeratoCrate> = lists
        .iter()
        .filter(|l| !l.folder)
        .map(|l| {
            let mut folders = Vec::new();
            let mut up = l.parent;
            while let Some(p) = up {
                folders.insert(0, lists[p].name.clone());
                up = lists[p].parent;
            }
            SeratoCrate {
                folders,
                name: l.name.clone(),
                files: files(l),
            }
        })
        .collect();
    let elsewhere = export::write_crates(serato, &crates)?;
    let mut note = format!(
        "Sent {written} track{} and {} crate{} to Serato",
        if written == 1 { "" } else { "s" },
        crates.len(),
        if crates.len() == 1 { "" } else { "s" }
    );
    if failed > 0 {
        note += &format!(
            "; {failed} file{} couldn't be written",
            if failed == 1 { "" } else { "s" }
        );
    }
    if elsewhere > 0 {
        note += &format!("; {elsewhere} on another drive left out of crates");
    }
    Ok(note)
}
