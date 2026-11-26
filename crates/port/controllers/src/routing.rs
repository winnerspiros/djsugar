//! Which deck each device plays, and which devices play instruments. A DJ controller drives the decks its mapping names
//! ("As mapped").

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use djsugar_control::{ControlHandle, ControlRegistry};

use crate::manager::device_key;

/// The decks a device can be moved to.
pub const DECKS: usize = 4;

/// `[ChannelN],live_input` for the instrument plugin.
const LIVE_INSTRUMENT: f64 = 3.0;

/// Which deck a device plays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeckChoice {
    /// Follow for devices that play instruments, As mapped for the rest.
    #[default]
    Auto,
    /// The decks its mapping names.
    Mapped,
    /// The deck holding the instrument plugin or the Groove Box.
    Follow,
    /// This deck, 1 to 4: the mapping's deck 1 is moved there.
    Deck(u8),
}

impl DeckChoice {
    pub(crate) fn to_u8(self) -> u8 {
        match self {
            DeckChoice::Auto => 0,
            DeckChoice::Mapped => 1,
            DeckChoice::Follow => 2,
            DeckChoice::Deck(d) => 2 + d.clamp(1, DECKS as u8),
        }
    }

    pub(crate) fn from_u8(v: u8) -> Self {
        match v {
            1 => DeckChoice::Mapped,
            2 => DeckChoice::Follow,
            3..=6 => DeckChoice::Deck(v - 2),
            _ => DeckChoice::Auto,
        }
    }

    /// As kept in settings: "mapped", "follow", "1" to "4"; Auto is not kept.
    pub fn to_setting(self) -> Option<String> {
        match self {
            DeckChoice::Auto => None,
            DeckChoice::Mapped => Some("mapped".into()),
            DeckChoice::Follow => Some("follow".into()),
            DeckChoice::Deck(d) => Some(d.to_string()),
        }
    }

    pub fn from_setting(s: &str) -> Self {
        match s {
            "mapped" => DeckChoice::Mapped,
            "follow" => DeckChoice::Follow,
            d => match d.parse::<u8>() {
                Ok(d @ 1..=4) => DeckChoice::Deck(d),
                _ => DeckChoice::Auto,
            },
        }
    }

    /// What Auto means for a device: Follow when it plays instruments
    /// (`keys`), As mapped otherwise.
    pub fn resolved(self, keys: bool) -> Self {
        match self {
            DeckChoice::Auto if keys => DeckChoice::Follow,
            DeckChoice::Auto => DeckChoice::Mapped,
            c => c,
        }
    }
}

/// The choices, by device key; one cell each, shared with the device's
/// thread.
static ROUTES: Mutex<Option<HashMap<String, Arc<AtomicU8>>>> = Mutex::new(None);

/// The cell holding `port`'s choice, made Auto if it has none.
pub(crate) fn route(port: &str) -> Arc<AtomicU8> {
    let mut routes = ROUTES.lock().unwrap();
    routes.get_or_insert_with(HashMap::new).entry(device_key(port)).or_default().clone()
}

/// The deck `port` plays.
pub fn deck_choice(port: &str) -> DeckChoice {
    DeckChoice::from_u8(route(port).load(Ordering::Relaxed))
}

/// Sets the deck `port` plays; a running device moves at once.
pub fn set_deck_choice(port: &str, choice: DeckChoice) {
    route(port).store(choice.to_u8(), Ordering::Relaxed);
}

/// Every choice that isn't Auto, by device key, to keep in settings.
pub fn deck_choices() -> HashMap<String, DeckChoice> {
    ROUTES
        .lock()
        .unwrap()
        .iter()
        .flatten()
        .map(|(k, v)| (k.clone(), DeckChoice::from_u8(v.load(Ordering::Relaxed))))
        .filter(|(_, c)| *c != DeckChoice::Auto)
        .collect()
}

/// Takes the choices kept in settings, by device key.
pub fn load_deck_choices(choices: HashMap<String, DeckChoice>) {
    let mut routes = ROUTES.lock().unwrap();
    let routes = routes.get_or_insert_with(HashMap::new);
    for (key, choice) in choices {
        routes.entry(key).or_default().store(choice.to_u8(), Ordering::Relaxed);
    }
}

/// Whether `port` is a keyboard's DAW or Mackie Control port: it sends
/// transport buttons as notes, so it never plays an instrument on its own.
pub fn is_daw_port(port: &str) -> bool {
    port.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|t| matches!(t, "daw" | "mcu" | "hui" | "mackie"))
}

/// The deck controls an instrument or Groove Box lives on, looked up fast.
pub(crate) struct DeckWatch {
    live: Vec<ControlHandle>,
    groove: Vec<ControlHandle>,
}

impl DeckWatch {
    pub(crate) fn new(reg: &ControlRegistry) -> Self {
        let get = |g: String, k: &str| reg.free(&g, k);
        Self {
            live: (1..=DECKS).map(|d| get(format!("[Channel{d}]"), "live_input")).collect(),
            groove: (1..=DECKS).map(|d| get(format!("[Groove{d}]"), "enabled")).collect(),
        }
    }

    /// The deck, 1 to 4, playing the instrument plugin.
    pub(crate) fn instrument_deck(&self) -> Option<usize> {
        self.live.iter().position(|c| c.get().round() == LIVE_INSTRUMENT).map(|i| i + 1)
    }

    /// The deck a following device plays: the instrument plugin's, else the
    /// first Groove Box.
    pub(crate) fn followed(&self) -> Option<usize> {
        self.instrument_deck().or_else(|| self.groove.iter().position(|c| c.get_bool()).map(|i| i + 1))
    }

    /// How many decks along a device's decks move: 0 keeps them as mapped.
    pub(crate) fn shift(&self, choice: DeckChoice, keys: bool) -> usize {
        let target = match choice.resolved(keys) {
            DeckChoice::Follow => self.followed(),
            DeckChoice::Deck(d) => Some(d as usize),
            _ => None,
        };
        target.map_or(0, |t| (t - 1) % DECKS)
    }

    /// Whether a keyboard set to `choice` reaches the instrument now: one
    /// pinned to a deck only while the instrument is on that deck.
    pub(crate) fn plays(&self, choice: DeckChoice) -> bool {
        match choice {
            DeckChoice::Deck(d) => self.instrument_deck() == Some(d as usize),
            _ => true,
        }
    }
}

/// `group` with every deck number moved `shift` decks along, wrapping after
/// deck 4: `[Channel1]`, its racks (`[EqualizerRack1_[Channel1]_Effect1]`),
/// its stems and `[Groove1]`.
pub(crate) fn shift_group(group: &str, shift: usize) -> std::borrow::Cow<'_, str> {
    if shift == 0 || !(group.contains("[Channel") || group.contains("[Groove")) {
        return std::borrow::Cow::Borrowed(group);
    }
    let mut out = String::with_capacity(group.len());
    let mut rest = group;
    while !rest.is_empty() {
        let next = ["[Channel", "[Groove"].iter().filter_map(|p| rest.find(p).map(|i| (i, p.len()))).min();
        let Some((at, len)) = next else {
            out.push_str(rest);
            break;
        };
        let (head, tail) = rest.split_at(at + len);
        out.push_str(head);
        let digits = tail.chars().take_while(|c| c.is_ascii_digit()).count();
        match tail[..digits].parse::<usize>() {
            Ok(d @ 1..=DECKS) => out.push_str(&((d - 1 + shift) % DECKS + 1).to_string()),
            _ => out.push_str(&tail[..digits]),
        }
        rest = &tail[digits..];
    }
    std::borrow::Cow::Owned(out)
}

/// Which keyboard plays the instrument: the hash of its device key
/// ([`device_hash`]), 0 for any.
static SOURCE_DEVICE: AtomicU64 = AtomicU64::new(0);
/// And on which MIDI channel, 1 to 16; 0 for any.
static SOURCE_CHANNEL: AtomicU8 = AtomicU8::new(0);

/// A short fingerprint of `port`'s device, the same for each of its ports'
/// spellings (see [`device_key`]); never 0.
pub fn device_hash(port: &str) -> u64 {
    // FNV-1a, so it is the same on every run.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in device_key(port).bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h.max(1)
}

/// Lets only keyboard `device` (a port name, none for any) on `channel` (1 to 16, 0 for any) play the instrument. A
/// setup with several keyboards, or one sending several channels (a sequencer's tracks), then plays it from one of them
/// only.
pub fn set_instrument_source(device: Option<&str>, channel: u8) {
    SOURCE_DEVICE.store(device.map_or(0, device_hash), Ordering::Relaxed);
    SOURCE_CHANNEL.store(if channel <= 16 { channel } else { 0 }, Ordering::Relaxed);
}

/// Whether a message with `status` from the device hashed `device` may
/// play the instrument (see [`set_instrument_source`]).
pub(crate) fn from_source(device: u64, status: u8) -> bool {
    let want = SOURCE_DEVICE.load(Ordering::Relaxed);
    let channel = SOURCE_CHANNEL.load(Ordering::Relaxed);
    (want == 0 || want == device) && (channel == 0 || status & 15 == channel - 1)
}

/// The keyboards heard so far this run, and the last one heard.
#[derive(Default)]
struct Heard {
    seen: HashSet<String>,
    new: Vec<String>,
    last: Option<(String, Instant)>,
}

static HEARD: Mutex<Option<Heard>> = Mutex::new(None);

/// A keyboard sent a note to the instrument.
pub(crate) fn heard(port: &str) {
    let mut heard = HEARD.lock().unwrap();
    let h = heard.get_or_insert_with(Heard::default);
    if h.seen.insert(port.to_string()) {
        h.new.push(port.to_string());
    }
    match &mut h.last {
        Some((p, t)) if p == port => *t = Instant::now(),
        last => *last = Some((port.to_string(), Instant::now())),
    }
}

/// Keyboards that played the instrument for the first time since the last
/// call: for a notice saying so.
pub fn take_new_players() -> Vec<String> {
    HEARD.lock().unwrap().as_mut().map(|h| std::mem::take(&mut h.new)).unwrap_or_default()
}

/// The keyboard that played the instrument last, and when.
pub fn last_player() -> Option<(String, Instant)> {
    HEARD.lock().unwrap().as_ref().and_then(|h| h.last.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_chosen_keyboard_and_channel_play_the_instrument() {
        let (keys, pads) = (device_hash("KeyStep Pro"), device_hash("Maschine"));
        assert_ne!(keys, pads);
        assert_eq!(keys, device_hash("KeyStep Pro MIDI 1"), "any port of it");
        assert!(from_source(pads, 0x92));
        set_instrument_source(Some("KeyStep Pro"), 2);
        assert!(from_source(keys, 0x91), "channel 2");
        assert!(!from_source(keys, 0x90), "channel 1");
        assert!(!from_source(pads, 0x91));
        set_instrument_source(Some("KeyStep Pro"), 0);
        assert!(from_source(keys, 0x9f));
        set_instrument_source(None, 0);
        assert!(from_source(pads, 0x90));
    }

    #[test]
    fn groups_move_along_and_wrap() {
        assert_eq!(shift_group("[Channel1]", 2), "[Channel3]");
        assert_eq!(shift_group("[Channel2]", 3), "[Channel1]");
        assert_eq!(shift_group("[EqualizerRack1_[Channel1]_Effect1]", 1), "[EqualizerRack1_[Channel2]_Effect1]");
        assert_eq!(shift_group("[Groove1]", 3), "[Groove4]");
        assert_eq!(shift_group("[Master]", 2), "[Master]");
        assert_eq!(shift_group("[Channel1]", 0), "[Channel1]");
        // Racks numbered after the deck move only the deck.
        assert_eq!(shift_group("[QuickEffectRack1_[Channel2]]", 1), "[QuickEffectRack1_[Channel3]]");
    }

    #[test]
    fn choices_round_trip_through_settings() {
        for c in [DeckChoice::Mapped, DeckChoice::Follow, DeckChoice::Deck(1), DeckChoice::Deck(4)] {
            assert_eq!(DeckChoice::from_setting(&c.to_setting().unwrap()), c);
            assert_eq!(DeckChoice::from_u8(c.to_u8()), c);
        }
        assert_eq!(DeckChoice::Auto.to_setting(), None);
    }

    #[test]
    fn daw_ports_are_told_apart() {
        assert!(is_daw_port("Arturia KeyLab Essential 61 DAW In"));
        assert!(is_daw_port("Komplete Kontrol M DAW"));
        assert!(is_daw_port("KeyLab mk3 MCU"));
        assert!(!is_daw_port("Arturia KeyLab Essential 61 MIDI In"));
        assert!(!is_daw_port("Arturia MicroFreak"));
        assert!(!is_daw_port("XONE:K2"));
    }

    #[test]
    fn following_devices_go_where_the_instrument_is() {
        let reg = ControlRegistry::new();
        let w = DeckWatch::new(&reg);
        // Nothing to follow: as mapped.
        assert_eq!(w.shift(DeckChoice::Follow, true), 0);
        reg.get("[Groove2]", "enabled").unwrap().set(1.0);
        assert_eq!(w.shift(DeckChoice::Follow, false), 1);
        // The instrument plugin wins over a Groove Box.
        reg.get("[Channel4]", "live_input").unwrap().set(3.0);
        assert_eq!(w.shift(DeckChoice::Follow, false), 3);
        // Auto follows only for devices that play instruments.
        assert_eq!(w.shift(DeckChoice::Auto, true), 3);
        assert_eq!(w.shift(DeckChoice::Auto, false), 0);
        assert_eq!(w.shift(DeckChoice::Deck(2), false), 1);
        assert!(w.plays(DeckChoice::Deck(4)));
        assert!(!w.plays(DeckChoice::Deck(3)));
        assert!(w.plays(DeckChoice::Follow));
    }
}
