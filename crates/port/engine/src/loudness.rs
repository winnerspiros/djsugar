//! Track loudness for auto gain: EBU R128 / ITU-R BS.1770 integrated
//! loudness, measured during analysis, and the sample peak.
//!
//! The meter K-weights the signal (a high shelf for the head, then a high
//! pass), takes its power in 400 ms blocks every 100 ms, and averages the
//! blocks that pass two gates: above -70 LUFS, then above 10 LU below the
//! average of those. Silence and quiet intros so don't pull a track's
//! loudness down.

/// How loud a track is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loudness {
    /// Integrated loudness in LUFS.
    pub lufs: f32,
    /// The loudest sample, as a linear level (1 = 0 dBFS).
    pub peak: f32,
}

/// Highest auto gain boost and cut, in dB.
const MAX_BOOST_DB: f32 = 12.0;
const MAX_CUT_DB: f32 = 24.0;

impl Loudness {
    /// The gain that brings this track to `target` LUFS. Quiet tracks are
    /// only boosted until their peak reaches `ceiling` (a linear level), so
    /// auto gain alone never drives a track into the master limiter.
    pub fn gain(&self, target: f32, ceiling: f32) -> f32 {
        let db = (target - self.lufs).clamp(-MAX_CUT_DB, MAX_BOOST_DB);
        let gain = 10f32.powf(db / 20.0);
        if gain > 1.0 && self.peak > 0.0 {
            gain.min(ceiling / self.peak).max(1.0)
        } else {
            gain
        }
    }
}

/// A biquad on two channels, in f64: the K-weighting high pass sits at
/// 38 Hz, where f32 coefficients lose precision at high sample rates.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [[f64; 2]; 2],
}

impl Biquad {
    fn process(&mut self, x: f64, c: usize) -> f64 {
        let y = self.b[0] * x + self.z[c][0];
        self.z[c][0] = self.b[1] * x - self.a[0] * y + self.z[c][1];
        self.z[c][1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// The two K-weighting stages of BS.1770 for any sample rate, as libebur128
/// derives them from the 48 kHz filters.
fn k_weighting(sample_rate: f64) -> [Biquad; 2] {
    use std::f64::consts::PI;
    let shelf = {
        let (f0, g, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
        let k = (PI * f0 / sample_rate).tan();
        let vh = 10f64.powf(g / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let a0 = 1.0 + k / q + k * k;
        Biquad {
            b: [
                (vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0,
            ],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [[0.0; 2]; 2],
        }
    };
    let high_pass = {
        let (f0, q) = (38.13547087602444, 0.5003270373238773);
        let k = (PI * f0 / sample_rate).tan();
        let a0 = 1.0 + k / q + k * k;
        Biquad {
            b: [1.0, -2.0, 1.0],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [[0.0; 2]; 2],
        }
    };
    [shelf, high_pass]
}

/// Measures loudness a frame at a time.
pub struct LoudnessMeter {
    filters: [Biquad; 2],
    /// Frames in 100 ms.
    step: usize,
    /// Summed power of each finished 100 ms step, both channels.
    steps: Vec<f64>,
    current: f64,
    in_step: usize,
    peak: f32,
}

impl LoudnessMeter {
    pub fn new(sample_rate: f64) -> Self {
        let sample_rate = sample_rate.max(1.0);
        LoudnessMeter {
            filters: k_weighting(sample_rate),
            step: ((sample_rate / 10.0).round() as usize).max(1),
            steps: Vec::new(),
            current: 0.0,
            in_step: 0,
            peak: 0.0,
        }
    }

    pub fn push(&mut self, frame: [f32; 2]) {
        for (c, &x) in frame.iter().enumerate() {
            self.peak = self.peak.max(x.abs());
            let y = self.filters[0].process(x as f64, c);
            let y = self.filters[1].process(y, c);
            self.current += y * y;
        }
        self.in_step += 1;
        if self.in_step == self.step {
            self.steps.push(self.current);
            self.current = 0.0;
            self.in_step = 0;
        }
    }

    /// The loudness of everything pushed, or None if it is shorter than a
    /// block or silent.
    pub fn finish(&self) -> Option<Loudness> {
        let block_frames = (4 * self.step) as f64;
        let blocks: Vec<f64> = self
            .steps
            .windows(4)
            .map(|w| w.iter().sum::<f64>() / block_frames)
            .collect();
        Some(Loudness {
            lufs: integrated(&blocks)? as f32,
            peak: self.peak,
        })
    }
}

/// Loudness in LUFS of a K-weighted mean square summed over the channels.
fn lufs(power: f64) -> f64 {
    -0.691 + 10.0 * power.log10()
}

/// Integrated loudness of 400 ms block powers, gated as BS.1770 says:
/// blocks below -70 LUFS are dropped, then blocks more than 10 LU below
/// the average of the rest.
fn integrated(blocks: &[f64]) -> Option<f64> {
    let mean_above = |gate: f64| {
        let (sum, n) = blocks
            .iter()
            .filter(|&&p| p > 0.0 && lufs(p) > gate)
            .fold((0.0, 0usize), |(s, n), &p| (s + p, n + 1));
        (n > 0).then(|| sum / n as f64)
    };
    let ungated = mean_above(-70.0)?;
    Some(lufs(mean_above(lufs(ungated) - 10.0)?))
}

/// Loudness range in LU of short-term (3 s) powers, as EBU Tech 3342
/// measures it: the spread between the 10th and 95th percentile of the
/// short-term loudness, leaving out values below -70 LUFS and more than
/// 20 LU below the average.
fn loudness_range(short_terms: &[f64]) -> Option<f64> {
    let above: Vec<f64> = short_terms
        .iter()
        .copied()
        .filter(|&p| p > 0.0 && lufs(p) > -70.0)
        .collect();
    if above.is_empty() {
        return None;
    }
    let gate = lufs(above.iter().sum::<f64>() / above.len() as f64) - 20.0;
    let mut values: Vec<f64> = above.into_iter().map(lufs).filter(|&l| l > gate).collect();
    values.sort_by(f64::total_cmp);
    let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
    Some(at(0.95) - at(0.10))
}

/// Taps per phase of the true-peak interpolator.
const TP_TAPS: usize = 12;

/// Finds the peaks between samples, as ITU-R BS.1770 Annex 2 does: the
/// signal is upsampled four times with a windowed-sinc interpolator and the
/// highest of the original and in-between values is the true peak. Lossy
/// encoders and DACs rebuild those in-between values, so they can go over
/// full scale even when no sample does.
///
/// Each value comes out [`TruePeak::LATENCY`] frames late.
#[derive(Clone)]
pub struct TruePeak {
    /// Coefficients of the three in-between phases (1/4, 2/4, 3/4).
    phases: [[f32; TP_TAPS]; 3],
    history: [[f32; TP_TAPS]; 2],
    at: usize,
}

impl Default for TruePeak {
    fn default() -> Self {
        Self::new()
    }
}

impl TruePeak {
    /// Frames between a sample going in and its true peak coming out.
    pub const LATENCY: usize = TP_TAPS / 2;

    pub fn new() -> Self {
        let half = (TP_TAPS / 2) as f64;
        let mut phases = [[0.0f32; TP_TAPS]; 3];
        for (p, phase) in phases.iter_mut().enumerate() {
            let frac = (p + 1) as f64 / 4.0;
            let mut taps = [0.0f64; TP_TAPS];
            for (j, tap) in taps.iter_mut().enumerate() {
                // History slot j holds the sample j frames back; the value
                // wanted sits `half - frac` frames back.
                let t = j as f64 - half + frac;
                let sinc = if t.abs() < 1e-12 {
                    1.0
                } else {
                    (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t)
                };
                // A Hann window over the filter's span.
                let w = 0.5 + 0.5 * (std::f64::consts::PI * t / (half + 0.5)).cos();
                *tap = sinc * w;
            }
            let sum: f64 = taps.iter().sum();
            for (c, t) in phase.iter_mut().zip(taps) {
                *c = (t / sum) as f32;
            }
        }
        Self {
            phases,
            history: [[0.0; TP_TAPS]; 2],
            at: 0,
        }
    }

    /// Takes the next frame and returns the true peak (a linear level) of
    /// the frame [`Self::LATENCY`] back and the stretch up to the next one.
    pub fn push(&mut self, frame: [f32; 2]) -> f32 {
        self.at = (self.at + TP_TAPS - 1) % TP_TAPS;
        let mut peak = 0.0f32;
        for (c, &x) in frame.iter().enumerate() {
            let h = &mut self.history[c];
            h[self.at] = x;
            peak = peak.max(h[(self.at + Self::LATENCY) % TP_TAPS].abs());
            for phase in &self.phases {
                let mut y = 0.0f32;
                for (j, &k) in phase.iter().enumerate() {
                    y += k * h[(self.at + j) % TP_TAPS];
                }
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    pub fn clear(&mut self) {
        self.history = [[0.0; TP_TAPS]; 2];
    }
}

/// Most short-term readings the loudness range keeps: ten hours at ten a
/// second. Older ones are dropped.
const MAX_SHORT_TERMS: usize = 10 * 3600 * 10;

/// Measures the master's loudness as it plays, the way a broadcast loudness
/// meter does (EBU Tech 3341): momentary (the last 400 ms), short-term (the
/// last 3 s), integrated since the last reset, the loudness range (EBU
/// Tech 3342) and the highest true peak.
pub struct LiveLoudness {
    sample_rate: f64,
    filters: [Biquad; 2],
    step: usize,
    current: f64,
    in_step: usize,
    /// Summed power of the last 30 steps (3 s), newest last.
    recent: std::collections::VecDeque<f64>,
    /// Power of every 400 ms block since the reset, one per 100 ms.
    blocks: Vec<f64>,
    /// Power of every 3 s window since the reset, one per 100 ms.
    short_terms: Vec<f64>,
    true_peak: TruePeak,
    max_peak: f32,
}

impl LiveLoudness {
    pub fn new(sample_rate: f64) -> Self {
        let sample_rate = sample_rate.max(1.0);
        Self {
            sample_rate,
            filters: k_weighting(sample_rate),
            step: ((sample_rate / 10.0).round() as usize).max(1),
            current: 0.0,
            in_step: 0,
            recent: std::collections::VecDeque::with_capacity(31),
            blocks: Vec::new(),
            short_terms: Vec::new(),
            true_peak: TruePeak::new(),
            max_peak: 0.0,
        }
    }

    pub fn push(&mut self, frame: [f32; 2]) {
        self.max_peak = self.max_peak.max(self.true_peak.push(frame));
        for (c, &x) in frame.iter().enumerate() {
            let y = self.filters[0].process(x as f64, c);
            let y = self.filters[1].process(y, c);
            self.current += y * y;
        }
        self.in_step += 1;
        if self.in_step < self.step {
            return;
        }
        if self.recent.len() == 30 {
            self.recent.pop_front();
        }
        self.recent.push_back(self.current);
        self.current = 0.0;
        self.in_step = 0;
        if let Some(p) = self.window(4) {
            self.blocks.push(p);
        }
        if let Some(p) = self.window(30) {
            if self.short_terms.len() == MAX_SHORT_TERMS {
                self.short_terms.drain(..MAX_SHORT_TERMS / 10);
            }
            self.short_terms.push(p);
        }
    }

    /// Mean power of the last `steps` steps, once there are that many.
    fn window(&self, steps: usize) -> Option<f64> {
        (self.recent.len() >= steps)
            .then(|| self.recent.iter().rev().take(steps).sum::<f64>() / (steps * self.step) as f64)
    }

    /// Starts integrated loudness, loudness range and the highest true peak
    /// again. Momentary and short-term carry on.
    pub fn reset(&mut self) {
        self.blocks.clear();
        self.short_terms.clear();
        self.max_peak = 0.0;
    }

    /// Loudness of the last 400 ms, in LUFS.
    pub fn momentary(&self) -> Option<f64> {
        self.window(4).filter(|&p| p > 0.0).map(lufs)
    }

    /// Loudness of the last 3 s, in LUFS.
    pub fn short_term(&self) -> Option<f64> {
        self.window(30).filter(|&p| p > 0.0).map(lufs)
    }

    /// Gated loudness since the reset, in LUFS.
    pub fn integrated(&self) -> Option<f64> {
        integrated(&self.blocks)
    }

    /// Loudness range since the reset, in LU.
    pub fn range(&self) -> Option<f64> {
        loudness_range(&self.short_terms)
    }

    /// The highest true peak since the reset, in dBTP.
    pub fn true_peak(&self) -> Option<f64> {
        (self.max_peak > 0.0).then(|| 20.0 * (self.max_peak as f64).log10())
    }

    /// Seconds measured since the reset (by the 100 ms steps).
    pub fn seconds(&self) -> f64 {
        (self.blocks.len() as f64 + 3.0) * self.step as f64 / self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(sample_rate: f64, seconds: f64, f: impl Fn(f64) -> [f32; 2]) -> Option<Loudness> {
        let mut m = LoudnessMeter::new(sample_rate);
        for i in 0..(seconds * sample_rate) as usize {
            m.push(f(i as f64 / sample_rate));
        }
        m.finish()
    }

    #[test]
    fn full_scale_1k_sine_reads_minus_3_lufs() {
        // EBU Tech 3341: a 0 dBFS 997 Hz sine in one channel is -3.01 LUFS.
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            let l = measure(sr, 5.0, |t| {
                let v = (std::f64::consts::TAU * 997.0 * t).sin() as f32;
                [v, 0.0]
            })
            .unwrap();
            assert!((l.lufs + 3.01).abs() < 0.05, "{sr}: {}", l.lufs);
            assert!((l.peak - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn gates_out_silence() {
        // Tech 3341 case 3 idea: loud passages with silence between read as
        // the loud passages alone.
        let tone = |t: f64| {
            let v = 0.1 * (std::f64::consts::TAU * 1000.0 * t).sin() as f32;
            [v, v]
        };
        let steady = measure(48_000.0, 10.0, tone).unwrap();
        let gapped = measure(
            48_000.0,
            30.0,
            |t| if t < 10.0 { tone(t) } else { [0.0; 2] },
        )
        .unwrap();
        assert!((steady.lufs - gapped.lufs).abs() < 0.1);
        assert!(measure(48_000.0, 2.0, |_| [0.0; 2]).is_none());
    }

    fn live(sample_rate: f64, seconds: f64, f: impl Fn(f64) -> [f32; 2]) -> LiveLoudness {
        let mut m = LiveLoudness::new(sample_rate);
        for i in 0..(seconds * sample_rate) as usize {
            m.push(f(i as f64 / sample_rate));
        }
        m
    }

    fn sine(db: f64, freq: f64) -> impl Fn(f64) -> [f32; 2] {
        let amp = 10f64.powf(db / 20.0);
        move |t| {
            let v = (amp * (std::f64::consts::TAU * freq * t).sin()) as f32;
            [v, v]
        }
    }

    #[test]
    fn live_meter_reads_the_tech_3341_tones() {
        // Tech 3341 case 1: a 1 kHz sine at -23 dBFS on both channels is
        // -23 LUFS momentary, short-term and integrated.
        for sr in [44_100.0, 48_000.0] {
            let m = live(sr, 20.0, sine(-23.0, 1000.0));
            for (name, v) in [
                ("momentary", m.momentary()),
                ("short-term", m.short_term()),
                ("integrated", m.integrated()),
            ] {
                let v = v.unwrap();
                assert!((v + 23.0).abs() < 0.1, "{sr} {name}: {v}");
            }
        }
        // Case 2: -33 dBFS is -33 LUFS.
        let m = live(48_000.0, 20.0, sine(-33.0, 1000.0));
        assert!((m.integrated().unwrap() + 33.0).abs() < 0.1);
    }

    #[test]
    fn live_meter_gates_like_tech_3341_case_5() {
        // 20 s at -26, 20.1 s at -20, 20 s at -26 dBFS is -23 LUFS
        // integrated: the quieter parts count, gated against the louder.
        let m = live(48_000.0, 60.1, |t| {
            let db = if (20.0..40.1).contains(&t) {
                -20.0
            } else {
                -26.0
            };
            sine(db, 1000.0)(t)
        });
        let i = m.integrated().unwrap();
        assert!((i + 23.0).abs() < 0.1, "{i}");
    }

    #[test]
    fn loudness_range_follows_tech_3342() {
        // Case 1: 20 s at -20 dBFS then 20 s at -30 dBFS is 10 LU.
        let m = live(48_000.0, 40.0, |t| {
            sine(if t < 20.0 { -20.0 } else { -30.0 }, 1000.0)(t)
        });
        let lra = m.range().unwrap();
        assert!((lra - 10.0).abs() < 1.0, "{lra}");
        // Case 2: -20 then -15 dBFS is 5 LU.
        let m = live(48_000.0, 40.0, |t| {
            sine(if t < 20.0 { -20.0 } else { -15.0 }, 1000.0)(t)
        });
        let lra = m.range().unwrap();
        assert!((lra - 5.0).abs() < 1.0, "{lra}");
    }

    #[test]
    fn true_peak_sees_peaks_between_samples() {
        // A sine at a quarter of the sample rate, sampled 45 degrees off its
        // crests: every sample is at 0.707 but the wave reaches 1.0.
        let sr = 48_000.0;
        let mut tp = TruePeak::new();
        let mut sample_peak = 0.0f32;
        let mut true_peak = 0.0f32;
        for i in 0..4800 {
            let v = (std::f64::consts::TAU * 12_000.0 * i as f64 / sr + std::f64::consts::FRAC_PI_4)
                .sin() as f32;
            sample_peak = sample_peak.max(v.abs());
            true_peak = true_peak.max(tp.push([v, v]));
        }
        assert!((sample_peak - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
        assert!(true_peak > 0.97 && true_peak < 1.03, "{true_peak}");
        // A low tone's true peak is its sample peak.
        let mut tp = TruePeak::new();
        let mut peak = 0.0f32;
        for i in 0..4800 {
            let v = 0.5 * (std::f64::consts::TAU * 100.0 * i as f64 / sr).sin() as f32;
            peak = peak.max(tp.push([v, v]));
        }
        assert!((peak - 0.5).abs() < 0.005, "{peak}");
    }

    #[test]
    fn live_meter_resets_integrated_but_not_short_term() {
        let mut m = live(48_000.0, 10.0, sine(-20.0, 1000.0));
        assert!(m.true_peak().unwrap() > -20.5);
        m.reset();
        assert!(m.integrated().is_none());
        assert!(m.true_peak().is_none());
        assert!((m.short_term().unwrap() + 20.0).abs() < 0.2);
    }

    #[test]
    fn gain_reaches_target_but_caps_boost_at_peak() {
        let loud = Loudness {
            lufs: -8.0,
            peak: 1.0,
        };
        assert!((20.0 * loud.gain(-14.0, 0.97).log10() + 6.0).abs() < 1e-3);
        // Needs +10 dB, but its peak allows only +6 dB.
        let quiet = Loudness {
            lufs: -24.0,
            peak: 0.5,
        };
        assert!((quiet.gain(-14.0, 1.0) - 2.0).abs() < 1e-4);
        // Never cuts a quiet track because of its peak.
        let peaky = Loudness {
            lufs: -16.0,
            peak: 1.0,
        };
        assert_eq!(peaky.gain(-14.0, 0.97), 1.0);
    }
}
