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

/// The YouTube hit that is the track: the first whose length is the track's, give or take, or else the first.
fn closest(found: &[Hit], seconds: u32) -> Option<&Hit> {
    found.iter().find(|h| seconds > 0 && h.seconds.abs_diff(seconds) <= 3).or(found.first())
}

static ACCOUNT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Tells where the account is kept, so lyrics can be asked for from anywhere.
pub fn kept_in(dir: &Path) {
    let _ = ACCOUNT.set(dir.to_path_buf());
}

/// Spotify's lyrics for a track of any origin: the track is looked up by name and must run as long, to the
/// few seconds. None without an account, a match or lyrics.
pub fn words(artist: &str, title: &str, seconds: f64) -> Option<String> {
    let dir = ACCOUNT.get().filter(|d| djsugar_spotify::logged_in(d))?;
    let found = list(dir, &search(&format!("{artist} {title}")), None).ok()?;
    let long = seconds.round() as u32;
    let same = found.hits.iter().find(|h| !h.id.starts_with(LIST) && long > 0 && h.seconds.abs_diff(long) <= 3)?;
    djsugar_spotify::lyrics(dir, &same.id).ok().flatten()
}

/// The track as a file in `cache`. Spotify gives the name and the lyrics; the sound is its match on YouTube,
/// looked for once and remembered.
pub fn fetch(
    dir: &Path,
    youtube: &YouTube,
    hit: &Hit,
    cache: &Path,
    progress: sugar_net::Progress<'_>,
) -> Result<PathBuf> {
    let words = std::thread::spawn({
        let (d, id) = (dir.to_path_buf(), hit.id.clone());
        move || djsugar_spotify::lyrics(&d, &id).ok().flatten()
    });
    let known = cache.join(format!("spotify-{}.txt", hit.id));
    let video = match std::fs::read_to_string(&known) {
        Ok(video) => video,
        Err(_) => {
            let began = std::time::Instant::now();
            let found = youtube.search(&format!("{} {}", hit.artist, hit.title), None)?;
            crate::log::note(&format!("spotify {}: matched in {} ms", hit.id, began.elapsed().as_millis()));
            let best = closest(&found.hits, hit.seconds).ok_or(Error::Refused("Nothing found".into()))?;
            let _ = std::fs::create_dir_all(cache);
            let _ = std::fs::write(&known, &best.id);
            best.id.clone()
        }
    };
    let sound = youtube.fetch(&video, cache, progress)?;
    let lrc = sound.with_extension("lrc");
    // The words may take longer than the sound; the deck does not wait for them.
    if !lrc.exists() {
        std::thread::spawn(move || {
            if let Ok(Some(text)) = words.join() {
                let _ = std::fs::write(lrc, text);
            }
        });
    }
    Ok(sound)
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

    #[test]
    fn the_match_is_the_hit_of_the_same_length() {
        let hit = |id: &str, seconds| Hit {
            id: id.into(),
            title: String::new(),
            artist: String::new(),
            seconds,
            art: String::new(),
        };
        let found = [hit("video", 251), hit("audio", 201), hit("live", 340)];
        assert_eq!(closest(&found, 200).map(|h| h.id.as_str()), Some("audio"));
        assert_eq!(closest(&found, 90).map(|h| h.id.as_str()), Some("video"), "none fits: the first");
        assert_eq!(closest(&found, 0).map(|h| h.id.as_str()), Some("video"));
        assert!(closest(&[], 200).is_none());
    }
}
