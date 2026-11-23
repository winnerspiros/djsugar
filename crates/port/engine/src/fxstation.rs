//! The FX Station: a remix effector on the main mix, in the style of the remix stations of DJ booths. Four sections, all locked to the beat grid of the deck that leads (or to a tapped tempo): - Scene: build ups (BPF Echo, Echo, Noise, Spiral Up, Reverb Up) and break downs
//! (HPF Echo, LPF Echo, Crush Echo, Spiral Down, Reverb Down) on a big dial, with two sub parameters. - Isolate: three bands (low, mid, high) with Isolator, Cut/Add, Trans/Roll and Gate/Drive modes. - X-Pad: kick, snare, clap and hi-hat rolls from 1/1 to 1/16 of a beat,
//! with pitch, an overdub loop of a bar, mutes and four banks. - Release: Vinyl Brake, Echo or Back Spin, turning everything else off. Controls are in the [FxStation] group. Processed on the engine thread, without allocations or locks; it costs almost nothing while idle.

pub mod dsp;
pub mod fdnreverb;
pub mod instruments;
pub mod scenes;
pub mod vinylmotion;

use std::f32::consts::TAU;
use std::sync::Arc;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};

use crate::fxstation::dsp::{soft_clip, Crossover, DelayLine, PitchShifter, Smoother, Svf};
use crate::fxstation::fdnreverb::FdnReverb;
use crate::fxstation::instruments::{Instrument, Instruments};
use crate::fxstation::scenes::{gate_gain, Drive, Flanger, Phaser, Roll, SweepFilter, WsolaPitch};
use crate::fxstation::vinylmotion::{BackSpinMotion, NeedleShaper};

const TWO_PI: f32 = TAU;

const K_DECKS: usize = 4;
const K_BANDS: usize = 3;
const K_VOICES: usize = 16;
const K_INSTRUMENTS: usize = 4;
const K_BANKS: usize = 4;
const K_LOOP_STEPS: usize = 64;

const K_MAX_SAMPLE_RATE: f32 = 96000.0;
const K_MAX_ECHO_SECONDS: f32 = 2.2;
const K_HISTORY_SECONDS: f32 = 4.0;
const K_MAX_ROLL_SECONDS: f32 = 0.5;
const K_MAX_SCENE_ROLL_SECONDS: f32 = 2.0;

const K_SWEEP_BEATS: [f64; 6] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
const K_SLICE_BEATS: [f64; 5] = [1.0 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0];
const K_ECHO_BEATS: [f64; 6] = [0.125, 0.25, 0.375, 0.5, 0.75, 1.0];
const K_ECHO_BEAT_COUNT: usize = 6;
const K_SILENT_LEVEL: f32 = 1e-5;
const K_LOW_MID_FREQUENCY: f32 = 300.0;
const K_MID_HIGH_FREQUENCY: f32 = 3000.0;
const K_PITCH_WINDOW_FRAMES: usize = 2048;
const K_BACK_SPIN_PEAK: f64 = 8.0;

const K_GROUP: &str = "[FxStation]";

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    BpfEcho,
    Echo,
    Noise,
    SpiralUp,
    ReverbUp,
    HpfEcho,
    LpfEcho,
    CrushEcho,
    SpiralDown,
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
    const COUNT: usize = 18;

    fn from_index(idx: usize) -> Self {
        match idx {
            0 => Scene::BpfEcho,
            1 => Scene::Echo,
            2 => Scene::Noise,
            3 => Scene::SpiralUp,
            4 => Scene::ReverbUp,
            5 => Scene::HpfEcho,
            6 => Scene::LpfEcho,
            7 => Scene::CrushEcho,
            8 => Scene::SpiralDown,
            9 => Scene::ReverbDown,
            10 => Scene::Flanger,
            11 => Scene::Phaser,
            12 => Scene::Filter,
            13 => Scene::Roll,
            14 => Scene::Gater,
            15 => Scene::Pitch,
            16 => Scene::Drive,
            _ => Scene::BpfEcho,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolateMode {
    Isolator,
    CutAdd,
    TransRoll,
    GateDrive,
}

impl IsolateMode {
    fn from_index(idx: usize) -> Self {
        match idx {
            0 => IsolateMode::Isolator,
            1 => IsolateMode::CutAdd,
            2 => IsolateMode::TransRoll,
            3 => IsolateMode::GateDrive,
            _ => IsolateMode::Isolator,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseType {
    VinylBrake,
    Echo,
    BackSpin,
}

impl ReleaseType {
    fn from_index(idx: usize) -> Self {
        match idx {
            0 => ReleaseType::VinylBrake,
            1 => ReleaseType::Echo,
            2 => ReleaseType::BackSpin,
            _ => ReleaseType::VinylBrake,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DeckHandles {
    play: Option<ControlHandle>,
    bpm: Option<ControlHandle>,
    beat_distance: Option<ControlHandle>,
    volume: Option<ControlHandle>,
    sync_leader: Option<ControlHandle>,
}

#[derive(Clone, Debug, Default)]
struct Voice {
    instrument: Option<Arc<Instrument>>,
    instrument_idx: usize,
    position: f64,
    rate: f64,
    gain: f32,
    fade: f32,
    active: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct BandRoll {
    length: usize,
    recorded: usize,
    play_position: usize,
    active: bool,
}

/// The FX Station effect processor.
pub struct FxStation {
    // Controls
    scene_type: ControlHandle,
    scene_on: ControlHandle,
    scene_dial: ControlHandle,
    scene_param1: ControlHandle,
    scene_param2: ControlHandle,
    isolate_mode: ControlHandle,
    isolate_bands: [ControlHandle; K_BANDS],
    xpad_instrument: ControlHandle,
    source_input: ControlHandle,
    source_xpad: ControlHandle,
    xpad_division: ControlHandle,
    xpad_rolls: [ControlHandle; 5],
    xpad_hits: [ControlHandle; K_INSTRUMENTS],
    xpad_pitch: ControlHandle,
    xpad_volume: ControlHandle,
    xpad_overdub: ControlHandle,
    xpad_mutes: [ControlHandle; K_INSTRUMENTS],
    xpad_clear: ControlHandle,
    xpad_bank: ControlHandle,
    xpad_reload: ControlHandle,
    xpad_bank_sounds: [ControlHandle; K_BANKS],
    xpad_loop_steps: ControlHandle,
    release_type_ctrl: ControlHandle,
    release_ctrl: ControlHandle,
    release_active_ctrl: ControlHandle,
    bpm_ctrl: ControlHandle,
    bpm_auto: ControlHandle,
    tap: ControlHandle,
    nudge_ctrl: ControlHandle,
    quantize: ControlHandle,
    beat_ctrl: ControlHandle,
    source: ControlHandle,

    // Deck handles (resolved at process time)
    deck_handles: [DeckHandles; K_DECKS],

    // Instruments
    instruments: Instruments,

    // Sample rate
    sample_rate: f32,

    // Clock
    beat: f64,
    bpm: f64,
    nudge: f64,
    frame_count: u64,
    taps: [u64; 8],
    tap_count: usize,
    previous_tap: bool,
    previous_release: bool,
    previous_clear: bool,
    previous_hits: [bool; K_INSTRUMENTS],
    previous_division: usize,
    roll_instrument: usize,
    roll_origin: f64,

    // Scene
    echo: DelayLine,
    release_echo: DelayLine,
    spiral: PitchShifter,
    reverb: FdnReverb,
    scene_filter: Svf,
    wet_filter: Svf,
    scene_send: Smoother,
    scene_amount: Smoother,
    echo_delay: Smoother,
    wet_level: f32,
    crush_hold: [f32; 2],
    crush_counter: usize,
    scene_silent_frames: usize,
    scene_active: bool,
    last_scene_type: usize,

    // Added scenes
    scene_mix: Smoother,
    scene_dial_value: Smoother,
    scene_mix_target: f32,
    scene_dial_target: f32,
    scene_coefficient: f32,
    flanger: Flanger,
    phaser: Phaser,
    sweep_filter: SweepFilter,
    roll: Roll,
    pitch: WsolaPitch,
    drive: Drive,

    // Isolate
    low_mid: Crossover,
    mid_high: Crossover,
    low_all_pass: Crossover,
    band_gain: [Smoother; K_BANDS],
    band_envelope: [f32; K_BANDS],
    roll_buffers: [DelayLine; K_BANDS],
    rolls: [BandRoll; K_BANDS],
    isolate_mix: Smoother,
    isolate_active: bool,

    // X-Pad
    voices: [Voice; K_VOICES],
    loop_pattern: [[bool; K_LOOP_STEPS]; K_INSTRUMENTS],
    loop_has_steps: bool,

    // Release
    history: DelayLine,
    release_active: bool,
    release_waiting: bool,
    release_type: ReleaseType,
    release_elapsed: f64,
    release_read: f64,
    back_spin_needle: NeedleShaper,
    live_gain: Smoother,

    // Random
    random: fastrand::Rng,
}

impl FxStation {
    /// Creates a new FX Station with the given control registry.
    pub fn new(reg: &Arc<ControlRegistry>, sample_rate: f32) -> Self {
        let make_control = |key: &str, default: f64| -> ControlHandle {
            let h = ControlRegistry::create(reg, K_GROUP, key, default, f64::MIN, f64::MAX, Behavior::Free);
            h.set(default);
            h
        };

        let make_knob = |key: &str, min: f64, max: f64, default: f64| -> ControlHandle {
            let h = ControlRegistry::create(reg, K_GROUP, key, default, min, max, Behavior::Linear);
            h.set(default);
            h
        };

        let make_button = |key: &str, toggle: bool, states: usize, default: f64| -> ControlHandle {
            let behavior = if toggle { Behavior::Toggle } else { Behavior::Push };
            let h = ControlRegistry::create(reg, K_GROUP, key, default, 0.0, (states - 1) as f64, behavior);
            h.set(default);
            h
        };

        let scene_type = make_button("scene_type", true, Scene::COUNT, 0.0);
        let scene_on = make_button("scene_on", true, 2, 0.0);
        let scene_dial = make_knob("scene_dial", 0.0, 1.0, 0.5);
        let scene_param1 = make_knob("scene_param1", 0.0, 1.0, 0.5);
        let scene_param2 = make_knob("scene_param2", 0.0, 1.0, 0.5);
        let isolate_mode = make_button("isolate_mode", true, 4, 0.0);
        let isolate_bands = [
            make_knob("isolate_low", -1.0, 1.0, 0.0),
            make_knob("isolate_mid", -1.0, 1.0, 0.0),
            make_knob("isolate_high", -1.0, 1.0, 0.0),
        ];
        let xpad_instrument = make_button("xpad_instrument", true, K_INSTRUMENTS, 0.0);
        let source_input = make_button("fx_source_input", true, 2, 1.0);
        let source_xpad = make_button("fx_source_xpad", true, 2, 0.0);
        let xpad_division = make_control("xpad_division", 0.0);
        let xpad_rolls = [
            make_button("xpad_roll_1", false, 2, 0.0),
            make_button("xpad_roll_2", false, 2, 0.0),
            make_button("xpad_roll_3", false, 2, 0.0),
            make_button("xpad_roll_4", false, 2, 0.0),
            make_button("xpad_roll_5", false, 2, 0.0),
        ];
        let xpad_hits = [
            make_button("xpad_hit_1", false, 2, 0.0),
            make_button("xpad_hit_2", false, 2, 0.0),
            make_button("xpad_hit_3", false, 2, 0.0),
            make_button("xpad_hit_4", false, 2, 0.0),
        ];
        let xpad_pitch = make_knob("xpad_pitch", -1.0, 1.0, 0.0);
        let xpad_volume = make_knob("xpad_volume", 0.0, 1.0, 0.8);
        let xpad_overdub = make_button("xpad_overdub", true, 2, 0.0);
        let xpad_mutes = [
            make_button("xpad_mute_1", true, 2, 0.0),
            make_button("xpad_mute_2", true, 2, 0.0),
            make_button("xpad_mute_3", true, 2, 0.0),
            make_button("xpad_mute_4", true, 2, 0.0),
        ];
        let xpad_clear = make_button("xpad_clear", false, 2, 0.0);
        let xpad_bank = make_button("xpad_bank", true, K_BANKS, 0.0);
        let xpad_reload = make_button("xpad_reload", false, 2, 0.0);
        let xpad_bank_sounds = [
            make_control("xpad_bank_sounds_1", 0.0),
            make_control("xpad_bank_sounds_2", 0.0),
            make_control("xpad_bank_sounds_3", 0.0),
            make_control("xpad_bank_sounds_4", 0.0),
        ];
        let xpad_loop_steps = make_control("xpad_loop_steps", 0.0);
        let release_type_ctrl = make_button("release_type", true, 3, 0.0);
        let release_ctrl = make_button("release", false, 2, 0.0);
        let release_active_ctrl = make_control("release_active", 0.0);
        let bpm_ctrl = make_control("bpm", 120.0);
        let bpm_auto = make_button("bpm_auto", true, 2, 1.0);
        let tap = make_button("tap", false, 2, 0.0);
        let nudge_ctrl = make_control("nudge", 0.0);
        let quantize = make_button("quantize", true, 2, 1.0);
        let beat_ctrl = make_control("beat", 0.0);
        let source = make_control("source", 0.0);

        // Pre-allocate all buffers at max sample rate
        let max_rate = K_MAX_SAMPLE_RATE as usize;
        let mut echo = DelayLine::default();
        echo.allocate((K_MAX_ECHO_SECONDS * K_MAX_SAMPLE_RATE) as usize);
        let mut release_echo = DelayLine::default();
        release_echo.allocate((K_MAX_ECHO_SECONDS * K_MAX_SAMPLE_RATE) as usize);
        let mut history = DelayLine::default();
        history.allocate((K_HISTORY_SECONDS * K_MAX_SAMPLE_RATE) as usize);

        let mut spiral = PitchShifter::default();
        spiral.allocate(K_PITCH_WINDOW_FRAMES);

        let mut reverb = FdnReverb::default();
        reverb.setup(K_MAX_SAMPLE_RATE);
        reverb.set_modulation(0.9);

        let mut flanger = Flanger::default();
        flanger.allocate(K_MAX_SAMPLE_RATE);

        let mut phaser = Phaser::default();

        let mut roll = Roll::default();
        roll.allocate(K_MAX_SAMPLE_RATE, K_MAX_SCENE_ROLL_SECONDS);

        let mut pitch = WsolaPitch::default();
        pitch.allocate(K_MAX_SAMPLE_RATE);

        let mut roll_buffers = std::array::from_fn(|_| DelayLine::default());
        for buffer in &mut roll_buffers {
            buffer.allocate((K_MAX_ROLL_SECONDS * K_MAX_SAMPLE_RATE) as usize);
        }

        let instruments = Instruments::new();

        // Resolve deck handles upfront
        let mut deck_handles: [DeckHandles; K_DECKS] = std::array::from_fn(|_| DeckHandles::default());
        for i in 0..K_DECKS {
            let group = format!("[Channel{}]", i + 1);
            deck_handles[i].play = ControlRegistry::get(reg, &group, "play");
            deck_handles[i].bpm = ControlRegistry::get(reg, &group, "bpm");
            deck_handles[i].beat_distance = ControlRegistry::get(reg, &group, "beat_distance");
            deck_handles[i].volume = ControlRegistry::get(reg, &group, "volume");
            deck_handles[i].sync_leader = ControlRegistry::get(reg, &group, "sync_leader");
        }

        Self {
            scene_type,
            scene_on,
            scene_dial,
            scene_param1,
            scene_param2,
            isolate_mode,
            isolate_bands,
            xpad_instrument,
            source_input,
            source_xpad,
            xpad_division,
            xpad_rolls,
            xpad_hits,
            xpad_pitch,
            xpad_volume,
            xpad_overdub,
            xpad_mutes,
            xpad_clear,
            xpad_bank,
            xpad_reload,
            xpad_bank_sounds,
            xpad_loop_steps,
            release_type_ctrl,
            release_ctrl,
            release_active_ctrl,
            bpm_ctrl,
            bpm_auto,
            tap,
            nudge_ctrl,
            quantize,
            beat_ctrl,
            source,
            deck_handles: std::array::from_fn(|_| DeckHandles::default()),
            instruments,
            sample_rate: 0.0,
            beat: 0.0,
            bpm: 120.0,
            nudge: 0.0,
            frame_count: 0,
            taps: [0; 8],
            tap_count: 0,
            previous_tap: false,
            previous_release: false,
            previous_clear: false,
            previous_hits: [false; K_INSTRUMENTS],
            previous_division: 0,
            roll_instrument: 0,
            roll_origin: 0.0,
            echo,
            release_echo,
            spiral,
            reverb,
            scene_filter: Svf::default(),
            wet_filter: Svf::default(),
            scene_send: Smoother::default(),
            scene_amount: Smoother::default(),
            echo_delay: Smoother::default(),
            wet_level: 0.0,
            crush_hold: [0.0, 0.0],
            crush_counter: 0,
            scene_silent_frames: 0,
            scene_active: false,
            last_scene_type: usize::MAX,
            scene_mix: Smoother::default(),
            scene_dial_value: Smoother::default(),
            scene_mix_target: 0.0,
            scene_dial_target: 0.5,
            scene_coefficient: 1.0,
            flanger,
            phaser,
            sweep_filter: SweepFilter::default(),
            roll,
            pitch,
            drive: Drive::default(),
            low_mid: Crossover::default(),
            mid_high: Crossover::default(),
            low_all_pass: Crossover::default(),
            band_gain: [Smoother::default(); K_BANDS],
            band_envelope: [0.0; K_BANDS],
            roll_buffers,
            rolls: [BandRoll::default(); K_BANDS],
            isolate_mix: Smoother::default(),
            isolate_active: false,
            voices: std::array::from_fn(|_| Voice::default()),
            loop_pattern: [[false; K_LOOP_STEPS]; K_INSTRUMENTS],
            loop_has_steps: false,
            history,
            release_active: false,
            release_waiting: false,
            release_type: ReleaseType::VinylBrake,
            release_elapsed: 0.0,
            release_read: 0.0,
            back_spin_needle: NeedleShaper::default(),
            live_gain: Smoother::default(),
            random: fastrand::Rng::with_seed(1234),
        }
    }

    /// Prepares the FX Station for a new sample rate.
    pub fn prepare(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let rate = sample_rate.min(K_MAX_SAMPLE_RATE);
        self.reverb.setup(rate);
        self.reverb.set_modulation(0.9);
        self.pitch.set_sample_rate(rate);
        self.low_mid.set(K_LOW_MID_FREQUENCY, sample_rate);
        self.mid_high.set(K_MID_HIGH_FREQUENCY, sample_rate);
        self.low_all_pass.set(K_MID_HIGH_FREQUENCY, sample_rate);
        self.echo.clear();
        self.release_echo.clear();
        self.spiral.clear();
        self.history.clear();
        self.scene_filter.reset();
        self.wet_filter.reset();
        self.low_mid.reset();
        self.mid_high.reset();
        self.low_all_pass.reset();
        self.live_gain.reset(1.0);
    }

    fn beat_frames(&self) -> f64 {
        60.0 / self.bpm.max(20.0) * self.sample_rate as f64
    }

    fn fraction(x: f64) -> f64 {
        x - x.floor()
    }

    fn pick<T: Copy>(values: &[T], knob: f32) -> T {
        let n = values.len();
        let idx = (knob * n as f32).clamp(0.0, (n - 1) as f32) as usize;
        values[idx]
    }

    fn update_clock(&mut self, frames: usize) {
        // The leading deck: the sync leader if it plays, else the loudest one playing.
        let mut source = None;
        if self.bpm_auto.get() > 0.0 {
            let mut loudest = -1.0;
            for i in 0..K_DECKS {
                let deck = &self.deck_handles[i];
                let p_play = deck.play.as_ref();
                let p_bpm = deck.bpm.as_ref();
                if p_play.is_none() || p_bpm.is_none() {
                    continue;
                }
                if p_play.unwrap().get() <= 0.0 || p_bpm.unwrap().get() <= 0.0 {
                    continue;
                }
                let p_leader = deck.sync_leader.as_ref();
                let p_volume = deck.volume.as_ref();
                let loudness = if let Some(l) = p_leader {
                    if l.get() > 0.0 {
                        10.0
                    } else {
                        0.0
                    }
                } else {
                    0.0
                } + p_volume.map_or(1.0, |v| v.get() as f64);
                if loudness > loudest {
                    loudest = loudness;
                    source = Some(i);
                }
            }
        }

        if let Some(idx) = source {
            let deck = &self.deck_handles[idx];
            if let Some(p_bpm) = deck.bpm.as_ref() {
                self.bpm = p_bpm.get();
            }
            if let Some(p_dist) = deck.beat_distance.as_ref() {
                let diff = Self::fraction(p_dist.get() + self.nudge) - Self::fraction(self.beat);
                let diff = if diff > 0.5 {
                    diff - 1.0
                } else if diff < -0.5 {
                    diff + 1.0
                } else {
                    diff
                };
                self.beat += diff;
            }
        }

        self.source.set(source.map_or(0, |i| i + 1) as f64);

        // Nudge: a hundredth of a beat per tenth of a second while held.
        let nudge_val = self.nudge_ctrl.get();
        if nudge_val != 0.0 {
            let step = 0.1 * nudge_val * frames as f64 / self.sample_rate as f64;
            if source.is_some() {
                self.nudge += step;
            } else {
                self.beat += step;
            }
        }

        // Tap: the tempo of the last taps, the beat on the last one.
        let tap = self.tap.get() > 0.0;
        if tap && !self.previous_tap {
            if self.tap_count > 0
                && self.frame_count - self.taps[(self.tap_count - 1) % self.taps.len()] > 2 * self.sample_rate as u64
            {
                self.tap_count = 0;
            }
            self.taps[self.tap_count % self.taps.len()] = self.frame_count;
            self.tap_count += 1;
            let taps = self.tap_count.min(self.taps.len());
            if taps >= 2 {
                let first = self.taps[(self.tap_count - taps) % self.taps.len()];
                let interval = (self.frame_count - first) as f64 / (taps - 1) as f64;
                self.bpm = (60.0 * self.sample_rate as f64 / interval).clamp(40.0, 240.0);
                self.bpm_auto.set(0.0);
            }
            self.beat = self.beat.round();
        }
        self.previous_tap = tap;

        if (self.bpm_ctrl.get() - self.bpm).abs() > 0.005 {
            self.bpm_ctrl.set(self.bpm);
        }
    }

    fn handle_buttons(&mut self) {
        let clear = self.xpad_clear.get() > 0.0;
        if clear && !self.previous_clear {
            for i in 0..K_INSTRUMENTS {
                self.loop_pattern[i].fill(false);
            }
            self.loop_has_steps = false;
            self.xpad_loop_steps.set(0.0);
        }
        self.previous_clear = clear;

        // Hits of the pads, recorded into the loop while overdubbing.
        for i in 0..K_INSTRUMENTS {
            let velocity = self.xpad_hits[i].get();
            let hit = velocity > 0.0;
            if hit && !self.previous_hits[i] {
                self.trigger_instrument(i, (self.xpad_volume.get() * velocity.min(1.0)) as f32);
                if self.xpad_overdub.get() > 0.0 {
                    let step = ((self.beat * 16.0).round() as i64 % K_LOOP_STEPS as i64 + K_LOOP_STEPS as i64) as usize
                        % K_LOOP_STEPS;
                    self.loop_pattern[i][step] = true;
                    self.loop_has_steps = true;
                }
            }
            self.previous_hits[i] = hit;
        }

        let release = self.release_ctrl.get() > 0.0;
        if release && !self.previous_release {
            self.start_release();
        }
        self.previous_release = release;
    }

    fn start_release(&mut self) {
        self.release_active = true;
        self.release_waiting = false;
        self.release_type = ReleaseType::from_index(self.release_type_ctrl.get() as usize);
        self.release_elapsed = 0.0;
        self.release_read = self.history.write_index() as f64 - 1.0;
        self.release_echo.clear();
        self.back_spin_needle.reset();
        self.release_active_ctrl.set(1.0);

        // Everything else off at once.
        self.scene_on.set(0.0);
        self.scene_send.reset(0.0);
        self.scene_amount.reset(0.0);
        self.wet_level = 0.0;
        self.echo.clear();
        self.spiral.clear();
        self.reverb.clear();
        self.scene_active = false;
        for band in &self.isolate_bands {
            band.set(0.0);
        }
        self.xpad_division.set(0.0);
        for roll in &self.xpad_rolls {
            roll.set(0.0);
        }
        self.xpad_overdub.set(0.0);
        for i in 0..K_INSTRUMENTS {
            self.loop_pattern[i].fill(false);
        }
        self.loop_has_steps = false;
        self.xpad_loop_steps.set(0.0);
        for voice in &mut self.voices {
            voice.active = false;
        }
    }

    fn process_scene(&mut self, left: &mut f32, right: &mut f32) -> bool {
        let scene = Scene::from_index(self.last_scene_type);
        let send = self.scene_send.value;
        let amount = self.scene_amount.value;
        let wet_level = self.wet_level;
        let param2 = self.scene_param2.get() as f32;
        let dry_left = *left;
        let dry_right = *right;
        let mut wet_left = 0.0;
        let mut wet_right = 0.0;

        match scene {
            Scene::BpfEcho
            | Scene::Echo
            | Scene::HpfEcho
            | Scene::LpfEcho
            | Scene::CrushEcho
            | Scene::SpiralUp
            | Scene::SpiralDown => {
                let delay = self.echo_delay.value;
                wet_left = self.echo.read(0, delay);
                wet_right = self.echo.read(1, delay);
                let mut feedback_left = wet_left;
                let mut feedback_right = wet_right;
                let in_left = dry_left * send;
                let in_right = dry_right * send;
                let feedback = 0.25 + 0.6 * param2;

                match scene {
                    Scene::BpfEcho => {
                        let (_, band_left, _) = self.wet_filter.process(0, in_left + feedback_left * feedback);
                        let (_, band_right, _) = self.wet_filter.process(1, in_right + feedback_right * feedback);
                        self.echo.write(soft_clip(band_left), soft_clip(band_right));
                    }
                    Scene::SpiralUp | Scene::SpiralDown => {
                        let semitones = 1.0 + 6.0 * param2;
                        let ratio = 2.0_f32.powf(if scene == Scene::SpiralUp { semitones } else { -semitones } / 12.0);
                        self.spiral.process(&mut feedback_left, &mut feedback_right, ratio);
                        let feedback = 0.55 + 0.35 * wet_level;
                        self.echo.write(
                            soft_clip(in_left + feedback_left * feedback),
                            soft_clip(in_right + feedback_right * feedback),
                        );
                    }
                    _ => {
                        self.echo.write(
                            soft_clip(in_left + feedback_left * feedback),
                            soft_clip(in_right + feedback_right * feedback),
                        );
                    }
                }

                *left = dry_left + wet_left * wet_level;
                *right = dry_right + wet_right * wet_level;

                if scene == Scene::HpfEcho || scene == Scene::LpfEcho {
                    let (low, _, high) = self.scene_filter.process(0, *left);
                    *left = if scene == Scene::HpfEcho { high } else { low };
                    let (low, _, high) = self.scene_filter.process(1, *right);
                    *right = if scene == Scene::HpfEcho { high } else { low };
                } else if scene == Scene::CrushEcho {
                    let bits = 16.0 - 12.0 * amount;
                    let steps = 2.0_f32.powf(bits);
                    let hold = 1 + (7.0 * amount) as usize;
                    self.crush_counter += 1;
                    if self.crush_counter >= hold {
                        self.crush_counter = 0;
                        self.crush_hold[0] = (soft_clip(*left * (1.0 + 2.0 * amount)) * steps).round() / steps;
                        self.crush_hold[1] = (soft_clip(*right * (1.0 + 2.0 * amount)) * steps).round() / steps;
                    }
                    *left = *left + (self.crush_hold[0] - *left) * amount;
                    *right = *right + (self.crush_hold[1] - *right) * amount;
                }
            }
            Scene::Noise => {
                let level = 0.4 * amount * amount;
                let n: f32 = self.random.f32() * 2.0 - 1.0;
                let (_, _, high) = self.scene_filter.process(0, n);
                wet_left = high * level;
                let n: f32 = self.random.f32() * 2.0 - 1.0;
                let (_, _, high) = self.scene_filter.process(1, n);
                wet_right = high * level;
                *left = dry_left + wet_left;
                *right = dry_right + wet_right;
            }
            Scene::ReverbUp | Scene::ReverbDown => {
                let mut in_left = dry_left * send;
                let mut in_right = dry_right * send;
                if scene == Scene::ReverbUp {
                    let (_, _, high) = self.wet_filter.process(0, in_left);
                    in_left = high;
                    let (_, _, high) = self.wet_filter.process(1, in_right);
                    in_right = high;
                }
                self.reverb.process(in_left, in_right, &mut wet_left, &mut wet_right);
                if scene == Scene::ReverbUp {
                    *left = dry_left + wet_left * 1.5 * wet_level;
                    *right = dry_right + wet_right * 1.5 * wet_level;
                } else {
                    let dry_gain = 1.0 - 0.85 * amount;
                    *left = dry_left * dry_gain + wet_left * 1.5 * wet_level;
                    *right = dry_right * dry_gain + wet_right * 1.5 * wet_level;
                    let (low, _, _) = self.scene_filter.process(0, *left);
                    *left = low;
                    let (low, _, _) = self.scene_filter.process(1, *right);
                    *right = low;
                }
            }
            _ => return self.process_added_scene(left, right),
        }

        wet_left.abs() + wet_right.abs() > K_SILENT_LEVEL
    }

    fn process_added_scene(&mut self, left: &mut f32, right: &mut f32) -> bool {
        let scene = Scene::from_index(self.last_scene_type);
        let mix = self.scene_mix.step(self.scene_mix_target, self.scene_coefficient);
        let dial = self.scene_dial_value.step(self.scene_dial_target, self.scene_coefficient);
        if mix < 1e-4 && self.scene_mix_target <= 0.0 {
            self.roll.stop();
            return false;
        }
        let param1 = self.scene_param1.get() as f32;
        let param2 = self.scene_param2.get() as f32;
        let dry_left = *left;
        let dry_right = *right;
        let mut amount = dial;

        match scene {
            Scene::Flanger => {
                self.flanger.process(
                    left,
                    right,
                    (self.beat / Self::pick(&K_SWEEP_BEATS, param1)) as f32,
                    0.95 * param2,
                    self.sample_rate,
                );
            }
            Scene::Phaser => {
                self.phaser.process(
                    left,
                    right,
                    (self.beat / Self::pick(&K_SWEEP_BEATS, param1)) as f32,
                    0.9 * param2,
                    self.sample_rate,
                );
            }
            Scene::Filter => {
                self.sweep_filter.process(left, right, 2.0 * dial - 1.0, param2, self.sample_rate);
                amount = 1.0;
            }
            Scene::Roll => {
                let slice_frames = (Self::pick(&K_SLICE_BEATS, param1) * self.beat_frames()) as usize;
                self.roll.process(left, right, slice_frames, param2, self.sample_rate);
                amount = (2.0 * dial).min(1.0);
            }
            Scene::Gater => {
                let gain = gate_gain(self.beat, Self::pick(&K_SLICE_BEATS, param1), 0.5, param2);
                *left *= gain;
                *right *= gain;
            }
            Scene::Pitch => {
                let semitones = (2.0 * dial - 1.0) * (1.0 + 23.0 * param1);
                self.pitch.process(left, right, 2.0_f32.powf(semitones / 12.0));
                amount = 0.5 + 0.5 * param2;
            }
            Scene::Drive => {
                self.drive.process(left, right, dial, param1, param2, self.sample_rate);
                amount = 1.0;
            }
            _ => return false,
        }

        let wet = mix * amount;
        *left = dry_left + (*left - dry_left) * wet;
        *right = dry_right + (*right - dry_right) * wet;
        true
    }

    fn process_isolate(&mut self, left: &mut f32, right: &mut f32, beat: f64) {
        let mode = IsolateMode::from_index(self.isolate_mode.get() as usize);
        let input = [*left, *right];
        let mut output = [0.0, 0.0];
        let roll_frames = self.beat_frames() / 8.0;
        let gate_coefficient = 1.0 - (-1.0 / (0.002 * self.sample_rate as f64)).exp() as f32;

        for band in 0..K_BANDS {
            let value = self.isolate_bands[band].get() as f32;
            let mut target = 1.0;
            let mut roll = false;

            match mode {
                IsolateMode::Isolator => {
                    target = 1.0 + value;
                }
                IsolateMode::CutAdd => {
                    if value < 0.0 {
                        target = if Self::fraction(beat * 2.0) < 0.5 { 1.0 } else { 1.0 + value };
                    } else {
                        let pulse = (-Self::fraction(beat * 4.0) * 6.0).exp() as f32;
                        target = 1.0 + 1.5 * value * pulse;
                    }
                }
                IsolateMode::TransRoll => {
                    if value < 0.0 {
                        target = if Self::fraction(beat * 4.0) < 0.5 { 1.0 } else { 1.0 + value };
                    } else if value > 0.05 {
                        roll = true;
                    }
                }
                IsolateMode::GateDrive => {}
            }

            self.band_gain[band].step(target, gate_coefficient);

            let band_roll = &mut self.rolls[band];
            if roll && !band_roll.active {
                band_roll.active = true;
                band_roll.length = (roll_frames as usize).clamp(64, self.roll_buffers[band].max_frames() - 2);
                band_roll.recorded = 0;
                band_roll.play_position = 0;
            } else if !roll {
                band_roll.active = false;
            }
        }

        // Split both channels into the three bands.
        let mut bands = [[0.0; 2]; K_BANDS];
        for channel in 0..2 {
            let (low, rest) = self.low_mid.split(channel, input[channel]);
            let (mid, high) = self.mid_high.split(channel, rest);
            let (low_low, low_high) = self.low_all_pass.split(channel, low);
            bands[0][channel] = low_low + low_high;
            bands[1][channel] = mid;
            bands[2][channel] = high;
        }

        for band in 0..K_BANDS {
            let value = self.isolate_bands[band].get() as f32;
            let mut gain = self.band_gain[band].value;
            let band_roll = &mut self.rolls[band];

            // Gate/Drive.
            if mode == IsolateMode::GateDrive {
                if value < 0.0 {
                    let level = 0.5 * (bands[band][0].abs() + bands[band][1].abs());
                    let envelope = &mut self.band_envelope[band];
                    *envelope += (level - *envelope) * if level > *envelope { 0.3 } else { 0.0015 };
                    let threshold = 0.25 * (-value) * (-value);
                    gain *= if *envelope > threshold { 1.0 } else { (*envelope / threshold).max(0.0) };
                }
            }

            for channel in 0..2 {
                let mut sample = bands[band][channel];
                if mode == IsolateMode::GateDrive && value > 0.0 {
                    let amount = 1.0 + 8.0 * value;
                    sample = soft_clip(sample * amount) / (1.0 + 2.0 * value);
                }
                output[channel] += sample * gain;
            }

            // Trans/Roll: the band repeats the last eighth of a beat.
            if band_roll.active {
                let buffer = &mut self.roll_buffers[band];
                if band_roll.recorded < band_roll.length {
                    buffer.write(bands[band][0], bands[band][1]);
                    band_roll.recorded += 1;
                } else {
                    let mix = value.clamp(0.0, 1.0);
                    let position =
                        buffer.write_index() as f64 - band_roll.length as f64 + band_roll.play_position as f64;
                    for channel in 0..2 {
                        let repeated = buffer.at(channel, position);
                        output[channel] += (repeated - bands[band][channel]) * mix * gain;
                    }
                    band_roll.play_position += 1;
                    if band_roll.play_position >= band_roll.length {
                        band_roll.play_position = 0;
                    }
                }
            }
        }

        *left = output[0];
        *right = output[1];
    }

    fn trigger_instrument(&mut self, instrument: usize, velocity: f32) {
        let bank_index = self.xpad_bank.get() as usize % K_BANKS;
        let bank = self.instruments.bank(bank_index);
        let factory = self.instruments.bank(0);

        let p_instrument = bank
            .as_ref()
            .and_then(|b| {
                if !b.instruments[instrument].is_empty() {
                    Some(Arc::new(b.instruments[instrument].clone()))
                } else {
                    None
                }
            })
            .or_else(|| {
                factory.as_ref().and_then(|b| {
                    if !b.instruments[instrument].is_empty() {
                        Some(Arc::new(b.instruments[instrument].clone()))
                    } else {
                        None
                    }
                })
            });

        let Some(p_instrument) = p_instrument else { return };

        // A new hit chokes the last one of the instrument (rolls stay clean).
        let mut p_free: Option<&mut Voice> = None;
        for voice in &mut self.voices {
            if voice.active && voice.instrument_idx == instrument && voice.fade == 0.0 {
                voice.fade = 1.0 / (0.003 * self.sample_rate);
            }
            if !voice.active && p_free.is_none() {
                p_free = Some(voice);
            }
        }

        let p_free = if let Some(v) = p_free {
            v
        } else {
            // The oldest voice.
            self.voices.iter_mut().max_by_key(|v| v.position as u64).unwrap()
        };

        let semitones = 12.0 * self.xpad_pitch.get();
        let sample_rate = p_instrument.sample_rate;
        p_free.instrument = Some(p_instrument);
        p_free.instrument_idx = instrument;
        p_free.position = 0.0;
        p_free.rate = sample_rate as f64 / self.sample_rate as f64 * 2.0_f64.powf(semitones / 12.0);
        p_free.gain = velocity;
        p_free.fade = 0.0;
        p_free.active = true;
    }

    fn xpad_division(&self) -> usize {
        let ribbon = (self.xpad_division.get() as usize).clamp(0, 5);
        if ribbon > 0 {
            return ribbon;
        }
        for i in (0..self.xpad_rolls.len()).rev() {
            if self.xpad_rolls[i].get() > 0.0 {
                return i + 1;
            }
        }
        0
    }

    fn process_xpad(&mut self, left: &mut f32, right: &mut f32, beat: f64, previous_beat: f64) {
        let division = self.xpad_division();
        let instrument = (self.xpad_instrument.get() as usize).clamp(0, K_INSTRUMENTS - 1);
        let quantize = self.quantize.get() > 0.0;
        let volume = self.xpad_volume.get() as f32;

        let step_at = |at_beat: f64| -> usize { (at_beat * 16.0).floor() as usize };

        // The loop of the last bar: a step every sixteenth of a beat.
        if self.loop_has_steps && step_at(beat) != step_at(previous_beat) {
            let step = (step_at(beat) % K_LOOP_STEPS + K_LOOP_STEPS) % K_LOOP_STEPS;
            for i in 0..K_INSTRUMENTS {
                let rolling = division > 0 && i == instrument;
                if self.loop_pattern[i][step] && !rolling && self.xpad_mutes[i].get() <= 0.0 {
                    self.trigger_instrument(i, volume);
                }
            }
        }

        // The roll of the ribbon: 1/1 to 1/16 of a beat.
        if division > 0 {
            let step_beats = 1.0 / 2.0_f64.powi(division as i32 - 1);
            let mut hit = false;
            if division != self.previous_division && self.previous_division == 0 {
                self.roll_origin = if quantize { 0.0 } else { beat };
                self.roll_instrument = instrument;
                hit = !quantize;
            } else {
                let origin = if quantize { 0.0 } else { self.roll_origin };
                hit = ((beat - origin) / step_beats).floor() != ((previous_beat - origin) / step_beats).floor();
            }
            if hit {
                self.trigger_instrument(instrument, volume);
                if self.xpad_overdub.get() > 0.0 {
                    let step = ((beat * 16.0).round() as i64 % K_LOOP_STEPS as i64 + K_LOOP_STEPS as i64) as usize
                        % K_LOOP_STEPS;
                    self.loop_pattern[instrument][step] = true;
                    if !self.loop_has_steps {
                        self.loop_has_steps = true;
                    }
                }
            }
        }
        self.previous_division = division;

        // The voices.
        let mut xpad_left = 0.0;
        let mut xpad_right = 0.0;
        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }
            let Some(instrument) = &voice.instrument else { continue };
            if voice.position >= instrument.frames() as f64 - 1.0 {
                voice.active = false;
                continue;
            }
            if voice.fade > 0.0 {
                voice.gain -= voice.fade;
                if voice.gain <= 0.0 {
                    voice.active = false;
                    continue;
                }
            }
            let idx = voice.position as usize;
            let frac = voice.position - idx as f64;
            let next = (idx + 1).min(instrument.frames() - 1);
            xpad_left += (instrument.samples[2 * idx]
                + (instrument.samples[2 * next] - instrument.samples[2 * idx]) * frac as f32)
                * voice.gain;
            xpad_right += (instrument.samples[2 * idx + 1]
                + (instrument.samples[2 * next + 1] - instrument.samples[2 * idx + 1]) * frac as f32)
                * voice.gain;
            voice.position += voice.rate;
        }
        *left += xpad_left;
        *right += xpad_right;
    }

    fn process_release(&mut self, left: &mut f32, right: &mut f32, beat: f64, previous_beat: f64) {
        let beat_length = self.beat_frames();
        let quantize = self.quantize.get() > 0.0;
        let mut out_left = 0.0;
        let mut out_right = 0.0;
        let mut duration = 0.0;

        if !self.release_waiting {
            let t = self.release_elapsed / beat_length;
            match self.release_type {
                ReleaseType::VinylBrake => {
                    duration = 2.0;
                    let rate = (1.0 - t / duration).max(0.0);
                    out_left = self.history.at(0, self.release_read);
                    out_right = self.history.at(1, self.release_read);
                    self.release_read += rate * rate;
                }
                ReleaseType::BackSpin => {
                    duration = 1.5;
                    let velocity = BackSpinMotion::velocity(t / duration, K_BACK_SPIN_PEAK);
                    out_left = self.history.at(0, self.release_read);
                    out_right = self.history.at(1, self.release_read);
                    self.back_spin_needle.process(&mut out_left, &mut out_right, velocity);
                    self.release_read += velocity;
                }
                ReleaseType::Echo => {
                    duration = 4.0;
                    let delay = (0.5 * beat_length).min(self.release_echo.max_frames() as f64 - 4.0) as f32;
                    let echo_left = self.release_echo.read(0, delay);
                    let echo_right = self.release_echo.read(1, delay);
                    let input = if t < 0.25 { 1.0 } else { 0.0 };
                    self.release_echo.write(*left * input + echo_left * 0.65, *right * input + echo_right * 0.65);
                    let level = (1.0 - t / duration).max(0.0) as f32;
                    out_left = if t < 0.25 { *left } else { echo_left } * level;
                    out_right = if t < 0.25 { *right } else { echo_right } * level;
                }
            }
            self.release_elapsed += 1.0;
            if self.release_elapsed / beat_length >= duration {
                self.release_waiting = true;
            }
        }

        // Back on the next beat (or at once without quantize).
        if self.release_waiting && (!quantize || beat.floor() != previous_beat.floor()) {
            self.release_active = false;
            self.release_waiting = false;
            self.live_gain.reset(0.0);
            self.release_active_ctrl.set(0.0);
            return;
        }

        *left = out_left;
        *right = out_right;
    }

    /// Processes the main mix in place (interleaved stereo).
    pub fn process(&mut self, buf: &mut [[f32; 2]], sample_rate: f32) {
        let frames = buf.len();
        if sample_rate <= 0.0 || frames == 0 {
            return;
        }
        if sample_rate != self.sample_rate {
            self.prepare(sample_rate);
        }

        self.update_clock(frames);
        self.handle_buttons();

        // Scene parameters, smoothed per buffer.
        let scene_type = (self.scene_type.get() as usize).clamp(0, Scene::COUNT - 1);
        if scene_type != self.last_scene_type {
            // Another effect: no tail of the last one.
            self.echo.clear();
            self.spiral.clear();
            self.reverb.clear();
            self.scene_filter.reset();
            self.wet_filter.reset();
            self.flanger.clear();
            self.phaser.clear();
            self.sweep_filter.clear();
            self.roll.stop();
            self.pitch.clear();
            self.drive.clear();
            self.scene_mix.reset(0.0);
            self.last_scene_type = scene_type;
        }

        let scene_on = self.scene_on.get() > 0.0;
        let dial = self.scene_dial.get() as f32;
        self.scene_mix_target = if scene_on { 1.0 } else { 0.0 };
        self.scene_dial_target = dial;
        self.scene_coefficient = 1.0 - (-1.0 / (0.01 * self.sample_rate as f64)).exp() as f32;

        if scene_type == Scene::Roll as usize && scene_on && !self.roll.active() {
            self.roll.start();
            self.scene_mix.reset(1.0);
        }

        let param1 = self.scene_param1.get() as f32;
        let param2 = self.scene_param2.get() as f32;
        let buffer_coefficient = 1.0 - (-(frames as f32) / (0.03 * self.sample_rate)).exp();
        self.scene_amount.step(if scene_on { dial } else { 0.0 }, buffer_coefficient);
        if scene_on {
            self.wet_level = self.scene_amount.value;
        }
        let send_target = if scene_on { dial } else { 0.0 };
        if scene_on {
            self.scene_active = true;
            self.scene_silent_frames = 0;
        }

        let amount = self.scene_amount.value;
        match Scene::from_index(scene_type) {
            Scene::BpfEcho => {
                self.wet_filter.set(250.0 * 2.0_f32.powf(5.0 * amount), 1.5 + 4.0 * param2, self.sample_rate);
            }
            Scene::HpfEcho => {
                self.scene_filter.set(20.0 * 2.0_f32.powf(7.0 * amount), 0.9, self.sample_rate);
            }
            Scene::LpfEcho => {
                self.scene_filter.set(20000.0 * 2.0_f32.powf(-6.0 * amount), 0.9, self.sample_rate);
            }
            Scene::Noise => {
                self.scene_filter.set(
                    200.0 * 2.0_f32.powf(5.5 * amount) * 2.0_f32.powf(3.0 * (param1 - 0.5)),
                    0.7 + 6.0 * param2,
                    self.sample_rate,
                );
            }
            Scene::ReverbUp => {
                self.wet_filter.set(100.0 * 2.0_f32.powf(5.0 * amount), 0.7, self.sample_rate);
                self.set_reverb(param1, param2);
            }
            Scene::ReverbDown => {
                self.scene_filter.set(20000.0 * 2.0_f32.powf(-4.3 * amount), 0.7, self.sample_rate);
                self.set_reverb(param1, param2);
            }
            _ => {}
        }

        // The echo time of the first sub parameter, in beats.
        let echo_index = (param1 * K_ECHO_BEAT_COUNT as f32).clamp(0.0, (K_ECHO_BEAT_COUNT - 1) as f32) as usize;
        let echo_target =
            (K_ECHO_BEATS[echo_index] * self.beat_frames()).min(self.echo.max_frames() as f64 - 4.0) as f32;
        if self.echo_delay.value <= 0.0 {
            self.echo_delay.reset(echo_target);
        }

        // Isolate is on while a band is turned (or a roll plays).
        let mut isolate_wanted = false;
        for band in &self.isolate_bands {
            isolate_wanted |= band.get().abs() > 0.01;
        }
        if isolate_wanted && !self.isolate_active {
            self.isolate_active = true;
            self.isolate_mix.reset(0.0);
            self.low_mid.reset();
            self.mid_high.reset();
            self.low_all_pass.reset();
            for gain in &mut self.band_gain {
                gain.reset(1.0);
            }
        }

        let xpad_active = self.xpad_division() > 0 || self.loop_has_steps || self.voices.iter().any(|v| v.active);

        let beat_increment = self.bpm / 60.0 / self.sample_rate as f64;
        let sample_coefficient = 1.0 - (-1.0 / (0.005 * self.sample_rate as f64)).exp() as f32;
        let delay_coefficient = 1.0 - (-1.0 / (0.05 * self.sample_rate as f64)).exp() as f32;
        let mut scene_audible = false;

        let source_input = self.source_input.get() > 0.0;
        let source_xpad = self.source_xpad.get() > 0.0;

        for frame in 0..frames {
            let previous_beat = self.beat;
            self.beat += beat_increment;

            let input_left = buf[frame][0];
            let input_right = buf[frame][1];
            let mut xpad_left = 0.0;
            let mut xpad_right = 0.0;

            if xpad_active {
                self.process_xpad(&mut xpad_left, &mut xpad_right, self.beat, previous_beat);
            }

            let mut left = (if source_input { input_left } else { 0.0 }) + (if source_xpad { xpad_left } else { 0.0 });
            let mut right =
                (if source_input { input_right } else { 0.0 }) + (if source_xpad { xpad_right } else { 0.0 });
            let bypass_left =
                (if source_input { 0.0 } else { input_left }) + (if source_xpad { 0.0 } else { xpad_left });
            let bypass_right =
                (if source_input { 0.0 } else { input_right }) + (if source_xpad { 0.0 } else { xpad_right });

            if self.isolate_active {
                let dry_left = left;
                let dry_right = right;
                self.process_isolate(&mut left, &mut right, self.beat);
                let mix = self.isolate_mix.step(if isolate_wanted { 1.0 } else { 0.0 }, sample_coefficient);
                left = dry_left + (left - dry_left) * mix;
                right = dry_right + (right - dry_right) * mix;
            }

            if self.scene_active {
                self.scene_send.step(send_target, sample_coefficient);
                self.echo_delay.step(echo_target, delay_coefficient);
                scene_audible = self.process_scene(&mut left, &mut right) || scene_audible;
            }

            left += bypass_left;
            right += bypass_right;
            self.history.write(left, right);

            if self.release_active {
                self.process_release(&mut left, &mut right, self.beat, previous_beat);
            } else if self.live_gain.value < 1.0 {
                let gain = self.live_gain.step(1.0, 0.01);
                left *= gain;
                right *= gain;
                if gain > 0.999 {
                    self.live_gain.reset(1.0);
                }
            }

            buf[frame][0] = left.clamp(-1.0, 1.0);
            buf[frame][1] = right.clamp(-1.0, 1.0);
        }

        self.frame_count += frames as u64;

        // Idle sections cost nothing.
        if self.scene_active && !scene_on && !scene_audible {
            self.scene_silent_frames += frames;
            if self.scene_silent_frames > self.sample_rate as usize {
                self.scene_active = false;
                self.echo.clear();
                self.spiral.clear();
                self.reverb.clear();
            }
        }
        if self.isolate_active && !isolate_wanted && self.isolate_mix.value < 0.001 {
            self.isolate_active = false;
            for roll in &mut self.rolls {
                roll.active = false;
            }
        }
        if self.loop_has_steps {
            let mut steps = 0;
            for instrument in &self.loop_pattern {
                steps += instrument.iter().filter(|&&b| b).count();
            }
            self.xpad_loop_steps.set(steps as f64);
        }
        self.beat_ctrl.set(self.beat.rem_euclid(4.0));
    }

    fn set_reverb(&mut self, size: f32, tone: f32) {
        self.reverb.set_size(size, false);
        self.reverb.set_decay(1.0 + 7.0 * size);
        self.reverb.set_damping(16000.0 * 2.0_f32.powf(-4.0 * tone));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_control::ControlRegistry;
    use std::sync::Arc;

    #[test]
    fn test_silence_idle() {
        let reg = Arc::new(ControlRegistry::new());
        let mut fx = FxStation::new(&reg, 48000.0);
        let mut buf = [[0.0f32, 0.0]; 512];
        fx.process(&mut buf, 48000.0);
        for frame in &buf {
            assert_eq!(frame[0], 0.0);
            assert_eq!(frame[1], 0.0);
        }
    }

    #[test]
    fn test_echo_scene() {
        let reg = Arc::new(ControlRegistry::new());
        let mut fx = FxStation::new(&reg, 48000.0);
        fx.scene_type.set(1.0); // Echo
        fx.scene_on.set(1.0);
        fx.scene_dial.set(1.0);
        fx.scene_param1.set(0.5); // 0.5 beat delay
        fx.scene_param2.set(0.5);

        // The send fades in over about 30 ms; let it settle, then click.
        let mut settle = vec![[0.0f32; 2]; 4800];
        fx.process(&mut settle, 48000.0);
        let mut buf = vec![[0.0f32; 2]; 48000];
        buf[0] = [1.0, 1.0];
        fx.process(&mut buf, 48000.0);

        // Check that there's a delayed echo (around 0.5 beats = 0.5 * 60/120 * 48000 = 12000 samples)
        let mut found_echo = false;
        for frame in &buf[10000..20000] {
            if frame[0].abs() > 0.01 || frame[1].abs() > 0.01 {
                found_echo = true;
                break;
            }
        }
        assert!(found_echo, "Echo should produce a delayed copy");
    }

    #[test]
    fn test_release_brake() {
        let reg = Arc::new(ControlRegistry::new());
        let mut fx = FxStation::new(&reg, 48000.0);

        // Play some audio first to fill history
        let mut buf = [[0.1f32, 0.1]; 48000];
        fx.process(&mut buf, 48000.0);

        // Trigger release (vinyl brake)
        fx.release_type_ctrl.set(0.0); // VinylBrake
        fx.release_ctrl.set(1.0);

        // Process a few more buffers - should fade to silence
        for _ in 0..10 {
            let mut buf = [[0.0f32, 0.0]; 4800];
            fx.process(&mut buf, 48000.0);
        }

        // After release completes, output should be silence
        let mut buf = [[0.0f32, 0.0]; 4800];
        fx.process(&mut buf, 48000.0);
        for frame in &buf {
            assert_eq!(frame[0], 0.0, "Release brake should end in silence");
            assert_eq!(frame[1], 0.0);
        }
    }
}
