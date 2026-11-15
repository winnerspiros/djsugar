//! The karaoke screen: a window of its own showing the line to sing big,
//! the word being sung filling with lilac (spread over the line by length
//! when the lyrics are timed only by line), the next lines under it, and a
//! count-in of dots, one a beat, before a line after a break. It opens full
//! screen on another display when there is one, and follows the track
//! playing, as the Player's large lyrics do.
//!
//! While it is open the Lyrics tab has a Vocals slider: the vocal stem's
//! level on the deck followed. The level set there carries over to the next
//! track that deck, or Automix, plays.

use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::lyrics::{Line, Timing};
use djsugar_engine::stems::MAX_STEMS;
use djsugar_engine::{stem_group, NUM_DECKS};
use slint::winit_030::winit::window::Fullscreen;
use slint::winit_030::WinitWindowAccessor;
use slint::{CloseRequestResponse, ComponentHandle, SharedString};

use super::super::Browser;
use super::PLAYING;
use crate::{AppWindow, KaraokeWindow, LyricsUi};

/// Beats counted in before a line.
const COUNT_IN: f64 = 4.0;
/// Seconds a line stays up once sung, unless the next one starts first.
const LINGER: f64 = 1.5;
/// How long the guide vocal level is held on a newly followed track, so a
/// reset on load doesn't undo it.
const HOLD_GUIDE: Duration = Duration::from_millis(1500);

pub(super) struct Karaoke {
    window: Option<KaraokeWindow>,
    /// Each deck's stems' levels and its tempo.
    levels: Vec<Vec<ControlHandle>>,
    bpm: Vec<ControlHandle>,
    /// The guide vocal level the DJ set, held on each track followed.
    guide: Option<f64>,
    /// The deck and track followed, and until when the guide is held there.
    followed: Option<(usize, usize)>,
    hold_until: Option<Instant>,
    /// The karaoke page on the network (see [`crate::now_playing`]): served
    /// while the screen is open and the DJ turned it on; when it was last
    /// sent, and the lyrics it has.
    page: bool,
    page_sent: Option<Instant>,
    page_lines: Option<u64>,
}

/// The setting: the karaoke page, off at first.
const PAGE: &str = "karaoke_page";
/// How often the karaoke page is told where the track is; it keeps time
/// itself in between.
const PAGE_EVERY: Duration = Duration::from_millis(100);

/// Lyrics for the karaoke page: each line with its words' times.
fn page_lines(id: u64, lines: &[Line]) -> String {
    let lines: Vec<serde_json::Value> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let until = lines.get(i + 1).and_then(|n| n.start);
            let words: Vec<serde_json::Value> = l
                .word_times(until)
                .into_iter()
                .map(|(s, e, t)| serde_json::json!([s, e, t]))
                .collect();
            serde_json::json!({ "text": l.text, "start": l.start, "words": words })
        })
        .collect();
    serde_json::json!({ "id": id, "lines": lines }).to_string()
}

impl Karaoke {
    pub(super) fn new(registry: &ControlRegistry) -> Self {
        let levels = (0..NUM_DECKS)
            .map(|d| {
                (0..MAX_STEMS)
                    .map(|s| {
                        registry
                            .get(&stem_group(d, s), "volume")
                            .expect("stem level")
                    })
                    .collect()
            })
            .collect();
        let bpm = (0..NUM_DECKS)
            .map(|d| {
                registry
                    .get(&djsugar_engine::deck_group(d), "bpm")
                    .expect("deck bpm")
            })
            .collect();
        Self {
            window: None,
            levels,
            bpm,
            guide: None,
            followed: None,
            hold_until: None,
            page: false,
            page_sent: None,
            page_lines: None,
        }
    }
}

/// What the screen shows of the lyrics at `seconds`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Screen {
    pub before: String,
    pub sung: String,
    pub word: String,
    pub part: f64,
    pub rest: String,
    pub next: String,
    pub after: String,
    pub letters: usize,
    pub count_in: i32,
}

fn sung(line: &Line) -> bool {
    line.start.is_some() && !line.text.trim().is_empty()
}

/// The screen at `seconds` into timed `lines`, with a beat `beat` seconds
/// long: the line being sung, or else the next one to sing, counted in.
pub(super) fn screen(lines: &[Line], seconds: f64, beat: f64) -> Screen {
    let start_of = |i: usize| lines.get(i).and_then(|l| l.start);
    let next_sung = |from: usize| (from..lines.len()).find(|&i| sung(&lines[i]));
    let current = djsugar_engine::lyrics::line_at(lines, seconds);
    // The line being sung, while it is or just was.
    let singing = current.filter(|&i| {
        if !sung(&lines[i]) {
            return false;
        }
        let until = start_of(i + 1);
        let end = lines[i].word_times(until).last().map_or(seconds, |w| w.1);
        seconds < end + LINGER || until.is_some_and(|u| u - seconds < LINGER)
    });
    let focus = singing.or_else(|| next_sung(current.map_or(0, |i| i + 1)));
    let Some(focus) = focus else {
        // All sung: the last line stays up, faded.
        let before = current.and_then(|i| (0..=i).rev().find(|&j| sung(&lines[j])));
        return Screen {
            before: before
                .map(|i| lines[i].text.trim().to_string())
                .unwrap_or_default(),
            ..Default::default()
        };
    };
    let line = &lines[focus];
    let mut s = Screen {
        before: (0..focus)
            .rev()
            .find(|&j| sung(&lines[j]))
            .map(|i| lines[i].text.trim().to_string())
            .unwrap_or_default(),
        letters: line.text.trim().chars().count(),
        ..Default::default()
    };
    let next = next_sung(focus + 1);
    s.next = next
        .map(|i| lines[i].text.trim().to_string())
        .unwrap_or_default();
    s.after = next
        .and_then(|i| next_sung(i + 1))
        .map(|i| lines[i].text.trim().to_string())
        .unwrap_or_default();
    if singing.is_some() {
        let fill = line.fill_at(seconds, start_of(focus + 1));
        s.sung = fill.sung;
        s.word = fill.word;
        s.part = fill.part;
        s.rest = fill.rest;
    } else {
        s.rest = line.text.trim().to_string();
        let to_go = line.start.unwrap_or(seconds) - seconds;
        let beats = to_go / beat.max(0.1);
        if to_go > 0.0 && beats <= COUNT_IN {
            s.count_in = beats.ceil() as i32;
        }
    }
    s
}

/// Another display than the one `app` is on, when there is one.
fn other_display(app: &AppWindow) -> Option<slint::winit_030::winit::monitor::MonitorHandle> {
    app.window().with_winit_window(|w| {
        let here = w.current_monitor();
        w.available_monitors().find(|m| Some(m) != here.as_ref())
    })?
}

fn toggle_full_screen(k: &KaraokeWindow) {
    k.window().with_winit_window(|w| {
        if w.fullscreen().is_some() {
            w.set_fullscreen(None);
        } else {
            w.set_fullscreen(Some(Fullscreen::Borderless(w.current_monitor())));
        }
    });
}

impl Browser {
    pub(super) fn install_karaoke(
        app: &AppWindow,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let ui = app.global::<LyricsUi>();
        let w = with.clone();
        ui.on_set_karaoke(move |on| w(&|b| b.set_karaoke(on)));
        let w = with.clone();
        ui.on_set_vocals(move |level| w(&|b| b.set_vocals(level as f64)));
        let w = with.clone();
        ui.on_set_page(move |on| {
            crate::settings::set(PAGE, serde_json::json!(on));
            w(&|b| b.serve_page(on))
        });
        // Closing DJ Sugar closes the karaoke screen too, or the app would run
        // on behind it.
        let w = with.clone();
        app.window().on_close_requested(move || {
            w(&|b| b.set_karaoke(false));
            CloseRequestResponse::HideWindow
        });
    }

    fn set_karaoke(&mut self, on: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<LyricsUi>();
        ui.set_karaoke(on);
        let k = &mut self.lyrics.karaoke;
        if !on {
            if let Some(w) = k.window.take() {
                let _ = w.hide();
            }
            k.guide = None;
            k.followed = None;
            ui.set_vocals(-1.0);
            self.serve_page(false);
            return;
        }
        if k.window.is_some() {
            return;
        }
        let Ok(window) = KaraokeWindow::new() else {
            ui.set_karaoke(false);
            return;
        };
        window.on_toggle_full_screen({
            let weak = window.as_weak();
            move || {
                if let Some(k) = weak.upgrade() {
                    toggle_full_screen(&k);
                }
            }
        });
        window.on_leave_full_screen({
            let weak = window.as_weak();
            move || {
                if let Some(k) = weak.upgrade() {
                    k.window().with_winit_window(|w| w.set_fullscreen(None));
                }
            }
        });
        let weak_app = app.as_weak();
        window.window().on_close_requested(move || {
            if let Some(app) = weak_app.upgrade() {
                // Through the tab's switch, so the browser lets it go.
                app.global::<LyricsUi>().invoke_set_karaoke(false);
            }
            CloseRequestResponse::HideWindow
        });
        if window.show().is_err() {
            ui.set_karaoke(false);
            return;
        }
        // Full screen on a second display; a window here when there's one.
        if let Some(display) = other_display(&app) {
            window.window().with_winit_window(|w| {
                w.set_fullscreen(Some(Fullscreen::Borderless(Some(display))))
            });
        }
        k.window = Some(window);
        k.followed = None;
        self.serve_page(crate::settings::flag(PAGE, false));
        // The screen follows the track playing; so does the tab meanwhile.
        self.lyrics.subject = PLAYING;
        ui.set_subject(PLAYING);
        self.poll_lyrics(&app, super::LYRICS_TAB);
    }

    /// Serves the karaoke page, or stops.
    fn serve_page(&mut self, on: bool) {
        let k = &mut self.lyrics.karaoke;
        k.page = on;
        k.page_sent = None;
        k.page_lines = None;
        crate::now_playing::set_karaoke_page(on);
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<LyricsUi>();
            ui.set_page(on);
            ui.set_page_url(SharedString::new());
        }
    }

    /// Tells the karaoke page the track, where it is, and its lyrics when
    /// they change.
    fn publish_page(&mut self, state: i32, deck: Option<usize>) {
        if let Some(app) = self.app.upgrade() {
            // The address comes once the server has started.
            let url = crate::now_playing::karaoke_url();
            let ui = app.global::<LyricsUi>();
            if ui.get_page_url() != url.as_str() {
                ui.set_page_url(url.into());
            }
        }
        let k = &mut self.lyrics.karaoke;
        if k.page_sent.is_some_and(|at| at.elapsed() < PAGE_EVERY) {
            return;
        }
        k.page_sent = Some(Instant::now());
        let generation = self.lyrics.generation;
        let lines = match &self.lyrics.lyrics {
            Some(l) if state == 3 && k.page_lines != Some(generation) => {
                k.page_lines = Some(generation);
                Some(page_lines(generation, &l.lines))
            }
            _ => None,
        };
        let (title, artist) = self
            .lyrics
            .shown
            .as_ref()
            .map(|(_, t)| (t.title.clone(), t.artist.clone()))
            .unwrap_or_default();
        let (seconds, playing, bpm) = deck.map_or((0.0, false, 0.0), |d| {
            let p = &self.lyrics.decks[d];
            (
                p.playposition.get() * p.duration.get(),
                p.play.get_bool(),
                k.bpm[d].get(),
            )
        });
        let now = serde_json::json!({
            // The lyrics the page should have; none until they are sent.
            "id": k.page_lines.filter(|&g| g == generation && state == 3),
            "state": state,
            "title": title,
            "artist": artist,
            "t": seconds,
            "playing": playing,
            "bpm": bpm,
        });
        djsugar_remote::overlay::set_karaoke(now.to_string(), lines);
    }

    /// The vocal stem of `deck`'s track: the one named so, or the fourth,
    /// where separated stems put the voice.
    fn vocal_stem(&self, deck: usize) -> usize {
        let loaded = self.engine.loaded_tracks();
        let loaded = loaded.read().ok();
        loaded
            .as_ref()
            .and_then(|l| l[deck].clone())
            .and_then(|t| {
                let stems = t.audio.as_ref()?.stems()?.clone();
                stems
                    .info
                    .iter()
                    .position(|i| i.name.to_lowercase().contains("vocal"))
            })
            .unwrap_or(3)
            .min(MAX_STEMS - 1)
    }

    fn set_vocals(&mut self, level: f64) {
        let level = level.clamp(0.0, 1.0);
        let Some(deck) = self.lyrics.deck else {
            return;
        };
        let stem = self.vocal_stem(deck);
        self.lyrics.karaoke.levels[deck][stem].set(level);
        self.lyrics.karaoke.guide = Some(level);
        if let Some(app) = self.app.upgrade() {
            app.global::<LyricsUi>().set_vocals(level as f32);
        }
    }

    /// Keeps the karaoke screen and the Vocals slider up to date; called
    /// from `poll_lyrics` each frame while the screen is open.
    pub(super) fn poll_karaoke(&mut self, app: &AppWindow) {
        if self.lyrics.karaoke.window.is_none() {
            return;
        }
        let deck = self.lyrics.deck;
        let track = self.lyrics.shown.as_ref().map(|(i, _)| *i);
        // The guide vocal, held on each new track followed.
        let followed = deck.zip(track);
        let stem = deck.map(|d| self.vocal_stem(d));
        let k = &mut self.lyrics.karaoke;
        if followed != k.followed {
            k.followed = followed;
            k.hold_until = Some(Instant::now() + HOLD_GUIDE);
        }
        if let (Some(d), Some(s)) = (deck, stem) {
            let level = &k.levels[d][s];
            if let (Some(g), Some(until)) = (k.guide, k.hold_until) {
                if Instant::now() < until && (level.get() - g).abs() > 1e-3 {
                    level.set(g);
                }
            }
            app.global::<LyricsUi>()
                .set_vocals(level.get().clamp(0.0, 1.0) as f32);
        } else {
            app.global::<LyricsUi>().set_vocals(-1.0);
        }
        let state = match (&self.lyrics.shown, &self.lyrics.lyrics) {
            (None, _) => 0,
            (Some(_), None) => 1,
            (Some(_), Some(l)) if l.timing() == Timing::Plain => 2,
            _ => 3,
        };
        if self.lyrics.karaoke.page {
            self.publish_page(state, deck);
        }
        let k = &self.lyrics.karaoke;
        let Some(window) = &k.window else {
            return;
        };
        if let Some((_, t)) = &self.lyrics.shown {
            window.set_song(t.title.as_str().into());
            window.set_artist(t.artist.as_str().into());
        }
        window.set_state(state);
        let (Some(l), Some(d), 3) = (&self.lyrics.lyrics, deck, state) else {
            return;
        };
        let p = &self.lyrics.decks[d];
        let seconds = p.playposition.get() * p.duration.get();
        let bpm = k.bpm[d].get();
        let beat = if bpm > 0.0 { 60.0 / bpm } else { 0.5 };
        let s = screen(&l.lines, seconds, beat);
        window.set_before(s.before.into());
        window.set_sung(s.sung.into());
        window.set_word(s.word.into());
        window.set_part(s.part as f32);
        window.set_rest(s.rest.into());
        window.set_next(s.next.into());
        window.set_after(s.after.into());
        window.set_letters(s.letters as i32);
        window.set_count_in(s.count_in);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::lyrics::parse;

    #[test]
    fn page_lines_carry_word_times() {
        let lines = parse("[00:10.00]Hi there\n[00:12.00]<00:12.00>Next <00:12.50>one").unwrap();
        let v: serde_json::Value = serde_json::from_str(&page_lines(7, &lines)).unwrap();
        assert_eq!(v["id"], 7);
        assert_eq!(v["lines"][0]["text"], "Hi there");
        assert_eq!(v["lines"][0]["start"], 10.0);
        assert_eq!(v["lines"][0]["words"][0][2], "Hi ");
        assert_eq!(v["lines"][1]["words"][1][0], 12.5);
        let plain = parse("Just words").unwrap();
        let v: serde_json::Value = serde_json::from_str(&page_lines(1, &plain)).unwrap();
        assert!(v["lines"][0]["start"].is_null());
    }

    #[test]
    fn counts_in_then_sings() {
        let lines =
            parse("[00:10.00]First line here\n[00:12.00]Second line\n[00:20.00]\n[00:30.00]Third")
                .unwrap();
        // Long before: the first line waits, with no count yet.
        let s = screen(&lines, 2.0, 0.5);
        assert_eq!(s.rest, "First line here");
        assert_eq!(
            (s.count_in, s.next.as_str(), s.after.as_str()),
            (0, "Second line", "Third")
        );
        // Four beats of half a second before it: four dots, then one.
        assert_eq!(screen(&lines, 8.0, 0.5).count_in, 4);
        assert_eq!(screen(&lines, 9.8, 0.5).count_in, 1);
        // Sung: the word being sung fills.
        let s = screen(&lines, 10.2, 0.5);
        assert_eq!((s.sung.as_str(), s.word.as_str()), ("", "First"));
        assert!(s.part > 0.0 && s.count_in == 0);
        assert_eq!(s.letters, "First line here".len());
        // After the break at 0:20 the third line is counted in.
        let s = screen(&lines, 28.5, 0.5);
        assert_eq!(
            (s.before.as_str(), s.rest.as_str(), s.count_in),
            ("Second line", "Third", 3)
        );
        // All sung.
        let s = screen(&lines, 60.0, 0.5);
        assert_eq!((s.before.as_str(), s.rest.as_str()), ("Third", ""));
    }
}
