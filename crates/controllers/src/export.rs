//! Writing a mapping out: as one self-contained `.djsugar-mapping` to share,
//! or as a Mixxx `.midi.xml` with its scripts. Mixxx has no DJSugarScript,
//! layers or long presses, so what it can't hold is listed in a report.

use std::path::{Path, PathBuf};

use djsugar_script::{Expr, Kind};

use crate::format::DjSugarMapping;
use crate::mapping::{Input, Mapping, Options, When, REQUIRED_SCRIPT};

/// What an export wrote and what it had to leave out.
#[derive(Debug, Default)]
pub struct Report {
    pub written: Vec<PathBuf>,
    /// One line per thing left out: "SHIFT_K2_ENC_BTM_L: a shift layer".
    pub left_out: Vec<String>,
}

/// The mapping with the scripts it needs inside it, so the file works on
/// another computer: every script file found in `own_dir` (the DJ's own
/// mappings) goes in `sources`. Files DJ Sugar ships stay referenced.
pub fn self_contained(m: &DjSugarMapping, shipped_dir: &Path, own_dir: &Path) -> DjSugarMapping {
    let mut out = m.clone();
    for (file, _) in &m.scripts {
        if shipped_dir.join(file).is_file() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(own_dir.join(file)) {
            out.sources.insert(file.clone(), text);
        }
    }
    out
}

/// Writes a shared `.djsugar-mapping`.
pub fn write_djsugar(
    m: &DjSugarMapping,
    shipped_dir: &Path,
    own_dir: &Path,
    path: &Path,
) -> std::io::Result<Report> {
    self_contained(m, shipped_dir, own_dir).write(path)?;
    Ok(Report {
        written: vec![path.to_path_buf()],
        left_out: Vec::new(),
    })
}

/// Reads a shared `.djsugar-mapping`, writing the scripts it carries into
/// `own_dir` so it runs.
pub fn read_djsugar(path: &Path, own_dir: &Path) -> Result<DjSugarMapping, String> {
    let mut m = DjSugarMapping::read(path)?;
    for (file, text) in std::mem::take(&mut m.sources) {
        // Only plain file names: a shared file mustn't write elsewhere.
        if file.contains('/') || file.contains('\\') || file.starts_with('.') {
            continue;
        }
        std::fs::create_dir_all(own_dir).map_err(|e| e.to_string())?;
        std::fs::write(own_dir.join(&file), text).map_err(|e| e.to_string())?;
    }
    Ok(m)
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// An DJSugarScript row that is just one control, as Mixxx's group and key
/// and whether it's pressed (a button) rather than set from a fader.
fn plain(line: &str) -> Option<(String, String, bool)> {
    let script = djsugar_script::parse(line).ok()?;
    let mut e = &script.0;
    let mut deck = None;
    while let Expr::OnDeck(djsugar_script::Deck::Number(d), inner) = e {
        deck = Some(*d);
        e = inner;
    }
    let Expr::Action { verb, args } = e else {
        return None;
    };
    let group = |d: Option<u8>| d.map(|d| format!("[Channel{d}]"));
    let named = |button: bool| -> Option<(String, String, bool)> {
        let c = args.first()?.text();
        let (g, k) =
            crate::format::split_control(c).or_else(|| Some((group(deck)?, c.to_string())))?;
        Some((g, k, button))
    };
    match verb.kind {
        Kind::Toggle(item) | Kind::Press(item) => Some((group(deck)?, item.to_string(), true)),
        Kind::Value(item) if args.first().is_none_or(|a| a.number().is_nan()) => {
            Some((group(deck)?, item.to_string(), false))
        }
        Kind::ToggleNamed | Kind::PressNamed => named(true),
        Kind::Adjust if args.get(1).is_none_or(|a| a.number().is_nan()) => named(false),
        Kind::Play | Kind::Pause => None,
        _ => None,
    }
}

/// The mapping as Mixxx writes it, with its base merged in.
pub fn mixxx_xml(m: &DjSugarMapping, built: &Mapping, report: &mut Report) -> String {
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    x.push_str("<MixxxControllerPreset mixxxVersion=\"2.4.0\" schemaVersion=\"1\">\n  <info>\n");
    x.push_str(&format!("    <name>{}</name>\n", xml(&m.name)));
    if !m.author.is_empty() {
        x.push_str(&format!("    <author>{}</author>\n", xml(&m.author)));
    }
    let description = if m.description.is_empty() {
        "Exported from DJ Sugar.".to_string()
    } else {
        m.description.clone()
    };
    x.push_str(&format!(
        "    <description>{}</description>\n",
        xml(&description)
    ));
    x.push_str("  </info>\n");
    let id = m
        .controller
        .name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();
    x.push_str(&format!(
        "  <controller id=\"{}\">\n    <scriptfiles>\n",
        xml(&id)
    ));
    for s in &built.scripts {
        let file = s
            .path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        if file == REQUIRED_SCRIPT {
            continue;
        }
        x.push_str(&format!(
            "      <file filename=\"{}\" functionprefix=\"{}\"/>\n",
            xml(&file),
            xml(&s.prefix)
        ));
    }
    x.push_str("    </scriptfiles>\n    <controls>\n");
    let mut keys: Vec<&(u8, u8)> = built.inputs.keys().collect();
    keys.sort();
    for key in keys {
        for input in &built.inputs[key] {
            if let Some(c) = control(input, *key, report) {
                x.push_str(&c);
            }
        }
    }
    x.push_str("    </controls>\n    <outputs>\n");
    for o in &built.outputs {
        if let Some(line) = &o.script {
            report.left_out.push(format!(
                "Light {:02X} {:02X}: an DJSugarScript light ({line})",
                o.status, o.control
            ));
            continue;
        }
        x.push_str("      <output>\n");
        x.push_str(&format!("        <group>{}</group>\n", xml(&o.group)));
        x.push_str(&format!("        <key>{}</key>\n", xml(&o.key)));
        x.push_str(&format!("        <status>0x{:02X}</status>\n", o.status));
        x.push_str(&format!("        <midino>0x{:02X}</midino>\n", o.control));
        x.push_str(&format!("        <on>0x{:02X}</on>\n", o.on));
        x.push_str(&format!("        <off>0x{:02X}</off>\n", o.off));
        x.push_str(&format!("        <minimum>{}</minimum>\n", o.min));
        if o.max < f64::MAX / 2.0 {
            x.push_str(&format!("        <maximum>{}</maximum>\n", o.max));
        }
        x.push_str("      </output>\n");
    }
    x.push_str("    </outputs>\n  </controller>\n</MixxxControllerPreset>\n");
    if !m.variables.is_empty() {
        report.left_out.push(format!(
            "Variables {}: Mixxx has none outside its scripts",
            m.variables
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    x
}

fn control(input: &Input, (status, number): (u8, u8), report: &mut Report) -> Option<String> {
    let what = if input.name.is_empty() {
        format!("{status:02X} {number:02X}")
    } else {
        input.name.clone()
    };
    if !input.layer.is_empty() {
        report
            .left_out
            .push(format!("{what}: on a layer ({})", layer_text(&input.layer)));
        return None;
    }
    if input.when != When::Press {
        report
            .left_out
            .push(format!("{what}: a {} press", input.when.name()));
        return None;
    }
    let (group, key, options) = match &input.action {
        Some(line) => match plain(line) {
            Some((g, k, button)) => (
                g,
                k,
                if button {
                    Options::from_names(["button"])
                } else {
                    input.options
                },
            ),
            None => {
                report
                    .left_out
                    .push(format!("{what}: DJSugarScript ({line})"));
                return None;
            }
        },
        None => (input.group.clone(), input.key.clone(), input.options),
    };
    let mut c = String::from("      <control>\n");
    c.push_str(&format!("        <group>{}</group>\n", xml(&group)));
    c.push_str(&format!("        <key>{}</key>\n", xml(&key)));
    if !input.description.is_empty() {
        c.push_str(&format!(
            "        <description>{}</description>\n",
            xml(&input.description)
        ));
    }
    c.push_str(&format!("        <status>0x{status:02X}</status>\n"));
    c.push_str(&format!("        <midino>0x{number:02X}</midino>\n"));
    c.push_str("        <options>\n");
    let names = options.names();
    if names.is_empty() {
        c.push_str("          <normal/>\n");
    }
    for n in names {
        c.push_str(&format!("          <{n}/>\n"));
    }
    c.push_str("        </options>\n      </control>\n");
    Some(c)
}

fn layer_text(layer: &[(String, Option<f64>)]) -> String {
    layer
        .iter()
        .map(|(n, v)| match v {
            Some(v) => format!("{n}={v}"),
            None => n.clone(),
        })
        .collect::<Vec<_>>()
        .join(" & ")
}

/// Writes `<dir>/<name>.midi.xml` and copies its script files next to it.
pub fn write_mixxx(m: &DjSugarMapping, built: &Mapping, dir: &Path) -> std::io::Result<Report> {
    let mut report = Report::default();
    let text = mixxx_xml(m, built, &mut report);
    let stem: String = m
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.midi.xml", stem.trim()));
    std::fs::write(&path, text)?;
    report.written.push(path);
    for s in &built.scripts {
        let Some(file) = s.path.file_name() else {
            continue;
        };
        if file == REQUIRED_SCRIPT || !s.path.is_file() {
            continue;
        }
        let to = dir.join(file);
        if to != s.path {
            std::fs::copy(&s.path, &to)?;
            report.written.push(to);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{Midi, Row};

    #[test]
    fn one_control_rows_become_bindings() {
        assert_eq!(
            plain("deck 1 sync"),
            Some(("[Channel1]".into(), "sync_enabled".into(), true))
        );
        assert_eq!(
            plain("press '[Channel2],hotcue_3_clear'"),
            Some(("[Channel2]".into(), "hotcue_3_clear".into(), true))
        );
        assert_eq!(
            plain("adjust '[EffectRack1_EffectUnit1_Effect1],parameter1'"),
            Some((
                "[EffectRack1_EffectUnit1_Effect1]".into(),
                "parameter1".into(),
                false
            ))
        );
        assert_eq!(plain("(var 'k2mode' ? hot_cue 1 : sync)"), None);
    }

    #[test]
    fn exports_what_mixxx_can_hold_and_reports_the_rest() {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        let base = Mapping::load(&dir.join("Pioneer-DDJ-FLX4.midi.xml")).unwrap();
        let mut m = DjSugarMapping::new("FLX4 mine");
        m.controller.name = "DDJ-FLX4".into();
        m.based_on = Some("Pioneer-DDJ-FLX4.midi.xml".into());
        m.rows
            .push(Row::script(Midi::new(0x90, 0x0B), "deck 1 sync"));
        let mut long = Row::script(Midi::new(0x90, 0x0C), "deck 1 loop 4");
        long.when = When::Long;
        long.name = "CUE".into();
        m.rows.push(long);
        let built = m.build(Some(&base), &dir);
        let mut report = Report::default();
        let text = mixxx_xml(&m, &built, &mut report);
        assert!(text.contains("<key>sync_enabled</key>"));
        assert!(text.contains("PioneerDDJFLX4"));
        assert!(report
            .left_out
            .iter()
            .any(|l| l.starts_with("CUE: a long press")));
        // Mixxx reads it back.
        let again = Mapping::parse(&text, &dir).unwrap();
        assert!(again.inputs[&(0x90, 0x0B)]
            .iter()
            .any(|i| i.key == "sync_enabled"));
        assert!(!again.outputs.is_empty());
    }

    #[test]
    fn shared_files_carry_their_scripts() {
        let tmp = std::env::temp_dir().join(format!("djsugar-export-{}", std::process::id()));
        let own = tmp.join("own");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join("mine.js"), "var Mine = {};").unwrap();
        let mut m = DjSugarMapping::new("Mine");
        m.scripts.push(("mine.js".into(), "Mine".into()));
        let path = tmp.join("Mine.djsugar-mapping");
        write_djsugar(&m, &tmp.join("shipped"), &own, &path).unwrap();
        let other = tmp.join("other");
        let back = read_djsugar(&path, &other).unwrap();
        assert_eq!(back.name, "Mine");
        assert_eq!(
            std::fs::read_to_string(other.join("mine.js")).unwrap(),
            "var Mine = {};"
        );
        let _ = std::fs::remove_dir_all(tmp);
    }
}
