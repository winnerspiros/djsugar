//! Spotify for the DJ Sugar decks, on librespot (Spotify's own client
//! protocol, no developer app or client id to register):
//! - login on Spotify's web login page (Wi-Fi or mobile data)
//! - the account is kept as reusable credentials in the cache directory
//! - browsing (Liked Songs, playlists, search) through Spotify's client
//!   endpoints on the session (browse.rs), not the rate-limited Web API
//! - tracks: the encrypted Ogg Vorbis file is fetched, decrypted and written
//!   as a plain .ogg that Mixxx loads like any local file (Spotify Premium)

mod browse;

use std::ffi::{c_char, c_void, CStr, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use futures_util::stream::{FuturesUnordered, StreamExt};
use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::SessionConfig;
use librespot_core::http_client::HttpClient;
use librespot_core::session::Session;
use librespot_core::spotify_id::SpotifyId;
use librespot_core::spotify_uri::SpotifyUri;
use librespot_metadata::audio::{AudioFileFormat, AudioFiles, AudioItem};
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::sync::watch;

/// Spotify's Ogg files start with a custom packet (normalisation data) the
/// Vorbis decoders don't know; the playable stream begins after it
const SPOTIFY_OGG_HEADER_END: u64 = 0xa7;

/// Best first. Ogg only: Mixxx decodes it everywhere (MP3 files are
/// encrypted differently and FLAC is not served to third-party clients)
const FORMATS: [AudioFileFormat; 3] = [
    AudioFileFormat::OGG_VORBIS_320,
    AudioFileFormat::OGG_VORBIS_160,
    AudioFileFormat::OGG_VORBIS_96,
];

/// The client id librespot logs in with on the web (Spotify's own; its
/// login page takes a loopback redirect)
const WEB_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
const WEB_PORT: u16 = 5588;
/// The login's scopes (the session itself only needs "streaming")
const WEB_API_SCOPES: &str = "streaming user-read-private user-library-read \
playlist-read-private playlist-read-collaborative";
/// Left by builds that browsed through the Web API; removed at logout
const OLD_WEB_TOKEN_FILE: &str = "web_token.json";

type Progress = Box<dyn Fn(f64) + Send>;

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("spotify")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

/// The logged-in session; made again from the stored credentials when the
/// connection dropped
fn current() -> &'static Mutex<Option<Session>> {
    static SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(None))
}

/// Ends a running login (spotify_cancel_login)
fn cancel() -> &'static watch::Sender<u64> {
    static CANCEL: OnceLock<watch::Sender<u64>> = OnceLock::new();
    CANCEL.get_or_init(|| watch::channel(0).0)
}

/// The redirect URL caught by the in-app login page (spotify_login_redirect)
fn redirect() -> &'static watch::Sender<String> {
    static REDIRECT: OnceLock<watch::Sender<String>> = OnceLock::new();
    REDIRECT.get_or_init(|| watch::channel(String::new()).0)
}

/// What a redirect query means for the login with `state`: None when it is
/// not this login's, else its code or error
fn answer(query: &str, state: &str) -> Option<Result<String, String>> {
    if query_value(query, "state").as_deref() != Some(state) {
        return None;
    }
    Some(
        query_value(query, "code")
            .ok_or_else(|| query_value(query, "error").unwrap_or_else(|| "refused".to_owned())),
    )
}

async fn cancelled(mut rx: watch::Receiver<u64>) {
    let _ = rx.changed().await;
}

fn cache(dir: &Path) -> Result<Cache, String> {
    Cache::new(Some(dir), None, None, None).map_err(|e| format!("Cache: {e}"))
}

/// One per install: Spotify lists the device under it
fn device_id(dir: &Path) -> String {
    let path = dir.join("device_id");
    if let Ok(id) = std::fs::read_to_string(&path) {
        if id.trim().len() == 40 {
            return id.trim().to_owned();
        }
    }
    let mut bytes = [0u8; 20];
    rand::rng().fill_bytes(&mut bytes);
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let _ = std::fs::write(&path, &id);
    id
}

fn config(dir: &Path) -> SessionConfig {
    let mut config = SessionConfig::default();
    config.device_id = device_id(dir);
    config.tmp_dir = dir.join("tmp");
    let _ = std::fs::create_dir_all(&config.tmp_dir);
    config
}

/// Logs in and keeps the session; the reusable credentials Spotify returns
/// are stored in `dir`, so later starts need no login
async fn connect(dir: &Path, credentials: Credentials) -> Result<Session, String> {
    let session = Session::new(config(dir), Some(cache(dir)?));
    session
        .connect(credentials, true)
        .await
        .map_err(|e| format!("Spotify login: {e}"))?;
    // Not shut down: a download may still use the old one; it ends with
    // its last user
    *current().lock().unwrap() = Some(session.clone());
    Ok(session)
}

fn live_session() -> Option<Session> {
    current()
        .lock()
        .unwrap()
        .as_ref()
        .filter(|session| !session.is_invalid())
        .cloned()
}

/// The logged-in session; one connect at a time, so downloads started
/// together share it
async fn session(dir: &Path) -> Result<Session, String> {
    static CONNECTING: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    if let Some(session) = live_session() {
        return Ok(session);
    }
    let _connecting = CONNECTING.get_or_init(Default::default).lock().await;
    if let Some(session) = live_session() {
        return Ok(session); // made while this one waited
    }
    let credentials = cache(dir)?
        .credentials()
        .ok_or_else(|| "Not logged in to Spotify".to_owned())?;
    connect(dir, credentials).await
}

fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
                continue;
            }
            (b'+', _) => out.push(b' '),
            (b, _) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn form_encode(pairs: &[(&str, &str)]) -> String {
    let encode = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Opens DJ Sugar again (BackToAppActivity)
const BACK_TO_APP: &str = "djsugar://back";

fn html_page(title: &str, text: &str) -> String {
    format!(
        "<!doctype html><html><head><meta name=\"viewport\" content=\"width=device-width\">\
         </head><body style=\"font-family:sans-serif;text-align:center;margin-top:30vh;\
         background:#121212;color:#fff\"><h2>{title}</h2><p>{text}</p>\
         <p><a href=\"{BACK_TO_APP}\" style=\"color:#1ed760;font-size:1.2em\">Back to DJ Sugar</a></p>\
         <script>location.href=\"{BACK_TO_APP}\";</script></body></html>"
    )
}

/// No connection (DNS, connect), not an answer from Spotify
fn is_network_error(error: &str) -> bool {
    let error = error.to_lowercase();
    [
        "lookup address",
        "dns error",
        "client error (connect)",
        "connection refused",
        "connection reset",
        "network is unreachable",
        "timed out",
    ]
    .iter()
    .any(|marker| error.contains(marker))
}

/// Android keeps the network from an app in the background for a moment:
/// the login steps after the browser are tried again (only when the
/// network failed) until it is in front
async fn retry<T, F, Fut>(mut step: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut attempt = 0;
    loop {
        match step().await {
            Ok(value) => return Ok(value),
            Err(e) if attempt < 30 && is_network_error(&e) => {
                attempt += 1;
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// The code: from the in-app login page, or the browser's redirect to
/// http://127.0.0.1:5588/login
async fn wait_for_code(
    listener: TcpListener,
    mut redirects: watch::Receiver<String>,
    state: &str,
) -> Result<String, String> {
    loop {
        let (mut stream, _) = tokio::select! {
            accepted = listener.accept() => accepted.map_err(|e| e.to_string())?,
            changed = redirects.changed() => {
                changed.map_err(|e| e.to_string())?;
                let url = redirects.borrow_and_update().clone();
                let query = url.split_once('?').map_or("", |(_, q)| q);
                match answer(query, state) {
                    Some(result) => return result,
                    None => continue,
                }
            }
        };
        let mut request = vec![0u8; 8192];
        let n = match tokio::time::timeout(Duration::from_secs(10), stream.read(&mut request)).await
        {
            Ok(Ok(n)) => n,
            _ => continue,
        };
        let request = String::from_utf8_lossy(&request[..n]).into_owned();
        let target = request.split_whitespace().nth(1).unwrap_or("");
        let Some(query) = target.strip_prefix("/login?") else {
            // favicon and the like
            let _ = stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            continue;
        };
        // Not from the login started here (another page, an old tab): answered,
        // but the login keeps waiting for its own
        let result = answer(query, state);
        let body = if matches!(result, Some(Ok(_))) {
            html_page("Logged in to Spotify", "Go back to DJ Sugar.")
        } else {
            html_page("Spotify login failed", "Go back to DJ Sugar and try again.")
        };
        let _ = stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await;
        let _ = stream.shutdown().await;
        if let Some(result) = result {
            return result;
        }
    }
}

/// Spotify's login page in the browser (PKCE), back on a loopback port
async fn login_web(dir: PathBuf, open_url: Box<dyn Fn(&str) + Send>) -> Result<String, String> {
    // First: a cancel sent from now on is seen
    let rx = cancel().subscribe();
    tokio::select! {
        user = login_web_steps(dir, open_url) => user,
        () = cancelled(rx) => Err("cancelled".to_owned()),
    }
}

/// The port of a login just cancelled can take a moment to be free
async fn bind_login_port() -> Result<TcpListener, String> {
    let mut attempt = 0;
    loop {
        match TcpListener::bind(("127.0.0.1", WEB_PORT)).await {
            Ok(listener) => return Ok(listener),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && attempt < 20 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(format!("Login port {WEB_PORT}: {e}")),
        }
    }
}

async fn login_web_steps(
    dir: PathBuf,
    open_url: Box<dyn Fn(&str) + Send>,
) -> Result<String, String> {
    let redirect = format!("http://127.0.0.1:{WEB_PORT}/login");
    let mut redirects = self::redirect().subscribe();
    redirects.mark_unchanged(); // only redirects of this login
    let listener = bind_login_port().await?;
    let mut random = [0u8; 64];
    rand::rng().fill_bytes(&mut random);
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = URL_SAFE_NO_PAD.encode(&random[..16]);
    open_url(&format!(
        "https://accounts.spotify.com/authorize?{}",
        form_encode(&[
            ("client_id", WEB_CLIENT_ID),
            ("response_type", "code"),
            ("redirect_uri", &redirect),
            ("scope", WEB_API_SCOPES),
            ("code_challenge_method", "S256"),
            ("code_challenge", &challenge),
            ("state", &state),
        ])
    ));
    let code = wait_for_code(listener, redirects, &state).await?;
    let json = token_request(&[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect),
        ("client_id", WEB_CLIENT_ID),
        ("code_verifier", &verifier),
    ])
    .await?;
    let token = json["access_token"]
        .as_str()
        .ok_or_else(|| format!("Spotify login: {json}"))?
        .to_owned();
    retry(|| connect(&dir, Credentials::with_access_token(token.clone())))
        .await
        .map(|session| session.username())
}

/// accounts.spotify.com/api/token, retried while the network is away
async fn token_request(form: &[(&str, &str)]) -> Result<serde_json::Value, String> {
    let body = form_encode(form);
    let response = retry(|| async {
        let request = http::Request::post("https://accounts.spotify.com/api/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(bytes::Bytes::from(body.clone()))
            .map_err(|e| e.to_string())?;
        HttpClient::new(None)
            .request_body(request)
            .await
            .map_err(|e| format!("Spotify login: {e:?}"))
    })
    .await?;
    serde_json::from_slice(&response).map_err(|e| format!("Spotify login: {e}"))
}

/// The track, or a playable alternative of it (another release of the same
/// recording) when it is not available in the user's country
async fn playable_item(session: &Session, uri: SpotifyUri) -> Result<AudioItem, String> {
    let item = AudioItem::get_file(session, uri)
        .await
        .map_err(|e| format!("Track info: {e}"))?;
    if item.availability.is_ok() && !item.files.is_empty() {
        return Ok(item);
    }
    let Some(alternatives) = item.alternatives.clone() else {
        return Err(match item.availability {
            Err(e) => format!("Not playable: {e}"),
            Ok(()) => "Not playable here".to_owned(),
        });
    };
    let mut lookups: FuturesUnordered<_> = alternatives
        .0
        .into_iter()
        .map(|alt| AudioItem::get_file(session, alt))
        .collect();
    while let Some(alt) = lookups.next().await {
        if let Ok(alt) = alt {
            if alt.availability.is_ok() && !alt.files.is_empty() {
                return Ok(alt);
            }
        }
    }
    Err("Not playable in this country".to_owned())
}

async fn download(
    dir: PathBuf,
    track_id: String,
    out_path: PathBuf,
    progress: Progress,
) -> Result<(), String> {
    let id = SpotifyId::from_base62(&track_id).map_err(|e| format!("Track id: {e}"))?;
    let session = session(&dir).await?;
    let item = playable_item(&session, SpotifyUri::Track { id }).await?;
    let file_id_track = SpotifyId::try_from(&item.track_id).map_err(|e| e.to_string())?;
    let (format, file_id) = FORMATS
        .iter()
        .find_map(|f| item.files.get(f).map(|id| (*f, *id)))
        .ok_or_else(|| "No Ogg Vorbis file for this track".to_owned())?;
    debug_assert!(AudioFiles::is_ogg_vorbis(format));
    progress(0.05);

    // Ahead of the file: the key comes from the access point, the file
    // from the CDN. Without a key (no Premium) there is nothing to play.
    let key = session
        .audio_key()
        .request(file_id_track, file_id)
        .await
        .map_err(|e| format!("No audio key (Spotify Premium needed): {e}"))?;
    let encrypted = AudioFile::open(&session, file_id, 40 * 1024)
        .await
        .map_err(|e| format!("Audio file: {e}"))?;
    let controller = encrypted
        .get_stream_loader_controller()
        .map_err(|e| e.to_string())?;
    let total = controller.len() as u64;
    // Whole file at once, not paced like playback
    controller.set_random_access_mode();

    // Reading blocks on the download: off the async workers
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut decrypted = AudioDecrypt::new(Some(key), encrypted);
        decrypted
            .seek(SeekFrom::Start(SPOTIFY_OGG_HEADER_END))
            .map_err(|e| e.to_string())?;
        let partial = out_path.with_extension("ogg.part");
        let mut out = File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
        let mut buffer = vec![0u8; 64 * 1024];
        let mut done = SPOTIFY_OGG_HEADER_END;
        loop {
            let n = decrypted
                .read(&mut buffer)
                .map_err(|e| format!("Download: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            if total > 0 {
                progress(0.05 + 0.9 * (done as f64 / total as f64).min(1.0));
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        drop(out);
        if total > 0 && done < total {
            let _ = std::fs::remove_file(&partial);
            return Err("Download ended early".to_owned());
        }
        std::fs::rename(&partial, &out_path).map_err(|e| e.to_string())?;
        progress(1.0);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ================================ C API ================================

unsafe fn arg(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        CStr::from_ptr(value).to_string_lossy().into_owned()
    }
}

fn c_string(value: String) -> *mut c_char {
    CString::new(value.replace('\0', " "))
        .unwrap_or_default()
        .into_raw()
}

/// A panic in librespot ends in an error message, not in a crashed app
fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err("Spotify: internal error".to_owned()))
}

/// NULL (and `*out` set) on success, the error message otherwise
unsafe fn finish(result: Result<String, String>, out: *mut *mut c_char) -> *mut c_char {
    match result {
        Ok(value) => {
            if !out.is_null() {
                *out = c_string(value);
            }
            std::ptr::null_mut()
        }
        Err(message) => c_string(message),
    }
}

struct UserPtr(*mut c_void);
// The C++ side guarantees `user` outlives the blocking call
unsafe impl Send for UserPtr {}

/// # Safety
/// See spotifybridge.h
#[no_mangle]
pub unsafe extern "C" fn spotify_login_web(
    cache_dir: *const c_char,
    open_url: Option<extern "C" fn(*mut c_void, *const c_char)>,
    user: *mut c_void,
    out_user: *mut *mut c_char,
) -> *mut c_char {
    let user = UserPtr(user);
    let open: Box<dyn Fn(&str) + Send> = Box::new(move |url| {
        let user = &user;
        if let (Some(callback), Ok(url)) = (open_url, CString::new(url)) {
            callback(user.0, url.as_ptr());
        }
    });
    let result = guarded(|| runtime().block_on(login_web(PathBuf::from(arg(cache_dir)), open)));
    finish(result, out_user)
}

/// # Safety
/// See spotifybridge.h
#[no_mangle]
pub unsafe extern "C" fn spotify_login_redirect(url: *const c_char) {
    redirect().send_replace(arg(url));
}

#[no_mangle]
pub extern "C" fn spotify_cancel_login() {
    cancel().send_modify(|n| *n += 1);
}

/// # Safety
/// See spotifybridge.h
#[no_mangle]
pub unsafe extern "C" fn spotify_logged_in(cache_dir: *const c_char) -> bool {
    cache(Path::new(&arg(cache_dir))).is_ok_and(|c| c.credentials().is_some())
}

/// # Safety
/// See spotifybridge.h
#[no_mangle]
pub unsafe extern "C" fn spotify_logout(cache_dir: *const c_char) {
    if let Some(session) = current().lock().unwrap().take() {
        session.shutdown();
    }
    let dir = PathBuf::from(arg(cache_dir));
    let _ = std::fs::remove_file(dir.join("credentials.json"));
    let _ = std::fs::remove_file(dir.join(OLD_WEB_TOKEN_FILE));
}

/// # Safety
/// See spotifybridge.h
#[no_mangle]
pub unsafe extern "C" fn spotify_browse(
    cache_dir: *const c_char,
    request: *const c_char,
    offset: i32,
    limit: i32,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    let (dir, request) = (PathBuf::from(arg(cache_dir)), arg(request));
    let (offset, limit) = (offset.max(0) as usize, limit.clamp(1, 100) as usize);
    let result = guarded(|| {
        runtime().block_on(async {
            let session = session(&dir).await?;
            browse::browse(&session, &request, offset, limit)
                .await
                .map(|json| json.to_string())
        })
    });
    finish(result, out_json)
}

/// # Safety
/// See spotifybridge.h
#[no_mangle]
pub unsafe extern "C" fn spotify_download(
    cache_dir: *const c_char,
    track_id: *const c_char,
    out_path: *const c_char,
    progress: Option<extern "C" fn(*mut c_void, f64)>,
    user: *mut c_void,
) -> *mut c_char {
    let user = UserPtr(user);
    let report: Progress = Box::new(move |fraction| {
        let user = &user;
        if let Some(callback) = progress {
            callback(user.0, fraction);
        }
    });
    let result = guarded(|| {
        runtime().block_on(download(
            PathBuf::from(arg(cache_dir)),
            arg(track_id),
            PathBuf::from(arg(out_path)),
            report,
        ))
    });
    finish(result.map(|()| String::new()), std::ptr::null_mut())
}

/// # Safety
/// `message` must come from this library (or be NULL)
#[no_mangle]
pub unsafe extern "C" fn spotify_free_string(message: *mut c_char) {
    if !message.is_null() {
        drop(CString::from_raw(message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_queries() {
        assert_eq!(
            query_value("code=AQ%2Bx_y&state=s1", "code").as_deref(),
            Some("AQ+x_y")
        );
        assert_eq!(query_value("state=s1", "code"), None);
        assert_eq!(percent_decode("a%zz%4"), "a%zz%4");
        assert_eq!(percent_decode("a+b%20c"), "a b c");
    }

    #[test]
    fn tells_network_errors() {
        assert!(is_network_error("Spotify login: Unknown error {failed to lookup address information: No address associated with hostname}"));
        assert!(is_network_error(
            "Spotify login: Service unavailable { client error (Connect) }"
        ));
        assert!(!is_network_error(
            "Error { kind: InvalidArgument, error: StatusCode(400) }"
        ));
        assert!(!is_network_error("Spotify login: Bad connection ID"));
        assert!(!is_network_error(
            "Spotify token: Service unavailable { MercuryResponse status_code: 403 }"
        ));
        assert!(!is_network_error(
            "Spotify login: Permission denied { Login failed with reason: Bad credentials }"
        ));
    }

    #[test]
    fn encodes_forms() {
        assert_eq!(
            form_encode(&[
                ("redirect_uri", "http://127.0.0.1:5588/login"),
                ("s", "a b")
            ]),
            "redirect_uri=http%3A%2F%2F127.0.0.1%3A5588%2Flogin&s=a%20b"
        );
    }
}
