//! The sampler: sixteen players, `[Sampler1]` to `[Sampler16]`, for the
//! pads' Sampler page. Out of the box the first eight hold short built-in
//! sounds, made here when the engine starts (kick, snare, clap, hat, air
//! horn, siren, laser, crash), so the pads work with no sample files; the
//! app puts its sample packs and the DJ's own files on them with
//! [`crate::AudioEngine::set_sample`]. Samplers go to the main mix after
//! the crossfader, like Mixxx's.
//!
//! Pressing a pad (`cue_gotoandplay`) does what the slot's `play_mode`
//! says: 0 one shot (plays to the end; pressing again restarts), 1 hold
//! (plays while held), 2 on/off (pressing again stops) or 3 loop (repeats
//! until pressed again; `repeat` loops in any mode). `cue_gotoandstop`
//! stops. `pregain` is the slot's level, silent to +6 dB.
//! `LoadSelectedTrack` and `eject` are for the app, which owns what each
//! slot holds.
//!
//! Samples are decoded and resampled off the audio thread and arrive over a
//! wait-free queue; ones the players are done with go back over another to
//! be freed off it, so nothing allocates or frees on the audio thread.

use std::f32::consts::TAU;
use std::path::Path;
use std::sync::Arc;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};

use crate::controls::Edge;
use crate::decode::Decoder;
use crate::track::Audio;

pub const NUM_SAMPLERS: usize = 16;

/// The built-in sounds, in pad order.
pub const SAMPLE_NAMES: [&str; 8] = [
    "Kick", "Snare", "Clap", "Hat", "Horn", "Siren", "Laser", "Crash",
];

/// The longest sample a pad takes; longer files belong on a deck.
pub const MAX_SAMPLE_SECONDS: f64 = 60.0;

/// Play modes, as `[SamplerN],play_mode` numbers them.
pub const ONE_SHOT: u8 = 0;
pub const HOLD: u8 = 1;
pub const ON_OFF: u8 = 2;
pub const LOOP: u8 = 3;

/// Frames over which a cut-off sample fades out, so it doesn't click.
const FADE_FRAMES: usize = 96;

pub fn sampler_group(n: usize) -> String {
    format!("[Sampler{}]", n + 1)
}

/// A sample ready to play: stereo 16-bit frames at the engine's rate.
pub struct Sample {
    frames: Box<[[i16; 2]]>,
    rate: f64,
}

impl Sample {
    /// Built-in sound `n` (see [`SAMPLE_NAMES`]) at `rate`.
    pub fn builtin(n: usize, rate: f64) -> Sample {
        let frames = sound(n, rate as f32).into_iter().map(|v| [v, v]);
        Sample::from_frames(frames, rate)
    }

    /// Decodes the audio file at `path` for an engine running at `rate`.
    /// Files longer than [`MAX_SAMPLE_SECONDS`] are refused.
    pub fn load(path: &Path, rate: f64) -> Result<Sample, String> {
        let decoder = Decoder::open(path)?;
        let max = (MAX_SAMPLE_SECONDS * decoder.audio().sample_rate.max(1) as f64) as usize;
        if decoder.length_known() && decoder.audio().len() > max {
            return Err(too_long(path));
        }
        let mut long = false;
        let audio = decoder.run(|a| {
            long = a.decoded() > max;
            !long
        })?;
        if long {
            return Err(too_long(path));
        }
        Ok(Sample::from_audio(&audio, rate))
    }

    /// `audio` resampled to `rate`.
    pub fn from_audio(audio: &Audio, rate: f64) -> Sample {
        let len = audio.decoded();
        let step = audio.sample_rate as f64 / rate;
        let out = if step > 0.0 {
            (len as f64 / step) as usize
        } else {
            0
        };
        let frames = (0..out).map(|i| {
            let x = i as f64 * step;
            let (j, t) = (x as usize, (x.fract()) as f32);
            let (a, b) = (
                audio.frame(j),
                audio.frame((j + 1).min(len.saturating_sub(1))),
            );
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        });
        Sample::from_frames(frames, rate)
    }

    /// A mono sound at `rate`, the same on both sides.
    pub(crate) fn from_mono(frames: &[f32], rate: f64) -> Sample {
        Sample::from_frames(frames.iter().map(|&v| [v, v]), rate)
    }

    fn from_frames(frames: impl Iterator<Item = [f32; 2]>, rate: f64) -> Sample {
        let q = |v: f32| (v.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        Sample {
            frames: frames.map(|[l, r]| [q(l), q(r)]).collect(),
            rate,
        }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn seconds(&self) -> f64 {
        self.len() as f64 / self.rate
    }

    /// The sample as [`Audio`], for the preview player.
    pub fn to_audio(&self) -> Audio {
        let f = |v: i16| v as f32 / i16::MAX as f32;
        let frames = self.frames.iter().map(|&[l, r]| [f(l), f(r)]).collect();
        Audio::from_frames(self.rate as u32, frames)
    }

    #[inline]
    pub(crate) fn frame(&self, i: usize) -> [f32; 2] {
        let f = |v: i16| v as f32 * (1.0 / i16::MAX as f32);
        self.frames.get(i).map_or([0.0; 2], |&[l, r]| [f(l), f(r)])
    }
}

fn too_long(path: &Path) -> String {
    format!(
        "{} is longer than a minute, so it can't go on a sampler pad. Load it on a deck instead.",
        path.file_name().unwrap_or_default().to_string_lossy()
    )
}

/// The app's end of the sampler's queues.
pub(crate) struct SamplerInput {
    samples: rtrb::Producer<(usize, Option<Arc<Sample>>)>,
    done: rtrb::Consumer<Arc<Sample>>,
}

impl SamplerInput {
    /// Puts `sample` on slot `n`, or empties it. Frees samples the players
    /// have finished with. False if the queue is full.
    pub(crate) fn send(&mut self, n: usize, sample: Option<Arc<Sample>>) -> bool {
        while self.done.pop().is_ok() {}
        self.samples.push((n, sample)).is_ok()
    }
}

struct Player {
    sample: Option<Arc<Sample>>,
    pos: usize,
    playing: bool,
    /// The sound that was cut off, fading out: its position and the frames
    /// of fade left.
    tail: Option<(Arc<Sample>, usize, usize)>,
    down: bool,
    go: ControlHandle,
    go_edge: Edge,
    stop: ControlHandle,
    stop_edge: Edge,
    play: ControlHandle,
    play_indicator: ControlHandle,
    mode: ControlHandle,
    repeat: ControlHandle,
    loaded: ControlHandle,
    duration: ControlHandle,
    volume: ControlHandle,
    pregain: ControlHandle,
    /// The level being played (volume × pregain), gliding to `target`, so
    /// turning a pad's level while it plays doesn't zip.
    gain: f32,
    /// The level the controls ask for, read once per buffer.
    target: f32,
}

impl Player {
    fn looping(&self) -> bool {
        self.mode.get() as u8 == LOOP || self.repeat.get_bool()
    }

    /// Stops, fading out what was playing.
    fn cut(&mut self) {
        if self.playing {
            if let Some(s) = &self.sample {
                self.tail = Some((s.clone(), self.pos, FADE_FRAMES));
            }
        }
        self.playing = false;
        self.pos = 0;
    }

    fn start(&mut self) {
        self.cut();
        self.playing = self.sample.is_some();
    }
}

pub(crate) struct Samplers {
    players: Vec<Player>,
    /// How far a level glides towards its target each frame.
    glide: f32,
    /// Whether any player is sounding (playing or fading), so a buffer
    /// with none skips them all.
    active: bool,
    incoming: rtrb::Consumer<(usize, Option<Arc<Sample>>)>,
    done: rtrb::Producer<Arc<Sample>>,
}

impl Samplers {
    pub(crate) fn new(reg: &ControlRegistry, sample_rate: f64) -> (Self, SamplerInput) {
        for g in ["[Master]", "[App]"] {
            if let Some(c) = reg.get(g, "num_samplers") {
                c.set(NUM_SAMPLERS as f64);
            }
        }
        let players = (0..NUM_SAMPLERS)
            .map(|n| {
                let g = sampler_group(n);
                let g = g.as_str();
                let sample =
                    (n < SAMPLE_NAMES.len()).then(|| Arc::new(Sample::builtin(n, sample_rate)));
                let loaded = reg.indicator(g, "track_loaded", 1.0);
                let duration = reg.indicator(g, "duration", MAX_SAMPLE_SECONDS * 2.0);
                loaded.set_bool(sample.is_some());
                // The app answers these: they change what the slot holds.
                reg.push(g, "LoadSelectedTrack");
                reg.push(g, "eject");
                duration.set(sample.as_ref().map_or(0.0, |s| s.seconds()));
                Player {
                    sample,
                    pos: 0,
                    playing: false,
                    tail: None,
                    down: false,
                    go: reg.push(g, "cue_gotoandplay"),
                    go_edge: Edge::default(),
                    stop: reg.push(g, "cue_gotoandstop"),
                    stop_edge: Edge::default(),
                    play: reg.toggle(g, "play"),
                    play_indicator: reg.indicator(g, "play_indicator", 1.0),
                    mode: reg.create(g, "play_mode", 0.0, 0.0, 3.0, Behavior::Free),
                    repeat: reg.toggle(g, "repeat"),
                    loaded,
                    duration,
                    volume: reg.create(g, "volume", 1.0, 0.0, 1.0, Behavior::Linear),
                    pregain: reg.create(g, "pregain", 1.0, 0.0, 2.0, Behavior::CenteredOnDefault),
                    gain: 1.0,
                    target: 1.0,
                }
            })
            .collect();
        let (samples, incoming) = rtrb::RingBuffer::new(NUM_SAMPLERS * 2);
        let (done, done_rx) = rtrb::RingBuffer::new(NUM_SAMPLERS * 4);
        (
            Self {
                players,
                // About 5 ms to settle.
                glide: (1.0 / (0.005 * sample_rate)).min(1.0) as f32,
                active: false,
                incoming,
                done,
            },
            SamplerInput {
                samples,
                done: done_rx,
            },
        )
    }

    /// Reads the controls and new samples before a buffer.
    pub(crate) fn update(&mut self) {
        while let Ok((n, sample)) = self.incoming.pop() {
            let Some(p) = self.players.get_mut(n) else {
                continue;
            };
            p.cut();
            p.loaded.set_bool(sample.is_some());
            p.duration.set(sample.as_ref().map_or(0.0, |s| s.seconds()));
            let old = std::mem::replace(&mut p.sample, sample);
            // The fading tail keeps its own reference; return both.
            for s in old.into_iter().chain(p.tail.take().map(|t| t.0)) {
                let _ = self.done.push(s);
            }
        }
        for p in &mut self.players {
            let was_playing = p.playing;
            let down = p.go.get_bool();
            let pressed = p.go_edge.pressed(&p.go);
            let released = p.down && !down;
            p.down = down;
            let mode = p.mode.get() as u8;
            if p.stop_edge.pressed(&p.stop) {
                p.cut();
            } else if pressed {
                if p.playing && (mode == ON_OFF || mode == LOOP) {
                    p.cut();
                } else {
                    p.start();
                }
                // A tap too quick to see still plays in hold mode.
                p.down = true;
            } else if released && mode == HOLD {
                p.cut();
            } else if p.play.get_bool() != p.playing {
                // Play starts it from wherever it stopped (the start once played out).
                if p.play.get_bool() {
                    let at_end = p.sample.as_ref().is_none_or(|s| p.pos >= s.len());
                    let pos = if at_end { 0 } else { p.pos };
                    p.playing = p.sample.is_some();
                    p.pos = pos;
                } else {
                    let pos = p.pos;
                    p.cut();
                    p.pos = pos;
                }
            }
            p.play.set_bool(p.playing);
            p.target = (p.volume.get() * p.pregain.get()) as f32;
            if !(was_playing && p.playing) {
                // A sound starting starts at the level set; only a level
                // turned while one plays glides.
                p.gain = p.target;
            }
        }
        self.active = self.players.iter().any(|p| p.playing || p.tail.is_some());
    }

    /// The samplers' next frame, mixed.
    pub(crate) fn next(&mut self) -> [f32; 2] {
        let mut sum = [0.0f32; 2];
        if !self.active {
            return sum;
        }
        for p in &mut self.players {
            if !p.playing && p.tail.is_none() {
                continue;
            }
            p.gain += (p.target - p.gain) * self.glide;
            let gain = p.gain;
            if let Some((s, pos, left)) = &mut p.tail {
                let g = gain * *left as f32 / FADE_FRAMES as f32;
                let v = s.frame(*pos);
                sum[0] += v[0] * g;
                sum[1] += v[1] * g;
                *pos += 1;
                *left -= 1;
                if *left == 0 {
                    if let Some((s, ..)) = p.tail.take() {
                        let _ = self.done.push(s);
                    }
                }
            }
            if !p.playing {
                continue;
            }
            let Some(s) = &p.sample else {
                p.playing = false;
                continue;
            };
            if p.pos >= s.len() {
                if p.looping() && !s.is_empty() {
                    p.pos = 0;
                } else {
                    p.playing = false;
                    continue;
                }
            }
            let v = s.frame(p.pos);
            sum[0] += v[0] * gain;
            sum[1] += v[1] * gain;
            p.pos += 1;
        }
        sum
    }

    /// Writes the controls after a buffer.
    pub(crate) fn publish(&mut self) {
        for p in &mut self.players {
            if !p.playing && p.play.get_bool() {
                p.play.set_bool(false);
            }
            p.play_indicator.set_bool(p.playing);
        }
    }
}

/// A small deterministic noise source, so the sounds are the same every run.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// Built-in sound `n` at `sr`, mono, peaking around 0.8.
fn sound(n: usize, sr: f32) -> Vec<f32> {
    let len = |seconds: f32| (seconds * sr) as usize;
    let t = |i: usize| i as f32 / sr;
    let mut noise = Noise(0x9e37_79b9 ^ n as u32);
    let mut phase = 0.0f32;
    let mut lp = 0.0f32;
    let mut hp_prev = 0.0f32;
    let mut hp = 0.0f32;
    match n {
        // Kick: a sine sweeping from 150 Hz down to 45 Hz.
        0 => (0..len(0.45))
            .map(|i| {
                let t = t(i);
                phase += TAU * (45.0 + 105.0 * (-t * 28.0).exp()) / sr;
                phase.sin() * (-t * 7.0).exp() * 0.9
            })
            .collect(),
        // Snare: a 190 Hz body and a burst of noise.
        1 => (0..len(0.3))
            .map(|i| {
                let t = t(i);
                phase += TAU * 190.0 / sr;
                (phase.sin() * (-t * 25.0).exp() * 0.5 + noise.next() * (-t * 14.0).exp() * 0.45)
                    * 0.9
            })
            .collect(),
        // Clap: three quick noise bursts and a tail, band-limited.
        2 => (0..len(0.35))
            .map(|i| {
                let t = t(i);
                let burst = [0.0, 0.011, 0.023]
                    .iter()
                    .map(|&s| {
                        if t >= s {
                            (-(t - s) * 180.0).exp()
                        } else {
                            0.0
                        }
                    })
                    .sum::<f32>()
                    + 0.6 * (-t * 12.0).exp();
                let x = noise.next();
                lp += 0.35 * (x - lp);
                (x - lp) * burst * 0.7
            })
            .collect(),
        // Closed hi-hat: high-passed noise, short.
        3 => (0..len(0.12))
            .map(|i| {
                let x = noise.next();
                hp = 0.6 * (hp + x - hp_prev);
                hp_prev = x;
                hp * (-t(i) * 45.0).exp() * 0.7
            })
            .collect(),
        // Air horn: three detuned saws in a minor third, with a little decay.
        4 => (0..len(1.2))
            .map(|i| {
                let t = t(i);
                let saw = |hz: f32| 2.0 * (t * hz).fract() - 1.0;
                let env = (t * 40.0).min(1.0) * if t > 1.0 { (1.2 - t) * 5.0 } else { 1.0 };
                let x = (saw(466.0) + saw(469.0) + saw(554.0)) / 3.0;
                lp += 0.25 * (x - lp);
                lp * env * 0.75
            })
            .collect(),
        // Siren: a square wave sweeping up and down.
        5 => (0..len(1.6))
            .map(|i| {
                let t = t(i);
                let hz = 700.0 + 350.0 * (TAU * 1.25 * t).sin();
                phase += TAU * hz / sr;
                let x = phase.sin().signum();
                lp += 0.2 * (x - lp);
                lp * (t * 30.0).min(1.0) * ((1.6 - t) * 6.0).min(1.0) * 0.45
            })
            .collect(),
        // Laser: a fast downward sweep.
        6 => (0..len(0.4))
            .map(|i| {
                let t = t(i);
                phase += TAU * (180.0 + 2800.0 * (-t * 14.0).exp()) / sr;
                phase.sin() * (-t * 6.0).exp() * 0.6
            })
            .collect(),
        // Crash: long, bright noise.
        _ => (0..len(1.8))
            .map(|i| {
                let x = noise.next();
                hp = 0.75 * (hp + x - hp_prev);
                hp_prev = x;
                hp * (-t(i) * 2.6).exp() * 0.5
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn samplers() -> (Arc<ControlRegistry>, Samplers, SamplerInput) {
        let reg = ControlRegistry::new();
        let (s, input) = Samplers::new(&reg, RATE);
        (reg, s, input)
    }

    fn run(s: &mut Samplers, frames: usize) -> f32 {
        let loud = (0..frames).map(|_| s.next()[0].abs()).fold(0.0, f32::max);
        s.publish();
        loud
    }

    fn press(reg: &ControlRegistry, s: &mut Samplers, n: usize, down: bool) {
        reg.get(&sampler_group(n), "cue_gotoandplay")
            .unwrap()
            .set_bool(down);
        s.update();
    }

    fn playing(reg: &ControlRegistry, n: usize) -> bool {
        reg.get(&sampler_group(n), "play_indicator")
            .unwrap()
            .get_bool()
    }

    #[test]
    fn sounds_are_short_and_in_range() {
        for (n, &name) in SAMPLE_NAMES.iter().enumerate() {
            let s = sound(n, 48_000.0);
            let peak = s.iter().fold(0.0f32, |p, v| p.max(v.abs()));
            assert!(s.len() < 48_000 * 2, "{name}");
            assert!(peak > 0.1 && peak <= 1.0, "{name} peaks at {peak}");
        }
    }

    #[test]
    fn sixteen_slots_and_the_first_eight_have_sounds() {
        let (reg, _, _) = samplers();
        for n in 0..NUM_SAMPLERS {
            let loaded = reg.get(&sampler_group(n), "track_loaded").unwrap();
            assert_eq!(loaded.get_bool(), n < 8, "slot {}", n + 1);
        }
    }

    #[test]
    fn a_pad_plays_its_sound_once() {
        let (reg, mut s, _) = samplers();
        press(&reg, &mut s, 0, true);
        assert!(run(&mut s, 4_800) > 0.3);
        assert!(playing(&reg, 0));
        press(&reg, &mut s, 0, false);
        run(&mut s, 48_000);
        assert!(!playing(&reg, 0));
    }

    #[test]
    fn hold_stops_on_release() {
        let (reg, mut s, _) = samplers();
        reg.get("[Sampler5]", "play_mode").unwrap().set(HOLD as f64);
        press(&reg, &mut s, 4, true);
        run(&mut s, 4_800);
        assert!(playing(&reg, 4));
        press(&reg, &mut s, 4, false);
        run(&mut s, FADE_FRAMES);
        assert!(!playing(&reg, 4));
        assert_eq!(run(&mut s, 100), 0.0, "faded out");
    }

    #[test]
    fn on_off_stops_on_second_press() {
        let (reg, mut s, _) = samplers();
        reg.get("[Sampler5]", "play_mode")
            .unwrap()
            .set(ON_OFF as f64);
        press(&reg, &mut s, 4, true);
        press(&reg, &mut s, 4, false);
        run(&mut s, 4_800);
        assert!(playing(&reg, 4));
        press(&reg, &mut s, 4, true);
        run(&mut s, 10);
        assert!(!playing(&reg, 4));
    }

    #[test]
    fn loop_repeats_until_pressed_again() {
        let (reg, mut s, _) = samplers();
        reg.get("[Sampler1]", "play_mode").unwrap().set(LOOP as f64);
        press(&reg, &mut s, 0, true);
        press(&reg, &mut s, 0, false);
        // The kick is under half a second; three seconds later it still plays.
        run(&mut s, 3 * 48_000);
        assert!(playing(&reg, 0));
        press(&reg, &mut s, 0, true);
        run(&mut s, 10);
        assert!(!playing(&reg, 0));
    }

    #[test]
    fn stop_and_level() {
        let (reg, mut s, _) = samplers();
        press(&reg, &mut s, 4, true);
        let full = run(&mut s, 4_800);
        reg.get("[Sampler5]", "cue_gotoandstop").unwrap().set(1.0);
        s.update();
        run(&mut s, 10);
        assert!(!playing(&reg, 4));
        reg.get("[Sampler5]", "pregain").unwrap().set(0.5);
        press(&reg, &mut s, 4, false);
        press(&reg, &mut s, 4, true);
        let half = run(&mut s, 4_800);
        assert!((half / full - 0.5).abs() < 0.05, "{half} vs {full}");
    }

    #[test]
    fn turning_the_level_while_playing_glides() {
        let (reg, mut s, mut input) = samplers();
        let tone = Audio::from_frames(48_000, vec![[0.5, 0.5]; 48_000]);
        assert!(input.send(0, Some(Arc::new(Sample::from_audio(&tone, RATE)))));
        s.update();
        press(&reg, &mut s, 0, true);
        assert!((s.next()[0] - 0.5).abs() < 0.01, "starts at its level");
        run(&mut s, 100);
        reg.get("[Sampler1]", "pregain").unwrap().set(0.0);
        s.update();
        let frames: Vec<f32> = (0..2_000).map(|_| s.next()[0]).collect();
        let step = frames
            .windows(2)
            .fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(step < 0.01, "no jump: {step}");
        assert!(frames[1_999].abs() < 1e-3, "down to silence");
    }

    #[test]
    fn idle_pads_cost_nothing() {
        let (_, mut s, _) = samplers();
        s.update();
        assert!(!s.active);
        assert_eq!(s.next(), [0.0; 2]);
    }

    #[test]
    fn new_sample_replaces_a_slot() {
        let (reg, mut s, mut input) = samplers();
        let tone = Audio::from_frames(44_100, vec![[0.5, -0.5]; 44_100]);
        let sample = Sample::from_audio(&tone, RATE);
        assert!((sample.seconds() - 1.0).abs() < 0.01);
        assert!(input.send(9, Some(Arc::new(sample))));
        s.update();
        let g = sampler_group(9);
        assert!(reg.get(&g, "track_loaded").unwrap().get_bool());
        assert!((reg.get(&g, "duration").unwrap().get() - 1.0).abs() < 0.01);
        press(&reg, &mut s, 9, true);
        let v = s.next();
        assert!((v[0] - 0.5).abs() < 0.01 && (v[1] + 0.5).abs() < 0.01);
        assert!(input.send(9, None));
        s.update();
        assert!(!reg.get(&g, "track_loaded").unwrap().get_bool());
        press(&reg, &mut s, 9, false);
        press(&reg, &mut s, 9, true);
        run(&mut s, FADE_FRAMES + 1);
        assert!(!playing(&reg, 9));
    }

    #[test]
    fn long_files_are_refused() {
        let dir = std::env::temp_dir().join(format!("djsugar-sampler-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("long.wav");
        crate::decode::tests::write_wav(&path, 8_000, 1, &vec![0i16; 8_000 * 61]);
        let err = Sample::load(&path, RATE).err().unwrap();
        assert!(err.contains("longer than a minute"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
