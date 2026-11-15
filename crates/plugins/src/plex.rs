//! A Plex Media Server as a source in the rail: the DJ's own music
//! libraries searched by album, artist, song or playlist, and the original
//! files downloaded into the collection.
//!
//! Every call carries the DJ's `X-Plex-Token` and asks for JSON. Searches go
//! to `/hubs/search`, an album's or artist's contents to
//! `/library/metadata/<id>/children`, playlists to
//! `/playlists?playlistType=audio`, and a file downloads from its part's
//! `key` with `download=1`.

use serde_json::Value;

use crate::server::{self, encode, matches, Results, Server, ServerSource, Song};
use crate::source::{Found, QueryKind, SearchSource, SourceInfo, SourceQuery};
use crate::{Manifest, Plugin, PluginConfig, Setting, SettingKind};

const MANIFEST: Manifest = Manifest {
    id: "plex",
    name: "Plex",
    // The DJ's own server, at the address they fill in.
    hosts: &[],
    downloads: true,
};

/// How big list covers are fetched, in pixels.
const THUMB: u32 = 96;

/// A chevron, as a generic rail icon.
const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 30"><rect width="30" height="30" rx="7" fill="#282a2d"/><path d="M11 7h4.6l5.4 8-5.4 8H11l5.4-8z" fill="#e5a00d"/></svg>"##;

pub struct Plex {
    source: ServerSource<PlexServer>,
}

impl Default for Plex {
    fn default() -> Self {
        Self::new()
    }
}

impl Plex {
    pub fn new() -> Self {
        Self {
            source: ServerSource::new("plex", PlexServer::default()),
        }
    }
}

impl Plugin for Plex {
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
                "Your own Plex Media Server",
                "Searches open the albums, artists and playlists in its music libraries, \
                 and downloads bring the original files into your collection.",
            ),
            Setting::new(
                "server",
                SettingKind::Text,
                "Server address",
                "For example http://192.168.1.20:32400",
            ),
            Setting::new(
                "token",
                SettingKind::Secret,
                "Plex token",
                "In Plex Web, open any track's ⋯ menu › Get Info › View XML: the token is \
                 the X-Plex-Token at the end of the address that opens.",
            ),
        ]
        .into_iter()
        .chain(server::storage_settings(MANIFEST.name))
        .collect()
    }

    fn configure(&mut self, config: &PluginConfig) {
        let s = &mut self.source.server;
        s.url = server::server(config.get("server"));
        s.token = config.get("token").trim().to_string();
    }
}

#[derive(Default)]
pub(crate) struct PlexServer {
    url: String,
    token: String,
}

impl PlexServer {
    /// `path` on the server, signed with the token.
    fn url(&self, path: &str) -> String {
        let sep = if path.contains('?') { '&' } else { '?' };
        format!(
            "{}{path}{sep}X-Plex-Token={}",
            self.url,
            encode(&self.token)
        )
    }

    /// The server's `MediaContainer` for `path`.
    fn get(&self, path: &str) -> Result<Value, String> {
        let mut response = server::agent()
            .get(&self.url(path))
            .header("Accept", "application/json")
            .header("X-Plex-Product", "DJ Sugar")
            .header("X-Plex-Client-Identifier", "djsugar")
            .call()
            .map_err(|e| format!("Couldn't reach the server: {e}"))?;
        match response.status().as_u16() {
            401 => return Err("Plex didn't accept the token.".into()),
            s if s >= 400 => return Err(format!("Plex answered {s}.")),
            _ => {}
        }
        let v: Value = response
            .body_mut()
            .read_json()
            .map_err(|_| "That address isn't a Plex server.".to_string())?;
        Ok(v["MediaContainer"].clone())
    }

    /// A picture, scaled down on the server for the list.
    fn thumb(&self, path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }
        self.url(&format!(
            "/photo/:/transcode?width={THUMB}&height={THUMB}&minSize=1&upscale=1&url={}",
            encode(path)
        ))
    }

    fn picture(&self, path: &str) -> String {
        if path.is_empty() {
            String::new()
        } else {
            self.url(path)
        }
    }

    fn song(&self, v: &Value) -> Option<Song> {
        if text(&v["type"]) != "track" {
            return None;
        }
        let id = text(&v["ratingKey"]);
        if id.is_empty() {
            return None;
        }
        let media = &v["Media"][0];
        let part = &media["Part"][0];
        let suffix = match text(&part["container"]) {
            c if !c.is_empty() => c,
            _ => match text(&media["container"]) {
                c if !c.is_empty() => c,
                _ => extension(&text(&part["file"])),
            },
        };
        let key = text(&part["key"]);
        let album_artist = text(&v["grandparentTitle"]);
        let artist = match text(&v["originalTitle"]) {
            a if !a.is_empty() => a,
            _ => album_artist.clone(),
        };
        let cover = match text(&v["parentThumb"]) {
            t if !t.is_empty() => t,
            _ => text(&v["thumb"]),
        };
        Some(Song {
            key: format!("track:{id}"),
            title: text(&v["title"]),
            artist,
            album_artist,
            album: text(&v["parentTitle"]),
            year: match text(&v["parentYear"]) {
                y if !y.is_empty() => y,
                _ => text(&v["year"]),
            },
            number: text(&v["index"]),
            duration: v["duration"].as_f64().unwrap_or(0.0) / 1000.0,
            suffix,
            bitrate: media["bitrate"].as_u64().unwrap_or(0),
            size: part["size"].as_u64().unwrap_or(0),
            file_url: if key.is_empty() {
                String::new()
            } else {
                self.url(&format!("{key}?download=1"))
            },
            cover_url: self.picture(&cover),
            thumb_url: self.thumb(&cover),
        })
    }

    fn album(&self, v: &Value) -> Option<Found> {
        let id = text(&v["ratingKey"]);
        if id.is_empty() || text(&v["type"]) != "album" {
            return None;
        }
        let name = text(&v["title"]);
        Some(Found {
            key: format!("album:{id}"),
            folder: true,
            artist: text(&v["parentTitle"]),
            title: name.clone(),
            album: name,
            year: text(&v["year"]),
            track_count: v["leafCount"].as_u64().unwrap_or(0) as usize,
            from: MANIFEST.name.into(),
            cover_url: self.thumb(&text(&v["thumb"])),
            ..Default::default()
        })
    }

    fn playlist(&self, v: &Value) -> Option<Found> {
        let id = text(&v["ratingKey"]);
        if id.is_empty() {
            return None;
        }
        let name = text(&v["title"]);
        let picture = match text(&v["thumb"]) {
            t if !t.is_empty() => t,
            _ => text(&v["composite"]),
        };
        Some(Found {
            key: format!("playlist:{id}"),
            folder: true,
            title: name.clone(),
            album: name,
            duration: v["duration"].as_f64().unwrap_or(0.0) / 1000.0,
            track_count: v["leafCount"].as_u64().unwrap_or(0) as usize,
            from: "Playlist".into(),
            cover_url: self.thumb(&picture),
            ..Default::default()
        })
    }

    /// What `/hubs/search` found, by hub type ("album", "track", "artist").
    fn hubs(&self, q: &str, limit: u32) -> Result<Vec<(String, Vec<Value>)>, String> {
        let v = self.get(&format!("/hubs/search?query={}&limit={limit}", encode(q)))?;
        Ok(list(&v["Hub"])
            .iter()
            .map(|h| (text(&h["type"]), list(&h["Metadata"])))
            .collect())
    }
}

impl Server for PlexServer {
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
            QueryKind::Playlist if matches("", q) => "All your music playlists".to_string(),
            QueryKind::Playlist => format!("Music playlists called {q}"),
            _ if q.is_empty() => return Vec::new(),
            QueryKind::Artist => format!("Albums by artists called {q}"),
            QueryKind::Album => format!("Albums called {q}"),
            QueryKind::Track => format!("Songs called {q}"),
            QueryKind::Auto => format!("Albums and songs matching {q}"),
        };
        vec![line, "Downloads are the original files.".into()]
    }

    fn missing(&self) -> Option<String> {
        (self.url.is_empty() || self.token.is_empty())
            .then(|| "Fill in your server and token in Settings › Sources › Plex first.".into())
    }

    fn search(&self, query: &SourceQuery) -> Result<Results, String> {
        let q = query.text.trim();
        let mut out = Results::default();
        match query.kind {
            QueryKind::Playlist => {
                let v = self.get("/playlists?playlistType=audio")?;
                out.folders = list(&v["Metadata"])
                    .iter()
                    .filter(|p| matches(&text(&p["title"]), q))
                    .filter_map(|p| self.playlist(p))
                    .collect();
            }
            QueryKind::Artist => {
                for (kind, items) in self.hubs(q, 5)? {
                    if kind != "artist" {
                        continue;
                    }
                    for a in items {
                        let v = self.get(&format!(
                            "/library/metadata/{}/children",
                            encode(&text(&a["ratingKey"]))
                        ))?;
                        out.folders
                            .extend(list(&v["Metadata"]).iter().filter_map(|a| self.album(a)));
                    }
                }
            }
            kind => {
                let limit = if kind == QueryKind::Auto { 50 } else { 100 };
                for (hub, items) in self.hubs(q, limit)? {
                    match (hub.as_str(), kind) {
                        ("album", QueryKind::Auto | QueryKind::Album) => out
                            .folders
                            .extend(items.iter().filter_map(|a| self.album(a))),
                        ("track", QueryKind::Auto | QueryKind::Track) => {
                            out.songs.extend(items.iter().filter_map(|s| self.song(s)))
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(out)
    }

    fn folder(&self, key: &str) -> Result<Vec<Song>, String> {
        let path = match key.split_once(':') {
            Some(("album", id)) => format!("/library/metadata/{}/children", encode(id)),
            Some(("playlist", id)) => format!("/playlists/{}/items", encode(id)),
            _ => return Err("Not a folder.".into()),
        };
        let v = self.get(&path)?;
        Ok(list(&v["Metadata"])
            .iter()
            .filter_map(|s| self.song(s))
            .collect())
    }

    fn resolve(&self, song: &Song) -> Result<Song, String> {
        let id = song.key.strip_prefix("track:").unwrap_or(&song.key);
        let v = self.get(&format!("/library/metadata/{}", encode(id)))?;
        list(&v["Metadata"])
            .first()
            .and_then(|m| self.song(m))
            .filter(|s| !s.file_url.is_empty())
            .ok_or_else(|| format!("Plex has no file for {}.", song.title))
    }
}

fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

/// "flac" from "/music/a/03 Kerala.flac".
fn extension(file: &str) -> String {
    std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
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
    use serde_json::json;

    fn server() -> PlexServer {
        PlexServer {
            url: "http://plex.lan:32400".into(),
            token: "abc".into(),
        }
    }

    #[test]
    fn signs_addresses() {
        let s = server();
        assert_eq!(
            s.url("/library/sections"),
            "http://plex.lan:32400/library/sections?X-Plex-Token=abc"
        );
        assert_eq!(
            s.url("/library/parts/9/1/file.flac?download=1"),
            "http://plex.lan:32400/library/parts/9/1/file.flac?download=1&X-Plex-Token=abc"
        );
        assert!(s
            .thumb("/library/metadata/5/thumb/1")
            .contains("url=%2Flibrary%2Fmetadata%2F5%2Fthumb%2F1&X-Plex-Token=abc"));
    }

    #[test]
    fn reads_tracks() {
        let s = server();
        let song = s
            .song(&json!({
                "type": "track", "ratingKey": "101", "title": "Kerala",
                "grandparentTitle": "Bonobo", "parentTitle": "Migration", "parentYear": 2017,
                "index": 3, "duration": 251000, "parentThumb": "/library/metadata/90/thumb/1",
                "Media": [{ "bitrate": 1011, "audioCodec": "flac", "container": "flac",
                    "Part": [{ "key": "/library/parts/7/1/file.flac", "size": 31000000,
                               "file": "/music/Bonobo/Migration/03 Kerala.flac" }] }]
            }))
            .unwrap();
        assert_eq!(song.key, "track:101");
        assert_eq!(song.artist, "Bonobo");
        assert_eq!(song.duration, 251.0);
        assert_eq!(song.suffix, "flac");
        assert_eq!(
            song.file_url,
            "http://plex.lan:32400/library/parts/7/1/file.flac?download=1&X-Plex-Token=abc"
        );
        let files = song.files(std::path::Path::new("/dl"));
        assert_eq!(
            files[1].to,
            std::path::PathBuf::from("/dl/Bonobo/Migration/03 Kerala.flac")
        );
        // A search hit without its media is resolved before downloading.
        let bare = s
            .song(&json!({ "type": "track", "ratingKey": "5", "title": "X",
                           "originalTitle": "Guest", "grandparentTitle": "VA" }))
            .unwrap();
        assert!(bare.file_url.is_empty());
        assert_eq!(
            (bare.artist.as_str(), bare.album_artist.as_str()),
            ("Guest", "VA")
        );
        assert!(s
            .song(&json!({ "type": "movie", "ratingKey": "1" }))
            .is_none());
    }

    #[test]
    fn reads_albums_and_playlists() {
        let s = server();
        let a = s
            .album(
                &json!({ "type": "album", "ratingKey": "90", "title": "Migration",
                            "parentTitle": "Bonobo", "year": 2017, "leafCount": 12,
                            "thumb": "/library/metadata/90/thumb/1" }),
            )
            .unwrap();
        assert_eq!((a.key.as_str(), a.track_count), ("album:90", 12));
        let p = s
            .playlist(
                &json!({ "ratingKey": "300", "title": "Warm-up", "leafCount": 40,
                               "composite": "/playlists/300/composite/1" }),
            )
            .unwrap();
        assert_eq!(p.key, "playlist:300");
        assert!(p.folder);
    }

    #[test]
    fn needs_a_token() {
        assert!(PlexServer::default().missing().is_some());
        assert!(server().missing().is_none());
    }
}
