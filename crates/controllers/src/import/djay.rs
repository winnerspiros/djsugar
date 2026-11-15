//! djay Pro mappings (`.djayMidiMapping`): an Apple property list whose
//! `controls` each bind a MIDI message (`midiMessageType` 1 note, 3 CC,
//! `midiChannel` 0-15, `midiData` the number) to a djay key path such as
//! `turntable1.playPause`. `modifier` rows act while the controller's
//! modifier (djay's SHIFT, `application.modifier`) is held; `condition`
//! (`modifier2 == 3`) picks a row by a numbered modifier.

use serde_json::{Map, Value};

use super::words::{
    beats, deck, eq, eq_kill, filter, fx_slot, fx_unit, indicator, library, master,
};
use super::Import;
use crate::format::{Light, LightSource, Midi, DjSugarMapping, Row, Variable};
use crate::mapping::When;

/// An XML property list as JSON.
fn plist(xml: &str) -> Result<Value, String> {
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let root = doc
        .root_element()
        .children()
        .find(|n| n.is_element())
        .ok_or("empty property list")?;
    Ok(value(root))
}

fn value(n: roxmltree::Node) -> Value {
    match n.tag_name().name() {
        "dict" => {
            let mut out = Map::new();
            let kids: Vec<_> = n.children().filter(|c| c.is_element()).collect();
            for pair in kids.chunks(2) {
                if let [k, v] = pair {
                    out.insert(k.text().unwrap_or("").to_string(), value(*v));
                }
            }
            Value::Object(out)
        }
        "array" => Value::Array(n.children().filter(|c| c.is_element()).map(value).collect()),
        "integer" | "real" => n
            .text()
            .and_then(|t| t.trim().parse::<f64>().ok())
            .map(Value::from)
            .unwrap_or(Value::Null),
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(n.text().unwrap_or("").to_string()),
    }
}

enum Act {
    Bind(String, &'static [&'static str]),
    Script(String),
}

/// What a djay key path does. Note buttons need no options; a CC button
/// gets `button` from the caller.
fn translate(path: &str) -> Option<Act> {
    let b = |c: String| Some(Act::Bind(c, &[]));
    if let Some(rest) = path.strip_prefix("mixer.") {
        let digit = |p: &str| p.parse::<u8>().ok().filter(|d| (1..=4).contains(d));
        return match rest {
            "crossfade" => b(master("crossfader")),
            "masterLevel" => b(master("gain")),
            "monitorMix" => b(master("headMix")),
            "monitorLevel" => b(master("headGain")),
            _ => {
                if let Some(d) = rest.strip_prefix("lineVolume").and_then(digit) {
                    b(deck(d, "volume"))
                } else if let Some(d) = rest.strip_prefix("monitorActive").and_then(digit) {
                    b(deck(d, "pfl"))
                } else {
                    None
                }
            }
        };
    }
    if let Some(rest) = path.strip_prefix("musicLibrary.") {
        return match rest {
            "libraryRotary" => Some(Act::Bind(library("MoveVertical"), &["selectknob"])),
            "sectionRotary" => Some(Act::Bind(library("MoveFocus"), &["selectknob"])),
            "libraryBack" => b(library("MoveFocusBackward")),
            _ => {
                let d = rest.strip_prefix("load")?.parse::<u8>().ok()?;
                b(deck(d, "LoadSelectedTrack"))
            }
        };
    }
    let rest = path.strip_prefix("turntable")?;
    let (d, what) = rest.split_once('.')?;
    let d: u8 = d.parse().ok()?;
    let trailing = |p: &str| -> Option<u8> { what.strip_prefix(p)?.parse().ok() };
    if let Some(n) = trailing("cueOrJumpIfAlreadySet")
        .or_else(|| trailing("cuePositionOrJumpConsideringPlayState"))
        .or_else(|| trailing("cuePosition"))
    {
        return b(deck(d, &format!("hotcue_{n}_activate")));
    }
    if let Some(n) = trailing("clearCuePoint") {
        return b(deck(d, &format!("hotcue_{n}_clear")));
    }
    // autoLoop4BeatInterval, autoLoop025BeatInterval (0.25 beats).
    if let Some(size) = what
        .strip_prefix("autoLoop")
        .and_then(|r| r.strip_suffix("BeatInterval"))
    {
        let n = match size.strip_prefix('0') {
            Some(frac) if !frac.is_empty() => format!("0.{frac}").parse::<f64>().ok()?,
            _ => size.parse::<f64>().ok()?,
        };
        return b(deck(d, &format!("beatloop_{}_toggle", beats(n))));
    }
    match what {
        "playPause" => b(deck(d, "play")),
        "cue" => b(deck(d, "cue_default")),
        "gotoZero" | "gotoBeginning" => b(deck(d, "start")),
        "bpmSync" => b(deck(d, "sync_enabled")),
        "speed" => b(deck(d, "rate")),
        "pitchBendMinus" => b(deck(d, "rate_temp_down")),
        "pitchBendPlus" => b(deck(d, "rate_temp_up")),
        "gain" => b(deck(d, "pregain")),
        "resetGain" => b(deck(d, "pregain_set_default")),
        "lowEQ" => b(eq(d, 1)),
        "midEQ" => b(eq(d, 2)),
        "highEQ" => b(eq(d, 3)),
        "lowEQKill" => b(eq_kill(d, 1)),
        "midEQKill" => b(eq_kill(d, 2)),
        "highEQKill" => b(eq_kill(d, 3)),
        "filter" => b(filter(d)),
        "loopIn" => b(deck(d, "loop_in")),
        "loopOutAndReloopOrUnloop" => b(deck(d, "loop_out")),
        "reloop" => b(deck(d, "reloop_toggle")),
        "autoLoopOnOff" => Some(Act::Script(format!("deck {d} loop 4"))),
        "autoLoopDurationHalf" => b(deck(d, "loop_halve")),
        "autoLoopDurationDouble" => b(deck(d, "loop_double")),
        "deckSlipToggle" => b(deck(d, "slip_enabled")),
        "pitchOnOff" => b(deck(d, "keylock")),
        "fxActive" => b(fx_slot(d, 1, "enabled")),
        "fx1WetDryValue" => b(fx_unit(d, "mix")),
        "fx1ParameterValue" => b(fx_slot(d, 1, "meta")),
        "fx1SelectNext" => b(fx_slot(d, 1, "next_effect")),
        _ => None,
    }
}

pub(super) fn import(text: &str, file: &str) -> Result<Import, String> {
    let p = plist(text)?;
    let controls = p["controls"]
        .as_array()
        .ok_or("This djay Pro file has no controls")?;
    let device = p["endpointName"].as_str().unwrap_or("").to_string();
    let title = file
        .strip_suffix(".djayMidiMapping")
        .unwrap_or(file)
        .to_string();
    let mut m = DjSugarMapping::new(&format!("{title} (djay Pro)"));
    m.controller.name = device.clone();
    if !device.is_empty() {
        m.controller.ports.push(device);
    }
    let mut variables: Vec<String> = Vec::new();
    for c in controls {
        let path = c["keyPath"].as_str().unwrap_or("");
        let ch = c["midiChannel"].as_f64().unwrap_or(0.0) as u8 & 0x0F;
        let data = c["midiData"].as_f64().unwrap_or(0.0) as u8 & 0x7F;
        let note = c["midiMessageType"].as_f64() == Some(1.0);
        let msg = Midi::new(if note { 0x90 } else { 0xB0 } | ch, data);
        let rotary = c["controlType"].as_str() == Some("rotary");
        let mut layer = Vec::new();
        if c["modifier"].as_bool() == Some(true) {
            layer.push("modifier".to_string());
        }
        if let Some(cond) = c["condition"].as_str() {
            // "modifier2 == 3"
            if let Some((name, v)) = cond.split_once("==") {
                let name = name.trim().to_string();
                layer.push(format!("{name}={}", v.trim()));
                if !variables.contains(&name) {
                    variables.push(name);
                }
            }
        }
        let mut rows = Vec::new();
        if path == "application.modifier" {
            if !variables.contains(&"modifier".to_string()) {
                variables.push("modifier".into());
            }
            rows.push(Row::script(msg, "set_var 'modifier' 1"));
            let mut up = Row::script(msg, "set_var 'modifier' 0");
            up.when = When::Release;
            rows.push(up);
        } else {
            rows.push(match translate(path) {
                Some(Act::Bind(control, opts)) => {
                    let mut o: Vec<&str> = opts.to_vec();
                    if !note && !rotary && c["controlType"].is_null() && is_button(path) {
                        o.push("button");
                    }
                    if c["flipped"].as_bool() == Some(true) && !rotary {
                        o.push("invert");
                    }
                    let mut r = Row::bind(msg, &control, &o);
                    if c["pickupMode"].as_bool() == Some(true) {
                        r.takeover = Some("pickup".into());
                    }
                    if c.get("output").is_some() {
                        let mut l = Light::new(
                            msg,
                            LightSource::Bind {
                                control: indicator(&control),
                                min: 0.5,
                                max: f64::MAX,
                                on: c["output"]["midiMaxValue"].as_f64().unwrap_or(127.0) as u8,
                                off: 0,
                            },
                        );
                        l.name = path.to_string();
                        m.lights.push(l);
                    }
                    r
                }
                Some(Act::Script(s)) => Row::script(msg, &s),
                None => Row::unsupported(
                    msg,
                    path,
                    path,
                    "DJ Sugar has no match for this djay Pro function yet",
                ),
            });
        }
        for mut r in rows {
            r.name = path.to_string();
            r.layer = layer.join(" & ");
            m.rows.push(r);
        }
    }
    for name in variables {
        m.variables.push(Variable {
            name,
            default: 0.0,
            saved: false,
        });
    }
    Ok(Import {
        mappings: vec![m],
        ..Default::default()
    })
}

/// Key paths that are pressed rather than turned.
fn is_button(path: &str) -> bool {
    ![
        "Level",
        "Mix",
        "EQ",
        "gain",
        "filter",
        "speed",
        "crossfade",
        "Volume",
        "Value",
        "Rotary",
    ]
    .iter()
    .any(|w| path.ends_with(w) || path.contains(&format!(".{w}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Action;

    const K2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>endpointName</key><string>XONE:K2</string>
<key>controls</key><array>
<dict><key>keyPath</key><string>mixer.lineVolume2</string><key>midiChannel</key><integer>14</integer><key>midiData</key><integer>18</integer><key>midiMessageType</key><integer>3</integer></dict>
<dict><key>keyPath</key><string>turntable1.playPause</string><key>midiChannel</key><integer>14</integer><key>midiData</key><integer>108</integer><key>midiMessageType</key><integer>1</integer><key>output</key><dict/></dict>
<dict><key>keyPath</key><string>application.modifier</string><key>midiChannel</key><integer>14</integer><key>midiData</key><integer>15</integer><key>midiMessageType</key><integer>1</integer></dict>
<dict><key>keyPath</key><string>turntable1.clearCuePoint1</string><key>midiChannel</key><integer>14</integer><key>midiData</key><integer>28</integer><key>midiMessageType</key><integer>1</integer><key>modifier</key><true/></dict>
<dict><key>keyPath</key><string>turntable1.autoLoop025BeatInterval</string><key>midiChannel</key><integer>0</integer><key>midiData</key><integer>3</integer><key>midiMessageType</key><integer>1</integer><key>condition</key><string>modifier2 == 3</string></dict>
<dict><key>keyPath</key><string>turntable1.jogSeekMove</string><key>midiChannel</key><integer>14</integer><key>midiData</key><integer>20</integer><key>midiMessageType</key><integer>3</integer></dict>
</array></dict></plist>"#;

    #[test]
    fn reads_a_djay_mapping() {
        let i = import(K2, "XONE.K2.djayMidiMapping").unwrap();
        let m = &i.mappings[0];
        assert_eq!(m.name, "XONE.K2 (djay Pro)");
        assert_eq!(m.controller.ports, ["XONE:K2"]);
        let vol = &m.rows[0];
        assert_eq!(vol.midi, Midi::new(0xBE, 18));
        assert!(
            matches!(&vol.action, Action::Bind { control, .. } if control == "[Channel2],volume")
        );
        assert_eq!(m.lights.len(), 1);
        let clear = m
            .rows
            .iter()
            .find(|r| r.name == "turntable1.clearCuePoint1")
            .unwrap();
        assert_eq!(clear.layer, "modifier");
        let small = m
            .rows
            .iter()
            .find(|r| r.name.contains("autoLoop025"))
            .unwrap();
        assert_eq!(small.layer, "modifier2=3");
        assert!(
            matches!(&small.action, Action::Bind { control, .. } if control == "[Channel1],beatloop_0.25_toggle")
        );
        assert!(
            m.rows
                .iter()
                .find(|r| r.name.contains("jogSeek"))
                .unwrap()
                .off
        );
        assert_eq!(
            m.rows
                .iter()
                .filter(|r| r.name == "application.modifier")
                .count(),
            2
        );
    }
}
