//! The side panel's Cues tab: the hot cues, memory cues and saved loops of
//! the track on a deck, to name, colour, move between pads and memory, and
//! jump to. Prep's pads show the names and colours set here.
//!
//! The cues the deck shows are in [`super::CueWatch::saved`], which
//! [`Browser::keep`] keeps up to date from the deck's pads; an edit here
//! changes that list, saves it, and sets the pads to match.

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::{deck_group, Hotcue, Track, MEMORY_CUE, NUM_DECKS, NUM_HOTCUES};
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{AppWindow, CueList, CueRow};

/// The hot cue colours, as `Theme.hotcues`, for pads and the colour dot.
pub(super) const PALETTE: [u32; 8] = [
    0x0064c8, 0x04bc00, 0xdcb800, 0xc45800, 0xc0002c, 0xc42874, 0x7838c4, 0x10bcbc,
];
/// A memory cue's colour until one is picked: the app's lilac.
const MEMORY_COLOR: u32 = 0xb69cff;
/// The Prep layout's number (see `PREP_LAYOUT` in main.rs).
const PREP_LAYOUT: i32 = 3;
/// Our hotcue_N_type for a cue and a saved loop.
const CUE_TYPE: f64 = 1.0;
const LOOP_TYPE: f64 = 4.0;

/// A deck's pad controls: enabled, position, end, type.
struct Pad {
    enabled: ControlHandle,
    position: ControlHandle,
    end: ControlHandle,
    kind: ControlHandle,
}

/// A deck's controls the tab reads and sets.
struct DeckCues {
    pads: Vec<Pad>,
    playposition: ControlHandle,
    duration: ControlHandle,
    loop_enabled: ControlHandle,
    loop_start: ControlHandle,
    loop_end: ControlHandle,
}

pub(super) struct CueState {
    decks: Vec<DeckCues>,
    /// The cues shown, in their order in the list.
    rows: Vec<Hotcue>,
    /// What the list was made from, so it is made again only on a change.
    shown: Option<(usize, Option<usize>, Vec<Hotcue>, bool)>,
}

impl CueState {
    pub(super) fn new(registry: &ControlRegistry) -> Self {
        let decks = (0..NUM_DECKS)
            .map(|d| {
                let g = deck_group(d);
                let c = |k: &str| registry.get(&g, k).expect("deck control");
                DeckCues {
                    pads: (1..=NUM_HOTCUES)
                        .map(|n| Pad {
                            enabled: c(&format!("hotcue_{n}_enabled")),
                            position: c(&format!("hotcue_{n}_position")),
                            end: c(&format!("hotcue_{n}_endposition")),
                            kind: c(&format!("hotcue_{n}_type")),
                        })
                        .collect(),
                    playposition: c("playposition"),
                    duration: c("duration"),
                    loop_enabled: c("loop_enabled"),
                    loop_start: c("loop_start_position"),
                    loop_end: c("loop_end_position"),
                }
            })
            .collect();
        Self {
            decks,
            rows: Vec::new(),
            shown: None,
        }
    }
}

/// The colour a cue shows: its own, else its pad's, else lilac.
fn shown_color(c: &Hotcue) -> u32 {
    c.color.unwrap_or(if c.is_memory() {
        MEMORY_COLOR
    } else {
        PALETTE[c.number % PALETTE.len()]
    })
}

fn color(rgb: u32) -> Color {
    Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// m:ss.t, as the pads show it.
fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0);
    let minutes = (s / 60.0).floor();
    let secs = s - minutes * 60.0;
    format!("{}:{:04.1}", minutes as u64, (secs * 10.0).floor() / 10.0)
}

/// Where a cue is, by bar, or how long a loop is.
fn detail(t: Option<&Track>, c: &Hotcue) -> String {
    let Some(t) = t.filter(|t| t.bpm > 0.0) else {
        return match c.end {
            Some(end) => format!("{:.1} s", end - c.seconds),
            None => String::new(),
        };
    };
    match c.end {
        Some(end) => {
            let bars = (t.beat_at(end) - t.beat_at(c.seconds)) / 4.0;
            let bars = (bars * 4.0).round() / 4.0;
            if bars == 1.0 {
                "1 bar".into()
            } else {
                format!("{bars} bars")
            }
        }
        None => {
            let beat = t.beat_at(c.seconds) - crate::beat_counter::first_bar_beat(t);
            let bar = (beat / 4.0 + 1e-6).floor() as i64 + 1;
            if bar < 1 {
                "before bar 1".into()
            } else {
                format!("bar {bar}")
            }
        }
    }
}

/// The next free memory cue number.
fn next_memory(cues: &[Hotcue]) -> usize {
    cues.iter()
        .filter(|c| c.is_memory())
        .map(|c| c.number + 1)
        .max()
        .unwrap_or(MEMORY_CUE)
}

impl Browser {
    pub(super) fn install_cues(
        app: &AppWindow,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let ui = app.global::<CueList>();
        let w = with.clone();
        ui.on_pick_deck(move |d| {
            w(&|b| {
                if let Some(app) = b.app.upgrade() {
                    app.global::<CueList>().set_deck(d.clamp(0, 1));
                }
                b.refresh_cues();
            })
        });
        let w = with.clone();
        ui.on_rename(move |row, name| {
            let name = name.trim().to_string();
            w(&|b| b.edit_cue(row, |c| c.name = name.clone()))
        });
        let w = with.clone();
        ui.on_recolor(move |row| {
            w(&|b| {
                b.edit_cue(row, |c| {
                    let now = shown_color(c);
                    let next = PALETTE
                        .iter()
                        .position(|&p| p == now)
                        .map_or(0, |i| (i + 1) % PALETTE.len());
                    c.color = Some(PALETTE[next]);
                })
            })
        });
        let w = with.clone();
        ui.on_renumber(move |row, pad| w(&|b| b.renumber_cue(row, pad)));
        let w = with.clone();
        ui.on_remove(move |row| w(&|b| b.remove_cue(row)));
        let w = with.clone();
        ui.on_go(move |row| w(&|b| b.go_to_cue(row)));
        let w = with.clone();
        ui.on_add_memory(move || w(&|b| b.add_memory_cue()));
    }

    /// The deck the tab follows: deck 1 in Prep.
    pub(super) fn cue_deck(&self) -> usize {
        let Some(app) = self.app.upgrade() else {
            return 0;
        };
        if app.get_layout() == PREP_LAYOUT {
            0
        } else {
            app.global::<CueList>().get_deck().clamp(0, 1) as usize
        }
    }

    /// The track on `deck`, from the library.
    pub(super) fn cue_track(&self, deck: usize) -> Option<&std::sync::Arc<Track>> {
        self.loaded[deck]
            .and_then(|id| self.index_of.get(&id))
            .map(|&i| &self.library[i])
    }

    /// The cues of the deck's track, as the deck has them now.
    fn deck_cues(&self, deck: usize) -> Vec<Hotcue> {
        match (self.loaded[deck], self.watch[deck].track) {
            (Some(id), Some((watched, _))) if id == watched => self.watch[deck].saved.clone(),
            _ => self
                .cue_track(deck)
                .map(|t| t.info.hotcues.clone())
                .unwrap_or_default(),
        }
    }

    /// Lists the deck's cues again, when they changed. Called every frame.
    pub(super) fn refresh_cues(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let deck = self.cue_deck();
        let prep = app.get_layout() == PREP_LAYOUT;
        // Cues and Checklist are Prep's tabs: out of Prep, the first tab
        // that shows.
        super::side_tabs::keep_open_tab(&app, prep);
        let cues = self.deck_cues(deck);
        let key = (deck, self.loaded[deck], cues.clone(), prep);
        if self.cues.shown.as_ref() == Some(&key) {
            return;
        }
        let track = self.cue_track(deck).cloned();
        let mut rows = cues.clone();
        rows.sort_by(|a, b| {
            a.seconds
                .total_cmp(&b.seconds)
                .then(a.number.cmp(&b.number))
        });
        let ui = app.global::<CueList>();
        ui.set_prep(prep);
        ui.set_title(
            track
                .as_ref()
                .map(|t| {
                    if t.artist.is_empty() {
                        t.title.clone()
                    } else {
                        format!("{} · {}", t.artist, t.title)
                    }
                })
                .unwrap_or_default()
                .into(),
        );
        let model: Vec<CueRow> = rows
            .iter()
            .map(|c| CueRow {
                pad: if c.is_memory() || c.number >= NUM_HOTCUES {
                    -1
                } else {
                    c.number as i32
                },
                is_loop: c.end.is_some(),
                time: clock(c.seconds).into(),
                detail: detail(track.as_deref(), c).into(),
                name: SharedString::from(c.name.as_str()),
                color: color(shown_color(c)),
            })
            .collect();
        ui.set_rows(ModelRc::new(VecModel::from(model)));
        // Prep's pads are deck 1's.
        let pad_cues = if deck == 0 {
            cues.clone()
        } else {
            self.deck_cues(0)
        };
        let pad = |n: usize| pad_cues.iter().find(|c| c.number == n);
        let names: Vec<SharedString> = (0..NUM_HOTCUES)
            .map(|n| pad(n).map(|c| c.name.as_str().into()).unwrap_or_default())
            .collect();
        let colors: Vec<Color> = (0..NUM_HOTCUES)
            .map(|n| {
                pad(n)
                    .and_then(|c| c.color)
                    .map_or(Color::from_argb_u8(0, 0, 0, 0), color)
            })
            .collect();
        ui.set_pad_names(ModelRc::new(VecModel::from(names)));
        ui.set_pad_colors(ModelRc::new(VecModel::from(colors)));
        self.cues.rows = rows;
        self.cues.shown = Some(key);
    }

    /// Changes the deck's cues with `change`, saves them and sets the pads
    /// whose cue changed.
    fn change_cues(&mut self, deck: usize, change: impl FnOnce(&mut Vec<Hotcue>)) {
        let Some(id) = self.loaded[deck] else {
            return;
        };
        let before = self.deck_cues(deck);
        let mut cues = before.clone();
        change(&mut cues);
        if cues == before {
            return;
        }
        // The pads first, so the deck's next look at them finds these cues.
        let d = &self.cues.decks[deck];
        for (n, pad) in d.pads.iter().enumerate() {
            let was = before.iter().find(|c| c.number == n);
            let now = cues.iter().find(|c| c.number == n);
            let same = |a: &Hotcue, b: &Hotcue| a.seconds == b.seconds && a.end == b.end;
            match (was, now) {
                (Some(a), Some(b)) if same(a, b) => {}
                (_, Some(c)) => {
                    pad.position.set(c.seconds);
                    pad.end.set(c.end.unwrap_or(-1.0));
                    pad.kind
                        .set(if c.end.is_some() { LOOP_TYPE } else { CUE_TYPE });
                    pad.enabled.set_bool(true);
                }
                (Some(_), None) => {
                    pad.enabled.set_bool(false);
                    pad.kind.set(0.0);
                    pad.end.set(-1.0);
                }
                (None, None) => {}
            }
        }
        let w = &mut self.watch[deck];
        if w.track.map(|(t, _)| t) == Some(id) {
            w.saved = cues.clone();
        }
        if djsugar_library::row_id(id).is_some() && self.saved() {
            if let Some(db) = &mut self.db {
                if let Err(e) = db.save_cues(id, &cues) {
                    eprintln!("can't save the cues: {e}");
                }
            }
        }
        if let Some(&i) = self.index_of.get(&id) {
            let mut e = (*self.library[i]).clone();
            e.info.hotcues = cues;
            self.library[i] = std::sync::Arc::new(e);
        }
        self.refresh_cues();
        self.refresh_rows();
    }

    /// The cue on list row `row`, by number and place.
    fn cue_at(&self, row: i32) -> Option<(usize, f64)> {
        let c = self.cues.rows.get(usize::try_from(row).ok()?)?;
        Some((c.number, c.seconds))
    }

    fn edit_cue(&mut self, row: i32, edit: impl Fn(&mut Hotcue)) {
        let Some((number, seconds)) = self.cue_at(row) else {
            return;
        };
        let deck = self.cue_deck();
        self.change_cues(deck, |cues| {
            if let Some(c) = cues
                .iter_mut()
                .find(|c| c.number == number && c.seconds == seconds)
            {
                edit(c);
            }
        });
    }

    /// Moves the cue on `row` to hot cue `pad` (from 0), or to the memory
    /// cues (-1). A cue on that pad takes this one's place, or becomes a
    /// memory cue.
    fn renumber_cue(&mut self, row: i32, pad: i32) {
        let Some((number, seconds)) = self.cue_at(row) else {
            return;
        };
        let deck = self.cue_deck();
        self.change_cues(deck, |cues| {
            let Some(k) = cues
                .iter()
                .position(|c| c.number == number && c.seconds == seconds)
            else {
                return;
            };
            let memory = next_memory(cues);
            match usize::try_from(pad).ok().filter(|&p| p < NUM_HOTCUES) {
                Some(to) if to != number => {
                    let from_pad = number < NUM_HOTCUES;
                    if let Some(other) = cues.iter_mut().find(|c| c.number == to) {
                        other.number = if from_pad { number } else { memory };
                    }
                    cues[k].number = to;
                }
                None if number < NUM_HOTCUES => cues[k].number = memory,
                _ => {}
            }
        });
    }

    fn remove_cue(&mut self, row: i32) {
        let Some((number, seconds)) = self.cue_at(row) else {
            return;
        };
        let deck = self.cue_deck();
        self.change_cues(deck, |cues| {
            cues.retain(|c| !(c.number == number && c.seconds == seconds))
        });
    }

    fn go_to_cue(&mut self, row: i32) {
        let Some((_, seconds)) = self.cue_at(row) else {
            return;
        };
        let d = &self.cues.decks[self.cue_deck()];
        let duration = d.duration.get();
        if duration > 0.0 {
            d.playposition.set((seconds / duration).clamp(0.0, 1.0));
        }
    }

    /// A memory cue at the playhead, or the playing loop as a memory loop.
    fn add_memory_cue(&mut self) {
        let deck = self.cue_deck();
        let d = &self.cues.decks[deck];
        let duration = d.duration.get();
        if self.loaded[deck].is_none() || duration <= 0.0 {
            return;
        }
        let (start, end) = (d.loop_start.get(), d.loop_end.get());
        let (seconds, end) = if d.loop_enabled.get_bool() && end > start && start >= 0.0 {
            (start, Some(end))
        } else {
            (d.playposition.get() * duration, None)
        };
        self.change_cues(deck, |cues| {
            if cues
                .iter()
                .any(|c| c.is_memory() && (c.seconds - seconds).abs() < 0.01 && c.end == end)
            {
                return;
            }
            let number = next_memory(cues);
            cues.push(Hotcue {
                number,
                seconds,
                name: String::new(),
                end,
                color: None,
            });
        });
    }
}

impl Browser {
    /// Quick review's Tab: to the start of the next phrase of the track on
    /// the Prep deck (`direction` 1), or back to the start of this one, or
    /// the one before when just past a start (-1). Without phrases, 8 bars
    /// at a time.
    pub(super) fn jump_phrase(&mut self, direction: i32) {
        let loaded = self.engine.loaded_tracks();
        let Some(t) = loaded.read().ok().and_then(|l| l[0].clone()) else {
            return;
        };
        let d = &self.cues.decks[0];
        let duration = d.duration.get();
        if duration <= 0.0 {
            return;
        }
        let now = d.playposition.get() * duration;
        let phrases = if djsugar_engine::phrases::waveform_ready(&t) {
            djsugar_engine::phrases::detect(&t)
        } else {
            djsugar_engine::phrases::stored(&t)
        };
        let mut starts: Vec<f64> = phrases.iter().map(|p| p.start).collect();
        if starts.len() < 2 {
            if t.bpm <= 0.0 {
                return;
            }
            // Every 8 bars along the grid.
            let first = t.seconds_at(crate::beat_counter::first_bar_beat(&t));
            let step = 32.0 * 60.0 / t.bpm;
            starts = (0..)
                .map(|n| first + n as f64 * step)
                .take_while(|&s| s < duration)
                .collect();
        }
        if let Some(to) = phrase_target(&starts, now, direction) {
            d.playposition.set((to / duration).clamp(0.0, 1.0));
        }
    }
}

/// Where Tab goes from `now` among phrase `starts` (in order): the next
/// start, or back to this phrase's start (the one before when within a
/// second of it), or the track's start.
fn phrase_target(starts: &[f64], now: f64, direction: i32) -> Option<f64> {
    if direction > 0 {
        starts.iter().copied().find(|&s| s > now + 0.05)
    } else {
        starts
            .iter()
            .copied()
            .rev()
            .find(|&s| s < now - 1.0)
            .or(Some(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_goes_phrase_by_phrase() {
        let starts = [0.0, 15.5, 31.0, 62.0];
        assert_eq!(phrase_target(&starts, 0.0, 1), Some(15.5));
        assert_eq!(phrase_target(&starts, 15.5, 1), Some(31.0));
        assert_eq!(phrase_target(&starts, 70.0, 1), None);
        assert_eq!(phrase_target(&starts, 40.0, -1), Some(31.0));
        assert_eq!(phrase_target(&starts, 31.5, -1), Some(15.5));
        assert_eq!(phrase_target(&starts, 0.5, -1), Some(0.0));
    }

    #[test]
    fn clock_reads_like_the_pads() {
        assert_eq!(clock(0.0), "0:00.0");
        assert_eq!(clock(75.36), "1:15.3");
    }

    #[test]
    fn memory_cues_number_on_from_the_last() {
        let cue = |number| Hotcue {
            number,
            seconds: 0.0,
            name: String::new(),
            end: None,
            color: None,
        };
        assert_eq!(next_memory(&[cue(0)]), MEMORY_CUE);
        assert_eq!(next_memory(&[cue(MEMORY_CUE + 2), cue(3)]), MEMORY_CUE + 3);
    }
}
