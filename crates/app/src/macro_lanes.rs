//! Macro lanes: a second view of an instrument deck's scrolling waveform,
//! its knobs drawn as automation lines up to the play line.
//!
//! The deck shows the eight knobs of the page its deck shows, each line in
//! its own colour across the full height. Only the knobs that moved in view
//! get a line, so knobs left alone (often at the same value) don't pile up
//! on one another; the list beside the play line names all eight. The
//! values are noted each frame on the deck's live clock and kept for a few
//! seconds.

use std::collections::VecDeque;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::NUM_DECKS;
use djsugar_waveform::{Lanes, Mark, MarkKind, MAX_MARKS};

/// Knobs a page shows, each a colour (`MarkKind::Level`'s 1 to 8).
pub const KNOBS: usize = 8;
/// How opaque the sound is under the lines.
const UNDER_LINES: f32 = 0.25;
/// Values are kept this long behind the play line, in seconds.
const KEEP_SECONDS: f64 = 10.0;
/// A value is noted at most this often, in seconds.
const STEP_SECONDS: f64 = 0.05;
/// A knob counts as moved when it spans more than this in view.
const MOVED: f32 = 0.005;
/// The lit colour, Electric Lilac.
const LILAC: [f32; 4] = [0.714, 0.612, 1.0, 1.0];

/// The instrument's knobs, `[Instrument1],parameterN`, all of them.
pub struct KnobHandles {
    instrument: Vec<ControlHandle>,
}

impl KnobHandles {
    pub fn new(reg: &ControlRegistry) -> Self {
        let instrument = (1..)
            .map_while(|k| reg.get("[Instrument1]", &format!("parameter{k}")))
            .collect();
        Self { instrument }
    }

    /// The values now, from 0 to 1, of the knobs on `page` (eight to a
    /// page). Past the instrument's last parameter, 0.
    pub fn values(&self, page: usize) -> [f32; KNOBS] {
        std::array::from_fn(|i| {
            self.instrument
                .get(page * KNOBS + i)
                .map_or(0.0, |c| c.get().clamp(0.0, 1.0) as f32)
        })
    }
}

#[derive(Default)]
struct Deck {
    page: Option<usize>,
    /// (clock, values), oldest first.
    seen: VecDeque<(f64, [f32; KNOBS])>,
}

#[derive(Default)]
pub struct MacroLanes {
    decks: [Deck; NUM_DECKS],
}

impl MacroLanes {
    /// Forgets a deck's values, as when its lines are hidden.
    pub fn clear(&mut self, d: usize) {
        self.decks[d] = Deck::default();
    }

    /// Notes the deck's knobs on `page` at `at` on its live clock. Values
    /// from another page, or from before the clock went back, are dropped.
    pub fn note(&mut self, d: usize, page: usize, at: f64, values: [f32; KNOBS]) {
        let deck = &mut self.decks[d];
        if deck.page != Some(page) || deck.seen.back().is_some_and(|&(t, _)| t > at) {
            *deck = Deck {
                page: Some(page),
                seen: VecDeque::new(),
            };
        }
        // Too soon after the one before the last: the last moves up to
        // here instead, so values are noted about every STEP_SECONDS.
        let n = deck.seen.len();
        if n > 1 && at - deck.seen[n - 2].0 < STEP_SECONDS {
            deck.seen[n - 1] = (at, values);
        } else {
            deck.seen.push_back((at, values));
        }
        while deck.seen.len() > 2 && deck.seen[1].0 < at - KEEP_SECONDS {
            deck.seen.pop_front();
        }
    }

    /// The deck's lines for a window starting at `start` on its clock and
    /// ending at the play line `at`, over `back` seconds to its left, and
    /// which knobs moved in that time (only those are drawn).
    pub fn lanes(
        &self,
        d: usize,
        start: f64,
        at: f64,
        back: f64,
        color: [f32; 4],
    ) -> (Lanes, [bool; KNOBS]) {
        let deck = &self.decks[d];
        let from = at - back;
        // The last value before the view, then each one in it.
        let first = deck.seen.iter().rposition(|&(t, _)| t <= from).unwrap_or(0);
        let points: [Vec<(f64, f32)>; KNOBS] = std::array::from_fn(|i| {
            let mut points: Vec<(f64, f32)> = deck
                .seen
                .iter()
                .skip(first)
                .map(|&(t, v)| (t.max(from), v[i]))
                .collect();
            if let Some(&(t, v)) = points.last() {
                if t < at {
                    points.push((at, v));
                }
            }
            points
        });
        let moved: [bool; KNOBS] = std::array::from_fn(|i| {
            let (lo, hi) = points[i]
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), &(_, v)| {
                    (lo.min(v), hi.max(v))
                });
            hi - lo > MOVED
        });
        // The marks the shader draws, shared by the knobs that moved.
        let budget = MAX_MARKS / moved.iter().filter(|&&m| m).count().max(1);
        let mut marks = Vec::new();
        for i in (0..KNOBS).filter(|&i| moved[i]) {
            for w in simplify(&points[i], budget).windows(2) {
                let ((t0, v0), (t1, v1)) = (w[0], w[1]);
                if t1 > t0 {
                    marks.push(Mark {
                        start: t0 - start,
                        end: t1 - start,
                        lane: 0,
                        kind: MarkKind::Level {
                            from: v0,
                            to: v1,
                            color: i as u8 + 1,
                        },
                    });
                }
            }
        }
        let lanes = Lanes {
            count: 1,
            tinted: 0,
            marks,
            trace: UNDER_LINES,
            color,
            lit: LILAC,
        };
        (lanes, moved)
    }
}

/// The line through `points` in at most `segments` straight pieces: the
/// points furthest from the line so far are kept first, so a sweep keeps
/// its shape and a knob left alone is one piece.
fn simplify(points: &[(f64, f32)], segments: usize) -> Vec<(f64, f32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut keep = vec![0, points.len() - 1];
    while keep.len() <= segments {
        // The point furthest (in value) from the piece it falls in.
        let mut best: Option<(f32, usize)> = None;
        for w in keep.windows(2) {
            let (a, b) = (points[w[0]], points[w[1]]);
            for (j, &(t, v)) in points.iter().enumerate().take(w[1]).skip(w[0] + 1) {
                let f = if b.0 > a.0 {
                    ((t - a.0) / (b.0 - a.0)) as f32
                } else {
                    0.0
                };
                let off = (v - (a.1 + (b.1 - a.1) * f)).abs();
                if best.is_none_or(|(o, _)| off > o) {
                    best = Some((off, j));
                }
            }
        }
        match best {
            Some((off, j)) if off > 0.002 => {
                let at = keep.partition_point(|&k| k < j);
                keep.insert(at, j);
            }
            _ => break,
        }
    }
    keep.into_iter().map(|k| points[k]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

    /// Each piece of knob `knob`'s line (colour knob + 1).
    fn levels(lanes: &Lanes, knob: u8) -> Vec<(f64, f64, f32, f32)> {
        lanes
            .marks
            .iter()
            .filter_map(|m| match m.kind {
                MarkKind::Level { from, to, color } if color == knob + 1 => {
                    assert_eq!(m.lane, 0);
                    Some((m.start, m.end, from, to))
                }
                MarkKind::Level { .. } => None,
                k => panic!("not a level: {k:?}"),
            })
            .collect()
    }

    #[test]
    fn knobs_left_alone_draw_no_line() {
        let mut m = MacroLanes::default();
        for f in 0..300 {
            m.note(0, 0, 10.0 + f as f64 / 60.0, [0.25; KNOBS]);
        }
        let (l, moved) = m.lanes(0, 7.0, 15.0, 4.0, RED);
        assert_eq!(l.count, 1);
        assert_eq!(moved, [false; KNOBS]);
        assert!(l.marks.is_empty());
    }

    #[test]
    fn a_sweep_keeps_its_shape_in_its_colour() {
        let mut m = MacroLanes::default();
        // Knob 3 sweeps up and down over 4 s; the rest stay put.
        for f in 0..240 {
            let t = f as f64 / 60.0;
            let mut v = [0.5; KNOBS];
            v[2] = (0.5 + 0.5 * (t * 3.0).sin()) as f32;
            m.note(1, 0, t, v);
        }
        let (l, moved) = m.lanes(1, 0.0, 4.0, 4.0, RED);
        assert_eq!(moved.iter().filter(|&&m| m).count(), 1);
        assert!(moved[2]);
        let sweep = levels(&l, 2);
        assert_eq!(sweep.len(), l.marks.len());
        assert!(
            sweep.len() > 8 && sweep.len() <= MAX_MARKS,
            "{}",
            sweep.len()
        );
        // The pieces join up, follow the sine closely and end at the play line.
        for w in sweep.windows(2) {
            assert!((w[0].1 - w[1].0).abs() < 1e-9 && (w[0].3 - w[1].2).abs() < 1e-6);
        }
        for &(s, _, a, _) in &sweep {
            let want = 0.5 + 0.5 * (s * 3.0).sin();
            assert!((a as f64 - want).abs() < 0.05, "at {s}: {a} vs {want}");
        }
        assert!((sweep.last().unwrap().1 - 4.0).abs() < 1e-9);
    }

    #[test]
    fn a_knob_moved_then_left_still_draws_while_in_view() {
        let mut m = MacroLanes::default();
        m.note(0, 0, 1.0, [0.1; KNOBS]);
        let mut v = [0.1; KNOBS];
        v[7] = 0.8;
        m.note(0, 0, 2.0, v);
        m.note(0, 0, 3.0, v);
        let (l, moved) = m.lanes(0, 0.0, 4.0, 4.0, RED);
        assert!(moved[7] && !moved[0]);
        let line = levels(&l, 7);
        assert_eq!(line.first().map(|p| p.2), Some(0.1));
        assert_eq!(line.last().map(|p| (p.1, p.3)), Some((4.0, 0.8)));
        // Once the move scrolls out of view, it is left alone again.
        let (_, moved) = m.lanes(0, 0.0, 4.0, 1.5, RED);
        assert!(!moved[7]);
    }

    #[test]
    fn another_page_or_a_clock_going_back_starts_over() {
        let mut m = MacroLanes::default();
        let mut v = [0.1; KNOBS];
        m.note(0, 0, 1.0, v);
        v[0] = 0.5;
        m.note(0, 0, 2.0, v);
        m.note(0, 1, 3.0, [0.9; KNOBS]);
        assert_eq!(m.lanes(0, 0.0, 4.0, 4.0, RED).1, [false; KNOBS]);
        m.note(0, 1, 0.5, [0.3; KNOBS]);
        assert_eq!(m.decks[0].seen.len(), 1);
    }

    #[test]
    fn old_values_are_dropped() {
        let mut m = MacroLanes::default();
        for f in 0..(30 * 60) {
            m.note(2, 0, f as f64 / 60.0, [0.0; KNOBS]);
        }
        let kept = m.decks[2].seen.len() as f64;
        assert!(kept < 2.0 * (KEEP_SECONDS + 1.0) / STEP_SECONDS, "{kept}");
    }
}
