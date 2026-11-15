//! Scores DJ Sugar's phrase detection against the phrases another app found
//! for the same tracks (rekordbox's, brought in with its library import).
//!
//!     cargo run --release -p phrase-bench -- LIBRARY [--limit N] [--csv]
//!
//! LIBRARY is DJ Sugar's `library.sqlite` (on macOS in
//! `~/Library/Application Support/DJSugar`). Every track there with imported
//! phrases whose file can be read is decoded and analysed afresh (nothing
//! is written to the library or the cache), on the library's own beat grid,
//! and its detected phrases are compared with the imported ones:
//!
//! - boundaries: precision, recall and F1 with a cut counting as found
//!   within 1 bar, and within 4 bars;
//! - intro end and outro start, what Automix mixes on: the error in bars;
//! - labels: the share of bars whose kind agrees (Drop and Chorus count as
//!   rekordbox's Chorus, Build as Up, Breakdown as Down or Bridge);
//! - the same boundaries without the harmony (chroma), to see what it adds.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::phrases::{self, Phrase, PhraseKind};
use djsugar_engine::track::{Chroma, MarkerKind, Source, Track};
use djsugar_engine::{analysis, decode};

struct Options {
    library: PathBuf,
    limit: usize,
    csv: bool,
}

fn options() -> Result<Options, String> {
    let mut o = Options {
        library: PathBuf::new(),
        limit: usize::MAX,
        csv: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--limit" => {
                o.limit = args
                    .next()
                    .and_then(|n| n.parse().ok())
                    .ok_or("--limit needs a number")?
            }
            "--csv" => o.csv = true,
            "-h" | "--help" => return Err(String::new()),
            _ if o.library.as_os_str().is_empty() => o.library = PathBuf::from(a),
            _ => return Err(format!("unexpected argument {a}")),
        }
    }
    if o.library.as_os_str().is_empty() {
        return Err("give the path of DJ Sugar's library.sqlite".into());
    }
    Ok(o)
}

/// Phrase families both apps share.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Family {
    Intro,
    Up,
    Chorus,
    Down,
    Verse,
    Outro,
    Other,
}

fn theirs_family(name: &str) -> Family {
    match name {
        "Intro" => Family::Intro,
        "Up" => Family::Up,
        "Chorus" => Family::Chorus,
        "Down" | "Bridge" => Family::Down,
        "Outro" => Family::Outro,
        n if n.starts_with("Verse") => Family::Verse,
        _ => Family::Other,
    }
}

fn ours_family(kind: PhraseKind) -> Family {
    match kind {
        PhraseKind::Intro => Family::Intro,
        PhraseKind::Build => Family::Up,
        PhraseKind::Drop | PhraseKind::Chorus => Family::Chorus,
        PhraseKind::Breakdown => Family::Down,
        PhraseKind::Verse => Family::Verse,
        PhraseKind::Outro => Family::Outro,
    }
}

/// Phrase starts in bars and families, earliest first.
type Bars = Vec<(f64, Family)>;

/// Matches `found` cuts to `truth` cuts one to one within `tolerance`
/// bars: how many matched.
fn hits(truth: &[f64], found: &[f64], tolerance: f64) -> usize {
    let mut used = vec![false; found.len()];
    let mut n = 0;
    for t in truth {
        let best = found
            .iter()
            .enumerate()
            .filter(|(i, f)| !used[*i] && (*f - t).abs() <= tolerance)
            .min_by(|a, b| (a.1 - t).abs().total_cmp(&(b.1 - t).abs()));
        if let Some((i, _)) = best {
            used[i] = true;
            n += 1;
        }
    }
    n
}

#[derive(Default)]
struct Score {
    truth: usize,
    found: usize,
    hit1: usize,
    hit4: usize,
    plain_found: usize,
    plain_hit1: usize,
    agree_bars: f64,
    bars: f64,
    intro_err: Vec<f64>,
    outro_err: Vec<f64>,
}

fn f1(hit: usize, truth: usize, found: usize) -> (f64, f64, f64) {
    let p = hit as f64 / found.max(1) as f64;
    let r = hit as f64 / truth.max(1) as f64;
    let f = if p + r > 0.0 {
        2.0 * p * r / (p + r)
    } else {
        0.0
    };
    (p, r, f)
}

fn cuts(b: &Bars) -> Vec<f64> {
    b.iter().skip(1).map(|p| p.0).collect()
}

/// Where the intro ends and the outro starts, in bars, if there are both.
fn edges(b: &Bars) -> (Option<f64>, Option<f64>) {
    let intro_end = (b.first().map(|p| p.1) == Some(Family::Intro))
        .then(|| b.get(1).map(|p| p.0))
        .flatten();
    let outro = b.iter().rev().find(|p| p.1 == Family::Outro).map(|p| p.0);
    (intro_end, outro)
}

/// The share of bars `0..end` whose families agree.
fn agreement(a: &Bars, b: &Bars, end: f64) -> f64 {
    let at = |v: &Bars, bar: f64| v.iter().rev().find(|p| p.0 <= bar).map(|p| p.1);
    let n = end.floor() as usize;
    (0..n)
        .filter(|&i| {
            let x = i as f64 + 0.5;
            at(a, x).is_some() && at(a, x) == at(b, x)
        })
        .count() as f64
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn analysed(track: &Track, path: &Path) -> Result<Track, String> {
    let audio = decode::decode(path)?;
    let hint = (track.bpm > 0.0).then_some(track.bpm);
    let a = analysis::analyse(&audio, hint);
    let mut t = track.clone();
    // The library's grid (rekordbox's, from the import) when it has one,
    // so both apps count the same bars.
    if t.grid.is_empty() || t.bpm <= 0.0 {
        t.bpm = a.bpm;
        t.first_beat = a.first_beat;
        t.set_grid(a.grid.clone());
    }
    let finest = a.waveform.levels.first().map_or(0, Vec::len);
    if t.duration <= 0.0 && a.waveform.bins_per_second > 0.0 {
        t.duration = finest as f64 / a.waveform.bins_per_second;
    }
    t.waveform = Arc::new(a.waveform);
    Ok(t)
}

fn main() {
    let o = match options() {
        Ok(o) => o,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            eprintln!("usage: phrase-bench LIBRARY.sqlite [--limit N] [--csv]");
            std::process::exit(2);
        }
    };
    let library = match djsugar_library::Library::open(&o.library) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("can't open {}: {e}", o.library.display());
            std::process::exit(1);
        }
    };
    let tracks = library.tracks(|p| p.exists()).unwrap_or_default();
    let with_phrases: Vec<&Track> = tracks
        .iter()
        .filter(|t| phrases::imported(t) && matches!(t.source, Source::File(_)))
        .take(o.limit)
        .collect();
    eprintln!("{} tracks with imported phrases", with_phrases.len());
    if o.csv {
        println!("artist,title,bpm,truth_cuts,found_cuts,hit_1bar,hit_4bar,plain_hit_1bar,label_agree,intro_err,outro_err");
    }
    let mut total = Score::default();
    for track in with_phrases {
        let Source::File(path) = &track.source else {
            continue;
        };
        let t = match analysed(track, path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipped {}: {e}", path.display());
                continue;
            }
        };
        let bar = |s: f64| t.beat_at(s) / 4.0;
        let mut truth: Bars = t
            .info
            .markers
            .iter()
            .filter(|m| m.kind == MarkerKind::Phrase && !m.detected)
            .map(|m| (bar(m.seconds), theirs_family(&m.name)))
            .collect();
        truth.sort_by(|a, b| a.0.total_cmp(&b.0));
        let to_bars = |p: &[Phrase]| -> Bars {
            p.iter()
                .map(|p| (bar(p.start), ours_family(p.kind)))
                .collect()
        };
        let ours = to_bars(&phrases::detect(&t));
        let mut plain_t = t.clone();
        let mut w = (*plain_t.waveform).clone();
        w.chroma = Chroma::default();
        plain_t.waveform = Arc::new(w);
        let plain = to_bars(&phrases::detect(&plain_t));

        let (tc, oc, pc) = (cuts(&truth), cuts(&ours), cuts(&plain));
        let (h1, h4, ph1) = (
            hits(&tc, &oc, 1.0),
            hits(&tc, &oc, 4.0),
            hits(&tc, &pc, 1.0),
        );
        let end = bar(t.duration);
        let agree = agreement(&truth, &ours, end);
        let (ti, to) = edges(&truth);
        let (oi, oo) = edges(&ours);
        let intro_err = ti.zip(oi).map(|(a, b)| (a - b).abs());
        let outro_err = to.zip(oo).map(|(a, b)| (a - b).abs());
        total.truth += tc.len();
        total.found += oc.len();
        total.hit1 += h1;
        total.hit4 += h4;
        total.plain_found += pc.len();
        total.plain_hit1 += ph1;
        total.agree_bars += agree;
        total.bars += end.floor();
        total.intro_err.extend(intro_err);
        total.outro_err.extend(outro_err);
        let fmt = |v: Option<f64>| v.map_or(String::new(), |v| format!("{v:.1}"));
        if o.csv {
            println!(
                "\"{}\",\"{}\",{:.1},{},{},{},{},{},{:.2},{},{}",
                t.artist.replace('"', "'"),
                t.title.replace('"', "'"),
                t.bpm,
                tc.len(),
                oc.len(),
                h1,
                h4,
                ph1,
                agree / end.floor().max(1.0),
                fmt(intro_err),
                fmt(outro_err)
            );
        } else {
            let (_, _, f) = f1(h1, tc.len(), oc.len());
            println!(
                "{:>4.0}% cuts  {:>3.0}% labels  intro ±{:<5} outro ±{:<5} {} – {}",
                100.0 * f,
                100.0 * agree / end.floor().max(1.0),
                fmt(intro_err),
                fmt(outro_err),
                t.artist,
                t.title
            );
        }
    }
    let (p1, r1, f1_1) = f1(total.hit1, total.truth, total.found);
    let (_, _, f1_4) = f1(total.hit4, total.truth, total.found);
    let (_, _, plain) = f1(total.plain_hit1, total.truth, total.plain_found);
    eprintln!();
    eprintln!(
        "cuts within 1 bar: precision {:.0}%, recall {:.0}%, F1 {:.0}%",
        100.0 * p1,
        100.0 * r1,
        100.0 * f1_1
    );
    eprintln!("cuts within 4 bars: F1 {:.0}%", 100.0 * f1_4);
    eprintln!(
        "without harmony (chroma): F1 {:.0}% within 1 bar",
        100.0 * plain
    );
    eprintln!(
        "labels agree on {:.0}% of bars",
        100.0 * total.agree_bars / total.bars.max(1.0)
    );
    eprintln!(
        "median error: intro end {:.1} bars ({} tracks), outro start {:.1} bars ({} tracks)",
        median(&mut total.intro_err),
        total.intro_err.len(),
        median(&mut total.outro_err),
        total.outro_err.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_match_one_to_one_within_the_tolerance() {
        let truth = [16.0, 32.0, 64.0];
        assert_eq!(hits(&truth, &[16.5, 31.0, 40.0], 1.0), 2);
        // One found cut can't answer two.
        assert_eq!(hits(&[16.0, 17.0], &[16.5], 1.0), 1);
        assert_eq!(hits(&truth, &[20.0], 4.0), 1);
        let (p, r, f) = f1(2, 3, 4);
        assert!((p - 0.5).abs() < 1e-9 && (r - 2.0 / 3.0).abs() < 1e-9 && f > 0.5);
    }

    #[test]
    fn edges_and_labels() {
        let truth: Bars = vec![
            (0.0, Family::Intro),
            (16.0, Family::Chorus),
            (48.0, Family::Outro),
        ];
        let ours: Bars = vec![
            (0.0, Family::Intro),
            (12.0, Family::Chorus),
            (48.0, Family::Outro),
        ];
        assert_eq!(edges(&truth), (Some(16.0), Some(48.0)));
        assert_eq!(agreement(&truth, &ours, 64.0), 60.0);
        assert_eq!(ours_family(PhraseKind::Drop), theirs_family("Chorus"));
        assert_eq!(theirs_family("Verse 2"), Family::Verse);
    }
}
