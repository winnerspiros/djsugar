//! Scrobbling: the tracks the audience hears go to the DJ's ListenBrainz
//! and Last.fm profiles (see `lastfm`), as a music player's do.
//!
//! Off until switched on in Settings › Player with the DJ's own user
//! token. The track is the deck heard most in the master mix (as for the
//! now-playing feed); a deck cued in the headphones never counts. A track
//! is sent as "playing now" after a few seconds, and as a listen once it
//! has been heard for half its length or four minutes, whichever comes
//! first (tracks under 30 seconds never count), as ListenBrainz and
//! Last.fm ask. Listens that can't be sent wait in a file and go later.
//! Runs on the UI thread once a second; requests go on a thread of their
//! own.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use djsugar_control::ControlRegistry;
use djsugar_engine::{LoadedTracks, Track};
use serde_json::{json, Value};

use crate::{broadcast, settings};

/// The switch: off at first.
pub const ENABLED: &str = "scrobble_listenbrainz";
/// The DJ's ListenBrainz user token.
pub const TOKEN: &str = "listenbrainz_token";

const API: &str = "https://api.listenbrainz.org/1";
/// Seconds heard before a track is sent as playing now.
const NOW_AFTER: f64 = 5.0;
/// Listens sent in one request.
const BATCH: usize = 100;
/// How often listens that couldn't be sent are tried again.
const RETRY: Duration = Duration::from_secs(120);

/// Set when a setting changes, so the next tick reads them again.
static CHANGED: AtomicBool = AtomicBool::new(true);
/// What Settings shows under the switch.
static STATUS: Mutex<String> = Mutex::new(String::new());

/// Tells the scrobbler its settings changed.
pub fn settings_changed() {
    CHANGED.store(true, Ordering::Release);
}

/// Who is signed in and what was sent, or what went wrong.
pub fn status() -> String {
    STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

fn set_status(s: String) {
    if let Ok(mut status) = STATUS.lock() {
        *status = s;
    }
}

/// Whether a track of `duration` seconds heard for `heard` seconds counts
/// as a listen: half of it or four minutes, and never under 30 seconds.
/// An unknown length (0) needs the four minutes.
pub fn counts(heard: f64, duration: f64) -> bool {
    if duration <= 0.0 {
        return heard >= 240.0;
    }
    duration >= 30.0 && heard >= (duration / 2.0).min(240.0)
}

/// The track as ListenBrainz takes it, heard from `at` (Unix seconds) or
/// playing now (None).
fn listen(t: &Track, at: Option<i64>) -> Value {
    let mut info = json!({
        "media_player": "DJ Sugar",
        "submission_client": "DJ Sugar",
        "submission_client_version": env!("CARGO_PKG_VERSION"),
    });
    if t.duration > 0.0 {
        info["duration_ms"] = json!((t.duration * 1000.0).round() as u64);
    }
    if !t.info.isrc.trim().is_empty() {
        info["isrc"] = json!(t.info.isrc.trim());
    }
    if t.info.track_number > 0 {
        info["tracknumber"] = json!(t.info.track_number);
    }
    let mut meta = json!({
        "artist_name": t.artist.trim(),
        "track_name": t.title.trim(),
        "additional_info": info,
    });
    if !t.album.trim().is_empty() {
        meta["release_name"] = json!(t.album.trim());
    }
    let mut out = json!({ "track_metadata": meta });
    if let Some(at) = at {
        out["listened_at"] = json!(at);
    }
    out
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The track heard now, and how long it has been.
struct Heard {
    track: Arc<Track>,
    /// When it started being heard (Unix seconds).
    started: i64,
    seconds: f64,
    sent_now: bool,
    sent: bool,
}

enum Job {
    /// The token changed: empty stops sending.
    Token(String),
    PlayingNow(Value),
    Listen(Value),
}

pub struct Scrobbler {
    registry: Arc<ControlRegistry>,
    loaded: LoadedTracks,
    /// Sending to ListenBrainz, and to Last.fm.
    on: bool,
    lastfm_on: bool,
    heard: Option<Heard>,
    last: Instant,
    jobs: Sender<Job>,
    lastfm: Sender<crate::lastfm::Job>,
}

impl Scrobbler {
    pub fn new(registry: Arc<ControlRegistry>, loaded: LoadedTracks) -> Self {
        let (jobs, rx) = mpsc::channel::<Job>();
        let _ = std::thread::Builder::new()
            .name("scrobbling".into())
            .spawn(move || worker(rx));
        Self {
            registry,
            loaded,
            on: false,
            lastfm_on: false,
            heard: None,
            last: Instant::now(),
            jobs,
            lastfm: crate::lastfm::start(),
        }
    }

    /// Follows the mix; call about once a second.
    pub fn tick(&mut self) {
        let dt = self.last.elapsed().as_secs_f64().min(5.0);
        self.last = Instant::now();
        if CHANGED.swap(false, Ordering::AcqRel) {
            let token = settings::string(TOKEN).trim().to_string();
            self.on = settings::flag(ENABLED, false) && !token.is_empty();
            let _ = self
                .jobs
                .send(Job::Token(if self.on { token } else { String::new() }));
            let account = crate::lastfm::Account::read();
            self.lastfm_on = settings::flag(crate::lastfm::ENABLED, false) && account.ready();
            let _ = self
                .lastfm
                .send(crate::lastfm::Job::Account(if self.lastfm_on {
                    account
                } else {
                    Default::default()
                }));
            if !self.on && !self.lastfm_on {
                self.heard = None;
            }
        }
        if !self.on && !self.lastfm_on {
            return;
        }
        let Some((_, track)) = broadcast::audible_deck(&self.registry, &self.loaded) else {
            return;
        };
        if track.artist.trim().is_empty() || track.title.trim().is_empty() {
            return;
        }
        let same = self.heard.as_ref().is_some_and(|h| h.track.id == track.id);
        if !same {
            self.heard = Some(Heard {
                track,
                started: now(),
                seconds: 0.0,
                sent_now: false,
                sent: false,
            });
        }
        let Some(h) = &mut self.heard else { return };
        h.seconds += dt;
        if !h.sent_now && h.seconds >= NOW_AFTER {
            h.sent_now = true;
            if self.on {
                let _ = self.jobs.send(Job::PlayingNow(listen(&h.track, None)));
            }
            if self.lastfm_on {
                let _ = self
                    .lastfm
                    .send(crate::lastfm::Job::PlayingNow(crate::lastfm::play(
                        &h.track, None,
                    )));
            }
        }
        if !h.sent && counts(h.seconds, h.track.duration) {
            h.sent = true;
            if self.on {
                let _ = self
                    .jobs
                    .send(Job::Listen(listen(&h.track, Some(h.started))));
            }
            if self.lastfm_on {
                let _ = self
                    .lastfm
                    .send(crate::lastfm::Job::Scrobble(crate::lastfm::play(
                        &h.track,
                        Some(h.started),
                    )));
            }
        }
    }
}

/// Why a request failed.
#[derive(Debug, PartialEq)]
enum Failed {
    /// ListenBrainz didn't take the token.
    Token,
    /// No answer, or one that went wrong.
    Unreachable,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(concat!("DJSugar/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn submit(agent: &ureq::Agent, token: &str, kind: &str, payload: &[Value]) -> Result<(), Failed> {
    let body = json!({ "listen_type": kind, "payload": payload });
    match agent
        .post(&format!("{API}/submit-listens"))
        .header("Authorization", &format!("Token {token}"))
        .send_json(&body)
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::StatusCode(401)) => Err(Failed::Token),
        Err(e) => {
            eprintln!("can't send plays to ListenBrainz: {e}");
            Err(Failed::Unreachable)
        }
    }
}

/// The ListenBrainz user the token belongs to.
fn user_of(agent: &ureq::Agent, token: &str) -> Result<String, Failed> {
    let mut r = agent
        .get(&format!("{API}/validate-token"))
        .header("Authorization", &format!("Token {token}"))
        .call()
        .map_err(|_| Failed::Unreachable)?;
    let v: Value = r.body_mut().read_json().map_err(|_| Failed::Unreachable)?;
    match v["user_name"].as_str() {
        Some(u) if v["valid"].as_bool() == Some(true) => Ok(u.to_string()),
        _ => Err(Failed::Token),
    }
}

fn pending_file() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("scrobbles-waiting.json"))
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
    let text = serde_json::to_string(pending).unwrap_or_default();
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

/// What Settings says while signed in.
fn signed_in(user: &str, sent: usize, waiting: usize) -> String {
    let mut s = format!("Signed in as {user}");
    if sent > 0 {
        s += &format!(" · {sent} play{} sent", if sent == 1 { "" } else { "s" });
    }
    if waiting > 0 {
        s += &format!(" · {waiting} waiting to be sent");
    }
    s
}

fn worker(jobs: mpsc::Receiver<Job>) {
    let agent = agent();
    let mut token = String::new();
    let mut user: Option<String> = None;
    let mut pending = read_pending();
    let mut sent = 0;
    loop {
        match jobs.recv_timeout(RETRY) {
            Ok(Job::Token(t)) => {
                token = t;
                user = None;
                if token.is_empty() {
                    set_status(String::new());
                    continue;
                }
                set_status("Checking the token…".into());
                match user_of(&agent, &token) {
                    Ok(u) => user = Some(u),
                    Err(Failed::Token) => {
                        set_status(
                            "ListenBrainz didn't accept this token. Copy it again from \
                             listenbrainz.org/settings."
                                .into(),
                        );
                        continue;
                    }
                    Err(Failed::Unreachable) => set_status(
                        "Can't reach ListenBrainz now; plays wait until it answers.".into(),
                    ),
                }
            }
            Ok(Job::PlayingNow(p)) => {
                if !token.is_empty() {
                    let _ = submit(&agent, &token, "playing_now", &[p]);
                }
                continue;
            }
            Ok(Job::Listen(p)) => {
                pending.push(p);
                write_pending(&pending);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if token.is_empty() {
            continue;
        }
        while !pending.is_empty() {
            let n = pending.len().min(BATCH);
            let kind = if n == 1 { "single" } else { "import" };
            match submit(&agent, &token, kind, &pending[..n]) {
                Ok(()) => {
                    pending.drain(..n);
                    sent += n;
                    write_pending(&pending);
                }
                Err(Failed::Token) => {
                    user = None;
                    set_status(
                        "ListenBrainz didn't accept this token. Copy it again from \
                         listenbrainz.org/settings."
                            .into(),
                    );
                    break;
                }
                Err(Failed::Unreachable) => break,
            }
        }
        if user.is_none() {
            // Not checked yet (ListenBrainz didn't answer): try again.
            if let Ok(u) = user_of(&agent, &token) {
                user = Some(u);
            }
        }
        match &user {
            Some(u) => set_status(signed_in(u, sent, pending.len())),
            None if !pending.is_empty() => set_status(format!(
                "Can't reach ListenBrainz now · {} waiting to be sent",
                pending.len()
            )),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_a_play_counts() {
        // Half of a short track.
        assert!(!counts(89.0, 180.0));
        assert!(counts(90.0, 180.0));
        // Four minutes of a long one.
        assert!(!counts(239.0, 600.0));
        assert!(counts(240.0, 600.0));
        // Never under 30 seconds.
        assert!(!counts(25.0, 25.0));
        // An unknown length needs the four minutes.
        assert!(!counts(200.0, 0.0));
        assert!(counts(240.0, 0.0));
    }

    #[test]
    fn a_listen_as_listenbrainz_takes_it() {
        let mut t = (*djsugar_engine::synth::sample_library()[0]).clone();
        t.artist = "Lilac Waves".into();
        t.title = "Midnight Drive".into();
        t.album = "Night Shift".into();
        t.duration = 200.0;
        t.info.track_number = 1;
        let v = listen(&t, Some(1_700_000_000));
        assert_eq!(v["listened_at"], 1_700_000_000);
        assert_eq!(v["track_metadata"]["artist_name"], "Lilac Waves");
        assert_eq!(v["track_metadata"]["track_name"], "Midnight Drive");
        assert_eq!(v["track_metadata"]["release_name"], "Night Shift");
        assert_eq!(
            v["track_metadata"]["additional_info"]["duration_ms"],
            200_000
        );
        assert_eq!(
            v["track_metadata"]["additional_info"]["media_player"],
            "DJ Sugar"
        );
        assert!(listen(&t, None).get("listened_at").is_none());
    }

    #[test]
    fn what_settings_says() {
        assert_eq!(signed_in("lilac", 0, 0), "Signed in as lilac");
        assert_eq!(
            signed_in("dj", 1, 2),
            "Signed in as dj · 1 play sent · 2 waiting to be sent"
        );
    }
}
