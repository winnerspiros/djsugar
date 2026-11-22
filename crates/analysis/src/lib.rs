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

    #[test]
    fn silence_has_no_grid() {
        assert!(grid(44100, &vec![[0.0; 2]; 44100 * 10]).is_none());
    }
}
