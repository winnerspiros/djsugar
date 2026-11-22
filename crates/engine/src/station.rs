//! Master effects station: beat synced scenes, a three band isolator and
//! release effects, run on the main bus after the decks are mixed.
//! Every buffer is allocated up front; `run` never allocates.

use crate::biquad::{Biquad, Kind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scene {
    #[default]
    Echo,
    HpfEcho,
    LpfEcho,
    CrushEcho,
    SpiralUp,
    SpiralDown,
    Noise,
    ReverbUp,
    ReverbDown,
    Flanger,
    Phaser,
    Filter,
    Roll,
    Gater,
    Pitch,
    Drive,
}

impl Scene {
    pub const ALL: [Self; 16] = [
        Self::Echo,
        Self::HpfEcho,
        Self::LpfEcho,
        Self::CrushEcho,
        Self::SpiralUp,
        Self::SpiralDown,
        Self::Noise,
        Self::ReverbUp,
        Self::ReverbDown,
        Self::Flanger,
        Self::Phaser,
        Self::Filter,
        Self::Roll,
        Self::Gater,
        Self::Pitch,
        Self::Drive,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Echo => "ECHO",
            Self::HpfEcho => "HPF ECHO",
            Self::LpfEcho => "LPF ECHO",
            Self::CrushEcho => "CRUSH",
            Self::SpiralUp => "SPIRAL UP",
            Self::SpiralDown => "SPIRAL DN",
            Self::Noise => "NOISE",
            Self::ReverbUp => "REVERB UP",
            Self::ReverbDown => "REVERB DN",
            Self::Flanger => "FLANGER",
            Self::Phaser => "PHASER",
            Self::Filter => "FILTER",
            Self::Roll => "ROLL",
            Self::Gater => "GATER",
            Self::Pitch => "PITCH",
            Self::Drive => "DRIVE",
        }
    }

    const fn echoes(self) -> bool {
        matches!(self, Self::Echo | Self::HpfEcho | Self::LpfEcho | Self::CrushEcho | Self::SpiralUp | Self::SpiralDown)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Release {
    #[default]
    Brake,
    Echo,
    BackSpin,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Set {
    Scene(Scene),
    On(bool),
    Dial(f32),
    Params(f32, f32),
    Isolate([f32; 3]),
    Bpm(f64),
    Release(Release),
}

/// Echo times of the first parameter, in beats.
const ECHO_BEATS: [f64; 6] = [0.125, 0.25, 0.375, 0.5, 0.75, 1.0];
/// Sweep and slice lengths of the first parameter, in beats.
const SWEEP_BEATS: [f64; 6] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
const SLICE_BEATS: [f64; 5] = [0.0625, 0.125, 0.25, 0.5, 1.0];
const PITCH_WINDOW: f32 = 2048.0;

fn pick<T: Copy, const N: usize>(v: &[T; N], knob: f32) -> T {
    v[((knob * N as f32) as usize).min(N - 1)]
}

fn soft(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

/// Stereo delay line with linear interpolated reads.
#[derive(Debug)]
struct Line {
    buf: Vec<[f32; 2]>,
    w: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self { buf: vec![[0.0; 2]; len.max(4)], w: 0 }
    }

    fn write(&mut self, s: [f32; 2]) {
        self.buf[self.w] = s;
        self.w = (self.w + 1) % self.buf.len();
    }

    /// Sample `delay` frames before the next write slot.
    fn read(&self, delay: f32) -> [f32; 2] {
        self.at(self.w as f32 - delay.clamp(1.0, self.buf.len() as f32 - 2.0))
    }

    fn at(&self, pos: f32) -> [f32; 2] {
        let n = self.buf.len() as f32;
        let p = pos.rem_euclid(n);
        let i = p as usize % self.buf.len();
        let j = (i + 1) % self.buf.len();
        let t = p.fract();
        let (a, b) = (self.buf[i], self.buf[j]);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }

    fn clear(&mut self) {
        self.buf.fill([0.0; 2]);
    }
}

/// Damped feedback comb.
#[derive(Debug)]
struct Comb {
    buf: Vec<f32>,
    i: usize,
    low: f32,
}

impl Comb {
    fn run(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buf[self.i];
        self.low = y + (self.low - y) * damp;
        self.buf[self.i] = x + self.low * feedback;
        self.i = (self.i + 1) % self.buf.len();
        y
    }
}

#[derive(Debug)]
struct AllPass {
    buf: Vec<f32>,
    i: usize,
}

impl AllPass {
    fn run(&mut self, x: f32) -> f32 {
        let d = self.buf[self.i];
        let y = d - x;
        self.buf[self.i] = x + d * 0.5;
        self.i = (self.i + 1) % self.buf.len();
        y
    }
}

/// Small room: four combs into two all-passes per side, lengths spread apart.
#[derive(Debug)]
struct Room {
    combs: [[Comb; 4]; 2],
    passes: [[AllPass; 2]; 2],
    feedback: f32,
    damp: f32,
}

impl Room {
    fn new(rate: f32) -> Self {
        let len = |ms: f32| ((ms * rate / 1000.0) as usize).max(2);
        let comb = |ms: f32| Comb { buf: vec![0.0; len(ms)], i: 0, low: 0.0 };
        let pass = |ms: f32| AllPass { buf: vec![0.0; len(ms)], i: 0 };
        let side = |o: f32| [comb(25.3 + o), comb(26.9 + o), comb(28.9 + o), comb(30.7 + o)];
        Self {
            combs: [side(0.0), side(0.53)],
            passes: [[pass(5.1), pass(12.6)], [pass(5.6), pass(13.1)]],
            feedback: 0.84,
            damp: 0.3,
        }
    }

    fn run(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mono = (x[0] + x[1]) * 0.15;
        let mut out = [0.0; 2];
        for (k, o) in out.iter_mut().enumerate() {
            let mut s = 0.0;
            for c in &mut self.combs[k] {
                s += c.run(mono, self.feedback, self.damp);
            }
            for p in &mut self.passes[k] {
                s = p.run(s);
            }
            *o = s;
        }
        out
    }

    fn clear(&mut self) {
        for c in self.combs.iter_mut().flatten() {
            c.buf.fill(0.0);
            c.low = 0.0;
        }
        for p in self.passes.iter_mut().flatten() {
            p.buf.fill(0.0);
        }
    }
}

#[derive(Debug)]
pub struct Station {
    rate: f32,
    bpm: f64,
    beat: f64,
    scene: Scene,
    on: bool,
    dial: f32,
    p1: f32,
    p2: f32,
    mix: f32,
    amount: f32,
    tail: u32,
    echo: Line,
    delay: f32,
    room: Room,
    filt: Biquad,
    wet_filt: Biquad,
    crush: [f32; 2],
    crush_n: u32,
    noise: u32,
    short: Line,
    phase_z: [[f32; 2]; 4],
    phase_fb: [f32; 2],
    roll: Vec<[f32; 2]>,
    roll_fill: usize,
    roll_pos: usize,
    pitch_pos: f32,
    gate: f32,
    iso: [f32; 3],
    iso_gain: [f32; 3],
    low: Biquad,
    high: Biquad,
    history: Line,
    release: Release,
    released: Option<(f64, f32)>,
    rel_echo: Line,
}

impl Station {
    pub fn new(rate: f32) -> Self {
        let frames = |s: f32| (s * rate) as usize;
        let mut low = Biquad::identity();
        low.set(Kind::Low, 300.0, std::f32::consts::FRAC_1_SQRT_2, rate);
        let mut high = Biquad::identity();
        high.set(Kind::High, 3000.0, std::f32::consts::FRAC_1_SQRT_2, rate);
        Self {
            rate,
            bpm: 120.0,
            beat: 0.0,
            scene: Scene::Echo,
            on: false,
            dial: 0.5,
            p1: 0.5,
            p2: 0.5,
            mix: 0.0,
            amount: 0.0,
            tail: 0,
            echo: Line::new(frames(4.0)),
            delay: 0.0,
            room: Room::new(rate),
            filt: Biquad::identity(),
            wet_filt: Biquad::identity(),
            crush: [0.0; 2],
            crush_n: 0,
            noise: 0x9e37_79b9,
            short: Line::new(frames(0.05)),
            phase_z: [[0.0; 2]; 4],
            phase_fb: [0.0; 2],
            roll: vec![[0.0; 2]; frames(4.0)],
            roll_fill: 0,
            roll_pos: 0,
            pitch_pos: 0.0,
            gate: 1.0,
            iso: [0.0; 3],
            iso_gain: [1.0; 3],
            low,
            high,
            history: Line::new(frames(8.0)),
            release: Release::Brake,
            released: None,
            rel_echo: Line::new(frames(4.0)),
        }
    }

    fn beat_frames(&self) -> f64 {
        60.0 / self.bpm.max(20.0) * f64::from(self.rate)
    }

    pub fn active(&self) -> bool {
        self.on
            || self.mix > 1e-4
            || self.tail > 0
            || self.released.is_some()
            || self.iso.iter().any(|v| v.abs() > 0.01)
    }

    pub fn set(&mut self, s: Set) {
        match s {
            Set::Scene(sc) if sc != self.scene => {
                self.scene = sc;
                self.echo.clear();
                self.room.clear();
                self.short.clear();
                self.filt = Biquad::identity();
                self.wet_filt = Biquad::identity();
                self.phase_z = [[0.0; 2]; 4];
                self.mix = 0.0;
                self.roll_fill = usize::MAX;
            }
            Set::Scene(_) => {}
            Set::On(on) => {
                if on && !self.on {
                    self.roll_fill = 0;
                    self.roll_pos = 0;
                }
                self.on = on;
            }
            Set::Dial(v) => self.dial = v.clamp(0.0, 1.0),
            Set::Params(a, b) => (self.p1, self.p2) = (a.clamp(0.0, 1.0), b.clamp(0.0, 1.0)),
            Set::Isolate(v) => self.iso = v.map(|x| x.clamp(-1.0, 1.0)),
            Set::Bpm(b) => self.bpm = b.clamp(20.0, 300.0),
            Set::Release(r) => {
                self.release = r;
                self.released = Some((0.0, self.history.w as f32 - 1.0));
                self.on = false;
                self.mix = 0.0;
                self.echo.clear();
                self.room.clear();
                self.rel_echo.clear();
                self.iso = [0.0; 3];
            }
        }
    }

    fn noise(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn prepare(&mut self) {
        let a = self.amount;
        let (p1, p2, r) = (self.p1, self.p2, self.rate);
        match self.scene {
            Scene::HpfEcho => self.filt.set(Kind::High, 20.0 * 2f32.powf(7.0 * a), 0.9, r),
            Scene::LpfEcho => self.filt.set(Kind::Low, 20000.0 * 2f32.powf(-6.0 * a), 0.9, r),
            Scene::Noise => self.filt.set(Kind::High, 200.0 * 2f32.powf(5.5 * a + 3.0 * (p1 - 0.5)), 0.7 + 6.0 * p2, r),
            Scene::ReverbUp => self.wet_filt.set(Kind::High, 100.0 * 2f32.powf(5.0 * a), 0.7, r),
            Scene::ReverbDown => self.filt.set(Kind::Low, 20000.0 * 2f32.powf(-4.3 * a), 0.7, r),
            Scene::Filter => {
                let d = self.dial * 2.0 - 1.0;
                let q = 0.7 + 5.0 * p2;
                if d < 0.0 {
                    self.filt.set(Kind::Low, 20000.0 * 2f32.powf(9.0 * d), q, r);
                } else {
                    self.filt.set(Kind::High, 20.0 * 2f32.powf(9.0 * d), q, r);
                }
            }
            _ => {}
        }
        if matches!(self.scene, Scene::ReverbUp | Scene::ReverbDown) {
            self.room.feedback = 0.7 + 0.27 * p1;
            self.room.damp = 0.6 - 0.5 * p2;
        }
        let k = ((ECHO_BEATS.len() as f32 * p1) as usize).min(ECHO_BEATS.len() - 1);
        let target = (ECHO_BEATS[k] * self.beat_frames()) as f32;
        self.delay = if self.delay <= 0.0 { target } else { self.delay + (target - self.delay) * 0.2 };
    }

    fn scene(&mut self, x: [f32; 2]) -> [f32; 2] {
        let (send, amount, p1, p2) = (if self.on { self.dial } else { 0.0 }, self.amount, self.p1, self.p2);
        let (y, wet);
        let sc = self.scene;
        if sc.echoes() {
            wet = self.echo.read(self.delay);
            let mut out;
            let mut fb = 0.25 + 0.6 * p2;
            let mut back = wet;
            if matches!(sc, Scene::SpiralUp | Scene::SpiralDown) {
                let semis = (1.0 + 6.0 * p2) * if sc == Scene::SpiralUp { 1.0 } else { -1.0 };
                let ratio = 2f32.powf(semis / 12.0);
                self.short.write(wet);
                self.pitch_pos =
                    (self.pitch_pos + 1.0 - ratio).rem_euclid(PITCH_WINDOW.min(self.short.buf.len() as f32 - 4.0));
                back = self.shift(self.pitch_pos);
                fb = 0.55 + 0.35 * amount;
            }
            self.echo.write([soft(x[0] * send + back[0] * fb), soft(x[1] * send + back[1] * fb)]);
            out = [x[0] + wet[0] * amount, x[1] + wet[1] * amount];
            match sc {
                Scene::HpfEcho | Scene::LpfEcho => out = [self.filt.run(0, out[0]), self.filt.run(1, out[1])],
                Scene::CrushEcho => {
                    let steps = 2f32.powf(16.0 - 12.0 * amount);
                    self.crush_n += 1;
                    if self.crush_n > (7.0 * amount) as u32 {
                        self.crush_n = 0;
                        self.crush = out.map(|v| (soft(v * (1.0 + 2.0 * amount)) * steps).round() / steps);
                    }
                    out = [out[0] + (self.crush[0] - out[0]) * amount, out[1] + (self.crush[1] - out[1]) * amount];
                }
                _ => {}
            }
            y = out;
        } else {
            match sc {
                Scene::Noise => {
                    let level = 0.4 * amount * amount;
                    let (a, b) = (self.noise(), self.noise());
                    wet = [self.filt.run(0, a) * level, self.filt.run(1, b) * level];
                    y = [x[0] + wet[0], x[1] + wet[1]];
                }
                Scene::ReverbUp | Scene::ReverbDown => {
                    let mut i = [x[0] * send, x[1] * send];
                    if sc == Scene::ReverbUp {
                        i = [self.wet_filt.run(0, i[0]), self.wet_filt.run(1, i[1])];
                    }
                    wet = self.room.run(i);
                    let dry = if sc == Scene::ReverbDown { 1.0 - 0.85 * amount } else { 1.0 };
                    let out = [x[0] * dry + wet[0] * 1.5 * amount, x[1] * dry + wet[1] * 1.5 * amount];
                    y = if sc == Scene::ReverbDown {
                        [self.filt.run(0, out[0]), self.filt.run(1, out[1])]
                    } else {
                        out
                    };
                }
                _ => return self.added(x, p1, p2),
            }
        }
        if wet[0].abs() + wet[1].abs() > 1e-5 {
            self.tail = self.rate as u32;
        } else {
            self.tail = self.tail.saturating_sub(1);
        }
        y
    }

    fn shift(&self, pos: f32) -> [f32; 2] {
        let w = PITCH_WINDOW.min(self.short.buf.len() as f32 - 4.0);
        let other = (pos + w * 0.5) % w;
        let ga = (std::f32::consts::PI * pos / w).sin();
        let gb = (std::f32::consts::PI * other / w).sin();
        let (a, b) = (self.short.read(pos + 1.0), self.short.read(other + 1.0));
        [a[0] * ga * ga + b[0] * gb * gb, a[1] * ga * ga + b[1] * gb * gb]
    }

    /// Insert effects: blended by the smoothed on and off mix.
    fn added(&mut self, x: [f32; 2], p1: f32, p2: f32) -> [f32; 2] {
        let target = if self.on { 1.0 } else { 0.0 };
        self.mix += (target - self.mix) * (1.0 / (0.01 * self.rate));
        if self.mix < 1e-4 && !self.on {
            self.roll_fill = usize::MAX;
            return x;
        }
        let dial = self.dial;
        let beat = self.beat;
        let mut y = x;
        let mut amount = dial;
        match self.scene {
            Scene::Flanger | Scene::Phaser => {
                let lfo = (beat / pick(&SWEEP_BEATS, p1)).fract() as f32;
                let tri = 1.0 - (lfo * 2.0 - 1.0).abs();
                if self.scene == Scene::Flanger {
                    let d = (0.0005 + 0.0045 * tri) * self.rate;
                    let w = self.short.read(d);
                    let fb = 0.95 * p2;
                    self.short.write([x[0] + w[0] * fb, x[1] + w[1] * fb]);
                    y = [(x[0] + w[0]) * 0.7, (x[1] + w[1]) * 0.7];
                } else {
                    let f = 200.0 * 2f32.powf(4.5 * tri);
                    let t = (std::f32::consts::PI * f / self.rate).tan();
                    let c = (t - 1.0) / (t + 1.0);
                    for k in 0..2 {
                        let mut s = x[k] + self.phase_fb[k] * 0.9 * p2;
                        for z in &mut self.phase_z {
                            let o = c * s + z[k];
                            z[k] = s - c * o;
                            s = o;
                        }
                        self.phase_fb[k] = s;
                        y[k] = (x[k] + s) * 0.7;
                    }
                }
            }
            Scene::Filter => {
                y = [self.filt.run(0, x[0]), self.filt.run(1, x[1])];
                amount = 1.0;
            }
            Scene::Roll => {
                let len = ((pick(&SLICE_BEATS, p1) * self.beat_frames()) as usize).clamp(16, self.roll.len());
                if self.roll_fill < len {
                    self.roll[self.roll_fill] = x;
                    self.roll_fill += 1;
                } else if self.roll_fill != usize::MAX {
                    // A soft edge at each repeat, longer with the second knob
                    let i = self.roll_pos % len;
                    self.roll_pos += 1;
                    let edge = (16.0 + 240.0 * p2).min(len as f32 * 0.25);
                    let g = (i as f32 / edge).min((len - i) as f32 / edge).min(1.0);
                    y = self.roll[i].map(|v| v * g);
                }
                amount = (2.0 * dial).min(1.0);
            }
            Scene::Gater => {
                let ph = (beat / pick(&SLICE_BEATS, p1)).fract();
                let target = if ph < 0.5 { 1.0 } else { 1.0 - p2.max(0.2) };
                self.gate += (target - self.gate) * (1.0 / (0.002 * self.rate));
                y = [x[0] * self.gate, x[1] * self.gate];
            }
            Scene::Pitch => {
                let semis = (2.0 * dial - 1.0) * (1.0 + 23.0 * p1);
                let ratio = 2f32.powf(semis / 12.0);
                self.short.write(x);
                let w = PITCH_WINDOW.min(self.short.buf.len() as f32 - 4.0);
                self.pitch_pos = (self.pitch_pos + 1.0 - ratio).rem_euclid(w);
                y = self.shift(self.pitch_pos);
                amount = 0.5 + 0.5 * p2;
            }
            Scene::Drive => {
                let g = 1.0 + 24.0 * dial * dial;
                let tone = 0.2 + 0.8 * p1;
                for (c, v) in self.crush.iter_mut().zip(x) {
                    let d = soft(v * g) * (1.0 + 0.5 * p2) / (1.0 + 0.1 * g).sqrt();
                    *c += (d - *c) * tone;
                }
                y = self.crush;
                amount = 1.0;
            }
            _ => return x,
        }
        let wet = self.mix * amount;
        [x[0] + (y[0] - x[0]) * wet, x[1] + (y[1] - x[1]) * wet]
    }

    fn isolate(&mut self, x: [f32; 2]) -> [f32; 2] {
        for (g, v) in self.iso_gain.iter_mut().zip(self.iso) {
            *g += (1.0 + v - *g) * 0.002;
        }
        let mut y = [0.0; 2];
        for k in 0..2 {
            let low = self.low.run(k, x[k]);
            let high = self.high.run(k, x[k]);
            let mid = x[k] - low - high;
            y[k] = low * self.iso_gain[0] + mid * self.iso_gain[1] + high * self.iso_gain[2];
        }
        y
    }

    /// Returns the release output, or `None` once the release is over.
    fn release(&mut self) -> Option<[f32; 2]> {
        let (t, pos) = self.released?;
        let beat = self.beat_frames() as f32;
        let (out, next, len) = match self.release {
            Release::Brake => {
                let len = beat * 2.0;
                let speed = (1.0 - t as f32 / len).max(0.0);
                let start = self.history.w as f32 - len * 0.5 - 2.0;
                let p = if t == 0.0 { start } else { pos };
                (self.history.at(p).map(|v| v * speed.sqrt()), p + speed, len)
            }
            Release::BackSpin => {
                let len = beat * 1.5;
                let u = t as f32 / len;
                let speed = -3.0 * (1.0 - u).max(0.0);
                let fade = (1.0 - u).max(0.0);
                (self.history.at(pos).map(|v| v * fade), pos + speed, len)
            }
            Release::Echo => {
                let len = beat * 4.0;
                let d = beat * 0.75;
                let src = if t < f64::from(beat) * 0.5 { self.history.at(pos) } else { [0.0; 2] };
                let w = self.rel_echo.read(d);
                self.rel_echo.write([src[0] + w[0] * 0.6, src[1] + w[1] * 0.6]);
                let fade = (1.0 - t as f32 / len).max(0.0);
                ([(src[0] + w[0]) * fade, (src[1] + w[1]) * fade], pos + 1.0, len)
            }
        };
        if t as f32 >= len {
            self.released = None;
            return None;
        }
        self.released = Some((t + 1.0, next));
        Some(out)
    }

    pub fn run(&mut self, out: &mut [f32]) {
        let frames = out.len() / 2;
        let step = self.bpm / 60.0 / f64::from(self.rate);
        let target = if self.on { self.dial } else { 0.0 };
        let coeff = 1.0 - (-(frames as f32) / (0.03 * self.rate)).exp();
        self.amount += (target - self.amount) * coeff;
        if !self.active() {
            self.beat += step * frames as f64;
            for f in out.as_chunks::<2>().0 {
                self.history.write(*f);
            }
            return;
        }
        self.prepare();
        for f in out.as_chunks_mut::<2>().0 {
            self.beat += step;
            if let Some(r) = self.release() {
                *f = r;
                continue;
            }
            self.history.write(*f);
            let mut s = self.scene(*f);
            if self.iso.iter().any(|v| v.abs() > 0.01) || self.iso_gain.iter().any(|g| (g - 1.0).abs() > 1e-3) {
                s = self.isolate(s);
            }
            *f = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize) -> Vec<f32> {
        (0..n)
            .flat_map(|i| {
                let v = (i as f32 * 0.05).sin() * 0.5;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn idle_station_is_transparent() {
        let mut s = Station::new(48000.0);
        let x = tone(512);
        let mut y = x.clone();
        s.run(&mut y);
        assert_eq!(x, y);
    }

    #[test]
    fn every_scene_stays_finite_and_bounded() {
        for sc in Scene::ALL {
            let mut s = Station::new(48000.0);
            s.set(Set::Scene(sc));
            s.set(Set::Dial(0.9));
            s.set(Set::On(true));
            let mut changed = false;
            for _ in 0..40 {
                let x = tone(512);
                let mut y = x.clone();
                s.run(&mut y);
                assert!(y.iter().all(|v| v.is_finite() && v.abs() < 8.0), "{sc:?}");
                changed |= x != y;
            }
            assert!(changed, "{sc:?} does nothing");
        }
    }

    #[test]
    fn echo_rings_after_release_of_the_dial() {
        let mut s = Station::new(48000.0);
        s.set(Set::Bpm(120.0));
        s.set(Set::Dial(1.0));
        s.set(Set::On(true));
        for _ in 0..40 {
            s.run(&mut tone(512));
        }
        s.set(Set::On(false));
        let mut silence = vec![0.0; 1024];
        s.run(&mut silence);
        assert!(silence.iter().any(|v| v.abs() > 1e-3));
    }

    #[test]
    fn isolator_kills_a_band() {
        let mut s = Station::new(48000.0);
        s.set(Set::Isolate([-1.0, -1.0, -1.0]));
        let mut y = tone(48000);
        s.run(&mut y);
        let tail = &y[y.len() - 2000..];
        assert!(tail.iter().all(|v| v.abs() < 0.02));
    }

    #[test]
    fn release_brakes_then_hands_back() {
        let mut s = Station::new(48000.0);
        s.set(Set::Bpm(120.0));
        for _ in 0..200 {
            s.run(&mut tone(512));
        }
        for r in [Release::Brake, Release::BackSpin, Release::Echo] {
            s.set(Set::Release(r));
            assert!(s.active());
            for _ in 0..1000 {
                let mut y = tone(512);
                s.run(&mut y);
                assert!(y.iter().all(|v| v.is_finite()));
            }
            assert!(s.released.is_none(), "{r:?}");
        }
    }
}
