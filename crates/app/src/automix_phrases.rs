//! Where Automix's Phrases length mixes: from a phrase change of the
//! outgoing track (its outro, or an imported fade-out point), bringing the
//! incoming track in from the first bar of its intro, for as many bars as
//! its intro lasts, so its first phrase change lands as the mix ends.

use djsugar_engine::phrases::{self, Phrase, PhraseKind};
use djsugar_engine::track::MarkerKind;
use djsugar_engine::Track;

/// Shortest and longest phrase mix, in bars of the outgoing track.
const MIN_BARS: f64 = 8.0;
const MAX_BARS: f64 = 32.0;
/// How far back from where its sound starts the incoming track may be cued,
/// to start on the first bar of its phrase.
const BACK_BARS: f64 = 2.0;

/// The names VirtualDJ (and Traktor, on import) give their automix points:
/// where to fade out of a track, and where its music really starts.
const FADE_OUT: &str = "fadeStart";
const REAL_START: &str = "realStart";

/// A phrase mix on the outgoing track, in its seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub start: f64,
    pub end: f64,
    /// A phrase change of the outgoing track inside the mix, nearest its
    /// middle: where Bass swap swaps.
    pub swap: Option<f64>,
    /// Where the mix starts from: the outgoing track's outro, another
    /// phrase change, or an imported fade-out point.
    pub from: Start,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Start {
    Outro,
    Phrase,
    Imported,
}

/// The track's phrases, earliest first: found from its waveform when that
/// is whole, otherwise the ones stored with it (which may be imported).
pub fn of(t: &Track) -> Vec<Phrase> {
    if phrases::imported(t) || !phrases::waveform_ready(t) {
        let mut p: Vec<Phrase> = t
            .info
            .markers
            .iter()
            .filter(|m| m.kind == MarkerKind::Phrase)
            .map(|m| Phrase {
                start: m.seconds,
                kind: PhraseKind::from_name(&m.name).unwrap_or(match m.name.as_str() {
                    // rekordbox's names for the same parts.
                    "Up" => PhraseKind::Build,
                    "Down" | "Bridge" => PhraseKind::Breakdown,
                    n if n.starts_with("Verse") => PhraseKind::Verse,
                    _ => PhraseKind::Chorus,
                }),
                energy: 0,
            })
            .collect();
        p.sort_by(|a, b| a.start.total_cmp(&b.start));
        p
    } else {
        phrases::detect(t)
    }
}

fn point(t: &Track, name: &str) -> Option<f64> {
    t.info
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Automix && m.name == name)
        .map(|m| m.seconds)
}

/// Bars from `a` to `b` on `t`'s grid.
fn bars(t: &Track, a: f64, b: f64) -> f64 {
    (t.beat_at(b) - t.beat_at(a)) / 4.0
}

/// Where to cue the incoming track: its imported start point, else the
/// first bar of the phrase its sound starts in (when that is at most 2
/// bars back), else the bar line its sound starts in.
pub fn in_point(t: &Track, phrases: &[Phrase], sound_start: f64) -> f64 {
    if let Some(p) = point(t, REAL_START).filter(|p| *p < t.duration) {
        return p.max(0.0);
    }
    if t.bpm <= 0.0 {
        return sound_start;
    }
    let phrase = phrases.iter().rev().find(|p| p.start <= sound_start + 1e-3);
    if let Some(p) = phrase.filter(|p| bars(t, p.start, sound_start) <= BACK_BARS) {
        return p.start.max(0.0);
    }
    // The bar line at or before the sound.
    let beat = t.beat_at(sound_start);
    let bar = (beat / 4.0).floor() * 4.0;
    t.seconds_at(bar).clamp(0.0, sound_start)
}

/// The phrase mix from `out` (whose sound ends at `sound_end`) into `inc`,
/// cued at `cue`; None when the outgoing track has no tempo, or neither an
/// imported fade-out point nor a phrase change 8 to 32 bars before its end.
pub fn plan(
    out: &Track,
    out_phrases: &[Phrase],
    sound_end: f64,
    inc: &Track,
    in_phrases: &[Phrase],
    cue: f64,
) -> Option<Plan> {
    if out.bpm <= 0.0 {
        return None;
    }
    let swap = |start: f64, end: f64| {
        let mid = (start + end) / 2.0;
        out_phrases
            .iter()
            .map(|p| p.start)
            .filter(|&s| s > start + 1e-3 && s < end - 1e-3)
            .min_by(|a, b| (a - mid).abs().total_cmp(&(b - mid).abs()))
    };
    if let Some(s) = point(out, FADE_OUT).filter(|&s| s >= 0.0 && bars(out, s, sound_end) >= 1.0) {
        return Some(Plan {
            start: s,
            end: sound_end,
            swap: swap(s, sound_end),
            from: Start::Imported,
        });
    }
    let outro = out_phrases
        .iter()
        .rev()
        .find(|p| p.kind == PhraseKind::Outro && p.start > 0.0)
        .filter(|p| bars(out, p.start, sound_end) >= MIN_BARS);
    let (start, from) = match outro {
        Some(p) => (p.start, Start::Outro),
        None => {
            let p = out_phrases.iter().rev().find(|p| {
                let b = bars(out, p.start, sound_end);
                p.start > 0.0 && (MIN_BARS..=MAX_BARS).contains(&b)
            })?;
            (p.start, Start::Phrase)
        }
    };
    let room = bars(out, start, sound_end).floor();
    // The incoming intro: from the cue to its next phrase change.
    let intro = in_phrases
        .iter()
        .find(|p| p.start > cue + 1e-3)
        .filter(|_| inc.bpm > 0.0)
        .map(|p| bars(inc, cue, p.start).round());
    let mut length = room.min(MAX_BARS);
    if let Some(i) = intro {
        length = length.min(i);
    }
    let length = ((length / 4.0).floor() * 4.0).max(MIN_BARS).min(room);
    let end = out
        .seconds_at(out.beat_at(start) + 4.0 * length)
        .min(sound_end);
    Some(Plan {
        start,
        end,
        swap: swap(start, end),
        from,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::track::Marker;

    /// A 120 BPM track (a bar is 2 s) with phrases starting on these bars.
    fn track(bars: f64, phrases: &[(f64, PhraseKind)]) -> (Track, Vec<Phrase>) {
        let mut t = (*djsugar_engine::synth::sample_library()[0]).clone();
        t.bpm = 120.0;
        t.first_beat = 0.0;
        t.grid = Default::default();
        t.duration = bars * 2.0;
        t.info.markers.clear();
        let p = phrases
            .iter()
            .map(|&(bar, kind)| Phrase {
                start: bar * 2.0,
                kind,
                energy: 0,
            })
            .collect();
        (t, p)
    }

    use PhraseKind::*;

    #[test]
    fn intro_over_outro() {
        let (out, op) = track(96.0, &[(0.0, Intro), (16.0, Drop), (80.0, Outro)]);
        let (inc, ip) = track(96.0, &[(0.0, Intro), (16.0, Drop)]);
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        // From the outro's first bar, as long as the intro: 16 bars.
        assert_eq!((p.start, p.end, p.from), (160.0, 192.0, Start::Outro));
    }

    #[test]
    fn a_short_intro_makes_a_short_mix_from_the_outro() {
        let (out, op) = track(96.0, &[(0.0, Intro), (16.0, Drop), (64.0, Outro)]);
        let (inc, ip) = track(96.0, &[(0.0, Intro), (8.0, Drop)]);
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        assert_eq!((p.start, p.end), (128.0, 144.0), "8 bars from bar 64");
        // An intro of 4 bars still gets 8; one of 40, at most 32.
        let (inc, ip) = track(96.0, &[(0.0, Intro), (4.0, Drop)]);
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        assert_eq!(p.end - p.start, 16.0);
        let (inc, ip) = track(96.0, &[(0.0, Intro), (40.0, Drop)]);
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        assert_eq!(p.end - p.start, 64.0);
    }

    #[test]
    fn without_an_outro_the_last_phrase_change_in_reach() {
        // No outro; the last change is 24 bars before the end.
        let (out, op) = track(96.0, &[(0.0, Intro), (16.0, Drop), (72.0, Chorus)]);
        let (inc, ip) = track(96.0, &[]);
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        assert_eq!((p.start, p.end, p.from), (144.0, 192.0, Start::Phrase));
        // A last change only 4 bars from the end, and none in reach: no plan.
        let (out, op) = track(96.0, &[(0.0, Intro), (92.0, Outro)]);
        assert!(plan(&out, &op, out.duration, &inc, &ip, 0.0).is_none());
    }

    #[test]
    fn an_imported_fade_out_point_wins() {
        let (mut out, op) = track(96.0, &[(0.0, Intro), (16.0, Drop), (80.0, Outro)]);
        out.info.markers.push(Marker {
            kind: MarkerKind::Automix,
            seconds: 170.0,
            end: None,
            name: FADE_OUT.into(),
            color: None,
            detected: false,
        });
        let (inc, ip) = track(96.0, &[(0.0, Intro), (16.0, Drop)]);
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        assert_eq!((p.start, p.end, p.from), (170.0, 192.0, Start::Imported));
    }

    #[test]
    fn bass_swaps_on_the_phrase_change_nearest_the_middle() {
        let (out, op) = track(96.0, &[(0.0, Intro), (64.0, Breakdown), (72.0, Outro)]);
        let (inc, ip) = track(96.0, &[(0.0, Intro), (16.0, Drop)]);
        // No outro of 8 bars... it has one: 24 bars from bar 72; intro caps at 16.
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        assert_eq!((p.start, p.end), (144.0, 176.0));
        assert_eq!(p.swap, None, "no change inside the mix");
        let (out, op) = track(96.0, &[(0.0, Intro), (64.0, Outro), (72.0, Breakdown)]);
        let p = plan(&out, &op, out.duration, &inc, &ip, 0.0).unwrap();
        assert_eq!((p.start, p.swap), (128.0, Some(144.0)));
    }

    #[test]
    fn the_incoming_track_starts_on_its_first_bar() {
        let (mut t, p) = track(96.0, &[(0.0, Intro), (16.0, Drop)]);
        // Sound a little after the downbeat: back to the phrase's start.
        assert_eq!(in_point(&t, &p, 0.3), 0.0);
        // Sound only from bar 5 of the intro: on its bar line.
        assert_eq!(in_point(&t, &p, 9.1), 8.0);
        t.info.markers.push(Marker {
            kind: MarkerKind::Automix,
            seconds: 4.0,
            end: None,
            name: REAL_START.into(),
            color: None,
            detected: false,
        });
        assert_eq!(in_point(&t, &p, 0.3), 4.0, "an imported start point wins");
    }
}
