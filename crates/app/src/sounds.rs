//! Pad sounds made in the app: a take from the microphone, and a part cut out of a longer sound.

use std::path::{Path, PathBuf};

/// How many columns a sound's outline has.
pub const POINTS: usize = 600;
/// The shortest part that can be cut out, in seconds.
const SHORTEST: f32 = 0.05;
/// The fade at each end of a cut, in seconds, so a cut in the middle of a sound never clicks.
const FADE: f32 = 0.004;

/// Makes a take ready to be a pad: the quiet before and after it goes (under -45 dB of its peak, keeping 5 ms of
/// lead-in), it is brought to -1 dB and its last 10 ms fade out. A take of silence comes back empty.
pub fn tighten(take: &mut Vec<f32>, rate: u32) {
    let peak = take.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak < 1e-4 {
        take.clear();
        return;
    }
    let loud = |s: &f32| s.abs() > peak * 0.0056;
    let first = take.iter().position(loud).unwrap_or(0).saturating_sub(rate as usize / 200);
    let last = take.iter().rposition(loud).map_or(take.len(), |i| i + 1);
    take.truncate(last);
    take.drain(..first);
    let (len, fade) = (take.len(), take.len().min(rate as usize / 100));
    for (i, s) in take.iter_mut().enumerate() {
        let left = len - i;
        *s *= 0.891 / peak * if left < fade { left as f32 / fade as f32 } else { 1.0 };
    }
}

/// A 16-bit WAV file's bytes.
pub fn wav(channels: u16, rate: u32, samples: &[f32]) -> Vec<u8> {
    let bytes = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend(b"RIFF");
    out.extend((36 + bytes).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(channels.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * u32::from(channels) * 2).to_le_bytes());
    out.extend((channels * 2).to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(bytes.to_le_bytes());
    for s in samples {
        out.extend(((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

/// A file in `dir` that is not there yet, called `name` or, when that is taken, `name-2`, `name-3` and on.
pub fn free(dir: &Path, name: &str) -> PathBuf {
    let base: String = name.trim().chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).collect();
    let base = if base.is_empty() { "sample".to_string() } else { base };
    let mut path = dir.join(format!("{base}.wav"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base}-{n}.wav"));
        n += 1;
    }
    path
}

/// A sound open for cutting: all of it, its outline, and the part chosen, as shares of its length.
pub struct Cut {
    pub name: String,
    pub rate: u32,
    frames: Vec<[f32; 2]>,
    pub outline: Vec<f32>,
    pub from: f32,
    pub to: f32,
}

impl Cut {
    pub fn new(name: String, rate: u32, frames: Vec<[f32; 2]>) -> Self {
        let per = frames.len().div_ceil(POINTS).max(1);
        let mut outline: Vec<f32> =
            frames.chunks(per).map(|c| c.iter().fold(0f32, |m, f| m.max(f[0].abs()).max(f[1].abs()))).collect();
        let top = outline.iter().fold(0f32, |m, v| m.max(*v));
        if top > 0.0 {
            outline.iter_mut().for_each(|v| *v /= top);
        }
        Self { name, rate, frames, outline, from: 0.0, to: 1.0 }
    }

    /// Reads the sound in the file at `path`, whatever kind of audio it is.
    pub fn open(path: &Path) -> Result<Self, String> {
        let audio = djsugar_engine::decode::decode(path)?;
        let frames: Vec<[f32; 2]> = audio.decoded_chunks().flatten().copied().collect();
        if frames.is_empty() {
            return Err("That sound is empty".into());
        }
        let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("sample").to_string();
        Ok(Self::new(name, audio.sample_rate, frames))
    }

    pub fn seconds(&self) -> f32 {
        self.frames.len() as f32 / self.rate.max(1) as f32
    }

    /// The least the two ends may be apart, as a share of the length.
    fn least(&self) -> f32 {
        (SHORTEST / self.seconds().max(SHORTEST)).min(1.0)
    }

    pub fn set_from(&mut self, v: f32) {
        self.from = v.clamp(0.0, self.to - self.least()).max(0.0);
    }

    pub fn set_to(&mut self, v: f32) {
        self.to = v.clamp(self.from + self.least(), 1.0).min(1.0);
    }

    /// The chosen part, both channels in turn, with a short fade at each end.
    pub fn part(&self) -> Vec<f32> {
        let len = self.frames.len();
        let first = ((self.from * len as f32) as usize).min(len);
        let last = ((self.to * len as f32).ceil() as usize).clamp(first, len);
        let frames = last - first;
        let fade = ((FADE * self.rate as f32) as usize).min(frames / 4).max(1);
        self.frames[first..last]
            .iter()
            .enumerate()
            .flat_map(|(i, f)| {
                let gain = (i.min(frames - 1 - i) as f32 / fade as f32).min(1.0);
                [f[0] * gain, f[1] * gain]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_take_loses_its_quiet_ends_and_is_brought_to_level() {
        let mut take = vec![0.0; 48_000];
        take.extend((0..4800).map(|i| 0.25 * (i as f32 * 0.3).sin()));
        take.extend(vec![0.0001; 24_000]);
        tighten(&mut take, 48_000);
        // 5 ms of lead-in is kept before the sound.
        assert!((take.len() as i64 - (4800 + 240)).abs() < 8, "{}", take.len());
        assert!(take[..230].iter().all(|s| *s == 0.0));
        let peak = take.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.891).abs() < 0.01, "{peak}");
        assert!(take.last().unwrap().abs() < 0.01, "it ends faded out");
    }

    #[test]
    fn a_take_of_silence_is_nothing() {
        let mut take = vec![0.00001; 10_000];
        tighten(&mut take, 48_000);
        assert!(take.is_empty());
    }

    #[test]
    fn wav_says_what_it_holds() {
        let w = wav(2, 44_100, &[0.0, 1.0, -1.0, 0.5]);
        assert_eq!(w.len(), 44 + 8);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([w[22], w[23]]), 2);
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 44_100);
        assert_eq!(u32::from_le_bytes([w[28], w[29], w[30], w[31]]), 176_400);
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 8);
        assert_eq!(i16::from_le_bytes([w[46], w[47]]), 32_767);
        assert_eq!(i16::from_le_bytes([w[48], w[49]]), -32_767);
    }

    #[test]
    fn a_name_in_use_gets_a_number_and_unsafe_letters_go() {
        let dir = std::env::temp_dir().join(format!("djsugar-sounds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free(&dir, " a/b:c "), dir.join("a_b_c.wav"));
        assert_eq!(free(&dir, ""), dir.join("sample.wav"));
        std::fs::write(dir.join("kick.wav"), b"").unwrap();
        assert_eq!(free(&dir, "kick"), dir.join("kick-2.wav"));
        std::fs::write(dir.join("kick-2.wav"), b"").unwrap();
        assert_eq!(free(&dir, "kick"), dir.join("kick-3.wav"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_cut_is_the_chosen_part_with_faded_ends() {
        let frames: Vec<[f32; 2]> = (0..48_000).map(|i| [0.5, -(i as f32) / 48_000.0]).collect();
        let mut cut = Cut::new("x".into(), 48_000, frames);
        assert_eq!(cut.outline.len(), POINTS);
        assert!((cut.outline[POINTS - 1] - 1.0).abs() < 1e-3 && (cut.outline[0] - 0.5).abs() < 0.01);
        cut.set_from(0.25);
        cut.set_to(0.5);
        let part = cut.part();
        assert_eq!(part.len(), 2 * 12_000);
        assert_eq!(part[0], 0.0, "it opens from nothing");
        assert!(part[part.len() - 2].abs() < 1e-6, "and closes to nothing");
        assert_eq!(part[2 * 6000], 0.5);
        assert!((part[2 * 6000 + 1] + 18_000.0 / 48_000.0).abs() < 1e-4, "the right channel, from the right place");
    }

    #[test]
    fn the_ends_of_a_cut_never_cross() {
        let mut cut = Cut::new("x".into(), 48_000, vec![[0.1; 2]; 48_000]);
        cut.set_to(0.3);
        cut.set_from(0.9);
        assert!((cut.from - 0.25).abs() < 1e-5, "{}", cut.from);
        cut.set_to(0.0);
        assert!((cut.to - 0.3).abs() < 1e-5, "{}", cut.to);
        let mut tiny = Cut::new("x".into(), 48_000, vec![[0.1; 2]; 480]);
        tiny.set_from(0.7);
        assert_eq!((tiny.from, tiny.to), (0.0, 1.0), "a sound shorter than the least is taken whole");
    }
}
