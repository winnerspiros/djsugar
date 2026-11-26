//! Spotify in the streaming hub: the account's lists as hits, and its tracks as files for the decks.

use std::path::{Path, PathBuf};

use serde_json::Value;
use sugar_net::youtube::YouTube;
use sugar_net::{Error, Hit, Result};

use crate::hub::Page;

/// Tracks asked for at a time.
const PAGE: usize = 50;
/// What a playlist's ID starts with among the hits, where a track's is bare.
pub const LIST: &str = "playlist:";
/// The account's liked songs, the list shown first.
pub const LIKED: &str = "liked";
pub const LISTS: &str = "playlists";

/// The request for a search.
pub fn search(query: &str) -> String {
    format!("search:{query}")
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// A track or playlist of a reply as a hit.
fn hit(v: &Value) -> Hit {
    match v["name"].as_str() {
        Some(name) => Hit {
            id: format!("{LIST}{}", text(&v["id"])),
            title: name.to_owned(),
            artist: format!("{} tracks", v["count"].as_i64().unwrap_or(0)),
            seconds: 0,
            art: text(&v["artwork"]),
        },
        None => Hit {
            id: text(&v["id"]),
            title: text(&v["title"]),
            artist: text(&v["artist"]),
            seconds: v["durationSec"].as_u64().unwrap_or(0) as u32,
            art: text(&v["artwork"]),
        },
    }
}

/// A reply's hits and where its next page starts.
fn page(reply: &Value) -> Page {
    let hits = reply["items"].as_array().map(|a| a.iter().map(hit).collect()).unwrap_or_default();
    let more = reply["next"].as_i64().filter(|&n| n >= 0).map(|n| n.to_string());
    Page { hits, more }
}

/// One page of `request`, from the start or from where `more` says.
pub fn list(dir: &Path, request: &str, more: Option<&str>) -> Result<Page> {
    let offset = more.and_then(|m| m.parse().ok()).unwrap_or(0);
    djsugar_spotify::browse(dir, request, offset, PAGE).map(|v| page(&v)).map_err(Error::Refused)
}

/// The account's name as Spotify shows it.
pub fn name(dir: &Path) -> Option<String> {
    let v = djsugar_spotify::browse(dir, "profile", 0, 1).ok()?;
    v["name"].as_str().map(str::to_owned)
}

/// The track as a file in `cache`. Spotify hands audio to Premium accounts only,
/// so a track it will not give is fetched as its closest match on YouTube.
pub fn fetch(
    dir: &Path,
    youtube: &YouTube,
    hit: &Hit,
    cache: &Path,
    progress: &(dyn Fn(u64, u64) + Sync),
    said: &(dyn Fn(String) + Sync),
) -> Result<PathBuf> {
    let path = cache.join(format!("spotify-{}.ogg", hit.id));
    if path.is_file() {
        return Ok(path);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let (d, id, out) = (dir.to_path_buf(), hit.id.clone(), path.clone());
    let job = std::thread::spawn(move || {
        djsugar_spotify::fetch(&d, &id, &out, move |f| {
            let _ = tx.send(f);
        })
    });
    for f in rx {
        progress((f * 1000.0) as u64, 1000);
    }
    let why = match job.join() {
        Ok(Ok(())) => return Ok(path),
        Ok(Err(why)) => why,
        Err(_) => "Spotify: internal error".to_owned(),
    };
    said(format!("{why}. Looking for \"{}\" on YouTube", hit.title));
    let found = youtube.search(&format!("{} {}", hit.artist, hit.title), None)?;
    let best = found.hits.first().ok_or(Error::Refused(why))?;
    youtube.fetch(&best.id, cache, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_become_hits() {
        let tracks = serde_json::json!({ "items": [{ "id": "abc", "title": "One", "artist": "A, B",
            "durationSec": 201, "artwork": "https://i.scdn.co/image/ff" }], "next": 50 });
        let p = page(&tracks);
        assert_eq!(p.more.as_deref(), Some("50"));
        assert_eq!((p.hits[0].id.as_str(), p.hits[0].artist.as_str(), p.hits[0].seconds), ("abc", "A, B", 201));
        let lists = serde_json::json!({ "items": [{ "id": "xyz", "name": "Warm up", "count": 12, "artwork": "" }],
            "next": -1 });
        let p = page(&lists);
        assert_eq!(p.more, None);
        assert_eq!((p.hits[0].id.as_str(), p.hits[0].artist.as_str()), ("playlist:xyz", "12 tracks"));
    }
}
