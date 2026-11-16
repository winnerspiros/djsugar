//! Finds MIDI controllers, picks the Mixxx mapping for each by name, and runs
//! each one on its own thread. Plugging a controller in or out is picked up
//! within about a second, and [`Controllers::refresh`] reconnects them all.
//! USB HID controllers (see [`crate::hid`]) are found the same way, by the
//! vendor and product IDs their mappings name, and listed with the MIDI ones.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use djsugar_control::ControlRegistry;
use midir::{Ignore, MidiInput, MidiOutput};

use crate::controller::Controller;
use crate::format::DjSugarMapping;
use crate::hid;
use crate::mapping::Mapping;

const CLIENT: &str = "DJ Sugar";
const RESCAN: Duration = Duration::from_secs(1);
/// LEDs and connection callbacks are refreshed at least this often.
const POLL: Duration = Duration::from_millis(10);

/// A controller that is connected and running its mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectedController {
    pub port: String,
    pub mapping: String,
    /// The Mixxx mapping's file name: the one it runs, or the one the own
    /// mapping it runs is based on.
    pub file: String,
}

/// Which mapping a device uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MappingChoice {
    /// The mapping whose name matches the device's (see [`match_mapping`]).
    Auto,
    /// Not used as a controller.
    Off,
    /// This mapping file, by file name in the mappings folder.
    File(String),
    /// One of the DJ's own mappings, by its file: one made with Quick map,
    /// standing alone (or put over the Mixxx mapping it names).
    Own(PathBuf),
    /// No mapping: it plays instruments, even if a mapping matches its name
    /// or it is a DAW port.
    Instrument,
}

/// A MIDI device that is plugged in, whether or not it has a mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    pub name: String,
    /// The file name of the mapping it would use, or uses.
    pub mapping_file: Option<String>,
    /// That mapping's name, while it runs.
    pub running: Option<String>,
    /// A USB HID controller rather than a MIDI port: it runs an HID mapping,
    /// which can't be edited or mapped by hand.
    pub hid: bool,
    /// Why it isn't running, when DJ Sugar knows: an HID device the system
    /// wouldn't open, say.
    pub problem: Option<String>,
    /// It plays instruments: it has no mapping (see [`Controllers::keyboards`]).
    pub keys: bool,
}

/// Running controller support. Dropping it stops every controller, which
/// runs the mappings' `shutdown` functions so LEDs go dark.
pub struct Controllers {
    connected: Arc<Mutex<Vec<ConnectedController>>>,
    shared: Arc<Shared>,
    mappings: Vec<(String, PathBuf)>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Where notes from MIDI keyboards go: the instrument plugin. Called on the
/// device's MIDI thread with each channel message (status, two data bytes).
pub type NoteSink = Arc<dyn Fn([u8; 3]) + Send + Sync>;

/// What the device thread and the app share besides `connected`.
#[derive(Default)]
struct Shared {
    /// Where devices with no mapping send their notes: MIDI keyboards play
    /// the instrument.
    notes: Mutex<Option<NoteSink>>,
    /// The devices playing the instrument now.
    keyboards: Mutex<Vec<String>>,
    /// Every MIDI input seen at the last scan, with the mapping file picked.
    ports: Mutex<Vec<(String, Option<PathBuf>)>>,
    /// Choices by [`device_key`]; devices not listed are Auto.
    choices: Mutex<HashMap<String, MappingChoice>>,
    /// The DJ's own mapping run in place of a Mixxx mapping, by the Mixxx
    /// mapping's file name: the own mapping's file.
    versions: Mutex<HashMap<String, PathBuf>>,
    /// Mixxx mappings (by file name) whose controllers restart at the next
    /// scan, because the own mapping they run changed.
    stale: Mutex<HashSet<String>>,
    /// Set when a choice changes, so the next scan comes at once.
    rescan: AtomicBool,
    /// Set by [`Controllers::refresh`]: every device is closed and the
    /// system's MIDI device list is read again from scratch.
    reconnect: AtomicBool,
    /// The HID controllers plugged in, by port name: the name of the
    /// mapping Automatic picks for each.
    hid_ports: Mutex<HashMap<String, String>>,
    /// Why a device couldn't be opened, by port name.
    problems: Mutex<HashMap<String, String>>,
}

/// A device's name without the parts that change between systems or
/// plug-ins (client and port numbers, "MIDI 1"): what choices are kept by.
pub fn device_key(port: &str) -> String {
    tokens(port).join(" ")
}

impl Controllers {
    /// Starts watching for devices. `choices` are the mappings picked
    /// before, by [`device_key`]; `versions` the DJ's own mappings used in
    /// place of Mixxx mappings (see [`Controllers::set_version`]).
    pub fn start(
        registry: Arc<ControlRegistry>,
        mappings_dir: PathBuf,
        choices: HashMap<String, MappingChoice>,
        versions: HashMap<String, PathBuf>,
    ) -> Self {
        let connected = Arc::new(Mutex::new(Vec::new()));
        let shared = Arc::new(Shared {
            choices: Mutex::new(choices),
            versions: Mutex::new(versions),
            ..Default::default()
        });
        let stop = Arc::new(AtomicBool::new(false));
        let mappings = list_mappings(&mappings_dir);
        let thread = {
            let (connected, shared, stop) = (connected.clone(), shared.clone(), stop.clone());
            let (dir, mappings) = (mappings_dir, mappings.clone());
            std::thread::Builder::new()
                .name("midi-devices".into())
                .spawn(move || watch_devices(registry, dir, mappings, connected, shared, stop))
                .expect("spawn MIDI device thread")
        };
        Self {
            connected,
            shared,
            mappings,
            stop,
            thread: Some(thread),
        }
    }

    pub fn connected(&self) -> Vec<ConnectedController> {
        self.connected.lock().unwrap().clone()
    }

    /// The MIDI devices plugged in now, and what each one runs.
    pub fn ports(&self) -> Vec<Port> {
        let connected = self.connected();
        let hid_ports = self.shared.hid_ports.lock().unwrap().clone();
        let problems = self.shared.problems.lock().unwrap().clone();
        let keyboards = self.keyboards();
        let ports: Vec<Port> = self
            .shared
            .ports
            .lock()
            .unwrap()
            .iter()
            .map(|(name, path)| Port {
                name: name.clone(),
                mapping_file: path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|f| f.to_string_lossy().into_owned()),
                running: connected
                    .iter()
                    .find(|c| &c.port == name)
                    .map(|c| c.mapping.clone()),
                hid: hid_ports.contains_key(name),
                problem: problems.get(name).cloned(),
                keys: keyboards.contains(name),
            })
            .collect();
        ports
    }

    /// Sends the notes of MIDI devices that have no mapping (keyboards) to
    /// `sink`, or nowhere.
    pub fn set_note_sink(&self, sink: Option<NoteSink>) {
        *self.shared.notes.lock().unwrap() = sink;
    }

    /// The MIDI devices with no mapping, whose notes play the instrument.
    pub fn keyboards(&self) -> Vec<String> {
        self.shared.keyboards.lock().unwrap().clone()
    }

    /// Every mapping there is: its name and file name.
    pub fn mappings(&self) -> Vec<(String, String)> {
        self.mappings
            .iter()
            .filter_map(|(name, path)| {
                Some((
                    name.clone(),
                    path.file_name()?.to_string_lossy().into_owned(),
                ))
            })
            .collect()
    }

    /// The name of the mapping Automatic picks for `port`.
    pub fn auto_mapping(&self, port: &str) -> Option<String> {
        if let Some(name) = self.shared.hid_ports.lock().unwrap().get(port) {
            return Some(name.clone());
        }
        let path = match_mapping(port, &self.mappings)?;
        self.mappings
            .iter()
            .find(|(_, p)| p == path)
            .map(|(n, _)| n.clone())
    }

    pub fn choice(&self, port: &str) -> MappingChoice {
        self.shared
            .choices
            .lock()
            .unwrap()
            .get(&device_key(port))
            .cloned()
            .unwrap_or(MappingChoice::Auto)
    }

    /// Changes the mapping `port` uses; a running controller restarts with it.
    pub fn set_choice(&self, port: &str, choice: MappingChoice) {
        let mut choices = self.shared.choices.lock().unwrap();
        if choice == MappingChoice::Auto {
            choices.remove(&device_key(port));
        } else {
            choices.insert(device_key(port), choice);
        }
        self.shared.rescan.store(true, Ordering::Relaxed);
    }

    /// Runs the DJ's own mapping in the file `own` (see [`crate::own`]) in
    /// place of the Mixxx mapping `based_on` (a file name), or the Mixxx
    /// mapping itself again for `None`. Its controllers restart with it.
    pub fn set_version(&self, based_on: &str, own: Option<PathBuf>) {
        let mut versions = self.shared.versions.lock().unwrap();
        match own {
            Some(p) => versions.insert(based_on.to_string(), p),
            None => versions.remove(based_on),
        };
        self.reload(based_on);
    }

    /// Restarts the controllers running the Mixxx mapping `based_on`, or an
    /// own mapping based on it: after that own mapping was changed.
    pub fn reload(&self, based_on: &str) {
        self.shared
            .stale
            .lock()
            .unwrap()
            .insert(based_on.to_string());
        self.shared.rescan.store(true, Ordering::Relaxed);
    }

    /// Looks for devices now, closing and reopening every controller: for a
    /// controller that was plugged in but is not showing, or stopped
    /// answering.
    pub fn refresh(&self) {
        self.shared.reconnect.store(true, Ordering::Relaxed);
    }

    pub fn choices(&self) -> HashMap<String, MappingChoice> {
        self.shared.choices.lock().unwrap().clone()
    }

    /// Starts or stops keeping a copy of every message the controllers
    /// send, for the Controller Editor's "press a control to find it".
    pub fn set_monitor(&self, on: bool) {
        *MONITOR.lock().unwrap() = on.then(Vec::new);
    }

    /// While on, messages from the controllers are only watched (see
    /// [`Controllers::set_monitor`]), not acted on: for Map mode, where moving
    /// a control teaches it rather than playing a deck.
    pub fn set_learning(&self, on: bool) {
        LEARNING.store(on, Ordering::Relaxed);
    }

    /// The messages received since the last call, with the port each came
    /// from, while the monitor is on.
    pub fn take_monitored(&self) -> Vec<Monitored> {
        MONITOR
            .lock()
            .unwrap()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
}

/// A message from a controller, with the port it came from.
type Monitored = (String, Vec<u8>);

/// Messages kept for [`Controllers::take_monitored`]; `None` while off.
static MONITOR: Mutex<Option<Vec<Monitored>>> = Mutex::new(None);

/// See [`Controllers::set_learning`].
static LEARNING: AtomicBool = AtomicBool::new(false);

fn monitor(port: &str, msg: &[u8]) {
    if let Some(seen) = MONITOR.lock().unwrap().as_mut() {
        // Nobody is reading: don't grow without end.
        if seen.len() < 4096 {
            seen.push((port.to_string(), msg.to_vec()));
        }
    }
}

impl Drop for Controllers {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Words of a device or mapping name, minus ones that say nothing about
/// which device it is ("MIDI", port numbers). A model number joined to its
/// letters by a hyphen stays one word ("DDJ-800" → "ddj800"), so it isn't
/// dropped as a port number.
fn tokens(name: &str) -> Vec<String> {
    let lower = name.to_lowercase().replace("&amp;", " ");
    let chars: Vec<char> = lower.chars().collect();
    let joined: String = chars
        .iter()
        .enumerate()
        .filter(|&(i, &c)| {
            !(c == '-'
                && i > 0
                && chars[i - 1].is_alphabetic()
                && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
        })
        .map(|(_, &c)| c)
        .collect();
    joined
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .filter(|t| !matches!(*t, "midi" | "port" | "in" | "out" | "usb" | "and"))
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// The mapping for a MIDI port: every word of the port's name must appear in
/// the mapping's name ("DDJ-FLX4" → "Pioneer DDJ-FLX4", "XONE:K2" →
/// "Allen&Heath Xone K1/K2/K3"); among those, the most specific name wins.
pub fn match_mapping<'a>(port: &str, mappings: &'a [(String, PathBuf)]) -> Option<&'a Path> {
    let port_tokens = tokens(port);
    if port_tokens.is_empty() {
        return None;
    }
    mappings
        .iter()
        .filter_map(|(name, path)| {
            let m = tokens(name);
            port_tokens
                .iter()
                .all(|t| m.contains(t))
                .then_some((m.len(), path))
        })
        .min_by_key(|(extra, _)| *extra)
        .map(|(_, p)| p.as_path())
}

/// Names of all MIDI mappings in `dir`.
pub fn list_mappings(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.to_string_lossy().ends_with(".midi.xml") || crate::mapping::is_djsugar_file(p)
        })
        .filter_map(|p| Mapping::read_name(&p).map(|n| (n, p)))
        .collect();
    v.sort();
    v
}

/// The HID controllers plugged in, each with a port name of its own and the
/// mapping Automatic picks: the device's name for itself, or the mapping's,
/// numbered if two are alike.
fn hid_ports(
    devices: Vec<hid::DeviceInfo>,
    mappings: &[hid::HidMapping],
) -> Vec<(String, hid::DeviceInfo, &hid::HidMapping)> {
    let mut out: Vec<(String, hid::DeviceInfo, &hid::HidMapping)> = Vec::new();
    for d in devices {
        let Some(m) = hid::match_mapping(&d, mappings) else {
            continue;
        };
        let base = if d.product.is_empty() {
            m.0.clone()
        } else {
            d.product.clone()
        };
        let mut name = base.clone();
        let mut n = 1;
        while out.iter().any(|(p, _, _)| *p == name) {
            n += 1;
            name = format!("{base} #{n}");
        }
        out.push((name, d, m));
    }
    out
}

/// The HID mapping file `port` should run, given the choices: the one
/// matched, or another HID mapping picked by file name; none when off.
fn pick_hid_mapping(
    port: &str,
    matched: &Path,
    dir: &Path,
    choices: &HashMap<String, MappingChoice>,
) -> Option<PathBuf> {
    match choices.get(&device_key(port)) {
        Some(MappingChoice::Off) => None,
        Some(MappingChoice::File(f)) if f.ends_with(".hid.xml") && dir.join(f).is_file() => {
            Some(dir.join(f))
        }
        _ => Some(matched.to_path_buf()),
    }
}

struct Device {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    mapping: PathBuf,
    own: Option<PathBuf>,
}

/// The own mapping run in place of the Mixxx mapping at `path`, if any.
fn own_for(path: &Path, versions: &HashMap<String, PathBuf>) -> Option<PathBuf> {
    let file = path.file_name()?.to_string_lossy();
    versions.get(file.as_ref()).filter(|p| p.is_file()).cloned()
}

/// Whether `file` is one of the DJ's own mappings rather than a Mixxx one.
pub fn is_own_file(file: &str) -> bool {
    file.ends_with(".json")
}

/// The mapping a controller runs: the Mixxx mapping at `path`, or the own
/// mapping `own` put over it; or the own mapping at `path`, put over the
/// Mixxx mapping it names, if any.
pub(crate) fn load_mapping(path: &Path, own: Option<&Path>) -> Result<Mapping, String> {
    if is_own_file(&path.to_string_lossy()) {
        let mine = DjSugarMapping::read(path)?;
        let dir = crate::default_mappings_dir().unwrap_or_default();
        let base = match &mine.based_on {
            Some(b) => Some(Mapping::load(&dir.join(b))?),
            None => None,
        };
        return Ok(mine.build(base.as_ref(), &dir));
    }
    let base = Mapping::load(path)?;
    let Some(own) = own else {
        return Ok(base);
    };
    let mine = DjSugarMapping::read(own)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    Ok(mine.build(Some(&base), dir))
}

/// The mapping file `port` should run, given the choices.
fn pick_mapping(
    port: &str,
    dir: &Path,
    mappings: &[(String, PathBuf)],
    choices: &HashMap<String, MappingChoice>,
) -> Option<PathBuf> {
    match choices.get(&device_key(port)) {
        Some(MappingChoice::Off | MappingChoice::Instrument) => None,
        Some(MappingChoice::File(f)) if dir.join(f).is_file() => Some(dir.join(f)),
        Some(MappingChoice::Own(p)) if p.is_file() => Some(p.clone()),
        _ => match_mapping(port, mappings).map(Path::to_path_buf),
    }
}

fn watch_devices(
    registry: Arc<ControlRegistry>,
    dir: PathBuf,
    mappings: Vec<(String, PathBuf)>,
    connected: Arc<Mutex<Vec<ConnectedController>>>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
) {
    let stand_ins = stand_in_devices();
    let mut input = MidiInput::new(CLIENT).ok();
    let hid_mappings = hid::list_mappings(&dir);
    let mut hid_vendors: Vec<u16> = hid_mappings
        .iter()
        .flat_map(|(_, _, products)| products.iter().map(|p| p.vendor_id))
        .collect();
    hid_vendors.sort_unstable();
    hid_vendors.dedup();
    let mut hid_api = if hid_mappings.is_empty() {
        None
    } else {
        match hid::Hid::new() {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("USB HID is unavailable ({e}); HID controllers are disabled");
                None
            }
        }
    };
    if input.is_none() && stand_ins.is_empty() && hid_api.is_none() {
        eprintln!("MIDI is unavailable; controllers are disabled");
        return;
    }

    let mut devices: HashMap<String, Device> = HashMap::new();
    let mut keyboards: HashMap<String, Keyboard> = HashMap::new();
    let mut last_scan: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        let mut rescan = shared.rescan.swap(false, Ordering::Relaxed);
        if shared.reconnect.swap(false, Ordering::Relaxed) {
            rescan = true;
            for (port, d) in devices.drain() {
                d.stop.store(true, Ordering::Relaxed);
                let _ = d.thread.join();
                connected.lock().unwrap().retain(|c| c.port != port);
            }
            for (_, k) in keyboards.drain() {
                k.stop.store(true, Ordering::Relaxed);
                let _ = k.thread.join();
            }
            if let Ok(fresh) = MidiInput::new(CLIENT) {
                input = Some(fresh);
            }
            shared.problems.lock().unwrap().clear();
        }
        if rescan || last_scan.is_none_or(|t| t.elapsed() >= RESCAN) {
            last_scan = Some(Instant::now());
            let mut ports: Vec<String> = input
                .iter()
                .flat_map(|input| {
                    input
                        .ports()
                        .iter()
                        .filter_map(|p| input.port_name(p).ok())
                        .collect::<Vec<_>>()
                })
                .filter(|n| !n.contains(CLIENT) && !n.contains("Midi Through"))
                .collect();
            ports.extend(stand_ins.iter().map(|s| s.name.clone()));
            let found = match hid_api.as_mut() {
                Some(h) => hid_ports(h.scan(&hid_vendors), &hid_mappings),
                None => Vec::new(),
            };
            let mut picked: Vec<(String, Option<PathBuf>)> = {
                let choices = shared.choices.lock().unwrap();
                ports
                    .iter()
                    .map(|p| (p.clone(), pick_mapping(p, &dir, &mappings, &choices)))
                    .collect()
            };
            {
                let choices = shared.choices.lock().unwrap();
                picked.extend(
                    found
                        .iter()
                        .map(|(p, _, m)| (p.clone(), pick_hid_mapping(p, &m.1, &dir, &choices))),
                );
            }
            *shared.hid_ports.lock().unwrap() = found
                .iter()
                .map(|(p, _, m)| (p.clone(), m.0.clone()))
                .collect();
            // A device unplugged may open next time it's plugged in.
            shared
                .problems
                .lock()
                .unwrap()
                .retain(|p, _| found.iter().any(|(f, _, _)| f == p));
            // HID devices found, by port name.
            let hid_found: HashMap<String, hid::DeviceInfo> =
                found.into_iter().map(|(p, d, _)| (p, d)).collect();
            let versions = shared.versions.lock().unwrap().clone();
            let stale = std::mem::take(&mut *shared.stale.lock().unwrap());
            // Unplugged, its thread ended (e.g. the mapping failed to load),
            // given another mapping, or its own mapping changed.
            let gone: Vec<String> = devices
                .iter()
                .filter(|(port, d)| {
                    let file = d.mapping.file_name().map(|f| f.to_string_lossy());
                    d.thread.is_finished()
                        || file.is_some_and(|f| stale.contains(f.as_ref()))
                        || d.own != own_for(&d.mapping, &versions)
                        || !picked
                            .iter()
                            .any(|(p, m)| p == *port && m.as_ref() == Some(&d.mapping))
                })
                .map(|(port, _)| port.clone())
                .collect();
            for port in gone {
                let d = devices.remove(&port).unwrap();
                d.stop.store(true, Ordering::Relaxed);
                let _ = d.thread.join();
                connected.lock().unwrap().retain(|c| c.port != port);
            }
            *shared.ports.lock().unwrap() = picked.clone();
            follow_keyboards(&mut keyboards, &picked, &stand_ins, &shared, &registry);
            for (port, path) in picked {
                if devices.contains_key(&port)
                    || connected.lock().unwrap().iter().any(|c| c.port == port)
                {
                    continue;
                }
                let Some(path) = path else {
                    continue;
                };
                if let Some(info) = hid_found.get(&port) {
                    // Not again and again: once it failed, only after it's
                    // plugged in again, or on Refresh.
                    if shared.problems.lock().unwrap().contains_key(&port) {
                        continue;
                    }
                    let opened = hid_api
                        .as_ref()
                        .map(|h| h.open(info))
                        .unwrap_or_else(|| Err("USB HID is unavailable".into()));
                    let device = match opened {
                        Ok(d) => d,
                        Err(e) => {
                            let problem = hid_open_problem(&e);
                            eprintln!("{port}: cannot open the HID device: {e}");
                            shared
                                .problems
                                .lock()
                                .unwrap()
                                .insert(port.clone(), problem);
                            continue;
                        }
                    };
                    let d_stop = Arc::new(AtomicBool::new(false));
                    let thread = {
                        let (registry, path, port, d_stop, connected) = (
                            registry.clone(),
                            path.clone(),
                            port.clone(),
                            d_stop.clone(),
                            connected.clone(),
                        );
                        std::thread::Builder::new()
                            .name(format!("hid {port}"))
                            .spawn(move || {
                                run_hid(&port, &path, device, registry, &d_stop, &connected)
                            })
                            .expect("spawn controller thread")
                    };
                    devices.insert(
                        port,
                        Device {
                            stop: d_stop,
                            thread,
                            mapping: path,
                            own: None,
                        },
                    );
                    continue;
                }
                let own = own_for(&path, &versions);
                let d_stop = Arc::new(AtomicBool::new(false));
                let stand_in = stand_ins.iter().find(|s| s.name == port).map(|s| {
                    let (tx, rx) = mpsc::channel();
                    *s.sender.lock().unwrap() = Some(tx);
                    rx
                });
                let thread = {
                    let (registry, path, own, port, d_stop, connected) = (
                        registry.clone(),
                        path.clone(),
                        own.clone(),
                        port.clone(),
                        d_stop.clone(),
                        connected.clone(),
                    );
                    std::thread::Builder::new()
                        .name(format!("midi {port}"))
                        .spawn(move || match stand_in {
                            Some(rx) => {
                                let name = port.clone();
                                run_controller(
                                    &port,
                                    &path,
                                    own.as_deref(),
                                    registry,
                                    &d_stop,
                                    &connected,
                                    rx,
                                    |m| eprintln!("{name} <- {m:02X?}"),
                                )
                            }
                            None => run_device(
                                &port,
                                &path,
                                own.as_deref(),
                                registry,
                                &d_stop,
                                &connected,
                            ),
                        })
                        .expect("spawn controller thread")
                };
                devices.insert(
                    port,
                    Device {
                        stop: d_stop,
                        thread,
                        mapping: path,
                        own,
                    },
                );
            }
        }
        wait_for_system(Duration::from_millis(100));
    }
    for (_, d) in devices.drain() {
        d.stop.store(true, Ordering::Relaxed);
        let _ = d.thread.join();
    }
    for (_, k) in keyboards.drain() {
        k.stop.store(true, Ordering::Relaxed);
        let _ = k.thread.join();
    }
}

/// A device with no mapping, open to play the instrument.
struct Keyboard {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

/// Opens the devices that have no mapping (and aren't the computer's own
/// ports or turned off) as keyboards, and closes those that went away or
/// got a mapping.
fn follow_keyboards(
    keyboards: &mut HashMap<String, Keyboard>,
    picked: &[(String, Option<PathBuf>)],
    stand_ins: &[StandIn],
    shared: &Arc<Shared>,
    registry: &Arc<ControlRegistry>,
) {
    let choices = shared.choices.lock().unwrap().clone();
    let hid_ports = shared.hid_ports.lock().unwrap().clone();
    let wanted: Vec<&String> = picked
        .iter()
        .filter(|(port, path)| {
            plays_instruments(
                port,
                path.is_some(),
                choices.get(&device_key(port)),
                hid_ports.contains_key(port.as_str()),
            ) && !stand_ins.iter().any(|s| &s.name == port)
        })
        .map(|(port, _)| port)
        .collect();
    keyboards.retain(|port, k| {
        let keep = wanted.contains(&port) && !k.thread.is_finished();
        if !keep {
            k.stop.store(true, Ordering::Relaxed);
        }
        keep
    });
    for port in wanted {
        if keyboards.contains_key(port) {
            continue;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (port, stop, shared) = (port.clone(), stop.clone(), shared.clone());
            let watch = crate::routing::DeckWatch::new(registry);
            eprintln!("{port}: plays instruments");
            std::thread::Builder::new()
                .name(format!("midi keys {port}"))
                .spawn(move || run_keyboard(&port, &stop, &shared, watch))
                .expect("spawn MIDI keyboard thread")
        };
        keyboards.insert(port.clone(), Keyboard { stop, thread });
    }
    let mut names: Vec<String> = keyboards.keys().cloned().collect();
    names.sort();
    *shared.keyboards.lock().unwrap() = names;
}

/// Whether a device plays instruments: one with no mapping running, unless
/// it is off, one of the computer's own ports, a keyboard's DAW port, or the
/// MIDI side of an HID controller that runs its HID mapping (the Maschine
/// Mikro MK3's). Picking "Play instruments" for it always does.
fn plays_instruments(
    port: &str,
    mapped: bool,
    choice: Option<&MappingChoice>,
    hid_twin: bool,
) -> bool {
    match choice {
        Some(MappingChoice::Instrument) => true,
        Some(MappingChoice::Off) => false,
        _ => {
            !mapped
                && !hid_twin
                && !crate::is_system_port(port)
                && !crate::routing::is_daw_port(port)
        }
    }
}

/// Channel messages (notes, controllers, pitch bend) from `port` go to the
/// note sink until `stop`, while the deck it is set to holds the instrument
/// and it is the keyboard and channel chosen to play it (see
/// [`crate::routing`]).
fn run_keyboard(
    port: &str,
    stop: &AtomicBool,
    shared: &Arc<Shared>,
    watch: crate::routing::DeckWatch,
) {
    let Ok(mut input) = MidiInput::new(CLIENT) else {
        return;
    };
    input.ignore(Ignore::All);
    let Some(in_port) = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).as_deref() == Ok(port))
    else {
        return;
    };
    let (name, shared_in) = (port.to_string(), shared.clone());
    let route = crate::routing::route(port);
    let device = crate::routing::device_hash(port);
    let conn = input.connect(
        &in_port,
        "DJ Sugar keys",
        move |_, msg, _| {
            monitor(&name, msg);
            if LEARNING.load(Ordering::Relaxed) {
                return;
            }
            let Some(data) = channel_message(msg) else {
                return;
            };
            let note_on = data[0] & 0xF0 == 0x90 && data[2] > 0;
            // Pinned to a deck without the instrument: nothing reaches it,
            // but note offs still do so no note is left hanging.
            let choice = crate::routing::DeckChoice::from_u8(route.load(Ordering::Relaxed));
            let note_off = data[0] & 0xF0 == 0x80 || data[0] & 0xF0 == 0x90 && data[2] == 0;
            if !(watch.plays(choice) && crate::routing::from_source(device, data[0])) && !note_off {
                return;
            }
            if let Some(sink) = shared_in.notes.lock().unwrap().as_ref() {
                if note_on {
                    crate::routing::heard(&name);
                }
                sink(data);
            }
        },
        (),
    );
    let Ok(_conn) = conn else {
        return;
    };
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A channel voice message as three bytes, the unused one 0; anything else
/// (system messages, running status cut short) is none.
fn channel_message(msg: &[u8]) -> Option<[u8; 3]> {
    let status = *msg.first()?;
    let len = match status & 0xF0 {
        0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => 3,
        0xC0 | 0xD0 => 2,
        _ => return None,
    };
    if msg.len() < len {
        return None;
    }
    Some([status, msg[1], if len == 3 { msg[2] } else { 0 }])
}

/// Waits `time` on the device thread. On macOS the thread's run loop runs
/// meanwhile: CoreMIDI tells this process about devices plugged in or out
/// through the run loop of the thread that opened MIDI first, and without it
/// the device list stays as it was at start.
#[cfg(target_os = "macos")]
fn wait_for_system(time: Duration) {
    use std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after: u8) -> i32;
    }
    const FINISHED: i32 = 1;
    let start = Instant::now();
    // SAFETY: runs this thread's own run loop in the default mode.
    let result = unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, time.as_secs_f64(), 0) };
    // With nothing to wait on, the run loop returns at once.
    if result == FINISHED {
        std::thread::sleep(time.saturating_sub(start.elapsed()));
    }
}

#[cfg(not(target_os = "macos"))]
fn wait_for_system(time: Duration) {
    std::thread::sleep(time);
}

fn run_device(
    port: &str,
    mapping_path: &Path,
    own: Option<&Path>,
    registry: Arc<ControlRegistry>,
    stop: &AtomicBool,
    connected: &Mutex<Vec<ConnectedController>>,
) {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let Ok(mut input) = MidiInput::new(CLIENT) else {
        return;
    };
    input.ignore(Ignore::TimeAndActiveSense);
    let Some(in_port) = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).as_deref() == Ok(port))
    else {
        return;
    };
    let _in_conn = match input.connect(
        &in_port,
        "DJ Sugar in",
        move |_, msg, _| {
            let _ = tx.send(msg.to_vec());
        },
        (),
    ) {
        Ok(c) => c,
        Err(e) => {
            // Windows lets only one app at a time open a MIDI device.
            let hint = if cfg!(windows) {
                " (is another DJ app using it?)"
            } else {
                ""
            };
            eprintln!("{port}: cannot open MIDI input: {e}{hint}");
            return;
        }
    };
    // The output with the same name, or failing that the closest one: some
    // systems name the two directions slightly differently.
    let mut out_conn = MidiOutput::new(CLIENT).ok().and_then(|output| {
        let ports = output.ports();
        let want = tokens(port);
        let p = ports
            .iter()
            .find(|p| output.port_name(p).as_deref() == Ok(port))
            .or_else(|| {
                ports
                    .iter()
                    .find(|p| output.port_name(p).is_ok_and(|n| tokens(&n) == want))
            })?
            .clone();
        output.connect(&p, "DJ Sugar out").ok()
    });
    let send = move |m: &[u8]| {
        if let Some(out) = out_conn.as_mut() {
            let _ = out.send(m);
        }
    };
    run_controller(port, mapping_path, own, registry, stop, connected, rx, send);
}

/// Runs a mapping until `stop` is set or the input ends.
#[allow(clippy::too_many_arguments)]
fn run_controller(
    port: &str,
    mapping_path: &Path,
    own: Option<&Path>,
    registry: Arc<ControlRegistry>,
    stop: &AtomicBool,
    connected: &Mutex<Vec<ConnectedController>>,
    rx: mpsc::Receiver<Vec<u8>>,
    mut send: impl FnMut(&[u8]),
) {
    let mapping = match load_mapping(mapping_path, own) {
        Ok(m) => m,
        Err(e) => {
            let file = own.unwrap_or(mapping_path);
            eprintln!("{port}: cannot read {}: {e}", file.display());
            return;
        }
    };
    let mapping_name = mapping.name.clone();
    let mut controller = match Controller::new(port, mapping, registry) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{port}: mapping {mapping_name} failed to load: {e}");
            return;
        }
    };
    controller.publish_positions();
    controller.follow_deck_choice(port);
    eprintln!("{port}: using mapping {mapping_name}");
    connected.lock().unwrap().push(ConnectedController {
        port: port.to_string(),
        mapping: mapping_name,
        file: mapping_path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
    });
    let mut flush = |c: &mut Controller| {
        for m in c.take_output() {
            send(&m);
        }
    };
    flush(&mut controller);
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        let deadline = controller
            .next_deadline()
            .map_or(now + POLL, |d| d.min(now + POLL));
        match rx.recv_timeout(deadline.saturating_duration_since(now)) {
            Ok(msg) => {
                let learning = LEARNING.load(Ordering::Relaxed);
                monitor(port, &msg);
                if !learning {
                    controller.receive(&msg);
                }
                while let Ok(msg) = rx.try_recv() {
                    monitor(port, &msg);
                    if !learning {
                        controller.receive(&msg);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        controller.poll(Instant::now());
        flush(&mut controller);
    }
    controller.shutdown();
    controller.poll(Instant::now());
    flush(&mut controller);
    connected.lock().unwrap().retain(|c| c.port != port);
}

/// What to tell the DJ when an HID device won't open.
fn hid_open_problem(error: &str) -> String {
    if cfg!(target_os = "linux") {
        "Linux didn't let DJ Sugar open it. Add the udev rule from the controllers guide, \
         then plug it in again."
            .into()
    } else if cfg!(windows) {
        format!("Windows didn't let DJ Sugar open it ({error}). Is another DJ app using it?")
    } else {
        format!("The system didn't let DJ Sugar open it ({error}). Quit Traktor and try again.")
    }
}

/// Runs an HID mapping until `stop` is set or the device goes away.
fn run_hid(
    port: &str,
    mapping_path: &Path,
    device: hid::Device,
    registry: Arc<ControlRegistry>,
    stop: &AtomicBool,
    connected: &Mutex<Vec<ConnectedController>>,
) {
    let mapping = match Mapping::load(mapping_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{port}: cannot read {}: {e}", mapping_path.display());
            return;
        }
    };
    let mapping_name = mapping.name.clone();
    let device = std::rc::Rc::new(device);
    let mut controller = match Controller::new_hid(port, mapping, registry, device.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{port}: mapping {mapping_name} failed to load: {e}");
            return;
        }
    };
    controller.follow_deck_choice(port);
    eprintln!("{port}: using HID mapping {mapping_name}");
    connected.lock().unwrap().push(ConnectedController {
        port: port.to_string(),
        mapping: mapping_name,
        file: mapping_path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
    });
    let flush = |c: &mut Controller| {
        for report in c.take_reports() {
            if let Err(e) = device.write(&report) {
                eprintln!("{port}: HID write failed: {e}");
                c.report_failed(report[0]);
            }
        }
    };
    flush(&mut controller);
    let mut buf = [0u8; hid::BUFFER];
    'run: while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        let deadline = controller
            .next_deadline()
            .map_or(now + POLL, |d| d.min(now + POLL));
        let mut wait = deadline.saturating_duration_since(now).as_millis() as i32;
        // Everything waiting, then on to timers and lights.
        loop {
            match device.read(&mut buf, wait) {
                Ok(0) => break,
                Ok(n) => controller.receive_report(&buf[..n]),
                Err(e) => {
                    // Unplugged: the next scan notices it's gone.
                    eprintln!("{port}: HID read failed: {e}");
                    break 'run;
                }
            }
            wait = 0;
        }
        controller.poll(Instant::now());
        flush(&mut controller);
    }
    controller.shutdown();
    controller.poll(Instant::now());
    flush(&mut controller);
    connected.lock().unwrap().retain(|c| c.port != port);
}

/// A stand-in device for trying mappings without hardware:
/// `DJSUGAR_VIRTUAL_MIDI="Midi Fighter Twister=/tmp/twister"` reads MIDI
/// messages as hex lines ("B0 11 40") from that file or FIFO, as if they came
/// from a device with that name. It's listed with the devices plugged in,
/// so it gets a mapping the same way. Messages to the device are printed.
struct StandIn {
    name: String,
    /// Where the lines read go: the controller running for it now.
    sender: Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>,
}

fn stand_in_devices() -> Vec<StandIn> {
    let Ok(spec) = std::env::var("DJSUGAR_VIRTUAL_MIDI") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in spec.split(';').filter(|e| !e.is_empty()) {
        let Some((port, file)) = entry.split_once('=') else {
            continue;
        };
        let sender: Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>> = Arc::default();
        let (to, file) = (sender.clone(), PathBuf::from(file));
        std::thread::spawn(move || {
            use std::io::BufRead;
            let Ok(f) = std::fs::File::open(&file) else {
                return;
            };
            for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                let bytes: Vec<u8> = line
                    .split_whitespace()
                    .filter_map(|b| u8::from_str_radix(b, 16).ok())
                    .collect();
                if let (false, Some(tx)) = (bytes.is_empty(), to.lock().unwrap().as_ref()) {
                    let _ = tx.send(bytes);
                }
            }
        });
        out.push(StandIn {
            name: port.to_string(),
            sender,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_devices_with_nothing_else_to_do_play_instruments() {
        let off = MappingChoice::Off;
        let keys = MappingChoice::Instrument;
        assert!(plays_instruments("Arturia MicroFreak", false, None, false));
        // A DJ controller running its mapping never does.
        assert!(!plays_instruments("XONE:K2", true, None, false));
        assert!(!plays_instruments(
            "Arturia MicroFreak",
            false,
            Some(&off),
            false
        ));
        // DAW ports, the computer's own ports and an HID controller's MIDI
        // side don't, unless picked.
        assert!(!plays_instruments(
            "Komplete Kontrol M DAW",
            false,
            None,
            false
        ));
        assert!(!plays_instruments("Network Session 1", false, None, false));
        assert!(!plays_instruments("Maschine Mikro MK3", false, None, true));
        assert!(plays_instruments(
            "Komplete Kontrol M DAW",
            false,
            Some(&keys),
            false
        ));
    }

    #[test]
    fn keyboards_send_channel_messages_only() {
        assert_eq!(channel_message(&[0x90, 60, 100]), Some([0x90, 60, 100]));
        assert_eq!(channel_message(&[0xE1, 0, 64]), Some([0xE1, 0, 64]));
        assert_eq!(channel_message(&[0xC0, 5]), Some([0xC0, 5, 0]));
        assert_eq!(channel_message(&[0x90, 60]), None, "cut short");
        assert_eq!(channel_message(&[0xF8]), None, "clock");
        assert_eq!(channel_message(&[]), None);
    }

    #[test]
    fn matches_ports_to_mappings() {
        let dir = crate::default_mappings_dir().unwrap();
        let mappings = list_mappings(&dir);
        assert!(mappings.len() > 100);
        let name = |port: &str| {
            match_mapping(port, &mappings)
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        };
        for (port, file) in [
            ("DDJ-FLX4", "Pioneer-DDJ-FLX4.midi.xml"),
            ("DDJ-FLX4:DDJ-FLX4 MIDI 1 24:0", "Pioneer-DDJ-FLX4.midi.xml"),
            // DJ Sugar's own K2 mapping comes before Mixxx's.
            (
                "XONE:K2",
                "Allen and Heath Xone K2 - 2 Decks.djsugar-mapping",
            ),
            (
                "XONE:K2:XONE:K2 MIDI 1 28:0",
                "Allen and Heath Xone K2 - 2 Decks.djsugar-mapping",
            ),
            ("PIONEER DDJ-SX", "Pioneer DDJ-SX.midi.xml"),
            ("PIONEER DDJ-SX2", "Pioneer DDJ-SX2.midi.xml"),
            ("PIONEER DDJ-RX", "Pioneer DDJ-RX.midi.xml"),
            ("PIONEER DDJ-800", "Pioneer-DDJ-800.midi.xml"),
            ("XDJ-AZ", "Pioneer-XDJ-AZ.midi.xml"),
            ("DDJ-FLX10", "Pioneer-DDJ-FLX10.midi.xml"),
            ("DDJ-FLX6", "Pioneer-DDJ-FLX6.midi.xml"),
            ("XDJ-RX3", "Pioneer-XDJ-RX3.midi.xml"),
            ("DDJ-400", "Pioneer-DDJ-400.midi.xml"),
            ("DDJ-200", "Pioneer DDJ-200.midi.xml"),
            (
                "Midi Fighter Twister",
                "DJ TechTools MIDI Fighter Twister.midi.xml",
            ),
            (
                "Midi Fighter Twister:Midi Fighter Twister MIDI 1 32:0",
                "DJ TechTools MIDI Fighter Twister.midi.xml",
            ),
        ] {
            assert_eq!(name(port).as_deref(), Some(file), "{port}");
        }
        assert_eq!(name("Generic USB MIDI 1"), None);
    }
}
