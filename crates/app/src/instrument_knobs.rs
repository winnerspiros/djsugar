//! Which parameter each of the instrument's knobs turns. A right click on a
//! knob (`ParamKnob` in `ui/deck.slint`) searches all of the plugin's
//! parameters and puts the one picked there; the plugin restarts with its
//! sound to take it, and keeps the pick (beside its kept sound, and in a
//! preset saved from it).

use std::rc::Rc;

use djsugar_engine::AudioEngine;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{AppWindow, InstrumentsUi, ParamMatch};

/// The most parameters a search lists.
const SHOWN: usize = 300;

/// `all`'s parameters whose name holds every word of `text`, ignoring
/// case, in the plugin's order.
fn matches(all: &[(u32, String)], text: &str) -> Vec<(u32, String)> {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    all.iter()
        .filter(|(_, name)| {
            let name = name.to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        })
        .take(SHOWN)
        .cloned()
        .collect()
}

pub fn install(app: &AppWindow, engine: Rc<AudioEngine>) {
    let ui = app.global::<InstrumentsUi>();
    let weak = app.as_weak();
    let e = engine.clone();
    ui.on_search_params(move |text| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let found: Vec<ParamMatch> = matches(&e.instrument_all_parameters(), &text)
            .into_iter()
            .map(|(id, name)| ParamMatch {
                id: id as i32,
                name: SharedString::from(name),
            })
            .collect();
        app.global::<InstrumentsUi>()
            .set_param_matches(ModelRc::new(VecModel::from(found)));
    });
    ui.on_pick_knob(move |knob, id| {
        let id = u32::try_from(id).ok();
        if let Err(e) = engine.pick_instrument_knob(knob.max(0) as usize, id) {
            eprintln!("instrument knob {}: {e}", knob + 1);
        }
    });
}

/// Shows which knobs were picked and what each turns, when they changed.
pub fn show(app: &AppWindow, engine: &AudioEngine, last: &mut (Vec<(usize, u32)>, Vec<u32>)) {
    let now = (engine.instrument_picks(), engine.instrument_knob_ids());
    if now == *last {
        return;
    }
    let ui = app.global::<InstrumentsUi>();
    let mut picked = vec![false; now.1.len()];
    for &(knob, _) in &now.0 {
        if let Some(p) = picked.get_mut(knob) {
            *p = true;
        }
    }
    ui.set_knob_picked(ModelRc::new(VecModel::from(picked)));
    let ids: Vec<i32> = now.1.iter().map(|&id| id as i32).collect();
    ui.set_knob_ids(ModelRc::new(VecModel::from(ids)));
    *last = now;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_matches_every_word_in_any_case() {
        let all = vec![
            (1, "Filter Cutoff".to_string()),
            (2, "Filter Resonance".to_string()),
            (3, "Env Attack".to_string()),
        ];
        let ids = |text| {
            matches(&all, text)
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(""), [1, 2, 3]);
        assert_eq!(ids("filter"), [1, 2]);
        assert_eq!(ids("res FIL"), [2]);
        assert!(ids("lfo").is_empty());
    }
}
