//! Effect units: `[EffectRack1_EffectUnitN]` holds three effect slots in series, `[EffectRack1_EffectUnitN_EffectM]`, and is routed to any of the decks and the main mix. Each (unit, slot, target) has its own effect instance so a unit can process both decks
//! and the main mix at once. All memory is allocated here, up front, and nothing is allocated on the audio thread. Delay memory sits in a [`Pool`] of buffers in two sizes (two seconds of stereo for the echoes, rolls and brakes, and enough for the reverb and
//! flanger), lent to an instance when it starts sounding and taken back once it has gone quiet, so the rack holds a few buffers instead of one big one per instance. A slot fades out before it switches, and fades in and out when it is turned on or off.

use std::f32::consts::{PI, TAU};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};
use djsugar_plugin_host::{Connection, Midi, Transport};

use crate::controls::{
    deck_group, effect_group, effect_unit_group, Edge, BEAT_FX_UNIT, EFFECTS_PER_UNIT, MIC_FX_UNIT, NUM_DECKS,
    NUM_EFFECT_UNITS,
};
use crate::dsp::Svf;
use crate::inputs::{input_group, MICROPHONE, NUM_INPUTS};

/// The built-in effects, in `loaded_effect` order.
pub const AVAILABLE_EFFECTS: [&str; 20] = [
    "Echo",
    "Reverb",
    "Filter",
    "Flanger",
    "Bitcrusher",
    "Phaser",
    "Gate",
    "Roll",
    "Echo Out",
    "Brake",
    "Noise",
    "Spiral",
    "Drive",
    "Chorus",
    "Ladder",
    "Hall",
    "Rev Roll",
    "Filter Echo",
    "Tremolo",
    "Autopan",
];

/// Parameter knobs (`parameterK`) and buttons (`button_parameterK`) per slot.
pub const MAX_PARAMETERS: usize = 4;
pub const MAX_BUTTON_PARAMETERS: usize = 2;

/// The effects each slot starts with, as `loaded_effect` values.
/// Each deck's own unit starts with Reverb, Echo and Gate (a beat-synced cut).
pub const DEFAULT_EFFECTS: [[usize; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS] =
    [[2, 1, 7], [2, 1, 7], [2, 1, 7], [2, 1, 7], [1, 0, 0], [0; 3], [0; 3], [0; 3], [0; 3], [2, 1, 3]];

/// How long a released effect's tail rings on, in beats (see `release`).
const RELEASE_BEATS: f32 = 8.0;
/// Taps further apart than this, in seconds, start a new tap tempo.
pub(crate) const TAP_GAP: f64 = 2.0;
/// Tap intervals averaged into the tapped tempo.
pub(crate) const TAPS: usize = 4;

/// `loaded_effect` values from here on are plugins: `PLUGIN_BASE + i` is
/// plugin `i` of the list the app hands the plugin host (see
/// [`crate::plugins::PluginHost::set_plugins`]).
pub const PLUGIN_BASE: usize = KINDS.len() + 1;
/// The most plugins a slot can choose from.
pub const MAX_PLUGINS: usize = 1024;

/// How long one plugin may take in a buffer that started at `start`: half the buffer, and all plugins together no more than
/// [`PLUGIN_SHARE`] of it, so several slow plugins can't make the audio drop out (each waits in turn). `None` once the buffer's
/// plugin time is used up: that plugin plays dry this buffer without being sent the block, so it isn't counted late.
fn plugin_deadline(start: std::time::Instant, frames: usize, sample_rate: f32) -> Option<std::time::Duration> {
    let block = std::time::Duration::from_secs_f32(frames as f32 / sample_rate);
    let left = (start + block.mul_f32(PLUGIN_SHARE)).checked_duration_since(std::time::Instant::now())?;
    // Too little left to be worth a round trip.
    (left >= std::time::Duration::from_micros(100)).then(|| left.min(block / 2))
}

/// The part of each buffer's time that plugins may use together.
const PLUGIN_SHARE: f32 = 0.6;

/// The plugin a `loaded_effect` value refers to, as an index into the list.
pub fn plugin_index(loaded_effect: f64) -> Option<usize> {
    let v = loaded_effect.round();
    (v >= PLUGIN_BASE as f64).then(|| v as usize - PLUGIN_BASE)
}

/// The name of the effect a `loaded_effect` value refers to: 1 is the first of
/// [`AVAILABLE_EFFECTS`], 0 means the slot is empty ("").
pub fn effect_name(loaded_effect: f64) -> &'static str {
    Kind::from_loaded(loaded_effect).map_or("", |k| AVAILABLE_EFFECTS[k as usize])
}

/// Names of the loaded effect's parameters, in `parameterK` order.
pub fn parameter_names(loaded_effect: f64) -> &'static [&'static str] {
    Kind::from_loaded(loaded_effect).map_or(&[], |k| MANIFESTS[k as usize].names)
}

/// Signal paths an effect unit can be routed to: the decks, the main mix,
/// then the inputs (see [`crate::inputs`]).
pub(crate) const TARGETS: usize = NUM_DECKS + 1 + NUM_INPUTS;
pub(crate) const MAIN: usize = NUM_DECKS;

/// The target of input `input` (see [`crate::inputs`]).
pub(crate) const fn input_target(input: usize) -> usize {
    MAIN + 1 + input
}

/// Whether `unit` can process `target`: the microphone's unit only the
/// inputs, the others only the decks and the main mix. Instances are only
/// made for these pairs, which keeps the rack's memory down.
fn routable(unit: usize, target: usize) -> bool {
    (unit == MIC_FX_UNIT) == (target > MAIN)
}

/// A unit's control that routes it to `target`.
pub(crate) fn route_name(target: usize) -> String {
    let group = if target == MAIN {
        "[Master]".to_string()
    } else if target > MAIN {
        input_group(target - MAIN - 1).to_string()
    } else {
        deck_group(target)
    };
    format!("group_{group}_enable")
}

/// Each (unit, slot, target)'s instance number, if the unit can be routed
/// there. The plugin host uses the same numbers.
pub(crate) type Indices = [[[Option<u16>; TARGETS]; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS];

pub(crate) fn instance_indices() -> (Indices, usize) {
    let mut n = 0;
    let index = std::array::from_fn(|u| {
        std::array::from_fn(|_| {
            std::array::from_fn(|t| {
                routable(u, t).then(|| {
                    n += 1;
                    n as u16 - 1
                })
            })
        })
    });
    (index, n)
}

/// Longest echo, in seconds; it sizes the pool's long buffers.
const ECHO_MAX_SECONDS: f32 = 2.0;
const FLANGER_MAX_SECONDS: f32 = 0.02;

/// Buffers in the rack's pool, of each size. More effects than this sounding at once (each on
/// each of its targets) is far beyond any real set: four decks with three echoes each, plus the
/// Beat FX on two decks and pad FX, is about 20. An effect that finds the pool empty stays off.
const LONG_BUFFERS: usize = 32;
const SHORT_BUFFERS: usize = 32;

/// The delay memory an effect needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Memory {
    None,
    /// The reverb's tank or the flanger's delay: [`short_size`].
    Short,
    /// Two seconds of stereo: [`long_size`].
    Long,
}

/// Floats in a long buffer: Echo's stereo delay line, which Roll, Brake,
/// Echo Out and Spiral use too.
fn long_size(sample_rate: f32) -> usize {
    2 * Echo::new(sample_rate).len
}

/// Floats in a short buffer: the reverb's tank, or the flanger's lines.
fn short_size(sample_rate: f32) -> usize {
    Reverb::new(sample_rate).size.max(2 * Flanger::new(sample_rate).len).max(extra::Chorus::size(sample_rate))
}

/// Delay buffers waiting to be lent to an instance. Both stacks are created
/// with room for every buffer, so giving one back never allocates.
struct Pool {
    long: Vec<Box<[f32]>>,
    short: Vec<Box<[f32]>>,
    long_size: usize,
    short_size: usize,
}

impl Pool {
    fn new(sample_rate: f32, long: usize, short: usize) -> Self {
        let long_size = long_size(sample_rate);
        let short_size = short_size(sample_rate);
        let make = |n: usize, size: usize| {
            let mut v = Vec::with_capacity(long + short);
            v.extend((0..n).map(|_| vec![0.0f32; size].into_boxed_slice()));
            v
        };
        Self { long: make(long, long_size), short: make(short, short_size), long_size, short_size }
    }

    /// A buffer at least as big as `need`: a short one if there is one left,
    /// otherwise a long one.
    fn take(&mut self, need: Memory) -> Option<Box<[f32]>> {
        match need {
            Memory::None => None,
            Memory::Short => self.short.pop().or_else(|| self.long.pop()),
            Memory::Long => self.long.pop(),
        }
    }

    fn give(&mut self, buf: Box<[f32]>) {
        if buf.is_empty() {
            return;
        }
        if buf.len() >= self.long_size {
            self.long.push(buf);
        } else {
            self.short.push(buf);
        }
    }

    /// Whether `buf` is big enough for an effect needing `need`.
    fn fits(&self, buf: &[f32], need: Memory) -> bool {
        match need {
            Memory::None => true,
            Memory::Short => buf.len() >= self.short_size,
            Memory::Long => buf.len() >= self.long_size,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Echo,
    Reverb,
    Filter,
    Flanger,
    Bitcrusher,
    Phaser,
    Gate,
    Roll,
    EchoOut,
    Brake,
    Noise,
    Spiral,
    Drive,
    Chorus,
    Ladder,
    Hall,
    RevRoll,
    FilterEcho,
    Tremolo,
    Autopan,
}

const KINDS: [Kind; 20] = [
    Kind::Echo,
    Kind::Reverb,
    Kind::Filter,
    Kind::Flanger,
    Kind::Bitcrusher,
    Kind::Phaser,
    Kind::Gate,
    Kind::Roll,
    Kind::EchoOut,
    Kind::Brake,
    Kind::Noise,
    Kind::Spiral,
    Kind::Drive,
    Kind::Chorus,
    Kind::Ladder,
    Kind::Hall,
    Kind::RevRoll,
    Kind::FilterEcho,
    Kind::Tremolo,
    Kind::Autopan,
];

impl Kind {
    fn from_loaded(v: f64) -> Option<Kind> {
        let i = v.round();
        (i >= 1.0 && i <= KINDS.len() as f64).then(|| KINDS[i as usize - 1])
    }

    /// The delay memory the effect needs from the pool.
    fn memory(self) -> Memory {
        match self {
            Kind::Echo
            | Kind::Roll
            | Kind::EchoOut
            | Kind::Brake
            | Kind::Spiral
            | Kind::Hall
            | Kind::RevRoll
            | Kind::FilterEcho => Memory::Long,
            Kind::Reverb | Kind::Flanger | Kind::Chorus => Memory::Short,
            Kind::Filter
            | Kind::Bitcrusher
            | Kind::Phaser
            | Kind::Gate
            | Kind::Noise
            | Kind::Drive
            | Kind::Ladder
            | Kind::Tremolo
            | Kind::Autopan => Memory::None,
        }
    }
}

/// An effect's parameters (all normalized 0..1) and their defaults.
struct Manifest {
    names: &'static [&'static str],
    defaults: &'static [f64],
}

const MANIFESTS: [Manifest; 20] = [
    Manifest { names: &["Time", "Feedback", "Ping Pong", "Send"], defaults: &[0.5, 0.5, 0.0, 1.0] },
    Manifest { names: &["Decay", "Bandwidth", "Damping", "Send"], defaults: &[0.5, 0.75, 0.5, 1.0] },
    Manifest { names: &["Low Pass", "Resonance", "High Pass"], defaults: &[1.0, 0.25, 0.0] },
    Manifest { names: &["Speed", "Depth", "Manual", "Regeneration"], defaults: &[0.4, 0.5, 0.4, 0.5] },
    Manifest { names: &["Bit Depth", "Downsampling"], defaults: &[1.0, 1.0] },
    Manifest { names: &["Speed", "Depth", "Feedback", "Stages"], defaults: &[0.4, 0.5, 0.5, 0.4] },
    Manifest { names: &["Length", "Depth", "Width"], defaults: &[0.5, 0.8, 0.5] },
    Manifest { names: &["Rate"], defaults: &[0.25] },
    Manifest { names: &["Time", "Feedback", "Length"], defaults: &[0.5, 0.6, 0.67] },
    Manifest { names: &["Length"], defaults: &[0.33] },
    Manifest { names: &["Level", "Sweep", "Resonance"], defaults: &[0.5, 0.5, 0.4] },
    Manifest { names: &["Time", "Feedback", "Send"], defaults: &[0.67, 0.6, 1.0] },
    Manifest { names: &["Drive", "Tone"], defaults: &[0.5, 0.7] },
    Manifest { names: &["Time", "Depth", "Spread"], defaults: &[0.5, 0.5, 0.7] },
    Manifest { names: &["Cutoff", "Resonance", "Drive"], defaults: &[1.0, 0.5, 0.3] },
    Manifest { names: &["Decay", "Pre-delay", "Damping", "Send"], defaults: &[0.5, 0.3, 0.4, 1.0] },
    Manifest { names: &["Rate"], defaults: &[0.25] },
    Manifest { names: &["Time", "Feedback", "Filter", "Send"], defaults: &[0.5, 0.6, 0.75, 1.0] },
    Manifest { names: &["Rate", "Depth", "Shape", "Width"], defaults: &[0.67, 0.8, 0.0, 0.5] },
    Manifest { names: &["Period", "Width", "Smoothing"], defaults: &[0.75, 0.8, 0.5] },
];

/// The beat switch's range: 1/16 to 4 beats, as powers of two.
const BEATS_MIN: i32 = -4;
const BEATS_MAX: i32 = 2;
/// Where a slot's beat switch starts.
const DEFAULT_BEATS: f64 = 0.25;

/// The parameter that sets an effect's length in beats, as
/// (index, lowest and highest power of two, inverted): the inverse of
/// how its `process` reads it with [`beats`].
fn time_parameter(kind: Kind) -> Option<(usize, i32, i32, bool)> {
    match kind {
        Kind::Echo | Kind::FilterEcho => Some((0, -4, 2, false)),
        Kind::Flanger | Kind::Phaser => Some((0, -1, 5, true)),
        Kind::Gate => Some((0, -4, 0, false)),
        Kind::Roll | Kind::RevRoll => Some((0, -4, 0, true)),
        Kind::EchoOut | Kind::Spiral => Some((0, -2, 0, false)),
        Kind::Brake => Some((0, 0, 3, false)),
        Kind::Chorus => Some((0, 0, 2, false)),
        Kind::Tremolo => Some((0, -4, 2, false)),
        Kind::Autopan => Some((0, -2, 2, false)),
        Kind::Reverb | Kind::Filter | Kind::Bitcrusher | Kind::Noise | Kind::Drive | Kind::Ladder | Kind::Hall => None,
    }
}

/// The parameters the meta knob drives:
/// meta 0 is subtle and 1 strong. The knob is unipolar, so Filter is neutral
/// at 0 and turning up sweeps its high-pass.
fn link(kind: Kind, meta: f64) -> [Option<f64>; MAX_PARAMETERS] {
    match kind {
        Kind::Echo | Kind::Reverb | Kind::Hall => [None, None, None, Some(meta)],
        Kind::Filter => [Some(1.0), None, Some(meta), None],
        Kind::Flanger | Kind::Phaser | Kind::Gate | Kind::Tremolo | Kind::Autopan => [None, Some(meta), None, None],
        // Bitcrusher and Noise leave their second control to the FX bar's
        // slider.
        Kind::Bitcrusher => [None, Some(1.0 - 0.95 * meta), None, None],
        // Turning up rolls shorter slices; brakes and echo-outs get longer.
        // Their length is the slot's beat switch.
        Kind::Roll | Kind::RevRoll | Kind::Brake => [None; MAX_PARAMETERS],
        Kind::EchoOut => [None, Some(meta), None, None],
        Kind::Noise => [Some(meta), None, None, None],
        Kind::Spiral => [None, None, Some(meta), None],
        // Drive leaves its tone to the FX bar's slider.
        Kind::Drive => [Some(meta), None, None, None],
        Kind::Chorus => [None, Some(meta), None, None],
        // Turning up closes the ladder, from fully open.
        Kind::Ladder => [Some(1.0 - 0.85 * meta), None, None, None],
        // Opens the send and sweeps the high-pass in the repeats up.
        Kind::FilterEcho => [None, None, Some(0.5 + 0.5 * meta), Some(meta)],
    }
}

struct SlotControls {
    enabled: ControlHandle,
    meta: ControlHandle,
    /// Length in beats, 1/16 to 4 (0 for effects without one), as the
    /// slot's beat switch shows it.
    beats: ControlHandle,
    beats_seen: f64,
    params: [ControlHandle; MAX_PARAMETERS],
    loaded_effect: ControlHandle,
    loaded: ControlHandle,
    num_parameters: ControlHandle,
    #[allow(dead_code)] // read by the plugin host through the registry
    show_editor: ControlHandle,
    effect_selector: ControlHandle,
    next: ControlHandle,
    prev: ControlHandle,
    clear: ControlHandle,
    edges: [Edge; 3],
    /// The effect the parameters are set up for.
    kind: Option<Kind>,
    /// Or the plugin, by its place in the plugin list.
    plugin: Option<u16>,
    /// `[Master],num_plugins`: how many plugins the selector steps through.
    num_plugins: ControlHandle,
    meta_seen: f64,
}

impl SlotControls {
    fn create(reg: &ControlRegistry, unit: usize, slot: usize) -> Self {
        let g = effect_group(unit, slot);
        let g = g.as_str();
        let n = (KINDS.len() + MAX_PLUGINS) as f64;
        for k in 1..=MAX_BUTTON_PARAMETERS {
            reg.toggle(g, &format!("button_parameter{k}"));
        }
        reg.indicator(g, "num_button_parameters", MAX_BUTTON_PARAMETERS as f64);
        let mut s = Self {
            enabled: reg.toggle(g, "enabled"),
            // The Beat FX and the mic's effects start half way, so turning
            // them on is heard at once.
            meta: reg.create(
                g,
                "meta",
                if unit == BEAT_FX_UNIT || unit == MIC_FX_UNIT { 0.5 } else { 0.0 },
                0.0,
                1.0,
                Behavior::Linear,
            ),
            beats: reg.create(g, "beats", 1.0, 0.0, 4.0, Behavior::Linear),
            beats_seen: f64::NAN,
            params: std::array::from_fn(|k| {
                reg.create(g, &format!("parameter{}", k + 1), 0.5, 0.0, 1.0, Behavior::Linear)
            }),
            loaded_effect: reg.create(g, "loaded_effect", DEFAULT_EFFECTS[unit][slot] as f64, 0.0, n, Behavior::Linear),
            loaded: reg.indicator(g, "loaded", 1.0),
            num_parameters: reg.indicator(g, "num_parameters", MAX_PARAMETERS as f64),
            // A plugin's own window: the plugin host opens it while this is
            // on, and turns it off when the window closes.
            show_editor: reg.toggle(g, "show_editor"),
            effect_selector: reg.create(g, "effect_selector", 0.0, -n, n, Behavior::Free),
            next: reg.push(g, "next_effect"),
            prev: reg.push(g, "prev_effect"),
            clear: reg.push(g, "clear"),
            edges: [Edge::default(); 3],
            kind: None,
            plugin: None,
            num_plugins: reg.indicator("[Master]", "num_plugins", MAX_PLUGINS as f64),
            meta_seen: f64::NAN,
        };
        s.update();
        s
    }

    /// Handles effect selection and the meta knob; returns the loaded effect.
    fn update(&mut self) -> Option<Kind> {
        let mut step = 0i32;
        let sel = self.effect_selector.get();
        if sel != 0.0 {
            step += sel.signum() as i32;
            self.effect_selector.set(0.0);
        }
        if self.edges[0].pressed(&self.next) {
            step += 1;
        }
        if self.edges[1].pressed(&self.prev) {
            step -= 1;
        }
        if self.edges[2].pressed(&self.clear) {
            self.loaded_effect.set(0.0);
        }
        if step != 0 {
            // Cycles through the effects, then the plugins; an empty slot
            // steps to either end.
            let plugins = (self.num_plugins.get().round() as usize).min(MAX_PLUGINS);
            let n = (KINDS.len() + plugins) as i32;
            let cur = self.loaded_effect.get().round() as i32;
            let next = if cur == 0 {
                if step > 0 {
                    1
                } else {
                    n
                }
            } else {
                (cur - 1 + step).rem_euclid(n) + 1
            };
            self.loaded_effect.set(next as f64);
        }

        let kind = Kind::from_loaded(self.loaded_effect.get());
        let plugin = plugin_index(self.loaded_effect.get()).map(|p| p as u16);
        if plugin != self.plugin {
            // The plugin host sets the knobs to the plugin's own values
            // once it has loaded it; until then they sit in the middle.
            self.plugin = plugin;
            if plugin.is_some() {
                for p in &self.params {
                    p.set(0.5);
                }
                self.loaded.set_bool(true);
                self.num_parameters.set(0.0);
                self.beats.set(0.0);
                self.beats_seen = 0.0;
            }
        }
        if kind != self.kind || (kind.is_none() && plugin.is_none() && self.loaded.get_bool()) {
            self.kind = kind;
            // A newly loaded effect starts from its defaults, then follows meta.
            let defaults = kind.map_or(&[][..], |k| MANIFESTS[k as usize].defaults);
            for (k, p) in self.params.iter().enumerate() {
                p.set(defaults.get(k).copied().unwrap_or(0.5));
            }
            self.loaded.set_bool(kind.is_some() || plugin.is_some());
            if plugin.is_none() {
                self.num_parameters.set(defaults.len() as f64);
                // The beat switch starts at 1/4 beat, or the nearest length
                // the effect allows.
                self.beats.set(if kind.and_then(time_parameter).is_some() { DEFAULT_BEATS } else { 0.0 });
                self.beats_seen = f64::NAN;
            }
            self.meta_seen = f64::NAN;
        }
        let meta = self.meta.get();
        if meta != self.meta_seen {
            self.meta_seen = meta;
            if let Some(k) = kind {
                for (p, v) in self.params.iter().zip(link(k, meta)) {
                    if let Some(v) = v {
                        p.set(v);
                    }
                }
            }
        }
        let wanted = self.beats.get();
        if wanted != self.beats_seen {
            match kind.and_then(time_parameter) {
                Some((i, lo, hi, inverted)) => {
                    // The nearest power of two the effect and the switch both allow.
                    let power = if wanted > 0.0 { wanted.log2().round() as i32 } else { 0 }
                        .clamp(lo.max(BEATS_MIN), hi.min(BEATS_MAX));
                    let p = (power - lo) as f64 / (hi - lo) as f64;
                    self.params[i].set(if inverted { 1.0 - p } else { p });
                    self.beats.set(2f64.powi(power));
                }
                None => self.beats.set(0.0),
            }
            self.beats_seen = self.beats.get();
        }
        kind
    }
}

struct UnitControls {
    enabled: ControlHandle,
    mix: ControlHandle,
    super1: ControlHandle,
    /// group_[ChannelN]_enable for each deck, group_[Master]_enable, then
    /// group_[Microphone]_enable and the other inputs' (see
    /// [`crate::inputs::input_group`]).
    routes: [ControlHandle; TARGETS],
    super_seen: u32,
    /// The unit's own tempo, from `bpm_tap`; 0 follows each target's tempo.
    bpm: ControlHandle,
    /// Pushes: `bpm_tap` taps a tempo, `bpm_auto` goes back to following the
    /// decks, and `release` turns the unit's effects off and lets their
    /// tails (echoes, reverb) ring out instead of cutting them.
    tap: ControlHandle,
    auto: ControlHandle,
    release: ControlHandle,
    edges: [Edge; 3],
    /// When the last tap came, on the engine clock, and the intervals before it.
    last_tap: f64,
    intervals: [f64; TAPS],
    taps: usize,
}

impl UnitControls {
    fn create(reg: &ControlRegistry, unit: usize) -> Self {
        let g = effect_unit_group(unit);
        let g = g.as_str();
        let n = EFFECTS_PER_UNIT as f64;
        reg.toggle(g, "mix_mode");
        reg.create(g, "focused_effect", 0.0, 0.0, n, Behavior::Linear);
        reg.toggle(g, "show_focus");
        reg.toggle(g, "show_parameters");
        reg.toggle(g, "controller_input_active");
        reg.indicator(g, "num_effects", n).set(n);
        reg.indicator(g, "num_effectslots", n).set(n);
        // Chain presets are not implemented: -1 means no preset.
        reg.create(g, "loaded_chain_preset", -1.0, -1.0, 64.0, Behavior::Free);
        reg.indicator(g, "num_chain_presets", 64.0).set(1.0);
        reg.push(g, "next_chain_preset");
        reg.push(g, "prev_chain_preset");
        reg.create(g, "chain_preset_selector", 0.0, -64.0, 64.0, Behavior::Free);
        // The headphones are stored only, as are targets the unit can't be
        // routed to.
        reg.toggle(g, "group_[Headphone]_enable");
        let routes = std::array::from_fn(|t| {
            if t == MAIN {
                reg.toggle(g, &route_name(t))
            } else if t > MAIN {
                let input = t - MAIN - 1;
                let r = reg.toggle(g, &route_name(t));
                // The microphone's unit starts on the microphone.
                if unit == MIC_FX_UNIT && input == MICROPHONE {
                    r.set_bool(true);
                }
                r
            } else {
                let r = reg.toggle(g, &route_name(t));
                // Each deck's unit starts on its deck, as the decks show; the
                // Beat FX on decks 1 and 2. Pad FX units are routed when
                // their pads are held.
                if t == unit || (unit == BEAT_FX_UNIT && t < 2) {
                    r.set_bool(true);
                }
                r
            }
        });
        let super1 = reg.create(g, "super1", 0.0, 0.0, 1.0, Behavior::Linear);
        Self {
            enabled: {
                let e = reg.toggle(g, "enabled");
                e.set_bool(true);
                e
            },
            mix: reg.create(g, "mix", 1.0, 0.0, 1.0, Behavior::Linear),
            super_seen: super1.version(),
            super1,
            routes,
            bpm: reg.create(g, "bpm", 0.0, 0.0, 300.0, Behavior::Free),
            tap: reg.push(g, "bpm_tap"),
            auto: reg.push(g, "bpm_auto"),
            release: reg.push(g, "release"),
            edges: [Edge::default(); 3],
            last_tap: f64::NEG_INFINITY,
            intervals: [0.0; TAPS],
            taps: 0,
        }
    }

    /// Takes in `bpm_tap` and `bpm_auto` presses; `now` is the engine clock
    /// in seconds.
    fn update_tempo(&mut self, now: f64) {
        if self.edges[1].pressed(&self.auto) {
            self.bpm.set(0.0);
            self.taps = 0;
        }
        if self.edges[0].pressed(&self.tap) {
            let gap = now - self.last_tap;
            self.last_tap = now;
            if gap > TAP_GAP || gap <= 0.0 {
                self.taps = 0;
                return;
            }
            self.intervals.rotate_right(1);
            self.intervals[0] = gap;
            self.taps = (self.taps + 1).min(TAPS);
            let mean = self.intervals[..self.taps].iter().sum::<f64>() / self.taps as f64;
            self.bpm.set((60.0 / mean).clamp(40.0, 300.0));
        }
    }
}

/// Per-buffer values an effect needs besides its parameters.
#[derive(Clone, Copy)]
struct Ctx {
    sample_rate: f32,
    /// Length of a beat at the target's current tempo.
    beat_seconds: f32,
    /// Where the target is in its beat grid at the start of the buffer, in
    /// beats, if it has one and the unit follows it (not a tapped tempo).
    /// Gates, LFOs and rolls lock to it.
    beat: Option<f64>,
}

/// Phase errors (in cycles) bigger than this are a jump (a seek, a loop, a
/// newly started effect) and snap; smaller ones are drift and are pulled in
/// over about [`LOCK_FRAMES`] frames so the sweep never jumps.
const LOCK_SNAP: f32 = 0.05;
const LOCK_FRAMES: f32 = 1024.0;

/// Locks a cycle of `cycle_beats` beats to the beat grid: `phase` (0..1) is where the effect's
/// cycle is at the start of the buffer and `step` its per-frame advance, which this nudges so the
/// cycle lands on the grid. `fresh` snaps the first time. Without a grid it leaves both alone.
fn lock_to_beat(phase: &mut f32, step: &mut f32, fresh: &mut bool, beat: Option<f64>, cycle_beats: f32) {
    let Some(beat) = beat else {
        return;
    };
    let want = (beat / cycle_beats as f64).rem_euclid(1.0) as f32;
    let err = (want - *phase + 0.5).rem_euclid(1.0) - 0.5;
    if *fresh || err.abs() > LOCK_SNAP {
        *fresh = false;
        *phase = want;
    } else {
        *step += err / LOCK_FRAMES;
    }
}

/// One effect instance: every effect type's state, of which one is in use.
struct Instance {
    kind: Option<Kind>,
    /// A plugin instead, by its place in the plugin list; it plays through
    /// whichever helper the plugin host attached (see [`Attached`]).
    plugin: Option<u16>,
    /// Output level reached at the end of the last buffer, and the level it
    /// fades towards (see [`FX_FADE_SECONDS`]).
    gain: f32,
    target: f32,
    /// Samples left of a released tail (see `release`), and its full length.
    release: f32,
    release_len: f32,
    /// Delay memory lent by the rack's [`Pool`] while the instance sounds
    /// (empty when it needs none or is quiet).
    mem: Box<[f32]>,
    echo: Echo,
    reverb: Reverb,
    filter: Filter,
    flanger: Flanger,
    crusher: Bitcrusher,
    phaser: Phaser,
    gate: Gate,
    roll: Roll,
    echo_out: EchoOut,
    brake: Brake,
    noise: Noise,
    spiral: Spiral,
    drive: extra::Drive,
    chorus: extra::Chorus,
    ladder: extra::Ladder,
    tremolo: extra::Tremolo,
    autopan: extra::Autopan,
    hall: extra::Hall,
    filter_echo: extra::FilterEcho,
}

impl Instance {
    /// An instance without delay memory: the rack lends it some from its
    /// [`Pool`] when it starts sounding.
    fn new(sample_rate: f32) -> Self {
        let echo = Echo::new(sample_rate);
        let echo_len = echo.len;
        Self {
            kind: None,
            plugin: None,
            gain: 0.0,
            target: 0.0,
            release: 0.0,
            release_len: 0.0,
            mem: Box::default(),
            echo,
            reverb: Reverb::new(sample_rate),
            filter: Filter::new(sample_rate),
            flanger: Flanger::new(sample_rate),
            crusher: Bitcrusher::default(),
            phaser: Phaser::default(),
            gate: Gate::default(),
            roll: Roll::new(echo_len),
            echo_out: EchoOut::new(sample_rate),
            brake: Brake::new(echo_len),
            noise: Noise::default(),
            spiral: Spiral::new(sample_rate),
            drive: extra::Drive::new(sample_rate),
            chorus: extra::Chorus::new(sample_rate),
            ladder: extra::Ladder::new(sample_rate),
            tremolo: extra::Tremolo::default(),
            autopan: extra::Autopan::default(),
            hall: extra::Hall::new(sample_rate),
            filter_echo: extra::FilterEcho::new(sample_rate),
        }
    }

    /// An instance with its own buffer, big enough for any effect, for a
    /// deck's Color FX (which is always ready) and tests.
    fn with_memory(sample_rate: f32) -> Self {
        let mut inst = Self::new(sample_rate);
        inst.mem = vec![0.0; long_size(sample_rate).max(short_size(sample_rate))].into_boxed_slice();
        inst
    }

    fn active(&self) -> bool {
        self.gain > 0.0 || self.target > 0.0 || self.release > 0.0
    }

    /// The tail of a released effect: what it plays with no more input,
    /// fading out over the last quarter of the release.
    #[inline]
    fn tail(&mut self) -> [f32; 2] {
        let fade = (self.release / (0.25 * self.release_len)).min(1.0);
        self.release -= 1.0;
        if self.release <= 0.0 {
            self.release = 0.0;
        }
        let y = self.process([0.0; 2]);
        [y[0] * fade, y[1] * fade]
    }

    /// Clears the loaded effect's state so it starts silent, without old tails.
    fn reset(&mut self) {
        match self.kind {
            Some(Kind::Echo) => self.echo.reset(&mut self.mem),
            Some(Kind::Reverb) => self.reverb.reset(&mut self.mem),
            Some(Kind::Filter) => self.filter.reset(),
            Some(Kind::Flanger) => self.flanger.reset(&mut self.mem),
            Some(Kind::Bitcrusher) => self.crusher = Bitcrusher::default(),
            Some(Kind::Phaser) => self.phaser = Phaser::default(),
            Some(Kind::Gate) => self.gate = Gate::default(),
            Some(Kind::Roll | Kind::RevRoll) => self.roll.reset(),
            Some(Kind::EchoOut) => self.echo_out.reset(&mut self.mem),
            Some(Kind::Brake) => self.brake.reset(&mut self.mem),
            Some(Kind::Noise) => self.noise.reset(),
            Some(Kind::Spiral) => self.spiral.reset(&mut self.mem),
            Some(Kind::Drive) => self.drive.reset(),
            Some(Kind::Chorus) => self.chorus.reset(&mut self.mem),
            Some(Kind::Ladder) => self.ladder.reset(),
            Some(Kind::Tremolo) => self.tremolo = extra::Tremolo::default(),
            Some(Kind::Autopan) => self.autopan = extra::Autopan::default(),
            Some(Kind::Hall) => self.hall.reset(&mut self.mem),
            Some(Kind::FilterEcho) => self.filter_echo.reset(&mut self.mem),
            None => {}
        }
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        match self.kind {
            Some(Kind::Echo) => self.echo.prepare(p, ctx),
            Some(Kind::Reverb) => self.reverb.prepare(p),
            Some(Kind::Filter) => self.filter.prepare(p),
            Some(Kind::Flanger) => self.flanger.prepare(p, ctx),
            Some(Kind::Bitcrusher) => self.crusher.prepare(p),
            Some(Kind::Phaser) => self.phaser.prepare(p, ctx),
            Some(Kind::Gate) => self.gate.prepare(p, ctx),
            Some(Kind::Roll) => {
                self.roll.reverse = false;
                self.roll.prepare(p, ctx)
            }
            Some(Kind::RevRoll) => {
                self.roll.reverse = true;
                self.roll.prepare(p, ctx)
            }
            Some(Kind::EchoOut) => self.echo_out.prepare(p, ctx),
            Some(Kind::Brake) => self.brake.prepare(p, ctx),
            Some(Kind::Noise) => self.noise.prepare(p, ctx),
            Some(Kind::Spiral) => self.spiral.prepare(p, ctx),
            Some(Kind::Drive) => self.drive.prepare(p, ctx),
            Some(Kind::Chorus) => self.chorus.prepare(p, ctx),
            Some(Kind::Ladder) => self.ladder.prepare(p),
            Some(Kind::Tremolo) => self.tremolo.prepare(p, ctx),
            Some(Kind::Autopan) => self.autopan.prepare(p, ctx),
            Some(Kind::Hall) => self.hall.prepare(p),
            Some(Kind::FilterEcho) => self.filter_echo.prepare(p, ctx),
            None => {}
        }
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        match self.kind {
            Some(Kind::Echo) => self.echo.process(&mut self.mem, x),
            Some(Kind::Reverb) => self.reverb.process(&mut self.mem, x),
            Some(Kind::Filter) => self.filter.process(x),
            Some(Kind::Flanger) => self.flanger.process(&mut self.mem, x),
            Some(Kind::Bitcrusher) => self.crusher.process(x),
            Some(Kind::Phaser) => self.phaser.process(x),
            Some(Kind::Gate) => self.gate.process(x),
            Some(Kind::Roll | Kind::RevRoll) => self.roll.process(&mut self.mem, x),
            Some(Kind::EchoOut) => self.echo_out.process(&mut self.mem, x),
            Some(Kind::Brake) => self.brake.process(&mut self.mem, x),
            Some(Kind::Noise) => self.noise.process(x),
            Some(Kind::Spiral) => self.spiral.process(&mut self.mem, x),
            Some(Kind::Drive) => self.drive.process(x),
            Some(Kind::Chorus) => self.chorus.process(&mut self.mem, x),
            Some(Kind::Ladder) => self.ladder.process(x),
            Some(Kind::Tremolo) => self.tremolo.process(x),
            Some(Kind::Autopan) => self.autopan.process(x),
            Some(Kind::Hall) => self.hall.process(&mut self.mem, x),
            Some(Kind::FilterEcho) => self.filter_echo.process(&mut self.mem, x),
            None => x,
        }
    }
}

/// All effect units: their controls and their instances.
pub(crate) struct EffectRack {
    sample_rate: f32,
    units: [UnitControls; NUM_EFFECT_UNITS],
    slots: [[SlotControls; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS],
    /// Indexed by [`EffectRack::index`].
    instances: Vec<Instance>,
    /// Delay memory, lent to the instances that are sounding.
    pool: Pool,
    /// Each (unit, slot, target)'s instance, if the unit can be routed there.
    index: [[[Option<u16>; TARGETS]; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS],
    /// Unit mix at the start and end of this buffer.
    mix: [(f32, f32); NUM_EFFECT_UNITS],
    /// Whether a unit has any slot sounding on a target this buffer.
    active: [[bool; TARGETS]; NUM_EFFECT_UNITS],
    /// Each target's beat-grid position at the start of this buffer (see
    /// [`EffectRack::set_beats`]).
    beats: [Option<f64>; TARGETS],
    /// Scratch for [`EffectRack::process_blocks`]: a unit's wet signal and
    /// released tails, [`BLOCK`] frames for each of up to [`PARALLEL`]
    /// blocks, made up front.
    wet: Vec<[f32; 2]>,
    tails: Vec<[f32; 2]>,
    /// Plugin helpers, by instance, as the plugin host attached them.
    attached: Vec<Option<Box<Attached>>>,
    /// Each target's tempo and position, for plugins.
    transport: [Transport; TARGETS],
    /// Planar scratch for plugins: left and right in, left and right out.
    planar: [Vec<f32>; 4],
    /// When this buffer started (see [`EffectRack::prepare`]): every plugin
    /// in it shares one time budget from here.
    block_start: std::time::Instant,
    from_host: rtrb::Consumer<PluginMsg>,
    to_host: rtrb::Producer<Box<Attached>>,
    link: Option<PluginLink>,
    legacy: LegacyFlanger,
    /// The Instrument input's plugin, as the plugin host attached it.
    instrument: Option<Box<Attached>>,
    /// Notes for the instrument, from [`NoteInput`], and those waiting to
    /// reach it (kept while it is late, so no note-off is lost).
    notes: rtrb::Consumer<Midi>,
    notes_waiting: Vec<Midi>,
    note_input: Option<NoteInput>,
    /// The scale notes move to, and the chords they play, on their way to
    /// the instrument (see [`crate::scale`]), and the scale now.
    scale: Box<crate::scale::NoteMapper>,
    /// The arpeggiator after them, and this buffer's notes on their way
    /// to it.
    arp: Box<crate::arp::Arp>,
    arp_in: Vec<Midi>,
    scale_mask: u16,
    /// What the instrument plays, for its deck to show.
    view: Arc<InstrumentView>,
    /// Its last [`SCOPE_SAMPLES`] samples (mono), and where the next goes.
    scope_ring: Vec<f32>,
    scope_pos: usize,
    /// The view shows nothing playing.
    view_clear: bool,
    /// The instrument's block for this buffer went to its helper (see
    /// [`EffectRack::start_instrument`]).
    instrument_sent: bool,
    /// Frames in a row the instrument has played silence, untouched: past
    /// [`SLEEP_AFTER`] seconds it sleeps (no blocks go to it) until a note,
    /// a knob or a deck wakes it.
    instrument_quiet: usize,
}

/// Seconds of silence, with no notes held and off any deck, before the
/// instrument sleeps.
const SLEEP_AFTER: f32 = 1.0;
/// Quieter than this (about -100 dBFS) counts as silence.
const SILENCE: f32 = 1e-5;

/// Bins the instrument's scope shows, each its lowest and highest sample.
pub const SCOPE_BINS: usize = 128;
/// Samples the scope spans: about 43 ms at 48 kHz.
const SCOPE_SAMPLES: usize = SCOPE_BINS * 16;

/// What the instrument plays, written on the audio thread each buffer for
/// the app to show: the notes held and a scope of its sound.
pub struct InstrumentView {
    held: [AtomicU64; 2],
    /// Each bin's lowest then highest sample, as `f32` bits.
    scope: [AtomicU32; 2 * SCOPE_BINS],
}

impl Default for InstrumentView {
    fn default() -> Self {
        Self { held: std::array::from_fn(|_| AtomicU64::new(0)), scope: std::array::from_fn(|_| AtomicU32::new(0)) }
    }
}

impl InstrumentView {
    /// The notes held, bit `n` for MIDI note `n`.
    pub fn held(&self) -> u128 {
        let lo = self.held[0].load(Ordering::Relaxed) as u128;
        let hi = self.held[1].load(Ordering::Relaxed) as u128;
        lo | hi << 64
    }

    /// The scope, oldest first: each bin's (lowest, highest) sample.
    pub fn scope(&self) -> Vec<(f32, f32)> {
        (0..SCOPE_BINS)
            .map(|b| {
                let v = |i: usize| f32::from_bits(self.scope[i].load(Ordering::Relaxed));
                (v(2 * b), v(2 * b + 1))
            })
            .collect()
    }

    fn set_held(&self, held: u128) {
        self.held[0].store(held as u64, Ordering::Relaxed);
        self.held[1].store((held >> 64) as u64, Ordering::Relaxed);
    }
}

/// `held` after the note messages in `notes` (any channel); All Notes Off
/// and All Sound Off let go of every one.
fn held_after(mut held: u128, notes: &[Midi]) -> u128 {
    for n in notes {
        if let Some((_, key, _)) = n.as_note_on() {
            held |= 1 << (key & 127);
        } else if let Some((_, key)) = n.as_note_off() {
            held &= !(1 << (key & 127));
        } else if n.data[0] & 0xf0 == 0xb0 && matches!(n.data[1], 120 | 123) {
            held = 0;
        }
    }
    held
}

/// `loaded_effect` for the Flanger.
const FLANGER: f64 = 4.0;

/// The legacy flanger, which older controller mappings still drive (the Numark V7's FX section, among others): `[ChannelN],flanger`
/// turns it on for a deck, and `[Flanger]`'s `lfoDepth`, `lfoDelay` (microseconds) and `lfoPeriod` (microseconds) set
/// it for all of them. It plays through the deck's Pad FX unit, which is free while no pad is held.
struct LegacyFlanger {
    decks: [ControlHandle; NUM_DECKS],
    on: [bool; NUM_DECKS],
    /// Turned on last buffer: the knobs still to apply.
    pending: [bool; NUM_DECKS],
    /// lfoDepth, lfoDelay, lfoPeriod, and the versions last applied.
    knobs: [ControlHandle; 3],
    seen: [u32; 3],
}

impl LegacyFlanger {
    fn create(reg: &ControlRegistry) -> Self {
        let g = "[Flanger]";
        let knobs = [
            reg.create(g, "lfoDepth", 1.0, 0.0, 1.0, Behavior::Linear),
            reg.create(g, "lfoDelay", 3333.0, 50.0, 10_000.0, Behavior::Linear),
            reg.create(g, "lfoPeriod", 1_000_000.0, 50_000.0, 2_000_000.0, Behavior::Linear),
        ];
        Self {
            decks: std::array::from_fn(|d| reg.toggle(&deck_group(d), "flanger")),
            on: [false; NUM_DECKS],
            pending: [false; NUM_DECKS],
            seen: knobs.each_ref().map(|k| k.version()),
            knobs,
        }
    }

    /// The Flanger's Speed, Depth (through meta) and Manual for the knobs.
    fn parameters(&self) -> [f64; 3] {
        let [depth, delay, period] = self.knobs.each_ref().map(|k| k.get());
        [
            // A longer period is a slower sweep.
            1.0 - ((period - 50_000.0) / 1_950_000.0).clamp(0.0, 1.0),
            depth.clamp(0.0, 1.0),
            ((delay - 50.0) / 9_950.0).clamp(0.0, 1.0),
        ]
    }

    fn update(
        &mut self,
        units: &[UnitControls; NUM_EFFECT_UNITS],
        slots: &[[SlotControls; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS],
    ) {
        let versions = self.knobs.each_ref().map(|k| k.version());
        let turned = versions != self.seen;
        self.seen = versions;
        let [speed, depth, manual] = self.parameters();
        for d in 0..NUM_DECKS {
            let u = crate::controls::pad_fx_unit(d);
            let slot = &slots[u][0];
            let on = self.decks[d].get_bool();
            if on != self.on[d] {
                self.on[d] = on;
                if on {
                    slot.loaded_effect.set(FLANGER);
                    units[u].routes[d].set_bool(true);
                    units[u].enabled.set_bool(true);
                    slot.enabled.set_bool(true);
                    // A newly loaded effect takes its defaults this buffer;
                    // the knobs go on next.
                    self.pending[d] = true;
                } else {
                    slot.enabled.set_bool(false);
                }
                continue;
            }
            if on && (turned || self.pending[d]) {
                self.pending[d] = false;
                // Meta drives the Flanger's depth.
                slot.meta.set(depth);
                slot.params[0].set(speed);
                slot.params[2].set(manual);
            }
        }
    }
}

/// Where notes for the Instrument input come in: a MIDI keyboard, the
/// Keyboard pads or a controller. Each goes to the instrument at the start
/// of the next buffer.
pub struct NoteInput {
    tx: rtrb::Producer<Midi>,
}

impl NoteInput {
    /// Queues a MIDI message (three bytes: status, key, velocity); false
    /// when the queue is full.
    pub fn send(&mut self, data: [u8; 3]) -> bool {
        self.tx.push(Midi { frame: 0, data }).is_ok()
    }
}

/// A plugin helper attached to one instance.
pub(crate) struct Attached {
    pub(crate) conn: Connection,
    /// The plugin it runs, by its place in the plugin list.
    pub(crate) plugin: u16,
    /// The plugin's range for each knob the slot shows.
    pub(crate) ranges: Vec<(f32, f32)>,
    /// The knob values last sent, one per range.
    pub(crate) sent: Vec<f32>,
}

/// From the plugin host to the audio thread.
pub(crate) enum PluginMsg {
    Attach(u16, Box<Attached>),
    Detach(u16),
    /// The Instrument input's plugin, or none.
    Instrument(Option<Box<Attached>>),
}

/// The plugin host's half of the rack's queues: it attaches helpers and
/// gets the detached ones back, to drop them off the audio thread.
pub struct PluginLink {
    pub(crate) to_engine: rtrb::Producer<PluginMsg>,
    pub(crate) from_engine: rtrb::Consumer<Box<Attached>>,
}

/// The longest block [`EffectRack::process_block`] takes; the engine works
/// in blocks no longer than this.
pub(crate) const BLOCK: usize = 4096;

/// The most blocks [`EffectRack::process_blocks`] takes at once: one per
/// deck.
pub(crate) const PARALLEL: usize = NUM_DECKS;

impl EffectRack {
    pub(crate) fn new(reg: &ControlRegistry, sample_rate: f64) -> Self {
        Self::with_pool(reg, sample_rate, LONG_BUFFERS, SHORT_BUFFERS)
    }

    /// A rack whose pool holds `long` and `short` buffers.
    fn with_pool(reg: &ControlRegistry, sample_rate: f64, long: usize, short: usize) -> Self {
        let sample_rate = sample_rate as f32;
        let (index, n) = instance_indices();
        let (to_engine, from_host) = rtrb::RingBuffer::new(2 * n);
        let (to_host, from_engine) = rtrb::RingBuffer::new(2 * n);
        let (notes_tx, notes) = rtrb::RingBuffer::new(djsugar_plugin_host::shm::MAX_EVENTS);
        let units: [UnitControls; NUM_EFFECT_UNITS] = std::array::from_fn(|u| UnitControls::create(reg, u));
        reg.indicator("[Master]", "num_effectsavailable", KINDS.len() as f64).set(KINDS.len() as f64);
        Self {
            sample_rate,
            mix: std::array::from_fn(|u| {
                let m = units[u].mix.get() as f32;
                (m, m)
            }),
            units,
            slots: std::array::from_fn(|u| std::array::from_fn(|s| SlotControls::create(reg, u, s))),
            instances: (0..n).map(|_| Instance::new(sample_rate)).collect(),
            pool: Pool::new(sample_rate, long, short),
            index,
            active: [[false; TARGETS]; NUM_EFFECT_UNITS],
            beats: [None; TARGETS],
            wet: vec![[0.0; 2]; BLOCK * PARALLEL],
            tails: vec![[0.0; 2]; BLOCK * PARALLEL],
            attached: (0..n).map(|_| None).collect(),
            transport: [Transport::default(); TARGETS],
            planar: std::array::from_fn(|_| vec![0.0; BLOCK]),
            block_start: std::time::Instant::now(),
            from_host,
            to_host,
            link: Some(PluginLink { to_engine, from_engine }),
            legacy: LegacyFlanger::create(reg),
            instrument: None,
            notes,
            notes_waiting: Vec::with_capacity(djsugar_plugin_host::shm::MAX_EVENTS),
            note_input: Some(NoteInput { tx: notes_tx }),
            scale: Box::default(),
            arp: Box::default(),
            arp_in: Vec::with_capacity(djsugar_plugin_host::shm::MAX_EVENTS),
            scale_mask: 0,
            view: Arc::default(),
            scope_ring: vec![0.0; SCOPE_SAMPLES],
            scope_pos: 0,
            view_clear: true,
            instrument_sent: false,
            instrument_quiet: 0,
        }
    }

    /// What the instrument plays, for its deck to show.
    pub(crate) fn instrument_view(&self) -> Arc<InstrumentView> {
        self.view.clone()
    }

    /// Shows `buf` (what the instrument just played) and the notes it
    /// took in the instrument's view; nothing when `buf` is none.
    fn show_instrument(&mut self, buf: Option<&[[f32; 2]]>, held: u128) {
        let Some(buf) = buf else {
            if !self.view_clear {
                self.view_clear = true;
                self.view.set_held(0);
                self.scope_ring.fill(0.0);
                for v in &self.view.scope {
                    v.store(0, Ordering::Relaxed);
                }
            }
            return;
        };
        self.view_clear = false;
        self.view.set_held(held);
        for s in buf {
            self.scope_ring[self.scope_pos] = 0.5 * (s[0] + s[1]);
            self.scope_pos = (self.scope_pos + 1) % SCOPE_SAMPLES;
        }
        let per = SCOPE_SAMPLES / SCOPE_BINS;
        for b in 0..SCOPE_BINS {
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for k in 0..per {
                let v = self.scope_ring[(self.scope_pos + b * per + k) % SCOPE_SAMPLES];
                lo = lo.min(v);
                hi = hi.max(v);
            }
            self.view.scope[2 * b].store(lo.to_bits(), Ordering::Relaxed);
            self.view.scope[2 * b + 1].store(hi.to_bits(), Ordering::Relaxed);
        }
    }

    /// Each target's position in its beat grid, in beats, for this buffer:
    /// set before [`EffectRack::prepare`]. `None` where there is no grid
    /// (no track, no tempo, the inputs); those targets' effects run free.
    pub(crate) fn set_beats(&mut self, beats: [Option<f64>; TARGETS]) {
        self.beats = beats;
    }

    /// Bytes of delay memory the rack holds, lent out or not.
    #[cfg(test)]
    fn memory_bytes(&self) -> usize {
        let pooled: usize = self.pool.long.iter().chain(&self.pool.short).map(|b| b.len()).sum();
        let lent: usize = self.instances.iter().map(|i| i.mem.len()).sum();
        (pooled + lent) * std::mem::size_of::<f32>()
    }

    /// Where notes for the instrument come in, once.
    pub(crate) fn take_note_input(&mut self) -> Option<NoteInput> {
        self.note_input.take()
    }

    /// Queues a note for the instrument from the audio thread itself (the
    /// Instrument input's pads).
    pub(crate) fn queue_note(&mut self, data: [u8; 3]) {
        self.scale.map(Midi { frame: 0, data }, self.scale_mask, &mut self.notes_waiting);
    }

    /// The scale notes for the instrument move to from now on: a bit per
    /// pitch class from C, 0 for none (see [`crate::scale`]).
    pub(crate) fn set_scale(&mut self, mask: u16) {
        self.scale_mask = mask;
    }

    /// The chord each note plays from now on (see [`crate::scale::chord`]),
    /// and whether the next chord held is learned as `shape`.
    pub(crate) fn set_chord(&mut self, mode: u8, shape: u32, learn: bool) {
        self.scale.set_chord(mode, shape, learn);
    }

    /// The arpeggiator's settings from now on.
    pub(crate) fn set_arp(&mut self, settings: crate::arp::ArpSettings) {
        self.arp.set(settings, &mut self.notes_waiting);
    }

    /// The chord just learned, once.
    pub(crate) fn take_learned_chord(&mut self) -> Option<u32> {
        self.scale.take_learned()
    }

    pub(crate) fn has_instrument(&self) -> bool {
        self.instrument.is_some()
    }

    /// The first half of playing the instrument: takes the notes that came in and hands the plugin its block of `frames`, at the tempo of `target`, without waiting for
    /// it. [`EffectRack::finish_instrument`] picks the sound up later in the buffer, so the plugin works while the decks are worked out. `params` are its knobs (see
    /// [`crate::inputs::INSTRUMENT_PARAMETERS`]). While it is silent, holds no notes, gets none and is on no deck (`on_deck`) it sleeps: no block goes to it at all.
    pub(crate) fn start_instrument(&mut self, frames: usize, target: usize, params: &[f32], on_deck: bool) {
        self.instrument_sent = false;
        let cap = self.arp_in.capacity();
        while let Ok(n) = self.notes.pop() {
            if self.arp_in.len() < cap {
                self.scale.map(n, self.scale_mask, &mut self.arp_in);
            }
        }
        if self.arp.is_on() {
            self.arp.process(&self.arp_in, frames, self.sample_rate, self.transport[target], &mut self.notes_waiting);
        } else {
            for &m in &self.arp_in {
                if self.notes_waiting.len() < self.notes_waiting.capacity() {
                    self.notes_waiting.push(m);
                }
            }
        }
        self.arp_in.clear();
        let Some(a) = self.instrument.as_deref_mut() else {
            self.notes_waiting.clear();
            return;
        };
        let turned = a.send_params(params);
        let busy = on_deck || turned || !self.notes_waiting.is_empty() || self.view.held() != 0;
        if busy {
            self.instrument_quiet = 0;
        }
        let asleep = self.instrument_quiet as f32 >= SLEEP_AFTER * self.sample_rate;
        if asleep || plugin_deadline(self.block_start, frames, self.sample_rate).is_none() {
            return;
        }
        let [silent, ..] = &mut self.planar;
        silent[..frames].fill(0.0);
        self.instrument_sent =
            a.conn.send([&silent[..frames], &silent[..frames]], self.transport[target], &self.notes_waiting).is_ok();
    }

    /// Whether the instrument sleeps: no blocks go to it.
    #[cfg(test)]
    pub(crate) fn instrument_asleep(&self) -> bool {
        self.instrument.is_some()
            && !self.instrument_sent
            && self.instrument_quiet as f32 >= SLEEP_AFTER * self.sample_rate
    }

    /// The second half: the instrument's sound for this buffer into `buf`,
    /// silence without one, while it sleeps or when it is late. True when
    /// it is attached.
    pub(crate) fn finish_instrument(&mut self, buf: &mut [[f32; 2]]) -> bool {
        let frames = buf.len();
        if self.instrument.is_none() {
            buf.fill([0.0; 2]);
            self.show_instrument(None, 0);
            return false;
        }
        if !std::mem::take(&mut self.instrument_sent) {
            buf.fill([0.0; 2]);
            return true;
        }
        let Some(a) = self.instrument.as_deref_mut() else {
            return false;
        };
        let deadline = plugin_deadline(self.block_start, frames, self.sample_rate).unwrap_or_default();
        let [_, _, out_l, out_r] = &mut self.planar;
        let out = a.conn.collect([&mut out_l[..frames], &mut out_r[..frames]], deadline);
        if out.is_wet() {
            let held = held_after(self.view.held(), &self.notes_waiting);
            self.notes_waiting.clear();
            let mut peak = 0.0f32;
            for (f, s) in buf.iter_mut().enumerate() {
                *s = [out_l[f], out_r[f]];
                peak = peak.max(s[0].abs()).max(s[1].abs());
            }
            self.instrument_quiet = if peak < SILENCE { self.instrument_quiet.saturating_add(frames) } else { 0 };
            self.show_instrument(Some(buf), held);
        } else {
            buf.fill([0.0; 2]);
        }
        true
    }

    /// The plugin host's half of the queues, once.
    pub(crate) fn take_plugin_link(&mut self) -> Option<PluginLink> {
        self.link.take()
    }

    /// Each target's tempo, beat and play state, for the plugins this buffer.
    pub(crate) fn set_transport(&mut self, transport: [Transport; TARGETS]) {
        self.transport = transport;
    }

    /// Takes the plugin host's attach and detach requests. A detached
    /// helper goes back to the host, which drops it off the audio thread.
    fn take_plugin_messages(&mut self) {
        while let Ok(msg) = self.from_host.pop() {
            let (i, new) = match msg {
                PluginMsg::Attach(i, a) => (i, Some(a)),
                PluginMsg::Detach(i) => (i, None),
                PluginMsg::Instrument(a) => {
                    self.instrument_quiet = 0;
                    self.instrument_sent = false;
                    if let Some(old) = std::mem::replace(&mut self.instrument, a) {
                        let _ = self.to_host.push(old);
                    }
                    continue;
                }
            };
            let Some(slot) = self.attached.get_mut(i as usize) else {
                continue;
            };
            if let Some(old) = std::mem::replace(slot, new) {
                // The queue holds one per instance and more; if it is ever
                // full the drop here only closes a pipe.
                let _ = self.to_host.push(old);
            }
        }
    }

    #[cfg(test)]
    fn instance(&mut self, unit: usize, slot: usize, target: usize) -> Option<&mut Instance> {
        let i = self.index[unit][slot][target]?;
        Some(&mut self.instances[i as usize])
    }

    /// Reads the controls once per buffer. `beat_seconds` is each target's
    /// beat length, for tempo-synced effects; `now` is the engine clock in
    /// seconds, for tap tempo.
    pub(crate) fn prepare(&mut self, beat_seconds: [f64; TARGETS], now: f64) {
        self.block_start = std::time::Instant::now();
        self.take_plugin_messages();
        self.legacy.update(&self.units, &self.slots);
        for u in 0..NUM_EFFECT_UNITS {
            let unit = &mut self.units[u];
            unit.update_tempo(now);
            let bpm = unit.bpm.get();
            let beat_seconds = beat_seconds.map(|b| if bpm > 0.0 { 60.0 / bpm } else { b });
            let release = unit.edges[2].pressed(&unit.release);
            let version = unit.super1.version();
            if version != unit.super_seen {
                // super1 drives every slot's meta knob.
                unit.super_seen = version;
                let v = unit.super1.get();
                for slot in &self.slots[u] {
                    slot.meta.set(v);
                }
            }
            let on = unit.enabled.get_bool();
            let routed: [bool; TARGETS] = std::array::from_fn(|t| unit.routes[t].get_bool());
            self.mix[u] = (self.mix[u].1, unit.mix.get() as f32);
            self.active[u] = [false; TARGETS];
            for s in 0..EFFECTS_PER_UNIT {
                let slot = &mut self.slots[u][s];
                let kind = slot.update();
                let plugin = slot.plugin;
                let releasing = release && slot.enabled.get_bool();
                if releasing {
                    slot.enabled.set_bool(false);
                }
                let enabled = on && slot.enabled.get_bool();
                let params = slot.params.each_ref().map(|p| p.get() as f32);
                for (t, &routed) in routed.iter().enumerate() {
                    let Some(i) = self.index[u][s][t] else {
                        continue;
                    };
                    let inst = &mut self.instances[i as usize];
                    // A plugin's tail isn't ours to play: it just fades out.
                    if releasing && inst.gain > 0.0 && inst.plugin.is_none() {
                        // The tail takes over from the effect at once.
                        inst.release = RELEASE_BEATS * (beat_seconds[t] as f32) * self.sample_rate;
                        inst.release_len = inst.release;
                        inst.gain = 0.0;
                    }
                    // Switch effects only once the old one has faded out.
                    let same = inst.kind == kind && inst.plugin == plugin;
                    if !same && inst.gain == 0.0 && inst.release == 0.0 {
                        inst.kind = kind;
                        inst.plugin = plugin;
                    }
                    let same = inst.kind == kind && inst.plugin == plugin;
                    let mut on = enabled && routed && same && (kind.is_some() || plugin.is_some());
                    if inst.gain == 0.0 && on {
                        // Starting afresh: borrow delay memory if the effect
                        // needs it. With the pool empty it stays off, and
                        // tries again next buffer.
                        let need = kind.map_or(Memory::None, Kind::memory);
                        if !self.pool.fits(&inst.mem, need) {
                            let old = std::mem::take(&mut inst.mem);
                            self.pool.give(old);
                            match self.pool.take(need) {
                                Some(buf) => inst.mem = buf,
                                None => on = false,
                            }
                        }
                        if on {
                            inst.release = 0.0;
                            inst.reset();
                        }
                    }
                    inst.target = if on { 1.0 } else { 0.0 };
                    if !inst.active() && !inst.mem.is_empty() {
                        // Quiet: the memory goes back for another effect.
                        let old = std::mem::take(&mut inst.mem);
                        self.pool.give(old);
                    }
                    if inst.active() {
                        // While the old effect fades out it keeps its settings;
                        // the parameters already belong to the new one.
                        if same {
                            let ctx = Ctx {
                                sample_rate: self.sample_rate,
                                beat_seconds: beat_seconds[t] as f32,
                                beat: if bpm > 0.0 { None } else { self.beats[t] },
                            };
                            inst.prepare(&params, ctx);
                            if let Some(a) = self.attached[i as usize].as_deref_mut() {
                                a.send_params(&params);
                            }
                        }
                        self.active[u][t] = true;
                    }
                }
            }
        }
    }

    /// Auto reset on load: switches off the effects of every unit sent to `deck`, unless the unit
    /// also sounds on the main mix or on another deck that is playing. Each effect's beats, parameters
    /// and the unit's dry/wet stay as they were, ready for the next time it is switched on.
    pub(crate) fn reset_deck(&mut self, deck: usize, playing: [bool; NUM_DECKS]) {
        for (u, unit) in self.units.iter().enumerate() {
            let routed = |t: usize| unit.routes[t].get_bool();
            let elsewhere = routed(MAIN) || (0..NUM_DECKS).any(|t| t != deck && playing[t] && routed(t));
            if u == MIC_FX_UNIT || !routed(deck) || elsewhere {
                continue;
            }
            for slot in &self.slots[u] {
                slot.enabled.set_bool(false);
            }
        }
    }

    /// Lets go of a deck's Pad FX: they play only while a pad is held, so a
    /// newly loaded track starts without them whatever auto reset says (a
    /// pad still on after a missed release would otherwise carry over).
    pub(crate) fn release_pad_fx(&mut self, deck: usize) {
        for slot in &self.slots[crate::controls::pad_fx_unit(deck)] {
            slot.enabled.set_bool(false);
        }
    }

    /// Runs a block of `target` through the units routed to it, in place
    /// (see [`EffectRack::process_blocks`]).
    pub(crate) fn process_block(&mut self, target: usize, buf: &mut [[f32; 2]]) {
        self.process_blocks(&mut [(target, buf)]);
    }

    /// Runs each block through the units routed to its target, in place: unit by unit and slot by slot, a whole block at a time, which is where plugin slots can join in. Each instance sees the same samples in the same order as frame by frame, so the result
    /// is the same. Frame `f` of a block is `(f + 1) / frames` of the way through the buffer, for the dry/wet ramp; effects fade on and off at a fixed rate. The blocks go through side by side: at each slot every target's plugin gets its block first, the
    /// built-in effects run meanwhile, and only then does the rack wait for the plugins. So plugins on different decks work at the same time, each in its own helper, instead of one after another. At most [`PARALLEL`] blocks, all the same length.
    pub(crate) fn process_blocks(&mut self, blocks: &mut [(usize, &mut [[f32; 2]])]) {
        let Some(frames) = blocks.first().map(|(_, b)| b.len()) else {
            return;
        };
        assert!(frames <= BLOCK, "blocks are at most BLOCK frames");
        assert!(blocks.len() <= PARALLEL, "at most PARALLEL blocks at once");
        assert!(blocks.iter().all(|(_, b)| b.len() == frames));
        let ramp = |f: usize| (f + 1) as f32 / frames as f32;
        let step = fade_step(self.sample_rate);
        let faded = |f: usize| (f + 1) as f32 * step;
        let mut wet = std::mem::take(&mut self.wet);
        let mut tails = std::mem::take(&mut self.tails);
        let span = |k: usize| k * BLOCK..k * BLOCK + frames;
        for u in 0..NUM_EFFECT_UNITS {
            let mut live = [false; PARALLEL];
            for (k, (target, buf)) in blocks.iter().enumerate() {
                live[k] = self.active[u][*target];
                if live[k] {
                    wet[span(k)].copy_from_slice(buf);
                    tails[span(k)].fill([0.0; 2]);
                }
            }
            if !live.contains(&true) {
                continue;
            }
            for s in 0..EFFECTS_PER_UNIT {
                // Each block's plugin in this slot gets its block.
                let mut sent = [false; PARALLEL];
                for (k, (target, _)) in blocks.iter().enumerate() {
                    if !live[k] {
                        continue;
                    }
                    let Some(i) = self.index[u][s][*target] else {
                        continue;
                    };
                    let inst = &self.instances[i as usize];
                    let Some(plugin) = inst.plugin.filter(|_| inst.active()) else {
                        continue;
                    };
                    // Through the helper, or dry while it isn't attached,
                    // is late or is gone.
                    let Some(a) = self.attached[i as usize].as_deref_mut() else {
                        continue;
                    };
                    if a.plugin != plugin || plugin_deadline(self.block_start, frames, self.sample_rate).is_none() {
                        continue;
                    }
                    let [in_l, in_r, ..] = &mut self.planar;
                    for (f, w) in wet[span(k)].iter().enumerate() {
                        in_l[f] = w[0];
                        in_r[f] = w[1];
                    }
                    sent[k] = a.conn.send([&in_l[..frames], &in_r[..frames]], self.transport[*target], &[]).is_ok();
                }
                // The built-in effects run while the plugins work.
                for (k, (target, _)) in blocks.iter().enumerate() {
                    if !live[k] {
                        continue;
                    }
                    let Some(i) = self.index[u][s][*target] else {
                        continue;
                    };
                    let inst = &mut self.instances[i as usize];
                    if !inst.active() || inst.plugin.is_some() {
                        continue;
                    }
                    if inst.release > 0.0 {
                        for t in &mut tails[span(k)] {
                            let y = inst.tail();
                            t[0] += y[0];
                            t[1] += y[1];
                        }
                        continue;
                    }
                    let (g0, g1) = (inst.gain, inst.target);
                    for (f, w) in wet[span(k)].iter_mut().enumerate() {
                        let g = fade(g0, g1, faded(f));
                        let y = inst.process(*w);
                        for c in 0..2 {
                            w[c] += (y[c] - w[c]) * g;
                        }
                    }
                }
                // Then the plugins' blocks come back, each within what is
                // left of the buffer's plugin time.
                for (k, (target, _)) in blocks.iter().enumerate() {
                    if !sent[k] {
                        continue;
                    }
                    let Some(i) = self.index[u][s][*target] else {
                        continue;
                    };
                    let Some(a) = self.attached[i as usize].as_deref_mut() else {
                        continue;
                    };
                    let deadline = plugin_deadline(self.block_start, frames, self.sample_rate).unwrap_or_default();
                    let [_, _, out_l, out_r] = &mut self.planar;
                    let out = a.conn.collect([&mut out_l[..frames], &mut out_r[..frames]], deadline);
                    if !out.is_wet() {
                        continue;
                    }
                    let inst = &self.instances[i as usize];
                    let (g0, g1) = (inst.gain, inst.target);
                    for (f, w) in wet[span(k)].iter_mut().enumerate() {
                        let g = fade(g0, g1, faded(f));
                        w[0] += (out_l[f] - w[0]) * g;
                        w[1] += (out_r[f] - w[1]) * g;
                    }
                }
            }
            // Dry/wet. Every effect's output already carries the dry signal
            // where it musically belongs (an echo plays over the original), so
            // A separate Dry+Wet mode would sound the same.
            let (a, b) = self.mix[u];
            for (k, (_, buf)) in blocks.iter_mut().enumerate() {
                if !live[k] {
                    continue;
                }
                let (wet, tails) = (&wet[span(k)], &tails[span(k)]);
                for (f, x) in buf.iter_mut().enumerate() {
                    let m = a + (b - a) * ramp(f);
                    for c in 0..2 {
                        x[c] += (wet[f][c] - x[c]) * m + tails[f][c] * m;
                    }
                }
            }
        }
        self.wet = wet;
        self.tails = tails;
    }

    /// Runs frame `f` of a buffer `frames` long of `target` through the units
    /// routed to it.
    #[cfg(test)]
    pub(crate) fn process(&mut self, target: usize, mut x: [f32; 2], f: usize, frames: usize) -> [f32; 2] {
        let r = (f + 1) as f32 / frames as f32;
        let by = (f + 1) as f32 * fade_step(self.sample_rate);
        for u in 0..NUM_EFFECT_UNITS {
            if !self.active[u][target] {
                continue;
            }
            let mut wet = x;
            let mut tails = [0.0f32; 2];
            for s in 0..EFFECTS_PER_UNIT {
                let Some(inst) = self.instance(u, s, target) else {
                    continue;
                };
                if !inst.active() {
                    continue;
                }
                if inst.release > 0.0 {
                    let y = inst.tail();
                    tails[0] += y[0];
                    tails[1] += y[1];
                    continue;
                }
                let g = fade(inst.gain, inst.target, by);
                let y = inst.process(wet);
                for c in 0..2 {
                    wet[c] += (y[c] - wet[c]) * g;
                }
            }
            // Dry/wet. Every effect's output already carries the dry signal
            // where it musically belongs (an echo plays over the original), so
            // A separate Dry+Wet mode would sound the same.
            let (a, b) = self.mix[u];
            let m = a + (b - a) * r;
            for c in 0..2 {
                x[c] += (wet[c] - x[c]) * m + tails[c] * m;
            }
        }
        x
    }

    /// Ends a buffer `frames` long: the next one fades on from where this
    /// one finished.
    pub(crate) fn finish(&mut self, frames: usize) {
        let by = frames as f32 * fade_step(self.sample_rate);
        for inst in &mut self.instances {
            inst.gain = fade(inst.gain, inst.target, by);
        }
    }
}

/// How long an effect takes to fade in or out when it is switched, in
/// seconds: at least 128 samples at any rate, whatever the buffer size, so
/// switching never clicks (a 32-frame buffer used to switch in 0.7 ms).
pub(crate) const FX_FADE_SECONDS: f32 = 0.005;

/// How far an on/off fade moves in one sample at `sample_rate`.
pub(crate) fn fade_step(sample_rate: f32) -> f32 {
    1.0 / (FX_FADE_SECONDS * sample_rate).max(1.0)
}

/// `from` moved by `by` towards `to`, stopping there.
#[inline]
pub(crate) fn fade(from: f32, to: f32, by: f32) -> f32 {
    if to > from {
        (from + by).min(to)
    } else {
        (from - by).max(to)
    }
}

/// Per-sample glide of effect parameters towards the value read for the
/// buffer: about 5 ms at 48 kHz, like the filters' cutoff glide.
const GLIDE: f32 = 0.004;

/// Moves `v` one sample's glide towards `target`.
#[inline]
fn glide(v: &mut f32, target: f32) {
    let d = target - *v;
    if d != 0.0 {
        // Lands exactly, so a settled knob costs only the comparison.
        *v = if d.abs() < 1e-6 { target } else { *v + d * GLIDE };
    }
}

/// Four-point cubic (Catmull-Rom) interpolation between `y1` and `y2`, `t` of the way, with `y0` before
/// and `y3` after them, after Signalsmith DSP's cubic interpolator (MIT). Halfway between samples it
/// loses under 1 dB at 10 kHz, where linear interpolation loses 2 dB, so swept delays keep their highs.
#[inline]
fn cubic(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let a = 0.5 * (y2 - y0);
    let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    y1 + t * (a + t * (b + t * c))
}

impl Attached {
    /// Sends the knobs that moved, scaled to the plugin's own ranges.
    /// Sends the knobs that moved; true when any did.
    fn send_params(&mut self, params: &[f32]) -> bool {
        let mut moved = false;
        for (k, (&(lo, hi), &v)) in self.ranges.iter().zip(params).enumerate() {
            if v != self.sent[k] {
                self.sent[k] = v;
                self.conn.set_param(k, lo + v * (hi - lo));
                moved = true;
            }
        }
        moved
    }
}

/// Reads a delay line `delay` samples behind `write` (the next slot to be
/// written), with cubic interpolation.
#[inline]
fn tap(buf: &[f32], write: usize, delay: f32) -> f32 {
    let len = buf.len();
    let mut pos = write as f32 - delay;
    if pos < 0.0 {
        pos += len as f32;
    }
    let i = (pos as usize).min(len - 1);
    let t = pos - i as f32;
    if i >= 1 && i + 2 < len {
        // Away from the buffer's ends: one bounds check for all four.
        let w = &buf[i - 1..i + 3];
        return cubic(w[0], w[1], w[2], w[3], t);
    }
    let at = |k: usize| buf[(i + len + k - 1) % len];
    cubic(at(0), at(1), at(2), at(3), t)
}

/// `2^lo .. 2^hi` beats, in whole powers of two, picked by a 0..1 parameter.
fn beats(p: f32, lo: i32, hi: i32) -> f32 {
    2f32.powi(lo + (p * (hi - lo) as f32).round() as i32)
}

/// Gentle saturation for feedback paths, so repeats can't build up and clip
/// hard: a rational tanh approximation, close to linear for small signals.
#[inline]
fn soft(v: f32) -> f32 {
    let v = v.clamp(-3.0, 3.0);
    v * (27.0 + v * v) / (27.0 + 9.0 * v * v)
}

/// Tempo-synced stereo echo with filtered, optionally ping-ponging repeats.
struct Echo {
    len: usize,
    write: usize,
    /// Delay in samples, gliding towards `target` so time changes do not click.
    delay: f32,
    target: f32,
    /// Feedback, gliding towards `feedback_target` so turning it doesn't zip.
    feedback: f32,
    feedback_target: f32,
    /// Ping-pong and send, gliding towards their targets.
    pingpong: f32,
    pingpong_target: f32,
    send: f32,
    send_target: f32,
    tone: [f32; 2],
    /// One-pole coefficient darkening each repeat (about 6 kHz).
    tone_coef: f32,
    /// Reset since the last `prepare`: the glides start at their targets.
    fresh: bool,
}

impl Echo {
    fn new(sample_rate: f32) -> Self {
        Self {
            len: (ECHO_MAX_SECONDS * sample_rate) as usize + 4,
            write: 0,
            delay: -1.0,
            target: 1.0,
            feedback: 0.0,
            feedback_target: 0.0,
            pingpong: 0.0,
            pingpong_target: 0.0,
            send: 0.0,
            send_target: 0.0,
            tone: [0.0; 2],
            tone_coef: 1.0 - (-TAU * 6_000.0 / sample_rate).exp(),
            fresh: true,
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        self.write = 0;
        self.delay = -1.0;
        self.tone = [0.0; 2];
        self.fresh = true;
    }

    /// Starts the glides at their targets, the first time after a reset.
    fn settle(&mut self) {
        if self.fresh {
            self.fresh = false;
            self.feedback = self.feedback_target;
            self.pingpong = self.pingpong_target;
            self.send = self.send_target;
        }
    }

    /// Sets the delay to `beats` at the current tempo, within the buffer.
    fn set_time(&mut self, beats: f32, ctx: Ctx) {
        self.target = (beats * ctx.beat_seconds * ctx.sample_rate).clamp(1.0, (self.len - 2) as f32);
        if self.delay < 0.0 {
            self.delay = self.target;
        }
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time: 1/16 to 4 beats (as long as the buffer allows at slow tempos).
        self.set_time(beats(p[0], -4, 2), ctx);
        self.feedback_target = p[1] * 0.9;
        self.pingpong_target = p[2];
        self.send_target = p[3];
        self.settle();
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        self.process_with(mem, x, |v| v)
    }

    /// Like `process`, with `filter` applied to the repeats on their way
    /// back into the delay (after the tape-like darkening).
    #[inline]
    fn process_with(&mut self, mem: &mut [f32], x: [f32; 2], mut filter: impl FnMut([f32; 2]) -> [f32; 2]) -> [f32; 2] {
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        self.delay += (self.target - self.delay) * 0.0005;
        self.feedback += (self.feedback_target - self.feedback) * 0.002;
        glide(&mut self.pingpong, self.pingpong_target);
        glide(&mut self.send, self.send_target);
        let e = [tap(l, self.write, self.delay), tap(r, self.write, self.delay)];
        let pp = self.pingpong;
        let fed = [e[0] * (1.0 - pp) + e[1] * pp, e[1] * (1.0 - pp) + e[0] * pp];
        // Each repeat a little darker, as on a tape echo.
        for (t, f) in self.tone.iter_mut().zip(fed) {
            *t += (f - *t) * self.tone_coef;
        }
        let back = filter(self.tone);
        l[self.write] = x[0] * self.send + soft(back[0] * self.feedback);
        r[self.write] = x[1] * self.send + soft(back[1] * self.feedback);
        self.write += 1;
        if self.write == self.len {
            self.write = 0;
        }
        [x[0] + e[0], x[1] + e[1]]
    }
}

/// A delay line inside an instance's shared buffer.
#[derive(Clone, Copy, Default)]
struct Line {
    at: usize,
    len: usize,
    pos: usize,
}

impl Line {
    /// The sample written `delay` samples ago (1 = the last one written).
    #[inline]
    fn read(&self, mem: &[f32], delay: usize) -> f32 {
        let d = delay.min(self.len);
        let i = if self.pos >= d { self.pos - d } else { self.pos + self.len - d };
        mem[self.at + i]
    }

    /// Like [`Line::read`], between samples, with linear interpolation. The reverb's tank only
    /// swings a few samples for a slow chorus, where cubic interpolation costs a third more CPU for
    /// no audible change (the slight softening of the highs is far below the tank's own damping).
    #[inline]
    fn read_frac(&self, mem: &[f32], delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.len - 1) as f32);
        let i = d as usize;
        let f = d - i as f32;
        let a = self.read(mem, i);
        let b = self.read(mem, i + 1);
        a + (b - a) * f
    }

    #[inline]
    fn write(&mut self, mem: &mut [f32], v: f32) {
        mem[self.at + self.pos] = v;
        self.pos += 1;
        if self.pos == self.len {
            self.pos = 0;
        }
    }

    /// A Schroeder allpass with this line as its delay: returns the output.
    #[inline]
    fn allpass(&mut self, mem: &mut [f32], x: f32, g: f32) -> f32 {
        let d = self.read(mem, self.len);
        let v = x - g * d;
        self.write(mem, v);
        d + g * v
    }
}

/// Jon Dattorro's plate reverb ("Effect Design, Part 1", JAES 1997), the algorithm: input diffusers
/// into a figure-eight tank of modulated allpasses, damped delays and decay, with the stereo output
/// tapped from several points of the tank. Smoother and less metallic than Freeverb's parallel combs.
struct Reverb {
    /// Four input diffusers, then per tank half: modulated allpass, delay,
    /// allpass, delay.
    diffusers: [Line; 4],
    tank: [[Line; 4]; 2],
    /// Buffer space all of the above need.
    size: usize,
    /// The tunings, rescaled from the paper's 29.761 kHz.
    scale: f32,
    lfo: f32,
    lfo_step: f32,
    bandwidth: f32,
    input: f32,
    damping: f32,
    damped: [f32; 2],
    decay: f32,
    send: f32,
    /// Decay, bandwidth, damping and send as last set, which the values
    /// above glide towards per sample.
    targets: [f32; 4],
    fresh: bool,
    /// The tank's output from the other half, fed round the figure eight.
    cross: [f32; 2],
    /// [`OUTPUT_TAPS`] at this sample rate.
    taps: [[usize; 7]; 2],
}

/// Dattorro's delay lengths, at 29 761 Hz.
const DIFFUSER_TUNING: [usize; 4] = [142, 107, 379, 277];
const TANK_TUNING: [[usize; 4]; 2] = [[672, 4453, 1800, 3720], [908, 4217, 2656, 3163]];
/// Where the left and right outputs tap the tank, at 29 761 Hz (Dattorro's
/// table 2), in the order `process` sums them.
const OUTPUT_TAPS: [[usize; 7]; 2] =
    [[266, 2974, 1913, 1996, 1990, 187, 1066], [353, 3627, 1228, 2673, 2111, 335, 121]];
/// The modulated allpasses swing this many samples (at 29 761 Hz) each way.
const TANK_EXCURSION: f32 = 16.0;

impl Reverb {
    fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 29_761.0;
        let mut offset = 0;
        let mut place = |len: usize, extra: usize| {
            let len = ((len as f32 * scale) as usize).max(1) + extra;
            let line = Line { at: offset, len, pos: 0 };
            offset += len;
            line
        };
        let diffusers = DIFFUSER_TUNING.map(|l| place(l, 0));
        let excursion = (TANK_EXCURSION * scale) as usize + 2;
        let tank = TANK_TUNING
            .map(|half| [place(half[0], excursion), place(half[1], 0), place(half[2], 0), place(half[3], 0)]);
        Self {
            diffusers,
            tank,
            size: offset,
            scale,
            lfo: 0.0,
            lfo_step: 1.0 / sample_rate,
            bandwidth: 0.9995,
            input: 0.0,
            damping: 0.0005,
            damped: [0.0; 2],
            decay: 0.5,
            send: 0.0,
            targets: [0.5, 0.9995, 0.0005, 0.0],
            fresh: true,
            cross: [0.0; 2],
            taps: OUTPUT_TAPS.map(|side| side.map(|n| ((n as f32 * scale) as usize).max(1))),
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..self.size].fill(0.0);
        self.input = 0.0;
        self.damped = [0.0; 2];
        self.cross = [0.0; 2];
        self.fresh = true;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        // Decay: a short room to a long hall.
        self.targets = [0.2 + 0.77 * p[0], 0.1 + 0.8995 * p[1], 0.8 * p[2], p[3]];
        if self.fresh {
            self.fresh = false;
            [self.decay, self.bandwidth, self.damping, self.send] = self.targets;
        }
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let [decay, bandwidth, damping, send] = self.targets;
        glide(&mut self.decay, decay);
        glide(&mut self.bandwidth, bandwidth);
        glide(&mut self.damping, damping);
        glide(&mut self.send, send);
        self.input += ((x[0] + x[1]) * 0.5 - self.input) * self.bandwidth;
        let mut v = self.input;
        for (d, g) in self.diffusers.iter_mut().zip([0.75, 0.75, 0.625, 0.625]) {
            v = d.allpass(mem, v, g);
        }
        // About 1 Hz of chorus in the tank keeps the tail from ringing.
        self.lfo += self.lfo_step;
        if self.lfo >= 1.0 {
            self.lfo -= 1.0;
        }
        let (sin, cos) = (self.lfo * TAU).sin_cos();
        let swing = TANK_EXCURSION * self.scale;
        let mut out = [0.0f32; 2];
        for half in 0..2 {
            let mod_ = if half == 0 { sin } else { cos };
            let [ap1, d1, ap2, d2] = &mut self.tank[half];
            // The decay diffuser: an allpass whose length swings with the LFO.
            let input = v + self.cross[1 - half] * self.decay;
            let base = ap1.len as f32 - swing - 1.0;
            let delayed = ap1.read_frac(mem, base + swing * mod_);
            let w = input + 0.7 * delayed;
            ap1.write(mem, w);
            let a = delayed - 0.7 * w;
            d1.write(mem, a);
            let t = d1.read(mem, d1.len);
            self.damped[half] += (t - self.damped[half]) * (1.0 - self.damping);
            let b = ap2.allpass(mem, self.damped[half] * self.decay, -0.5);
            d2.write(mem, b);
            self.cross[half] = d2.read(mem, d2.len);
        }
        let t = &self.taps;
        let [_, l2, l3, l4] = &self.tank[0];
        let [_, r2, r3, r4] = &self.tank[1];
        out[0] = r2.read(mem, t[0][0]) + r2.read(mem, t[0][1]) - r3.read(mem, t[0][2]) + r4.read(mem, t[0][3])
            - l2.read(mem, t[0][4])
            - l3.read(mem, t[0][5])
            - l4.read(mem, t[0][6]);
        out[1] = l2.read(mem, t[1][0]) + l2.read(mem, t[1][1]) - l3.read(mem, t[1][2]) + l4.read(mem, t[1][3])
            - r2.read(mem, t[1][4])
            - r3.read(mem, t[1][5])
            - r4.read(mem, t[1][6]);
        [x[0] + out[0] * 0.6 * self.send, x[1] + out[1] * 0.6 * self.send]
    }
}

/// Resonant low-pass and high-pass in series,
/// but with state-variable filters that glide instead of jumping each buffer.
struct Filter {
    sample_rate: f32,
    lp: Svf,
    hp: Svf,
}

impl Filter {
    fn new(sample_rate: f32) -> Self {
        Self { sample_rate, lp: Svf::default(), hp: Svf::default() }
    }

    fn reset(&mut self) {
        self.lp = Svf::default();
        self.hp = Svf::default();
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        let [lp, res, hp] = [p[0], p[1], p[2]];
        // Low-pass 20 Hz..20 kHz, high-pass 20 Hz..10 kHz.
        let lp_hz = 20.0 * 1000f32.powf(lp);
        let hp_hz = 20.0 * 500f32.powf(hp);
        let mut q = 0.5 + 4.0 * res * res;
        // Two resonant peaks close together would add up: keep it tamer then.
        if lp < 0.995 && hp > 0.005 && lp_hz < 4.0 * hp_hz {
            q = q.min(1.0);
        }
        self.lp.set(self.sample_rate, lp_hz, q);
        self.hp.set(self.sample_rate, hp_hz, q);
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let y = self.lp.process(x)[0];
        self.hp.process(y)[2]
    }
}

/// A short swept delay with feedback; the channels' sweeps are 90° apart.
struct Flanger {
    len: usize,
    write: usize,
    phase: f32,
    step: f32,
    /// Centre delay in samples, depth, feedback and level makeup, gliding
    /// towards `targets` (in that order).
    centre: f32,
    depth: f32,
    feedback: f32,
    makeup: f32,
    targets: [f32; 4],
    fresh: bool,
    /// The LFO hasn't locked to the beat since the reset.
    unlocked: bool,
}

impl Flanger {
    fn new(sample_rate: f32) -> Self {
        Self {
            len: (FLANGER_MAX_SECONDS * sample_rate) as usize + 4,
            write: 0,
            phase: 0.0,
            step: 0.0,
            centre: 1.0,
            depth: 0.0,
            feedback: 0.0,
            makeup: 1.0,
            targets: [1.0, 0.0, 0.0, 1.0],
            fresh: true,
            unlocked: true,
        }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        self.write = 0;
        self.phase = 0.0;
        self.fresh = true;
        self.unlocked = true;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Speed: one sweep every 32 beats down to every half beat, in time
        // with the track and starting on a beat of it. Manual: 0.5..8 ms
        // centre delay.
        let cycle = beats(1.0 - p[0], -1, 5);
        self.step = 1.0 / (cycle * ctx.beat_seconds * ctx.sample_rate);
        lock_to_beat(&mut self.phase, &mut self.step, &mut self.unlocked, ctx.beat, cycle);
        self.targets = [(0.0005 + 0.0075 * p[2]) * ctx.sample_rate, p[1], 0.9 * p[3], flanger_makeup(p[1], p[2], p[3])];
        if self.fresh {
            self.fresh = false;
            [self.centre, self.depth, self.feedback, self.makeup] = self.targets;
        }
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let [centre, depth, feedback, makeup] = self.targets;
        glide(&mut self.centre, centre);
        glide(&mut self.depth, depth);
        glide(&mut self.feedback, feedback);
        glide(&mut self.makeup, makeup);
        let lfo = (self.phase * TAU).sin_cos();
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        let mut y = [0.0; 2];
        for (c, (buf, sweep)) in [(l, lfo.0), (r, lfo.1)].into_iter().enumerate() {
            let d = tap(buf, self.write, self.centre * (1.0 + 0.9 * sweep));
            buf[self.write] = x[c] + soft(d * self.feedback);
            y[c] = (x[c] * (1.0 - 0.5 * self.depth) + d * 0.5 * self.depth) * self.makeup;
        }
        self.write += 1;
        if self.write == self.len {
            self.write = 0;
        }
        y
    }
}

/// Flanger level makeup for its depth, manual and feedback knobs (0..1). At depth `d` the dry signal is `1 - d/2`
/// plus the copy at `d/2`, which sums 3 dB quieter at full depth; this puts that back, less at the shortest delays
/// (bass stays in phase) and with feedback (its resonance makes up the rest).
fn flanger_makeup(depth: f32, manual: f32, feedback: f32) -> f32 {
    let (dry, wet) = (1.0 - 0.5 * depth, 0.5 * depth);
    let loss = dry * dry + wet * wet;
    let share = (manual / 0.4).min(1.0) * (1.0 - feedback * feedback);
    loss.powf(-0.5 * share)
}

/// Reduces bit depth and sample rate.
struct Bitcrusher {
    step: f32,
    ratio: f32,
    acc: f32,
    held: [f32; 2],
}

impl Default for Bitcrusher {
    fn default() -> Self {
        Self { step: 0.0, ratio: 1.0, acc: 1.0, held: [0.0; 2] }
    }
}

impl Bitcrusher {
    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS]) {
        // Bit depth: 2..16 bits. Downsampling: 1 keeps every sample, 0 keeps one in 50.
        let bits = 2.0 + 14.0 * p[0];
        self.step = 2.0 / 2f32.powf(bits);
        self.ratio = 0.02 + 0.98 * p[1] * p[1];
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.acc += self.ratio;
        if self.acc >= 1.0 {
            self.acc -= 1.0;
            self.held = x;
        }
        self.held.map(|v| (v / self.step).round() * self.step)
    }
}

const PHASER_MAX_STAGES: usize = 12;
/// Frames between updates of the phaser's sweep.
const PHASER_UPDATE: u32 = 16;
/// Per-sample step of the crossfade when the phaser's stage count changes:
/// about 10 ms at 48 kHz.
const PHASER_STAGE_FADE: f32 = 1.0 / 480.0;

/// A chain of swept first-order allpasses mixed with the input.
#[derive(Default)]
struct Phaser {
    phase: f32,
    step: f32,
    /// The stage count in use, and the one it is crossfading from (the
    /// chain is serial, so both outputs come from one pass).
    stages: usize,
    old_stages: usize,
    /// How far the crossfade from `old_stages` has got, 0..1.
    fade: f32,
    /// Depth and feedback, gliding towards `targets`.
    depth: f32,
    feedback: f32,
    targets: [f32; 2],
    /// The sweep has locked to the beat since the reset.
    locked: bool,
    sample_rate: f32,
    coef: [f32; 2],
    state: [[f32; PHASER_MAX_STAGES]; 2],
    last: [f32; 2],
    countdown: u32,
}

impl Phaser {
    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Speed: one sweep every 32 beats down to every half beat, in time
        // with the track and starting on a beat of it. Stages: 2..12.
        let cycle = beats(1.0 - p[0], -1, 5);
        self.step = 1.0 / (cycle * ctx.beat_seconds * ctx.sample_rate);
        let mut unlocked = !self.locked;
        lock_to_beat(&mut self.phase, &mut self.step, &mut unlocked, ctx.beat, cycle);
        self.locked = !unlocked;
        if ctx.beat.is_some() {
            // A new phase takes effect at once, not at the next update.
            self.countdown = 0;
        }
        self.targets = [p[1], 0.9 * p[2]];
        let stages = 2 + 2 * (p[3] * 5.0).round() as usize;
        if self.stages == 0 {
            // Fresh: no glides or crossfade.
            [self.depth, self.feedback] = self.targets;
            self.old_stages = stages;
            self.fade = 1.0;
        } else if stages != self.stages {
            // Added stages start silent rather than with stale state.
            let busy = self.stages.max(self.old_stages);
            for state in &mut self.state {
                for z in state.iter_mut().take(stages).skip(busy) {
                    *z = 0.0;
                }
            }
            self.old_stages = self.stages;
            self.fade = 0.0;
        }
        self.stages = stages;
        self.sample_rate = ctx.sample_rate;
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        if self.countdown == 0 {
            self.countdown = PHASER_UPDATE;
            let (s, c) = (self.phase * TAU).sin_cos();
            for (coef, lfo) in self.coef.iter_mut().zip([s, c]) {
                // Notches sweep 200 Hz..3 kHz.
                let f = 200.0 * 15f32.powf((1.0 + lfo) * 0.5);
                let t = (PI * f / self.sample_rate).tan();
                *coef = (t - 1.0) / (t + 1.0);
            }
        }
        self.countdown -= 1;
        glide(&mut self.depth, self.targets[0]);
        glide(&mut self.feedback, self.targets[1]);
        let fading = self.fade < 1.0;
        let n = if fading {
            self.fade = (self.fade + PHASER_STAGE_FADE).min(1.0);
            self.stages.max(self.old_stages)
        } else {
            self.stages
        };
        let mut y = [0.0; 2];
        for c in 0..2 {
            let a = self.coef[c];
            let mut v = x[c] + self.last[c] * self.feedback;
            let (mut new, mut old) = (v, v);
            for (k, z) in self.state[c][..n].iter_mut().enumerate() {
                let out = a * v + *z;
                *z = v - a * out;
                v = out;
                if k + 1 == self.stages {
                    new = v;
                }
                if k + 1 == self.old_stages {
                    old = v;
                }
            }
            let v = if fading { old + (new - old) * self.fade } else { new };
            self.last[c] = v;
            y[c] = x[c] * (1.0 - 0.5 * self.depth) + v * 0.5 * self.depth;
        }
        y
    }
}

/// Trans / gate: chops the sound in time with the beat, with a short slew
/// on each edge so it doesn't click. It opens on the beat grid's divisions
/// (or, without a grid, from the moment it is turned on).
struct Gate {
    phase: f32,
    step: f32,
    depth: f32,
    width: f32,
    gain: f32,
    slew: f32,
    unlocked: bool,
}

impl Default for Gate {
    fn default() -> Self {
        Self { phase: 0.0, step: 0.0, depth: 0.0, width: 0.5, gain: 1.0, slew: 1.0, unlocked: true }
    }
}

impl Gate {
    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Length: 1/16 to 1 beat per open-and-close.
        let cycle = beats(p[0], -4, 0);
        self.step = 1.0 / (cycle * ctx.beat_seconds * ctx.sample_rate);
        lock_to_beat(&mut self.phase, &mut self.step, &mut self.unlocked, ctx.beat, cycle);
        self.depth = p[1];
        self.width = 0.1 + 0.8 * p[2];
        // Each edge takes about 2 ms.
        self.slew = 1.0 / (0.002 * ctx.sample_rate);
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let target = if self.phase < self.width { 1.0 } else { 1.0 - self.depth };
        self.gain += (target - self.gain).clamp(-self.slew, self.slew);
        [x[0] * self.gain, x[1] * self.gain]
    }
}

/// Beat roll: repeats a slice of 1/16 to 1 beat, each repeat faded in and out over 3 ms so the seams
/// don't click. With a beat grid it starts recording on the next division of the grid (the slice, or
/// a quarter beat for longer slices), so the roll lands on the music's beat; without one, at once.
struct Roll {
    len: usize,
    recorded: usize,
    slice: usize,
    pos: usize,
    fade: f32,
    /// Frames to play live before recording starts, and whether that wait
    /// has been worked out since the reset.
    wait: usize,
    waiting_set: bool,
    /// Rev Roll: plays the slice backwards.
    reverse: bool,
}

impl Roll {
    fn new(len: usize) -> Self {
        Self { len, recorded: 0, slice: 1, pos: 0, fade: 1.0, wait: 0, waiting_set: false, reverse: false }
    }

    fn reset(&mut self) {
        self.recorded = 0;
        self.pos = 0;
        self.wait = 0;
        self.waiting_set = false;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Rate: 0 rolls whole beats, 1 rolls 1/16 beats.
        let b = beats(1.0 - p[0], -4, 0);
        self.slice = ((b * ctx.beat_seconds * ctx.sample_rate) as usize).clamp(64, self.len - 1);
        self.fade = 0.003 * ctx.sample_rate;
        if !self.waiting_set {
            self.waiting_set = true;
            if let Some(beat) = ctx.beat {
                let grid = b.min(0.25) as f64;
                let to_next = (grid - beat.rem_euclid(grid)) % grid;
                self.wait = (to_next as f32 * ctx.beat_seconds * ctx.sample_rate) as usize;
            }
        }
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        if self.wait > 0 {
            self.wait -= 1;
            return x;
        }
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        if self.recorded < self.len {
            l[self.recorded] = x[0];
            r[self.recorded] = x[1];
            self.recorded += 1;
        }
        // Until the first slice is in, the roll plays it live.
        if self.recorded < self.slice {
            return x;
        }
        let slice = self.slice.min(self.recorded);
        if self.pos >= slice {
            self.pos = 0;
        }
        let p = self.pos;
        self.pos += 1;
        let env = (p as f32 / self.fade).min((slice - p) as f32 / self.fade).min(1.0);
        let i = if self.reverse { slice - 1 - p } else { p };
        [l[i] * env, r[i] * env]
    }
}

/// Echo out: catches the sound for a few beats and lets the echoes ring out
/// while the original fades away, for ending a track or a hard cut.
struct EchoOut {
    echo: Echo,
    /// Samples since it was turned on, and how long it listens for.
    t: f32,
    capture: f32,
}

impl EchoOut {
    fn new(sample_rate: f32) -> Self {
        Self { echo: Echo::new(sample_rate), t: 0.0, capture: 1.0 }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        self.echo.reset(mem);
        self.t = 0.0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time: 1/4 to 1 beat. Length: the original fades out over 1/2 to 4 beats.
        self.echo.set_time(beats(p[0], -2, 0), ctx);
        self.echo.feedback_target = 0.5 + 0.45 * p[1];
        self.echo.pingpong_target = 0.3;
        self.echo.send_target = 1.0 - (self.t / self.capture).min(1.0);
        self.echo.settle();
        self.capture = beats(p[2], -1, 2) * ctx.beat_seconds * ctx.sample_rate;
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let left = 1.0 - (self.t / self.capture).min(1.0);
        self.t += 1.0;
        // The fade is smooth already: no glide on top.
        self.echo.send = left;
        self.echo.send_target = left;
        let y = self.echo.process(mem, x);
        // The echo's output carries the dry signal: take away what has faded.
        [y[0] - x[0] * (1.0 - left), y[1] - x[1] * (1.0 - left)]
    }
}

/// Vinyl brake: slows the sound to a stop over 1 to 8 beats, pitch falling
/// with it, by playing back what it records at a falling speed.
struct Brake {
    len: usize,
    write: usize,
    /// How far behind the newest sample the playback has fallen.
    lag: f32,
    speed: f32,
    decel: f32,
}

impl Brake {
    fn new(len: usize) -> Self {
        Self { len, write: 0, lag: 0.0, speed: 1.0, decel: 0.0 }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        mem[..2 * self.len].fill(0.0);
        self.write = 0;
        self.lag = 0.0;
        self.speed = 1.0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Playback falls behind by half the stop time, which the buffer
        // (2 s) has to hold.
        let seconds = (beats(p[0], 0, 3) * ctx.beat_seconds).min(2.0 * ECHO_MAX_SECONDS - 0.2);
        self.decel = 1.0 / (seconds * ctx.sample_rate);
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        let (l, r) = mem[..2 * self.len].split_at_mut(self.len);
        l[self.write] = x[0];
        r[self.write] = x[1];
        self.write += 1;
        if self.write == self.len {
            self.write = 0;
        }
        if self.speed <= 0.0 {
            return [0.0; 2];
        }
        let delay = (1.0 + self.lag).min((self.len - 2) as f32);
        // Fade the last moment so the stop doesn't click.
        let g = (self.speed * 20.0).min(1.0);
        let y = [tap(l, self.write, delay) * g, tap(r, self.write, delay) * g];
        self.lag += 1.0 - self.speed;
        self.speed -= self.decel;
        y
    }
}

/// Noise sweep for build-ups: band-passed noise over the music, its level
/// and pitch rising together on the meta knob.
#[derive(Default)]
struct Noise {
    seed: u32,
    level: f32,
    gain: f32,
    filter: Svf,
}

impl Noise {
    fn reset(&mut self) {
        self.filter = Svf::default();
        self.gain = 0.0;
        if self.seed == 0 {
            self.seed = 0x2545_f491;
        }
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        self.level = 0.3 * p[0];
        // Sweep: the noise band from 300 Hz to 12 kHz. Resonance narrows it.
        self.filter.set(ctx.sample_rate, 300.0 * 40f32.powf(p[1]), 0.6 + 4.0 * p[2]);
    }

    #[inline]
    fn next(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    #[inline]
    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.gain += (self.level - self.gain) * 0.002;
        let n = [self.next(), self.next()];
        let band = self.filter.process(n)[1];
        // The band-pass output peaks at 1/k; scale it back to the noise level.
        let k = self.filter.k;
        [x[0] + band[0] * k * self.gain, x[1] + band[1] * k * self.gain]
    }
}

/// Spiral: an echo whose repeats come faster and faster, so the tail climbs
/// in pitch, after Pioneer's effect of that name.
struct Spiral {
    echo: Echo,
    /// The echo time it starts from, in samples, and samples since it was turned on.
    base: f32,
    t: f32,
    /// Samples for the echo time to halve.
    halving: f32,
}

impl Spiral {
    fn new(sample_rate: f32) -> Self {
        Self { echo: Echo::new(sample_rate), base: 1.0, t: 0.0, halving: 1.0 }
    }

    fn reset(&mut self, mem: &mut [f32]) {
        self.echo.reset(mem);
        self.t = 0.0;
    }

    fn prepare(&mut self, p: &[f32; MAX_PARAMETERS], ctx: Ctx) {
        // Time: 1/4 to 1 beat to start from; it halves every 4 beats.
        self.echo.set_time(beats(p[0], -2, 0), ctx);
        self.base = self.echo.target;
        self.echo.feedback_target = 0.4 + 0.5 * p[1];
        self.echo.pingpong_target = 0.5;
        self.echo.send_target = p[2];
        self.echo.settle();
        self.halving = 4.0 * ctx.beat_seconds * ctx.sample_rate;
    }

    #[inline]
    fn process(&mut self, mem: &mut [f32], x: [f32; 2]) -> [f32; 2] {
        // Down to an eighth of the starting time, then it holds there.
        self.echo.target = (self.base * (-self.t / self.halving).exp2()).max(self.base / 8.0);
        self.t += 1.0;
        self.echo.process(mem, x)
    }
}

/// The Color FX each deck's filter knob can play, Pioneer-style, as `[QuickEffectRack1_[ChannelN]],loaded_chain_preset`
/// values 1 and up (0 is none: the knob does nothing). Each deck picks its own from the menu beside
/// its filter knob; the DDJ-FLX4's SHIFT + SMART CFX steps both decks.
pub const COLOR_FX: [&str; 5] = ["Filter", "Space", "Dub Echo", "Noise", "Crush"];

/// The Color FX of a `loaded_chain_preset` value ("" for none).
pub fn color_fx_name(preset: f64) -> &'static str {
    let i = preset.round();
    if i >= 1.0 && i <= COLOR_FX.len() as f64 {
        COLOR_FX[i as usize - 1]
    } else {
        ""
    }
}

/// How long a Space or Dub Echo tail rings on once the knob is back in the
/// middle, in seconds, before the deck stops running it.
const COLOR_TAIL_SECONDS: f32 = 6.0;

/// A deck's Color FX: one bipolar knob, off in the middle. Filter sweeps a low-pass to the left and a high-pass to the
/// right; Space and Dub Echo send the sound, filtered the same way but more gently, into a reverb or a tempo-synced
/// echo, more the further the knob turns; Noise adds filtered white noise; Crush lowers the bit depth and sample rate.
pub(crate) struct ColorFx {
    preset: usize,
    sweep: crate::dsp::SweepFilter,
    inst: Instance,
    /// How far the knob is from the middle, 0..1, and how much of the effect
    /// (as opposed to plain filtering) is heard, gliding.
    amount: f32,
    wet: f32,
    /// Samples left of a ringing tail after the knob came back.
    tail: f32,
    sample_rate: f32,
}

impl ColorFx {
    pub(crate) fn new(sample_rate: f64) -> Self {
        let mut fx = Self {
            preset: 1,
            sweep: crate::dsp::SweepFilter::new(sample_rate),
            inst: Instance::with_memory(sample_rate as f32),
            amount: 0.0,
            wet: 0.0,
            tail: 0.0,
            sample_rate: sample_rate as f32,
        };
        fx.load(1);
        fx
    }

    fn load(&mut self, preset: usize) {
        self.preset = preset;
        self.inst.kind = match preset {
            2 => Some(Kind::Reverb),
            3 => Some(Kind::Echo),
            4 => Some(Kind::Noise),
            5 => Some(Kind::Bitcrusher),
            _ => None,
        };
        self.inst.reset();
        self.tail = 0.0;
        self.wet = 0.0;
    }

    /// Once per buffer: the chosen preset, the knob (0..1, 0.5 off) and the
    /// deck's beat length in seconds.
    pub(crate) fn prepare(&mut self, preset: f64, knob: f64, beat_seconds: f64) {
        let preset = preset.round().clamp(0.0, COLOR_FX.len() as f64) as usize;
        if preset != self.preset {
            self.load(preset);
        }
        let side = knob - 0.5;
        let a = (side.abs() * 2.0).min(1.0) as f32;
        self.amount = a;
        // Filter alone sweeps all the way; the others filter gently.
        let filter_knob = match preset {
            1 => knob,
            2 | 3 => 0.5 + side * 0.55,
            4 => 0.5 + side * 0.35,
            5 => 0.5 + side * 0.25,
            _ => 0.5,
        };
        self.sweep.set(filter_knob);
        if a > 0.005 {
            self.tail = COLOR_TAIL_SECONDS * self.sample_rate;
        }
        let ctx = Ctx { sample_rate: self.sample_rate, beat_seconds: beat_seconds as f32, beat: None };
        let p = match preset {
            // Decay, bandwidth, damping, send.
            2 => [0.55 + 0.4 * a, 0.7, 0.35, a],
            // Half a beat; feedback grows with the knob; send.
            3 => [beats_parameter(0.5, -4, 2), 0.55 + 0.35 * a, 0.0, a],
            // Level, band (low noise to the left, high to the right), resonance.
            4 => [a, (0.5 + side as f32).clamp(0.0, 1.0), 0.3, 0.0],
            // Bit depth, downsampling.
            5 => [1.0 - 0.75 * a, 1.0 - 0.92 * a, 0.0, 0.0],
            _ => [0.0; MAX_PARAMETERS],
        };
        if self.inst.kind.is_some() {
            self.inst.prepare(&p, ctx);
        }
    }

    #[inline]
    pub(crate) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let y = self.sweep.process(x);
        let Some(kind) = self.inst.kind else {
            return y;
        };
        if self.tail <= 0.0 && self.amount == 0.0 {
            self.wet = 0.0;
            return y;
        }
        self.tail -= 1.0;
        // Crush fades in over the first stretch of the knob, so a knob a hair
        // off the middle doesn't quantize; the others scale themselves.
        let goal = match kind {
            Kind::Bitcrusher => (self.amount / 0.05).min(1.0),
            _ => 1.0,
        };
        self.wet += (goal - self.wet) * 0.002;
        if (goal - self.wet).abs() < 1e-5 {
            self.wet = goal;
        }
        let z = self.inst.process(y);
        [y[0] + (z[0] - y[0]) * self.wet, y[1] + (z[1] - y[1]) * self.wet]
    }
}

/// The 0..1 parameter that [`beats`] reads back as `wanted` beats.
fn beats_parameter(wanted: f32, lo: i32, hi: i32) -> f32 {
    (wanted.log2() - lo as f32) / (hi - lo) as f32
}

/// The effects a deck's stems can be sent into, as `[ChannelN],stem_fx` values 1 and up. One
/// per deck: the stems with their quick effect on (`[QuickEffectRack1_[ChannelN_StemM]],enabled`)
/// share it, which sounds the same as one each since both effects are linear.
pub const STEM_FX: [&str; 2] = ["Echo", "Reverb"];

/// How long a stem effect rings on once nothing is sent into it, in
/// seconds, before the deck stops running it.
const STEM_TAIL_SECONDS: f32 = 8.0;

/// The echo's time, in beats.
const STEM_ECHO_BEATS: f32 = 0.5;

/// A deck's stem effect: the stems sent to it (each at its level, with the dry stems
/// still playing in the deck) come back as the effect alone, added to the deck.
/// Muting a sent stem leaves its echo or reverb ringing on: the echo out of a vocal.
pub(crate) struct StemFx {
    preset: usize,
    inst: Instance,
    /// Samples the tail rings on once nothing is sent, and its full length.
    tail: f32,
    tail_len: f32,
    sample_rate: f32,
}

impl StemFx {
    pub(crate) fn new(sample_rate: f64) -> Self {
        let mut fx = Self {
            preset: 0,
            inst: Instance::with_memory(sample_rate as f32),
            tail: 0.0,
            tail_len: STEM_TAIL_SECONDS * sample_rate as f32,
            sample_rate: sample_rate as f32,
        };
        fx.load(1);
        fx
    }

    fn load(&mut self, preset: usize) {
        self.preset = preset;
        self.inst.kind = Some(if preset == 2 { Kind::Reverb } else { Kind::Echo });
        self.clear();
    }

    /// Silences the effect at once, tail and all (a new track).
    pub(crate) fn clear(&mut self) {
        self.inst.reset();
        self.tail = 0.0;
    }

    /// Once per buffer: the chosen effect (`stem_fx`) and the deck's beat
    /// length in seconds.
    pub(crate) fn prepare(&mut self, preset: f64, beat_seconds: f64) {
        let preset = (preset.round() as usize).clamp(1, STEM_FX.len());
        if preset != self.preset {
            self.load(preset);
        }
        let ctx = Ctx { sample_rate: self.sample_rate, beat_seconds: beat_seconds as f32, beat: None };
        let p = match preset {
            // Decay, bandwidth, damping, send: a big, slightly dark hall.
            2 => [0.8, 0.7, 0.35, 1.0],
            // Half a beat, repeats fading over a couple of bars, a little
            // ping-pong, full send.
            _ => [beats_parameter(STEM_ECHO_BEATS, -4, 2), 0.72, 0.25, 1.0],
        };
        self.inst.prepare(&p, ctx);
    }

    /// How long a stem must have been sent before muting it is heard, in
    /// seconds: one echo, so the echo holds what was just played rather
    /// than starting empty when the effect and the mute come together.
    pub(crate) fn capture_seconds(&self, beat_seconds: f64) -> f64 {
        match self.preset {
            2 => 0.25,
            _ => STEM_ECHO_BEATS as f64 * beat_seconds,
        }
    }

    /// Whether the effect still has something to play.
    pub(crate) fn ringing(&self) -> bool {
        self.tail > 0.0
    }

    /// The effect alone for `x` sent in; `sending` while any stem is sent.
    #[inline]
    pub(crate) fn process(&mut self, x: [f32; 2], sending: bool) -> [f32; 2] {
        if sending {
            self.tail = self.tail_len;
        } else if self.tail <= 0.0 {
            return [0.0; 2];
        } else {
            self.tail -= 1.0;
            if self.tail <= 0.0 {
                // Rung out: start clean next time.
                self.clear();
                return [0.0; 2];
            }
        }
        let y = self.inst.process(x);
        // Fades over the last quarter of the tail, so cutting it is silent.
        let fade = (self.tail / (0.25 * self.tail_len)).min(1.0);
        [(y[0] - x[0]) * fade, (y[1] - x[1]) * fade]
    }
}

mod extra;

#[cfg(test)]
mod bench;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_notes_follow_note_ons_offs_and_all_notes_off() {
        let on = |k| Midi::note_on(0, 0, k, 100);
        let held = held_after(0, &[on(60), on(64), Midi::note_off(0, 3, 60)]);
        assert_eq!(held, 1 << 64, "any channel's note-off ends it");
        let zero_velocity = Midi { frame: 0, data: [0x90, 64, 0] };
        assert_eq!(held_after(held, &[zero_velocity]), 0);
        let all_off = Midi { frame: 0, data: [0xb2, 123, 0] };
        assert_eq!(held_after(held | 1 << 36, &[all_off]), 0);
    }

    const SR: f32 = 48_000.0;
    const CTX: Ctx = Ctx { sample_rate: SR, beat_seconds: 0.5, beat: None };

    /// An instance of `kind` with its default parameters, fed `input` frames.
    fn run(kind: Kind, input: impl Fn(usize) -> f32, frames: usize) -> Vec<f32> {
        let mut inst = Instance::with_memory(SR);
        inst.kind = Some(kind);
        inst.reset();
        let defaults = MANIFESTS[kind as usize].defaults;
        let p = std::array::from_fn(|k| defaults.get(k).copied().unwrap_or(0.5) as f32);
        inst.prepare(&p, CTX);
        (0..frames)
            .map(|i| {
                let v = input(i);
                inst.process([v, v])[0]
            })
            .collect()
    }

    fn tone(i: usize) -> f32 {
        (TAU * 220.0 * i as f32 / SR).sin() * 0.5
    }

    /// A deck's Color FX `preset` with the knob at `knob`, fed a tone for
    /// half a second and then silence for another; returns the two halves.
    fn color(preset: f64, knob: f64) -> (Vec<f32>, Vec<f32>) {
        let mut fx = ColorFx::new(SR as f64);
        fx.prepare(preset, knob, 0.5);
        let half = SR as usize / 2;
        let a = (0..half).map(|i| fx.process([tone(i); 2])[0]).collect();
        let b = (0..half).map(|_| fx.process([0.0; 2])[0]).collect();
        (a, b)
    }

    #[test]
    fn color_fx_in_the_middle_leaves_the_sound_alone() {
        for preset in 0..=COLOR_FX.len() {
            let (a, b) = color(preset as f64, 0.5);
            let dry: Vec<f32> = (0..a.len()).map(tone).collect();
            let err = a.iter().zip(&dry).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max);
            assert!(err < 1e-6, "preset {preset} changed the sound by {err}");
            assert!(rms(&b) == 0.0, "preset {preset} made sound out of silence");
        }
    }

    #[test]
    fn space_and_dub_echo_ring_on_after_the_sound_stops() {
        for preset in [2.0, 3.0] {
            for knob in [0.1, 0.9] {
                let (_, tail) = color(preset, knob);
                assert!(rms(&tail) > 0.005, "preset {preset} at {knob} left no tail");
            }
        }
    }

    #[test]
    fn noise_and_crush_change_the_sound() {
        for preset in [4.0, 5.0] {
            let (a, _) = color(preset, 1.0);
            let dry: Vec<f32> = (0..a.len()).map(tone).collect();
            let diff: Vec<f32> = a.iter().zip(&dry).map(|(x, y)| x - y).collect();
            assert!(rms(&diff) > 0.01, "preset {preset} did nothing");
        }
    }

    #[test]
    fn color_fx_names() {
        assert_eq!(color_fx_name(1.0), "Filter");
        assert_eq!(color_fx_name(3.0), "Dub Echo");
        assert_eq!(color_fx_name(0.0), "");
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn the_beat_switch_sets_each_effects_length() {
        let reg = ControlRegistry::new();
        let mut slot = SlotControls::create(&reg, 0, 0);
        let set = |k: &str, v: f64| reg.get("[EffectRack1_EffectUnit1_Effect1]", k).unwrap().set(v);
        let get = |k: &str| reg.get("[EffectRack1_EffectUnit1_Effect1]", k).unwrap().get();
        // Echo: 1/16 to 4 beats, all of them.
        set("loaded_effect", 1.0);
        slot.update();
        // A new effect starts at 1/4 beat.
        assert_eq!(get("beats"), 0.25);
        for want in [0.0625, 0.5, 4.0] {
            set("beats", want);
            slot.update();
            assert_eq!(get("beats"), want);
            assert_eq!(beats(get("parameter1") as f32, -4, 2) as f64, want);
        }
        // Flanger sweeps 1/2 beat at the shortest; the switch stops there.
        set("loaded_effect", 4.0);
        slot.update();
        set("beats", 0.125);
        slot.update();
        assert_eq!(get("beats"), 0.5);
        assert_eq!(beats(1.0 - get("parameter1") as f32, -1, 5), 0.5);
        // Reverb has no length.
        set("loaded_effect", 2.0);
        slot.update();
        assert_eq!(get("beats"), 0.0);
    }

    #[test]
    fn legacy_flanger_plays_through_the_decks_pad_fx_unit() {
        let reg = ControlRegistry::new();
        let mut rack = EffectRack::new(&reg, SR as f64);
        let beats = [0.5; TARGETS];
        let unit = |k: &str| reg.get("[EffectRack1_EffectUnit7]", k).unwrap();
        let slot = |k: &str| reg.get("[EffectRack1_EffectUnit7_Effect1]", k).unwrap();
        let flanger = |k: &str| reg.get("[Flanger]", k).unwrap();
        assert!(!unit("group_[Channel2]_enable").get_bool());

        // Deck 2's flanger loads the Flanger into its Pad FX unit, on deck 2.
        reg.get("[Channel2]", "flanger").unwrap().set_bool(true);
        rack.prepare(beats, 0.0);
        rack.prepare(beats, 0.01);
        assert_eq!(slot("loaded_effect").get(), FLANGER);
        assert!(slot("enabled").get_bool());
        assert!(unit("group_[Channel2]_enable").get_bool());
        assert_eq!(slot("meta").get(), 1.0, "lfoDepth starts full");

        // The old knobs follow: depth through meta, delay to Manual, a
        // shorter period to a faster Speed.
        flanger("lfoDepth").set(0.25);
        flanger("lfoDelay").set(50.0);
        flanger("lfoPeriod").set(50_000.0);
        rack.prepare(beats, 0.02);
        assert_eq!(slot("meta").get(), 0.25);
        assert_eq!(slot("parameter3").get(), 0.0);
        assert_eq!(slot("parameter1").get(), 1.0);

        reg.get("[Channel2]", "flanger").unwrap().set_bool(false);
        rack.prepare(beats, 0.03);
        assert!(!slot("enabled").get_bool());
    }

    #[test]
    fn beat_fx_taps_a_tempo_and_releases_into_its_tail() {
        let reg = ControlRegistry::new();
        let mut rack = EffectRack::new(&reg, SR as f64);
        let unit = |k: &str| reg.get("[EffectRack1_EffectUnit5]", k).unwrap();
        let slot = |k: &str| reg.get("[EffectRack1_EffectUnit5_Effect1]", k).unwrap();
        let beats = [0.5; TARGETS];
        // Routed to both decks, with Echo loaded.
        assert!(unit("group_[Channel1]_enable").get_bool());
        assert!(unit("group_[Channel2]_enable").get_bool());
        assert_eq!(slot("loaded_effect").get(), 1.0);

        // Taps half a second apart: 120 BPM; AUTO goes back to the decks.
        for i in 0..4 {
            unit("bpm_tap").set(1.0);
            rack.prepare(beats, i as f64 * 0.5);
            unit("bpm_tap").set(0.0);
            rack.prepare(beats, i as f64 * 0.5 + 0.01);
        }
        assert!((unit("bpm").get() - 120.0).abs() < 1e-6);
        unit("bpm_auto").set(1.0);
        rack.prepare(beats, 3.0);
        unit("bpm_auto").set(0.0);
        assert_eq!(unit("bpm").get(), 0.0);

        // Echo on deck 1, then released: the deck plays dry again with the
        // echoes of what came before ringing on, and later they end.
        let buffer = |rack: &mut EffectRack, input: f32, now: f64| {
            rack.prepare(beats, now);
            let out: Vec<f32> = (0..480)
                .map(|i| {
                    let v = input * tone(i);
                    rack.process(0, [v, v], i, 480)[0]
                })
                .collect();
            rack.finish(480);
            out
        };
        slot("meta").set(0.8);
        slot("enabled").set_bool(true);
        for i in 0..100 {
            buffer(&mut rack, 1.0, 4.0 + i as f64 * 0.01);
        }
        unit("release").set(1.0);
        let released = buffer(&mut rack, 0.0, 5.0);
        unit("release").set(0.0);
        assert!(!slot("enabled").get_bool(), "release turns the effect off");
        let mut later = Vec::new();
        for i in 0..50 {
            later.extend(buffer(&mut rack, 0.0, 5.01 + i as f64 * 0.01));
        }
        assert!(rms(&released) + rms(&later) > 0.01, "the echoes ring on");
        // Eight beats at 120 BPM is 4 s: well after that, silence.
        let mut end = Vec::new();
        for i in 0..500 {
            end = buffer(&mut rack, 0.0, 6.0 + i as f64 * 0.01);
        }
        assert!(rms(&end) < 1e-6, "the tail ends: {}", rms(&end));
    }

    #[test]
    fn switching_an_effect_fades_over_a_fixed_time_even_in_short_buffers() {
        // A 2-bit Bitcrusher turns a steady 0.3 into 0.5, so the output
        // shows how far the effect has faded in.
        let reg = ControlRegistry::new();
        let mut rack = EffectRack::new(&reg, SR as f64);
        let unit = |k: &str| reg.get("[EffectRack1_EffectUnit1]", k).unwrap();
        let slot = |s: usize, k: &str| reg.get(&format!("[EffectRack1_EffectUnit1_Effect{s}]"), k).unwrap();
        unit("group_[Channel1]_enable").set(1.0);
        unit("enabled").set(1.0);
        unit("mix").set(1.0);
        slot(1, "loaded_effect").set(5.0);
        for s in 1..=3 {
            slot(s, "enabled").set(0.0);
        }
        let beats = [0.5; TARGETS];
        // Loading sets the slot's knobs to the effect's defaults.
        rack.prepare(beats, 0.0);
        slot(1, "parameter1").set(0.0);
        slot(1, "parameter2").set(1.0);
        const FRAMES: usize = 32;
        let run = |rack: &mut EffectRack, buffers: usize| {
            let mut level = Vec::new();
            for n in 0..buffers {
                rack.prepare(beats, n as f64 * 0.001);
                let mut buf = [[0.3f32; 2]; FRAMES];
                rack.process_block(0, &mut buf);
                rack.finish(FRAMES);
                level.extend(buf.iter().map(|x| (x[0] - 0.3) / 0.2));
            }
            level
        };
        rack.prepare(beats, 0.0);
        slot(1, "enabled").set(1.0);
        let on = run(&mut rack, 20);
        let fade = (FX_FADE_SECONDS * SR) as usize;
        assert!(fade >= 128);
        // Not all the way on after one short buffer, but within the fade.
        assert!(on[FRAMES - 1] < 0.5, "{}", on[FRAMES - 1]);
        assert!((on[fade] - 1.0).abs() < 1e-3, "{}", on[fade]);
        let largest = on.windows(2).map(|w| w[1] - w[0]).fold(0.0, f32::max);
        assert!(largest <= 1.5 / fade as f32, "steps by {largest}");
        // And off again just as smoothly.
        slot(1, "enabled").set(0.0);
        let off = run(&mut rack, 20);
        assert!(off[FRAMES - 1] > 0.5, "{}", off[FRAMES - 1]);
        assert!(off[fade].abs() < 1e-3, "{}", off[fade]);
    }

    #[test]
    fn a_block_at_a_time_sounds_the_same_as_frame_by_frame() {
        // Two identical racks: one run frame by frame, one a block at a time.
        let setup = || {
            let reg = ControlRegistry::new();
            let rack = EffectRack::new(&reg, SR as f64);
            for u in [1, 5] {
                let unit = format!("[EffectRack1_EffectUnit{u}]");
                reg.get(&unit, "group_[Channel1]_enable").unwrap().set(1.0);
                reg.get(&unit, "enabled").unwrap().set(1.0);
                reg.get(&unit, "mix").unwrap().set(0.7);
                for s in 1..=3 {
                    let slot = format!("[EffectRack1_EffectUnit{u}_Effect{s}]");
                    reg.get(&slot, "enabled").unwrap().set(1.0);
                }
            }
            (reg, rack)
        };
        let (reg_a, mut a) = setup();
        let (reg_b, mut b) = setup();
        let beats = [0.5; TARGETS];
        for n in 0..60 {
            // Something changes now and then: the mix, and a release.
            for reg in [&reg_a, &reg_b] {
                let unit = |k: &str| reg.get("[EffectRack1_EffectUnit1]", k).unwrap();
                unit("mix").set(0.3 + 0.01 * n as f64);
                reg.get("[EffectRack1_EffectUnit5]", "release").unwrap().set(if n == 40 { 1.0 } else { 0.0 });
            }
            let now = n as f64 * 0.01;
            a.prepare(beats, now);
            b.prepare(beats, now);
            let frames = 256 + n % 7;
            let input: Vec<[f32; 2]> = (0..frames)
                .map(|i| {
                    let v = tone(n * 300 + i);
                    [v, -0.5 * v]
                })
                .collect();
            let frame_by_frame: Vec<[f32; 2]> =
                input.iter().enumerate().map(|(f, &x)| a.process(0, x, f, frames)).collect();
            let mut block = input.clone();
            b.process_block(0, &mut block);
            assert_eq!(frame_by_frame, block, "buffer {n}");
            a.finish(frames);
            b.finish(frames);
        }
    }

    #[test]
    fn brake_comes_to_a_stop() {
        // Default length: 2 beats, 1 s at 120 BPM.
        let out = run(Kind::Brake, tone, 72_000);
        assert!(rms(&out[..4_800]) > 0.3);
        assert_eq!(rms(&out[60_000..]), 0.0);
    }

    #[test]
    fn roll_repeats_its_first_slice() {
        // A click at the start, then silence: rolled, the click comes back.
        let out = run(
            Kind::Roll,
            |i| {
                if i < 10 {
                    0.0
                } else if i < 400 {
                    0.8
                } else {
                    0.0
                }
            },
            48_000,
        );
        let late = out[24_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(late > 0.5, "{late}");
    }

    #[test]
    fn echo_out_fades_the_original_and_keeps_the_tail() {
        let out = run(Kind::EchoOut, |i| if i < 24_000 { tone(i) } else { 0.0 }, 96_000);
        // Still echoing after the input stopped.
        assert!(rms(&out[30_000..40_000]) > 0.02);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() < 1.5));
    }

    #[test]
    fn gate_chops_in_time() {
        let out = run(Kind::Gate, |_| 0.5, 48_000);
        let lo = out.iter().fold(1.0f32, |m, v| m.min(v.abs()));
        let hi = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(lo < 0.15 && hi > 0.45, "{lo} {hi}");
    }

    #[test]
    fn reverb_tail_decays_and_stays_finite() {
        let out = run(Kind::Reverb, |i| if i < 480 { tone(i) } else { 0.0 }, 5 * 48_000);
        assert!(rms(&out[4_800..9_600]) > 0.001, "a tail after the input");
        assert!(rms(&out[4 * 48_000..]) < rms(&out[4_800..9_600]), "that decays");
        assert!(out.iter().all(|v| v.is_finite()));
    }
}
