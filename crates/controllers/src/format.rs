//! DJ Sugar's controller mapping format: one JSON file holding a mapping of
//! the DJ's own or one imported from another DJ app, either standing alone
//! or `based_on` a Mixxx mapping and holding only what it changes. A
//! superset of Mixxx's `.midi.xml`: [`DjSugarMapping::from_mixxx`] loses
//! nothing. The reference is `docs/dev/mapping-format.md`.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::mapping::{Input, Mapping, Options, Output, ScriptFile, When, REQUIRED_SCRIPT};

pub const FORMAT: &str = "djsugar-mapping";
pub const VERSION: u64 = 1;
/// What a shared mapping file is called: `My FLX4.djsugar-mapping`.
pub const EXTENSION: &str = "djsugar-mapping";

/// A MIDI message a row listens to or a light sends: `"90 0B"`, a 14-bit
/// pair `"B0 00+B0 20"` (MSB then LSB), or pitch bend `"E0"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Midi {
    pub status: u8,
    pub number: u8,
    /// The second half of a 14-bit pair.
    pub lsb: Option<(u8, u8)>,
}

impl Midi {
    pub fn new(status: u8, number: u8) -> Self {
        Self {
            status,
            number,
            lsb: None,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let one = |t: &str| -> Option<(u8, u8)> {
            let mut parts = t.split_whitespace();
            let status = u8::from_str_radix(parts.next()?, 16).ok()?;
            let number = match parts.next() {
                Some(n) => u8::from_str_radix(n, 16).ok()?,
                None if status & 0xF0 == 0xE0 => 0,
                None => return None,
            };
            if status < 0x80 || number > 0x7F || parts.next().is_some() {
                return None;
            }
            Some((status, number))
        };
        let (first, second) = match text.split_once('+') {
            Some((a, b)) => (a, Some(b)),
            None => (text, None),
        };
        let (status, number) = one(first)?;
        let lsb = match second {
            Some(b) => Some(one(b)?),
            None => None,
        };
        Some(Self {
            status,
            number,
            lsb,
        })
    }

    pub fn text(&self) -> String {
        let one = |s: u8, n: u8| {
            if s & 0xF0 == 0xE0 {
                format!("{s:02X}")
            } else {
                format!("{s:02X} {n:02X}")
            }
        };
        match self.lsb {
            Some((s, n)) => format!("{}+{}", one(self.status, self.number), one(s, n)),
            None => one(self.status, self.number),
        }
    }

    fn key(&self) -> (u8, u8) {
        (self.status, self.number)
    }
}

/// What a row does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A control, `[Channel1],play`, with Mixxx's option names.
    Bind {
        control: String,
        options: Vec<String>,
    },
    /// A function of the mapping's JavaScript, called with this group.
    Js { function: String, group: String },
    /// An DJSugarScript line.
    Script(String),
    /// Nothing: an `off` or `remove` row.
    Nothing,
}

/// What one MIDI message does on one layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub midi: Midi,
    /// The control's name in the app it came from: `PLAY_PAUSE`.
    pub name: String,
    /// Variables the row needs, as written: `"shift"`, `"m1=2 & m2=0"`.
    pub layer: String,
    pub when: When,
    pub action: Action,
    /// `"jump"`, `"pickup"` or `"scale"`.
    pub takeover: Option<String>,
    /// The part of the control's range the hardware covers, 0 to 1.
    pub range: Option<(f64, f64)>,
    /// Kept but doing nothing, greyed in the editor.
    pub off: bool,
    /// The action as the other app wrote it.
    pub original: String,
    /// Why it's off.
    pub note: String,
    pub description: String,
    /// Takes the base mapping's bindings for this message away.
    pub remove: bool,
    /// A `script` row on a relative encoder (1 a click right, 127 a click
    /// left): the line gets the clicks, -1 or 1 or more, instead of a level.
    pub steps: bool,
    /// The row's deck is where its control sits (a column of a K2), so the
    /// mapping's `deck_order` picks it when used with four decks.
    pub by_position: bool,
    /// Fields this version doesn't know, kept when written back.
    pub extra: Map<String, Value>,
}

impl Row {
    pub fn new(midi: Midi, action: Action) -> Self {
        Self {
            midi,
            name: String::new(),
            layer: String::new(),
            when: When::Press,
            action,
            takeover: None,
            range: None,
            off: false,
            original: String::new(),
            note: String::new(),
            description: String::new(),
            remove: false,
            steps: false,
            by_position: false,
            extra: Map::new(),
        }
    }

    /// A row for something DJ Sugar can't do yet: kept greyed with the other
    /// app's text and why.
    pub fn unsupported(midi: Midi, name: &str, original: &str, note: &str) -> Self {
        Self {
            name: name.to_string(),
            off: true,
            original: original.to_string(),
            note: note.to_string(),
            ..Self::new(midi, Action::Nothing)
        }
    }

    pub fn bind(midi: Midi, control: &str, options: &[&str]) -> Self {
        Self::new(
            midi,
            Action::Bind {
                control: control.to_string(),
                options: options.iter().map(|o| o.to_string()).collect(),
            },
        )
    }

    pub fn script(midi: Midi, line: &str) -> Self {
        Self::new(midi, Action::Script(line.to_string()))
    }

    /// The layer's conditions: `("m1", Some(2.0))`, or `("shift", None)`
    /// for anything above 0.
    pub fn layer_conditions(&self) -> Vec<(String, Option<f64>)> {
        parse_layer(&self.layer)
    }
}

/// `"m1=2 & shift"` → `[("m1", Some(2)), ("shift", None)]`.
pub fn parse_layer(layer: &str) -> Vec<(String, Option<f64>)> {
    layer
        .split('&')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((name, v)) => (name.trim().to_string(), v.trim().parse().ok()),
            None => (p.to_string(), None),
        })
        .collect()
}

/// What drives a light.
#[derive(Clone, Debug, PartialEq)]
pub enum LightSource {
    /// Mixxx's `<output>`: `on` while the control is within `min..=max`.
    Bind {
        control: String,
        min: f64,
        max: f64,
        on: u8,
        off: u8,
    },
    /// Set by the mapping's JavaScript; listed for the editor.
    Js(String),
    /// An DJSugarScript line ending in `on`, `off`, `blink`, `color`, `led`.
    Script(String),
    Nothing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    pub midi: Midi,
    pub name: String,
    pub source: LightSource,
    /// What `color 'name'` sends on this controller.
    pub colors: Vec<(String, u8)>,
    /// The colours are notes above this one, sent at full velocity (Xone
    /// K2), rather than velocities of this note.
    pub color_notes: bool,
    /// As for rows: the light's deck follows `deck_order`.
    pub by_position: bool,
    /// As for rows: a `script` light shows only while the layer holds, and
    /// is off otherwise.
    pub layer: String,
    pub off: bool,
    pub original: String,
    pub note: String,
    pub extra: Map<String, Value>,
}

impl Light {
    pub fn new(midi: Midi, source: LightSource) -> Self {
        Self {
            midi,
            name: String::new(),
            source,
            colors: Vec::new(),
            color_notes: false,
            by_position: false,
            layer: String::new(),
            off: false,
            original: String::new(),
            note: String::new(),
            extra: Map::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControllerInfo {
    pub name: String,
    /// MIDI port names it matches.
    pub ports: Vec<String>,
    /// DJ Sugar's drawing of it: `flx4`, `xone-k2`, `twister`.
    pub layout: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imported {
    /// `VirtualDJ`, `Traktor`, `rekordbox`, `djay Pro`, `Mixxx`.
    pub app: String,
    pub file: String,
    pub date: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub name: String,
    pub default: f64,
    /// Kept across restarts.
    pub saved: bool,
}

/// A mapping in DJ Sugar's format.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DjSugarMapping {
    pub name: String,
    pub author: String,
    pub description: String,
    pub controller: ControllerInfo,
    /// The Mixxx mapping underneath, by file name.
    pub based_on: Option<String>,
    pub imported: Option<Imported>,
    /// JavaScript files as `(file, function prefix)`.
    pub scripts: Vec<(String, String)>,
    /// Mixxx's `<settings>` definitions and the DJ's `values`, as JSON.
    pub settings: Value,
    pub variables: Vec<Variable>,
    /// Whether the controller is used with two decks or four. Rows and
    /// lights on the layer `decks=2` or `decks=4` only work in that use, and
    /// the variable `decks` holds it.
    pub decks: Option<u8>,
    /// With four decks, the deck at each position from left to right:
    /// `[3, 1, 2, 4]` as on DJ Sugar's 4-Deck screen.
    pub deck_order: Option<[u8; 4]>,
    pub rows: Vec<Row>,
    pub lights: Vec<Light>,
    /// In a shared file: script files (and a `based_on` mapping DJ Sugar
    /// doesn't ship) by file name.
    pub sources: BTreeMap<String, String>,
    pub extra: Map<String, Value>,
}

fn take_str(o: &mut Map<String, Value>, key: &str) -> String {
    match o.remove(key) {
        Some(Value::String(s)) => s,
        _ => String::new(),
    }
}

fn take_bool(o: &mut Map<String, Value>, key: &str) -> bool {
    matches!(o.remove(key), Some(Value::Bool(true)))
}

fn put(o: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        o.insert(key.into(), json!(value));
    }
}

impl Row {
    fn from_json(v: &Value) -> Result<Self, String> {
        let mut o = v.as_object().cloned().ok_or("a row isn't an object")?;
        let midi_text = take_str(&mut o, "midi");
        let midi = Midi::parse(&midi_text).ok_or_else(|| format!("bad midi '{midi_text}'"))?;
        let options: Vec<String> = match o.remove("options") {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        let group = take_str(&mut o, "group");
        let action = if let Some(Value::String(c)) = o.remove("bind") {
            Action::Bind {
                control: c,
                options,
            }
        } else if let Some(Value::String(f)) = o.remove("js") {
            Action::Js { function: f, group }
        } else if let Some(Value::String(s)) = o.remove("script") {
            Action::Script(s)
        } else {
            Action::Nothing
        };
        let when_text = take_str(&mut o, "when");
        let when = When::from_name(&when_text).ok_or_else(|| format!("bad when '{when_text}'"))?;
        let takeover = match take_str(&mut o, "takeover") {
            t if t.is_empty() => None,
            t => Some(t),
        };
        let range = match o.remove("range") {
            Some(Value::Array(a)) if a.len() == 2 => {
                Some((a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(1.0)))
            }
            _ => None,
        };
        Ok(Self {
            midi,
            name: take_str(&mut o, "name"),
            layer: take_str(&mut o, "layer"),
            when,
            action,
            takeover,
            range,
            off: take_bool(&mut o, "off"),
            original: take_str(&mut o, "original"),
            note: take_str(&mut o, "note"),
            description: take_str(&mut o, "description"),
            remove: take_bool(&mut o, "remove"),
            steps: take_bool(&mut o, "steps"),
            by_position: take_bool(&mut o, "by_position"),
            extra: o,
        })
    }

    fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("midi".into(), json!(self.midi.text()));
        put(&mut o, "name", &self.name);
        put(&mut o, "layer", &self.layer);
        if self.when != When::Press {
            o.insert("when".into(), json!(self.when.name()));
        }
        match &self.action {
            Action::Bind { control, options } => {
                o.insert("bind".into(), json!(control));
                if !options.is_empty() {
                    o.insert("options".into(), json!(options));
                }
            }
            Action::Js { function, group } => {
                o.insert("js".into(), json!(function));
                put(&mut o, "group", group);
            }
            Action::Script(s) => {
                o.insert("script".into(), json!(s));
            }
            Action::Nothing => {}
        }
        if let Some(t) = &self.takeover {
            o.insert("takeover".into(), json!(t));
        }
        if let Some((a, b)) = self.range {
            o.insert("range".into(), json!([a, b]));
        }
        if self.off {
            o.insert("off".into(), json!(true));
        }
        put(&mut o, "original", &self.original);
        put(&mut o, "note", &self.note);
        put(&mut o, "description", &self.description);
        if self.steps {
            o.insert("steps".into(), json!(true));
        }
        if self.by_position {
            o.insert("by_position".into(), json!(true));
        }
        if self.remove {
            o.insert("remove".into(), json!(true));
        }
        for (k, v) in &self.extra {
            o.entry(k.clone()).or_insert(v.clone());
        }
        Value::Object(o)
    }
}

impl Light {
    fn from_json(v: &Value) -> Result<Self, String> {
        let mut o = v.as_object().cloned().ok_or("a light isn't an object")?;
        let midi_text = take_str(&mut o, "midi");
        let midi = Midi::parse(&midi_text).ok_or_else(|| format!("bad midi '{midi_text}'"))?;
        let num =
            |o: &Map<String, Value>, k: &str, d: f64| o.get(k).and_then(Value::as_f64).unwrap_or(d);
        let source = if let Some(Value::String(c)) = o.remove("bind") {
            let s = LightSource::Bind {
                control: c,
                min: num(&o, "min", 0.5),
                max: num(&o, "max", 1.0),
                on: num(&o, "on_value", 127.0) as u8,
                off: num(&o, "off_value", 0.0) as u8,
            };
            for k in ["min", "max", "on_value", "off_value"] {
                o.remove(k);
            }
            s
        } else if let Some(Value::String(f)) = o.remove("js") {
            LightSource::Js(f)
        } else if let Some(Value::String(s)) = o.remove("script") {
            LightSource::Script(s)
        } else {
            LightSource::Nothing
        };
        let colors = match o.remove("colors") {
            Some(Value::Object(c)) => c
                .iter()
                .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n.min(127) as u8)))
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            midi,
            name: take_str(&mut o, "name"),
            source,
            colors,
            color_notes: take_bool(&mut o, "color_notes"),
            by_position: take_bool(&mut o, "by_position"),
            layer: take_str(&mut o, "layer"),
            off: take_bool(&mut o, "off"),
            original: take_str(&mut o, "original"),
            note: take_str(&mut o, "note"),
            extra: o,
        })
    }

    fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("midi".into(), json!(self.midi.text()));
        put(&mut o, "name", &self.name);
        match &self.source {
            LightSource::Bind {
                control,
                min,
                max,
                on,
                off,
            } => {
                o.insert("bind".into(), json!(control));
                o.insert("min".into(), json!(min));
                o.insert("max".into(), json!(max));
                o.insert("on_value".into(), json!(on));
                o.insert("off_value".into(), json!(off));
            }
            LightSource::Js(f) => {
                o.insert("js".into(), json!(f));
            }
            LightSource::Script(s) => {
                o.insert("script".into(), json!(s));
            }
            LightSource::Nothing => {}
        }
        if !self.colors.is_empty() {
            let c: Map<String, Value> = self
                .colors
                .iter()
                .map(|(k, v)| (k.clone(), json!(v)))
                .collect();
            o.insert("colors".into(), Value::Object(c));
        }
        if self.color_notes {
            o.insert("color_notes".into(), json!(true));
        }
        if self.by_position {
            o.insert("by_position".into(), json!(true));
        }
        put(&mut o, "layer", &self.layer);
        if self.off {
            o.insert("off".into(), json!(true));
        }
        put(&mut o, "original", &self.original);
        put(&mut o, "note", &self.note);
        for (k, v) in &self.extra {
            o.entry(k.clone()).or_insert(v.clone());
        }
        Value::Object(o)
    }
}

/// Splits `[Channel1],play` into its group and key. Groups may hold
/// brackets of their own: `[QuickEffectRack1_[Channel1]],super1`.
pub fn split_control(control: &str) -> Option<(String, String)> {
    let (group, key) = control.split_once("],")?;
    group
        .starts_with('[')
        .then(|| (format!("{group}]"), key.trim().to_string()))
}

/// The variable that holds whether the controller is used with two decks or
/// four (see [`DjSugarMapping::decks`]).
pub const DECKS: &str = "decks";

/// `text` with the decks 1 to 4 it names (`deck 2`, `[Channel2]`, effect
/// unit 2) moved to `order`: deck n becomes `order[n - 1]`.
pub fn reorder_decks(text: &str, order: [u8; 4]) -> String {
    const BEFORE: [&str; 3] = ["deck ", "[Channel", "EffectUnit"];
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        let hit = BEFORE
            .iter()
            .find(|p| text[i..].starts_with(*p))
            .map(|p| p.len());
        if let Some(n) = hit {
            let d = i + n;
            let digit = b.get(d).filter(|c| (b'1'..=b'4').contains(c));
            let alone = !b.get(d + 1).is_some_and(|c| c.is_ascii_digit());
            if let (Some(&c), true) = (digit, alone) {
                out.push_str(&text[i..d]);
                out.push((b'0' + order[(c - b'1') as usize]) as char);
                i = d + 1;
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

impl DjSugarMapping {
    /// The rows and lights for the decks the controller is used with: those
    /// on the other use's layer go, the `decks=` condition goes from the
    /// rest, and with four decks the ones `by_position` follow `deck_order`.
    pub fn for_decks(&self) -> (Vec<Row>, Vec<Light>) {
        let order = match (self.decks, self.deck_order) {
            (Some(4), Some(o)) => Some(o),
            _ => None,
        };
        // The layer without its `decks=` part, or None when it's for the
        // other use.
        let layer_for = |layer: &str| -> Option<String> {
            let Some(d) = self.decks else {
                return Some(layer.to_string());
            };
            let conds = parse_layer(layer);
            if conds
                .iter()
                .any(|(n, v)| n == DECKS && v.is_some_and(|v| v != d as f64))
            {
                return None;
            }
            Some(
                conds
                    .into_iter()
                    .filter(|(n, _)| n != DECKS)
                    .map(|(n, v)| match v {
                        Some(v) => format!("{n}={v}"),
                        None => n,
                    })
                    .collect::<Vec<_>>()
                    .join(" & "),
            )
        };
        let rows = self
            .rows
            .iter()
            .filter_map(|r| {
                let mut r = r.clone();
                r.layer = layer_for(&r.layer)?;
                if let (Some(o), true) = (order, r.by_position) {
                    r.action = match r.action {
                        Action::Bind { control, options } => Action::Bind {
                            control: reorder_decks(&control, o),
                            options,
                        },
                        Action::Script(line) => Action::Script(reorder_decks(&line, o)),
                        a => a,
                    };
                }
                Some(r)
            })
            .collect();
        let lights = self
            .lights
            .iter()
            .filter_map(|l| {
                let mut l = l.clone();
                l.layer = layer_for(&l.layer)?;
                if !l.layer.is_empty() {
                    if let LightSource::Script(line) = &l.source {
                        let conds: Vec<String> = parse_layer(&l.layer)
                            .into_iter()
                            .map(|(n, v)| match v {
                                Some(v) => format!("var '{n}' {v}"),
                                None => format!("var '{n}'"),
                            })
                            .collect();
                        l.source =
                            LightSource::Script(format!("({} ? {line} : off)", conds.join(" & ")));
                    }
                    l.layer.clear();
                }
                if let (Some(o), true) = (order, l.by_position) {
                    l.source = match l.source {
                        LightSource::Bind {
                            control,
                            min,
                            max,
                            on,
                            off,
                        } => LightSource::Bind {
                            control: reorder_decks(&control, o),
                            min,
                            max,
                            on,
                            off,
                        },
                        LightSource::Script(line) => LightSource::Script(reorder_decks(&line, o)),
                        s => s,
                    };
                }
                Some(l)
            })
            .collect();
        (rows, lights)
    }

    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Reads a mapping, including the files DJ Sugar 0.4 wrote (`name`,
    /// `based_on`, `changes`, no `format`).
    pub fn from_json(v: &Value) -> Result<Self, String> {
        let mut o = v.as_object().cloned().ok_or("not a JSON object")?;
        match o.remove("format") {
            Some(Value::String(f)) if f == FORMAT => {}
            None => {}
            Some(_) => return Err("not an DJ Sugar mapping".into()),
        }
        let version = o.remove("version").and_then(|v| v.as_u64()).unwrap_or(1);
        if version > VERSION {
            return Err(format!(
                "made by a newer DJ Sugar (format version {version}); update DJ Sugar to open it"
            ));
        }
        let mut rows = Vec::new();
        for key in ["rows", "changes"] {
            if let Some(Value::Array(a)) = o.remove(key) {
                for r in &a {
                    rows.push(Row::from_json(r)?);
                }
            }
        }
        let mut lights = Vec::new();
        if let Some(Value::Array(a)) = o.remove("lights") {
            for l in &a {
                lights.push(Light::from_json(l)?);
            }
        }
        let controller = match o.remove("controller") {
            Some(Value::Object(mut c)) => ControllerInfo {
                name: take_str(&mut c, "name"),
                ports: c
                    .get("ports")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| p.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                layout: c.get("layout").and_then(Value::as_str).map(str::to_string),
            },
            _ => ControllerInfo::default(),
        };
        let imported = match o.remove("imported") {
            Some(Value::Object(mut i)) => Some(Imported {
                app: take_str(&mut i, "app"),
                file: take_str(&mut i, "file"),
                date: take_str(&mut i, "date"),
            }),
            _ => None,
        };
        let scripts = match o.remove("scripts") {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|s| {
                    Some((
                        s["file"].as_str()?.to_string(),
                        s["prefix"].as_str().unwrap_or("").to_string(),
                    ))
                })
                .collect(),
            _ => Vec::new(),
        };
        let variables = match o.remove("variables") {
            Some(Value::Object(vars)) => vars
                .iter()
                .map(|(name, v)| Variable {
                    name: name.clone(),
                    default: v["default"].as_f64().unwrap_or(0.0),
                    saved: v["saved"].as_bool().unwrap_or(false),
                })
                .collect(),
            _ => Vec::new(),
        };
        let sources = match o.remove("sources") {
            Some(Value::Object(s)) => s
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect(),
            _ => BTreeMap::new(),
        };
        let based_on = match take_str(&mut o, "based_on") {
            b if b.is_empty() => None,
            b => Some(b),
        };
        Ok(Self {
            name: take_str(&mut o, "name"),
            author: take_str(&mut o, "author"),
            description: take_str(&mut o, "description"),
            controller,
            based_on,
            imported,
            scripts,
            settings: o.remove("settings").unwrap_or(Value::Null),
            variables,
            decks: o
                .remove("decks")
                .and_then(|d| d.as_u64())
                .filter(|d| *d == 2 || *d == 4)
                .map(|d| d as u8),
            deck_order: o.remove("deck_order").and_then(|d| {
                let a: Vec<u8> = d
                    .as_array()?
                    .iter()
                    .filter_map(|x| x.as_u64())
                    .map(|x| x as u8)
                    .collect();
                let mut sorted = a.clone();
                sorted.sort_unstable();
                (sorted == [1, 2, 3, 4]).then(|| [a[0], a[1], a[2], a[3]])
            }),
            rows,
            lights,
            sources,
            extra: o,
        })
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("format".into(), json!(FORMAT));
        o.insert("version".into(), json!(VERSION));
        o.insert("name".into(), json!(self.name));
        put(&mut o, "author", &self.author);
        put(&mut o, "description", &self.description);
        let c = &self.controller;
        if !c.name.is_empty() || !c.ports.is_empty() || c.layout.is_some() {
            let mut m = Map::new();
            put(&mut m, "name", &c.name);
            if !c.ports.is_empty() {
                m.insert("ports".into(), json!(c.ports));
            }
            if let Some(l) = &c.layout {
                m.insert("layout".into(), json!(l));
            }
            o.insert("controller".into(), Value::Object(m));
        }
        if let Some(b) = &self.based_on {
            o.insert("based_on".into(), json!(b));
        }
        if let Some(i) = &self.imported {
            let mut m = Map::new();
            put(&mut m, "app", &i.app);
            put(&mut m, "file", &i.file);
            put(&mut m, "date", &i.date);
            o.insert("imported".into(), Value::Object(m));
        }
        if !self.scripts.is_empty() {
            let s: Vec<Value> = self
                .scripts
                .iter()
                .map(|(f, p)| json!({ "file": f, "prefix": p }))
                .collect();
            o.insert("scripts".into(), json!(s));
        }
        if !self.settings.is_null() {
            o.insert("settings".into(), self.settings.clone());
        }
        if !self.variables.is_empty() {
            let v: Map<String, Value> = self
                .variables
                .iter()
                .map(|v| {
                    (
                        v.name.clone(),
                        json!({ "default": v.default, "saved": v.saved }),
                    )
                })
                .collect();
            o.insert("variables".into(), Value::Object(v));
        }
        if let Some(d) = self.decks {
            o.insert("decks".into(), json!(d));
        }
        if let Some(order) = self.deck_order {
            o.insert("deck_order".into(), json!(order));
        }
        o.insert(
            "rows".into(),
            Value::Array(self.rows.iter().map(Row::to_json).collect()),
        );
        if !self.lights.is_empty() {
            o.insert(
                "lights".into(),
                Value::Array(self.lights.iter().map(Light::to_json).collect()),
            );
        }
        if !self.sources.is_empty() {
            o.insert("sources".into(), json!(self.sources));
        }
        for (k, v) in &self.extra {
            o.entry(k.clone()).or_insert(v.clone());
        }
        Value::Object(o)
    }

    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Self::from_json(&v)
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&self.to_json())?)
    }

    /// A Mixxx mapping in this format, with nothing lost.
    pub fn from_mixxx(m: &Mapping) -> Self {
        let mut keys: Vec<(u8, u8)> = m.inputs.keys().copied().collect();
        keys.sort();
        let mut rows: Vec<Row> = Vec::new();
        // The other halves of 14-bit pairs already written.
        let mut paired: Vec<((u8, u8), String, String)> = Vec::new();
        for key in keys {
            for input in &m.inputs[&key] {
                let o = input.options;
                let mut midi = Midi::new(key.0, key.1);
                if o.has(Options::FOURTEEN_BIT_LSB) || o.has(Options::FOURTEEN_BIT_MSB) {
                    let id = (key, input.group.clone(), input.key.clone());
                    if paired.contains(&id) {
                        continue;
                    }
                    let want_lsb = o.has(Options::FOURTEEN_BIT_MSB);
                    let other = m.inputs.iter().find_map(|(k, list)| {
                        list.iter()
                            .find(|i| {
                                i.group == input.group
                                    && i.key == input.key
                                    && if want_lsb {
                                        i.options.has(Options::FOURTEEN_BIT_LSB)
                                    } else {
                                        i.options.has(Options::FOURTEEN_BIT_MSB)
                                    }
                            })
                            .map(|_| *k)
                    });
                    if let Some(k) = other {
                        paired.push((k, input.group.clone(), input.key.clone()));
                        midi = if want_lsb {
                            Midi {
                                lsb: Some(k),
                                ..midi
                            }
                        } else {
                            Midi {
                                status: k.0,
                                number: k.1,
                                lsb: Some(key),
                            }
                        };
                    }
                }
                let names: Vec<String> = Options::new(
                    o.bits()
                        & !(Options::SCRIPT
                            | Options::FOURTEEN_BIT_MSB
                            | Options::FOURTEEN_BIT_LSB),
                )
                .names()
                .into_iter()
                .map(str::to_string)
                .collect();
                let action = if o.has(Options::SCRIPT) {
                    Action::Js {
                        function: input.key.clone(),
                        group: input.group.clone(),
                    }
                } else {
                    Action::Bind {
                        control: format!("{},{}", input.group, input.key),
                        options: names,
                    }
                };
                let mut row = Row::new(midi, action);
                row.description = input.description.clone();
                rows.push(row);
            }
        }
        let lights = m
            .outputs
            .iter()
            .map(|o| {
                Light::new(
                    Midi::new(o.status, o.control),
                    LightSource::Bind {
                        control: format!("{},{}", o.group, o.key),
                        min: o.min,
                        max: o.max,
                        on: o.on,
                        off: o.off,
                    },
                )
            })
            .collect();
        let settings: Vec<Value> = m
            .setting_defs
            .iter()
            .map(|d| {
                json!({
                    "variable": d.variable,
                    "type": d.kind,
                    "label": d.label,
                    "default": d.default,
                    "choices": d.choices,
                    "description": d.description,
                })
            })
            .collect();
        let file = m
            .path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            name: m.name.clone(),
            author: m.author.clone(),
            imported: Some(Imported {
                app: "Mixxx".into(),
                file,
                date: String::new(),
            }),
            scripts: m
                .scripts
                .iter()
                .filter(|s| !s.path.ends_with(REQUIRED_SCRIPT))
                .map(|s| {
                    (
                        s.path
                            .file_name()
                            .map(|f| f.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        s.prefix.clone(),
                    )
                })
                .collect(),
            settings: if settings.is_empty() {
                Value::Null
            } else {
                json!({ "options": settings })
            },
            rows,
            lights,
            ..Default::default()
        }
    }

    /// The mapping as a controller runs it: `base` (the `based_on` Mixxx
    /// mapping, loaded by the caller) with the rows and lights put over it.
    /// Script files are looked up in `scripts_dir`.
    pub fn build(&self, base: Option<&Mapping>, scripts_dir: &Path) -> Mapping {
        let mut m = match base {
            Some(b) => b.clone(),
            None => Mapping {
                name: String::new(),
                author: String::new(),
                path: Default::default(),
                scripts: Vec::new(),
                settings: Vec::new(),
                setting_defs: Vec::new(),
                inputs: Default::default(),
                outputs: Vec::new(),
                products: Vec::new(),
                variables: Vec::new(),
            },
        };
        m.name = self.name.clone();
        if !self.author.is_empty() {
            m.author = self.author.clone();
        }
        for (file, prefix) in &self.scripts {
            if m.scripts.is_empty() {
                m.scripts.push(ScriptFile {
                    path: scripts_dir.join(REQUIRED_SCRIPT),
                    prefix: String::new(),
                });
            }
            let path = scripts_dir.join(file);
            if !m.scripts.iter().any(|s| s.path == path) {
                m.scripts.push(ScriptFile {
                    path,
                    prefix: prefix.clone(),
                });
            }
        }
        for v in &self.variables {
            m.variables.retain(|(n, _)| n != &v.name);
            m.variables.push((v.name.clone(), v.default));
        }
        if let Some(d) = self.decks {
            m.variables.retain(|(n, _)| n != DECKS);
            m.variables.push((DECKS.into(), d as f64));
        }
        let (rows, lights) = self.for_decks();

        // First the base's bindings the rows replace go, so rows of this
        // file sharing a message all stay.
        let own_messages: Vec<(u8, u8)> = rows.iter().map(|r| r.midi.key()).collect();
        for row in rows.iter().filter(|r| !r.off) {
            let layer = row.layer_conditions();
            let mut keys = vec![row.midi.key()];
            keys.extend(row.midi.lsb);
            // A note-on row also takes over the base's note-off binding of
            // the same note, unless the file has a row of its own for it.
            if row.midi.status & 0xF0 == 0x90 {
                let off = (0x80 | (row.midi.status & 0x0F), row.midi.number);
                if !own_messages.contains(&off) {
                    keys.push(off);
                }
            }
            // An import says everything the message does on every layer, so
            // the base's bindings of it all go; the DJ's own rows replace
            // only the layer and press they're on.
            let whole = self.imported.is_some();
            for k in &keys {
                if let Some(list) = m.inputs.get_mut(k) {
                    list.retain(|i| !whole && !(i.layer == layer && i.when == row.when));
                    if list.is_empty() {
                        m.inputs.remove(k);
                    }
                }
            }
        }
        for row in rows.iter().filter(|r| !r.off && !r.remove) {
            let layer = row.layer_conditions();
            let base_input = Input {
                layer: layer.clone(),
                when: row.when,
                name: row.name.clone(),
                description: row.description.clone(),
                ..Default::default()
            };
            match &row.action {
                Action::Bind { control, options } => {
                    let Some((group, key)) = split_control(control) else {
                        continue;
                    };
                    let mut bits = Options::from_names(options.iter().map(String::as_str)).bits();
                    if row.takeover.as_deref() == Some("pickup") {
                        bits |= Options::SOFT_TAKEOVER;
                    }
                    let input = Input {
                        group,
                        key,
                        ..base_input
                    };
                    match row.midi.lsb {
                        Some(lsb) => {
                            push(
                                &mut m,
                                row.midi.key(),
                                Input {
                                    options: Options::new(bits | Options::FOURTEEN_BIT_MSB),
                                    ..input.clone()
                                },
                            );
                            push(
                                &mut m,
                                lsb,
                                Input {
                                    options: Options::new(bits | Options::FOURTEEN_BIT_LSB),
                                    ..input
                                },
                            );
                        }
                        None => push(
                            &mut m,
                            row.midi.key(),
                            Input {
                                options: Options::new(bits),
                                ..input
                            },
                        ),
                    }
                }
                Action::Js { function, group } => push(
                    &mut m,
                    row.midi.key(),
                    Input {
                        group: group.clone(),
                        key: function.clone(),
                        options: Options::new(Options::SCRIPT),
                        ..base_input
                    },
                ),
                Action::Script(line) => {
                    let input = Input {
                        action: Some(line.clone()),
                        options: if row.steps {
                            Options::new(Options::SELECT_KNOB)
                        } else {
                            Options::default()
                        },
                        ..base_input
                    };
                    match row.midi.lsb {
                        Some(lsb) => {
                            push(
                                &mut m,
                                row.midi.key(),
                                Input {
                                    options: Options::new(Options::FOURTEEN_BIT_MSB),
                                    ..input.clone()
                                },
                            );
                            push(
                                &mut m,
                                lsb,
                                Input {
                                    options: Options::new(Options::FOURTEEN_BIT_LSB),
                                    ..input
                                },
                            );
                        }
                        None => {
                            // A controller that lets go with a Note Off
                            // (the Xone K2) sends it on its own status: the
                            // row sees it as the release, for SHIFT and for
                            // long and tap presses.
                            let off = (0x80 | (row.midi.status & 0x0F), row.midi.number);
                            if row.midi.status & 0xF0 == 0x90 && !own_messages.contains(&off) {
                                push(&mut m, off, input.clone());
                            }
                            push(&mut m, row.midi.key(), input)
                        }
                    }
                }
                Action::Nothing => {}
            }
        }

        for light in lights.iter().filter(|l| !l.off) {
            let key = light.midi.key();
            if !matches!(light.source, LightSource::Js(_) | LightSource::Nothing) {
                m.outputs.retain(|o| (o.status, o.control) != key);
            }
        }
        for light in lights.iter().filter(|l| !l.off) {
            let (s, n) = light.midi.key();
            let output = match &light.source {
                LightSource::Bind {
                    control,
                    min,
                    max,
                    on,
                    off,
                } => {
                    let Some((group, key)) = split_control(control) else {
                        continue;
                    };
                    Output {
                        group,
                        key,
                        status: s,
                        control: n,
                        on: *on,
                        off: *off,
                        min: *min,
                        max: *max,
                        ..Default::default()
                    }
                }
                LightSource::Script(line) => Output {
                    status: s,
                    control: n,
                    on: 0x7F,
                    script: Some(line.clone()),
                    colors: light.colors.clone(),
                    color_notes: light.color_notes,
                    ..Default::default()
                },
                LightSource::Js(_) | LightSource::Nothing => continue,
            };
            m.outputs.push(output);
        }
        m
    }

    /// How many rows work and how many are greyed, for an import report.
    pub fn counts(&self) -> (usize, usize) {
        let off = self.rows.iter().filter(|r| r.off).count()
            + self.lights.iter().filter(|l| l.off).count();
        (self.rows.len() + self.lights.len() - off, off)
    }
}

fn push(m: &mut Mapping, key: (u8, u8), input: Input) {
    m.inputs.entry(key).or_default().push(input);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_text_round_trips() {
        for t in ["90 0B", "B0 00+B0 20", "E1"] {
            assert_eq!(Midi::parse(t).unwrap().text(), t);
        }
        assert_eq!(Midi::parse("b0 13"), Some(Midi::new(0xB0, 0x13)));
        assert!(Midi::parse("90").is_none());
        assert!(Midi::parse("12 34").is_none());
        assert!(Midi::parse("90 80").is_none());
    }

    /// A K2 lets go of a button with a Note Off: a row on the Note On hears
    /// it as the release.
    #[test]
    fn note_rows_hear_their_note_off() {
        let mut m = DjSugarMapping::new("K2");
        let mut up = Row::script(Midi::new(0x9E, 0x0F), "set_var 'shift' 0");
        up.when = When::Release;
        m.rows = vec![Row::script(Midi::new(0x9E, 0x0F), "set_var 'shift' 1"), up];
        let built = m.build(None, Path::new("."));
        assert_eq!(built.inputs[&(0x9E, 0x0F)].len(), 2);
        assert_eq!(built.inputs[&(0x8E, 0x0F)].len(), 2);
    }

    #[test]
    fn layers_parse() {
        assert_eq!(
            parse_layer("m1=2 & shift"),
            vec![("m1".to_string(), Some(2.0)), ("shift".to_string(), None)]
        );
        assert!(parse_layer("").is_empty());
    }

    #[test]
    fn splits_controls() {
        assert_eq!(
            split_control("[QuickEffectRack1_[Channel1]],super1"),
            Some(("[QuickEffectRack1_[Channel1]]".into(), "super1".into()))
        );
        assert_eq!(
            split_control("[EqualizerRack1_[Channel2]_Effect1],parameter3"),
            Some((
                "[EqualizerRack1_[Channel2]_Effect1]".into(),
                "parameter3".into()
            ))
        );
        assert_eq!(split_control("play"), None);
    }

    #[test]
    fn reads_djsugar_0_4_files() {
        let v =
            json!({ "name": "FLX4 house", "based_on": "Pioneer-DDJ-FLX4.midi.xml", "changes": [] });
        let m = DjSugarMapping::from_json(&v).unwrap();
        assert_eq!(m.name, "FLX4 house");
        assert_eq!(m.based_on.as_deref(), Some("Pioneer-DDJ-FLX4.midi.xml"));
        assert!(m.rows.is_empty());
        assert!(DjSugarMapping::from_json(&json!({ "format": "x" })).is_err());
        assert!(DjSugarMapping::from_json(&json!({ "format": FORMAT, "version": 9 })).is_err());
    }

    #[test]
    fn rows_round_trip_and_keep_unknown_fields() {
        let v = json!({
            "format": FORMAT, "version": 1, "name": "Test",
            "variables": { "k2mode": { "default": 0.0, "saved": true } },
            "rows": [
                { "midi": "97 04", "name": "HOTCUE_PAD5", "script": "deck 1 play_pause" },
                { "midi": "B0 13+B0 33", "bind": "[Channel1],volume", "takeover": "pickup" },
                { "midi": "90 58", "when": "long", "layer": "shift", "script": "master_deck", "future": 3 },
                { "midi": "B0 21", "off": true, "original": "touchwheel", "note": "no verb yet" }
            ],
            "lights": [ { "midi": "97 04", "script": "deck 1 play ? on : off", "colors": { "red": 15 } } ],
            "later": { "a": 1 }
        });
        let m = DjSugarMapping::from_json(&v).unwrap();
        assert_eq!(m.rows.len(), 4);
        assert_eq!(m.rows[2].when, When::Long);
        assert_eq!(m.counts(), (4, 1));
        let again = DjSugarMapping::from_json(&m.to_json()).unwrap();
        assert_eq!(again, m);
        assert_eq!(m.to_json()["later"]["a"], 1);
        assert_eq!(m.to_json()["rows"][2]["future"], 3);
    }

    #[test]
    fn rows_go_over_the_base_mapping() {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        let base = Mapping::load(&dir.join("Pioneer-DDJ-FLX4.midi.xml")).unwrap();
        let play = (0x90, 0x0B);
        assert!(base.inputs.contains_key(&play));
        let mut own = DjSugarMapping::new("Mine");
        own.based_on = Some("Pioneer-DDJ-FLX4.midi.xml".into());
        own.rows
            .push(Row::script(Midi::new(0x90, 0x0B), "deck 1 sync"));
        own.rows.push(Row::unsupported(
            Midi::new(0x90, 0x0C),
            "CUE",
            "pioneer_cue",
            "no verb yet",
        ));
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&play];
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].action.as_deref(), Some("deck 1 sync"));
        // The greyed row leaves the base binding working.
        assert_eq!(
            built.inputs[&(0x90, 0x0C)].len(),
            base.inputs[&(0x90, 0x0C)].len()
        );
        assert_eq!(built.name, "Mine");
    }

    #[test]
    fn a_mixxx_mapping_converts_and_builds_back_the_same() {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        for file in [
            "Pioneer-DDJ-FLX4.midi.xml",
            "Allen and Heath Xone K2.midi.xml",
            "DJ TechTools MIDI Fighter Twister.midi.xml",
        ] {
            let mixxx = Mapping::load(&dir.join(file)).unwrap();
            let own = DjSugarMapping::from_mixxx(&mixxx);
            let again = DjSugarMapping::from_json(&own.to_json()).unwrap();
            let built = again.build(None, &dir);
            let count = |m: &Mapping| m.inputs.values().map(Vec::len).sum::<usize>();
            assert_eq!(count(&built), count(&mixxx), "{file}");
            for (k, list) in &mixxx.inputs {
                for i in list {
                    assert!(
                        built.inputs[k].iter().any(|b| b.group == i.group
                            && b.key == i.key
                            && b.options == i.options),
                        "{file} {k:02X?} {}",
                        i.key
                    );
                }
            }
            assert_eq!(built.outputs.len(), mixxx.outputs.len(), "{file}");
            assert_eq!(built.scripts.len(), mixxx.scripts.len(), "{file}");
        }
    }
}
