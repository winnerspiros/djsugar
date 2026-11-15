//! The scrolling waveform of a deck playing a live source (an input, an
//! instrument plugin or the Groove Box): the deck's sound as it comes in,
//! drawn up to the play line, on the leading deck's beat grid.

use std::collections::VecDeque;
use std::time::Instant;

use djsugar_engine::live_wave::LiveSlice;
use djsugar_engine::track::{Track, Waveform, WaveformBin, WAVEFORM_RATE};
use djsugar_engine::NUM_DECKS;
use djsugar_waveform::{
    GridSection, Lanes, Mark, MarkKind, PhraseKind, GRID_SECTIONS, MAX_MARKS, MAX_PHRASES,
};

use crate::smooth::SmoothClock;

/// Slices drawn: about 8 s, the half of the waveform behind the play line
/// with room to spare. A power of two, so each coarser level halves it.
pub const WINDOW: usize = 8192;

/// The bands carry less energy than the whole signal; lifted as a track's
/// waveform lifts them (see the analysis), so the colours match.
const BAND_GAIN: [f32; 4] = [1.0, 1.4, 2.0, 1.0];

/// A slice as a waveform bin: the square root of each level against full
/// scale, from 0 to 255, as a track's bins are against its loudest peak.
pub fn bin(s: &LiveSlice) -> WaveformBin {
    let q = |v: f32, k: usize| ((v * BAND_GAIN[k]).clamp(0.0, 1.0).sqrt() * 255.0).round() as u8;
    std::array::from_fn(|i| {
        if i < 4 {
            q(s.peak[i], i)
        } else {
            q(s.rms[i - 4], i - 4)
        }
    })
}

/// Seconds of the note roll on either side of the play line whose notes set
/// the rows: half the scrolling waveform at normal speed.
const ROLL_SECONDS: f64 = 5.0;
/// The fewest rows the note roll shows, so a few notes don't fill the lane.
const MIN_ROWS: u32 = 8;
/// How opaque the sound is under the note roll.
const UNDER_NOTES: f32 = 0.3;
/// Held notes are outlined in Electric Lilac.
const LILAC: [f32; 4] = [0.714, 0.612, 1.0, 1.0];

/// One deck's sound so far.
#[derive(Default)]
struct Trace {
    /// The latest bins, oldest first; at most a window and a little more.
    bins: VecDeque<WaveformBin>,
    /// Slices received since the deck went live.
    received: u64,
    clock: SmoothClock,
    notes: NoteRoll,
}

/// One note on the roll, in seconds on the deck's live clock.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Note {
    note: u8,
    start: f64,
    /// None while it is held.
    end: Option<f64>,
}

/// The notes an instrument deck plays, as they start and end.
#[derive(Default)]
struct NoteRoll {
    held: u128,
    notes: VecDeque<Note>,
    /// The scale now, a bit per pitch class from C (0 for none): its rows
    /// are tinted.
    scale: u16,
}

impl NoteRoll {
    /// Notes now held (bit `n` for MIDI note `n`) at `at` on the deck's clock.
    fn update(&mut self, held: u128, at: f64) {
        let released = self.held & !held;
        let pressed = held & !self.held;
        self.held = held;
        for n in self.notes.iter_mut().filter(|n| n.end.is_none()) {
            if released >> n.note & 1 == 1 {
                n.end = Some(at);
            }
        }
        for note in (0..128u8).filter(|&n| pressed >> n & 1 == 1) {
            self.notes.push_back(Note {
                note,
                start: at,
                end: None,
            });
        }
        // Notes let go before the window are gone from it.
        let keep = at - WINDOW as f64 / WAVEFORM_RATE;
        while self
            .notes
            .front()
            .is_some_and(|n| n.end.is_some_and(|e| e < keep))
        {
            self.notes.pop_front();
        }
    }

    /// The notes as lanes over a window that starts at `start` on the
    /// deck's clock, with the play line at `at`; none until a note shows.
    fn lanes(&self, start: f64, at: f64, color: [f32; 4]) -> Option<Lanes> {
        let shown = || {
            self.notes
                .iter()
                .filter(move |n| n.end.unwrap_or(at) >= at - ROLL_SECONDS)
        };
        let lo = shown().map(|n| n.note).min()? as i32;
        let hi = shown().map(|n| n.note).max()? as i32;
        // Rows for the notes in view, centred in at least MIN_ROWS.
        let count = ((hi - lo + 1) as u32).max(MIN_ROWS);
        let bottom = lo - (count as i32 - (hi - lo + 1)) / 2;
        let mut marks: Vec<Mark> = shown()
            .filter(|n| n.note as i32 >= bottom && (n.note as i32) < bottom + count as i32)
            .map(|n| Mark {
                start: n.start - start,
                end: n.end.unwrap_or(at) - start,
                lane: (n.note as i32 - bottom) as u32,
                kind: if n.end.is_some() {
                    MarkKind::Played
                } else {
                    MarkKind::Held
                },
            })
            .collect();
        // The latest, if there are more than the shader draws.
        if marks.len() > MAX_MARKS {
            marks.drain(..marks.len() - MAX_MARKS);
        }
        let tinted = (0..count.min(128))
            .filter(|&r| self.scale >> (bottom + r as i32).rem_euclid(12) & 1 == 1)
            .fold(0u128, |t, r| t | 1 << r);
        Some(Lanes {
            count,
            tinted,
            marks,
            trace: UNDER_NOTES,
            color,
            lit: LILAC,
        })
    }
}

/// What a live deck's scrolling waveform draws this frame.
pub struct Window {
    /// The waveform: [`WINDOW`] slices ending at the play line, and the
    /// coarser levels made from them.
    pub levels: Vec<Vec<WaveformBin>>,
    /// The play line, in seconds into the window.
    pub play_seconds: f64,
    /// The play line on the deck's live clock: seconds since it went live.
    pub clock: f64,
    /// An instrument deck's note roll, once it has notes in view.
    pub lanes: Option<Lanes>,
}

/// The decks' live sound, read from the engine on every frame.
pub struct LiveTraces {
    rx: [rtrb::Consumer<LiveSlice>; NUM_DECKS],
    decks: [Trace; NUM_DECKS],
}

impl LiveTraces {
    pub fn new(rx: [rtrb::Consumer<LiveSlice>; NUM_DECKS]) -> Self {
        Self {
            rx,
            decks: Default::default(),
        }
    }

    /// Takes what came in. `live[d]` says whether deck `d` plays live now;
    /// a deck that doesn't is cleared, so its next live part starts afresh.
    pub fn poll(&mut self, live: [bool; NUM_DECKS]) {
        for (d, rx) in self.rx.iter_mut().enumerate() {
            let trace = &mut self.decks[d];
            if !live[d] {
                while rx.pop().is_ok() {}
                if trace.received > 0 {
                    *trace = Trace::default();
                }
                continue;
            }
            while let Ok(s) = rx.pop() {
                trace.push(bin(&s));
            }
        }
    }

    /// Deck `d`'s waveform to draw at `now`, once anything has come in.
    /// `held`: the notes an instrument deck holds now (bit `n` for MIDI
    /// note `n`), for its note roll in `color`; None on other decks.
    /// The scale an instrument deck's note roll tints, a bit per pitch
    /// class from C (0 for none).
    pub fn set_scale(&mut self, d: usize, scale: u16) {
        self.decks[d].notes.scale = scale;
    }

    pub fn window(
        &mut self,
        d: usize,
        now: Instant,
        held: Option<u128>,
        color: [f32; 4],
    ) -> Option<Window> {
        self.decks[d].window(now, held, color)
    }
}

impl Trace {
    fn push(&mut self, b: WaveformBin) {
        self.bins.push_back(b);
        self.received += 1;
        // Room for the play line to lag what came in by a buffer or two.
        while self.bins.len() > WINDOW + 1024 {
            self.bins.pop_front();
        }
    }

    fn window(&mut self, now: Instant, held: Option<u128>, color: [f32; 4]) -> Option<Window> {
        if self.received == 0 {
            return None;
        }
        // Slices come a buffer at a time; the play line moves on smoothly
        // between them and draws only what it has passed.
        let came = self.received as f64 / WAVEFORM_RATE;
        let shown = self.clock.next(came, 1.0, now).min(came);
        let end = (shown * WAVEFORM_RATE).floor().max(0.0) as u64;
        // The bins up to the play line, the newest last.
        let kept = self.bins.len() as u64;
        let first_held = self.received - kept;
        let upto = (end.max(first_held) - first_held) as usize;
        let mut finest = vec![[0u8; 8]; WINDOW];
        let take = upto.min(WINDOW);
        for (i, b) in self.bins.range(upto - take..upto).enumerate() {
            finest[WINDOW - take + i] = *b;
        }
        let into = (shown * WAVEFORM_RATE - end as f64).max(0.0) / WAVEFORM_RATE;
        let play_seconds = WINDOW as f64 / WAVEFORM_RATE + into;
        let lanes = match held {
            Some(held) => {
                self.notes.update(held, shown);
                self.notes.lanes(shown - play_seconds, shown, color)
            }
            None => {
                self.notes = NoteRoll::default();
                None
            }
        };
        Some(Window {
            levels: Waveform::from_finest(WAVEFORM_RATE, finest).levels,
            play_seconds,
            clock: shown,
            lanes,
        })
    }
}

/// The leading deck's beat grid moved onto a live deck's window, so the live
/// part's beats line up with it. The leader is at beat `beat` (beat 0 a bar
/// line), at `bpm` as its grid says there, playing at `speed` times that;
/// the live deck's play line is `play_seconds` into its window.
pub fn leader_grid(
    bpm: f64,
    beat: f64,
    speed: f64,
    play_seconds: f64,
) -> [Option<GridSection>; GRID_SECTIONS] {
    let mut g = [None; GRID_SECTIONS];
    let bpm = bpm * speed;
    if bpm > 0.0 && speed > 0.0 {
        g[0] = Some(GridSection {
            seconds: play_seconds - beat * 60.0 / bpm,
            bpm,
            beat: 0.0,
        });
    }
    g
}

/// [`leader_grid`] for a leading track at `seconds`.
pub fn track_grid(
    leader: &Track,
    seconds: f64,
    speed: f64,
    play_seconds: f64,
) -> [Option<GridSection>; GRID_SECTIONS] {
    leader_grid(
        leader.bpm_at(seconds),
        leader.beat_at(seconds),
        speed,
        play_seconds,
    )
}

/// The leading track's phrases moved onto a live deck's window, as
/// [`track_grid`] moves its beats: each phrase starts as many beats from the
/// live play line as it is from the leader's position, at the leader's
/// tempo now. Phrases over before the window's left edge are left out, so
/// the ones in view are among the first [`MAX_PHRASES`] the shader draws.
pub fn track_phrases(
    leader: &Track,
    seconds: f64,
    speed: f64,
    play_seconds: f64,
    phrases: &[(f64, PhraseKind)],
) -> Vec<(f64, PhraseKind)> {
    let bpm = leader.bpm_at(seconds) * speed;
    if bpm <= 0.0 || speed <= 0.0 {
        return Vec::new();
    }
    let now = leader.beat_at(seconds);
    let moved: Vec<_> = phrases
        .iter()
        .map(|&(start, kind)| {
            let beats = leader.beat_at(start) - now;
            (play_seconds + beats * 60.0 / bpm, kind)
        })
        .collect();
    // The phrase under the window's left edge, and every one after it.
    let left = play_seconds - crate::VISIBLE_SECONDS;
    let first = moved.iter().rposition(|&(s, _)| s <= left).unwrap_or(0);
    moved.into_iter().skip(first).take(MAX_PHRASES).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn traces() -> (LiveTraces, [rtrb::Producer<LiveSlice>; NUM_DECKS]) {
        let mut tx = Vec::new();
        let mut rx = Vec::new();
        for _ in 0..NUM_DECKS {
            let (p, c) = rtrb::RingBuffer::new(WINDOW * 2);
            tx.push(p);
            rx.push(c);
        }
        (
            LiveTraces::new(rx.try_into().ok().unwrap()),
            tx.try_into().ok().unwrap(),
        )
    }

    fn loud() -> LiveSlice {
        LiveSlice {
            peak: [0.5; 4],
            rms: [0.25; 4],
        }
    }

    #[test]
    fn full_scale_is_the_top_of_the_waveform() {
        let full = LiveSlice {
            peak: [1.0; 4],
            rms: [1.0; 4],
        };
        assert_eq!(bin(&full), [255; 8]);
        assert_eq!(bin(&LiveSlice::default()), [0; 8]);
        // A quarter of full scale reads half way up, as on a track.
        assert_eq!(bin(&loud())[7], 128);
    }

    #[test]
    fn the_window_ends_at_the_play_line() {
        let (mut live, mut tx) = traces();
        for _ in 0..500 {
            tx[1].push(loud()).unwrap();
        }
        live.poll([false, true, false, false]);
        let w = live.window(1, Instant::now(), None, [1.0; 4]).unwrap();
        assert_eq!(w.levels[0].len(), WINDOW);
        assert!((w.play_seconds - WINDOW as f64 / WAVEFORM_RATE).abs() < 1e-9);
        // Half a second of sound at the end, silence before it.
        assert_ne!(w.levels[0][WINDOW - 1], [0; 8]);
        assert_ne!(w.levels[0][WINDOW - 500], [0; 8]);
        assert_eq!(w.levels[0][WINDOW - 501], [0; 8]);
        assert!(live.window(0, Instant::now(), None, [1.0; 4]).is_none());
    }

    #[test]
    fn a_deck_that_stops_playing_live_is_cleared() {
        let (mut live, mut tx) = traces();
        tx[0].push(loud()).unwrap();
        live.poll([true, false, false, false]);
        assert!(live.window(0, Instant::now(), None, [1.0; 4]).is_some());
        tx[0].push(loud()).unwrap();
        live.poll([false; NUM_DECKS]);
        assert!(live.window(0, Instant::now(), None, [1.0; 4]).is_none());
    }

    #[test]
    fn the_play_line_moves_on_smoothly_and_never_past_the_sound() {
        let (mut live, mut tx) = traces();
        let start = Instant::now();
        let mut last = 0.0;
        // Buffers of 10 ms, drawn at 60 frames a second.
        let mut sent = 0;
        for frame in 0..120 {
            let now = start + Duration::from_secs_f64(frame as f64 / 60.0);
            while (sent as f64) < (frame as f64 / 60.0) * 1000.0 {
                for _ in 0..10 {
                    tx[2].push(loud()).unwrap();
                }
                sent += 10;
            }
            live.poll([false, false, true, false]);
            if let Some(w) = live.window(2, now, None, [1.0; 4]) {
                // Whatever is drawn ends at the play line.
                let into = w.play_seconds - WINDOW as f64 / WAVEFORM_RATE;
                assert!((0.0..0.001).contains(&into), "frame {frame}: {into}");
                let shown = live.decks[2].received as f64; // slices
                assert!(shown >= last);
                last = shown;
            }
        }
    }

    #[test]
    fn the_leaders_beats_line_up_on_the_live_deck() {
        // The leader is on beat 20, a bar line: so is the live play line.
        let g = leader_grid(120.0, 20.0, 1.0, 8.192)[0].unwrap();
        assert_eq!(g.bpm, 120.0);
        let beats_at_play = (8.192 - g.seconds) * g.bpm / 60.0;
        assert!((beats_at_play - 20.0).abs() < 1e-9);
        // Sped up, the beats come faster.
        let fast = leader_grid(120.0, 20.0, 1.08, 8.192)[0].unwrap();
        assert!((fast.bpm - 129.6).abs() < 1e-9);
        // No tempo, no grid.
        assert!(leader_grid(0.0, 20.0, 1.0, 8.192)[0].is_none());
    }

    #[test]
    fn notes_play_on_rows_fitted_to_the_range() {
        let mut roll = NoteRoll::default();
        let bit = |n: u8| 1u128 << n;
        // C4 at 1 s, E4 joins at 1.5 s, C4 let go at 2 s; drawn at 2.5 s.
        roll.update(bit(60), 1.0);
        roll.update(bit(60) | bit(64), 1.5);
        roll.update(bit(64), 2.0);
        let lanes = roll.lanes(0.0, 2.5, [1.0; 4]).unwrap();
        // Five notes' range, centred in eight rows: C4 on row 1.
        assert_eq!(lanes.count, MIN_ROWS);
        assert_eq!(lanes.trace, UNDER_NOTES);
        let c = lanes.marks[0];
        assert_eq!(
            (c.start, c.end, c.lane, c.kind),
            (1.0, 2.0, 1, MarkKind::Played)
        );
        let e = lanes.marks[1];
        assert_eq!(
            (e.start, e.end, e.lane, e.kind),
            (1.5, 2.5, 5, MarkKind::Held)
        );
        // Two octaves apart, a row each.
        roll.update(bit(64) | bit(88), 3.0);
        assert_eq!(roll.lanes(0.0, 3.0, [1.0; 4]).unwrap().count, 29);
    }

    #[test]
    fn the_scale_rows_are_tinted() {
        let mut roll = NoteRoll::default();
        roll.update(1 << 60 | 1 << 64, 1.0);
        assert_eq!(roll.lanes(0.0, 2.0, [1.0; 4]).unwrap().tinted, 0);
        // C major; rows from B3 (C4 on row 1): B C . D . E F .
        roll.scale = 0b1010_1011_0101;
        let lanes = roll.lanes(0.0, 2.0, [1.0; 4]).unwrap();
        assert_eq!(lanes.tinted, 0b0110_1011);
    }

    #[test]
    fn no_notes_in_view_shows_the_sound_alone() {
        let mut roll = NoteRoll::default();
        assert!(roll.lanes(0.0, 1.0, [1.0; 4]).is_none());
        roll.update(1 << 60, 1.0);
        roll.update(0, 1.2);
        // Long gone from view.
        assert!(roll
            .lanes(0.0, 1.2 + ROLL_SECONDS + 1.0, [1.0; 4])
            .is_none());
        // And from memory once past the window.
        roll.update(0, 20.0);
        assert!(roll.notes.is_empty());
    }

    /// The leader's phrases land on the live deck the same beats away from
    /// its play line, at the leader's speed; long-gone ones are left out.
    #[test]
    fn moves_the_leaders_phrases_onto_the_live_window() {
        let mut t = (*djsugar_engine::sample_library()[0]).clone();
        t.bpm = 120.0;
        t.first_beat = 0.0;
        t.grid = Default::default();
        t.duration = 600.0;
        let phrases = [
            (0.0, PhraseKind::Intro),
            (32.0, PhraseKind::Build),
            (64.0, PhraseKind::Drop),
            (96.0, PhraseKind::Outro),
        ];
        // The leader is 4 s (8 beats) before its drop; the live play line
        // is 8 s into its window.
        let moved = track_phrases(&t, 60.0, 1.0, 8.0, &phrases);
        assert_eq!(moved[0], (-20.0, PhraseKind::Build));
        assert_eq!(moved[1], (12.0, PhraseKind::Drop));
        assert_eq!(moved.len(), 3, "the intro, long over, is left out");
        // Played 10% faster, the 8 beats pass sooner.
        let fast = track_phrases(&t, 60.0, 1.1, 8.0, &phrases);
        assert!((fast[1].0 - (8.0 + 4.0 / 1.1)).abs() < 1e-9);
        assert!(track_phrases(&t, 60.0, 0.0, 8.0, &phrases).is_empty());
    }
}
