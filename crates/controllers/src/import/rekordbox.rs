//! rekordbox MIDI Learn exports (`.csv`). rekordbox runs Pioneer's own
//! controllers without a file, so these only exist for controllers the DJ
//! mapped with MIDI Learn. A line is: function, (unused), control type,
//! the MIDI in for global use and decks 1 to 4, the MIDI out the same way,
//! then options (`Blink=500;Min=127;Max=64;`) and a comment.

use super::words::{beats, deck, eq, filter, fx_slot, indicator, library, master};
use super::Import;
use crate::format::{Action, DjSugarMapping, Light, LightSource, Midi, Row};

enum Act {
    Bind(String, &'static [&'static str]),
    Script(String),
}

/// What a rekordbox function does on deck `d` (1 when it's global).
fn translate(function: &str, kind: &str, d: u8) -> Option<Act> {
    let button: &'static [&'static str] = if kind == "Button" || kind.ends_with("Pad") {
        &["button"]
    } else {
        &[]
    };
    let bind = |c: String| Some(Act::Bind(c, button));
    let (base, shift) = match function.strip_suffix("+Shift") {
        Some(b) => (b, true),
        None => (function, false),
    };
    if let Some(n) = base
        .strip_prefix("PAD")
        .and_then(|r| r.strip_suffix("_HotCue"))
        .and_then(|n| n.parse::<u8>().ok())
    {
        let what = if shift { "clear" } else { "activate" };
        return bind(deck(d, &format!("hotcue_{n}_{what}")));
    }
    if shift {
        return None;
    }
    // FX1-2: knob 2 of FX unit 1; FX1-2On: its button.
    if let Some(rest) = base.strip_prefix("FX") {
        let (unit, rest) = rest.split_once('-')?;
        let unit: u8 = unit.parse().ok()?;
        let (slot, on) = match rest.strip_suffix("On") {
            Some(s) => (s, true),
            None => (rest, false),
        };
        let slot: u8 = slot.parse().ok()?;
        return Some(if on {
            Act::Bind(fx_slot(unit, slot, "enabled"), &["button"])
        } else {
            Act::Bind(fx_slot(unit, slot, "meta"), &[])
        });
    }
    match base {
        "PlayPause" => bind(deck(d, "play")),
        "Cue" => bind(deck(d, "cue_default")),
        "Sync" => bind(deck(d, "sync_enabled")),
        "MasterTempo" | "KeyLock" => bind(deck(d, "keylock")),
        "Load" => bind(deck(d, "LoadSelectedTrack")),
        "JumpToTrackStart" => bind(deck(d, "start")),
        "AutoLoop" => Some(Act::Script(format!("deck {d} loop 4"))),
        "LoopIn" => bind(deck(d, "loop_in")),
        "LoopOut" => bind(deck(d, "loop_out")),
        "Reloop" | "ReloopExit" => bind(deck(d, "reloop_toggle")),
        "LoopHalf" => bind(deck(d, "loop_halve")),
        "LoopDouble" => bind(deck(d, "loop_double")),
        // An endless knob: turned left (127) halves, right (1) doubles.
        "LoopHalfDouble" => Some(Act::Script(format!(
            "deck {d} (value > 50% ? loop_half : loop_double)"
        ))),
        "ChannelFader" => Some(Act::Bind(deck(d, "volume"), &[])),
        "Trim" => Some(Act::Bind(deck(d, "pregain"), &[])),
        "EQHigh" => Some(Act::Bind(eq(d, 3), &[])),
        "EQMid" => Some(Act::Bind(eq(d, 2), &[])),
        "EQLow" => Some(Act::Bind(eq(d, 1), &[])),
        "ColorFX" | "Filter" | "CFX" => Some(Act::Bind(filter(d), &[])),
        "TempoSlider" => Some(Act::Bind(deck(d, "rate"), &[])),
        "HeadphoneCue" | "CueHeadphone" | "Headphone" => bind(deck(d, "pfl")),
        "Crossfader" => Some(Act::Bind(master("crossfader"), &[])),
        "MasterLevel" => Some(Act::Bind(master("gain"), &[])),
        "HeadphonesMix" | "HeadphoneMix" => Some(Act::Bind(master("headMix"), &[])),
        "HeadphonesLevel" | "HeadphoneLevel" => Some(Act::Bind(master("headGain"), &[])),
        "Browse" => Some(Act::Bind(library("MoveVertical"), &["selectknob"])),
        "BrowsePress" => bind(library("GoToItem")),
        "Shift" => Some(Act::Script("set_var 'shift' 1".into())),
        "BeatJumpForward" => bind(deck(d, "beatjump_forward")),
        "BeatJumpBackward" => bind(deck(d, "beatjump_backward")),
        _ => {
            let size = base.strip_prefix("BeatLoop")?.parse::<f64>().ok()?;
            bind(deck(d, &format!("beatloop_{}_toggle", beats(size))))
        }
    }
}

/// `9E34` → status 9E, number 34. rekordbox writes notes as 8n or 9n
/// alike; DJ Sugar listens for the note-on.
fn midi(cell: &str) -> Option<Midi> {
    let cell = cell.trim();
    if cell.len() != 4 {
        return None;
    }
    let status = u8::from_str_radix(&cell[..2], 16).ok()?;
    let number = u8::from_str_radix(&cell[2..], 16).ok()?;
    let status = if status & 0xF0 == 0x80 {
        status | 0x10
    } else {
        status
    };
    (status >= 0x80 && number < 0x80).then(|| Midi::new(status, number))
}

fn option(options: &str, name: &str) -> Option<u8> {
    options
        .split(';')
        .find_map(|o| o.strip_prefix(name)?.strip_prefix('=')?.trim().parse().ok())
}

pub(super) fn import(text: &str) -> Result<Import, String> {
    let mut lines = text.lines();
    let head = lines.next().unwrap_or("");
    let name = head
        .strip_prefix("@file,")
        .and_then(|r| r.split_once(','))
        .map(|(_, n)| n.trim().to_string())
        .ok_or("This isn't a rekordbox MIDI mapping")?;
    let mut m = DjSugarMapping::new(&format!("{name} (rekordbox)"));
    m.controller.name = name.clone();
    m.controller.ports.push(name.clone());
    let mut uses_shift = false;
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        if cells.len() < 13 {
            continue;
        }
        let (function, kind) = (cells[0].trim(), cells[2].trim());
        let options = cells.get(13).copied().unwrap_or("");
        // Placeholders rekordbox keeps for lights of controls with nothing
        // mapped.
        if function == "NoFunction" || function.is_empty() {
            continue;
        }
        // Global in, then decks 1 to 4.
        for (col, d) in [(3, 0u8), (4, 1), (5, 2), (6, 3), (7, 4)] {
            let Some(msg) = midi(cells[col]) else {
                continue;
            };
            let target = d.max(1);
            let mut row = match translate(function, kind, target) {
                Some(Act::Bind(c, o)) => Row::bind(msg, &c, o),
                Some(Act::Script(s)) => Row::script(msg, &s),
                None => Row::unsupported(
                    msg,
                    function,
                    function,
                    "DJ Sugar has no match for this rekordbox function yet",
                ),
            };
            row.name = if d == 0 {
                function.to_string()
            } else {
                format!("{function} (deck {d})")
            };
            if function == "Shift" {
                uses_shift = true;
                let mut up = Row::script(msg, "set_var 'shift' 0");
                up.when = crate::mapping::When::Release;
                up.name = row.name.clone();
                m.rows.push(row);
                m.rows.push(up);
                continue;
            }
            m.rows.push(row);
        }
        // Outputs: global, then decks 1 to 4.
        for (col, d) in [(8, 0u8), (9, 1), (10, 2), (11, 3), (12, 4)] {
            let Some(msg) = midi(cells[col]) else {
                continue;
            };
            let target = d.max(1);
            let Some(Act::Bind(c, _)) = translate(function, kind, target) else {
                continue;
            };
            let mut light = Light::new(
                msg,
                LightSource::Bind {
                    control: indicator(&c),
                    min: 0.5,
                    max: f64::MAX,
                    on: option(options, "Max").unwrap_or(0x7F),
                    off: option(options, "Min").unwrap_or(0),
                },
            );
            light.name = function.to_string();
            m.lights.push(light);
        }
    }
    if uses_shift {
        m.variables.push(crate::format::Variable {
            name: "shift".into(),
            default: 0.0,
            saved: false,
        });
    }
    if m.rows.is_empty() {
        return Err(format!(
            "{name} has no MIDI Learn rows: rekordbox runs this controller itself, so there is nothing in the file to import"
        ));
    }
    let _ = Action::Nothing;
    Ok(Import {
        mappings: vec![m],
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const K2: &str = "@file,1,XONE:K2
AutoLoop,,Button,,9E34,9E37,,,,9E34,9E37,,,,
Browse,,Rotary,BE14,,,,,,,,,,,
EQHigh,,KnobSlider,,BE05,BE06,,,,,,,,Fast;,
Cue,,Button,,8E1D,8E1E,,,,8E1D,8E1E,,,Fast;Blink=500;Priority=50;,
Load,,Button,,9E0D,9E0E,,,,9E0D,9E0E,,,Min=127;Max=64;,
NoFunction,,Button,,8E1A,,,,,8E1A,,,,Fast;Blink=1000;Priority=50;,
PAD1_HotCue+Shift,,DdjSxPad,,9E24,,,,,9E24,,,,Fast;,
LoopHalfDouble,,Rotary,,BE00,BE03,,,,,,,,Fast;,
Mystery,,Button,9E40,,,,,,,,,,,
";

    #[test]
    fn reads_a_k2_export() {
        let i = import(K2).unwrap();
        let m = &i.mappings[0];
        assert_eq!(m.name, "XONE:K2 (rekordbox)");
        let find = |name: &str| m.rows.iter().find(|r| r.name == name).unwrap();
        assert_eq!(
            find("AutoLoop (deck 2)").action,
            Action::Script("deck 2 loop 4".into())
        );
        assert_eq!(find("Cue (deck 1)").midi, Midi::new(0x9E, 0x1D));
        assert!(
            matches!(&find("EQHigh (deck 2)").action, Action::Bind { control, .. } if control == "[EqualizerRack1_[Channel2]_Effect1],parameter3")
        );
        assert!(
            matches!(&find("PAD1_HotCue+Shift (deck 1)").action, Action::Bind { control, .. } if control == "[Channel1],hotcue_1_clear")
        );
        assert!(find("Mystery").off);
        assert!(!m.rows.iter().any(|r| r.name.starts_with("NoFunction")));
        let load = m.lights.iter().find(|l| l.name == "Load").unwrap();
        assert!(matches!(
            load.source,
            LightSource::Bind {
                on: 64,
                off: 127,
                ..
            }
        ));
        assert!(i.summary().contains("rows work"));
    }

    #[test]
    fn built_in_controllers_have_nothing_to_import() {
        assert!(import("@file,1,DDJ-FLX4\n").is_err());
    }
}
