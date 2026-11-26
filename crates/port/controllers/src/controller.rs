//! One controller running one mapping: MIDI in goes to controls or script
//! functions, control changes come back out as LED messages. Follows the stock mappings'
//! `MidiController` and `MidiOutputHandler`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};

use crate::hid::{Io, Repeats};
use crate::host::{HidLink, Host, Target};
use crate::mapping::{Input, Mapping, Options, Output};
use crate::rows::ScriptRows;
use crate::script::Script;

struct OutputState {
    output: Output,
    control: ControlHandle,
    version: Option<u32>,
    last: Option<u8>,
}

pub struct Controller {
    name: String,
    mapping: Mapping,
    host: Rc<RefCell<Host>>,
    script: Script,
    /// Script prefixes, for `init`, `shutdown` and `incomingData`.
    prefixes: Vec<String>,
    /// The first half of a 14-bit pair, waiting for the second.
    fourteen_bit: Vec<(Input, u8)>,
    outputs: Vec<OutputState>,
    /// When to send every LED again. Some controllers (the DDJ-FLX4) ignore
    /// what arrives while they are still starting up, so the states sent
    /// with `init` are repeated once they're ready.
    resend_at: Option<Instant>,
    /// `DJSUGAR_MIDI_DEBUG` set: log messages both ways.
    debug: bool,
    /// The deck 3 and 4 switches and deck shift as the outputs last
    /// resolved them.
    layers: crate::host::RouteState,
    /// DJ Sugar mappings: DJSugarScript rows and lights.
    rows: ScriptRows,
    /// HID: the last input report, to skip repeats.
    repeats: Repeats,
}

/// The device an HID mapping talks to when there is none (a test, or the
/// Controller Editor reading the mapping): writes go nowhere.
struct NoDevice;

impl Io for NoDevice {
    fn write(&self, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
    fn send_feature(&self, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
    fn get_input(&self, _: u8) -> Option<Vec<u8>> {
        None
    }
    fn get_feature(&self, _: u8) -> Option<Vec<u8>> {
        None
    }
}

/// How long after loading a mapping its LEDs are sent again.
const RESEND_AFTER: Duration = Duration::from_millis(1500);

/// `computeValue`: applies a binding's options to a 7-bit value.
fn compute_value(options: Options, prev: f64, value: f64) -> f64 {
    if options.is_empty() {
        return value;
    }
    let mut v = value;
    if options.has(Options::INVERT) {
        v = 127.0 - v;
    }
    if options.has(Options::ROT64) || options.has(Options::ROT64_INV) {
        let mut diff = v - 64.0;
        if diff == -1.0 || diff == 1.0 {
            diff /= 16.0;
        } else {
            diff += if diff > 0.0 { -1.0 } else { 1.0 };
        }
        let t = if options.has(Options::ROT64) { prev + diff } else { prev - diff };
        t.clamp(0.0, 127.0)
    } else if options.has(Options::ROT64_FAST) {
        (prev + (v - 64.0) * 1.5).clamp(0.0, 127.0)
    } else if options.has(Options::DIFF) {
        if v >= 64.0 {
            v -= 128.0;
        }
        prev + v
    } else if options.has(Options::SELECT_KNOB) {
        if v >= 64.0 {
            v - 128.0
        } else {
            v
        }
    } else if options.has(Options::BUTTON) {
        if v != 0.0 {
            1.0
        } else {
            0.0
        }
    } else if options.has(Options::SWITCH) {
        1.0
    } else if options.has(Options::SPREAD64) {
        v - 64.0
    } else if options.has(Options::HERC_JOG) || options.has(Options::HERC_JOG_FAST) {
        if v > 64.0 {
            v -= 128.0;
        }
        if options.has(Options::HERC_JOG_FAST) {
            prev + v * 3.0
        } else {
            prev + v
        }
    } else {
        v
    }
}

/// Whether a binding's values say where the hardware is (a knob or fader),
/// rather than steps (an encoder) or presses.
fn is_absolute(options: Options) -> bool {
    const RELATIVE: u32 = Options::ROT64
        | Options::ROT64_INV
        | Options::ROT64_FAST
        | Options::DIFF
        | Options::BUTTON
        | Options::SWITCH
        | Options::HERC_JOG
        | Options::HERC_JOG_FAST
        | Options::SPREAD64
        | Options::SELECT_KNOB;
    !options.has(RELATIVE)
}

/// `getParameterForMidi` of a potmeter, for soft takeover.
fn parameter_for_midi(midi: f64) -> f64 {
    if midi > 64.0 {
        (midi - 1.0) / 126.0
    } else {
        midi / 128.0
    }
}

impl Controller {
    /// Loads the mapping's scripts and calls their `init` functions.
    pub fn new(name: &str, mapping: Mapping, registry: Arc<ControlRegistry>) -> Result<Self, String> {
        Self::with_device(name, mapping, registry, None)
    }

    /// Like [`Self::new`], for an HID mapping talking to `device`: its
    /// scripts' `controller.getInputReport` and the like reach it at once,
    /// and [`Self::take_reports`] gives the output reports to write.
    pub fn new_hid(
        name: &str,
        mapping: Mapping,
        registry: Arc<ControlRegistry>,
        device: Rc<dyn Io>,
    ) -> Result<Self, String> {
        Self::with_device(name, mapping, registry, Some(device))
    }

    fn with_device(
        name: &str,
        mapping: Mapping,
        registry: Arc<ControlRegistry>,
        device: Option<Rc<dyn Io>>,
    ) -> Result<Self, String> {
        let host = Rc::new(RefCell::new(Host::new(registry, name.to_string())));
        if mapping.is_hid() {
            host.borrow_mut().hid =
                Some(HidLink { io: device.unwrap_or_else(|| Rc::new(NoDevice)), out: Default::default() });
        }
        host.borrow_mut().script_bound = mapping
            .inputs
            .iter()
            .filter(|(_, inputs)| inputs.iter().any(|i| i.options.has(Options::SCRIPT)))
            .map(|(k, _)| *k)
            .collect();
        let script = Script::new(host.clone(), &mapping.settings, mapping.is_hid())?;
        let mut prefixes = Vec::new();
        // A mapping that never names deck 3 or 4 was written for two decks:
        // the deck 3 and 4 switches move its sides over.
        let names_decks_3_4 = |text: &str| text.contains("Channel3") || text.contains("Channel4");
        let mut four_decks = mapping
            .inputs
            .values()
            .flatten()
            .map(|i| &i.group)
            .chain(mapping.outputs.iter().map(|o| &o.group))
            .any(|g| names_decks_3_4(g));
        let mut sources = Vec::new();
        for file in &mapping.scripts {
            let source = std::fs::read_to_string(&file.path).map_err(|e| format!("{}: {e}", file.path.display()))?;
            four_decks |= names_decks_3_4(&source);
            sources.push((file, source));
        }
        if !four_decks {
            host.borrow_mut().enable_layers();
        }
        for (file, source) in sources {
            let label = file.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            script.eval_file(&label, &source)?;
            if !file.prefix.is_empty() && !prefixes.contains(&file.prefix) {
                prefixes.push(file.prefix.clone());
            }
        }
        let mut plain_outputs = mapping.outputs.clone();
        let rows = ScriptRows::new(&mapping, &mut plain_outputs);
        let outputs = {
            let h = host.borrow();
            plain_outputs
                .iter()
                .map(|o| OutputState {
                    control: match h.resolve(&o.group, &o.key) {
                        Target::Control(c) | Target::Sub(c, _) => c,
                    },
                    output: o.clone(),
                    version: None,
                    last: None,
                })
                .collect()
        };
        let mut c = Self {
            name: name.to_string(),
            mapping,
            host,
            script,
            prefixes,
            fourteen_bit: Vec::new(),
            outputs,
            resend_at: Some(Instant::now() + RESEND_AFTER),
            debug: std::env::var_os("DJSUGAR_MIDI_DEBUG").is_some(),
            layers: ([false; 2], 0),
            rows,
            repeats: Repeats::default(),
        };
        let id = serde_str(&c.name);
        for p in c.prefixes.clone() {
            if c.script.has_function(&format!("{p}.init")) {
                c.script.eval(&format!("{p}.init({id}, false)"));
            }
        }
        c.poll(Instant::now());
        Ok(c)
    }

    /// Starts noting every control the mapping's scripts write, for
    /// [`Self::take_trace`]; see the Controller Editor's probe.
    pub fn start_trace(&mut self) {
        self.host.borrow_mut().trace = Some(Vec::new());
    }

    /// The controls scripts wrote since the last call, while tracing, with
    /// the last value other than 0 written to each.
    pub fn take_trace(&mut self) -> Vec<(djsugar_control::ConfigKey, f64)> {
        match &mut self.host.borrow_mut().trace {
            Some(t) => std::mem::take(t),
            None => Vec::new(),
        }
    }

    pub fn mapping_name(&self) -> &str {
        &self.mapping.name
    }

    /// Handles one incoming MIDI message.
    pub fn receive(&mut self, msg: &[u8]) {
        let Some(&status) = msg.first() else { return };
        if self.debug {
            eprintln!("{} -> {msg:02X?}", self.name);
        }
        // Clock and other real-time messages aren't mapped.
        if status >= 0xF8 {
            return;
        }
        if status == 0xF0 || msg.len() > 3 {
            for p in self.prefixes.clone() {
                if self.script.has_function(&format!("{p}.incomingData")) {
                    self.script.call_incoming_data(&p, msg);
                }
            }
            return;
        }
        let control = self.host.borrow().routed_input(status, msg.get(1).copied().unwrap_or(0));
        let value = msg.get(2).copied().unwrap_or(0);
        self.host.borrow_mut().input = Some((status, control));
        if let Some(inputs) = self.mapping.inputs.get(&(status, control)).cloned() {
            for input in self.rows.shown_through(&inputs) {
                self.process(input, status, control, value);
            }
            self.rows.finish(status, control, value, Instant::now());
        }
        if self.host.borrow().has_input_handlers(status, control) {
            self.script.fire_input(status, control, value);
        }
        self.host.borrow_mut().input = None;
    }

    /// Handles one HID input report, as read from the device (starting with the report ID if the device numbers its
    /// reports): the mapping's `incomingData` functions get it. A report the same as the last is skipped.
    pub fn receive_report(&mut self, report: &[u8]) {
        if report.is_empty() || self.repeats.is_repeat(report) {
            return;
        }
        if self.debug {
            eprintln!("{} -> {report:02X?}", self.name);
        }
        for p in self.prefixes.clone() {
            if self.script.has_function(&format!("{p}.incomingData")) {
                self.script.call_incoming_report(&p, report);
            }
        }
    }

    /// HID output reports to write to the device, each starting with its
    /// report ID, oldest first.
    pub fn take_reports(&mut self) -> Vec<Vec<u8>> {
        let out = match &mut self.host.borrow_mut().hid {
            Some(hid) => hid.out.take(),
            None => return Vec::new(),
        };
        if self.debug {
            for r in &out {
                eprintln!("{} <- {r:02X?}", self.name);
            }
        }
        out
    }

    /// A report couldn't be written: send its next one even if unchanged.
    pub fn report_failed(&mut self, id: u8) {
        if let Some(hid) = &mut self.host.borrow_mut().hid {
            hid.out.forget(id);
        }
    }

    fn process(&mut self, input: &Input, status: u8, control: u8, value: u8) {
        let opcode = status & 0xF0;
        if !input.layer.is_empty() && !self.rows.layer_holds(&input.layer) {
            return;
        }
        if input.action.is_some() {
            // The LSB of a 14-bit pair only refines the MSB's value.
            if input.options.has(Options::FOURTEEN_BIT_LSB) {
                return;
            }
            let v = if opcode == 0xE0 {
                (((value as u32) << 7) | control as u32) as f64 / 16383.0
            } else if input.options.has(Options::SELECT_KNOB) {
                // A relative encoder's clicks, 127 being one to the left.
                if value >= 64 {
                    value as f64 - 128.0
                } else {
                    value as f64
                }
            } else {
                value as f64 / 127.0
            };
            let registry = self.host.borrow().registry.clone();
            self.rows.handle(&registry, input, status, control, v, Instant::now());
            return;
        }
        if input.options.has(Options::SCRIPT) {
            let channel = (status & 0x0F) as f64;
            self.script.call_binding(&input.key, (channel, control as f64, value as f64, status as f64, &input.group));
            return;
        }
        let target = self.host.borrow().resolve(&input.group, &input.key);
        let fourteen = input.options.has(Options::FOURTEEN_BIT_MSB) || input.options.has(Options::FOURTEEN_BIT_LSB);
        if !fourteen {
            self.fourteen_bit.clear();
        }
        let new_value = if fourteen {
            let pending = self.fourteen_bit.iter().position(|(i, _)| {
                i.group == input.group
                    && i.key == input.key
                    && i.options.has(Options::FOURTEEN_BIT_MSB) != input.options.has(Options::FOURTEEN_BIT_MSB)
            });
            let Some(k) = pending else {
                self.fourteen_bit.retain(|(i, _)| !(i.group == input.group && i.key == input.key));
                self.fourteen_bit.push((input.clone(), value));
                return;
            };
            let (_, other) = self.fourteen_bit.remove(k);
            let joint = if input.options.has(Options::FOURTEEN_BIT_MSB) {
                ((value as u32) << 7) | other as u32
            } else {
                ((other as u32) << 7) | value as u32
            };
            (joint as f64 / 128.0).min(127.0)
        } else if opcode == 0xE0 {
            ((((value as u32) << 7) | control as u32) as f64 / 128.0).min(127.0)
        } else {
            compute_value(input.options, target.midi_parameter(), value as f64)
        };
        let note_off = opcode == 0x80 && !(input.options.has(Options::BUTTON) || input.options.has(Options::SWITCH));
        // Every knob and fader picks up (see `pickup`), whatever the
        // mapping says. Not encoders and buttons: their values are steps or
        // presses, not where the hardware is.
        if target.is_potmeter() && (opcode == 0xB0 || opcode == 0xE0) && is_absolute(input.options) {
            let mut h = self.host.borrow_mut();
            h.soft_takeover(&input.group, &input.key, true);
            if let Some(p) = h.soft_take(&input.group, &input.key, &target, parameter_for_midi(new_value)) {
                drop(h);
                if let Target::Control(c) = &target {
                    c.set_parameter(p);
                }
                return;
            }
            if h.soft_has(&input.group, &input.key) {
                return;
            }
        }
        target.set_from_midi(note_off, new_value);
    }

    /// Lets the device's deck choice in Settings (see [`crate::routing`])
    /// move it to other decks: `port` is the device's port name.
    pub fn follow_deck_choice(&mut self, port: &str) {
        self.host.borrow_mut().set_route(crate::routing::route(port));
        self.rows.shift = self.host.borrow().shift;
    }

    /// Runs timers, scratching, connection callbacks and LED outputs.
    pub fn poll(&mut self, now: Instant) {
        self.host.borrow_mut().refresh_shift();
        self.rows.shift = self.host.borrow().shift;
        self.host.borrow_mut().process_scratch(now);
        let due = self.host.borrow_mut().due_timers(now);
        for id in due {
            self.script.fire_timer(id);
        }
        // Callbacks can change more controls; a few rounds settle chains of
        // connections without letting a feedback loop spin forever.
        for _ in 0..4 {
            let changed = self.host.borrow_mut().changed_watches();
            if changed.is_empty() {
                break;
            }
            for (id, value) in changed {
                self.script.fire_connection(id, value);
            }
        }
        if self.resend_at.is_some_and(|t| now >= t) {
            self.resend_at = None;
            for o in &mut self.outputs {
                o.version = None;
                o.last = None;
            }
            self.rows.resend();
        }
        {
            let registry = self.host.borrow().registry.clone();
            let mut sent = Vec::new();
            self.rows.poll(&registry, now, &mut sent);
            self.host.borrow_mut().sent.extend(sent);
        }
        let mut h = self.host.borrow_mut();
        // A side switched decks, or the instrument the device follows
        // moved: the LEDs follow the other deck.
        let layers = h.route_state();
        if layers != self.layers {
            self.layers = layers;
            h.layers_moved();
            for o in &mut self.outputs {
                o.control = match h.resolve(&o.output.group, &o.output.key) {
                    Target::Control(c) | Target::Sub(c, _) => c,
                };
                o.version = None;
            }
        }
        for o in &mut self.outputs {
            let v = o.control.version();
            if o.version == Some(v) {
                continue;
            }
            o.version = Some(v);
            let value = o.control.get();
            let byte = if value >= o.output.min && value <= o.output.max { o.output.on } else { o.output.off };
            if o.last != Some(byte) && byte != 0xFF {
                h.sent.push(vec![o.output.status, o.output.control, byte]);
                o.last = Some(byte);
            }
        }
    }

    /// Script errors so far (exceptions, bad bindings).
    pub fn errors(&self) -> Vec<String> {
        let mut out = self.host.borrow().errors.clone();
        out.extend(self.rows.errors.iter().cloned());
        out
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        [self.host.borrow().next_deadline(), self.resend_at, self.rows.next_deadline()].into_iter().flatten().min()
    }

    /// MIDI messages to send to the device, oldest first.
    pub fn take_output(&mut self) -> Vec<Vec<u8>> {
        let out = self.host.borrow_mut().take_routed();
        {
            let mut h = self.host.borrow_mut();
            for m in &out {
                h.sent_back(m);
            }
        }
        if self.debug {
            for m in &out {
                eprintln!("{} <- {m:02X?}", self.name);
            }
        }
        out
    }

    /// Shows where this controller's knobs and faders are on screen (see
    /// [`crate::pickup`]). Off for stand-ins such as the editor's probe.
    pub fn publish_positions(&mut self) {
        self.host.borrow_mut().publish = true;
    }

    /// Calls the scripts' `shutdown` functions (they usually turn LEDs off)
    /// and lets go of any deck this controller still holds by its jog.
    pub fn shutdown(&mut self) {
        crate::pickup::forget_port(&self.name);
        for p in self.prefixes.clone() {
            if self.script.has_function(&format!("{p}.shutdown")) {
                self.script.eval(&format!("{p}.shutdown()"));
            }
        }
        self.host.borrow_mut().release_decks();
    }
}

/// A JS string literal.
fn serde_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
