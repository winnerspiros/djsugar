//! DJ Sugar mappings' DJSugarScript rows and lights, as a controller runs them: layers made of the mapping's variables,
//! long, tap and double presses, fader and knob values handed to the line, and lights that ask a line what to show.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use djsugar_control::ControlRegistry;
use djsugar_script::{level_control, query_control, run_control, Call, Error, Host, Kind, Script};

use crate::mapping::{Input, Mapping, Options, Output, When, DOUBLE_PRESS, LONG_PRESS};

/// How often a script light is asked again.
const LIGHT_EVERY: Duration = Duration::from_millis(40);

/// What a light line ended in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shown {
    On,
    Off,
    Blink(u32),
    /// Blinks in a colour: the byte `color` would send.
    BlinkByte(u32, u8),
    Byte(u8),
}

struct ScriptLight {
    output: Output,
    script: Option<Script>,
    /// Shows a level (an LED ring, a meter) rather than on or off.
    level: bool,
    /// The note and value sent last.
    last: Option<(u8, u8)>,
}

pub(crate) struct ScriptRows {
    vars: HashMap<String, f64>,
    parsed: HashMap<String, Option<Script>>,
    held: HashMap<(u8, u8), Instant>,
    last_press: HashMap<(u8, u8), Instant>,
    /// Long presses waiting to happen: when, the message, the row.
    pending: Vec<(Instant, (u8, u8), Input)>,
    lights: Vec<ScriptLight>,
    lights_due: Instant,
    start: Instant,
    pub(crate) errors: Vec<String>,
    /// How many decks along the device's decks are moved (see
    /// [`crate::routing`]): a row's `deck 1` plays the deck it follows.
    pub(crate) shift: usize,
}

/// Runs a line for a controller: control verbs on the registry, variables
/// and lights here.
struct RowHost<'a> {
    registry: &'a ControlRegistry,
    vars: &'a mut HashMap<String, f64>,
    input: Option<f64>,
    /// `input` is an encoder's clicks.
    steps: bool,
    shown: Option<Shown>,
    colors: &'a [(String, u8)],
    /// See [`ScriptRows::shift`].
    shift: usize,
}

/// `call` with its deck moved `shift` decks along, wrapping after deck 4.
fn shifted<'c>(call: &Call<'c>, shift: usize) -> Call<'c> {
    let decks = crate::routing::DECKS;
    Call {
        verb: call.verb,
        deck: call.deck.map(|d| ((d as usize - 1 + shift) % decks + 1) as u8),
        args: call.args,
        input: call.input,
        steps: call.steps,
    }
}

impl RowHost<'_> {
    /// The byte `color 'name'` sends on this light, if it knows the colour.
    fn color(&self, name: &str) -> Option<u8> {
        let want = name.to_ascii_lowercase();
        self.colors.iter().find(|(c, _)| c.to_ascii_lowercase() == want).map(|(_, b)| *b)
    }
}

impl Host for RowHost<'_> {
    fn master_deck(&self) -> u8 {
        let master = (1..=4u8)
            .find(|d| self.registry.get(&format!("[Channel{d}]"), "sync_leader").is_some_and(|c| c.get() > 0.0))
            .unwrap_or(1);
        // Verbs move along by `shift` after this: the master stays put.
        let decks = crate::routing::DECKS;
        ((master as usize - 1 + decks - self.shift % decks) % decks + 1) as u8
    }

    fn run(&mut self, call: &Call) -> Result<(), Error> {
        let name = || call.args.first().map(|a| a.text().to_string()).unwrap_or_default();
        match call.verb.kind {
            Kind::SetVar => {
                let v = call.args.get(1).map(|a| a.number()).unwrap_or(0.0);
                self.vars.insert(name(), v);
            }
            Kind::ToggleVar => {
                let v = self.vars.entry(name()).or_insert(0.0);
                *v = if *v == 0.0 { 1.0 } else { 0.0 };
            }
            Kind::Light => {
                let arg = call.args.first().map(|a| a.number()).unwrap_or(0.0);
                self.shown = Some(match call.verb.name {
                    "on" => Shown::On,
                    "off" => Shown::Off,
                    "blink" => {
                        let ms = arg.max(50.0) as u32;
                        let colour = call.args.get(1).map(|a| a.text()).unwrap_or("");
                        match self.color(colour) {
                            Some(b) => Shown::BlinkByte(ms, b),
                            None => Shown::Blink(ms),
                        }
                    }
                    "led" => Shown::Byte(arg.clamp(0.0, 127.0) as u8),
                    _ => match self.color(&name()) {
                        Some(b) => Shown::Byte(b),
                        None => Shown::On,
                    },
                });
            }
            _ => run_control(self.registry, &shifted(call, self.shift))?,
        }
        Ok(())
    }

    fn query(&mut self, call: &Call) -> Result<bool, Error> {
        if call.verb.kind == Kind::Var {
            let name = call.args.first().map(|a| a.text()).unwrap_or("");
            let now = self.vars.get(name).copied().unwrap_or(0.0);
            return Ok(match call.args.get(1).map(|a| a.number()) {
                Some(v) if !v.is_nan() => now == v,
                _ => now > 0.0,
            });
        }
        query_control(self.registry, &shifted(call, self.shift))
    }

    fn level(&mut self, call: &Call) -> Result<f64, Error> {
        if call.verb.kind == Kind::Var {
            let name = call.args.first().map(|a| a.text()).unwrap_or("");
            return Ok(self.vars.get(name).copied().unwrap_or(0.0));
        }
        level_control(self.registry, &shifted(call, self.shift))
    }

    fn input(&self) -> Option<f64> {
        self.input
    }

    fn steps(&self) -> bool {
        self.steps
    }
}

impl ScriptRows {
    /// Takes the mapping's script lights out of `outputs`; the rest stay
    /// for the usual control-driven lights.
    pub(crate) fn new(mapping: &Mapping, outputs: &mut Vec<Output>) -> Self {
        let mut lights = Vec::new();
        outputs.retain(|o| match &o.script {
            Some(line) => {
                let script = djsugar_script::parse(line).ok();
                lights.push(ScriptLight {
                    output: o.clone(),
                    level: script.as_ref().is_some_and(|s| s.is_level()),
                    script,
                    last: None,
                });
                false
            }
            None => true,
        });
        let now = Instant::now();
        Self {
            vars: mapping.variables.iter().cloned().collect(),
            parsed: HashMap::new(),
            held: HashMap::new(),
            last_press: HashMap::new(),
            pending: Vec::new(),
            lights,
            lights_due: now,
            start: now,
            errors: Vec::new(),
            shift: 0,
        }
    }

    /// Whether a row's layer holds now.
    pub(crate) fn layer_holds(&self, layer: &[(String, Option<f64>)]) -> bool {
        layer.iter().all(|(name, want)| {
            let now = self.vars.get(name).copied().unwrap_or(0.0);
            match want {
                Some(v) => now == *v,
                None => now > 0.0,
            }
        })
    }

    /// The bindings of one message that act now. A binding on a layer that holds wins over the same message's bindings
    /// on fewer of its conditions, as a shifted button does only its shifted action: `shift` over no layer, `shift &
    /// m1=2` over `shift`.
    pub(crate) fn shown_through<'i>(&self, inputs: &'i [Input]) -> Vec<&'i Input> {
        let holding: Vec<&Input> = inputs.iter().filter(|i| self.layer_holds(&i.layer)).collect();
        holding
            .iter()
            .filter(|i| {
                !holding.iter().any(|o| {
                    o.when == i.when && o.layer.len() > i.layer.len() && i.layer.iter().all(|c| o.layer.contains(c))
                })
            })
            .copied()
            .collect()
    }

    fn run(&mut self, registry: &ControlRegistry, line: &str, input: Option<f64>, steps: bool) {
        let script = self.parsed.entry(line.to_string()).or_insert_with(|| djsugar_script::parse(line).ok()).clone();
        let Some(script) = script else {
            self.error(format!("can't read the row '{line}'"));
            return;
        };
        let mut host =
            RowHost { registry, vars: &mut self.vars, input, steps, shown: None, colors: &[], shift: self.shift };
        if let Err(e) = script.run(&mut host) {
            self.error(format!("'{line}': {e}"));
        }
    }

    fn error(&mut self, message: String) {
        if self.errors.len() < 50 && !self.errors.contains(&message) {
            self.errors.push(message);
        }
    }

    /// One message for a row with an DJSugarScript line. `value` is 0 to 1
    /// (a 14-bit pair already joined).
    pub(crate) fn handle(
        &mut self,
        registry: &ControlRegistry,
        input: &Input,
        status: u8,
        number: u8,
        value: f64,
        now: Instant,
    ) {
        let Some(line) = input.action.clone() else {
            return;
        };
        let key = (status & 0x0F | 0x90, number);
        let opcode = status & 0xF0;
        let note = opcode == 0x80 || opcode == 0x90;
        let continuous = !note && !input.options.has(Options::BUTTON) && input.when == When::Press;
        if continuous {
            let steps = input.options.has(Options::SELECT_KNOB);
            self.run(registry, &line, Some(value), steps);
            return;
        }
        let pressed = opcode != 0x80 && value > 0.0;
        if pressed {
            let first = !self.held.contains_key(&key);
            if first {
                self.held.insert(key, now);
            }
            match input.when {
                When::Press => self.run(registry, &line, Some(value), false),
                When::Double => {
                    if self.last_press.get(&key).is_some_and(|t| now.duration_since(*t) <= DOUBLE_PRESS) {
                        self.run(registry, &line, Some(value), false);
                    }
                }
                When::Long => self.pending.push((now + LONG_PRESS, key, input.clone())),
                When::Release | When::Tap => {}
            }
        } else {
            let since = self.held.get(&key).map(|t| now.duration_since(*t));
            match input.when {
                When::Release => self.run(registry, &line, Some(0.0), false),
                When::Tap if since.is_some_and(|d| d < LONG_PRESS) => self.run(registry, &line, Some(0.0), false),
                When::Long => self.pending.retain(|(_, k, i)| !(*k == key && i.action == input.action)),
                _ => {}
            }
        }
    }

    /// After every row of a message has seen it: remembers presses for
    /// double and tap presses.
    pub(crate) fn finish(&mut self, status: u8, number: u8, value: u8, now: Instant) {
        let opcode = status & 0xF0;
        let key = (status & 0x0F | 0x90, number);
        if (opcode == 0x80 || value == 0) && self.held.remove(&key).is_some() {
            self.last_press.insert(key, now);
        }
    }

    /// Runs long presses that are due and asks the lights; the messages to
    /// send go in `sent`.
    pub(crate) fn poll(&mut self, registry: &ControlRegistry, now: Instant, sent: &mut Vec<Vec<u8>>) {
        let due: Vec<(Instant, (u8, u8), Input)> = {
            let (due, wait) = self.pending.drain(..).partition(|(t, _, _)| *t <= now);
            self.pending = wait;
            due
        };
        for (_, key, input) in due {
            if self.held.contains_key(&key) {
                if let Some(line) = &input.action {
                    self.run(registry, line, Some(1.0), false);
                }
            }
        }
        if now < self.lights_due || self.lights.is_empty() {
            return;
        }
        self.lights_due = now + LIGHT_EVERY;
        let millis = now.duration_since(self.start).as_millis() as u64;
        for light in &mut self.lights {
            let Some(script) = &light.script else {
                continue;
            };
            let mut host = RowHost {
                registry,
                vars: &mut self.vars,
                input: None,
                steps: false,
                shown: None,
                colors: &light.output.colors,
                shift: self.shift,
            };
            let o = &light.output;
            let (number, byte) = if light.level {
                match script.level(&mut host) {
                    Ok(v) => (o.control, (v.clamp(0.0, 1.0) * 127.0).round() as u8),
                    Err(_) => continue,
                }
            } else {
                if script.run(&mut host).is_err() {
                    continue;
                }
                match host.shown.unwrap_or(Shown::Off) {
                    Shown::On => (o.control, o.on),
                    Shown::Off => (o.control, o.off),
                    Shown::Byte(b) if o.color_notes => (o.control.saturating_add(b).min(127), 0x7F),
                    Shown::Byte(b) => (o.control, b),
                    Shown::Blink(ms) => {
                        if (millis / ms as u64).is_multiple_of(2) {
                            (o.control, o.on)
                        } else {
                            (o.control, o.off)
                        }
                    }
                    Shown::BlinkByte(ms, b) => {
                        let lit = (millis / ms as u64).is_multiple_of(2);
                        match (lit, o.color_notes) {
                            (true, true) => (o.control.saturating_add(b).min(127), 0x7F),
                            (true, false) => (o.control, b),
                            (false, _) => (o.control, o.off),
                        }
                    }
                }
            };
            if light.last != Some((number, byte)) {
                light.last = Some((number, byte));
                sent.push(vec![o.status, number, byte]);
            }
        }
    }

    /// Sends every light again on the next poll.
    pub(crate) fn resend(&mut self) {
        for l in &mut self.lights {
            l.last = None;
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        let long = self.pending.iter().map(|(t, _, _)| *t).min();
        let light = (!self.lights.is_empty()).then_some(self.lights_due);
        match (long, light) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_control::Behavior;

    fn registry() -> std::sync::Arc<ControlRegistry> {
        let reg = ControlRegistry::new();
        for d in 1..=2 {
            let g = format!("[Channel{d}]");
            reg.toggle(&g, "play");
            reg.toggle(&g, "sync_enabled");
            reg.create(&g, "volume", 1.0, 0.0, 1.0, Behavior::Linear);
            reg.indicator(&g, "vu_meter", 1.0);
        }
        reg
    }

    fn row(line: &str, when: When, layer: &[(&str, Option<f64>)]) -> Input {
        Input {
            action: Some(line.into()),
            when,
            layer: layer.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
            ..Default::default()
        }
    }

    fn empty() -> Mapping {
        Mapping {
            name: String::new(),
            author: String::new(),
            path: Default::default(),
            scripts: Vec::new(),
            settings: Vec::new(),
            setting_defs: Vec::new(),
            inputs: Default::default(),
            outputs: Vec::new(),
            products: Vec::new(),
            variables: vec![("shift".into(), 0.0)],
        }
    }

    #[test]
    fn faders_hand_their_value_and_buttons_press() {
        let reg = registry();
        let mut rows = ScriptRows::new(&empty(), &mut Vec::new());
        let t = Instant::now();
        rows.handle(&reg, &row("deck 2 level", When::Press, &[]), 0xB0, 0x13, 0.5, t);
        assert_eq!(reg.get("[Channel2]", "volume").unwrap().get(), 0.5);
        rows.handle(&reg, &row("deck 1 play_pause", When::Press, &[]), 0x90, 0x0B, 1.0, t);
        rows.finish(0x90, 0x0B, 127, t);
        rows.handle(&reg, &row("deck 1 play_pause", When::Press, &[]), 0x80, 0x0B, 0.0, t);
        rows.finish(0x80, 0x0B, 0, t);
        assert_eq!(reg.get("[Channel1]", "play").unwrap().get(), 1.0);
    }

    #[test]
    fn layers_long_and_tap_presses() {
        let reg = registry();
        let mut rows = ScriptRows::new(&empty(), &mut Vec::new());
        let t = Instant::now();
        let shift_on = row("set_var 'shift' 1", When::Press, &[]);
        rows.handle(&reg, &shift_on, 0x90, 0x3F, 1.0, t);
        assert!(rows.layer_holds(&[("shift".into(), None)]));
        let long = row("deck 1 sync", When::Long, &[]);
        let tap = row("deck 1 play", When::Tap, &[]);
        // A short press: only the tap row.
        for r in [&long, &tap] {
            rows.handle(&reg, r, 0x90, 0x58, 1.0, t);
        }
        rows.finish(0x90, 0x58, 127, t);
        let soon = t + Duration::from_millis(100);
        for r in [&long, &tap] {
            rows.handle(&reg, r, 0x80, 0x58, 0.0, soon);
        }
        rows.finish(0x80, 0x58, 0, soon);
        rows.poll(&reg, t + Duration::from_secs(1), &mut Vec::new());
        assert_eq!(reg.get("[Channel1]", "play").unwrap().get(), 1.0);
        assert_eq!(reg.get("[Channel1]", "sync_enabled").unwrap().get(), 0.0);
        // Held: the long row once it's due.
        let t2 = t + Duration::from_secs(2);
        for r in [&long, &tap] {
            rows.handle(&reg, r, 0x90, 0x58, 1.0, t2);
        }
        rows.finish(0x90, 0x58, 127, t2);
        rows.poll(&reg, t2 + Duration::from_millis(600), &mut Vec::new());
        assert_eq!(reg.get("[Channel1]", "sync_enabled").unwrap().get(), 1.0);
    }

    #[test]
    fn a_held_layer_wins_over_no_layer() {
        let mut rows = ScriptRows::new(&empty(), &mut Vec::new());
        let plain = row("deck 1 play_pause", When::Press, &[]);
        let shifted = row("deck 1 goto_start", When::Press, &[("shift", None)]);
        let long = row("deck 1 sync", When::Long, &[]);
        let both = [plain.clone(), shifted.clone(), long.clone()];
        let lines = |rows: &ScriptRows| -> Vec<String> {
            rows.shown_through(&both).iter().filter_map(|i| i.action.clone()).collect()
        };
        assert_eq!(lines(&rows), ["deck 1 play_pause", "deck 1 sync"]);
        rows.vars.insert("shift".into(), 1.0);
        assert_eq!(lines(&rows), ["deck 1 goto_start", "deck 1 sync"]);
    }

    #[test]
    fn encoder_clicks_move_a_level_a_step() {
        let reg = registry();
        let mut rows = ScriptRows::new(&empty(), &mut Vec::new());
        let t = Instant::now();
        let mut enc = row("deck 1 level", When::Press, &[]);
        enc.options = Options::new(Options::SELECT_KNOB);
        reg.get("[Channel1]", "volume").unwrap().set(0.5);
        rows.handle(&reg, &enc, 0xB0, 0x00, 2.0, t);
        rows.handle(&reg, &enc, 0xB0, 0x00, -1.0, t);
        let v = reg.get("[Channel1]", "volume").unwrap().get();
        assert!((v - 0.5 - djsugar_script::STEP).abs() < 1e-9, "{v}");
    }

    #[test]
    fn lights_ask_their_line() {
        let reg = registry();
        let mut outputs = vec![Output {
            status: 0xB0,
            control: 0x30,
            on: 0x7F,
            script: Some("deck 1 vu_meter > 75% ? off : color 'orange'".into()),
            colors: vec![("orange".into(), 51)],
            ..Default::default()
        }];
        let mut rows = ScriptRows::new(&empty(), &mut outputs);
        assert!(outputs.is_empty());
        let mut sent = Vec::new();
        let t = Instant::now();
        rows.poll(&reg, t, &mut sent);
        assert_eq!(sent, vec![vec![0xB0, 0x30, 51]]);
        reg.get("[Channel1]", "vu_meter").unwrap().set(0.9);
        rows.poll(&reg, t + Duration::from_millis(50), &mut sent);
        assert_eq!(sent.last(), Some(&vec![0xB0, 0x30, 0]));
    }
}
