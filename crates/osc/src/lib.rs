//! Open Sound Control: every control by address, with feedback.
//!
//! Tablets (TouchOSC, Open Stage Control), visuals (Resolume, VDMX,
//! TouchDesigner) and show software (QLab, lighting desks) speak OSC over
//! UDP. Each control in the registry gets an address made from its name:
//! `[Channel1],play` is `/channel1/play` (see [`address_of`]). A message
//! with one number sets the control, as a MIDI controller would; a message
//! with no arguments asks for its value. A client that set, asked for or
//! `/subscribe`d to a control is sent its new value whenever it changes, at
//! most 30 times a second.
//!
//! Everything runs on one thread of its own. Setting a control is the same
//! atomic write a controller mapping makes, so nothing here touches the
//! audio thread. Dropping [`Osc`] closes the port.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use djsugar_control::{ConfigKey, ControlHandle, ControlRegistry};
use rosc::address::{Matcher, OscAddress};
use rosc::OscPacket;
pub use rosc::{OscMessage, OscType};

/// The port DJ Sugar listens on unless the DJ picks another.
pub const DEFAULT_PORT: u16 = 9000;
/// The most feedback passes a second, so the most times a control's value
/// is sent each second.
pub const FEEDBACK_RATE: u32 = 30;
/// Clients remembered at once; the one heard from longest ago goes first.
/// Send targets don't count.
const MAX_CLIENTS: usize = 32;
/// How often new controls (made when a skin, mapping or plugin loads) are
/// looked for.
const TABLE_CHECK: Duration = Duration::from_secs(1);

/// How the OSC port is set up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The UDP port to listen on; 0 picks a free one (for tests).
    pub port: u16,
    /// Listen only for messages from this computer.
    pub local_only: bool,
    /// Ignore incoming messages other than `/subscribe` and `/unsubscribe`.
    pub send_only: bool,
    /// Where to send feedback and events. A message from one of these
    /// computers is answered here rather than at the port it came from,
    /// since apps such as TouchOSC listen on a port of their own.
    pub targets: Vec<SocketAddr>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            local_only: true,
            send_only: false,
            targets: Vec::new(),
        }
    }
}

/// A control's OSC address: the group lower-cased with its brackets
/// dropped and spaces made `_`, then the key. `[Channel1],play` is
/// `/channel1/play`, `[EqualizerRack1_[Channel1]_Effect1],parameter1` is
/// `/equalizerrack1_channel1_effect1/parameter1`. Brackets are OSC pattern
/// characters, so they can't stay.
pub fn address_of(key: &ConfigKey) -> String {
    let group: String = key
        .group
        .chars()
        .filter(|c| !matches!(c, '[' | ']'))
        .map(|c| {
            if c == ' ' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    format!("/{group}/{}", key.item)
}

/// Parses a send target, `host:port` or a bare host (port 9000). Names
/// are looked up once here.
pub fn parse_target(text: &str) -> Option<SocketAddr> {
    use std::net::ToSocketAddrs;
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Some(addr);
    }
    if let Ok(ip) = text.parse::<IpAddr>() {
        return Some(SocketAddr::new(ip, DEFAULT_PORT));
    }
    let with_port = if text.contains(':') {
        text.to_string()
    } else {
        format!("{text}:{DEFAULT_PORT}")
    };
    with_port.to_socket_addrs().ok()?.find(|a| a.is_ipv4())
}

/// The running OSC port. Dropping it stops the thread and closes the port.
pub struct Osc {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    port: u16,
    clients: Arc<AtomicUsize>,
    outbox: Arc<Mutex<Vec<OscMessage>>>,
}

impl Osc {
    /// Opens the port and starts answering. Fails if the port is taken.
    pub fn start(registry: Arc<ControlRegistry>, config: Config) -> io::Result<Self> {
        let ip = if config.local_only {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        };
        let socket = UdpSocket::bind(SocketAddr::new(ip, config.port))?;
        socket.set_read_timeout(Some(Duration::from_millis(5)))?;
        let port = socket.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let clients = Arc::new(AtomicUsize::new(0));
        let outbox = Arc::new(Mutex::new(Vec::new()));
        let mut server = Server::new(socket, registry, config, clients.clone(), outbox.clone());
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("osc".into())
                .spawn(move || server.run(&stop))?
        };
        Ok(Self {
            stop,
            thread: Some(thread),
            port,
            clients,
            outbox,
        })
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Apps that have talked to it, send targets not included.
    pub fn clients(&self) -> usize {
        self.clients.load(Ordering::Relaxed)
    }

    /// Sends an event, such as a track loading, to every send target and
    /// to each client subscribed to a pattern that matches its address,
    /// within a few milliseconds.
    pub fn send_event(&self, message: OscMessage) {
        if let Ok(mut out) = self.outbox.lock() {
            out.push(message);
        }
    }
}

impl Drop for Osc {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Entry {
    address: OscAddress,
    handle: ControlHandle,
}

/// Every control by its lower-cased address.
#[derive(Default)]
struct Table {
    entries: HashMap<String, Entry>,
    /// The registry's size when built: a different size means new controls.
    built_from: usize,
}

impl Table {
    fn build(registry: &ControlRegistry) -> Self {
        let mut entries = HashMap::new();
        let keys = registry.keys().into_iter().chain(registry.alias_keys());
        for key in keys {
            let address = address_of(&key);
            let Ok(osc) = OscAddress::new(address.clone()) else {
                continue;
            };
            let Some(handle) = registry.get(&key.group, &key.item) else {
                continue;
            };
            entries
                .entry(address.to_ascii_lowercase())
                .or_insert(Entry {
                    address: osc,
                    handle,
                });
        }
        Self {
            entries,
            built_from: registry.len(),
        }
    }

    /// The controls an address names: one, or every match of a pattern.
    fn lookup(&self, address: &str) -> Vec<(&String, &Entry)> {
        let lower = address.to_ascii_lowercase();
        if !lower.contains(['*', '?', '[', '{']) {
            return self.entries.get_key_value(&lower).into_iter().collect();
        }
        match Matcher::new(&lower) {
            Ok(m) => self.matching(&m),
            Err(_) => Vec::new(),
        }
    }

    fn matching(&self, m: &Matcher) -> Vec<(&String, &Entry)> {
        self.entries
            .iter()
            .filter(|(lower, _)| {
                OscAddress::new((*lower).clone()).is_ok_and(|a| m.match_address(&a))
            })
            .collect()
    }
}

/// A control a client follows.
struct Watch {
    /// The address it's sent under: the client's own spelling.
    reply: String,
    /// The version last sent (or set by this client); `None` sends now.
    sent: Option<u32>,
}

struct Client {
    /// Where its feedback goes.
    to: SocketAddr,
    /// A send target from Settings, never forgotten.
    target: bool,
    last_heard: Instant,
    /// Controls followed, by lower-cased address.
    watching: HashMap<String, Watch>,
    /// `/subscribe` patterns, lower-cased, kept so controls made later
    /// are followed too.
    patterns: Vec<Matcher>,
}

impl Client {
    fn new(to: SocketAddr, target: bool) -> Self {
        Self {
            to,
            target,
            last_heard: Instant::now(),
            watching: HashMap::new(),
            patterns: Vec::new(),
        }
    }

    fn watch(&mut self, lower: &str, reply: &str, sent: Option<u32>) {
        let w = self.watching.entry(lower.to_string()).or_insert(Watch {
            reply: reply.to_string(),
            sent: None,
        });
        w.reply = reply.to_string();
        w.sent = sent;
    }

    /// Follows every control in the table its patterns match.
    fn follow_patterns(&mut self, table: &Table) {
        for m in &self.patterns {
            for (lower, e) in table.matching(m) {
                self.watching.entry(lower.clone()).or_insert(Watch {
                    reply: e.address.to_string(),
                    sent: None,
                });
            }
        }
    }
}

struct Server {
    socket: UdpSocket,
    registry: Arc<ControlRegistry>,
    config: Config,
    table: Table,
    clients: Vec<Client>,
    client_count: Arc<AtomicUsize>,
    outbox: Arc<Mutex<Vec<OscMessage>>>,
}

impl Server {
    fn new(
        socket: UdpSocket,
        registry: Arc<ControlRegistry>,
        config: Config,
        client_count: Arc<AtomicUsize>,
        outbox: Arc<Mutex<Vec<OscMessage>>>,
    ) -> Self {
        let clients = config
            .targets
            .iter()
            .map(|t| Client::new(*t, true))
            .collect();
        Self {
            table: Table::build(&registry),
            socket,
            registry,
            config,
            clients,
            client_count,
            outbox,
        }
    }

    fn run(&mut self, stop: &AtomicBool) {
        let mut buf = vec![0u8; 65_536];
        let period = Duration::from_secs(1) / FEEDBACK_RATE;
        let mut last_feedback = Instant::now();
        let mut last_check = Instant::now();
        while !stop.load(Ordering::Acquire) {
            match self.socket.recv_from(&mut buf) {
                Ok((n, from)) => {
                    if let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..n]) {
                        self.packet(packet, from);
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                // Windows reports an ICMP "port unreachable" from an
                // earlier send as an error on the next receive.
                Err(_) => {}
            }
            self.events();
            let now = Instant::now();
            if now.duration_since(last_check) >= TABLE_CHECK {
                last_check = now;
                if self.registry.len() != self.table.built_from {
                    self.table = Table::build(&self.registry);
                    for c in &mut self.clients {
                        c.follow_patterns(&self.table);
                    }
                }
            }
            if now.duration_since(last_feedback) >= period {
                last_feedback = now;
                self.feedback();
            }
        }
    }

    fn packet(&mut self, packet: OscPacket, from: SocketAddr) {
        match packet {
            OscPacket::Message(m) => self.message(m, from),
            OscPacket::Bundle(b) => {
                for p in b.content {
                    self.packet(p, from);
                }
            }
        }
    }

    /// The client a message came from: a send target on the same computer
    /// if there is one, otherwise the address it was sent from.
    fn client(&mut self, from: SocketAddr) -> &mut Client {
        let i = match self
            .clients
            .iter()
            .position(|c| c.target && c.to.ip() == from.ip())
            .or_else(|| self.clients.iter().position(|c| c.to == from))
        {
            Some(i) => i,
            None => {
                let others = self.clients.iter().filter(|c| !c.target).count();
                if others >= MAX_CLIENTS {
                    if let Some(oldest) = self
                        .clients
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| !c.target)
                        .min_by_key(|(_, c)| c.last_heard)
                        .map(|(i, _)| i)
                    {
                        self.clients.remove(oldest);
                    }
                }
                self.clients.push(Client::new(from, false));
                self.client_count.store(
                    self.clients.iter().filter(|c| !c.target).count(),
                    Ordering::Relaxed,
                );
                self.clients.len() - 1
            }
        };
        let c = &mut self.clients[i];
        c.last_heard = Instant::now();
        c
    }

    fn message(&mut self, m: OscMessage, from: SocketAddr) {
        match m.addr.as_str() {
            "/subscribe" | "/unsubscribe" => {
                let on = m.addr == "/subscribe";
                let patterns: Vec<Matcher> = m
                    .args
                    .iter()
                    .filter_map(|a| match a {
                        OscType::String(s) => Matcher::new(&s.to_ascii_lowercase()).ok(),
                        _ => None,
                    })
                    .collect();
                let table = std::mem::take(&mut self.table);
                let client = self.client(from);
                for p in patterns {
                    if on {
                        client.patterns.push(p);
                    } else {
                        client.patterns.retain(|q| q.pattern != p.pattern);
                        for (lower, _) in table.matching(&p) {
                            client.watching.remove(lower);
                        }
                    }
                }
                if on {
                    client.follow_patterns(&table);
                }
                self.table = table;
            }
            _ if self.config.send_only => {}
            address => {
                let address = address.to_string();
                let value = m.args.first().and_then(number);
                let table = std::mem::take(&mut self.table);
                let found = table.lookup(&address);
                if found.is_empty() {
                    self.table = table;
                    return;
                }
                let patterned = found.len() > 1 || address.contains(['*', '?', '[', '{']);
                let mut replies = Vec::new();
                let client = self.client(from);
                for (lower, e) in found {
                    let reply = if patterned {
                        e.address.to_string()
                    } else {
                        address.clone()
                    };
                    match value {
                        Some(v) => {
                            e.handle.set(v);
                            // Not echoed back to the app that moved it,
                            // unless it didn't move (a locked control).
                            let moved = (e.handle.get() - v).abs() < 1e-9;
                            client.watch(lower, &reply, moved.then(|| e.handle.version()));
                        }
                        None if m.args.is_empty() => {
                            client.watch(lower, &reply, Some(e.handle.version()));
                            replies.push(value_message(&reply, e.handle.get()));
                        }
                        // A string or anything else: not a value.
                        None => {}
                    }
                }
                let to = client.to;
                for r in replies {
                    self.send(&r, to);
                }
                self.table = table;
            }
        }
    }

    /// Sends the events waiting to every send target, and to each client
    /// that subscribed to a pattern matching them (`/subscribe "/beat"`).
    fn events(&mut self) {
        let events = match self.outbox.lock() {
            Ok(mut o) if !o.is_empty() => std::mem::take(&mut *o),
            _ => return,
        };
        for e in &events {
            let address = OscAddress::new(e.addr.clone()).ok();
            for c in &self.clients {
                let wanted = c.target
                    || address
                        .as_ref()
                        .is_some_and(|a| c.patterns.iter().any(|p| p.match_address(a)));
                if wanted {
                    self.send(e, c.to);
                }
            }
        }
    }

    /// Sends the controls that changed since the last pass.
    fn feedback(&mut self) {
        let mut out: Vec<(OscMessage, SocketAddr)> = Vec::new();
        for c in &mut self.clients {
            for (lower, w) in &mut c.watching {
                let Some(e) = self.table.entries.get(lower) else {
                    continue;
                };
                let v = e.handle.version();
                if w.sent != Some(v) {
                    w.sent = Some(v);
                    out.push((value_message(&w.reply, e.handle.get()), c.to));
                }
            }
        }
        for (m, to) in out {
            self.send(&m, to);
        }
    }

    fn send(&self, m: &OscMessage, to: SocketAddr) {
        if let Ok(bytes) = rosc::encoder::encode(&OscPacket::Message(m.clone())) {
            let _ = self.socket.send_to(&bytes, to);
        }
    }
}

fn value_message(address: &str, value: f64) -> OscMessage {
    OscMessage {
        addr: address.to_string(),
        args: vec![OscType::Float(value as f32)],
    }
}

/// A value from an OSC argument: any number, or true/false as 1/0.
fn number(arg: &OscType) -> Option<f64> {
    match arg {
        OscType::Float(v) => Some(*v as f64),
        OscType::Double(v) => Some(*v),
        OscType::Int(v) => Some(*v as f64),
        OscType::Long(v) => Some(*v as f64),
        OscType::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        OscType::Inf => None,
        _ => None,
    }
    .filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests;
