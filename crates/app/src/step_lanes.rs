//! A Groove Box deck's scrolling waveform as step lanes: its eight tracks
//! as rows, the hits it played behind the play line and the hits to come
//! ahead of it, fills marked.
//!
//! The hits played come from the engine with the beat each fell on; the
//! ones to come are worked out from the patterns, the groove waiting for
//! the next bar and the fills, as the Groove Box will play them.

use std::collections::VecDeque;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::groove::{fill_seed, groove_group, GrooveHit, GROOVES, ROWS, STEPS};
use djsugar_engine::groove_fills::{auto_fill, held_fill};
use djsugar_engine::NUM_DECKS;
use djsugar_waveform::{Lanes, Mark, MarkKind, MAX_MARKS};

/// How opaque the sound is under the lanes.
const UNDER_STEPS: f32 = 0.3;
/// A hit is drawn this much of a step long.
const HIT_LENGTH: f64 = 0.6;
/// Hits are kept this long behind the play line, in seconds.
const KEEP_SECONDS: f64 = 10.0;
/// Fills are drawn in Electric Lilac.
const LILAC: [f32; 4] = [0.714, 0.612, 1.0, 1.0];

/// The Groove Box controls the lanes read, for one deck.
pub struct GrooveHandles {
    beat: ControlHandle,
    bpm: ControlHandle,
    position: ControlHandle,
    playing: ControlHandle,
    queued: ControlHandle,
    auto_fill: ControlHandle,
    fill: ControlHandle,
    patterns: [[ControlHandle; ROWS]; GROOVES],
    mute: [ControlHandle; ROWS],
    solo: [ControlHandle; ROWS],
    seed: u32,
}

impl GrooveHandles {
    pub fn new(reg: &ControlRegistry, deck: usize) -> Self {
        let g = groove_group(deck);
        let c = |k: &str| {
            reg.get(&g, k)
                .unwrap_or_else(|| panic!("missing control {g},{k}"))
        };
        Self {
            beat: c("beat"),
            bpm: c("bpm"),
            position: c("position"),
            playing: c("groove_playing"),
            queued: c("groove_queued"),
            auto_fill: c("auto_fill"),
            fill: c("fill"),
            patterns: std::array::from_fn(|p| {
                std::array::from_fn(|r| c(&format!("pattern{}_row{}", p + 1, r + 1)))
            }),
            mute: std::array::from_fn(|r| c(&format!("row{}_mute", r + 1))),
            solo: std::array::from_fn(|r| c(&format!("row{}_solo", r + 1))),
            seed: fill_seed(deck),
        }
    }

    /// What the Groove Box plays now and next.
    pub fn state(&self) -> GrooveState {
        let any_solo = self.solo.iter().any(|s| s.get_bool());
        GrooveState {
            beat: self.beat.get(),
            bpm: self.bpm.get(),
            running: self.position.get() >= 0.0,
            patterns: std::array::from_fn(|p| {
                std::array::from_fn(|r| self.patterns[p][r].get() as u32)
            }),
            playing: (self.playing.get().round() as usize).min(GROOVES - 1),
            queued: (self.queued.get().round() as usize).min(GROOVES - 1),
            auto_fill: self.auto_fill.get().round() as u8,
            fill_held: self.fill.get_bool(),
            audible: std::array::from_fn(|r| {
                if any_solo {
                    self.solo[r].get_bool()
                } else {
                    !self.mute[r].get_bool()
                }
            }),
            seed: self.seed,
        }
    }
}

/// A Groove Box's state as the lanes need it.
#[derive(Clone, Debug, PartialEq)]
pub struct GrooveState {
    /// Its beat at the end of the last buffer, and its tempo.
    pub beat: f64,
    pub bpm: f64,
    pub running: bool,
    /// Each groove's tracks, a bit per step.
    pub patterns: [[u32; ROWS]; GROOVES],
    /// The groove playing, and the one to play from the next bar.
    pub playing: usize,
    pub queued: usize,
    pub auto_fill: u8,
    pub fill_held: bool,
    pub audible: [bool; ROWS],
    pub seed: u32,
}

/// One deck's hits and how its beats line up with its live clock.
#[derive(Default)]
struct Steps {
    hits: VecDeque<GrooveHit>,
    /// The beat at second 0 of the live clock, smoothed.
    offset: Option<f64>,
}

/// The Groove Box decks' step lanes.
pub struct StepLanes {
    rx: [rtrb::Consumer<GrooveHit>; NUM_DECKS],
    decks: [Steps; NUM_DECKS],
}

impl StepLanes {
    pub fn new(rx: [rtrb::Consumer<GrooveHit>; NUM_DECKS]) -> Self {
        Self {
            rx,
            decks: Default::default(),
        }
    }

    /// Takes the hits played. `groove[d]`: whether deck `d` plays its
    /// Groove Box; the others are cleared.
    pub fn poll(&mut self, groove: [bool; NUM_DECKS]) {
        for (d, rx) in self.rx.iter_mut().enumerate() {
            let steps = &mut self.decks[d];
            if !groove[d] {
                while rx.pop().is_ok() {}
                *steps = Steps::default();
                continue;
            }
            while let Ok(h) = rx.pop() {
                steps.hits.push_back(h);
            }
        }
    }

    /// Deck `d`'s lanes over a window starting at `start` on its live clock,
    /// with the play line at `at` and `ahead` seconds shown past it.
    pub fn lanes(
        &mut self,
        d: usize,
        state: &GrooveState,
        start: f64,
        at: f64,
        ahead: f64,
        color: [f32; 4],
    ) -> Lanes {
        let steps = &mut self.decks[d];
        let per_second = state.bpm.max(1.0) / 60.0;
        let mut marks = Vec::new();
        if state.running {
            // The beat's offset from the clock, smoothed over frames: the
            // beat is read once a buffer, the clock moves on every frame.
            let seen = state.beat - at * per_second;
            steps.offset = Some(match steps.offset {
                Some(o) if (seen - o).abs() < 0.25 => o + (seen - o) * 0.1,
                _ => seen,
            });
        }
        let Some(offset) = steps.offset else {
            return lanes(Vec::new(), color);
        };
        let time = |beat: f64| (beat - offset) / per_second;
        let step_seconds = 0.25 / per_second;
        let now = at * per_second + offset;
        while steps
            .hits
            .front()
            .is_some_and(|h| time(h.beat) < at - KEEP_SECONDS)
        {
            steps.hits.pop_front();
        }
        for h in steps.hits.iter().filter(|h| h.beat <= now) {
            let t = time(h.beat);
            marks.push(Mark {
                start: t - start,
                end: t + step_seconds * HIT_LENGTH - start,
                lane: (ROWS - 1 - h.row as usize) as u32,
                kind: if h.fill {
                    MarkKind::Fill
                } else {
                    MarkKind::Played
                },
            });
        }
        if state.running {
            for (s, row, fill) in coming(state, now, now + ahead * per_second) {
                let t = time(s as f64 / 4.0);
                marks.push(Mark {
                    start: t - start,
                    end: t + step_seconds * HIT_LENGTH - start,
                    lane: (ROWS - 1 - row) as u32,
                    kind: if fill {
                        MarkKind::FillComing
                    } else {
                        MarkKind::Coming
                    },
                });
            }
        }
        // The nearest to the play line, if there are more than the shader
        // draws: the oldest go first.
        if marks.len() > MAX_MARKS {
            marks.drain(..marks.len() - MAX_MARKS);
        }
        lanes(marks, color)
    }
}

fn lanes(marks: Vec<Mark>, color: [f32; 4]) -> Lanes {
    Lanes {
        count: ROWS as u32,
        tinted: 0,
        marks,
        trace: UNDER_STEPS,
        color,
        lit: LILAC,
    }
}

/// The hits still to come after beat `from` up to beat `to`: each one's
/// 16th (counted from beat 0), track, and whether it is part of a fill.
/// The groove waiting plays from the next bar; Auto fill or a held FILL
/// takes over the steps it covers.
pub fn coming(state: &GrooveState, from: f64, to: f64) -> Vec<(i64, usize, bool)> {
    let mut out = Vec::new();
    let first = (from * 4.0).floor() as i64 + 1;
    let last = (to * 4.0).floor() as i64;
    let this_bar = (from / 4.0).floor() as i64;
    for s in first..=last {
        let bar = s.div_euclid(STEPS as i64);
        let step = s.rem_euclid(STEPS as i64) as usize;
        let groove = if bar > this_bar {
            state.queued
        } else {
            state.playing
        };
        let fill = if state.fill_held && bar == this_bar {
            Some(held_fill(bar, state.seed))
        } else {
            auto_fill(state.auto_fill, bar, state.seed).filter(|f| step >= f.from)
        };
        for row in (0..ROWS).filter(|&r| state.audible[r]) {
            let hit = match fill {
                Some(f) => f.hits(step)[row] > 0.0,
                None => state.patterns[groove][row] >> step & 1 == 1,
            };
            if hit {
                out.push((s, row, fill.is_some()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> GrooveState {
        let mut patterns = [[0u32; ROWS]; GROOVES];
        // Groove A: four on the floor; groove B: kick on 1 only.
        patterns[0][0] = 0x1111;
        patterns[1][0] = 0x0001;
        GrooveState {
            beat: 0.0,
            bpm: 120.0,
            running: true,
            patterns,
            playing: 0,
            queued: 0,
            auto_fill: 0,
            fill_held: false,
            audible: [true; ROWS],
            seed: 1,
        }
    }

    #[test]
    fn coming_hits_follow_the_pattern() {
        let s = state();
        // From just after beat 0 to the end of the bar: kicks on 2, 3, 4.
        let hits = coming(&s, 0.1, 3.99);
        assert_eq!(hits, vec![(4, 0, false), (8, 0, false), (12, 0, false)]);
    }

    #[test]
    fn the_groove_waiting_plays_from_the_next_bar() {
        let mut s = state();
        s.queued = 1;
        let hits: Vec<i64> = coming(&s, 0.1, 7.99).iter().map(|h| h.0).collect();
        // Groove A's kicks to the bar line, then B's single kick.
        assert_eq!(hits, vec![4, 8, 12, 16]);
    }

    #[test]
    fn auto_fill_takes_over_the_last_beat_of_the_fourth_bar() {
        let mut s = state();
        s.auto_fill = 1;
        // Bar 4 (counted from 0: bar 3) is beats 12 to 16.
        let hits = coming(&s, 12.1, 15.99);
        assert!(hits.iter().filter(|h| h.0 < 60).all(|h| !h.2));
        assert!(hits.iter().any(|h| h.0 >= 60 && h.2));
    }

    #[test]
    fn muted_tracks_have_nothing_coming() {
        let mut s = state();
        s.audible[0] = false;
        assert!(coming(&s, 0.1, 3.99).is_empty());
    }

    #[test]
    fn played_hits_line_up_with_the_clock() {
        let (mut tx, rx) = rtrb::RingBuffer::new(16);
        let rx: [_; NUM_DECKS] = [
            rx,
            rtrb::RingBuffer::new(1).1,
            rtrb::RingBuffer::new(1).1,
            rtrb::RingBuffer::new(1).1,
        ];
        let mut lanes = StepLanes::new(rx);
        tx.push(GrooveHit {
            beat: 8.0,
            row: 0,
            velocity: 1.0,
            fill: false,
        })
        .unwrap();
        lanes.poll([true, false, false, false]);
        // At 120 BPM the groove is on beat 9 at second 10 of the clock:
        // beat 8 fell half a second earlier, at second 9.5.
        let mut s = state();
        s.beat = 9.0;
        let l = lanes.lanes(0, &s, 2.0, 10.0, 0.0, [1.0; 4]);
        let played = l.marks.iter().find(|m| m.kind == MarkKind::Played).unwrap();
        assert!((played.start - 7.5).abs() < 1e-9);
        // The kick, on the top row.
        assert_eq!(played.lane, ROWS as u32 - 1);
    }
}
