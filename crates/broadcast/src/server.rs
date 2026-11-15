//! Talking to the servers: the source connection that carries the stream,
//! now-playing updates, and listener counts.
//!
//! - Icecast 2: `PUT /mount` with HTTP basic auth (what libshout does since
//!   Icecast 2.4), then the stream as the request body.
//! - Shoutcast 1 (and Shoutcast 2 in its v1 mode): the password on the port
//!   after the stream's, an `OK2` back, `icy-` headers, then the stream.
//! - Now-playing: Icecast's `/admin/metadata?mode=updinfo` and Shoutcast's
//!   `/admin.cgi?mode=updinfo`, for MP3 streams; Ogg streams carry it inside
//!   (see `encode.rs`).
//! - Listeners: Icecast's `/status-json.xsl`.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::{ServerKind, Station};

/// How long connecting and the server's answer may take.
const TIMEOUT: Duration = Duration::from_secs(10);
/// How long a write may block before the station counts as stalled.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Why a station couldn't connect, in words for the DJ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// Worth trying again: the network, the server being down.
    Retry(String),
    /// Not worth trying again until the settings change.
    Fatal(String),
}

impl ConnectError {
    pub fn message(&self) -> &str {
        match self {
            ConnectError::Retry(m) | ConnectError::Fatal(m) => m,
        }
    }
}

/// A plain or TLS connection.
pub trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

fn tls_config() -> Arc<rustls::ClientConfig> {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth(),
    )
}

/// Opens a TCP connection, wrapped in TLS when `tls`.
fn open(host: &str, port: u16, tls: bool) -> Result<Box<dyn Stream>, ConnectError> {
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|_| ConnectError::Retry(format!("Couldn't find the server {host}")))?
        .collect();
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, TIMEOUT) {
            Ok(tcp) => {
                let _ = tcp.set_nodelay(true);
                let _ = tcp.set_read_timeout(Some(TIMEOUT));
                let _ = tcp.set_write_timeout(Some(WRITE_TIMEOUT));
                if !tls {
                    return Ok(Box::new(tcp));
                }
                let name =
                    rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|_| {
                        ConnectError::Fatal(format!("{host} isn't a valid server name"))
                    })?;
                let conn = rustls::ClientConnection::new(tls_config(), name)
                    .map_err(|e| ConnectError::Fatal(e.to_string()))?;
                return Ok(Box::new(rustls::StreamOwned::new(conn, tcp)));
            }
            Err(e) => last = Some(e),
        }
    }
    Err(ConnectError::Retry(match last {
        Some(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            format!("{host} isn't accepting connections on port {port}")
        }
        _ => format!("Couldn't reach {host}:{port}"),
    }))
}

fn basic_auth(user: &str, password: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
}

/// Reads an HTTP status line and headers; returns the status code.
fn read_status(r: &mut impl BufRead) -> io::Result<(u16, String)> {
    let mut line = String::new();
    r.read_line(&mut line)?;
    let code = line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    // The headers, up to the blank line.
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 || h.trim().is_empty() {
            break;
        }
    }
    Ok((code, line.trim().to_string()))
}

fn mount(station: &Station) -> String {
    let m = station.mount.trim();
    if m.starts_with('/') {
        m.to_string()
    } else {
        format!("/{m}")
    }
}

/// Connects as the source for `station` and returns the connection, ready
/// for the stream's bytes.
pub fn connect(
    station: &Station,
    password: &str,
    content_type: &str,
) -> Result<Box<dyn Stream>, ConnectError> {
    match station.server {
        ServerKind::Icecast => connect_icecast(station, password, content_type),
        ServerKind::Shoutcast => connect_shoutcast(station, password, content_type),
    }
}

fn ice_headers(station: &Station, prefix: &str) -> String {
    let mut h = String::new();
    let mut add = |name: &str, value: &str| {
        if !value.trim().is_empty() {
            h.push_str(&format!(
                "{prefix}{name}: {}\r\n",
                value.replace(['\r', '\n'], " ")
            ));
        }
    };
    add("name", &station.stream_name);
    add("description", &station.description);
    add("genre", &station.genre);
    add("url", &station.website);
    add("pub", if station.public { "1" } else { "0" });
    if prefix == "ice-" {
        h.push_str(&format!(
            "ice-audio-info: bitrate={};channels=2\r\n",
            station.bitrate
        ));
    } else {
        h.push_str(&format!("icy-br: {}\r\n", station.bitrate));
    }
    h
}

fn connect_icecast(
    station: &Station,
    password: &str,
    content_type: &str,
) -> Result<Box<dyn Stream>, ConnectError> {
    let mut s = open(&station.host, station.port, station.tls)?;
    let user = if station.user.trim().is_empty() {
        "source"
    } else {
        station.user.trim()
    };
    let request = format!(
        "PUT {} HTTP/1.1\r\nHost: {}:{}\r\nAuthorization: Basic {}\r\nUser-Agent: DJSugar/{}\r\n\
         Content-Type: {}\r\n{}Expect: 100-continue\r\n\r\n",
        mount(station),
        station.host,
        station.port,
        basic_auth(user, password),
        env!("CARGO_PKG_VERSION"),
        content_type,
        ice_headers(station, "ice-"),
    );
    let net = |e: io::Error| ConnectError::Retry(format!("The connection dropped: {e}"));
    s.write_all(request.as_bytes()).map_err(net)?;
    s.flush().map_err(net)?;
    let mut r = BufReader::new(&mut s);
    let (code, line) = read_status(&mut r).map_err(net)?;
    drop(r);
    match code {
        100 | 200 => Ok(s),
        401 => Err(ConnectError::Fatal(
            "The server turned down the user name or password".into(),
        )),
        403 => Err(ConnectError::Retry(format!(
            "The mount point {} is in use or not allowed",
            mount(station)
        ))),
        0 => Err(ConnectError::Retry(
            "The server didn't answer like an Icecast server".into(),
        )),
        _ => Err(ConnectError::Retry(format!("The server said: {line}"))),
    }
}

fn connect_shoutcast(
    station: &Station,
    password: &str,
    content_type: &str,
) -> Result<Box<dyn Stream>, ConnectError> {
    // Shoutcast 1 takes sources on the port after the listeners'.
    let mut s = open(&station.host, station.port.saturating_add(1), station.tls)?;
    let net = |e: io::Error| ConnectError::Retry(format!("The connection dropped: {e}"));
    s.write_all(format!("{password}\r\n").as_bytes())
        .map_err(net)?;
    s.flush().map_err(net)?;
    let mut line = String::new();
    {
        let mut r = BufReader::new(&mut s);
        r.read_line(&mut line).map_err(net)?;
        // Shoutcast 2 adds "icy-caps" lines; read up to the blank line.
        if line.starts_with("OK") {
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).map_err(net)? == 0 || h.trim().is_empty() {
                    break;
                }
            }
        }
    }
    if !line.starts_with("OK") {
        return Err(if line.to_ascii_lowercase().contains("password") {
            ConnectError::Fatal("The server turned down the password".into())
        } else {
            ConnectError::Retry(format!("The server said: {}", line.trim()))
        });
    }
    let headers = format!(
        "content-type: {content_type}\r\n{}\r\n",
        ice_headers(station, "icy-")
    );
    s.write_all(headers.as_bytes()).map_err(net)?;
    s.flush().map_err(net)?;
    Ok(s)
}

/// Sends a simple GET and returns the status and body.
fn get(
    host: &str,
    port: u16,
    tls: bool,
    path: &str,
    auth: Option<&str>,
) -> Result<(u16, String), String> {
    let mut s = open(host, port, tls).map_err(|e| e.message().to_string())?;
    let auth = auth
        .map(|a| format!("Authorization: Basic {a}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "GET {path} HTTP/1.0\r\nHost: {host}:{port}\r\nUser-Agent: DJSugar/{}\r\n{auth}\r\n",
        env!("CARGO_PKG_VERSION")
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let (code, _) = read_status(&mut r).map_err(|e| e.to_string())?;
    let mut body = String::new();
    let _ = r.take(1 << 20).read_to_string(&mut body);
    Ok((code, body))
}

/// Tells the server what's playing, for formats that don't carry it.
pub fn update_title(station: &Station, password: &str, text: &str) -> Result<(), String> {
    let song = utf8_percent_encode(text, NON_ALPHANUMERIC).to_string();
    let (code, _) = match station.server {
        ServerKind::Icecast => {
            let user = if station.user.trim().is_empty() {
                "source"
            } else {
                station.user.trim()
            };
            let m = utf8_percent_encode(&mount(station), NON_ALPHANUMERIC).to_string();
            get(
                &station.host,
                station.port,
                station.tls,
                &format!("/admin/metadata?mount={m}&mode=updinfo&song={song}&charset=UTF-8"),
                Some(&basic_auth(user, password)),
            )?
        }
        ServerKind::Shoutcast => {
            let pass = utf8_percent_encode(password, NON_ALPHANUMERIC).to_string();
            get(
                &station.host,
                station.port,
                station.tls,
                &format!("/admin.cgi?pass={pass}&mode=updinfo&song={song}"),
                None,
            )?
        }
    };
    if code == 200 {
        Ok(())
    } else {
        Err(format!("the server answered {code}"))
    }
}

/// Listeners on the station's mount, from Icecast's status page.
pub fn listeners(station: &Station) -> Option<u32> {
    if station.server != ServerKind::Icecast {
        return None;
    }
    let (code, body) = get(
        &station.host,
        station.port,
        station.tls,
        "/status-json.xsl",
        None,
    )
    .ok()?;
    if code != 200 {
        return None;
    }
    listeners_in(&body, &mount(station))
}

/// Finds `mount`'s listener count in Icecast's status JSON, where
/// `icestats.source` is one object or a list of them.
pub(crate) fn listeners_in(json: &str, mount: &str) -> Option<u32> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let sources = &v["icestats"]["source"];
    let list: Vec<&serde_json::Value> = match sources {
        serde_json::Value::Array(a) => a.iter().collect(),
        serde_json::Value::Object(_) => vec![sources],
        _ => return None,
    };
    list.into_iter()
        .find(|s| s["listenurl"].as_str().is_some_and(|u| u.ends_with(mount)))
        .and_then(|s| s["listeners"].as_u64())
        .map(|n| n as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn station(port: u16) -> Station {
        Station {
            host: "127.0.0.1".into(),
            port,
            mount: "live".into(),
            stream_name: "Test FM".into(),
            ..Station::default()
        }
    }

    /// A one-shot server: answers the first request with `reply` and hands
    /// back what it read.
    fn server(reply: &'static str) -> (u16, std::thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = s.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
                if n == 0 || got.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            s.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&got).into_owned()
        });
        (port, h)
    }

    #[test]
    fn icecast_source_request_is_a_put_with_auth_and_headers() {
        let (port, h) = server("HTTP/1.1 100 Continue\r\n\r\n");
        let s = connect(&station(port), "hackme", "audio/ogg");
        assert!(s.is_ok());
        let req = h.join().unwrap();
        assert!(req.starts_with("PUT /live HTTP/1.1\r\n"), "{req}");
        // source:hackme
        assert!(req.contains("Authorization: Basic c291cmNlOmhhY2ttZQ==\r\n"));
        assert!(req.contains("Content-Type: audio/ogg\r\n"));
        assert!(req.contains("ice-name: Test FM\r\n"));
        assert!(req.contains("ice-pub: 0\r\n"));
    }

    #[test]
    fn a_wrong_password_is_not_retried() {
        let (port, _h) = server("HTTP/1.1 401 Authentication Required\r\n\r\n");
        let e = connect(&station(port), "nope", "audio/ogg").err().unwrap();
        assert!(matches!(e, ConnectError::Fatal(_)), "{e:?}");
        assert_eq!(
            e.message(),
            "The server turned down the user name or password"
        );
    }

    #[test]
    fn no_server_is_retried() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let e = connect(&station(port), "x", "audio/ogg").err().unwrap();
        assert!(matches!(e, ConnectError::Retry(_)), "{e:?}");
    }

    #[test]
    fn finds_the_listeners_of_the_mount() {
        let one = r#"{"icestats":{"source":{"listenurl":"http://radio:8000/live","listeners":7}}}"#;
        assert_eq!(listeners_in(one, "/live"), Some(7));
        let many = r#"{"icestats":{"source":[
            {"listenurl":"http://radio:8000/other","listeners":3},
            {"listenurl":"http://radio:8000/live","listeners":12}]}}"#;
        assert_eq!(listeners_in(many, "/live"), Some(12));
        assert_eq!(listeners_in(many, "/gone"), None);
    }
}
