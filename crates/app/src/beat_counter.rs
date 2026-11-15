//! The beat counter in each deck header: which beat of the bar is playing,
//! which bar of the phrase, and how many bars until the next phrase, so a
//! DJ can see the downbeat and the next phrase change coming, as
//! VirtualDJ's beat squares and Traktor's phrase counter show.
//!
//! Bars are counted on the grid the waveform draws (a bar line every
//! fourth beat from beat 0, the first at [`first_bar_beat`]). Phrases come
//! from the phrase analysis; a track without phrases counts in phrases of
//! eight bars from its first bar.

use djsugar_engine::{phrases, Track};
use djsugar_waveform::PhraseKind;

/// Bars in a phrase when the track has no phrase analysis.
pub const DEFAULT_PHRASE_BARS: i64 = 8;

/// What the beat counter shows: off, the beats alone (1), or beats and
/// phrases.
pub const MODE_OFF: i32 = 0;
pub const MODE_PHRASES: i32 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct Count {
    /// The beat in the bar, 0 to 3 (0 is the downbeat).
    pub beat: i64,
    /// The bar in its group of four, 0 to 3, counted from the phrase's
    /// start so the strip fills up to each phrase change.
    pub bar_in_group: i64,
    /// The phrase, from 1, and the bar in it, from 1; 0 for both before the
    /// first bar.
    pub phrase: i64,
    pub bar_in_phrase: i64,
    /// Bars until the next phrase starts, counting this one (1 in the last
    /// bar); 0 when no phrase follows.
    pub bars_left: i64,
    /// The next phrase's kind, when the track has phrases.
    pub next: Option<PhraseKind>,
}

impl Count {
    /// "phrase.bar.beat", as Traktor's phrase counter shows it; just the
    /// beat before the first bar.
    pub fn label(&self) -> String {
        if self.phrase == 0 {
            format!("–.–.{}", self.beat + 1)
        } else {
            format!("{}.{}.{}", self.phrase, self.bar_in_phrase, self.beat + 1)
        }
    }

    /// "↓ 6 › DROP": bars to the next phrase and what it is.
    pub fn countdown(&self) -> String {
        match (self.bars_left, self.next) {
            (0, _) => String::new(),
            (n, Some(kind)) => format!("↓ {n} › {}", kind_name(kind)),
            (n, None) => format!("↓ {n}"),
        }
    }
}

/// A track's phrases, earliest first, as the overview draws them: found
/// from its waveform when it has one, otherwise those stored with it.
pub fn track_phrases(t: &Track) -> Vec<(f64, PhraseKind)> {
    let found = if phrases::waveform_ready(t) {
        phrases::detect(t)
    } else {
        phrases::stored(t)
    };
    found
        .into_iter()
        .map(|p| {
            let kind = match p.kind {
                phrases::PhraseKind::Intro => PhraseKind::Intro,
                phrases::PhraseKind::Build => PhraseKind::Build,
                phrases::PhraseKind::Drop => PhraseKind::Drop,
                phrases::PhraseKind::Breakdown => PhraseKind::Breakdown,
                phrases::PhraseKind::Outro => PhraseKind::Outro,
                phrases::PhraseKind::Chorus => PhraseKind::Chorus,
                phrases::PhraseKind::Verse => PhraseKind::Verse,
            };
            (p.start, kind)
        })
        .collect()
}

/// The phrase kind as the counter names it, short enough for the header.
pub fn kind_name(kind: PhraseKind) -> &'static str {
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

/// The colour the counter names a phrase in: the overview's phrase colour,
/// lighter for intros and outros, whose dark blue is hard to read as text.
pub fn kind_color(kind: PhraseKind) -> u32 {
    match kind {
        PhraseKind::Intro | PhraseKind::Outro => 0x5b9cc2,
        PhraseKind::Build => 0x9472d5,
        PhraseKind::Drop => 0xe0607d,
        PhraseKind::Breakdown => 0xe99b68,
        PhraseKind::Chorus => 0x48c5a1,
        PhraseKind::Verse => 0xd8bc62,
    }
}

/// The beat number of the track's first bar: the first bar line at or
/// after its start, on the grid the waveform draws (bar lines every fourth
/// beat from beat 0).
pub fn first_bar_beat(t: &Track) -> f64 {
    let at_start = if t.grid.is_empty() {
        if t.bpm <= 0.0 {
            return 0.0;
        }
        -t.first_beat * t.bpm / 60.0
    } else {
        t.grid.beat_at(0.0)
    };
    // A hair of tolerance so a bar right at the start is bar 1.
    (at_start / 4.0 - 1e-6).ceil() * 4.0
}

/// Where `seconds` falls in `t`'s bars and phrases; `None` without a tempo.
/// `phrases` are the track's phrases, earliest first, as the overview
/// draws them.
pub fn count(t: &Track, seconds: f64, phrases: &[(f64, PhraseKind)]) -> Option<Count> {
    if t.bpm <= 0.0 {
        return None;
    }
    // A hair ahead, so a position read exactly on a beat counts as that beat.
    let beat = t.beat_at(seconds) + 1e-6;
    let first = first_bar_beat(t);
    let beat_in_bar = (beat.floor() as i64).rem_euclid(4);
    let bar = ((beat - first) / 4.0).floor() as i64;
    let bar_of = |s: f64| ((t.beat_at(s) - first) / 4.0).round() as i64;
    // The phrase holding this bar, its first bar and the next phrase.
    let (index, start, next) = match phrases.iter().rposition(|(s, _)| bar_of(*s) <= bar) {
        Some(i) => {
            let next = phrases.get(i + 1).map(|&(s, kind)| (bar_of(s), Some(kind)));
            (i as i64, bar_of(phrases[i].0), next)
        }
        None if phrases.is_empty() => {
            let i = bar.div_euclid(DEFAULT_PHRASE_BARS);
            let start = i * DEFAULT_PHRASE_BARS;
            (i, start, Some((start + DEFAULT_PHRASE_BARS, None)))
        }
        // Before the first phrase.
        None => (
            -1,
            bar,
            phrases.first().map(|&(s, kind)| (bar_of(s), Some(kind))),
        ),
    };
    let in_phrase = bar - start;
    let (bars_left, next) = match next {
        Some((at, kind)) if at > bar => (at - bar, kind),
        _ => (0, None),
    };
    let counted = bar >= 0 && index >= 0;
    Some(Count {
        beat: beat_in_bar,
        bar_in_group: in_phrase.rem_euclid(4),
        phrase: if counted { index + 1 } else { 0 },
        bar_in_phrase: if counted { in_phrase + 1 } else { 0 },
        bars_left,
        next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A steady 120 BPM track (half a second a beat, two seconds a bar)
    /// whose first beat is at `first_beat`.
    fn track(first_beat: f64) -> Track {
        let mut t = (*djsugar_engine::sample_library()[0]).clone();
        t.bpm = 120.0;
        t.first_beat = first_beat;
        t.grid = Default::default();
        t.duration = 600.0;
        t
    }

    #[test]
    fn counts_beats_bars_and_eight_bar_phrases_without_phrases() {
        let t = track(0.0);
        let c = count(&t, 0.0, &[]).unwrap();
        assert_eq!(
            (c.beat, c.phrase, c.bar_in_phrase, c.bars_left),
            (0, 1, 1, 8)
        );
        assert_eq!(c.label(), "1.1.1");
        // Beat 3 of bar 2.
        let c = count(&t, 2.0 + 1.0 + 0.1, &[]).unwrap();
        assert_eq!((c.beat, c.bar_in_phrase, c.bar_in_group), (2, 2, 1));
        // The last bar of phrase 1, then the first of phrase 2.
        let c = count(&t, 14.0, &[]).unwrap();
        assert_eq!((c.phrase, c.bar_in_phrase, c.bars_left), (1, 8, 1));
        assert_eq!(c.countdown(), "↓ 1");
        let c = count(&t, 16.0, &[]).unwrap();
        assert_eq!((c.phrase, c.bar_in_phrase, c.bar_in_group), (2, 1, 0));
    }

    #[test]
    fn a_pickup_before_the_first_bar_shows_beats_only() {
        // First beat at 1.5 s: beat -3 falls at 0 s, so 0 to 1.5 s is a
        // pickup before bar 1.
        let t = track(1.5);
        let c = count(&t, 0.6, &[]).unwrap();
        assert_eq!(c.phrase, 0);
        assert!(c.label().starts_with("–.–."));
        let c = count(&t, 1.5, &[]).unwrap();
        assert_eq!((c.beat, c.phrase, c.bar_in_phrase), (0, 1, 1));
    }

    #[test]
    fn follows_the_phrases_and_names_the_next_one() {
        let t = track(0.0);
        // Intro of 16 bars (32 s), a build of 8 bars, then the drop.
        let phrases = [
            (0.0, PhraseKind::Intro),
            (32.0, PhraseKind::Build),
            (48.0, PhraseKind::Drop),
        ];
        let c = count(&t, 20.0, &phrases).unwrap();
        assert_eq!((c.phrase, c.bar_in_phrase, c.bars_left), (1, 11, 6));
        assert_eq!(c.next, Some(PhraseKind::Build));
        assert_eq!(c.countdown(), "↓ 6 › BUILD");
        // The bar strip counts from the phrase's start.
        assert_eq!(c.bar_in_group, 2);
        let c = count(&t, 47.0, &phrases).unwrap();
        assert_eq!((c.phrase, c.bar_in_phrase, c.bars_left), (2, 8, 1));
        assert_eq!(c.countdown(), "↓ 1 › DROP");
        // In the last phrase nothing follows.
        let c = count(&t, 60.0, &phrases).unwrap();
        assert_eq!((c.phrase, c.bars_left), (3, 0));
        assert_eq!(c.countdown(), "");
    }

    #[test]
    fn a_phrase_a_hair_off_the_bar_line_still_starts_on_it() {
        let t = track(0.0);
        let phrases = [(0.0, PhraseKind::Intro), (31.98, PhraseKind::Drop)];
        let c = count(&t, 32.0, &phrases).unwrap();
        assert_eq!((c.phrase, c.bar_in_phrase), (2, 1));
    }

    #[test]
    fn no_count_without_a_tempo() {
        let mut t = track(0.0);
        t.bpm = 0.0;
        assert_eq!(count(&t, 1.0, &[]), None);
    }
}
