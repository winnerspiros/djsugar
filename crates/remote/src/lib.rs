//! The phone remote: DJ Sugar serves a small web page on the local network,
//! and a phone that opens it controls Automix and the main levels.
//!
//! Nothing listens until the DJ switches the remote on. A phone pairs once,
//! with the one-time code in the QR code the app shows (or the PIN beside
//! it), and keeps a device token; the DJ can revoke a device at any time.
//!
//! The page talks over one WebSocket, `/ws`, in JSON, with the same
//! envelope add-ons use (`crates/plugins/src/remote.rs`):
//!
//! - request: `{"id": 1, "method": "automix.skip", "params": {}}`
//! - answer: `{"id": 1, "result": ...}` or `{"id": 1, "error": {"message": "..."}}`
//! - pushed state: `{"event": "automix", "data": {...}}`
//!
//! The first message is `hello` (a paired device's `device` and `token`)
//! or `pair` (the QR code's `code`, or a `pin`, with a `name`). After that,
//! requests go to the app through [`Server::poll`] and answers come back
//! with [`Server::reply`]. The app pushes state with [`Server::publish`];
//! a phone gets the latest of every topic as soon as it is let in, then
//! each change.
//!
//! The server runs on plain threads, one per phone: there are only ever a
//! few, and the app has no async runtime. Nothing here touches audio.

pub mod http;
pub mod overlay;
pub mod pairing;
pub mod qr;

use std::collections::{BTreeMap, HashMap};
use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

pub use pairing::{Device, PairError};

/// The port tried first; the next few are tried if it's taken.
pub const DEFAULT_PORT: u16 = 7477;
const PORT_TRIES: u16 = 10;
/// Connections at once; a party has one or two DJ phones.
const MAX_CLIENTS: usize = 16;
/// Largest message a phone may send.
const MAX_MESSAGE: usize = 64 * 1024;
/// Messages a phone may send per second, on average, and in a burst.
const RATE: f64 = 30.0;
const BURST: f64 = 60.0;
/// Wrong `hello`s before the connection is closed.
const MAX_BAD_HELLOS: u32 = 3;
/// How often a quiet connection is pinged, and when it counts as gone.
const PING_EVERY: Duration = Duration::from_secs(15);
const GONE_AFTER: Duration = Duration::from_secs(45);
/// How long each connection waits on its socket before checking for
/// things to send; also the most a pushed change waits.
const POLL: Duration = Duration::from_millis(30);

/// The phone page, styled like the app.
pub const PAGE: &str = include_str!("page.html");

/// Something from a phone for the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A paired phone asks for something; answer with [`Server::reply`].
    Request {
        client: u64,
        id: Value,
        method: String,
        params: Value,
    },
    /// A phone was let in: `new` when it has just been paired.
    Connected {
        client: u64,
        name: String,
        new: bool,
    },
    Disconnected {
        client: u64,
    },
}

struct Client {
    tx: Sender<Outgoing>,
    /// The paired device it logged in as, once it has.
    device: Option<String>,
}

enum Outgoing {
    Text(String),
    /// The device was revoked: say so and hang up.
    Revoked,
}

/// Finds a track's album art: given the art id the app published with the
/// track and the longest side wanted in pixels, the image as JPEG.
pub type ArtSource = Arc<dyn Fn(&str, u32) -> Option<Vec<u8>> + Send + Sync>;

/// Art sizes the page asks for: list thumbnails and the big cover.
pub const ART_SIZES: [u32; 2] = [96, 640];
/// Covers kept ready to send; the cache starts over beyond this.
const ART_CACHE: usize = 600;

struct Shared {
    pairing: pairing::Pairing,
    art: Option<ArtSource>,
    /// Paired phones put this in art addresses, so nobody else can fetch
    /// covers; new each time the remote starts.
    art_key: String,
    art_cache: HashMap<String, Arc<Vec<u8>>>,
    clients: HashMap<u64, Client>,
    /// The latest message for each topic, as sent.
    topics: BTreeMap<String, String>,
    events: Vec<Event>,
    next_client: u64,
}

/// The running remote. Dropping it stops it and disconnects every phone.
pub struct Server {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    port: u16,
    thread: Option<JoinHandle<()>>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

impl Server {
    /// Starts listening on every network, on `port` or one of the next few.
    pub fn start(port: u16, devices: Vec<Device>) -> io::Result<Server> {
        let listener = bind(port)?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let shared = Arc::new(Mutex::new(Shared {
            pairing: pairing::Pairing::new(devices),
            art: None,
            art_key: pairing::random_hex(16),
            art_cache: HashMap::new(),
            clients: HashMap::new(),
            topics: BTreeMap::new(),
            events: Vec::new(),
            next_client: 1,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let shared = shared.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("remote".into())
                .spawn(move || accept_loop(listener, shared, stop))?
        };
        Ok(Server {
            shared,
            stop,
            port,
            thread: Some(thread),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Where album art comes from.
    pub fn set_art_source(&self, source: ArtSource) {
        lock(&self.shared).art = Some(source);
    }

    /// What phones asked for since the last call.
    pub fn poll(&self) -> Vec<Event> {
        std::mem::take(&mut lock(&self.shared).events)
    }

    /// Answers a [`Event::Request`].
    pub fn reply(&self, client: u64, id: Value, result: Result<Value, String>) {
        let msg = match result {
            Ok(r) => json!({"id": id, "result": r}),
            Err(e) => json!({"id": id, "error": {"message": e}}),
        };
        if let Some(c) = lock(&self.shared).clients.get(&client) {
            let _ = c.tx.send(Outgoing::Text(msg.to_string()));
        }
    }

    /// Pushes the state of `topic` to every paired phone, if it changed.
    pub fn publish(&self, topic: &str, data: Value) {
        let msg = json!({"event": topic, "data": data}).to_string();
        let mut s = lock(&self.shared);
        if s.topics.get(topic) == Some(&msg) {
            return;
        }
        for c in s.clients.values().filter(|c| c.device.is_some()) {
            let _ = c.tx.send(Outgoing::Text(msg.clone()));
        }
        s.topics.insert(topic.to_string(), msg);
    }

    /// The QR code's one-time code and its generation (which changes with
    /// the code), renewed once it has expired or been used.
    pub fn pair_code(&self) -> (String, u64) {
        lock(&self.shared).pairing.code(Instant::now())
    }

    /// Starts a new one-time code unless the current one still works for
    /// at least a minute, for when the QR code comes into view.
    pub fn refresh_code(&self) {
        let mut s = lock(&self.shared);
        let now = Instant::now();
        if s.pairing.code_expires().saturating_duration_since(now) < Duration::from_secs(60) {
            s.pairing.renew_code(now);
        }
    }

    /// How long the current code still works.
    pub fn code_left(&self) -> Duration {
        let s = lock(&self.shared);
        s.pairing
            .code_expires()
            .saturating_duration_since(Instant::now())
    }

    pub fn pin(&self) -> String {
        lock(&self.shared).pairing.pin().to_string()
    }

    pub fn devices(&self) -> Vec<Device> {
        lock(&self.shared).pairing.devices().to_vec()
    }

    /// Device ids with a phone connected now.
    pub fn connected(&self) -> Vec<String> {
        let s = lock(&self.shared);
        let mut ids: Vec<String> = s
            .clients
            .values()
            .filter_map(|c| c.device.clone())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Forgets a device and hangs up on it.
    pub fn revoke(&self, id: &str) {
        let mut s = lock(&self.shared);
        s.pairing.revoke(id);
        for c in s.clients.values_mut() {
            if c.device.as_deref() == Some(id) {
                c.device = None;
                let _ = c.tx.send(Outgoing::Revoked);
            }
        }
    }

    /// The devices, when they changed since the last call, to save them.
    pub fn take_changed_devices(&self) -> Option<Vec<Device>> {
        let mut s = lock(&self.shared);
        s.pairing
            .take_changed()
            .then(|| s.pairing.devices().to_vec())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn bind(port: u16) -> io::Result<TcpListener> {
    let mut last = None;
    for p in port..port.saturating_add(PORT_TRIES) {
        match TcpListener::bind((Ipv4Addr::UNSPECIFIED, p)) {
            Ok(l) => return Ok(l),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::new(ErrorKind::AddrInUse, "no free port")))
}

/// This computer's addresses a phone could reach, most likely first:
/// private IPv4 addresses (home and venue networks, hotspots) before
/// others; no loopback or self-assigned ones.
pub fn local_addresses() -> Vec<(String, IpAddr)> {
    let mut found: Vec<(String, IpAddr)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified() => {
                Some((i.name.clone(), IpAddr::V4(v4)))
            }
            _ => None,
        })
        .collect();
    found.sort_by_key(|(name, ip)| {
        let private = matches!(ip, IpAddr::V4(v4) if v4.is_private());
        // Wi-Fi and Ethernet before VPNs, bridges and virtual machines.
        let usual = ["en", "eth", "wl", "Wi-Fi", "Ethernet"]
            .iter()
            .any(|p| name.starts_with(p));
        (!private, !usual)
    });
    found.dedup_by(|a, b| a.1 == b.1);
    found
}

/// The address a phone opens to pair with `code`. The code is after `#`,
/// so the browser never sends it in a request or a `Referer`.
pub fn pair_url(ip: IpAddr, port: u16, code: &str) -> String {
    format!("{}#pair={code}", base_url(ip, port))
}

pub fn base_url(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V6(v6) => format!("http://[{v6}]:{port}/"),
        IpAddr::V4(v4) => format!("http://{v4}:{port}/"),
    }
}

fn accept_loop(listener: TcpListener, shared: Arc<Mutex<Shared>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let shared = shared.clone();
                let stop = stop.clone();
                let _ = std::thread::Builder::new()
                    .name("remote client".into())
                    .spawn(move || serve(stream, shared, stop));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

fn serve(mut stream: TcpStream, shared: Arc<Mutex<Shared>>, stop: Arc<AtomicBool>) {
    // Accepted sockets keep the listener's non-blocking mode on macOS.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_nodelay(true);
    let wait = Duration::from_secs(5);
    let _ = stream.set_read_timeout(Some(wait));
    let _ = stream.set_write_timeout(Some(wait));
    let Ok(Some(head)) = http::peek_head(&stream, wait) else {
        let _ = stream.shutdown(Shutdown::Both);
        return;
    };
    if !http::request_allowed(&head) {
        let _ = http::consume(&mut stream, head.len);
        let _ = http::respond(&mut stream, "403 Forbidden", "text/plain", b"Forbidden");
        return;
    }
    let path = head.path.split('?').next().unwrap_or("");
    match (head.method.as_str(), path) {
        ("GET", "/ws") if head.is_upgrade() => {
            if lock(&shared).clients.len() >= MAX_CLIENTS {
                let _ = http::consume(&mut stream, head.len);
                let _ = http::respond(
                    &mut stream,
                    "503 Service Unavailable",
                    "text/plain",
                    b"Busy",
                );
                return;
            }
            let config = WebSocketConfig::default()
                .max_message_size(Some(MAX_MESSAGE))
                .max_frame_size(Some(MAX_MESSAGE));
            if let Ok(ws) = tungstenite::accept_with_config(stream, Some(config)) {
                websocket(ws, &shared, &stop);
            }
        }
        ("GET", p) if p.starts_with("/art/") => {
            let _ = http::consume(&mut stream, head.len);
            let id = &p["/art/".len()..];
            let size = http::query(&head.path, "s")
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|s| ART_SIZES.contains(s));
            let key = http::query(&head.path, "k").unwrap_or("");
            let image = match size {
                Some(size) if is_art_id(id) => art(&shared, id, size, key),
                _ => None,
            };
            let _ = match image {
                Some(jpeg) => http::respond_cached(
                    &mut stream,
                    "200 OK",
                    "image/jpeg",
                    &jpeg,
                    "private, max-age=86400",
                ),
                None => http::respond(&mut stream, "404 Not Found", "text/plain", b"No art"),
            };
        }
        ("GET", "/" | "/index.html") => {
            let _ = http::consume(&mut stream, head.len);
            let _ = http::respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
            );
        }
        _ => {
            let _ = http::consume(&mut stream, head.len);
            let _ = http::respond(&mut stream, "404 Not Found", "text/plain", b"Not found");
        }
    }
}

/// Art ids are short hex strings the app makes up.
fn is_art_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit())
}

/// A cover, from the cache or the app's art source, for a request that
/// carries the art key.
fn art(shared: &Mutex<Shared>, id: &str, size: u32, key: &str) -> Option<Arc<Vec<u8>>> {
    let cache_key = format!("{id}:{size}");
    let source = {
        let s = lock(shared);
        if !pairing::same(key, &s.art_key) {
            return None;
        }
        if let Some(hit) = s.art_cache.get(&cache_key) {
            return Some(hit.clone());
        }
        s.art.clone()?
    };
    // Found outside the lock: reading tags and scaling takes a moment.
    let jpeg = Arc::new(source(id, size)?);
    let mut s = lock(shared);
    if s.art_cache.len() >= ART_CACHE {
        s.art_cache.clear();
    }
    s.art_cache.insert(cache_key, jpeg.clone());
    Some(jpeg)
}

/// One phone's connection, until it closes or the server stops.
fn websocket(mut ws: WebSocket<TcpStream>, shared: &Arc<Mutex<Shared>>, stop: &AtomicBool) {
    let _ = ws.get_mut().set_read_timeout(Some(POLL));
    let (tx, rx) = mpsc::channel();
    let client = {
        let mut s = lock(shared);
        let id = s.next_client;
        s.next_client += 1;
        s.clients.insert(id, Client { tx, device: None });
        id
    };
    let mut conn = Connection {
        client,
        shared,
        authed: false,
        bad_hellos: 0,
        tokens: BURST,
        last_fill: Instant::now(),
    };
    run(&mut ws, &mut conn, &rx, stop);
    let _ = ws.close(None);
    let _ = ws.flush();
    let mut s = lock(shared);
    s.clients.remove(&client);
    if conn.authed {
        s.events.push(Event::Disconnected { client });
    }
}

struct Connection<'a> {
    client: u64,
    shared: &'a Arc<Mutex<Shared>>,
    authed: bool,
    bad_hellos: u32,
    /// Rate limit: a bucket of messages that refills at [`RATE`].
    tokens: f64,
    last_fill: Instant,
}

fn is_wait(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(io) if matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
}

fn run(
    ws: &mut WebSocket<TcpStream>,
    conn: &mut Connection,
    rx: &Receiver<Outgoing>,
    stop: &AtomicBool,
) {
    let mut last_heard = Instant::now();
    let mut last_ping = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        match ws.read() {
            Ok(Message::Text(text)) => {
                last_heard = Instant::now();
                match conn.handle(text.as_str()) {
                    Handled::Reply(r) => {
                        if ws.send(Message::text(r)).is_err() {
                            return;
                        }
                    }
                    Handled::Nothing => {}
                    Handled::Close(r) => {
                        let _ = ws.send(Message::text(r));
                        return;
                    }
                }
            }
            Ok(Message::Close(_)) => return,
            Ok(_) => last_heard = Instant::now(),
            Err(e) if is_wait(&e) => {}
            Err(_) => return,
        }
        loop {
            match rx.try_recv() {
                Ok(Outgoing::Text(t)) => {
                    if ws.send(Message::text(t)).is_err() {
                        return;
                    }
                }
                Ok(Outgoing::Revoked) => {
                    let _ = ws.send(Message::text(json!({"event": "revoked"}).to_string()));
                    return;
                }
                Err(_) => break,
            }
        }
        if last_heard.elapsed() > GONE_AFTER {
            return;
        }
        if last_ping.elapsed() > PING_EVERY {
            last_ping = Instant::now();
            if ws.send(Message::Ping(Vec::new().into())).is_err() {
                return;
            }
        }
        match ws.flush() {
            Ok(()) => {}
            Err(e) if is_wait(&e) => {}
            Err(_) => return,
        }
    }
}

enum Handled {
    Reply(String),
    Nothing,
    Close(String),
}

fn error(id: &Value, message: &str) -> String {
    json!({"id": id, "error": {"message": message}}).to_string()
}

impl Connection<'_> {
    fn handle(&mut self, text: &str) -> Handled {
        let now = Instant::now();
        self.tokens =
            (self.tokens + now.duration_since(self.last_fill).as_secs_f64() * RATE).min(BURST);
        self.last_fill = now;
        let Ok(msg) = serde_json::from_str::<Value>(text) else {
            return Handled::Reply(error(&Value::Null, "Not JSON"));
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        if self.tokens < 1.0 {
            return Handled::Reply(error(&id, "Too many messages"));
        }
        self.tokens -= 1.0;
        let method = msg["method"].as_str().unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "hello" => self.hello(id, &params),
            "pair" => self.pair(id, &params),
            "ping" => Handled::Reply(json!({"id": id, "result": "pong"}).to_string()),
            _ if !self.authed => Handled::Reply(error(&id, "Not paired")),
            _ => {
                let mut s = lock(self.shared);
                // Revoked while connected.
                if !s
                    .clients
                    .get(&self.client)
                    .is_some_and(|c| c.device.is_some())
                {
                    return Handled::Close(json!({"event": "revoked"}).to_string());
                }
                s.events.push(Event::Request {
                    client: self.client,
                    id,
                    method: method.to_string(),
                    params,
                });
                Handled::Nothing
            }
        }
    }

    fn hello(&mut self, id: Value, params: &Value) -> Handled {
        let device = params["device"].as_str().unwrap_or("");
        let token = params["token"].as_str().unwrap_or("");
        let mut s = lock(self.shared);
        let Some(d) = s.pairing.check(device, token) else {
            drop(s);
            self.bad_hellos += 1;
            let reply = json!({"id": id, "error": {"message": "This phone isn't paired any more. Scan the QR code in DJ Sugar again.", "unpaired": true}}).to_string();
            return if self.bad_hellos >= MAX_BAD_HELLOS {
                Handled::Close(reply)
            } else {
                Handled::Reply(reply)
            };
        };
        self.let_in(&mut s, &d, false);
        let art_key = s.art_key.clone();
        Handled::Reply(
            json!({"id": id, "result": {"device": d.id, "name": d.name, "art_key": art_key}})
                .to_string(),
        )
    }

    fn pair(&mut self, id: Value, params: &Value) -> Handled {
        let name = params["name"].as_str().unwrap_or("Phone");
        let now = Instant::now();
        let mut s = lock(self.shared);
        let result = if let Some(code) = params["code"].as_str() {
            s.pairing.pair_with_code(code, name, now)
        } else if let Some(pin) = params["pin"].as_str() {
            s.pairing.pair_with_pin(pin, name, now)
        } else {
            Err(PairError::Wrong)
        };
        match result {
            Ok((d, token)) => {
                self.let_in(&mut s, &d, true);
                Handled::Reply(
                    json!({"id": id, "result": {"device": d.id, "name": d.name, "token": token, "art_key": s.art_key}})
                        .to_string(),
                )
            }
            Err(e) => Handled::Reply(error(&id, &e.message())),
        }
    }

    /// Lets the phone in as device `d`, and queues the latest of every
    /// topic for it (sent right after the answer).
    fn let_in(&mut self, s: &mut Shared, d: &Device, new: bool) {
        self.authed = true;
        let topics: Vec<String> = s.topics.values().cloned().collect();
        if let Some(c) = s.clients.get_mut(&self.client) {
            c.device = Some(d.id.clone());
            for t in topics {
                let _ = c.tx.send(Outgoing::Text(t));
            }
        }
        s.events.push(Event::Connected {
            client: self.client,
            name: d.name.clone(),
            new,
        });
    }
}

#[cfg(test)]
mod tests;
