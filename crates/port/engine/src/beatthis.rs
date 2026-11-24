//! Neural beat and downbeat detection with Beat This! (Foscarin, Schlüter and Widmer, ISMIR 2024; MIT licence), run through the `beat-this` crate on its pure-Rust runtime. The
//! DSP analysis gives a track its tempo and a first grid within a second or so; this runs after it, finds each beat and downbeat, and fits the grid again through them (see `crate::gridfit`),
//! so bar 1 lands on a downbeat. Its two model files are downloaded the first time they are needed, as the stem models are; without them the DSP grid stays.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use beat_this::{BeatThis, RtenRuntime, Runtime};

use crate::analysis::{tag_tempo, Analysis, TempoHint};
use crate::beats::BeatGrid;
use crate::gridfit;
use crate::separation::model_or_fetch;
use crate::track::Audio;

/// The log-mel front end Beat This! runs on.
pub const MEL: &str = "beat-this-mel.onnx";
/// The small beat model (2M parameters).
pub const SMALL: &str = "beat-this-small.onnx";

/// Each model file and its SHA-256, as the `stem-models` release of DJ Sugar
/// has them (`.github/workflows/stem-models.yml`).
pub const MODELS: [(&str, &str); 2] = [
    (MEL, "fdd59e65c515331308e4c8841edf99972deca646bdf6197744c2a5b7755e3de9"),
    (SMALL, "a5f8d39d989f31859454ba27afe61c5317ca95e4d9373e6853e5361b8937172f"),
];

static ENABLED: AtomicBool = AtomicBool::new(true);
/// The models couldn't be had (no network, say): stop trying until Beat
/// This! is turned on again, so each track isn't decoded again for nothing.
static UNAVAILABLE: AtomicBool = AtomicBool::new(false);

/// Whether tracks get their beats from Beat This!, unless changed with
/// [`set_enabled`].
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Sets whether tracks analysed from now on get their beats from Beat This!.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    UNAVAILABLE.store(false, Ordering::Relaxed);
}

/// Whether `analysis` should be refined with [`refine`]: it has a tempo, and
/// its beats came from the DSP analysis while Beat This! is on.
pub fn wanted(analysis: &Analysis) -> bool {
    enabled() && !UNAVAILABLE.load(Ordering::Relaxed) && !analysis.neural_beats && analysis.bpm > 0.0
}

/// Beat and downbeat times (seconds) in the decoded `audio`.
fn detect(audio: &Audio) -> Result<(Vec<f64>, Vec<f64>), String> {
    // Loaded once; one track at a time.
    type Model = <RtenRuntime as Runtime>::Model;
    static TRACKER: Mutex<Option<BeatThis<Model>>> = Mutex::new(None);
    let mut tracker = TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    if tracker.is_none() {
        let loaded = model_or_fetch(MEL, &MODELS).and_then(|mel| {
            let small = model_or_fetch(SMALL, &MODELS)?;
            BeatThis::new(&RtenRuntime, &mel, &small).map_err(|e| e.to_string())
        });
        match loaded {
            Ok(bt) => *tracker = Some(bt),
            Err(e) => {
                UNAVAILABLE.store(true, Ordering::Relaxed);
                return Err(e);
            }
        }
    }
    if crate::background::is_low() {
        crate::background::lower_rten_priority();
    }
    let bt = tracker.as_mut().unwrap();
    let mut stream = bt.stream(audio.sample_rate).map_err(|e| e.to_string())?;
    let mut mono = Vec::new();
    for chunk in audio.decoded_chunks() {
        mono.clear();
        mono.extend(chunk.iter().map(|f| (f[0] + f[1]) * 0.5));
        stream.push(&mono).map_err(|e| e.to_string())?;
    }
    let found = stream.finish().map_err(|e| e.to_string())?;
    let secs = |v: Vec<f32>| v.into_iter().map(f64::from).collect();
    Ok((secs(found.beats), secs(found.downbeats)))
}

/// Gives `analysis` the grid Beat This!'s beats and downbeats fit, at the tempo of those beats folded into the BPM range (the
/// DSP tempo can be far off on slow, soft music, so it isn't used); the range is the track's genre's when it has a known one.
/// A tempo from tags (in `hint`, as [`crate::analysis::analyse`] takes it) stays steady, with bar 1 moved onto a downbeat.
pub fn refine(analysis: &mut Analysis, audio: &Audio, hint: impl Into<TempoHint>) -> Result<(), String> {
    let hint = hint.into();
    let bpm_hint = hint.bpm;
    let (beats, downbeats) = detect(audio)?;
    let (beats, bpm) = in_range(beats, &downbeats, hint.range());
    let fitted = gridfit::fit(&beats, Some(&downbeats), bpm).ok_or("too few beats to fit a grid")?;
    let grid = match tag_tempo(bpm_hint) {
        Some(bpm) => BeatGrid::constant(bpm, fitted.grid.markers()[0].seconds),
        None => fitted.grid,
    };
    analysis.bpm = grid.main_bpm(audio.duration());
    analysis.first_beat = grid.first_beat();
    analysis.grid = grid;
    analysis.grid_unsure = fitted.unsure;
    analysis.neural_beats = true;
    Ok(())
}

/// The tempo of `beats` (from the median gap) and the beats at that tempo,
/// halved or doubled into `range`: halving keeps the beats that fall on
/// more downbeats, doubling adds one between each two.
fn in_range(mut beats: Vec<f64>, downbeats: &[f64], (lo, hi): (f64, f64)) -> (Vec<f64>, f64) {
    let tempo = |beats: &[f64]| {
        let mut gaps: Vec<f64> = beats.windows(2).map(|w| w[1] - w[0]).collect();
        if gaps.is_empty() {
            return 0.0;
        }
        let mid = gaps.len() / 2;
        60.0 / *gaps.select_nth_unstable_by(mid, f64::total_cmp).1
    };
    let mut bpm = tempo(&beats);
    // Twice is enough for any range at least an octave wide.
    for _ in 0..2 {
        if bpm > hi {
            let on_downbeat = |&b: &f64| downbeats.iter().any(|d| (d - b).abs() < 0.07);
            let odd = beats.iter().skip(1).step_by(2).filter(|b| on_downbeat(b)).count();
            let even = beats.iter().step_by(2).filter(|b| on_downbeat(b)).count();
            beats = beats.into_iter().skip(usize::from(odd > even)).step_by(2).collect();
        } else if bpm > 0.0 && bpm < lo {
            let mids: Vec<f64> = beats.windows(2).map(|w| (w[0] + w[1]) / 2.0).collect();
            beats.extend(mids);
            beats.sort_by(f64::total_cmp);
        } else {
            break;
        }
        bpm = tempo(&beats);
    }
    (beats, bpm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyse;
    use crate::synth;

    #[test]
    fn folds_the_beats_tempo_into_the_range() {
        let at = |bpm: f64, n: usize| (0..n).map(|k| 0.3 + k as f64 * 60.0 / bpm).collect::<Vec<_>>();
        let (beats, bpm) = in_range(at(67.2, 100), &[], (60.0, 180.0));
        assert_eq!(beats.len(), 100);
        assert!((bpm - 67.2).abs() < 0.01, "{bpm}");
        // Too fast: every other beat, the ones on downbeats.
        let fast = at(200.0, 100);
        let downbeats: Vec<f64> = fast.iter().skip(1).step_by(8).copied().collect();
        let (beats, bpm) = in_range(fast.clone(), &downbeats, (60.0, 180.0));
        assert!((bpm - 100.0).abs() < 0.01, "{bpm}");
        assert_eq!(beats[0], fast[1]);
        // Too slow: a beat between each two.
        let (beats, bpm) = in_range(at(62.0, 50), &[], (70.0, 180.0));
        assert!((bpm - 124.0).abs() < 0.01, "{bpm}");
        assert_eq!(beats.len(), 99);
    }

    /// Runs only where the models are in `DJSUGAR_MODELS` (they aren't
    /// downloaded in tests).
    #[test]
    fn puts_bar_one_on_a_downbeat() {
        if MODELS.iter().any(|(n, _)| crate::separation::model(n).is_none()) {
            eprintln!("skipped: no Beat This! models in DJSUGAR_MODELS");
            return;
        }
        let audio = synth::render_with(124.0, 0.3, 40.0, 7);
        let mut a = analyse(&audio, None);
        refine(&mut a, &audio, None).unwrap();
        assert!(a.neural_beats);
        assert!(!a.grid.is_dynamic(), "{:?}", a.grid);
        assert!((a.bpm - 124.0).abs() < 0.1, "{}", a.bpm);
        let beat = 60.0 / 124.0;
        let off = (a.first_beat - 0.3).rem_euclid(beat);
        assert!(off.min(beat - off) < 0.03, "first beat {}", a.first_beat);
    }
}
