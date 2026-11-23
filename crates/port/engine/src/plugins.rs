//! The plugin host: starts a helper process for every effect slot that has a plugin loaded on a target its unit is routed to, attaches it to the audio thread, and watches it. The audio thread never waits for a plugin (see
//! `djsugar-plugin-host`): a block the helper can't deliver in time plays dry. This thread decides what happens next. A helper that crashes, or misses too many blocks in a row, is restarted once with its slot switched off;
//! the second time, or when it keeps using too much of the block's time, the plugin is switched off for the rest of the session and its slots are emptied. Nothing turns back on by itself in the middle of a set.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
pub use djsugar_plugin_host::presets as plugin_presets;
pub use djsugar_plugin_host::{presets as own_presets, scan, scan_audio_units, Descriptor, Limits, Plugin, ScanError};
use djsugar_plugin_host::{spawn, Action, Editor, Helper, Options, Policy, Status};

use crate::controls::{effect_group, effect_unit_group, EFFECTS_PER_UNIT, NUM_EFFECT_UNITS};
use crate::effects::{
    instance_indices, plugin_index, route_name, Attached, PluginLink, PluginMsg, BLOCK, MAIN, MAX_PARAMETERS,
    MAX_PLUGINS, PLUGIN_BASE, TARGETS,
};
use crate::inputs::{input_group, AUXILIARIES, AUXILIARY, INSTRUMENT, INSTRUMENT_PARAMETERS, MICROPHONE, NUM_INPUTS};

const INSTRUMENT_GROUP: &str = input_group(INSTRUMENT);

/// One plugin the slots can load, from the app's scan.
#[derive(Clone, Debug)]
pub struct PluginEntry {
    /// Stays the same between lists and sessions (format, file and id), so
    /// a slot keeps its plugin when the list changes around it.
    pub key: String,
    pub name: String,
    pub plugin: Plugin,
}

/// What happened to a plugin, for the app to tell the DJ.
#[derive(Clone, Debug, PartialEq)]
pub enum PluginEvent {
    /// It didn't start; its slot is emptied.
    Failed { name: String, error: String },
    /// It crashed or hung and was restarted; its slot is switched off.
    Restarted { name: String },
    /// It crashed again, or ran too slow: off for the rest of the session.
    Disabled { name: String, reason: String },
    /// Its own window was asked for and can't be shown.
    NoWindow { name: String, reason: String },
}

impl std::fmt::Display for PluginEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginEvent::Failed { name, error } => {
                write!(f, "{name} didn't start ({error}), so its slot is empty.")
            }
            PluginEvent::Restarted { name } => write!(
                f,
                "{name} stopped working and was restarted. The music kept playing; switch the effect on again when you want it."
            ),
            PluginEvent::Disabled { name, reason } => write!(
                f,
                "{name} is off until you restart DJ Sugar: {reason}. The music kept playing."
            ),
            PluginEvent::NoWindow { name, reason } => {
                write!(f, "{name}'s window can't open: {reason}.")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostOptions {
    /// The `djsugar-plugin-host` binary.
    pub helper: PathBuf,
    pub sample_rate: u32,
    /// How often the controls and the helpers are checked.
    pub poll: Duration,
    pub limits: Limits,
}

impl HostOptions {
    pub fn new(helper: PathBuf, sample_rate: u32) -> Self {
        Self { helper, sample_rate, poll: Duration::from_millis(50), limits: Limits::default() }
    }
}

#[derive(Default)]
struct List {
    entries: Vec<PluginEntry>,
    /// The instruments the Instrument input can load (see
    /// [`PluginHost::set_instruments`]).
    instruments: Vec<PluginEntry>,
    version: u64,
    /// Where each plugin's last sound is kept (see
    /// [`PluginHost::set_sound_folder`]); none keeps nothing.
    sounds: Option<PathBuf>,
    /// Asked of the instrument, served on the plugin host thread.
    requests: Vec<SoundRequest>,
    /// The playing instrument's parameter names, as its knobs show them.
    instrument_params: Vec<String>,
    /// Every parameter of the playing instrument, (id, name), to pick from
    /// for a knob.
    instrument_all: Vec<(u32, String)>,
    /// The knobs the DJ picked a parameter for on the playing instrument:
    /// (knob, parameter id).
    instrument_picks: Vec<(usize, u32)>,
    /// The parameter id each of the playing instrument's knobs turns.
    instrument_ids: Vec<u32>,
}

/// What the app asks of the instrument's sound (see
/// [`PluginHost::instrument_sound`]).
enum SoundRequest {
    Save(Sender<Result<(String, Vec<u8>), String>>),
    Load(String, Vec<u8>, Sender<Result<(), String>>),
    /// Puts parameter id (none: the plugin's own choice) on a knob of the
    /// playing instrument.
    Pick(usize, Option<u32>, Sender<Result<(), String>>),
    /// All of plugin `key`'s knob picks, as a preset has them.
    Picks(String, Vec<(usize, u32)>, Sender<Result<(), String>>),
    /// One of plugin `key`'s own presets, by the reference its preset list
    /// gave (see `djsugar_plugin_host::presets`).
    Preset(String, String, Sender<Result<(), String>>),
}

/// How long the app waits for a plugin to hand over or take a sound.
const SOUND_WAIT: Duration = Duration::from_secs(8);

/// Runs the plugin host thread; dropping it stops the thread and every helper.
pub struct PluginHost {
    list: Arc<Mutex<List>>,
    num_plugins: ControlHandle,
    events: Receiver<PluginEvent>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl PluginHost {
    pub fn start(reg: &ControlRegistry, link: PluginLink, opts: HostOptions) -> Self {
        let list = Arc::new(Mutex::new(List::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let mut sup = Supervisor::new(reg, link, opts, list.clone(), tx);
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("plugin host".into())
                .spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        sup.tick();
                        std::thread::sleep(sup.opts.poll);
                    }
                    sup.shut_down();
                })
                .expect("the plugin host thread starts")
        };
        Self {
            list,
            num_plugins: reg.indicator("[Master]", "num_plugins", MAX_PLUGINS as f64),
            events,
            stop,
            thread: Mutex::new(Some(thread)),
        }
    }

    /// The plugins the slots can load: `loaded_effect` [`PLUGIN_BASE`] + i is `entries[i]`.
    /// With a new list, slots keep their plugin (by key) at its new place, slots whose
    /// plugin left are emptied, and every plugin gets a fresh start.
    pub fn set_plugins(&self, mut entries: Vec<PluginEntry>) {
        entries.truncate(MAX_PLUGINS);
        let n = entries.len();
        let mut list = self.list.lock().unwrap();
        list.entries = entries;
        list.version += 1;
        self.num_plugins.set(n as f64);
    }

    /// The instruments the Instrument input can load:
    /// `[Instrument1],instrument` i + 1 is `entries[i]`, 0 none. With a new
    /// list the input keeps its instrument (by key) if it is still there.
    pub fn set_instruments(&self, mut entries: Vec<PluginEntry>) {
        entries.truncate(MAX_PLUGINS);
        let mut list = self.list.lock().unwrap();
        list.instruments = entries;
        list.version += 1;
    }

    /// Keeps each plugin's sound (its whole state) in `folder`: saved when
    /// it stops, is swapped out, its window closes or DJ Sugar quits, and
    /// loaded when it starts again, so tweaks made in its window last.
    pub fn set_sound_folder(&self, folder: PathBuf) {
        self.list.lock().unwrap().sounds = Some(folder);
    }

    /// The Instrument input's plugin and its whole sound now, for a preset:
    /// (its key, its state). Waits for the plugin, a few seconds at most.
    pub fn instrument_sound(&self) -> Result<(String, Vec<u8>), String> {
        let (tx, rx) = mpsc::channel();
        self.list.lock().unwrap().requests.push(SoundRequest::Save(tx));
        rx.recv_timeout(SOUND_WAIT).unwrap_or_else(|_| Err("the instrument didn't answer".into()))
    }

    /// Gives instrument plugin `key` the sound `data` (a preset's): now if
    /// it is playing, else when it next starts. Waits for the plugin, a few
    /// seconds at most.
    pub fn load_instrument_sound(&self, key: &str, data: Vec<u8>) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.list.lock().unwrap().requests.push(SoundRequest::Load(key.into(), data, tx));
        rx.recv_timeout(SOUND_WAIT).unwrap_or_else(|_| Err("the instrument didn't answer".into()))
    }

    /// Loads one of instrument plugin `key`'s own presets, by the reference its preset
    /// list gave: now if it is playing, else as soon as it starts. The sound it makes
    /// is kept as the plugin's. Waits for the plugin, a few seconds at most.
    pub fn load_instrument_preset(&self, key: &str, reference: &str) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.list.lock().unwrap().requests.push(SoundRequest::Preset(key.into(), reference.into(), tx));
        rx.recv_timeout(SOUND_WAIT).unwrap_or_else(|_| Err("the instrument didn't answer".into()))
    }

    /// The playing instrument's parameter names: `[Instrument1],parameterN`
    /// is the N-th. Empty while none plays.
    pub fn instrument_parameters(&self) -> Vec<String> {
        self.list.lock().unwrap().instrument_params.clone()
    }

    /// Every parameter of the playing instrument, (id, name) in its own
    /// order, to pick from for a knob. Empty while none plays.
    pub fn instrument_all_parameters(&self) -> Vec<(u32, String)> {
        self.list.lock().unwrap().instrument_all.clone()
    }

    /// The knobs the DJ picked a parameter for on the playing instrument:
    /// (knob from 0, parameter id), for a preset.
    pub fn instrument_picks(&self) -> Vec<(usize, u32)> {
        self.list.lock().unwrap().instrument_picks.clone()
    }

    /// The parameter id each of the playing instrument's knobs turns.
    pub fn instrument_knob_ids(&self) -> Vec<u32> {
        self.list.lock().unwrap().instrument_ids.clone()
    }

    /// Puts the playing instrument's parameter `id` on knob `knob` (from
    /// 0), or gives the knob back to the plugin's own choice. The plugin
    /// restarts with its sound to take it, and keeps the pick from then on.
    pub fn pick_instrument_knob(&self, knob: usize, id: Option<u32>) -> Result<(), String> {
        self.ask(|tx| SoundRequest::Pick(knob, id, tx))
    }

    /// Sets all of instrument plugin `key`'s knob picks (a preset's),
    /// restarting it if it plays and they changed.
    pub fn set_instrument_picks(&self, key: &str, picks: Vec<(usize, u32)>) -> Result<(), String> {
        self.ask(|tx| SoundRequest::Picks(key.into(), picks, tx))
    }

    fn ask(&self, request: impl FnOnce(Sender<Result<(), String>>) -> SoundRequest) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.list.lock().unwrap().requests.push(request(tx));
        rx.recv_timeout(SOUND_WAIT).unwrap_or_else(|_| Err("the instrument didn't answer".into()))
    }

    /// What happened since the last call.
    pub fn events(&self) -> Vec<PluginEvent> {
        self.events.try_iter().collect()
    }
}

impl PluginHost {
    /// Keeps every running plugin's sound and stops them all, waiting until
    /// done. For quitting: the app may exit without dropping the engine.
    pub fn shut_down(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.lock().unwrap().take() {
            let _ = t.join();
        }
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.shut_down();
    }
}

struct SlotHandles {
    loaded_effect: ControlHandle,
    enabled: ControlHandle,
    params: [ControlHandle; MAX_PARAMETERS],
    num_parameters: ControlHandle,
    show_editor: ControlHandle,
}

/// A helper running for one instance.
struct Running {
    plugin: u16,
    /// The plugin's key, for its sound file.
    key: String,
    helper: Helper,
    unit: usize,
    slot: usize,
    /// Its editor was asked for (see [`Supervisor::follow_editors`]).
    editor: bool,
}

/// Where an instance plays, for its window's title.
fn target_name(target: usize) -> String {
    if target < MAIN {
        format!("Deck {}", target + 1)
    } else if target == MAIN {
        "Master".into()
    } else {
        match (target - MAIN - 1).min(NUM_INPUTS - 1) {
            MICROPHONE => "Mic".into(),
            AUXILIARY => "Aux".into(),
            INSTRUMENT => "Instrument".into(),
            i => format!("Aux {}", AUXILIARIES.iter().position(|&a| a == i).unwrap_or(0) + 1),
        }
    }
}

/// Keeps a helper's editor in step with a `show_editor` control: asks for
/// it, takes it away, and turns the control off when the window was closed
/// or can't open. Returns why it can't, when it can't.
fn follow_editor(helper: &Helper, asked: &mut bool, control: &ControlHandle, want: bool) -> Option<String> {
    if want && !*asked {
        helper.set_editor(true);
        *asked = true;
    } else if !want && *asked {
        helper.set_editor(false);
        *asked = false;
    } else if *asked && !helper.editor_asked() {
        // The helper let go of it: the DJ closed the window, or it failed.
        *asked = false;
        control.set_bool(false);
        if helper.editor() == Editor::Missing {
            return Some(if cfg!(any(target_os = "macos", windows)) {
                "it has no window of its own".into()
            } else {
                "it has no window of its own, or there is no X11 display (XWayland) to show it on".into()
            });
        }
    }
    None
}

struct Supervisor {
    opts: HostOptions,
    list: Arc<Mutex<List>>,
    version: u64,
    entries: Vec<PluginEntry>,
    events: Sender<PluginEvent>,
    link: PluginLink,
    slots: Vec<[SlotHandles; EFFECTS_PER_UNIT]>,
    routes: Vec<[ControlHandle; TARGETS]>,
    index: crate::effects::Indices,
    running: Vec<Option<Running>>,
    /// The plugin whose own values each slot's knobs were set to. A plugin
    /// newly chosen gets its knobs from the plugin; a restarted one, or one
    /// on another target, keeps them as the DJ left them.
    knobs: Vec<[Option<u16>; EFFECTS_PER_UNIT]>,
    /// Per plugin, this session.
    policies: HashMap<u16, Policy>,
    disabled: HashSet<u16>,
    instrument: InstrumentState,
    /// When open windows' sounds are next kept (see
    /// [`Supervisor::keep_open_sounds`]).
    next_keep: Instant,
}

/// The Instrument input's plugin.
struct InstrumentState {
    entries: Vec<PluginEntry>,
    /// `[Instrument1],instrument`: 1 + its place in `entries`, 0 for none.
    choice: ControlHandle,
    /// `[Instrument1],main_mix`, switched off after a crash.
    on: ControlHandle,
    running: Option<(u16, Helper)>,
    /// The running plugin's key, for its sound file.
    running_key: String,
    /// `[Instrument1],show_editor`, and whether its editor was asked for.
    show_editor: ControlHandle,
    editor: bool,
    /// `[Instrument1],parameterN` and `num_parameters`.
    params: Vec<ControlHandle>,
    num_parameters: ControlHandle,
    /// Following the plugin's own changes to its parameters.
    follow: ParamFollow,
    policies: HashMap<u16, Policy>,
    disabled: HashSet<u16>,
    /// A plugin's own preset to load once it starts: (key, reference).
    pending_preset: Option<(String, String)>,
}

/// Where `value` sits in `min..max`, 0 to 1.
fn normalized(value: f64, min: f64, max: f64) -> f64 {
    let span = max - min;
    if span > 0.0 {
        ((value - min) / span).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// How long after the DJ moves a knob the plugin's own value for it is
/// left alone, so a value read back late doesn't pull the knob back.
const HANDS_OFF: Duration = Duration::from_millis(400);

/// Moves the instrument's knobs when the plugin changes its own values
/// (in its window, or with a loaded sound), except one the DJ is turning.
#[derive(Default)]
struct ParamFollow {
    /// Each parameter's range.
    ranges: Vec<(f64, f64)>,
    /// The read-back the values below are from.
    seq: u32,
    /// The plugin's values last read back, 0 to 1.
    seen: Vec<f64>,
    /// Each knob's value at the last step, and when the DJ last moved it.
    knob: Vec<f64>,
    touched: Vec<Option<Instant>>,
}

impl ParamFollow {
    fn new(knobs: &[ControlHandle], ranges: Vec<(f64, f64)>) -> Self {
        let now: Vec<f64> = knobs.iter().map(|k| k.get()).collect();
        Self { ranges, seq: 0, seen: now.clone(), touched: vec![None; now.len()], knob: now }
    }

    /// Takes read-back `seq`'s `values` (in the plugin's own ranges).
    fn step(&mut self, knobs: &[ControlHandle], seq: u32, values: &[f32]) {
        let now = Instant::now();
        for (k, knob) in self.knob.iter_mut().enumerate() {
            let v = knobs[k].get();
            if v != *knob {
                *knob = v;
                self.touched[k] = Some(now);
            }
        }
        if seq == self.seq {
            return;
        }
        self.seq = seq;
        for (k, &raw) in values.iter().enumerate().take(self.seen.len()) {
            let (min, max) = self.ranges[k];
            let v = normalized(raw as f64, min, max);
            // Left for later while the DJ turns it, so a change the plugin
            // made meanwhile still lands.
            let busy = self.touched[k].is_some_and(|t| now - t < HANDS_OFF);
            if busy || (v - self.seen[k]).abs() < 1e-4 {
                continue;
            }
            self.seen[k] = v;
            if (v - self.knob[k]).abs() >= 1e-4 {
                knobs[k].set(v);
                self.knob[k] = v;
            }
        }
    }
}

/// How often an open window's plugin has its sound kept.
const KEEP_EVERY: Duration = Duration::from_secs(20);

/// The file plugin `key`'s last sound is kept in: the key's hash, since
/// keys hold paths.
fn sound_file(folder: &Path, key: &str) -> PathBuf {
    // FNV-1a: the same file for the same key on every run and version.
    let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    folder.join(format!("{hash:016x}.sound"))
}

/// The file plugin `key`'s knob picks are kept in, beside its sound: one
/// `knob id` pair a line.
fn picks_file(folder: &Path, key: &str) -> PathBuf {
    sound_file(folder, key).with_extension("knobs")
}

fn kept_picks(list: &Mutex<List>, key: &str) -> Vec<(usize, u32)> {
    let Some(folder) = list.lock().unwrap().sounds.clone() else {
        return Vec::new();
    };
    std::fs::read_to_string(picks_file(&folder, key))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (knob, id) = l.split_once(' ')?;
            Some((knob.parse().ok()?, id.parse().ok()?))
        })
        .collect()
}

fn keep_picks(list: &Mutex<List>, key: &str, picks: &[(usize, u32)]) {
    let Some(folder) = list.lock().unwrap().sounds.clone() else {
        return;
    };
    let file = picks_file(&folder, key);
    if picks.is_empty() {
        let _ = std::fs::remove_file(file);
        return;
    }
    let text: String = picks.iter().map(|(k, id)| format!("{k} {id}\n")).collect();
    let _ = std::fs::create_dir_all(&folder);
    if let Err(e) = std::fs::write(&file, text) {
        eprintln!("plugin {key}: its knob picks weren't kept: {e}");
    }
}

/// `picks` with knob `knob` given parameter `id` (none: back to the
/// plugin's own choice). A parameter sits on one knob at a time.
fn with_pick(mut picks: Vec<(usize, u32)>, knob: usize, id: Option<u32>) -> Vec<(usize, u32)> {
    picks.retain(|&(k, i)| k != knob && Some(i) != id);
    if let Some(id) = id {
        picks.push((knob, id));
    }
    picks.sort_unstable();
    picks
}

/// The sound plugin `key` last had, if one was kept.
fn kept_sound(list: &Mutex<List>, key: &str) -> Option<Vec<u8>> {
    let folder = list.lock().unwrap().sounds.clone()?;
    let data = std::fs::read(sound_file(&folder, key)).ok()?;
    eprintln!("plugin {key}: starting from its kept sound ({} bytes)", data.len());
    Some(data)
}

/// Keeps a running plugin's sound for next time. A helper that crashed or
/// hung is left alone: its last kept sound stays.
fn keep_sound(list: &Mutex<List>, key: &str, helper: &Helper) {
    let Some(folder) = list.lock().unwrap().sounds.clone() else {
        return;
    };
    if key.is_empty() {
        return;
    }
    if helper.is_gone() || helper.missed_in_row() > 0 {
        eprintln!("plugin {key}: crashed or stuck, so its last kept sound stays");
        return;
    }
    let data = match helper.save_state() {
        Ok(data) => data,
        Err(e) => {
            eprintln!("plugin {key}: its sound wasn't kept: {e}");
            return;
        }
    };
    let file = sound_file(&folder, key);
    let tmp = file.with_extension("tmp");
    let _ = std::fs::create_dir_all(&folder);
    let kept = std::fs::write(&tmp, &data).and_then(|()| std::fs::rename(&tmp, &file));
    match kept {
        Ok(()) => eprintln!("plugin {key}: sound kept ({} bytes)", data.len()),
        Err(e) => eprintln!("plugin {key}: its sound wasn't kept: {e}"),
    }
}

fn get(reg: &ControlRegistry, group: &str, item: &str) -> ControlHandle {
    reg.get(group, item).unwrap_or_else(|| panic!("{group},{item} exists: the rack makes it"))
}

impl Supervisor {
    fn new(
        reg: &ControlRegistry,
        link: PluginLink,
        opts: HostOptions,
        list: Arc<Mutex<List>>,
        events: Sender<PluginEvent>,
    ) -> Self {
        let (index, n) = instance_indices();
        let slots = (0..NUM_EFFECT_UNITS)
            .map(|u| {
                std::array::from_fn(|s| {
                    let g = effect_group(u, s);
                    SlotHandles {
                        loaded_effect: get(reg, &g, "loaded_effect"),
                        enabled: get(reg, &g, "enabled"),
                        params: std::array::from_fn(|k| get(reg, &g, &format!("parameter{}", k + 1))),
                        num_parameters: get(reg, &g, "num_parameters"),
                        show_editor: get(reg, &g, "show_editor"),
                    }
                })
            })
            .collect();
        let routes = (0..NUM_EFFECT_UNITS)
            .map(|u| {
                let g = effect_unit_group(u);
                std::array::from_fn(|t| get(reg, &g, &route_name(t)))
            })
            .collect();
        Self {
            opts,
            list,
            version: 0,
            entries: Vec::new(),
            events,
            link,
            slots,
            routes,
            index,
            running: (0..n).map(|_| None).collect(),
            knobs: vec![[None; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS],
            policies: HashMap::new(),
            disabled: HashSet::new(),
            instrument: InstrumentState {
                entries: Vec::new(),
                choice: get(reg, INSTRUMENT_GROUP, "instrument"),
                on: get(reg, INSTRUMENT_GROUP, "main_mix"),
                running: None,
                running_key: String::new(),
                show_editor: get(reg, INSTRUMENT_GROUP, "show_editor"),
                editor: false,
                params: (1..=INSTRUMENT_PARAMETERS)
                    .map(|k| get(reg, INSTRUMENT_GROUP, &format!("parameter{k}")))
                    .collect(),
                num_parameters: get(reg, INSTRUMENT_GROUP, "num_parameters"),
                follow: ParamFollow::default(),
                policies: HashMap::new(),
                disabled: HashSet::new(),
                pending_preset: None,
            },
            next_keep: Instant::now() + KEEP_EVERY,
        }
    }

    /// While a plugin's window is open its sound is kept now and then, not
    /// only when the window closes, so a crash or a forced quit loses little.
    fn keep_open_sounds(&mut self) {
        if Instant::now() < self.next_keep {
            return;
        }
        self.next_keep = Instant::now() + KEEP_EVERY;
        for r in self.running.iter().flatten().filter(|r| r.editor) {
            keep_sound(&self.list, &r.key, &r.helper);
        }
        let ins = &self.instrument;
        if let (true, Some((_, helper))) = (ins.editor, &ins.running) {
            keep_sound(&self.list, &ins.running_key, helper);
        }
    }

    fn tick(&mut self) {
        // Helpers the audio thread let go of end here, off the audio thread.
        while self.link.from_engine.pop().is_ok() {}
        self.take_list();
        self.watch();
        self.follow_slots();
        self.follow_instrument();
        self.follow_instrument_params();
        self.follow_editors();
        self.keep_open_sounds();
        self.serve_sound_requests();
    }

    /// Hands over or loads the instrument's sound, as the app asked.
    fn serve_sound_requests(&mut self) {
        let requests = std::mem::take(&mut self.list.lock().unwrap().requests);
        for r in requests {
            let ins = &self.instrument;
            match r {
                SoundRequest::Save(tx) => {
                    let result = match &ins.running {
                        Some((_, helper)) if !helper.is_gone() => {
                            helper.save_state().map(|data| (ins.running_key.clone(), data))
                        }
                        _ => Err("no instrument is playing".into()),
                    };
                    let _ = tx.send(result);
                }
                SoundRequest::Pick(knob, id, tx) => {
                    let result = if ins.running.is_some() {
                        let key = ins.running_key.clone();
                        let picks = with_pick(kept_picks(&self.list, &key), knob, id);
                        self.repick(&key, picks);
                        Ok(())
                    } else {
                        Err("no instrument is playing".into())
                    };
                    let _ = tx.send(result);
                }
                SoundRequest::Picks(key, mut picks, tx) => {
                    picks.sort_unstable();
                    if kept_picks(&self.list, &key) != picks {
                        self.repick(&key, picks);
                    }
                    let _ = tx.send(Ok(()));
                }
                SoundRequest::Preset(key, reference, tx) => {
                    let result = match &ins.running {
                        Some((_, helper)) if ins.running_key == key && !helper.is_gone() => {
                            let r = helper.load_preset(&reference);
                            if r.is_ok() {
                                keep_sound(&self.list, &key, helper);
                            }
                            r
                        }
                        _ => {
                            self.instrument.pending_preset = Some((key, reference));
                            Ok(())
                        }
                    };
                    let _ = tx.send(result);
                }
                SoundRequest::Load(key, data, tx) => {
                    // A sound given now replaces a preset still to load.
                    if self.instrument.pending_preset.as_ref().is_some_and(|(k, _)| *k == key) {
                        self.instrument.pending_preset = None;
                    }
                    let ins = &self.instrument;
                    // Kept as its last sound, so it starts from it too.
                    let folder = self.list.lock().unwrap().sounds.clone();
                    if let Some(folder) = folder {
                        let file = sound_file(&folder, &key);
                        let _ = std::fs::create_dir_all(&folder);
                        let _ = std::fs::write(&file, &data);
                    }
                    let result = match &ins.running {
                        Some((_, helper)) if ins.running_key == key && !helper.is_gone() => helper.load_state(&data),
                        _ => Ok(()),
                    };
                    let _ = tx.send(result);
                }
            }
        }
    }

    /// Keeps `picks` as plugin `key`'s and, if it plays, restarts it with
    /// its sound so its knobs take them.
    fn repick(&mut self, key: &str, picks: Vec<(usize, u32)>) {
        keep_picks(&self.list, key, &picks);
        let ins = &mut self.instrument;
        if ins.running.is_some() && ins.running_key == key {
            if let Some((_, helper)) = &ins.running {
                keep_sound(&self.list, key, helper);
            }
            ins.running = None;
            let _ = self.link.to_engine.push(PluginMsg::Instrument(None));
            // `follow_instrument` starts it again on its next round.
        }
    }

    /// Opens and closes plugin windows as the `show_editor` controls ask.
    /// A slot playing on several targets shows the first one's window.
    fn follow_editors(&mut self) {
        for u in 0..NUM_EFFECT_UNITS {
            for s in 0..EFFECTS_PER_UNIT {
                let control = &self.slots[u][s].show_editor;
                let mut want = control.get_bool();
                let mut any = false;
                for t in 0..TARGETS {
                    let Some(i) = self.index[u][s][t] else {
                        continue;
                    };
                    let Some(r) = self.running[i as usize].as_mut() else {
                        continue;
                    };
                    any = true;
                    let was_open = r.editor;
                    let reason = follow_editor(&r.helper, &mut r.editor, control, want);
                    if was_open && !r.editor {
                        keep_sound(&self.list, &r.key, &r.helper);
                    }
                    if let Some(reason) = reason {
                        let name = self.entries.get(r.plugin as usize).map_or_else(String::new, |e| e.name.clone());
                        let _ = self.events.send(PluginEvent::NoWindow { name, reason });
                    }
                    // Only the first instance shows its window.
                    want = false;
                }
                if !any && control.get_bool() {
                    // No plugin playing in this slot: nothing to show.
                    control.set_bool(false);
                    let name = plugin_index(self.slots[u][s].loaded_effect.get())
                        .and_then(|p| self.entries.get(p))
                        .map(|e| e.name.clone());
                    if let Some(name) = name {
                        let _ = self.events.send(PluginEvent::NoWindow {
                            name,
                            reason: "it opens once its FX unit plays on a deck or the master".into(),
                        });
                    }
                }
            }
        }
        let ins = &mut self.instrument;
        match ins.running.as_ref() {
            Some((plugin, helper)) => {
                let want = ins.show_editor.get_bool();
                let was_open = ins.editor;
                if let Some(reason) = follow_editor(helper, &mut ins.editor, &ins.show_editor, want) {
                    let name = ins.entries[*plugin as usize].name.clone();
                    let _ = self.events.send(PluginEvent::NoWindow { name, reason });
                }
                // Its window just closed: keep what the DJ did there.
                if was_open && !ins.editor {
                    keep_sound(&self.list, &ins.running_key, helper);
                }
            }
            None => {
                ins.editor = false;
                ins.show_editor.set_bool(false);
            }
        }
    }

    fn take_list(&mut self) {
        let list = self.list.lock().unwrap();
        if list.version == self.version {
            return;
        }
        self.version = list.version;
        let old = std::mem::replace(&mut self.entries, list.entries.clone());
        let old_instruments = std::mem::replace(&mut self.instrument.entries, list.instruments.clone());
        drop(list);
        let ins = &mut self.instrument;
        ins.policies.clear();
        ins.disabled.clear();
        if let Some((_, helper)) = &ins.running {
            keep_sound(&self.list, &ins.running_key, helper);
        }
        if ins.running.take().is_some() {
            let _ = self.link.to_engine.push(PluginMsg::Instrument(None));
        }
        let chosen = ins.choice.get().round() as usize;
        if chosen > 0 {
            let moved = if old_instruments.is_empty() {
                (chosen <= ins.entries.len()).then_some(chosen - 1)
            } else {
                old_instruments.get(chosen - 1).and_then(|e| ins.entries.iter().position(|n| n.key == e.key))
            };
            ins.choice.set(moved.map_or(0.0, |n| (n + 1) as f64));
        }
        self.policies.clear();
        self.disabled.clear();
        self.knobs = vec![[None; EFFECTS_PER_UNIT]; NUM_EFFECT_UNITS];
        // Every helper may now run the wrong plugin: start again.
        for i in 0..self.running.len() {
            self.stop(i);
        }
        for unit in &self.slots {
            for slot in unit {
                let Some(p) = plugin_index(slot.loaded_effect.get()) else {
                    continue;
                };
                // The first time, slots restored from the last session already
                // point into this list.
                let moved = if self.version == 1 && old.is_empty() {
                    (p < self.entries.len()).then_some(p)
                } else {
                    old.get(p).and_then(|e| self.entries.iter().position(|n| n.key == e.key))
                };
                slot.loaded_effect.set(moved.map_or(0.0, |n| (PLUGIN_BASE + n) as f64));
            }
        }
    }

    /// Checks each helper: crashed, hung or too slow.
    fn watch(&mut self) {
        for i in 0..self.running.len() {
            let Some(r) = self.running[i].as_mut() else {
                continue;
            };
            let plugin = r.plugin;
            let policy = self.policies.entry(plugin).or_insert_with(|| Policy::new(self.opts.limits));
            let crashed = r.helper.status() != Status::Running || r.helper.is_gone();
            let missed = r.helper.missed_in_row();
            let action = if crashed {
                policy.on_crash()
            } else {
                match policy.on_missed(missed) {
                    Action::Keep => policy.on_load(r.helper.load()),
                    a => a,
                }
            };
            let (unit, slot) = (r.unit, r.slot);
            let name = self.name(plugin);
            match action {
                Action::Keep => {}
                Action::Restart => {
                    // The slot comes back off; the next pass starts a new helper.
                    self.slots[unit][slot].enabled.set_bool(false);
                    self.stop(i);
                    let _ = self.events.send(PluginEvent::Restarted { name });
                }
                Action::Disable => {
                    let reason = if crashed {
                        "it crashed again"
                    } else if missed > 0 {
                        "it stopped responding again"
                    } else {
                        "it used too much of the audio time"
                    };
                    self.disable(plugin);
                    let _ = self.events.send(PluginEvent::Disabled { name, reason: reason.into() });
                }
            }
        }
    }

    /// Starts helpers for slots that want one and stops the rest.
    fn follow_slots(&mut self) {
        for u in 0..NUM_EFFECT_UNITS {
            for s in 0..EFFECTS_PER_UNIT {
                let want = plugin_index(self.slots[u][s].loaded_effect.get())
                    .filter(|&p| p < self.entries.len())
                    .map(|p| p as u16)
                    .filter(|p| !self.disabled.contains(p));
                if want != self.knobs[u][s] {
                    self.knobs[u][s] = None;
                }
                for t in 0..TARGETS {
                    let Some(i) = self.index[u][s][t] else {
                        continue;
                    };
                    let i = i as usize;
                    let want = want.filter(|_| self.routes[u][t].get_bool());
                    let have = self.running[i].as_ref().map(|r| r.plugin);
                    if have == want {
                        continue;
                    }
                    if have.is_some() {
                        self.stop(i);
                    }
                    if let Some(p) = want {
                        self.start(i, u, s, p, t);
                    }
                }
            }
        }
    }

    fn start(&mut self, i: usize, unit: usize, slot: usize, plugin: u16, target: usize) {
        let entry = &self.entries[plugin as usize];
        let opts = Options {
            picks: Vec::new(),
            helper: self.opts.helper.clone(),
            plugin: entry.plugin.clone(),
            sample_rate: self.opts.sample_rate,
            max_frames: BLOCK,
            title: Some(format!("{} · {} · FX {}", entry.name, target_name(target), slot + 1)),
            state: kept_sound(&self.list, &entry.key),
        };
        match spawn(&opts) {
            Ok((conn, helper)) => {
                let info = helper.info();
                let slot_h = &self.slots[unit][slot];
                let shown = info.params.len().min(MAX_PARAMETERS);
                let fresh = self.knobs[unit][slot] != Some(plugin);
                let mut ranges = Vec::with_capacity(shown);
                let mut sent = vec![0.5; shown];
                for (k, p) in info.params.iter().take(shown).enumerate() {
                    let span = p.max - p.min;
                    let v = if span > 0.0 { ((p.value - p.min) / span).clamp(0.0, 1.0) } else { 0.0 };
                    if fresh {
                        slot_h.params[k].set(v);
                    }
                    sent[k] = slot_h.params[k].get() as f32;
                    conn.set_param(k, (p.min + sent[k] as f64 * span) as f32);
                    ranges.push((p.min as f32, p.max as f32));
                }
                slot_h.num_parameters.set(shown as f64);
                let attached = Box::new(Attached { conn, plugin, ranges, sent });
                if self.link.to_engine.push(PluginMsg::Attach(i as u16, attached)).is_ok() {
                    self.knobs[unit][slot] = Some(plugin);
                    self.running[i] = Some(Running {
                        plugin,
                        key: self.entries[plugin as usize].key.clone(),
                        helper,
                        unit,
                        slot,
                        editor: false,
                    });
                }
            }
            Err(e) => {
                let name = entry.name.clone();
                self.disable(plugin);
                let _ = self.events.send(PluginEvent::Failed { name, error: e.to_string() });
            }
        }
    }

    /// Detaches instance `i`'s helper and ends it.
    fn stop(&mut self, i: usize) {
        if let Some(r) = &self.running[i] {
            keep_sound(&self.list, &r.key, &r.helper);
        }
        if self.running[i].take().is_some() {
            // The engine hands the connection back; the helper itself ends
            // as it drops here, and its connection then plays dry until then.
            let _ = self.link.to_engine.push(PluginMsg::Detach(i as u16));
        }
    }

    /// Off for the session: stops its helpers and empties its slots.
    fn disable(&mut self, plugin: u16) {
        self.disabled.insert(plugin);
        for i in 0..self.running.len() {
            if self.running[i].as_ref().is_some_and(|r| r.plugin == plugin) {
                self.stop(i);
            }
        }
        let loaded = (PLUGIN_BASE + plugin as usize) as f64;
        for unit in &self.slots {
            for slot in unit {
                if slot.loaded_effect.get().round() == loaded {
                    slot.enabled.set_bool(false);
                    slot.loaded_effect.set(0.0);
                }
            }
        }
    }

    fn name(&self, plugin: u16) -> String {
        self.entries.get(plugin as usize).map_or_else(String::new, |e| e.name.clone())
    }

    /// Keeps the instrument's knobs on the plugin's own values, and empties
    /// them once it stops.
    fn follow_instrument_params(&mut self) {
        let ins = &mut self.instrument;
        match &ins.running {
            Some((_, helper)) => {
                let (seq, values) = helper.param_values();
                let n = ins.follow.seen.len();
                ins.follow.step(&ins.params[..n], seq, &values);
            }
            None if ins.num_parameters.get() != 0.0 => {
                ins.num_parameters.set(0.0);
                ins.follow = ParamFollow::default();
                let mut list = self.list.lock().unwrap();
                list.instrument_params.clear();
                list.instrument_all.clear();
                list.instrument_picks.clear();
                list.instrument_ids.clear();
            }
            None => {}
        }
    }

    /// Starts, stops and watches the Instrument input's helper.
    fn follow_instrument(&mut self) {
        let ins = &mut self.instrument;
        if let Some((plugin, helper)) = ins.running.as_mut() {
            let plugin = *plugin;
            let policy = ins.policies.entry(plugin).or_insert_with(|| Policy::new(self.opts.limits));
            let crashed = helper.status() != Status::Running || helper.is_gone();
            let missed = helper.missed_in_row();
            let action = if crashed {
                policy.on_crash()
            } else {
                match policy.on_missed(missed) {
                    Action::Keep => policy.on_load(helper.load()),
                    a => a,
                }
            };
            let name = ins.entries[plugin as usize].name.clone();
            match action {
                Action::Keep => {}
                Action::Restart => {
                    ins.on.set_bool(false);
                    ins.running = None;
                    let _ = self.link.to_engine.push(PluginMsg::Instrument(None));
                    let _ = self.events.send(PluginEvent::Restarted { name });
                }
                Action::Disable => {
                    ins.disabled.insert(plugin);
                    ins.on.set_bool(false);
                    ins.choice.set(0.0);
                    ins.running = None;
                    let _ = self.link.to_engine.push(PluginMsg::Instrument(None));
                    let reason = if crashed {
                        "it crashed again"
                    } else if missed > 0 {
                        "it stopped responding again"
                    } else {
                        "it used too much of the audio time"
                    };
                    let _ = self.events.send(PluginEvent::Disabled { name, reason: reason.into() });
                }
            }
        }
        let chosen = ins.choice.get().round() as usize;
        let want = (chosen > 0 && chosen <= ins.entries.len())
            .then(|| (chosen - 1) as u16)
            .filter(|p| !ins.disabled.contains(p));
        let have = ins.running.as_ref().map(|(p, _)| *p);
        if want == have {
            return;
        }
        if have.is_some() {
            if let Some((_, helper)) = &ins.running {
                keep_sound(&self.list, &ins.running_key, helper);
            }
            ins.running = None;
            let _ = self.link.to_engine.push(PluginMsg::Instrument(None));
        }
        let Some(plugin) = want else {
            return;
        };
        let entry = &ins.entries[plugin as usize];
        let picks = kept_picks(&self.list, &entry.key);
        let opts = Options {
            picks: picks.clone(),
            helper: self.opts.helper.clone(),
            plugin: entry.plugin.clone(),
            sample_rate: self.opts.sample_rate,
            max_frames: BLOCK,
            title: Some(format!("{} · Instrument", entry.name)),
            state: kept_sound(&self.list, &entry.key),
        };
        match spawn(&opts) {
            Ok((conn, helper)) => {
                // Its knobs start where the plugin is (its kept sound's
                // values), so nothing changes until the DJ turns one.
                let params = &helper.info().params;
                let shown = params.len().min(INSTRUMENT_PARAMETERS);
                let mut ranges = Vec::with_capacity(shown);
                let mut sent = Vec::with_capacity(shown);
                for (k, p) in params.iter().take(shown).enumerate() {
                    let v = normalized(p.value, p.min, p.max);
                    ins.params[k].set(v);
                    ranges.push((p.min as f32, p.max as f32));
                    sent.push(v as f32);
                }
                ins.num_parameters.set(shown as f64);
                ins.follow =
                    ParamFollow::new(&ins.params[..shown], params.iter().take(shown).map(|p| (p.min, p.max)).collect());
                {
                    let mut list = self.list.lock().unwrap();
                    list.instrument_params = params.iter().take(shown).map(|p| p.name.clone()).collect();
                    list.instrument_all = helper.info().all.clone();
                    list.instrument_ids = params.iter().take(shown).map(|p| p.id).collect();
                    list.instrument_picks = picks;
                }
                let attached = Box::new(Attached { conn, plugin, ranges, sent });
                if self.link.to_engine.push(PluginMsg::Instrument(Some(attached))).is_ok() {
                    ins.running_key = entry.key.clone();
                    ins.editor = false;
                    if let Some((_, reference)) = ins.pending_preset.take_if(|(k, _)| *k == entry.key) {
                        match helper.load_preset(&reference) {
                            Ok(()) => keep_sound(&self.list, &entry.key, &helper),
                            Err(e) => {
                                eprintln!("plugin {}: its preset didn't load: {e}", entry.key)
                            }
                        }
                    }
                    ins.running = Some((plugin, helper));
                }
            }
            Err(e) => {
                let name = entry.name.clone();
                ins.disabled.insert(plugin);
                ins.choice.set(0.0);
                let _ = self.events.send(PluginEvent::Failed { name, error: e.to_string() });
            }
        }
    }

    fn shut_down(&mut self) {
        for i in 0..self.running.len() {
            self.stop(i);
        }
        if let Some((_, helper)) = &self.instrument.running {
            keep_sound(&self.list, &self.instrument.running_key, helper);
        }
        if self.instrument.running.take().is_some() {
            let _ = self.link.to_engine.push(PluginMsg::Instrument(None));
        }
        while self.link.from_engine.pop().is_ok() {}
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::effects::{input_target, EffectRack};
    use crate::inputs::Input;

    const SR: u32 = 48_000;
    const FRAMES: usize = 480;

    /// Builds the helper once, next to this test binary.
    fn helper() -> PathBuf {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| {
            let exe = std::env::current_exe().unwrap();
            // target/<profile>/deps/<test>
            let profile = exe.parent().unwrap().parent().unwrap();
            let target = profile.parent().unwrap();
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let ok = std::process::Command::new(cargo)
                .args(["build", "-q", "-p", "djsugar-plugin-host", "--bin"])
                .arg("djsugar-plugin-host")
                .arg("--target-dir")
                .arg(target)
                .status()
                .expect("cargo runs")
                .success();
            assert!(ok, "the helper builds");
            let name = format!("djsugar-plugin-host{}", std::env::consts::EXE_SUFFIX);
            target.join("debug").join(name)
        })
        .clone()
    }

    struct Rig {
        reg: Arc<ControlRegistry>,
        rack: EffectRack,
        host: PluginHost,
        instrument: Input,
        notes: crate::effects::NoteInput,
        now: f64,
    }

    impl Rig {
        /// Deck 1's own unit, its first slot loaded with `effect`, fully wet.
        fn new(effect: &str) -> Self {
            let reg = ControlRegistry::new();
            let mut rack = EffectRack::new(&reg, SR as f64);
            let instrument = Input::new(&reg, INSTRUMENT, SR as f64);
            let notes = rack.take_note_input().unwrap();
            let link = rack.take_plugin_link().unwrap();
            let mut opts = HostOptions::new(helper(), SR);
            opts.poll = Duration::from_millis(2);
            let host = PluginHost::start(&reg, link, opts);
            host.set_plugins(vec![PluginEntry {
                key: effect.into(),
                name: format!("Test {effect}"),
                plugin: Plugin::Test(effect.into()),
            }]);
            let rig = Self { reg, rack, host, instrument, notes, now: 0.0 };
            rig.unit("mix").set(1.0);
            rig.slot("loaded_effect").set(PLUGIN_BASE as f64);
            rig.slot("enabled").set_bool(true);
            rig
        }

        fn unit(&self, item: &str) -> ControlHandle {
            self.reg.get("[EffectRack1_EffectUnit1]", item).unwrap()
        }

        fn slot(&self, item: &str) -> ControlHandle {
            self.reg.get("[EffectRack1_EffectUnit1_Effect1]", item).unwrap()
        }

        /// One buffer of deck 1: what came out for a steady 0.5 in.
        fn buffer(&mut self) -> f32 {
            self.rack.prepare([0.5; TARGETS], self.now);
            let mut buf = vec![[0.5f32, 0.5]; FRAMES];
            self.rack.process_block(0, &mut buf);
            self.rack.finish(FRAMES);
            self.now += FRAMES as f64 / SR as f64;
            buf[FRAMES - 1][0]
        }

        /// Plays buffers until `done` says so, for up to 10 s.
        fn until(&mut self, mut done: impl FnMut(&mut Self, f32) -> bool) {
            let end = Instant::now() + Duration::from_secs(10);
            loop {
                let out = self.buffer();
                if done(self, out) {
                    return;
                }
                assert!(Instant::now() < end, "timed out");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    #[test]
    fn a_plugin_plays_in_an_effect_slot_and_follows_its_knob() {
        let mut rig = Rig::new("test-gain");
        // Once the helper is up the knob shows the plugin's own value (gain 1
        // of 0..2) and the deck sounds as it did.
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        assert_eq!(rig.slot("parameter1").get(), 0.5);
        assert_eq!(rig.slot("loaded").get(), 1.0);
        for _ in 0..5 {
            assert!((rig.buffer() - 0.5).abs() < 1e-6);
        }
        // A quarter of the way: gain 0.5.
        rig.slot("parameter1").set(0.25);
        rig.until(|_, out| (out - 0.25).abs() < 1e-6);
        // Switched off, the deck is dry again.
        rig.slot("enabled").set_bool(false);
        rig.until(|_, out| (out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_crashing_plugin_restarts_once_then_is_switched_off() {
        let mut rig = Rig::new("test-crash");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        // It crashes after 10 blocks; the deck keeps playing, dry.
        rig.slot("parameter1").set(0.01);
        let mut events = Vec::new();
        rig.until(|r, out| {
            assert!((out - 0.5).abs() < 1e-6, "the deck plays on: {out}");
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert_eq!(events, vec![PluginEvent::Restarted { name: "Test test-crash".into() }]);
        assert!(!rig.slot("enabled").get_bool(), "back, but off");
        // Turned on again it keeps the DJ's setting (the plugin's own is
        // "never") and crashes again: off for the session.
        rig.slot("enabled").set_bool(true);
        events.clear();
        rig.until(|r, out| {
            assert!((out - 0.5).abs() < 1e-6, "the deck plays on: {out}");
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert!(matches!(&events[0], PluginEvent::Disabled { name, .. } if name == "Test test-crash"), "{events:?}");
        assert_eq!(rig.slot("loaded_effect").get(), 0.0, "the slot is emptied");
    }

    #[test]
    fn a_hung_plugin_plays_dry_and_is_restarted() {
        let mut rig = Rig::new("test-hang");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        rig.slot("parameter1").set(1.0);
        let mut events = Vec::new();
        let start = Instant::now();
        rig.until(|r, out| {
            assert!((out - 0.5).abs() < 1e-6, "dry while it hangs: {out}");
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert!(start.elapsed() < Duration::from_secs(5));
        assert_eq!(events, vec![PluginEvent::Restarted { name: "Test test-hang".into() }]);
    }

    #[test]
    fn a_plugin_that_fails_to_start_empties_its_slot() {
        let mut rig = Rig::new("no-such-effect");
        let mut events = Vec::new();
        rig.until(|r, _| {
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert!(
            matches!(&events[0], PluginEvent::Failed { error, .. } if error.contains("unknown effect")),
            "{events:?}"
        );
        assert_eq!(rig.slot("loaded_effect").get(), 0.0);
    }

    #[test]
    fn a_slot_keeps_its_plugin_when_the_list_changes() {
        let mut rig = Rig::new("test-gain");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        let entry = |key: &str| PluginEntry { key: key.into(), name: key.into(), plugin: Plugin::Test(key.into()) };
        rig.host.set_plugins(vec![entry("test-slow"), entry("test-gain")]);
        let second = (PLUGIN_BASE + 1) as f64;
        rig.until(|r, _| r.slot("loaded_effect").get() == second);
    }

    #[test]
    fn a_new_plugin_list_empties_slots_whose_plugin_left() {
        let mut rig = Rig::new("test-gain");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        rig.host.set_plugins(Vec::new());
        assert_eq!(rig.reg.get("[Master]", "num_plugins").unwrap().get(), 0.0);
        rig.until(|r, _| r.slot("loaded_effect").get() == 0.0);
        rig.until(|_, out| (out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_plugin_without_a_window_turns_its_edit_button_off() {
        let mut rig = Rig::new("test-slow");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        rig.slot("show_editor").set_bool(true);
        let mut events = Vec::new();
        rig.until(|r, _| {
            events.extend(r.host.events());
            !events.is_empty()
        });
        assert!(matches!(&events[0], PluginEvent::NoWindow { name, .. } if name == "Test test-slow"), "{events:?}");
        assert!(!rig.slot("show_editor").get_bool());
        // The plugin plays on.
        rig.until(|_, out| (out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_plugin_window_opens_and_closes_with_its_edit_button() {
        let mut rig = Rig::new("test-gain");
        rig.until(|r, _| r.slot("num_parameters").get() == 1.0);
        rig.slot("show_editor").set_bool(true);
        let display =
            cfg!(any(target_os = "macos", windows)) || std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
        if !display {
            // Nowhere to show it: the button goes off again.
            rig.until(|r, _| !r.slot("show_editor").get_bool());
            return;
        }
        // The window stays up while the button is on and the plugin plays.
        for _ in 0..50 {
            rig.buffer();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(rig.slot("show_editor").get_bool());
        rig.slot("show_editor").set_bool(false);
        rig.until(|_, out| (out - 0.5).abs() < 1e-6);
        assert!(rig.host.events().is_empty());
    }

    impl Rig {
        /// One buffer of the Instrument input, before its fader: its last
        /// frame's level.
        fn instrument_buffer(&mut self) -> f32 {
            self.instrument_buffer_on(false)
        }

        /// [`Rig::instrument_buffer`], with the input on a deck or not.
        fn instrument_buffer_on(&mut self, on_deck: bool) -> f32 {
            self.rack.prepare([0.5; TARGETS], self.now);
            let target = input_target(INSTRUMENT);
            self.instrument.start(FRAMES, &mut self.rack, target, on_deck);
            self.instrument.render(
                FRAMES,
                &mut self.rack,
                target,
                0.01,
                if on_deck { crate::inputs::Routing::Deck } else { crate::inputs::Routing::Main },
            );
            self.rack.finish(FRAMES);
            self.now += FRAMES as f64 / SR as f64;
            self.instrument.buf[FRAMES - 1][0]
        }
    }

    #[test]
    fn the_instruments_knobs_turn_it_and_follow_its_own_changes() {
        let mut rig = Rig::new("test-gain");
        rig.host.set_instruments(vec![PluginEntry {
            key: "test-synth".into(),
            name: "Test Synth".into(),
            plugin: Plugin::Test("test-synth".into()),
        }]);
        let group = input_group(INSTRUMENT);
        let count = rig.reg.get(group, "num_parameters").unwrap();
        let volume = rig.reg.get(group, "parameter1").unwrap();
        rig.reg.get(group, "instrument").unwrap().set(1.0);
        let end = Instant::now() + Duration::from_secs(10);
        while count.get() != 1.0 || !rig.reg.get(group, "input_configured").unwrap().get_bool() {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument starts");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(rig.host.instrument_parameters(), ["Volume"]);
        assert_eq!(volume.get(), 1.0, "the knob starts at the plugin's value");
        assert!(rig.notes.send([0x90, 127, 100]));
        assert!((rig.instrument_buffer() - 1.0).abs() < 1e-3);
        volume.set(0.5);
        assert!((rig.instrument_buffer() - 0.5).abs() < 1e-3);
        // A sound that sets the volume (the test synth takes 4 bytes as
        // its value) moves the knob to it, once the DJ's hands are off.
        rig.host.load_instrument_sound("test-synth", 0.25f32.to_le_bytes().to_vec()).unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        while (volume.get() - 0.25).abs() > 1e-6 {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the knob follows the plugin");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!((rig.instrument_buffer() - 0.25).abs() < 1e-3);
        // Taken off, it has no knobs.
        rig.reg.get(group, "instrument").unwrap().set(0.0);
        let end = Instant::now() + Duration::from_secs(5);
        while count.get() != 0.0 {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the knobs go");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rig.host.instrument_parameters().is_empty());
    }

    #[test]
    fn the_instrument_input_plays_the_notes_it_is_sent() {
        let mut rig = Rig::new("test-gain");
        rig.host.set_instruments(vec![PluginEntry {
            key: "test-synth".into(),
            name: "Test Synth".into(),
            plugin: Plugin::Test("test-synth".into()),
        }]);
        let group = input_group(INSTRUMENT);
        let configured = rig.reg.get(group, "input_configured").unwrap();
        rig.reg.get(group, "instrument").unwrap().set(1.0);
        // Ready once the helper is up.
        let end = Instant::now() + Duration::from_secs(10);
        while !configured.get_bool() {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument starts");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(rig.instrument_buffer().abs() < 1e-3, "silent until a note");
        let view = rig.rack.instrument_view();
        assert!(rig.notes.send([0x90, 127, 100]));
        assert!((rig.instrument_buffer() - 1.0).abs() < 1e-3);
        assert!((rig.instrument_buffer() - 1.0).abs() < 1e-3, "held");
        // Its deck sees the note held and the sound in the scope.
        assert_eq!(view.held(), 1 << 127);
        assert!((view.scope().last().unwrap().1 - 1.0).abs() < 1e-3);
        assert!(rig.notes.send([0x80, 127, 0]));
        assert!(rig.instrument_buffer().abs() < 1e-3);
        assert_eq!(view.held(), 0);
        // In a picked scale, C major, C# plays C, and the view shows the
        // scale and the note that sounds.
        rig.reg.get(group, "scale_mode").unwrap().set(2.0);
        assert!(rig.notes.send([0x90, 61, 100]));
        assert!((rig.instrument_buffer() - 60.0 / 127.0).abs() < 1e-3);
        assert_eq!(view.held(), 1 << 60);
        let mask = rig.reg.get(group, "scale_mask").unwrap().get();
        assert_eq!(mask as u16, crate::scale::mask(0, 0));
        assert!(rig.notes.send([0x80, 61, 0]));
        assert!(rig.instrument_buffer().abs() < 1e-3);
        // In chord mode Triad, D plays D minor (D F A) in C major.
        rig.reg.get(group, "chord_mode").unwrap().set(1.0);
        assert!(rig.notes.send([0x90, 62, 100]));
        rig.instrument_buffer();
        assert_eq!(view.held(), 1 << 62 | 1 << 65 | 1 << 69);
        assert!(rig.notes.send([0x80, 62, 0]));
        rig.instrument_buffer();
        assert_eq!(view.held(), 0);
        rig.reg.get(group, "chord_mode").unwrap().set(0.0);
        rig.reg.get(group, "scale_mode").unwrap().set(0.0);
        // With the arpeggiator on, a key held plays on its steps, and stops
        // when let go.
        rig.reg.get(group, "arp").unwrap().set(1.0);
        rig.instrument_buffer();
        assert!(rig.notes.send([0x90, 64, 100]));
        let mut played = false;
        for _ in 0..200 {
            if (rig.instrument_buffer() - 64.0 / 127.0).abs() < 1e-3 {
                played = true;
                break;
            }
        }
        assert!(played, "the arpeggiator plays the key held");
        assert!(rig.notes.send([0x80, 64, 0]));
        rig.instrument_buffer();
        assert!(rig.instrument_buffer().abs() < 1e-3);
        rig.reg.get(group, "arp").unwrap().set(0.0);
        rig.instrument_buffer();
        // The pads play from middle C; a pad let go after the octave moved
        // still stops its note.
        let pad = rig.reg.get(group, "pad_1").unwrap();
        pad.set(1.0);
        assert!((rig.instrument_buffer() - 60.0 / 127.0).abs() < 1e-3);
        rig.reg.get(group, "octave").unwrap().set(1.0);
        pad.set(0.0);
        assert!(rig.instrument_buffer().abs() < 1e-3);
        rig.reg.get(group, "pad_2").unwrap().set(1.0);
        assert!((rig.instrument_buffer() - 74.0 / 127.0).abs() < 1e-3);
        rig.reg.get(group, "pad_2").unwrap().set(0.0);
        assert!(rig.instrument_buffer().abs() < 1e-3);
        // With keyboard_pads on, a deck's KEYBOARD pads play it too.
        rig.reg.get(group, "octave").unwrap().set(0.0);
        rig.reg.get(group, "keyboard_pads").unwrap().set(1.0);
        let deck_pad = rig.reg.get("[Channel1]", "keyboard_3_activate").unwrap();
        deck_pad.set(1.0);
        assert!((rig.instrument_buffer() - 64.0 / 127.0).abs() < 1e-3);
        deck_pad.set(0.0);
        assert!(rig.instrument_buffer().abs() < 1e-3);
        // Taken off the input, it is silent and no longer ready.
        rig.reg.get(group, "instrument").unwrap().set(0.0);
        while configured.get_bool() {
            rig.instrument_buffer();
            assert!(Instant::now() < end + Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(rig.notes.send([0x90, 127, 100]));
        assert!(rig.instrument_buffer().abs() < 1e-3);
        assert_eq!(view.held(), 0, "nothing shows held without it");
        assert!(view.scope().iter().all(|&(lo, hi)| lo == 0.0 && hi == 0.0));
    }
    #[test]
    fn a_silent_instrument_off_its_deck_sleeps_until_a_note() {
        let mut rig = Rig::new("test-gain");
        rig.host.set_instruments(vec![PluginEntry {
            key: "test-synth".into(),
            name: "Test Synth".into(),
            plugin: Plugin::Test("test-synth".into()),
        }]);
        let group = input_group(INSTRUMENT);
        let configured = rig.reg.get(group, "input_configured").unwrap();
        rig.reg.get(group, "instrument").unwrap().set(1.0);
        let end = Instant::now() + Duration::from_secs(10);
        while !configured.get_bool() {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument starts");
            std::thread::sleep(Duration::from_millis(1));
        }
        // A second of silence, a buffer at a time.
        let second = SR as usize / FRAMES + 1;
        // On a deck it keeps playing (getting its blocks) however quiet.
        for _ in 0..2 * second {
            rig.instrument_buffer_on(true);
        }
        assert!(!rig.rack.instrument_asleep());
        // Off it, it sleeps after a second of silence.
        for _ in 0..second {
            rig.instrument_buffer();
        }
        assert!(rig.rack.instrument_asleep());
        // A note wakes it at once: it plays in the very next buffer.
        assert!(rig.notes.send([0x90, 127, 100]));
        assert!((rig.instrument_buffer() - 1.0).abs() < 1e-3);
        assert!(!rig.rack.instrument_asleep());
        // Held, it never sleeps, even off its deck.
        for _ in 0..2 * second {
            rig.instrument_buffer();
        }
        assert!(!rig.rack.instrument_asleep());
        assert!(rig.notes.send([0x80, 127, 0]));
        assert!(rig.instrument_buffer().abs() < 1e-3);
        // Put on a deck, it wakes too.
        for _ in 0..2 * second {
            rig.instrument_buffer();
        }
        assert!(rig.rack.instrument_asleep());
        rig.instrument_buffer_on(true);
        assert!(!rig.rack.instrument_asleep());
    }

    #[test]
    fn an_instruments_sound_is_kept_for_next_time() {
        let mut rig = Rig::new("test-gain");
        let folder = std::env::temp_dir().join(format!("djsugar-sounds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let file = sound_file(&folder, "test-synth");
        std::fs::write(&file, b"kept").unwrap();
        rig.host.set_sound_folder(folder.clone());
        rig.host.set_instruments(vec![PluginEntry {
            key: "test-synth".into(),
            name: "Test Synth".into(),
            plugin: Plugin::Test("test-synth".into()),
        }]);
        let group = input_group(INSTRUMENT);
        let configured = rig.reg.get(group, "input_configured").unwrap();
        rig.reg.get(group, "instrument").unwrap().set(1.0);
        let end = Instant::now() + Duration::from_secs(10);
        while !configured.get_bool() {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument starts");
            std::thread::sleep(Duration::from_millis(1));
        }
        // It started with the kept sound; taken off, it keeps it again.
        std::fs::remove_file(&file).unwrap();
        rig.reg.get(group, "instrument").unwrap().set(0.0);
        while !file.exists() {
            rig.instrument_buffer();
            assert!(Instant::now() < end + Duration::from_secs(10), "the sound is kept");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(std::fs::read(&file).unwrap(), b"kept");
        let _ = std::fs::remove_dir_all(&folder);
    }
    #[test]
    fn a_parameter_sits_on_one_knob_at_a_time() {
        let picks = with_pick(Vec::new(), 2, Some(40));
        assert_eq!(picks, [(2, 40)]);
        let picks = with_pick(picks, 0, Some(7));
        assert_eq!(picks, [(0, 7), (2, 40)]);
        // Moved to another knob, it leaves the first.
        let picks = with_pick(picks, 5, Some(40));
        assert_eq!(picks, [(0, 7), (5, 40)]);
        // Back to the plugin's own choice.
        assert_eq!(with_pick(picks, 0, None), [(5, 40)]);
    }

    #[test]
    fn a_knob_pick_is_kept_and_the_instrument_restarts_with_it() {
        let mut rig = Rig::new("test-gain");
        let folder = std::env::temp_dir().join(format!("djsugar-picks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        rig.host.set_sound_folder(folder.clone());
        rig.host.set_instruments(vec![PluginEntry {
            key: "test-synth".into(),
            name: "Test Synth".into(),
            plugin: Plugin::Test("test-synth".into()),
        }]);
        let group = input_group(INSTRUMENT);
        let configured = rig.reg.get(group, "input_configured").unwrap();
        assert!(rig.host.pick_instrument_knob(0, Some(0)).is_err(), "nothing playing");
        rig.reg.get(group, "instrument").unwrap().set(1.0);
        let end = Instant::now() + Duration::from_secs(10);
        while !configured.get_bool() {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument starts");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(rig.host.instrument_all_parameters(), [(0, "Volume".into())]);
        assert!(rig.host.instrument_picks().is_empty());
        rig.host.pick_instrument_knob(0, Some(0)).unwrap();
        assert_eq!(std::fs::read_to_string(picks_file(&folder, "test-synth")).unwrap(), "0 0\n");
        // It comes back with the pick.
        while rig.host.instrument_picks() != [(0, 0)] {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument restarts");
            std::thread::sleep(Duration::from_millis(1));
        }
        // A preset without picks gives the knobs back.
        rig.host.set_instrument_picks("test-synth", Vec::new()).unwrap();
        assert!(!picks_file(&folder, "test-synth").exists());
        while !rig.host.instrument_picks().is_empty() {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument restarts");
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_preset_sound_is_read_and_given_to_the_instrument() {
        let mut rig = Rig::new("test-gain");
        assert!(rig.host.instrument_sound().is_err(), "nothing playing");
        let folder = std::env::temp_dir().join(format!("djsugar-presets-{}", std::process::id()));
        rig.host.set_sound_folder(folder.clone());
        rig.host.set_instruments(vec![PluginEntry {
            key: "test-synth".into(),
            name: "Test Synth".into(),
            plugin: Plugin::Test("test-synth".into()),
        }]);
        // Given before it plays, it starts with it.
        rig.host.load_instrument_sound("test-synth", b"bass".to_vec()).unwrap();
        let group = input_group(INSTRUMENT);
        let configured = rig.reg.get(group, "input_configured").unwrap();
        rig.reg.get(group, "instrument").unwrap().set(1.0);
        let end = Instant::now() + Duration::from_secs(10);
        while !configured.get_bool() {
            rig.instrument_buffer();
            assert!(Instant::now() < end, "the instrument starts");
            std::thread::sleep(Duration::from_millis(1));
        }
        let (key, data) = rig.host.instrument_sound().unwrap();
        assert_eq!((key.as_str(), data.as_slice()), ("test-synth", &b"bass"[..]));
        // Given while it plays, it takes it now.
        rig.host.load_instrument_sound("test-synth", b"pad".to_vec()).unwrap();
        assert_eq!(rig.host.instrument_sound().unwrap().1, b"pad");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
