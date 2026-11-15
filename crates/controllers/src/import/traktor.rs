//! Traktor controller mappings (`.tsi`). The mapping is an XML entry,
//! `DeviceIO.Config.Controller`, whose value is base64 of a tree of
//! big-endian frames: a four-letter tag, a u32 size, then the body. Each
//! device (`DEVI`) has its MIDI ports, the MIDI messages it uses (`DCBM`)
//! and the assignments (`CMAI`): a Traktor command, whether it's an input
//! or an LED output, and its settings (`CMAD`): interaction, deck,
//! modifier conditions, the value a button sets and the LED range.
//! Traktor 3.3 and 4 files read the same; older files stop the settings
//! earlier, which the reader allows.

use std::collections::HashMap;

use base64::Engine;

use super::words::{
    beats, deck, eq, eq_kill, filter, fx_slot, fx_unit, indicator, library, master,
};
use super::Import;
use crate::format::{Light, LightSource, Midi, DjSugarMapping, Row, Variable};
use crate::mapping::When;

/// Reads frames out of a body.
struct Frames<'a> {
    b: &'a [u8],
}

impl<'a> Iterator for Frames<'a> {
    type Item = (&'a [u8], &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        if self.b.len() < 8 {
            return None;
        }
        let tag = &self.b[..4];
        let size = u32::from_be_bytes(self.b[4..8].try_into().ok()?) as usize;
        let body = self.b.get(8..8 + size)?;
        self.b = &self.b[8 + size..];
        Some((tag, body))
    }
}

fn frames(b: &[u8]) -> Frames<'_> {
    Frames { b }
}

fn child<'a>(b: &'a [u8], tag: &[u8]) -> Option<&'a [u8]> {
    frames(b).find(|(t, _)| *t == tag).map(|(_, body)| body)
}

/// A cursor over a frame body.
struct Cursor<'a> {
    b: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn u32(&mut self) -> Option<u32> {
        let v = u32::from_be_bytes(self.b.get(..4)?.try_into().ok()?);
        self.b = &self.b[4..];
        Some(v)
    }
    fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }
    fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }
    /// A u32 length in characters, then UTF-16BE.
    fn text(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        let bytes = self.b.get(..n * 2)?;
        self.b = &self.b[n * 2..];
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        Some(String::from_utf16_lossy(&units))
    }
}

/// A value Traktor stores as four bytes: a float for faders and knobs, a
/// whole number for lists such as a hot cue or a loop size.
#[derive(Clone, Copy, Debug, Default)]
struct Value(u32);

impl Value {
    fn int(self) -> i32 {
        self.0 as i32
    }
    fn number(self) -> f64 {
        let i = self.0 as i32;
        if (-1000..=1000).contains(&i) {
            i as f64
        } else {
            f32::from_bits(self.0) as f64
        }
    }
}

const IN: u32 = 0;

/// Interaction modes.
const TOGGLE: u32 = 1;
const HOLD: u32 = 2;
const DIRECT: u32 = 3;
const RELATIVE: u32 = 4;
const INC: u32 = 5;
const DEC: u32 = 6;
const RESET: u32 = 7;

/// Control types.
const FADER: u32 = 1;
const ENCODER: u32 = 2;

/// Modifier #1 to #8.
const MODIFIER: std::ops::RangeInclusive<u32> = 2548..=2555;

/// One assignment of a Traktor device.
#[derive(Clone, Debug, Default)]
struct Assignment {
    binding: u32,
    output: bool,
    command: u32,
    control_type: u32,
    interaction: u32,
    /// -1 follows the focused deck; 0 to 3 are decks A to D or FX units 1
    /// to 4.
    target: i32,
    invert: bool,
    soft_takeover: bool,
    set_to: Value,
    comment: String,
    conditions: Vec<(u32, i32)>,
    led_min: Value,
    led_max: Value,
    led_min_midi: i32,
    led_max_midi: i32,
}

fn assignment(b: &[u8]) -> Option<Assignment> {
    let mut c = Cursor { b };
    let mut a = Assignment {
        binding: c.u32()?,
        output: c.u32()? != IN,
        command: c.u32()?,
        ..Default::default()
    };
    let settings = child(c.b, b"CMAD")?;
    let mut s = Cursor { b: settings };
    // 1 and 2 in Traktor 1 files; 3 and 4 later.
    let version = s.u32()?;
    a.control_type = s.u32()?;
    a.interaction = s.u32()?;
    a.target = s.i32()?;
    let _auto_repeat = s.u32()?;
    a.invert = s.u32()? != 0;
    a.soft_takeover = s.u32()? != 0;
    let _sensitivity = s.f32()?;
    let _acceleration = s.f32()?;
    let _has_value_ui = s.u32()?;
    let _value_ui = s.u32()?;
    a.set_to = Value(s.u32()?);
    a.comment = s.text()?;
    for _ in 0..2 {
        if version < 3 {
            // Older files name a modifier by its number from 0, or -1.
            let index = s.i32()?;
            let value = s.i32()?;
            if (0..8).contains(&index) {
                a.conditions.push((MODIFIER.start() + index as u32, value));
            }
        } else {
            let id = s.u32()?;
            let _target = s.i32()?;
            let value = s.i32()?;
            if id != 0 && id != u32::MAX {
                a.conditions.push((id, value));
            }
        }
    }
    // LED settings; files from the oldest Traktors can stop here.
    let led = (|| {
        let _ = s.u32()?;
        let min = Value(s.u32()?);
        let _ = s.u32()?;
        let max = Value(s.u32()?);
        Some((min, max, s.i32()?, s.i32()?))
    })();
    if let Some((min, max, lo, hi)) = led {
        a.led_min = min;
        a.led_max = max;
        a.led_min_midi = lo;
        a.led_max_midi = hi;
    } else {
        a.led_max = Value(1.0f32.to_bits());
        a.led_max_midi = 127;
    }
    Some(a)
}

/// `Ch01.Note.G#9`, `Ch16.CC.032`, `Ch01.PitchBend`, or two CCs joined
/// with `+` for a 14-bit fader. Note names count from C-1 = 0.
fn midi_name(name: &str) -> Option<Midi> {
    let one = |part: &str| -> Option<(u8, u8)> {
        let mut bits = part.splitn(3, '.');
        let ch: u8 = bits.next()?.strip_prefix("Ch")?.parse().ok()?;
        let ch = ch.checked_sub(1).filter(|c| *c < 16)?;
        match (bits.next()?, bits.next()) {
            ("Note", Some(n)) => {
                let (letter, rest) = n.split_at(1);
                let base = match letter {
                    "C" => 0,
                    "D" => 2,
                    "E" => 4,
                    "F" => 5,
                    "G" => 7,
                    "A" => 9,
                    "B" => 11,
                    _ => return None,
                };
                let (sharp, octave) = match rest.strip_prefix('#') {
                    Some(o) => (1, o),
                    None => (0, rest),
                };
                let octave: i32 = octave.parse().ok()?;
                let number = (octave + 1) * 12 + base + sharp;
                (0..128)
                    .contains(&number)
                    .then_some((0x90 | ch, number as u8))
            }
            ("CC", Some(n)) => {
                let n: u8 = n.parse().ok()?;
                (n < 128).then_some((0xB0 | ch, n))
            }
            ("PitchBend", None) => Some((0xE0 | ch, 0)),
            _ => None,
        }
    };
    let (first, second) = match name.split_once('+') {
        Some((a, b)) => (a, Some(b)),
        None => (name, None),
    };
    let (status, number) = one(first)?;
    let lsb = match second {
        Some(b) => Some(one(b)?),
        None => None,
    };
    Some(Midi {
        status,
        number,
        lsb,
    })
}

/// What a Traktor command does in DJ Sugar.
enum Act {
    Bind(String, Vec<&'static str>),
    Script(String),
    /// Kept greyed with why.
    No(&'static str),
}

/// Beats of Traktor's loop size list: 0 is 1/32, 5 is 1, 10 is 32.
fn loop_size(v: i32) -> Option<f64> {
    (0..=10).contains(&v).then(|| 2f64.powi(v - 5))
}

/// Beats of Traktor's move sizes: 7 is +1 beat, -7 is -1, from 1/16 to 32.
fn move_size(v: i32) -> Option<(f64, bool)> {
    let n = v.abs();
    (3..=12).contains(&n).then(|| (2f64.powi(n - 7), v > 0))
}

/// The input side of a command on deck (or FX unit) `d`.
fn translate(a: &Assignment, d: u8, encoder: &'static str) -> Act {
    let i = a.interaction;
    let button = if a.control_type == FADER || a.control_type == ENCODER {
        vec![]
    } else {
        vec!["button"]
    };
    let bind = |c: String| Act::Bind(c, button.clone());
    let fader = |c: String| match i {
        DIRECT => Act::Bind(c, vec![]),
        RELATIVE => Act::Bind(c, vec![encoder]),
        RESET => Act::Script(format!("set '{c}' 0.5")),
        _ => Act::No("DJ Sugar only moves this from a fader, knob or endless knob"),
    };
    let unit = d;
    match a.command {
        100 => bind(deck(d, "play")),
        206 => bind(deck(d, "cue_default")),
        204 => bind(deck(d, "cue_gotoandplay")),
        125 => bind(deck(d, "sync_enabled")),
        2293 => bind(deck(d, "sync_leader")),
        400 | 405 => bind(deck(d, "keylock")),
        3076 => bind(deck(d, "LoadSelectedTrack")),
        2350 => bind(deck(d, "slip_enabled")),
        103 => fader(deck(d, "playposition")),
        123 => match i {
            INC => bind(deck(d, "rate_perm_up_small")),
            DEC => bind(deck(d, "rate_perm_down_small")),
            RESET => bind(deck(d, "rate_set_zero")),
            _ => fader(deck(d, "rate")),
        },
        402 => match i {
            INC => bind(deck(d, "pitch_up")),
            DEC => bind(deck(d, "pitch_down")),
            RESET => bind(deck(d, "reset_key")),
            DIRECT if button.is_empty() => Act::Bind(deck(d, "pitch_adjust"), vec![]),
            _ => Act::No("DJ Sugar can't set the key to this value from a button yet"),
        },
        202 => bind(deck(d, "reloop_toggle")),
        2392 => bind(deck(d, "loop_in")),
        2393 => bind(deck(d, "loop_out")),
        2192 => bind(deck(d, "beatloop_activate")),
        2196 => match i {
            INC => bind(deck(d, "loop_double")),
            DEC => bind(deck(d, "loop_halve")),
            RELATIVE => Act::Script(format!("deck {d} (value > 50% ? loop_half : loop_double)")),
            DIRECT if !button.is_empty() => match loop_size(a.set_to.int()) {
                Some(n) => Act::Script(format!("set '[Channel{d}],beatloop_size' {}", beats(n))),
                None => Act::No("Traktor loop size DJ Sugar doesn't have"),
            },
            _ => Act::No("DJ Sugar picks loop sizes with buttons or an endless knob"),
        },
        2317 => match loop_size(a.set_to.int()) {
            Some(n) => bind(deck(d, &format!("beatloop_{}_toggle", beats(n)))),
            None => Act::No("Traktor loop size DJ Sugar doesn't have"),
        },
        2328 => match a.set_to.int() {
            n @ 0..=7 => bind(deck(d, &format!("hotcue_{}_activate", n + 1))),
            _ => Act::No("Traktor hot cue DJ Sugar doesn't have"),
        },
        2331 => match a.set_to.int() {
            n @ 0..=7 => bind(deck(d, &format!("hotcue_{}_clear", n + 1))),
            _ => Act::No("Traktor hot cue DJ Sugar doesn't have"),
        },
        2380 | 2351 => match (i, move_size(a.set_to.int())) {
            (INC, _) => bind(deck(d, "beatjump_forward")),
            (DEC, _) => bind(deck(d, "beatjump_backward")),
            (RELATIVE, _) => Act::Script(format!(
                "deck {d} (value > 50% ? press 'beatjump_backward' : press 'beatjump_forward')"
            )),
            (_, Some((n, forward))) => bind(deck(
                d,
                &format!(
                    "beatjump_{}_{}",
                    beats(n),
                    if forward { "forward" } else { "backward" }
                ),
            )),
            _ => Act::No("Traktor moves by this size or by a loop, which DJ Sugar doesn't do yet"),
        },
        // Traktor's move size is DJ Sugar's beat jump size; its mode (beat
        // jump or loop move) stays beat jump.
        2372 => match (i, move_size(a.set_to.int())) {
            (_, Some((n, _))) if !button.is_empty() => {
                Act::Script(format!("set '[Channel{d}],beatjump_size' {}", beats(n)))
            }
            _ => Act::No("DJ Sugar sets the beat jump size from buttons"),
        },
        2391 => Act::No("DJ Sugar's move buttons always beat jump; moving the loop isn't mappable yet"),
        2311 | 2313 => bind(deck(d, "quantize")),
        // Mixer.
        102 => fader(deck(d, "volume")),
        117 => fader(deck(d, "pregain")),
        119 => bind(deck(d, "pfl")),
        303 => fader(eq(d, 3)),
        302 => fader(eq(d, 2)),
        301 => fader(eq(d, 1)),
        306 => bind(eq_kill(d, 3)),
        305 => bind(eq_kill(d, 2)),
        304 => bind(eq_kill(d, 1)),
        320 => fader(filter(d)),
        319 => bind(format!("[QuickEffectRack1_[Channel{d}]],enabled")),
        321..=324 => bind(fx_unit(
            (a.command - 320) as u8,
            &format!("group_[Channel{d}]_enable"),
        )),
        5 => fader(master("crossfader")),
        6 => fader(master("gain")),
        8 => fader(master("headGain")),
        17 => fader(master("headMix")),
        2408 => Act::Script(format!("set '[Channel{d}],orientation' 0")),
        2409 => Act::Script(format!("set '[Channel{d}],orientation' 2")),
        // FX units.
        369 => bind(fx_unit(unit, "enabled")),
        365 => fader(fx_unit(unit, "mix")),
        366..=368 => fader(fx_slot(unit, (a.command - 365) as u8, "meta")),
        370..=372 => bind(fx_slot(unit, (a.command - 369) as u8, "enabled")),
        362..=364 => {
            let slot = (a.command - 361) as u8;
            match i {
                INC => bind(fx_slot(unit, slot, "next_effect")),
                DEC => bind(fx_slot(unit, slot, "prev_effect")),
                RELATIVE => Act::Bind(fx_slot(unit, slot, "effect_selector"), vec![encoder]),
                _ => Act::No("DJ Sugar steps through effects rather than picking one by number"),
            }
        }
        // Browser.
        3200 => match i {
            INC => bind(library("MoveDown")),
            DEC => bind(library("MoveUp")),
            RELATIVE => Act::Bind(library("MoveVertical"), vec!["selectknob"]),
            _ => Act::No("DJ Sugar moves through the list with buttons or an endless knob"),
        },
        3328 => match i {
            RELATIVE => Act::Bind(library("MoveFocus"), vec!["selectknob"]),
            _ => Act::No("DJ Sugar has no folder tree to move through from a controller yet"),
        },
        120 | 2187 => Act::No("Jog wheels need the controller's own script; DJ Sugar's mappings for the FLX4 and others have one"),
        id if (233..=260).contains(&id) || (2000..=2010).contains(&id) => {
            Act::No("DJ Sugar has no Remix Decks")
        }
        _ => Act::No("DJ Sugar has no match for this Traktor command yet"),
    }
}

/// The light side of a command: the control whose value it shows.
fn translate_light(a: &Assignment, d: u8) -> Result<LightSource, &'static str> {
    let on = a.led_max_midi.clamp(0, 127) as u8;
    let off = a.led_min_midi.clamp(0, 127) as u8;
    let bind = |control: String| {
        Ok(LightSource::Bind {
            control,
            min: 0.5,
            max: f64::MAX,
            on,
            off,
        })
    };
    let shown = |c: String| bind(indicator(&c));
    match a.command {
        100 => shown(deck(d, "play")),
        206 => shown(deck(d, "cue_default")),
        125 => shown(deck(d, "sync_enabled")),
        2293 => shown(deck(d, "sync_leader")),
        400 | 405 => shown(deck(d, "keylock")),
        119 => shown(deck(d, "pfl")),
        202 | 203 => shown(deck(d, "loop_enabled")),
        2350 => shown(deck(d, "slip_enabled")),
        2311 | 2313 => shown(deck(d, "quantize")),
        319 => shown(format!("[QuickEffectRack1_[Channel{d}]],enabled")),
        321..=324 => shown(fx_unit(
            (a.command - 320) as u8,
            &format!("group_[Channel{d}]_enable"),
        )),
        369 => shown(fx_unit(d, "enabled")),
        370..=372 => shown(fx_slot(d, (a.command - 369) as u8, "enabled")),
        2333..=2340 => shown(deck(d, &format!("hotcue_{}_enabled", a.command - 2332))),
        2328 => match a.set_to.int() {
            n @ 0..=7 => shown(deck(d, &format!("hotcue_{}_enabled", n + 1))),
            _ => Err("Traktor hot cue DJ Sugar doesn't have"),
        },
        2317 => match loop_size(a.set_to.int()) {
            Some(n) => shown(deck(d, &format!("beatloop_{}_enabled", beats(n)))),
            None => Err("Traktor loop size DJ Sugar doesn't have"),
        },
        2712 | 2713 => Ok(LightSource::Bind {
            control: deck(d, "vu_meter"),
            min: a.led_min.number().max(0.0),
            max: f64::MAX,
            on,
            off,
        }),
        _ => Err("DJ Sugar has no match for this Traktor light yet"),
    }
}

/// `m1` for Modifier #1.
fn modifier(command: u32) -> Option<String> {
    MODIFIER
        .contains(&command)
        .then(|| format!("m{}", command - MODIFIER.start() + 1))
}

/// Rows for a modifier button: what it sets, and on release what it
/// puts back when it's held.
fn modifier_rows(a: &Assignment, var: &str, msg: Midi) -> Vec<Row> {
    let v = a.set_to.int();
    match a.interaction {
        HOLD => {
            let mut up = Row::script(msg, &format!("set_var '{var}' 0"));
            up.when = When::Release;
            vec![Row::script(msg, &format!("set_var '{var}' {v}")), up]
        }
        TOGGLE => vec![Row::script(
            msg,
            &format!("(var '{var}' {v} ? set_var '{var}' 0 : set_var '{var}' {v})"),
        )],
        DIRECT => vec![Row::script(msg, &format!("set_var '{var}' {v}"))],
        _ => vec![Row::unsupported(
            msg,
            "",
            "",
            "DJ Sugar sets modifiers by holding, toggling or setting them, not by stepping",
        )],
    }
}

/// Every Traktor command id with its internal name.
const COMMANDS: &str = include_str!("traktor-commands.txt");

/// Traktor's command names, for row names in the editor.
fn command_name(id: u32) -> String {
    let name = match id {
        100 => "Play/Pause",
        206 => "Cue",
        204 => "Cup (Cue Play)",
        125 => "Sync On",
        2293 => "Set as Tempo Master",
        400 => "Keylock On",
        405 => "Keylock On (Preserve Pitch)",
        3076 => "Load Selected",
        2350 => "Flux Mode On",
        103 => "Seek Position",
        123 => "Tempo Adjust",
        402 => "Key Adjust",
        202 => "Loop Active On",
        203 => "Is In Active Loop",
        2392 => "Loop In/Set Cue",
        2393 => "Loop Out",
        2192 => "Loop Set",
        2196 => "Loop Size Selector",
        2317 => "Loop Size Selector + Set",
        2318 => "Backward Loop Size Selector + Set",
        2328 => "Select/Set+Store Hotcue",
        2331 => "Delete Hotcue",
        2333..=2340 => return format!("Hotcue {} Type", id - 2332),
        2351 => "Move",
        2372 => "Move Size Selector",
        2380 => "Beatjump",
        2391 => "Move Mode Selector",
        102 => "Volume Adjust",
        117 => "Gain Adjust",
        119 => "Monitor Cue On",
        303 => "EQ High Adjust",
        302 => "EQ Mid Adjust",
        301 => "EQ Low Adjust",
        306 => "EQ High Kill",
        305 => "EQ Mid Kill",
        304 => "EQ Low Kill",
        320 => "Filter Adjust",
        319 => "Filter On",
        321..=324 => return format!("FX Unit {} On", id - 320),
        5 => "X-Fader Position",
        6 => "Master Volume Adjust",
        8 => "Monitor Volume Adjust",
        17 => "Monitor Mix Adjust",
        2408 => "X-Fader Assign Left",
        2311 => "Snap On",
        2313 => "Quantize On",
        2409 => "X-Fader Assign Right",
        369 => "FX Unit On",
        365 => "FX Dry/Wet Adjust",
        366..=368 => return format!("FX Knob {}", id - 365),
        370..=372 => return format!("FX Button {}", id - 369),
        362..=364 => return format!("FX Effect {} Selector", id - 361),
        3200 => "Browser List Select Up/Down",
        3328 => "Browser Tree Select Up/Down",
        120 => "Jog Turn",
        2187 => "Jog Touch On",
        2712 => "Deck Pre-Fader Level",
        2713 => "Deck Post-Fader Level",
        2548..=2555 => return format!("Modifier #{}", id - 2547),
        _ => {
            return COMMANDS
                .lines()
                .find_map(|l| {
                    let (n, i) = l.rsplit_once('=')?;
                    (i.trim().parse() == Ok(id)).then(|| {
                        // `DeckCommon_Loop_LoopOut` → `Loop LoopOut`.
                        let n = n.split_once('_').map_or(n, |(_, r)| r);
                        n.replace('_', " ")
                    })
                })
                .unwrap_or_else(|| format!("Traktor command {id}"));
        }
    };
    name.to_string()
}

/// One Traktor device, as it reads from the file.
struct Device {
    controller: String,
    port_in: String,
    port_out: String,
    assignments: Vec<Assignment>,
    bindings: HashMap<u32, String>,
    /// MIDI name → 7Fh/01h (true) or 3Fh/41h (false) for endless knobs.
    encoders: HashMap<String, bool>,
}

fn device(b: &[u8]) -> Option<Device> {
    let mut c = Cursor { b };
    let _name = c.text()?;
    let data = child(c.b, b"DDAT")?;
    let mut dev = Device {
        controller: String::new(),
        port_in: String::new(),
        port_out: String::new(),
        assignments: Vec::new(),
        bindings: HashMap::new(),
        encoders: HashMap::new(),
    };
    if let Some(ddic) = child(data, b"DDIC") {
        dev.controller = Cursor { b: ddic }.text().unwrap_or_default();
    }
    if let Some(ddpt) = child(data, b"DDPT") {
        let mut p = Cursor { b: ddpt };
        dev.port_in = p.text().unwrap_or_default();
        dev.port_out = p.text().unwrap_or_default();
    }
    if let Some(defs) = child(data, b"DDDC") {
        for list in [b"DDCI", b"DDCO"] {
            let Some(l) = child(defs, list) else { continue };
            for (tag, body) in frames(l.get(4..).unwrap_or(&[])) {
                if tag != b"DCDT" {
                    continue;
                }
                let mut d = Cursor { b: body };
                let (Some(name), Some(_kind), Some(_min), Some(_max), Some(mode)) =
                    (d.text(), d.u32(), d.f32(), d.f32(), d.u32())
                else {
                    continue;
                };
                dev.encoders.insert(name, mode == 1);
            }
        }
    }
    let cb = child(data, b"DDCB")?;
    if let Some(cmas) = child(cb, b"CMAS") {
        for (tag, body) in frames(cmas.get(4..).unwrap_or(&[])) {
            if tag == b"CMAI" {
                if let Some(a) = assignment(body) {
                    dev.assignments.push(a);
                }
            }
        }
    }
    if let Some(dcbm) = child(cb, b"DCBM") {
        for (tag, body) in frames(dcbm.get(4..).unwrap_or(&[])) {
            if tag != b"DCBM" {
                continue;
            }
            let mut d = Cursor { b: body };
            if let (Some(id), Some(name)) = (d.u32(), d.text()) {
                dev.bindings.insert(id, name);
            }
        }
    }
    Some(dev)
}

/// The base64 value of the controller entry.
fn config(text: &str) -> Result<Vec<u8>, String> {
    let at = text
        .find("Name=\"DeviceIO.Config.Controller\"")
        .ok_or("This Traktor file has no controller mapping in it")?;
    let rest = &text[at..];
    let start = rest.find("Value=\"").ok_or("Damaged Traktor file")? + 7;
    let end = rest[start..].find('"').ok_or("Damaged Traktor file")?;
    let value: String = rest[start..start + end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| "Damaged Traktor file: the mapping isn't readable".to_string())
}

/// `DDJ-FLX4 - Deck A (Play)` → `DDJ-FLX4`.
fn controller_name(dev: &Device) -> String {
    let c = dev.controller.split(" - ").next().unwrap_or("").trim();
    if !c.is_empty() {
        return c.to_string();
    }
    if !dev.port_in.is_empty() && dev.port_in != "All Ports" && dev.port_in != "None" {
        return dev.port_in.trim_end_matches(" In").to_string();
    }
    String::new()
}

pub(super) fn import(text: &str, file: &str) -> Result<Import, String> {
    let data = config(text)?;
    let root = child(&data, b"DIOM").ok_or("Damaged Traktor file")?;
    let devs = child(root, b"DEVS").ok_or("Damaged Traktor file")?;
    let devices: Vec<Device> = frames(devs.get(4..).unwrap_or(&[]))
        .filter(|(t, _)| *t == b"DEVI")
        .filter_map(|(_, b)| device(b))
        .collect();
    if devices.is_empty() {
        return Err("This Traktor file has no controllers in it".into());
    }
    // Traktor names its own files `01 - Controller - Allen & Heath - Xone 1D`.
    let title = file.strip_suffix(".tsi").unwrap_or(file);
    let title = title
        .rsplit(" - ")
        .next()
        .unwrap_or(title)
        .trim()
        .to_string();
    let mut import = Import::default();
    // Traktor 4 splits a controller into several devices on the same
    // port (deck A, pads, mixer); DJ Sugar keeps one mapping per port.
    let mut by_port: Vec<(String, Vec<&Device>)> = Vec::new();
    for d in &devices {
        match by_port.iter_mut().find(|(p, _)| *p == d.port_in) {
            Some((_, list)) => list.push(d),
            None => by_port.push((d.port_in.clone(), vec![d])),
        }
    }
    for (port, devs) in by_port {
        let controller = devs
            .iter()
            .map(|d| controller_name(d))
            .find(|n| !n.is_empty())
            .unwrap_or_else(|| title.clone());
        let mut m = DjSugarMapping::new(&format!("{controller} (Traktor)"));
        m.controller.name = controller.clone();
        if !port.is_empty() && port != "All Ports" && port != "None" {
            m.controller.ports.push(port.clone());
        }
        let mut variables: Vec<String> = Vec::new();
        let mut own_hardware = 0;
        let mut follows_focus = false;
        for dev in devs {
            for a in &dev.assignments {
                let Some(midi_text) = dev.bindings.get(&a.binding) else {
                    continue;
                };
                let Some(msg) = midi_name(midi_text) else {
                    // Native Instruments' own controllers name their
                    // controls (`Left.Play`) instead of MIDI messages.
                    own_hardware += 1;
                    continue;
                };
                if a.target < 0 {
                    follows_focus = true;
                }
                let d = (a.target.clamp(0, 3) + 1) as u8;
                let name = command_name(a.command);
                // Modifier conditions become the row's layer.
                let mut layer = Vec::new();
                let mut odd_condition = None;
                for (id, v) in &a.conditions {
                    match modifier(*id) {
                        Some(var) => {
                            layer.push(format!("{var}={v}"));
                            if !variables.contains(&var) {
                                variables.push(var);
                            }
                        }
                        None => odd_condition = Some(command_name(*id)),
                    }
                }
                let original = format!(
                    "{name}{}{}",
                    if a.target >= 0 {
                        format!(" ({})", ["A", "B", "C", "D"][a.target.clamp(0, 3) as usize])
                    } else {
                        String::new()
                    },
                    if a.comment.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", a.comment)
                    }
                );
                if a.output {
                    let source = match modifier(a.command) {
                        Some(var) => {
                            let lo = a.led_min.int();
                            let hi = a.led_max.int();
                            Ok(LightSource::Script(if lo == hi {
                                format!(
                                    "(var '{var}' {hi} ? led {} : led {})",
                                    a.led_max_midi.clamp(0, 127),
                                    a.led_min_midi.clamp(0, 127)
                                )
                            } else {
                                format!(
                                    "(var '{var}' > {} ? led {} : led {})",
                                    lo.max(hi) - 1,
                                    a.led_max_midi.clamp(0, 127),
                                    a.led_min_midi.clamp(0, 127)
                                )
                            }))
                        }
                        None => translate_light(a, d),
                    };
                    let mut light = match source {
                        Ok(s) => Light::new(msg, s),
                        Err(note) => {
                            let mut l = Light::new(msg, LightSource::Nothing);
                            l.off = true;
                            l.note = note.to_string();
                            l
                        }
                    };
                    light.name = name;
                    light.original = original;
                    if let Some(c) = &odd_condition {
                        light.off = true;
                        light.note = format!("Depends on Traktor's {c}, which DJ Sugar doesn't have");
                    }
                    m.lights.push(light);
                    continue;
                }
                let encoder = if dev.encoders.get(midi_text).copied().unwrap_or(true) {
                    "selectknob"
                } else {
                    "diff"
                };
                let mut rows = match modifier(a.command) {
                    Some(var) => {
                        if !variables.contains(&var) {
                            variables.push(var.clone());
                        }
                        modifier_rows(a, &var, msg)
                    }
                    None => match translate(a, d, encoder) {
                        Act::Bind(c, mut o) => {
                            if a.invert {
                                o.push("invert");
                            }
                            let mut r = Row::bind(msg, &c, &o);
                            if a.soft_takeover {
                                r.takeover = Some("pickup".into());
                            }
                            vec![r]
                        }
                        Act::Script(s) => vec![Row::script(msg, &s)],
                        Act::No(note) => vec![Row::unsupported(msg, "", "", note)],
                    },
                };
                for r in &mut rows {
                    r.name = name.clone();
                    if r.original.is_empty() {
                        r.original = original.clone();
                    }
                    r.layer = layer.join(" & ");
                    if let Some(c) = &odd_condition {
                        r.off = true;
                        r.note = format!("Depends on Traktor's {c}, which DJ Sugar doesn't have");
                        r.action = crate::format::Action::Nothing;
                    }
                    if !r.off {
                        r.original.clear();
                    }
                }
                m.rows.extend(rows);
            }
        }
        for name in variables {
            m.variables.push(Variable {
                name,
                default: 0.0,
                saved: false,
            });
        }
        if own_hardware > 0 {
            import.notes.push(format!(
                "{controller}: {own_hardware} assignments are for Native Instruments' own hardware, which talks to Traktor directly rather than over MIDI, so they can't come across."
            ));
        }
        if follows_focus {
            import.notes.push(format!(
                "{controller}: some assignments follow Traktor's focused deck; they're set to deck A here."
            ));
        }
        if m.rows.is_empty() && m.lights.is_empty() {
            continue;
        }
        import.mappings.push(m);
    }
    if import.mappings.is_empty() {
        return Err(import.notes.join(" ").chars().take(400).collect::<String>()
            + if import.notes.is_empty() {
                "This Traktor file has nothing DJ Sugar can map."
            } else {
                ""
            });
    }
    Ok(import)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_names() {
        assert_eq!(midi_name("Ch16.Note.G9"), Some(Midi::new(0x9F, 127)));
        assert_eq!(midi_name("Ch01.Note.C-1"), Some(Midi::new(0x90, 0)));
        assert_eq!(midi_name("Ch02.Note.C#3"), Some(Midi::new(0x91, 49)));
        assert_eq!(midi_name("Ch01.CC.032"), Some(Midi::new(0xB0, 32)));
        assert_eq!(midi_name("Ch01.PitchBend"), Some(Midi::new(0xE0, 0)));
        assert_eq!(
            midi_name("Ch01.CC.000+Ch01.CC.032").map(|m| m.lsb),
            Some(Some((0xB0, 32)))
        );
        assert_eq!(midi_name("Left.Play"), None);
    }

    #[test]
    fn sizes() {
        assert_eq!(loop_size(5), Some(1.0));
        assert_eq!(loop_size(0), Some(1.0 / 32.0));
        assert_eq!(move_size(-7), Some((1.0, false)));
        assert_eq!(move_size(9), Some((4.0, true)));
        assert_eq!(move_size(13), None);
    }

    /// Builds a `.tsi` the way Traktor writes one, with one device.
    fn frame(tag: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend((body.len() as u32).to_be_bytes());
        v.extend(body);
        v
    }
    fn text(s: &str) -> Vec<u8> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut v = (units.len() as u32).to_be_bytes().to_vec();
        for u in units {
            v.extend(u.to_be_bytes());
        }
        v
    }
    fn ints(xs: &[u32]) -> Vec<u8> {
        xs.iter().flat_map(|x| x.to_be_bytes()).collect()
    }
    #[allow(clippy::too_many_arguments)]
    fn cmai(
        binding: u32,
        out: u32,
        cmd: u32,
        ctype: u32,
        inter: u32,
        target: i32,
        set_to: u32,
        cond: (u32, u32),
    ) -> Vec<u8> {
        let mut s = ints(&[4, ctype, inter, target as u32, 0, 0, 0, 0, 0, 0, 0, set_to]);
        s.extend(text(""));
        s.extend(ints(&[cond.0, 0, cond.1, 0, 0, 0]));
        s.extend(ints(&[0, 0, 0, 1, 0, 127, 0, 0, 0, 0, 0]));
        let mut b = ints(&[binding, out, cmd]);
        b.extend(frame(b"CMAD", &s));
        frame(b"CMAI", &b)
    }

    fn sample() -> String {
        let maps = [
            cmai(1, 0, 100, 0, 1, 0, 0, (0, 0)),
            cmai(1, 1, 100, 65535, 8, 0, 0, (0, 0)),
            cmai(2, 0, 2548, 0, 2, -1, 1, (0, 0)),
            cmai(3, 0, 2328, 0, 3, 1, 2, (2548, 1)),
            cmai(4, 0, 102, 1, 3, 1, 0, (0, 0)),
            cmai(5, 0, 120, 2, 4, 0, 0, (0, 0)),
            cmai(6, 0, 100, 0, 1, 0, 0, (0, 0)),
        ];
        let mut cmas = ints(&[maps.len() as u32]);
        for m in &maps {
            cmas.extend(m);
        }
        let names = [
            "Ch01.Note.C3",
            "Ch01.Note.D3",
            "Ch02.Note.E3",
            "Ch02.CC.010",
            "Ch01.CC.020",
            "Left.Play",
        ];
        let mut dcbm = ints(&[names.len() as u32]);
        for (i, n) in names.iter().enumerate() {
            let mut b = ints(&[i as u32 + 1]);
            b.extend(text(n));
            dcbm.extend(frame(b"DCBM", &b));
        }
        let mut cb = frame(b"CMAS", &cmas);
        cb.extend(frame(b"DCBM", &dcbm));
        let mut ddat = frame(b"DDIC", &text("Xone:K2 - Decks"));
        let mut ports = text("XONE:K2");
        ports.extend(text("XONE:K2"));
        ddat.extend(frame(b"DDPT", &ports));
        ddat.extend(frame(b"DDCB", &cb));
        let mut devi = text("Generic MIDI");
        devi.extend(frame(b"DDAT", &ddat));
        let mut devs = ints(&[1]);
        devs.extend(frame(b"DEVI", &devi));
        let root = frame(b"DIOM", &frame(b"DEVS", &devs));
        let value = base64::engine::general_purpose::STANDARD.encode(root);
        format!("<?xml version=\"1.0\"?><NIXML><TraktorSettings><Entry Name=\"DeviceIO.Config.Controller\" Type=\"3\" Value=\"{value}\"></Entry></TraktorSettings></NIXML>")
    }

    #[test]
    fn reads_a_tsi() {
        let i = import(&sample(), "K2.tsi").unwrap();
        assert_eq!(i.mappings.len(), 1);
        let m = &i.mappings[0];
        assert_eq!(m.name, "Xone:K2 (Traktor)");
        assert_eq!(m.controller.ports, vec!["XONE:K2".to_string()]);
        let play = &m.rows[0];
        assert_eq!(play.midi, Midi::new(0x90, 48));
        assert!(
            matches!(&play.action, crate::format::Action::Bind { control, .. } if control == "[Channel1],play")
        );
        assert!(
            matches!(&m.lights[0].source, LightSource::Bind { control, on: 127, .. } if control == "[Channel1],play_indicator")
        );
        // Modifier #1 held: set on press, back to 0 on release.
        let shift: Vec<_> = m.rows.iter().filter(|r| r.name == "Modifier #1").collect();
        assert_eq!(shift.len(), 2);
        assert_eq!(shift[1].when, When::Release);
        let cue = m
            .rows
            .iter()
            .find(|r| r.name == "Select/Set+Store Hotcue")
            .unwrap();
        assert_eq!(cue.layer, "m1=1");
        assert!(
            matches!(&cue.action, crate::format::Action::Bind { control, .. } if control == "[Channel2],hotcue_3_activate")
        );
        let jog = m.rows.iter().find(|r| r.name == "Jog Turn").unwrap();
        assert!(jog.off && !jog.original.is_empty());
        assert_eq!(m.variables.len(), 1);
        assert!(i.notes.iter().any(|n| n.contains("own hardware")));
        assert!(i.notes.iter().any(|n| n.contains("focused deck")));
    }

    #[test]
    fn the_scripts_it_writes_parse() {
        let i = import(&sample(), "K2.tsi").unwrap();
        for r in &i.mappings[0].rows {
            if let crate::format::Action::Script(s) = &r.action {
                djsugar_script::parse(s).unwrap_or_else(|e| panic!("{s}: {e:?}"));
            }
        }
    }
}
