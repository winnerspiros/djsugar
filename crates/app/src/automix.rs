//! Automix's choices: how long a mix runs and what it asks of the mixer on the way.

use djsugar_engine::phrases::{Phrase, PhraseKind};

/// Mix lengths, in the order the panel offers them.
pub const LENGTHS: [&str; 7] = ["0 s", "12 s", "15 s", "30 s", "16 bars", "32 bars", "Phrases"];
/// The length that mixes by the tracks' phrases.
pub const PHRASES: u8 = 6;
/// Shortest and longest phrase mix, in bars.
const PHRASE_BARS: (f64, f64) = (8.0, 32.0);
/// How a mix moves, in the order the panel offers them.
pub const STYLES: [&str; 4] = ["Smooth", "Bass swap", "Filter", "Cut"];

const BASS_SWAP: u8 = 1;
const FILTER: u8 = 2;
pub const CUT: u8 = 3;

/// The mix's length in frames of the outgoing track, given one of its beats and its sample rate.
/// A cut switches over in one beat whatever the length.
pub fn frames(length: u8, style: u8, beat: f64, rate: f64) -> f64 {
    if style == CUT {
        return beat;
    }
    match length {
        0 => 0.0,
        1 => 12.0 * rate,
        2 => 15.0 * rate,
        3 => 30.0 * rate,
        5 => beat * 128.0,
        // Also what a phrase mix falls back to on a track with no phrases found.
        _ => beat * 64.0,
    }
}

/// A track's phrases from the waveform the deck draws: `detail` holds a slice's level and its low, mid and high
/// band for every `per` frames. Empty when there are too few bars or it all sounds alike.
pub fn phrases(detail: &[[u8; 4]], per: usize, grid: &sugar_analysis::Grid, rate: u32) -> Vec<Phrase> {
    let bar = grid.beat_frames(rate) * 4.0;
    if bar < per as f64 * 4.0 {
        return Vec::new();
    }
    let (mut at, mut starts, mut levels) = (grid.first_beat.rem_euclid(bar), Vec::new(), Vec::new());
    loop {
        let (i, j) = ((at / per as f64) as usize, ((at + bar) / per as f64) as usize);
        if j > detail.len() || i >= j {
            break;
        }
        let mut sum = [0f32; 3];
        for slice in &detail[i..j] {
            for (k, s) in sum.iter_mut().enumerate() {
                *s += (f32::from(slice[1 + k]) / 255.0).powi(2);
            }
        }
        starts.push(at / f64::from(rate));
        levels.push(sum.map(|s| s / (j - i) as f32));
        at += bar;
    }
    djsugar_engine::phrases::from_bars(&starts, levels)
}

/// Where a phrase mix starts on the outgoing track and how long it runs, in seconds: from its outro, or else its
/// last change 8 to 32 bars before the end, for as many bars as the incoming track's `intro` lasts, so that
/// track's first change lands as the mix ends. `bar` is a bar of the outgoing track; None when no phrase fits.
pub fn phrase_mix(out: &[Phrase], bar: f64, end: f64, intro: Option<f64>) -> Option<(f64, f64)> {
    if bar <= 0.0 {
        return None;
    }
    let (least, most) = PHRASE_BARS;
    let room = |p: &Phrase| (end - p.start) / bar;
    let late = |p: &&Phrase| p.start > 0.0 && room(p) >= least;
    let outro = out.iter().rev().find(|p| p.kind == PhraseKind::Outro).filter(late);
    let from = outro.or_else(|| out.iter().rev().find(|p| late(p) && room(p) <= most))?;
    let room = room(from).floor();
    let bars = intro.map_or(most, |i| i.round().min(most)).min(room);
    let bars = ((bars / 4.0).floor() * 4.0).max(least).min(room);
    Some((from.start, bars * bar))
}

/// What a mix asks of the mixer besides the crossfader, `t` of the way through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moves {
    /// The bass is cut on the outgoing deck, then on the incoming one.
    pub bass: [bool; 2],
    /// The outgoing deck's filter, 0 open to 1 fully high-passed.
    pub filter: f32,
}

pub fn moves(style: u8, t: f32) -> Moves {
    Moves {
        bass: if style == BASS_SWAP { [t >= 0.5, t < 0.5] } else { [false; 2] },
        filter: if style == FILTER { t * 0.8 } else { 0.0 },
    }
}

/// The orders the list can be put in, as the menu names them.
pub const ORDERS: [&str; 6] =
    ["Shuffle", "BPM low to high", "BPM high to low", "Harmonic path", "Smooth flow (key and BPM)", "Reverse"];

/// How long a track plays before it comes off the list.
pub const PLAYED_AFTER: f64 = 5.0;

/// What ordering goes by: a track's tempo, 0 when unmeasured, and its key.
pub type Fact = (f32, Option<sugar_analysis::Key>);

/// Steps round the key wheel between two keys, one more across major and minor; far when either is unknown.
fn key_steps(a: Option<sugar_analysis::Key>, b: Option<sugar_analysis::Key>) -> f32 {
    let (Some(a), Some(b)) = (a, b) else { return 6.0 };
    let turn = (i32::from(a.camelot_number()) - i32::from(b.camelot_number())).rem_euclid(12);
    (turn.min(12 - turn) + i32::from(a.minor != b.minor)) as f32
}

/// The order to play `facts` in after a track of `from`: each place holds the number of the track that moves there.
/// `seed` drives the shuffle.
pub fn arrange(order: u8, from: Fact, facts: &[Fact], mut seed: u64) -> Vec<usize> {
    let mut out: Vec<usize> = (0..facts.len()).collect();
    // Unmeasured tracks go last whichever way the tempo runs.
    let tempo = |n: usize, up: bool| match (facts[n].0, up) {
        (b, _) if b <= 0.0 => f32::MAX,
        (b, true) => b,
        (b, false) => -b,
    };
    match order {
        0 => {
            for n in (1..out.len()).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                out.swap(n, (seed % (n as u64 + 1)) as usize);
            }
        }
        1 => out.sort_by(|&a, &b| tempo(a, true).total_cmp(&tempo(b, true))),
        2 => out.sort_by(|&a, &b| tempo(a, false).total_cmp(&tempo(b, false))),
        3 | 4 => {
            // Each next track is the nearest to the one before it.
            let mut left = std::mem::take(&mut out);
            let mut at = from;
            while !left.is_empty() {
                let cost = |n: usize| {
                    let apart = if at.0 > 0.0 && facts[n].0 > 0.0 { (at.0 - facts[n].0).abs() / 4.0 } else { 2.0 };
                    key_steps(at.1, facts[n].1) + if order == 4 { apart } else { 0.0 }
                };
                let best = (0..left.len()).min_by(|&a, &b| cost(left[a]).total_cmp(&cost(left[b]))).unwrap_or(0);
                at = facts[left[best]];
                out.push(left.remove(best));
            }
        }
        _ => out.reverse(),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(at: &[(f64, PhraseKind)]) -> Vec<Phrase> {
        at.iter().map(|&(bar, kind)| Phrase { start: bar * 2.0, kind, energy: 0 }).collect()
    }

    #[test]
    fn a_phrase_mix_runs_from_the_outro_for_as_long_as_the_next_intro() {
        use PhraseKind::{Chorus, Drop, Intro, Outro};
        // Bars of two seconds; the track is 96 bars long.
        let out = parts(&[(0.0, Intro), (16.0, Drop), (64.0, Outro)]);
        assert_eq!(phrase_mix(&out, 2.0, 192.0, None), Some((128.0, 64.0)));
        assert_eq!(phrase_mix(&out, 2.0, 192.0, Some(16.0)), Some((128.0, 32.0)));
        assert_eq!(phrase_mix(&out, 2.0, 192.0, Some(3.0)), Some((128.0, 16.0)), "never under 8 bars");
        // No outro: the last change with 8 to 32 bars to go.
        let out = parts(&[(0.0, Intro), (40.0, Drop), (84.0, Chorus), (92.0, Drop)]);
        assert_eq!(phrase_mix(&out, 2.0, 192.0, None), Some((168.0, 24.0)));
        // An outro too near the end leaves no room; neither does a track of one long part.
        let out = parts(&[(0.0, Intro), (90.0, Outro)]);
        assert_eq!(phrase_mix(&out, 2.0, 192.0, None), None);
        assert_eq!(phrase_mix(&[], 2.0, 192.0, None), None);
    }

    #[test]
    fn phrases_are_found_in_the_deck_waveform() {
        // 120 BPM at 48 kHz: a bar is 96000 frames, 375 slices of 256. Quiet 16 bars, loud 32, quiet 16.
        let grid = sugar_analysis::Grid { bpm: 120.0, first_beat: 0.0 };
        let slice = |bar: usize| if (16..48).contains(&bar) { [250, 240, 200, 180] } else { [60, 5, 40, 30] };
        let detail: Vec<[u8; 4]> = (0..64 * 375).map(|n| slice(n / 375)).collect();
        let found = phrases(&detail, 256, &grid, 48_000);
        let starts: Vec<f64> = found.iter().map(|p| p.start).collect();
        assert_eq!(starts, [0.0, 32.0, 96.0], "{found:?}");
        assert!(phrases(&detail[..8 * 375], 256, &grid, 48_000).is_empty());
    }

    #[test]
    fn orders_keep_every_track_and_follow_tempo_or_key() {
        let key = |tonic, minor| Some(sugar_analysis::Key { tonic, minor });
        let facts = [(128.0, key(0, false)), (0.0, None), (120.0, key(2, false)), (124.0, key(7, false))];
        assert_eq!(arrange(1, (0.0, None), &facts, 1), [2, 3, 0, 1]);
        assert_eq!(arrange(2, (0.0, None), &facts, 1), [0, 3, 2, 1]);
        assert_eq!(arrange(5, (0.0, None), &facts, 1), [3, 2, 1, 0]);
        // From C: C itself, then G a step round the wheel, then D, and the unknown last.
        assert_eq!(arrange(3, (126.0, key(0, false)), &facts, 1), [0, 3, 2, 1]);
        let mut shuffled = arrange(0, (0.0, None), &facts, 0x9e37_79b9);
        shuffled.sort_unstable();
        assert_eq!(shuffled, [0, 1, 2, 3]);
    }

    #[test]
    fn lengths_follow_the_beat_or_the_clock() {
        let (beat, rate) = (22_050.0, 44_100.0);
        assert_eq!(frames(0, 0, beat, rate), 0.0);
        assert_eq!(frames(2, 0, beat, rate), 15.0 * rate);
        assert_eq!(frames(4, 0, beat, rate), 64.0 * beat);
        assert_eq!(frames(4, CUT, beat, rate), beat);
    }

    #[test]
    fn bass_swaps_halfway_and_only_the_filter_style_filters() {
        assert_eq!(moves(BASS_SWAP, 0.2).bass, [false, true]);
        assert_eq!(moves(BASS_SWAP, 0.7).bass, [true, false]);
        assert_eq!(moves(0, 0.7), Moves { bass: [false; 2], filter: 0.0 });
        assert!(moves(FILTER, 1.0).filter > 0.5);
    }
}
