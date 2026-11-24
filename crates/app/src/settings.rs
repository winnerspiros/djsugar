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
    FaderOn,
    FaderCurve,
    CutIn,
    CueMode,
    KeyMatch,
    AutoGain,
    GainTarget,
    BpmRange,
    SampleRate,
    Buffer,
    TempoMatch,
    SyncOnPlay,
    EndWarning,
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

/// Cut-in distances for the cut curve, as fractions of the travel.
pub const CUT_INS: [f32; 4] = [0.005, 0.01, 0.02, 0.05];
/// Loudness each track is brought to by auto gain, in dB.
pub const GAIN_TARGETS: [f32; 7] = [-8.0, -10.0, -12.0, -14.0, -16.0, -18.0, -23.0];
const GAIN_NAMES: [&str; 7] = ["-8 dB", "-10 dB", "-12 dB", "-14 dB", "-16 dB", "-18 dB", "-23 dB"];
/// Ranges detected tempos are folded into, by doubling or halving.
pub const BPM_RANGES: [(f64, f64); 6] =
    [(60.0, 120.0), (70.0, 140.0), (80.0, 160.0), (90.0, 180.0), (100.0, 200.0), (120.0, 240.0)];
const BPM_NAMES: [&str; 6] = ["60 - 120", "70 - 140", "80 - 160", "90 - 180", "100 - 200", "120 - 240"];

/// Sample rates and buffer sizes offered; the first of each is the device's own.
pub const RATES: [Option<u32>; 5] = [None, Some(44_100), Some(48_000), Some(88_200), Some(96_000)];
pub const BUFFERS: [Option<u32>; 7] = [None, Some(64), Some(128), Some(256), Some(512), Some(1024), Some(2048)];

/// Seconds before the end that the track end warning starts; 0 is off.
pub const END_WARNINGS: [u32; 4] = [0, 15, 30, 60];

pub const OPTIONS: [Spec; 24] = [
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
    Spec {
        opt: Opt::FaderOn,
        key: "xfader_enabled",
        title: "Crossfader",
        detail: "Off leaves both sides full, for mixing on the channel faders alone.",
        kind: Kind::Switch,
        default: 1,
    },
    Spec {
        opt: Opt::FaderCurve,
        key: "xfader_shape",
        title: "Crossfader curve",
        detail: "Smooth keeps both full in the middle; linear dips there; scratch cuts in at the very end.",
        kind: Kind::Segmented(&["Smooth", "Linear", "Scratch"]),
        default: 0,
    },
    Spec {
        opt: Opt::CutIn,
        key: "xfader_cut_in",
        title: "Cut-in",
        detail: "How far from its end the scratch curve brings a side fully in.",
        kind: Kind::Segmented(&["0.5%", "1%", "2%", "5%"]),
        default: 1,
    },
    Spec {
        opt: Opt::CueMode,
        key: "cue_mode",
        title: "CUE button",
        detail: "Press plays from the cue; Hold plays only while held and goes back on release.",
        kind: Kind::Segmented(&["Press", "Hold"]),
        default: 0,
    },
    Spec {
        opt: Opt::KeyMatch,
        key: "key_match_on_load",
        title: "Match key on load",
        detail: "Shifts a loaded track to the playing deck's key by the fewest semitones.",
        kind: Kind::Segmented(&["Off", "Match"]),
        default: 0,
    },
    Spec {
        opt: Opt::AutoGain,
        key: "auto_gain",
        title: "Auto gain",
        detail: "Brings each loaded track to the same loudness; the gain knob adds on top.",
        kind: Kind::Switch,
        default: 0,
    },
    Spec {
        opt: Opt::GainTarget,
        key: "auto_gain_target",
        title: "Auto gain level",
        detail: "The loudness auto gain aims for.",
        kind: Kind::Menu(&GAIN_NAMES),
        default: 3,
    },
    Spec {
        opt: Opt::BpmRange,
        key: "bpm_range",
        title: "BPM range",
        detail: "Detected tempos are doubled or halved into this range.",
        kind: Kind::Menu(&BPM_NAMES),
        default: 3,
    },
    Spec {
        opt: Opt::SampleRate,
        key: "sample_rate",
        title: "Sample rate",
        detail: "Takes effect the next time DJ Sugar starts.",
        kind: Kind::Menu(&["Device default", "44.1 kHz", "48 kHz", "88.2 kHz", "96 kHz"]),
        default: 0,
    },
    Spec {
        opt: Opt::Buffer,
        key: "buffer_frames",
        title: "Buffer size",
        detail: "Smaller answers faster; larger is safer from dropouts.",
        kind: Kind::Menu(&["Device default", "64", "128", "256", "512", "1024", "2048"]),
        default: 0,
    },
    Spec {
        opt: Opt::TempoMatch,
        key: "tempo_match_on_load",
        title: "Match tempo on load",
        detail: "A loaded track takes the playing deck's tempo.",
        kind: Kind::Switch,
        default: 0,
    },
    Spec {
        opt: Opt::SyncOnPlay,
        key: "sync_on_play",
        title: "Sync on play",
        detail: "Starting a deck while the other plays matches its tempo and lines up its beats.",
        kind: Kind::Switch,
        default: 0,
    },
    Spec {
        opt: Opt::EndWarning,
        key: "end_warning",
        title: "Track end warning",
        detail: "The deck's waveform flashes red this long before its track ends.",
        kind: Kind::Segmented(&["Off", "15 s", "30 s", "60 s"]),
        default: 2,
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

pub const PAGES: [&str; 8] = ["Audio", "Decks", "Mixer", "Keyboard", "Collection", "Analysis", "Appearance", "About"];

pub fn page(n: usize, roots: &[std::path::PathBuf], rate: u32, device: &str) -> Vec<Row> {
    match n {
        0 => vec![
            Row::Heading("OUTPUT"),
            Row::Info("Device", if device.is_empty() { "System default".into() } else { device.into() }),
            Row::Info("Running at", if rate > 0 { format!("{rate} Hz") } else { "No sound".into() }),
            Row::Opt(Opt::SampleRate),
            Row::Opt(Opt::Buffer),
        ],
        1 => vec![
            Row::Heading("TEMPO"),
            Row::Opt(Opt::RateRange),
            Row::Opt(Opt::TempoUp),
            Row::Heading("PLAYING"),
            Row::Opt(Opt::CueMode),
            Row::Opt(Opt::SyncOnPlay),
            Row::Opt(Opt::EndWarning),
            Row::Heading("TEMPO AND KEY ON LOAD"),
            Row::Opt(Opt::TempoMatch),
            Row::Opt(Opt::KeyMatch),
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
        2 => vec![
            Row::Heading("CROSSFADER"),
            Row::Opt(Opt::FaderOn),
            Row::Opt(Opt::FaderCurve),
            Row::Opt(Opt::CutIn),
            Row::Opt(Opt::ReverseFader),
            Row::Heading("LOUDNESS"),
            Row::Opt(Opt::AutoGain),
            Row::Opt(Opt::GainTarget),
        ],
        3 => {
            let mut v = Vec::with_capacity(SHORTCUTS.len() + 3);
            for (heading, keys) in SHORTCUTS {
                v.push(Row::Heading(heading));
                v.extend(keys.iter().map(|(k, what)| Row::Info(k, (*what).into())));
            }
            v
        }
        5 => vec![Row::Heading("TEMPO"), Row::Opt(Opt::BpmRange)],
        6 => vec![Row::Heading("KEYS"), Row::Opt(Opt::KeyNotation)],
        4 => {
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

type Keys = &'static [(&'static str, &'static str)];

/// Every keyboard shortcut, as the keyboard page lists them.
const SHORTCUTS: [(&str, Keys); 5] = [
    (
        "DECK 1 / DECK 2",
        &[
            ("D / L", "Play or pause; Shift sets the cue"),
            ("F / ;", "Cue; Shift stops at the cue"),
            ("A S / J K", "Beat jump back and forward; Shift by one beat"),
            ("1 / 6", "Sync"),
            ("T / Y", "Headphones"),
            ("Z X C V / M , . /", "Hot cues 1 to 4; Shift clears"),
            ("2 3 / 7 8", "Loop in and out; Shift jumps to them"),
            ("4 Q / 9 U", "Loop on or off; Shift with 4 or 9 also stops"),
            ("W E / I O", "Loop half or double"),
            ("B / N", "Low kill"),
        ],
    ),
    (
        "TEMPO",
        &[
            ("F1 F2 / F5 F6", "Tempo down and up; Shift in small steps"),
            ("F3 F4 / F7 F8", "Slow down or speed up while held"),
        ],
    ),
    ("MIXER", &[("G / H", "Crossfader a step left or right; Shift a small step"), ("Shift+F12", "Auto mix")]),
    (
        "LIBRARY",
        &[
            ("Up / Down", "Select; Page Up, Page Down, Home and End jump"),
            ("Enter", "Load the selected track"),
            ("Shift+Left / Right", "Load onto deck 1 or 2"),
            ("Ctrl+F", "Search"),
            ("Ctrl+N", "New playlist"),
        ],
    ),
    (
        "APP",
        &[
            ("Tab / Shift+Tab", "Next or previous page"),
            ("Space", "Decks or library"),
            ("Ctrl+Alt+1 to 6", "Go to a page"),
            ("Ctrl+R", "Record"),
            ("Ctrl+,", "Settings"),
            ("Escape or Back", "Close the open menu"),
            ("Right click", "The menu a long press opens"),
            ("Ctrl+=", "Zoom waveforms in"),
            ("Ctrl+-", "Zoom waveforms out"),
            ("Ctrl+0", "Reset waveform zoom"),
            ("Ctrl+Q", "Quit"),
        ],
    ),
];

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

/// The chosen BPM range, for analysis running away from the settings.
static BPM_RANGE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(3);

/// A detected grid folded into the chosen BPM range.
pub fn fold(g: sugar_analysis::Grid) -> sugar_analysis::Grid {
    let i = usize::from(BPM_RANGE.load(std::sync::atomic::Ordering::Relaxed));
    g.fold(BPM_RANGES[i.min(BPM_RANGES.len() - 1)])
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
        BPM_RANGE.store(s.get(Opt::BpmRange), std::sync::atomic::Ordering::Relaxed);
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
        if o == Opt::BpmRange {
            BPM_RANGE.store(v, std::sync::atomic::Ordering::Relaxed);
        }
        self.save();
    }

    pub fn curve(&self) -> sugar_engine::Curve {
        use sugar_engine::Curve;
        if !self.on(Opt::FaderOn) {
            return Curve::Off;
        }
        match self.get(Opt::FaderCurve) {
            1 => Curve::Linear,
            2 => Curve::Cut(CUT_INS[usize::from(self.get(Opt::CutIn)).min(CUT_INS.len() - 1)]),
            _ => Curve::Smooth,
        }
    }

    pub fn gain_target(&self) -> f32 {
        GAIN_TARGETS[usize::from(self.get(Opt::GainTarget)).min(GAIN_TARGETS.len() - 1)]
    }

    /// Seconds before a track's end that its deck warns; 0 is off.
    pub fn end_warning(&self) -> u32 {
        END_WARNINGS[usize::from(self.get(Opt::EndWarning)).min(END_WARNINGS.len() - 1)]
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
    fn options_sit_at_their_own_place() {
        for (i, o) in OPTIONS.iter().enumerate() {
            assert_eq!(o.opt as usize, i, "{}", o.key);
        }
    }

    #[test]
    fn options_are_in_enum_order() {
        for (i, o) in OPTIONS.iter().enumerate() {
            assert_eq!(o.opt as usize, i);
        }
    }

    #[test]
    fn curves_follow_the_switches() {
        use sugar_engine::Curve;
        let mut s = Settings::default();
        assert_eq!(s.curve(), Curve::Smooth);
        s.values[Opt::FaderCurve as usize] = 2;
        s.values[Opt::CutIn as usize] = 2;
        assert_eq!(s.curve(), Curve::Cut(0.02));
        s.values[Opt::FaderOn as usize] = 0;
        assert_eq!(s.curve(), Curve::Off);
    }

    #[test]
    fn tempos_fold_into_the_range() {
        let g = |bpm| sugar_analysis::Grid { bpm, first_beat: 0.0 };
        assert_eq!(g(65.0).fold(BPM_RANGES[3]).bpm, 130.0);
        assert_eq!(g(200.0).fold(BPM_RANGES[3]).bpm, 100.0);
        assert_eq!(g(174.0).fold(BPM_RANGES[1]).bpm, 87.0);
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
