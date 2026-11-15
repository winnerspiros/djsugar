//! The Prep layout's part of the browser: the track list goes to In Prep
//! when Prep opens, the deck shows the loaded track's stage, and Play
//! Ready, next track moves the track on and loads the next one.

use std::time::{Duration, Instant};

use djsugar_library::stages::Stage;
use slint::ComponentHandle;

use super::{staging::stage_number, Browser, Contents};
use crate::{AppWindow, PrepUi};

/// The deck Prep works on.
const PREP_DECK: usize = 0;
/// How long Play Ready, next track waits for the grid lock to be saved.
const LOCK_WAIT: Duration = Duration::from_millis(1500);

impl Browser {
    pub(super) fn install_prep(
        app: &AppWindow,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        app.global::<PrepUi>()
            .on_ready_next(move || w(&|b| b.prep_ready_next()));
    }

    /// Shows the In Prep list, as the Prep layout opens.
    pub fn show_in_prep(&mut self) {
        let Some(node) = self
            .nodes
            .iter()
            .position(|n| matches!(n.contents, Contents::Stage(Stage::Triage, None)))
        else {
            return;
        };
        if self.current_node != node {
            self.current_node = node;
            if let Some(app) = self.app.upgrade() {
                app.set_current_track(-1);
            }
            self.refresh_tree();
            self.refresh_rows();
        }
        self.refresh_prep();
    }

    /// The library index of the track on the Prep deck.
    fn prep_track(&self) -> Option<usize> {
        self.index_of.get(&self.loaded[PREP_DECK]?).copied()
    }

    /// The next track to prepare after `current`: the next In Prep row of
    /// the list shown (wrapping round), else the first In Prep track in
    /// the library.
    fn next_in_prep(&self, current: Option<usize>) -> Option<usize> {
        let in_prep = |i: usize| {
            Some(i) != current
                && self
                    .library
                    .get(i)
                    .is_some_and(|t| !t.info.missing && self.at_stage(t, Stage::Triage, None))
        };
        let rows: Vec<usize> = self.rows.iter().flatten().copied().collect();
        let start = current
            .and_then(|c| rows.iter().position(|&r| r == c))
            .map_or(0, |p| p + 1);
        rows[start.min(rows.len())..]
            .iter()
            .chain(&rows[..start.min(rows.len())])
            .copied()
            .find(|&i| in_prep(i))
            .or_else(|| (0..self.library.len()).find(|&i| in_prep(i)))
    }

    /// Play Ready, next track: the track on the Prep deck goes to Play
    /// Ready and its grid is locked (PrepUi.ready turned on the deck's
    /// `bpmlock`); the next In Prep track loads once the lock is saved, so
    /// it can't land on the new track instead (see [`Self::poll_prep`]).
    fn prep_ready_next(&mut self) {
        if self.prep_pending.is_some() {
            return;
        }
        let current = self.prep_track();
        let next = self.next_in_prep(current);
        if current.is_some() {
            self.review_done();
        }
        if let Some(i) = current {
            if self.library[i].info.stage != Stage::Collection.name() {
                self.change_stage(&[i], Stage::Collection, None);
            }
        }
        match current {
            Some(i) if !self.library[i].info.grid_locked => {
                self.prep_pending = Some((self.library[i].id, next, Instant::now()));
            }
            _ => self.load_prep(next),
        }
        self.refresh_prep();
    }

    /// Loads the next track to prepare, once the finished one's grid lock
    /// is in the library, or after a moment if it never comes (a track
    /// without a grid).
    pub fn poll_prep(&mut self) {
        let Some((id, next, asked)) = self.prep_pending else {
            return;
        };
        let locked = self
            .index_of
            .get(&id)
            .is_none_or(|&i| self.library[i].info.grid_locked);
        if locked || self.loaded[PREP_DECK] != Some(id) || asked.elapsed() > LOCK_WAIT {
            self.prep_pending = None;
            self.load_prep(next);
        }
    }

    fn load_prep(&mut self, next: Option<usize>) {
        if let Some(i) = next.filter(|&i| self.playable(i)) {
            self.adopt(&[i]);
            self.engine.load_track(PREP_DECK, self.library[i].clone());
        }
    }

    /// The stage of the track on the Prep deck, and how many are left In
    /// Prep, for the Prep layout.
    pub(super) fn refresh_prep(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let stage = self
            .prep_track()
            .map_or(0, |i| stage_number(&self.library[i].info.stage));
        let left = self
            .library
            .iter()
            .filter(|t| !t.info.missing && self.at_stage(t, Stage::Triage, None))
            .count();
        let ui = app.global::<PrepUi>();
        ui.set_stage(if stage > 3 { 0 } else { stage });
        ui.set_left(left as i32);
    }
}
