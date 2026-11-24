//! The DJ's settings: what each option is, its choices, and the saved values.

use sugar_analysis::Key;

/// One setting, by its place in [`OPTIONS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Opt {
    RateRange,
    TempoUp,
    KeyLock,
    Quantize,
    Protect,
    PflOnLoad,
    ResetTempo,
    ResetEq,
    ResetGain,
    ReverseFader,
    KeyNotation,
}

/// How a setting is shown: a switch, or choices side by side or in a menu.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Switch,
    Segmented(&'static [&'static str]),
    Menu(&'static [&'static str]),
}

pub struct Spec {
    pub opt: Opt,
    pub key: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub kind: Kind,
    pub default: u8,
}

/// Tempo fader ranges, either way, as fractions of the speed.
pub const RATE_RANGES: [f64; 8] = [0.04, 0.06, 0.08, 0.10, 0.16, 0.25, 0.50, 0.90];
const RATE_NAMES: [&str; 8] = ["4%", "6%", "8%", "10%", "16%", "25%", "50%", "90%"];
const NOTATIONS: [&str; 4] = ["Camelot  8A", "Open Key  1m", "Sharps  A#m", "Flats  Bbm"];

pub const OPTIONS: [Spec; 11] = [
    Spec {
        opt: Opt::RateRange,
        key: "rate_range",
        title: "Tempo fader range",
        detail: "How far the tempo fader changes the speed, either way.",
        kind: Kind::Menu(&RATE_NAMES),
        default: 2,
    },
    Spec {
        opt: Opt::TempoUp,
        key: "tempo_up_faster",
        title: "Tempo fader direction",
        detail: "Which way the tempo fader speeds the deck up. Players slow down as the fader goes up.",
        kind: Kind::Segmented(&["Up slows down", "Up speeds up"]),
        default: 0,
    },
    Spec {
        opt: Opt::KeyLock,
        key: "keylock_default",
        title: "Key lock on at load",
        detail: "A loaded track keeps its key when the tempo changes.",
        kind: Kind::Switch,
        default: 1,
    },
    Spec {
        opt: Opt::Quantize,
        key: "quantize",
        title: "Quantize on at load",
        detail: "Cues, loops and jumps land on the beat.",
        kind: Kind::Switch,
        default: 1,
    },
    Spec {
        opt: Opt::Protect,
        key: "protect_playing",
        title: "Protect playing decks",
        detail: "A track can't be loaded onto a deck while it plays.",
        kind: Kind::Switch,
        default: 1,
    },
    Spec {
        opt: Opt::PflOnLoad,
        key: "pfl_on_load",
        title: "Headphones follow the loaded deck",
        detail: "Loading a track cues its deck in the headphones and takes the other deck out.",
        kind: Kind::Switch,
        default: 1,
    },
    Spec {
        opt: Opt::ResetTempo,
        key: "reset_tempo",
        title: "Tempo",
        detail: "The tempo fader goes back to the middle.",
        kind: Kind::Switch,
        default: 1,
    },
    Spec {
        opt: Opt::ResetEq,
        key: "reset_eq",
        title: "EQ",
        detail: "High, mid and low go back to the middle.",
        kind: Kind::Switch,
        default: 1,
    },
    Spec {
        opt: Opt::ResetGain,
        key: "reset_gain",
        title: "Gain",
        detail: "The gain knob goes back to the middle.",
        kind: Kind::Switch,
        default: 0,
    },
    Spec {
        opt: Opt::ReverseFader,
        key: "reverse_crossfader",
        title: "Reverse crossfader",
        detail: "Deck 2 sits on the left of the crossfader, deck 1 on the right.",
        kind: Kind::Switch,
        default: 0,
    },
    Spec {
        opt: Opt::KeyNotation,
        key: "key_notation",
        title: "Key notation",
        detail: "How keys read on the decks and in the library.",
        kind: Kind::Menu(&NOTATIONS),
        default: 0,
    },
];

pub fn spec(o: Opt) -> &'static Spec {
    &OPTIONS[OPTIONS.iter().position(|s| s.opt == o).unwrap_or(0)]
}

/// A line of a settings page.
pub enum Row {
    Heading(&'static str),
    Opt(Opt),
    /// A label and a value, shown, not set.
    Info(&'static str, String),
    Rescan,
}

pub const PAGES: [&str; 5] = ["Decks", "Mixer", "Appearance", "Collection", "About"];

pub fn page(n: usize, roots: &[std::path::PathBuf], rate: u32) -> Vec<Row> {
    match n {
        0 => vec![
            Row::Heading("TEMPO"),
            Row::Opt(Opt::RateRange),
            Row::Opt(Opt::TempoUp),
            Row::Heading("LOADING"),
            Row::Opt(Opt::KeyLock),
            Row::Opt(Opt::Quantize),
            Row::Opt(Opt::Protect),
            Row::Opt(Opt::PflOnLoad),
            Row::Heading("AUTO RESET ON LOAD"),
            Row::Opt(Opt::ResetTempo),
            Row::Opt(Opt::ResetEq),
            Row::Opt(Opt::ResetGain),
        ],
        1 => vec![Row::Heading("CROSSFADER"), Row::Opt(Opt::ReverseFader)],
        2 => vec![Row::Heading("KEYS"), Row::Opt(Opt::KeyNotation)],
        3 => {
            let mut v = vec![Row::Heading("MUSIC FOLDERS")];
            if roots.is_empty() {
                v.push(Row::Info("No music folder", "Put tracks in Music or Download".into()));
            }
            v.extend(roots.iter().map(|r| Row::Info("Folder", r.display().to_string())));
            v.push(Row::Rescan);
            v
        }
        _ => vec![
            Row::Heading("DJ SUGAR"),
            Row::Info("Version", env!("CARGO_PKG_VERSION").into()),
            Row::Info("Audio output", if rate > 0 { format!("{rate} Hz") } else { "Unavailable".into() }),
        ],
    }
}

/// Saved values, one per option.
#[derive(Clone, Debug)]
pub struct Settings {
    values: [u8; OPTIONS.len()],
}

impl Default for Settings {
    fn default() -> Self {
        Settings { values: OPTIONS.map(|s| s.default) }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Self::default();
        let text = crate::store::dir().and_then(|d| std::fs::read_to_string(d.join("settings.txt")).ok());
        for line in text.as_deref().unwrap_or("").lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            if let (Some(i), Ok(v)) = (OPTIONS.iter().position(|o| o.key == k), v.trim().parse()) {
                s.values[i] = v;
            }
        }
        s
    }

    fn save(&self) {
        let Some(d) = crate::store::dir() else { return };
        let text: String = OPTIONS.iter().zip(self.values).map(|(o, v)| format!("{}={v}\n", o.key)).collect();
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("settings.txt"), text);
    }

    pub fn get(&self, o: Opt) -> u8 {
        self.values[o as usize]
    }

    pub fn on(&self, o: Opt) -> bool {
        self.get(o) != 0
    }

    pub fn set(&mut self, o: Opt, v: u8) {
        self.values[o as usize] = v;
        self.save();
    }

    pub fn rate_range(&self) -> f64 {
        RATE_RANGES[usize::from(self.get(Opt::RateRange)).min(RATE_RANGES.len() - 1)]
    }

    /// A key as the DJ chose to read keys.
    pub fn key(&self, k: Key) -> String {
        const SHARPS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        const FLATS: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
        let m = if k.minor { "m" } else { "" };
        match self.get(Opt::KeyNotation) {
            1 => {
                // Open Key counts from C major as 1d, a fifth a step, as Camelot does from B.
                let n = (k.camelot_number() + 4) % 12 + 1;
                format!("{n}{}", if k.minor { 'm' } else { 'd' })
            }
            2 => format!("{}{m}", SHARPS[usize::from(k.tonic) % 12]),
            3 => format!("{}{m}", FLATS[usize::from(k.tonic) % 12]),
            _ => k.camelot(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_in_enum_order() {
        for (i, o) in OPTIONS.iter().enumerate() {
            assert_eq!(o.opt as usize, i);
        }
    }

    #[test]
    fn keys_read_in_each_notation() {
        let mut s = Settings::default();
        let a_minor = Key { tonic: 9, minor: true };
        let c_major = Key { tonic: 0, minor: false };
        assert_eq!(s.key(a_minor), "8A");
        assert_eq!(s.key(c_major), "8B");
        s.values[Opt::KeyNotation as usize] = 1;
        assert_eq!(s.key(a_minor), "1m");
        assert_eq!(s.key(c_major), "1d");
        s.values[Opt::KeyNotation as usize] = 3;
        assert_eq!(s.key(Key { tonic: 10, minor: true }), "Bbm");
    }
}
