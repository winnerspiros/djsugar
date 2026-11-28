//! KuGou: synced lyrics without a key, for what LRCLIB and NetEase lack. A search by artist and title, the hit nearest
//! the track's length, the lyrics people sent in for that recording, and the best rated of them as LRC.

use serde_json::Value;

use crate::Result;
use crate::netease::{clean, get_from};

const SITE: &str = "https://www.kugou.com";
/// Seconds a hit's length may differ from the track's and still count.
const SLACK: f64 = 4.0;

/// Synced lyrics for a track as LRC text; None when KuGou has none timed for it.
pub fn find(artist: &str, title: &str, seconds: f64) -> Result<Option<String>> {
    if title.trim().is_empty() {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(format!("{} {}", artist.trim(), title.trim()).trim());
    let hits = get_from(
        &format!("http://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword={q}&page=1&pagesize=10"),
        SITE,
    )?;
    let Some(hash) = pick(&hits, seconds) else { return Ok(None) };
    let sent = get_from(&format!("https://krcs.kugou.com/search?ver=1&man=yes&client=mobi&hash={hash}"), SITE)?;
    let Some((id, key)) = best(&sent) else { return Ok(None) };
    let v = get_from(
        &format!("https://lyrics.kugou.com/download?ver=1&client=pc&id={id}&accesskey={key}&fmt=lrc&charset=utf8"),
        SITE,
    )?;
    let text = v["content"].as_str().and_then(crate::botguard::b64_decode).and_then(|b| String::from_utf8(b).ok());
    Ok(text.map(|t| clean(&t)).filter(|t| !t.is_empty()))
}

/// The first hit whose length is the track's, or the first of all when the length is unknown.
fn pick(v: &Value, seconds: f64) -> Option<&str> {
    let near = |h: &&Value| seconds <= 0.0 || h["duration"].as_f64().is_some_and(|s| (s - seconds).abs() <= SLACK);
    v["data"]["info"].as_array()?.iter().find(near)?["hash"]
        .as_str()
        .filter(|h| h.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// The lyrics rated highest of those sent in: their number and the key that opens them.
fn best(v: &Value) -> Option<(String, &str)> {
    let c = v["candidates"].as_array()?.iter().max_by_key(|c| c["score"].as_i64().unwrap_or(0))?;
    let id = c["id"].as_str().map(str::to_owned).or_else(|| c["id"].as_u64().map(|n| n.to_string()))?;
    let key = c["accesskey"].as_str().filter(|k| k.bytes().all(|b| b.is_ascii_alphanumeric()))?;
    id.bytes().all(|b| b.is_ascii_digit()).then_some((id, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_by_length_and_takes_the_best_rated_lyrics() {
        let v = serde_json::json!({"data": {"info": [{"hash": "aa11", "duration": 234}, {"hash": "bb22", "duration": 320}]}});
        assert_eq!(pick(&v, 321.0), Some("bb22"));
        assert_eq!(pick(&v, 0.0), Some("aa11"));
        assert_eq!(pick(&v, 100.0), None);
        let sent = serde_json::json!({"candidates": [
            {"id": "7", "accesskey": "K1", "score": 40}, {"id": "9", "accesskey": "K2", "score": 60}]});
        assert_eq!(best(&sent), Some(("9".to_string(), "K2")));
        assert_eq!(best(&serde_json::json!({"candidates": []})), None);
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time", 320.0).unwrap().unwrap();
        assert!(l.contains("One more time") && !l.contains("[ti:"), "{l}");
    }
}
