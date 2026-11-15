//! Keeps the Mixer's channels as they were left: each aux channel's TRIM,
//! PAN, fader, FX, on, solo, cue and mute, put back at launch and saved
//! (settings `mixer_strips`) as they change, looked at every couple of
//! seconds. The mic and the instrument keep only whether they are in the
//! Mixer and their strip controls; the mic's talkover never comes back on
//! by itself.

use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
use serde_json::{json, Map, Value};

use crate::settings;

/// Each aux channel's kept controls.
const AUX_KEYS: [&str; 8] = [
    "pregain", "pan", "volume", "fx", "main_mix", "solo", "pfl", "mute",
];
/// The mic's and the instrument's, as Mixer strips (not their on switch:
/// the mic's talkover, the instrument's main_mix that plays it on a deck).
const OTHER_KEYS: [&str; 7] = ["mixer", "pregain", "pan", "volume", "fx", "solo", "pfl"];

fn groups() -> Vec<(String, &'static [&'static str])> {
    (1..=8)
        .map(|n| (format!("[Auxiliary{n}]"), &AUX_KEYS[..]))
        .chain([
            ("[Microphone]".to_string(), &OTHER_KEYS[..]),
            ("[Instrument1]".to_string(), &OTHER_KEYS[..]),
        ])
        .collect()
}

pub struct Keeper {
    controls: Vec<(String, String, ControlHandle)>,
    seen: Vec<f64>,
    last_look: Instant,
}

impl Keeper {
    pub fn new(registry: &ControlRegistry) -> Self {
        let controls: Vec<(String, String, ControlHandle)> = groups()
            .into_iter()
            .flat_map(|(g, keys)| {
                keys.iter()
                    .filter_map(|k| Some((g.clone(), k.to_string(), registry.get(&g, k)?)))
                    .collect::<Vec<_>>()
            })
            .collect();
        restore(&controls, &settings::load()["mixer_strips"]);
        let seen = controls.iter().map(|(_, _, c)| c.get()).collect();
        Self {
            controls,
            seen,
            last_look: Instant::now(),
        }
    }

    /// Saves the channels if anything changed since the last look.
    pub fn tick(&mut self) {
        if self.last_look.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_look = Instant::now();
        let now: Vec<f64> = self.controls.iter().map(|(_, _, c)| c.get()).collect();
        if now == self.seen {
            return;
        }
        self.seen = now;
        settings::set("mixer_strips", snapshot(&self.controls));
    }
}

fn restore(controls: &[(String, String, ControlHandle)], saved: &Value) {
    for (g, k, c) in controls {
        if let Some(v) = saved[g.as_str()][k.as_str()].as_f64() {
            c.set(v);
        }
    }
}

fn snapshot(controls: &[(String, String, ControlHandle)]) -> Value {
    let mut all = Map::new();
    for (g, k, c) in controls {
        all.entry(g.clone())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("an object per group")
            .insert(k.clone(), json!(c.get()));
    }
    Value::Object(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(reg: &ControlRegistry) -> Vec<(String, String, ControlHandle)> {
        groups()
            .into_iter()
            .flat_map(|(g, keys)| {
                keys.iter()
                    .map(|k| {
                        let c = reg.get(&g, k).unwrap_or_else(|| panic!("{g},{k}"));
                        (g.clone(), k.to_string(), c)
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn a_channel_comes_back_as_it_was_left() {
        let reg = ControlRegistry::new();
        let (_engine, _input) = djsugar_engine::engine::Engine::new(&reg, 48_000.0);
        // Every kept control exists.
        let controls = kept(&reg);
        let get = |g: &str, k: &str| reg.get(g, k).unwrap();
        get("[Auxiliary3]", "pregain").set(2.0);
        get("[Auxiliary3]", "volume").set(0.4);
        get("[Auxiliary3]", "fx").set_bool(false);
        get("[Auxiliary3]", "main_mix").set_bool(true);
        get("[Microphone]", "mixer").set_bool(true);
        let saved = snapshot(&controls);

        let reg = ControlRegistry::new();
        let (_engine, _input) = djsugar_engine::engine::Engine::new(&reg, 48_000.0);
        restore(&kept(&reg), &saved);
        let get = |g: &str, k: &str| reg.get(g, k).unwrap().get();
        assert_eq!(get("[Auxiliary3]", "pregain"), 2.0);
        assert!((get("[Auxiliary3]", "volume") - 0.4).abs() < 1e-9);
        assert_eq!(get("[Auxiliary3]", "fx"), 0.0);
        assert_eq!(get("[Auxiliary3]", "main_mix"), 1.0);
        assert_eq!(get("[Microphone]", "mixer"), 1.0);
        // The mic's talkover isn't kept.
        assert_eq!(get("[Microphone]", "talkover"), 0.0);
    }
}
