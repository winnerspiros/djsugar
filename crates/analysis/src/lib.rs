const HOP: usize = 512;
const MIN_BPM: f64 = 70.0;
const MAX_BPM: f64 = 180.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub bpm: f64,
    pub first_beat: f64,
}

impl Grid {
    pub fn beat_frames(&self, rate: u32) -> f64 {
        60.0 * f64::from(rate) / self.bpm
    }

    pub fn snap(&self, rate: u32, frame: f64) -> f64 {
        let beat = self.beat_frames(rate);
        self.first_beat + ((frame - self.first_beat) / beat).round() * beat
    }
}

fn flux(frames: &[[f32; 2]]) -> Vec<f32> {
    let mut low = 0.0f32;
    let mut prev = 0.0f32;
    frames
        .chunks(HOP)
        .map(|c| {
            let mut e = 0.0f32;
            for s in c {
                let m = (s[0] + s[1]) * 0.5;
                low += 0.15 * (m - low);
                e += low * low + 0.25 * (m - low) * (m - low);
            }
            let e = (e / c.len() as f32).sqrt();
            let d = (e - prev).max(0.0);
            prev = e;
            d
        })
        .collect()
}

pub fn grid(rate: u32, frames: &[[f32; 2]]) -> Option<Grid> {
    let env = flux(frames);
    let hop_s = HOP as f64 / f64::from(rate);
    let lag_min = (60.0 / MAX_BPM / hop_s).floor() as usize;
    let lag_max = (60.0 / MIN_BPM / hop_s).ceil() as usize;
    if env.len() < lag_max * 4 {
        return None;
    }
    let mean = env.iter().sum::<f32>() / env.len() as f32;
    let env: Vec<f32> = env.iter().map(|v| v - mean).collect();
    let score = |lag: f64| -> f32 {
        let mut s = 0.0;
        for k in 1..=4 {
            let l = (lag * f64::from(k)).round() as usize;
            if l >= env.len() {
                break;
            }
            s += env[..env.len() - l].iter().zip(&env[l..]).map(|(a, b)| a * b).sum::<f32>() / k as f32;
        }
        s
    };
    let mut best = (0.0, f32::MIN);
    let mut lag = lag_min as f64;
    while lag <= lag_max as f64 {
        let s = score(lag);
        if s > best.1 {
            best = (lag, s);
        }
        lag += 0.05;
    }
    if best.1 <= 0.0 {
        return None;
    }
    let bpm = (60.0 / (best.0 * hop_s) * 10.0).round() / 10.0;
    let period = 60.0 / bpm / hop_s;
    let mut phase = (0, f32::MIN);
    for off in 0..period.ceil() as usize {
        let mut s = 0.0;
        let mut t = off as f64;
        while (t as usize) < env.len() {
            s += env[t as usize];
            t += period;
        }
        if s > phase.1 {
            phase = (off, s);
        }
    }
    Some(Grid { bpm, first_beat: (phase.0 * HOP) as f64 })
}

const MAJOR: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];
const NOTES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const BLOCK: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub tonic: u8,
    pub minor: bool,
}

impl Key {
    pub fn name(self) -> String {
        format!("{}{}", NOTES[usize::from(self.tonic)], if self.minor { "m" } else { "" })
    }

    pub fn camelot_number(self) -> u8 {
        let t = i32::from(self.tonic);
        ((t * 7 + if self.minor { 4 } else { 7 }).rem_euclid(12) + 1) as u8
    }

    pub fn camelot_order(self) -> u8 {
        self.camelot_number() * 2 + u8::from(!self.minor)
    }

    pub fn camelot(self) -> String {
        format!("{}{}", self.camelot_number(), if self.minor { 'A' } else { 'B' })
    }
}

fn goertzel(x: &[f32], w: f32) -> f32 {
    let c = 2.0 * w.cos();
    let (mut a, mut b) = (0.0f32, 0.0f32);
    for &v in x {
        let n = v + c * a - b;
        b = a;
        a = n;
    }
    a * a + b * b - c * a * b
}

pub fn key(rate: u32, frames: &[[f32; 2]]) -> Option<Key> {
    let mut chroma = [0.0f32; 12];
    let mut mono = vec![0.0f32; BLOCK];
    let blocks = frames.len() / BLOCK;
    let stride = (blocks / 240).max(1);
    for b in (0..blocks).step_by(stride) {
        for (m, f) in mono.iter_mut().zip(&frames[b * BLOCK..(b + 1) * BLOCK]) {
            *m = f[0] + f[1];
        }
        let mut frame = [0.0f32; 12];
        for midi in 43..91 {
            let hz = 440.0 * ((midi as f32 - 69.0) / 12.0).exp2();
            let e = goertzel(&mono, std::f32::consts::TAU * hz / rate as f32);
            frame[midi % 12] += e.sqrt();
        }
        let max = frame.iter().fold(0.0f32, |a, &b| a.max(b));
        if max > 1e-3 {
            for (c, f) in chroma.iter_mut().zip(frame) {
                *c += f / max;
            }
        }
    }
    if chroma.iter().all(|&c| c == 0.0) {
        return None;
    }
    let corr = |profile: &[f32; 12], tonic: usize| -> f32 {
        let mp = profile.iter().sum::<f32>() / 12.0;
        let mc = chroma.iter().sum::<f32>() / 12.0;
        let (mut num, mut dp, mut dc) = (0.0, 0.0, 0.0);
        for i in 0..12 {
            let p = profile[(i + 12 - tonic) % 12] - mp;
            let c = chroma[i] - mc;
            num += p * c;
            dp += p * p;
            dc += c * c;
        }
        num / (dp * dc).sqrt().max(1e-9)
    };
    let mut best = (Key { tonic: 0, minor: false }, f32::MIN);
    for t in 0..12 {
        for (minor, profile) in [(false, &MAJOR), (true, &MINOR)] {
            let r = corr(profile, t);
            if r > best.1 {
                best = (Key { tonic: t as u8, minor }, r);
            }
        }
    }
    Some(best.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clicks(rate: u32, bpm: f64, offset: f64, secs: f64) -> Vec<[f32; 2]> {
        let n = (f64::from(rate) * secs) as usize;
        let beat = 60.0 / bpm * f64::from(rate);
        let mut v = vec![[0.0; 2]; n];
        let mut t = offset;
        while (t as usize) < n {
            for i in 0..2000.min(n - t as usize) {
                let e = (-(i as f32) / 300.0).exp() * (i as f32 * 0.02).sin();
                v[t as usize + i] = [e, e];
            }
            t += beat;
        }
        v
    }

    #[test]
    fn finds_tempo_across_the_range() {
        for bpm in [84.0, 100.0, 124.0, 128.0, 140.0, 174.0] {
            let g = grid(44100, &clicks(44100, bpm, 3000.0, 30.0)).unwrap();
            assert!((g.bpm - bpm).abs() < 0.6, "{bpm} -> {}", g.bpm);
            let beat = g.beat_frames(44100);
            let err = (g.first_beat - 3000.0).rem_euclid(beat);
            assert!(err.min(beat - err) < 1100.0, "{bpm}: phase {}", g.first_beat);
        }
    }

    fn chords(rate: u32, progression: &[[i32; 3]], secs: f64) -> Vec<[f32; 2]> {
        let n = (f64::from(rate) * secs) as usize;
        let per = n / progression.len();
        (0..n)
            .map(|i| {
                let chord = progression[(i / per).min(progression.len() - 1)];
                let t = i as f32 / rate as f32;
                let v: f32 = chord
                    .iter()
                    .map(|&m| (std::f32::consts::TAU * 440.0 * ((m as f32 - 69.0) / 12.0).exp2() * t).sin() * 0.2)
                    .sum();
                [v, v]
            })
            .collect()
    }

    #[test]
    fn finds_major_and_minor_keys() {
        let c_major = [[60, 64, 67], [65, 69, 72], [67, 71, 74], [60, 64, 67]];
        let k = key(44100, &chords(44100, &c_major, 8.0)).unwrap();
        assert_eq!((k.name(), k.camelot()), ("C".into(), "8B".into()));
        let a_minor = [[57, 60, 64], [62, 65, 69], [64, 68, 71], [57, 60, 64]];
        let k = key(44100, &chords(44100, &a_minor, 8.0)).unwrap();
        assert_eq!((k.name(), k.camelot()), ("Am".into(), "8A".into()));
        let fs_minor = [[54, 57, 61], [59, 62, 66], [61, 65, 68], [54, 57, 61]];
        let k = key(44100, &chords(44100, &fs_minor, 8.0)).unwrap();
        assert_eq!(k.camelot(), "11A");
    }

    #[test]
    fn silence_has_no_grid() {
        assert!(grid(44100, &vec![[0.0; 2]; 44100 * 10]).is_none());
    }
}
