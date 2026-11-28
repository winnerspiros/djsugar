//! lrc.red: lyrics timed word by word, no key. A search by artist and title, the recording nearest the track's length,
//! and its lyrics as enhanced LRC.

use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, read};
use crate::{Error, Result};

const SITE: &str = "https://lrc.red";
const AGENT: &str = "djsugar (https://github.com/winnerspiros/djsugar)";
const TIMEOUT: Duration = Duration::from_secs(12);
/// Seconds a hit's length may differ from the track's and still count.
const SLACK: f64 = 4.0;

fn get(url: &str) -> Result<Option<String>> {
    let call = agent().get(url).header("User-Agent", AGENT).config().timeout_global(Some(TIMEOUT)).build().call();
    match read(call)? {
        (200..=299, body) => Ok(Some(body)),
        (404, _) => Ok(None),
        (s, _) => Err(Error::Status(s)),
    }
}

/// The recording of the track's length: what it is filed under.
fn pick(v: &Value, seconds: f64) -> Option<&str> {
    let near = |h: &&Value| h["duration"].as_f64().is_some_and(|s| (s - seconds).abs() <= SLACK);
    v["hits"].as_array()?.iter().find(near)?["isrc"].as_str().filter(|i| i.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// Word timed lyrics for a track as enhanced LRC; None when lrc.red has none for a recording of this length.
pub fn find(artist: &str, title: &str, seconds: f64) -> Result<Option<String>> {
    if title.trim().is_empty() || seconds <= 0.0 {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(format!("{} {}", artist.trim(), title.trim()).trim());
    let Some(found) = get(&format!("{SITE}/search.json?q={q}"))? else { return Ok(None) };
    let hits: Value = serde_json::from_str(&found).map_err(|_| Error::Parse("not JSON"))?;
    let Some(isrc) = pick(&hits, seconds) else { return Ok(None) };
    Ok(get(&format!("{SITE}/s/{isrc}.lrc"))?.map(|t| crate::netease::clean(&t)).filter(|t| !t.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_recording_of_the_tracks_length() {
        let v = serde_json::json!({"hits": [{"isrc": "A1", "duration": 480.0}, {"isrc": "B2", "duration": 320.357}, {"isrc": "../x", "duration": 100.0}]});
        assert_eq!(pick(&v, 320.0), Some("B2"));
        assert_eq!(pick(&v, 200.0), None);
        assert_eq!(pick(&v, 100.0), None);
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time", 320.0).unwrap().unwrap();
        assert!(l.starts_with('[') && l.contains(">One <"), "{l}");
    }
}
