//! Times each word of a track's lyrics by listening to it: Whisper hears the words with their times, and the lyrics the DJ has
//! are lined up with what it heard. Words heard well take its times; those between are spread over the gap by their length. A
//! track with no lyrics is written down instead. The model (OpenAI Whisper base, MIT) is downloaded when first needed.

use std::path::Path;

use crate::lyrics::{Line, Word};
use crate::whisper::Ear;

/// The model: what hears the sound, what writes the words, and the words it knows. About 290 MB together.
pub const MODELS: [(&str, &str); 3] = [
    ("whisper-base-encoder.onnx", "7fcea817bb2be4d86729b521e5a7fcbec28fa743edfed67e882b33ff15852540"),
    ("whisper-base-decoder.onnx", "cec3e1ce94220497665b629710fb24f049025d4f88673563f38e475af6b8a06d"),
    ("whisper-base-tokens.json", "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566"),
];

/// The share of the lyrics' words Whisper must have heard for the times
/// to be trusted.
const ENOUGH: f64 = 0.3;

/// A word Whisper heard, with when.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// Whether Whisper began a line with it.
    pub opens: bool,
}

/// What [`time_words`] is doing, for the DJ to see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// Downloading the model, the first time.
    Downloading,
    /// Listening, 0 to 100 percent.
    Listening(i32),
}

/// Where [`time_words`] reports its [`Stage`].
pub type Report = std::sync::Arc<dyn Fn(Stage) + Send + Sync>;

/// Times each word of `lines` by listening to the track at `path`: its vocal stem when it has one, else the whole
/// mix. Lines keep their text; each sung line gets its words' times.
pub fn time_words(path: &Path, lines: &[Line], report: Report) -> Result<Vec<Line>, String> {
    let told = report.clone();
    listen(path, report, |ear, samples| {
        let heard = ear.hear(samples, &|p| {
            told(Stage::Listening(p * 2 / 3));
            true
        })?;
        let mut timed = align(lines, &heard)?;
        place(ear, samples, &mut timed, &|p| told(Stage::Listening(66 + p / 3)))?;
        Ok(timed)
    })
}

/// Listens again for the words of `timed`, now that about when each is sung is known, and sets each where the
/// model is found listening for it.
fn place(ear: &Ear, samples: &[f32], timed: &mut [Line], told: &dyn Fn(i32)) -> Result<(), String> {
    let known: Vec<(&str, f64)> =
        timed.iter().flat_map(|l| l.words.iter().map(|w| (w.text.as_str(), w.start))).collect();
    let placed = ear.place(samples, &known, &|p| {
        told(p);
        true
    })?;
    let mut placed = placed.into_iter();
    let mut last = 0.0f64;
    for line in timed.iter_mut().filter(|l| !l.words.is_empty()) {
        for (word, found) in line.words.iter_mut().zip(&mut placed) {
            last = found.max(last);
            word.start = last;
        }
        line.start = line.words.first().map(|w| w.start);
        line.end = line.end.map(|e| e.max(last));
    }
    Ok(())
}

/// Every written word of `lines`: its line, its text with the space after, and its plain form.
fn written(lines: &[Line]) -> Vec<(usize, String, String)> {
    let mut written = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut words = words_of(&line.text);
        if let Some(last) = words.last_mut() {
            last.push(' ');
        }
        for w in words {
            let p = plain(&w);
            written.push((i, w, p));
        }
    }
    written
}

/// The words Whisper hears in the track at `path`.
pub fn hear(path: &Path, report: Report) -> Result<Vec<Heard>, String> {
    let told = report.clone();
    listen(path, report, |ear, samples| {
        ear.hear(samples, &|p| {
            told(Stage::Listening(p));
            true
        })
    })
}

/// Has the model, fetched if it must be, do `work` on the sound of the track at `path`.
fn listen<T>(path: &Path, report: Report, work: impl FnOnce(&Ear, &[f32]) -> Result<T, String>) -> Result<T, String> {
    if MODELS.iter().any(|(name, _)| crate::separation::model(name).is_none()) {
        report(Stage::Downloading);
    }
    let mut files = Vec::new();
    for (name, _) in MODELS {
        files.push(crate::separation::model_or_fetch(name, &MODELS)?);
    }
    let samples = vocals(path)?;
    report(Stage::Listening(0));
    if crate::background::is_low() {
        crate::background::lower_rten_priority();
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        work(&Ear::open(&files[0], &files[1], &files[2])?, &samples)
    }));
    result.unwrap_or_else(|_| Err("the listening model crashed".into()))
}

/// The track's vocals, mono, at the rate the model listens at.
fn vocals(path: &Path) -> Result<Vec<f32>, String> {
    use crate::stems::StemSource;
    let source = match crate::stems::find(path) {
        Some(StemSource::Files(files)) if files.len() == 4 => files[3].clone(),
        _ => path.to_owned(),
    };
    let audio = crate::decode::decode(&source)?;
    let mono: Vec<f32> = audio.decoded_chunks().flatten().map(|f| (f[0] + f[1]) * 0.5).collect();
    djsugar_separation::resample(&mono, audio.sample_rate, crate::whisper::RATE as u32)
}

fn plain(word: &str) -> String {
    word.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
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
            let next = (diagonal + usize::from(ca != *cb)).min(row[j] + 1).min(row[j + 1] + 1);
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
    let mut written = written(lines);
    for i in 0..written.len() {
        if written.get(i + 1).is_none_or(|next| next.0 != written[i].0) {
            written[i].1.pop();
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
    let mut times: Vec<Option<(f64, f64)>> =
        found.iter().map(|f| f.map(|j| (heard[j].start, heard[j].end.max(heard[j].start)))).collect();
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
        let from = first.checked_sub(1).and_then(|p| times[p]).map(|t| t.1).or_else(|| lines[written[first].0].start);
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
            words.push(Word { start, text: written[w].1.clone() });
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
        out.push(Line { start, text: line.text.trim().to_owned(), words, end });
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
const LINE_CHARS: usize = 64;

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
            let sentence = words.last().is_some_and(|w| w.text.trim_end().ends_with(['.', '?', '!']));
            if gap >= LINE_GAP || sentence || h.opens || chars >= LINE_CHARS {
                close(&mut lines, &mut words, end);
                chars = 0;
                if gap >= VERSE_GAP {
                    lines.push(Line { start: Some(end), ..Line::default() });
                }
            }
        }
        if let Some(last) = words.last_mut() {
            last.text.push(' ');
        }
        words.push(Word { start: h.start.max(end), text: text.to_owned() });
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
        Line { text: text.into(), ..Line::default() }
    }

    fn heard(words: &[(&str, f64)]) -> Vec<Heard> {
        words.iter().map(|(w, t)| Heard { start: *t, end: t + 0.4, text: (*w).into(), opens: false }).collect()
    }

    /// What was heard in `track`, kept beside it from one run to the next unless `DJSUGAR_REHEAR` is set.
    fn kept(track: &Path) -> Vec<Heard> {
        let file = track.with_extension("heard");
        let before = std::fs::read_to_string(&file).ok().filter(|_| std::env::var_os("DJSUGAR_REHEAR").is_none());
        if let Some(text) = before {
            return text
                .lines()
                .filter_map(|l| {
                    let mut p = l.splitn(4, '\t');
                    let (start, end, opens) = (p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()? == "1");
                    Some(Heard { start, end, opens, text: p.next()?.to_owned() })
                })
                .collect();
        }
        let heard = hear(track, std::sync::Arc::new(|_| ())).unwrap();
        let text: String =
            heard.iter().map(|h| format!("{}\t{}\t{}\t{}\n", h.start, h.end, u8::from(h.opens), h.text)).collect();
        std::fs::write(file, text).unwrap();
        heard
    }

    /// Run with `DJSUGAR_MARKED` naming a folder of tracks, each beside its lyrics (`.txt`) and a `.csv` whose first
    /// column is the second every word of them starts at, under a heading.
    #[test]
    #[ignore = "the model files and marked tracks of one's own"]
    fn lands_on_marked_words() {
        let Some(folder) = std::env::var_os("DJSUGAR_MARKED") else { return };
        let mut tracks: Vec<_> = std::fs::read_dir(folder).unwrap().flatten().map(|e| e.path()).collect();
        tracks.retain(|p| p.extension().is_some_and(|e| e == "mp3"));
        tracks.sort();
        let mut all: Vec<f64> = Vec::new();
        for track in tracks {
            let text = std::fs::read_to_string(track.with_extension("txt")).unwrap();
            let lines: Vec<Line> = text.lines().filter(|l| !l.trim().is_empty()).map(line).collect();
            let marks = std::fs::read_to_string(track.with_extension("csv")).unwrap();
            let marks: Vec<f64> = marks.lines().skip(1).filter_map(|l| l.split(',').next()?.parse().ok()).collect();
            let name = track.file_stem().unwrap().to_str().unwrap().to_owned();
            let mut timed = match align(&lines, &kept(&track)) {
                Ok(timed) => timed,
                Err(why) => {
                    println!("{name:32.32} {why}");
                    continue;
                }
            };
            let starts =
                |timed: &[Line]| timed.iter().flat_map(|l| l.words.iter().map(|w| w.start)).collect::<Vec<_>>();
            let rough = starts(&timed);
            assert_eq!(rough.len(), marks.len(), "{}", track.display());
            listen(&track, std::sync::Arc::new(|_| ()), |ear, samples| place(ear, samples, &mut timed, &|_| ()))
                .unwrap();
            let ours = starts(&timed);
            let off: Vec<f64> = ours.iter().zip(&marks).map(|(a, b)| a - b).collect();
            println!("{}", report(&off, &name));
            if let Some(out) = std::env::var_os("DJSUGAR_OFF") {
                let rows: String =
                    (0..marks.len()).map(|i| format!("{:.2}\t{:.2}\t{:.2}\n", marks[i], rough[i], ours[i])).collect();
                std::fs::write(Path::new(&out).join(track.with_extension("off").file_name().unwrap()), rows).unwrap();
            }
            all.extend(off);
        }
        println!("{}", report(&all, "all"));
    }

    /// How far off the marks a run was: the mean and middle distance, the lean, and the share within some bounds.
    fn report(off: &[f64], name: &str) -> String {
        let mut far: Vec<f64> = off.iter().map(|o| o.abs()).collect();
        far.sort_by(f64::total_cmp);
        let n = far.len().max(1) as f64;
        let within = |s: f64| 100.0 * far.iter().filter(|f| **f <= s).count() as f64 / n;
        let mut lean = off.to_vec();
        lean.sort_by(f64::total_cmp);
        format!(
            "{name:32.32} n={:4} mean={:.3} mid={:.3} lean={:+.3} in50={:.0}% in100={:.0}% in200={:.0}% in300={:.0}% in1s={:.0}%",
            far.len(),
            far.iter().sum::<f64>() / n,
            far[far.len() / 2],
            lean[lean.len() / 2],
            within(0.05),
            within(0.1),
            within(0.2),
            within(0.3),
            within(1.0),
        )
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
        let lines = vec![line("Hold the line"), line(""), line("Love isn't always on time")];
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
        let heard = heard(&[("hold", 10.0), ("the", 10.5), ("line", 11.0), ("on", 14.0), ("time", 14.5)]);
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
        let heard = heard(&[("yeah", 9.0), ("hold", 10.0), ("uh", 10.2), ("the", 10.5), ("line", 11.0)]);
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
        assert_eq!(texts, vec!["Hold the line.", "Love isn't always on time", "", "Oh no"]);
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
        let out = align(&lines, &heard(&[("hold", 1.0), ("the", 1.5), ("line", 2.0)])).unwrap();
        let back = crate::lyrics::parse(&crate::lyrics::to_lrc(&out)).unwrap();
        assert_eq!(back[0].words.len(), 3);
        assert_eq!(back[0].words[1].start, 1.5);
        assert_eq!(back[0].end, Some(2.4));
    }
}
