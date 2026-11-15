//! Reads Mixxx's `.midi.xml` controller mappings: script files, settings,
//! input bindings and LED outputs, following Mixxx's
//! `LegacyMidiControllerMappingFileHandler`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Loaded before a mapping's own scripts, as Mixxx does.
pub const REQUIRED_SCRIPT: &str = "common-controller-scripts.js";

/// How an incoming value is interpreted (`<options>` in the mapping).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options(u32);

impl Options {
    pub fn new(bits: u32) -> Self {
        Options(bits)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    /// Mixxx's names for the flags set, as written in `<options>`.
    pub fn names(self) -> Vec<&'static str> {
        NAMES
            .iter()
            .filter(|(_, flag)| self.has(*flag))
            .map(|(n, _)| *n)
            .collect()
    }

    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        Options(names.into_iter().fold(0, |acc, n| acc | Self::from_name(n)))
    }

    pub const INVERT: u32 = 1 << 0;
    pub const ROT64: u32 = 1 << 1;
    pub const ROT64_INV: u32 = 1 << 2;
    pub const ROT64_FAST: u32 = 1 << 3;
    pub const DIFF: u32 = 1 << 4;
    pub const BUTTON: u32 = 1 << 5;
    pub const SWITCH: u32 = 1 << 6;
    pub const HERC_JOG: u32 = 1 << 7;
    pub const HERC_JOG_FAST: u32 = 1 << 8;
    pub const SPREAD64: u32 = 1 << 9;
    pub const SELECT_KNOB: u32 = 1 << 10;
    pub const SOFT_TAKEOVER: u32 = 1 << 11;
    pub const SCRIPT: u32 = 1 << 12;
    pub const FOURTEEN_BIT_MSB: u32 = 1 << 13;
    pub const FOURTEEN_BIT_LSB: u32 = 1 << 14;

    pub fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn from_name(name: &str) -> u32 {
        match name.to_ascii_lowercase().as_str() {
            "invert" => Self::INVERT,
            "rot64" => Self::ROT64,
            "rot64inv" => Self::ROT64_INV,
            "rot64fast" => Self::ROT64_FAST,
            "diff" => Self::DIFF,
            "button" => Self::BUTTON,
            "switch" => Self::SWITCH,
            "hercjog" => Self::HERC_JOG,
            "hercjogfast" => Self::HERC_JOG_FAST,
            "spread64" => Self::SPREAD64,
            "selectknob" => Self::SELECT_KNOB,
            "soft-takeover" => Self::SOFT_TAKEOVER,
            "script-binding" => Self::SCRIPT,
            "fourteen-bit-msb" => Self::FOURTEEN_BIT_MSB,
            "fourteen-bit-lsb" => Self::FOURTEEN_BIT_LSB,
            // "normal" means no options.
            _ => 0,
        }
    }
}

/// Mixxx's option names, in the spelling its own mappings use.
const NAMES: [(&str, u32); 15] = [
    ("invert", Options::INVERT),
    ("rot64", Options::ROT64),
    ("rot64inv", Options::ROT64_INV),
    ("rot64fast", Options::ROT64_FAST),
    ("diff", Options::DIFF),
    ("button", Options::BUTTON),
    ("switch", Options::SWITCH),
    ("hercjog", Options::HERC_JOG),
    ("hercjogfast", Options::HERC_JOG_FAST),
    ("spread64", Options::SPREAD64),
    ("selectknob", Options::SELECT_KNOB),
    ("soft-takeover", Options::SOFT_TAKEOVER),
    ("Script-Binding", Options::SCRIPT),
    ("fourteen-bit-msb", Options::FOURTEEN_BIT_MSB),
    ("fourteen-bit-lsb", Options::FOURTEEN_BIT_LSB),
];

/// When a row of an DJ Sugar mapping acts, for buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum When {
    /// On press (and, for a fader or knob, on every move).
    #[default]
    Press,
    Release,
    /// Held for [`LONG_PRESS`].
    Long,
    /// Let go before [`LONG_PRESS`].
    Tap,
    /// Pressed twice within [`DOUBLE_PRESS`].
    Double,
}

/// How long a press is held to count as long.
pub const LONG_PRESS: std::time::Duration = std::time::Duration::from_millis(500);
/// How quickly two presses follow to count as a double press.
pub const DOUBLE_PRESS: std::time::Duration = std::time::Duration::from_millis(300);

impl When {
    pub fn name(self) -> &'static str {
        match self {
            When::Press => "press",
            When::Release => "release",
            When::Long => "long",
            When::Tap => "tap",
            When::Double => "double",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "press" | "" => When::Press,
            "release" => When::Release,
            "long" => When::Long,
            "tap" => When::Tap,
            "double" => When::Double,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub group: String,
    /// A control name, or for script bindings a JS expression naming a function.
    pub key: String,
    pub options: Options,
    /// What the mapping's author wrote about it, if anything.
    pub description: String,
    /// DJ Sugar mappings only: an DJSugarScript line to run instead of `key`.
    pub action: Option<String>,
    /// DJ Sugar mappings only: variables that must be set for the row to act,
    /// as `(name, value)`; a value of `None` means anything above 0.
    pub layer: Vec<(String, Option<f64>)>,
    pub when: When,
    /// The control's name in the app the row came from (`PLAY_PAUSE`).
    pub name: String,
}

/// Lights an LED while a control's value is within `min..=max`, or, in an
/// DJ Sugar mapping, as an DJSugarScript line says.
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub group: String,
    pub key: String,
    pub status: u8,
    pub control: u8,
    pub on: u8,
    pub off: u8,
    pub min: f64,
    pub max: f64,
    pub description: String,
    /// DJ Sugar mappings only: a line ending in `on`, `off`, `blink`,
    /// `color` or `led`, asked several times a second.
    pub script: Option<String>,
    /// What `color 'name'` sends.
    pub colors: Vec<(String, u8)>,
    /// The colours are added to the note, sent at full velocity, as on the
    /// Xone K2 (red on the note, amber 36 above, green 72 above).
    pub color_notes: bool,
}

/// A mapping setting's value, typed as Mixxx hands it to `engine.getSetting`.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

/// One of a mapping's own settings as the mapping describes it, for showing
/// it: its label, help text, kind and default.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingDef {
    pub variable: String,
    pub label: String,
    pub description: String,
    /// "boolean", "integer", "real", "enum" or "string".
    pub kind: String,
    /// The default as shown: "On", "Red", "4".
    pub default: String,
    /// An enum's choices, by label.
    pub choices: Vec<String>,
}

#[derive(Clone)]
pub struct ScriptFile {
    pub path: PathBuf,
    pub prefix: String,
}

#[derive(Clone)]
pub struct Mapping {
    pub name: String,
    pub author: String,
    pub path: PathBuf,
    pub scripts: Vec<ScriptFile>,
    pub settings: Vec<(String, SettingValue)>,
    /// The same settings with their labels, for showing them.
    pub setting_defs: Vec<SettingDef>,
    /// Keyed by `(status, midino)`; several bindings may share a key.
    pub inputs: HashMap<(u8, u8), Vec<Input>>,
    pub outputs: Vec<Output>,
    /// DJ Sugar mappings only: variables for DJSugarScript rows and lights,
    /// with their starting values.
    pub variables: Vec<(String, f64)>,
    /// The USB HID devices an HID mapping (`.hid.xml`) is for, from
    /// `<info><devices>`. Empty for MIDI mappings.
    pub products: Vec<crate::hid::Product>,
}

/// Parses `0x7F`, `127` or `0177` the way Qt's `toInt(&ok, 0)` does.
fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()
    } else if s.len() > 1 && s.starts_with('0') {
        i64::from_str_radix(&s[1..], 8).ok()
    } else {
        s.parse().ok()
    }
}

fn byte(s: Option<&str>, default: u8) -> u8 {
    s.and_then(parse_int)
        .map(|v| (v & 0xFF) as u8)
        .unwrap_or(default)
}

/// Parses XML, allowing a DTD: some mappings (the Behringer CMD MM-1) name
/// their MIDI numbers with entities declared in one.
fn parse_xml(xml: &str) -> Result<roxmltree::Document<'_>, roxmltree::Error> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    roxmltree::Document::parse_with_options(xml, options)
}

fn child<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name(name))
}

fn text<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    child(node, name).map(|n| n.text().unwrap_or("").trim())
}

/// Whether `path` is a mapping in DJ Sugar's own format that ships with
/// DJ Sugar, rather than a Mixxx `.midi.xml`.
pub fn is_djsugar_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e == crate::format::EXTENSION)
}

impl Mapping {
    /// Whether this is an HID mapping: it names USB HID devices, and its
    /// scripts read and write HID reports rather than MIDI messages.
    pub fn is_hid(&self) -> bool {
        !self.products.is_empty()
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        if is_djsugar_file(path) {
            return Self::load_djsugar(path);
        }
        let xml = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut m = Self::parse(&xml, path.parent().unwrap_or(Path::new(".")))?;
        m.path = path.to_path_buf();
        Ok(m)
    }

    /// One of DJ Sugar's own mappings that ships with it, in DJ Sugar's format
    /// (`.djsugar-mapping`): put over the Mixxx mapping it names, if any,
    /// with its scripts beside it.
    fn load_djsugar(path: &Path) -> Result<Self, String> {
        let mine = crate::format::DjSugarMapping::read(path)?;
        let dir = path.parent().unwrap_or(Path::new("."));
        let base = match &mine.based_on {
            Some(b) if dir.join(b) != path => Some(Self::load(&dir.join(b))?),
            _ => None,
        };
        let mut m = mine.build(base.as_ref(), dir);
        m.path = path.to_path_buf();
        Ok(m)
    }

    /// Reads just the `<info><name>` of a mapping, for matching devices
    /// without parsing every file in full.
    pub fn read_name(path: &Path) -> Option<String> {
        if is_djsugar_file(path) {
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
            return v["name"].as_str().map(str::to_string);
        }
        let xml = std::fs::read_to_string(path).ok()?;
        let doc = parse_xml(&xml).ok()?;
        let info = child(doc.root_element(), "info")?;
        text(info, "name").map(str::to_string)
    }

    /// Parses mapping XML; script files are looked up in `dir`.
    pub fn parse(xml: &str, dir: &Path) -> Result<Self, String> {
        let doc = parse_xml(xml).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let info = child(root, "info");
        let name = info.and_then(|i| text(i, "name")).unwrap_or("").to_string();
        let author = info
            .and_then(|i| text(i, "author"))
            .unwrap_or("")
            .to_string();
        let products = info
            .and_then(|i| child(i, "devices"))
            .map(|d| {
                d.children()
                    .filter(|n| n.has_tag_name("product"))
                    .filter_map(|p| crate::hid::Product::from_attributes(|name| p.attribute(name)))
                    .collect()
            })
            .unwrap_or_default();

        let controller = child(root, "controller").ok_or("no <controller> element")?;
        let mut scripts = vec![ScriptFile {
            path: dir.join(REQUIRED_SCRIPT),
            prefix: String::new(),
        }];
        if let Some(files) = child(controller, "scriptfiles") {
            for f in files.children().filter(|n| n.has_tag_name("file")) {
                let file = f.attribute("filename").unwrap_or("");
                if file.is_empty() || file.ends_with(".qml") {
                    continue;
                }
                scripts.push(ScriptFile {
                    path: dir.join(file),
                    prefix: f.attribute("functionprefix").unwrap_or("").to_string(),
                });
            }
        }

        let options: Vec<roxmltree::Node> = child(root, "settings")
            .map(|s| {
                s.descendants()
                    .filter(|n| n.has_tag_name("option"))
                    .collect()
            })
            .unwrap_or_default();
        let settings = options.iter().filter_map(|o| parse_setting(*o)).collect();
        let setting_defs = options.iter().filter_map(|o| setting_def(*o)).collect();

        let mut inputs: HashMap<(u8, u8), Vec<Input>> = HashMap::new();
        if let Some(controls) = child(controller, "controls") {
            for c in controls.children().filter(|n| n.has_tag_name("control")) {
                let options = child(c, "options")
                    .map(|o| {
                        o.children()
                            .filter(|n| n.is_element())
                            .fold(0, |acc, n| acc | Options::from_name(n.tag_name().name()))
                    })
                    .unwrap_or(0);
                inputs
                    .entry((byte(text(c, "status"), 0), byte(text(c, "midino"), 0)))
                    .or_default()
                    .push(Input {
                        group: text(c, "group").unwrap_or("").to_string(),
                        key: text(c, "key").unwrap_or("").to_string(),
                        options: Options(options),
                        description: description(c),
                        ..Default::default()
                    });
            }
        }

        let mut outputs = Vec::new();
        if let Some(outs) = child(controller, "outputs") {
            for o in outs.children().filter(|n| n.has_tag_name("output")) {
                let float = |name: &str, default: f64| {
                    text(o, name)
                        .and_then(|t| t.parse().ok())
                        .unwrap_or(default)
                };
                outputs.push(Output {
                    group: text(o, "group").unwrap_or("").to_string(),
                    key: text(o, "key").unwrap_or("").to_string(),
                    status: byte(text(o, "status"), 0),
                    control: byte(text(o, "midino"), 0),
                    on: byte(text(o, "on"), 0x7F),
                    off: byte(text(o, "off"), 0x00),
                    min: float("minimum", 0.0),
                    max: float("maximum", 1.0),
                    description: description(o),
                    ..Default::default()
                });
            }
        }

        Ok(Self {
            name,
            author,
            path: PathBuf::new(),
            scripts,
            settings,
            setting_defs,
            inputs,
            outputs,
            variables: Vec::new(),
            products,
        })
    }
}

/// A `<description>`, with the line breaks and indents of the XML squashed.
fn description(node: roxmltree::Node) -> String {
    text(node, "description")
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn setting_def(o: roxmltree::Node) -> Option<SettingDef> {
    let variable = o.attribute("variable")?.to_string();
    let kind = o.attribute("type").unwrap_or("string").to_ascii_lowercase();
    let values: Vec<_> = o.children().filter(|n| n.has_tag_name("value")).collect();
    let label_of = |v: &roxmltree::Node| {
        v.attribute("label")
            .map(str::to_string)
            .unwrap_or_else(|| v.text().unwrap_or("").trim().to_string())
    };
    let choices: Vec<String> = values.iter().map(label_of).collect();
    let default = match kind.as_str() {
        "enum" => values
            .iter()
            .find(|v| v.attribute("default") == Some("true"))
            .or(values.first())
            .map(label_of)
            .unwrap_or_default(),
        "boolean" => {
            if o.attribute("default")
                .is_some_and(|d| d.eq_ignore_ascii_case("true"))
            {
                "On".into()
            } else {
                "Off".into()
            }
        }
        _ => o.attribute("default").unwrap_or("").to_string(),
    };
    Some(SettingDef {
        label: o
            .attribute("label")
            .map_or_else(|| variable.clone(), str::to_string),
        variable,
        description: description(o),
        kind,
        default,
        choices,
    })
}

fn parse_setting(o: roxmltree::Node) -> Option<(String, SettingValue)> {
    let var = o.attribute("variable")?.to_string();
    let default = o.attribute("default").unwrap_or("");
    let value = match o
        .attribute("type")
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "boolean" => SettingValue::Bool(default.eq_ignore_ascii_case("true")),
        "integer" | "real" | "number" => SettingValue::Number(default.parse().unwrap_or(0.0)),
        "enum" => {
            let values: Vec<_> = o.children().filter(|n| n.has_tag_name("value")).collect();
            let chosen = values
                .iter()
                .find(|v| v.attribute("default") == Some("true"))
                .or(values.first())?;
            SettingValue::Text(chosen.text().unwrap_or("").trim().to_string())
        }
        _ => SettingValue::Text(default.to_string()),
    };
    Some((var, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<MixxxControllerPreset mixxxVersion="" schemaVersion="1">
  <info><name>Test Pad</name><author>me</author></info>
  <settings>
    <group label="Lights">
      <option variable="vuMeter" type="boolean" default="true" label="x"/>
      <option variable="color" type="enum" label="c">
        <value label="Off">off</value>
        <value label="Red" default="true">85</value>
      </option>
    </group>
    <option variable="steps" type="integer" default="4" min="1" max="8" label="s"/>
  </settings>
  <controller id="Test">
    <scriptfiles>
      <file filename="Test-scripts.js" functionprefix="Test"/>
    </scriptfiles>
    <controls>
      <control>
        <group>[Channel1]</group><key>play</key>
        <status>0x90</status><midino>0x0B</midino>
        <options><normal/></options>
      </control>
      <control>
        <group>[Channel1]</group><key>Test.jog</key>
        <status>0xB0</status><midino>33</midino>
        <options><Script-Binding/></options>
      </control>
      <control>
        <group>[Channel1]</group><key>rate</key>
        <status>0xB0</status><midino>0x00</midino>
        <options><fourteen-bit-msb/><soft-takeover/></options>
      </control>
    </controls>
    <outputs>
      <output>
        <group>[Channel1]</group><key>play_indicator</key>
        <status>0x90</status><midino>0x0B</midino>
        <on>0x7F</on><minimum>0.5</minimum>
      </output>
    </outputs>
  </controller>
</MixxxControllerPreset>"#;

    #[test]
    fn parses_bindings_settings_and_outputs() {
        let m = Mapping::parse(XML, Path::new("/maps")).unwrap();
        assert_eq!(m.name, "Test Pad");
        assert_eq!(m.scripts.len(), 2);
        assert_eq!(m.scripts[0].path, Path::new("/maps").join(REQUIRED_SCRIPT));
        assert_eq!(m.scripts[1].prefix, "Test");
        assert_eq!(
            m.settings,
            vec![
                ("vuMeter".into(), SettingValue::Bool(true)),
                ("color".into(), SettingValue::Text("85".into())),
                ("steps".into(), SettingValue::Number(4.0)),
            ]
        );
        assert_eq!(m.inputs[&(0x90, 0x0B)][0].key, "play");
        assert!(m.inputs[&(0x90, 0x0B)][0].options.is_empty());
        assert!(m.inputs[&(0xB0, 33)][0].options.has(Options::SCRIPT));
        let rate = &m.inputs[&(0xB0, 0)][0];
        assert!(rate.options.has(Options::FOURTEEN_BIT_MSB));
        assert!(rate.options.has(Options::SOFT_TAKEOVER));
        let o = &m.outputs[0];
        assert_eq!((o.status, o.control, o.on, o.off), (0x90, 0x0B, 0x7F, 0));
        assert_eq!((o.min, o.max), (0.5, 1.0));
    }

    #[test]
    fn reads_the_bundled_mappings() {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        for file in [
            "Pioneer-DDJ-FLX4.midi.xml",
            "Allen and Heath Xone K2.midi.xml",
            "DJ TechTools MIDI Fighter Twister.midi.xml",
        ] {
            let m = Mapping::load(&dir.join(file)).unwrap();
            assert!(!m.inputs.is_empty(), "{file}");
            for s in &m.scripts {
                assert!(s.path.exists(), "{}", s.path.display());
            }
        }
    }
}
