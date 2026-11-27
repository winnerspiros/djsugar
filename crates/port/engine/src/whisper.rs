//! Hears the words sung in a track: OpenAI's Whisper (MIT) on the rten runtime. The sound goes through in half-minute
//! windows; each word's time comes from where the model was listening as it wrote the word.

use std::ops::Range;
use std::path::Path;

use rten::{Model, NodeId, Value, ValueOrView, ValueView};
use rustfft::{num_complex::Complex32, FftPlanner};

use crate::word_timing::Heard;

/// What the model listens at, mono.
pub const RATE: usize = 16_000;
const FFT: usize = 400;
const HOP: usize = 160;
const MELS: usize = 80;
const BINS: usize = FFT / 2 + 1;
/// One window of sound, and its frames going in and coming out of the encoder.
const WINDOW: usize = 30 * RATE;
const FRAMES: usize = WINDOW / HOP;
const HEARD: usize = FRAMES / 2;
/// Seconds to one frame out of the encoder, and to one step of the time tokens.
const STEP: f64 = 0.02;
/// The most tokens written for one window.
const LONGEST: usize = 224;
/// The latest the first words of a window may be said to start, in steps.
const OPENING: usize = 50;
/// The base model's attention heads to a layer, and the width of each.
const HEADS: usize = 8;
const HEAD: usize = 64;
/// The base model's heads that follow the sound word by word, as (layer, head).
const FOLLOW: [(usize, usize); 8] = [(3, 1), (4, 2), (4, 3), (4, 7), (5, 1), (5, 2), (5, 4), (5, 6)];
/// A window this quiet is not listened to.
const QUIET: f32 = 1e-4;
/// A window the model calls silent this surely, and writes this unsurely, has no words.
const SILENT: f32 = 0.6;
const UNSURE: f32 = -1.0;
/// The longest a word is taken to last, seconds; and the first of a line, which is listened for through the pause before it.
const WORD: f64 = 1.5;
const FIRST: f64 = 0.6;
/// Languages written without spaces: there each token is a word.
const UNSPACED: [&str; 5] = ["zh", "ja", "th", "lo", "my"];

fn fault(e: impl std::fmt::Display) -> String {
    format!("the listening model failed: {e}")
}

/// The model's tokens: what each writes, and the ones that steer it.
struct Tokens {
    text: Vec<Vec<u8>>,
    end: usize,
    start: i32,
    write: i32,
    silent: usize,
    /// The first time token; the ones after it count on in steps.
    zero: usize,
    languages: Range<usize>,
    names: Vec<String>,
}

impl Tokens {
    fn read(path: &Path) -> Result<Self, String> {
        let file = std::fs::read(path).map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_slice(&file).map_err(|e| e.to_string())?;
        // Tokens are spelled a letter to a byte; bytes that don't print are moved up past 255.
        let mut bytes = std::collections::HashMap::new();
        let mut moved = 0;
        for b in 0..=255u8 {
            let prints = matches!(b, 33..=126 | 161..=172 | 174..=255);
            let c = if prints { char::from(b) } else { char::from_u32(256 + moved).unwrap_or(' ') };
            moved += u32::from(!prints);
            bytes.insert(c, b);
        }
        let vocab = json["model"]["vocab"].as_object().ok_or("no vocabulary")?;
        let added = json["added_tokens"].as_array().ok_or("no added tokens")?;
        let mut text = vec![Vec::new(); vocab.len() + added.len()];
        for (word, id) in vocab {
            if let Some(slot) = id.as_u64().and_then(|i| text.get_mut(i as usize)) {
                *slot = word.chars().filter_map(|c| bytes.get(&c).copied()).collect();
            }
        }
        let mut names = vec![String::new(); text.len()];
        for token in added {
            let known = token["id"].as_u64().zip(token["content"].as_str());
            if let Some((id, name)) = known.filter(|(id, _)| (*id as usize) < names.len()) {
                names[id as usize] = name.to_owned();
                text[id as usize].clear();
            }
        }
        let id = |name: &str| names.iter().position(|n| n == name).ok_or_else(|| format!("no {name} token"));
        let start = id("<|startoftranscript|>")?;
        let languages = start + 1..id("<|translate|>")?;
        Ok(Self {
            end: id("<|endoftext|>")?,
            start: start as i32,
            write: id("<|transcribe|>")? as i32,
            silent: id("<|nocaptions|>").or_else(|_| id("<|nospeech|>"))?,
            zero: id("<|0.00|>")?,
            languages,
            text,
            names,
        })
    }

    fn unspaced(&self, language: usize) -> bool {
        let name = self.names[language].trim_matches(['<', '|', '>']);
        UNSPACED.contains(&name)
    }
}

/// The filters that fold a spectrum into [`MELS`] bands: each a first bin and its weights.
fn bands() -> Vec<(usize, Vec<f32>)> {
    let mel = |hz: f64| if hz < 1000.0 { hz * 3.0 / 200.0 } else { 15.0 + (hz / 1000.0).ln() * 27.0 / 6.4f64.ln() };
    let hz = |mel: f64| if mel < 15.0 { mel * 200.0 / 3.0 } else { 1000.0 * ((mel - 15.0) * 6.4f64.ln() / 27.0).exp() };
    let top = mel(RATE as f64 / 2.0);
    let edge: Vec<f64> = (0..MELS + 2).map(|i| hz(top * i as f64 / (MELS + 1) as f64)).collect();
    (0..MELS)
        .map(|m| {
            let (low, mid, high) = (edge[m], edge[m + 1], edge[m + 2]);
            let weights: Vec<f32> = (0..BINS)
                .map(|bin| {
                    let f = bin as f64 * RATE as f64 / FFT as f64;
                    let rise = (f - low) / (mid - low);
                    let fall = (high - f) / (high - mid);
                    (rise.min(fall).max(0.0) * 2.0 / (high - low)) as f32
                })
                .collect();
            let first = weights.iter().position(|w| *w > 0.0).unwrap_or(0);
            let last = weights.iter().rposition(|w| *w > 0.0).map_or(first, |l| l + 1);
            (first, weights[first..last].to_vec())
        })
        .collect()
}

/// One window's loudness by band and frame, band after band, as the model takes it. `sound` is padded with silence.
fn picture(sound: &[f32], bands: &[(usize, Vec<f32>)]) -> Vec<f32> {
    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT);
    let bell: Vec<f32> = (0..FFT).map(|n| 0.5 - 0.5 * (std::f32::consts::TAU * n as f32 / FFT as f32).cos()).collect();
    // The sound is mirrored past both ends, so the first frame is centered on its start.
    let at = |i: isize| -> f32 {
        let last = WINDOW as isize - 1;
        let i = if i < 0 {
            -i
        } else if i > last {
            2 * last - i
        } else {
            i
        };
        sound.get(i as usize).copied().unwrap_or(0.0)
    };
    let mut out = vec![0.0f32; MELS * FRAMES];
    let mut frame = vec![Complex32::default(); FFT];
    let mut power = [0.0f32; BINS];
    for f in 0..FRAMES {
        let from = (f * HOP) as isize - (FFT / 2) as isize;
        for (n, x) in frame.iter_mut().enumerate() {
            *x = Complex32::new(at(from + n as isize) * bell[n], 0.0);
        }
        fft.process(&mut frame);
        for (p, x) in power.iter_mut().zip(&frame) {
            *p = x.norm_sqr();
        }
        for (m, (first, weights)) in bands.iter().enumerate() {
            let sum: f32 = weights.iter().zip(&power[*first..]).map(|(w, p)| w * p).sum();
            out[m * FRAMES + f] = sum.max(1e-10).log10();
        }
    }
    let floor = out.iter().copied().fold(f32::MIN, f32::max) - 8.0;
    for v in &mut out {
        *v = (v.max(floor) + 4.0) / 4.0;
    }
    out
}

/// The decoder and the names of what goes in and out of it.
struct Writer {
    model: Model,
    ids: NodeId,
    heard: NodeId,
    kept: NodeId,
    /// For each layer: its own key and value, then the key and value it has of the sound.
    past: Vec<NodeId>,
    present: Vec<NodeId>,
    logits: NodeId,
    /// The attention on the sound, for each layer [`FOLLOW`] names.
    looks: Vec<(usize, NodeId)>,
}

impl Writer {
    fn load(path: &Path) -> Result<Self, String> {
        let model = Model::load_file(path).map_err(fault)?;
        let id = |name: &str| model.node_id(name).map_err(fault);
        let mut past = Vec::new();
        let mut present = Vec::new();
        let mut layers = 0;
        while model.find_node(&format!("past_key_values.{layers}.decoder.key")).is_some() {
            for part in ["decoder.key", "decoder.value", "encoder.key", "encoder.value"] {
                past.push(id(&format!("past_key_values.{layers}.{part}"))?);
                present.push(id(&format!("present.{layers}.{part}"))?);
            }
            layers += 1;
        }
        let mut looks = Vec::new();
        for layer in 0..layers {
            if FOLLOW.iter().any(|(l, _)| *l == layer) {
                looks.push((layer, id(&format!("cross_attentions.{layer}"))?));
            }
        }
        if looks.is_empty() {
            return Err("the listening model doesn't say where it listens".into());
        }
        Ok(Self {
            ids: id("input_ids")?,
            heard: id("encoder_hidden_states")?,
            kept: id("use_cache_branch")?,
            logits: id("logits")?,
            past,
            present,
            looks,
            model,
        })
    }

    /// Feeds `ids` after what `memory` holds. Gives the scores for the token that follows, and where each of
    /// [`FOLLOW`]'s heads listened for it: [`HEARD`] weights a head.
    fn feed(&self, ids: &[i32], sound: &Value, memory: &mut Vec<Value>) -> Result<(Vec<f32>, Vec<f32>), String> {
        let first = memory.is_empty();
        if first {
            for _ in 0..self.past.len() {
                memory.push(Value::from_shape([1, HEADS, 0, HEAD], Vec::<f32>::new()).map_err(fault)?);
            }
        }
        let flag = [i32::from(!first)];
        let mut inputs: Vec<(NodeId, ValueOrView)> = vec![
            (self.ids, ValueView::from_shape([1, ids.len()], ids).map_err(fault)?.into()),
            (self.heard, sound.into()),
            (self.kept, ValueView::from_shape([0usize; 0], &flag[..]).map_err(fault)?.into()),
        ];
        inputs.extend(self.past.iter().zip(memory.iter()).map(|(id, v)| (*id, v.into())));
        // What a layer has of the sound is worked out once, on the first run.
        let renewed: Vec<usize> = (0..self.past.len()).filter(|i| first || i % 4 < 2).collect();
        let mut wanted = vec![self.logits];
        wanted.extend(renewed.iter().map(|i| self.present[*i]));
        wanted.extend(self.looks.iter().map(|(_, id)| *id));
        let mut out = self.model.run(inputs, &wanted, None).map_err(fault)?.into_iter();
        let (shape, scores): ([usize; 3], Vec<f32>) = out.next().ok_or("no scores")?.into_shape_vec().map_err(fault)?;
        let scores = scores[scores.len() - shape[2]..].to_vec();
        for i in renewed {
            memory[i] = out.next().ok_or("no memory")?;
        }
        let mut looked = Vec::with_capacity(FOLLOW.len() * HEARD);
        for ((layer, _), value) in self.looks.iter().zip(out) {
            let (shape, weights): ([usize; 4], Vec<f32>) = value.into_shape_vec().map_err(fault)?;
            let [_, heads, said, frames] = shape;
            if frames != HEARD {
                return Err("the listening model hears another length".into());
            }
            for (_, head) in FOLLOW.iter().filter(|(l, h)| l == layer && *h < heads) {
                let row = (head * said + said - 1) * frames;
                looked.extend_from_slice(&weights[row..row + frames]);
            }
        }
        Ok((scores, looked))
    }
}

/// Turns scores into the log of each token's chance.
fn chances(scores: &mut [f32]) {
    let top = scores.iter().copied().fold(f32::MIN, f32::max);
    let sum: f32 = scores.iter().map(|s| (s - top).exp()).sum();
    let shift = top + sum.ln();
    for s in scores {
        *s -= shift;
    }
}

/// The token to write next, with its chance: the likeliest one the order of words and times allows after `said`.
fn choose(mut scores: Vec<f32>, said: &[usize], t: &Tokens) -> (usize, f32) {
    const NEVER: f32 = f32::NEG_INFINITY;
    chances(&mut scores);
    scores[t.end + 1..t.zero].fill(NEVER);
    let timed = |i: usize| said.len() > i && said[said.len() - 1 - i] >= t.zero;
    if said.is_empty() {
        scores[..t.zero].fill(NEVER);
        scores[t.zero + OPENING + 1..].fill(NEVER);
    } else if timed(0) {
        // Times come in pairs, a line's end and the next one's start, and words go between the pairs.
        if said.len() < 2 || timed(1) {
            scores[t.zero..].fill(NEVER)
        } else {
            scores[..t.end].fill(NEVER)
        }
    }
    if let Some(last) = said.iter().rev().find(|s| **s >= t.zero) {
        let from = if timed(0) && !timed(1) { *last } else { last + 1 };
        let until = from.min(scores.len());
        scores[t.zero..until].fill(NEVER);
    }
    let best = |part: &[f32]| part.iter().copied().fold(NEVER, f32::max);
    let time = best(&scores[t.zero..]);
    let times: f32 = scores[t.zero..].iter().map(|s| (s - time).exp()).sum::<f32>().ln() + time;
    if times > best(&scores[..t.zero]) {
        scores[..t.zero].fill(NEVER);
    }
    let pick = (0..scores.len()).max_by(|a, b| scores[*a].total_cmp(&scores[*b])).unwrap_or(t.end);
    (pick, scores[pick])
}

/// The middle of each seven neighbors, along a row.
fn smooth(row: &mut [f32]) {
    let n = row.len() as isize;
    let from = row.to_vec();
    let at = |i: isize| {
        from[(if i < 0 {
            -i
        } else if i >= n {
            2 * n - 2 - i
        } else {
            i
        })
        .clamp(0, n - 1) as usize]
    };
    for (i, v) in row.iter_mut().enumerate() {
        let mut near: [f32; 7] = std::array::from_fn(|k| at(i as isize + k as isize - 3));
        near.sort_unstable_by(f32::total_cmp);
        *v = near[3];
    }
}

/// The frame each token starts at: `looked` holds, token after token, where [`FOLLOW`]'s heads listened; only the first
/// `frames` frames have sound. The tokens are laid along the frames in order, each where it was listened for most.
fn starts(looked: &[Vec<f32>], frames: usize) -> Vec<usize> {
    let tokens = looked.len();
    let heads = looked.first().map_or(0, |l| l.len() / HEARD);
    if tokens == 0 || heads == 0 || frames == 0 {
        return vec![0; tokens];
    }
    let mut cost = vec![0.0f32; tokens * frames];
    let mut row = vec![0.0f32; frames];
    for h in 0..heads {
        let of = |t: usize, f: usize| looked[t][h * HEARD + f];
        let mut mean = vec![0.0f32; frames];
        let mut spread = vec![0.0f32; frames];
        for f in 0..frames {
            mean[f] = (0..tokens).map(|t| of(t, f)).sum::<f32>() / tokens as f32;
            let var = (0..tokens).map(|t| (of(t, f) - mean[f]).powi(2)).sum::<f32>() / tokens as f32;
            spread[f] = var.sqrt().max(1e-9);
        }
        for t in 0..tokens {
            for f in 0..frames {
                row[f] = (of(t, f) - mean[f]) / spread[f];
            }
            smooth(&mut row);
            for f in 0..frames {
                cost[t * frames + f] -= row[f] / heads as f32;
            }
        }
    }
    // The cheapest way through, moving on a token, a frame, or both at each step.
    let wide = frames + 1;
    let mut total = vec![f32::INFINITY; (tokens + 1) * wide];
    let mut came = vec![0u8; (tokens + 1) * wide];
    total[0] = 0.0;
    for t in 1..=tokens {
        for f in 1..=frames {
            let both = total[(t - 1) * wide + f - 1];
            let token = total[(t - 1) * wide + f];
            let frame = total[t * wide + f - 1];
            let (least, way) = if both <= token && both <= frame {
                (both, 0)
            } else if token <= frame {
                (token, 1)
            } else {
                (frame, 2)
            };
            total[t * wide + f] = cost[(t - 1) * frames + f - 1] + least;
            came[t * wide + f] = way;
        }
    }
    let mut out = vec![0; tokens];
    let (mut t, mut f) = (tokens, frames);
    while t > 0 && f > 0 {
        out[t - 1] = f - 1;
        match came[t * wide + f] {
            0 => (t, f) = (t - 1, f - 1),
            1 => t -= 1,
            _ => f -= 1,
        }
    }
    out
}

/// One window's tokens with the second each starts at, from the window's start, and the seconds its line runs between.
struct Written {
    token: usize,
    at: f64,
    line: (f64, f64),
}

/// Joins a window's tokens into words. A token that opens with a space starts a word; where the language has no
/// spaces, every token that ends a letter does.
fn words(written: &[Written], t: &Tokens, unspaced: bool, offset: f64, out: &mut Vec<Heard>) {
    let mut bytes: Vec<u8> = Vec::new();
    // The word being joined: where it starts, and the line it is in.
    let mut span: Option<(f64, (f64, f64))> = None;
    let mut line = None;
    let mut close = |bytes: &mut Vec<u8>, span: &mut Option<(f64, (f64, f64))>, until: f64| {
        if let Some((start, (low, high))) = span.take() {
            let opens = line != Some(low);
            line = Some(low);
            let start = start.clamp(low, high);
            let end = until.clamp(start, high.max(start));
            let (start, end) = if opens { (start.max(end - FIRST), end) } else { (start, end.min(start + WORD)) };
            let text = String::from_utf8_lossy(bytes).trim().to_owned();
            if !text.is_empty() {
                out.push(Heard { start: offset + start, end: offset + end, text, opens });
            }
        }
        bytes.clear();
    };
    for w in written {
        let text = &t.text[w.token];
        let whole = std::str::from_utf8(&bytes).is_ok();
        if whole && (text.first() == Some(&b' ') || unspaced || span.is_some_and(|s| s.1 != w.line)) {
            close(&mut bytes, &mut span, w.at);
        }
        span.get_or_insert((w.at, w.line));
        bytes.extend_from_slice(text);
    }
    let until = written.last().map_or(0.0, |w| w.line.1);
    close(&mut bytes, &mut span, until);
}

/// The words sung in `samples` (mono, at [`RATE`]), each with its time. `progress` is told how far along it is, of 100,
/// and stops the listening by answering false.
pub fn hear(
    encoder: &Path,
    decoder: &Path,
    tokens: &Path,
    samples: &[f32],
    progress: &dyn Fn(i32) -> bool,
) -> Result<Vec<Heard>, String> {
    let t = Tokens::read(tokens)?;
    let listener = Model::load_file(encoder).map_err(fault)?;
    let writer = Writer::load(decoder)?;
    let bands = bands();
    let mut out = Vec::new();
    let mut language: Option<usize> = None;
    let mut seek = 0;
    while seek < samples.len() {
        if !progress((seek * 100 / samples.len()) as i32) {
            return Err("stopped".into());
        }
        let real = &samples[seek..samples.len().min(seek + WINDOW)];
        let loud = (real.iter().map(|s| s * s).sum::<f32>() / real.len() as f32).sqrt();
        if loud < QUIET {
            seek += WINDOW;
            continue;
        }
        let picture = picture(real, &bands);
        let input = ValueView::from_shape([1, MELS, FRAMES], &picture[..]).map_err(fault)?;
        let sound = listener.run_one(input.into(), None).map_err(fault)?;
        drop(picture);
        let mut memory = Vec::new();
        let (mut scores, _) = writer.feed(&[t.start], &sound, &mut memory)?;
        chances(&mut scores);
        let silent = scores[t.silent].exp();
        let spoken = language.unwrap_or_else(|| {
            t.languages.clone().max_by(|a, b| scores[*a].total_cmp(&scores[*b])).unwrap_or(t.languages.start)
        });
        let (mut scores, mut looked) = writer.feed(&[spoken as i32, t.write], &sound, &mut memory)?;
        let mut said: Vec<usize> = Vec::new();
        let mut looks: Vec<Vec<f32>> = Vec::new();
        let mut sure = 0.0;
        loop {
            let (token, chance) = choose(scores, &said, &t);
            sure += chance;
            looks.push(looked);
            if token == t.end || said.len() >= LONGEST {
                break;
            }
            said.push(token);
            (scores, looked) = writer.feed(&[token as i32], &sound, &mut memory)?;
        }
        drop(memory);
        let frames = real.len().div_ceil(HOP * 2).clamp(1, HEARD);
        // A window that stops mid-line is taken up again from that line's start.
        let pair = (1..said.len()).rev().find(|i| said[*i] >= t.zero && said[i - 1] >= t.zero);
        let whole = said.last().is_some_and(|l| *l >= t.zero) && !pair.is_some_and(|p| p + 1 == said.len());
        let (kept, moved) = match pair {
            Some(p) if !whole && said[p] > t.zero => (p, (said[p] - t.zero) * HOP * 2),
            _ => (said.len(), WINDOW),
        };
        seek += moved;
        if silent > SILENT && sure / (looks.len() as f32) < UNSURE {
            continue;
        }
        let at = starts(&looks, frames);
        let mut written = Vec::new();
        let mut opened = 0.0;
        for (i, token) in said.iter().enumerate().take(kept) {
            if *token >= t.zero {
                opened = (token - t.zero) as f64 * STEP;
                continue;
            }
            let closes = said[i..]
                .iter()
                .find(|s| **s >= t.zero)
                .map_or(WINDOW as f64 / RATE as f64, |s| (s - t.zero) as f64 * STEP);
            written.push(Written { token: *token, at: at[i] as f64 * STEP, line: (opened, closes.max(opened)) });
        }
        if !written.is_empty() {
            language = Some(spoken);
        }
        words(&written, &t, t.unspaced(spoken), (seek - moved) as f64 / RATE as f64, &mut out);
    }
    progress(100);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_cover_the_spectrum_in_order() {
        let bands = bands();
        assert_eq!(bands.len(), MELS);
        assert!(bands.windows(2).all(|w| w[0].0 <= w[1].0));
        assert!(bands.iter().all(|(first, w)| !w.is_empty() && first + w.len() <= BINS));
    }

    #[test]
    fn a_tone_lights_its_band() {
        let tone: Vec<f32> =
            (0..WINDOW).map(|n| (std::f32::consts::TAU * 1000.0 * n as f32 / RATE as f32).sin()).collect();
        let picture = picture(&tone, &bands());
        let level = |m: usize| picture[m * FRAMES + FRAMES / 2];
        let loudest = (0..MELS).max_by(|a, b| level(*a).total_cmp(&level(*b))).unwrap();
        // 1 kHz is 15 of the 2840 Hz scale's mels, which run to about 45.2 at 8 kHz.
        assert!((25..=28).contains(&loudest), "{loudest}");
        assert!(picture.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn tokens_follow_the_frames_they_were_heard_in() {
        // Three tokens, each listened for in its own third of 30 frames, by one head.
        let looked: Vec<Vec<f32>> =
            (0..3).map(|t| (0..HEARD).map(|f| if f / 10 == t { 1.0 } else { 0.0 }).collect()).collect();
        assert_eq!(starts(&looked, 30), vec![0, 10, 20]);
    }

    /// Run with `DJSUGAR_MODELS` holding the three model files and `DJSUGAR_TRACK` naming a 16 kHz mono WAV.
    #[test]
    #[ignore = "the model files and a track of one's own"]
    fn hears_a_real_track() {
        let models = std::path::PathBuf::from(std::env::var_os("DJSUGAR_MODELS").unwrap());
        let wav = std::fs::read(std::env::var_os("DJSUGAR_TRACK").unwrap()).unwrap();
        let samples: Vec<f32> =
            wav[44..].as_chunks::<2>().0.iter().map(|&b| f32::from(i16::from_le_bytes(b)) / 32768.0).collect();
        let [encoder, decoder, tokens] = crate::word_timing::MODELS.map(|(name, _)| models.join(name));
        let heard = hear(&encoder, &decoder, &tokens, &samples, &|_| true).unwrap();
        for h in &heard {
            println!("{:7.2} {:7.2} {}", h.start, h.end, h.text);
        }
        assert!(heard.len() > 20);
        assert!(heard.windows(2).all(|w| w[0].start <= w[1].start + 1.0));
    }
}
