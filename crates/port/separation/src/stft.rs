//! The short-time Fourier transform Spleeter was trained with: a periodic Hann
//! window of 4096 samples, hop 1024, frames starting at sample 0 and the end
//! padded with zeros (TensorFlow's `stft` with `pad_end`).

use std::sync::Arc;

use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

pub const FRAME: usize = 4096;
pub const HOP: usize = 1024;
pub const BINS: usize = FRAME / 2 + 1;

/// Overlap-adding the window twice (analysis and synthesis) at a quarter hop
/// sums to 1.5; Spleeter scales by 2/3 to undo it.
const WINDOW_GAIN: f32 = 2.0 / 3.0;

pub struct Stft {
    window: Vec<f32>,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
}

impl Default for Stft {
    fn default() -> Self {
        Self::new()
    }
}

impl Stft {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::new();
        Stft {
            window: (0..FRAME)
                .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FRAME as f32).cos())
                .collect(),
            forward: planner.plan_fft_forward(FRAME),
            inverse: planner.plan_fft_inverse(FRAME),
        }
    }

    /// Frames needed to cover `len` samples.
    pub fn frames(len: usize) -> usize {
        len.div_ceil(HOP)
    }

    /// Frame `k` of `x`: `BINS` complex values.
    pub fn frame(&self, x: &[f32], k: usize, out: &mut [Complex32]) {
        let start = k * HOP;
        let mut buf: Vec<f32> = (0..FRAME)
            .map(|i| x.get(start + i).copied().unwrap_or(0.0) * self.window[i])
            .collect();
        self.forward
            .process(&mut buf, out)
            .expect("FFT sizes are fixed");
    }

    /// The whole spectrogram of `x`, frame-major: `frames(len) * BINS` values.
    pub fn analyse(&self, x: &[f32]) -> Vec<Complex32> {
        let frames = Self::frames(x.len());
        let mut out = vec![Complex32::default(); frames * BINS];
        for (k, f) in out.chunks_mut(BINS).enumerate() {
            self.frame(x, k, f);
        }
        out
    }

    /// Overlap-adds frames `first..first + n` of `spec` (frame-major, `BINS`
    /// per frame) into `out`, where `out[0]` is sample 0 of the signal.
    /// Adding every frame once gives the signal back.
    pub fn synthesise(&self, spec: &[Complex32], first: usize, out: &mut [f32]) {
        let mut buf = vec![Complex32::default(); BINS];
        let mut time = vec![0.0f32; FRAME];
        for (j, f) in spec.chunks(BINS).enumerate() {
            buf.copy_from_slice(f);
            // A real signal's DC and Nyquist bins are real.
            buf[0].im = 0.0;
            buf[BINS - 1].im = 0.0;
            self.inverse
                .process(&mut buf, &mut time)
                .expect("FFT sizes are fixed");
            let start = (first + j) * HOP;
            for (i, t) in time.iter().enumerate() {
                if let Some(o) = out.get_mut(start + i) {
                    *o += t * self.window[i] * WINDOW_GAIN / FRAME as f32;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_then_synthesis_gives_the_signal_back() {
        let x: Vec<f32> = (0..50_000)
            .map(|i| (i as f32 * 0.013).sin() * 0.5 + ((i * 7919) % 101) as f32 / 404.0)
            .collect();
        let stft = Stft::new();
        let spec = stft.analyse(&x);
        let mut y = vec![0.0; x.len()];
        stft.synthesise(&spec, 0, &mut y);
        // The first and last 3 hops lack overlapping frames; Spleeter pads
        // the front with a frame of zeros for that reason.
        let worst = x[FRAME..x.len() - FRAME]
            .iter()
            .zip(&y[FRAME..x.len() - FRAME])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(worst < 1e-4, "worst error {worst}");
    }
}
