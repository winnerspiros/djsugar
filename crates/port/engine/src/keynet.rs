//! Neural key detection: KeyNet, the convolutional network of Korzeniowski
//! and Widmer ("Genre-Agnostic Key Classification with Convolutional Neural
//! Networks", ISMIR 2018), with the weights openkeyscan-analyzer trained on
//! the GiantSteps MTG key set (MIT licence; see `models/README.md`).
//!
//! The network reads a constant-Q spectrogram as librosa makes it: 105 bins,
//! two per semitone from 65 Hz, five frames a second, as log(1 + magnitude).
//! The filters here are librosa's, applied at the file's own rate in a single
//! pass instead of octave by octave; that comes within about half a percent
//! of librosa's values and gives the same keys.

use std::sync::{Arc, OnceLock};

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::key::Key;

/// Constant-Q bins per frame.
pub const BINS: usize = 105;
const BINS_PER_OCTAVE: f64 = 24.0;
const FMIN: f64 = 65.0;
/// Seconds between frames: librosa's hop of 8820 samples at 44.1 kHz.
const FRAME_SECONDS: f64 = 0.2;
/// The network halves the spectrogram's length three times.
const MIN_FRAMES: usize = 8;

/// The weights, as `scripts/export-keynet.py` writes them.
static WEIGHTS: &[u8] = include_bytes!("../models/keynet.bin");

/// One constant-Q filter in the frequency domain: the FFT bins it covers
/// from `first`, already scaled as librosa scales its output.
struct Filter {
    first: usize,
    taps: Vec<Complex32>,
}

/// Builds the constant-Q spectrogram as audio arrives.
pub struct Spectrogram {
    hop: usize,
    n_fft: usize,
    fft: Arc<dyn Fft<f32>>,
    filters: Vec<Filter>,
    /// Samples from absolute index `start` on; it begins with the half frame
    /// of silence librosa pads with.
    buf: Vec<f32>,
    start: isize,
    pushed: usize,
    frames: Vec<[f32; BINS]>,
    scratch: Vec<Complex32>,
}

impl Spectrogram {
    pub fn new(sample_rate: f64) -> Self {
        let sr = sample_rate.max(1.0);
        // librosa's relative bandwidth for 24 bins an octave, and the
        // filter length that gives each bin.
        let r = 2f64.powf(2.0 / BINS_PER_OCTAVE);
        let q = (r + 1.0) / (r - 1.0);
        let freq = |k: usize| FMIN * 2f64.powf(k as f64 / BINS_PER_OCTAVE);
        let length = |k: usize| q * sr / freq(k);
        let n_fft = (length(0).ceil() as usize).next_power_of_two();
        let fft = FftPlanner::new().plan_fft_forward(n_fft);
        let filters = (0..BINS)
            .map(|k| {
                let (f, len) = (freq(k), length(k));
                // Taps at whole samples from floor(-len/2) up to floor(len/2),
                // Hann windowed, scaled to sum to one, centred in n_fft.
                let lo = (-len / 2.0).floor();
                let n = ((len / 2.0).floor() - lo) as usize;
                let window =
                    |j: usize| 0.5 - 0.5 * (std::f64::consts::TAU * j as f64 / n as f64).cos();
                let sum: f64 = (0..n).map(window).sum();
                // librosa scales by len / n_fft before its FFT and divides
                // the result by sqrt(len); the network saw audio at 44.1 kHz,
                // where len is longer by 44100 / sr.
                let gain = len / n_fft as f64 / sum / len.sqrt() * (44_100.0 / sr).sqrt();
                let offset = (n_fft - n) / 2;
                let mut taps = vec![Complex32::new(0.0, 0.0); n_fft];
                for (j, tap) in taps[offset..offset + n].iter_mut().enumerate() {
                    let phase = std::f64::consts::TAU * f * (lo + j as f64) / sr;
                    let w = window(j) * gain;
                    *tap = Complex32::new((w * phase.cos()) as f32, (w * phase.sin()) as f32);
                }
                fft.process(&mut taps);
                taps.truncate(n_fft / 2 + 1);
                // Only the bins around the filter's frequency matter.
                let top = taps.iter().map(|t| t.norm()).fold(0.0, f32::max);
                let keep = |t: &Complex32| t.norm() > top * 1e-4;
                let first = taps.iter().position(keep).unwrap_or(0);
                let last = taps.iter().rposition(keep).unwrap_or(0);
                Filter {
                    first,
                    taps: taps[first..=last].to_vec(),
                }
            })
            .collect();
        Spectrogram {
            hop: (sr * FRAME_SECONDS).round() as usize,
            n_fft,
            fft,
            filters,
            buf: vec![0.0; n_fft / 2],
            start: -((n_fft / 2) as isize),
            pushed: 0,
            frames: Vec::new(),
            scratch: Vec::new(),
        }
    }

    /// Takes the next mono sample.
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.buf.push(x);
        self.pushed += 1;
        if self.pushed >= self.frames.len() * self.hop + self.n_fft / 2 {
            self.frame();
        }
    }

    /// Frame `frames.len()`, centred on its hop; the buffer holds all of it.
    fn frame(&mut self) {
        let from = (self.frames.len() * self.hop) as isize - (self.n_fft / 2) as isize;
        let at = (from - self.start) as usize;
        self.scratch.clear();
        self.scratch.extend(
            self.buf[at..at + self.n_fft]
                .iter()
                .map(|&x| Complex32::new(x, 0.0)),
        );
        self.fft.process(&mut self.scratch);
        let mut out = [0.0f32; BINS];
        for (o, f) in out.iter_mut().zip(&self.filters) {
            let spectrum = &self.scratch[f.first..f.first + f.taps.len()];
            let sum: Complex32 = f.taps.iter().zip(spectrum).map(|(a, b)| a * b).sum();
            *o = sum.norm().ln_1p();
        }
        self.frames.push(out);
        // Keep what the next frame needs.
        let next = (self.frames.len() * self.hop) as isize - (self.n_fft / 2) as isize;
        let drop = ((next - self.start).max(0) as usize).min(self.buf.len());
        self.buf.drain(..drop);
        self.start += drop as isize;
    }

    /// The frames of everything pushed, with the trailing half frame of
    /// silence librosa pads with, less the last two frames, which
    /// openkeyscan leaves out.
    fn finish(mut self) -> Vec<[f32; BINS]> {
        let total = self.pushed / self.hop + 1;
        while self.frames.len() < total {
            self.buf.push(0.0);
            if self.start + self.buf.len() as isize
                >= (self.frames.len() * self.hop + self.n_fft / 2) as isize
            {
                self.frame();
            }
        }
        self.frames.truncate(total.saturating_sub(2));
        self.frames
    }

    /// The key of everything pushed, or None for very short audio.
    pub fn key(self) -> Option<Key> {
        self.scores().map(|s| class_key(best(&s)))
    }

    /// The network's score for each class (see [`class_key`]), or None for
    /// very short audio.
    pub fn scores(self) -> Option<[f32; 24]> {
        let frames = self.finish();
        (frames.len() >= MIN_FRAMES).then(|| network().scores(&frames))
    }

    /// The spectrogram the network reads, frame by frame.
    pub fn frames(self) -> Vec<[f32; BINS]> {
        self.finish()
    }
}

/// A convolution with its batch norm folded in, followed by an ELU.
struct Conv {
    out: usize,
    inp: usize,
    k: usize,
    bias: Vec<f32>,
    /// `out` rows of `inp * k * k`.
    weights: Vec<f32>,
}

struct Network {
    layers: Vec<Conv>,
}

fn network() -> &'static Network {
    static NET: OnceLock<Network> = OnceLock::new();
    NET.get_or_init(|| Network::read(WEIGHTS).expect("the bundled key network is valid"))
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(head)
    }

    fn u32(&mut self) -> Option<usize> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize)
    }
}

/// A stack of `c` planes of `h` rows (frequency) by `w` columns (time).
struct Planes {
    c: usize,
    h: usize,
    w: usize,
    data: Vec<f32>,
}

impl Network {
    fn read(data: &[u8]) -> Option<Network> {
        let mut r = Reader(data.strip_prefix(b"OKSKEY01")?);
        let count = r.u32()?;
        let mut layers = Vec::with_capacity(count);
        for _ in 0..count {
            let (out, inp, kh, kw) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
            if kh != kw || kh % 2 == 0 {
                return None;
            }
            let bias = r
                .take(out * 4)?
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            let weights = r
                .take(out * inp * kh * kw * 2)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| f16_to_f32(u16::from_le_bytes(*b)))
                .collect();
            layers.push(Conv {
                out,
                inp,
                k: kh,
                bias,
                weights,
            });
        }
        r.0.is_empty().then_some(Network { layers })
    }

    /// The network's 24 scores for a spectrogram.
    fn scores(&self, frames: &[[f32; BINS]]) -> [f32; 24] {
        let (h, w) = (BINS, frames.len());
        let mut data = vec![0.0; h * w];
        for (t, f) in frames.iter().enumerate() {
            for (b, v) in f.iter().enumerate() {
                data[b * w + t] = *v;
            }
        }
        let mut x = Planes { c: 1, h, w, data };
        for (i, layer) in self.layers.iter().enumerate() {
            x = layer.apply(&x);
            // Pooling follows the second, fourth and sixth convolutions.
            if matches!(i, 1 | 3 | 5) {
                x = max_pool(&x);
            }
        }
        let n = (x.h * x.w).max(1) as f32;
        std::array::from_fn(|c| {
            x.data[c * x.h * x.w..(c + 1) * x.h * x.w]
                .iter()
                .sum::<f32>()
                / n
        })
    }
}

fn best(scores: &[f32; 24]) -> usize {
    (0..24).fold(0, |b, i| if scores[i] > scores[b] { i } else { b })
}

/// openkeyscan's classes follow the Camelot wheel: 0-11 are 1A-12A, 12-23
/// are 1B-12B.
pub fn class_key(class: usize) -> Key {
    let (number, letter) = (class % 12 + 1, if class < 12 { 'A' } else { 'B' });
    Key::parse(&format!("{number}{letter}")).expect("Camelot keys parse")
}

impl Conv {
    /// Zero-padded ("same") convolution, bias and ELU. Each output row is a
    /// matrix product of the weights with that row's input patches; the
    /// rows are shared out between threads.
    fn apply(&self, x: &Planes) -> Planes {
        assert_eq!(x.c, self.inp);
        let (h, w, k, pad) = (x.h, x.w, self.k, self.k / 2);
        let depth = self.inp * k * k;
        let mut out = vec![0.0f32; self.out * h * w];
        let threads = crate::background::threads(
            std::thread::available_parallelism()
                .map_or(1, |n| n.get() / 2)
                .clamp(1, 4),
        )
        .min(h);
        // Background analysis keeps its helpers at its own low priority.
        let low = crate::background::is_low();
        let target = SendPtr(out.as_mut_ptr());
        std::thread::scope(|scope| {
            for part in 0..threads {
                let target = &target;
                scope.spawn(move || {
                    if low {
                        crate::background::lower_priority();
                    }
                    let mut patches = vec![0.0f32; depth * w];
                    for row in (part..h).step_by(threads) {
                        for ci in 0..self.inp {
                            let plane = &x.data[ci * h * w..(ci + 1) * h * w];
                            for dy in 0..k {
                                let src = row as isize + dy as isize - pad as isize;
                                for dx in 0..k {
                                    let r = (ci * k + dy) * k + dx;
                                    let dst = &mut patches[r * w..(r + 1) * w];
                                    if !(0..h as isize).contains(&src) {
                                        dst.fill(0.0);
                                        continue;
                                    }
                                    let line = &plane[src as usize * w..(src as usize + 1) * w];
                                    let shift = dx as isize - pad as isize;
                                    for (t, d) in dst.iter_mut().enumerate() {
                                        let s = t as isize + shift;
                                        *d = if (0..w as isize).contains(&s) {
                                            line[s as usize]
                                        } else {
                                            0.0
                                        };
                                    }
                                }
                            }
                        }
                        // SAFETY: each thread writes only its own rows (`row`)
                        // of every output plane, so the writes never overlap,
                        // and `out` outlives the scope.
                        unsafe {
                            matrixmultiply::sgemm(
                                self.out,
                                depth,
                                w,
                                1.0,
                                self.weights.as_ptr(),
                                depth as isize,
                                1,
                                patches.as_ptr(),
                                w as isize,
                                1,
                                0.0,
                                target.0.add(row * w),
                                (h * w) as isize,
                                1,
                            );
                        }
                    }
                });
            }
        });
        for (o, plane) in out.chunks_exact_mut(h * w).enumerate() {
            let b = self.bias[o];
            for v in plane {
                let y = *v + b;
                *v = if y > 0.0 { y } else { y.exp_m1() };
            }
        }
        Planes {
            c: self.out,
            h,
            w,
            data: out,
        }
    }
}

struct SendPtr(*mut f32);
// SAFETY: see `Conv::apply`; threads write disjoint parts through it.
unsafe impl Sync for SendPtr {}

/// 2 x 2 max pooling, dropping an odd last row or column.
fn max_pool(x: &Planes) -> Planes {
    let (h, w) = (x.h / 2, x.w / 2);
    let mut data = Vec::with_capacity(x.c * h * w);
    for c in 0..x.c {
        let plane = &x.data[c * x.h * x.w..];
        for r in 0..h {
            for t in 0..w {
                let at = |rr: usize, tt: usize| plane[rr * x.w + tt];
                let (r2, t2) = (r * 2, t * 2);
                data.push(
                    at(r2, t2)
                        .max(at(r2, t2 + 1))
                        .max(at(r2 + 1, t2))
                        .max(at(r2 + 1, t2 + 1)),
                );
            }
        }
    }
    Planes { c: x.c, h, w, data }
}

fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) as u32) << 31;
    let exp = ((h >> 10) & 0x1f) as u32;
    let frac = (h & 0x3ff) as u32;
    let bits = match exp {
        0 if frac == 0 => sign,
        // Subnormal: normalise it.
        0 => {
            let shift = frac.leading_zeros() - 21;
            sign | ((113 - shift) << 23) | ((frac << (shift + 1)) & 0x3ff) << 13
        }
        0x1f => sign | 0x7f80_0000 | (frac << 13),
        _ => sign | ((exp + 112) << 23) | (frac << 13),
    };
    f32::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x3555), 0.333_251_95);
        assert_eq!(f16_to_f32(0x0001), 5.960_464_5e-8);
        assert_eq!(f16_to_f32(0x0000), 0.0);
    }

    #[test]
    fn classes_follow_the_camelot_wheel() {
        assert_eq!(class_key(0).camelot(), "1A");
        assert_eq!(class_key(7).name(), "Am");
        assert_eq!(class_key(19).name(), "C");
        assert_eq!(class_key(23).camelot(), "12B");
    }

    #[test]
    fn the_bundled_network_reads() {
        let net = network();
        assert_eq!(net.layers.len(), 9);
        assert_eq!(net.layers[8].out, 24);
    }
}
