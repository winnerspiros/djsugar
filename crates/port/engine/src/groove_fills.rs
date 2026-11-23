//! Drum fills and Remix for the Groove Box.
//!
//! **Auto fill**, as drum machines such as Sugar Bytes' DrumComputer do it:
//! Auto 1 plays a fill in the last beat of every 4th bar, the last two
//! beats of every 8th and the whole of every 16th; Auto 2 does the same
//! every 2nd, 4th and 8th bar. The bars count with the groove's beat, which
//! follows the leading deck, so the fills land on its phrases. After a fill
//! of two beats or more the cymbal track crashes on the next downbeat.
//!
//! **Remix** is a strip: where it is held picks one of 16 figures, and the
//! groove plays it until the strip is let go, then goes straight back. Low
//! figures roll the hats, the middle ones roll two or three sounds and then
//! swap the tracks' patterns between sounds, the top two fire 16th rolls
//! over everything.
//!
//! Fills and figures are worked out a step at a time from the step, the bar
//! and a seed, so nothing is stored or allocated on the audio thread.

use crate::groove::{ROWS, STEPS};

/// The tracks, by number.
const KICK: usize = 0;
const SNARE: usize = 1;
const HAT: usize = 2;
const OPEN: usize = 3;
const CLAP: usize = 4;
const RIM: usize = 5;
const COWBELL: usize = 6;
const CYMBAL: usize = 7;

/// Where a fill plays: from `from` (a step of the bar) to the bar's end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    pub from: usize,
    /// Which of the fill figures, picked per bar.
    pub kind: u32,
}

impl Fill {
    pub fn steps(&self) -> usize {
        STEPS - self.from
    }

    /// The fill's tracks on `step` (in it): the loudness of each track that
    /// plays, 0 for the ones that rest.
    pub fn hits(&self, step: usize) -> [f32; ROWS] {
        let mut v = [0f32; ROWS];
        let i = step.saturating_sub(self.from);
        let len = self.steps().max(1);
        // Louder toward the end.
        let up = 0.55 + 0.5 * (i as f32 + 1.0) / len as f32;
        let short = len <= 4;
        match (self.kind % 4, short) {
            // One beat: four snares, or snare-snare-kick-snare.
            (0 | 2, true) => v[SNARE] = up,
            (_, true) => {
                v[if i == 2 { KICK } else { SNARE }] = up;
            }
            // A snare roll: 8ths, then 16ths for the last beat.
            (0, false) => {
                if i.is_multiple_of(2) || i + 4 >= len {
                    v[SNARE] = up;
                }
                if i == 0 {
                    v[KICK] = 1.0;
                }
            }
            // Kick and snare trading 16ths, hats on the 8ths.
            (1, false) => {
                v[if i % 4 == 2 { KICK } else { SNARE }] = up;
                if i.is_multiple_of(2) {
                    v[HAT] = 0.6;
                }
            }
            // Rim and clap, building.
            (2, false) => {
                v[RIM] = up * 0.9;
                if i.is_multiple_of(4) || i + 2 >= len {
                    v[CLAP] = up;
                }
                if i == 0 {
                    v[KICK] = 1.0;
                }
            }
            // Threes against the bar: snare, hat, kick.
            _ => {
                v[[SNARE, HAT, KICK][i % 3]] = up;
            }
        }
        v
    }
}

/// The fill in bar `bar` (counted from 0) for Auto fill `auto` (1 or 2),
/// if that bar has one; `seed` varies the figure from Groove Box to Groove
/// Box.
pub fn auto_fill(auto: u8, bar: i64, seed: u32) -> Option<Fill> {
    let every = match auto {
        1 => 4,
        2 => 2,
        _ => return None,
    };
    let n = bar + 1;
    let from = if n % (every * 4) == 0 {
        0
    } else if n % (every * 2) == 0 {
        8
    } else if n % every == 0 {
        12
    } else {
        return None;
    };
    Some(Fill {
        from,
        kind: figure(bar, seed),
    })
}

/// A fill that plays the whole bar, for FILL held.
pub fn held_fill(bar: i64, seed: u32) -> Fill {
    Fill {
        from: 0,
        kind: figure(bar, seed),
    }
}

fn figure(bar: i64, seed: u32) -> u32 {
    let x = (bar as u32).wrapping_mul(2_654_435_761) ^ seed;
    (x ^ (x >> 15)).wrapping_mul(0x2c1b_3c6d) >> 28
}

/// The Remix figure for a strip at `position` (0-1; 0 is off): 1-16.
pub fn remix_level(position: f64) -> u8 {
    if position <= 0.0 {
        0
    } else {
        (position.clamp(0.0, 1.0) * 15.999).floor() as u8 + 1
    }
}

/// Remix figure `level` (1-16) on `step`: each track's loudness (0 rests),
/// from `normal`, what each track would play on this step without Remix,
/// and `patterns`, each track's steps (for the figures that swap them).
pub fn remix(level: u8, step: usize, normal: [f32; ROWS], patterns: [u32; ROWS]) -> [f32; ROWS] {
    let mut v = normal;
    let accent = if step.is_multiple_of(4) { 1.0 } else { 0.7 };
    let roll = |v: &mut [f32; ROWS], rows: &[usize]| {
        for &r in rows {
            v[r] = accent;
        }
    };
    match level {
        // Hat rolls, then hats with one more sound.
        1 => roll(&mut v, &[HAT]),
        2 => {
            roll(&mut v, &[HAT]);
            v[OPEN] = 0.0;
        }
        3 => roll(&mut v, &[HAT, RIM]),
        4 => roll(&mut v, &[HAT, COWBELL]),
        // Two or three sounds rolling.
        5 => roll(&mut v, &[SNARE, RIM]),
        6 => roll(&mut v, &[SNARE, CLAP]),
        7 => roll(&mut v, &[RIM, CLAP, COWBELL]),
        8 => roll(&mut v, &[SNARE, HAT, RIM]),
        // The tracks' patterns move to other sounds (the kick keeps its own).
        9..=14 => {
            let shift = (level - 8) as usize;
            for (r, out) in v.iter_mut().enumerate().skip(1) {
                let from = 1 + (r - 1 + shift) % (ROWS - 1);
                *out = if patterns[from] & (1 << step) != 0 {
                    0.9
                } else {
                    0.0
                };
            }
            if level >= 12 {
                // Busier: the hats roll under it.
                v[HAT] = v[HAT].max(0.6);
            }
        }
        // 16th rolls over the kick, snare and hats; then everything.
        15 => {
            roll(&mut v, &[KICK, SNARE, HAT]);
            v[CYMBAL] = 0.0;
        }
        _ => {
            for (r, out) in v.iter_mut().enumerate() {
                *out = if r == CYMBAL { 0.0 } else { accent };
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_1_fills_bars_4_8_and_16() {
        let from = |bar| auto_fill(1, bar, 0).map(|f| f.from);
        // Bars counted from 0: the 4th is bar 3.
        assert_eq!(from(0), None);
        assert_eq!(from(3), Some(12), "the last beat of the 4th bar");
        assert_eq!(from(7), Some(8), "the last two beats of the 8th");
        assert_eq!(from(11), Some(12));
        assert_eq!(from(15), Some(0), "the whole 16th bar");
        assert_eq!(from(4), None);
    }

    #[test]
    fn auto_2_fills_twice_as_often() {
        let from = |bar| auto_fill(2, bar, 0).map(|f| f.from);
        assert_eq!(from(1), Some(12));
        assert_eq!(from(3), Some(8));
        assert_eq!(from(7), Some(0));
        assert_eq!(from(2), None);
        assert_eq!(auto_fill(0, 15, 0), None);
    }

    #[test]
    fn every_fill_rolls_through_its_last_beat_louder_than_it_starts() {
        for kind in 0..16 {
            for from in [0, 8, 12] {
                let f = Fill { from, kind };
                let loudest = |step| f.hits(step).iter().cloned().fold(0.0, f32::max);
                for step in 12..STEPS {
                    assert!(
                        loudest(step) > 0.0,
                        "kind {kind} from {from}: step {step} is silent"
                    );
                }
                assert!(
                    loudest(STEPS - 1) > loudest(from),
                    "kind {kind} from {from} builds"
                );
            }
        }
    }

    #[test]
    fn remix_levels_cover_the_strip() {
        assert_eq!(remix_level(0.0), 0);
        assert_eq!(remix_level(0.01), 1);
        assert_eq!(remix_level(0.5), 8);
        assert_eq!(remix_level(0.52), 9);
        assert_eq!(remix_level(1.0), 16);
    }

    #[test]
    fn low_remix_rolls_the_hats_and_high_rolls_everything() {
        let normal = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let pats = [1, 0, 0, 0, 0, 0, 0, 0];
        let low = remix(1, 3, normal, pats);
        assert!(low[HAT] > 0.0);
        assert_eq!(low[KICK], 1.0, "the rest plays as it was");
        let top = remix(16, 3, [0.0; ROWS], pats);
        assert!(top[..CYMBAL].iter().all(|&v| v > 0.0));
    }

    #[test]
    fn swapping_remix_moves_patterns_but_keeps_the_kick() {
        let mut pats = [0u32; ROWS];
        pats[SNARE] = 1 << 4;
        pats[HAT] = 1 << 2;
        let normal = [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let v = remix(9, 4, normal, pats);
        assert_eq!(v[KICK], 0.5);
        // With a shift of one, track r plays track r+1's steps: the snare's
        // pattern moves down to the cymbal (wrapping round tracks 1-7).
        assert!(v[CYMBAL] > 0.0);
        assert_eq!(v[SNARE], 0.0);
    }
}
