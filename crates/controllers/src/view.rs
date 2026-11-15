//! The Controller Editor's picture of a mapping: every mapped control as a
//! row saying what it does, grouped by function, plus what each LED shows
//! and the mapping's own settings. Built from the XML, the probe (for script
//! bindings) and, where there is one, the controller's layout.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use djsugar_control::{ConfigKey, ControlRegistry};

use crate::catalog::{describe, describe_value, humanize, Function};
use crate::format::DjSugarMapping;
use crate::layout::Layout;
use crate::mapping::{Input, Mapping, Options, SettingDef, When};
use crate::probe::{is_shift_binding, message_key, probe, Effect, MessageKey, Probe};

/// One thing a control does.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub function: Function,
    /// The hardware control: "PLAY/PAUSE", "PAD 2", "ENCODER 5".
    pub control: String,
    /// Its id in the layout, when there is a picture.
    pub part: Option<String>,
    /// "SHIFT", "Hot cue", "Bank 2 press"; empty for the plain press.
    pub layer: String,
    /// "button", "pad", "knob", "encoder", "fader", "jog" or "switch".
    pub kind: String,
    /// The messages it comes in on (two for a 14-bit fader).
    pub keys: Vec<MessageKey>,
    /// "Note 0B · ch 1", "CC 13, 33 · ch 1".
    pub midi: String,
    /// The script function behind it, for script bindings.
    pub script: Option<String>,
    /// What the mapping's author wrote about it.
    pub description: String,
    /// The binding's options in words: "Pick-up, 14-bit".
    pub options: String,
    /// The bindings behind it, for editing it.
    pub sources: Vec<Source>,
    /// Changed in the DJ's own mapping shown.
    pub edited: bool,
    /// Kept in the own mapping but doing nothing (a row an import couldn't
    /// translate), with why.
    pub off: bool,
    pub note: String,
}

/// One binding behind a row: the message and what it is bound to.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub status: u8,
    pub number: u8,
    pub group: String,
    /// A control, or a script function for script bindings.
    pub key: String,
    pub options: Options,
    /// An DJSugarScript line, in an DJ Sugar mapping.
    pub action: Option<String>,
    pub layer: Vec<(String, Option<f64>)>,
    pub when: When,
}

impl Source {
    fn of(status: u8, number: u8, input: &Input) -> Self {
        Source {
            status,
            number,
            group: input.group.clone(),
            key: input.key.clone(),
            options: input.options,
            action: input.action.clone(),
            layer: input.layer.clone(),
            when: input.when,
        }
    }

    pub fn message(&self) -> MessageKey {
        message_key(self.status, self.number)
    }

    /// A binding to a control (not a script).
    pub fn is_control(&self) -> bool {
        self.action.is_none() && !self.options.has(Options::SCRIPT)
    }
}

/// What an LED shows.
#[derive(Clone, Debug, PartialEq)]
pub struct LightRow {
    pub function: Function,
    pub control: String,
    pub part: Option<String>,
    pub key: MessageKey,
    pub midi: String,
}

pub struct MappingView {
    pub name: String,
    pub author: String,
    pub file: String,
    pub rows: Vec<Row>,
    pub lights: Vec<LightRow>,
    pub options: Vec<SettingDef>,
    pub layout: Option<Layout>,
    /// Problems running the mapping in the sandbox, if any.
    pub notes: Vec<String>,
}

/// "Note 0B · ch 1", "CC 13 · ch 7", "Pitch bend · ch 2".
pub fn message_text(status: u8, number: u8) -> String {
    let ch = (status & 0x0F) + 1;
    match status & 0xF0 {
        0x80 | 0x90 => format!("Note {number:02X} · ch {ch}"),
        0xB0 => format!("CC {number:02X} · ch {ch}"),
        0xE0 => format!("Pitch bend · ch {ch}"),
        0xA0 => format!("Aftertouch {number:02X} · ch {ch}"),
        0xC0 => format!("Program {number:02X} · ch {ch}"),
        _ => format!("{status:02X} {number:02X}"),
    }
}

/// "CC 13 · ch 1" and "CC 33 · ch 1" → "CC 13, 33 · ch 1"; otherwise both,
/// with " / " between.
fn join_messages(a: &str, b: &str) -> String {
    if let Some((msg, ch)) = b.rsplit_once(" · ") {
        if let Some((kind, num)) = msg.rsplit_once(' ') {
            let head = format!("{kind} ");
            let tail = format!(" · {ch}");
            if let Some(nums) = a.strip_prefix(&head).and_then(|r| r.strip_suffix(&tail)) {
                if !nums.contains(' ') || nums.contains(", ") {
                    return format!("{head}{nums}, {num}{tail}");
                }
            }
        }
    }
    format!("{a} / {b}")
}

fn options_text(o: Options) -> String {
    let words = [
        (Options::SCRIPT, "Script"),
        (Options::INVERT, "Invert"),
        (Options::ROT64, "Relative (64 = centre)"),
        (Options::ROT64_INV, "Relative, reversed"),
        (Options::ROT64_FAST, "Relative, fast"),
        (Options::DIFF, "Relative (two's complement)"),
        (Options::SELECT_KNOB, "Endless (select knob)"),
        (Options::BUTTON, "Button"),
        (Options::SWITCH, "Switch"),
        (Options::HERC_JOG, "Jog"),
        (Options::HERC_JOG_FAST, "Jog, fast"),
        (Options::SPREAD64, "Centred at 64"),
        (Options::SOFT_TAKEOVER, "Pick-up"),
        (Options::FOURTEEN_BIT_MSB, "14-bit"),
        (Options::FOURTEEN_BIT_LSB, "14-bit"),
    ];
    let mut out: Vec<&str> = Vec::new();
    for (flag, w) in words {
        if o.has(flag) && !out.contains(&w) {
            out.push(w);
        }
    }
    out.join(", ")
}

fn guess_kind(status: u8, o: Options) -> &'static str {
    match status & 0xF0 {
        0x80 | 0x90 => "button",
        0xE0 => "fader",
        _ if o.has(Options::SELECT_KNOB)
            || o.has(Options::ROT64)
            || o.has(Options::ROT64_INV)
            || o.has(Options::ROT64_FAST)
            || o.has(Options::DIFF) =>
        {
            "encoder"
        }
        _ if o.has(Options::HERC_JOG) || o.has(Options::HERC_JOG_FAST) => "jog",
        _ => "knob",
    }
}

/// "PLAY/PAUSE +SHIFT (DECK1) - press - Reverse…" → ("PLAY/PAUSE (DECK1)",
/// "Reverse…", true).
fn split_description(d: &str) -> (String, String, bool) {
    let parts: Vec<&str> = d.split(" - ").map(str::trim).collect();
    let head = parts.first().copied().unwrap_or("");
    let shift = head.to_ascii_lowercase().contains("shift");
    let name = head
        .replace("+SHIFT", "")
        .replace("+ SHIFT", "")
        .replace("+ shift", "")
        .replace("SHIFT +", "")
        .replace("Shift +", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let what = if parts.len() > 1 {
        parts.last().copied().unwrap_or("").to_string()
    } else {
        String::new()
    };
    (name, what, shift)
}

/// The part of a script binding that names the control or what it does:
/// `XoneK2.controllers[0xE].columns[1].knobs[2].input` → "Columns 1 knobs 2";
/// `MidiFighterTwister.controller.toggleDeck` → "Toggle deck".
fn script_name(key: &str) -> String {
    let mut parts: Vec<&str> = key.split('.').collect();
    if parts.last() == Some(&"input") {
        parts.pop();
    }
    let tail: Vec<String> = parts
        .iter()
        .skip(1)
        .filter(|p| !p.starts_with("controller"))
        .map(|p| p.replace(['[', ']', '"'], " "))
        .collect();
    let last = tail.last().cloned().unwrap_or_else(|| key.to_string());
    humanize(last.trim())
}

/// Natural order: "Hot cue 2" before "Hot cue 10".
fn natural(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut v: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match v.last_mut() {
                Some((ld, t)) if *ld == d => t.push(c),
                _ => v.push((d, c.to_string())),
            }
        }
        v
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for ((da, ta), (db, tb)) in ca.iter().zip(cb.iter()) {
        let o = if *da && *db {
            ta.parse::<u64>()
                .unwrap_or(0)
                .cmp(&tb.parse::<u64>().unwrap_or(0))
        } else {
            ta.to_lowercase().cmp(&tb.to_lowercase())
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    ca.len().cmp(&cb.len())
}

/// Controls a script touches that say something to a DJ (the probe has
/// already dropped ones scripts make up for their own state).
fn meaningful(k: &ConfigKey) -> bool {
    !k.item.ends_with("_indicator") && !k.item.contains("VuMeter")
}

fn functions(touched: &[Effect]) -> Vec<Function> {
    let mut out: Vec<Function> = Vec::new();
    for (k, v) in touched {
        if !meaningful(k) {
            continue;
        }
        let f = describe_value(&k.group, &k.item, *v);
        if !out.contains(&f) {
            out.push(f);
        }
    }
    // Plumbing next to a real function (a mode flag the script also set, the
    // size it set before jumping) goes; it stays when it's all there is.
    if out.iter().any(|f| f.category != "Controller") {
        out.retain(|f| f.category != "Controller");
    }
    let setting = |f: &Function| f.name.contains(" size");
    if out.iter().any(|f| !setting(f)) {
        out.retain(|f| !setting(f));
    }
    out
}

struct Builder<'a> {
    mapping: &'a Mapping,
    layout: Option<&'a Layout>,
    probe: &'a Probe,
    rows: Vec<Row>,
}

impl Builder<'_> {
    fn add(&mut self, status: u8, number: u8, input: &Input) {
        let key = message_key(status, number);
        let (desc_control, desc_what, desc_shift) = split_description(&input.description);
        let found = self.layout.and_then(|l| l.find(status, number));
        let script = match &input.action {
            Some(line) => Some(line.clone()),
            None => input
                .options
                .has(Options::SCRIPT)
                .then(|| input.key.clone()),
        };
        let control = match (found, &script) {
            (Some((p, _)), _) => p.label.clone(),
            _ if !desc_control.is_empty() => desc_control.clone(),
            _ if !input.name.is_empty() => humanize(&input.name),
            (None, Some(s)) => script_name(s),
            _ => message_text(status, number),
        };
        let base_layer = match found {
            Some((_, m)) => m.layer.clone(),
            None if desc_shift => "SHIFT".into(),
            None => String::new(),
        };
        // An DJ Sugar mapping's own layer and when: "SHIFT", "Long press".
        let mut extra: Vec<String> = input
            .layer
            .iter()
            .map(|(n, v)| match v {
                Some(v) => format!("{} {v}", n.to_uppercase()),
                None => n.to_uppercase(),
            })
            .collect();
        if input.when != When::Press {
            extra.push(when_text(input.when).into());
        }
        let base_layer = std::iter::once(base_layer)
            .chain(extra)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" + ");
        let kind = match found {
            Some((p, _)) => p.kind.clone(),
            None => guess_kind(status, input.options).into(),
        };
        let make = |function: Function, layer: String| Row {
            function,
            control: control.clone(),
            part: found.map(|(p, _)| p.id.clone()),
            layer,
            kind: kind.clone(),
            keys: vec![key],
            midi: message_text(status, number),
            script: script.clone(),
            description: input.description.clone(),
            options: options_text(input.options),
            sources: vec![Source::of(status, number, input)],
            edited: false,
            off: false,
            note: String::new(),
        };
        let fallback = |what: &str| {
            let target = describe(&input.group, "").target;
            let name = if is_shift_binding(&input.key) {
                "SHIFT".to_string()
            } else if !what.is_empty() {
                what.to_string()
            } else if let Some(s) = &script {
                script_name(s)
            } else {
                humanize(&input.key)
            };
            Function::new("Controller", name, target)
        };
        // A control the picture lists with its own SHIFT message: the
        // controller sends that one while SHIFT is held, so the script is
        // shifted for it and never for the plain message.
        let sends_shift =
            |m: &crate::layout::PartMessage| m.layer.split(" + ").any(|l| l.starts_with("SHIFT"));
        let hardware_shift = found.is_some_and(|(p, _)| p.midi.iter().any(sends_shift));
        let shifted_message = found.is_some_and(|(_, m)| sends_shift(m));
        let mut new_rows = Vec::new();
        if let Some(line) = &input.action {
            let name = if input.name.is_empty() {
                line.clone()
            } else {
                humanize(&input.name)
            };
            new_rows.push(make(script_function(line, name), base_layer));
        } else if script.is_none() {
            new_rows.push(make(describe(&input.group, &input.key), base_layer));
        } else {
            let normal = match self.probe.shifted.get(&key) {
                Some(shifted) if shifted_message => functions(shifted),
                _ => functions(self.probe.normal.get(&key).map_or(&[][..], |v| v)),
            };
            // A SHIFT button is just that, whatever its script resets.
            let shift = is_shift_binding(&input.key);
            if shift || normal.is_empty() {
                new_rows.push(make(fallback(&desc_what), base_layer.clone()));
            }
            for f in normal.into_iter().filter(|_| !shift) {
                new_rows.push(make(f, base_layer.clone()));
            }
            if let Some(shifted) = self
                .probe
                .shifted
                .get(&key)
                .filter(|_| !shift && !hardware_shift)
            {
                let layer = if base_layer.is_empty() {
                    "SHIFT".to_string()
                } else {
                    format!("{base_layer} + SHIFT")
                };
                for f in functions(shifted) {
                    if !new_rows.iter().any(|r| r.function == f) {
                        new_rows.push(make(f, layer.clone()));
                    }
                }
            }
        }
        for r in new_rows {
            // The same function on the same control and layer from another
            // message (the other half of a 14-bit pair, the same button on
            // another channel) joins that row.
            match self.rows.iter_mut().find(|o| {
                o.function == r.function
                    && o.layer == r.layer
                    && o.control == r.control
                    && o.part == r.part
            }) {
                Some(o) if !o.keys.contains(&key) => {
                    o.keys.push(key);
                    o.midi = join_messages(&o.midi, &r.midi);
                    o.sources.extend(r.sources);
                }
                Some(o) => {
                    for s in r.sources {
                        if !o.sources.contains(&s) {
                            o.sources.push(s);
                        }
                    }
                }
                None => self.rows.push(r),
            }
        }
    }
}

fn when_text(w: When) -> &'static str {
    match w {
        When::Press => "Press",
        When::Release => "Release",
        When::Long => "Long press",
        When::Tap => "Tap",
        When::Double => "Double press",
    }
}

/// What an DJSugarScript line does: `deck 1 play` is Play on Deck 1, as a
/// binding to that control would be; anything else is named `name`.
fn script_function(line: &str, name: String) -> Function {
    let words: Vec<&str> = line.split_whitespace().collect();
    if let [kind, n, control] = words[..] {
        let group = match kind {
            "deck" => format!("[Channel{n}]"),
            "sampler" => format!("[Sampler{n}]"),
            "fx" => format!("[EffectRack1_EffectUnit{n}]"),
            _ => String::new(),
        };
        if !group.is_empty() && control.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return describe(&group, control);
        }
    }
    // `(var 'k2mode' ? hot_cue 1 : play_pause)`: both, as "Hot cue 1 /
    // Play pause".
    let l = unwrap_parens(line.trim());
    if let Some((_, rest)) = split_top(l, '?') {
        if let Some((a, b)) = split_top(rest, ':') {
            if let (Some(a), Some(b)) = (line_function(a), line_function(b)) {
                let target = if a.target == b.target {
                    a.target
                } else {
                    String::new()
                };
                return Function::new(a.category, format!("{} / {}", a.name, b.name), target);
            }
        }
    }
    line_function(l).unwrap_or_else(|| Function::new("Controller", name, ""))
}

fn unwrap_parens(s: &str) -> &str {
    let s = s.trim();
    match s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        // Only when the two belong together: `(a) & (b)` stays.
        Some(inner) if split_top(inner, ')').is_none() || !inner.contains(')') => inner.trim(),
        _ => s,
    }
}

/// `s` split at its first `sep` outside parentheses and quotes.
fn split_top(s: &str, sep: char) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    let mut quoted = false;
    for (i, c) in s.char_indices() {
        match c {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted && depth > 0 => depth -= 1,
            c if c == sep && !quoted && depth == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}

/// One plain action in an DJSugarScript line, in words: `deck 1 hot_cue 4`
/// is "Hot cue 4" on Deck 1, `press '[Channel2],hotcue_3_clear'` what
/// Mixxx's control does.
fn line_function(line: &str) -> Option<Function> {
    let line = unwrap_parens(line);
    if line.contains('&') || line.contains('?') {
        return None;
    }
    let mut words = line.split_whitespace().peekable();
    let mut deck = None;
    if words.peek() == Some(&"deck") {
        words.next();
        deck = words.next().map(str::to_string);
    }
    let verb = words.next()?;
    let rest: Vec<&str> = words.collect();
    let group = deck.as_ref().map(|d| format!("[Channel{d}]"));
    let target = deck
        .as_ref()
        .map(|d| format!("Deck {d}"))
        .unwrap_or_default();
    if matches!(verb, "press" | "toggle" | "set" | "adjust" | "get") {
        let quoted = rest.first()?.trim_matches('\'');
        let (g, k) = match quoted.strip_prefix('[').and_then(|r| r.split_once("],")) {
            Some((g, k)) => (format!("[{g}]"), k.to_string()),
            None => (group?, quoted.to_string()),
        };
        return Some(describe(&g, &k));
    }
    // Verbs that are a Mixxx control underneath read as that control.
    let n = rest.first().map(|a| a.trim_matches('\'')).unwrap_or("");
    let key = match verb {
        "play" | "pause" | "play_pause" => Some("play".to_string()),
        "sync" => Some("sync_enabled".into()),
        "key_lock" => Some("keylock".into()),
        "slip" => Some("slip_enabled".into()),
        "pfl" => Some("pfl".into()),
        "hot_cue" if !n.is_empty() => Some(format!("hotcue_{n}_activate")),
        "loop" if !n.is_empty() => Some(format!("beatloop_{n}_toggle")),
        "loop_exit" => Some("reloop_toggle".into()),
        "loop_half" => Some("loop_halve".into()),
        "loop_double" => Some("loop_double".into()),
        "loop_in" => Some("loop_in".into()),
        "loop_out" => Some("loop_out".into()),
        "pitch_zero" => Some("rate_set_zero".into()),
        "goto_start" => Some("start".into()),
        "load" => Some("LoadSelectedTrack".into()),
        "volume" | "level" => Some("volume".into()),
        "gain" => Some("pregain".into()),
        "jump" => Some("beatjump".into()),
        "loop_move" => Some("loop_move_loops".into()),
        _ => None,
    };
    if let (Some(g), Some(k)) = (&group, key) {
        let f = describe(g, &k);
        if f.category != "Controller" {
            return Some(f);
        }
    }
    let mut name = humanize(verb);
    for a in &rest {
        name.push(' ');
        name.push_str(a.trim_matches('\''));
    }
    Some(Function::new("Controller", name, target))
}

/// Reads the mapping at `path` and works out what it does. `registry` must
/// hold the engine's controls and be used for nothing else (see
/// [`crate::probe::probe`]).
pub fn build(path: &Path, registry: Arc<ControlRegistry>) -> Result<MappingView, String> {
    let mapping = Mapping::load(path)?;
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    build_from(&mapping, file, registry)
}

/// Like [`build`], for the DJ's own mapping `own` put over the Mixxx mapping
/// at `path` (the one it's based on): rows it changes are marked, and rows
/// it keeps greyed are listed with why. `path` can also be the own
/// mapping's file, for one that stands alone (made with Quick map).
pub fn build_own(
    path: &Path,
    own: &DjSugarMapping,
    registry: Arc<ControlRegistry>,
) -> Result<MappingView, String> {
    let (base, dir) = if crate::is_own_file(&path.to_string_lossy()) {
        let dir = crate::default_mappings_dir().unwrap_or_default();
        let base = match &own.based_on {
            Some(b) => Some(Mapping::load(&dir.join(b))?),
            None => None,
        };
        (base, dir)
    } else {
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        (Some(Mapping::load(path)?), dir)
    };
    let mapping = own.build(base.as_ref(), &dir);
    // One that stands alone changes nothing: all its rows are its own.
    let alone = base.is_none();
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut v = build_from(&mapping, file, registry)?;
    for row in &mut v.rows {
        row.edited = !alone && crate::edit::is_edited(own, row);
    }
    let layout = v.layout.clone();
    // A message the own mapping takes away and gives nothing else on its
    // layer: a row saying so, to change or put back.
    for r in own.rows.iter().filter(|r| r.remove && !r.off) {
        let (s, n) = (r.midi.status, r.midi.number);
        let replaced = own.rows.iter().any(|o| {
            !o.remove && !o.off && (o.midi.status, o.midi.number) == (s, n) && o.layer == r.layer
        });
        if replaced {
            continue;
        }
        let found = layout.as_ref().and_then(|l| l.find(s, n));
        let name = if r.name.is_empty() {
            message_text(s, n)
        } else {
            r.name.clone()
        };
        let mut layer = r.layer.to_uppercase();
        if r.when != When::Press {
            layer = [layer, when_text(r.when).into()]
                .into_iter()
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" + ");
        }
        v.rows.push(Row {
            function: Function::new("Controller", "Does nothing", ""),
            control: found.map_or(name, |(p, _)| p.label.clone()),
            part: found.map(|(p, _)| p.id.clone()),
            layer,
            kind: found.map_or_else(
                || guess_kind(s, Options::default()).into(),
                |(p, _)| p.kind.clone(),
            ),
            keys: vec![message_key(s, n)],
            midi: message_text(s, n),
            script: None,
            description: String::new(),
            options: String::new(),
            sources: vec![Source {
                status: s,
                number: n,
                group: String::new(),
                key: String::new(),
                options: Options::default(),
                action: None,
                layer: r.layer_conditions(),
                when: r.when,
            }],
            edited: true,
            off: false,
            note: String::new(),
        });
    }
    for r in own.rows.iter().filter(|r| r.off) {
        let (s, n) = (r.midi.status, r.midi.number);
        let found = layout.as_ref().and_then(|l| l.find(s, n));
        let name = if r.name.is_empty() {
            message_text(s, n)
        } else {
            humanize(&r.name)
        };
        let what = if r.original.is_empty() {
            "Not translated".to_string()
        } else {
            r.original.clone()
        };
        v.rows.push(Row {
            function: Function::new("Controller", what, ""),
            control: found.map_or(name, |(p, _)| p.label.clone()),
            part: found.map(|(p, _)| p.id.clone()),
            layer: r.layer.to_uppercase(),
            kind: found.map_or_else(
                || guess_kind(s, Options::default()).into(),
                |(p, _)| p.kind.clone(),
            ),
            keys: vec![message_key(s, n)],
            midi: message_text(s, n),
            script: None,
            description: r.description.clone(),
            options: String::new(),
            sources: Vec::new(),
            edited: false,
            off: true,
            note: r.note.clone(),
        });
    }
    Ok(v)
}

/// Works out what `mapping` does; `file` is the Mixxx mapping's file name,
/// for its picture.
pub fn build_from(
    mapping: &Mapping,
    file: String,
    registry: Arc<ControlRegistry>,
) -> Result<MappingView, String> {
    let mapping = mapping.clone();
    let layout = Layout::for_file(&file);
    let mut notes = Vec::new();
    let probed = match probe(&mapping, registry.clone()) {
        Ok(p) => p,
        Err(e) => {
            notes.push(format!("The mapping's script didn't start: {e}"));
            Probe::default()
        }
    };
    notes.extend(probed.errors.iter().take(3).cloned());

    let mut messages: Vec<(u8, u8)> = mapping.inputs.keys().copied().collect();
    messages.sort();
    let mut b = Builder {
        mapping: &mapping,
        layout: layout.as_ref(),
        probe: &probed,

        rows: Vec::new(),
    };
    for (s, n) in messages {
        if layout.as_ref().is_some_and(|l| !l.shows_channel(s)) {
            continue;
        }
        if s & 0xF0 == 0x80 && b.mapping.inputs.contains_key(&(0x90 | (s & 0x0F), n)) {
            continue;
        }
        for input in b.mapping.inputs[&(s, n)].clone() {
            b.add(s, n, &input);
        }
    }
    let mut rows = b.rows;
    rows.sort_by(|a, b| {
        a.function
            .order()
            .cmp(&b.function.order())
            .then_with(|| natural(&a.function.target, &b.function.target))
            .then_with(|| natural(&a.function.name, &b.function.name))
            .then_with(|| a.layer.cmp(&b.layer))
            .then_with(|| a.keys.cmp(&b.keys))
    });

    let mut lights: Vec<LightRow> = Vec::new();
    let mut seen: HashSet<(String, MessageKey)> = HashSet::new();
    let mut add_light = |k: &ConfigKey, status: u8, number: u8| {
        if layout.as_ref().is_some_and(|l| !l.shows_channel(status)) {
            return;
        }
        let f = describe(&k.group, &k.item);
        let key = message_key(status, number);
        if !seen.insert((format!("{}{}{}", f.category, f.name, f.target), key)) {
            return;
        }
        let found = layout.as_ref().and_then(|l| l.find(status, number));
        lights.push(LightRow {
            control: found.map_or_else(|| message_text(status, number), |(p, _)| p.label.clone()),
            part: found.map(|(p, _)| p.id.clone()),
            function: f,
            key,
            midi: message_text(status, number),
        });
    };
    for (k, msgs) in &probed.lights {
        for &(s, n) in msgs {
            add_light(k, s, n);
        }
    }
    for o in &mapping.outputs {
        add_light(
            &ConfigKey::new(o.group.clone(), o.key.clone()),
            o.status,
            o.control,
        );
    }
    lights.sort_by(|a, b| {
        a.function
            .order()
            .cmp(&b.function.order())
            .then_with(|| natural(&a.function.target, &b.function.target))
            .then_with(|| natural(&a.function.name, &b.function.name))
            .then_with(|| a.key.cmp(&b.key))
    });

    Ok(MappingView {
        name: mapping.name.clone(),
        author: mapping.author.clone(),
        file,
        rows,
        lights,
        options: mapping.setting_defs.clone(),
        layout,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_rows_read_in_words() {
        let f = script_function(
            "(var 'k2mode' ? deck 1 hot_cue 4 : deck 1 play_pause)",
            "K22D_BUTTON_MNOP_INNER".into(),
        );
        assert_eq!(f.category, "Hot cues");
        assert!(f.name.contains(" / "), "{}", f.name);
        assert_eq!(f.target, "Deck 1");
        let f = script_function("press '[Channel2],hotcue_3_clear'", String::new());
        assert_eq!(f.target, "Deck 2");
        assert_eq!(
            script_function("expand_browser", String::new()).name,
            "Expand browser"
        );
    }

    #[test]
    fn splits_descriptions() {
        assert_eq!(
            split_description("PLAY/PAUSE +SHIFT (DECK1) - press - Reverse playback"),
            ("PLAY/PAUSE (DECK1)".into(), "Reverse playback".into(), true)
        );
        assert_eq!(
            split_description("Sets the decks pregain."),
            ("Sets the decks pregain.".into(), String::new(), false)
        );
    }

    #[test]
    fn names_script_bindings() {
        assert_eq!(
            script_name("MidiFighterTwister.controller.toggleDeck"),
            "Toggle deck"
        );
        assert_eq!(
            script_name("PioneerDDJFLX4.cueLoopCallLeft"),
            "Cue loop call left"
        );
    }

    #[test]
    fn joins_messages() {
        assert_eq!(
            join_messages("CC 13 · ch 1", "CC 33 · ch 1"),
            "CC 13, 33 · ch 1"
        );
        assert_eq!(
            join_messages("Note 47 · ch 5", "Note 47 · ch 6"),
            "Note 47 · ch 5 / Note 47 · ch 6"
        );
    }

    #[test]
    fn sorts_naturally() {
        assert_eq!(natural("Hot cue 2", "Hot cue 10"), Ordering::Less);
        assert_eq!(natural("Deck 1", "Deck 1"), Ordering::Equal);
    }
}
