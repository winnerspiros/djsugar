//! The app's side of the Groove Box (`djsugar_engine::groove`): the kits'
//! and the groove library's names for the screen, and the Groove Boxes kept
//! across restarts in `settings.json` under `groove_box`, one object per
//! deck of its controls' values.

use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::groove::{groove_group, GROOVES, ROWS};
use djsugar_engine::NUM_DECKS;
use djsugar_engine::{kits, patterns};
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{settings, AppWindow, GrooveGenre, GrooveKit, GrooveName, GrooveUi};

/// The layout of the saved Groove Boxes. Before 2 there were four kits
/// (now from index 2) and four rows, whose sounds were numbered in each
/// kit's own order; before 3, nine rows (8 crash, 9 ride), now eight.
pub(crate) const LAYOUT: f64 = 3.0;

/// The controls a Groove Box keeps, by name.
pub(crate) fn kept_keys() -> Vec<String> {
    let mut keys: Vec<String> = [
        "enabled",
        "kit",
        "groove",
        "genre",
        "pattern",
        "chain",
        "bpm",
        "follow",
        "tempo_scale",
        "kick_key",
        "vary",
        "auto_fill",
        "swing",
        "duck_deck",
        "duck_amount",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for g in 1..=GROOVES {
        for r in 1..=ROWS {
            keys.push(format!("pattern{g}_row{r}"));
            keys.push(format!("pattern{g}_row{r}_soft"));
            keys.push(format!("pattern{g}_row{r}_accent"));
            keys.push(format!("pattern{g}_row{r}_maybe"));
        }
    }
    for r in 1..=ROWS {
        for k in [
            "mute", "solo", "level", "pitch", "decay", "tone", "pan", "sound", "lock",
        ] {
            keys.push(format!("row{r}_{k}"));
        }
    }
    keys
}

/// Shows the kits' names and the groove library's.
pub fn install(app: &AppWindow) {
    let ui = app.global::<GrooveUi>();
    let kits: Vec<GrooveKit> = kits::KITS
        .iter()
        .map(|k| GrooveKit {
            name: k.name.into(),
            about: k.about.into(),
            sounds: ModelRc::new(VecModel::from(
                k.sounds().map(SharedString::from).collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_kits(ModelRc::new(VecModel::from(kits)));
    let genres: Vec<GrooveGenre> = patterns::library()
        .iter()
        .map(|g| GrooveGenre {
            name: g.name.as_str().into(),
            grooves: ModelRc::new(VecModel::from(
                g.grooves
                    .iter()
                    .map(|p| GrooveName {
                        name: p.name.as_str().into(),
                        about: p.about.as_str().into(),
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_genres(ModelRc::new(VecModel::from(genres)));
}

/// Brings a Groove Box saved by an older DJ Sugar up to [`LAYOUT`]: the kit
/// it picked moves to its new place, and its rows' sounds go back to the
/// rows' own (rows 1-4 were already kick, snare, hat and open hat).
pub(crate) fn upgrade(values: &mut Map<String, Value>) {
    let layout = values.get("layout").and_then(Value::as_f64).unwrap_or(1.0);
    if layout < 2.0 {
        if let Some(kit) = values.get("kit").and_then(Value::as_f64) {
            values.insert("kit".into(), json!(kit + 2.0));
        }
        for r in 1..=4 {
            values.remove(&format!("row{r}_sound"));
        }
    }
    if layout < 3.0 {
        merge_ninth_row(values);
    }
}

/// Moves the old ninth row (ride) into row 8 (Cymbal) wherever row 8 (the
/// crash) has no hits, and has row 8 play the ride if it took any.
fn merge_ninth_row(values: &mut Map<String, Value>) {
    let num = |v: &Map<String, Value>, k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let mut took_ride = false;
    for g in 1..=GROOVES {
        let (crash, ride) = (format!("pattern{g}_row8"), format!("pattern{g}_row9"));
        if num(values, &crash) == 0.0 && num(values, &ride) != 0.0 {
            took_ride = true;
            for part in ["", "_soft", "_accent"] {
                let v = num(values, &format!("{ride}{part}"));
                values.insert(format!("{crash}{part}"), json!(v));
            }
        }
        for part in ["", "_soft", "_accent", "_maybe"] {
            values.remove(&format!("{ride}{part}"));
        }
    }
    if took_ride && num(values, "row8_sound") <= 7.0 {
        values.insert("row8_sound".into(), json!(8.0));
    }
    for k in [
        "mute", "solo", "level", "pitch", "decay", "tone", "pan", "sound",
    ] {
        values.remove(&format!("row9_{k}"));
    }
}

/// Keeps the Groove Boxes: puts back what was saved, then saves what
/// changes (looked at every couple of seconds).
pub struct Keeper {
    controls: Vec<Vec<(String, ControlHandle)>>,
    seen: Vec<Vec<f64>>,
    last_look: Instant,
}

impl Keeper {
    pub fn new(registry: &ControlRegistry) -> Self {
        let keys = kept_keys();
        let controls: Vec<Vec<(String, ControlHandle)>> = (0..NUM_DECKS)
            .map(|d| {
                let g = groove_group(d);
                keys.iter()
                    .filter_map(|k| Some((k.clone(), registry.get(&g, k)?)))
                    // The input the deck plays instead (see
                    // `crate::instruments`), kept beside its Groove Box.
                    .chain(
                        ["live_input", "live_trim", "live_pan"]
                            .into_iter()
                            .filter_map(|k| {
                                registry
                                    .get(&format!("[Channel{}]", d + 1), k)
                                    .map(|c| (k.to_string(), c))
                            }),
                    )
                    .collect()
            })
            .collect();
        let saved = settings::load()["groove_box"].clone();
        for (d, deck) in controls.iter().enumerate() {
            let Some(mut values) = saved[groove_group(d)].as_object().cloned() else {
                continue;
            };
            upgrade(&mut values);
            for (key, c) in deck {
                if let Some(v) = values.get(key).and_then(Value::as_f64) {
                    c.set(v);
                }
            }
        }
        let seen = snapshot(&controls);
        Self {
            controls,
            seen,
            last_look: Instant::now(),
        }
    }

    /// Saves the Groove Boxes if anything changed since the last look.
    pub fn tick(&mut self) {
        if self.last_look.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_look = Instant::now();
        let now = snapshot(&self.controls);
        if now == self.seen {
            return;
        }
        self.seen = now;
        let mut all = Map::new();
        for (d, deck) in self.controls.iter().enumerate() {
            let mut values: Map<String, Value> = deck
                .iter()
                .map(|(k, c)| (k.clone(), json!(c.get())))
                .collect();
            values.insert("layout".into(), json!(LAYOUT));
            all.insert(groove_group(d), Value::Object(values));
        }
        settings::set("groove_box", Value::Object(all));
    }
}

fn snapshot(controls: &[Vec<(String, ControlHandle)>]) -> Vec<Vec<f64>> {
    controls
        .iter()
        .map(|deck| deck.iter().map(|(_, c)| c.get()).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kept_control_exists() {
        let reg = ControlRegistry::new();
        let (_engine, _input) = djsugar_engine::engine::Engine::new(&reg, 48_000.0);
        for key in kept_keys() {
            assert!(reg.get("[Groove1]", &key).is_some(), "{key}");
        }
    }

    #[test]
    fn the_old_ninth_row_moves_into_the_cymbal_track() {
        let mut old = json!({
            "layout": 2.0,
            "pattern1_row9": 4369.0, "pattern1_row9_accent": 1.0,
            "pattern2_row8": 1.0, "pattern2_row9": 2.0,
            "row9_level": 0.5,
        });
        let v = old.as_object_mut().unwrap();
        upgrade(v);
        assert_eq!(v["pattern1_row8"], json!(4369.0), "the ride moves over");
        assert_eq!(v["pattern1_row8_accent"], json!(1.0));
        assert_eq!(v["pattern2_row8"], json!(1.0), "a crash stays");
        assert_eq!(v["row8_sound"], json!(8.0), "and plays the ride");
        assert!(v.keys().all(|k| !k.contains("row9")));
    }

    #[test]
    fn an_old_groove_box_moves_to_the_new_kits() {
        let mut old = json!({"kit": 1.0, "row2_sound": 1.0, "row2_level": 0.8});
        let values = old.as_object_mut().unwrap();
        upgrade(values);
        assert_eq!(values["kit"], json!(3.0), "Punch is now the fourth kit");
        assert!(values.get("row2_sound").is_none());
        assert_eq!(values["row2_level"], json!(0.8));
        let mut new = json!({"kit": 1.0, "layout": LAYOUT});
        upgrade(new.as_object_mut().unwrap());
        assert_eq!(new["kit"], json!(1.0));
    }
}
