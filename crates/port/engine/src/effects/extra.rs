//! The v0.5 effects: Drive, Chorus, Ladder, Hall and Filter Echo (Rev Roll
//! is [`super::Roll`] played backwards). Written for DJ Sugar after the
//! designs named on each; no code is copied from those projects.

use std::f32::consts::{PI, TAU};

use super::{beats, glide, soft, tap, Ctx, Echo, Line, KNOBS};
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

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
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

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
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

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS]) {
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

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS]) {
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

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
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

// ---------------------------------------------------------------------------
// Tremolo and Autopan

/// A beat-locked sweep from 0 to 1, shared by the two effects below.
#[derive(Default)]
struct Sweep {
    phase: f32,
    step: f32,
    locked: bool,
}

impl Sweep {
    fn prepare(&mut self, cycle: f32, ctx: Ctx) {
        self.step = 1.0 / (cycle * ctx.beat_seconds * ctx.sample_rate);
        let mut fresh = !self.locked;
        super::lock_to_beat(&mut self.phase, &mut self.step, &mut fresh, ctx.beat, cycle);
        self.locked = true;
    }

    #[inline]
    fn tick(&mut self) -> f32 {
        let at = self.phase;
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        at
    }
}

/// The level pulsing in time: a sine, a pump that ducks on the beat and recovers, or a gate. Rate is 1/16 to 4 beats,
/// Width how much of the cycle the pump takes to recover or the gate stays open.
#[derive(Default)]
pub(super) struct Tremolo {
    sweep: Sweep,
    depth: f32,
    depth_target: f32,
    shape: u8,
    width: f32,
    gain: f32,
}

impl Tremolo {
    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
        if !self.sweep.locked {
            self.depth = p[1];
            self.gain = 1.0;
        }
        self.sweep.prepare(beats(p[0], -4, 2), ctx);
        self.depth_target = p[1];
        self.shape = (p[2] * 2.0).round() as u8;
        self.width = p[3].clamp(0.05, 0.95);
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let phase = self.sweep.tick();
        let open = match self.shape {
            0 => 0.5 + 0.5 * (phase * TAU).cos(),
            1 => {
                let t = (phase / self.width).min(1.0);
                1.0 - (1.0 - t) * (1.0 - t)
            }
            _ => f32::from(u8::from(phase < self.width)),
        };
        glide(&mut self.depth, self.depth_target);
        glide(&mut self.gain, 1.0 - self.depth * (1.0 - open));
        [x[0] * self.gain, x[1] * self.gain]
    }
}

/// The sound swung from side to side in time. Period is 1/4 to 4 beats, Width how far it goes, Smoothing how round the
/// swing is: none jumps from one side to the other.
#[derive(Default)]
pub(super) struct Autopan {
    sweep: Sweep,
    width: f32,
    width_target: f32,
    smoothing: f32,
    pan: f32,
}

impl Autopan {
    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
        if !self.sweep.locked {
            self.width = p[1];
        }
        self.sweep.prepare(beats(p[0], -2, 2), ctx);
        self.width_target = p[1];
        self.smoothing = p[2].clamp(0.0, 1.0);
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let sine = (self.sweep.tick() * TAU).sin();
        // Less smoothing squares the swing off, towards a hard left and right.
        let sharp = 1.0 + 24.0 * (1.0 - self.smoothing) * (1.0 - self.smoothing);
        glide(&mut self.width, self.width_target);
        glide(&mut self.pan, (sine * sharp).clamp(-1.0, 1.0) * self.width);
        [x[0] * (1.0 - self.pan.max(0.0)), x[1] * (1.0 + self.pan.min(0.0))]
    }
}

/// An echo off a worn tape: its speed wanders slowly (wow) and shivers (flutter), so the repeats drift in pitch, and
/// each one comes back duller and with less bass. Age is how worn: more wander, a lower top.
pub(super) struct TapeEcho {
    echo: Echo,
    /// The delay asked for, before the tape's wander.
    base: f32,
    wow: f32,
    flutter: f32,
    age: f32,
    age_target: f32,
    /// What each side's top and bottom filters hold.
    top: [f32; 2],
    bottom: [f32; 2],
    sample_rate: f32,
}

/// Cycles a second of the tape's slow wander and of its shiver.
const WOW: f32 = 0.55;
const FLUTTER: f32 = 6.8;

impl TapeEcho {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            echo: Echo::new(sample_rate),
            base: 1.0,
            wow: 0.0,
            flutter: 0.0,
            age: 0.0,
            age_target: 0.0,
            top: [0.0; 2],
            bottom: [0.0; 2],
            sample_rate,
        }
    }

    pub(super) fn reset(&mut self, mem: &mut [f32]) {
        self.echo.reset(mem);
        self.top = [0.0; 2];
        self.bottom = [0.0; 2];
        self.age = self.age_target;
    }

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
        self.echo.set_time(beats(p[0], -4, 2), ctx);
        self.base = self.echo.target;
        // A little over one, so a full knob builds up into the tape's saturation instead of dying away.
        self.echo.feedback_target = p[1] * 1.05;
        self.echo.pingpong_target = 0.0;
        self.echo.send_target = p[3] * 0.75;
        self.echo.settle();
        self.age_target = p[2].clamp(0.0, 1.0);
    }

    #[inline]
    pub(super) fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.age, self.age_target);
        self.wow = (self.wow + WOW / self.sample_rate).fract();
        self.flutter = (self.flutter + FLUTTER / self.sample_rate).fract();
        // Up to 4.4 ms of slow wander, always later than asked, and half a millisecond of shiver about it.
        let wander = 2.2 * (1.0 + (self.wow * TAU).sin()) + 0.5 * (self.flutter * TAU).sin();
        let late = self.age * wander * 0.001 * self.sample_rate;
        self.echo.target = (self.base + late).clamp(1.0, (self.echo.len - 2) as f32);
        // The top comes down from 14 kHz to 2.5 kHz as the tape ages; under 90 Hz goes either way.
        let top = 1.0 - (-TAU * 14_000.0 * (2_500.0f32 / 14_000.0).powf(self.age) / self.sample_rate).exp();
        let bottom = 1.0 - (-TAU * 90.0 / self.sample_rate).exp();
        let (tops, bottoms) = (&mut self.top, &mut self.bottom);
        self.echo.process_with(mem, x, |v| {
            let mut y = v;
            for c in 0..2 {
                tops[c] += (v[c] - tops[c]) * top;
                bottoms[c] += (tops[c] - bottoms[c]) * bottom;
                y[c] = tops[c] - bottoms[c];
            }
            y
        })
    }
}

/// What a step of the glitch plays.
#[derive(Clone, Copy, Default, PartialEq)]
enum Cut {
    /// The sound as it comes.
    #[default]
    Live,
    Again,
    Backwards,
    HalfSpeed,
}

/// A stretch of what was heard, being played back.
#[derive(Clone, Copy, Default)]
struct Voice {
    cut: Cut,
    /// The frame, counted since the effect came on, the stretch starts at, and how far into it the voice is.
    start: u64,
    at: f32,
}

/// Seconds one step's sound takes to give way to the next's.
const GLITCH_FADE: f32 = 0.003;

/// On each step of the beat, by chance, the step before is played again instead of the music: as it was, backwards or
/// at half speed. Step is 1/16 to 1 beat; Reverse and Half Speed are the share of glitches played those ways.
pub(super) struct Glitch {
    len: usize,
    written: u64,
    /// Where this step and the one before it started.
    step: u64,
    before: u64,
    frames: usize,
    now: Voice,
    old: Voice,
    fading: usize,
    fade: usize,
    sweep: Sweep,
    last: f32,
    chance: f32,
    reverse: f32,
    half: f32,
    seed: u32,
}

impl Glitch {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            len: Echo::new(sample_rate).len,
            written: 0,
            step: 0,
            before: 0,
            frames: 64,
            now: Voice::default(),
            old: Voice::default(),
            fading: 0,
            fade: ((GLITCH_FADE * sample_rate) as usize).max(1),
            sweep: Sweep::default(),
            last: 0.0,
            chance: 0.0,
            reverse: 0.0,
            half: 0.0,
            seed: 0x6117_C4A5,
        }
    }

    pub(super) fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        let (len, fade, seed) = (self.len, self.fade, self.seed);
        *self = Self { len, fade, seed, ..Self::new(48_000.0) };
    }

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
        let step = beats(p[0], -4, 0);
        self.sweep.prepare(step, ctx);
        self.frames = ((step * ctx.beat_seconds * ctx.sample_rate) as usize).clamp(64, self.len / 2);
        self.chance = p[1];
        self.reverse = p[2];
        self.half = p[3];
    }

    /// A number from 0 up to 1, the same run of them every time.
    fn roll(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// What `voice` plays now, or `live` when it plays the music or its stretch is no longer kept.
    fn read(&self, mem: &[f32], voice: Voice, live: [f32; 2]) -> [f32; 2] {
        let last = (self.frames - 1) as f32;
        let into = match voice.cut {
            Cut::Live => return live,
            Cut::Backwards => last - voice.at,
            Cut::Again | Cut::HalfSpeed => voice.at,
        }
        .clamp(0.0, last);
        let frame = voice.start + into as u64;
        if frame >= self.written || frame + self.len as u64 <= self.written {
            return live;
        }
        let next = (frame + 1).min(self.written - 1);
        let t = into.fract();
        let (a, b) = ((frame % self.len as u64) as usize, (next % self.len as u64) as usize);
        [mem[a] + (mem[b] - mem[a]) * t, mem[self.len + a] + (mem[self.len + b] - mem[self.len + a]) * t]
    }

    /// A new step: what it plays is picked, and what was playing fades out under it.
    fn pick(&mut self) {
        self.before = self.step;
        self.step = self.written - 1;
        let mut next = Voice::default();
        // A whole step has to have gone by to have one to play.
        if (self.step - self.before) as usize >= self.frames / 2 && self.roll() < self.chance {
            let way = self.roll();
            next.cut = if way < self.reverse {
                Cut::Backwards
            } else if way < self.reverse + self.half {
                Cut::HalfSpeed
            } else {
                Cut::Again
            };
            // Half the time a glitch keeps hold of the stretch the last one played, so it stutters.
            let stay = self.now.cut != Cut::Live && self.roll() < 0.5;
            next.start = if stay { self.now.start } else { self.before };
        }
        if next.cut != Cut::Live || self.now.cut != Cut::Live {
            self.old = self.now;
            self.fading = self.fade;
        }
        self.now = next;
    }

    #[inline]
    pub(super) fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let slot = (self.written % self.len as u64) as usize;
        mem[slot] = x[0];
        mem[self.len + slot] = x[1];
        self.written += 1;
        let phase = self.sweep.tick();
        if phase < self.last {
            self.pick();
        }
        self.last = phase;
        let mut y = self.read(mem, self.now, x);
        if self.fading > 0 {
            let t = self.fading as f32 / self.fade as f32 * PI * 0.5;
            let old = self.read(mem, self.old, x);
            y = [old[0] * t.sin() + y[0] * t.cos(), old[1] * t.sin() + y[1] * t.cos()];
            self.old.at += if self.old.cut == Cut::HalfSpeed { 0.5 } else { 1.0 };
            self.fading -= 1;
        }
        self.now.at += if self.now.cut == Cut::HalfSpeed { 0.5 } else { 1.0 };
        y
    }
}

/// The share of a back spin the hand takes to catch the record and throw it back.
const CATCH: f32 = 0.08;
/// How far back a spin goes, in its own lengths, for each unit of its top speed.
const BACK: f32 = (1.0 - CATCH) / 3.0 + CATCH / 2.0;
/// The fastest a record is thrown back, in times its playing speed.
const THROW: f32 = 4.0;

/// How fast the record turns `t` of the way through a back spin whose top speed is `peak`: forward, through a stop as
/// the hand catches it, back at `peak`, then coasting down to a stop.
fn spin(t: f32, peak: f32) -> f32 {
    if t >= 1.0 {
        0.0
    } else if t < CATCH {
        let u = t / CATCH;
        1.0 - (1.0 + peak) * u * u * (3.0 - 2.0 * u)
    } else {
        let u = (t - CATCH) / (1.0 - CATCH);
        -peak * (1.0 - u) * (1.0 - u)
    }
}

/// A back spin: from the next beat the record is thrown backwards and coasts to a stop, over 1/4 to 8 beats, and stays
/// silent until the effect is turned off. It spins back through what it heard since it was turned on.
pub(super) struct Backspin {
    len: usize,
    written: usize,
    spinning: bool,
    at: f32,
    elapsed: f32,
    frames: f32,
    peak: f32,
    beat: f32,
    sweep: Sweep,
    last: f32,
    mix: f32,
    mix_target: f32,
}

impl Backspin {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            len: Echo::new(sample_rate).len,
            written: 0,
            spinning: false,
            at: 0.0,
            elapsed: 0.0,
            frames: 1.0,
            peak: 0.0,
            beat: 1.0,
            sweep: Sweep::default(),
            last: 0.0,
            mix: 1.0,
            mix_target: 1.0,
        }
    }

    pub(super) fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        let (len, mix) = (self.len, self.mix_target);
        *self = Self { len, mix, mix_target: mix, ..Self::new(48_000.0) };
    }

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
        self.sweep.prepare(1.0, ctx);
        self.beat = ctx.beat_seconds * ctx.sample_rate;
        self.frames = (beats(p[0], -2, 3) * self.beat).clamp(256.0, self.len as f32);
        self.mix_target = p[1].clamp(0.0, 1.0);
    }

    /// The sound `at` frames into what was heard, on a curve through the frames either side.
    fn read(&self, mem: &[f32]) -> [f32; 2] {
        let at = self.at.clamp(1.0, (self.written.max(4) - 3) as f32);
        let (i, t) = (at as usize, at.fract());
        std::array::from_fn(|c| {
            let m = &mem[c * self.len..];
            let (p0, p1, p2, p3) = (m[i - 1], m[i], m[i + 1], m[i + 2]);
            let c1 = 0.5 * (p2 - p0);
            let c2 = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
            let c3 = 0.5 * (p3 - p0) + 1.5 * (p1 - p2);
            ((c3 * t + c2) * t + c1) * t + p1
        })
    }

    #[inline]
    pub(super) fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.mix, self.mix_target);
        let phase = self.sweep.tick();
        if !self.spinning {
            if self.written < self.len {
                mem[self.written] = x[0];
                mem[self.len + self.written] = x[1];
                self.written += 1;
            }
            // Thrown on the beat, or once the memory is full when no beat came.
            let on_beat = phase < self.last && self.written as f32 > 0.25 * self.beat;
            if (on_beat || self.written == self.len) && self.written > 4 {
                self.spinning = true;
                self.at = (self.written - 2) as f32;
                // No faster than what was heard reaches back for.
                self.peak = ((self.at / self.frames + CATCH / 2.0) / BACK).clamp(0.0, THROW);
            }
            self.last = phase;
            return x;
        }
        let speed = spin((self.elapsed / self.frames).min(1.0), self.peak);
        let y = self.read(mem);
        // A record turning slower than it plays is quieter, and silent when still.
        let level = speed.abs().min(1.0);
        self.at += speed;
        self.elapsed += 1.0;
        [x[0] + (y[0] * level - x[0]) * self.mix, x[1] + (y[1] * level - x[1]) * self.mix]
    }
}

/// Semitones the pitch knob reaches either way.
const SHIFT: f32 = 12.0;

/// Moves the pitch up to an octave either way without changing the tempo; the knob's middle is no shift. Two heads
/// read the last moments faster or slower than they were written, each fading out as it runs out of them while the
/// other takes over. Grain is how long a head reads, 25 to 100 ms: short for drums, long for voices and pads.
pub(super) struct PitchShift {
    /// Frames the line holds for each side, and where the next is written.
    len: usize,
    write: usize,
    phase: f32,
    window: f32,
    semitones: f32,
    semitones_target: f32,
    mix: f32,
    mix_target: f32,
    sample_rate: f32,
    fresh: bool,
}

impl PitchShift {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            len: (0.1 * sample_rate) as usize + 8,
            write: 0,
            phase: 0.0,
            window: 0.0625 * sample_rate,
            semitones: 0.0,
            semitones_target: 0.0,
            mix: 1.0,
            mix_target: 1.0,
            sample_rate,
            fresh: true,
        }
    }

    pub(super) fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        *self = Self::new(self.sample_rate);
    }

    pub(super) fn prepare(&mut self, p: &[f32; KNOBS]) {
        self.semitones_target = (p[0].clamp(0.0, 1.0) * 2.0 - 1.0) * SHIFT;
        self.mix_target = p[1].clamp(0.0, 1.0);
        self.window = ((0.025 + 0.075 * p[2].clamp(0.0, 1.0)) * self.sample_rate).clamp(64.0, (self.len - 4) as f32);
        if self.fresh {
            self.fresh = false;
            self.semitones = self.semitones_target;
            self.mix = self.mix_target;
        }
    }

    /// One side's sound `back` frames ago, on a curve through the frames either side.
    fn read(&self, side: &[f32], back: f32) -> f32 {
        let at = (self.write + 2 * self.len) as f32 - 1.0 - back;
        let (i, t) = (at as usize, at.fract());
        let s = |k: usize| side[(i + k - 1) % self.len];
        let (p0, p1, p2, p3) = (s(0), s(1), s(2), s(3));
        let c1 = 0.5 * (p2 - p0);
        let c2 = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
        let c3 = 0.5 * (p3 - p0) + 1.5 * (p1 - p2);
        ((c3 * t + c2) * t + c1) * t + p1
    }

    #[inline]
    pub(super) fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.semitones, self.semitones_target);
        glide(&mut self.mix, self.mix_target);
        mem[self.write] = x[0];
        mem[self.len + self.write] = x[1];
        self.write = (self.write + 1) % self.len;
        // The heads move at the ratio while the writing moves at one, so how far back they read changes by the rest.
        let ratio = (self.semitones / 12.0).exp2();
        self.phase = (self.phase + (1.0 - ratio) / self.window).rem_euclid(1.0);
        let other = (self.phase + 0.5).fract();
        let near = 0.5 - 0.5 * (TAU * self.phase).cos();
        // Within a quarter semitone of the middle the shifter fades away, so the middle is the sound untouched.
        let wet = self.mix * (self.semitones.abs() * 4.0).min(1.0);
        std::array::from_fn(|c| {
            let side = &mem[c * self.len..(c + 1) * self.len];
            let y = near * self.read(side, 2.0 + self.phase * self.window)
                + (1.0 - near) * self.read(side, 2.0 + other * self.window);
            x[c] + (y - x[c]) * wet
        })
    }
}

/// Decibels the knee is wide.
const KNEE: f32 = 6.0;

/// How far a level is turned down, in decibels (never above 0): nothing below the knee, `1 - 1/ratio` of what is over
/// the threshold above it, and a curve between the two across the knee.
fn reduction(level: f32, threshold: f32, ratio: f32) -> f32 {
    let over = level - threshold;
    let slope = 1.0 / ratio - 1.0;
    if 2.0 * over < -KNEE {
        0.0
    } else if 2.0 * over.abs() <= KNEE {
        let x = over + KNEE * 0.5;
        slope * x * x / (2.0 * KNEE)
    } else {
        slope * over
    }
}

/// What a one-pole smoother keeps each frame to settle in `seconds`.
fn pole(seconds: f32, sample_rate: f32) -> f32 {
    (-1.0 / (seconds * sample_rate).max(1.0)).exp()
}

/// A glue compressor on one knob. Amount lowers the threshold from 0 to -30 dB and raises the ratio from 1.5 to 10
/// together; the loudness lost is made up. Attack is 0.1 to 100 ms, Release 20 ms to 1.5 s, Mix blends the dry sound in.
pub(super) struct Compressor {
    amount: f32,
    amount_target: f32,
    mix: f32,
    mix_target: f32,
    attack: f32,
    release: f32,
    slow: f32,
    /// The reduction now, and a slower follower of it that holds through loud stretches so they do not pump.
    envelope: f32,
    held: f32,
    fresh: bool,
}

impl Default for Compressor {
    fn default() -> Self {
        Self {
            amount: 0.0,
            amount_target: 0.0,
            mix: 1.0,
            mix_target: 1.0,
            attack: 0.0,
            release: 0.0,
            slow: 0.0,
            envelope: 0.0,
            held: 0.0,
            fresh: true,
        }
    }
}

impl Compressor {
    pub(super) fn prepare(&mut self, p: &[f32; KNOBS], ctx: Ctx) {
        self.amount_target = p[0].clamp(0.0, 1.0);
        self.mix_target = p[3].clamp(0.0, 1.0);
        if self.fresh {
            self.fresh = false;
            self.amount = self.amount_target;
            self.mix = self.mix_target;
        }
        let attack = 0.0001 * 1000f32.powf(p[1].clamp(0.0, 1.0));
        let release = 0.02 * 75f32.powf(p[2].clamp(0.0, 1.0));
        self.attack = pole(attack, ctx.sample_rate);
        self.release = pole(release, ctx.sample_rate);
        self.slow = pole(release * 4.0, ctx.sample_rate);
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.amount, self.amount_target);
        glide(&mut self.mix, self.mix_target);
        let threshold = -30.0 * self.amount;
        let ratio = 1.5 + 8.5 * self.amount * self.amount;
        let peak = x[0].abs().max(x[1].abs()).max(1e-6);
        let want = reduction(20.0 * peak.log10(), threshold, ratio);
        // More reduction comes on at the attack's pace, less at the release's.
        let keep = if want < self.envelope { self.attack } else { self.release };
        self.envelope = want + keep * (self.envelope - want);
        self.held = self.envelope.min(self.envelope + self.slow * (self.held - self.envelope));
        // Two fifths of what a full-scale peak loses is given back, which keeps the loudness without clipping.
        let makeup = -0.4 * reduction(0.0, threshold, ratio);
        let gain = 10f32.powf((0.5 * (self.envelope + self.held) + makeup) / 20.0);
        let wet = 1.0 + (gain - 1.0) * self.mix;
        [x[0] * wet, x[1] * wet]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx { sample_rate: 48_000.0, beat_seconds: 0.5, beat: Some(0.0) }
    }

    #[test]
    fn tremolo_pulses_once_a_cycle_and_never_adds_level() {
        let mut t = Tremolo::default();
        // One beat a cycle, full depth, gate shape, half open.
        t.prepare(&[0.67, 1.0, 1.0, 0.5], ctx());
        let out: Vec<f32> = (0..24_000).map(|_| t.process([1.0, 1.0])[0]).collect();
        assert!(out.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(out[6_000] > 0.95, "open in the first half: {}", out[6_000]);
        assert!(out[20_000] < 0.05, "shut in the second: {}", out[20_000]);
    }

    /// Four seconds of a rising ramp through the glitch at a quarter-beat step, 6 000 frames: what went in and what
    /// came out.
    fn glitched(chance: f32, reverse: f32, half: f32) -> Vec<(f32, f32)> {
        let mut g = Glitch::new(48_000.0);
        let mut mem = vec![0.0; super::super::long_size(48_000.0)];
        g.reset(&mut mem);
        g.prepare(&[0.5, chance, reverse, half], ctx());
        (0..192_000).map(|i| (i as f32, g.process(&mut mem, [i as f32, -(i as f32)])[0])).collect()
    }

    #[test]
    fn glitch_plays_earlier_steps_again_and_nothing_else() {
        assert!(glitched(0.0, 0.0, 0.0).iter().all(|(x, y)| x == y), "no chance, no change");
        // Every step glitched, none backwards or slow: away from the joins each frame is the one a whole number of
        // steps back.
        let again = glitched(1.0, 0.0, 0.0);
        for step in 2..32 {
            let (x, y) = again[step * 6_000 + 3_000];
            let back = (x - y) / 6_000.0;
            // The steps are counted in a float, so one may start a frame early or late.
            assert!(back > 0.99 && (back - back.round()).abs() < 0.001, "step {step}: {back} steps back");
        }
        // Backwards, the ramp falls; at half speed it climbs half as fast.
        let rev = glitched(1.0, 1.0, 0.0);
        assert_eq!(rev[15_001].1 - rev[15_000].1, -1.0);
        let slow = glitched(1.0, 0.0, 1.0);
        assert_eq!(slow[15_002].1 - slow[15_000].1, 1.0);
        // Only ever sound that came in; where two steps cross over, both are heard.
        assert!(rev.iter().chain(&slow).all(|(x, y)| (-0.01..=*x * 1.42).contains(y)));
    }

    /// Times a second a 440 Hz tone crosses zero going up once shifted by the pitch knob at `knob`.
    fn shifted(knob: f32) -> f32 {
        let mut s = PitchShift::new(48_000.0);
        let mut mem = vec![0.0; super::super::long_size(48_000.0)];
        s.reset(&mut mem);
        s.prepare(&[knob, 1.0, 0.5, 0.0]);
        let out: Vec<f32> =
            (0..96_000).map(|i| s.process(&mut mem, [(i as f32 * 440.0 * TAU / 48_000.0).sin(); 2])[0]).collect();
        out[48_000..].windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as f32
    }

    #[test]
    fn pitch_shift_moves_a_tone_an_octave_either_way_and_leaves_the_middle_alone() {
        // A crossing may fall on either edge of the second counted.
        assert!((shifted(0.5) - 440.0).abs() <= 1.0);
        let (up, down) = (shifted(1.0), shifted(0.0));
        assert!((up - 880.0).abs() < 30.0, "an octave up: {up}");
        assert!((down - 220.0).abs() < 15.0, "an octave down: {down}");
        let fifth = shifted(0.5 + 7.0 / 24.0);
        assert!((fifth - 659.3).abs() < 25.0, "a fifth up: {fifth}");
    }

    #[test]
    fn backspin_throws_the_record_back_on_the_beat_and_stops() {
        let mut b = Backspin::new(48_000.0);
        let mut mem = vec![0.0; super::super::long_size(48_000.0)];
        b.reset(&mut mem);
        // Time two fifths up is one beat, 24 000 frames; the ramp says where in the sound each frame comes from.
        b.prepare(&[0.4, 1.0, 0.0, 0.0], ctx());
        let out: Vec<f32> = (0..96_000).map(|i| b.process(&mut mem, [i as f32, i as f32])[0]).collect();
        let thrown = out.iter().zip(0..).position(|(y, i)| *y != i as f32).unwrap();
        assert!((23_990..=24_010).contains(&thrown), "thrown at {thrown}");
        // Soon after the catch it runs backwards, faster than it played.
        let fast = out[thrown + 4_000] - out[thrown + 4_001];
        assert!(fast > 1.5, "{fast} frames back each frame");
        assert!(out[thrown..].iter().all(|y| (0.0..=thrown as f32 * 1.01).contains(y)), "only what was heard");
        // A beat later the record has stopped, and stays so.
        assert!(out[thrown + 24_100..].iter().all(|y| *y == 0.0));
        // The curve itself: playing speed, through a stop, the top speed back, then still.
        assert_eq!(spin(0.0, 3.0), 1.0);
        assert!((spin(CATCH, 3.0) - -3.0).abs() < 1e-5);
        assert_eq!(spin(1.0, 3.0), 0.0);
    }

    /// What a click leaves behind it: `seconds` of the tape echo's repeats alone, at a beat's delay.
    fn tape(feedback: f32, age: f32, seconds: usize) -> Vec<f32> {
        let mut t = TapeEcho::new(48_000.0);
        let mut mem = vec![0.0; super::super::long_size(48_000.0)];
        t.reset(&mut mem);
        (0..seconds * 48_000)
            .map(|i| {
                // Prepared each buffer, as the engine does.
                if i % 512 == 0 {
                    t.prepare(&[0.67, feedback, age, 1.0], ctx());
                }
                let x = if i < 48 { 1.0 } else { 0.0 };
                t.process(&mut mem, [x, x])[0] - x
            })
            .collect()
    }

    #[test]
    fn tape_echo_repeats_on_the_beat_duller_with_age_and_never_runs_off() {
        // Time two thirds up is one beat, 24 000 frames: nothing before it, the click's echo at it.
        let young = tape(0.5, 0.0, 2);
        assert!(young[..23_900].iter().all(|v| v.abs() < 1e-4));
        let loud = |s: &[f32]| s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(loud(&young[23_900..24_200]) > 0.3, "{}", loud(&young[23_900..24_200]));
        // The second time round it is quieter, and on an old tape duller.
        assert!(loud(&young[47_800..]) < loud(&young[23_900..24_200]));
        let old = tape(0.5, 1.0, 2);
        // How much the sound moves from one frame to the next: its top.
        let edge = |s: &[f32]| s.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>();
        let (dull, bright) = (edge(&old[47_800..]), edge(&young[47_800..]));
        assert!(dull < 0.6 * bright, "{dull} {bright}");
        // Feedback all the way up builds, but the tape holds it.
        let full = tape(1.0, 0.5, 20);
        assert!(full.iter().all(|v| v.is_finite() && v.abs() < 3.0), "{}", loud(&full));
        assert!(loud(&full[19 * 48_000..]) > 0.01, "still ringing after twenty seconds");
    }

    /// The level a steady tone of `level` comes out at once the compressor has settled.
    fn settled(amount: f32, mix: f32, level: f32) -> f32 {
        let mut c = Compressor::default();
        c.prepare(&[amount, 0.667, 0.467, mix], ctx());
        (0..96_000).map(|_| c.process([level, level])[0]).last().unwrap()
    }

    #[test]
    fn compressor_evens_loud_and_quiet_out_and_stays_under_full_scale() {
        let (loud, quiet) = (settled(1.0, 1.0, 1.0), settled(1.0, 1.0, 0.05));
        assert!(loud <= 1.0, "a full-scale peak stays under full scale: {loud}");
        assert!(quiet > 0.05, "quiet parts are lifted by the makeup: {quiet}");
        assert!(loud / quiet < 8.0, "20:1 going in, {} coming out", loud / quiet);
        // With no dry sound mixed away, nothing changes.
        assert_eq!(settled(1.0, 0.0, 0.5), 0.5);
        // The knee joins its two ends.
        assert_eq!(reduction(-13.1, -10.0, 4.0), 0.0);
        assert!((reduction(-7.0, -10.0, 4.0) - -2.25).abs() < 1e-4);
        assert!((reduction(0.0, -10.0, 4.0) - -7.5).abs() < 1e-4);
    }

    #[test]
    fn autopan_swings_to_each_side_and_does_nothing_at_no_width() {
        let mut a = Autopan::default();
        a.prepare(&[0.5, 1.0, 1.0, 0.0], ctx());
        let out: Vec<[f32; 2]> = (0..24_000).map(|_| a.process([1.0, 1.0])).collect();
        // A beat is 24 000 frames: a quarter in it is hard one way, three quarters in the other.
        assert!(out[6_000][0] < 0.05 && out[6_000][1] > 0.95, "{:?}", out[6_000]);
        assert!(out[18_000][1] < 0.05 && out[18_000][0] > 0.95, "{:?}", out[18_000]);
        let mut a = Autopan::default();
        a.prepare(&[0.5, 0.0, 1.0, 0.0], ctx());
        assert!((0..24_000).all(|_| a.process([1.0, 1.0]) == [1.0, 1.0]));
    }

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
