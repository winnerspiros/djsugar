//! TIDAL browsing: sign in, then list the playlists and favourite tracks.
//!
//! Like Spotify (see [`crate::spotify`]), TIDAL's API gives track details but
//! no audio for a DJ app, so a TIDAL track plays only when the collection
//! holds the same track.
//!
//! Sign-in uses OAuth with PKCE and the DJ's own client ID: they register an
//! app at developer.tidal.com with [`REDIRECT_URI`] as its redirect URI and
//! the scopes in [`SCOPES`]. The client ID and refresh token are kept in
//! `tidal.json` in the config folder, so the next start signs in by itself.
//! The API is JSON:API at `openapi.tidal.com/v2`, paged by `links.next`.
//!
//! Network calls run on a worker thread, which speaks the same [`Request`]s
//! and [`Event`]s as Spotify's.

use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::spotify::{
    code_challenge, encode, open_in_browser, random_token, wait_for_code, Event, Playlist, Request,
    Saved, Service, StreamTrack, LIKED, PORT, REDIRECT_URI,
};

const LOGIN: &str = "https://login.tidal.com/authorize";
const TOKEN: &str = "https://auth.tidal.com/v1/oauth2/token";
const API: &str = "https://openapi.tidal.com/v2";
/// Reading the DJ's name, own playlists, and saved playlists and tracks.
pub const SCOPES: &str = "user.read playlists.read collection.read";
/// Where the sign-in is kept, in the config folder.
const SAVED: &str = "tidal.json";
/// How many tracks `/tracks` takes at once.
const TRACK_BATCH: usize = 20;

/// Starts TIDAL's worker. With a saved sign-in it reconnects straight away.
pub fn start() -> Service {
    let (requests, rx) = channel();
    let (tx, events) = channel();
    let saved = Saved::load(SAVED).filter(|_| crate::browser::STREAMING_ENABLED);
    let client_id = std::env::var("DJSUGAR_TIDAL_CLIENT_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| saved.as_ref().map(|s| s.client_id.clone()))
        .unwrap_or_default();
    std::thread::Builder::new()
        .name("tidal".into())
        .spawn(move || Worker::new(tx).run(rx, saved))
        .expect("spawn tidal thread");
    Service::new(requests, events, client_id)
}

struct Session {
    client_id: String,
    access_token: String,
    refresh_token: String,
    expires: Instant,
}

struct Worker {
    agent: ureq::Agent,
    events: Sender<Event>,
    session: Option<Session>,
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
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    fn run(mut self, requests: Receiver<Request>, saved: Option<Saved>) {
        if let Some(saved) = saved.filter(|s| !s.refresh_token.is_empty()) {
            self.emit(Event::Status("Signing in…".into()));
            let result = self
                .refresh(&saved.client_id, &saved.refresh_token)
                .and_then(|s| {
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
                Request::Connect(client_id) => {
                    let result = self.login(client_id.trim()).and_then(|s| {
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
                Request::Lookup(ids) => match self.lookup(&ids) {
                    Ok(found) => self.emit(Event::Found(found.into_values().collect())),
                    Err(e) => eprintln!("TIDAL: can't look tracks up: {e}"),
                },
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
                            client_id: s.client_id,
                            refresh_token: String::new(),
                            ..Default::default()
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
        let me = self.get(&format!("{API}/users/me"))?;
        let a = &me["data"]["attributes"];
        let user = [&a["firstName"], &a["username"], &a["email"]]
            .iter()
            .find_map(|v| v.as_str().filter(|s| !s.is_empty()))
            .unwrap_or("you")
            .to_string();
        let mut playlists = vec![Playlist {
            key: LIKED.into(),
            name: "My Tracks".into(),
            ..Default::default()
        }];
        // Their own playlists, then those they saved.
        let (own, _) = self.get_all(&format!("{API}/playlists?filter%5Bowners.id%5D=me"))?;
        playlists.extend(own.iter().filter_map(parse_playlist));
        let (_, saved) = self.get_all(&format!(
            "{API}/userCollectionPlaylists/me/relationships/items?include=items"
        ))?;
        for p in saved.iter().filter_map(parse_playlist) {
            if !playlists.iter().any(|o| o.key == p.key) {
                playlists.push(p);
            }
        }
        self.emit(Event::Connected { user, playlists });
        Ok(())
    }

    /// A playlist's tracks (or the saved tracks, for [`LIKED`]), in order.
    fn tracks(&mut self, key: &str) -> Result<Vec<StreamTrack>, String> {
        let url = if key == LIKED {
            format!("{API}/userCollectionTracks/me/relationships/items")
        } else {
            format!("{API}/playlists/{}/relationships/items", encode(key))
        };
        let (items, _) = self.get_all(&url)?;
        let ids: Vec<String> = items
            .iter()
            .filter(|i| i["type"] == "tracks")
            .filter_map(|i| i["id"].as_str().map(str::to_string))
            .collect();
        let found = self.lookup(&ids)?;
        Ok(ids.iter().filter_map(|id| found.get(id).cloned()).collect())
    }

    /// Tracks by their ids, with their artists and albums, a batch at a time.
    fn lookup(&mut self, ids: &[String]) -> Result<HashMap<String, StreamTrack>, String> {
        let mut found: HashMap<String, StreamTrack> = HashMap::new();
        for batch in ids.chunks(TRACK_BATCH) {
            let mut url = format!("{API}/tracks?include=artists&include=albums");
            for id in batch {
                url.push_str("&filter%5Bid%5D=");
                url.push_str(&encode(id));
            }
            let page = self.get(&url)?;
            for t in parse_tracks(&page) {
                found.insert(t.id.clone(), t);
            }
        }
        Ok(found)
    }

    fn login(&mut self, client_id: &str) -> Result<Session, String> {
        if client_id.is_empty() {
            return Err("Enter your TIDAL app's client ID first.".into());
        }
        let listener = TcpListener::bind(("127.0.0.1", PORT))
            .map_err(|e| format!("Couldn't listen on port {PORT} for TIDAL's reply: {e}"))?;
        let verifier = random_token(48);
        let state = random_token(12);
        let url = authorize_url(client_id, &code_challenge(&verifier), &state);
        if !open_in_browser(&url) {
            eprintln!("Open this address to sign in to TIDAL: {url}");
        }
        self.emit(Event::Status("Finish signing in in your browser…".into()));
        let code = wait_for_code(&listener, &state, "TIDAL")?;
        let response = self.token(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", client_id),
            ("code_verifier", &verifier),
            ("scope", SCOPES),
        ])?;
        self.session_from(client_id, response, None)
    }

    fn refresh(&self, client_id: &str, refresh_token: &str) -> Result<Session, String> {
        let response = self.token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("scope", SCOPES),
        ])?;
        self.session_from(client_id, response, Some(refresh_token))
    }

    fn token(&self, form: &[(&str, &str)]) -> Result<Value, String> {
        let mut response = self
            .agent
            .post(TOKEN)
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

    /// TIDAL may hand out a new refresh token; the old one stays valid otherwise.
    fn session_from(
        &self,
        client_id: &str,
        v: Value,
        old_refresh: Option<&str>,
    ) -> Result<Session, String> {
        let access_token = v["access_token"]
            .as_str()
            .ok_or("TIDAL sent no access token")?
            .to_string();
        let refresh_token = v["refresh_token"]
            .as_str()
            .or(old_refresh)
            .unwrap_or_default()
            .to_string();
        let lifetime = v["expires_in"].as_u64().unwrap_or(3600);
        Saved {
            client_id: client_id.to_string(),
            refresh_token: refresh_token.clone(),
            ..Default::default()
        }
        .store(SAVED);
        Ok(Session {
            client_id: client_id.to_string(),
            access_token,
            refresh_token,
            expires: Instant::now() + Duration::from_secs(lifetime),
        })
    }

    fn renew(&mut self) -> Result<(), String> {
        let s = self.session.as_ref().ok_or("Not signed in to TIDAL")?;
        let fresh = self.refresh(&s.client_id, &s.refresh_token)?;
        self.session = Some(fresh);
        Ok(())
    }

    /// A GET against the API, renewing the token and waiting out rate limits.
    fn get(&mut self, url: &str) -> Result<Value, String> {
        let mut renewed = false;
        for _ in 0..5 {
            let s = self.session.as_ref().ok_or("Not signed in to TIDAL")?;
            if !renewed && s.expires < Instant::now() + Duration::from_secs(60) {
                self.renew()?;
                renewed = true;
                continue;
            }
            let mut response = self
                .agent
                .get(url)
                .header("Authorization", &format!("Bearer {}", s.access_token))
                .header("Accept", "application/vnd.api+json")
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
        Err("TIDAL kept refusing the request".into())
    }

    /// Follows `links.next` to the end: every page's `data`, and every
    /// page's `included` resources.
    fn get_all(&mut self, url: &str) -> Result<(Vec<Value>, Vec<Value>), String> {
        let (mut data, mut included) = (Vec::new(), Vec::new());
        let mut next = Some(url.to_string());
        while let Some(url) = next {
            let page = self.get(&url)?;
            if let Some(a) = page["data"].as_array() {
                data.extend(a.iter().cloned());
            }
            if let Some(a) = page["included"].as_array() {
                included.extend(a.iter().cloned());
            }
            next = page["links"]["next"].as_str().map(absolute);
        }
        Ok((data, included))
    }
}

/// A `links.next` as a full address: the API gives one relative to its host
/// or to `/v2`.
fn absolute(link: &str) -> String {
    if link.starts_with("http") {
        link.to_string()
    } else if link.starts_with("/v2/") {
        format!("https://openapi.tidal.com{link}")
    } else {
        format!("{API}/{}", link.trim_start_matches('/'))
    }
}

fn error_message(status: u16, body: &Value) -> String {
    let detail = body["errors"][0]["detail"]
        .as_str()
        .or(body["error_description"].as_str())
        .or(body["error"].as_str());
    match (status, detail) {
        (401, _) => "TIDAL signed you out. Sign in again.".into(),
        (_, Some(d)) => format!("TIDAL said: {d}"),
        _ => format!("TIDAL answered {status}"),
    }
}

fn authorize_url(client_id: &str, challenge: &str, state: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT_URI),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge),
        ("scope", SCOPES),
        ("state", state),
    ];
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect();
    format!("{LOGIN}?{}", query.join("&"))
}

/// A playlist resource; anything else (a folder) is none.
fn parse_playlist(v: &Value) -> Option<Playlist> {
    if v["type"] != "playlists" {
        return None;
    }
    let a = &v["attributes"];
    Some(Playlist {
        key: v["id"].as_str()?.to_string(),
        name: a["name"].as_str().unwrap_or("Playlist").to_string(),
        total: a["numberOfItems"].as_u64().unwrap_or(0) as usize,
        version: a["lastModifiedAt"].as_str().unwrap_or_default().to_string(),
    })
}

/// The tracks of a `/tracks` document, their artists and album names taken
/// from what it included.
fn parse_tracks(doc: &Value) -> Vec<StreamTrack> {
    let mut names: HashMap<(String, String), String> = HashMap::new();
    for r in doc["included"].as_array().into_iter().flatten() {
        let (Some(kind), Some(id)) = (r["type"].as_str(), r["id"].as_str()) else {
            continue;
        };
        let a = &r["attributes"];
        if let Some(n) = a["name"].as_str().or(a["title"].as_str()) {
            names.insert((kind.to_string(), id.to_string()), n.to_string());
        }
    }
    let linked = |t: &Value, rel: &str, kind: &str| -> Vec<String> {
        t["relationships"][rel]["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l["id"].as_str())
            .filter_map(|id| names.get(&(kind.to_string(), id.to_string())).cloned())
            .collect()
    };
    doc["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["type"] == "tracks")
        .filter_map(|t| {
            let id = t["id"].as_str()?.to_string();
            let a = &t["attributes"];
            let mut title = a["title"].as_str()?.to_string();
            // "Song" and version "Extended Mix" are "Song (Extended Mix)".
            if let Some(v) = a["version"].as_str().filter(|v| !v.is_empty()) {
                title = format!("{title} ({v})");
            }
            Some(StreamTrack {
                page: format!("https://tidal.com/browse/track/{id}"),
                isrc: a["isrc"].as_str().unwrap_or_default().to_string(),
                artists: linked(t, "artists", "artists"),
                album: linked(t, "albums", "albums")
                    .into_iter()
                    .next()
                    .unwrap_or_default(),
                duration: iso_seconds(a["duration"].as_str().unwrap_or_default()),
                title,
                id,
            })
        })
        .collect()
}

/// Seconds in an ISO 8601 duration such as "PT3M5S" or "PT1H2M3.5S".
fn iso_seconds(s: &str) -> f64 {
    let Some(time) = s.split('T').nth(1) else {
        return 0.0;
    };
    let mut total = 0.0;
    let mut number = String::new();
    for c in time.chars() {
        match c {
            'H' | 'M' | 'S' => {
                let n: f64 = number.parse().unwrap_or(0.0);
                total += n * match c {
                    'H' => 3600.0,
                    'M' => 60.0,
                    _ => 1.0,
                };
                number.clear();
            }
            c => number.push(c),
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn durations() {
        assert_eq!(iso_seconds("PT3M5S"), 185.0);
        assert_eq!(iso_seconds("PT1H2M3.5S"), 3723.5);
        assert_eq!(iso_seconds("PT58S"), 58.0);
        assert_eq!(iso_seconds(""), 0.0);
    }

    #[test]
    fn next_pages() {
        assert_eq!(
            absolute("/playlists?page%5Bcursor%5D=x"),
            "https://openapi.tidal.com/v2/playlists?page%5Bcursor%5D=x"
        );
        assert_eq!(
            absolute("/v2/playlists?page%5Bcursor%5D=x"),
            "https://openapi.tidal.com/v2/playlists?page%5Bcursor%5D=x"
        );
    }

    #[test]
    fn builds_the_authorize_address() {
        let url = authorize_url("abc", "chal", "st");
        assert!(url.starts_with("https://login.tidal.com/authorize?client_id=abc"));
        assert!(url.contains("scope=user.read%20playlists.read%20collection.read"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn reads_playlists_and_tracks() {
        let p = parse_playlist(&json!({ "id": "p1", "type": "playlists",
            "attributes": { "name": "Warm up", "numberOfItems": 12 } }))
        .unwrap();
        assert_eq!(
            (p.key.as_str(), p.name.as_str(), p.total),
            ("p1", "Warm up", 12)
        );
        assert!(parse_playlist(&json!({ "id": "f", "type": "userCollectionFolders" })).is_none());

        let doc = json!({
            "data": [{
                "id": "75413016", "type": "tracks",
                "attributes": { "title": "Insomnia", "version": "Extended Mix",
                                "isrc": "GBARL9600049", "duration": "PT8M40S" },
                "relationships": {
                    "artists": { "data": [{ "id": "a1", "type": "artists" }] },
                    "albums": { "data": [{ "id": "b1", "type": "albums" }] }
                }
            }],
            "included": [
                { "id": "a1", "type": "artists", "attributes": { "name": "Faithless" } },
                { "id": "b1", "type": "albums", "attributes": { "title": "Reverence" } }
            ]
        });
        let t = &parse_tracks(&doc)[0];
        assert_eq!(t.title, "Insomnia (Extended Mix)");
        assert_eq!(
            (t.artist(), t.album.as_str()),
            ("Faithless".into(), "Reverence")
        );
        assert_eq!((t.duration, t.isrc.as_str()), (520.0, "GBARL9600049"));
        assert_eq!(t.page, "https://tidal.com/browse/track/75413016");
    }
}
