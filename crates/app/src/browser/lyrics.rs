//! The side panel's Lyrics tab: the lyrics of the track playing (or the
//! one selected, or a deck's), read from an `.lrc` file beside it or from
//! its tags (see [`djsugar_engine::lyrics`]). Timed lyrics follow the deck
//! the track is on: the line sung now is lit, and a click on a line plays
//! from it. Find looks the track up on LRCLIB when the DJ has turned that
//! on (see [`crate::lrclib`]) and saves the pick as an `.lrc` beside it.
//!
//! The same lyrics show large in Player mode when the Player bar's Lyrics
//! button is on, and on the karaoke screen (see [`karaoke`]); both always
//! follow the track playing.

mod karaoke;
mod timing;
pub(super) mod wave;

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::lyrics::{self, Lyrics, Origin, Timing};
use djsugar_engine::{deck_group, Source, Track, NUM_DECKS};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::lrclib::{self, Found, Miss, Query};
use crate::{settings, AppWindow, LyricLine, LyricMatch, LyricsUi};

/// The side panel's Lyrics tab. Past Prep's Cues (4) and Checklist (5),
/// which come and go with the layout.
pub(super) const LYRICS_TAB: usize = 6;

/// What the tab follows.
const PLAYING: i32 = 0;
const SELECTED: i32 = 1;
/// 2 and 3 are decks A and B.
const FIRST_DECK: i32 = 2;

/// What a look-up on LRCLIB gives back.
type Looked = Result<Vec<Found>, Miss>;

/// A deck's controls the tab reads and sets.
struct DeckPlay {
    play: ControlHandle,
    playposition: ControlHandle,
    duration: ControlHandle,
}

pub(super) struct LyricsState {
    subject: i32,
    decks: Vec<DeckPlay>,
    /// The deck the Playing choice last followed, kept while it plays on.
    playing: Option<usize>,
    /// The track shown, where it is in `library`, and the deck it is on.
    shown: Option<(usize, Arc<Track>)>,
    deck: Option<usize>,
    /// The lyrics being read for the track shown, by generation.
    job: Option<(u64, Receiver<Option<Lyrics>>)>,
    generation: u64,
    lyrics: Option<Lyrics>,
    current: i32,
    /// The current line split into sung and to come, as last shown.
    sung: (String, String),
    /// A look-up on LRCLIB running, by generation, and what it found.
    finding: Option<(u64, Receiver<Looked>)>,
    found: Vec<Found>,
    karaoke: karaoke::Karaoke,
    wave: wave::WaveState,
    /// Words being timed with Whisper (Time words).
    timing: Option<timing::Job>,
}

impl LyricsState {
    pub(super) fn new(registry: &ControlRegistry) -> Self {
        let decks = (0..NUM_DECKS)
            .map(|d| {
                let g = deck_group(d);
                let c = |k: &str| registry.get(&g, k).expect("deck control");
                DeckPlay {
                    play: c("play"),
                    playposition: c("playposition"),
                    duration: c("duration"),
                }
            })
            .collect();
        Self {
            subject: PLAYING,
            decks,
            playing: None,
            shown: None,
            deck: None,
            job: None,
            generation: 0,
            lyrics: None,
            current: -1,
            sung: Default::default(),
            finding: None,
            found: Vec::new(),
            karaoke: karaoke::Karaoke::new(registry),
            wave: Default::default(),
            timing: None,
        }
    }
}

/// m:ss for a line's time.
fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// What the tab says about where the lyrics came from.
fn describe(l: &Lyrics) -> String {
    let timing = match l.timing() {
        Timing::Words => "Timed by word",
        Timing::Lines => "Timed by line",
        Timing::Plain => "Not timed",
    };
    let from = match &l.origin {
        Origin::Lrc(p) => p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "an .lrc file".into()),
        Origin::Tag => "the file's tags".into(),
        Origin::Saved(_) => "saved by DJ Sugar".into(),
    };
    if l.auto {
        format!("Transcribed by DJ Sugar · {from}")
    } else {
        format!("{timing} · {from}")
    }
}

/// Where DJ Sugar keeps lyrics for tracks whose folder can't be written.
fn store() -> Option<std::path::PathBuf> {
    settings::config_dir().map(|d| d.join("lyrics"))
}

/// A row of the Find list.
fn match_row(f: &Found, duration: f64) -> LyricMatch {
    let mut detail = f.artist.clone();
    if !f.album.is_empty() {
        detail.push_str(" · ");
        detail.push_str(&f.album);
    }
    if f.duration > 0.0 {
        detail.push_str(" · ");
        detail.push_str(&clock(f.duration));
    }
    LyricMatch {
        title: f.title.clone().into(),
        detail: detail.into(),
        kind: if f.instrumental {
            2
        } else if f.is_synced() {
            0
        } else {
            1
        },
        fits: f.fits(duration),
    }
}

impl Browser {
    pub(super) fn install_lyrics(
        app: &AppWindow,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let ui = app.global::<LyricsUi>();
        let w = with.clone();
        ui.on_set_subject(move |s| {
            w(&|b| {
                b.lyrics.subject = s.clamp(PLAYING, FIRST_DECK + 1);
                if let Some(app) = b.app.upgrade() {
                    app.global::<LyricsUi>().set_subject(b.lyrics.subject);
                    b.poll_lyrics(&app, LYRICS_TAB);
                }
            })
        });
        let w = with.clone();
        ui.on_seek(move |line| w(&|b| b.seek_to_line(line)));
        let w = with.clone();
        ui.on_find(move || w(&|b| b.find_lyrics()));
        let w = with.clone();
        ui.on_use_found(move |i| w(&|b| b.use_found(i)));
        let w = with.clone();
        ui.on_close_found(move || w(&|b| b.close_found()));
        let w = with.clone();
        ui.on_enable_lookup(move || {
            crate::settings::set(lrclib::ENABLED, serde_json::json!(true));
            w(&|b| b.find_lyrics())
        });
        let w = with.clone();
        ui.on_time_words(move || w(&|b| b.time_words()));
        Self::install_karaoke(app, with);
    }

    /// The deck `index` (in `library`) is loaded on, the first if several.
    fn deck_with(&self, index: usize) -> Option<usize> {
        let id = self.library[index].id;
        (0..NUM_DECKS).find(|&d| self.loaded[d] == Some(id))
    }

    /// The track the tab is to show, and the deck it is on.
    fn lyrics_subject(&mut self, app: &AppWindow) -> (Option<usize>, Option<usize>) {
        // Player mode's large lyrics and the karaoke screen follow the
        // track playing.
        let ui = app.global::<LyricsUi>();
        let subject = if ui.get_big() || ui.get_karaoke() {
            PLAYING
        } else {
            self.lyrics.subject
        };
        match subject {
            PLAYING => {
                let playing =
                    |d: usize| self.loaded[d].is_some() && self.lyrics.decks[d].play.get_bool();
                // Automix's live deck; else stay with the deck followed while
                // it plays; else the first deck playing; else the last one
                // followed, paused; else the first deck with a track.
                let live = usize::try_from(app.get_automix_live())
                    .ok()
                    .filter(|&d| d < NUM_DECKS && playing(d));
                let deck = live
                    .or(self.lyrics.playing.filter(|&d| playing(d)))
                    .or_else(|| (0..NUM_DECKS).find(|&d| playing(d)))
                    .or(self.lyrics.playing.filter(|&d| self.loaded[d].is_some()))
                    .or_else(|| (0..NUM_DECKS).find(|&d| self.loaded[d].is_some()));
                self.lyrics.playing = deck;
                (deck.and_then(|d| self.deck_track(d)), deck)
            }
            SELECTED => {
                let index = usize::try_from(app.get_current_track())
                    .ok()
                    .and_then(|r| self.rows.get(r).copied().flatten());
                (index, index.and_then(|i| self.deck_with(i)))
            }
            d => {
                let deck = (d - FIRST_DECK) as usize;
                (self.deck_track(deck), Some(deck))
            }
        }
    }

    /// Keeps the Lyrics tab up to date; called every frame.
    pub(super) fn poll_lyrics(&mut self, app: &AppWindow, tab: usize) {
        self.poll_timing(app);
        let ui = app.global::<LyricsUi>();
        if tab != LYRICS_TAB && !ui.get_big() && !ui.get_karaoke() {
            return;
        }
        let (index, deck) = self.lyrics_subject(app);
        let now = index.map(|i| (i, self.library[i].clone()));
        let other = match (&now, &self.lyrics.shown) {
            (Some((i, t)), Some((j, u))) => i != j || t.id != u.id,
            (None, None) => false,
            _ => true,
        };
        if other {
            self.lyrics.shown = now.clone();
            self.lyrics.lyrics = None;
            self.lyrics.current = -1;
            self.lyrics.generation += 1;
            self.lyrics.job = None;
            self.close_found_ui(&ui);
            ui.set_time_note(SharedString::new());
            ui.set_can_time(false);
            ui.set_can_transcribe(false);
            ui.set_has_track(now.is_some());
            ui.set_can_find(
                now.as_ref()
                    .is_some_and(|(_, t)| matches!(t.source, Source::File(_))),
            );
            ui.set_title(
                now.as_ref()
                    .map(|(_, t)| {
                        if t.artist.is_empty() {
                            t.title.clone()
                        } else {
                            format!("{} · {}", t.artist, t.title)
                        }
                    })
                    .unwrap_or_default()
                    .into(),
            );
            ui.set_source(SharedString::new());
            ui.set_lines(ModelRc::default());
            ui.set_current(-1);
            if let Some((_, t)) = &now {
                if let Source::File(path) = &t.source {
                    let (tx, rx) = mpsc::channel();
                    let path = path.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(lyrics::find_with(&path, store().as_deref()));
                    });
                    self.lyrics.job = Some((self.lyrics.generation, rx));
                }
            }
        }
        if deck != self.lyrics.deck {
            self.lyrics.deck = deck;
        }
        if let Some((generation, rx)) = &self.lyrics.job {
            if let Ok(found) = rx.try_recv() {
                if *generation == self.lyrics.generation {
                    self.show_lyrics(&ui, found);
                }
                self.lyrics.job = None;
            }
        }
        // The line sung now, while the track is on a deck.
        let current = match (&self.lyrics.lyrics, deck) {
            (Some(l), Some(d)) => {
                let p = &self.lyrics.decks[d];
                l.line_at(p.playposition.get() * p.duration.get())
                    .map_or(-1, |i| i as i32)
            }
            _ => -1,
        };
        ui.set_can_seek(
            deck.is_some()
                && self
                    .lyrics
                    .lyrics
                    .as_ref()
                    .is_some_and(|l| l.timing() != Timing::Plain),
        );
        if current != self.lyrics.current {
            self.lyrics.current = current;
            ui.set_current(current);
        }
        // The current line, sung and to come, for the large view.
        let sung = match (&self.lyrics.lyrics, deck, usize::try_from(current)) {
            (Some(l), Some(d), Ok(i)) => {
                let p = &self.lyrics.decks[d];
                l.lines[i].sung_at(p.playposition.get() * p.duration.get())
            }
            _ => Default::default(),
        };
        if sung != self.lyrics.sung {
            ui.set_sung(sung.0.as_str().into());
            ui.set_to_come(sung.1.as_str().into());
            self.lyrics.sung = sung;
        }
        self.poll_karaoke(app);
        self.poll_find(&ui);
    }

    /// Looks the track shown up on LRCLIB, when the DJ has turned that on.
    fn find_lyrics(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<LyricsUi>();
        ui.set_finding(true);
        ui.set_found(ModelRc::default());
        self.lyrics.found.clear();
        // Off: the list opens to say so, with a button that turns it on.
        ui.set_lookup_off(!lrclib::enabled());
        if !lrclib::enabled() {
            ui.set_find_note(
                "Finding lyrics online is off. Turning it on sends the track's title, artist, \
                 album and length to LRCLIB (lrclib.net) when you press Find. It's also in \
                 Settings › Collection › Health."
                    .into(),
            );
            return;
        }
        let Some((_, t)) = &self.lyrics.shown else {
            ui.set_finding(false);
            return;
        };
        let q = Query {
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            duration: t.duration,
        };
        ui.set_find_note("Looking on LRCLIB…".into());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(lrclib::find(&q));
        });
        self.lyrics.finding = Some((self.lyrics.generation, rx));
    }

    fn poll_find(&mut self, ui: &LyricsUi) {
        let Some((generation, rx)) = &self.lyrics.finding else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        let current = *generation == self.lyrics.generation;
        self.lyrics.finding = None;
        if !current {
            return;
        }
        let duration = self.lyrics.shown.as_ref().map_or(0.0, |(_, t)| t.duration);
        match result {
            Ok(found) => {
                let rows: Vec<LyricMatch> = found.iter().map(|f| match_row(f, duration)).collect();
                ui.set_find_note(SharedString::new());
                ui.set_found(ModelRc::new(VecModel::from(rows)));
                self.lyrics.found = found;
            }
            Err(Miss::None) => ui.set_find_note("LRCLIB has no lyrics for this track.".into()),
            Err(Miss::Offline) => ui
                .set_find_note("Couldn't reach LRCLIB. Check the connection and try again.".into()),
        }
    }

    /// Saves the picked lyrics beside the track and shows them.
    fn use_found(&mut self, i: i32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<LyricsUi>();
        let Some(f) = usize::try_from(i)
            .ok()
            .and_then(|i| self.lyrics.found.get(i))
            .cloned()
        else {
            return;
        };
        let Some(Source::File(path)) = self.lyrics.shown.as_ref().map(|(_, t)| t.source.clone())
        else {
            return;
        };
        if f.instrumental || f.text().trim().is_empty() {
            ui.set_find_note("That one is marked instrumental: no words to save.".into());
            return;
        }
        match lyrics::save(&path, store().as_deref(), f.text()) {
            Ok(_) => {
                self.close_found_ui(&ui);
                self.lyrics.wave.reload();
                // Read again, from the file just written.
                self.lyrics.shown = None;
                self.poll_lyrics(&app, LYRICS_TAB);
            }
            Err(e) => ui.set_find_note(format!("Couldn't save the lyrics: {e}").into()),
        }
    }

    fn close_found(&mut self) {
        if let Some(app) = self.app.upgrade() {
            self.close_found_ui(&app.global::<LyricsUi>());
        }
    }

    fn close_found_ui(&mut self, ui: &LyricsUi) {
        self.lyrics.finding = None;
        self.lyrics.found.clear();
        ui.set_finding(false);
        ui.set_lookup_off(false);
        ui.set_find_note(SharedString::new());
        ui.set_found(ModelRc::default());
    }

    fn show_lyrics(&mut self, ui: &LyricsUi, found: Option<Lyrics>) {
        match &found {
            Some(l) => {
                let lines: Vec<LyricLine> = l
                    .lines
                    .iter()
                    .map(|line| LyricLine {
                        time: line.start.map(clock).unwrap_or_default().into(),
                        text: line.text.clone().into(),
                    })
                    .collect();
                ui.set_source(describe(l).into());
                ui.set_lines(ModelRc::new(VecModel::from(lines)));
            }
            None => {
                ui.set_source(SharedString::new());
                ui.set_lines(ModelRc::default());
            }
        }
        self.lyrics.current = -1;
        ui.set_current(-1);
        self.lyrics.lyrics = found;
        self.show_timing(ui);
    }

    /// Plays the deck the track is on from the start of `line`.
    fn seek_to_line(&mut self, line: i32) {
        let (Some(l), Some(d)) = (&self.lyrics.lyrics, self.lyrics.deck) else {
            return;
        };
        let Some(start) = usize::try_from(line)
            .ok()
            .and_then(|i| l.lines.get(i))
            .and_then(|l| l.start)
        else {
            return;
        };
        let p = &self.lyrics.decks[d];
        let duration = p.duration.get();
        if duration > 0.0 {
            p.playposition.set((start / duration).clamp(0.0, 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn clock_reads_as_minutes() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(61.9), "1:01");
        assert_eq!(clock(600.0), "10:00");
    }

    #[test]
    fn describes_where_from() {
        let lines = lyrics::parse("[00:01.00]<00:01.00>Hi <00:01.50>there").unwrap();
        let l = Lyrics {
            lines,
            origin: Origin::Lrc(PathBuf::from("/music/Song.lrc")),
            auto: false,
        };
        assert_eq!(describe(&l), "Timed by word · Song.lrc");
        let l = Lyrics {
            lines: lyrics::parse("Just words").unwrap(),
            origin: Origin::Tag,
            auto: false,
        };
        assert_eq!(describe(&l), "Not timed · the file's tags");
        let l = Lyrics {
            lines: lyrics::parse("[00:01.00]<00:01.00>Hi <00:01.50>there").unwrap(),
            origin: Origin::Lrc(PathBuf::from("/music/Song.lrc")),
            auto: true,
        };
        assert_eq!(describe(&l), "Transcribed by DJ Sugar · Song.lrc");
    }
}
