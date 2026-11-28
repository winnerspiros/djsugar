//! QQ Music: synced lyrics without a key, asked last of the timed sources. A search by artist and title, the hit of the
//! track's length, then that song's LRC.

use serde_json::Value;

use crate::Result;
use crate::netease::{clean, get_from};

const API: &str = "https://c.y.qq.com";
const SITE: &str = "https://y.qq.com";
/// Seconds a hit's length may differ from the track's and still count.
const SLACK: f64 = 4.0;
/// How many of the hits are asked their length.
const ASKED: usize = 3;

/// Synced lyrics for a track as LRC text; None when QQ Music has none timed for it.
pub fn find(artist: &str, title: &str, seconds: f64) -> Result<Option<String>> {
    if title.trim().is_empty() {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(format!("{} {}", artist.trim(), title.trim()).trim());
    let hits = get_from(&format!("{API}/splcloud/fcgi-bin/smartbox_new.fcg?key={q}&format=json"), SITE)?;
    // The search does not say how long a song is; each hit is asked.
    let of_length = |mid: &&str| {
        seconds <= 0.0
            || get_from(&format!("{API}/v8/fcg-bin/fcg_play_single_song.fcg?songmid={mid}&format=json"), SITE)
                .is_ok_and(|v| v["data"][0]["interval"].as_f64().is_some_and(|s| (s - seconds).abs() <= SLACK))
    };
    let Some(mid) = songs(&hits).into_iter().take(ASKED).find(of_length) else { return Ok(None) };
    let v =
        get_from(&format!("{API}/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={mid}&format=json&nobase64=1"), SITE)?;
    Ok(v["lyric"].as_str().map(clean).filter(|t| !t.is_empty()))
}

/// What the search found, as the names QQ Music knows the songs by.
fn songs(v: &Value) -> Vec<&str> {
    let known = |m: &&str| !m.is_empty() && m.bytes().all(|b| b.is_ascii_alphanumeric());
    let list = v["data"]["song"]["itemlist"].as_array();
    list.map(|l| l.iter().filter_map(|h| h["mid"].as_str()).filter(known).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_songs_of_a_search() {
        let v =
            serde_json::json!({"data": {"song": {"itemlist": [{"mid": "001cEPY70WKJXz"}, {"mid": "a/b"}, {"id": 4}]}}});
        assert_eq!(songs(&v), ["001cEPY70WKJXz"]);
        assert!(songs(&serde_json::json!({"code": 1})).is_empty());
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time", 320.0).unwrap().unwrap();
        assert!(l.contains("One more time") && !l.contains("[ti:"), "{l}");
        assert!(find("Daft Punk", "One More Time", 100.0).unwrap().is_none());
    }
}
