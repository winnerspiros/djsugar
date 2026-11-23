//! Track analysis: the 3-band waveform summary, tempo, beat phase and key (see [`crate::key`] and [`crate::keynet`]). The tempo detector works on an onset envelope (rises in bass energy and overall level, 200 per second). It picks the strongest autocorrelation lag in the tempo range (60 to 180 BPM unless set otherwise, or the
//! octave around the track's genre's tempo; see [`genre_range`]), then refines it to 0.01 BPM by folding the envelope at each candidate period and keeping the one whose beats line up best. When the beats then fit no steady grid, the spectral flux (rises across the whole spectrum) gets a second say in the tempo. Live and older
//! recordings drift, so a steady grid misses their beats. A dynamic-programming beat tracker (Ellis, "Beat Tracking by Dynamic Programming", 2007) then follows the beats one by one, and a grid is fitted through them: steady unless the beats move away from one tempo for a while, then in tempo sections (see [`crate::gridfit`]).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::beats::BeatGrid;
use crate::dsp::Isolator;
use crate::gridfit::{self, Fitted};
use crate::key::{self, Detector, Key, KeyDetector};
use crate::keynet::Spectrogram;
use crate::loudness::{Loudness, LoudnessMeter};
use crate::track::{Audio, Waveform, WAVEFORM_RATE};

/// Onset envelope rate used for tempo and phase detection.
const ONSET_RATE: f64 = 200.0;

#[derive(Clone)]
pub struct Analysis {
    pub waveform: Waveform,
    /// Detected key, None for silence.
    pub key: Option<Key>,
    /// The detector that found `key`.
    pub key_detector: Detector,
    pub bpm: f64,
    pub first_beat: f64,
    /// The beat grid: steady at `bpm` from `first_beat`, or sections at
    /// different tempos when the track drifts. Empty without a tempo.
    pub grid: BeatGrid,
    /// The detected beats fit the grid poorly: worth checking by ear.
    pub grid_unsure: bool,
    /// The grid comes from Beat This!'s beats (see [`crate::beatthis`])
    /// rather than the DSP beat tracker's.
    pub neural_beats: bool,
    /// EBU R128 loudness and peak, for auto gain; None for silence.
    pub loudness: Option<Loudness>,
}

/// The tempo range tracks are analysed in, unless changed with
/// [`set_bpm_range`].
pub const DEFAULT_BPM_RANGE: (f64, f64) = (60.0, 180.0);
static BPM_RANGE: AtomicU64 = AtomicU64::new(pack_range(DEFAULT_BPM_RANGE));

const fn pack_range((lo, hi): (f64, f64)) -> u64 {
    ((lo as u64) << 32) | hi as u64
}

/// The tempo range tracks are analysed in, in whole BPM.
pub fn bpm_range() -> (f64, f64) {
    let v = BPM_RANGE.load(Ordering::Relaxed);
    ((v >> 32) as f64, (v & 0xffff_ffff) as f64)
}

/// Sets the tempo range for tracks analysed from now on. The top must be
/// nearly twice the bottom, so every tempo has a place in it.
pub fn set_bpm_range(lo: f64, hi: f64) {
    let lo = lo.clamp(40.0, 200.0).round();
    let hi = hi.round().max((lo * 1.9).ceil()).min(400.0);
    BPM_RANGE.store(pack_range((lo, hi)), Ordering::Relaxed);
}
/// What analysis is told about a track before it listens: a tempo from tags (see
/// [`tag_tempo`]) and the track's genre, which picks the octave tempos are read in
/// (see [`genre_range`]). `None` and `Some(bpm)` convert to a hint without a genre.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TempoHint {
    pub bpm: Option<f64>,
    pub genre: String,
}

impl From<Option<f64>> for TempoHint {
    fn from(bpm: Option<f64>) -> Self {
        TempoHint { bpm, genre: String::new() }
    }
}

impl TempoHint {
    /// The tempo range to read this track's tempo in: its genre's, or the
    /// one set with [`set_bpm_range`].
    pub fn range(&self) -> (f64, f64) {
        genre_range(&self.genre).unwrap_or_else(bpm_range)
    }

    /// The tempo the detector leans towards: the genre's, or 120 BPM.
    fn prior(&self) -> f64 {
        genre_tempo(&self.genre).unwrap_or(PRIOR_BPM)
    }
}

/// Typical tempos of genres whose tempo is clear, as (words in the genre,
/// tempo). The first match wins, so narrower genres come first. Genres that
/// span many tempos (pop, rock, electronic, dance) aren't here.
const GENRE_TEMPOS: &[(&[&str], f64)] = &[
    (&["drum & bass", "drum and bass", "drum n bass", "drum'n'bass", "dnb", "d&b", "jungle", "neurofunk"], 172.0),
    (&["footwork", "juke"], 160.0),
    (&["hardstyle", "rawstyle"], 150.0),
    (&["gabber", "hardcore", "frenchcore"], 170.0),
    (&["dubstep", "riddim", "brostep"], 140.0),
    (&["grime"], 140.0),
    (&["psytrance", "psy-trance", "psy trance", "goa"], 142.0),
    (&["trance"], 136.0),
    (&["uk garage", "2-step", "2 step", "speed garage"], 132.0),
    (&["breakbeat", "breaks"], 130.0),
    (&["techno"], 130.0),
    (&["moombahton"], 108.0),
    (&["reggaeton", "dembow", "dancehall"], 95.0),
    (&["hip hop", "hip-hop", "hiphop", "rap", "r&b", "rnb", "trap"], 90.0),
    (&["house", "garage", "disco", "big room", "electro"], 124.0),
    (&["ambient", "downtempo", "chillout", "chill out", "lounge", "trip hop", "trip-hop", "lo-fi", "lofi"], 85.0),
];

/// The tempo range for tracks of `genre`: the octave around the genre's typical tempo, so
/// half and double tempo readings fold to the one the genre is known by (drum & bass at 174
/// rather than 87, hip-hop at 90 rather than 180). None for an unknown or empty genre.
pub fn genre_range(genre: &str) -> Option<(f64, f64)> {
    genre_tempo(genre).map(genre_range_of)
}

/// The typical tempo of `genre`, from [`GENRE_TEMPOS`].
fn genre_tempo(genre: &str) -> Option<f64> {
    let genre = genre.to_lowercase();
    let has = |word: &str| {
        genre.match_indices(word).any(|(i, _)| {
            let before = genre[..i].chars().next_back();
            let after = genre[i + word.len()..].chars().next();
            // Whole words only, so "rap" isn't found in "trap" or "scraps".
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
    };
    GENRE_TEMPOS.iter().find(|(words, _)| words.iter().any(|w| has(w))).map(|&(_, tempo)| tempo)
}

/// The octave around `tempo`, in whole BPM.
fn genre_range_of(tempo: f64) -> (f64, f64) {
    let lo = (tempo / std::f64::consts::SQRT_2).round();
    (lo, 2.0 * lo)
}

const PHASE_BINS: usize = 64;

/// Per-slice accumulators: band peaks and sums of squares (low, mid, high, all).
#[derive(Clone, Copy, Default)]
struct Slice {
    peak: [f32; 4],
    power: [f32; 4],
    frames: u32,
}

impl Slice {
    fn rms(&self) -> [f32; 4] {
        let n = self.frames.max(1) as f32;
        self.power.map(|p| (p / n).sqrt())
    }
}

/// The tempo from tags in `bpm_hint`, if it is a plausible one; the analysis
/// then keeps it, steady.
pub fn tag_tempo(bpm_hint: Option<f64>) -> Option<f64> {
    bpm_hint.filter(|b| (40.0..=250.0).contains(b))
}

/// Analyses decoded audio. A tempo from the file's tags, if any, is used
/// instead of detection; the beat phase is always detected.
pub fn analyse(audio: &Audio, hint: impl Into<TempoHint>) -> Analysis {
    let mut a = Analyser::new(audio.sample_rate, audio.len());
    for chunk in audio.decoded_chunks() {
        a.feed(chunk);
    }
    a.finish(hint)
}

/// The key of decoded `audio` by `detector`, for a track whose other
/// analysis is already known.
pub fn detect_key(audio: &Audio, detector: Detector) -> Option<Key> {
    let sr = audio.sample_rate.max(1) as f64;
    let standard = || {
        let mut eq = Isolator::new(sr);
        let mut key = KeyDetector::new(sr);
        for f in audio.decoded_chunks().flatten() {
            let [lo, mid, _] = eq.split(*f);
            key.push((lo[0] + lo[1] + mid[0] + mid[1]) * 0.5);
        }
        key.key()
    };
    match detector {
        Detector::Standard => standard(),
        Detector::Neural => {
            let mut s = Spectrogram::new(sr);
            for f in audio.decoded_chunks().flatten() {
                s.push((f[0] + f[1]) * 0.5);
            }
            s.key().or_else(standard)
        }
    }
}

/// Analyses audio a piece at a time as it is decoded, so the waveform can be
/// drawn while the rest of the track is still being read.
pub struct Analyser {
    bin_len: f64,
    slice_len: f64,
    eq: Isolator,
    key: KeyDetector,
    /// The neural detector's input, when it is the one chosen.
    neural: Option<Spectrogram>,
    loudness: LoudnessMeter,
    flux: SpectralFlux,
    slices: Vec<Slice>,
    energy: Vec<[f32; 2]>,
    /// Frames fed so far.
    frames: usize,
}

impl Analyser {
    /// An analyser for audio of about `expected_frames` frames; the waveform
    /// spans that length from the start, so the part not analysed yet is flat.
    pub fn new(sample_rate: u32, expected_frames: usize) -> Self {
        let sr = sample_rate.max(1) as f64;
        let bin_len = sr / ONSET_RATE;
        let slice_len = sr / WAVEFORM_RATE;
        let slices = (expected_frames as f64 / slice_len).ceil().max(1.0) as usize;
        let bins = (expected_frames as f64 / bin_len).ceil().max(1.0) as usize;
        Analyser {
            bin_len,
            slice_len,
            eq: Isolator::new(sr),
            key: KeyDetector::new(sr),
            neural: (key::detector() == Detector::Neural).then(|| Spectrogram::new(sr)),
            loudness: LoudnessMeter::new(sr),
            flux: SpectralFlux::new(sr, bin_len, bins),
            slices: vec![Slice::default(); slices],
            energy: vec![[0.0; 2]; bins],
            frames: 0,
        }
    }

    /// Frames analysed so far.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Analyses the next frames of the track.
    pub fn feed(&mut self, frames: &[[f32; 2]]) {
        for f in frames {
            let i = self.frames;
            self.frames += 1;
            let b = (i as f64 / self.bin_len) as usize;
            let s = (i as f64 / self.slice_len) as usize;
            // The decoder's length can be an estimate; grow past it.
            if b >= self.energy.len() {
                self.energy.resize(b + 1, [0.0; 2]);
            }
            if s >= self.slices.len() {
                self.slices.resize(s + 1, Slice::default());
            }
            self.loudness.push(*f);
            let [lo, mid, hi] = self.eq.split(*f);
            self.key.push((lo[0] + lo[1] + mid[0] + mid[1]) * 0.5);
            let mono = (f[0] + f[1]) * 0.5;
            self.flux.push(mono);
            if let Some(n) = self.neural.as_mut() {
                n.push(mono);
            }
            let bands = [lo, mid, hi, *f];
            let slice = &mut self.slices[s];
            for (k, x) in bands.iter().enumerate() {
                slice.peak[k] = slice.peak[k].max(x[0].abs().max(x[1].abs()));
                slice.power[k] += (x[0] * x[0] + x[1] * x[1]) * 0.5;
            }
            slice.frames += 1;
            let lo_mono = (lo[0] + lo[1]) * 0.5;
            self.energy[b][0] += lo_mono * lo_mono;
            self.energy[b][1] += mono * mono;
        }
    }

    /// The waveform so far, scaled to the loudest peak so far.
    pub fn waveform(&self) -> Waveform {
        summarise(&self.slices)
    }

    /// Finishes the analysis: the final waveform, tempo and first beat.
    pub fn finish(mut self, hint: impl Into<TempoHint>) -> Analysis {
        let hint = hint.into();
        let (bpm_hint, range, prior) = (hint.bpm, hint.range(), hint.prior());
        // Drop the part of an over-estimated length that never came.
        let slices = (self.frames as f64 / self.slice_len).ceil().max(1.0) as usize;
        let bins = (self.frames as f64 / self.bin_len).ceil().max(1.0) as usize;
        self.slices.truncate(slices);
        self.energy.truncate(bins);
        let bin_len = self.bin_len;
        let energy = &self.energy;

        // Onset envelope: positive changes in log energy, bass weighted.
        let level = |e: f32| (1.0 + 1000.0 * (e / bin_len as f32).sqrt()).ln();
        let mut onset = vec![0.0f32; energy.len()];
        for i in 1..energy.len() {
            let d_lo = level(energy[i][0]) - level(energy[i - 1][0]);
            let d_all = level(energy[i][1]) - level(energy[i - 1][1]);
            onset[i] = d_lo.max(0.0) + 0.5 * d_all.max(0.0);
        }
        let mut waveform = summarise(&self.slices);
        waveform.chroma = self.key.timeline();
        let seconds = self.frames as f64 / bin_len / ONSET_RATE;
        let mut pick = match tag_tempo(bpm_hint) {
            // A tempo from tags is taken as steady.
            Some(bpm) => Pick::steady(&onset, bpm),
            None => Pick::at(&onset, detect_bpm(&onset, range, prior)),
        };
        // The bass and level can keep a rhythm that isn't the beat: a bass line playing every beat and a half reads as two thirds of the tempo, and its
        // beats then drift off any grid. Then the spectral flux, which also hears the soft attacks of pads, plucks and vocals, gets a say in the tempo;
        // its tempo is taken when its beats sit on one grid better. (On its own the flux follows hi-hats too readily, so it isn't the first opinion.)
        if tag_tempo(bpm_hint).is_none() && pick.bpm > 0.0 && pick.share < STEADY_SHARE {
            let flux = std::mem::take(&mut self.flux).finish(onset.len());
            let (sd_level, sd_flux) = (std_dev(&onset), std_dev(&flux));
            let both: Vec<f32> = onset.iter().zip(&flux).map(|(o, f)| o / sd_level + f / sd_flux).collect();
            let bpm = detect_bpm(&both, range, prior);
            if bpm > 0.0 && (bpm / pick.bpm - 1.0).abs() > 0.01 {
                let other = Pick::at(&onset, bpm);
                if other.share > pick.share + SHARE_MARGIN {
                    pick = other;
                }
            }
        }
        let Pick { bpm, first_beat, fitted, .. } = pick;
        // The tracked beats say whether the tempo really moves; if it
        // doesn't, the folded tempo and phase are the more precise.
        let grid_unsure = fitted.as_ref().is_some_and(|f| f.unsure);
        let grid = match fitted {
            _ if bpm <= 0.0 => BeatGrid::default(),
            Some(f) if f.grid.is_dynamic() => f.grid,
            _ => BeatGrid::constant(bpm, first_beat),
        };
        let (bpm, first_beat) =
            if grid.is_dynamic() { (grid.main_bpm(seconds), grid.first_beat()) } else { (bpm, first_beat) };
        let key_detector = match self.neural {
            Some(_) => Detector::Neural,
            None => Detector::Standard,
        };
        // Audio too short for the network gets the standard detector's key.
        let key = self.neural.and_then(Spectrogram::key).or_else(|| self.key.key());
        Analysis {
            waveform,
            key,
            key_detector,
            bpm,
            first_beat,
            grid,
            grid_unsure,
            neural_beats: false,
            loudness: self.loudness.finish(),
        }
    }
}

/// Quantises the slices; the coarser levels are built from the result.
fn summarise(slices: &[Slice]) -> Waveform {
    let top = slices.iter().map(|s| s.peak[3]).fold(1e-6f32, f32::max);
    // The bands carry less energy than the whole signal; lift them so the
    // colours read clearly.
    const BAND_GAIN: [f32; 4] = [1.0, 1.4, 2.0, 1.0];
    let q = |v: f32, k: usize| ((v * BAND_GAIN[k] / top).clamp(0.0, 1.0).sqrt() * 255.0).round() as u8;
    let finest = slices
        .iter()
        .map(|s| {
            let rms = s.rms();
            std::array::from_fn(|i| if i < 4 { q(s.peak[i], i) } else { q(rms[i - 4], i - 4) })
        })
        .collect();
    Waveform::from_finest(WAVEFORM_RATE, finest)
}

/// Standard deviation of `v`; never zero, so it can divide.
fn std_dev(v: &[f32]) -> f32 {
    let n = v.len().max(1) as f32;
    let mean = v.iter().sum::<f32>() / n;
    (v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n).sqrt().max(1e-9)
}

/// Samples in each spectrum of the spectral flux (about 43 ms at 48 kHz).
const FLUX_FFT: usize = 2048;
/// The highest frequency the flux listens to; above it is mostly hiss and
/// cymbal wash.
const FLUX_TOP_HZ: f64 = 11_000.0;
/// Half the window (in seconds) of the running mean taken off the flux, so
/// a busy stretch doesn't outvote the beats in a quiet one.
const FLUX_MEAN_SECS: f64 = 0.5;

/// Spectral flux, one value per onset bin: how much the log-magnitude spectrum rises from
/// one bin to the next, summed over frequency (as the Queen Mary and librosa onset detectors
/// do). Fed the mono signal a sample at a time; each spectrum is centred on its bin.
#[derive(Default)]
struct SpectralFlux {
    fft: Option<Arc<dyn Fft<f32>>>,
    window: Vec<f32>,
    /// The last [`FLUX_FFT`] samples.
    ring: Vec<f32>,
    /// Samples pushed so far.
    pos: usize,
    bin_len: f64,
    /// The log magnitudes of the last spectrum, up to [`FLUX_TOP_HZ`].
    last: Vec<f32>,
    buf: Vec<Complex32>,
    values: Vec<f32>,
}

impl SpectralFlux {
    fn new(sample_rate: f64, bin_len: f64, bins: usize) -> Self {
        let top = ((FLUX_FFT as f64 * FLUX_TOP_HZ / sample_rate) as usize).clamp(2, FLUX_FFT / 2);
        SpectralFlux {
            fft: Some(FftPlanner::new().plan_fft_forward(FLUX_FFT)),
            window: (0..FLUX_FFT)
                .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FLUX_FFT as f32).cos())
                .collect(),
            ring: vec![0.0; FLUX_FFT],
            pos: 0,
            bin_len,
            last: vec![0.0; top],
            buf: vec![Complex32::default(); FLUX_FFT],
            values: Vec::with_capacity(bins),
        }
    }

    /// Where the spectrum of the next bin starts, in samples (negative for
    /// the first few, which start before the track).
    fn next_start(&self) -> isize {
        (self.values.len() as f64 * self.bin_len) as isize - (FLUX_FFT / 2) as isize
    }

    fn push(&mut self, x: f32) {
        self.ring[self.pos % FLUX_FFT] = x;
        self.pos += 1;
        while self.next_start() + FLUX_FFT as isize <= self.pos as isize {
            self.spectrum();
        }
    }

    /// The next bin's flux, from the samples pushed so far (silence before
    /// the track and after the last sample).
    fn spectrum(&mut self) {
        let Some(fft) = self.fft.clone() else {
            return;
        };
        let start = self.next_start();
        for (k, c) in self.buf.iter_mut().enumerate() {
            let j = start + k as isize;
            let x = if j >= 0 && (j as usize) < self.pos { self.ring[j as usize % FLUX_FFT] } else { 0.0 };
            *c = Complex32::new(x * self.window[k], 0.0);
        }
        fft.process(&mut self.buf);
        let mut rise = 0.0;
        for (last, c) in self.last.iter_mut().zip(&self.buf[1..]) {
            let l = (1.0 + 100.0 * c.norm()).ln();
            rise += (l - *last).max(0.0);
            *last = l;
        }
        self.values.push(rise);
    }

    /// The flux for `bins` onset bins, less its running mean.
    fn finish(mut self, bins: usize) -> Vec<f32> {
        if self.fft.is_none() {
            return vec![0.0; bins];
        }
        while self.values.len() < bins {
            self.spectrum();
        }
        self.values.truncate(bins);
        let half = (FLUX_MEAN_SECS * ONSET_RATE) as usize;
        let mut sums = Vec::with_capacity(bins + 1);
        sums.push(0.0f64);
        for &v in &self.values {
            sums.push(sums.last().unwrap() + v as f64);
        }
        (0..bins)
            .map(|i| {
                let (lo, hi) = (i.saturating_sub(half), (i + half + 1).min(bins));
                let mean = (sums[hi] - sums[lo]) / (hi - lo) as f64;
                (self.values[i] - mean as f32).max(0.0)
            })
            .collect()
    }
}

/// Folds the envelope at `period` (in bins) and returns the best beat phase
/// (0..1) and how strongly the beats line up there.
fn fold(onset: &[f32], period: f64) -> (f64, f32) {
    // Each phase bin holds the mean onset of the bins that fold into it: a
    // period near a whole number of bins folds more of them into some phase
    // bins than others, and plain sums would make those tempos look strong.
    let mut hist = [0.0f32; PHASE_BINS];
    let mut count = [0u32; PHASE_BINS];
    for (i, &o) in onset.iter().enumerate() {
        let ph = (i as f64 / period).fract();
        let k = (ph * PHASE_BINS as f64) as usize % PHASE_BINS;
        hist[k] += o.max(0.0);
        count[k] += 1;
    }
    for (h, &c) in hist.iter_mut().zip(&count) {
        *h /= c.max(1) as f32;
    }
    let total: f32 = hist.iter().sum::<f32>().max(1e-9);
    let mut best = (0, 0.0f32);
    for k in 0..PHASE_BINS {
        let s = hist[(k + PHASE_BINS - 1) % PHASE_BINS] * 0.5 + hist[k] + hist[(k + 1) % PHASE_BINS] * 0.5;
        if s > best.1 {
            best = (k, s);
        }
    }
    // Centre of mass around the peak for sub-bin accuracy.
    let (mut w, mut acc) = (0.0f64, 0.0f64);
    for d in -1i32..=1 {
        let k = (best.0 as i32 + d).rem_euclid(PHASE_BINS as i32) as usize;
        w += hist[k] as f64;
        acc += hist[k] as f64 * (best.0 as f64 + d as f64 + 0.5);
    }
    let phase = if w > 0.0 { acc / w } else { best.0 as f64 + 0.5 } / PHASE_BINS as f64;
    (phase.rem_euclid(1.0), best.1 / total)
}

/// Centre and width (standard deviation) of the tempo detector's lean.
const PRIOR_BPM: f64 = 120.0;
const PRIOR_OCTAVES: f64 = 1.0;

/// The tempo of `onset` in `(min_bpm, max_bpm)`, leaning towards `prior`.
fn detect_bpm(onset: &[f32], (min_bpm, max_bpm): (f64, f64), prior: f64) -> f64 {
    let lag_of = |bpm: f64| ONSET_RATE * 60.0 / bpm;
    let (min_lag, max_lag) = (lag_of(max_bpm).floor() as usize, lag_of(min_bpm).ceil() as usize);
    if onset.len() < max_lag * 4 {
        return 0.0;
    }
    let mean = onset.iter().sum::<f32>() / onset.len() as f32;
    let centred: Vec<f32> = onset.iter().map(|o| o - mean).collect();
    let mut best = (min_lag, f32::MIN);
    for lag in min_lag..=max_lag {
        // Also reward the lag's multiples so bars outvote syncopation.
        let mut s = 0.0;
        for k in 1..=4 {
            let l = lag * k;
            if l >= centred.len() {
                break;
            }
            let c: f32 = centred[l..].iter().zip(&centred[..centred.len() - l]).map(|(a, b)| a * b).sum();
            s += c / k as f32;
        }
        // Lean towards tempos near the prior (120 BPM or the genre's), so a half-time reading only
        // wins when it is clearly stronger.
        let octaves = (ONSET_RATE * 60.0 / lag as f64 / prior).log2();
        let w = (-0.5 * (octaves / PRIOR_OCTAVES).powi(2)).exp() as f32;
        let s = if s > 0.0 { s * w } else { s / w };
        if s > best.1 {
            best = (lag, s);
        }
    }
    let coarse = ONSET_RATE * 60.0 / best.0 as f64;

    // Refine around the coarse tempo (one lag step either side).
    let lo = (ONSET_RATE * 60.0 / (best.0 as f64 + 1.0)).max(min_bpm);
    let hi = (ONSET_RATE * 60.0 / (best.0 as f64 - 1.0)).min(max_bpm);
    let mut fine = (coarse, f32::MIN);
    let mut bpm = lo;
    while bpm <= hi {
        let (_, score) = fold(onset, lag_of(bpm));
        if score > fine.1 {
            fine = (bpm, score);
        }
        bpm += 0.01;
    }
    let b = fine.0;
    if (b - b.round()).abs() < 0.06 {
        b.round()
    } else {
        (b * 100.0).round() / 100.0
    }
}

/// How strong an onset (in standard deviations) marks a beat clearly.
const CLEAR_ONSET: f64 = 2.0;

/// Beat times (in onset bins) found by dynamic programming: each beat is the strongest onset
/// reachable from the best path to it, with a penalty for spacing away from `period` (Ellis 2007).
/// Each beat comes with whether a clear onset marks it; through a breakdown the path only guesses.
fn track_beats(onset: &[f32], period: f64) -> Vec<(usize, bool)> {
    let n = onset.len();
    if n < (period * 8.0) as usize {
        return Vec::new();
    }
    let mean = onset.iter().sum::<f32>() / n as f32;
    let sd = (onset.iter().map(|o| (o - mean).powi(2)).sum::<f32>() / n as f32).sqrt().max(1e-9);
    let norm: Vec<f64> = onset.iter().map(|o| (*o / sd) as f64).collect();
    // How tightly beats keep to the period; Ellis's default.
    const TIGHTNESS: f64 = 100.0;
    let (lo, hi) = ((period * 0.5).round() as usize, (period * 2.0).round() as usize);
    let mut score = vec![0.0f64; n];
    let mut from = vec![usize::MAX; n];
    for i in 0..n {
        let mut best = (0.0, usize::MAX);
        let first = i.saturating_sub(hi);
        let last = i.saturating_sub(lo).min(i);
        for (prev, &sp) in score[first..last].iter().enumerate().map(|(k, s)| (k + first, s)) {
            let gap = (i - prev) as f64 / period;
            let s = sp - TIGHTNESS * gap.ln().powi(2);
            if best.1 == usize::MAX || s > best.0 {
                best = (s, prev);
            }
        }
        score[i] = norm[i] + best.0.max(0.0);
        from[i] = if best.0 > 0.0 { best.1 } else { usize::MAX };
    }
    // End on the best-scoring beat in the last period.
    let tail = n.saturating_sub(period.ceil() as usize);
    let mut i = (tail..n).max_by(|&a, &b| score[a].total_cmp(&score[b])).unwrap();
    let mut beats = vec![i];
    while from[i] != usize::MAX {
        i = from[i];
        beats.push(i);
    }
    beats.reverse();
    // A beat is clear when a strong onset sits right on it.
    beats
        .into_iter()
        .map(|b| {
            let around = &norm[b.saturating_sub(2)..(b + 3).min(n)];
            (b, around.iter().copied().fold(0.0, f64::max) >= CLEAR_ONSET)
        })
        .collect()
}

/// Share of the clearly marked beats that must fall on the steady grid for
/// the track to keep it.
const STEADY_SHARE: f64 = 0.75;
/// How much more of its beats a second opinion on the tempo must hold on
/// its steady grid to be taken.
const SHARE_MARGIN: f64 = 0.15;

/// A tempo and the grid its tracked beats fit.
struct Pick {
    bpm: f64,
    first_beat: f64,
    /// The share of the clearly marked beats on the steady grid.
    share: f64,
    /// The grid fitted to the beats, when the steady grid doesn't hold
    /// them.
    fitted: Option<Fitted>,
}

impl Pick {
    /// Steady at `bpm`, from the best phase.
    fn steady(onset: &[f32], bpm: f64) -> Pick {
        let first_beat = if bpm > 0.0 {
            let period = ONSET_RATE * 60.0 / bpm;
            let (phase, _) = fold(onset, period);
            phase * period / ONSET_RATE
        } else {
            0.0
        };
        Pick { bpm, first_beat, share: 1.0, fitted: None }
    }

    /// At `bpm`, steady unless the beats [`track_beats`] finds keep off the
    /// steady grid; then fitted to them (see [`gridfit`]).
    fn at(onset: &[f32], bpm: f64) -> Pick {
        let mut pick = Pick::steady(onset, bpm);
        if bpm <= 0.0 {
            return pick;
        }
        let beats: Vec<f64> = track_beats(onset, ONSET_RATE * 60.0 / bpm)
            .into_iter()
            .filter(|&(_, clear)| clear)
            .map(|(b, _)| b as f64 / ONSET_RATE)
            .collect();
        pick.share = gridfit::share_on(&beats, &BeatGrid::constant(bpm, pick.first_beat));
        if pick.share < STEADY_SHARE {
            pick.fitted = gridfit::fit(&beats, None, bpm);
        }
        pick
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth;

    #[test]
    fn finds_tempo_and_first_beat_of_synth_track() {
        for (bpm, first_beat) in [(124.0, 0.21), (128.0, 0.0), (97.0, 0.35)] {
            let audio = synth::render_with(bpm, first_beat, 60.0, 7);
            let a = analyse(&audio, None);
            assert!((a.bpm - bpm).abs() < 0.02, "{bpm}: got {}", a.bpm);
            let beat = 60.0 / bpm;
            let err = (a.first_beat - first_beat).rem_euclid(beat);
            let err = err.min(beat - err);
            assert!(err < 0.015, "{bpm}: first beat {} vs {first_beat}", a.first_beat);
            let w = &a.waveform;
            assert_eq!(w.levels[0].len(), (60.0 * WAVEFORM_RATE) as usize);
            assert_eq!(w.levels.last().unwrap().len(), 1);
            let max = |k: usize| w.levels[0].iter().map(|b| b[k]).max().unwrap();
            assert!(max(0) > 180 && max(1) > 80 && max(2) > 80, "{} {} {}", max(0), max(1), max(2));
            // Peaks are never below RMS, and coarser levels keep the peak.
            assert!(w.levels[0].iter().all(|b| (0..4).all(|k| b[k] >= b[k + 4])));
            assert_eq!(w.levels.last().unwrap()[0][3], max(3));
        }
    }

    #[test]
    fn analysing_piece_by_piece_matches_all_at_once() {
        let audio = synth::render_with(128.0, 0.05, 30.0, 5);
        let whole = analyse(&audio, None);
        let frames: Vec<[f32; 2]> = audio.decoded_chunks().flatten().copied().collect();
        // Overestimate the length, as a decoder's estimate may.
        let mut a = Analyser::new(audio.sample_rate, audio.len() + 5_000);
        let partial = {
            a.feed(&frames[..1000]);
            a.waveform()
        };
        assert_eq!(partial.levels[0].len(), whole.waveform.levels[0].len() + 114);
        for piece in frames[1000..].chunks(777) {
            a.feed(piece);
        }
        let pieces = a.finish(None);
        assert_eq!(pieces.bpm, whole.bpm);
        assert_eq!(pieces.first_beat, whole.first_beat);
        assert_eq!(pieces.waveform.levels, whole.waveform.levels);
    }

    #[test]
    fn steady_tracks_get_a_steady_grid() {
        for bpm in [124.0, 97.0] {
            let audio = synth::render_with(bpm, 0.2, 60.0, 9);
            let a = analyse(&audio, None);
            assert!(!a.grid.is_dynamic(), "{bpm}: {:?}", a.grid);
            assert!((a.grid.bpm_at(30.0) - a.bpm).abs() < 1e-9);
        }
    }

    #[test]
    fn follows_a_tempo_change() {
        // 64 beats at 120 BPM (32 s), then 60 s at 126 BPM.
        let first = synth::render_with(120.0, 0.0, 32.0, 4);
        let second = synth::render_with(126.0, 0.0, 60.0, 4);
        let frames: Vec<[f32; 2]> = first.decoded_chunks().chain(second.decoded_chunks()).flatten().copied().collect();
        let audio = Audio::from_frames(first.sample_rate, frames);
        let a = analyse(&audio, None);
        assert!(a.grid.is_dynamic(), "{:?}", a.grid);
        assert!((a.grid.bpm_at(10.0) - 120.0).abs() < 0.3, "{:?}", a.grid);
        assert!((a.grid.bpm_at(70.0) - 126.0).abs() < 0.3, "{:?}", a.grid);
        assert!((a.bpm - a.grid.bpm_at(70.0)).abs() < 1e-9);
        // Beats on the second part land on the grid.
        for k in 0..20 {
            let t = 32.0 + 60.0 / 126.0 * (k as f64 * 5.0);
            let off = (t - a.grid.snap(t, false)).abs();
            assert!(off < 0.02, "beat at {t} is {off} s off the grid {:?}", a.grid);
        }
    }

    /// A 90 BPM track whose bass plays across the beat (a beat and a half
    /// apart, then one) under a shaker on each beat, much as Peace Sine's
    /// "Ancient Sands" does: the bass and level alone hear 120 BPM.
    #[test]
    fn hears_the_beat_over_a_bass_across_it() {
        let sr = 44_100u32;
        let secs = 60.0;
        let beat = 60.0 / 90.0;
        let mut frames = vec![[0.0f32; 2]; (secs * sr as f64) as usize];
        let tone = |frames: &mut [[f32; 2]], at: f64, hz: f64, amp: f64, decay: f64, len: f64| {
            let start = (at * sr as f64) as usize;
            for i in 0..(len * sr as f64) as usize {
                let Some(f) = frames.get_mut(start + i) else {
                    break;
                };
                let t = i as f64 / sr as f64;
                let x = (amp * (-t / decay).exp() * (std::f64::consts::TAU * hz * t).sin()) as f32;
                f[0] += x;
                f[1] += x;
            }
        };
        // The bass on beats 1, 2½ and 4 of each bar: a beat and a half
        // apart, then one.
        let mut bar = 0.0;
        while bar < secs {
            for at in [0.0, 1.5, 3.0] {
                tone(&mut frames, bar + at * beat, 50.0, 0.3, 0.25, 0.9);
            }
            bar += 4.0 * beat;
        }
        let mut k = 0.0;
        while k * beat < secs {
            // A soft shaker: a short burst of noise.
            let amp = 0.1;
            let start = ((k * beat) * sr as f64) as usize;
            let mut seed = 0x2545_f491_u32.wrapping_add(k as u32);
            for i in 0..(0.08 * sr as f64) as usize {
                let Some(f) = frames.get_mut(start + i) else {
                    break;
                };
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = seed as f64 / u32::MAX as f64 * 2.0 - 1.0;
                let x = (amp * (-(i as f64) / (0.02 * sr as f64)).exp() * noise) as f32;
                f[0] += x;
                f[1] += x;
            }
            k += 1.0;
        }
        let audio = Audio::from_frames(sr, frames);
        let a = analyse(&audio, None);
        assert!((a.bpm - 90.0).abs() < 0.05, "{}", a.bpm);
        assert!(!a.grid.is_dynamic(), "{:?}", a.grid);
    }

    #[test]
    fn uses_tag_tempo() {
        let audio = synth::render_with(126.0, 0.1, 20.0, 3);
        assert_eq!(analyse(&audio, Some(126.0)).bpm, 126.0);
    }

    #[test]
    fn a_genre_picks_the_octave_its_tempo_is_known_by() {
        assert_eq!(genre_range("Drum & Bass"), Some((122.0, 244.0)));
        assert_eq!(genre_range("Liquid Drum and Bass"), Some((122.0, 244.0)));
        assert_eq!(genre_range("Hip-Hop"), Some((64.0, 128.0)));
        assert_eq!(genre_range("Dubstep"), Some((99.0, 198.0)));
        assert_eq!(genre_range("Tech House"), Some((88.0, 176.0)));
        assert_eq!(genre_range("Ambient"), Some((60.0, 120.0)));
        assert_eq!(genre_range("Trap"), genre_range("Rap"));
        // Whole words only: "rap" isn't found in "scraps".
        assert_eq!(genre_range("Scraps"), None);
        // Electronic isn't electro, and wide genres keep the set range.
        assert_eq!(genre_range("Electronic"), None);
        assert_eq!(genre_range("Pop"), None);
        assert_eq!(genre_range(""), None);
        // Every range is an octave, so each tempo has one place in it.
        for (_, tempo) in GENRE_TEMPOS {
            let (lo, hi) = genre_range_of(*tempo);
            assert_eq!(hi, 2.0 * lo);
            assert!((lo..hi).contains(tempo));
        }
        let hint = TempoHint { bpm: None, genre: "Pop".into() };
        assert_eq!(hint.range(), bpm_range());
    }

    #[test]
    fn a_genre_moves_the_tempo_detectors_lean() {
        let hint = |genre: &str| TempoHint { bpm: None, genre: genre.into() };
        assert_eq!(hint("").prior(), PRIOR_BPM);
        assert_eq!(hint("Jungle").prior(), 172.0);
        assert_eq!(hint("Hip Hop").prior(), 90.0);
    }

    #[test]
    fn a_genre_folds_a_detected_tempo_into_its_octave() {
        let audio = synth::render_with(87.0, 0.1, 20.0, 5);
        let dnb = TempoHint { bpm: None, genre: "Drum & Bass".into() };
        assert!((analyse(&audio, dnb).bpm - 174.0).abs() < 0.1);
        assert!((analyse(&audio, None).bpm - 87.0).abs() < 0.1);
        // A tempo from tags still wins.
        let tagged = TempoHint { bpm: Some(87.0), genre: "Drum & Bass".into() };
        assert_eq!(analyse(&audio, tagged).bpm, 87.0);
    }
}
