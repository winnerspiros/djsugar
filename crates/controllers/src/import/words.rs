//! DJ Sugar's control names for the things every DJ app maps, shared by the
//! importers.

/// `[Channel2],play`.
pub(crate) fn deck(d: u8, item: &str) -> String {
    format!("[Channel{d}],{item}")
}

/// An EQ band of a deck: 1 low, 2 mid, 3 high.
pub(crate) fn eq(d: u8, band: u8) -> String {
    format!("[EqualizerRack1_[Channel{d}]_Effect1],parameter{band}")
}

pub(crate) fn eq_kill(d: u8, band: u8) -> String {
    format!("[EqualizerRack1_[Channel{d}]_Effect1],button_parameter{band}")
}

pub(crate) fn filter(d: u8) -> String {
    format!("[QuickEffectRack1_[Channel{d}]],super1")
}

pub(crate) fn fx_unit(unit: u8, item: &str) -> String {
    format!("[EffectRack1_EffectUnit{unit}],{item}")
}

pub(crate) fn fx_slot(unit: u8, slot: u8, item: &str) -> String {
    format!("[EffectRack1_EffectUnit{unit}_Effect{slot}],{item}")
}

pub(crate) fn master(item: &str) -> String {
    format!("[Master],{item}")
}

pub(crate) fn library(item: &str) -> String {
    format!("[Library],{item}")
}

/// A beat count as Mixxx writes it in control names: `0.5`, `4`.
pub(crate) fn beats(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The control that lights for a control a button presses: `play` lights
/// with `play_indicator`, `cue_default` with `cue_indicator`.
pub(crate) fn indicator(control: &str) -> String {
    let Some((group, key)) = crate::format::split_control(control) else {
        return control.to_string();
    };
    let key = match key.as_str() {
        "play" => "play_indicator".to_string(),
        "cue_default" => "cue_indicator".to_string(),
        k if k.starts_with("hotcue_") && k.ends_with("_activate") => {
            k.replace("_activate", "_enabled")
        }
        k if k.starts_with("beatloop_") && k.ends_with("_toggle") => {
            k.replace("_toggle", "_enabled")
        }
        "reloop_toggle" | "loop_in" | "loop_out" => "loop_enabled".to_string(),
        k => k.to_string(),
    };
    format!("{group},{key}")
}
