//! The v0.5 effects: Drive, Chorus, Ladder, Hall and Filter Echo (Rev Roll
//! is [`super::Roll`] played backwards). Written for DJ Sugar after the
//! designs named on each; no code is copied from those projects.

use std::f32::consts::{PI, TAU};

use super::{beats, glide, soft, tap, Ctx, Echo, Line, MAX_PARAMETERS};
use crate::dsp::Svf;

// ---------------------------------------------------------------------------
// Oversampling

/// Taps of the two half-band filters of the 4x oversampler: 48 to 96 kHz
/// (narrow transition, so long) and 96 to 192 kHz (wide, so short). Both are
/// 4k + 3 long, so their centre tap is odd and every other tap is zero.
const STAGE1_TAPS: usize = 51;
const STAGE2_TAPS: usize = 23;

/// A Kaiser-windowed half-band low-pass, `N` taps, gain 1 at DC. Kaiser's
/// beta 7.86 gives about 80 dB of stopband.
fn halfband<const N: usize>() -> [f32; N] {
    fn bessel_i0(x: f64) -> f64 {
        let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
        while term > 1e-12 * sum {
            term *= (x / (2.0 * k)) * (x / (2.0 * k));
            sum += term;
            k += 1.0;
        }
        sum
    }
    let beta = 7.86;
    let c = (N - 1) as f64 / 2.0;
    let mut h = [0.0f32; N];
    for (j, v) in h.iter_mut().enumerate() {
        let k = j as f64 - c;
        let sinc = if k == 0.0 { 0.5 } else { (std::f64::consts::PI * k / 2.0).sin() / (std::f64::consts::PI * k) };
        let r = k / c;
        let w = bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(beta);
        *v = (sinc * w) as f32;
    }
    h
}

/// A history of the last `N` samples, each written twice (`M` >= 2N), so
/// the newest `N` always sit together in memory and any of them is one
/// index away, with no wrapping.
#[derive(Clone, Copy)]
struct History<const N: usize, const M: usize> {
    buf: [f32; M],
    pos: usize,
}

impl<const N: usize, const M: usize> Default for History<N, M> {
    fn default() -> Self {
        assert!(M >= 2 * N);
        Self { buf: [0.0; M], pos: 0 }
    }
}

impl<const N: usize, const M: usize> History<N, M> {
    #[inline]
    fn push(&mut self, v: f32) {
        self.pos += 1;
        if self.pos == N {
            self.pos = 0;
        }
        self.buf[self.pos] = v;
        self.buf[self.pos + N] = v;
    }

    /// Sample `k` back (0 = the newest), for `k < N`.
    #[inline]
    fn back(&self, k: usize) -> f32 {
        self.buf[self.pos + N - k]
    }
}

/// One factor-of-two stage: interpolates one sample into two, and
/// decimates two back into one, through the same half-band filter.
#[derive(Clone, Copy)]
struct Stage<const N: usize, const M: usize> {
    h: [f32; N],
    up: History<N, M>,
    down: History<N, M>,
}

impl<const N: usize, const M: usize> Stage<N, M> {
    fn new() -> Self {
        Self { h: halfband::<N>(), up: History::default(), down: History::default() }
    }

    fn reset(&mut self) {
        self.up = History::default();
        self.down = History::default();
    }

    /// Two samples at twice the rate for `x`. Zero-stuffing and filtering
    /// with 2h: the even outputs use h's non-zero (even) taps, the odd ones
    /// only its centre, a plain delay.
    #[inline]
    fn up(&mut self, x: f32) -> [f32; 2] {
        self.up.push(x);
        let c = (N - 1) / 2;
        let mut a = 0.0;
        for j in (0..N).step_by(2) {
            a += self.h[j] * self.up.back(j / 2);
        }
        [2.0 * a, self.up.back((c - 1) / 2)]
    }

    /// One sample at half the rate from two.
    #[inline]
    fn down(&mut self, x: [f32; 2]) -> f32 {
        self.down.push(x[0]);
        self.down.push(x[1]);
        let c = (N - 1) / 2;
        let mut a = self.h[c] * self.down.back(c);
        for j in (0..N).step_by(2) {
            a += self.h[j] * self.down.back(j);
        }
        a
    }
}

/// 4x oversampling around a nonlinearity, one channel.
#[derive(Clone, Copy)]
struct Oversampler {
    s1: Stage<STAGE1_TAPS, { 2 * STAGE1_TAPS + 1 }>,
    s2: Stage<STAGE2_TAPS, { 2 * STAGE2_TAPS + 1 }>,
}

impl Oversampler {
    fn new() -> Self {
        Self { s1: Stage::new(), s2: Stage::new() }
    }

    fn reset(&mut self) {
        self.s1.reset();
        self.s2.reset();
    }

    #[inline]
    fn run(&mut self, x: f32, mut f: impl FnMut(f32) -> f32) -> f32 {
        let a = self.s1.up(x);
        let mut mid = [0.0; 2];
        for (m, v) in mid.iter_mut().zip(a) {
            let b = self.s2.up(v);
            *m = self.s2.down([f(b[0]), f(b[1])]);
        }
        self.s1.down(mid)
    }
}

/// A smooth saturator with no corner (unlike clipping), so its harmonics
/// fall away fast: x / sqrt(1 + x^2).
#[inline]
fn saturate(x: f32) -> f32 {
    x / (1.0 + x * x).sqrt()
}

/// [`saturate`] with first-order antiderivative anti-aliasing (Parker, Zavalishin et al., "Reducing the aliasing of nonlinear waveshaping
/// using continuous-time convolution", DAFx 2016): the average of the curve over the step from the last input to this one, which
/// damps the very high harmonics a hard-driven sine makes before they can fold back. `last` is the previous input.
#[inline]
fn saturate_adaa(x: f32, last: &mut f32) -> f32 {
    // The antiderivative of x / sqrt(1 + x^2).
    let big_f = |v: f32| (1.0 + v * v).sqrt();
    let dx = x - *last;
    let y = if dx.abs() > 1e-3 { (big_f(x) - big_f(*last)) / dx } else { saturate(0.5 * (x + *last)) };
    *last = x;
    y
}

// ---------------------------------------------------------------------------
// Drive

/// Overdrive: the sound pushed into a smooth saturator, 4x oversampled and antiderivative anti-aliased so the harmonics don't
/// fold back as aliasing, after the approach of Signalsmith Basics' Crunch (MIT). A little bias gives it even harmonics,
/// like a valve; a tone control rounds the top; the level is matched so turning the drive up adds grit, not volume.
pub(super) struct Drive {
    over: [Oversampler; 2],
    /// Gain into the saturator, and the tone low-pass coefficient, gliding.
    gain: f32,
    gain_target: f32,
    tone: f32,
    tone_target: f32,
    lp: [f32; 2],
    /// The saturator's last input on each channel, at the 4x rate.
    last: [f32; 2],
    /// DC blocker state (the bias makes some).
    dc_x: [f32; 2],
    dc_y: [f32; 2],
    dc_coef: f32,
    fresh: bool,
}

/// The saturator's bias, for even harmonics.
const DRIVE_BIAS: f32 = 0.2;

impl Drive {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            over: [Oversampler::new(); 2],
            gain: 1.0,
            gain_target: 1.0,
            tone: 1.0,
            tone_target: 1.0,
            lp: [0.0; 2],
            last: [DRIVE_BIAS; 2],
            dc_x: [0.0; 2],
            dc_y: [0.0; 2],
            dc_coef: 1.0 - TAU * 10.0 / sample_rate,
            fresh: true,
        }
    }

    pub(super) fn reset(&mut self) {
        for o in &mut self.over {
            o.reset();
        }
        self.lp = [0.0; 2];
        self.last = [DRIVE_BIAS; 2];
        self.dc_x = [0.0; 2];
        self.dc_y = [0.0; 2];
        self.fresh = true;
    }

    pub(super) fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Drive: 0 to 24 dB into the saturator (more turns a sine into a
        // square whose highest harmonics alias however it is done). Tone: a
        // 1.5 to 20 kHz low-pass.
        self.gain_target = 10f32.powf(1.2 * p[0]);
        let hz = 1_500.0 * (20_000.0f32 / 1_500.0).powf(p[1]);
        self.tone_target = 1.0 - (-TAU * hz.min(0.45 * ctx.sample_rate) / ctx.sample_rate).exp();
        if self.fresh {
            self.fresh = false;
            self.gain = self.gain_target;
            self.tone = self.tone_target;
        }
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.gain, self.gain_target);
        glide(&mut self.tone, self.tone_target);
        let g = self.gain;
        let bias = saturate(DRIVE_BIAS);
        // Matches a -10 dBFS sine's level at any drive.
        let makeup = (1.0 + (0.3 * g) * (0.3 * g)).sqrt() / g;
        let mut y = [0.0; 2];
        for c in 0..2 {
            let last = &mut self.last[c];
            let v = self.over[c].run(x[c] * g, |v| saturate_adaa(v + DRIVE_BIAS, last) - bias);
            // DC blocker, then tone.
            let d = v - self.dc_x[c] + self.dc_coef * self.dc_y[c];
            self.dc_x[c] = v;
            self.dc_y[c] = d;
            self.lp[c] += (d * makeup - self.lp[c]) * self.tone;
            y[c] = self.lp[c];
        }
        y
    }
}

// ---------------------------------------------------------------------------
// Chorus

const CHORUS_VOICES: usize = 3;
/// The voices' centre delay and the most they swing either way, in seconds.
const CHORUS_CENTRE: f32 = 0.014;
const CHORUS_SWING: f32 = 0.006;

/// Three slowly swung copies of the sound a few milliseconds late, spread
/// across the stereo field, after Signalsmith Basics' Chorus (MIT). Its
/// sweep is in beats and locks to the beat grid.
pub(super) struct Chorus {
    len: usize,
    write: usize,
    phase: f32,
    step: f32,
    /// Depth and stereo spread, gliding towards their targets.
    depth: f32,
    spread: f32,
    targets: [f32; 2],
    centre: f32,
    swing: f32,
    fresh: bool,
    unlocked: bool,
}

impl Chorus {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            len: ((CHORUS_CENTRE + CHORUS_SWING) * sample_rate) as usize + 8,
            write: 0,
            phase: 0.0,
            step: 0.0,
            depth: 0.0,
            spread: 0.0,
            targets: [0.0; 2],
            centre: CHORUS_CENTRE * sample_rate,
            swing: CHORUS_SWING * sample_rate,
            fresh: true,
            unlocked: true,
        }
    }

    /// Floats of delay memory it needs.
    pub(super) fn size(sample_rate: f32) -> usize {
        2 * Self::new(sample_rate).len
    }

    pub(super) fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        self.write = 0;
        self.phase = 0.0;
        self.fresh = true;
        self.unlocked = true;
    }

    pub(super) fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time: one sweep every 1 to 4 beats. Depth: how far the voices
        // swing and how loud they are. Spread: mono to wide.
        let cycle = beats(p[0], 0, 2);
        self.step = 1.0 / (cycle * ctx.beat_seconds * ctx.sample_rate);
        super::lock_to_beat(&mut self.phase, &mut self.step, &mut self.unlocked, ctx.beat, cycle);
        self.targets = [p[1], p[2]];
        if self.fresh {
            self.fresh = false;
            [self.depth, self.spread] = self.targets;
        }
    }

    #[inline]
    pub(super) fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.depth, self.targets[0]);
        glide(&mut self.spread, self.targets[1]);
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        l[self.write] = x[0];
        r[self.write] = x[1];
        self.write += 1;
        if self.write == self.len {
            self.write = 0;
        }
        let mut wet = [0.0f32; 2];
        // Each voice a third of a cycle on; the right side a quarter
        // further, more as the spread opens. Two sin_cos a frame, and the
        // voices by rotation.
        let (s0, c0) = (self.phase * TAU).sin_cos();
        let (s1, c1) = (self.phase * TAU + 0.25 * TAU * self.spread).sin_cos();
        const ROT: [(f32, f32); CHORUS_VOICES] = [(0.0, 1.0), (0.866_025_4, -0.5), (-0.866_025_4, -0.5)];
        let amount = self.swing * self.depth;
        for (sin, cos) in ROT {
            for (c, (buf, (s, co))) in [(&*l, (s0, c0)), (&*r, (s1, c1))].into_iter().enumerate() {
                // sin(a + b) = sin a cos b + cos a sin b.
                let lfo = s * cos + co * sin;
                wet[c] += tap(buf, self.write, self.centre + amount * lfo);
            }
        }
        let g = 0.5 * self.depth;
        // Spread also pans the voices apart: mid stays, sides grow.
        let mid = 0.5 * (wet[0] + wet[1]);
        let side = 0.5 * (wet[0] - wet[1]) * self.spread;
        [x[0] + (mid + side) * g, x[1] + (mid - side) * g]
    }
}

// ---------------------------------------------------------------------------
// Ladder

/// A Moog-style 4-pole low-pass ladder with resonance and a saturating input, in Zavalishin's
/// zero-delay-feedback form ("The Art of VA Filter Design"), so its cutoff can sweep fast
/// without zipper or instability. Modeled on the Moog ladder (Huovilainen's model).
pub(super) struct Ladder {
    sample_rate: f32,
    s: [[f32; 4]; 2],
    /// tan(pi fc / fs), resonance (0..4) and drive, gliding.
    g: f32,
    g_target: f32,
    k: f32,
    k_target: f32,
    drive: f32,
    drive_target: f32,
    fresh: bool,
}

impl Ladder {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            s: [[0.0; 4]; 2],
            g: 1.0,
            g_target: 1.0,
            k: 0.0,
            k_target: 0.0,
            drive: 1.0,
            drive_target: 1.0,
            fresh: true,
        }
    }

    pub(super) fn reset(&mut self) {
        self.s = [[0.0; 4]; 2];
        self.fresh = true;
    }

    pub(super) fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        // Cutoff: 60 Hz to 20 kHz. Resonance: up to just short of
        // self-oscillation. Drive: 0 to 18 dB into the ladder.
        let hz = (60.0 * (20_000.0f32 / 60.0).powf(p[0])).min(0.45 * self.sample_rate);
        self.g_target = (PI * hz / self.sample_rate).tan();
        self.k_target = 3.8 * p[1];
        self.drive_target = 10f32.powf(0.9 * p[2]);
        if self.fresh {
            self.fresh = false;
            self.g = self.g_target;
            self.k = self.k_target;
            self.drive = self.drive_target;
        }
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        // The cutoff glides as the filters' does, about 5 ms, but no more than about a
        // hundred-and-twentieth of an octave per sample (the whole range in about 20
        // ms): the resonance sweeping fast through the music would ring like a click.
        let step = (self.g_target - self.g) * 0.004;
        let most = self.g * 0.006;
        self.g += step.clamp(-most, most);
        glide(&mut self.k, self.k_target);
        glide(&mut self.drive, self.drive_target);
        let (g, k) = (self.g, self.k);
        let big_g = g / (1.0 + g);
        let g2 = big_g * big_g;
        let g4 = g2 * g2;
        // Resonance thins the bass; give some of it back.
        let makeup = (1.0 + 0.5 * k) / self.drive.sqrt();
        let mut y = [0.0; 2];
        for c in 0..2 {
            let s = &mut self.s[c];
            // The ladder's output as a linear function of its input u:
            // y4 = G^4 u + sum.
            let inv = 1.0 / (1.0 + g);
            let sum = (big_g * g2 * s[0] + g2 * s[1] + big_g * s[2] + s[3]) * inv;
            let u = (x[c] * self.drive - k * sum) / (1.0 + k * g4);
            let mut v = saturate(u);
            for z in s.iter_mut() {
                let a = (v - *z) * big_g;
                let out = a + *z;
                *z = out + a;
                v = out;
            }
            y[c] = v * makeup;
        }
        y
    }
}

// ---------------------------------------------------------------------------
// Hall

/// Delay lengths of the hall's eight lines at 48 kHz, in samples: spread
/// between about 30 and 95 ms and mutually prime, so their echoes never
/// line up.
const HALL_LINES: [usize; 8] = [1_433, 1_601, 1_867, 2_053, 2_399, 2_803, 3_259, 4_567];
/// The input diffusers' lengths at 48 kHz.
const HALL_DIFFUSERS: [usize; 4] = [223, 337, 509, 761];
/// Longest pre-delay, in seconds.
const HALL_PREDELAY: f32 = 0.12;

/// A concert hall: a feedback delay network of eight lines mixed by a Hadamard matrix, after
/// Signalsmith's "Let's write a reverb" design (MIT), with input diffusion, gentle modulation, a
/// pre-delay and damping. Bigger and smoother than the plate, for long blends and breakdowns.
pub(super) struct Hall {
    lines: [Line; 8],
    /// Each line's delay in samples (its buffer has room to swing past it).
    base: [f32; 8],
    diffusers: [Line; 4],
    pre: Line,
    /// Buffer space all of the above need.
    size: usize,
    scale: f32,
    damped: [f32; 8],
    lfo: f32,
    lfo_step: f32,
    /// Decay (RT60 in seconds), pre-delay (samples), damping coefficient
    /// and send, gliding towards `targets`.
    rt60: f32,
    predelay: f32,
    damping: f32,
    send: f32,
    targets: [f32; 4],
    /// Each line's feedback gain for the current decay.
    gains: [f32; 8],
    sample_rate: f32,
    fresh: bool,
}

impl Hall {
    pub(super) fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 48_000.0;
        let mut offset = 0;
        let mut place = |len: usize| {
            let len = ((len as f32 * scale) as usize).max(4);
            let line = Line { at: offset, len, pos: 0 };
            offset += len;
            line
        };
        // Every line gets room for the modulated ones to swing.
        let lines = HALL_LINES.map(|l| place(l + 64));
        let base = HALL_LINES.map(|l| (l as f32 * scale).floor());
        let diffusers = HALL_DIFFUSERS.map(&mut place);
        let pre = place((HALL_PREDELAY * 48_000.0) as usize + 8);
        Self {
            lines,
            base,
            diffusers,
            pre,
            size: offset,
            scale,
            damped: [0.0; 8],
            lfo: 0.0,
            lfo_step: 0.3 / sample_rate,
            rt60: 2.0,
            predelay: 1.0,
            damping: 0.0,
            send: 0.0,
            targets: [2.0, 1.0, 0.0, 0.0],
            gains: [0.0; 8],
            sample_rate,
            fresh: true,
        }
    }

    /// Floats of delay memory it needs.
    #[cfg(test)]
    fn size(sample_rate: f32) -> usize {
        Self::new(sample_rate).size
    }

    pub(super) fn reset(&mut self, mem: &mut [f32]) {
        mem[..self.size].fill(0.0);
        self.damped = [0.0; 8];
        self.fresh = true;
    }

    pub(super) fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        // Decay: 1 to 12 seconds. Pre-delay: 0 to 120 ms. Damping: how
        // fast the highs die (a 16 kHz to 2 kHz low-pass in the lines).
        let hz = 16_000.0 * (2_000.0f32 / 16_000.0).powf(p[2]);
        self.targets = [
            12f32.powf(p[0]),
            1.0 + p[1] * HALL_PREDELAY * self.sample_rate,
            1.0 - (-TAU * hz.min(0.45 * self.sample_rate) / self.sample_rate).exp(),
            p[3],
        ];
        if self.fresh {
            self.fresh = false;
            [self.rt60, self.predelay, self.damping, self.send] = self.targets;
        }
        // Gains follow the decay's glide closely enough per buffer.
        self.rt60 += (self.targets[0] - self.rt60) * 0.5;
        for (g, base) in self.gains.iter_mut().zip(self.base) {
            let seconds = base / self.sample_rate;
            *g = 10f32.powf(-3.0 * seconds / self.rt60);
        }
    }

    #[inline]
    pub(super) fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.predelay, self.targets[1]);
        glide(&mut self.damping, self.targets[2]);
        glide(&mut self.send, self.targets[3]);
        self.pre.write(mem, 0.5 * (x[0] + x[1]));
        let mut v = self.pre.read_frac(mem, self.predelay);
        for (d, g) in self.diffusers.iter_mut().zip([0.7, 0.65, 0.6, 0.55]) {
            v = d.allpass(mem, v, g);
        }
        self.lfo += self.lfo_step;
        if self.lfo >= 1.0 {
            self.lfo -= 1.0;
        }
        let (sin, cos) = (self.lfo * TAU).sin_cos();
        let swing = 24.0 * self.scale;
        // Read each line's output, damped.
        let mut o = [0.0f32; 8];
        for (i, line) in self.lines.iter().enumerate() {
            let base = self.base[i];
            let r = match i {
                3 => line.read_frac(mem, base + swing * (1.0 + sin)),
                6 => line.read_frac(mem, base + swing * (1.0 + cos)),
                _ => line.read(mem, base as usize),
            };
            self.damped[i] += (r - self.damped[i]) * self.damping;
            o[i] = self.damped[i] * self.gains[i];
        }
        // Stereo out: two different sign patterns over the lines.
        let left = o[0] - o[1] + o[2] + o[3] - o[4] + o[5] - o[6] + o[7];
        let right = o[0] + o[1] - o[2] + o[3] + o[4] - o[5] - o[6] + o[7];
        // Mix with an 8x8 Hadamard matrix (fast, energy-preserving).
        let mut h = o;
        let mut span = 1;
        while span < 8 {
            for i in (0..8).step_by(2 * span) {
                for j in i..i + span {
                    let (a, b) = (h[j], h[j + span]);
                    h[j] = a + b;
                    h[j + span] = a - b;
                }
            }
            span *= 2;
        }
        let norm = 1.0 / 8f32.sqrt();
        for (i, line) in self.lines.iter_mut().enumerate() {
            // The input enters every line, alternating in sign.
            let input = if i % 2 == 0 { v } else { -v };
            line.write(mem, soft(h[i] * norm + input * 0.35));
        }
        // About as loud as the plate at the same send.
        let g = 1.05 * self.send;
        [x[0] + left * g, x[1] + right * g]
    }
}

// ---------------------------------------------------------------------------
// Filter Echo

/// An echo whose repeats run through a filter: turned left, each repeat is darker (low-pass);
/// turned right, thinner (high-pass), as Pioneer's LPF and HPF Echo do. Its meta knob opens
/// the send and sweeps the high-pass up together, for a washed-out exit.
pub(super) struct FilterEcho {
    pub(super) echo: Echo,
    lp: Svf,
    hp: Svf,
    /// How much of each filter is in the loop, gliding.
    mix: [f32; 2],
    mix_target: [f32; 2],
    sample_rate: f32,
}

impl FilterEcho {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            echo: Echo::new(sample_rate),
            lp: Svf::default(),
            hp: Svf::default(),
            mix: [0.0; 2],
            mix_target: [0.0; 2],
            sample_rate,
        }
    }

    pub(super) fn reset(&mut self, mem: &mut [f32]) {
        self.echo.reset(mem);
        self.lp = Svf::default();
        self.hp = Svf::default();
        self.mix = self.mix_target;
    }

    pub(super) fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time and feedback as Echo; Filter: 0.5 is none, left a low-pass
        // down to 300 Hz, right a high-pass up to 3 kHz; send.
        let side = p[2] - 0.5;
        let lp = 300.0 * (20_000.0f32 / 300.0).powf((1.0 + 2.0 * side).min(1.0));
        let hp = 20.0 * (3_000.0f32 / 20.0).powf((2.0 * side).max(0.0));
        self.lp.set(self.sample_rate, lp, 0.9);
        self.hp.set(self.sample_rate, hp, 0.9);
        let amount = |v: f32| (v * 20.0).clamp(0.0, 1.0);
        self.mix_target = [amount(-side), amount(side)];
        self.echo.prepare(&[p[0], p[1], 0.0, p[3]], ctx);
    }

    #[inline]
    pub(super) fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        for (m, t) in self.mix.iter_mut().zip(self.mix_target) {
            glide(m, t);
        }
        let (lp, hp, mix) = (&mut self.lp, &mut self.hp, self.mix);
        self.echo.process_with(mem, x, |v| {
            let a = lp.process(v)[0];
            let b = hp.process(v)[2];
            let mut y = v;
            for c in 0..2 {
                y[c] += (a[c] - v[c]) * mix[0] + (b[c] - v[c]) * mix[1];
            }
            y
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oversampler_passes_the_audio_band_flat() {
        // A 1 kHz and a 15 kHz tone through the oversampler with nothing
        // in the middle come out at the same level, only delayed.
        for hz in [1_000.0f32, 15_000.0] {
            let mut o = Oversampler::new();
            let n = 48_000;
            let out: Vec<f32> = (0..n).map(|i| o.run((TAU * hz * i as f32 / 48_000.0).sin(), |v| v)).collect();
            let peak = out[1_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!((peak - 1.0).abs() < 0.03, "{hz} Hz came out at {peak}");
        }
    }

    #[test]
    fn drive_does_not_alias() {
        // A 5 kHz sine at full drive: everything audible that isn't one of
        // its harmonics (aliasing folded back from above Nyquist) stays
        // 60 dB under the fundamental.
        use rustfft::{num_complex::Complex, FftPlanner};
        let sr = 48_000.0;
        let mut d = Drive::new(sr);
        d.prepare(&[1.0, 1.0, 0.0, 0.0], Ctx { sample_rate: sr, beat_seconds: 0.5, beat: None });
        let n = 16_384;
        let skip = 4_096;
        // A frequency that falls exactly on an FFT bin, near 5 kHz.
        let bin = (5_000.0 * n as f32 / sr).round();
        let hz = bin * sr / n as f32;
        let out: Vec<f32> = (0..skip + n)
            .map(|i| {
                let x = (TAU * hz * i as f32 / sr).sin() * 0.9;
                d.process([x, x])[0]
            })
            .collect();
        let mut buf: Vec<Complex<f32>> = out[skip..]
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                // Blackman-Harris window keeps leakage far below 60 dB.
                let t = TAU * i as f32 / n as f32;
                let w = 0.35875 - 0.48829 * t.cos() + 0.14128 * (2.0 * t).cos() - 0.01168 * (3.0 * t).cos();
                Complex::new(v * w, 0.0)
            })
            .collect();
        FftPlanner::new().plan_fft_forward(n).process(&mut buf);
        let mag: Vec<f32> = buf[..n / 2].iter().map(|c| c.norm()).collect();
        let f0 = bin as usize;
        let fundamental = mag[f0 - 3..=f0 + 3].iter().fold(0.0f32, |m, &v| m.max(v));
        let mut worst = 0.0f32;
        // Up to 20 kHz: what can be heard. (Between 20 and 24 kHz the
        // half-band filters' transition lets a little through, unheard.)
        let audible = (20_000.0 * n as f32 / sr) as usize;
        for (k, &m) in mag.iter().enumerate().take(audible).skip(8) {
            // Skip the harmonics (and the window's main lobe around them).
            let h = (k as f32 / f0 as f32).round() as usize;
            if h >= 1 && (k as isize - (h * f0) as isize).abs() <= 4 {
                continue;
            }
            worst = worst.max(m);
        }
        let db = 20.0 * (worst / fundamental).log10();
        assert!(db < -60.0, "aliasing at {db:.1} dB");
    }

    #[test]
    fn the_hall_fits_in_a_long_buffer() {
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            assert!(Hall::size(sr) <= super::super::long_size(sr));
            assert!(Chorus::size(sr) <= super::super::short_size(sr));
        }
    }
}
