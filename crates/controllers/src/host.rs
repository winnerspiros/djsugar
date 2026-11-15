//! What a controller script can reach: controls (with `_set_default`
//! style shortcuts and soft takeover), MIDI output, timers and the
//! scratch/brake filters. Script natives and MIDI input both go through here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_control::{Behavior, ConfigKey, ControlHandle, ControlRegistry};

use crate::pickup::{self, Takeover};

/// Shortcut controls Mixxx adds next to every knob, e.g. `pregain_set_default`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Sub {
    SetDefault,
    SetZero,
    SetOne,
    SetMinusOne,
    Up,
    Down,
    UpSmall,
    DownSmall,
    Toggle,
    MinusToggle,
}

const SUBS: [(&str, Sub); 10] = [
    ("_set_default", Sub::SetDefault),
    ("_set_zero", Sub::SetZero),
    ("_set_one", Sub::SetOne),
    ("_set_minus_one", Sub::SetMinusOne),
    ("_up_small", Sub::UpSmall),
    ("_down_small", Sub::DownSmall),
    ("_up", Sub::Up),
    ("_down", Sub::Down),
    ("_minus_toggle", Sub::MinusToggle),
    ("_toggle", Sub::Toggle),
];

/// Controls whose writes are steps to apply rather than positions: two
/// identical steps arriving between polls must both count, so writes add up
/// and whoever acts on them takes the total and sets it back to 0.
const RELATIVE: [&str; 11] = [
    "jog",
    "jog_bend",
    "beatjump",
    "loop_move",
    "loop_move_loops",
    "MoveVertical",
    "MoveHorizontal",
    "MoveFocus",
    "ScrollVertical",
    "SelectTrackKnob",
    "SelectPlaylist",
];

/// Whether writes to the control named `key` are steps rather than
/// positions (a jog wheel, the browse knob): see [`RELATIVE`].
pub(crate) fn is_relative(key: &str) -> bool {
    RELATIVE.contains(&key)
}

/// One-shot library actions. Each press is counted, so presses arriving
/// faster than the UI polls are not lost; the UI performs them and subtracts
/// what it handled. Only applies to the free controls the UI creates.
const ACTIONS: [&str; 18] = [
    "LoadSelectedTrack",
    "LoadSelectedTrackAndPlay",
    "LoadSelectedIntoFirstStopped",
    "SelectNextTrack",
    "SelectPrevTrack",
    "SelectNextPlaylist",
    "SelectPrevPlaylist",
    "MoveUp",
    "MoveDown",
    "MoveLeft",
    "MoveRight",
    "MoveFocusForward",
    "MoveFocusBackward",
    "GoToItem",
    "AutoDjAddBottom",
    "AutoDjAddTop",
    "ToggleSelectedSidebarItem",
    // DJ Sugar's own: opens the Linked tab of the side panel.
    "ShowLinked",
];

fn is_action(h: &ControlHandle) -> bool {
    h.behavior() == Behavior::Free && ACTIONS.contains(&h.key().item.as_str())
}

#[derive(Clone)]
pub(crate) enum Target {
    Control(ControlHandle),
    Sub(ControlHandle, Sub),
}

impl Target {
    pub(crate) fn get(&self) -> f64 {
        match self {
            Target::Control(h) => h.get(),
            Target::Sub(..) => 0.0,
        }
    }

    pub(crate) fn parameter(&self) -> f64 {
        match self {
            Target::Control(h) => h.get_parameter(),
            Target::Sub(..) => 0.0,
        }
    }

    fn handle(&self) -> &ControlHandle {
        match self {
            Target::Control(h) | Target::Sub(h, _) => h,
        }
    }

    /// The value a 0..1 parameter stands for on this control.
    pub(crate) fn value_for_parameter(&self, p: f64) -> f64 {
        let h = self.handle();
        let (min, max) = h.range();
        let d = h.default_value();
        match h.behavior() {
            Behavior::Free => p,
            Behavior::CenteredOnDefault => {
                let p = p.clamp(0.0, 1.0);
                if p <= 0.5 {
                    min + (d - min) * p * 2.0
                } else {
                    d + (max - d) * (p - 0.5) * 2.0
                }
            }
            _ => min + (max - min) * p.clamp(0.0, 1.0),
        }
    }

    pub(crate) fn parameter_for_value(&self, v: f64) -> f64 {
        let h = self.handle();
        let (min, max) = h.range();
        let d = h.default_value();
        match h.behavior() {
            Behavior::Free => v,
            Behavior::CenteredOnDefault => {
                if v <= d {
                    if d > min {
                        0.5 * (v - min) / (d - min)
                    } else {
                        0.5
                    }
                } else if max > d {
                    0.5 + 0.5 * (v - d) / (max - d)
                } else {
                    0.5
                }
            }
            _ if max > min => (v - min) / (max - min),
            _ => 0.0,
        }
    }

    /// Writes a value as a script would with `engine.setValue`.
    pub(crate) fn set(&self, v: f64) {
        match self {
            Target::Control(h) => {
                if is_action(h) {
                    if v > 0.0 {
                        h.set(h.get() + 1.0);
                    }
                } else if RELATIVE.contains(&h.key().item.as_str()) {
                    h.set(h.get() + v);
                } else {
                    h.set(v);
                }
            }
            Target::Sub(h, sub) => {
                if v > 0.0 {
                    apply_sub(h, *sub);
                }
            }
        }
    }

    pub(crate) fn default_value(&self) -> f64 {
        match self {
            Target::Control(h) => h.default_value(),
            Target::Sub(..) => 0.0,
        }
    }

    pub(crate) fn is_potmeter(&self) -> bool {
        matches!(self, Target::Control(h) if matches!(h.behavior(), Behavior::Linear | Behavior::CenteredOnDefault))
    }

    /// Writes a value arriving from a mapped MIDI message, the way Mixxx's
    /// `setValueFromMidi` does for each kind of control.
    pub(crate) fn set_from_midi(&self, note_off: bool, midi: f64) {
        let pressed = !(note_off || midi == 0.0);
        match self {
            Target::Sub(h, sub) => {
                if pressed {
                    apply_sub(h, *sub);
                }
            }
            Target::Control(h) => match h.behavior() {
                Behavior::Toggle => {
                    if pressed {
                        h.toggle();
                    }
                }
                Behavior::Push => h.set_bool(pressed),
                Behavior::Linear | Behavior::CenteredOnDefault => {
                    h.set_parameter(potmeter_parameter(midi))
                }
                Behavior::Free if is_action(h) => {
                    if pressed {
                        h.set(h.get() + 1.0);
                    }
                }
                Behavior::Free if RELATIVE.contains(&h.key().item.as_str()) => {
                    h.set(h.get() + midi)
                }
                Behavior::Free => h.set(midi / 127.0),
                Behavior::Indicator => h.set_parameter(midi / 127.0),
            },
        }
    }

    /// The control's value as a 0..127 MIDI value, for relative options.
    pub(crate) fn midi_parameter(&self) -> f64 {
        match self {
            Target::Sub(..) => 0.0,
            Target::Control(h) => match h.behavior() {
                Behavior::Linear | Behavior::CenteredOnDefault => {
                    let p = h.get_parameter();
                    if p > 0.5 {
                        p * 126.0 + 1.0
                    } else {
                        p * 128.0
                    }
                }
                Behavior::Free => h.get(),
                _ => h.get_parameter() * 127.0,
            },
        }
    }
}

/// Mixxx's potmeter MIDI curve: 64 is exactly the centre.
fn potmeter_parameter(midi: f64) -> f64 {
    if midi > 64.0 {
        (midi - 1.0) / 126.0
    } else {
        midi / 128.0
    }
}

fn apply_sub(h: &ControlHandle, sub: Sub) {
    let step = |d: f64| h.set_parameter(h.get_parameter() + d);
    match sub {
        Sub::SetDefault => h.reset(),
        Sub::SetZero => h.set(0.0),
        Sub::SetOne => h.set(1.0),
        Sub::SetMinusOne => h.set(-1.0),
        Sub::Up => step(0.1),
        Sub::Down => step(-0.1),
        Sub::UpSmall => step(0.01),
        Sub::DownSmall => step(-0.01),
        Sub::Toggle => h.set(if h.get() > 0.0 { 0.0 } else { 1.0 }),
        Sub::MinusToggle => h.set(if h.get() < 0.0 { 0.0 } else { -1.0 }),
    }
}

/// Mixxx's alpha-beta filter, which turns jog wheel ticks into a smooth
/// playback speed.
#[derive(Default)]
struct AlphaBeta {
    dt: f64,
    x: f64,
    v: f64,
    alpha: f64,
    beta: f64,
}

impl Scratch {
    /// Pulls the smooth filter to its quick twin when they disagree by
    /// more than the twin's wobble: all the way at twice `trust`, not at
    /// all within it.
    fn follow_quick(&mut self) {
        let gap = self.quick.v - self.filter.v;
        let w = ((gap.abs() - self.trust) / self.trust).clamp(0.0, 1.0);
        if w > 0.0 {
            self.filter.v += gap * w;
            self.filter.x += (self.quick.x - self.filter.x) * w;
        }
    }
}

impl AlphaBeta {
    fn init(&mut self, dt: f64, v: f64, alpha: f64, beta: f64) {
        *self = Self {
            dt,
            x: 0.0,
            v,
            alpha,
            beta,
        };
    }

    fn observation(&mut self, dx: f64) {
        let predicted_x = self.x + self.dt * self.v;
        let residual = dx - predicted_x;
        self.x = predicted_x + residual * self.alpha;
        self.v += residual * self.beta / self.dt;
        self.x -= dx;
    }
}

const SCRATCH_DT: f64 = 0.001;
/// DJ Sugar: how much faster the scratch filter's quick twin follows the
/// wheel than the mapping's own filter (alpha times this, beta times its
/// square). With Mixxx's usual 1/8 and 1/256 the deck heard a hand's
/// change of speed about 26 ms late and a fast baby scratch at two thirds
/// of its speed; the twin halves that lag.
const SCRATCH_QUICK: f64 = 3.0;
/// The quick twin's own wobble at normal speed, in multiples of normal
/// speed, times the wheel's ticks per second at normal speed (it wobbles
/// less the finer the wheel). Measured with the latency probe: 4% on the
/// DDJ-FLX4's 400 ticks a second.
const SCRATCH_QUICK_NOISE: f64 = 16.0;
/// The twins have to disagree by this many times that wobble before the
/// smooth filter is pulled to the quick one, so a steady hand still plays
/// as smoothly as before.
const SCRATCH_TRUST: f64 = 2.0;
const BRAKE_RAMP_TO: f64 = 0.01;
const MAX_DECKS: usize = 8;
/// Deck controls a controller holds on while a button is down, let go of
/// when the controller goes away (see [`Host::release_decks`]).
const HELD_KEYS: [&str; 6] = [
    "reverse",
    "reverseroll",
    "rate_temp_up",
    "rate_temp_down",
    "rate_temp_up_small",
    "rate_temp_down_small",
];

#[derive(Default)]
struct Scratch {
    /// Running at 1 kHz (scratch timer).
    active: bool,
    /// This controller turned the deck's `scratch2_enable` on and hasn't
    /// turned it off yet (see [`Host::release_decks`]).
    held: bool,
    dx: f64,
    accumulator: f64,
    ramp: bool,
    ramp_to: f64,
    brake: bool,
    spinback: bool,
    soft_start: bool,
    last_movement: Option<Instant>,
    filter: AlphaBeta,
    /// DJ Sugar: a quicker filter on the same ticks. While the hand changes
    /// speed faster than the smooth filter follows, the smooth one is
    /// pulled to it (see [`SCRATCH_QUICK`]).
    quick: AlphaBeta,
    /// How far apart the twins may drift (speed, multiples of normal)
    /// before the quick one leads: [`SCRATCH_TRUST`] times its wobble.
    trust: f64,
}

struct Watch {
    group: String,
    key: String,
    control: ControlHandle,
    version: u32,
}

/// `[Controller],deck3` and `deck4`: while on, a two-deck controller's left
/// side plays deck 3 instead of 1, and its right side deck 4 instead of 2,
/// as a deck select button does on four-deck controllers.
pub const LAYER_GROUP: &str = "[Controller]";
pub const LAYER_KEYS: [&str; 2] = ["deck3", "deck4"];

struct Timer {
    due: Instant,
    interval: Duration,
    one_shot: bool,
}

pub(crate) struct Host {
    pub(crate) registry: Arc<ControlRegistry>,
    pub(crate) name: String,
    /// Script errors, kept for the UI and tests.
    pub(crate) errors: Vec<String>,
    /// MIDI messages waiting to be sent.
    pub(crate) sent: Vec<Vec<u8>>,
    watches: Vec<Option<Watch>>,
    /// Controls `engine.trigger` named since the last poll. Their
    /// connections run later, not inside the script call that triggered them.
    triggered: Vec<ConfigKey>,
    /// Messages bound to a script function in the mapping XML; Mixxx gives
    /// these no `midi.makeInputHandler` callbacks.
    pub(crate) script_bound: HashSet<(u8, u8)>,
    /// How many `midi.makeInputHandler` callbacks each message has.
    input_handlers: HashMap<(u8, u8), u32>,
    timers: BTreeMap<u32, Timer>,
    next_timer: u32,
    soft: HashMap<(String, String), Takeover>,
    /// The message being handled, while a controller's MIDI is: writes it
    /// makes are the hardware moving.
    pub(crate) input: Option<(u8, u8)>,
    /// Whether hardware positions are shown on screen (a real controller,
    /// not the Controller Editor's probe).
    pub(crate) publish: bool,
    /// Messages the mapping also sends back, as motorised faders and LED
    /// rings do: the hardware follows the screen, so needs no marker.
    followed: HashSet<(u8, u8)>,
    scratch: Vec<Scratch>,
    scratch_tick: Instant,
    /// The deck 3 and 4 switches, for a mapping written for two decks; a
    /// mapping that names decks 3 or 4 itself has none.
    layers: Option<[ControlHandle; 2]>,
    /// The switches and deck shift as the watches last resolved them.
    layers_seen: RouteState,
    /// The deck this device plays (see [`crate::routing`]): its choice,
    /// shared with Settings, and the decks it can follow.
    route: Option<(Arc<std::sync::atomic::AtomicU8>, crate::routing::DeckWatch)>,
    /// How many decks along this device's decks are moved now.
    pub(crate) shift: usize,
    /// While set, every control a script writes and the value, by the name
    /// it acts on (`[Channel1],pregain_set_default` stays as written; layers
    /// and `[Main]` are resolved). Writes soft takeover drops still count.
    /// For the Controller Editor, which wants to know what a script does.
    pub(crate) trace: Option<Vec<(ConfigKey, f64)>>,
    /// DJ Sugar: notes a script moved with `midi.routeNote`, by MIDI channel:
    /// a pad pressed on the device acts as the note it is routed to, and that
    /// note's lights show on it. For pad mode buttons that open a pad page
    /// by place, whatever page is in it.
    note_routes: HashMap<u8, [u8; 128]>,
    /// The last value sent to each note, by status and note, to show on a
    /// pad newly routed to it.
    note_values: HashMap<(u8, u8), u8>,
    routes_changed: bool,
    /// An HID controller's device and the output reports for it.
    pub(crate) hid: Option<HidLink>,
}

/// What an HID mapping's `controller` object talks to.
pub(crate) struct HidLink {
    pub(crate) io: std::rc::Rc<dyn crate::hid::Io>,
    pub(crate) out: crate::hid::Outputs,
}

/// The deck 3 and 4 switches and the deck shift (see [`Host::route_state`]).
pub(crate) type RouteState = ([bool; 2], usize);

pub(crate) fn deck_group(deck: usize) -> String {
    format!("[Channel{deck}]")
}

impl Host {
    pub(crate) fn new(registry: Arc<ControlRegistry>, name: String) -> Self {
        Self {
            registry,
            name,
            errors: Vec::new(),
            sent: Vec::new(),
            note_routes: HashMap::new(),
            note_values: HashMap::new(),
            routes_changed: false,
            hid: None,
            watches: Vec::new(),
            triggered: Vec::new(),
            script_bound: HashSet::new(),
            input_handlers: HashMap::new(),
            timers: BTreeMap::new(),
            next_timer: 1,
            soft: HashMap::new(),
            input: None,
            publish: false,
            followed: HashSet::new(),
            scratch: (0..=MAX_DECKS).map(|_| Scratch::default()).collect(),
            scratch_tick: Instant::now(),
            layers: None,
            layers_seen: ([false; 2], 0),
            route: None,
            shift: 0,
            trace: None,
        }
    }

    /// Lets the deck 3 and 4 switches redirect this controller's decks.
    pub(crate) fn enable_layers(&mut self) {
        self.layers = Some(LAYER_KEYS.map(|k| self.registry.toggle(LAYER_GROUP, k)));
    }

    /// Which of the deck 3 and 4 switches are on.
    pub(crate) fn layer_state(&self) -> [bool; 2] {
        match &self.layers {
            Some(l) => [l[0].get_bool(), l[1].get_bool()],
            None => [false; 2],
        }
    }

    /// Lets the device's deck choice (Follow, a deck) move its decks.
    pub(crate) fn set_route(&mut self, route: Arc<std::sync::atomic::AtomicU8>) {
        let watch = crate::routing::DeckWatch::new(&self.registry);
        self.route = Some((route, watch));
        self.refresh_shift();
    }

    /// Works out again how far this device's decks move: the instrument it
    /// follows may have moved to another deck.
    pub(crate) fn refresh_shift(&mut self) {
        self.shift = match &self.route {
            Some((route, watch)) => {
                let choice = crate::routing::DeckChoice::from_u8(
                    route.load(std::sync::atomic::Ordering::Relaxed),
                );
                watch.shift(choice, false)
            }
            None => 0,
        };
    }

    /// The deck 3 and 4 switches and the deck shift: what moves this
    /// device's controls to other decks.
    pub(crate) fn route_state(&self) -> RouteState {
        (self.layer_state(), self.shift)
    }

    /// `group` with deck 1 or 2 swapped for 3 or 4 when that side is
    /// switched over (`[Channel1]`, its EQ and filter racks and its stems),
    /// then moved along to the deck the device follows.
    fn layered<'a>(&self, group: &'a str) -> std::borrow::Cow<'a, str> {
        let out = self.switched(group);
        if let std::borrow::Cow::Owned(s) = crate::routing::shift_group(&out, self.shift) {
            return std::borrow::Cow::Owned(s);
        }
        out
    }

    fn switched<'a>(&self, group: &'a str) -> std::borrow::Cow<'a, str> {
        let mut out = std::borrow::Cow::Borrowed(group);
        for (side, on) in self.layer_state().into_iter().enumerate() {
            if !on {
                continue;
            }
            let (from, to) = (side + 1, side + 3);
            for end in ["]", "_"] {
                let a = format!("[Channel{from}{end}");
                if out.contains(&a) {
                    out = std::borrow::Cow::Owned(out.replace(&a, &format!("[Channel{to}{end}")));
                }
            }
        }
        out
    }

    pub(crate) fn log(&self, message: &str) {
        eprintln!("[{}] {message}", self.name);
    }

    pub(crate) fn error(&mut self, message: &str) {
        eprintln!("[{}] error: {message}", self.name);
        if self.errors.len() < 100 {
            self.errors.push(message.to_string());
        }
    }

    /// Finds a control by name. Shortcut names (`rate_set_default`) resolve
    /// to their base control; names nothing implements become free controls
    /// so scripts can still keep state in them, as they do in Mixxx.
    pub(crate) fn resolve(&self, group: &str, key: &str) -> Target {
        let group = self.layered(group);
        let group = match group.as_ref() {
            "[Main]" => "[Master]",
            g => g,
        };
        if let Some((g, k)) = self.mixer_knob(group, key) {
            if let Some(h) = self.registry.get(&g, k) {
                return Target::Control(h);
            }
        }
        if let Some(h) = self.registry.get(group, key) {
            return Target::Control(h);
        }
        for (suffix, sub) in SUBS {
            if let Some(base) = key.strip_suffix(suffix) {
                if let Some(h) = self.registry.get(group, base) {
                    return Target::Sub(h, sub);
                }
            }
        }
        Target::Control(self.registry.free(group, key))
    }

    /// Where an EQ or filter knob goes while the mixer's side knobs are
    /// switched to stems or sculpt (`[Mixer],knob_mode` 1 or 2): the knob in
    /// the same place on screen, so a controller's high, mid, low and filter
    /// knobs work whatever the mixer shows.
    fn mixer_knob(&self, group: &str, key: &str) -> Option<(String, &'static str)> {
        let place = if let Some(rest) = group.strip_prefix("[EqualizerRack1_[Channel") {
            let deck = rest.strip_suffix("]_Effect1]")?;
            let place = match key {
                "parameter3" => 0,
                "parameter2" => 1,
                "parameter1" => 2,
                _ => return None,
            };
            (deck, place)
        } else {
            let rest = group.strip_prefix("[QuickEffectRack1_[Channel")?;
            if key != "super1" {
                return None;
            }
            (rest.strip_suffix("]]")?, 3)
        };
        let (deck, place) = place;
        deck.parse::<usize>().ok()?;
        let mode = self.registry.get("[Mixer]", "knob_mode")?.get();
        if mode == 1.0 {
            // Top to bottom: hi-hats, vocals, kick and bass, as on screen.
            let stem = [5, 4, 1, 2][place];
            Some((format!("[Channel{deck}_Stem{stem}]"), "volume"))
        } else if mode == 2.0 {
            let key = ["lpf", "freq", "gain", "hpf"][place];
            Some((format!("[Sculpt_[Channel{deck}]]"), key))
        } else {
            None
        }
    }

    /// Script writes. Values for knobs with soft takeover enabled may be
    /// dropped, or scaled.
    pub(crate) fn set_value(&mut self, group: &str, key: &str, v: f64) {
        let t = self.resolve(group, key);
        self.record(key, &t, v);
        match self.soft_take(group, key, &t, t.parameter_for_value(v)) {
            Some(p) => t.set(t.value_for_parameter(p)),
            None if self.soft_has(group, key) => {}
            None => t.set(v),
        }
    }

    pub(crate) fn set_parameter(&mut self, group: &str, key: &str, p: f64) {
        let t = self.resolve(group, key);
        self.record(key, &t, t.value_for_parameter(p));
        match self.soft_take(group, key, &t, p) {
            Some(p) => t.set(t.value_for_parameter(p)),
            None if self.soft_has(group, key) => {}
            None => t.set(t.value_for_parameter(p)),
        }
    }

    /// Notes a script write while tracing; `key` as the script named it.
    pub(crate) fn record(&mut self, key: &str, t: &Target, value: f64) {
        if let Some(trace) = &mut self.trace {
            let k = match t {
                Target::Control(h) => h.key().clone(),
                Target::Sub(h, _) => ConfigKey::new(h.key().group.clone(), key),
            };
            match trace.iter_mut().find(|(t, _)| *t == k) {
                // A press that writes 1 then 0 is still "1".
                Some((_, v)) if value != 0.0 => *v = value,
                Some(_) => {}
                None => trace.push((k, value)),
            }
        }
    }

    pub(crate) fn soft_has(&self, group: &str, key: &str) -> bool {
        self.soft
            .contains_key(&(group.to_string(), key.to_string()))
    }

    /// Soft takeover of a write of parameter `p`: the parameter to set, or
    /// `None` when it is dropped or the control has no soft takeover.
    pub(crate) fn soft_take(&mut self, group: &str, key: &str, t: &Target, p: f64) -> Option<f64> {
        let input = self.input;
        let publish = self.publish;
        let scale = pickup::scale(&crate::device_key(&self.name));
        let followed = input.is_some_and(|i| self.followed.contains(&i));
        let st = self.soft.get_mut(&(group.to_string(), key.to_string()))?;
        let out = st.take(t.parameter(), p, scale);
        if let (Some(i), Target::Control(h)) = (input, t) {
            st.source = Some(i);
            let k = h.key().clone();
            if publish && !followed {
                if let Some(old) = st.moved.as_ref().filter(|old| **old != k) {
                    pickup::forget(old, &self.name);
                }
                pickup::note(&k, p, &self.name);
            }
            st.moved = Some(k);
        }
        out
    }

    pub(crate) fn soft_takeover(&mut self, group: &str, key: &str, enable: bool) {
        let k = (group.to_string(), key.to_string());
        if !enable {
            if let Some(old) = self.soft.remove(&k).and_then(|st| st.moved) {
                pickup::forget(&old, &self.name);
            }
            return;
        }
        if self.soft.contains_key(&k) {
            return;
        }
        let t = self.resolve(group, key);
        if let Target::Control(h) = &t {
            if t.is_potmeter() && !pickup::always_jumps(h.key()) {
                self.soft.insert(k, Takeover::new());
            }
        }
    }

    pub(crate) fn soft_takeover_ignore_next(&mut self, group: &str, key: &str) {
        if let Some(st) = self.soft.get_mut(&(group.to_string(), key.to_string())) {
            st.ignore_next = true;
        }
    }

    pub(crate) fn soft_takeover_will_ignore(&self, group: &str, key: &str, p: f64) -> bool {
        match self.soft.get(&(group.to_string(), key.to_string())) {
            Some(st) => st.will_ignore(self.resolve(group, key).parameter(), p, Instant::now()),
            None => false,
        }
    }

    /// The mapping sent `msg`: a knob or fader it moves is driven from here
    /// too (motorised, or an LED ring), so the hardware is where the screen
    /// is and needs no marker.
    pub(crate) fn sent_back(&mut self, msg: &[u8]) {
        let [status, control, _] = *msg else { return };
        if status & 0xF0 != 0xB0 || self.followed.contains(&(status, control)) {
            return;
        }
        let moved: Vec<ConfigKey> = self
            .soft
            .values()
            .filter(|st| st.source == Some((status, control)))
            .filter_map(|st| st.moved.clone())
            .collect();
        if moved.is_empty() {
            return;
        }
        self.followed.insert((status, control));
        for k in moved {
            pickup::forget(&k, &self.name);
        }
    }

    /// A deck switch moved the hardware over to other controls: its markers
    /// move with it.
    pub(crate) fn layers_moved(&mut self) {
        if !self.publish {
            return;
        }
        let names: Vec<(String, String)> = self.soft.keys().cloned().collect();
        for (group, key) in names {
            let Target::Control(h) = self.resolve(&group, &key) else {
                continue;
            };
            let k = h.key().clone();
            let st = self.soft.get_mut(&(group, key)).unwrap();
            let (Some(source), Some(old), Some(p)) = (st.source, st.moved.clone(), st.hardware())
            else {
                continue;
            };
            if old == k || self.followed.contains(&source) {
                continue;
            }
            pickup::forget(&old, &self.name);
            pickup::note(&k, p, &self.name);
            st.moved = Some(k);
        }
    }

    pub(crate) fn watch(&mut self, group: &str, key: &str) -> u32 {
        let control = match self.resolve(group, key) {
            Target::Control(h) | Target::Sub(h, _) => h,
        };
        let version = control.version();
        self.watches.push(Some(Watch {
            group: group.to_string(),
            key: key.to_string(),
            control,
            version,
        }));
        self.watches.len() as u32
    }

    pub(crate) fn unwatch(&mut self, id: u32) {
        if let Some(w) = self.watches.get_mut(id.wrapping_sub(1) as usize) {
            *w = None;
        }
    }

    pub(crate) fn add_input_handlers(&mut self, status: u8, control: u8, delta: i32) {
        let n = self.input_handlers.entry((status, control)).or_insert(0);
        *n = n.saturating_add_signed(delta);
        if *n == 0 {
            self.input_handlers.remove(&(status, control));
        }
    }

    pub(crate) fn has_input_handlers(&self, status: u8, control: u8) -> bool {
        self.input_handlers.contains_key(&(status, control))
    }

    /// Queues the connections to a control to run on the next poll.
    pub(crate) fn trigger(&mut self, group: &str, key: &str) {
        let key = match self.resolve(group, key) {
            Target::Control(h) | Target::Sub(h, _) => h.key().clone(),
        };
        if !self.triggered.contains(&key) {
            self.triggered.push(key);
        }
    }

    /// Connections whose control changed or was triggered since the last
    /// call, with values.
    pub(crate) fn changed_watches(&mut self) -> Vec<(u32, f64)> {
        let triggered = std::mem::take(&mut self.triggered);
        // A side switched decks: every connection now follows the other
        // deck, and fires so the LEDs show it.
        let layers = self.route_state();
        let switched = layers != self.layers_seen;
        if switched {
            self.layers_seen = layers;
            let resolved: Vec<Option<ControlHandle>> = self
                .watches
                .iter()
                .map(|w| {
                    w.as_ref().map(|w| match self.resolve(&w.group, &w.key) {
                        Target::Control(h) | Target::Sub(h, _) => h,
                    })
                })
                .collect();
            for (w, c) in self.watches.iter_mut().zip(resolved) {
                if let (Some(w), Some(c)) = (w, c) {
                    w.control = c;
                }
            }
        }
        let mut out = Vec::new();
        for (i, w) in self.watches.iter_mut().enumerate() {
            if let Some(w) = w {
                let v = w.control.version();
                if switched || v != w.version || triggered.contains(w.control.key()) {
                    w.version = v;
                    out.push((i as u32 + 1, w.control.get()));
                }
            }
        }
        out
    }

    pub(crate) fn timer_start(&mut self, ms: f64, one_shot: bool) -> u32 {
        let id = self.next_timer;
        self.next_timer += 1;
        // Mixxx takes the interval as a 32-bit int and raises it to 20 ms;
        // Infinity converts to 0 there, so it gets 20 ms too.
        let ms = if ms.is_finite() {
            ms.clamp(20.0, i32::MAX as f64)
        } else {
            20.0
        };
        let interval = Duration::from_secs_f64(ms / 1000.0);
        self.timers.insert(
            id,
            Timer {
                due: Instant::now() + interval,
                interval,
                one_shot,
            },
        );
        id
    }

    pub(crate) fn timer_stop(&mut self, id: u32) {
        self.timers.remove(&id);
    }

    /// Timers that are due, rescheduling repeating ones.
    pub(crate) fn due_timers(&mut self, now: Instant) -> Vec<u32> {
        let due: Vec<u32> = self
            .timers
            .iter()
            .filter(|(_, t)| t.due <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in &due {
            let t = self.timers.get_mut(id).unwrap();
            if t.one_shot {
                self.timers.remove(id);
            } else {
                t.due += t.interval;
                if t.due < now {
                    t.due = now + t.interval;
                }
            }
        }
        due
    }

    /// When the controller next needs attention.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        let timer = self.timers.values().map(|t| t.due).min();
        let scratch = self
            .scratch
            .iter()
            .any(|s| s.active)
            .then(|| self.scratch_tick + Duration::from_millis(1));
        match (timer, scratch) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn control(&self, deck: usize, key: &str) -> ControlHandle {
        match self.resolve(&deck_group(deck), key) {
            Target::Control(h) | Target::Sub(h, _) => h,
        }
    }

    fn deck_rate(&self, deck: usize) -> f64 {
        let dir = if self.control(deck, "rate_dir").get() < 0.0 {
            -1.0
        } else {
            1.0
        };
        let rate =
            1.0 + self.control(deck, "rate").get() * self.control(deck, "rateRange").get() * dir;
        if self.control(deck, "reverse").get_bool() {
            -rate
        } else {
            rate
        }
    }

    fn deck_ok(deck: f64) -> Option<usize> {
        (deck >= 1.0 && deck <= MAX_DECKS as f64).then_some(deck as usize)
    }

    pub(crate) fn scratch_enable(
        &mut self,
        deck: f64,
        intervals_per_rev: f64,
        rpm: f64,
        alpha: f64,
        beta: f64,
        ramp: bool,
    ) {
        let Some(d) = Self::deck_ok(deck) else { return };
        let per_second = rpm * intervals_per_rev / 60.0;
        if per_second == 0.0 {
            self.log("Invalid rpm or intervalsPerRev supplied to scratchEnable");
            return;
        }
        let enable = self.control(d, "scratch2_enable");
        let mut v0 = 0.0;
        if ramp {
            if enable.get_bool() {
                v0 = self.control(d, "scratch2").get();
            } else if self.control(d, "play").get_bool() {
                v0 = self.deck_rate(d);
            }
        }
        let s = &mut self.scratch[d];
        s.dx = 1.0 / per_second;
        s.accumulator = 0.0;
        s.ramp = false;
        s.brake = false;
        let (alpha, beta) = if alpha != 0.0 && beta != 0.0 {
            (alpha, beta)
        } else {
            (1.0 / 8.0, (1.0 / 8.0) / 32.0)
        };
        s.filter.init(SCRATCH_DT, v0, alpha, beta);
        s.quick.init(
            SCRATCH_DT,
            v0,
            (alpha * SCRATCH_QUICK).min(1.0),
            (beta * SCRATCH_QUICK * SCRATCH_QUICK).min(0.5),
        );
        s.trust = SCRATCH_TRUST * SCRATCH_QUICK_NOISE / per_second;
        s.held = true;
        self.start_scratch_timer(d);
        enable.set_bool(true);
    }

    fn start_scratch_timer(&mut self, d: usize) {
        if !self.scratch.iter().any(|s| s.active) {
            self.scratch_tick = Instant::now();
        }
        self.scratch[d].active = true;
    }

    pub(crate) fn scratch_tick(&mut self, deck: f64, interval: f64) {
        let Some(d) = Self::deck_ok(deck) else { return };
        let s = &mut self.scratch[d];
        s.last_movement = Some(Instant::now());
        s.accumulator += interval;
    }

    pub(crate) fn scratch_disable(&mut self, deck: f64, ramp: bool) {
        let Some(d) = Self::deck_ok(deck) else { return };
        let mut ramp_to = 0.0;
        if !ramp {
            self.control(d, "scratch2_enable").set_bool(false);
            self.scratch[d].held = false;
        } else if self.control(d, "play").get_bool() {
            ramp_to = self.deck_rate(d);
        }
        let s = &mut self.scratch[d];
        s.ramp_to = ramp_to;
        s.last_movement = Some(Instant::now());
        s.ramp = true;
    }

    pub(crate) fn is_scratching(&self, deck: f64) -> bool {
        Self::deck_ok(deck).is_some_and(|d| self.control(d, "scratch2_enable").get_bool())
    }

    /// `engine.brake`, `engine.spinback` and `engine.softStart`, which Mixxx
    /// runs through the same filter as scratching.
    pub(crate) fn ramp(&mut self, deck: f64, kind: &str, activate: bool, factor: f64, rate: f64) {
        let Some(d) = Self::deck_ok(deck) else { return };
        let mut factor = factor;
        if factor > 1.0 {
            factor = (factor - 1.0) / 10.0 + 1.0;
        }
        let (alpha, beta) = (1.0 / 512.0, (1.0 / 512.0) / 1024.0 * factor);
        let enable = self.control(d, "scratch2_enable");
        self.scratch[d].held = activate;
        if kind == "softStart" {
            enable.set_bool(activate);
            let mut v0 = 0.0;
            let ramp_to = self.deck_rate(d);
            let s = &mut self.scratch[d];
            s.soft_start = activate;
            s.active = false;
            if !activate {
                return;
            }
            if s.brake || s.spinback {
                s.brake = false;
                s.spinback = false;
                v0 = s.filter.v;
            }
            s.ramp_to = ramp_to;
            s.filter.init(SCRATCH_DT, v0, alpha, beta);
            s.trust = 0.0;
            s.ramp = true;
            self.control(d, "play").set_bool(true);
            self.control(d, "scratch2").set(v0);
            self.start_scratch_timer(d);
            return;
        }
        enable.set_bool(activate);
        if !activate {
            let s = &mut self.scratch[d];
            s.brake = false;
            s.spinback = false;
            s.active = false;
            return;
        }
        let rate = if kind == "spinback" { rate } else { rate.abs() };
        let v0 = if rate < -BRAKE_RAMP_TO {
            let s = &mut self.scratch[d];
            s.spinback = true;
            s.brake = false;
            s.ramp_to = -BRAKE_RAMP_TO;
            rate
        } else if rate > BRAKE_RAMP_TO {
            let deck_rate = self.deck_rate(d);
            let s = &mut self.scratch[d];
            if s.spinback || s.brake {
                return;
            }
            s.brake = true;
            s.spinback = false;
            s.ramp_to = BRAKE_RAMP_TO;
            if s.soft_start {
                s.soft_start = false;
                s.filter.v
            } else {
                deck_rate
            }
        } else {
            let s = &mut self.scratch[d];
            s.brake = false;
            s.spinback = false;
            s.active = false;
            self.control(d, "play").set_bool(false);
            return;
        };
        self.control(d, "scratch2").set(v0);
        let s = &mut self.scratch[d];
        s.filter.init(SCRATCH_DT, v0, alpha, beta);
        s.trust = 0.0;
        s.ramp = true;
        self.start_scratch_timer(d);
    }

    pub(crate) fn ramp_active(&self, deck: f64, kind: &str) -> bool {
        Self::deck_ok(deck).is_some_and(|d| {
            let s = &self.scratch[d];
            match kind {
                "brake" => s.brake,
                "spinback" => s.spinback,
                _ => s.soft_start,
            }
        })
    }

    /// Runs the 1 kHz scratch filters for the time that has passed.
    pub(crate) fn process_scratch(&mut self, now: Instant) {
        if !self.scratch.iter().any(|s| s.active) {
            self.scratch_tick = now;
            return;
        }
        let elapsed = now.saturating_duration_since(self.scratch_tick);
        let ticks = (elapsed.as_secs_f64() / SCRATCH_DT) as u32;
        if ticks == 0 {
            return;
        }
        self.scratch_tick += Duration::from_secs_f64(ticks as f64 * SCRATCH_DT);
        // After a stall, catch up a little but don't spin for long.
        for _ in 0..ticks.min(50) {
            for d in 1..=MAX_DECKS {
                if self.scratch[d].active {
                    self.scratch_step(d, now);
                }
            }
        }
    }

    fn scratch_step(&mut self, d: usize, now: Instant) {
        let playing = self.control(d, "play").get_bool();
        let loaded = self.control(d, "track_loaded").get_bool();
        let s = &mut self.scratch[d];
        let idle = s
            .last_movement
            .is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_millis(1));
        // Ramping after the wheel was let go, or a soft start: steer towards
        // the target speed; otherwise follow the wheel.
        if s.soft_start || (s.ramp && idle) {
            s.filter.observation(s.ramp_to * SCRATCH_DT);
            // The quick twin waits where the smooth filter is, for the hand.
            s.quick.x = s.filter.x;
            s.quick.v = s.filter.v;
        } else {
            let dx = s.dx * s.accumulator;
            s.filter.observation(dx);
            if s.trust > 0.0 {
                s.quick.observation(dx);
                s.follow_quick();
            }
        }
        let rate = s.filter.v;
        s.accumulator = 0.0;
        let done = (s.ramp && (s.ramp_to - rate).abs() <= 0.00001)
            || (s.brake && rate < s.ramp_to)
            || ((s.spinback || s.soft_start) && rate > s.ramp_to)
            || ((s.brake || s.soft_start) && !playing)
            || !loaded;
        let stop = done && (s.brake || s.spinback);
        if done {
            s.ramp = false;
            s.active = false;
            s.dx = 0.0;
            s.brake = false;
            s.spinback = false;
            s.soft_start = false;
        }
        let scratch2 = self.control(d, "scratch2");
        scratch2.set(rate);
        if stop {
            scratch2.set(0.0);
            self.control(d, "play").set_bool(false);
        }
        if done {
            self.control(d, "scratch2_enable").set_bool(false);
            self.scratch[d].held = false;
        }
    }

    /// Lets go of every deck this controller is scratching, ramping, braking
    /// or holding, as if the hand came off the wheel at once. Called when the
    /// controller goes away (unplugged, the computer slept, Refresh): the
    /// touch-release it would have sent never comes, and a deck left with
    /// `scratch2_enable` on stops playing and only follows `scratch2`.
    pub(crate) fn release_decks(&mut self) {
        for d in 1..=MAX_DECKS {
            // Held by a button: the button's release never comes either.
            // Which controller pressed it isn't known, so a deck playing
            // backwards loses its reverse whichever controller goes away.
            for key in HELD_KEYS {
                if let Some(h) = self.registry.get(&format!("[Channel{d}]"), key) {
                    if h.get_bool() {
                        h.set(0.0);
                    }
                }
            }
            let s = &mut self.scratch[d];
            let busy = s.held || s.active || s.ramp || s.brake || s.spinback || s.soft_start;
            if !busy {
                continue;
            }
            *s = Scratch::default();
            let enable = self.control(d, "scratch2_enable");
            if enable.get_bool() {
                enable.set_bool(false);
                self.control(d, "scratch2").set(0.0);
            }
        }
    }
}

impl Host {
    /// Makes `from` on MIDI channel `channel` act as `to` (see
    /// `note_routes`); `from == to` undoes it.
    pub(crate) fn route_note(&mut self, channel: u8, from: u8, to: u8) {
        let routes = self
            .note_routes
            .entry(channel & 0x0F)
            .or_insert_with(|| std::array::from_fn(|i| i as u8));
        let (from, to) = ((from & 0x7F) as usize, to & 0x7F);
        if routes[from] != to {
            routes[from] = to;
            self.routes_changed = true;
        }
    }

    /// The note an incoming note on or off acts as.
    pub(crate) fn routed_input(&self, status: u8, control: u8) -> u8 {
        if !matches!(status & 0xF0, 0x80 | 0x90) {
            return control;
        }
        match self.note_routes.get(&(status & 0x0F)) {
            Some(routes) => routes[(control & 0x7F) as usize],
            None => control,
        }
    }

    /// The messages waiting to be sent, with the lights of routed notes
    /// moved onto the pads routed to them.
    pub(crate) fn take_routed(&mut self) -> Vec<Vec<u8>> {
        let sent = std::mem::take(&mut self.sent);
        if self.note_routes.is_empty() {
            return sent;
        }
        let mut out = Vec::with_capacity(sent.len());
        for m in sent {
            let routes = match m[..] {
                [status, _, _] if status & 0xF0 == 0x90 => self.note_routes.get(&(status & 0x0F)),
                _ => None,
            };
            let Some(routes) = routes else {
                out.push(m);
                continue;
            };
            let (status, note, value) = (m[0], m[1] & 0x7F, m[2]);
            self.note_values.insert((status, note), value);
            for (from, to) in routes.iter().enumerate() {
                if *to == note {
                    out.push(vec![status, from as u8, value]);
                }
            }
        }
        if std::mem::take(&mut self.routes_changed) {
            // Every routed pad shows what its note last showed.
            let mut statuses: Vec<u8> = self.note_values.keys().map(|(s, _)| *s).collect();
            statuses.sort_unstable();
            statuses.dedup();
            for status in statuses {
                let Some(routes) = self.note_routes.get(&(status & 0x0F)) else {
                    continue;
                };
                for (from, to) in routes.iter().enumerate() {
                    if let Some(v) = self.note_values.get(&(status, *to)) {
                        out.push(vec![status, from as u8, *v]);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        let reg = ControlRegistry::new();
        reg.create(
            "[Channel1]",
            "pregain",
            1.0,
            0.0,
            4.0,
            Behavior::CenteredOnDefault,
        );
        reg.create("[Channel1]", "rate", 0.0, -1.0, 1.0, Behavior::Linear);
        reg.create("[Channel1]", "rateRange", 0.08, 0.0, 1.0, Behavior::Linear);
        reg.toggle("[Channel1]", "play");
        reg.indicator("[Channel1]", "track_loaded", 1.0).set(1.0);
        reg.toggle("[Channel1]", "scratch2_enable");
        reg.toggle("[Channel1]", "reverse");
        reg.create(
            "[Channel1]",
            "scratch2",
            0.0,
            -100.0,
            100.0,
            Behavior::Linear,
        );
        Host::new(reg, "test".into())
    }

    #[test]
    fn shortcut_names_act_on_the_base_control() {
        let mut h = host();
        h.set_value("[Channel1]", "pregain", 3.0);
        h.set_value("[Channel1]", "pregain_set_default", 1.0);
        assert_eq!(h.resolve("[Channel1]", "pregain").get(), 1.0);
        h.set_value("[Channel1]", "rate_set_one", 1.0);
        assert_eq!(h.resolve("[Channel1]", "rate").get(), 1.0);
        h.set_value("[Channel1]", "rate_down", 1.0);
        assert!((h.resolve("[Channel1]", "rate").get() - 0.8).abs() < 1e-9);
        // Unknown names keep what scripts store in them.
        h.set_value("[EffectRack1_EffectUnit1]", "show_focus", 1.0);
        assert_eq!(
            h.resolve("[EffectRack1_EffectUnit1]", "show_focus").get(),
            1.0
        );
        // Relative controls add up.
        h.set_value("[Library]", "MoveVertical", 1.0);
        h.set_value("[Library]", "MoveVertical", 1.0);
        assert_eq!(h.resolve("[Library]", "MoveVertical").get(), 2.0);
    }

    #[test]
    fn soft_takeover_does_not_snap_back_after_an_outside_move() {
        let mut h = host();
        h.soft_takeover("[Channel1]", "rate", true);
        // The fader sits where the control is: taken.
        h.set_parameter("[Channel1]", "rate", 0.5);
        assert_eq!(h.control(1, "rate").get(), 0.0);
        // Something else moves the control within the grace period (a range
        // change, reset on load, sync, the mouse).
        h.control(1, "rate").set(-0.5);
        // The hardware's next message is stale: it must not snap it back.
        h.set_parameter("[Channel1]", "rate", 0.52);
        assert_eq!(h.control(1, "rate").get(), -0.5);
        // Reaching the new position picks it up again.
        h.set_parameter("[Channel1]", "rate", 0.24);
        assert!((h.control(1, "rate").get() + 0.52).abs() < 1e-9);
        // And fast moves from there are followed straight away.
        h.set_parameter("[Channel1]", "rate", 0.9);
        assert!((h.control(1, "rate").get() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn a_synced_follower_still_follows_its_tempo_fader() {
        let mut h = host();
        h.soft_takeover("[Channel1]", "rate", true);
        h.set_parameter("[Channel1]", "rate", 0.5);
        // Sync rewrites the follower's rate every buffer, drifting a hair
        // from what the fader set.
        h.control(1, "rate").set(0.003);
        // A fast fader move straight after is still followed.
        h.set_parameter("[Channel1]", "rate", 0.7);
        assert!((h.control(1, "rate").get() - 0.4).abs() < 1e-9);
        h.control(1, "rate").set(0.401);
        h.set_parameter("[Channel1]", "rate", 0.9);
        assert!((h.control(1, "rate").get() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn soft_takeover_waits_for_the_knob_to_catch_up() {
        let mut h = host();
        h.soft_takeover("[Channel1]", "pregain", true);
        // The first value is always dropped.
        h.set_parameter("[Channel1]", "pregain", 0.1);
        assert_eq!(h.resolve("[Channel1]", "pregain").get(), 1.0);
        // Still well below the knob's position on screen: ignored.
        std::thread::sleep(Duration::from_millis(60));
        h.set_parameter("[Channel1]", "pregain", 0.2);
        assert_eq!(h.resolve("[Channel1]", "pregain").get(), 1.0);
        // Close to the current value: taken from here on.
        h.set_parameter("[Channel1]", "pregain", 0.49);
        assert!(h.resolve("[Channel1]", "pregain").get() < 1.0);
        h.set_parameter("[Channel1]", "pregain", 0.7);
        assert!(h.resolve("[Channel1]", "pregain").get() > 1.0);
    }

    #[test]
    fn a_controller_that_goes_away_lets_go_of_the_deck() {
        let mut h = host();
        h.control(1, "play").set_bool(true);
        h.scratch_enable(1.0, 720.0, 100.0 / 3.0, 1.0 / 8.0, 1.0 / 256.0, false);
        h.scratch_tick(1.0, 3.0);
        h.process_scratch(Instant::now() + Duration::from_millis(5));
        assert!(h.is_scratching(1.0));
        // Unplugged with the hand on the wheel: no touch-release comes.
        h.release_decks();
        assert!(!h.is_scratching(1.0));
        assert_eq!(h.resolve("[Channel1]", "scratch2").get(), 0.0);
        assert!(h.control(1, "play").get_bool());
        // Nothing left running for the timer.
        assert!(h.scratch.iter().all(|s| !s.active));
    }

    #[test]
    fn a_controller_that_goes_away_lets_go_of_reverse() {
        let mut h = host();
        h.set_value("[Channel1]", "reverse", 1.0);
        h.release_decks();
        assert_eq!(h.resolve("[Channel1]", "reverse").get(), 0.0);
    }

    #[test]
    fn releasing_leaves_decks_other_controllers_hold_alone() {
        let mut h = host();
        // Another controller (or the screen) is scratching deck 1.
        h.control(1, "scratch2_enable").set_bool(true);
        h.release_decks();
        assert!(h.is_scratching(1.0));
    }

    /// The latency probe: a hand on a DDJ-FLX4-like wheel (720 ticks a
    /// turn, a MIDI message every 2 ms), fed through the 1 kHz filter.
    /// Returns the speed heard each millisecond.
    fn probe(hand: impl Fn(f64) -> f64, ms: u64) -> (Vec<f64>, Vec<f64>) {
        let (ipr, rpm) = (720.0, 100.0 / 3.0);
        let per_second = ipr * rpm / 60.0;
        let mut h = host();
        h.scratch_enable(1.0, ipr, rpm, 1.0 / 8.0, 1.0 / 256.0, false);
        let start = Instant::now();
        let (mut at, mut sent) = (0.0f64, 0.0f64);
        let (mut heard, mut wanted) = (Vec::new(), Vec::new());
        for ms in 0..ms {
            let t = ms as f64 / 1000.0;
            at += hand(t) * per_second / 1000.0;
            if ms % 2 == 0 {
                let ticks = at.trunc() - sent;
                sent += ticks;
                if ticks != 0.0 {
                    h.scratch_tick(1.0, ticks);
                }
            }
            h.process_scratch(start + Duration::from_millis(ms + 1));
            heard.push(h.resolve("[Channel1]", "scratch2").get());
            wanted.push(hand(t));
        }
        (heard, wanted)
    }

    #[test]
    fn scratch_latency_probe() {
        // Grabbing the record and pushing it at normal speed.
        let (heard, _) = probe(|_| 1.0, 1500);
        let reach = |f: f64| heard.iter().position(|&v| v >= f).unwrap() + 1;
        let (half, most) = (reach(0.5), reach(0.9));
        // A steady hand: the wobble left in the speed.
        let tail = &heard[500..];
        let mean = tail.iter().sum::<f64>() / tail.len() as f64;
        let wobble =
            (tail.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
        // Baby scratches: back and forth eight times a second, up to 3x speed.
        let (fast, hand) = probe(|t| 3.0 * (std::f64::consts::TAU * 8.0 * t).sin(), 1500);
        let peak = fast[300..].iter().cloned().fold(0.0, f64::max);
        let lag = (0..80)
            .min_by(|&a, &b| {
                let err = |l: usize| {
                    (300..1500)
                        .map(|i| (fast[i] - hand[i - l]).powi(2))
                        .sum::<f64>()
                };
                err(a).total_cmp(&err(b))
            })
            .unwrap();
        eprintln!(
            "scratch probe: 50% in {half} ms, 90% in {most} ms, wobble {:.2}%, \
             8 Hz scratch lag {lag} ms, peak {peak:.2} of 3",
            wobble * 100.0
        );
        // Mixxx's filter alone: 26 ms, 63 ms, 0.46%, 26 ms and 1.8.
        assert!(half <= 16, "{half} ms to half speed");
        assert!(most <= 40, "{most} ms to 90%");
        assert!(wobble < 0.01, "wobble {wobble}");
        assert!(lag <= 14, "8 Hz lag {lag} ms");
        assert!(peak > 2.5, "8 Hz peak {peak}");
    }

    #[test]
    fn scratch_ticks_become_speed() {
        let mut h = host();
        h.scratch_enable(1.0, 128.0, 33.0 + 1.0 / 3.0, 1.0 / 8.0, 1.0 / 256.0, false);
        assert!(h.is_scratching(1.0));
        // One revolution per 1.8 s is normal speed; feed that for 300 ms.
        let start = Instant::now();
        let per_ms = 128.0 * (100.0 / 3.0) / 60.0 / 1000.0;
        for ms in 1..=300u64 {
            h.scratch_tick(1.0, per_ms);
            h.process_scratch(start + Duration::from_millis(ms));
        }
        let speed = h.resolve("[Channel1]", "scratch2").get();
        assert!((speed - 1.0).abs() < 0.1, "{speed}");
        // Backwards.
        for ms in 301..=600u64 {
            h.scratch_tick(1.0, -per_ms);
            h.process_scratch(start + Duration::from_millis(ms));
        }
        let speed = h.resolve("[Channel1]", "scratch2").get();
        assert!((speed + 1.0).abs() < 0.1, "{speed}");
        h.scratch_disable(1.0, false);
        assert!(!h.is_scratching(1.0));
    }
}
