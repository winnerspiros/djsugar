//! The Looper: a deck that records and plays four layers locked to the mix's bars (`[LooperN]`, N the deck). Everything the looper hears goes into a ring that always runs: the main mix (a buffer late, since the mix is made after the decks) or one input (microphone, auxiliary inputs, the instrument). Each bar line is stamped with where it fell in the ring. Recording a layer is then copying the bars between two stamps out of the ring, and CAPTURE copies the bars just heard the same way, without
//! `[LooperN],length` bars) -> playing, or overdubbing when `after` is on. Tapping a playing layer overdubs it; tapping again plays it. UNDO takes back the last recording or overdub and REDO brings it back: an overdub's additions are kept in a pass buffer, cleared in blocks as they are first written, so starting one costs nothing. Memory (a ring and five layer-sized buffers, about 48 seconds each) is made off the audio thread when a deck becomes a looper (see [`supply`]) and handed back when it stops being one.
//! `[LooperN],length` bars) -> playing, or overdubbing when `after` is on. Tapping a playing layer overdubs it; tapping again plays it. UNDO takes back the last recording or overdub and REDO brings it back: an overdub's additions are kept in a pass buffer, cleared in blocks as they are first written, so starting one costs nothing. Memory (a ring and five layer-sized buffers, about 48 seconds each) is made off the audio thread when a deck becomes a looper (see [`supply`]) and handed back when it

#[cfg(test)]
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};

use crate::controls::Edge;
use crate::groove::{Clock, Source};

/// Layers on each looper.
pub const LAYERS: usize = 4;
/// The longest layer, and the ring, in seconds: 16 bars at 80 BPM.
pub const MAX_SECONDS: f64 = 48.0;
/// Bar lengths a layer can be recorded at, and captured at.
pub const LENGTHS: [u32; 5] = [1, 2, 4, 8, 16];
pub const CAPTURE_LENGTHS: [u32; 4] = [1, 2, 4, 8];
/// Beats in a bar.
const BAR: f64 = 4.0;
/// Bar lines remembered, for CAPTURE: more than the longest capture.
const STAMPS: usize = 40;
/// Frames copied out of the ring per buffer.
const COPY_CHUNK: usize = 32_768;
/// Frames over which a loop's end blends into what came just before its
/// start, so it goes round without a click (about 5 ms).
const SEAM: usize = 256;
/// The overdub pass is cleared in blocks this long as they are written.
const PASS_BLOCK: usize = 1024;

/// What `[LooperN],layerK_state` shows.
pub const EMPTY: u8 = 0;
pub const ARMED: u8 = 1;
pub const RECORDING: u8 = 2;
pub const PLAYING: u8 = 3;
pub const OVERDUBBING: u8 = 4;

/// `[LooperN],input`: 0 the main mix, else 1 + the engine's input, as
/// `[ChannelN],live_input` counts them (1 microphone, 2 aux, 3 instrument,
/// 4 aux 2, 5 aux 3).
pub const INPUT_MIX: usize = 0;

pub fn looper_group(deck: usize) -> String {
    format!("[Looper{}]", deck + 1)
}

/// A looper's audio buffers, made off the audio thread.
pub struct LooperMemory {
    ring: Vec<[f32; 2]>,
    layers: [Vec<[f32; 2]>; LAYERS],
    pass: Vec<[f32; 2]>,
    /// The overdub each block of `pass` was last written in.
    pass_gen: Vec<u32>,
}

impl LooperMemory {
    /// Buffers for `rate`. They are zeroed by the system as first touched,
    /// so a looper that only records short loops uses little memory.
    pub fn new(rate: f64) -> Box<Self> {
        let n = (MAX_SECONDS * rate) as usize;
        Box::new(Self {
            ring: vec![[0.0; 2]; n],
            layers: std::array::from_fn(|_| vec![[0.0; 2]; n]),
            pass: vec![[0.0; 2]; n],
            pass_gen: vec![0; n.div_ceil(PASS_BLOCK)],
        })
    }

    fn capacity(&self) -> usize {
        self.ring.len()
    }
}

pub struct LayerControls {
    pub tap: ControlHandle,
    tap_edge: Edge,
    pub clear: ControlHandle,
    clear_edge: Edge,
    pub mute: ControlHandle,
    pub volume: ControlHandle,
    pub state: ControlHandle,
    /// Its length in bars, 0 while empty.
    pub bars: ControlHandle,
    /// Where it is in its loop, 0 to 1.
    pub position: ControlHandle,
}

pub struct LooperControls {
    pub enabled: ControlHandle,
    /// The looper has its memory and works.
    pub ready: ControlHandle,
    pub input: ControlHandle,
    /// Bars a new layer records.
    pub length: ControlHandle,
    pub capture: ControlHandle,
    capture_edge: Edge,
    /// Bars CAPTURE takes.
    pub capture_bars: ControlHandle,
    pub undo: ControlHandle,
    undo_edge: Edge,
    pub redo: ControlHandle,
    redo_edge: Edge,
    /// After recording a layer, overdub it at once instead of playing it.
    pub after: ControlHandle,
    /// The looper's own tempo, used while nothing leads; shows the tempo
    /// followed otherwise.
    pub bpm: ControlHandle,
    pub following: ControlHandle,
    /// The beat in the bar, 0 to 4, for the bar lights.
    pub bar_beat: ControlHandle,
    /// Lit for a moment when a recording or capture didn't fit.
    pub too_long: ControlHandle,
    pub layers: [LayerControls; LAYERS],
}

impl LooperControls {
    fn new(reg: &ControlRegistry, deck: usize) -> Self {
        let g = looper_group(deck);
        let g = g.as_str();
        Self {
            enabled: reg.toggle(g, "enabled"),
            ready: reg.indicator(g, "ready", 1.0),
            input: reg.create(g, "input", INPUT_MIX as f64, 0.0, crate::inputs::NUM_INPUTS as f64, Behavior::Free),
            length: reg.create(g, "length", 4.0, 1.0, 16.0, Behavior::Free),
            capture: reg.push(g, "capture"),
            capture_edge: Edge::default(),
            capture_bars: reg.create(g, "capture_bars", 4.0, 1.0, 8.0, Behavior::Free),
            undo: reg.push(g, "undo"),
            undo_edge: Edge::default(),
            redo: reg.push(g, "redo"),
            redo_edge: Edge::default(),
            after: reg.toggle(g, "after"),
            bpm: reg.create(g, "bpm", 120.0, 40.0, 240.0, Behavior::Free),
            following: reg.indicator(g, "following", 1.0),
            bar_beat: reg.indicator(g, "bar_beat", BAR),
            too_long: reg.indicator(g, "too_long", 1.0),
            layers: std::array::from_fn(|k| {
                let n = k + 1;
                LayerControls {
                    tap: reg.push(g, &format!("layer{n}_tap")),
                    tap_edge: Edge::default(),
                    clear: reg.push(g, &format!("layer{n}_clear")),
                    clear_edge: Edge::default(),
                    mute: reg.toggle(g, &format!("layer{n}_mute")),
                    volume: reg.create(g, &format!("layer{n}_volume"), 1.0, 0.0, 1.0, Behavior::Linear),
                    state: reg.indicator(g, &format!("layer{n}_state"), OVERDUBBING as f64),
                    bars: reg.indicator(g, &format!("layer{n}_bars"), 16.0),
                    position: reg.indicator(g, &format!("layer{n}_position"), 1.0),
                }
            }),
        }
    }
}

/// The nearest length from `lengths` to a control's value.
fn nearest(lengths: &[u32], v: f64) -> u32 {
    *lengths.iter().min_by(|a, b| (**a as f64 - v).abs().partial_cmp(&(**b as f64 - v).abs()).unwrap()).unwrap()
}

#[derive(Clone, Copy, Debug, Default)]
struct Layer {
    state: u8,
    /// Its loop: from beat `start`, `beats` long, held in `frames` frames.
    start: f64,
    beats: f64,
    frames: usize,
    /// While recording: the ring frame its first bar began at.
    from: u64,
    /// Copying out of the ring: the frames between `from` and `to`, of
    /// which `copied` are done.
    to: u64,
    copied: usize,
    copying: bool,
    /// Undone: kept (REDO brings it back) but silent and free for a new
    /// recording.
    undone: bool,
}

impl Layer {
    fn free(&self) -> bool {
        self.state == EMPTY
    }

    /// Where beat `b` falls in the loop's frames, and how far between two.
    fn place(&self, b: f64) -> (usize, f32) {
        let at = (b - self.start).rem_euclid(self.beats) / self.beats * self.frames as f64;
        let i = (at.floor() as usize).min(self.frames - 1);
        (i, (at - i as f64) as f32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Undo {
    Recorded(usize),
    Overdub(usize),
}

pub struct Looper {
    pub c: LooperControls,
    mem: Option<Box<LooperMemory>>,
    layers: [Layer; LAYERS],
    clock: Clock,
    rate: f64,
    /// Frames an input comes in behind the sound the player played along
    /// to (`[Master],input_latency`): bar lines are stamped that much later
    /// in the ring when recording an input, so the take lands on the beat.
    pub late: u64,
    /// The ring frame of the next buffer's first frame, and of the last
    /// buffer's; the ring holds audio for frames before `written`.
    t: u64,
    t_last: u64,
    written: u64,
    /// The last bar lines: (beat of the bar line, ring frame), newest last.
    stamps: [(f64, u64); STAMPS],
    stamped: usize,
    last_beat: f64,
    undo: Option<Undo>,
    undone: bool,
    gen: u32,
    was_playing: bool,
    too_long_for: usize,
}

/// What one buffer of a looper hears.
pub struct Heard<'a> {
    /// The picked input this buffer, played through the deck.
    pub input: Option<&'a [[f32; 2]]>,
    /// The main mix of the last buffer, when that is what it records.
    pub mix: Option<&'a [[f32; 2]]>,
}

impl Looper {
    pub fn new(reg: &ControlRegistry, deck: usize, rate: f64) -> Self {
        Self {
            c: LooperControls::new(reg, deck),
            mem: None,
            layers: [Layer::default(); LAYERS],
            clock: Clock::default(),
            rate,
            late: 0,
            t: 0,
            t_last: 0,
            written: 0,
            stamps: [(0.0, 0); STAMPS],
            stamped: 0,
            last_beat: 0.0,
            undo: None,
            undone: false,
            gen: 0,
            was_playing: false,
            too_long_for: 0,
        }
    }

    pub fn enabled(&self) -> bool {
        self.c.enabled.get_bool()
    }

    /// Enabled with its memory: the deck plays the looper.
    pub fn active(&self) -> bool {
        self.enabled() && self.mem.is_some()
    }

    /// The input it records and plays through (1 + the engine's input),
    /// or 0 for the main mix.
    pub fn input(&self) -> usize {
        self.c.input.get().round().clamp(0.0, crate::inputs::NUM_INPUTS as f64) as usize
    }

    /// Takes new memory; returns the old, to free off the audio thread.
    pub fn set_memory(&mut self, mem: Option<Box<LooperMemory>>) -> Option<Box<LooperMemory>> {
        self.layers = [Layer::default(); LAYERS];
        self.undo = None;
        self.stamped = 0;
        self.written = self.t;
        self.c.ready.set_bool(mem.is_some());
        self.publish_layers();
        std::mem::replace(&mut self.mem, mem)
    }

    /// Whether the memory should go back: off with memory still held.
    pub fn idle_memory(&self) -> bool {
        !self.enabled() && self.mem.is_some()
    }

    fn frames_for(&self, beats: f64, bpm: f64) -> usize {
        (beats * 60.0 / bpm.max(1.0) * self.rate).ceil() as usize
    }

    /// Room in the ring for a loop of `beats` at `bpm`, with a buffer's
    /// worth to spare and the seam's lead-in.
    fn fits(&self, beats: f64, bpm: f64) -> bool {
        let cap = self.mem.as_ref().map_or(0, |m| m.capacity());
        self.frames_for(beats, bpm) + crate::effects::BLOCK * 2 + SEAM < cap
    }

    fn refuse(&mut self) {
        self.too_long_for = (self.rate as usize).max(1);
        self.c.too_long.set_bool(true);
    }

    /// Plays one buffer into `out` (the layers, and the input when it plays through)
    /// and records what it hears. `play` is the deck's PLAY. Returns true when the
    /// deck should start playing (a layer was tapped or captured on a stopped deck).
    pub fn render(&mut self, play: bool, source: Source, heard: Heard, out: &mut [[f32; 2]]) -> bool {
        let frames = out.len();
        if self.mem.is_none() {
            out.fill([0.0; 2]);
            return false;
        }
        self.c.following.set_bool(source.beat.is_some());
        if source.beat.is_some() {
            self.c.bpm.set(source.bpm);
        }
        // Started with nothing to follow and nothing recorded: bar 1 starts now.
        if play && !self.was_playing && source.beat.is_none() && self.layers.iter().all(Layer::free) {
            self.clock.start(source);
            self.last_beat = 0.0;
        }
        if !self.clock.running {
            self.clock.start(source);
            self.last_beat = self.clock.beat;
        }
        self.was_playing = play;
        let mut start = false;

        // What the controls ask.
        let bpm = source.bpm;
        for k in 0..LAYERS {
            let l = &mut self.c.layers[k];
            if l.clear_edge.pressed(&l.clear) {
                self.layers[k] = Layer::default();
                if matches!(self.undo, Some(Undo::Recorded(j) | Undo::Overdub(j)) if j == k) {
                    self.undo = None;
                }
            }
            if l.tap_edge.pressed(&l.tap) {
                start |= self.tap(k, bpm);
            }
        }
        if self.c.capture_edge.pressed(&self.c.capture) {
            start |= self.capture(bpm);
        }
        if self.c.undo_edge.pressed(&self.c.undo) {
            self.take_back();
        }
        if self.c.redo_edge.pressed(&self.c.redo) {
            self.bring_back();
        }
        if self.too_long_for > 0 {
            self.too_long_for = self.too_long_for.saturating_sub(frames);
            if self.too_long_for == 0 {
                self.c.too_long.set_bool(false);
            }
        }

        let span = self.clock.advance(frames, self.rate, source);
        let mem = self.mem.as_mut().unwrap();
        let cap = mem.ring.len() as u64;
        // Into the ring: this buffer's input, or the last buffer's mix
        // where it was heard.
        if let Some(input) = heard.input {
            for (f, s) in input.iter().enumerate().take(frames) {
                mem.ring[((self.t + f as u64) % cap) as usize] = *s;
            }
            self.written = self.t + frames as u64;
        } else if let Some(mix) = heard.mix {
            let n = mix.len().min((self.t - self.t_last) as usize);
            for (f, s) in mix.iter().enumerate().take(n) {
                mem.ring[((self.t_last + f as u64) % cap) as usize] = *s;
            }
            self.written = self.t_last + n as u64;
        }

        // Copies out of the ring.
        for (k, layer) in self.layers.iter_mut().enumerate() {
            if layer.copying {
                copy_some(mem, k, layer, self.written);
            }
        }

        let thru = heard.input;
        let vols: [f32; LAYERS] = std::array::from_fn(|k| {
            if self.c.layers[k].mute.get_bool() {
                0.0
            } else {
                self.c.layers[k].volume.get() as f32
            }
        });
        for f in 0..frames {
            let b = span.beat_at(f);
            // Bar lines start and end recordings, and are stamped for CAPTURE.
            // A hair of tolerance, so rounding doesn't put a bar line a frame late.
            let bar = ((b + 1e-7) / BAR).floor();
            if b > self.last_beat && bar > ((self.last_beat + 1e-7) / BAR).floor() {
                let late = if heard.input.is_some() { self.late } else { 0 };
                let at = self.t + f as u64 + late;
                self.on_bar(bar * BAR, at);
            }
            self.last_beat = b;
            let mem = self.mem.as_mut().unwrap();
            let mut s = thru.map_or([0.0; 2], |i| i[f]);
            let dub = heard.input.map(|i| i[f]).or(heard.mix.and_then(|m| m.get(f).copied()));
            for (k, layer) in self.layers.iter().enumerate() {
                if !(layer.state == PLAYING || layer.state == OVERDUBBING) {
                    continue;
                }
                let (i, frac) = layer.place(b);
                let ready = if layer.copying { layer.copied } else { layer.frames };
                if i >= ready {
                    continue;
                }
                let buf = &mut mem.layers[k];
                if play {
                    let j = if i + 1 >= layer.frames { 0 } else { i + 1 };
                    let (a, c) = (buf[i], buf[j]);
                    let v = vols[k];
                    s[0] += (a[0] + (c[0] - a[0]) * frac) * v;
                    s[1] += (a[1] + (c[1] - a[1]) * frac) * v;
                }
                if layer.state == OVERDUBBING {
                    if let Some(x) = dub {
                        buf[i][0] += x[0];
                        buf[i][1] += x[1];
                        let block = i / PASS_BLOCK;
                        if mem.pass_gen[block] != self.gen {
                            let end = ((block + 1) * PASS_BLOCK).min(mem.pass.len());
                            mem.pass[block * PASS_BLOCK..end].fill([0.0; 2]);
                            mem.pass_gen[block] = self.gen;
                        }
                        mem.pass[i][0] += x[0];
                        mem.pass[i][1] += x[1];
                    }
                }
            }
            out[f] = s;
        }
        self.t_last = self.t;
        self.t += frames as u64;
        let b = self.last_beat;
        self.c.bar_beat.set(b.rem_euclid(BAR));
        for (k, layer) in self.layers.iter().enumerate() {
            let pos = if layer.state == PLAYING || layer.state == OVERDUBBING {
                (b - layer.start).rem_euclid(layer.beats) / layer.beats
            } else {
                0.0
            };
            self.c.layers[k].position.set(pos);
        }
        self.publish_layers();
        start && !play
    }

    fn publish_layers(&self) {
        for (k, layer) in self.layers.iter().enumerate() {
            self.c.layers[k].state.set(layer.state as f64);
            let bars = if layer.state == EMPTY { 0.0 } else { layer.beats / BAR };
            self.c.layers[k].bars.set(bars);
        }
    }

    /// A tap on layer `k`. True when the deck should start.
    fn tap(&mut self, k: usize, bpm: f64) -> bool {
        match self.layers[k].state {
            EMPTY => {
                let bars = nearest(&LENGTHS, self.c.length.get());
                let beats = bars as f64 * BAR;
                if !self.fits(beats, bpm) {
                    self.refuse();
                    return false;
                }
                if matches!(self.undo, Some(Undo::Recorded(j) | Undo::Overdub(j)) if j == k) {
                    self.undo = None;
                }
                self.layers[k] = Layer { state: ARMED, beats, ..Layer::default() };
                true
            }
            ARMED => {
                self.layers[k] = Layer::default();
                false
            }
            PLAYING => {
                self.overdub(k);
                false
            }
            OVERDUBBING => {
                self.layers[k].state = PLAYING;
                false
            }
            _ => false,
        }
    }

    fn overdub(&mut self, k: usize) {
        self.gen = self.gen.wrapping_add(1).max(1);
        self.undo = Some(Undo::Overdub(k));
        self.undone = false;
        self.layers[k].state = OVERDUBBING;
    }

    /// A bar line at beat `bar`, heard at ring frame `at`.
    fn on_bar(&mut self, bar: f64, at: u64) {
        if self.stamped == STAMPS {
            self.stamps.copy_within(1.., 0);
            self.stamped -= 1;
        }
        self.stamps[self.stamped] = (bar, at);
        self.stamped += 1;
        let after = self.c.after.get_bool();
        let mut finished = None;
        for k in 0..LAYERS {
            let layer = &mut self.layers[k];
            match layer.state {
                ARMED => {
                    layer.state = RECORDING;
                    layer.start = bar;
                    layer.from = at;
                }
                RECORDING if bar >= layer.start + layer.beats - 1e-6 => {
                    let frames = (at - layer.from) as usize;
                    if frames == 0 || !self_fits(self.mem.as_deref(), frames) {
                        *layer = Layer::default();
                        self.too_long_for = (self.rate as usize).max(1);
                        self.c.too_long.set_bool(true);
                        continue;
                    }
                    layer.frames = frames;
                    layer.to = at;
                    layer.copied = 0;
                    layer.copying = true;
                    layer.state = PLAYING;
                    finished = Some(k);
                }
                _ => {}
            }
        }
        if let Some(k) = finished {
            self.undo = Some(Undo::Recorded(k));
            self.undone = false;
            if after {
                self.overdub(k);
            }
        }
    }

    /// CAPTURE: the last bars heard into the first free layer.
    fn capture(&mut self, bpm: f64) -> bool {
        let bars = nearest(&CAPTURE_LENGTHS, self.c.capture_bars.get());
        let beats = bars as f64 * BAR;
        let Some(k) = self.layers.iter().position(Layer::free) else {
            self.refuse();
            return false;
        };
        let stamps = &self.stamps[..self.stamped];
        // The newest bar line with audio heard up to it.
        let Some(&(end_bar, to)) = stamps.iter().rev().find(|(_, t)| *t <= self.written) else {
            self.refuse();
            return false;
        };
        let Some(&(_, from)) = stamps.iter().find(|(b, _)| (*b - (end_bar - beats)).abs() < 1e-6) else {
            self.refuse();
            return false;
        };
        let frames = (to - from) as usize;
        let oldest = self.t.saturating_sub(self.mem.as_ref().map_or(0, |m| m.capacity()) as u64);
        if frames == 0
            || !self.fits(beats, bpm)
            || !self_fits(self.mem.as_deref(), frames)
            || from < oldest + SEAM as u64
        {
            self.refuse();
            return false;
        }
        self.layers[k] = Layer {
            state: PLAYING,
            start: end_bar - beats,
            beats,
            frames,
            from,
            to,
            copied: 0,
            copying: true,
            undone: false,
        };
        self.undo = Some(Undo::Recorded(k));
        self.undone = false;
        true
    }

    /// UNDO: cancels an armed or recording layer, else takes back the last
    /// recording or overdub.
    fn take_back(&mut self) {
        if let Some(k) = self.layers.iter().position(|l| l.state == ARMED || l.state == RECORDING) {
            self.layers[k] = Layer::default();
            return;
        }
        if self.undone {
            return;
        }
        match self.undo {
            Some(Undo::Recorded(k)) => {
                let l = &mut self.layers[k];
                if l.state == OVERDUBBING {
                    l.state = PLAYING;
                }
                l.undone = true;
                l.state = EMPTY;
                self.undone = true;
            }
            Some(Undo::Overdub(k)) => {
                if self.layers[k].state == OVERDUBBING {
                    self.layers[k].state = PLAYING;
                }
                self.apply_pass(k, -1.0);
                self.undone = true;
            }
            None => {}
        }
    }

    fn bring_back(&mut self) {
        if !self.undone {
            return;
        }
        match self.undo {
            Some(Undo::Recorded(k)) => {
                let l = &mut self.layers[k];
                if l.undone && l.state == EMPTY {
                    l.undone = false;
                    l.state = PLAYING;
                }
            }
            Some(Undo::Overdub(k)) => self.apply_pass(k, 1.0),
            None => {}
        }
        self.undone = false;
    }

    /// Adds (or with -1 takes away) the last overdub's pass from layer `k`.
    fn apply_pass(&mut self, k: usize, sign: f32) {
        let frames = self.layers[k].frames;
        let gen = self.gen;
        let Some(mem) = self.mem.as_mut() else {
            return;
        };
        for block in 0..frames.div_ceil(PASS_BLOCK) {
            if mem.pass_gen[block] != gen {
                continue;
            }
            let end = ((block + 1) * PASS_BLOCK).min(frames);
            for i in block * PASS_BLOCK..end {
                mem.layers[k][i][0] += sign * mem.pass[i][0];
                mem.layers[k][i][1] += sign * mem.pass[i][1];
            }
        }
    }

    /// A layer's audio, as it plays (for tests and saving).
    pub fn layer_audio(&self, k: usize) -> Option<&[[f32; 2]]> {
        let l = self.layers.get(k)?;
        if l.state == EMPTY {
            return None;
        }
        Some(&self.mem.as_ref()?.layers[k][..l.frames])
    }
}

fn self_fits(mem: Option<&LooperMemory>, frames: usize) -> bool {
    mem.is_some_and(|m| frames + SEAM < m.capacity())
}

/// Copies the next chunk of layer `k`'s loop out of the ring, up to what
/// has been heard; the last chunk blends the loop's end into the audio
/// just before its start, so it goes round without a click.
fn copy_some(mem: &mut LooperMemory, k: usize, layer: &mut Layer, written: u64) {
    let cap = mem.ring.len() as u64;
    let have = written.saturating_sub(layer.from) as usize;
    let until = layer.frames.min(have).min(layer.copied + COPY_CHUNK);
    for i in layer.copied..until {
        mem.layers[k][i] = mem.ring[((layer.from + i as u64) % cap) as usize];
    }
    layer.copied = layer.copied.max(until);
    if layer.copied < layer.frames {
        return;
    }
    layer.copying = false;
    let n = SEAM.min(layer.frames / 2);
    for j in 0..n {
        let w = (j + 1) as f32 / (n + 1) as f32;
        let i = layer.frames - n + j;
        let pre = mem.ring[((layer.from + cap - (n - j) as u64) % cap) as usize];
        let x = &mut mem.layers[k][i];
        x[0] = x[0] * (1.0 - w) + pre[0] * w;
        x[1] = x[1] * (1.0 - w) + pre[1] * w;
    }
}

/// The app side of the loopers' memory: makes it when a deck becomes a
/// looper and frees it when it stops being one, off the audio thread.
pub struct LooperSupply {
    pub(crate) to_engine: rtrb::Producer<(usize, Box<LooperMemory>)>,
    pub(crate) from_engine: rtrb::Consumer<Box<LooperMemory>>,
}

/// The engine's ends of [`LooperSupply`].
pub struct LooperIntake {
    pub(crate) from_app: rtrb::Consumer<(usize, Box<LooperMemory>)>,
    pub(crate) to_app: rtrb::Producer<Box<LooperMemory>>,
}

pub fn channel(decks: usize) -> (LooperSupply, LooperIntake) {
    let (to_engine, from_app) = rtrb::RingBuffer::new(decks * 2);
    let (to_app, from_engine) = rtrb::RingBuffer::new(decks * 2);
    (LooperSupply { to_engine, from_engine }, LooperIntake { from_app, to_app })
}

/// Watches `[LooperN],enabled` and `ready` on its own thread: a looper
/// turned on without memory gets some, and memory handed back is freed.
pub fn supply(mut s: LooperSupply, registry: Weak<ControlRegistry>, decks: usize, rate: f64) {
    let spawned = std::thread::Builder::new().name("looper-memory".into()).spawn(move || {
        let mut sent = vec![false; decks];
        loop {
            let Some(reg) = registry.upgrade() else {
                return;
            };
            while let Ok(old) = s.from_engine.pop() {
                drop(old);
            }
            for (deck, sent) in sent.iter_mut().enumerate() {
                let g = looper_group(deck);
                let on = reg.get(&g, "enabled").is_some_and(|c| c.get_bool());
                let ready = reg.get(&g, "ready").is_some_and(|c| c.get_bool());
                if on && !ready && !*sent {
                    *sent = s.to_engine.push((deck, LooperMemory::new(rate))).is_ok();
                } else if ready || !on {
                    *sent = false;
                }
            }
            drop(reg);
            std::thread::sleep(Duration::from_millis(40));
        }
    });
    if let Err(e) = spawned {
        eprintln!("could not start the looper memory thread: {e}");
    }
}

/// Every deck's looper and its memory coming and going.
pub struct Loopers {
    pub loopers: Vec<Looper>,
    intake: LooperIntake,
}

impl Loopers {
    pub fn new(reg: &ControlRegistry, decks: usize, rate: f64, intake: LooperIntake) -> Self {
        Self { loopers: (0..decks).map(|d| Looper::new(reg, d, rate)).collect(), intake }
    }

    /// Takes memory sent and hands back memory of loopers turned off.
    pub fn update(&mut self) {
        while let Ok((deck, mem)) = self.intake.from_app.pop() {
            match self.loopers.get_mut(deck) {
                Some(l) if l.enabled() && l.mem.is_none() => {
                    l.set_memory(Some(mem));
                }
                _ => {
                    let _ = self.intake.to_app.push(mem);
                }
            }
        }
        for l in &mut self.loopers {
            if l.idle_memory() && self.intake.to_app.slots() > 0 {
                if let Some(old) = l.set_memory(None) {
                    let _ = self.intake.to_app.push(old);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slow rate keeps the buffers small: a bar at 120 BPM is 16 000 frames.
    const RATE: f64 = 8_000.0;
    const BUF: usize = 200;
    const BAR_FRAMES: usize = 16_000;

    fn looper() -> (Arc<ControlRegistry>, Looper) {
        let reg = ControlRegistry::new();
        let mut l = Looper::new(&reg, 0, RATE);
        l.c.enabled.set_bool(true);
        l.set_memory(Some(LooperMemory::new(RATE)));
        (reg, l)
    }

    fn own() -> Source {
        Source { bpm: 120.0, beat: None }
    }

    /// Runs `buffers` buffers hearing `input(frame)` as an input; returns
    /// what came out, one value (left) per frame.
    fn run(l: &mut Looper, play: bool, from: usize, buffers: usize, input: impl Fn(usize) -> f32) -> Vec<f32> {
        let mut out = Vec::new();
        for b in 0..buffers {
            let start = from + b * BUF;
            let inp: Vec<[f32; 2]> = (0..BUF).map(|f| [input(start + f); 2]).collect();
            let mut o = vec![[0.0; 2]; BUF];
            l.render(play, own(), Heard { input: Some(&inp), mix: None }, &mut o);
            out.extend(o.iter().map(|s| s[0]));
        }
        out
    }

    fn press(c: &ControlHandle) {
        c.set(1.0);
    }

    fn release(c: &ControlHandle) {
        c.set(0.0);
    }

    #[test]
    fn records_a_layer_on_the_bar_and_plays_it_in_time() {
        let (_reg, mut l) = looper();
        l.c.length.set(1.0);
        let (tap, state) = (l.c.layers[0].tap.clone(), l.c.layers[0].state.clone());
        press(&tap);
        // The first buffer: armed, waiting for bar 2.
        run(&mut l, true, 0, 1, |_| 0.0);
        release(&tap);
        assert_eq!(state.get() as u8, ARMED);
        // Bar 2 plays a ramp that is recorded; bar 3 plays silence.
        let ramp = |f: usize| {
            if (BAR_FRAMES..2 * BAR_FRAMES).contains(&f) {
                (f - BAR_FRAMES) as f32 / BAR_FRAMES as f32
            } else {
                0.0
            }
        };
        run(&mut l, true, BUF, BAR_FRAMES / BUF + 5, ramp);
        assert_eq!(state.get() as u8, RECORDING);
        let out = run(&mut l, true, BAR_FRAMES + BUF * 6, (2 * BAR_FRAMES) / BUF, ramp);
        assert_eq!(state.get() as u8, PLAYING);
        assert_eq!(l.c.layers[0].bars.get(), 1.0);
        // Bar 3 plays the ramp back from its first beat, lined up with the bar.
        let bar3 = 2 * BAR_FRAMES - (BAR_FRAMES + BUF * 6);
        for k in [100, 4_000, 8_000, 12_000] {
            let expected = k as f32 / BAR_FRAMES as f32;
            assert!((out[bar3 + k] - expected).abs() < 0.01, "frame {k}: {} vs {expected}", out[bar3 + k]);
        }
    }

    #[test]
    fn an_input_heard_late_is_recorded_on_the_beat() {
        const LATE: usize = 1_000;
        let (_reg, mut l) = looper();
        l.late = LATE as u64;
        l.c.length.set(1.0);
        let (tap, state) = (l.c.layers[0].tap.clone(), l.c.layers[0].state.clone());
        press(&tap);
        run(&mut l, true, 0, 1, |_| 0.0);
        release(&tap);
        // The player plays bar 2's ramp on time; it reaches the input late.
        let ramp = |f: usize| {
            let f = f.wrapping_sub(LATE);
            if (BAR_FRAMES..2 * BAR_FRAMES).contains(&f) {
                (f - BAR_FRAMES) as f32 / BAR_FRAMES as f32
            } else {
                0.0
            }
        };
        run(&mut l, true, BUF, BAR_FRAMES / BUF + 5, ramp);
        let out = run(&mut l, true, BAR_FRAMES + BUF * 6, (2 * BAR_FRAMES) / BUF, ramp);
        assert_eq!(state.get() as u8, PLAYING);
        let bar3 = 2 * BAR_FRAMES - (BAR_FRAMES + BUF * 6);
        // Past the tail of bar 2 still coming in (and heard through).
        for k in [2_000, 4_000, 8_000, 12_000] {
            let expected = k as f32 / BAR_FRAMES as f32;
            assert!((out[bar3 + k] - expected).abs() < 0.01, "frame {k}: {} vs {expected}", out[bar3 + k]);
        }
    }

    #[test]
    fn capture_takes_the_last_bars_heard() {
        let (_reg, mut l) = looper();
        l.c.capture_bars.set(1.0);
        // Each bar plays its own level: bar 1 is 0.1, bar 2 0.2, and so on.
        let level = |f: usize| (f / BAR_FRAMES + 1) as f32 / 10.0;
        let half = BAR_FRAMES / 2;
        run(&mut l, true, 0, (3 * BAR_FRAMES + half) / BUF, level);
        press(&l.c.capture.clone());
        run(&mut l, true, 3 * BAR_FRAMES + half, 2, level);
        assert_eq!(l.c.layers[0].state.get() as u8, PLAYING);
        // Copied over the next buffers: bar 3 (0.3), the last whole bar.
        run(&mut l, true, 3 * BAR_FRAMES + half + 2 * BUF, 4, level);
        let audio = l.layer_audio(0).unwrap();
        assert_eq!(audio.len(), BAR_FRAMES);
        assert!((audio[BAR_FRAMES / 2][0] - 0.3).abs() < 1e-6);
        // The end blends into the bar before, so it goes round smoothly.
        let near_end = audio[audio.len() - 128][0];
        assert!((near_end - 0.25).abs() < 0.01, "{near_end}");
    }

    #[test]
    fn undo_takes_back_an_overdub_and_redo_brings_it_back() {
        let (_reg, mut l) = looper();
        l.c.length.set(1.0);
        let tap = l.c.layers[0].tap.clone();
        press(&tap);
        run(&mut l, true, 0, 1, |_| 0.0);
        release(&tap);
        // Records 0.25 over bar 2, then plays it.
        let mut at = BUF;
        let n = (2 * BAR_FRAMES - at) / BUF + 3;
        run(&mut l, true, at, n, |_| 0.25);
        at += n * BUF;
        run(&mut l, true, at, 2, |_| 0.0);
        at += 2 * BUF;
        assert_eq!(l.c.layers[0].state.get() as u8, PLAYING);
        assert!((l.layer_audio(0).unwrap()[5_000][0] - 0.25).abs() < 1e-6);
        // Overdub 0.5 for a whole loop.
        press(&tap);
        run(&mut l, true, at, 1, |_| 0.5);
        release(&tap);
        at += BUF;
        run(&mut l, true, at, BAR_FRAMES / BUF + 1, |_| 0.5);
        at += (BAR_FRAMES / BUF + 1) * BUF;
        press(&tap);
        run(&mut l, true, at, 1, |_| 0.0);
        release(&tap);
        at += BUF;
        assert_eq!(l.c.layers[0].state.get() as u8, PLAYING);
        assert!((l.layer_audio(0).unwrap()[5_000][0] - 0.75).abs() < 1e-6);
        press(&l.c.undo.clone());
        run(&mut l, true, at, 1, |_| 0.0);
        at += BUF;
        assert!((l.layer_audio(0).unwrap()[5_000][0] - 0.25).abs() < 1e-6);
        press(&l.c.redo.clone());
        run(&mut l, true, at, 1, |_| 0.0);
        assert!((l.layer_audio(0).unwrap()[5_000][0] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn undo_takes_back_a_recording_and_too_long_is_refused() {
        let (_reg, mut l) = looper();
        l.c.length.set(1.0);
        press(&l.c.capture_bars.clone());
        l.c.capture_bars.set(1.0);
        run(&mut l, true, 0, (2 * BAR_FRAMES) / BUF + 2, |_| 0.1);
        press(&l.c.capture.clone());
        run(&mut l, true, 0, 3, |_| 0.1);
        assert_eq!(l.c.layers[0].state.get() as u8, PLAYING);
        press(&l.c.undo.clone());
        run(&mut l, true, 0, 1, |_| 0.1);
        assert_eq!(l.c.layers[0].state.get() as u8, EMPTY);
        press(&l.c.redo.clone());
        run(&mut l, true, 0, 1, |_| 0.1);
        assert_eq!(l.c.layers[0].state.get() as u8, PLAYING);
        // 16 bars at 40 BPM is 96 s, past the 48 s a layer holds.
        l.c.length.set(16.0);
        l.c.bpm.set(40.0);
        let tap = l.c.layers[1].tap.clone();
        press(&tap);
        let mut o = vec![[0.0; 2]; BUF];
        let inp = vec![[0.0; 2]; BUF];
        l.render(true, Source { bpm: 40.0, beat: None }, Heard { input: Some(&inp), mix: None }, &mut o);
        assert_eq!(l.c.layers[1].state.get() as u8, EMPTY);
        assert!(l.c.too_long.get_bool());
    }
}
