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

#[cfg(test)]
mod tests {
    use super::*;

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
