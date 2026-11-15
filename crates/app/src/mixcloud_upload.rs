//! Signing in to Mixcloud and uploading a show.
//!
//! Mixcloud's OAuth needs the app's client secret to swap the sign-in code
//! for a token and has no desktop flow, so the swap happens on github.com/winnerspiros/djsugar
//! (`website/functions/mixcloud`). DJ Sugar listens on 127.0.0.1, opens
//! `https://github.com/winnerspiros/djsugarmixcloud/login?port=…&state=…` in the browser, the DJ
//! signs in on mixcloud.com, and the token comes back to the listener with
//! the same state. It is kept in the system keychain; Mixcloud tokens don't
//! expire, but the DJ can revoke one, which shows up as an OAuth error.
//!
//! The keychain is only read when an upload needs the token, and once per
//! run: on macOS each read can ask for the login keychain password (always
//! for a build whose code signature the item doesn't know yet). Whether the
//! DJ is signed in, which the menus need at launch, comes from a marker
//! file next to the settings instead, which holds no secret.
//!
//! The upload is one multipart POST to `https://api.mixcloud.com/upload/`,
//! the MP3 streamed from disk.

use std::fs::File;
use std::io::{self, Cursor, Read};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use djsugar_broadcast::mixcloud::Show;
use serde_json::Value;

use crate::spotify::{
    decode, encode, open_in_browser, random_token, read_request_line, respond, LOGIN_TIMEOUT,
};

const SIGN_IN: &str = "https://github.com/winnerspiros/djsugarmixcloud/login";
const UPLOAD: &str = "https://api.mixcloud.com/upload/";
const KEYCHAIN_SERVICE: &str = "DJ Sugar Mixcloud";
const KEYCHAIN_USER: &str = "token";

fn keychain() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(|e| e.to_string())
}

/// The token as read from (or written to) the keychain this run: `None`
/// until the first read, so the keychain is asked at most once.
static TOKEN: Mutex<Option<Option<String>>> = Mutex::new(None);

/// The marker saying a token is in the keychain.
fn marker() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("mixcloud-signed-in"))
}

fn set_marker(on: bool) {
    let Some(path) = marker() else { return };
    if on {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, b"");
    } else {
        let _ = std::fs::remove_file(&path);
    }
}

/// Whether the DJ signed in to Mixcloud, without touching the keychain.
pub fn signed_in() -> bool {
    if let Some(known) = &*TOKEN.lock().unwrap_or_else(|e| e.into_inner()) {
        return known.is_some();
    }
    marker().is_some_and(|p| p.exists())
}

/// The token from the last sign-in, if any. Reads the keychain the first
/// time only; call it off the UI thread, since the read can wait on the
/// DJ typing their password.
pub fn saved_token() -> Option<String> {
    if let Some(known) = &*TOKEN.lock().unwrap_or_else(|e| e.into_inner()) {
        return known.clone();
    }
    // Not holding the lock while the keychain may be waiting on the DJ.
    let token = keychain()
        .ok()
        .and_then(|k| k.get_password().ok())
        .filter(|t| !t.is_empty());
    let mut cached = TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(known) = &*cached {
        // Signed in or out meanwhile.
        return known.clone();
    }
    // Signed in before the marker existed, or the item was deleted.
    set_marker(token.is_some());
    *cached = Some(token.clone());
    token
}

fn save_token(token: &str) {
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Some(token.to_string()));
    set_marker(true);
    if let Err(e) = keychain().and_then(|k| k.set_password(token).map_err(|e| e.to_string())) {
        eprintln!("can't keep the Mixcloud sign-in: {e}");
    }
}

/// Signs out: forgets the token.
pub fn forget_token() {
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(None);
    set_marker(false);
    if let Ok(k) = keychain() {
        let _ = k.delete_credential();
    }
}

/// Opens the browser to sign in and waits for the token (blocking, up to
/// five minutes). Keeps it in the keychain.
pub fn sign_in() -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let state = random_token(24);
    let url = format!("{SIGN_IN}?port={port}&state={state}");
    if !open_in_browser(&url) {
        return Err(format!("Couldn't open the browser. Go to {url}"));
    }
    let token = wait_for_token(&listener, &state)?;
    save_token(&token);
    Ok(token)
}

/// The query of "GET /mixcloud?a=b HTTP/1.1", or `None` for another path.
fn mixcloud_query(request_line: &str) -> Option<Vec<(String, String)>> {
    let target = request_line.split_whitespace().nth(1)?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/mixcloud" {
        return None;
    }
    Some(
        query
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                (decode(k), decode(v))
            })
            .collect(),
    )
}

fn wait_for_token(listener: &TcpListener, state: &str) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + LOGIN_TIMEOUT;
    while Instant::now() < deadline {
        let mut stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let line = read_request_line(&mut stream).unwrap_or_default();
        let Some(query) = mixcloud_query(&line) else {
            respond(&mut stream, "404 Not Found", "");
            continue;
        };
        let param = |k: &str| query.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        // Anything without this sign-in's state (another page poking the
        // port) is turned away, so nobody can slip in their own account.
        if param("state").as_deref() != Some(state) {
            respond(
                &mut stream,
                "400 Bad Request",
                "This sign-in link is out of date. Try again from DJ Sugar.",
            );
            continue;
        }
        if let Some(token) = param("token").filter(|t| !t.is_empty()) {
            respond(
                &mut stream,
                "200 OK",
                "Signed in to Mixcloud. You can close this tab and go back to DJ Sugar.",
            );
            return Ok(token);
        }
        let error = param("error").unwrap_or_default();
        respond(
            &mut stream,
            "200 OK",
            "Mixcloud sign-in didn't finish. You can close this tab.",
        );
        return Err(if error == "access_denied" || error == "cancelled" {
            "Mixcloud sign-in was cancelled.".into()
        } else {
            format!("Mixcloud sign-in failed: {error}")
        });
    }
    Err("Mixcloud sign-in timed out.".into())
}

/// Why an upload failed.
#[derive(Debug, PartialEq)]
pub enum UploadError {
    /// The token was revoked or is wrong: sign in again.
    SignedOut,
    Other(String),
}

/// A multipart body: the text fields and the file's header before it, the
/// closing boundary after.
fn multipart(fields: &[(String, String)], file_name: &str, boundary: &str) -> (Vec<u8>, Vec<u8>) {
    let mut head = Vec::new();
    for (k, v) in fields {
        head.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
                .as_bytes(),
        );
    }
    let name = file_name.replace(['"', '\r', '\n'], "_");
    head.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"mp3\"; filename=\"{name}\"\r\n\
             Content-Type: audio/mpeg\r\n\r\n"
        )
        .as_bytes(),
    );
    let tail = format!("\r\n--{boundary}--\r\n").into_bytes();
    (head, tail)
}

/// Counts what has been read, for the progress.
struct Counted<R, F: FnMut(u64)> {
    inner: R,
    read: u64,
    each: F,
}

impl<R: Read, F: FnMut(u64)> Read for Counted<R, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        (self.each)(self.read);
        Ok(n)
    }
}

/// What Mixcloud answered: the show's address, or why not.
fn answer(status: u16, body: &str) -> Result<String, UploadError> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if let Some(key) = v["result"]["key"].as_str() {
        if v["result"]["success"].as_bool() != Some(false) {
            return Ok(format!("https://www.mixcloud.com{key}"));
        }
    }
    let error = &v["error"];
    if error["type"].as_str() == Some("OAuthException") || status == 401 {
        return Err(UploadError::SignedOut);
    }
    let message = error["message"]
        .as_str()
        .or(v["result"]["message"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Mixcloud answered {status}"));
    // Field errors: {"details": {"name": ["…"]}}.
    let details: Vec<String> = error["details"]
        .as_object()
        .map(|d| {
            d.iter()
                .flat_map(|(k, v)| {
                    v.as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|m| m.as_str())
                        .map(move |m| format!("{k}: {m}"))
                })
                .collect()
        })
        .unwrap_or_default();
    Err(UploadError::Other(if details.is_empty() {
        message
    } else {
        format!("{message} ({})", details.join("; "))
    }))
}

/// Uploads `mp3` as `show`, calling `progress` with the share sent (0..1).
/// Returns the show's address on mixcloud.com.
pub fn upload(
    token: &str,
    show: &Show,
    mp3: &Path,
    progress: impl FnMut(f64),
) -> Result<String, UploadError> {
    upload_to(UPLOAD, token, show, mp3, progress)
}

fn upload_to(
    endpoint: &str,
    token: &str,
    show: &Show,
    mp3: &Path,
    mut progress: impl FnMut(f64),
) -> Result<String, UploadError> {
    let other = |e: String| UploadError::Other(e);
    let file = File::open(mp3).map_err(|e| other(format!("{}: {e}", mp3.display())))?;
    let size = file.metadata().map_err(|e| other(e.to_string()))?.len();
    let problems = show.problems(size);
    if !problems.is_empty() {
        return Err(other(problems.join(" ")));
    }
    let boundary = format!("DJSugar{}", random_token(12));
    let name = mp3
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let (head, tail) = multipart(&show.upload_fields(), &name, &boundary);
    let total = head.len() as u64 + size + tail.len() as u64;
    let mut body = Counted {
        inner: Cursor::new(head).chain(file).chain(Cursor::new(tail)),
        read: 0,
        each: move |n| progress(n as f64 / total as f64),
    };
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(30)))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let url = format!("{endpoint}?access_token={}", encode(token));
    let mut res = agent
        .post(&url)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("Content-Length", total.to_string())
        .send(ureq::SendBody::from_reader(&mut body))
        .map_err(|e| other(e.to_string()))?;
    let status = res.status().as_u16();
    let text = res
        .body_mut()
        .read_to_string()
        .map_err(|e| other(e.to_string()))?;
    answer(status, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_wraps_fields_and_file() {
        let fields = vec![
            ("name".to_string(), "Friday".to_string()),
            ("sections-0-start_time".to_string(), "0".to_string()),
        ];
        let (head, tail) = multipart(&fields, "a\"b.mp3", "XX");
        let head = String::from_utf8(head).unwrap();
        assert!(head.starts_with(
            "--XX\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nFriday\r\n"
        ));
        assert!(
            head.contains("name=\"mp3\"; filename=\"a_b.mp3\"\r\nContent-Type: audio/mpeg\r\n\r\n")
        );
        assert_eq!(tail, b"\r\n--XX--\r\n");
    }

    #[test]
    fn reads_mixclouds_answers() {
        assert_eq!(
            answer(
                200,
                r#"{"result": {"success": true, "key": "/dj/friday/", "message": "Uploaded"}}"#
            ),
            Ok("https://www.mixcloud.com/dj/friday/".into())
        );
        assert_eq!(
            answer(
                403,
                r#"{"error": {"type": "OAuthException", "message": "An invalid access token was provided."}}"#
            ),
            Err(UploadError::SignedOut)
        );
        assert_eq!(
            answer(
                400,
                r#"{"error": {"type": "UploadLimitException", "message": "Bad upload", "details": {"name": ["Too long"]}}}"#
            ),
            Err(UploadError::Other("Bad upload (name: Too long)".into()))
        );
    }

    #[test]
    fn only_the_mixcloud_path_is_read() {
        let q = mixcloud_query("GET /mixcloud?state=abc&token=a%2Bb HTTP/1.1").unwrap();
        assert_eq!(q[1], ("token".to_string(), "a+b".to_string()));
        assert!(mixcloud_query("GET /callback?code=x HTTP/1.1").is_none());
    }

    /// The whole request reaches the server: fields, the file and the
    /// closing boundary, with the length it says.
    #[test]
    fn streams_the_file_to_the_server() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("djsugar-mc-up-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mp3 = dir.join("mix.mp3");
        let audio: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&mp3, &audio).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 65536];
            let mut want = None;
            loop {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
                if want.is_none() {
                    if let Some(end) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&got[..end]).to_lowercase();
                        let len: usize = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        want = Some(end + 4 + len);
                    }
                }
                if want.is_some_and(|w| got.len() >= w) {
                    break;
                }
            }
            let body = r#"{"result": {"success": true, "key": "/dj/mix/"}}"#;
            write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            got
        });
        let show = Show {
            name: "Mix".into(),
            ..Default::default()
        };
        let mut last = 0.0;
        let url = upload_to(
            &format!("http://127.0.0.1:{port}/upload/"),
            "t+k",
            &show,
            &mp3,
            |p| last = p,
        )
        .unwrap();
        assert_eq!(url, "https://www.mixcloud.com/dj/mix/");
        assert!((last - 1.0).abs() < 1e-9);
        let got = server.join().unwrap();
        let text = String::from_utf8_lossy(&got);
        assert!(text.starts_with("POST /upload/?access_token=t%2Bk HTTP/1.1"));
        assert!(text.contains("name=\"name\"\r\n\r\nMix\r\n"));
        // The file arrives whole.
        assert!(got.windows(audio.len()).any(|w| w == &audio[..]));
        assert!(text.trim_end().ends_with("--"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
