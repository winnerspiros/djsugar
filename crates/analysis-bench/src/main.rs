//! Scores DJ Sugar's tempo, beat grid and key analysis against tracks whose
//! answers are known, and against what Mixxx's analyzers find for the same
//! tracks.
//!
//!     cargo run --release -p analysis-bench -- corpus DIR
//!     cargo run --release -p analysis-bench -- score DIR [--mixxx FILE] [--check]
//!
//! `corpus` writes the test tracks: synthesised from a fixed seed (drums,
//! bass and chords in a style, tempo and key), so the same files come out
//! everywhere and none have to be stored. `truth.json` lists the answers.
//!
//! `score` analyses each track as the app does and scores:
//! - tempo: right, an octave or a third off (half, double, 2/3, 3/2), wrong;
//! - phase: how far the grid's beats are from the real ones, in ms;
//! - key: the MIREX weighting (1 right, 0.5 a fifth off, 0.3 the relative,
//!   0.2 the parallel key).
//!
//! `--mixxx FILE` adds Mixxx's results (one JSON object per line, as the
//! reference analyzer prints them: Queen Mary beats through Mixxx's
//! constant-grid step, Queen Mary key held longest) scored the same way.
//! `--check` fails when DJ Sugar scores below Mixxx on any of the three.

mod corpus;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use djsugar_engine::analysis::{self, TempoHint};
use djsugar_engine::decode;
use djsugar_engine::key::Key;
use serde_json::Value;

/// A beat closer than this to the real one counts as on it.
const PHASE_OK_MS: f64 = 20.0;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("corpus") if args.len() == 2 => corpus::write(Path::new(&args[1])),
        Some("score") if args.len() >= 2 => score(&args[1..]),
        _ => Err("usage: analysis-bench corpus DIR | score DIR [--mixxx FILE] [--check]".into()),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// What an analyzer found for one track.
#[derive(Clone, Copy, Debug)]
struct Found {
    bpm: f64,
    first_beat: f64,
    key: Option<Key>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tempo {
    Right,
    /// Half, double, two thirds or three halves of the real tempo.
    Octave,
    Wrong,
}

fn tempo(found: f64, truth: f64) -> Tempo {
    if (found - truth).abs() < 0.05 {
        return Tempo::Right;
    }
    let near = |r: f64| (found / truth - r).abs() < 0.002;
    if [0.5, 2.0, 2.0 / 3.0, 1.5].into_iter().any(near) {
        Tempo::Octave
    } else {
        Tempo::Wrong
    }
}

/// How far the found grid's beat is from the nearest real beat, in ms.
fn phase_ms(found: &Found, truth_bpm: f64, truth_first: f64) -> f64 {
    let period = 60.0 / truth_bpm;
    let d = (found.first_beat - truth_first).rem_euclid(period);
    d.min(period - d) * 1000.0
}

/// The MIREX key score.
fn key_score(found: Option<Key>, truth: Key) -> f64 {
    let Some(k) = found else { return 0.0 };
    if k == truth {
        return 1.0;
    }
    let fifth = k.minor == truth.minor
        && ((k.tonic + 7) % 12 == truth.tonic || (truth.tonic + 7) % 12 == k.tonic);
    let relative = k.minor != truth.minor && k.camelot_number() == truth.camelot_number();
    let parallel = k.minor != truth.minor && k.tonic == truth.tonic;
    if fifth {
        0.5
    } else if relative {
        0.3
    } else if parallel {
        0.2
    } else {
        0.0
    }
}

/// Mixxx's ChromaticKey: 1-12 major from C, 13-24 minor from C.
fn mixxx_key(n: i64) -> Option<Key> {
    (1..=24)
        .contains(&n)
        .then(|| Key::from_index((n - 1) as u8))
        .flatten()
}

#[derive(Default)]
struct Totals {
    tracks: usize,
    tempo_right: usize,
    tempo_octave: usize,
    phase_ok: usize,
    key: f64,
}

impl Totals {
    fn add(&mut self, f: &Found, t: &corpus::Truth) -> (Tempo, f64, f64) {
        let tp = tempo(f.bpm, t.bpm);
        let ph = phase_ms(f, t.bpm, t.first_beat);
        let k = key_score(f.key, t.key);
        self.tracks += 1;
        self.tempo_right += usize::from(tp == Tempo::Right);
        self.tempo_octave += usize::from(tp == Tempo::Octave);
        self.phase_ok += usize::from(tp == Tempo::Right && ph < PHASE_OK_MS);
        self.key += k;
        (tp, ph, k)
    }

    fn line(&self, who: &str) -> String {
        let n = self.tracks.max(1) as f64;
        format!(
            "{who:<9} tempo {:>3}/{} right ({} octave off)  grid on the beat {:>3}/{}  key {:.1}%",
            self.tempo_right,
            self.tracks,
            self.tempo_octave,
            self.phase_ok,
            self.tracks,
            100.0 * self.key / n
        )
    }
}

fn score(args: &[String]) -> Result<bool, String> {
    let dir = PathBuf::from(&args[0]);
    let mut mixxx_file = None;
    let mut check = false;
    let mut rest = args[1..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--mixxx" => mixxx_file = Some(rest.next().ok_or("--mixxx needs a file")?),
            "--check" => check = true,
            _ => return Err(format!("unexpected argument {a}")),
        }
    }
    let truth = corpus::read_truth(&dir)?;
    let mixxx: HashMap<String, Found> = match mixxx_file {
        Some(f) => read_mixxx(Path::new(f))?,
        None => HashMap::new(),
    };

    let (mut ours, mut theirs) = (Totals::default(), Totals::default());
    println!(
        "{:<28} {:>7} | {:>8} {:>6} {:>5} {:>4} | {:>8} {:>6} {:>5} {:>4}",
        "track", "tempo", "ours", "ms", "key", "", "mixxx", "ms", "key", ""
    );
    for t in &truth {
        let audio = decode::decode(&dir.join(&t.file))?;
        let a = analysis::analyse(&audio, TempoHint::default());
        let f = Found {
            bpm: a.bpm,
            first_beat: a.first_beat,
            key: a.key,
        };
        let (tp, ph, k) = ours.add(&f, t);
        let mut row = format!(
            "{:<28} {:>7.2} | {:>8.3} {:>6.1} {:>5} {:>4.1}",
            t.file,
            t.bpm,
            f.bpm,
            ph,
            f.key.map(Key::camelot).unwrap_or_default(),
            k
        );
        if let Some(m) = mixxx.get(&t.file) {
            let (_, mph, mk) = theirs.add(m, t);
            row += &format!(
                " | {:>8.3} {:>6.1} {:>5} {:>4.1}",
                m.bpm,
                mph,
                m.key.map(Key::camelot).unwrap_or_default(),
                mk
            );
        }
        if tp != Tempo::Right {
            row += "  <- tempo";
        }
        println!("{row}");
    }
    println!();
    println!("{}", ours.line("DJ Sugar"));
    if theirs.tracks > 0 {
        println!("{}", theirs.line("Mixxx"));
    }
    if !check || theirs.tracks == 0 {
        return Ok(true);
    }
    let behind = ours.tempo_right < theirs.tempo_right
        || ours.phase_ok < theirs.phase_ok
        || ours.key + 1e-9 < theirs.key;
    if behind {
        eprintln!("DJ Sugar scores below Mixxx");
    }
    Ok(!behind)
}

fn read_mixxx(path: &Path) -> Result<HashMap<String, Found>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = HashMap::new();
    for line in text.lines().filter(|l| l.starts_with('{')) {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let file = v["file"].as_str().unwrap_or_default().to_string();
        out.insert(
            file,
            Found {
                bpm: v["bpm"].as_f64().unwrap_or(0.0),
                first_beat: v["first_beat"].as_f64().unwrap_or(0.0),
                key: mixxx_key(v["key"].as_i64().unwrap_or(0)),
            },
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tonic: u8, minor: bool) -> Key {
        Key { tonic, minor }
    }

    #[test]
    fn scores_keys_the_mirex_way() {
        let a_minor = key(9, true);
        assert_eq!(key_score(Some(a_minor), a_minor), 1.0);
        assert_eq!(key_score(Some(key(4, true)), a_minor), 0.5);
        assert_eq!(key_score(Some(key(2, true)), a_minor), 0.5);
        assert_eq!(key_score(Some(key(0, false)), a_minor), 0.3);
        assert_eq!(key_score(Some(key(9, false)), a_minor), 0.2);
        assert_eq!(key_score(Some(key(1, true)), a_minor), 0.0);
        assert_eq!(key_score(None, a_minor), 0.0);
    }

    #[test]
    fn reads_mixxx_keys() {
        assert_eq!(mixxx_key(1), Some(key(0, false)));
        assert_eq!(mixxx_key(22), Some(key(9, true)));
        assert_eq!(mixxx_key(0), None);
    }

    #[test]
    fn tells_octave_errors_from_wrong_tempos() {
        assert_eq!(tempo(128.0, 128.0), Tempo::Right);
        assert_eq!(tempo(64.0, 128.0), Tempo::Octave);
        assert_eq!(tempo(116.0, 174.0), Tempo::Octave);
        assert_eq!(tempo(125.0, 128.0), Tempo::Wrong);
    }

    #[test]
    fn phase_wraps_to_the_nearest_beat() {
        let f = |first_beat| Found {
            bpm: 120.0,
            first_beat,
            key: None,
        };
        assert!((phase_ms(&f(0.51), 120.0, 0.0) - 10.0).abs() < 1e-6);
        assert!((phase_ms(&f(0.49), 120.0, 0.0) - 10.0).abs() < 1e-6);
    }
}
