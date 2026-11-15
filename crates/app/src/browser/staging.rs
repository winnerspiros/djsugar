//! The stages in the browser (see [`djsugar_library::stages`]): Inbox, Triage
//! and Collection at the top of the tree with their counts, each stage's
//! subfolders under it (triage crates such as "Yoga STG"), and moving
//! tracks between them from the track menu or by dropping them on a stage.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::{Source, Track};
use djsugar_library::query::Indexed;
use djsugar_library::stages::{self, Folders, Stage};
use djsugar_library::{Change, PlaylistKind};
use slint::{ModelRc, SharedString, VecModel};

use super::{
    file_path, is_browsed, Browser, Contents, Node, Undo, KIND_FOLDER, KIND_INBOX, KIND_READY,
    KIND_TRIAGE,
};
use crate::analysis_queue::AnalysisQueue;
use crate::{settings, LibraryEdit};

/// The tree's stage nodes: each stage, with its folder's subfolders under
/// Triage and Collection.
pub(super) fn stage_nodes(folders: &Folders) -> Vec<Node> {
    let mut nodes = Vec::new();
    for (stage, kind) in [
        (Stage::Inbox, KIND_INBOX),
        (Stage::Triage, KIND_TRIAGE),
        (Stage::Collection, KIND_READY),
    ] {
        nodes.push(Node::new(
            stage.title(),
            0,
            kind,
            Contents::Stage(stage, None),
        ));
        if stage == Stage::Inbox {
            continue;
        }
        let Some(dir) = folders.folder(stage) else {
            continue;
        };
        for name in folders.crates(stage) {
            let sub = dir.join(&name);
            nodes.push(Node::new(
                name,
                1,
                KIND_FOLDER,
                Contents::Stage(stage, Some(sub)),
            ));
        }
    }
    nodes
}

/// The name of the list a triage crate's tracks join in the collection:
/// the folder's name without a trailing " STG" ("Yoga STG" goes to "Yoga").
fn crate_list_name(folder: &str) -> &str {
    let trimmed = folder.trim();
    let lower = trimmed.to_lowercase();
    for suffix in [" stg", " staging"] {
        if lower.ends_with(suffix) && trimmed.len() > suffix.len() {
            return trimmed[..trimmed.len() - suffix.len()].trim_end();
        }
    }
    trimmed
}

/// The number `LibraryEdit.stage-of` gives a stage.
pub(super) fn stage_number(stage: &str) -> i32 {
    match Stage::from_name(stage) {
        None => 0,
        Some(Stage::Inbox) => 1,
        Some(Stage::Triage) => 2,
        Some(Stage::Collection) => 3,
    }
}

fn stage_from_number(n: i32) -> Option<Stage> {
    match n {
        1 => Some(Stage::Inbox),
        2 => Some(Stage::Triage),
        3 => Some(Stage::Collection),
        _ => None,
    }
}

/// What the undo line calls a change.
fn change_label(stage: Stage, crate_name: Option<&str>) -> String {
    match (stage, crate_name) {
        (Stage::Triage, Some(c)) => format!("Send to In Prep › {c}"),
        (Stage::Triage, None) => "Send to In Prep".into(),
        (Stage::Collection, _) => "Send to Play Ready".into(),
        (Stage::Inbox, _) => "Back to Inbox".into(),
    }
}

impl Browser {
    pub(super) fn install_staging(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_stage_of(move |row| {
            let stage = std::cell::Cell::new(0);
            w(&|b| {
                let n = b
                    .rows
                    .get(row as usize)
                    .copied()
                    .flatten()
                    .and_then(|i| b.library.get(i))
                    .map_or(0, |t| stage_number(&t.info.stage));
                stage.set(n);
            });
            stage.get()
        });
        let w = with.clone();
        edit.on_set_stage(move |first, last, stage, crate_index| {
            w(&|b| {
                let Some(stage) = stage_from_number(stage) else {
                    return;
                };
                let dir = usize::try_from(crate_index).ok().and_then(|i| {
                    let name = b.folders.crates(Stage::Triage).into_iter().nth(i)?;
                    Some(b.folders.triage.as_ref()?.join(name))
                });
                let indices = b.chosen_tracks(first.max(0) as usize, last.max(0) as usize);
                b.change_stage(&indices, stage, dir.as_deref());
            })
        });
        let w = with.clone();
        edit.on_undo_stage(move || w(&|b| b.undo_stage()));
    }

    /// Whether `t` is at `stage` (and headed for the subfolder `dir`, when
    /// given: where its file is, or will be once its move is made).
    pub(super) fn at_stage(&self, t: &Track, stage: Stage, dir: Option<&Path>) -> bool {
        t.info.stage == stage.name()
            && !is_browsed(t)
            && dir.is_none_or(|d| {
                self.pending_moves
                    .get(&t.id)
                    .map(PathBuf::as_path)
                    .or_else(|| file_path(t))
                    .is_some_and(|p| p.starts_with(d))
            })
    }

    /// Reads which files are waiting to move, and where to.
    pub(super) fn read_pending_moves(&mut self) {
        self.pending_moves = self
            .db
            .as_ref()
            .and_then(|db| db.pending_moves().ok())
            .unwrap_or_default()
            .into_iter()
            .map(|m| (m.track, m.to))
            .collect();
    }

    /// The stage a row of the tree stands for, and its subfolder.
    pub(super) fn stage_at(&self, row: usize) -> Option<(Stage, Option<PathBuf>)> {
        match &self.nodes.get(*self.visible_nodes.get(row)?)?.contents {
            Contents::Stage(stage, dir) => Some((*stage, dir.clone())),
            _ => None,
        }
    }

    /// Moves library tracks (indices into `library`) to `stage`, into the
    /// subfolder `dir` of its folder when given (a triage crate). The
    /// stage is saved at once; files move now if the DJ has file moves on,
    /// else their moves wait (see [`djsugar_library::Library::apply_moves`]).
    /// Going from the inbox to triage, a file is renamed "Artist - Title
    /// (Remix)"; going from a triage crate to the collection, the track
    /// joins the list named after the crate.
    pub(super) fn change_stage(&mut self, indices: &[usize], stage: Stage, dir: Option<&Path>) {
        if !self.folders.is_set() || !self.saved() {
            return;
        }
        self.adopt(indices);
        let crate_name = dir
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned());
        let rename = stage == Stage::Triage && settings::rename_to_triage();
        let mut planned: HashSet<PathBuf> = HashSet::new();
        let mut picked: Vec<(usize, Option<PathBuf>)> = Vec::new();
        let mut crate_lists: HashMap<String, Vec<usize>> = HashMap::new();
        for &i in indices {
            let Some(t) = self.library.get(i).filter(|t| !is_browsed(t)) else {
                continue;
            };
            let Some(path) = file_path(t) else { continue };
            let here = self.folders.stage_of(path) == Some(stage)
                && crate_name
                    .as_deref()
                    .is_none_or(|c| self.folders.crate_of(stage, path).as_deref() == Some(c));
            let to = if here {
                None
            } else {
                let from_inbox = matches!(t.info.stage.as_str(), "" | "inbox");
                let name = (rename && from_inbox)
                    .then(|| stages::clean_name(t, path))
                    .flatten();
                stages::destination(
                    &self.folders,
                    stage,
                    crate_name.as_deref(),
                    path,
                    name.as_deref(),
                    |p| p.exists() || planned.contains(p) || self.path_index.contains_key(p),
                )
            };
            if let Some(to) = &to {
                planned.insert(to.clone());
            }
            if stage == Stage::Collection {
                if let Some(c) = self.folders.crate_of(Stage::Triage, path) {
                    crate_lists.entry(c).or_default().push(t.id);
                }
            }
            picked.push((i, to));
        }
        if picked.is_empty() {
            return;
        }
        let Some(db) = &mut self.db else { return };
        let changes: Vec<Change> = picked
            .iter()
            .map(|(i, to)| Change {
                track: self.library[*i].id,
                to: to.clone(),
            })
            .collect();
        let batch = match db.change_stage(&changes, stage) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("can't change the stage: {e}");
                return;
            }
        };
        let failed: HashSet<PathBuf> = if settings::move_files() {
            match db.apply_moves() {
                Ok(r) => r
                    .failed
                    .into_iter()
                    .map(|(path, why)| {
                        eprintln!("can't move {}: {why}", path.display());
                        path
                    })
                    .collect(),
                Err(e) => {
                    eprintln!("can't move files: {e}");
                    return;
                }
            }
        } else {
            HashSet::new()
        };
        let mut lists_changed = false;
        for (name, ids) in crate_lists {
            let name = crate_list_name(&name);
            let existing = db.playlists().ok().and_then(|lists| {
                lists
                    .into_iter()
                    .find(|p| {
                        matches!(p.kind, PlaylistKind::List | PlaylistKind::Crate)
                            && p.name.eq_ignore_ascii_case(name)
                    })
                    .map(|p| p.id)
            });
            let list = match existing {
                Some(id) => Ok(id),
                None => {
                    lists_changed = true;
                    db.create_crate(name, None)
                }
            };
            if let Err(e) = list.and_then(|id| db.add_to_playlist(id, &ids)) {
                eprintln!("can't add to the {name} list: {e}");
            }
        }
        let moving = settings::move_files();
        for (i, to) in &picked {
            let mut t = (*self.library[*i]).clone();
            t.info.stage = stage.name().into();
            if let (true, Some(to), Some(from)) = (moving, to, file_path(&t).map(Path::to_path_buf))
            {
                if !failed.contains(&from) {
                    self.path_index.remove(&from);
                    self.path_index.insert(to.clone(), *i);
                    t.source = Source::File(to.clone());
                }
            }
            self.indexed[*i] = Indexed::new(&t);
            self.library[*i] = Arc::new(t);
        }
        self.read_pending_moves();
        let label = change_label(stage, crate_name.as_deref());
        let n = picked.len();
        self.undo = Some(Undo {
            batch: Some(batch),
            text: format!("Undo {label} ({n} track{})", if n == 1 { "" } else { "s" }),
            restore: Vec::new(),
            changes: None,
            relinked: Vec::new(),
        });
        if stage == Stage::Triage {
            let tracks: Vec<Arc<Track>> = picked
                .iter()
                .map(|(i, _)| self.library[*i].clone())
                .collect();
            self.analysis.add(&tracks);
        }
        if lists_changed {
            self.reload_tree(None);
        } else {
            self.refresh_tree();
        }
        self.refresh_rows();
        self.refresh_prep();
    }

    /// Takes back the last stage change (stages, file moves and all), or
    /// the last merge of copies.
    fn undo_stage(&mut self) {
        let Some(undo) = self.undo.take() else {
            return;
        };
        if let Some(batch) = undo.changes {
            self.undo_tag_changes(batch);
            return;
        }
        if !undo.relinked.is_empty() {
            self.unlink(&undo.relinked);
            return;
        }
        let Some(db) = &mut self.db else { return };
        let report = match undo.batch.map(|b| db.undo_batch(b)) {
            None => Default::default(),
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                eprintln!("can't undo: {e}");
                return;
            }
        };
        for (path, why) in &report.failed {
            eprintln!("can't move {} back: {why}", path.display());
        }
        if !undo.restore.is_empty() {
            if let Err(e) = db.restore_tracks(Some(&undo.restore)) {
                eprintln!("can't bring the copies back: {e}");
            }
            self.health_status = format!(
                "{} back in your collection.",
                super::health::tracks(undo.restore.len())
            );
            self.reload_library();
            self.refresh_tree();
            self.refresh_rows();
            return;
        }
        // Stages and paths come back from the library as it is now.
        let tracks = match db.all_tracks(Path::exists) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("can't read the library: {e}");
                return;
            }
        };
        let by_id: HashMap<usize, Track> = tracks.into_iter().map(|t| (t.id, t)).collect();
        self.read_pending_moves();
        for i in 0..self.library.len() {
            let Some(saved) = by_id.get(&self.library[i].id) else {
                continue;
            };
            let current = &self.library[i];
            if saved.info.stage == current.info.stage && file_path(saved) == file_path(current) {
                continue;
            }
            let mut t = (**current).clone();
            t.info.stage = saved.info.stage.clone();
            if let (Some(old), Some(new)) = (file_path(current), file_path(saved)) {
                if old != new {
                    self.path_index.remove(old);
                    self.path_index.insert(new.to_path_buf(), i);
                }
            }
            t.source = saved.source.clone();
            self.indexed[i] = Indexed::new(&t);
            self.library[i] = Arc::new(t);
        }
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_prep();
    }

    /// Shows the track menu's stage lines as things are now.
    pub(super) fn refresh_stage_ui(&self, edit: &LibraryEdit) {
        let on = self.folders.is_set() && self.saved();
        edit.set_stages(on);
        let crates: Vec<SharedString> = if on {
            self.folders
                .crates(Stage::Triage)
                .into_iter()
                .map(SharedString::from)
                .collect()
        } else {
            Vec::new()
        };
        edit.set_triage_crates(ModelRc::new(VecModel::from(crates)));
        let undo = self.undo.as_ref().map_or("", |u| u.text.as_str());
        edit.set_undo_text(undo.into());
        crate::app_menu::set_undo_text(undo);
    }

    /// Queues the tracks in triage that still need their tempo or key.
    pub(super) fn queue_triage_analysis(&mut self) {
        let triage: Vec<Arc<Track>> = self
            .library
            .iter()
            .filter(|t| self.at_stage(t, Stage::Triage, None) && AnalysisQueue::needs(t))
            .cloned()
            .collect();
        self.analysis.add(&triage);
    }

    /// Saves what background analysis found since last time, then the
    /// covers found online for the tracks analysed.
    ///
    /// Tracks that finish close together are saved and shown together, at
    /// most every [`super::analysis::APPLY_EVERY`], so a long batch doesn't
    /// keep the UI busy.
    pub fn poll_analysis(&mut self) {
        let finished = self.analysis.take();
        let done = self
            .analysis_shown
            .due(finished, self.analysis.progress().is_none());
        if !done.is_empty() {
            for t in &done {
                self.learn_background(t);
            }
            let done: Vec<Option<Arc<Track>>> =
                done.into_iter().map(|t| Some(Arc::new(t))).collect();
            self.learn_analysis(&done);
            self.refresh_tree();
        }
        for (id, data) in self.analysis.take_covers() {
            if let Some(&i) = self.index_of.get(&id) {
                if let Err(e) = self.keep_cover(i, &data) {
                    eprintln!("can't keep the cover found: {e}");
                }
            }
        }
        self.show_analysis();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_lists_drop_the_staging_suffix() {
        assert_eq!(crate_list_name("Yoga STG"), "Yoga");
        assert_eq!(crate_list_name("Journey stg"), "Journey");
        assert_eq!(crate_list_name("Vault Staging"), "Vault");
        assert_eq!(crate_list_name("Albums"), "Albums");
        assert_eq!(crate_list_name("STG"), "STG");
    }
}
