//! Deezer's Spleeter 4-stem model (MIT): one U-Net per stem predicts a
//! magnitude spectrogram, and the four predictions become ratio masks on the
//! mix's spectrogram, so the stems add up to the mix exactly.
//!
//! Weights come from `scripts/models/convert_spleeter.py`. The network follows
//! `spleeter/model/functions/unet.py` with the 4-stem settings (ELU in both
//! halves, TensorFlow "same" padding). It runs on burn ([`gpu::Net`]) or on
//! plain CPU matrix products ([`cpu::Net`]), which are much faster than
//! burn's CPU backend for these convolutions.

pub mod cpu;
pub mod gpu;

use realfft::num_complex::Complex32;
use safetensors::SafeTensors;

use crate::stft::{Stft, BINS, FRAME, HOP};

/// The stems in the order the model and the weights file have them.
pub const STEMS: [&str; 4] = ["vocals", "drums", "bass", "other"];

/// Frames per window the network sees (about 11.9 s).
pub const T: usize = 512;
/// Frequency bins the network sees (up to 11 kHz); the rest of the mask is
/// the average of these.
pub const F: usize = 1024;
const EPSILON: f32 = 1e-10;

/// Separated stereo stems, in [`STEMS`] order: `[stem][channel][sample]`.
pub type Stems = Vec<[Vec<f32>; 2]>;

/// The four U-Nets on some device.
pub trait Net {
    /// Each stem's estimated magnitudes for one window of the mix's
    /// magnitudes (`[channel][frame][bin]`, 2 × [`T`] × [`F`]), in the same
    /// layout, one after the other in [`STEMS`] order.
    fn estimate(&self, magnitudes: Vec<f32>) -> Vec<f32>;
}

/// One stem's U-Net on its own, for when only that stem's level is wanted.
pub trait OneStem {
    /// Stem `stem`'s estimated magnitudes for one window, laid out as in
    /// [`Net::estimate`] (2 × [`T`] × [`F`]).
    fn estimate_stem(&self, stem: usize, magnitudes: &[f32]) -> Vec<f32>;
}

/// Frames of the vocal level per second (one per STFT hop at 44.1 kHz).
pub const VOCAL_RATE: f64 = crate::SAMPLE_RATE as f64 / HOP as f64;

/// How much of a stereo track at 44.1 kHz is vocals, frame by frame
/// ([`VOCAL_RATE`] a second): the vocal network's share of the mix's
/// energy from 150 Hz to 8 kHz, 0 to 1. Only the vocal U-Net runs and no
/// audio is rebuilt, so this costs about a quarter of a separation.
/// `keep_going` is asked between windows; false stops early with what is
/// done so far (the rest 0).
pub fn vocal_levels(
    net: &impl OneStem,
    channels: [&[f32]; 2],
    mut keep_going: impl FnMut() -> bool,
) -> Vec<f32> {
    let len = channels[0].len();
    if len == 0 {
        return Vec::new();
    }
    let stft = Stft::new();
    let padded: Vec<Vec<f32>> = channels
        .iter()
        .map(|c| {
            let mut p = vec![0.0; FRAME];
            p.extend_from_slice(c);
            p
        })
        .collect();
    let frames = Stft::frames(len + FRAME);
    // The bins from 150 Hz to 8 kHz, where voices are.
    let hz = |f: f64| (f * FRAME as f64 / crate::SAMPLE_RATE as f64) as usize;
    let (lo, hi) = (hz(150.0), hz(8000.0).min(F));
    let mut row = vec![Complex32::default(); BINS];
    let mut out = vec![0.0f32; frames];
    for w in 0..frames.div_ceil(T) {
        if !keep_going() {
            break;
        }
        let f0 = w * T;
        let count = (frames - f0).min(T);
        let mut mags = vec![0.0f32; 2 * T * F];
        for (c, p) in padded.iter().enumerate() {
            for f in 0..count {
                stft.frame(p, f0 + f, &mut row);
                let dst = &mut mags[(c * T + f) * F..][..F];
                for (d, z) in dst.iter_mut().zip(&row[..F]) {
                    *d = z.norm();
                }
            }
        }
        let est = net.estimate_stem(0, &mags);
        for f in 0..count {
            let (mut vocal, mut mix) = (0.0f32, 0.0f32);
            for c in 0..2 {
                let at = (c * T + f) * F;
                for b in lo..hi {
                    let m = mags[at + b];
                    let v = est[at + b].min(m);
                    vocal += v * v;
                    mix += m * m;
                }
            }
            out[f0 + f] = if mix > 1e-6 {
                (vocal / mix).sqrt()
            } else {
                0.0
            };
        }
    }
    // Frame k is centred on padded sample k·HOP + FRAME/2, which is the
    // track's sample k·HOP − FRAME/2: drop the frames before the track.
    out.drain(..(FRAME / 2 / HOP).min(out.len()));
    out
}

/// A tensor from the weights file as f32, with its shape.
pub(crate) fn tensor(st: &SafeTensors, name: &str) -> Result<(Vec<f32>, Vec<usize>), String> {
    let t = st.tensor(name).map_err(|e| format!("{name}: {e}"))?;
    let data = t
        .data()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| half::f16::from_le_bytes(*b).to_f32())
        .collect();
    Ok((data, t.shape().to_vec()))
}

/// Spleeter's masks from the four estimates, in place: each stem's share of
/// the summed squares.
fn to_masks(estimates: &mut [f32]) {
    let n = estimates.len() / STEMS.len();
    for i in 0..n {
        let total: f32 = (0..STEMS.len())
            .map(|s| estimates[s * n + i].powi(2))
            .sum::<f32>()
            + EPSILON;
        for s in 0..STEMS.len() {
            let e = &mut estimates[s * n + i];
            *e = (e.powi(2) + EPSILON / STEMS.len() as f32) / total;
        }
    }
}

/// Separates a stereo track at 44.1 kHz with `net`. Windows are separated
/// starting with the one holding sample `from`, then onwards to the end, then
/// back to the start; `done(window, windows)` is called after each.
pub fn separate(
    net: &impl Net,
    channels: [&[f32]; 2],
    from: usize,
    done: impl FnMut(usize, usize),
) -> Stems {
    let len = channels[0].len();
    let mut out: Stems = Vec::new();
    run(
        net,
        channels,
        from,
        len.max(1),
        0,
        |_, parts| {
            out = parts
                .iter()
                .map(|[l, r]| [l.to_vec(), r.to_vec()])
                .collect();
            true
        },
        done,
    );
    if out.is_empty() {
        out = (0..STEMS.len()).map(|_| [Vec::new(), Vec::new()]).collect();
    }
    out
}

/// Separates like [`separate`], handing over each stretch of `chunk`
/// samples as soon as it is final (every window that overlaps it is done),
/// so the stems can play while the rest is separated, and keeping only the
/// windows still needed in memory. `ready(k, stems)` gets chunk `k`,
/// samples `k * chunk..`, as `[stem][channel][sample]`, preceded by up to
/// `lead` samples before it (for filters to settle on; those may lack a
/// window that isn't done yet); chunks come in no particular order. Stops
/// early, returning false, when `ready` returns false.
pub fn separate_in_chunks(
    net: &impl Net,
    channels: [&[f32]; 2],
    from: usize,
    chunk: usize,
    lead: usize,
    ready: impl FnMut(usize, Vec<[&[f32]; 2]>) -> bool,
) -> bool {
    run(net, channels, from, chunk, lead, ready, |_, _| {})
}

/// One window's stems, overlap-added from its own frames:
/// `[stem][channel][sample]`, from padded sample `first frame × HOP`.
type Local = Vec<[Vec<f32>; 2]>;

fn run(
    net: &impl Net,
    channels: [&[f32]; 2],
    from: usize,
    chunk: usize,
    lead: usize,
    mut ready: impl FnMut(usize, Vec<[&[f32]; 2]>) -> bool,
    mut done: impl FnMut(usize, usize),
) -> bool {
    let stft = Stft::new();
    let len = channels[0].len();
    if len == 0 {
        return true;
    }
    // Spleeter pads the front with a frame of zeros so the first samples
    // are covered by as many frames as the rest.
    let padded: Vec<Vec<f32>> = channels
        .iter()
        .map(|c| {
            let mut p = vec![0.0; FRAME];
            p.extend_from_slice(c);
            p
        })
        .collect();
    let frames = Stft::frames(len + FRAME);
    let windows = frames.div_ceil(T).max(1);
    let first = ((from + FRAME) / HOP / T).min(windows - 1);
    let order = (first..windows).chain((0..first).rev());
    // The unpadded samples window `w` adds to.
    let span = |w: usize| {
        let f0 = w * T;
        let count = (frames - f0).min(T);
        (
            (f0 * HOP).saturating_sub(FRAME),
            ((f0 + count - 1) * HOP).min(len),
        )
    };
    let overlaps = |w: usize, (a, b): (usize, usize)| {
        let (s, e) = span(w);
        s < b && e > a
    };
    let chunks = len.div_ceil(chunk);
    let range = |k: usize| (k * chunk, ((k + 1) * chunk).min(len));

    let mut local: Vec<Option<Local>> = (0..windows).map(|_| None).collect();
    let mut finished = vec![false; windows];
    let mut sent = vec![false; chunks];
    let mut spec = vec![Complex32::default(); T * BINS];
    let mut masked = vec![Complex32::default(); T * BINS];
    for (n, w) in order.enumerate() {
        let f0 = w * T;
        let count = (frames - f0).min(T);
        let mut specs = Vec::with_capacity(2);
        let mut mags = vec![0.0f32; 2 * T * F];
        for (c, p) in padded.iter().enumerate() {
            for f in 0..count {
                let row = &mut spec[f * BINS..][..BINS];
                stft.frame(p, f0 + f, row);
                let dst = &mut mags[(c * T + f) * F..][..F];
                for (d, z) in dst.iter_mut().zip(&row[..F]) {
                    *d = z.norm();
                }
            }
            specs.push(spec[..count * BINS].to_vec());
        }
        let mut masks = net.estimate(mags);
        to_masks(&mut masks);
        let size = (count - 1) * HOP + FRAME;
        let mut stems: Local = (0..STEMS.len())
            .map(|_| [vec![0.0; size], vec![0.0; size]])
            .collect();
        for (s, stem) in stems.iter_mut().enumerate() {
            for (c, spec) in specs.iter().enumerate() {
                for f in 0..count {
                    let m = &masks[((s * 2 + c) * T + f) * F..][..F];
                    let high = m.iter().sum::<f32>() / F as f32;
                    let src = &spec[f * BINS..][..BINS];
                    let dst = &mut masked[f * BINS..][..BINS];
                    for (b, (d, z)) in dst.iter_mut().zip(src).enumerate() {
                        *d = z * m.get(b).copied().unwrap_or(high);
                    }
                }
                stft.synthesise(&masked[..count * BINS], 0, &mut stem[c]);
            }
        }
        local[w] = Some(stems);
        finished[w] = true;
        done(n, windows);

        for (k, sent) in sent.iter_mut().enumerate() {
            let (a, b) = range(k);
            if *sent
                || !(0..windows)
                    .filter(|&v| overlaps(v, (a, b)))
                    .all(|v| finished[v])
            {
                continue;
            }
            *sent = true;
            let a = a.saturating_sub(lead);
            let mut parts: Vec<[Vec<f32>; 2]> = (0..STEMS.len())
                .map(|_| [vec![0.0; b - a], vec![0.0; b - a]])
                .collect();
            for (v, l) in local.iter().enumerate() {
                let Some(l) = l else { continue };
                // Local sample j is unpadded sample j + v·T·HOP − FRAME.
                let origin = (v * T * HOP) as isize - FRAME as isize;
                for (part, stem) in parts.iter_mut().zip(l) {
                    for (dst, src) in part.iter_mut().zip(stem) {
                        for (i, d) in dst.iter_mut().enumerate() {
                            let j = (a + i) as isize - origin;
                            if let Some(x) = usize::try_from(j).ok().and_then(|j| src.get(j)) {
                                *d += x;
                            }
                        }
                    }
                }
            }
            let view = parts.iter().map(|[l, r]| [&l[..], &r[..]]).collect();
            if !ready(k, view) {
                return false;
            }
        }
        // Windows every overlapping chunk has had are no longer needed.
        for (v, l) in local.iter_mut().enumerate() {
            if l.is_some() && (0..chunks).all(|k| sent[k] || !overlaps(v, range(k))) {
                *l = None;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Estimates each stem as a fixed share of the mix, different per bin.
    struct Fake;

    impl OneStem for Fake {
        fn estimate_stem(&self, _stem: usize, magnitudes: &[f32]) -> Vec<f32> {
            // Vocals in the second half of the frames of each window only.
            magnitudes
                .iter()
                .enumerate()
                .map(|(i, m)| if (i % (T * F)) / F >= T / 2 { *m } else { 0.0 })
                .collect()
        }
    }

    #[test]
    fn vocal_levels_follow_the_vocal_estimate() {
        let len = T * HOP * 2;
        let x: Vec<f32> = (0..len).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let levels = vocal_levels(&Fake, [&x, &x], || true);
        assert_eq!(levels.len(), Stft::frames(len + FRAME) - FRAME / 2 / HOP);
        // Window 1's second half is all vocals, its first half none (two
        // frames in to stay clear of the padding).
        let w1 = T - FRAME / 2 / HOP;
        assert!(levels[w1 + 10] < 0.01, "{}", levels[w1 + 10]);
        assert!(
            levels[w1 + T / 2 + 10] > 0.99,
            "{}",
            levels[w1 + T / 2 + 10]
        );
    }

    impl Net for Fake {
        fn estimate(&self, magnitudes: Vec<f32>) -> Vec<f32> {
            (0..STEMS.len())
                .flat_map(|s| {
                    magnitudes
                        .iter()
                        .enumerate()
                        .map(move |(i, m)| m * (1 + (s + i) % 3) as f32)
                })
                .collect()
        }
    }

    #[test]
    fn chunks_match_the_whole_separation() {
        let len = 3 * T * HOP + 12_345;
        let channels: [Vec<f32>; 2] = [0.013f32, 0.021].map(|w| {
            (0..len)
                .map(|i| (i as f32 * w).sin() * 0.4 + ((i * 7919) % 97) as f32 / 400.0)
                .collect()
        });
        let input = [&channels[0][..], &channels[1][..]];
        let whole = separate(&Fake, input, 0, |_, _| {});
        let chunk = 100_000;
        let mut got = vec![false; len.div_ceil(chunk)];
        let lead = 1000;
        let complete = separate_in_chunks(&Fake, input, 2 * T * HOP, chunk, lead, |k, parts| {
            assert!(!got[k], "chunk {k} twice");
            got[k] = true;
            for (s, [l, r]) in parts.iter().enumerate() {
                let a = (k * chunk).saturating_sub(lead);
                assert_eq!(l.len(), (len - k * chunk).min(chunk) + k * chunk - a);
                let l = &l[k * chunk - a..];
                let r = &r[k * chunk - a..];
                let a = k * chunk;
                for (i, (x, y)) in l.iter().zip(r.iter()).enumerate() {
                    assert!(
                        (x - whole[s][0][a + i]).abs() < 1e-5,
                        "stem {s} at {}",
                        a + i
                    );
                    assert!((y - whole[s][1][a + i]).abs() < 1e-5);
                }
            }
            true
        });
        assert!(complete);
        assert!(got.iter().all(|&g| g));
        // The stems add up to the mix.
        let i = len / 2;
        let sum: f32 = whole.iter().map(|s| s[0][i]).sum();
        assert!((sum - channels[0][i]).abs() < 1e-3);
    }
}
