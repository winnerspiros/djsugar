//! Lyrics along the bottom of each deck's big waveform: the timed lyrics
//! of the track on the deck, read when it loads (see
//! [`djsugar_engine::lyrics::find_with`]), each line a strip from when it
//! is sung to when it ends. The lane itself is drawn in Slint
//! (`LyricsLane`), scrolling with the waveform.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use djsugar_engine::lyrics::{self, Line, Lyrics};
use djsugar_engine::{Source, NUM_DECKS};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::super::Browser;
use super::store;
use crate::{AppWindow, DeckLines, WaveLine, WaveLyrics, WaveWord};

/// Whether the lyrics lane shows: Vocals › Lyrics in the waveforms
/// button's drop-down (see `wave_look`, which keeps the choice).
static SHOWN_NOW: AtomicBool = AtomicBool::new(true);

pub fn shown() -> bool {
    SHOWN_NOW.load(Ordering::Relaxed)
}

/// Vocals was picked: the lyrics lane shows with Lyrics only.
pub fn set_shown(on: bool) {
    SHOWN_NOW.store(on, Ordering::Relaxed);
}

#[derive(Default)]
struct Deck {
    /// The track whose lyrics the lane has or is reading.
    id: Option<usize>,
    job: Option<Receiver<Option<Lyrics>>>,
}

#[derive(Default)]
pub(in crate::browser) struct WaveState {
    decks: Vec<Deck>,
}

impl WaveState {
    /// Reads every deck's lyrics again, as after new ones were saved.
    pub(super) fn reload(&mut self) {
        for d in &mut self.decks {
            d.id = None;
            d.job = None;
        }
    }
}

/// The longest a line's last word is shown held, past what singing it
/// takes: a line timed to run on to a long gap shouldn't stretch across it.
const HOLD: f64 = 1.5;

/// The lines of timed lyrics that are sung, each word with when it starts
/// and ends (spread over the line by length when only the line is timed).
pub(super) fn wave_lines(lines: &[Line]) -> Vec<WaveLine> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.text.trim().is_empty())
        .filter_map(|(i, l)| {
            let start = l.start?;
            let until = lines.get(i + 1).and_then(|n| n.start);
            let mut times = l.word_times(until);
            if let Some(last) = times.last_mut() {
                let sung = lyrics::word_length(last.2.trim().chars().count());
                last.1 = last.1.min(last.0 + sung + HOLD);
            }
            let end = times.last().map_or(start, |w| w.1).max(start + 0.2);
            let words: Vec<WaveWord> = times
                .into_iter()
                .filter(|w| !w.2.trim().is_empty())
                .map(|(s, e, text)| WaveWord {
                    start: s as f32,
                    end: e.max(s + 0.05) as f32,
                    text: text.trim().into(),
                })
                .collect();
            Some(WaveLine {
                start: start as f32,
                end: end as f32,
                words: ModelRc::new(VecModel::from(words)),
            })
        })
        .collect()
}

impl Browser {
    /// Keeps each deck's lyrics lane up to date; called every frame.
    pub(in crate::browser) fn poll_wave_lyrics(&mut self, app: &AppWindow) {
        let ui = app.global::<WaveLyrics>();
        let on = shown();
        if ui.get_shown() != on {
            ui.set_shown(on);
        }
        let model = ui.get_decks();
        if model.row_count() != NUM_DECKS {
            ui.set_decks(ModelRc::new(VecModel::from(vec![
                DeckLines::default();
                NUM_DECKS
            ])));
        }
        let model = ui.get_decks();
        if self.lyrics.wave.decks.len() != NUM_DECKS {
            self.lyrics.wave.decks = (0..NUM_DECKS).map(|_| Deck::default()).collect();
        }
        for d in 0..NUM_DECKS {
            let id = if on { self.loaded[d] } else { None };
            if id != self.lyrics.wave.decks[d].id {
                let path = self.deck_track(d).filter(|_| id.is_some()).and_then(|i| {
                    match &self.library[i].source {
                        Source::File(p) => Some(p.clone()),
                        _ => None,
                    }
                });
                let deck = &mut self.lyrics.wave.decks[d];
                deck.id = id;
                deck.job = path.map(|path| {
                    let (tx, rx) = mpsc::channel();
                    let store = store();
                    std::thread::spawn(move || {
                        let _ = tx.send(lyrics::find_with(&path, store.as_deref()));
                    });
                    rx
                });
                model.set_row_data(d, DeckLines::default());
            }
            let deck = &mut self.lyrics.wave.decks[d];
            if let Some(found) = deck.job.as_ref().and_then(|rx| rx.try_recv().ok()) {
                deck.job = None;
                let lines = found.map(|l| wave_lines(&l.lines)).unwrap_or_default();
                model.set_row_data(
                    d,
                    DeckLines {
                        lines: ModelRc::new(VecModel::from(lines)),
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_run_until_sung() {
        let lines = lyrics::parse("[00:10.00]Hi there\n[00:11.00]\n[00:20.00]Next").unwrap();
        let w = wave_lines(&lines);
        assert_eq!(w.len(), 2, "breaks have no strip");
        assert_eq!(w[0].start, 10.0);
        // Ends by the next line's start.
        assert!(w[0].end <= 11.0 && w[0].end > 10.0);
        let words: Vec<WaveWord> = w[0].words.iter().collect();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hi");
        assert_eq!(words[0].start, 10.0);
        assert!(words[1].start > 10.0 && words[1].start < words[1].end);
        assert!(lyrics::parse("Plain")
            .map(|l| wave_lines(&l))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn words_sit_at_their_own_times() {
        let text = "[00:10.00]<00:10.00>Too <00:10.40>many <00:11.20>fathers<00:30.00>";
        let w = wave_lines(&lyrics::parse(text).unwrap());
        let words: Vec<WaveWord> = w[0].words.iter().collect();
        let starts: Vec<f32> = words.iter().map(|w| w.start).collect();
        assert_eq!(starts, vec![10.0, 10.4, 11.2]);
        assert_eq!(words[0].end, 10.4);
        // The last word isn't held across the long gap the line ends on.
        assert!(words[2].end < 14.0, "{}", words[2].end);
        assert_eq!(w[0].end, words[2].end);
    }
}
