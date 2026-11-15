//! Simple top views of controllers for the Controller Editor: where each
//! control sits, its shape and kind, and the MIDI messages it sends. DJ Sugar
//! draws these itself from `layouts/*.json`, one for each controller on the
//! [support](crate::support) list that has a mapping.
//!
//! A message is written `"[layer|]SS NN"` in hex: `"SHIFT|90 0E"`, or
//! `"B? 00"` for any channel (the Xone K2's channel is set on the device).

use serde_json::Value;

use crate::probe::message_key;

const LAYOUTS: [&str; 18] = [
    include_str!("../layouts/flx4.json"),
    include_str!("../layouts/ddj-flx6.json"),
    include_str!("../layouts/ddj-flx10.json"),
    include_str!("../layouts/xdj-rx3.json"),
    include_str!("../layouts/xdj-az.json"),
    include_str!("../layouts/ddj-400.json"),
    include_str!("../layouts/ddj-800.json"),
    include_str!("../layouts/ddj-200.json"),
    include_str!("../layouts/ddj-sb3.json"),
    include_str!("../layouts/ddj-sx.json"),
    include_str!("../layouts/ddj-sx2.json"),
    include_str!("../layouts/ddj-rx.json"),
    include_str!("../layouts/mixtrack-platinum-fx.json"),
    include_str!("../layouts/mixtrack-pro-fx.json"),
    include_str!("../layouts/inpulse-500.json"),
    include_str!("../layouts/xone-k2.json"),
    include_str!("../layouts/twister.json"),
    include_str!("../layouts/numark-v7.json"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct PartMessage {
    /// The status byte; with `any_channel` its low four bits are 0.
    pub status: u8,
    pub any_channel: bool,
    pub number: u8,
    /// "SHIFT", "Hot cue", "Bank 2"; empty for the plain message.
    pub layer: String,
}

impl PartMessage {
    pub fn matches(&self, status: u8, number: u8) -> bool {
        let (s, n) = message_key(status, number);
        let (mine, _) = message_key(self.status, self.number);
        n == self.number
            && if self.any_channel {
                s & 0xF0 == mine & 0xF0
            } else {
                s == mine
            }
    }
}

/// One control on the controller.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub id: String,
    pub label: String,
    /// "button", "pad", "knob", "encoder", "fader", "jog" or "switch".
    pub kind: String,
    pub round: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub midi: Vec<PartMessage>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The mapping file this draws.
    pub file: String,
    /// Other mapping files for the same controller: DJ Sugar's own mappings
    /// of it (`.djsugar-mapping`).
    pub also: Vec<String>,
    pub width: f32,
    pub height: f32,
    /// The MIDI channel (1-16) to list for messages that leave it open.
    pub channel: Option<u8>,
    /// Outlines: x, y, width, height, corner radius.
    pub panels: Vec<[f32; 5]>,
    pub parts: Vec<Part>,
}

fn parse_message(s: &str) -> Option<PartMessage> {
    let (layer, msg) = match s.rsplit_once('|') {
        Some((l, m)) => (l.to_string(), m),
        None => (String::new(), s),
    };
    let mut it = msg.split_whitespace();
    let st = it.next()?;
    let number = u8::from_str_radix(it.next()?, 16).ok()?;
    let (status, any_channel) = match st.strip_suffix('?') {
        Some(hi) => (u8::from_str_radix(hi, 16).ok()? << 4, true),
        None => (u8::from_str_radix(st, 16).ok()?, false),
    };
    Some(PartMessage {
        status,
        any_channel,
        number,
        layer,
    })
}

fn f(v: &Value, key: &str) -> f32 {
    v[key].as_f64().unwrap_or(0.0) as f32
}

fn parse(json: &str) -> Option<Layout> {
    let v: Value = serde_json::from_str(json).ok()?;
    let parts = v["controls"]
        .as_array()?
        .iter()
        .map(|c| Part {
            id: c["id"].as_str().unwrap_or("").into(),
            label: c["label"].as_str().unwrap_or("").into(),
            kind: c["kind"].as_str().unwrap_or("button").into(),
            round: c["round"].as_bool().unwrap_or(false),
            x: f(c, "x"),
            y: f(c, "y"),
            w: f(c, "w"),
            h: f(c, "h"),
            midi: c["midi"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| m.as_str().and_then(parse_message))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    let panels = v["panels"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let p = p.as_array()?;
                    let n = |i: usize| p.get(i).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    Some([n(0), n(1), n(2), n(3), n(4)])
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Layout {
        file: v["file"].as_str()?.into(),
        also: v["also"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|f| f.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        width: f(&v, "width"),
        height: f(&v, "height"),
        channel: v["channel"].as_u64().map(|c| c as u8),
        panels,
        parts,
    })
}

impl Layout {
    /// The layout for a mapping file, by file name.
    pub fn for_file(file_name: &str) -> Option<Self> {
        LAYOUTS
            .iter()
            .filter_map(|j| parse(j))
            .find(|l| l.file == file_name || l.also.iter().any(|f| f == file_name))
    }

    /// The mapping files there are pictures for.
    pub fn files() -> Vec<String> {
        LAYOUTS
            .iter()
            .filter_map(|j| parse(j))
            .flat_map(|l| std::iter::once(l.file).chain(l.also))
            .collect()
    }

    /// The control that sends this message, and the layer it is on.
    pub fn find(&self, status: u8, number: u8) -> Option<(&Part, &PartMessage)> {
        self.parts.iter().find_map(|p| {
            p.midi
                .iter()
                .find(|m| m.matches(status, number))
                .map(|m| (p, m))
        })
    }

    /// Whether a message is on the channel this layout lists.
    pub fn shows_channel(&self, status: u8) -> bool {
        match self.channel {
            Some(c) if status < 0xF0 => (status & 0x0F) + 1 == c,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_parse_and_find_controls() {
        for file in [
            "Pioneer-DDJ-FLX4.midi.xml",
            "Allen and Heath Xone K2.midi.xml",
            "DJ TechTools MIDI Fighter Twister.midi.xml",
        ] {
            let l = Layout::for_file(file).unwrap_or_else(|| panic!("{file}"));
            assert!(!l.parts.is_empty());
        }
        let flx = Layout::for_file("Pioneer-DDJ-FLX4.midi.xml").unwrap();
        let (p, m) = flx.find(0x90, 0x0E).unwrap();
        assert_eq!(
            (p.label.as_str(), m.layer.as_str()),
            ("PLAY/PAUSE", "SHIFT")
        );
        let (p, m) = flx.find(0x98, 0x41).unwrap();
        assert_eq!(
            (p.id.as_str(), m.layer.as_str()),
            ("l-pad2", "Keyboard + SHIFT")
        );
        let k2 = Layout::for_file("Allen and Heath Xone K2.midi.xml").unwrap();
        assert_eq!(k2.find(0x8E, 0x0F).unwrap().0.label, "SHIFT");
        assert!(k2.shows_channel(0x9E) && !k2.shows_channel(0x9D));
    }

    /// Every message the bundled mappings bind is somewhere on the picture.
    #[test]
    fn layouts_cover_their_mappings() {
        let dir = crate::default_mappings_dir().unwrap();
        for file in Layout::files() {
            let file = file.as_str();
            let l = Layout::for_file(file).unwrap();
            let m = crate::Mapping::load(&dir.join(file)).unwrap();
            let mut missing: Vec<String> = m
                .inputs
                .keys()
                .filter(|(s, _)| l.shows_channel(*s))
                .filter(|(s, n)| l.find(*s, *n).is_none())
                .map(|(s, n)| format!("{s:02X} {n:02X}"))
                .collect();
            missing.sort();
            assert!(
                missing.is_empty(),
                "{file}: not on the picture: {missing:?}"
            );
        }
    }
}
