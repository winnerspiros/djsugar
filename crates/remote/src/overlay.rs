//! The now-playing overlay for streaming software (OBS, Streamlabs): a page
//! with a clear background at `/now-playing`, the track as JSON at
//! `/now-playing.json` and its cover at `/now-playing/cover`.
//!
//! The same server shows the karaoke page at `/karaoke`, the words of the
//! track playing for a TV or another screen: what is sung now at
//! `/karaoke.json` and the lyrics, with each word's time, at
//! `/karaoke/lines.json`. The page keeps time itself between requests.
//!
//! It is read-only and needs no pairing. It listens on this computer only
//! (127.0.0.1) unless the DJ allows other devices on the network. The page
//! asks for the JSON once a second, so nothing has to be pushed.

use std::io::{self, ErrorKind};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::http;

/// The port tried first; the next few are tried if it's taken.
pub const DEFAULT_PORT: u16 = 7487;
const PORT_TRIES: u16 = 10;

/// The overlay page; `__STYLE__` in it is replaced by the style's name.
pub const PAGE: &str = include_str!("overlay.html");

/// The karaoke page.
pub const KARAOKE_PAGE: &str = include_str!("karaoke.html");

/// What the karaoke page shows, set with [`set_karaoke`]: the moment's
/// JSON and the lyrics' JSON. Kept apart from any one server, so it
/// survives the overlay being restarted.
static KARAOKE: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));

/// What the karaoke page shows: `now`, the track and where it is, often;
/// `lines`, the lyrics with their word times, when they change.
pub fn set_karaoke(now: String, lines: Option<String>) {
    let mut k = KARAOKE.lock().unwrap_or_else(|e| e.into_inner());
    k.0 = now;
    if let Some(lines) = lines {
        k.1 = lines;
    }
}

fn karaoke_json(lines: bool) -> String {
    let k = KARAOKE.lock().unwrap_or_else(|e| e.into_inner());
    let json = if lines { &k.1 } else { &k.0 };
    if json.is_empty() {
        "{}".into()
    } else {
        json.clone()
    }
}

/// How the overlay looks.
pub const STYLES: [&str; 3] = ["lower-third", "corner", "ticker"];

#[derive(Default)]
struct Shown {
    json: String,
    cover: Option<Arc<Vec<u8>>>,
    style: usize,
}

/// The running overlay server. Dropping it stops it.
pub struct Overlay {
    shown: Arc<Mutex<Shown>>,
    stop: Arc<AtomicBool>,
    port: u16,
    network: bool,
    thread: Option<JoinHandle<()>>,
}

impl Overlay {
    /// Listens on `port` (or the next free one): on this computer only, or
    /// on every network when `network` is true.
    pub fn start(port: u16, network: bool) -> io::Result<Overlay> {
        let ip = if network {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        let mut found = None;
        let mut last = None;
        for p in port..port.saturating_add(PORT_TRIES) {
            match TcpListener::bind((ip, p)) {
                Ok(l) => {
                    found = Some(l);
                    break;
                }
                Err(e) => last = Some(e),
            }
        }
        let listener = found.ok_or_else(|| {
            last.unwrap_or_else(|| io::Error::new(ErrorKind::AddrInUse, "no free port"))
        })?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let shown = Arc::new(Mutex::new(Shown {
            json: "{}".into(),
            ..Default::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let shown = shown.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("now-playing overlay".into())
                .spawn(move || accept_loop(listener, shown, stop))?
        };
        Ok(Overlay {
            shown,
            stop,
            port,
            network,
            thread: Some(thread),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Whether other devices on the network may open it.
    pub fn network(&self) -> bool {
        self.network
    }

    /// The overlay's address on this computer.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/now-playing", self.port)
    }

    /// What the page shows: the JSON, the cover (JPEG or PNG) and the style
    /// (an index into [`STYLES`]).
    pub fn set(&self, json: String, cover: Option<Arc<Vec<u8>>>, style: usize) {
        let mut s = self.shown.lock().unwrap();
        s.json = json;
        s.cover = cover;
        s.style = style.min(STYLES.len() - 1);
    }

    pub fn set_style(&self, style: usize) {
        self.shown.lock().unwrap().style = style.min(STYLES.len() - 1);
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn accept_loop(listener: TcpListener, shown: Arc<Mutex<Shown>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let shown = shown.clone();
                let _ = std::thread::Builder::new()
                    .name("overlay request".into())
                    .spawn(move || serve(stream, &shown));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

fn serve(mut stream: TcpStream, shown: &Mutex<Shown>) {
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let wait = Duration::from_secs(5);
    let _ = stream.set_read_timeout(Some(wait));
    let _ = stream.set_write_timeout(Some(wait));
    let Ok(Some(head)) = http::peek_head(&stream, wait) else {
        let _ = stream.shutdown(Shutdown::Both);
        return;
    };
    let _ = http::consume(&mut stream, head.len);
    if !http::request_allowed(&head) {
        let _ = http::respond(&mut stream, "403 Forbidden", "text/plain", b"Forbidden");
        return;
    }
    if head.method != "GET" {
        let _ = http::respond(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"Read only",
        );
        return;
    }
    let path = head.path.split('?').next().unwrap_or("");
    let _ = match path {
        "/now-playing" | "/now-playing/" => {
            let style = STYLES[shown.lock().unwrap().style];
            let page = PAGE.replace("__STYLE__", style);
            http::respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                page.as_bytes(),
            )
        }
        "/now-playing.json" => {
            let json = shown.lock().unwrap().json.clone();
            http::respond(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                json.as_bytes(),
            )
        }
        "/now-playing/cover" => {
            let cover = shown.lock().unwrap().cover.clone();
            match cover {
                Some(bytes) => {
                    let kind = if bytes.starts_with(b"\x89PNG") {
                        "image/png"
                    } else {
                        "image/jpeg"
                    };
                    http::respond(&mut stream, "200 OK", kind, &bytes)
                }
                None => http::respond(&mut stream, "404 Not Found", "text/plain", b"No cover"),
            }
        }
        "/karaoke" | "/karaoke/" => http::respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            KARAOKE_PAGE.as_bytes(),
        ),
        "/karaoke.json" | "/karaoke/lines.json" => http::respond(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            karaoke_json(path == "/karaoke/lines.json").as_bytes(),
        ),
        _ => http::respond(&mut stream, "404 Not Found", "text/plain", b"Not found"),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn get(port: u16, path: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            s,
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn serves_the_page_and_the_track() {
        let o = Overlay::start(DEFAULT_PORT + 50, false).unwrap();
        o.set(r#"{"title":"Tune"}"#.into(), None, 1);
        let page = get(o.port(), "/now-playing");
        assert!(page.starts_with("HTTP/1.1 200"));
        assert!(page.contains("corner"));
        let json = get(o.port(), "/now-playing.json");
        assert!(json.ends_with(r#"{"title":"Tune"}"#), "{json}");
        assert!(get(o.port(), "/now-playing/cover").starts_with("HTTP/1.1 404"));
        assert!(get(o.port(), "/ws").starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn serves_the_karaoke_page() {
        let o = Overlay::start(DEFAULT_PORT + 60, false).unwrap();
        assert!(get(o.port(), "/karaoke").contains("DJ Sugar Karaoke"));
        set_karaoke(r#"{"id":1}"#.into(), Some(r#"{"id":1,"lines":[]}"#.into()));
        assert!(get(o.port(), "/karaoke.json").ends_with(r#"{"id":1}"#));
        set_karaoke(r#"{"id":2}"#.into(), None);
        assert!(get(o.port(), "/karaoke/lines.json").ends_with(r#"{"id":1,"lines":[]}"#));
    }
}
