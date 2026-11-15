//! The Prep checklist: what makes a track ready to play, in the side
//! panel's Checklist tab. The app ticks its own lines when the track has
//! what they ask for (each can be turned off in Settings › Collection ›
//! Stages, and ticked by hand when the track is fine without); the DJ's own
//! lines are ticked by hand, with a click or Option+1–9. A personal score
//! (1 to 10) goes with it. Play Ready lights up when every line is ticked.
//!
//! Also the quick review clock: how long the track on the Prep deck has
//! taken, and the average per track sent to Play Ready from Prep.

use std::time::Instant;

use djsugar_engine::Track;
use djsugar_library::checklist::MAX_SCORE;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::{columns, Browser};
use crate::{settings, AppWindow, CheckRow, ChecklistUi, PrepUi};

/// The app's lines: settings key and name.
use settings::PREP_APP_LINES as APP_LINES;
/// Hot cues the cues line asks for.
const CUES_WANTED: usize = 4;
/// Longer than this on one track isn't a review (the DJ left it loaded).
const LONGEST_REVIEW: f64 = 30.0 * 60.0;

/// One line of a track's checklist.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Line {
    /// What a hand tick is kept as: the DJ's line's name, or "app:" and
    /// the app line's key.
    pub key: String,
    pub name: String,
    pub detail: String,
    /// Ticked by the app.
    pub auto: bool,
    pub ticked: bool,
}

/// What the Checklist tab shows: the track's name, its lines and score,
/// and Prep's count (ticked, of how many).
pub(super) type Shown = (String, Vec<Line>, Option<u8>, (usize, usize));

/// The quick review clock.
#[derive(Default)]
pub(super) struct Review {
    /// The track on the Prep deck, by id, and since when.
    track: Option<(usize, Instant)>,
    /// The seconds shown.
    shown: i32,
}

/// Which lines there are, from the settings: the app's turned on, and the
/// DJ's own.
#[derive(Clone)]
pub(super) struct Setup {
    app: Vec<bool>,
    own: Vec<String>,
}

/// How long the lines read from the settings are used before reading them
/// again (they change in Settings, and rows ask for them many times).
const SETUP_AGE: std::time::Duration = std::time::Duration::from_secs(1);

impl Setup {
    fn read() -> Self {
        Setup {
            app: APP_LINES
                .iter()
                .map(|(key, _)| settings::flag(&settings::prep_check_key(key), true))
                .collect(),
            own: settings::prep_lines(),
        }
    }
}

impl Browser {
    /// `t`'s checklist, the app's lines first.
    pub(super) fn checklist(&self, t: &Track) -> Vec<Line> {
        let marks = self.marks.get(&t.id);
        let by_hand = |key: &str| marks.is_some_and(|m| m.ticks.iter().any(|k| k == key));
        let setup = self.checklist_setup();
        let mut out = Vec::new();
        for (i, (key, name)) in APP_LINES.into_iter().enumerate() {
            if !setup.app[i] {
                continue;
            }
            let (auto, detail) = match key {
                "grid" => {
                    let i = &t.info;
                    let analysed = matches!(i.grid_from.as_str(), "" | "analysis");
                    if t.bpm <= 0.0 && t.grid.is_empty() {
                        (false, "No grid yet".into())
                    } else if i.grid_locked {
                        (true, "Locked".into())
                    } else if !analysed && !i.grid_unsure {
                        (true, "Edited".into())
                    } else if i.grid_unsure {
                        (false, "The analysis wasn't sure: check it".into())
                    } else {
                        (false, "Detected: check it, then lock it or tick it".into())
                    }
                }
                "cues" => {
                    let n = columns::cue_count(t);
                    (n >= CUES_WANTED, format!("{n} of {CUES_WANTED} hot cues"))
                }
                "genre" => (
                    !t.genre.trim().is_empty(),
                    if t.genre.trim().is_empty() {
                        "No genre".into()
                    } else {
                        t.genre.clone()
                    },
                ),
                "rating" => (
                    t.rating > 0,
                    if t.rating > 0 {
                        "★".repeat(t.rating.min(5) as usize)
                    } else {
                        "No rating".into()
                    },
                ),
                _ => match self.health.has_cover(t.id) {
                    Some(true) => (true, String::new()),
                    Some(false) => (false, "No cover".into()),
                    None => (false, "Not looked at yet (Collection Health)".into()),
                },
            };
            let hand_key = format!("app:{key}");
            let hand = !auto && by_hand(&hand_key);
            out.push(Line {
                key: hand_key,
                name: name.into(),
                detail: if hand {
                    "Ticked by hand".into()
                } else {
                    detail
                },
                auto,
                ticked: auto || hand,
            });
        }
        for name in setup.own {
            out.push(Line {
                ticked: by_hand(&name),
                key: name.clone(),
                name,
                detail: String::new(),
                auto: false,
            });
        }
        out
    }

    /// The lines there are, read again from the settings once a second.
    fn checklist_setup(&self) -> Setup {
        let mut cached = self.checklist_setup.borrow_mut();
        match &*cached {
            Some((at, setup)) if at.elapsed() < SETUP_AGE => setup.clone(),
            _ => {
                let setup = Setup::read();
                *cached = Some((Instant::now(), setup.clone()));
                setup
            }
        }
    }

    /// Lines ticked on `t`, of how many.
    pub(super) fn ready_count(&self, t: &Track) -> (usize, usize) {
        let lines = self.checklist(t);
        (lines.iter().filter(|l| l.ticked).count(), lines.len())
    }

    /// What `t`'s row shows in the Prep columns `ids` (worked out only for
    /// the columns shown).
    pub(super) fn prep_facts(&self, t: &Track, ids: &[i32]) -> columns::PrepFacts {
        columns::PrepFacts {
            cover: self.health.has_cover(t.id),
            score: self.score(t.id),
            ready: ids.contains(&columns::READY).then(|| self.ready_count(t)),
            ..if ids.contains(&columns::PROBLEM) || ids.contains(&columns::FIX) {
                let (problem, fix) = self.health_cells(t);
                columns::PrepFacts {
                    problem,
                    fix,
                    ..Default::default()
                }
            } else {
                Default::default()
            }
        }
    }

    pub(super) fn score(&self, id: usize) -> Option<u8> {
        self.marks.get(&id).and_then(|m| m.score)
    }

    pub(super) fn load_marks(&mut self) {
        if let Some(db) = &self.db {
            self.marks = db.prep_marks().unwrap_or_default();
        }
    }

    pub(super) fn install_checklist(
        app: &AppWindow,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let ui = app.global::<ChecklistUi>();
        let w = with.clone();
        ui.on_tick(move |row| w(&|b| b.tick_line(row)));
        let w = with.clone();
        ui.on_set_score(move |score| w(&|b| b.set_score(score)));
        let weak = app.as_weak();
        ui.on_edit_lines(move || {
            if let Some(app) = weak.upgrade() {
                // Settings › Collection, its Stages tab.
                app.global::<crate::Preferences>()
                    .invoke_show_tab(crate::prefs::LIBRARY_PAGE, crate::prefs::STAGES_TAB as i32);
            }
        });
        let w = with.clone();
        app.global::<PrepUi>()
            .on_phrase(move |dir| w(&|b| b.jump_phrase(dir)));
    }

    /// The track the tab is for: the one on the deck the Cues tab follows.
    fn checklist_track(&self) -> Option<std::sync::Arc<Track>> {
        self.cue_track(self.cue_deck()).cloned()
    }

    /// Ticks line `row` by hand, or takes the tick off. The app's own
    /// ticks stay.
    fn tick_line(&mut self, row: i32) {
        let Some(t) = self.checklist_track() else {
            return;
        };
        let Some(line) = usize::try_from(row)
            .ok()
            .and_then(|r| self.checklist(&t).into_iter().nth(r))
        else {
            return;
        };
        if line.auto {
            return;
        }
        let on = !line.ticked;
        if let Some(db) = &self.db {
            if let Err(e) = db.set_prep_tick(t.id, &line.key, on) {
                eprintln!("can't save the tick: {e}");
            }
        }
        let marks = self.marks.entry(t.id).or_default();
        marks.ticks.retain(|k| *k != line.key);
        if on {
            marks.ticks.push(line.key);
        }
        self.refresh_checklist();
        self.refresh_rows();
    }

    fn set_score(&mut self, score: i32) {
        let Some(t) = self.checklist_track() else {
            return;
        };
        let score = u8::try_from(score.clamp(0, MAX_SCORE as i32))
            .ok()
            .filter(|&s| s > 0);
        if let Some(db) = &self.db {
            if let Err(e) = db.set_prep_score(t.id, score) {
                eprintln!("can't save the score: {e}");
            }
        }
        self.marks.entry(t.id).or_default().score = score;
        self.refresh_checklist();
        self.refresh_rows();
    }

    /// Fills the Checklist tab and Prep's count again, when they changed.
    /// Called every frame.
    pub(super) fn refresh_checklist(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let track = self.checklist_track();
        let lines = track
            .as_ref()
            .map(|t| self.checklist(t))
            .unwrap_or_default();
        let score = track.as_ref().and_then(|t| self.score(t.id));
        let title = track
            .as_ref()
            .map(|t| {
                if t.artist.is_empty() {
                    t.title.clone()
                } else {
                    format!("{} · {}", t.artist, t.title)
                }
            })
            .unwrap_or_default();
        // Prep's header counts deck 1's track.
        let prep = self
            .cue_track(0)
            .map(|t| self.ready_count(t))
            .unwrap_or_default();
        let key = (title, lines, score, prep);
        if self.checklist_shown.as_ref() == Some(&key) {
            return;
        }
        let (title, lines, score, prep) = &key;
        let ui = app.global::<ChecklistUi>();
        ui.set_title(title.as_str().into());
        let rows: Vec<CheckRow> = lines
            .iter()
            .map(|l| CheckRow {
                name: SharedString::from(l.name.as_str()),
                detail: SharedString::from(l.detail.as_str()),
                auto: l.auto,
                ticked: l.ticked,
            })
            .collect();
        ui.set_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_done(lines.iter().filter(|l| l.ticked).count() as i32);
        ui.set_score(score.map_or(0, i32::from));
        let p = app.global::<PrepUi>();
        p.set_done(prep.0 as i32);
        p.set_total(prep.1 as i32);
        self.checklist_shown = Some(key);
    }

    /// Keeps the quick review clock: restarts it when another track comes
    /// onto the Prep deck. Called every frame.
    pub(super) fn tick_review(&mut self) {
        let id = self.loaded[0];
        if self.review.track.map(|(t, _)| t) != id {
            self.review.track = id.map(|t| (t, Instant::now()));
        }
        let seconds = self
            .review
            .track
            .map_or(0, |(_, since)| since.elapsed().as_secs() as i32);
        if seconds != self.review.shown {
            self.review.shown = seconds;
            if let Some(app) = self.app.upgrade() {
                app.global::<PrepUi>().set_track_seconds(seconds);
            }
        }
    }

    /// The track on the Prep deck is done: its time counts towards the
    /// average.
    pub(super) fn review_done(&mut self) {
        let Some((_, since)) = self.review.track else {
            return;
        };
        let spent = since.elapsed().as_secs_f64();
        if spent < LONGEST_REVIEW {
            let (total, tracks) = settings::prep_review();
            settings::set_prep_review(total + spent, tracks + 1);
        }
        self.show_review_average();
    }

    pub(super) fn show_review_average(&self) {
        let (total, tracks) = settings::prep_review();
        let average = if tracks > 0 {
            total / tracks as f64
        } else {
            0.0
        };
        if let Some(app) = self.app.upgrade() {
            app.global::<PrepUi>()
                .set_average_seconds(average.round() as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_lines_have_their_own_keys() {
        let mut keys: Vec<&str> = APP_LINES.iter().map(|(k, _)| *k).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), APP_LINES.len());
    }
}
