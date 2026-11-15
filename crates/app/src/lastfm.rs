//! Scrobbling to Last.fm, beside ListenBrainz (see `scrobble`), with the
//! DJ's own API account: DJ Sugar ships no key. The DJ makes one at
//! last.fm/api/account/create, pastes the key and shared secret in
//! Settings › Broadcast and signs in once in the browser; the session key
//! that comes back is kept in the settings (out of backups, as its name
//! holds "token") and doesn't expire.
//!
//! Requests are signed as Last.fm's API asks: the parameters' names and
//! values in name order, then the secret, through MD5. Plays that can't be
//! sent wait in a file and go later, up to 50 at a time.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use djsugar_engine::Track;
use serde_json::{json, Value};

use crate::settings;

/// The switch: off at first.
pub const ENABLED: &str = "scrobble_lastfm";
pub const API_KEY: &str = "lastfm_api_key";
pub const SECRET: &str = "lastfm_secret";
/// The session key from signing in ("token" keeps it out of backups).
pub const SESSION: &str = "lastfm_session_token";
/// Who signed in.
pub const USER: &str = "lastfm_user";

const API: &str = "https://ws.audioscrobbler.com/2.0/";
const AUTH: &str = "https://www.last.fm/api/auth/";
/// Scrobbles sent in one request (Last.fm takes up to 50).
const BATCH: usize = 50;
const RETRY: Duration = Duration::from_secs(120);
/// How long signing in waits for the DJ to allow access in the browser.
const SIGN_IN_WAIT: Duration = Duration::from_secs(300);

static STATUS: Mutex<String> = Mutex::new(String::new());
/// Set when the status changes, for Settings to show it.
static FRESH: AtomicBool = AtomicBool::new(false);
static SIGNING_IN: AtomicBool = AtomicBool::new(false);

pub fn status() -> String {
    STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

fn set_status(s: String) {
    if let Ok(mut status) = STATUS.lock() {
        if *status != s {
            *status = s;
            FRESH.store(true, Ordering::Release);
        }
    }
}

/// Whether the status changed since this was last asked.
pub fn take_fresh() -> bool {
    FRESH.swap(false, Ordering::AcqRel)
}

pub fn signing_in() -> bool {
    SIGNING_IN.load(Ordering::Acquire)
}

/// The key, secret and session from the settings.
#[derive(Clone, Default, PartialEq)]
pub struct Account {
    pub key: String,
    pub secret: String,
    pub session: String,
}

impl Account {
    pub fn read() -> Self {
        Self {
            key: settings::string(API_KEY).trim().to_string(),
            secret: settings::string(SECRET).trim().to_string(),
            session: settings::string(SESSION).trim().to_string(),
        }
    }

    pub fn ready(&self) -> bool {
        !self.key.is_empty() && !self.secret.is_empty() && !self.session.is_empty()
    }
}

/// The signature Last.fm asks for: every parameter but `format`, by name,
/// each name followed by its value, then the secret, through MD5.
pub fn signature(params: &BTreeMap<String, String>, secret: &str) -> String {
    let mut text = String::new();
    for (k, v) in params {
        if k != "format" && k != "callback" {
            text.push_str(k);
            text.push_str(v);
        }
    }
    text.push_str(secret);
    format!("{:x}", md5::compute(text.as_bytes()))
}

/// A play as Last.fm takes it: artist, track, album, duration and track
/// number, and when it started (for a scrobble).
pub fn play(t: &Track, at: Option<i64>) -> Value {
    let mut p = json!({ "artist": t.artist.trim(), "track": t.title.trim() });
    if !t.album.trim().is_empty() {
        p["album"] = json!(t.album.trim());
    }
    if t.duration > 0.0 {
        p["duration"] = json!(t.duration.round() as u64);
    }
    if t.info.track_number > 0 {
        p["trackNumber"] = json!(t.info.track_number);
    }
    if let Some(at) = at {
        p["timestamp"] = json!(at);
    }
    p
}

/// The parameters of a batch of scrobbles: `artist[0]`, `track[0]` and so
/// on.
fn scrobble_params(plays: &[Value]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (i, p) in plays.iter().enumerate() {
        if let Some(fields) = p.as_object() {
            for (k, v) in fields {
                let v = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                out.insert(format!("{k}[{i}]"), v);
            }
        }
    }
    out
}

#[derive(Debug, PartialEq)]
enum Failed {
    /// The session or key isn't accepted: sign in again.
    Session(String),
    /// Not allowed in the browser yet (while signing in).
    NotYet,
    Unreachable,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(concat!("DJSugar/", env!("CARGO_PKG_VERSION")))
        // Last.fm answers errors as JSON with a 4xx status.
        .http_status_as_error(false)
        .build()
        .into()
}

/// Calls `method` with `params`, signed; a POST for writes.
fn call(
    agent: &ureq::Agent,
    account: &Account,
    method: &str,
    mut params: BTreeMap<String, String>,
    post: bool,
) -> Result<Value, Failed> {
    params.insert("method".into(), method.into());
    params.insert("api_key".into(), account.key.clone());
    let sig = signature(&params, &account.secret);
    params.insert("api_sig".into(), sig);
    params.insert("format".into(), "json".into());
    let answer = if post {
        agent
            .post(API)
            .send_form(params.iter().map(|(k, v)| (k.as_str(), v.as_str())))
    } else {
        let mut req = agent.get(API);
        for (k, v) in &params {
            req = req.query(k, v);
        }
        req.call()
    };
    let mut r = answer.map_err(|e| {
        eprintln!("can't reach Last.fm: {e}");
        Failed::Unreachable
    })?;
    let v: Value = r.body_mut().read_json().map_err(|_| Failed::Unreachable)?;
    match v["error"].as_i64() {
        None => Ok(v),
        // Not allowed yet, while signing in.
        Some(14) => Err(Failed::NotYet),
        // Bad key, signature or session, or a suspended key.
        Some(4 | 9 | 10 | 13 | 26) => Err(Failed::Session(
            v["message"]
                .as_str()
                .unwrap_or("Last.fm refused the request")
                .to_string(),
        )),
        Some(_) => Err(Failed::Unreachable),
    }
}

/// Signs in on a thread of its own: gets a token, opens Last.fm in the
/// browser for the DJ to allow access, then waits for the session.
pub fn sign_in() {
    if SIGNING_IN.swap(true, Ordering::AcqRel) {
        return;
    }
    let mut account = Account::read();
    account.session.clear();
    let _ = std::thread::Builder::new()
        .name("last.fm sign-in".into())
        .spawn(move || {
            let agent = agent();
            let done = |s: String| {
                SIGNING_IN.store(false, Ordering::Release);
                set_status(s);
            };
            if account.key.is_empty() || account.secret.is_empty() {
                done("Paste your API key and shared secret first.".into());
                return;
            }
            let token = match call(&agent, &account, "auth.getToken", BTreeMap::new(), false) {
                Ok(v) => v["token"].as_str().unwrap_or_default().to_string(),
                Err(Failed::Session(m)) => {
                    return done(format!("Last.fm: {m}. Check the key and secret."))
                }
                Err(_) => return done("Can't reach Last.fm now.".into()),
            };
            if token.is_empty() {
                return done("Last.fm didn't start the sign-in.".into());
            }
            set_status("Allow DJ Sugar in the browser, then come back here.".into());
            crate::spotify::open_in_browser(&format!(
                "{AUTH}?api_key={}&token={token}",
                account.key
            ));
            let start = Instant::now();
            while start.elapsed() < SIGN_IN_WAIT {
                std::thread::sleep(Duration::from_secs(3));
                let params = BTreeMap::from([("token".to_string(), token.clone())]);
                match call(&agent, &account, "auth.getSession", params, false) {
                    Ok(v) => {
                        let key = v["session"]["key"].as_str().unwrap_or_default();
                        let name = v["session"]["name"].as_str().unwrap_or_default();
                        if key.is_empty() {
                            return done("Last.fm didn't send a session.".into());
                        }
                        settings::set(SESSION, json!(key));
                        settings::set(USER, json!(name));
                        crate::scrobble::settings_changed();
                        return done(format!("Signed in as {name}"));
                    }
                    Err(Failed::NotYet) => continue,
                    Err(Failed::Session(m)) => return done(format!("Last.fm: {m}")),
                    Err(Failed::Unreachable) => continue,
                }
            }
            done("Sign-in timed out. Press Sign in to try again.".into());
        });
}

/// Forgets the session.
pub fn sign_out() {
    settings::set(SESSION, json!(""));
    settings::set(USER, json!(""));
    set_status(String::new());
    crate::scrobble::settings_changed();
}

pub enum Job {
    /// The account changed: no session stops sending.
    Account(Account),
    PlayingNow(Value),
    Scrobble(Value),
}

/// Starts the thread that sends to Last.fm.
pub fn start() -> Sender<Job> {
    let (jobs, rx) = mpsc::channel::<Job>();
    let _ = std::thread::Builder::new()
        .name("last.fm".into())
        .spawn(move || worker(rx));
    jobs
}

fn pending_file() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("lastfm-waiting.json"))
}

fn read_pending() -> Vec<Value> {
    pending_file()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_pending(pending: &[Value]) {
    let Some(path) = pending_file() else { return };
    if pending.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, serde_json::to_string(pending).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

fn worker(jobs: mpsc::Receiver<Job>) {
    let agent = agent();
    let mut account = Account::default();
    let mut pending = read_pending();
    let mut sent = 0;
    loop {
        match jobs.recv_timeout(RETRY) {
            Ok(Job::Account(a)) => {
                account = a;
                if !account.ready() {
                    continue;
                }
            }
            Ok(Job::PlayingNow(p)) => {
                if account.ready() {
                    let params: BTreeMap<String, String> = p
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(k, v)| {
                            let v = match v {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            (k.clone(), v)
                        })
                        .chain([("sk".to_string(), account.session.clone())])
                        .collect();
                    let _ = call(&agent, &account, "track.updateNowPlaying", params, true);
                }
                continue;
            }
            Ok(Job::Scrobble(p)) => {
                pending.push(p);
                write_pending(&pending);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if !account.ready() {
            continue;
        }
        while !pending.is_empty() {
            let n = pending.len().min(BATCH);
            let mut params = scrobble_params(&pending[..n]);
            params.insert("sk".into(), account.session.clone());
            match call(&agent, &account, "track.scrobble", params, true) {
                Ok(_) => {
                    pending.drain(..n);
                    sent += n;
                    write_pending(&pending);
                }
                Err(Failed::Session(m)) => {
                    set_status(format!("Last.fm: {m}. Sign in again."));
                    break;
                }
                Err(_) => break,
            }
        }
        let user = settings::string(USER);
        let mut s = format!("Signed in as {user}");
        if sent > 0 {
            s += &format!(" · {sent} play{} sent", if sent == 1 { "" } else { "s" });
        }
        if !pending.is_empty() {
            s += &format!(" · {} waiting to be sent", pending.len());
        }
        if !status().starts_with("Last.fm:") || sent > 0 {
            set_status(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_as_last_fm_asks() {
        // Names in order, each followed by its value, then the secret;
        // `format` left out.
        let params = BTreeMap::from([
            ("method".to_string(), "auth.getSession".to_string()),
            ("api_key".to_string(), "k".to_string()),
            ("token".to_string(), "t".to_string()),
            ("format".to_string(), "json".to_string()),
        ]);
        let expected = format!(
            "{:x}",
            md5::compute(b"api_keykmethodauth.getSessiontokentsecret")
        );
        assert_eq!(signature(&params, "secret"), expected);
    }

    #[test]
    fn scrobbles_are_numbered() {
        let plays = [
            json!({ "artist": "A", "track": "One", "timestamp": 10 }),
            json!({ "artist": "B", "track": "Two", "timestamp": 20 }),
        ];
        let p = scrobble_params(&plays);
        assert_eq!(p["artist[0]"], "A");
        assert_eq!(p["track[1]"], "Two");
        assert_eq!(p["timestamp[1]"], "20");
    }

    #[test]
    fn a_play_as_last_fm_takes_it() {
        let mut t = (*djsugar_engine::synth::sample_library()[0]).clone();
        t.artist = "Lilac Waves".into();
        t.title = "Midnight Drive".into();
        t.album = "Night Shift".into();
        t.duration = 200.4;
        let p = play(&t, Some(5));
        assert_eq!(p["artist"], "Lilac Waves");
        assert_eq!(p["album"], "Night Shift");
        assert_eq!(p["duration"], 200);
        assert_eq!(p["timestamp"], 5);
        assert!(play(&t, None).get("timestamp").is_none());
    }
}
