//! A Jellyfin (or Emby) server as a source in the rail: the DJ's own music
//! searched by album, artist, song or playlist, played from the cache and
//! brought into the collection as the original files.
//!
//! DJ Sugar signs in with the username and password at
//! `/Users/AuthenticateByName` and keeps the access token it gets back for
//! this run. Calls carry it in an `X-Emby-Token` header, and addresses the
//! app fetches by themselves (downloads, covers) carry it as `api_key`.
//! Both Jellyfin and Emby read both.

use std::sync::Mutex;

use serde_json::{json, Value};

use crate::server::{self, encode, matches, Results, Server, ServerSource, Song};
use crate::source::{Found, QueryKind, SearchSource, SourceInfo, SourceQuery};
use crate::{Manifest, Plugin, PluginConfig, Setting, SettingKind};

const MANIFEST: Manifest = Manifest {
    id: "jellyfin",
    name: "Jellyfin",
    // The DJ's own server, at the address they fill in.
    hosts: &[],
    downloads: true,
};

/// How big list covers are fetched, in pixels.
const THUMB: u32 = 96;

/// Jellyfin's runtimes are in ticks of 100 ns.
const TICKS_PER_SECOND: f64 = 10_000_000.0;

/// A triangle in a rounded square, as a generic rail icon.
const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 30"><rect width="30" height="30" rx="7" fill="#101a2b"/><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#aa5cc3"/><stop offset="1" stop-color="#00a4dc"/></linearGradient></defs><path d="M15 6.5 24 22H6z" fill="none" stroke="url(#g)" stroke-width="2.4" stroke-linejoin="round"/><path d="M15 13 18.6 19.5h-7.2z" fill="url(#g)"/></svg>"##;

pub struct Jellyfin {
    source: ServerSource<JellyfinServer>,
}

impl Default for Jellyfin {
    fn default() -> Self {
        Self::new()
    }
}

impl Jellyfin {
    pub fn new() -> Self {
        Self {
            source: ServerSource::new("jellyfin", JellyfinServer::default()),
        }
    }
}

impl Plugin for Jellyfin {
    fn manifest(&self) -> &Manifest {
        &MANIFEST
    }

    fn source(&mut self) -> Option<&mut dyn SearchSource> {
        Some(&mut self.source)
    }

    fn settings(&self) -> Vec<Setting> {
        vec![
            Setting::new(
                "",
                SettingKind::Note,
                "Your own Jellyfin or Emby server",
                "Searches open the albums, artists and playlists in its music libraries; \
                 tracks play from a cache or come into your collection as the original files.",
            ),
            Setting::new(
                "server",
                SettingKind::Text,
                "Server address",
                "For example http://192.168.1.20:8096",
            ),
            Setting::new("user", SettingKind::Text, "Username", ""),
            Setting::new(
                "password",
                SettingKind::Secret,
                "Password",
                "Sent once to sign in; DJ Sugar then uses the server's access token.",
            ),
        ]
        .into_iter()
        .chain(server::storage_settings(MANIFEST.name))
        .collect()
    }

    fn configure(&mut self, config: &PluginConfig) {
        let s = &mut self.source.server;
        s.url = server::server(config.get("server"));
        s.user = config.get("user").trim().to_string();
        s.password = config.get("password").to_string();
        // Signed in again with what was filled in, when next needed.
        s.session = Mutex::new(None);
    }
}

/// A sign-in: the access token and the user's id.
#[derive(Clone, Debug, PartialEq)]
struct Session {
    token: String,
    user_id: String,
}

#[derive(Default)]
pub(crate) struct JellyfinServer {
    url: String,
    user: String,
    password: String,
    session: Mutex<Option<Session>>,
}

/// How DJ Sugar introduces itself to the server.
fn client_header(token: Option<&str>) -> String {
    let mut h = format!(
        "MediaBrowser Client=\"DJ Sugar\", Device=\"DJ Sugar\", DeviceId=\"djsugar\", Version=\"{}\"",
        env!("CARGO_PKG_VERSION")
    );
    if let Some(t) = token {
        h.push_str(&format!(", Token=\"{t}\""));
    }
    h
}

impl JellyfinServer {
    /// Signs in, or the sign-in from earlier this run.
    fn session(&self) -> Result<Session, String> {
        if let Some(s) = self.session.lock().ok().and_then(|s| s.clone()) {
            return Ok(s);
        }
        let mut response = server::agent()
            .post(&format!("{}/Users/AuthenticateByName", self.url))
            .header("Authorization", &client_header(None))
            .header("X-Emby-Authorization", &client_header(None))
            .send_json(json!({ "Username": self.user, "Pw": self.password }))
            .map_err(|e| format!("Couldn't reach the server: {e}"))?;
        match response.status().as_u16() {
            401 | 403 => return Err("The server didn't accept the username or password.".into()),
            s if s >= 400 => return Err(format!("The server answered {s}.")),
            _ => {}
        }
        let v: Value = response
            .body_mut()
            .read_json()
            .map_err(|_| "That address isn't a Jellyfin or Emby server.".to_string())?;
        let session = parse_session(&v).ok_or("The server didn't sign in.")?;
        if let Ok(mut s) = self.session.lock() {
            *s = Some(session.clone());
        }
        Ok(session)
    }

    /// `path` on the server, as JSON. Signs in again once if the token was
    /// turned down.
    fn get(&self, path: &str) -> Result<Value, String> {
        for attempt in 0..2 {
            let session = self.session()?;
            let mut response = server::agent()
                .get(&format!("{}{path}", self.url))
                .header("X-Emby-Token", &session.token)
                .header("Authorization", &client_header(Some(&session.token)))
                .header("Accept", "application/json")
                .call()
                .map_err(|e| format!("Couldn't reach the server: {e}"))?;
            match response.status().as_u16() {
                401 if attempt == 0 => {
                    if let Ok(mut s) = self.session.lock() {
                        *s = None;
                    }
                    continue;
                }
                401 => return Err("The server didn't accept the sign-in.".into()),
                s if s >= 400 => return Err(format!("The server answered {s}.")),
                _ => {}
            }
            return response
                .body_mut()
                .read_json()
                .map_err(|_| "The server's answer wasn't JSON.".to_string());
        }
        Err("The server didn't accept the sign-in.".into())
    }

    /// The user's items matching `query` (already encoded), with what a
    /// song needs to download.
    fn items(&self, query: &str) -> Result<Vec<Value>, String> {
        let user = self.session()?.user_id;
        let v = self.get(&format!(
            "/Users/{}/Items?Recursive=true&Fields=MediaSources,ProductionYear,ChildCount&{query}",
            encode(&user)
        ))?;
        Ok(list(&v["Items"]))
    }

    /// An address the app fetches by itself, signed with the token.
    fn signed(&self, path: &str, token: &str) -> String {
        let sep = if path.contains('?') { '&' } else { '?' };
        format!("{}{path}{sep}api_key={}", self.url, encode(token))
    }

    fn picture(&self, id: &str, token: &str, size: Option<u32>) -> String {
        if id.is_empty() {
            return String::new();
        }
        let path = match size {
            Some(s) => format!("/Items/{}/Images/Primary?maxHeight={s}", encode(id)),
            None => format!("/Items/{}/Images/Primary", encode(id)),
        };
        self.signed(&path, token)
    }

    fn song(&self, v: &Value, token: &str) -> Option<Song> {
        if text(&v["Type"]) != "Audio" {
            return None;
        }
        let id = text(&v["Id"]);
        if id.is_empty() {
            return None;
        }
        let media = &v["MediaSources"][0];
        let suffix = match text(&media["Container"]) {
            c if !c.is_empty() => c,
            _ => text(&v["Container"]),
        };
        let album_artist = text(&v["AlbumArtist"]);
        let artists: Vec<String> = list(&v["Artists"]).iter().map(text).collect();
        let artist = if artists.is_empty() {
            album_artist.clone()
        } else {
            artists.join(", ")
        };
        // The album's picture, else the song's own.
        let cover_id = match text(&v["AlbumId"]) {
            a if !a.is_empty() && !text(&v["AlbumPrimaryImageTag"]).is_empty() => a,
            _ => id.clone(),
        };
        Some(Song {
            key: format!("song:{id}"),
            title: text(&v["Name"]),
            artist,
            album_artist,
            album: text(&v["Album"]),
            year: text(&v["ProductionYear"]),
            number: text(&v["IndexNumber"]),
            duration: v["RunTimeTicks"].as_f64().unwrap_or(0.0) / TICKS_PER_SECOND,
            suffix,
            bitrate: media["Bitrate"].as_u64().unwrap_or(0) / 1000,
            size: media["Size"].as_u64().unwrap_or(0),
            file_url: self.signed(&format!("/Items/{}/Download", encode(&id)), token),
            cover_url: self.picture(&cover_id, token, None),
            thumb_url: self.picture(&cover_id, token, Some(THUMB)),
        })
    }

    fn album(&self, v: &Value, token: &str) -> Option<Found> {
        let id = text(&v["Id"]);
        if id.is_empty() {
            return None;
        }
        let name = text(&v["Name"]);
        let playlist = text(&v["Type"]) == "Playlist";
        Some(Found {
            key: format!("{}:{id}", if playlist { "playlist" } else { "album" }),
            folder: true,
            artist: text(&v["AlbumArtist"]),
            title: name.clone(),
            album: name,
            year: text(&v["ProductionYear"]),
            duration: v["RunTimeTicks"].as_f64().unwrap_or(0.0) / TICKS_PER_SECOND,
            track_count: v["ChildCount"].as_u64().unwrap_or(0) as usize,
            from: if playlist {
                "Playlist".into()
            } else {
                MANIFEST.name.into()
            },
            cover_url: self.picture(&id, token, Some(THUMB)),
            ..Default::default()
        })
    }
}

impl Server for JellyfinServer {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            name: MANIFEST.name,
            icon_svg: ICON,
            hint: "An artist, an album, a song, or a playlist's name",
        }
    }

    fn describe(&self, query: &SourceQuery) -> Vec<String> {
        let q = query.text.trim();
        let line = match query.kind {
            QueryKind::Playlist if matches("", q) => "All your playlists".to_string(),
            QueryKind::Playlist => format!("Playlists called {q}"),
            _ if q.is_empty() => return Vec::new(),
            QueryKind::Artist => format!("Albums by artists called {q}"),
            QueryKind::Album => format!("Albums called {q}"),
            QueryKind::Track => format!("Songs called {q}"),
            QueryKind::Auto => format!("Albums and songs matching {q}"),
        };
        vec![
            line,
            "Plays from a cache; + Collection keeps the original files.".into(),
        ]
    }

    fn missing(&self) -> Option<String> {
        (self.url.is_empty() || self.user.is_empty())
            .then(|| "Fill in your server in Settings › Sources › Jellyfin first.".into())
    }

    fn search(&self, query: &SourceQuery) -> Result<Results, String> {
        let q = query.text.trim();
        let token = self.session()?.token;
        let term = encode(q);
        let mut out = Results::default();
        match query.kind {
            QueryKind::Playlist => {
                out.folders = self
                    .items("IncludeItemTypes=Playlist")?
                    .iter()
                    .filter(|p| matches(&text(&p["Name"]), q))
                    .filter(|p| matches!(text(&p["MediaType"]).as_str(), "Audio" | ""))
                    .filter_map(|p| self.album(p, &token))
                    .collect();
            }
            QueryKind::Artist => {
                let user = self.session()?.user_id;
                let v = self.get(&format!(
                    "/Artists?searchTerm={term}&Limit=5&UserId={}",
                    encode(&user)
                ))?;
                for a in list(&v["Items"]) {
                    let albums = self.items(&format!(
                        "IncludeItemTypes=MusicAlbum&ArtistIds={}&SortBy=ProductionYear,SortName",
                        encode(&text(&a["Id"]))
                    ))?;
                    out.folders
                        .extend(albums.iter().filter_map(|a| self.album(a, &token)));
                }
            }
            kind => {
                let (albums, songs) = match kind {
                    QueryKind::Album => (100, 0),
                    QueryKind::Track => (0, 200),
                    _ => (30, 100),
                };
                if albums > 0 {
                    out.folders = self
                        .items(&format!(
                            "IncludeItemTypes=MusicAlbum&searchTerm={term}&Limit={albums}"
                        ))?
                        .iter()
                        .filter_map(|a| self.album(a, &token))
                        .collect();
                }
                if songs > 0 {
                    out.songs = self
                        .items(&format!(
                            "IncludeItemTypes=Audio&searchTerm={term}&Limit={songs}"
                        ))?
                        .iter()
                        .filter_map(|s| self.song(s, &token))
                        .collect();
                }
            }
        }
        Ok(out)
    }

    fn folder(&self, key: &str) -> Result<Vec<Song>, String> {
        let token = self.session()?.token;
        let items = match key.split_once(':') {
            Some(("album", id)) => self.items(&format!(
                "IncludeItemTypes=Audio&ParentId={}&SortBy=ParentIndexNumber,IndexNumber,SortName",
                encode(id)
            ))?,
            Some(("playlist", id)) => {
                let user = self.session()?.user_id;
                let v = self.get(&format!(
                    "/Playlists/{}/Items?UserId={}&Fields=MediaSources,ProductionYear",
                    encode(id),
                    encode(&user)
                ))?;
                list(&v["Items"])
            }
            _ => return Err("Not a folder.".into()),
        };
        Ok(items.iter().filter_map(|s| self.song(s, &token)).collect())
    }
}

fn parse_session(v: &Value) -> Option<Session> {
    let token = text(&v["AccessToken"]);
    let user_id = text(&v["User"]["Id"]);
    (!token.is_empty() && !user_id.is_empty()).then_some(Session { token, user_id })
}

fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

/// A value as text: numbers as written.
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn server() -> JellyfinServer {
        JellyfinServer {
            url: "http://jf.lan:8096".into(),
            user: "dj".into(),
            password: "sesame".into(),
            session: Mutex::new(None),
        }
    }

    #[test]
    fn reads_sign_ins() {
        let s = parse_session(&json!({ "AccessToken": "tok", "User": { "Id": "u1" } }));
        assert_eq!(
            s,
            Some(Session {
                token: "tok".into(),
                user_id: "u1".into()
            })
        );
        assert_eq!(parse_session(&json!({ "AccessToken": "tok" })), None);
        assert!(client_header(Some("t")).ends_with(", Token=\"t\""));
    }

    #[test]
    fn reads_songs_and_albums() {
        let s = server();
        let song = s
            .song(
                &json!({
                    "Type": "Audio", "Id": "a1", "Name": "Kerala", "Artists": ["Bonobo"],
                    "AlbumArtist": "Bonobo", "Album": "Migration", "AlbumId": "al9",
                    "AlbumPrimaryImageTag": "x", "ProductionYear": 2017, "IndexNumber": 3,
                    "RunTimeTicks": 2_510_000_000u64,
                    "MediaSources": [{ "Container": "flac", "Size": 31000000, "Bitrate": 1011000 }]
                }),
                "tok",
            )
            .unwrap();
        assert_eq!(song.key, "song:a1");
        assert_eq!(song.duration, 251.0);
        assert_eq!(song.bitrate, 1011);
        assert_eq!(
            song.file_url,
            "http://jf.lan:8096/Items/a1/Download?api_key=tok"
        );
        assert_eq!(
            song.thumb_url,
            "http://jf.lan:8096/Items/al9/Images/Primary?maxHeight=96&api_key=tok"
        );
        // A path in the album's folder, with the system's separators.
        assert_eq!(
            Path::new(&song.found("Jellyfin").file),
            Path::new("Bonobo/Migration/03 Kerala.flac")
        );
        assert!(s
            .song(&json!({ "Type": "Movie", "Id": "m" }), "tok")
            .is_none());

        let album = s
            .album(
                &json!({ "Type": "MusicAlbum", "Id": "al9", "Name": "Migration",
                         "AlbumArtist": "Bonobo", "ChildCount": 12 }),
                "tok",
            )
            .unwrap();
        assert_eq!((album.key.as_str(), album.track_count), ("album:al9", 12));
        let playlist = s
            .album(
                &json!({ "Type": "Playlist", "Id": "p1", "Name": "Warm-up" }),
                "tok",
            )
            .unwrap();
        assert_eq!(playlist.key, "playlist:p1");
    }

    /// A stand-in Jellyfin server on a local port: a sign-in, one album
    /// with one song.
    fn mock_server() -> String {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut length = 0usize;
                let mut token = false;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap() == 0 || h == "\r\n" {
                        break;
                    }
                    let lower = h.to_lowercase();
                    if let Some(n) = lower.strip_prefix("content-length:") {
                        length = n.trim().parse().unwrap_or(0);
                    }
                    if lower.starts_with("x-emby-token: tok") {
                        token = true;
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let path = line.split(' ').nth(1).unwrap_or("").to_string();
                let song = json!({ "Type": "Audio", "Id": "a1", "Name": "Kerala",
                    "Artists": ["Bonobo"], "AlbumArtist": "Bonobo", "Album": "Migration",
                    "AlbumId": "al9", "AlbumPrimaryImageTag": "x", "IndexNumber": 3,
                    "MediaSources": [{ "Container": "flac", "Size": 5 }] });
                let (status, kind, out): (u16, &str, Vec<u8>) = if path
                    == "/Users/AuthenticateByName"
                {
                    let ok = String::from_utf8_lossy(&body).contains("sesame");
                    if ok {
                        (
                            200,
                            "application/json",
                            json!({ "AccessToken": "tok",
                                "User": { "Id": "u1" } })
                            .to_string()
                            .into_bytes(),
                        )
                    } else {
                        (401, "text/plain", b"no".to_vec())
                    }
                } else if path.starts_with("/Users/u1/Items") && !token {
                    (401, "text/plain", b"no".to_vec())
                } else if path.starts_with("/Users/u1/Items") && path.contains("MusicAlbum") {
                    (
                        200,
                        "application/json",
                        json!({ "Items": [{ "Type": "MusicAlbum",
                            "Id": "al9", "Name": "Migration", "AlbumArtist": "Bonobo" }] })
                        .to_string()
                        .into_bytes(),
                    )
                } else if path.starts_with("/Users/u1/Items") {
                    (
                        200,
                        "application/json",
                        json!({ "Items": [song] }).to_string().into_bytes(),
                    )
                } else if path.starts_with("/Items/a1/Download") && path.contains("api_key=tok") {
                    (200, "audio/flac", b"fLaC!".to_vec())
                } else if path.starts_with("/Items/al9/Images") {
                    (200, "image/jpeg", b"jpeg".to_vec())
                } else {
                    (404, "text/plain", b"no".to_vec())
                };
                let mut s = stream;
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    out.len()
                );
                let _ = s.write_all(&out);
            }
        });
        format!("127.0.0.1:{port}")
    }

    #[test]
    fn signs_in_searches_and_downloads() {
        use crate::source::Progress;
        let address = mock_server();
        let mut jf = Jellyfin::new();
        let mut config = PluginConfig::default();
        config.values.insert("server".into(), address.clone());
        config.values.insert("user".into(), "dj".into());
        config.values.insert("password".into(), "wrong".into());
        jf.configure(&config);
        let query = SourceQuery {
            text: "kerala".into(),
            ..Default::default()
        };
        let err = jf.source().unwrap().start(&query).unwrap_err().to_string();
        assert!(err.contains("username or password"), "{err}");

        config.values.insert("password".into(), "sesame".into());
        jf.configure(&config);
        let source = jf.source().unwrap();
        let id = source.start(&query).unwrap();
        let snap = source.results(&id).unwrap();
        assert_eq!(snap.items.len(), 2);
        let album = snap.items.iter().find(|f| f.folder).unwrap().clone();
        let song = snap.items.iter().find(|f| !f.folder).unwrap().clone();
        assert_eq!(source.open_folder(&id, &album).unwrap().len(), 1);

        let dir = std::env::temp_dir().join(format!("djsugar-jf-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        source
            .download(&id, std::slice::from_ref(&song), &dir)
            .unwrap();
        let mut done = None;
        for _ in 0..200 {
            for u in source.progress() {
                match u.progress {
                    Progress::Done(paths) => done = Some(paths),
                    Progress::Failed(e) => panic!("download failed: {e}"),
                    _ => {}
                }
            }
            if done.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let paths = done.expect("the download finished");
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"fLaC!");
        assert!(dir.join("Bonobo/Migration/cover.jpg").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
