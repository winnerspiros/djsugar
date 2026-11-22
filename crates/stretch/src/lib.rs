use std::f32::consts::TAU;
use std::sync::Arc;

use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

const ONSET_RISE: f32 = 0.25;
const MAX_CHUNK: usize = 1 << 15;

impl std::fmt::Debug for Stretch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stretch").field("channels", &self.channels).field("block", &self.block).finish()
    }
}

pub struct Stretch {
    channels: usize,
    block: usize,
    hop: usize,
    bins: usize,
    window: Vec<f32>,
    synth: Vec<f32>,
    fwd: Arc<dyn RealToComplex<f32>>,
    inv: Arc<dyn ComplexToReal<f32>>,
    fwd_scratch: Vec<Complex32>,
    inv_scratch: Vec<Complex32>,
    frame: Vec<f32>,
    spectrum: Vec<Complex32>,
    history: Vec<Vec<f32>>,
    filled: usize,
    base: usize,
    acc: Vec<Vec<f32>>,
    taken: usize,
    state: Vec<Channel>,
    last_end: Option<usize>,
    fresh: bool,
    transpose: f32,
    mag: Vec<f32>,
    freq: Vec<f32>,
    src_phase: Vec<f32>,
    out_mag: Vec<f32>,
    peaks: Vec<usize>,
    job: Option<Job>,
    split: bool,
    analysed: Vec<Complex32>,
}

struct Channel {
    phase: Vec<f32>,
    out_phase: Vec<f32>,
    mag: Vec<f32>,
    spec: Vec<Complex32>,
    pending: Vec<f32>,
}

struct Job {
    end: usize,
    reuse: bool,
    step: usize,
}

impl Stretch {
    pub fn new(channels: u32, block: usize, interval: usize) -> Self {
        let channels = channels.max(1) as usize;
        let block = block.max(16) & !1;
        let hop = interval.clamp(1, block / 2);
        let bins = block / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(block);
        let inv = planner.plan_fft_inverse(block);
        let window: Vec<f32> = (0..block).map(|k| 0.5 - 0.5 * (TAU * k as f32 / block as f32).cos()).collect();
        let mut overlap = vec![0.0f32; hop];
        for (k, w) in window.iter().enumerate() {
            overlap[k % hop] += w * w;
        }
        let synth = window.iter().enumerate().map(|(k, w)| w / (block as f32 * overlap[k % hop].max(1e-6))).collect();
        // Room for a chunk, and for one more while a split block reads
        // input from the chunk before.
        let capacity = block + hop + 2 * MAX_CHUNK;
        let mut s = Self {
            channels,
            block,
            hop,
            bins,
            window,
            synth,
            fwd_scratch: fwd.make_scratch_vec(),
            inv_scratch: inv.make_scratch_vec(),
            frame: vec![0.0; block],
            spectrum: vec![Complex32::default(); bins],
            fwd,
            inv,
            history: vec![vec![0.0; capacity]; channels],
            filled: 0,
            base: 0,
            acc: vec![vec![0.0; block]; channels],
            taken: 0,
            state: (0..channels)
                .map(|_| Channel {
                    phase: vec![0.0; bins],
                    out_phase: vec![0.0; bins],
                    mag: vec![0.0; bins],
                    spec: vec![Complex32::default(); bins],
                    pending: vec![0.0; block],
                })
                .collect(),
            last_end: None,
            fresh: true,
            transpose: 1.0,
            mag: vec![0.0; bins],
            freq: vec![0.0; bins],
            src_phase: vec![0.0; bins],
            out_mag: vec![0.0; bins],
            peaks: Vec::with_capacity(bins),
            job: None,
            split: false,
            analysed: vec![Complex32::default(); bins],
        };
        s.reset();
        s
    }

    pub fn standard(channels: u32, sample_rate: u32) -> Self {
        let hop = (sample_rate as f32 * 0.03).round() as usize;
        Self::new(channels, 4 * hop, hop)
    }

    pub fn split(channels: u32, sample_rate: u32) -> Self {
        let mut s = Self::standard(channels, sample_rate);
        s.split = true;
        s
    }

    pub fn light(channels: u32, sample_rate: u32) -> Self {
        let hop = (sample_rate as f32 * 0.025).round() as usize;
        Self::new(channels, 4 * hop, hop)
    }

    pub fn input_latency(&self) -> usize {
        self.block / 2
    }

    pub fn output_latency(&self) -> usize {
        self.block / 2 + if self.split { self.hop } else { 0 }
    }

    pub fn reset(&mut self) {
        for h in &mut self.history {
            h.fill(0.0);
        }
        for a in &mut self.acc {
            a.fill(0.0);
        }
        self.filled = self.block + self.hop;
        self.base = 0;
        self.taken = self.hop;
        self.last_end = None;
        self.fresh = true;
        self.job = None;
        for st in &mut self.state {
            st.pending.fill(0.0);
        }
    }

    pub fn set_transpose(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.transpose = factor;
        }
    }

    pub fn set_semitones(&mut self, semitones: f32) {
        self.set_transpose((semitones / 12.0).exp2());
    }

    pub fn seek(&mut self, input: &[f32]) {
        self.reset();
        for chunk in input.chunks(MAX_CHUNK * self.channels) {
            self.append(chunk);
        }
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let ch = self.channels;
        let (n_in, n_out) = (input.len() / ch, output.len() / ch);
        if n_in <= MAX_CHUNK {
            return self.process_chunk(input, output);
        }
        let pieces = n_in.div_ceil(MAX_CHUNK);
        for p in 0..pieces {
            let (a, b) = (p * n_in / pieces, (p + 1) * n_in / pieces);
            let (oa, ob) = (a * n_out / n_in, b * n_out / n_in);
            self.process_chunk(&input[a * ch..b * ch], &mut output[oa * ch..ob * ch]);
        }
    }

    fn process_chunk(&mut self, input: &[f32], output: &mut [f32]) {
        let ch = self.channels;
        let (n_in, n_out) = (input.len() / ch, output.len() / ch);
        let start = self.base + self.filled;
        self.append(input);
        for i in 0..n_out {
            if self.taken == self.hop {
                let end = start + i * n_in / n_out;
                let mut primed = false;
                if self.fresh {
                    // Fill the overlap from the blocks before, so the
                    // output starts at full level.
                    let first = self.base + self.block + self.hop;
                    let back = self.block / self.hop - usize::from(!self.split);
                    for k in (1..=back).rev() {
                        if let Some(e) = end.checked_sub(k * self.hop).filter(|&e| e >= first) {
                            self.next_block(e);
                            primed = true;
                        }
                    }
                }
                if !self.split {
                    self.next_block(end);
                } else {
                    if !primed {
                        self.finish_job();
                        self.commit();
                    }
                    self.begin(end);
                }
            } else if let Some(job) = &self.job {
                // Keep the block in hand on pace to finish with the interval.
                let due = self.taken * 2 * self.channels / self.hop;
                if job.step < due {
                    self.step();
                }
            }
            for c in 0..ch {
                output[i * ch + c] = self.acc[c][self.taken];
            }
            self.taken += 1;
        }
    }

    fn append(&mut self, input: &[f32]) {
        let ch = self.channels;
        let n = input.len() / ch;
        if n == 0 {
            return;
        }
        let capacity = self.history[0].len();
        if self.filled + n > capacity {
            let mut drop = self.filled - (self.block + self.hop);
            if let Some(job) = &self.job {
                drop = drop.min(job.end - self.hop - self.block - self.base);
            }
            for h in &mut self.history {
                h.copy_within(drop..self.filled, 0);
            }
            self.base += drop;
            self.filled -= drop;
        }
        for (c, h) in self.history.iter_mut().enumerate() {
            let dst = &mut h[self.filled..self.filled + n];
            for (d, s) in dst.iter_mut().zip(input[c..].iter().step_by(ch)) {
                *d = *s;
            }
        }
        self.filled += n;
    }

    fn next_block(&mut self, end: usize) {
        self.begin(end);
        self.finish_job();
        self.commit();
    }

    fn begin(&mut self, end: usize) {
        let reuse = self.last_end == Some(end.wrapping_sub(self.hop));
        self.job = Some(Job { end, reuse, step: 0 });
    }

    fn finish_job(&mut self) {
        while self.job.is_some() {
            self.step();
        }
    }

    fn step(&mut self) {
        let Some(job) = &mut self.job else { return };
        let (end, reuse, s) = (job.end, job.reuse, job.step);
        job.step += 1;
        let c = s / 2;
        if s % 2 == 0 {
            self.analyse_channel(c, end, reuse);
        } else {
            self.synth_channel(c);
        }
        if s + 1 == 2 * self.channels {
            self.job = None;
            self.last_end = Some(end);
            self.fresh = false;
        }
    }

    fn commit(&mut self) {
        let (block, hop) = (self.block, self.hop);
        for (a, st) in self.acc.iter_mut().zip(&mut self.state) {
            a.copy_within(hop.., 0);
            a[block - hop..].fill(0.0);
            for (a, x) in a.iter_mut().zip(&st.pending) {
                *a += x;
            }
            st.pending.fill(0.0);
        }
        self.taken = 0;
    }

    fn analyse_channel(&mut self, c: usize, end: usize, reuse: bool) {
        let (block, hop, bins) = (self.block, self.hop, self.bins);
        let t = self.transpose;
        let bin_step = TAU / block as f32;
        if !reuse {
            self.analyse(c, end - hop);
            for (p, x) in self.state[c].phase.iter_mut().zip(&self.spectrum) {
                *p = x.arg();
            }
        }
        self.analyse(c, end);
        let st = &mut self.state[c];
        self.analysed.copy_from_slice(&self.spectrum);
        for k in 0..bins {
            let x = self.spectrum[k];
            let phase = x.arg();
            self.mag[k] = x.norm();
            let expected = k as f32 * bin_step * hop as f32;
            let dev = wrap(phase - st.phase[k] - expected);
            self.freq[k] = k as f32 * bin_step + dev / hop as f32;
            st.phase[k] = phase;
        }

        // A sharp rise in level is an onset: phases restart from the
        // analysis there, keeping the attack where it was.
        let (mut rise, mut total) = (0.0, 1e-9);
        for (m, prev) in self.mag.iter().zip(&mut st.mag) {
            rise += (m - *prev).max(0.0);
            total += m;
            *prev = *m;
        }
        let onset = self.fresh || rise > ONSET_RISE * total;

        self.peaks.clear();
        let m = &self.mag;
        for k in 2..bins.saturating_sub(2) {
            if m[k] > m[k - 1] && m[k] >= m[k + 1] && m[k] > m[k - 2] && m[k] >= m[k + 2] {
                self.peaks.push(k);
            }
        }

        // Each peak moves to its new frequency with the bins around it
        // (out to halfway to the next peak) moving rigidly alongside,
        // their phases kept relative to the peak's.
        self.src_phase.copy_from_slice(&st.out_phase);
        self.out_mag.fill(0.0);
        let out = &mut st.out_phase;
        for (n, &p) in self.peaks.iter().enumerate() {
            let q = (p as f32 * t).round() as usize;
            if q >= bins {
                break;
            }
            let lo = if n == 0 { 0 } else { (self.peaks[n - 1] + p).div_ceil(2) };
            let hi = self.peaks.get(n + 1).map_or(bins, |&next| (p + next).div_ceil(2));
            let peak_phase = if onset { st.phase[p] } else { wrap(self.src_phase[q] + self.freq[p] * t * hop as f32) };
            for k in lo..hi {
                let Some(j) = (k + q).checked_sub(p).filter(|&j| j < bins) else {
                    continue;
                };
                if self.mag[k] > self.out_mag[j] {
                    self.out_mag[j] = self.mag[k];
                    out[j] = peak_phase + st.phase[k] - st.phase[p];
                }
            }
        }

        if onset && t != 1.0 {
            // At an onset the frame is resampled whole about its centre
            // instead, which keeps the attack tight rather than spread
            // over the block. Odd bins carry the half-block shift.
            let x = &self.analysed;
            let centred = |k: usize| if k.is_multiple_of(2) { x[k] } else { -x[k] };
            for j in 0..bins {
                let src = j as f32 / t;
                let i = src as usize;
                let y = if i + 1 < bins {
                    let f = src - i as f32;
                    centred(i) * (1.0 - f) + centred(i + 1) * f
                } else {
                    Complex32::default()
                };
                let y = if j.is_multiple_of(2) { y } else { -y };
                self.spectrum[j] = y;
                out[j] = y.arg();
            }
        } else {
            for j in 0..bins {
                self.spectrum[j] = Complex32::from_polar(self.out_mag[j], out[j]);
            }
        }
        self.state[c].spec.copy_from_slice(&self.spectrum);
    }

    fn synth_channel(&mut self, c: usize) {
        let bins = self.bins;
        let st = &mut self.state[c];
        self.spectrum.copy_from_slice(&st.spec);
        self.spectrum[0].im = 0.0;
        self.spectrum[bins - 1].im = 0.0;
        let _ = self.inv.process_with_scratch(&mut self.spectrum, &mut self.frame, &mut self.inv_scratch);
        for ((p, x), w) in st.pending.iter_mut().zip(&self.frame).zip(&self.synth) {
            *p = x * w;
        }
    }

    fn analyse(&mut self, c: usize, end: usize) {
        let from = end - self.base - self.block;
        let src = &self.history[c][from..from + self.block];
        for ((f, s), w) in self.frame.iter_mut().zip(src).zip(&self.window) {
            *f = s * w;
        }
        let _ = self.fwd.process_with_scratch(&mut self.frame, &mut self.spectrum, &mut self.fwd_scratch);
    }
}

fn wrap(x: f32) -> f32 {
    x - TAU * (x / TAU).round()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    fn sine(freq: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = 0.5 * (TAU * freq * i as f32 / RATE as f32).sin();
                [s, s]
            })
            .collect()
    }

    fn pitch(x: &[f32]) -> f32 {
        let left: Vec<f32> = x.iter().step_by(2).copied().collect();
        let crossings = left.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        crossings as f32 * RATE as f32 / left.len() as f32
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn run(s: &mut Stretch, input: &[f32], rate: f64) -> Vec<f32> {
        let mut out = Vec::new();
        let mut buf = vec![0.0; 1024];
        let (mut at, mut fed) = (0usize, 0.0f64);
        while at < input.len() / 2 {
            fed += 512.0 * rate;
            let n = (fed as usize - at).min(input.len() / 2 - at);
            s.process(&input[at * 2..(at + n) * 2], &mut buf);
            at += n;
            out.extend_from_slice(&buf);
        }
        out
    }

    #[test]
    fn transpose_moves_pitch() {
        for (factor, want) in [(2.0, 880.0), (1.26, 554.4), (0.8, 352.0)] {
            let mut s = Stretch::standard(2, RATE);
            s.set_transpose(factor);
            let out = run(&mut s, &sine(440.0, RATE as usize * 3), 1.0);
            let steady = &out[RATE as usize..RATE as usize * 4];
            let got = pitch(steady);
            assert!((got / want - 1.0).abs() < 0.01, "{factor}: {got} vs {want}");
            assert!((rms(steady) / 0.3536 - 1.0).abs() < 0.12, "{}", rms(steady));
        }
    }

    #[test]
    fn stretching_keeps_pitch_and_level() {
        for rate in [0.5, 1.0, 1.5, 4.0] {
            let mut s = Stretch::standard(2, RATE);
            let out = run(&mut s, &sine(440.0, RATE as usize * 4), rate);
            let steady = &out[RATE as usize / 2..RATE as usize * 2];
            let got = pitch(steady);
            assert!((got / 440.0 - 1.0).abs() < 0.01, "{rate}: {got}");
            assert!((rms(steady) / 0.3536 - 1.0).abs() < 0.1, "{rate}: {}", rms(steady));
        }
    }

    #[test]
    fn unity_is_transparent_after_the_latency() {
        let mut s = Stretch::standard(2, RATE);
        let input = sine(440.0, RATE as usize * 2);
        let out = run(&mut s, &input, 1.0);
        let lag = s.input_latency() + s.output_latency();
        let r = RATE as usize;
        let (a, b) = (&input[r..3 * r], &out[r + 2 * lag..3 * r + 2 * lag]);
        let err = rms(&a.iter().zip(b).map(|(x, y)| x - y).collect::<Vec<_>>());
        assert!(err < 0.01, "{err}");
    }

    #[test]
    fn a_click_comes_out_after_the_latency() {
        let mut s = Stretch::standard(2, RATE);
        let mut input = vec![0.0f32; RATE as usize * 2];
        let at = 30_000;
        input[at * 2] = 1.0;
        input[at * 2 + 1] = 1.0;
        let out = run(&mut s, &input, 1.0);
        let peak = (0..out.len() / 2).max_by(|&a, &b| out[a * 2].abs().total_cmp(&out[b * 2].abs())).unwrap();
        let want = at + s.input_latency() + s.output_latency();
        assert!(peak.abs_diff(want) <= 2, "{peak} vs {want}");
    }

    #[test]
    fn seek_starts_without_a_fade_in() {
        let mut s = Stretch::standard(2, RATE);
        let input = sine(440.0, RATE as usize);
        let seed = 3 * s.input_latency();
        s.seek(&input[..seed * 2]);
        let mut out = vec![0.0; 2 * 2048];
        s.process(&input[seed * 2..(seed + 2048) * 2], &mut out);
        let level = rms(&out[1024..]);
        assert!((level / 0.3536 - 1.0).abs() < 0.15, "{level}");
    }

    #[test]
    fn empty_input_freezes() {
        let mut s = Stretch::standard(2, RATE);
        let input = sine(440.0, RATE as usize);
        run(&mut s, &input, 1.0);
        let mut out = vec![0.0; 2 * 4096];
        s.process(&[], &mut out);
        s.process(&[], &mut out);
        assert!((pitch(&out) / 440.0 - 1.0).abs() < 0.02, "{}", pitch(&out));
    }

    #[test]
    fn split_matches_whole_blocks_an_interval_later() {
        let input = sine(440.0, RATE as usize * 2);
        let mut a = Stretch::standard(2, RATE);
        let mut b = Stretch::split(2, RATE);
        a.set_transpose(1.2);
        b.set_transpose(1.2);
        let (x, y) = (run(&mut a, &input, 1.0), run(&mut b, &input, 1.0));
        let d = b.output_latency() - a.output_latency();
        let r = RATE as usize;
        let err = rms(&x[r..2 * r].iter().zip(&y[r + 2 * d..]).map(|(p, q)| p - q).collect::<Vec<_>>());
        assert!(err < 1e-4, "{err}");
    }

    #[test]
    fn split_seek_starts_without_a_fade_in() {
        let mut s = Stretch::split(2, RATE);
        let input = sine(440.0, RATE as usize);
        let seed = 3 * s.input_latency();
        s.seek(&input[..seed * 2]);
        let mut out = vec![0.0; 2 * 4096];
        s.process(&input[seed * 2..(seed + 4096) * 2], &mut out);
        let level = rms(&out[1024..]);
        assert!((level / 0.3536 - 1.0).abs() < 0.15, "{level}");
    }
}
