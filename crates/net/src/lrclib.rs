//! LRCLIB: free synced lyrics, no key. An exact lookup by artist, title and
//! length first, then a search whose closest timed match is taken.

use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, read};
use crate::{Error, Result};

const API: &str = "https://lrclib.net/api";
const AGENT: &str = "djsugar (https://github.com/winnerspiros/djsugar)";
const TIMEOUT: Duration = Duration::from_secs(12);
/// Seconds a search hit's length may differ from the track's and still count.
const SLACK: f64 = 4.0;

/// Lyrics for a track as LRC text: synced when LRCLIB has them, else plain.
/// None when LRCLIB has nothing for it.
pub fn find(artist: &str, title: &str, seconds: f64) -> Result<Option<String>> {
    let (a, t) = (crate::soundcloud::encode(artist.trim()), crate::soundcloud::encode(title.trim()));
    if title.trim().is_empty() {
        return Ok(None);
    }
    let exact = format!("{API}/get?artist_name={a}&track_name={t}&duration={}", seconds.round() as u64);
    if !artist.trim().is_empty() {
        match get(&exact) {
            Ok(v) => {
                if let Some(text) = text(&v) {
                    return Ok(Some(text));
                }
            }
            Err(Error::Status(404)) => {}
            Err(e) => return Err(e),
        }
    }
    let q = if artist.trim().is_empty() { format!("q={t}") } else { format!("track_name={t}&artist_name={a}") };
    let v = get(&format!("{API}/search?{q}"))?;
    Ok(best(&v, seconds))
}

fn get(url: &str) -> Result<Value> {
    match read(agent().get(url).header("User-Agent", AGENT).config().timeout_global(Some(TIMEOUT)).build().call())? {
        (200..=299, body) => serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON")),
        (s, _) => Err(Error::Status(s)),
    }
}

/// A record's lyrics: the synced ones, else the plain.
fn text(v: &Value) -> Option<String> {
    let pick = |k: &str| v[k].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    pick("syncedLyrics").or_else(|| pick("plainLyrics"))
}

/// The search hit nearest the track's length, synced ones first.
fn best(v: &Value, seconds: f64) -> Option<String> {
    let hits = v.as_array()?;
    let near = |h: &&Value| h["duration"].as_f64().is_none_or(|d| seconds <= 0.0 || (d - seconds).abs() <= SLACK);
    let synced = |h: &&Value| h["syncedLyrics"].as_str().is_some_and(|s| !s.trim().is_empty());
    hits.iter().filter(near).find(synced).or_else(|| hits.iter().find(near)).and_then(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_synced_near_length() {
        let v: Value = serde_json::from_str(
            r#"[{"duration":300,"syncedLyrics":"[00:01.00]far"},
                {"duration":181,"plainLyrics":"plain"},
                {"duration":182,"syncedLyrics":"[00:01.00]near","plainLyrics":"p"}]"#,
        )
        .unwrap();
        assert_eq!(best(&v, 180.0).as_deref(), Some("[00:01.00]near"));
        let v: Value = serde_json::from_str(r#"[{"duration":181,"plainLyrics":"plain"}]"#).unwrap();
        assert_eq!(best(&v, 180.0).as_deref(), Some("plain"));
        assert_eq!(best(&v, 500.0), None);
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time", 320.0).unwrap().unwrap();
        assert!(l.contains('['), "{l}");
    }
}
