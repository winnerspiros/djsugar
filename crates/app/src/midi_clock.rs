//! Settings › Controllers › MIDI Clock: the outputs clock goes to, each with
//! an offset, the input clock comes in from, and whether the clock keeps
//! running when the decks stop. They
//! are kept in settings.json and handed to the engine's clock thread (see
//! `djsugar_engine::midi_clock`). CLOCK OUT itself is in the MASTER panel.

use std::sync::OnceLock;

use djsugar_control::ControlRegistry;
use djsugar_engine::midi_clock::{MidiClock, OutputConfig, GROUP, MAX_OFFSET_MS, VIRTUAL_PORT};
use serde_json::{json, Value};
use slint::{ModelRc, SharedString, VecModel};

use crate::{settings, short_port_name, PrefItem};

const OUTPUTS_KEY: &str = "midi_clock_outputs";
const KEEP_RUNNING_KEY: &str = "midi_clock_keep_running";
const INPUT_KEY: &str = "midi_clock_input";

/// Row ids on the settings page.
const PORT: &str = "midi_clock_port:";
const OFFSET: &str = "midi_clock_offset:";
const ADD: &str = "midi_clock_add";
const KEEP_RUNNING: &str = "midi_clock_keep_running";
const INPUT: &str = "midi_clock_input";

/// Offsets offered, in milliseconds.
const OFFSET_STEP: f64 = 5.0;

static CLOCK: OnceLock<MidiClock> = OnceLock::new();

/// Hands the saved outputs and Keep running to the engine; once, at start.
pub fn install(clock: MidiClock, registry: &ControlRegistry) {
    clock.set_outputs(saved());
    clock.set_input(saved_input());
    if let Some(c) = registry.get(GROUP, "keep_running") {
        c.set_bool(settings::flag(KEEP_RUNNING_KEY, false));
    }
    let _ = CLOCK.set(clock);
}

fn saved() -> Vec<OutputConfig> {
    settings::load()[OUTPUTS_KEY]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| {
                    Some(OutputConfig {
                        port: o["port"].as_str()?.to_string(),
                        offset_ms: o["offset_ms"].as_f64().unwrap_or(0.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The input clock is read from; None for none.
fn saved_input() -> Option<String> {
    Some(settings::string(INPUT_KEY)).filter(|p| !p.is_empty())
}

fn save(outputs: Vec<OutputConfig>) {
    let list: Vec<Value> = outputs
        .iter()
        .map(|o| json!({ "port": o.port, "offset_ms": o.offset_ms }))
        .collect();
    settings::set(OUTPUTS_KEY, json!(list));
    if let Some(c) = CLOCK.get() {
        c.set_outputs(outputs);
    }
}

fn offsets() -> Vec<f64> {
    let n = (MAX_OFFSET_MS / OFFSET_STEP) as i32;
    (-n..=n).map(|i| i as f64 * OFFSET_STEP).collect()
}

fn offset_name(ms: f64) -> String {
    if ms == 0.0 {
        "0 ms".into()
    } else if ms > 0.0 {
        format!("+{ms:.0} ms")
    } else {
        format!("−{:.0} ms", -ms)
    }
}

/// The name an output goes by in its menu.
fn port_label(port: &str) -> String {
    if port == VIRTUAL_PORT {
        format!("{VIRTUAL_PORT} (other apps)")
    } else {
        short_port_name(port).to_string()
    }
}

fn strings<S: AsRef<str>>(v: &[S]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        v.iter()
            .map(|s| SharedString::from(s.as_ref()))
            .collect::<Vec<_>>(),
    ))
}

fn row(id: &str, kind: i32, title: &str, detail: &str) -> PrefItem {
    PrefItem {
        id: id.into(),
        kind,
        title: title.into(),
        detail: detail.into(),
        ..Default::default()
    }
}

/// The section's rows: a heading, each output with its offset under it, a
/// + adding one, and Keep running.
pub fn items() -> Vec<PrefItem> {
    let outputs = saved();
    let open = CLOCK.get().map(MidiClock::status).unwrap_or_default();
    let mut ports = MidiClock::output_ports();
    // A saved output that isn't plugged in still shows in its menu.
    for o in &outputs {
        if !ports.contains(&o.port) {
            ports.push(o.port.clone());
        }
    }
    let labels: Vec<String> = ports.iter().map(|p| port_label(p)).collect();
    let offset_names: Vec<String> = offsets().into_iter().map(offset_name).collect();
    let mut items = Vec::new();
    for (i, o) in outputs.iter().enumerate() {
        let missing = open.iter().any(|s| s.port == o.port && !s.open);
        items.push(PrefItem {
            choices: strings(&labels),
            chosen: ports.iter().position(|p| *p == o.port).unwrap_or(0) as i32,
            ..row(
                &format!("{PORT}{i}"),
                15,
                &format!("Output {}", i + 1),
                if missing { "Not connected" } else { "" },
            )
        });
        items.push(PrefItem {
            choices: strings(&offset_names),
            chosen: offsets()
                .iter()
                .position(|&ms| (ms - o.offset_ms).abs() < 0.5)
                .unwrap_or(offset_names.len() / 2) as i32,
            ..row(
                &format!("{OFFSET}{i}"),
                1,
                "Offset",
                "+ when the gear plays early, − when it plays late.",
            )
        });
    }
    items.push(row(
        ADD,
        14,
        "Add output",
        if outputs.is_empty() {
            "Send clock to drum machines, synths or other apps. Turn it on with CLOCK OUT in the MASTER panel."
        } else {
            ""
        },
    ));
    // Clock in: a drum machine or another app leading the decks.
    let input = saved_input();
    let mut inputs = MidiClock::input_ports();
    if let Some(p) = input.as_ref().filter(|p| !inputs.contains(p)) {
        inputs.push(p.clone());
    }
    let mut choices = vec!["None".to_string()];
    choices.extend(inputs.iter().map(|p| short_port_name(p).to_string()));
    let not_open = input.is_some() && !CLOCK.get().is_some_and(MidiClock::input_open);
    items.push(PrefItem {
        choices: strings(&choices),
        chosen: input
            .as_ref()
            .and_then(|p| inputs.iter().position(|q| q == p))
            .map_or(0, |i| i + 1) as i32,
        ..row(
            INPUT,
            1,
            "Clock input",
            if not_open {
                "Not connected"
            } else {
                "Decks follow it with SYNC follows: Clock in the MASTER panel."
            },
        )
    });
    items.push(PrefItem {
        on: settings::flag(KEEP_RUNNING_KEY, false),
        ..row(
            KEEP_RUNNING,
            0,
            "Keep running when the decks stop",
            "Off sends Stop to the gear.",
        )
    });
    items
}

fn index(id: &str, prefix: &str) -> Option<usize> {
    id.strip_prefix(prefix)?.split('#').next()?.parse().ok()
}

/// A switch on the section; true when it was one of its rows.
pub fn toggled(registry: &ControlRegistry, id: &str, on: bool) -> bool {
    if id != KEEP_RUNNING {
        return false;
    }
    settings::set(KEEP_RUNNING_KEY, json!(on));
    if let Some(c) = registry.get(GROUP, "keep_running") {
        c.set_bool(on);
    }
    true
}

/// A menu on the section: an output's port or offset.
pub fn picked(id: &str, choice: usize) -> bool {
    if id == INPUT {
        let inputs = MidiClock::input_ports();
        let mut all = inputs.clone();
        if let Some(p) = saved_input().filter(|p| !inputs.contains(p)) {
            all.push(p);
        }
        let port = choice.checked_sub(1).and_then(|i| all.get(i).cloned());
        settings::set(INPUT_KEY, json!(port.clone().unwrap_or_default()));
        if let Some(c) = CLOCK.get() {
            c.set_input(port);
        }
        return true;
    }
    let mut outputs = saved();
    if let Some(i) = index(id, PORT) {
        let mut ports = MidiClock::output_ports();
        for o in &outputs {
            if !ports.contains(&o.port) {
                ports.push(o.port.clone());
            }
        }
        if let (Some(o), Some(p)) = (outputs.get_mut(i), ports.get(choice)) {
            o.port = p.clone();
        }
    } else if let Some(i) = index(id, OFFSET) {
        if let (Some(o), Some(ms)) = (outputs.get_mut(i), offsets().get(choice)) {
            o.offset_ms = *ms;
        }
    } else {
        return false;
    }
    save(outputs);
    true
}

/// + and × on the section.
pub fn pressed(id: &str) -> bool {
    let mut outputs = saved();
    if id == ADD {
        // The first output not used yet, else the first there is.
        let ports = MidiClock::output_ports();
        let Some(port) = ports
            .iter()
            .find(|p| !outputs.iter().any(|o| &o.port == *p))
            .or(ports.first())
        else {
            return true;
        };
        outputs.push(OutputConfig {
            port: port.clone(),
            offset_ms: 0.0,
        });
    } else if let Some(i) = index(id, PORT).filter(|_| id.ends_with("#remove")) {
        if i < outputs.len() {
            outputs.remove(i);
        }
    } else {
        return false;
    }
    save(outputs);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_run_from_minus_to_plus_fifty() {
        let o = offsets();
        assert_eq!(o.first(), Some(&-50.0));
        assert_eq!(o.last(), Some(&50.0));
        assert_eq!(o.len(), 21);
        assert_eq!(offset_name(-10.0), "−10 ms");
        assert_eq!(offset_name(15.0), "+15 ms");
    }

    #[test]
    fn row_ids_give_back_their_index() {
        assert_eq!(index("midi_clock_port:2#remove", PORT), Some(2));
        assert_eq!(index("midi_clock_offset:0", OFFSET), Some(0));
        assert_eq!(index("midi_clock_add", PORT), None);
    }
}
