//! Settings brought over from Mixxx when its library is imported: the ones
//! DJ Sugar has an equivalent for, read from `mixxx.cfg` next to Mixxx's
//! database. A setting the DJ already chose here stays.

use std::path::Path;

use serde_json::{json, Value};

use crate::prefs::RATE_RANGES;
use djsugar_engine::key::Notation;

/// `mixxx.cfg` read as Mixxx writes it: `[Group]` lines, then `key value`
/// lines, the value running to the end of the line.
fn value<'a>(cfg: &'a str, group: &str, key: &str) -> Option<&'a str> {
    let mut in_group = false;
    for line in cfg.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_group = line == group;
        } else if in_group {
            let (k, v) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            if k == key {
                return Some(v.trim());
            }
        }
    }
    None
}

/// The settings `cfg` has equivalents for, as (DJ Sugar key, value).
pub fn from_config(cfg: &str) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    if let Some(n) = value(cfg, "[Key]", "KeyNotation") {
        let notation = match n {
            "Lancelot" | "Lancelot/Traditional" => Some(Notation::Camelot),
            "OpenKey" | "OpenKey/Traditional" => Some(Notation::OpenKey),
            "Traditional" => Some(Notation::Sharps),
            _ => None,
        };
        if let Some(n) = notation {
            out.push(("key_notation", json!(n.id())));
            out.push(("camelot_keys", json!(n == Notation::Camelot)));
        }
    }
    // The pitch fader's range, in percent: the nearest one we offer.
    if let Some(p) =
        value(cfg, "[Controls]", "RateRangePercent").and_then(|v| v.parse::<f64>().ok())
    {
        let range = p / 100.0;
        let nearest = RATE_RANGES
            .iter()
            .copied()
            .min_by(|a, b| (a - range).abs().total_cmp(&(b - range).abs()));
        if let Some(r) = nearest.filter(|_| p > 0.0) {
            out.push(("rate_range", json!(r)));
        }
    }
    // CUE when stopped: Numark's sets the cue and returns; every other
    // mode of Mixxx's previews while held, as ours does by default.
    if let Some(m) = value(cfg, "[Controls]", "CueDefault").and_then(|v| v.parse::<i32>().ok()) {
        out.push(("cue_mode", json!(if m == 3 { 3.0 } else { 0.0 })));
    }
    out
}

/// Fills in, from the `mixxx.cfg` beside `database`, the settings the DJ
/// hasn't set here. Returns how many were brought over.
pub fn bring_over(database: &Path) -> usize {
    let Some(cfg) = database
        .parent()
        .and_then(|d| std::fs::read_to_string(d.join("mixxx.cfg")).ok())
    else {
        return 0;
    };
    let ours = crate::settings::load();
    let mut n = 0;
    for (key, v) in from_config(&cfg) {
        if ours.get(key).is_none() {
            crate::settings::set(key, v);
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "[Config]\nVersion 2.5.0\n\n[Controls]\nCueDefault 3\nRateRangePercent 16\n\
                       RateDir -1\n\n[Key]\nKeyNotation OpenKey/Traditional\n\n[Library]\nRowHeight 20\n";

    #[test]
    fn reads_values_by_group() {
        assert_eq!(value(CFG, "[Controls]", "RateRangePercent"), Some("16"));
        assert_eq!(value(CFG, "[Config]", "Version"), Some("2.5.0"));
        assert_eq!(value(CFG, "[Key]", "RateRangePercent"), None);
    }

    #[test]
    fn brings_over_what_has_an_equivalent() {
        let got = from_config(CFG);
        assert!(got.contains(&("key_notation", json!(Notation::OpenKey.id()))));
        assert!(got.contains(&("camelot_keys", json!(false))));
        assert!(got.contains(&("rate_range", json!(0.16))));
        assert!(got.contains(&("cue_mode", json!(3.0))));
        assert_eq!(got.len(), 4);
    }

    #[test]
    fn takes_the_nearest_range_and_skips_what_it_cannot_read() {
        let got = from_config(
            "[Controls]\nRateRangePercent 10.5\nCueDefault 1\n[Key]\nKeyNotation Custom\n",
        );
        assert_eq!(
            got,
            vec![("rate_range", json!(0.10)), ("cue_mode", json!(0.0))]
        );
        assert!(from_config("").is_empty());
    }
}
