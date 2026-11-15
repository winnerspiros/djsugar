//! Finds out what a mapping does by trying it. Bundled mappings are mostly
//! JavaScript, so the XML alone can't say what a button does. The probe runs
//! the mapping against a sandbox copy of the engine's controls, presses each
//! control once (and again with SHIFT held), and notes which controls change
//! or which the script tries to change. It then switches each of those
//! controls on to see which LEDs the mapping lights for it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use djsugar_control::{ConfigKey, ControlHandle, ControlRegistry};

use crate::controller::Controller;
use crate::mapping::{Mapping, Options};

/// A MIDI input as the editor lists it: the status byte with note off
/// counted as note on, and the note or controller number.
pub type MessageKey = (u8, u8);

/// `0x8n` (note off) as `0x9n`; everything else as is.
pub fn message_key(status: u8, number: u8) -> MessageKey {
    if status & 0xF0 == 0x80 {
        (0x90 | (status & 0x0F), number)
    } else {
        (status, number)
    }
}

/// A control an input changed and the value it set.
pub type Effect = (ConfigKey, f64);

/// What the probe found.
#[derive(Clone, Debug, Default)]
pub struct Probe {
    /// The controls each input changes.
    pub normal: HashMap<MessageKey, Vec<Effect>>,
    /// The same with the mapping's SHIFT buttons held, where different.
    pub shifted: HashMap<MessageKey, Vec<Effect>>,
    /// The inputs that are SHIFT buttons.
    pub shift_keys: HashSet<MessageKey>,
    /// The LED messages (status, number) each control lights.
    pub lights: Vec<(ConfigKey, Vec<(u8, u8)>)>,
    /// Script errors while probing, for diagnostics.
    pub errors: Vec<String>,
}

/// Whether a binding is a SHIFT button: `PioneerDDJFLX4.shiftPressed`,
/// `XoneK2.shiftButton`, `MidiFighterTwister.controller.toggleShift`,
/// `MyController.shift.input`.
pub fn is_shift_binding(key: &str) -> bool {
    let mut parts: Vec<&str> = key.split('.').collect();
    if parts.last().is_some_and(|p| *p == "input") {
        parts.pop();
    }
    let last = parts.last().copied().unwrap_or("").to_ascii_lowercase();
    let last = last.split('[').next().unwrap_or("");
    matches!(
        last,
        "shift" | "shiftbutton" | "shiftpressed" | "toggleshift" | "shift_button" | "onshift"
    ) || key.eq_ignore_ascii_case("shift")
}

struct Sandbox {
    controller: Controller,
    controls: Vec<ControlHandle>,
    now: Instant,
    /// Controls that change with nothing pressed (blinking, timers).
    noise: HashSet<ConfigKey>,
    /// The engine's controls, as opposed to ones scripts make up to keep
    /// their own state in.
    engine: HashSet<ConfigKey>,
}

impl Sandbox {
    fn versions(&self) -> Vec<u32> {
        self.controls.iter().map(|c| c.version()).collect()
    }

    /// Sends `messages` and returns the controls that changed or were
    /// written, putting changed values back afterwards.
    fn effect(&mut self, messages: &[[u8; 3]]) -> Vec<Effect> {
        let before = self.versions();
        let values: Vec<f64> = self.controls.iter().map(|c| c.get()).collect();
        self.controller.take_trace();
        for m in messages {
            self.controller.receive(m);
            self.controller.poll(self.now);
        }
        let mut touched = self.controller.take_trace();
        for (i, c) in self.controls.iter().enumerate() {
            if c.version() != before[i] {
                if !touched.iter().any(|(k, _)| k == c.key()) {
                    touched.push((c.key().clone(), c.get()));
                }
                c.set(values[i]);
            }
        }
        self.controller.poll(self.now);
        self.controller.take_output();
        touched.retain(|(k, _)| {
            !self.noise.contains(k)
                && (self.engine.contains(k)
                    || matches!(
                        k.group.as_str(),
                        "[Library]" | "[Playlist]" | "[Skin]" | "[AutoDJ]"
                    ))
        });
        touched
    }
}

/// What a press of `status, number` sends: a press and release for notes, a
/// small turn for 14-bit faders and pitch bend, full on and off for other
/// controller numbers.
fn stimulus(mapping: &Mapping, status: u8, number: u8) -> Vec<[u8; 3]> {
    let channel = status & 0x0F;
    match status & 0xF0 {
        0x90 => {
            let off = if mapping.inputs.contains_key(&(0x80 | channel, number)) {
                [0x80 | channel, number, 0]
            } else {
                [status, number, 0]
            };
            vec![[status, number, 0x7F], off]
        }
        0x80 => vec![[status, number, 0]],
        0xE0 => vec![[status, 0, 0x41]],
        0xB0 => {
            // A 14-bit pair needs both halves before anything moves.
            let inputs = &mapping.inputs[&(status, number)];
            let msb = inputs
                .iter()
                .any(|i| i.options.has(Options::FOURTEEN_BIT_MSB));
            let lsb = inputs
                .iter()
                .any(|i| i.options.has(Options::FOURTEEN_BIT_LSB));
            let partner = |flag: u32| {
                inputs.iter().find_map(|i| {
                    mapping.inputs.iter().find_map(|(&(s, n), other)| {
                        (s == status
                            && n != number
                            && other.iter().any(|o| {
                                o.key == i.key && o.group == i.group && o.options.has(flag)
                            }))
                        .then_some(n)
                    })
                })
            };
            if msb {
                match partner(Options::FOURTEEN_BIT_LSB) {
                    Some(l) => vec![[status, number, 0x41], [status, l, 0]],
                    None => vec![[status, number, 0x41]],
                }
            } else if lsb {
                match partner(Options::FOURTEEN_BIT_MSB) {
                    Some(m) => vec![[status, m, 0x41], [status, number, 0]],
                    None => vec![[status, number, 0x41]],
                }
            } else {
                // Full on then off: a button sends this, an absolute knob
                // jumps there, an encoder reads it as small turns.
                vec![[status, number, 0x7F], [status, number, 0]]
            }
        }
        _ => vec![[status, number, 0x41]],
    }
}

/// Probes `mapping` against `registry`, which should hold every engine
/// control (an `Engine` created on it, not running) and nothing else uses.
pub fn probe(mapping: &Mapping, registry: Arc<ControlRegistry>) -> Result<Probe, String> {
    let mut controller = Controller::new("probe", mapping.clone(), registry.clone())?;
    let now = Instant::now();
    controller.poll(now);
    controller.take_output();
    controller.start_trace();
    let controls: Vec<ControlHandle> = registry
        .keys()
        .iter()
        .filter_map(|k| registry.get(&k.group, &k.item))
        .collect();
    let engine = controls.iter().map(|c| c.key().clone()).collect();
    let mut sb = Sandbox {
        controller,
        controls,
        now,
        noise: HashSet::new(),
        engine,
    };
    sb.noise = sb.effect(&[]).into_iter().map(|(k, _)| k).collect();

    // Inputs in a fixed order, each message once (note off with note on).
    let mut messages: Vec<(u8, u8)> = mapping.inputs.keys().copied().collect();
    messages.sort();
    messages.retain(|&(s, n)| {
        !(s & 0xF0 == 0x80 && mapping.inputs.contains_key(&(0x90 | (s & 0x0F), n)))
    });
    let shift: Vec<(u8, u8)> = messages
        .iter()
        .copied()
        .filter(|k| mapping.inputs[k].iter().any(|i| is_shift_binding(&i.key)))
        .collect();

    let mut out = Probe {
        shift_keys: shift.iter().map(|&(s, n)| message_key(s, n)).collect(),
        ..Default::default()
    };
    for &(s, n) in &messages {
        let touched = sb.effect(&stimulus(mapping, s, n));
        out.normal.insert(message_key(s, n), touched);
    }
    if !shift.is_empty() {
        // Hold every SHIFT button, then press everything else again.
        for &(s, n) in &shift {
            let press = match s & 0xF0 {
                0x80 => [0x90 | (s & 0x0F), n, 0x7F],
                _ => [s, n, 0x7F],
            };
            sb.controller.receive(&press);
        }
        sb.controller.poll(now);
        sb.controller.take_output();
        sb.controller.take_trace();
        for &(s, n) in &messages {
            if shift.contains(&(s, n)) {
                continue;
            }
            let key = message_key(s, n);
            let touched = sb.effect(&stimulus(mapping, s, n));
            if out.normal.get(&key) != Some(&touched) {
                out.shifted.insert(key, touched);
            }
        }
        for &(s, n) in &shift {
            sb.controller
                .receive(&stimulus(mapping, s, n).last().copied().unwrap_or([s, n, 0]));
        }
        sb.controller.poll(now);
    }

    // Lights: switch each control on and see what the mapping sends.
    let mut candidates: Vec<ConfigKey> = Vec::new();
    for (k, _) in out.normal.values().chain(out.shifted.values()).flatten() {
        if !candidates.contains(k) {
            candidates.push(k.clone());
        }
    }
    for o in &mapping.outputs {
        let k = ConfigKey::new(o.group.clone(), o.key.clone());
        if !candidates.contains(&k) {
            candidates.push(k);
        }
    }
    sb.controller.poll(now);
    sb.controller.take_output();
    for k in candidates {
        let Some(c) = registry.get(&k.group, &k.item) else {
            continue;
        };
        let old = c.get();
        let (min, max) = c.range();
        // Free controls have no range: 1 is "on" for them.
        let max = if max > 1e6 { 1.0 } else { max };
        c.set(if old == max { min.max(-1e6) } else { max });
        sb.controller.poll(now);
        let mut lit: Vec<(u8, u8)> = sb
            .controller
            .take_output()
            .into_iter()
            .filter(|m| m.len() == 3 && matches!(m[0] & 0xF0, 0x90 | 0xB0 | 0x80))
            .map(|m| (m[0], m[1]))
            .collect();
        lit.sort();
        lit.dedup();
        c.set(old);
        sb.controller.poll(now);
        sb.controller.take_output();
        // A control that lights a dozen LEDs is a refresh of everything,
        // not that control's own light.
        if !lit.is_empty() && lit.len() <= 8 {
            out.lights.push((k, lit));
        }
    }
    out.errors = sb.controller.errors();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_shift_buttons() {
        assert!(is_shift_binding("PioneerDDJFLX4.shiftPressed"));
        assert!(is_shift_binding("XoneK2.shiftButton"));
        assert!(is_shift_binding(
            "MidiFighterTwister.controller.toggleShift"
        ));
        assert!(is_shift_binding("Foo.shift.input"));
        assert!(!is_shift_binding("PioneerDDJFLX4.beatFxSelectShiftPressed"));
        assert!(!is_shift_binding("PioneerDDJFLX4.keyboardPadShiftPressed"));
    }
}
