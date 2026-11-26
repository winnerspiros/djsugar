//! Automix's choices: how long a mix runs and what it asks of the mixer on the way.

/// Mix lengths, in the order the panel offers them.
pub const LENGTHS: [&str; 6] = ["0 s", "12 s", "15 s", "30 s", "16 bars", "32 bars"];
/// How a mix moves, in the order the panel offers them.
pub const STYLES: [&str; 4] = ["Smooth", "Bass swap", "Filter", "Cut"];

const BASS_SWAP: u8 = 1;
const FILTER: u8 = 2;
const CUT: u8 = 3;

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
        4 => beat * 64.0,
        _ => beat * 128.0,
    }
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
