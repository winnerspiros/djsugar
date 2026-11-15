//! SoundCloud browsing: sign in, then list the DJ's likes, uploads and
//! playlists.
//!
//! SoundCloud's API terms don't allow mixing its streams or keeping them,
//! so a SoundCloud track plays on a deck only when the collection holds the
//! same track; any other opens on soundcloud.com.
//!
//! Sign-in uses OAuth with PKCE and the DJ's own app: they register one at
//! soundcloud.com/you/apps with [`REDIRECT_URI`] as its redirect URI, and
//! paste its client ID and secret (SoundCloud's token exchange needs both).
//! They and the refresh token are kept in `soundcloud.json` in the config
//! folder, so the next start signs in by itself. Lists are paged by
//! `next_href`.
//!
//! Network calls run on a worker thread, which speaks the same [`Request`]s
//! and [`Event`]s as Spotify's.

use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::spotify::{
    code_challenge, encode, open_in_browser, random_token, wait_for_code, Event, Playlist, Request,
    Saved, Service, StreamTrack, LIKED, PORT, REDIRECT_URI,
};

const LOGIN: &str = "https://secure.soundcloud.com/authorize";
const TOKEN: &str = "https://secure.soundcloud.com/oauth/token";
const API: &str = "https://api.soundcloud.com";
/// Where the sign-in is kept, in the config folder.
const SAVED: &str = "soundcloud.json";
/// The key of the DJ's own uploads in the tree.
const UPLOADS: &str = "uploads";
/// The most tracks a list reads.
const MAX_TRACKS: usize = 2000;

/// Starts SoundCloud's worker. With a saved sign-in it reconnects straight
/// away.
pub fn start() -> Service {
    let (requests, rx) = channel();
    let (tx, events) = channel();
    let saved = Saved::load(SAVED).filter(|_| crate::browser::STREAMING_ENABLED);
    let client_id = std::env::var("DJSUGAR_SOUNDCLOUD_CLIENT_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| saved.as_ref().map(|s| s.client_id.clone()))
        .unwrap_or_default();
    std::thread::Builder::new()
        .name("soundcloud".into())
        .spawn(move || Worker::new(tx).run(rx, saved))
        .expect("spawn soundcloud thread");
    Service::new(requests, events, client_id)
}

/// The app the DJ registered.
#[derive(Clone)]
struct App {
    client_id: String,
    secret: String,
}

struct Session {
    app: App,
    access_token: String,
    refresh_token: String,
    expires: Instant,
}

struct Worker {
    agent: ureq::Agent,
    events: Sender<Event>,
    session: Option<Session>,
    /// The secret from last time, used when the DJ signs in again with the
    /// same client ID and leaves the secret empty.
    saved_secret: String,
}

impl Worker {
    fn new(events: Sender<Event>) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            events,
            session: None,
            saved_secret: String::new(),
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    fn run(mut self, requests: Receiver<Request>, saved: Option<Saved>) {
        if let Some(saved) = &saved {
            self.saved_secret = saved.secret.clone();
        }
        if let Some(saved) = saved.filter(|s| !s.refresh_token.is_empty()) {
            self.emit(Event::Status("Signing in…".into()));
            let app = App {
                client_id: saved.client_id,
                secret: saved.secret,
            };
            let result = self.refresh(&app, &saved.refresh_token).and_then(|s| {
                self.session = Some(s);
                self.connected()
            });
            if let Err(e) = result {
                self.session = None;
                self.emit(Event::SignInFailed(format!("Couldn't sign in again: {e}")));
            }
        }
        for request in requests {
            match request {
                Request::Connect(typed) => {
                    let (client_id, secret) = split_keys(&typed);
                    let secret = if secret.is_empty() {
                        self.saved_secret.clone()
                    } else {
                        secret
                    };
                    let app = App { client_id, secret };
                    let result = self.login(&app).and_then(|s| {
                        self.session = Some(s);
                        self.connected()
                    });
                    if let Err(e) = result {
                        self.session = None;
                        self.emit(Event::Status(e));
                    }
                }
                Request::Tracks(key) => match self.tracks(&key) {
                    Ok(tracks) => self.emit(Event::Tracks { key, tracks }),
                    Err(error) => self.emit(Event::TracksFailed { key, error }),
                },
                // Only playlists' tracks are fetched from here.
                Request::Lookup(_) => {}
                Request::Playlists => {
                    let result = match self.session {
                        Some(_) => self.connected(),
                        None => Err("Not signed in".into()),
                    };
                    if let Err(e) = result {
                        self.emit(Event::SignInFailed(format!("Couldn't refresh: {e}")));
                    }
                }
                // Only Spotify makes playlists.
                Request::Export { list, .. } => self.emit(Event::ExportFailed {
                    list,
                    error: "Only Spotify can be exported to.".into(),
                }),
                Request::Disconnect => {
                    if let Some(s) = self.session.take() {
                        Saved {
                            client_id: s.app.client_id,
                            refresh_token: String::new(),
                            secret: s.app.secret,
                        }
                        .store(SAVED);
                    }
                    self.emit(Event::Disconnected);
                }
            }
        }
    }

    /// Fetches the DJ's name and playlists and reports the connection.
    fn connected(&mut self) -> Result<(), String> {
        let me = self.get(&format!("{API}/me"))?;
        let user = [&me["full_name"], &me["username"]]
            .iter()
            .find_map(|v| v.as_str().filter(|s| !s.trim().is_empty()))
            .unwrap_or("you")
            .to_string();
        let mut playlists = vec![
            Playlist {
                key: LIKED.into(),
                name: "Likes".into(),
                total: me["public_favorites_count"].as_u64().unwrap_or(0) as usize,
                ..Default::default()
            },
            Playlist {
                key: UPLOADS.into(),
                name: "My Tracks".into(),
                total: me["track_count"].as_u64().unwrap_or(0) as usize,
                ..Default::default()
            },
        ];
        // Their own playlists, then those they liked.
        for url in [
            format!("{API}/me/playlists?show_tracks=false"),
            format!("{API}/me/likes/playlists?show_tracks=false"),
        ] {
            for p in self
                .get_all(&url, MAX_TRACKS)?
                .iter()
                .filter_map(parse_playlist)
            {
                if !playlists.iter().any(|o| o.key == p.key) {
                    playlists.push(p);
                }
            }
        }
        self.emit(Event::Connected { user, playlists });
        Ok(())
    }

    /// A list's tracks, in order.
    fn tracks(&mut self, key: &str) -> Result<Vec<StreamTrack>, String> {
        let url = match key {
            LIKED => format!("{API}/me/likes/tracks"),
            UPLOADS => format!("{API}/me/tracks"),
            _ => format!("{API}/playlists/{}/tracks", encode(key)),
        };
        let items = self.get_all(&url, MAX_TRACKS)?;
        Ok(items.iter().filter_map(parse_track).collect())
    }

    fn login(&mut self, app: &App) -> Result<Session, String> {
        if app.client_id.is_empty() || app.secret.is_empty() {
            return Err("Enter your SoundCloud app's client ID and secret first.".into());
        }
        let listener = TcpListener::bind(("127.0.0.1", PORT))
            .map_err(|e| format!("Couldn't listen on port {PORT} for SoundCloud's reply: {e}"))?;
        let verifier = random_token(48);
        let state = random_token(12);
        let url = authorize_url(&app.client_id, &code_challenge(&verifier), &state);
        if !open_in_browser(&url) {
            eprintln!("Open this address to sign in to SoundCloud: {url}");
        }
        self.emit(Event::Status("Finish signing in in your browser…".into()));
        let code = wait_for_code(&listener, &state, "SoundCloud")?;
        let response = self.token(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", &app.client_id),
            ("client_secret", &app.secret),
            ("code_verifier", &verifier),
        ])?;
        self.session_from(app, response)
    }

    fn refresh(&self, app: &App, refresh_token: &str) -> Result<Session, String> {
        let response = self.token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &app.client_id),
            ("client_secret", &app.secret),
        ])?;
        self.session_from(app, response)
    }

    fn token(&self, form: &[(&str, &str)]) -> Result<Value, String> {
        let mut response = self
            .agent
            .post(TOKEN)
            .header("Accept", "application/json; charset=utf-8")
            .send_form(form.iter().copied())
            .map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body: Value = response
            .body_mut()
            .read_to_string()
            .ok()
            .and_then(|b| serde_json::from_str(&b).ok())
            .unwrap_or(Value::Null);
        if status != 200 {
            return Err(error_message(status, &body));
        }
        Ok(body)
    }

    /// SoundCloud's refresh tokens work once, so each new one is kept.
    fn session_from(&self, app: &App, v: Value) -> Result<Session, String> {
        let access_token = v["access_token"]
            .as_str()
            .ok_or("SoundCloud sent no access token")?
            .to_string();
        let refresh_token = v["refresh_token"].as_str().unwrap_or_default().to_string();
        let lifetime = v["expires_in"].as_u64().unwrap_or(3600);
        Saved {
            client_id: app.client_id.clone(),
            refresh_token: refresh_token.clone(),
            secret: app.secret.clone(),
        }
        .store(SAVED);
        Ok(Session {
            app: app.clone(),
            access_token,
            refresh_token,
            expires: Instant::now() + Duration::from_secs(lifetime),
        })
    }

    fn renew(&mut self) -> Result<(), String> {
        let s = self.session.as_ref().ok_or("Not signed in to SoundCloud")?;
        let fresh = self.refresh(&s.app, &s.refresh_token)?;
        self.session = Some(fresh);
        Ok(())
    }

    /// A GET against the API, renewing the token and waiting out rate limits.
    fn get(&mut self, url: &str) -> Result<Value, String> {
        let mut renewed = false;
        for _ in 0..5 {
            let s = self.session.as_ref().ok_or("Not signed in to SoundCloud")?;
            if !renewed && s.expires < Instant::now() + Duration::from_secs(60) {
                self.renew()?;
                renewed = true;
                continue;
            }
            let mut response = self
                .agent
                .get(url)
                .header("Authorization", &format!("OAuth {}", s.access_token))
                .header("Accept", "application/json; charset=utf-8")
                .call()
                .map_err(|e| e.to_string())?;
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            let body: Value = response
                .body_mut()
                .read_to_string()
                .ok()
                .and_then(|b| serde_json::from_str(&b).ok())
                .unwrap_or(Value::Null);
            match status {
                200 => return Ok(body),
                401 if !renewed => {
                    self.renew()?;
                    renewed = true;
                }
                429 => std::thread::sleep(Duration::from_secs(retry_after.unwrap_or(2).min(30))),
                _ => return Err(error_message(status, &body)),
            }
        }
        Err("SoundCloud kept refusing the request".into())
    }

    /// Follows `next_href` until the end or `max` items. A list may also
    /// come back as a plain array, unpaged.
    fn get_all(&mut self, url: &str, max: usize) -> Result<Vec<Value>, String> {
        let mut items = Vec::new();
        let join = if url.contains('?') { '&' } else { '?' };
        let mut next = Some(format!("{url}{join}linked_partitioning=true&limit=200"));
        while let Some(url) = next {
            let page = self.get(&url)?;
            let data = page["collection"]
                .as_array()
                .or(page.as_array())
                .cloned()
                .unwrap_or_default();
            if data.is_empty() {
                break;
            }
            items.extend(data);
            if items.len() >= max {
                items.truncate(max);
                break;
            }
            next = page["next_href"].as_str().map(str::to_string);
        }
        Ok(items)
    }
}

/// The client ID and secret as the sign-in form sends them, apart.
fn split_keys(typed: &str) -> (String, String) {
    let mut parts = typed.split_whitespace();
    let id = parts.next().unwrap_or_default().to_string();
    let secret = parts.next().unwrap_or_default().to_string();
    (id, secret)
}

fn error_message(status: u16, body: &Value) -> String {
    let detail = body["errors"][0]["error_message"]
        .as_str()
        .or(body["error_description"].as_str())
        .or(body["message"].as_str())
        .or(body["error"].as_str());
    match (status, detail) {
        (401, _) => "SoundCloud signed you out. Sign in again.".into(),
        (_, Some(d)) => format!("SoundCloud said: {d}"),
        _ => format!("SoundCloud answered {status}"),
    }
}

fn authorize_url(client_id: &str, challenge: &str, state: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT_URI),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge),
        ("state", state),
    ];
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect();
    format!("{LOGIN}?{}", query.join("&"))
}

/// A resource's URN, or one made from its numeric id.
fn urn(v: &Value, kind: &str) -> Option<String> {
    v["urn"]
        .as_str()
        .map(str::to_string)
        .or_else(|| v["id"].as_u64().map(|id| format!("soundcloud:{kind}:{id}")))
}

fn parse_playlist(v: &Value) -> Option<Playlist> {
    // A liked playlist may come wrapped as `{ "playlist": … }`.
    let v = if v["playlist"].is_object() {
        &v["playlist"]
    } else {
        v
    };
    Some(Playlist {
        key: urn(v, "playlists")?,
        name: v["title"].as_str().unwrap_or("Playlist").to_string(),
        total: v["track_count"].as_u64().unwrap_or(0) as usize,
        version: v["last_modified"].as_str().unwrap_or_default().to_string(),
    })
}

/// A track as a list row. A release's own artist, album and ISRC come from
/// its publisher details when the uploader filled them in.
fn parse_track(v: &Value) -> Option<StreamTrack> {
    let v = if v["track"].is_object() {
        &v["track"]
    } else {
        v
    };
    let id = urn(v, "tracks")?;
    let meta = &v["publisher_metadata"];
    let text = |x: &Value| {
        x.as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
    };
    let artist = text(&meta["artist"]).or_else(|| text(&v["user"]["username"]));
    Some(StreamTrack {
        page: text(&v["permalink_url"]).unwrap_or_else(|| "https://soundcloud.com".into()),
        isrc: text(&meta["isrc"]).unwrap_or_default(),
        title: text(&v["title"])?,
        artists: artist.into_iter().collect(),
        album: text(&meta["album_title"]).unwrap_or_default(),
        duration: v["duration"].as_f64().unwrap_or(0.0) / 1000.0,
        id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splits_the_keys() {
        assert_eq!(
            split_keys(" abc  s3cret "),
            ("abc".to_string(), "s3cret".to_string())
        );
        assert_eq!(split_keys("abc"), ("abc".to_string(), String::new()));
    }

    #[test]
    fn builds_the_authorize_address() {
        let url = authorize_url("abc", "chal", "st");
        assert!(url.starts_with("https://secure.soundcloud.com/authorize?client_id=abc"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8898%2Fcallback"));
    }

    #[test]
    fn reads_playlists_and_tracks() {
        let p = parse_playlist(
            &json!({ "urn": "soundcloud:playlists:7", "title": "Warm up", "track_count": 3 }),
        )
        .unwrap();
        assert_eq!(
            (p.key.as_str(), p.name.as_str(), p.total),
            ("soundcloud:playlists:7", "Warm up", 3)
        );
        let liked = parse_playlist(&json!({ "playlist": { "id": 9, "title": "Peak" } })).unwrap();
        assert_eq!(liked.key, "soundcloud:playlists:9");

        let t = parse_track(&json!({
            "id": 123, "title": "Insomnia (Extended Mix)", "duration": 520000,
            "permalink_url": "https://soundcloud.com/faithless/insomnia",
            "user": { "username": "faithless-official" },
            "publisher_metadata": { "artist": "Faithless", "album_title": "Reverence", "isrc": "GBARL9600049" }
        }))
        .unwrap();
        assert_eq!(t.id, "soundcloud:tracks:123");
        assert_eq!(
            (t.artist(), t.album.as_str(), t.isrc.as_str()),
            ("Faithless".into(), "Reverence", "GBARL9600049")
        );
        assert_eq!(t.duration, 520.0);
        assert_eq!(t.page, "https://soundcloud.com/faithless/insomnia");

        // Without publisher details the uploader is the artist.
        let t = parse_track(
            &json!({ "urn": "soundcloud:tracks:5", "title": "Edit", "user": { "username": "dj" } }),
        )
        .unwrap();
        assert_eq!((t.artist(), t.isrc.as_str()), ("dj".into(), ""));
    }
}
