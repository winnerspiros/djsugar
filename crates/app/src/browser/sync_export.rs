//! **Sync to other apps**: File › Export › Sync to other apps repeats the
//! last export of the whole Collection to each app it went to (Mixxx,
//! Engine DJ, VirtualDJ, Serato, rekordbox), into the same file or folder,
//! with no dialog. Each export merges into the app's library as the dialog's
//! does, with a backup. The line saying how it went shows on the Collection.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::Track;
use djsugar_plugins::traktor::nml_export::NmlList;
use serde_json::json;

use super::{Browser, Contents};
use crate::{settings, LibraryEdit};

/// One app's export: its file or folder, the tracks and the lists.
type Run = fn(&Path, &[Track], &[NmlList]) -> Result<String, String>;

/// The apps a Collection export can be repeated to: settings name, title
/// and export.
const APPS: [(&str, &str, Run); 5] = [
    ("mixxx", "Mixxx", super::mixxx_export::run),
    ("engine", "Engine DJ", super::engine_export::run),
    ("virtualdj", "VirtualDJ", super::virtualdj_export::run),
    ("serato", "Serato", super::serato_export::run),
    ("rekordbox_db", "rekordbox", super::rekordbox_db_export::run),
];

fn key(app: &str) -> String {
    format!("sync_target:{app}")
}

/// Notes where the whole Collection was exported to `app`, for the next sync.
pub(super) fn remember(app: &str, target: &Path) {
    settings::set(&key(app), json!(target.to_string_lossy()));
}

pub(super) struct State {
    tx: Sender<String>,
    rx: Receiver<String>,
    /// How the last sync went, or that one is running.
    note: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx, note: None }
    }
}

impl Browser {
    pub(super) fn install_sync_export(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_sync(move || w(&|b| b.sync_exports()));
    }

    /// Exports the Collection again to every app it went to before, on a
    /// thread.
    fn sync_exports(&mut self) {
        let targets: Vec<(&str, PathBuf, Run)> = APPS
            .iter()
            .filter_map(|&(id, title, run)| {
                let path = settings::string(&key(id));
                (!path.is_empty()).then(|| (title, PathBuf::from(path), run))
            })
            .collect();
        let collection = self
            .nodes
            .iter()
            .position(|n| matches!(n.contents, Contents::All))
            .filter(|&n| self.traktor_exportable(n));
        let Some(node) = collection.filter(|_| !targets.is_empty()) else {
            self.sync_export.note = Some(
                if targets.is_empty() {
                    "Nothing to sync yet: export the Collection to an app from File › Export first."
                } else {
                    "Open the Collection, then sync."
                }
                .into(),
            );
            self.refresh_rows();
            return;
        };
        let (tracks, lists) = self.traktor_lists(node);
        let names: Vec<&str> = targets.iter().map(|t| t.0).collect();
        self.sync_export.note = Some(format!("Syncing to {}…", names.join(", ")));
        self.refresh_rows();
        let tx = self.sync_export.tx.clone();
        std::thread::spawn(move || {
            let lines: Vec<String> = targets
                .into_iter()
                .map(|(title, path, run)| match run(&path, &tracks, &lists) {
                    Ok(done) => format!("{title}: {done}"),
                    Err(e) => format!("{title}: couldn't sync: {e}"),
                })
                .collect();
            let _ = tx.send(lines.join(" "));
        });
    }

    /// Shows how a sync went.
    pub fn poll_sync_export(&mut self) {
        while let Ok(note) = self.sync_export.rx.try_recv() {
            self.sync_export.note = Some(note);
            self.refresh_rows();
        }
    }

    /// How the last sync went, while the Collection is shown.
    pub(super) fn sync_note(&self) -> Option<&str> {
        let shown = self.nodes.get(self.current_node)?;
        matches!(shown.contents, Contents::All)
            .then_some(self.sync_export.note.as_deref())
            .flatten()
    }
}
