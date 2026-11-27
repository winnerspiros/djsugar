//! Where a deck is in its bar and its phrase, for the counter in the deck's title row.

use djsugar_engine::phrases::{Phrase, PhraseKind};
use sugar_analysis::Grid;

/// Bars a phrase is taken to have when none were found in the track.
const PLAIN: i64 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count {
    /// The beat of the bar, 0 the downbeat.
    pub beat: u8,
    /// The bar of its four, counted from the phrase's start.
    pub bar: u8,
    /// Bars until the next phrase, this one included; 0 when nothing follows.
    pub left: u32,
    /// What follows, when the track's phrases are known.
    pub next: Option<PhraseKind>,
}

/// The name of a phrase as the counter writes it.
pub fn name(kind: PhraseKind) -> &'static str {
    match kind {
        PhraseKind::Intro => "INTRO",
        PhraseKind::Build => "BUILD",
        PhraseKind::Drop => "DROP",
        PhraseKind::Breakdown => "BREAK",
        PhraseKind::Outro => "OUTRO",
        PhraseKind::Chorus => "CHORUS",
        PhraseKind::Verse => "VERSE",
    }
}

/// The colour a phrase is marked with along a track's overview.
pub fn colour(kind: PhraseKind) -> u32 {
    match kind {
        PhraseKind::Intro | PhraseKind::Outro => 0x5a_8d_ff,
        PhraseKind::Build => 0xf5_a0_2a,
        PhraseKind::Drop | PhraseKind::Chorus => 0xff_4f_a8,
        PhraseKind::Breakdown => 0x3c_cf_6e,
        PhraseKind::Verse => 0x8e_8e_93,
    }
}

/// The count at `frame` of a track with `grid`; phrases start on the bar nearest to their time.
pub fn count(grid: &Grid, rate: u32, frame: f64, phrases: &[Phrase]) -> Option<Count> {
    let per = grid.beat_frames(rate);
    if !per.is_finite() || per <= 0.0 {
        return None;
    }
    let beat = ((frame - grid.first_beat) / per + 1e-6).floor() as i64;
    let bar = beat.div_euclid(4);
    let bar_of = |seconds: f64| ((seconds * f64::from(rate) - grid.first_beat) / per / 4.0).round() as i64;
    let starts: Vec<(i64, PhraseKind)> = phrases.iter().map(|p| (bar_of(p.start), p.kind)).collect();
    let (from, until) = match starts.iter().rposition(|&(b, _)| b <= bar) {
        Some(n) => (starts[n].0, starts.get(n + 1).copied()),
        None if starts.is_empty() => (bar.div_euclid(PLAIN) * PLAIN, None),
        None => (bar, starts.first().copied()),
    };
    let left = match until {
        Some((b, _)) => b - bar,
        None if starts.is_empty() => from + PLAIN - bar,
        None => 0,
    };
    Some(Count {
        beat: beat.rem_euclid(4) as u8,
        bar: (bar - from).rem_euclid(4) as u8,
        left: left.max(0) as u32,
        next: until.map(|(_, kind)| kind),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn grid() -> Grid {
        // 120 beats a minute: a beat is half a second, a bar two.
        Grid { bpm: 120.0, first_beat: 0.0 }
    }

    fn at(seconds: f64, phrases: &[Phrase]) -> Count {
        count(&grid(), RATE, seconds * f64::from(RATE), phrases).unwrap()
    }

    #[test]
    fn counts_beats_and_plain_phrases_of_eight_bars() {
        assert_eq!(at(0.0, &[]), Count { beat: 0, bar: 0, left: 8, next: None });
        assert_eq!(at(1.6, &[]), Count { beat: 3, bar: 0, left: 8, next: None });
        assert_eq!(at(14.0, &[]), Count { beat: 0, bar: 3, left: 1, next: None });
        assert_eq!(at(16.0, &[]).left, 8);
    }

    #[test]
    fn counts_down_to_the_next_phrase() {
        let phrase = |start, kind| Phrase { start, kind, energy: 0 };
        let found = [phrase(0.0, PhraseKind::Intro), phrase(8.1, PhraseKind::Drop)];
        assert_eq!(at(6.5, &found), Count { beat: 1, bar: 3, left: 1, next: Some(PhraseKind::Drop) });
        assert_eq!(at(8.0, &found), Count { beat: 0, bar: 0, left: 0, next: None });
        assert_eq!(name(PhraseKind::Drop), "DROP");
    }

    #[test]
    fn counts_before_the_first_beat_and_refuses_no_tempo() {
        let late = Grid { bpm: 120.0, first_beat: 48_000.0 };
        assert_eq!(count(&late, RATE, 0.0, &[]).unwrap().beat, 2);
        assert!(count(&Grid { bpm: 0.0, first_beat: 0.0 }, RATE, 0.0, &[]).is_none());
    }
}
