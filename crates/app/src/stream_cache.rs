//! Streaming services' playlists, kept between runs so they show straight
//! away instead of after signing in and fetching them all again.
//!
//! Each service has a file in `streaming/` of the cache folder
//! ([`djsugar_engine::cache::cache_dir`]): the name signed in as, the list of
//! playlists, and the tracks of the playlists opened so far with the version
//! of the playlist they were fetched at. Only what the service's API showed
//! is kept (titles, artists, ids), never audio. Signing out deletes the file.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::spotify::{Playlist, StreamTrack};

/// Bumped when what is kept changes shape; older files are ignored.
const FORMAT: u64 = 1;

/// What is kept of a service.
#[derive(Debug, Default, PartialEq)]
pub struct Kept {
    pub user: String,
    pub playlists: Vec<Playlist>,
    /// The tracks of each playlist fetched, by its key.
    pub tracks: HashMap<String, Vec<StreamTrack>>,
    /// The [`Playlist::version`] each playlist's tracks were fetched at.
    pub versions: HashMap<String, String>,
}

/// A file to write and what goes in it.
type Write = (PathBuf, Vec<u8>);

fn path(service: &str) -> Option<PathBuf> {
    // Tests never read or write the DJ's own cache.
    if cfg!(test) {
        return None;
    }
    djsugar_engine::cache::cache_dir().map(|d| d.join("streaming").join(format!("{service}.json")))
}

/// What was kept of `service` (its id, such as "spotify") last time.
pub fn load(service: &str) -> Option<Kept> {
    from_bytes(&std::fs::read(path(service)?).ok()?)
}

/// Keeps `kept` for next time. The file is written on a thread of its own,
/// so a big collection of playlists doesn't hold up the screen.
pub fn store(service: &str, kept: &Kept) {
    let Some(path) = path(service) else { return };
    let bytes = to_bytes(kept);
    static WRITER: OnceLock<Mutex<Sender<Write>>> = OnceLock::new();
    let writer = WRITER.get_or_init(|| {
        let (tx, rx) = channel::<Write>();
        std::thread::Builder::new()
            .name("stream-cache".into())
            .spawn(move || {
                for (path, bytes) in rx {
                    write(&path, &bytes);
                }
            })
            .expect("spawn stream-cache thread");
        Mutex::new(tx)
    });
    if let Ok(tx) = writer.lock() {
        let _ = tx.send((path, bytes));
    }
}

/// Forgets what was kept of `service`, as signing out does.
pub fn remove(service: &str) {
    if let Some(path) = path(service) {
        let _ = std::fs::remove_file(path);
    }
}

/// Writes through a temporary file, so a crash never leaves half a file.
fn write(path: &PathBuf, bytes: &[u8]) {
    let Some(dir) = path.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn to_bytes(kept: &Kept) -> Vec<u8> {
    let playlists: Vec<Value> = kept
        .playlists
        .iter()
        .map(|p| json!({ "key": p.key, "name": p.name, "total": p.total, "version": p.version }))
        .collect();
    let tracks: serde_json::Map<String, Value> = kept
        .tracks
        .iter()
        .map(|(key, tracks)| {
            let list: Vec<Value> = tracks
                .iter()
                .map(|t| {
                    json!({
                        "id": t.id,
                        "page": t.page,
                        "isrc": t.isrc,
                        "title": t.title,
                        "artists": t.artists,
                        "album": t.album,
                        "duration": t.duration,
                    })
                })
                .collect();
            let entry = json!({
                "version": kept.versions.get(key).cloned().unwrap_or_default(),
                "tracks": list,
            });
            (key.clone(), entry)
        })
        .collect();
    json!({
        "format": FORMAT,
        "user": kept.user,
        "playlists": playlists,
        "tracks": tracks,
    })
    .to_string()
    .into_bytes()
}

fn from_bytes(bytes: &[u8]) -> Option<Kept> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if v["format"].as_u64() != Some(FORMAT) {
        return None;
    }
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let playlists = v["playlists"]
        .as_array()?
        .iter()
        .filter_map(|p| {
            Some(Playlist {
                key: p["key"].as_str()?.to_string(),
                name: text(&p["name"]),
                total: p["total"].as_u64().unwrap_or(0) as usize,
                version: text(&p["version"]),
            })
        })
        .collect();
    let mut kept = Kept {
        user: text(&v["user"]),
        playlists,
        ..Default::default()
    };
    for (key, entry) in v["tracks"].as_object().into_iter().flatten() {
        let tracks = entry["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| StreamTrack {
                id: text(&t["id"]),
                page: text(&t["page"]),
                isrc: text(&t["isrc"]),
                title: text(&t["title"]),
                artists: t["artists"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a.as_str().map(str::to_string))
                    .collect(),
                album: text(&t["album"]),
                duration: t["duration"].as_f64().unwrap_or(0.0),
            })
            .collect();
        kept.tracks.insert(key.clone(), tracks);
        kept.versions.insert(key.clone(), text(&entry["version"]));
    }
    Some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_playlists_and_tracks() {
        let mut kept = Kept {
            user: "dj".into(),
            playlists: vec![
                Playlist {
                    key: "liked".into(),
                    name: "Liked Songs".into(),
                    total: 2,
                    version: String::new(),
                },
                Playlist {
                    key: "37i9".into(),
                    name: "Warm-up".into(),
                    total: 1,
                    version: "snap1".into(),
                },
            ],
            ..Default::default()
        };
        kept.tracks.insert(
            "37i9".into(),
            vec![StreamTrack {
                id: "4uLU6hMCjMI75M1A2tKUQC".into(),
                page: "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC".into(),
                isrc: "GBARL9300135".into(),
                title: "Never Gonna Give You Up".into(),
                artists: vec!["Rick Astley".into()],
                album: "Whenever You Need Somebody".into(),
                duration: 213.5,
            }],
        );
        kept.versions.insert("37i9".into(), "snap1".into());
        assert_eq!(from_bytes(&to_bytes(&kept)), Some(kept));
    }

    #[test]
    fn ignores_other_formats() {
        assert_eq!(from_bytes(br#"{"format":99,"playlists":[]}"#), None);
        assert_eq!(from_bytes(b"not json"), None);
    }
}
