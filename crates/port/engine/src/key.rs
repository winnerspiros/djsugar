//! Musical key detection, after the Queen Mary key detector: a chromagram with three bins per semitone from C3 to C7, averaged over the
//! track, correlated with the major and minor chord profiles of Noland and Sandler (the ones in qm-dsp's `GetKeyMode`) in all 24 keys. Audio
//! arrives at the analyser's full rate; the low and mid bands (below about 2.5 kHz) are mixed to mono and decimated to around 11 kHz first.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

const BINS_PER_OCTAVE: usize = 36;
/// Samples per spectrum: 0.74 s at 11 kHz, fine enough to tell thirds of a
/// semitone apart at C3.
const FRAME: usize = 8192;
/// MIDI notes the chromagram covers: C3 up to C7.
const LOWEST_NOTE: f32 = 48.0;
const HIGHEST_NOTE: f32 = 96.0;

// qm-dsp's chord profiles. Bin 1 of each semitone's three is its centre;
// bin 1 is C.
const MAJOR: [f64; BINS_PER_OCTAVE] = [
    0.0384, 0.0629, 0.0258, 0.0121, 0.0146, 0.0106, 0.0364, 0.0610, 0.0267, 0.0126, 0.0121, 0.0086, 0.0364, 0.0623,
    0.0279, 0.0275, 0.0414, 0.0186, 0.0173, 0.0248, 0.0145, 0.0364, 0.0631, 0.0262, 0.0129, 0.0150, 0.0098, 0.0312,
    0.0521, 0.0235, 0.0129, 0.0142, 0.0095, 0.0289, 0.0478, 0.0239,
];
const MINOR: [f64; BINS_PER_OCTAVE] = [
    0.0375, 0.0682, 0.0299, 0.0119, 0.0138, 0.0093, 0.0296, 0.0543, 0.0257, 0.0292, 0.0519, 0.0246, 0.0159, 0.0234,
    0.0135, 0.0291, 0.0544, 0.0248, 0.0137, 0.0176, 0.0104, 0.0352, 0.0670, 0.0302, 0.0222, 0.0349, 0.0164, 0.0174,
    0.0297, 0.0166, 0.0222, 0.0401, 0.0202, 0.0175, 0.0270, 0.0146,
];

/// A key: `tonic` 0..12 from C, major or minor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub tonic: u8,
    pub minor: bool,
}

impl Key {
    /// The key in Camelot notation, as the library and decks show keys
    /// ("8B" is C major, "8A" A minor).
    pub fn camelot(self) -> String {
        // Relative majors share a number; each step of a fifth adds one.
        let major_tonic = if self.minor { self.tonic + 3 } else { self.tonic } as usize;
        let number = (major_tonic * 7 + 7) % 12 + 1;
        format!("{number}{}", if self.minor { 'A' } else { 'B' })
    }

    /// The key as notes, spelt with sharps ("C#", "A#m") unless the settings
    /// pick flats ("Db", "Bbm"); see [`set_notation`].
    pub fn name(self) -> String {
        self.spelt(notation() == Notation::Flats)
    }

    /// The key as notes, with sharps or flats for the black keys.
    pub fn spelt(self, flats: bool) -> String {
        const SHARPS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        const FLATS: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
        let note = if flats { FLATS } else { SHARPS }[self.tonic as usize];
        format!("{note}{}", if self.minor { "m" } else { "" })
    }

    /// The key in Open Key notation: "1d" is C major, "1m" A
    /// minor.
    pub fn open_key(self) -> String {
        let n = (self.camelot_number() + 4) % 12 + 1;
        format!("{n}{}", if self.minor { 'm' } else { 'd' })
    }

    /// The key on the wheel the settings pick: Camelot, or Open Key.
    pub fn wheel(self) -> String {
        if notation() == Notation::OpenKey {
            self.open_key()
        } else {
            self.camelot()
        }
    }

    /// The key as shown: on the wheel ([`Key::wheel`]) or as notes
    /// ([`Key::name`]).
    pub fn label(self, wheel: bool) -> String {
        if wheel {
            self.wheel()
        } else {
            self.name()
        }
    }

    /// Reads a key as tags and DJ software write it: Camelot ("8A"), Open
    /// Key ("1m", "1d") or notes ("Am", "F#", "Eb minor", "C maj").
    pub fn parse(text: &str) -> Option<Key> {
        // No allocation: the engine parses a loaded track's key on the
        // audio thread.
        let t = text.trim();
        let digits = t.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            let n: u8 = t[..digits].parse().ok()?;
            let [letter] = &t.as_bytes()[digits..] else {
                return None;
            };
            if !(1..=12).contains(&n) {
                return None;
            }
            let (camelot, minor) = match letter.to_ascii_lowercase() {
                b'a' => (n, true),
                b'b' => (n, false),
                // Open Key: 1d is C major (Camelot 8B), 1m A minor (8A).
                b'm' => ((n + 6) % 12 + 1, true),
                b'd' => ((n + 6) % 12 + 1, false),
                _ => return None,
            };
            // Inverse of `camelot`: 7 is its own inverse modulo 12.
            let major_tonic = (7 * (camelot as i32 - 8)).rem_euclid(12) as u8;
            let tonic = if minor { (major_tonic + 9) % 12 } else { major_tonic };
            return Some(Key { tonic, minor });
        }
        let mut chars = t.chars();
        let base: i32 = match chars.next()?.to_ascii_uppercase() {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return None,
        };
        let rest = chars.as_str();
        let (shift, rest) = match rest.chars().next() {
            Some(c @ ('#' | '\u{266f}')) => (1, &rest[c.len_utf8()..]),
            Some(c @ ('b' | '\u{266d}')) => (-1, &rest[c.len_utf8()..]),
            _ => (0, rest),
        };
        let is = |w: &str| rest.trim().eq_ignore_ascii_case(w);
        let minor = if ["", "maj", "major"].into_iter().any(is) {
            false
        } else if ["m", "min", "minor"].into_iter().any(is) {
            true
        } else {
            return None;
        };
        Some(Key { tonic: (base + shift).rem_euclid(12) as u8, minor })
    }

    /// The key `semitones` higher (or lower, when negative).
    pub fn transpose(self, semitones: i32) -> Key {
        Key { tonic: (self.tonic as i32 + semitones).rem_euclid(12) as u8, minor: self.minor }
    }

    /// The key as `key` control numbers it (ChromaticKey): 1-12 for
    /// C major to B major, 13-24 for C minor to B minor.
    pub fn control_number(self) -> f64 {
        (self.index() + 1) as f64
    }

    pub fn from_control_number(n: f64) -> Option<Key> {
        (n >= 1.0).then(|| Key::from_index(n as u8 - 1)).flatten()
    }

    /// Whether two keys mix well by the rule picked in the settings (see
    /// [`set_mixing`]).
    pub fn compatible(self, other: Key) -> bool {
        self.mixes(other, mixing())
    }

    /// Whether two keys mix well by the Camelot wheel: the same number, or a
    /// neighbouring number with the same letter (harmonic), or with either
    /// letter (fuzzy).
    pub fn mixes(self, other: Key, mixing: Mixing) -> bool {
        let (a, b) = (self.camelot_number(), other.camelot_number());
        let neighbours = (a + 12 - b) % 12 == 1 || (b + 12 - a) % 12 == 1;
        a == b || (neighbours && (mixing == Mixing::Fuzzy || self.minor == other.minor))
    }

    /// The smallest shift, up to `max` semitones either way, that makes a
    /// deck whose key is `self` mix with `target`: 0 when it already does,
    /// and down rather than up when both are as near.
    pub fn shift_to_mix(self, target: Key, max: i32, mixing: Mixing) -> Option<i32> {
        (0..=max).flat_map(|n| [-n, n]).find(|&n| self.transpose(n).mixes(target, mixing))
    }

    /// The key's number on the Camelot wheel, 1 to 12.
    pub fn camelot_number(self) -> u8 {
        let major_tonic = if self.minor { self.tonic + 3 } else { self.tonic } as usize;
        ((major_tonic * 7 + 7) % 12 + 1) as u8
    }

    /// The key a deck plays in: its file's key shifted by `semitones`, plus
    /// the speed's own pitch change unless key lock holds the key.
    pub fn playing(self, semitones: f64, speed: f64, keylock: bool) -> Key {
        let from_speed = if keylock || speed <= 0.0 { 0.0 } else { 12.0 * speed.log2() };
        self.transpose((semitones + from_speed).round() as i32)
    }

    /// The smallest shift, within ±6 semitones, that makes a deck whose
    /// unshifted key is `self` play in `target`'s key or its relative.
    pub fn shift_to_match(self, target: Key) -> i32 {
        (0..=6)
            .flat_map(|n| [n, -n])
            .find(|&n| self.transpose(n).camelot_number() == target.camelot_number())
            .unwrap_or(0)
    }

    /// 0..24 for storing: majors first.
    pub fn index(self) -> u8 {
        self.tonic + if self.minor { 12 } else { 0 }
    }

    pub fn from_index(i: u8) -> Option<Key> {
        (i < 24).then_some(Key { tonic: i % 12, minor: i >= 12 })
    }
}

/// Which detector finds the keys of newly analysed tracks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Detector {
    /// The chromagram detector below: fast, and needs no model.
    #[default]
    Standard,
    /// The neural network in [`crate::keynet`]: more accurate, slower.
    Neural,
}

impl Detector {
    pub const ALL: [Detector; 2] = [Detector::Standard, Detector::Neural];

    /// Its number in the analysis cache and the settings.
    pub fn id(self) -> u8 {
        match self {
            Detector::Standard => 0,
            Detector::Neural => 1,
        }
    }

    pub fn from_id(id: u8) -> Option<Detector> {
        Detector::ALL.into_iter().find(|d| d.id() == id)
    }
}

/// Which keys count as mixing with each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mixing {
    /// The Camelot wheel's four: the same key, its relative, and the
    /// neighbouring numbers with the same letter (8A: 8A, 8B, 7A, 9A).
    #[default]
    Harmonic,
    /// Fuzzy keymixing: the letter doesn't matter, so the neighbouring
    /// numbers' other keys mix too (8A: also 7B and 9B).
    Fuzzy,
}

impl Mixing {
    pub fn id(self) -> u8 {
        match self {
            Mixing::Harmonic => 0,
            Mixing::Fuzzy => 1,
        }
    }

    pub fn from_id(id: u8) -> Mixing {
        if id == 1 {
            Mixing::Fuzzy
        } else {
            Mixing::Harmonic
        }
    }
}

static MIXING: AtomicU8 = AtomicU8::new(0);

/// The rule [`Key::compatible`] follows, unless changed with [`set_mixing`].
pub fn mixing() -> Mixing {
    Mixing::from_id(MIXING.load(Ordering::Relaxed))
}

pub fn set_mixing(m: Mixing) {
    MIXING.store(m.id(), Ordering::Relaxed);
}

/// How keys are written, as picked in the settings. Clicking a deck's key
/// flips between a wheel and notes: Open Key flips to sharps, flats to
/// Camelot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Notation {
    #[default]
    Camelot,
    OpenKey,
    Sharps,
    Flats,
}

impl Notation {
    pub const ALL: [Notation; 4] = [Notation::Camelot, Notation::OpenKey, Notation::Sharps, Notation::Flats];

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn from_id(id: u8) -> Notation {
        Notation::ALL.get(id as usize).copied().unwrap_or_default()
    }

    /// Whether it writes keys on a wheel (Camelot, Open Key) or as notes.
    pub fn wheel(self) -> bool {
        matches!(self, Notation::Camelot | Notation::OpenKey)
    }
}

static NOTATION: AtomicU8 = AtomicU8::new(0);

/// The notation [`Key::wheel`] and [`Key::name`] follow, unless changed with
/// [`set_notation`].
pub fn notation() -> Notation {
    Notation::from_id(NOTATION.load(Ordering::Relaxed))
}

pub fn set_notation(n: Notation) {
    NOTATION.store(n.id(), Ordering::Relaxed);
}

static DETECTOR: AtomicU8 = AtomicU8::new(0);

/// The detector tracks are analysed with, unless changed with [`set_detector`].
pub fn detector() -> Detector {
    Detector::from_id(DETECTOR.load(Ordering::Relaxed)).unwrap_or_default()
}

/// Sets the detector for tracks analysed from now on. Tracks whose key an
/// earlier analysis found get it again from this one when next loaded.
pub fn set_detector(d: Detector) {
    DETECTOR.store(d.id(), Ordering::Relaxed);
}

pub struct KeyDetector {
    decimation: usize,
    /// Running sum of the current decimation group.
    acc: f32,
    acc_n: usize,
    buf: Vec<f32>,
    window: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
    /// For each spectrum bin in range: the chroma bins it feeds and how much.
    bin_map: Vec<(usize, [(usize, f32); 2])>,
    chroma: [f64; BINS_PER_OCTAVE],
    frames: usize,
    /// Each spectrum's 12 pitch classes, for [`KeyDetector::timeline`].
    steps: Vec<[u8; 12]>,
    /// Spectra per second.
    rate: f64,
}

impl KeyDetector {
    pub fn new(sample_rate: f64) -> Self {
        let decimation = ((sample_rate / 11_025.0).round() as usize).max(1);
        let rate = sample_rate as f32 / decimation as f32;
        let window = (0..FRAME).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FRAME as f32).cos()).collect();
        let mut bin_map = Vec::new();
        for k in 1..FRAME / 2 {
            let f = k as f32 * rate / FRAME as f32;
            let note = 69.0 + 12.0 * (f / 440.0).log2();
            if !(LOWEST_NOTE - 0.5..HIGHEST_NOTE - 0.5).contains(&note) {
                continue;
            }
            // Position in thirds of a semitone, with C's centre on bin 1;
            // split between the two nearest bins.
            let pos = (note - LOWEST_NOTE) * 3.0 + 1.0;
            let lo = pos.floor();
            let t = pos - lo;
            let b = |x: f32| (x as i64).rem_euclid(BINS_PER_OCTAVE as i64) as usize;
            bin_map.push((k, [(b(lo), 1.0 - t), (b(lo + 1.0), t)]));
        }
        KeyDetector {
            decimation,
            acc: 0.0,
            acc_n: 0,
            buf: Vec::with_capacity(FRAME),
            window,
            fft: FftPlanner::new().plan_fft_forward(FRAME),
            bin_map,
            chroma: [0.0; BINS_PER_OCTAVE],
            frames: 0,
            steps: Vec::new(),
            rate: rate as f64 / FRAME as f64,
        }
    }

    /// Takes one mono sample, already low-passed below about 2.5 kHz.
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.acc += x;
        self.acc_n += 1;
        if self.acc_n < self.decimation {
            return;
        }
        self.buf.push(self.acc / self.decimation as f32);
        self.acc = 0.0;
        self.acc_n = 0;
        if self.buf.len() == FRAME {
            self.frame();
            self.buf.clear();
        }
    }

    fn frame(&mut self) {
        let mut spectrum: Vec<Complex32> =
            self.buf.iter().zip(&self.window).map(|(x, w)| Complex32::new(x * w, 0.0)).collect();
        self.fft.process(&mut spectrum);
        let mut chroma = [0.0f64; BINS_PER_OCTAVE];
        for (k, targets) in &self.bin_map {
            let m = spectrum[*k].norm() as f64;
            for (b, w) in targets {
                chroma[*b] += m * *w as f64;
            }
        }
        // Each frame counts the same however loud it is, as in qm-dsp.
        let max = chroma.iter().copied().fold(0.0, f64::max);
        // The three bins of each semitone together (C's are 0..3).
        let mut classes = [0.0f64; 12];
        for (b, c) in chroma.iter().enumerate() {
            classes[b / 3] += c;
        }
        let top = classes.iter().copied().fold(0.0, f64::max);
        self.steps.push(if top > 1e-9 { classes.map(|c| (c / top * 255.0).round() as u8) } else { [0; 12] });
        if max > 1e-9 {
            for (sum, c) in self.chroma.iter_mut().zip(chroma) {
                *sum += c / max;
            }
            self.frames += 1;
        }
    }

    /// The pitch classes of everything pushed, one step per spectrum
    /// (about 0.74 s).
    pub fn timeline(&self) -> crate::track::Chroma {
        crate::track::Chroma { per_second: self.rate, steps: self.steps.clone() }
    }

    /// The key of everything pushed, or None for silence or very short audio.
    pub fn key(&self) -> Option<Key> {
        if self.frames == 0 {
            return None;
        }
        let centre = |v: &[f64; BINS_PER_OCTAVE]| {
            let mean = v.iter().sum::<f64>() / BINS_PER_OCTAVE as f64;
            v.map(|x| x - mean)
        };
        let data = centre(&self.chroma);
        let (major, minor) = (centre(&MAJOR), centre(&MINOR));
        let mut best = (f64::MIN, Key { tonic: 0, minor: false });
        // Profiles shift a whole semitone (three bins) at a time.
        for tonic in 0..12u8 {
            for (profile, minor) in [(&major, false), (&minor, true)] {
                let c = correlate(&data, profile, tonic as usize * 3);
                if c > best.0 {
                    best = (c, Key { tonic, minor });
                }
            }
        }
        Some(best.1)
    }
}

/// Pearson correlation of `data` with `profile` moved up by `shift` bins.
fn correlate(data: &[f64; BINS_PER_OCTAVE], profile: &[f64; BINS_PER_OCTAVE], shift: usize) -> f64 {
    let (mut num, mut a, mut b) = (0.0, 0.0, 0.0);
    for (i, d) in data.iter().enumerate() {
        let p = profile[(i + BINS_PER_OCTAVE - shift) % BINS_PER_OCTAVE];
        num += d * p;
        a += d * d;
        b += p * p;
    }
    let den = (a * b).sqrt();
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chords as organ-like tones (a few harmonics), two seconds each.
    fn chords(rate: f64, progression: &[[f32; 3]]) -> Vec<f32> {
        let mut out = Vec::new();
        for chord in progression.iter().cycle().take(progression.len() * 4) {
            for i in 0..(2.0 * rate) as usize {
                let t = i as f32 / rate as f32;
                let mut v = 0.0;
                for &note in chord {
                    // The root an octave lower too, like a bass line.
                    for (n, gain) in [(note, 1.0), (note - 12.0, 0.6 * (note == chord[0]) as u8 as f32)] {
                        let f = 440.0 * 2f32.powf((n - 69.0) / 12.0);
                        for h in 1..4 {
                            v += gain / h as f32 * (std::f32::consts::TAU * f * h as f32 * t).sin();
                        }
                    }
                }
                out.push(v * 0.05);
            }
        }
        out
    }

    fn detect(rate: f64, progression: &[[f32; 3]]) -> String {
        let mut k = KeyDetector::new(rate);
        for x in chords(rate, progression) {
            k.push(x);
        }
        k.key().unwrap().camelot()
    }

    #[test]
    fn finds_major_and_minor_keys() {
        // I IV V I in C major; i iv V in A minor (with its G sharp); I IV V in D.
        let c_major = [[60.0, 64.0, 67.0], [65.0, 69.0, 72.0], [67.0, 71.0, 74.0], [60.0, 64.0, 67.0]];
        let a_minor = [[57.0, 60.0, 64.0], [62.0, 65.0, 69.0], [64.0, 68.0, 71.0], [57.0, 60.0, 64.0]];
        let d_major = [[62.0, 66.0, 69.0], [67.0, 71.0, 74.0], [69.0, 73.0, 76.0], [62.0, 66.0, 69.0]];
        assert_eq!(detect(44_100.0, &c_major), "8B");
        assert_eq!(detect(48_000.0, &a_minor), "8A");
        assert_eq!(detect(44_100.0, &d_major), "10B");
    }

    #[test]
    fn reads_and_names_keys() {
        let p = |s: &str| Key::parse(s).map(|k| (k.name(), k.camelot()));
        let k = |n: &str, c: &str| Some((n.to_string(), c.to_string()));
        assert_eq!(p("8A"), k("Am", "8A"));
        assert_eq!(p("08b"), k("C", "8B"));
        assert_eq!(p("12B"), k("E", "12B"));
        assert_eq!(p("1m"), k("Am", "8A"));
        assert_eq!(p("2d"), k("G", "9B"));
        assert_eq!(p("F#m"), k("F#m", "11A"));
        assert_eq!(p("Gbm"), k("F#m", "11A"));
        assert_eq!(p("Eb minor"), k("D#m", "2A"));
        assert_eq!(p("B\u{266d}"), k("A#", "6B"));
        assert_eq!(p("Db"), k("C#", "3B"));
        assert_eq!(p("c maj"), k("C", "8B"));
        assert_eq!(p("Cb"), k("B", "1B"));
        for i in 0..24 {
            let key = Key::from_index(i).unwrap();
            assert_eq!(Key::parse(&key.camelot()), Some(key));
            assert_eq!(Key::parse(&key.name()), Some(key));
            assert_eq!(Key::parse(&key.spelt(true)), Some(key));
            assert_eq!(Key::parse(&key.open_key()), Some(key));
            assert!(!key.name().contains('b'));
        }
        let k = |s: &str| Key::parse(s).unwrap();
        assert_eq!(k("Bbm").spelt(true), "Bbm");
        assert_eq!(k("C#").spelt(true), "Db");
        assert_eq!(k("C").open_key(), "1d");
        assert_eq!(k("Am").open_key(), "1m");
        assert_eq!(k("F#m").open_key(), "4m");
        assert_eq!(Key::parse(""), None);
        assert_eq!(Key::parse("13A"), None);
        assert_eq!(Key::parse("H"), None);
        assert_eq!(Key::parse("Am7"), None);
    }

    #[test]
    fn shifts_and_matches_keys() {
        let k = |s: &str| Key::parse(s).unwrap();
        assert_eq!(k("Am").transpose(-1), k("G#m"));
        assert_eq!(k("C").playing(0.0, 2f64.powf(1.0 / 12.0), false), k("Db"));
        assert_eq!(k("C").playing(0.0, 1.08, true), k("C"));
        assert_eq!(k("C").playing(-2.0, 1.0, true), k("Bb"));
        // Fm to play with Em: one down (or Gm, its relative, two up).
        assert_eq!(k("Fm").shift_to_match(k("Em")), -1);
        assert_eq!(k("Am").shift_to_match(k("C")), 0);
        assert!(k("Am").compatible(k("Em")) && k("Am").compatible(k("C")));
        assert!(!k("Am").compatible(k("F#m")));
        // 8A with 7B (F) and 9B (G): only when fuzzy.
        let (h, f) = (Mixing::Harmonic, Mixing::Fuzzy);
        let mixes = |a: &str, mixing| (0..24).filter_map(Key::from_index).filter(|&b| k(a).mixes(b, mixing)).count();
        assert_eq!((mixes("8A", h), mixes("8A", f)), (4, 6));
        assert!(!k("Am").mixes(k("F"), h) && k("Am").mixes(k("F"), f));
        assert!(k("Am").mixes(k("G"), f) && !k("Am").mixes(k("D"), f));
        // Fuzzy reaches every key within two semitones, as the article says.
        for i in 0..24 {
            let a = Key::from_index(i).unwrap();
            for j in 0..24 {
                let b = Key::from_index(j).unwrap();
                let fuzzy = a.shift_to_mix(b, 2, f).expect("fuzzy reaches every key in ±2");
                assert!(a.transpose(fuzzy).mixes(b, f));
                if let Some(n) = a.shift_to_mix(b, 2, h) {
                    assert!(a.transpose(n).mixes(b, h) && n.abs() <= 2);
                }
            }
        }
        assert_eq!(k("Am").shift_to_mix(k("C"), 2, h), Some(0));
        // Fm (4A) with Em (9A): one down.
        assert_eq!(k("Fm").shift_to_mix(k("Em"), 2, h), Some(-1));
        // E (12B) with C (8B): one up is F (7B).
        assert_eq!(k("E").shift_to_mix(k("C"), 2, h), Some(1));
        // Am (8A) with Eb (5B): harmonic needs Cm (5A), three up.
        assert_eq!(k("Am").shift_to_mix(k("Eb"), 2, h), None);
        assert_eq!(Key::from_control_number(k("Am").control_number()), Some(k("Am")));
        assert_eq!(k("C").control_number(), 1.0);
    }

    #[test]
    fn camelot_wheel() {
        let k = |tonic, minor| Key { tonic, minor }.camelot();
        assert_eq!(k(0, false), "8B");
        assert_eq!(k(7, false), "9B");
        assert_eq!(k(1, false), "3B");
        assert_eq!(k(9, true), "8A");
        assert_eq!(k(0, true), "5A");
        assert_eq!(k(4, true), "9A");
        for i in 0..24 {
            assert_eq!(Key::from_index(i).unwrap().index(), i);
        }
    }
}
