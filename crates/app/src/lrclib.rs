//! Finding lyrics on LRCLIB (lrclib.net), a free community database of
//! synced lyrics with no key. On by default, and switched off in Settings ›
//! Collection › Health; asked only when the DJ presses Find in the Lyrics
//! tab, one request at a time, with DJ Sugar's User-Agent as LRCLIB asks.
//! Nothing is ever sent to LRCLIB but the track's names and length.

use djsugar_plugins::encode;
use serde_json::Value;

use crate::coverart::{try_json, Failed};
use crate::settings;

/// The switch in Settings › Collection › Health: on at first.
pub const ENABLED: &str = "lyrics_lookup";

/// Lengths within this many seconds match; LRCLIB matches the same way.
const SAME_LENGTH: f64 = 2.0;

const API: &str = "https://lrclib.net/api";

pub fn enabled() -> bool {
    settings::flag(ENABLED, true)
}

/// One record LRCLIB has.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Seconds.
    pub duration: f64,
    pub instrumental: bool,
    /// LRC text, timed by line (sometimes by word); empty when none.
    pub synced: String,
    pub plain: String,
}

impl Found {
    /// The lyrics to save: the synced ones when there are any.
    pub fn text(&self) -> &str {
        if self.synced.trim().is_empty() {
            &self.plain
        } else {
            &self.synced
        }
    }

    pub fn is_synced(&self) -> bool {
        !self.synced.trim().is_empty()
    }

    /// Whether its length is close to `seconds`, so its timing fits.
    pub fn fits(&self, seconds: f64) -> bool {
        seconds <= 0.0 || (self.duration - seconds).abs() <= SAME_LENGTH
    }
}

/// What a track is looked up by.
#[derive(Clone, Debug, Default)]
pub struct Query {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
}

fn record(v: &Value) -> Option<Found> {
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    let found = Found {
        title: s("trackName"),
        artist: s("artistName"),
        album: s("albumName"),
        duration: v["duration"].as_f64().unwrap_or(0.0),
        instrumental: v["instrumental"].as_bool().unwrap_or(false),
        synced: s("syncedLyrics"),
        plain: s("plainLyrics"),
    };
    (!found.title.is_empty()).then_some(found)
}

/// Best first: synced lyrics of the right length, then plain of the right
/// length, then the rest by how close their length is. Same lyrics only once.
fn rank(mut found: Vec<Found>, duration: f64) -> Vec<Found> {
    let mut seen = std::collections::HashSet::new();
    found.retain(|f| seen.insert((f.text().to_string(), f.instrumental)));
    found.sort_by(|a, b| {
        let key = |f: &Found| {
            (
                !f.fits(duration),
                f.instrumental,
                !f.is_synced(),
                f.text().trim().is_empty(),
            )
        };
        key(a).cmp(&key(b)).then(
            (a.duration - duration)
                .abs()
                .total_cmp(&(b.duration - duration).abs()),
        )
    });
    found.truncate(12);
    found
}

/// Why nothing came back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Miss {
    /// LRCLIB has nothing for the track.
    None,
    /// LRCLIB couldn't be reached.
    Offline,
}

/// Looks the track up: the exact match first (artist, title, album and
/// length), then a search by artist and title. Runs on the caller's thread.
pub fn find(q: &Query) -> Result<Vec<Found>, Miss> {
    let mut found = Vec::new();
    let mut reached = false;
    if q.duration > 0.0 {
        let mut url = format!(
            "{API}/get?artist_name={}&track_name={}&duration={}",
            encode(&q.artist),
            encode(&q.title),
            q.duration.round() as u64
        );
        if !q.album.trim().is_empty() {
            url.push_str(&format!("&album_name={}", encode(&q.album)));
        }
        match try_json(&url, None) {
            Ok(v) => {
                reached = true;
                found.extend(record(&v));
            }
            Err(Failed::NotThere) => reached = true,
            Err(Failed::Unreachable) => {}
        }
    }
    let mut url = format!("{API}/search?track_name={}", encode(&q.title));
    if !q.artist.trim().is_empty() {
        url.push_str(&format!("&artist_name={}", encode(&q.artist)));
    }
    match try_json(&url, None) {
        Ok(Value::Array(list)) => {
            reached = true;
            found.extend(list.iter().filter_map(record));
        }
        Ok(_) | Err(Failed::NotThere) => reached = true,
        Err(Failed::Unreachable) => {}
    }
    if found.is_empty() {
        return Err(if reached { Miss::None } else { Miss::Offline });
    }
    Ok(rank(found, q.duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn found(duration: f64, synced: &str, plain: &str) -> Found {
        Found {
            title: "Song".into(),
            artist: "Artist".into(),
            album: String::new(),
            duration,
            instrumental: false,
            synced: synced.into(),
            plain: plain.into(),
        }
    }

    #[test]
    fn reads_a_record() {
        let v = json!({
            "id": 3396226, "trackName": "I Want to Live", "artistName": "Borislav Slavov",
            "albumName": "Baldur's Gate 3", "duration": 233.0, "instrumental": false,
            "plainLyrics": "I feel your breath", "syncedLyrics": "[00:17.12] I feel your breath"
        });
        let f = record(&v).unwrap();
        assert_eq!(f.title, "I Want to Live");
        assert_eq!(f.duration, 233.0);
        assert!(f.is_synced());
        assert_eq!(f.text(), "[00:17.12] I feel your breath");
        let v = json!({ "trackName": "Quiet", "instrumental": true, "plainLyrics": null, "syncedLyrics": null });
        let f = record(&v).unwrap();
        assert!(f.instrumental);
        assert_eq!(f.text(), "");
        assert!(record(&json!({ "code": 404 })).is_none());
    }

    #[test]
    fn ranks_by_fit_then_timing() {
        let list = vec![
            found(200.0, "", "plain right length"),
            found(180.0, "[00:01.00]radio edit", ""),
            found(201.0, "[00:01.00]album", ""),
            found(201.5, "[00:01.00]album", ""),
        ];
        let ranked = rank(list, 200.0);
        assert_eq!(ranked.len(), 3, "the same lyrics only once");
        assert_eq!(ranked[0].synced, "[00:01.00]album");
        assert_eq!(ranked[1].plain, "plain right length");
        assert_eq!(ranked[2].synced, "[00:01.00]radio edit");
        assert!(!ranked[2].fits(200.0));
    }
}
