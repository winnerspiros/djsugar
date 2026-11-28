//! NetEase Cloud Music: synced lyrics without a key, for what LRCLIB lacks. A search by artist and title, the hit nearest the
//! track's length, then that song's LRC with the credit lines NetEase puts ahead of the words left out.

use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, read};
use crate::{Error, Result};

const API: &str = "https://music.163.com/api";
const TIMEOUT: Duration = Duration::from_secs(12);
/// Seconds a hit's length may differ from the track's and still count.
const SLACK: f64 = 4.0;
/// What a credit line starts with: lyricist, composer, arranger, producer.
const CREDITS: [&str; 4] = ["\u{4f5c}\u{8bcd}", "\u{4f5c}\u{66f2}", "\u{7f16}\u{66f2}", "\u{5236}\u{4f5c}\u{4eba}"];

/// Synced lyrics for a track as LRC text; None when NetEase has none timed for it.
pub fn find(artist: &str, title: &str, seconds: f64) -> Result<Option<String>> {
    if title.trim().is_empty() {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(format!("{} {}", artist.trim(), title.trim()).trim());
    let hits = get(&format!("{API}/search/get?s={q}&type=1&limit=10"))?;
    let Some(id) = pick(&hits, seconds) else { return Ok(None) };
    let v = get(&format!("{API}/song/lyric?id={id}&lv=1"))?;
    Ok(v["lrc"]["lyric"].as_str().map(clean).filter(|t| !t.is_empty()))
}

fn get(url: &str) -> Result<Value> {
    get_from(url, "https://music.163.com")
}

/// A service's JSON answer, asked for as its own site would.
pub(crate) fn get_from(url: &str, site: &str) -> Result<Value> {
    let call = agent()
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Referer", site)
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .call();
    match read(call)? {
        (200..=299, body) => serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON")),
        (s, _) => Err(Error::Status(s)),
    }
}

/// The first hit whose length is the track's, or the first of all when the length is unknown.
fn pick(v: &Value, seconds: f64) -> Option<u64> {
    let near =
        |h: &&Value| seconds <= 0.0 || h["duration"].as_f64().is_some_and(|ms| (ms / 1000.0 - seconds).abs() <= SLACK);
    v["result"]["songs"].as_array()?.iter().find(near)?["id"].as_u64()
}

/// The timed lines with words in them, credits left out.
pub(crate) fn clean(lrc: &str) -> String {
    let keep = |l: &&str| {
        let words = l.rsplit(']').next().unwrap_or("").trim();
        l.starts_with('[')
            && l.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
            && !words.is_empty()
            && !CREDITS.iter().any(|c| words.starts_with(c))
    };
    lrc.lines().map(str::trim).filter(keep).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_by_length_and_drops_credits() {
        let v =
            serde_json::json!({"result": {"songs": [{"id": 1, "duration": 480_040}, {"id": 2, "duration": 320_357}]}});
        assert_eq!(pick(&v, 320.0), Some(2));
        assert_eq!(pick(&v, 0.0), Some(1));
        assert_eq!(pick(&v, 100.0), None);
        let lrc = "[00:00.00] \u{4f5c}\u{8bcd} : Someone\n[00:04.13]\n[00:31.13]One more time\nloose";
        assert_eq!(clean(lrc), "[00:31.13]One more time");
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time", 320.0).unwrap().unwrap();
        assert!(l.contains("One more time"), "{l}");
    }
}
