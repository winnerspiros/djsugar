//! Times each word of a track's lyrics by listening to it: Whisper
//! (whisper.cpp, through whisper-rs) hears the vocals, word by word with
//! times, and the lyrics the DJ has are lined up with what it heard. Words
//! it heard well take its times; the words between are spread over the gap
//! by their length. A track with no lyrics can be transcribed instead:
//! the words heard, grouped into lines.
//!
//! The model (OpenAI Whisper large-v3-turbo, quantized, MIT) is downloaded
//! the first time it is needed, like the stem separation models.

use std::path::Path;

use crate::lyrics::{Line, Word};

/// The Whisper weights: large-v3-turbo, 5-bit, about 550 MB.
pub const MODEL: &str = "ggml-large-v3-turbo-q5_0.bin";

/// The model file and its SHA-1, as whisper.cpp publishes it (the
/// `stem-models` release of DJ Sugar carries the same bytes).
pub const MODELS: [(&str, &str); 1] = [(MODEL, "e050f7970618a659205450ad97eb95a18d69c9ee")];

/// Whisper listens at 16 kHz, mono.
const RATE: u32 = 16_000;

/// The share of the lyrics' words Whisper must have heard for the times
/// to be trusted.
const ENOUGH: f64 = 0.3;

/// A word Whisper heard, with when.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// What [`time_words`] is doing, for the DJ to see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// Downloading the model, the first time.
    Downloading,
    /// Separating the vocals from the track, when it has no stems yet.
    Separating,
    /// Listening, 0 to 100 percent.
    Listening(i32),
}

/// Where [`time_words`] reports its [`Stage`].
pub type Report = std::sync::Arc<dyn Fn(Stage) + Send + Sync>;

/// Times each word of `lines` by listening to the track at `path`: its
/// vocal stem when it has one (separated with HTDemucs first when it
/// doesn't, if that works), else the whole mix. Lines keep their text;
/// each sung line gets its words' times.
pub fn time_words(path: &Path, lines: &[Line], report: Report) -> Result<Vec<Line>, String> {
    let heard = hear(path, report)?;
    align(lines, &heard)
}

/// The words Whisper hears in the track at `path`.
pub fn hear(path: &Path, report: Report) -> Result<Vec<Heard>, String> {
    if crate::separation::model(MODEL).is_none() {
        report(Stage::Downloading);
    }
    let model = crate::separation::model_or_fetch(MODEL, &MODELS)?;
    let samples = vocals(path, &report)?;
    report(Stage::Listening(0));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        listen(&model, &samples, move |p| report(Stage::Listening(p)))
    }));
    result.unwrap_or_else(|_| Err("Whisper crashed".into()))
}

/// The track's vocals at 16 kHz, mono.
fn vocals(path: &Path, report: &Report) -> Result<Vec<f32>, String> {
    use crate::stems::StemSource;
    let stems = crate::stems::find(path).or_else(|| {
        report(Stage::Separating);
        crate::separation::separate_hq(path).ok()
    });
    let source = match stems {
        Some(StemSource::Files(files)) if files.len() == 4 => files[3].clone(),
        _ => path.to_owned(),
    };
    let audio = crate::decode::decode(&source)?;
    let mono: Vec<f32> = audio
        .decoded_chunks()
        .flatten()
        .map(|f| (f[0] + f[1]) * 0.5)
        .collect();
    djsugar_separation::resample(&mono, audio.sample_rate, RATE)
}

fn listen(
    model: &Path,
    samples: &[f32],
    mut progress: impl FnMut(i32) + 'static,
) -> Result<Vec<Heard>, String> {
    // Speech recognition needs a pure-Rust model runtime; until then nothing is heard.
    let _ = (model, samples, &mut progress);
    Err("word timing is not available yet".into())
}

fn plain(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// How well two plain words match: 2 the same, 1 close (a letter or two
/// off, as Whisper hears "gonna" for "going"), else none.
fn likeness(a: &str, b: &str) -> i32 {
    if a == b {
        return 2;
    }
    let (n, m) = (a.chars().count(), b.chars().count());
    if n < 3 || m < 3 {
        return -1;
    }
    let distance = edit_distance(a, b);
    if distance * 3 <= n.max(m) {
        1
    } else {
        -1
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (diagonal + usize::from(ca != *cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// Which heard word each written word matches, if any: the best global
/// alignment of the two (Needleman-Wunsch), where a skipped word on
/// either side costs 1.
fn matches(written: &[String], heard: &[String]) -> Vec<Option<usize>> {
    let (n, m) = (written.len(), heard.len());
    let width = m + 1;
    let mut score = vec![0i32; (n + 1) * width];
    for i in 0..=n {
        score[i * width] = -(i as i32);
    }
    for (j, s) in score.iter_mut().enumerate().take(width) {
        *s = -(j as i32);
    }
    for i in 1..=n {
        for j in 1..=m {
            let pair = score[(i - 1) * width + j - 1] + likeness(&written[i - 1], &heard[j - 1]);
            let skip = (score[(i - 1) * width + j]).max(score[i * width + j - 1]) - 1;
            score[i * width + j] = pair.max(skip);
        }
    }
    let mut out = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let here = score[i * width + j];
        let like = likeness(&written[i - 1], &heard[j - 1]);
        if like > 0 && here == score[(i - 1) * width + j - 1] + like {
            out[i - 1] = Some(j - 1);
            i -= 1;
            j -= 1;
        } else if here == score[(i - 1) * width + j] - 1 {
            i -= 1;
        } else if here == score[i * width + j - 1] - 1 {
            j -= 1;
        } else {
            i -= 1;
            j -= 1;
        }
    }
    out
}

/// The words of a line as written, each with the spaces after it, so
/// joined they make the line again.
fn words_of(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut spaces = false;
    for c in text.trim().chars() {
        if c.is_whitespace() {
            spaces = true;
        } else if spaces {
            out.push(std::mem::take(&mut word));
            spaces = false;
        }
        word.push(c);
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Lines `lines` up with the words Whisper heard and gives each sung line
/// its words' times. Errors when too few of the words were heard to trust.
pub fn align(lines: &[Line], heard: &[Heard]) -> Result<Vec<Line>, String> {
    // Every written word: (line, text, plain).
    let mut written: Vec<(usize, String, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for w in words_of(&line.text) {
            let p = plain(&w);
            written.push((i, w, p));
        }
    }
    let heard_plain: Vec<String> = heard.iter().map(|h| plain(&h.text)).collect();
    let keys: Vec<String> = written.iter().map(|w| w.2.clone()).collect();
    let found = matches(&keys, &heard_plain);
    let counted = keys.iter().filter(|k| !k.is_empty()).count();
    let hit = found.iter().filter(|f| f.is_some()).count();
    if counted == 0 || (hit as f64) < counted as f64 * ENOUGH {
        return Err("Whisper couldn't make out enough of the words".into());
    }
    // Each written word's (start, end), from its match or spread between
    // the matched words around it by length.
    let mut times: Vec<Option<(f64, f64)>> = found
        .iter()
        .map(|f| f.map(|j| (heard[j].start, heard[j].end.max(heard[j].start))))
        .collect();
    let length = |k: usize| written[k].1.chars().count().max(1) as f64;
    let mut k = 0;
    while k < times.len() {
        if times[k].is_some() {
            k += 1;
            continue;
        }
        let first = k;
        while k < times.len() && times[k].is_none() {
            k += 1;
        }
        // Words first..k have no time: fit them between their neighbors.
        let from = first
            .checked_sub(1)
            .and_then(|p| times[p])
            .map(|t| t.1)
            .or_else(|| lines[written[first].0].start);
        let to = times.get(k).copied().flatten().map(|t| t.0);
        let total: f64 = (first..k).map(length).sum();
        let (from, to) = match (from, to) {
            (Some(a), Some(b)) if b > a => (a, b),
            (Some(a), _) => (a, a + total * 0.08 + 0.35 * (k - first) as f64),
            (None, Some(b)) => ((b - total * 0.08 - 0.35 * (k - first) as f64).max(0.0), b),
            (None, None) => (0.0, 0.0),
        };
        let mut at = from;
        for (w, time) in times.iter_mut().enumerate().take(k).skip(first) {
            let span = (to - from) * length(w) / total;
            *time = Some((at, at + span));
            at += span;
        }
    }
    // Keep the times in order, as the lyrics are sung.
    let mut last = 0.0f64;
    for t in times.iter_mut().flatten() {
        t.0 = t.0.max(last);
        t.1 = t.1.max(t.0);
        last = t.0;
    }
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    let mut w = 0;
    let mut previous_end = 0.0;
    for (i, line) in lines.iter().enumerate() {
        let mut words = Vec::new();
        let mut end = None;
        while w < written.len() && written[w].0 == i {
            let (start, stop) = times[w].unwrap_or((previous_end, previous_end));
            words.push(Word {
                start,
                text: written[w].1.clone(),
            });
            end = Some(stop);
            w += 1;
        }
        let start = match words.first() {
            Some(first) => Some(first.start),
            // A break between verses: from the end of the line before.
            None => Some(line.start.unwrap_or(previous_end).max(previous_end)),
        };
        if let Some(e) = end {
            previous_end = e;
        }
        out.push(Line {
            start,
            text: line.text.trim().to_owned(),
            words,
            end,
        });
    }
    Ok(out)
}

/// Writes down the words sung in the track at `path`, as [`time_words`]
/// hears them, timed by word.
pub fn transcribe(path: &Path, report: Report) -> Result<Vec<Line>, String> {
    let heard = hear(path, report)?;
    let lines = lines_from(&heard);
    if lines.is_empty() {
        return Err("Whisper didn't hear any singing".into());
    }
    Ok(lines)
}

/// A pause this long starts a new line.
const LINE_GAP: f64 = 0.8;
/// A pause this long leaves a break between verses.
const VERSE_GAP: f64 = 5.0;
/// A line this long breaks at its next word.
const LINE_CHARS: usize = 42;

/// Groups heard words into lines: at a pause, after a sentence ends, or
/// when a line runs long; a long pause leaves an empty line between verses.
pub fn lines_from(heard: &[Heard]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut words: Vec<Word> = Vec::new();
    let mut chars = 0;
    let mut end = 0.0;
    let close = |lines: &mut Vec<Line>, words: &mut Vec<Word>, end: f64| {
        if words.is_empty() {
            return;
        }
        let text: String = words.iter().map(|w| w.text.as_str()).collect();
        lines.push(Line {
            start: Some(words[0].start),
            text: text.trim().to_owned(),
            words: std::mem::take(words),
            end: Some(end),
        });
    };
    for h in heard {
        let text = h.text.trim();
        if plain(text).is_empty() {
            continue;
        }
        if !words.is_empty() {
            let gap = h.start - end;
            let sentence = words
                .last()
                .is_some_and(|w| w.text.trim_end().ends_with(['.', '?', '!']));
            if gap >= LINE_GAP || sentence || chars >= LINE_CHARS {
                close(&mut lines, &mut words, end);
                chars = 0;
                if gap >= VERSE_GAP {
                    lines.push(Line {
                        start: Some(end),
                        ..Line::default()
                    });
                }
            }
        }
        if let Some(last) = words.last_mut() {
            last.text.push(' ');
        }
        words.push(Word {
            start: h.start.max(end),
            text: text.to_owned(),
        });
        chars += text.chars().count() + 1;
        end = h.end.max(h.start).max(end);
    }
    close(&mut lines, &mut words, end);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> Line {
        Line {
            text: text.into(),
            ..Line::default()
        }
    }

    fn heard(words: &[(&str, f64)]) -> Vec<Heard> {
        words
            .iter()
            .map(|(w, t)| Heard {
                start: *t,
                end: t + 0.4,
                text: (*w).into(),
            })
            .collect()
    }

    #[test]
    fn plain_words_ignore_case_and_marks() {
        assert_eq!(plain("Don't,"), "dont");
        assert_eq!(plain("(Hold"), "hold");
        assert_eq!(likeness("hold", "hold"), 2);
        assert_eq!(likeness("lovin", "loving"), 1);
        assert_eq!(likeness("the", "a"), -1);
    }

    #[test]
    fn splits_a_line_into_words_with_their_spaces() {
        assert_eq!(words_of("Hold the  line"), vec!["Hold ", "the  ", "line"]);
        assert_eq!(words_of(""), Vec::<String>::new());
    }

    #[test]
    fn times_the_words_whisper_heard() {
        let lines = vec![
            line("Hold the line"),
            line(""),
            line("Love isn't always on time"),
        ];
        let heard = heard(&[
            ("Hold", 10.0),
            ("the", 10.5),
            ("line", 11.0),
            ("love", 14.0),
            ("isn't", 14.5),
            ("always", 15.0),
            ("on", 15.6),
            ("time", 16.0),
        ]);
        let out = align(&lines, &heard).unwrap();
        assert_eq!(out[0].start, Some(10.0));
        assert_eq!(out[0].words[2].start, 11.0);
        assert_eq!(out[0].end, Some(11.4));
        assert_eq!(out[0].text, "Hold the line");
        // The break starts when the line before ends.
        assert_eq!(out[1].start, Some(11.4));
        assert!(out[1].words.is_empty());
        assert_eq!(out[2].words.len(), 5);
        assert_eq!(out[2].words[4].start, 16.0);
        let joined: String = out[2].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(joined, "Love isn't always on time");
    }

    #[test]
    fn spreads_words_whisper_missed_between_those_it_heard() {
        let lines = vec![line("Hold the line, love isn't always on time")];
        // Whisper missed "love isn't always".
        let heard = heard(&[
            ("hold", 10.0),
            ("the", 10.5),
            ("line", 11.0),
            ("on", 14.0),
            ("time", 14.5),
        ]);
        let out = align(&lines, &heard).unwrap();
        let starts: Vec<f64> = out[0].words.iter().map(|w| w.start).collect();
        assert_eq!(starts[2], 11.0);
        assert_eq!(starts[6], 14.0);
        // "love" starts when "line" ends; the three missed words fill the gap in order.
        assert!((starts[3] - 11.4).abs() < 1e-9);
        assert!(starts[3] < starts[4] && starts[4] < starts[5] && starts[5] < 14.0);
    }

    #[test]
    fn skips_words_whisper_heard_that_are_not_in_the_lyrics() {
        let lines = vec![line("Hold the line")];
        let heard = heard(&[
            ("yeah", 9.0),
            ("hold", 10.0),
            ("uh", 10.2),
            ("the", 10.5),
            ("line", 11.0),
        ]);
        let out = align(&lines, &heard).unwrap();
        let starts: Vec<f64> = out[0].words.iter().map(|w| w.start).collect();
        assert_eq!(starts, vec![10.0, 10.5, 11.0]);
    }

    #[test]
    fn refuses_when_it_heard_too_little() {
        let lines = vec![line("Hold the line, love isn't always on time")];
        let heard = heard(&[("something", 3.0), ("else", 4.0)]);
        assert!(align(&lines, &heard).is_err());
    }

    #[test]
    fn groups_heard_words_into_lines() {
        let heard = heard(&[
            ("Hold", 10.0),
            ("the", 10.5),
            ("line.", 11.0),
            ("Love", 11.6),
            ("isn't", 12.0),
            ("always", 12.5),
            ("on", 13.0),
            ("time", 13.5),
            // A pause between verses.
            ("Oh", 25.0),
            ("-", 25.2),
            ("no", 25.5),
        ]);
        let lines = lines_from(&heard);
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["Hold the line.", "Love isn't always on time", "", "Oh no"]
        );
        assert_eq!(lines[1].start, Some(11.6));
        assert_eq!(lines[1].end, Some(13.9));
        assert_eq!(lines[2].start, Some(13.9));
        assert_eq!(lines[3].words[1].start, 25.5);
        let back = crate::lyrics::parse(&crate::lyrics::to_lrc(&lines)).unwrap();
        assert_eq!(back[1].words.len(), 5);
        assert_eq!(back[1].text, "Love isn't always on time");
    }

    #[test]
    fn breaks_a_long_line() {
        let words: Vec<(&str, f64)> = (0..20).map(|i| ("singing", 1.0 + i as f64 * 0.5)).collect();
        let lines = lines_from(&heard(&words));
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| l.text.len() <= LINE_CHARS + 8));
    }

    #[test]
    fn writes_lyrics_that_read_back_word_timed() {
        let lines = vec![line("Hold the line")];
        let out = align(
            &lines,
            &heard(&[("hold", 1.0), ("the", 1.5), ("line", 2.0)]),
        )
        .unwrap();
        let back = crate::lyrics::parse(&crate::lyrics::to_lrc(&out)).unwrap();
        assert_eq!(back[0].words.len(), 3);
        assert_eq!(back[0].words[1].start, 1.5);
        assert_eq!(back[0].end, Some(2.4));
    }
}
