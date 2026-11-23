//! Measures how long sound takes from the speakers back into an input (output latency + air + input latency), so what a looper or a recording takes from a microphone
//! lines up with the beat the DJ played to. A probe of white noise plays on the main output; what the input heard is cross-correlated with it, and the strongest peak
//! is the delay. Noise has one sharp correlation peak, so echoes and a quiet room still give one answer, and a weak peak means the input never heard the probe.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

/// Seconds of noise the probe plays.
pub const PROBE_SECONDS: f64 = 0.25;
/// Seconds of input listened to after the probe starts: delays up to this
/// less the probe are found.
pub const LISTEN_SECONDS: f64 = 1.0;
/// Probe level: well below full scale, loud enough for a laptop microphone.
const LEVEL: f32 = 0.25;
/// How far above the correlation's typical size the peak must stand.
const CLEAR: f32 = 8.0;

/// The probe: the same noise every time, for `rate` frames per second.
pub fn probe(rate: u32) -> Vec<f32> {
    let mut seed = 0x9e37_79b9u32;
    (0..(PROBE_SECONDS * f64::from(rate)) as usize)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * LEVEL
        })
        .collect()
}

/// The delay in frames from `sent` to where it shows up in `heard`, or
/// `None` if `heard` doesn't clearly hold it.
pub fn delay(sent: &[f32], heard: &[f32]) -> Option<usize> {
    if sent.is_empty() || heard.len() < sent.len() {
        return None;
    }
    let n = (heard.len() + sent.len()).next_power_of_two();
    let mut planner = FftPlanner::<f32>::new();
    let (fwd, inv) = (planner.plan_fft_forward(n), planner.plan_fft_inverse(n));
    let spectrum = |x: &[f32]| {
        let mut v: Vec<Complex32> = x.iter().map(|&s| Complex32::new(s, 0.0)).collect();
        v.resize(n, Complex32::default());
        fwd.process(&mut v);
        v
    };
    let (s, mut h) = (spectrum(sent), spectrum(heard));
    for (h, s) in h.iter_mut().zip(&s) {
        *h *= s.conj();
    }
    inv.process(&mut h);
    let lags = &h[..=heard.len() - sent.len()];
    let (at, peak) = lags.iter().map(|c| c.re.abs()).enumerate().max_by(|a, b| a.1.total_cmp(&b.1))?;
    let typical = lags.iter().map(|c| c.re.abs()).sum::<f32>() / lags.len() as f32;
    (peak > typical * CLEAR).then_some(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// What a microphone hears: the probe `late` frames in, at `gain`, with
    /// an echo and room noise.
    fn room(late: usize, gain: f32, noise: f32) -> Vec<f32> {
        let p = probe(RATE);
        let mut heard = vec![0.0; (LISTEN_SECONDS * f64::from(RATE)) as usize];
        for (i, s) in p.iter().enumerate() {
            heard[late + i] += s * gain;
            heard[late + 900 + i] += s * gain * 0.4;
        }
        let mut seed = 7u32;
        for h in &mut heard {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *h += (seed as f32 / u32::MAX as f32 - 0.5) * noise;
        }
        heard
    }

    #[test]
    fn finds_the_delay_through_echo_and_noise() {
        let p = probe(RATE);
        for late in [0, 64, 1_234, 9_600, 30_000] {
            assert_eq!(delay(&p, &room(late, 0.05, 0.02)), Some(late));
        }
    }

    #[test]
    fn a_deaf_input_gives_no_answer() {
        assert_eq!(delay(&probe(RATE), &room(1000, 0.0, 0.02)), None);
        assert_eq!(delay(&probe(RATE), &[0.0; 100]), None);
    }
}
