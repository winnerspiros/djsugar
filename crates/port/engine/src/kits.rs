//! The Groove Box's built-in kits.
//!
//! Every sound is synthesised here when the engine starts, so the kits need
//! no sample files. The Analog and Synthetic kits play the drum models of
//! Mutable Instruments' Plaits (Emilie Gillet's, MIT, through the
//! `mi-plaits-dsp` port): a kick that is a resonant filter struck by a
//! pulse, a snare of tuned modes and noise, hats of square-wave metal or
//! ring-modulated noise. The other kits are drawn here in the style of the
//! classic analogue drum machines: a kick is a sine wave swept down in
//! pitch with a click on top, a snare two tones and filtered noise, a clap
//! a few quick bursts of noise, the hats six square waves at clashing
//! pitches through a high-pass filter (the way the 808 makes its cymbals),
//! the cowbell two square waves through a band-pass. Nothing is sampled
//! from another machine. Kits are named by their character, never after a
//! manufacturer's model.
//!
//! Every kit has the same nine sounds first, in [`ROLES`] order, so row N
//! of a groove plays the same kind of sound in any kit; some kits add
//! sounds of their own after them, for a row to switch to.

use std::f32::consts::TAU;
use std::sync::Arc;

use mi_plaits_dsp::drums::{
    analog_bass_drum::AnalogBassDrum, analog_snare_drum::AnalogSnareDrum, hihat,
    synthetic_bass_drum::SyntheticBassDrum, synthetic_snare_drum::SyntheticSnareDrum,
};

use crate::sampler::Sample;

/// The sounds every kit has, in the Groove Box's row order.
pub const ROLES: [&str; 9] = [
    "Kick",
    "Snare",
    "Closed hat",
    "Open hat",
    "Clap",
    "Rim",
    "Cowbell",
    "Crash",
    "Ride",
];

/// The most sounds a kit has (the nine roles and its own).
pub const MAX_SOUNDS: usize = 12;

pub struct KitSound {
    pub name: &'static str,
    pub sample: Arc<Sample>,
    /// The note the sound rings at, in Hz, for a kick (0 when it has no
    /// clear pitch or isn't a kick): what tuning it to a key starts from.
    pub pitch: f32,
}

pub struct Kit {
    pub name: &'static str,
    pub sounds: Vec<KitSound>,
}

pub struct KitInfo {
    pub name: &'static str,
    pub about: &'static str,
    /// Sounds after the nine [`ROLES`].
    pub extras: &'static [&'static str],
}

impl KitInfo {
    /// The kit's sounds' names, in order.
    pub fn sounds(&self) -> impl Iterator<Item = &'static str> + '_ {
        ROLES.iter().copied().chain(self.extras.iter().copied())
    }
}

pub const KITS: [KitInfo; 6] = [
    KitInfo {
        name: "Analog",
        about: "Round, warm analogue drums (Plaits drum models)",
        extras: &["Low tom", "High tom"],
    },
    KitInfo {
        name: "Synthetic",
        about: "Tight digital kick and snare, crisp hats (Plaits drum models)",
        extras: &["Low tom", "High tom"],
    },
    KitInfo {
        name: "Boom",
        about: "Deep, long kick and soft analogue drums, 808-style",
        extras: &["Tom"],
    },
    KitInfo {
        name: "Punch",
        about: "Short, punchy kick and bright hats for house and techno, 909-style",
        extras: &["Tom"],
    },
    KitInfo {
        name: "Tin",
        about: "Small, ticking rhythm-box drums, 606-style",
        extras: &["Shaker", "Tom"],
    },
    KitInfo {
        name: "Dub",
        about: "Soft kick, rimshot and a chord stab for dub techno",
        extras: &["Chord", "Shaker", "Tom"],
    },
];

/// Turns a row of steps written as text into a 16-bit pattern: `x`, `X`
/// or `o` is a hit, anything else a rest.
pub fn parse_steps(steps: &str) -> u32 {
    steps
        .chars()
        .take(16)
        .enumerate()
        .filter(|(_, c)| matches!(c, 'x' | 'X' | 'o'))
        .fold(0, |bits, (i, _)| bits | 1 << i)
}

pub fn build(rate: f64) -> Vec<Kit> {
    let sr = rate as f32;
    KITS.iter()
        .enumerate()
        .map(|(k, info)| Kit {
            name: info.name,
            sounds: info
                .sounds()
                .enumerate()
                .map(|(n, name)| {
                    let mono = sound(k, n, name, sr);
                    KitSound {
                        name,
                        pitch: if name == "Kick" {
                            pitch_of(&mono, sr)
                        } else {
                            0.0
                        },
                        sample: Arc::new(Sample::from_mono(&mono, rate)),
                    }
                })
                .collect(),
        })
        .collect()
}

/// The pitch a drum rings at once its attack has passed, in Hz (30-250),
/// by autocorrelation over the body of the sound; 0 when there is none.
pub fn pitch_of(sound: &[f32], sr: f32) -> f32 {
    let from = (0.03 * sr) as usize;
    let to = ((0.25 * sr) as usize).min(sound.len());
    if to <= from + (sr / 30.0) as usize {
        return 0.0;
    }
    let body = &sound[from..to];
    let energy: f32 = body.iter().map(|v| v * v).sum();
    if energy < 1e-6 {
        return 0.0;
    }
    let corr = |lag: usize| -> f32 {
        body.iter()
            .zip(&body[lag..])
            .map(|(a, b)| a * b)
            .sum::<f32>()
            / (body.len() - lag) as f32
    };
    let (min_lag, max_lag) = ((sr / 250.0) as usize, (sr / 30.0) as usize);
    let mut best = (0, 0.0f32);
    for lag in min_lag..=max_lag {
        let c = corr(lag);
        if c > best.1 {
            best = (lag, c);
        }
    }
    let zero = energy / body.len() as f32;
    if best.0 == 0 || best.1 < 0.3 * zero {
        return 0.0;
    }
    sr / best.0 as f32
}

/// Kit `k`'s sound `n`, called `name`.
fn sound(k: usize, n: usize, name: &str, sr: f32) -> Vec<f32> {
    let seed = (k * 16 + n) as u32 + 1;
    match (KITS[k].name, name) {
        ("Analog", "Kick") => plaits_kick(sr, 52.0, 0.45, 0.5, 0.25, 0.2),
        ("Analog", "Snare") => plaits_snare(sr, 190.0, 0.5, 0.45, 0.6),
        ("Analog", "Clap") => plaits_snare(sr, 330.0, 0.8, 0.35, 0.95),
        ("Analog", "Closed hat") => plaits_hat(sr, false, 0.25, 0.3),
        ("Analog", "Open hat") => plaits_hat(sr, false, 0.6, 0.3),
        ("Analog", "Low tom") => plaits_kick(sr, 95.0, 0.5, 0.45, 0.1, 0.0),
        ("Analog", "High tom") => plaits_kick(sr, 150.0, 0.55, 0.4, 0.1, 0.0),
        ("Synthetic", "Kick") => plaits_synth_kick(sr, 50.0),
        ("Synthetic", "Snare") => plaits_synth_snare(sr),
        ("Synthetic", "Closed hat") => plaits_hat(sr, true, 0.2, 0.6),
        ("Synthetic", "Open hat") => plaits_hat(sr, true, 0.6, 0.6),
        ("Synthetic", "Low tom") => plaits_kick(sr, 100.0, 0.6, 0.4, 0.3, 0.1),
        ("Synthetic", "High tom") => plaits_kick(sr, 160.0, 0.65, 0.35, 0.3, 0.1),
        ("Boom", "Kick") => kick(sr, 150.0, 48.0, 0.035, 0.9, 0.25, 1.0),
        ("Punch", "Kick") => kick(sr, 260.0, 52.0, 0.022, 0.34, 0.6, 2.2),
        ("Tin", "Kick") => kick(sr, 200.0, 70.0, 0.015, 0.2, 0.4, 1.4),
        (_, "Kick") => kick(sr, 110.0, 44.0, 0.04, 0.55, 0.1, 1.0),
        ("Boom", "Snare") => snare(sr, [180.0, 330.0], 0.12, 0.16, 0.5, seed),
        ("Punch", "Snare") => snare(sr, [200.0, 360.0], 0.08, 0.2, 0.75, seed),
        (_, "Snare") => snare(sr, [240.0, 420.0], 0.05, 0.1, 0.65, seed),
        ("Punch", "Clap") => clap(sr, 0.22, seed),
        (_, "Clap") => clap(sr, 0.16, seed),
        ("Punch", "Closed hat") => hat(sr, 0.05, 8000.0, seed),
        ("Tin", "Closed hat") => hat(sr, 0.03, 9000.0, seed),
        ("Dub", "Closed hat") => hat(sr, 0.035, 7000.0, seed),
        (_, "Closed hat") => hat(sr, 0.045, 7500.0, seed),
        ("Punch", "Open hat") => hat(sr, 0.35, 7500.0, seed),
        (_, "Open hat") => hat(sr, 0.28, 7000.0, seed),
        (_, "Ride") => ride(sr, seed),
        (_, "Crash") => crash(sr, seed),
        ("Tin", "Cowbell") => cowbell(sr, [800.0, 1180.0], 0.12),
        (_, "Cowbell") => cowbell(sr, [540.0, 800.0], 0.2),
        ("Dub", "Rim") => rim(sr, 0.05, seed),
        (_, "Rim") => rim(sr, 0.03, seed),
        ("Boom", "Tom") => tom(sr, 110.0, 0.45),
        (_, "Tom") => tom(sr, 140.0, 0.3),
        (_, "Shaker") => shaker(sr, seed),
        (_, "Chord") => chord(sr),
        _ => Vec::new(),
    }
}

// The Plaits drum models, struck once and rendered until they die away.

/// Frames per render call, as on the module.
const PLAITS_BLOCK: usize = 24;

/// Renders `seconds` of a model struck at the start, trimmed of its silent
/// tail and scaled to a peak of `peak`.
fn plaits(sr: f32, seconds: f32, peak: f32, mut render: impl FnMut(bool, &mut [f32])) -> Vec<f32> {
    let blocks = frames(sr, seconds).div_ceil(PLAITS_BLOCK);
    let mut out = Vec::with_capacity(blocks * PLAITS_BLOCK);
    let mut block = [0.0f32; PLAITS_BLOCK];
    for b in 0..blocks {
        render(b == 0, &mut block);
        out.extend_from_slice(&block);
    }
    let max = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let quiet = max * 1e-3;
    let end = out
        .iter()
        .rposition(|s| s.abs() > quiet)
        .map_or(0, |i| i + 1);
    out.truncate(end.max(1));
    finish(out, peak, sr)
}

/// The analogue kick: a resonant filter struck by a pulse.
fn plaits_kick(sr: f32, hz: f32, tone: f32, decay: f32, attack_fm: f32, self_fm: f32) -> Vec<f32> {
    let mut d = AnalogBassDrum::new();
    d.init(sr);
    plaits(sr, 1.2, 0.95, |t, o| {
        d.render(false, t, 0.8, hz / sr, tone, decay, attack_fm, self_fm, o)
    })
}

/// The analogue snare; bright and snappy, it stands in for a clap.
fn plaits_snare(sr: f32, hz: f32, tone: f32, decay: f32, snappy: f32) -> Vec<f32> {
    let mut d = AnalogSnareDrum::new();
    d.init(sr);
    plaits(sr, 0.8, 0.8, |t, o| {
        d.render(false, t, 0.8, hz / sr, tone, decay, snappy, o)
    })
}

/// Ring-modulated noise (`square` false) or six square waves, with a
/// closed (short `decay`) or open envelope.
fn plaits_hat(sr: f32, square: bool, decay: f32, noisiness: f32) -> Vec<f32> {
    let mut d = hihat::Hihat::new();
    d.init(sr);
    let (mut a, mut b) = ([0.0f32; PLAITS_BLOCK], [0.0f32; PLAITS_BLOCK]);
    let (noise, vca) = if square {
        (hihat::NoiseType::Square, hihat::VcaType::Linear)
    } else {
        (hihat::NoiseType::RingMod, hihat::VcaType::Swing)
    };
    plaits(sr, 1.0, 0.6, |t, o| {
        d.render(
            false,
            t,
            0.8,
            3000.0 / sr,
            0.5,
            decay,
            noisiness,
            &mut a,
            &mut b,
            o,
            noise,
            vca,
            !square,
            square,
        )
    })
}

fn plaits_synth_kick(sr: f32, hz: f32) -> Vec<f32> {
    let mut d = SyntheticBassDrum::new();
    d.init(sr);
    plaits(sr, 1.2, 0.95, |t, o| {
        d.render(false, t, 0.8, hz / sr, 0.5, 0.5, 0.4, 0.5, 0.3, o)
    })
}

fn plaits_synth_snare(sr: f32) -> Vec<f32> {
    let mut d = SyntheticSnareDrum::new();
    d.init(sr);
    plaits(sr, 0.8, 0.8, |t, o| {
        d.render(false, t, 0.8, 200.0 / sr, 0.5, 0.45, 0.6, o)
    })
}

/// A small, repeatable noise source.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// A two-pole filter (the RBJ cookbook's).
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

enum Shape {
    Low,
    High,
    Band,
}

impl Biquad {
    fn new(shape: Shape, freq: f32, q: f32, sr: f32) -> Self {
        let w = TAU * (freq / sr).min(0.49);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b = match shape {
            Shape::Low => [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
            Shape::High => [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            Shape::Band => [alpha, 0.0, -alpha],
        };
        Self {
            b: b.map(|v| v / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

fn frames(sr: f32, seconds: f32) -> usize {
    (sr * seconds) as usize
}

/// Scales to a peak of `peak`, and fades the last 2 ms so nothing clicks.
fn finish(mut v: Vec<f32>, peak: f32, sr: f32) -> Vec<f32> {
    let max = v.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 0.0 {
        v.iter_mut().for_each(|s| *s *= peak / max);
    }
    let fade = frames(sr, 0.002).min(v.len());
    let len = v.len();
    for (i, s) in v[len - fade..].iter_mut().enumerate() {
        *s *= 1.0 - i as f32 / fade as f32;
    }
    v
}

/// A sine swept from `from` to `to` Hz (time constant `sweep` s), dying away
/// over `decay` s, with a click of `click` and some `drive`.
fn kick(sr: f32, from: f32, to: f32, sweep: f32, decay: f32, click: f32, drive: f32) -> Vec<f32> {
    let n = frames(sr, decay * 4.0);
    let mut phase = 0.0f32;
    let mut noise = Noise(7);
    let mut hp = Biquad::new(Shape::High, 2000.0, 0.7, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let f = to + (from - to) * (-t / sweep).exp();
            phase += TAU * f / sr;
            let body = phase.sin() * (-t / decay).exp();
            let tick = hp.process(noise.next()) * (-t / 0.003).exp() * click;
            ((body + tick) * drive).tanh()
        })
        .collect();
    finish(out, 0.95, sr)
}

/// Two tones and band-passed noise.
fn snare(
    sr: f32,
    tones: [f32; 2],
    tone_decay: f32,
    noise_decay: f32,
    snap: f32,
    seed: u32,
) -> Vec<f32> {
    let n = frames(sr, noise_decay * 4.0);
    let mut noise = Noise(seed);
    let mut bp = Biquad::new(Shape::High, 1800.0, 0.6, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let tone = ((TAU * tones[0] * t).sin() + 0.6 * (TAU * tones[1] * t).sin())
                * (-t / tone_decay).exp();
            let rattle = bp.process(noise.next()) * (-t / noise_decay).exp();
            tone * (1.0 - snap) + rattle * snap * 1.6
        })
        .collect();
    finish(out, 0.8, sr)
}

/// Three quick bursts of band-passed noise, then a tail.
fn clap(sr: f32, tail: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, tail * 4.0);
    let mut noise = Noise(seed);
    let mut bp = Biquad::new(Shape::Band, 1100.0, 1.2, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let burst = [0.0, 0.011, 0.023]
                .iter()
                .map(|&at| {
                    let d = t - at;
                    if (0.0..0.03).contains(&d) {
                        (-d / 0.004).exp()
                    } else {
                        0.0
                    }
                })
                .fold(0.0f32, f32::max);
            let env = burst.max(if t > 0.023 {
                0.6 * (-(t - 0.023) / tail).exp()
            } else {
                0.0
            });
            bp.process(noise.next()) * env
        })
        .collect();
    finish(out, 0.8, sr)
}

/// Six square waves at clashing pitches, high-passed: metallic cymbals.
fn metal(sr: f32, n: usize, tone: f32, seed: u32) -> Vec<f32> {
    const PITCHES: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];
    let mut noise = Noise(seed);
    let mut hp1 = Biquad::new(Shape::High, tone, 0.7, sr);
    let mut hp2 = Biquad::new(Shape::High, tone, 0.7, sr);
    (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let squares: f32 = PITCHES
                .iter()
                .map(|f| {
                    if (f * 1.7 * t).fract() < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                })
                .sum::<f32>()
                / 6.0;
            hp2.process(hp1.process(squares * 0.8 + noise.next() * 0.25))
        })
        .collect()
}

fn hat(sr: f32, decay: f32, tone: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, decay * 5.0);
    let mut v = metal(sr, n, tone, seed);
    for (i, s) in v.iter_mut().enumerate() {
        *s *= (-(i as f32 / sr) / decay).exp();
    }
    finish(v, 0.6, sr)
}

fn ride(sr: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, 1.6);
    let mut v = metal(sr, n, 5000.0, seed);
    for (i, s) in v.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *s *= 0.4 * (-t / 0.02).exp() + 0.6 * (-t / 0.5).exp();
    }
    finish(v, 0.5, sr)
}

fn crash(sr: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, 2.2);
    let mut v = metal(sr, n, 3500.0, seed);
    let mut noise = Noise(seed ^ 0x5a5a);
    for (i, s) in v.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *s = (*s * 0.7 + noise.next() * 0.3) * (0.5 * (-t / 0.05).exp() + 0.5 * (-t / 0.7).exp());
    }
    finish(v, 0.5, sr)
}

/// Two square waves through a band-pass.
fn cowbell(sr: f32, pitches: [f32; 2], decay: f32) -> Vec<f32> {
    let n = frames(sr, decay * 4.0);
    let mut bp = Biquad::new(Shape::Band, pitches[1] * 1.2, 2.0, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let sq = |f: f32| if (f * t).fract() < 0.5 { 1.0 } else { -1.0 };
            let env = 0.6 * (-t / 0.01).exp() + 0.4 * (-t / decay).exp();
            bp.process(sq(pitches[0]) + sq(pitches[1])) * env
        })
        .collect();
    finish(out, 0.6, sr)
}

/// A short knock: a high tone and a click.
fn rim(sr: f32, decay: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, decay * 5.0);
    let mut noise = Noise(seed);
    let mut bp = Biquad::new(Shape::Band, 1700.0, 3.0, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let tone = (TAU * 1700.0 * t).sin() * 0.5 + (TAU * 500.0 * t).sin() * 0.3;
            (tone + bp.process(noise.next())) * (-t / decay).exp()
        })
        .collect();
    finish(out, 0.7, sr)
}

fn tom(sr: f32, pitch: f32, decay: f32) -> Vec<f32> {
    kick(sr, pitch * 1.6, pitch, 0.05, decay, 0.15, 1.0)
        .into_iter()
        .map(|s| s * 0.85)
        .collect()
}

fn shaker(sr: f32, seed: u32) -> Vec<f32> {
    let n = frames(sr, 0.18);
    let mut noise = Noise(seed);
    let mut hp = Biquad::new(Shape::High, 6000.0, 0.7, sr);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let env = (t / 0.015).min(1.0) * (-t / 0.05).exp();
            hp.process(noise.next()) * env
        })
        .collect();
    finish(out, 0.45, sr)
}

/// A minor chord of saw waves through a closing low-pass: dub techno's stab.
fn chord(sr: f32) -> Vec<f32> {
    // C minor 7 around middle C, with the root an octave down.
    const NOTES: [f32; 5] = [130.81, 261.63, 311.13, 392.0, 466.16];
    let n = frames(sr, 1.2);
    let mut lp = Biquad::new(Shape::Low, 1200.0, 0.9, sr);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        if i % 64 == 0 {
            // The filter closes as the stab dies away.
            let cutoff = 300.0 + 2200.0 * (-t / 0.12).exp();
            let fresh = Biquad::new(Shape::Low, cutoff, 0.9, sr);
            lp.b = fresh.b;
            lp.a = fresh.a;
        }
        let saw: f32 = NOTES
            .iter()
            .enumerate()
            .map(|(k, f)| {
                // A little detune per note, as a real synth drifts.
                let f = f * (1.0 + 0.002 * k as f32);
                (f * t).fract() * 2.0 - 1.0
            })
            .sum::<f32>()
            / NOTES.len() as f32;
        out.push(lp.process(saw) * (-t / 0.35).exp());
    }
    finish(out, 0.55, sr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kit_has_its_sounds_short_and_in_range() {
        let kits = build(48_000.0);
        assert_eq!(kits.len(), KITS.len());
        for kit in &kits {
            assert!(kit.sounds.len() <= MAX_SOUNDS, "{}", kit.name);
            for (role, s) in ROLES.iter().zip(&kit.sounds) {
                assert_eq!(*role, s.name, "{} plays the roles in order", kit.name);
            }
            for s in &kit.sounds {
                assert!(!s.sample.is_empty(), "{} {}", kit.name, s.name);
                assert!(s.sample.seconds() < 5.0, "{} {}", kit.name, s.name);
                let peak = (0..s.sample.len())
                    .map(|i| s.sample.frame(i)[0].abs())
                    .fold(0.0, f32::max);
                assert!(
                    peak > 0.2 && peak <= 1.0,
                    "{} {} peak {peak}",
                    kit.name,
                    s.name
                );
            }
        }
    }

    #[test]
    fn every_kick_rings_at_a_low_note() {
        for kit in build(48_000.0) {
            let hz = kit.sounds[0].pitch;
            assert!((30.0..120.0).contains(&hz), "{} kick at {hz} Hz", kit.name);
        }
    }

    #[test]
    fn steps_parse_to_sixteen_bits() {
        assert_eq!(parse_steps("x...x...x...x..."), 0b0001_0001_0001_0001);
        assert_eq!(parse_steps("o.X."), 0b0101);
    }
}
