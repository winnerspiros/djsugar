//! Apple Music browsing: sign in, then list the library's playlists and
//! songs.
//!
//! Like Spotify (see [`crate::spotify`]), Apple Music gives track details
//! but no audio for a DJ app, so a song plays only when the collection holds
//! the same track.
//!
//! Apple's API needs two tokens. The developer token is a JWT that DJ Sugar
//! signs with the DJ's own MusicKit key (a `.p8` file from their Apple
//! Developer account, with its Key ID and their Team ID). The music user
//! token comes from MusicKit JS: DJ Sugar serves a small sign-in page on
//! [`PORT`], which asks Apple for it and hands it back at `/callback`.
//! The Team ID, Key ID, key path and user token are kept in
//! `applemusic.json` in the config folder; the user token lasts about six
//! months.
//!
//! Network calls run on a worker thread, which speaks the same [`Request`]s
//! and [`Event`]s as Spotify's.

use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};
use serde_json::{json, Value};

use crate::spotify::{
    encode, open_in_browser, random_token, wait_for_code_after, Event, Playlist, Request, Saved,
    Service, StreamTrack, LIKED, PORT,
};

const API: &str = "https://api.music.apple.com";
const MUSICKIT: &str = "https://js-cdn.music.apple.com/musickit/v3/musickit.js";
/// Where the sign-in is kept, in the config folder.
const SAVED: &str = "applemusic.json";
/// How long a developer token DJ Sugar signs is good for.
const TOKEN_LIFETIME: u64 = 12 * 60 * 60;
/// The most songs a list reads.
const MAX_TRACKS: usize = 2000;

/// Starts Apple Music's worker. With a saved sign-in it reconnects straight
/// away.
pub fn start() -> Service {
    let (requests, rx) = channel();
    let (tx, events) = channel();
    let saved = Saved::load(SAVED).filter(|_| crate::browser::STREAMING_ENABLED);
    let team = saved
        .as_ref()
        .map(|s| s.client_id.clone())
        .unwrap_or_default();
    std::thread::Builder::new()
        .name("apple music".into())
        .spawn(move || Worker::new(tx).run(rx, saved))
        .expect("spawn apple music thread");
    Service::new(requests, events, team)
}

/// The DJ's MusicKit key.
#[derive(Clone, Default, PartialEq, Debug)]
struct Key {
    team: String,
    id: String,
    /// The `.p8` file.
    path: String,
}

impl Key {
    /// The Team ID, Key ID and key path as the sign-in form sends them, one
    /// a line; empty ones are taken from `saved`.
    fn from_form(typed: &str, saved: &Key) -> Key {
        let mut lines = typed.lines().map(str::trim);
        let mut field = |old: &str| {
            let v = lines.next().unwrap_or_default();
            if v.is_empty() { old } else { v }.to_string()
        };
        Key {
            team: field(&saved.team),
            id: field(&saved.id),
            path: field(&saved.path),
        }
    }

    fn saved(&self, user_token: &str) -> Saved {
        Saved {
            client_id: self.team.clone(),
            refresh_token: user_token.to_string(),
            secret: format!("{}\n{}", self.id, self.path),
        }
    }

    fn from_saved(s: &Saved) -> Key {
        let (id, path) = s.secret.split_once('\n').unwrap_or((&s.secret, ""));
        Key {
            team: s.client_id.clone(),
            id: id.to_string(),
            path: path.to_string(),
        }
    }

    /// A developer token signed with the key.
    fn developer_token(&self) -> Result<String, String> {
        if self.team.is_empty() || self.id.is_empty() || self.path.is_empty() {
            return Err(
                "Enter your Team ID, MusicKit Key ID and the path to its .p8 file first.".into(),
            );
        }
        let pem = std::fs::read_to_string(expand_home(&self.path))
            .map_err(|e| format!("Couldn't read the key file {}: {e}", self.path))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        sign_jwt(&pem, &self.team, &self.id, now)
    }
}

/// `~/` at the start of a path, as the home folder.
fn expand_home(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => path.to_string(),
    }
}

/// An ES256 JWT for Apple's API, from a PKCS#8 PEM key.
fn sign_jwt(pem: &str, team: &str, key_id: &str, now: u64) -> Result<String, String> {
    let der: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .map(str::trim)
        .collect();
    let der = STANDARD
        .decode(der)
        .map_err(|_| "The key file isn't a .p8 key.".to_string())?;
    let rng = SystemRandom::new();
    let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &der, &rng)
        .map_err(|_| "The key file isn't a MusicKit .p8 key.".to_string())?;
    let header = json!({ "alg": "ES256", "kid": key_id });
    let claims = json!({ "iss": team, "iat": now, "exp": now + TOKEN_LIFETIME });
    let signing_input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    );
    let signature = key
        .sign(&rng, signing_input.as_bytes())
        .map_err(|_| "Couldn't sign with the key.".to_string())?;
    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(signature.as_ref())
    ))
}

struct Session {
    key: Key,
    developer_token: String,
    user_token: String,
}

struct Worker {
    agent: ureq::Agent,
    events: Sender<Event>,
    session: Option<Session>,
    /// The key from last time, for fields the DJ leaves empty.
    saved_key: Key,
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
            saved_key: Key::default(),
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    fn run(mut self, requests: Receiver<Request>, saved: Option<Saved>) {
        if let Some(saved) = &saved {
            self.saved_key = Key::from_saved(saved);
        }
        if let Some(saved) = saved.filter(|s| !s.refresh_token.is_empty()) {
            self.emit(Event::Status("Signing in…".into()));
            let key = Key::from_saved(&saved);
            let result = key.developer_token().and_then(|developer_token| {
                self.session = Some(Session {
                    key,
                    developer_token,
                    user_token: saved.refresh_token,
                });
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
                    let key = Key::from_form(&typed, &self.saved_key);
                    let result = self.login(key).and_then(|s| {
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
                        s.key.saved("").store(SAVED);
                        self.saved_key = s.key;
                    }
                    self.emit(Event::Disconnected);
                }
            }
        }
    }

    /// Lists the library's playlists and reports the connection. Apple
    /// gives no name for the account.
    fn connected(&mut self) -> Result<(), String> {
        let mut playlists = vec![Playlist {
            key: LIKED.into(),
            name: "Songs".into(),
            ..Default::default()
        }];
        let items = self.get_all(&format!("{API}/v1/me/library/playlists"), MAX_TRACKS)?;
        playlists.extend(items.iter().filter_map(parse_playlist));
        self.emit(Event::Connected {
            user: "Apple Music".into(),
            playlists,
        });
        Ok(())
    }

    /// A playlist's songs (or the library's, for [`LIKED`]), in order, with
    /// their catalog entries for ISRCs.
    fn tracks(&mut self, key: &str) -> Result<Vec<StreamTrack>, String> {
        let url = if key == LIKED {
            format!("{API}/v1/me/library/songs?include=catalog")
        } else {
            format!(
                "{API}/v1/me/library/playlists/{}/tracks?include=catalog",
                encode(key)
            )
        };
        let items = self.get_all(&url, MAX_TRACKS)?;
        Ok(items.iter().filter_map(parse_song).collect())
    }

    /// Signs a developer token, then has the DJ sign in on DJ Sugar's page.
    fn login(&mut self, key: Key) -> Result<Session, String> {
        let developer_token = key.developer_token()?;
        let listener = TcpListener::bind(("127.0.0.1", PORT))
            .map_err(|e| format!("Couldn't listen on port {PORT} for Apple Music's reply: {e}"))?;
        let state = random_token(12);
        let page = sign_in_page(&developer_token, &state);
        let url = format!("http://127.0.0.1:{PORT}/start");
        if !open_in_browser(&url) {
            eprintln!("Open this address to sign in to Apple Music: {url}");
        }
        self.emit(Event::Status("Finish signing in in your browser…".into()));
        let user_token = wait_for_code_after(&listener, &state, "Apple Music", Some(&page))?;
        key.saved(&user_token).store(SAVED);
        self.saved_key = key.clone();
        Ok(Session {
            key,
            developer_token,
            user_token,
        })
    }

    /// A GET against the API, signing a new developer token when the old
    /// one runs out and waiting out rate limits.
    fn get(&mut self, url: &str) -> Result<Value, String> {
        let mut renewed = false;
        for _ in 0..5 {
            let s = self
                .session
                .as_ref()
                .ok_or("Not signed in to Apple Music")?;
            let mut response = self
                .agent
                .get(url)
                .header("Authorization", &format!("Bearer {}", s.developer_token))
                .header("Music-User-Token", &s.user_token)
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
                // The developer token ran out: sign a new one.
                401 if !renewed => {
                    let token = s.key.developer_token()?;
                    if let Some(s) = self.session.as_mut() {
                        s.developer_token = token;
                    }
                    renewed = true;
                }
                429 => std::thread::sleep(Duration::from_secs(retry_after.unwrap_or(2).min(30))),
                _ => return Err(error_message(status, &body)),
            }
        }
        Err("Apple Music kept refusing the request".into())
    }

    /// Follows `next` until the end or `max` items.
    fn get_all(&mut self, url: &str, max: usize) -> Result<Vec<Value>, String> {
        let mut items = Vec::new();
        let join = if url.contains('?') { '&' } else { '?' };
        let mut next = Some(format!("{url}{join}limit=100"));
        while let Some(url) = next {
            let page = self.get(&url)?;
            let data = page["data"].as_array().cloned().unwrap_or_default();
            if data.is_empty() {
                break;
            }
            items.extend(data);
            if items.len() >= max {
                items.truncate(max);
                break;
            }
            next = page["next"].as_str().map(|n| format!("{API}{n}"));
        }
        Ok(items)
    }
}

fn error_message(status: u16, body: &Value) -> String {
    let detail = body["errors"][0]["detail"]
        .as_str()
        .or(body["errors"][0]["title"].as_str());
    match (status, detail) {
        (401 | 403, _) => "Apple Music signed you out. Sign in again.".into(),
        (_, Some(d)) => format!("Apple Music said: {d}"),
        _ => format!("Apple Music answered {status}"),
    }
}

/// The page that asks MusicKit JS for the music user token and sends it to
/// `/callback` as the code.
fn sign_in_page(developer_token: &str, state: &str) -> String {
    format!(
        r#"<!doctype html><meta charset=utf-8><title>DJ Sugar</title>
<body style="font:16px -apple-system,sans-serif;background:#1c1c1e;color:#eee;display:grid;place-items:center;height:90vh">
<div style="text-align:center"><p id=msg>Sign in to Apple Music to see your library in DJ Sugar.</p>
<button id=go disabled style="font:inherit;padding:8px 20px;border-radius:16px;border:0;background:#fa2d48;color:#fff">Sign in with Apple Music</button></div>
<script src="{MUSICKIT}" async></script>
<script>
const back = (k, v) => location.href = "/callback?" + k + "=" + encodeURIComponent(v) + "&state={state}";
document.addEventListener("musickitloaded", async () => {{
  try {{
    await MusicKit.configure({{ developerToken: "{developer_token}", app: {{ name: "DJ Sugar", build: "1" }} }});
    const go = document.getElementById("go");
    go.disabled = false;
    go.onclick = async () => {{
      try {{ back("code", await MusicKit.getInstance().authorize()); }}
      catch (e) {{ back("error", "access_denied"); }}
    }};
  }} catch (e) {{ back("error", String(e)); }}
}});
</script>"#
    )
}

fn parse_playlist(v: &Value) -> Option<Playlist> {
    Some(Playlist {
        key: v["id"].as_str()?.to_string(),
        name: v["attributes"]["name"]
            .as_str()
            .unwrap_or("Playlist")
            .to_string(),
        total: 0,
        version: v["attributes"]["lastModifiedDate"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    })
}

/// A library song as a list row, with its catalog entry's ISRC and page
/// when Apple included one.
fn parse_song(v: &Value) -> Option<StreamTrack> {
    let a = &v["attributes"];
    let catalog = &v["relationships"]["catalog"]["data"][0];
    let catalog_id = catalog["id"]
        .as_str()
        .or(a["playParams"]["catalogId"].as_str());
    let page = catalog["attributes"]["url"]
        .as_str()
        .map(str::to_string)
        .or_else(|| catalog_id.map(|id| format!("https://music.apple.com/song/{id}")))
        .unwrap_or_else(|| "https://music.apple.com/library".into());
    Some(StreamTrack {
        id: v["id"].as_str()?.to_string(),
        page,
        isrc: catalog["attributes"]["isrc"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        title: a["name"].as_str()?.to_string(),
        artists: a["artistName"]
            .as_str()
            .map(|n| vec![n.to_string()])
            .unwrap_or_default(),
        album: a["albumName"].as_str().unwrap_or_default().to_string(),
        duration: a["durationInMillis"].as_f64().unwrap_or(0.0) / 1000.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{KeyPair, UnparsedPublicKey, ECDSA_P256_SHA256_FIXED};

    #[test]
    fn form_fields_fall_back_to_the_saved_key() {
        let saved = Key {
            team: "T1".into(),
            id: "K1".into(),
            path: "~/AuthKey.p8".into(),
        };
        assert_eq!(
            Key::from_form("T2\n\n/keys/new.p8", &saved),
            Key {
                team: "T2".into(),
                id: "K1".into(),
                path: "/keys/new.p8".into()
            }
        );
        assert_eq!(Key::from_saved(&saved.saved("tok")), saved);
    }

    #[test]
    fn signs_a_developer_token() {
        let rng = SystemRandom::new();
        let der = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            STANDARD.encode(der.as_ref())
        );
        let jwt = sign_jwt(&pem, "TEAM", "KEY", 1000).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(header, json!({ "alg": "ES256", "kid": "KEY" }));
        assert_eq!(claims["iss"], "TEAM");
        assert_eq!(claims["exp"], 1000 + TOKEN_LIFETIME);
        let pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der.as_ref(), &rng).unwrap();
        let public = UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, pair.public_key().as_ref());
        let signature = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        assert!(public
            .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature)
            .is_ok());
        assert!(sign_jwt("not a key", "T", "K", 0).is_err());
    }

    #[test]
    fn reads_playlists_and_songs() {
        let p = parse_playlist(&json!({ "id": "p.abc", "type": "library-playlists",
            "attributes": { "name": "Warm up" } }))
        .unwrap();
        assert_eq!((p.key.as_str(), p.name.as_str()), ("p.abc", "Warm up"));

        let t = parse_song(&json!({
            "id": "i.xyz", "type": "library-songs",
            "attributes": { "name": "Insomnia", "artistName": "Faithless",
                            "albumName": "Reverence", "durationInMillis": 520000,
                            "playParams": { "catalogId": "1440" } },
            "relationships": { "catalog": { "data": [{ "id": "1440", "type": "songs",
                "attributes": { "isrc": "GBARL9600049",
                                "url": "https://music.apple.com/us/album/insomnia/1?i=1440" } }] } }
        }))
        .unwrap();
        assert_eq!(
            (t.title.as_str(), t.artist(), t.album.as_str()),
            ("Insomnia", "Faithless".into(), "Reverence")
        );
        assert_eq!((t.duration, t.isrc.as_str()), (520.0, "GBARL9600049"));
        assert_eq!(t.page, "https://music.apple.com/us/album/insomnia/1?i=1440");

        // Without a catalog entry: no ISRC, a page from the catalog id.
        let t = parse_song(&json!({ "id": "i.2", "attributes": { "name": "Edit",
            "playParams": { "catalogId": "77" } } }))
        .unwrap();
        assert_eq!(
            (t.isrc.as_str(), t.page.as_str()),
            ("", "https://music.apple.com/song/77")
        );
    }

    #[test]
    fn the_page_sends_the_token_back() {
        let page = sign_in_page("dev.token", "st8");
        assert!(page.contains(r#"developerToken: "dev.token""#));
        assert!(page.contains("&state=st8"));
        assert!(page.contains(MUSICKIT));
    }
}
