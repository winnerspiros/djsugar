//! The test tracks: drums, bass and chords synthesised from a fixed seed
//! in a style, tempo and key, so their answers are known and the same files
//! come out on every machine.

use std::f64::consts::TAU;
use std::path::Path;

use djsugar_engine::key::Key;
use serde_json::{json, Value};

const RATE: u32 = 44_100;
const SECONDS: f64 = 75.0;

/// A track's answers.
pub struct Truth {
    pub file: String,
    pub bpm: f64,
    /// Where the first beat is, in seconds; the grid repeats from there.
    pub first_beat: f64,
    pub key: Key,
    /// The genre a DJ's file of this style would be tagged with, if any.
    pub genre: String,
}

#[derive(Clone, Copy, Debug)]
enum Style {
    /// Kick on every beat, clap on 2 and 4, open hats on the offbeats.
    House,
    /// Kick on every beat, sixteenth hats, rolling offbeat bass.
    Techno,
    /// Kick on 1 and the "and" of 2, snare on 2 and 4.
    Breaks,
    /// Kick on 1 and the "and" of 3, snare on 3: half-time feel.
    HipHop,
    /// Two-step: kick on 1 and the "and" of 3, snare on 2 and 4, fast.
    DrumAndBass,
    /// Kick on every beat, snare on the 3-3-2 dembow.
    Dembow,
    /// Shuffled eighths, kick on 1 and 3, snare on 2 and 4.
    Swing,
    /// Eight bars of chords alone before the drums come in.
    AmbientIntro,
}

/// The tracks: (style, tempo, tonic 0 = C, minor).
const TRACKS: [(Style, f64, u8, bool); 24] = [
    (Style::House, 124.0, 9, true),
    (Style::House, 122.0, 0, false),
    (Style::House, 128.0, 2, true),
    (Style::Techno, 130.0, 4, true),
    (Style::Techno, 138.0, 7, false),
    (Style::Techno, 126.0, 11, true),
    (Style::Breaks, 110.0, 5, false),
    (Style::Breaks, 132.0, 1, true),
    (Style::HipHop, 88.0, 3, false),
    (Style::HipHop, 94.0, 6, true),
    (Style::HipHop, 72.0, 10, false),
    (Style::DrumAndBass, 174.0, 8, true),
    (Style::DrumAndBass, 170.0, 2, false),
    (Style::Dembow, 95.0, 0, true),
    (Style::Dembow, 100.0, 9, false),
    (Style::Swing, 105.0, 5, true),
    (Style::Swing, 116.0, 4, false),
    (Style::AmbientIntro, 120.0, 7, true),
    (Style::AmbientIntro, 140.0, 11, false),
    (Style::House, 118.5, 1, false),
    (Style::Techno, 145.0, 3, true),
    (Style::Breaks, 125.0, 10, true),
    (Style::HipHop, 99.0, 8, false),
    (Style::Dembow, 92.0, 6, false),
];

/// A small fixed-seed generator: the same noise everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

fn hz(midi: f64) -> f64 {
    440.0 * 2f64.powf((midi - 69.0) / 12.0)
}

/// Adds a sound starting at `at` seconds, `gain` on the left and right.
fn mix(out: &mut [[f32; 2]], at: f64, gain: [f64; 2], sound: &[f64]) {
    let start = (at * RATE as f64).round() as usize;
    for (i, s) in sound.iter().enumerate() {
        let Some(frame) = out.get_mut(start + i) else {
            break;
        };
        frame[0] += (s * gain[0]) as f32;
        frame[1] += (s * gain[1]) as f32;
    }
}

fn kick() -> Vec<f64> {
    let n = (0.35 * RATE as f64) as usize;
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            phase += TAU * (45.0 + 110.0 * (-t * 30.0).exp()) / RATE as f64;
            phase.sin() * (-t * 9.0).exp()
        })
        .collect()
}

fn noise_hit(rng: &mut Rng, seconds: f64, decay: f64, bright: bool) -> Vec<f64> {
    let n = (seconds * RATE as f64) as usize;
    let mut last = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let x = rng.next();
            // A first difference brightens the noise for hats.
            let y = if bright { x - last } else { x };
            last = x;
            y * (-t * decay).exp()
        })
        .collect()
}

fn snare(rng: &mut Rng) -> Vec<f64> {
    let body = noise_hit(rng, 0.2, 25.0, false);
    body.iter()
        .enumerate()
        .map(|(i, n)| {
            let t = i as f64 / RATE as f64;
            0.6 * n + 0.5 * (TAU * 190.0 * t).sin() * (-t * 30.0).exp()
        })
        .collect()
}

/// A chord held for `seconds`: each note with a few harmonics.
fn chord(notes: &[f64], seconds: f64) -> Vec<f64> {
    let n = (seconds * RATE as f64) as usize;
    let fade = (0.02 * RATE as f64) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let env = (i.min(n - i) as f64 / fade as f64).min(1.0);
            let s: f64 = notes
                .iter()
                .map(|&m| {
                    let f = hz(m);
                    (1..=4)
                        .map(|h| (TAU * f * h as f64 * t).sin() / h as f64)
                        .sum::<f64>()
                })
                .sum();
            s * env / notes.len() as f64
        })
        .collect()
}

/// A bass note: a soft square an octave and more below the chords.
fn bass(midi: f64, seconds: f64) -> Vec<f64> {
    let n = (seconds * RATE as f64) as usize;
    let f = hz(midi);
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let s = (TAU * f * t).sin() + (TAU * 3.0 * f * t).sin() / 3.0;
            s * (-t * 3.0).exp() * (1.0 - (i as f64 / n as f64)).min(1.0)
        })
        .collect()
}

/// Four bars of chords for a key: i iv V i in minor, I V vi IV in major.
/// The minor V is major, so its leading tone tells the key from its
/// relative major (i VI III VII would hold the same notes as both).
fn progression(tonic: u8, minor: bool) -> [(i32, bool); 4] {
    let t = i32::from(tonic);
    if minor {
        [(t, true), (t + 5, true), (t + 7, false), (t, true)]
    } else {
        [(t, false), (t + 7, false), (t + 9, true), (t + 5, false)]
    }
}

fn triad(root: i32, minor: bool) -> Vec<f64> {
    // Around middle C, the root kept between F3 and E4.
    let r = 53 + (root - 53).rem_euclid(12);
    let third = if minor { 3 } else { 4 };
    vec![f64::from(r), f64::from(r + third), f64::from(r + 7)]
}

fn render(style: Style, bpm: f64, tonic: u8, minor: bool, seed: u64) -> (Vec<[f32; 2]>, f64) {
    let mut rng = Rng(seed);
    let beat = 60.0 / bpm;
    let bar = 4.0 * beat;
    // The music starts a little in, as tracks do.
    let first = 0.05 + 0.3 * (rng.next() + 1.0) / 2.0;
    let mut out = vec![[0f32; 2]; (SECONDS * RATE as f64) as usize];
    let (k, c, oh) = (
        kick(),
        snare(&mut rng),
        noise_hit(&mut rng, 0.15, 18.0, true),
    );
    let ch = noise_hit(&mut rng, 0.04, 80.0, true);
    let bars = ((SECONDS - first) / bar) as usize;
    let drums_from = if matches!(style, Style::AmbientIntro) {
        8
    } else {
        0
    };
    let swing = |eighth: usize| {
        if matches!(style, Style::Swing) && eighth % 2 == 1 {
            beat * (2.0 / 3.0)
        } else {
            beat * 0.5 * (eighth % 2) as f64
        }
    };
    let chords = progression(tonic, minor);
    for b in 0..bars {
        let t0 = first + b as f64 * bar;
        let (root, chord_minor) = chords[b % 4];
        mix(
            &mut out,
            t0,
            [0.22, 0.22],
            &chord(&triad(root, chord_minor), bar),
        );
        if b < drums_from {
            continue;
        }
        for q in 0..4 {
            let at = t0 + q as f64 * beat;
            let bass_note = f64::from(36 + (root - 36).rem_euclid(12));
            match style {
                Style::House | Style::AmbientIntro => {
                    mix(&mut out, at, [0.9, 0.9], &k);
                    if q % 2 == 1 {
                        mix(&mut out, at, [0.35, 0.4], &c);
                    }
                    mix(&mut out, at + beat * 0.5, [0.12, 0.2], &oh);
                    mix(
                        &mut out,
                        at + beat * 0.5,
                        [0.35, 0.35],
                        &bass(bass_note, beat * 0.4),
                    );
                }
                Style::Techno => {
                    mix(&mut out, at, [0.95, 0.95], &k);
                    for s in 0..4 {
                        mix(&mut out, at + beat * 0.25 * s as f64, [0.1, 0.07], &ch);
                    }
                    for s in 1..4 {
                        mix(
                            &mut out,
                            at + beat * 0.25 * s as f64,
                            [0.2, 0.2],
                            &bass(bass_note, beat * 0.2),
                        );
                    }
                }
                Style::Breaks | Style::Swing => {
                    let kicks: &[f64] = if matches!(style, Style::Breaks) {
                        &[0.0, 1.5, 2.75]
                    } else {
                        &[0.0, 2.0]
                    };
                    if q == 0 {
                        for &x in kicks {
                            mix(&mut out, t0 + x * beat, [0.9, 0.9], &k);
                        }
                        mix(&mut out, t0, [0.35, 0.35], &bass(bass_note, bar * 0.5));
                    }
                    if q % 2 == 1 {
                        mix(&mut out, at, [0.5, 0.5], &c);
                    }
                    for e in 0..2 {
                        mix(&mut out, at + swing(e), [0.1, 0.13], &ch);
                    }
                }
                Style::HipHop => {
                    if q == 0 {
                        mix(&mut out, t0, [0.9, 0.9], &k);
                        mix(&mut out, t0 + 2.5 * beat, [0.8, 0.8], &k);
                        mix(&mut out, t0 + 2.0 * beat, [0.55, 0.55], &c);
                        mix(&mut out, t0, [0.4, 0.4], &bass(bass_note, bar * 0.6));
                    }
                    for e in 0..2 {
                        mix(&mut out, at + swing(e), [0.08, 0.1], &ch);
                    }
                }
                Style::DrumAndBass => {
                    if q == 0 {
                        mix(&mut out, t0, [0.9, 0.9], &k);
                        mix(&mut out, t0 + 2.5 * beat, [0.8, 0.8], &k);
                        mix(&mut out, t0, [0.4, 0.4], &bass(bass_note, bar));
                    }
                    if q % 2 == 1 {
                        mix(&mut out, at, [0.55, 0.55], &c);
                    }
                    for e in 0..2 {
                        mix(&mut out, at + swing(e), [0.1, 0.08], &ch);
                    }
                }
                Style::Dembow => {
                    mix(&mut out, at, [0.9, 0.9], &k);
                    if q % 2 == 0 {
                        // 3-3-2 over two beats: the snare on the 4th and 7th sixteenth.
                        mix(&mut out, at + 0.75 * beat, [0.45, 0.45], &c);
                        mix(&mut out, at + 1.5 * beat, [0.45, 0.45], &c);
                        mix(&mut out, at, [0.35, 0.35], &bass(bass_note, beat * 1.5));
                    }
                    mix(&mut out, at + 0.5 * beat, [0.1, 0.12], &ch);
                }
            }
        }
    }
    let peak = out
        .iter()
        .flat_map(|f| f.iter())
        .fold(0f32, |m, s| m.max(s.abs()));
    let gain = if peak > 0.0 { 0.9 / peak } else { 1.0 };
    for f in &mut out {
        f[0] *= gain;
        f[1] *= gain;
    }
    (out, first)
}

fn write_wav(path: &Path, frames: &[[f32; 2]]) -> std::io::Result<()> {
    let data_len = (frames.len() * 4) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for f in frames {
        for s in f {
            let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(path, b)
}

impl Style {
    /// The genre tag a track in this style usually carries.
    fn genre(self) -> &'static str {
        match self {
            Style::House => "House",
            Style::Techno => "Techno",
            Style::Breaks => "Breaks",
            Style::HipHop => "Hip Hop",
            Style::DrumAndBass => "Drum & Bass",
            Style::Dembow => "Reggaeton",
            Style::Swing | Style::AmbientIntro => "",
        }
    }
}

fn name(i: usize, style: Style, bpm: f64) -> String {
    format!("{i:02}-{}-{bpm}.wav", format!("{style:?}").to_lowercase())
}

/// Writes the tracks and `truth.json` into `dir`.
pub fn write(dir: &Path) -> Result<bool, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut truth = Vec::new();
    for (i, &(style, bpm, tonic, minor)) in TRACKS.iter().enumerate() {
        let (frames, first) = render(style, bpm, tonic, minor, 0x9e37_79b9 + i as u64);
        let file = name(i, style, bpm);
        write_wav(&dir.join(&file), &frames).map_err(|e| format!("{file}: {e}"))?;
        let key = Key { tonic, minor };
        truth.push(json!({
            "file": file, "bpm": bpm, "first_beat": first, "key": key.index(), "genre": style.genre()
        }));
        println!("{file}  {bpm} BPM  {}", key.camelot());
    }
    let text = serde_json::to_string_pretty(&truth).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("truth.json"), text).map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn read_truth(dir: &Path) -> Result<Vec<Truth>, String> {
    let path = dir.join("truth.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    v.iter()
        .map(|t| {
            Ok(Truth {
                file: t["file"]
                    .as_str()
                    .ok_or("truth without a file")?
                    .to_string(),
                bpm: t["bpm"].as_f64().ok_or("truth without a tempo")?,
                first_beat: t["first_beat"].as_f64().unwrap_or(0.0),
                key: Key::from_index(t["key"].as_u64().unwrap_or(99) as u8)
                    .ok_or("truth without a key")?,
                genre: t["genre"].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect()
}
