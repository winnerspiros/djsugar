//! Settings › Controllers › OSC: starts and stops the OSC port
//! (`djsugar_osc`) as the settings say.
//!
//! Off at first. The port runs on a thread of its own; this only opens it
//! again when a setting changes. Switches apply at once; typed settings
//! (the port, the send targets) once typing pauses, so a half-typed
//! port number isn't opened for every key.
//!
//! While it runs, [`osc_events`](crate::osc_events) sends beats, phrases,
//! loads and hot cues, and when it's open to the network it's advertised
//! over Bonjour as `_osc._udp`, so TouchOSC and others list it.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use djsugar_control::ControlRegistry;
use djsugar_engine::{LoadedTracks, VisualPositions};
use djsugar_osc::{Config, Osc, DEFAULT_PORT};
use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::settings;

/// The switch: off at first.
pub const ENABLED: &str = "osc";
/// The port to listen on.
pub const PORT: &str = "osc_port";
/// Listen only for this computer: on at first.
pub const LOCAL_ONLY: &str = "osc_local_only";
/// Ignore incoming messages but subscriptions.
pub const SEND_ONLY: &str = "osc_send_only";
/// Where feedback and events go: `host:port`, separated by commas.
pub const TARGETS: &str = "osc_targets";

static REGISTRY: OnceLock<Arc<ControlRegistry>> = OnceLock::new();
static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

struct Running {
    config: Config,
    osc: Osc,
    /// The Bonjour advert, while open to the network.
    _advert: Option<Advert>,
}

/// `DJ Sugar._osc._udp.local.`, withdrawn when dropped.
struct Advert {
    daemon: ServiceDaemon,
    name: String,
}

impl Advert {
    fn start(port: u16) -> Option<Self> {
        let daemon = ServiceDaemon::new().ok()?;
        let host = format!("{}.local.", hostname().unwrap_or_else(|| "djsugar".into()));
        let info = ServiceInfo::new(
            "_osc._udp.local.",
            "DJ Sugar",
            &host,
            "",
            port,
            None::<std::collections::HashMap<String, String>>,
        )
        .ok()?
        .enable_addr_auto();
        let name = info.get_fullname().to_string();
        daemon.register(info).ok()?;
        Some(Self { daemon, name })
    }
}

impl Drop for Advert {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.name);
        let _ = self.daemon.shutdown();
    }
}

/// This computer's name for its `.local` address, letters, digits and
/// dashes only.
fn hostname() -> Option<String> {
    let name = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
        })?;
    let name = name.trim().trim_end_matches(".local");
    let clean: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    (!clean.is_empty()).then_some(clean)
}
/// Why the port couldn't open, if it couldn't.
static PROBLEM: Mutex<String> = Mutex::new(String::new());
/// When a typed setting last changed, until it's applied.
static CHANGED: Mutex<Option<Instant>> = Mutex::new(None);
/// How long typing must pause before a new port or target list applies.
const SETTLE: Duration = Duration::from_millis(800);

/// Starts OSC if it's switched on. Call once at startup.
pub fn install(
    registry: &Arc<ControlRegistry>,
    loaded: LoadedTracks,
    visual: Arc<VisualPositions>,
) {
    let _ = REGISTRY.set(registry.clone());
    apply();
    crate::osc_events::spawn(
        registry.clone(),
        loaded,
        visual,
        || RUNNING.lock().is_ok_and(|r| r.is_some()),
        |events| {
            if let Ok(r) = RUNNING.lock() {
                if let Some(r) = r.as_ref() {
                    for e in events {
                        r.osc.send_event(e);
                    }
                }
            }
        },
    );
}

/// Marks a typed setting changed; [`tick`] applies it.
pub fn settings_changed() {
    if let Ok(mut c) = CHANGED.lock() {
        *c = Some(Instant::now());
    }
}

/// Applies typed settings that changed. Call a couple of times a second.
pub fn tick() {
    let due = CHANGED.lock().is_ok_and(|mut c| {
        let due = c.is_some_and(|t| t.elapsed() >= SETTLE);
        if due {
            *c = None;
        }
        due
    });
    if due {
        apply();
    }
}

/// The port number in Settings, or 9000.
pub fn port() -> u16 {
    let p = settings::number(PORT, DEFAULT_PORT as f64);
    if (1.0..=65535.0).contains(&p) {
        p as u16
    } else {
        DEFAULT_PORT
    }
}

/// The send targets as typed, and which of them can't be understood.
pub fn targets() -> (Vec<std::net::SocketAddr>, Vec<String>) {
    let mut good = Vec::new();
    let mut bad = Vec::new();
    for t in settings::string(TARGETS).split([',', ' ', '\n']) {
        if t.trim().is_empty() {
            continue;
        }
        match djsugar_osc::parse_target(t) {
            Some(a) => good.push(a),
            None => bad.push(t.trim().to_string()),
        }
    }
    (good, bad)
}

fn config() -> Option<Config> {
    if !settings::flag(ENABLED, false) {
        return None;
    }
    Some(Config {
        port: port(),
        local_only: settings::flag(LOCAL_ONLY, true),
        send_only: settings::flag(SEND_ONLY, false),
        targets: targets().0,
    })
}

/// Opens, moves or closes the port to match the settings.
pub fn apply() {
    let Some(registry) = REGISTRY.get() else {
        return;
    };
    let wanted = config();
    let Ok(mut running) = RUNNING.lock() else {
        return;
    };
    if running.as_ref().map(|r| &r.config) == wanted.as_ref() {
        return;
    }
    // Close the old port first, so the same port number can open again.
    *running = None;
    let mut problem = String::new();
    if let Some(config) = wanted {
        match Osc::start(registry.clone(), config.clone()) {
            Ok(osc) => {
                let advert = (!config.local_only)
                    .then(|| Advert::start(osc.port()))
                    .flatten();
                *running = Some(Running {
                    config,
                    osc,
                    _advert: advert,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                problem = format!(
                    "Port {} is used by another app. Pick another port.",
                    config.port
                );
            }
            Err(e) => problem = format!("OSC couldn't start: {e}"),
        }
    }
    if let Ok(mut p) = PROBLEM.lock() {
        *p = problem;
    }
}

/// The line under the switch: where it listens and who's connected, or
/// what went wrong.
pub fn status() -> String {
    let problem = PROBLEM.lock().map(|p| p.clone()).unwrap_or_default();
    if !problem.is_empty() {
        return problem;
    }
    let Ok(running) = RUNNING.lock() else {
        return String::new();
    };
    let Some(Running { config, osc, .. }) = running.as_ref() else {
        return String::new();
    };
    let whom = if config.local_only {
        "apps on this computer".to_string()
    } else {
        match djsugar_remote::local_addresses().first() {
            Some((_, ip)) => format!("tablets and apps at {ip}"),
            None => "apps on the network".to_string(),
        }
    };
    let clients = match osc.clients() {
        0 => "no apps yet".to_string(),
        1 => "1 app".to_string(),
        n => format!("{n} apps"),
    };
    format!("Listening on port {} for {whom} · {clients}", osc.port())
}
