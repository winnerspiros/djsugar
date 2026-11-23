//! The added Scene effects of the FX Station: modulation, filter, roll,
//! gate, pitch and drive, synced to the beat of the FX Station clock.
//! Allocation free once allocated, for the engine thread.

use std::f32::consts::{PI, TAU};

// use crate::fxstation::dsp::soft_clip;

const TWO_PI: f32 = TAU;

/// A delay line read with 4-point Hermite interpolation: flat to about
/// 0.4 x the sample rate, no zipper or dulling when the delay moves.
#[derive(Clone, Debug, Default)]
pub struct HermiteLine {
    buffer: Vec<f32>,
    write: usize,
}

impl HermiteLine {
    pub fn allocate(&mut self, frames: usize) {
        self.buffer = vec![0.0; frames];
        self.write = 0;
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
    }

    /// The sample `delay` whole frames before the last write.
    pub fn at(&self, delay: usize) -> f32 {
        let size = self.buffer.len();
        let mut i = self.write.wrapping_sub(1 + delay);
        while i >= size {
            i = i.wrapping_add(size);
        }
        self.buffer[i]
    }

    pub fn write(&mut self, x: f32) {
        self.buffer[self.write] = x;
        self.write += 1;
        if self.write == self.buffer.len() {
            self.write = 0;
        }
    }

    /// The sample `delay` frames before the last write (delay >= 1).
    pub fn read(&self, delay: f32) -> f32 {
        let size = self.buffer.len();
        let delay = delay.clamp(1.0, (size - 4) as f32);
        let mut position = self.write as f32 - 1.0 - delay;
        while position < 0.0 {
            position += size as f32;
        }
        let i = position as usize;
        let t = position - i as f32;
        let xm1 = self.buffer[(i + size - 1) % size];
        let x0 = self.buffer[i % size];
        let x1 = self.buffer[(i + 1) % size];
        let x2 = self.buffer[(i + 2) % size];
        let c1 = 0.5 * (x1 - xm1);
        let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
        let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
        ((c3 * t + c2) * t + c1) * t + x0
    }
}

/// A DC blocker (one pole high pass at about 10 Hz).
#[derive(Clone, Copy, Debug, Default)]
pub struct DcBlocker {
    x1: f32,
    y1: f32,
}

impl DcBlocker {
    pub fn process(&mut self, x: f32, r: f32) -> f32 {
        let y = x - self.x1 + r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

/// A triangle of a phase in [0, 1), from -1 to 1, smooth at its turns.
#[inline]
fn soft_triangle(phase: f32) -> f32 {
    let tri = 1.0 - 4.0 * (phase - (phase + 0.5).floor()).abs();
    // A cubic easing of the corners: no clicks of the sweep direction.
    tri * (1.5 - 0.5 * tri * tri)
}

/// Flanger: a short delay swept by the beat, with feedback and the
/// dry signal on equal level (the deep comb notches of tape flanging).
/// Stereo with the right sweep a quarter of a cycle later.
#[derive(Clone, Debug, Default)]
pub struct Flanger {
    lines: [HermiteLine; 2],
    feedback: [f32; 2],
    dc: [DcBlocker; 2],
}

impl Flanger {
    pub fn allocate(&mut self, max_sample_rate: f32) {
        for line in &mut self.lines {
            line.allocate((0.02 * max_sample_rate) as usize + 8);
        }
    }

    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.feedback = [0.0, 0.0];
        for dc in &mut self.dc {
            dc.reset();
        }
    }

    /// phase: of the sweep (beats / sweep length); feedback in [0, 1).
    pub fn process(&mut self, left: &mut f32, right: &mut f32, phase: f32, feedback: f32, sample_rate: f32) {
        let input = [*left, *right];
        let mut output = [0.0, 0.0];
        for c in 0..2 {
            let lfo = 0.5 + 0.5 * soft_triangle(phase + 0.25 * c as f32);
            // 0.25 to 6 ms, swept exponentially: even notch movement.
            let ms = 0.25 * 24.0_f32.powf(lfo);
            let delayed = self.lines[c].read(ms * 0.001 * sample_rate);
            let x = input[c] + feedback * self.feedback[c];
            self.lines[c].write(x.tanh());
            self.feedback[c] = self.dc[c].process(delayed, 0.9995);
            output[c] = 0.5 * (input[c] + delayed);
        }
        *left = output[0];
        *right = output[1];
    }
}

/// Phaser: 8 first order all passes (zero delay feedback form, stable at
/// any sweep speed), swept on a log scale from 120 Hz to 5 kHz by the beat,
/// with resonance. The channels sweep in opposite directions.
#[derive(Clone, Debug, Default)]
pub struct Phaser {
    state: [[f32; 8]; 2],
    feedback: [f32; 2],
}

impl Phaser {
    pub fn clear(&mut self) {
        self.state = [[0.0; 8]; 2];
        self.feedback = [0.0, 0.0];
    }

    pub fn process(&mut self, left: &mut f32, right: &mut f32, phase: f32, resonance: f32, sample_rate: f32) {
        let input = [*left, *right];
        let mut output = [0.0, 0.0];
        for c in 0..2 {
            let lfo = 0.5 + 0.5 * soft_triangle(phase + 0.5 * c as f32);
            let frequency = 120.0_f32 * (5000.0_f32 / 120.0_f32).powf(lfo);
            let g = (PI * frequency.min(0.45 * sample_rate) / sample_rate).tan();
            let a = g / (1.0 + g);
            let mut x = (input[c] + resonance * self.feedback[c]).tanh();
            for s in 0..8 {
                // All pass from the one pole low pass: 2 * lp - x.
                let z = &mut self.state[c][s];
                let v = (x - *z) * a;
                let lp = v + *z;
                *z = lp + v;
                x = 2.0 * lp - x;
            }
            self.feedback[c] = x;
            output[c] = 0.5 * (input[c] + x);
        }
        *left = output[0];
        *right = output[1];
    }
}

/// DJ filter: one knob, low pass to the left of the center, high pass to the right; 24
/// dB/octave (two zero delay feedback state variable filters), resonance on the second one,
/// its output softly limited so that it sings without blowing up. Bypassed in the center.
#[derive(Clone, Debug, Default)]
pub struct SweepFilter {
    s: [[[f32; 2]; 2]; 2], // [channel][stage][ic1/ic2]
}

impl SweepFilter {
    pub fn clear(&mut self) {
        self.s = [[[0.0, 0.0]; 2]; 2];
    }

    /// position: -1 (low pass closed) .. 0 (open) .. 1 (high pass closed).
    pub fn process(&mut self, left: &mut f32, right: &mut f32, position: f32, resonance: f32, sample_rate: f32) {
        let low_pass = position < 0.0;
        let amount = position.abs().min(1.0);
        // The center is the dry signal, eased into over the first 5%.
        let wet = (amount / 0.05).min(1.0);
        if wet <= 0.0 {
            self.s = [[[0.0, 0.0]; 2]; 2];
            return;
        }
        // Low pass 20 kHz -> 60 Hz, high pass 20 Hz -> 8 kHz.
        let frequency = if low_pass {
            20000.0_f32 * (60.0_f32 / 20000.0_f32).powf(amount)
        } else {
            20.0_f32 * (8000.0_f32 / 20.0_f32).powf(amount)
        };
        let g = (PI * frequency.min(0.45 * sample_rate) / sample_rate).tan();
        // Butterworth pair (Q 0.54 and 1.31), the second one up to Q 8.
        let k1 = 1.0 / 0.5412;
        let k2 = 1.0 / (1.3066 + 6.7 * resonance * amount);
        let channels = [left, right];
        for c in 0..2 {
            let dry = *channels[c];
            let mut x = self.stage(c, 0, dry, g, k1, low_pass);
            x = self.stage(c, 1, x, g, k2, low_pass);
            if resonance > 0.0 {
                // The resonant peak, limited softly above full scale.
                x = 1.5 * (x / 1.5).tanh();
            }
            *channels[c] = dry + (x - dry) * wet;
        }
    }

    fn stage(&mut self, c: usize, s: usize, x: f32, g: f32, k: f32, low_pass: bool) -> f32 {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v3 = x - self.s[c][s][1];
        let v1 = a1 * self.s[c][s][0] + g * a1 * v3;
        let v2 = self.s[c][s][1] + g * v1;
        self.s[c][s][0] = 2.0 * v1 - self.s[c][s][0];
        self.s[c][s][1] = 2.0 * v2 - self.s[c][s][1];
        if low_pass {
            v2
        } else {
            x - k * v1 - v2
        }
    }
}

/// Beat roll: the first slice after the press is recorded while it plays,
/// then repeated, every repeat quieter with the decay. Each repeat fades
/// in and out over 2 ms, so the loop points never click.
#[derive(Clone, Debug, Default)]
pub struct Roll {
    buffer: Vec<f32>,
    recorded: usize,
    length: usize,
    position: usize,
    gain: f32,
    active: bool,
}

impl Roll {
    pub fn allocate(&mut self, max_sample_rate: f32, max_seconds: f32) {
        self.buffer = vec![0.0; 2 * (max_sample_rate * max_seconds) as usize];
    }

    pub fn start(&mut self) {
        self.recorded = 0;
        self.position = 0;
        self.gain = 1.0;
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn process(&mut self, left: &mut f32, right: &mut f32, slice_frames: usize, decay: f32, sample_rate: f32) {
        if !self.active {
            return;
        }
        let capacity = self.buffer.len() / 2;
        let slice_frames = slice_frames.clamp(64, capacity);
        if self.recorded < slice_frames {
            self.buffer[2 * self.recorded] = *left;
            self.buffer[2 * self.recorded + 1] = *right;
            self.recorded += 1;
            self.length = self.recorded;
            return;
        }
        let length = slice_frames.min(self.length);
        if self.position >= length {
            self.position = 0;
            self.gain *= 1.0 - 0.5 * decay;
        }
        let fade_frames = (0.002 * sample_rate).max(1.0) as usize;
        let edge = self.position.min(length - 1 - self.position);
        let fade = if edge >= fade_frames { 1.0 } else { 0.5 - 0.5 * (PI * edge as f32 / fade_frames as f32).cos() };
        *left = self.buffer[2 * self.position] * fade * self.gain;
        *right = self.buffer[2 * self.position + 1] * fade * self.gain;
        self.position += 1;
    }
}

/// Gate (trans): chops the sound on the beat grid, with raised cosine
/// edges whose length follows the shape knob (hard chop .. soft pulse).
#[inline]
pub fn gate_gain(beat: f64, division: f64, width: f32, softness: f32) -> f32 {
    let phase = beat / division - (beat / division).floor();
    let p = phase as f32;
    // Open for `width` of the step, with edges of up to a quarter step.
    let edge = 0.004 + 0.25 * softness;
    if p < edge {
        0.5 - 0.5 * (PI * p / edge).cos()
    } else if p < width - edge {
        1.0
    } else if p < width {
        0.5 - 0.5 * (PI * (width - p) / edge).cos()
    } else {
        0.0
    }
}

/// Pitch shifter (WSOLA): a read tap runs through the recent input at the pitch ratio. When it drifts out of its window it jumps
/// a period back or forward, to the position (searched over +-5 ms) whose waveform matches the one playing best, and crossfades
/// until the next jump (up to 30 ms). Joins in phase do not warble or comb like fixed grains; about 20 ms of latency.
#[derive(Clone, Debug, Default)]
pub struct WsolaPitch {
    lines: [HermiteLine; 2],
    mono: HermiteLine,
    sample_rate: f32,
    period: usize,
    search: usize,
    max_fade: usize,
    fade: usize,
    match_len: usize,
    delay: f32,
    old_delay: f32,
    fade_left: usize,
    low: [f32; 2],
}

impl WsolaPitch {
    const K_PERIOD: f32 = 0.03;
    const K_SEARCH: f32 = 0.006;
    const K_FADE: f32 = 0.03;
    const K_MATCH: f32 = 0.012;

    pub fn allocate(&mut self, max_sample_rate: f32) {
        let frames = (0.25 * max_sample_rate) as usize + 16;
        for line in &mut self.lines {
            line.allocate(frames);
        }
        self.mono.allocate(frames);
        self.set_sample_rate(max_sample_rate);
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.period = (Self::K_PERIOD * sample_rate) as usize;
        self.search = (Self::K_SEARCH * sample_rate) as usize;
        self.max_fade = (Self::K_FADE * sample_rate) as usize;
        self.fade = self.max_fade;
        self.match_len = (Self::K_MATCH * sample_rate) as usize;
        self.clear();
    }

    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.mono.clear();
        self.delay = -1.0;
        self.fade_left = 0;
        self.low = [0.0, 0.0];
    }

    pub fn process(&mut self, left: &mut f32, right: &mut f32, ratio: f32) {
        let ratio = ratio.clamp(0.25, 4.0);
        // The longest crossfade that ends before the next jump.
        self.fade = 16.max(self.max_fade.min((0.9 * self.period as f32 / (1.0 - ratio).abs().max(1e-3)) as usize));
        self.lines[0].write(*left);
        self.lines[1].write(*right);
        self.mono.write(*left + *right);
        // The window of the tap: the old tap still plays the fade, at the
        // ratio, and must stay behind the write position.
        let base = 4.0 + self.search as f32 + self.fade as f32 * (ratio - 1.0).max(0.0);
        if self.delay < 0.0 {
            self.delay = base + 0.5 * self.period as f32;
        }
        if self.fade_left == 0 {
            if self.delay < base {
                self.jump(self.delay + self.period as f32);
            } else if self.delay > base + self.period as f32 {
                self.jump(self.delay - self.period as f32);
            }
        }
        let mut l = self.lines[0].read(self.delay);
        let mut r = self.lines[1].read(self.delay);
        if self.fade_left > 0 {
            // Raised cosine: the joined waveforms are in phase, their
            // amplitudes add up to one.
            let t = self.fade_left as f32 / self.fade as f32;
            let out = 0.5 - 0.5 * (PI * t).cos();
            l = l * (1.0 - out) + self.lines[0].read(self.old_delay) * out;
            r = r * (1.0 - out) + self.lines[1].read(self.old_delay) * out;
            self.old_delay += 1.0 - ratio;
            self.fade_left -= 1;
        }
        self.delay += 1.0 - ratio;
        // Shifted up, the images above the old Nyquist are softened.
        if ratio > 1.0 {
            let cutoff = (0.45_f32.min(0.5 / ratio) * self.sample_rate).min(0.45 * self.sample_rate);
            let a = 1.0 - (-TWO_PI * cutoff / self.sample_rate).exp();
            self.low[0] += (l - self.low[0]) * a;
            self.low[1] += (r - self.low[1]) * a;
            let blend = (ratio - 1.0).min(1.0);
            l += (self.low[0] - l) * blend;
            r += (self.low[1] - r) * blend;
        } else {
            self.low = [l, r];
        }
        *left = l;
        *right = r;
    }

    fn jump(&mut self, center: f32) {
        // The waveform before the tap, against the one before each candidate.
        let from = self.delay as usize;
        let mut best = -2.0;
        let mut best_delay = center as usize;
        let mut candidate = (center as isize - self.search as isize).max(2) as usize;
        let max_candidate = (center as usize + self.search).min(self.lines[0].buffer.len() - 2);
        while candidate <= max_candidate {
            // Around the tap: what it played and, as far as recorded, what
            // the crossfade will play.
            let mut cross = 0.0;
            let mut energy = 1e-9;
            let ahead = (self.match_len / 2).min(from).min(candidate);
            let ahead_i = ahead as isize;
            let match_len_i = self.match_len as isize;
            for k in (-ahead_i..(match_len_i - ahead_i)).step_by(2) {
                let cand_idx = candidate as isize + k;
                let from_idx = from as isize + k;
                if cand_idx < 0 || from_idx < 0 {
                    continue;
                }
                let cand_idx = cand_idx as usize;
                let from_idx = from_idx as usize;
                if cand_idx >= self.mono.buffer.len() || from_idx >= self.mono.buffer.len() {
                    continue;
                }
                let y = self.mono.at(cand_idx);
                cross += self.mono.at(from_idx) * y;
                energy += y * y;
            }
            let score = cross / energy.sqrt();
            if score > best {
                best = score;
                best_delay = candidate;
            }
            candidate += 2;
        }
        self.old_delay = self.delay;
        self.delay = best_delay as f32 + (self.delay - from as f32);
        self.fade_left = self.fade;
    }
}

/// A 2x oversampler: a 31 tap half band low pass (Kaiser windowed sinc,
/// about 90 dB of stop band) for the way up and the way down.
#[derive(Clone, Debug)]
pub struct Oversampler2x {
    coefficients: [f32; 31],
    up: [f32; 31],
    down: [f32; 31],
}

impl Default for Oversampler2x {
    fn default() -> Self {
        const K_TAPS: usize = 31;
        const K_CENTER: usize = K_TAPS / 2;
        const K_BETA: f64 = 8.0;

        fn bessel0(x: f64) -> f64 {
            let mut sum = 1.0;
            let mut term = 1.0;
            for k in 1..25 {
                term *= (x / (2.0 * k as f64)) * (x / (2.0 * k as f64));
                sum += term;
            }
            sum
        }

        let mut coefficients = [0.0f32; K_TAPS];
        let mut sum = 0.0;
        for i in 0..K_TAPS {
            let n = i as i32 - K_CENTER as i32;
            let sinc = if n == 0 {
                0.5
            } else {
                let n_f = n as f64;
                (PI as f64 * n_f / 2.0).sin() / (PI as f64 * n_f)
            };
            let r = n as f64 / K_CENTER as f64;
            let window = bessel0(K_BETA * (1.0 - r * r).max(0.0).sqrt()) / bessel0(K_BETA);
            coefficients[i] = (sinc * window) as f32;
            sum += sinc * window;
        }
        for c in &mut coefficients {
            *c /= sum as f32;
        }

        Self { coefficients, up: [0.0; K_TAPS], down: [0.0; K_TAPS] }
    }
}

impl Oversampler2x {
    pub fn clear(&mut self) {
        self.up.fill(0.0);
        self.down.fill(0.0);
    }

    /// One input sample to two output samples.
    pub fn up(&mut self, x: f32, out0: &mut f32, out1: &mut f32) {
        // Push 2*x
        self.up.rotate_right(1);
        self.up[0] = 2.0 * x;
        *out0 = self.filter(self.up);
        // Push 0
        self.up.rotate_right(1);
        self.up[0] = 0.0;
        *out1 = self.filter(self.up);
    }

    /// Two samples to one.
    pub fn down(&mut self, x0: f32, x1: f32) -> f32 {
        self.down.rotate_right(1);
        self.down[0] = x0;
        self.down.rotate_right(1);
        self.down[0] = x1;
        self.filter(self.down)
    }

    fn filter(&self, line: [f32; 31]) -> f32 {
        let mut y = 0.0;
        for i in 0..31 {
            y += self.coefficients[i] * line[i];
        }
        y
    }
}

/// Drive: tanh saturation at twice the rate with antiderivative
/// anti-aliasing (no fizz of folded harmonics at high drive), a bias for
/// even harmonics, DC removed, a tone low pass, and the level matched.
#[derive(Clone, Debug, Default)]
pub struct Drive {
    oversamplers: [Oversampler2x; 2],
    x1: [f32; 2],
    tone: [f32; 2],
    dc: [DcBlocker; 2],
}

impl Drive {
    pub fn clear(&mut self) {
        self.x1 = [0.0, 0.0];
        self.tone = [0.0, 0.0];
        for dc in &mut self.dc {
            dc.reset();
        }
        for os in &mut self.oversamplers {
            os.clear();
        }
    }

    pub fn process(&mut self, left: &mut f32, right: &mut f32, amount: f32, tone: f32, bias: f32, sample_rate: f32) {
        let gain = 40.0_f32.powf(amount);
        // Level matched: louder drive is not mistaken for better drive.
        let makeup = gain.powf(-0.3);
        let offset = 0.4 * bias;
        let dc_offset = offset.tanh();
        let cutoff = 1500.0 * 12.0_f32.powf(tone);
        let lp = 1.0 - (-TWO_PI * cutoff.min(0.45 * sample_rate) / sample_rate).exp();
        let channels = [left, right];
        for c in 0..2 {
            let mut a = 0.0;
            let mut b = 0.0;
            self.oversamplers[c].up(*channels[c] * gain, &mut a, &mut b);
            a = self.shape(c, a + offset) - dc_offset;
            b = self.shape(c, b + offset) - dc_offset;
            let mut y = self.oversamplers[c].down(a, b);
            y = self.dc[c].process(y, 0.9995);
            self.tone[c] += (y - self.tone[c]) * lp;
            *channels[c] = self.tone[c] * makeup;
        }
    }

    fn shape(&mut self, c: usize, x: f32) -> f32 {
        let dx = x - self.x1[c];
        let y =
            if dx.abs() < 1e-4 { (0.5 * (x + self.x1[c])).tanh() } else { (log_cosh(x) - log_cosh(self.x1[c])) / dx };
        self.x1[c] = x;
        y
    }
}

fn log_cosh(x: f32) -> f32 {
    // log(cosh(x)) without overflow.
    let a = x.abs();
    a + (-2.0 * a).exp().ln_1p() - std::f32::consts::LN_2
}
