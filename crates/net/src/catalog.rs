//! The catalogues of Deezer and Apple Music, no account needed: they name tracks, their makers, lengths and
//! covers. Neither gives a DJ the sound, so a track is played from its match elsewhere.

use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, read};
use crate::soundcloud::encode;
use crate::{Error, Hit, Result};

const TIMEOUT: Duration = Duration::from_secs(15);
const PAGE: usize = 25;

#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

/// Which catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Deezer,
    Apple,
}

fn get(url: &str) -> Result<Value> {
    let (status, body) = read(agent().get(url).config().timeout_global(Some(TIMEOUT)).build().call())?;
    if status >= 400 {
        return Err(Error::Status(status));
    }
    serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))
}

/// A page of tracks matching `query`; `more` is how many were already given.
pub fn search(which: Which, query: &str, more: Option<&str>) -> Result<Page> {
    let from: usize = more.and_then(|m| m.parse().ok()).unwrap_or(0);
    let (hits, got): (Vec<Hit>, usize) = match which {
        Which::Deezer => {
            let v = get(&format!("https://api.deezer.com/search?q={}&index={from}&limit={PAGE}", encode(query)))?;
            let all = v["data"].as_array().cloned().unwrap_or_default();
            (all.iter().filter_map(deezer).collect(), all.len())
        }
        Which::Apple => {
            let v = get(&format!(
                "https://itunes.apple.com/search?term={}&media=music&entity=song&limit={PAGE}&offset={from}",
                encode(query)
            ))?;
            let all = v["results"].as_array().cloned().unwrap_or_default();
            (all.iter().filter_map(apple).collect(), all.len())
        }
    };
    Ok(Page { hits, more: (got >= PAGE).then(|| (from + got).to_string()) })
}

fn deezer(t: &Value) -> Option<Hit> {
    Some(Hit {
        id: t["id"].as_u64()?.to_string(),
        title: t["title"].as_str()?.to_owned(),
        artist: t["artist"]["name"].as_str().unwrap_or_default().to_owned(),
        seconds: t["duration"].as_u64().unwrap_or(0) as u32,
        art: t["album"]["cover_big"].as_str().unwrap_or_default().to_owned(),
    })
}

fn apple(t: &Value) -> Option<Hit> {
    Some(Hit {
        id: t["trackId"].as_u64()?.to_string(),
        title: t["trackName"].as_str()?.to_owned(),
        artist: t["artistName"].as_str().unwrap_or_default().to_owned(),
        seconds: (t["trackTimeMillis"].as_u64().unwrap_or(0) / 1000) as u32,
        // The cover comes at 100 px; the same address names larger ones.
        art: t["artworkUrl100"].as_str().unwrap_or_default().replace("100x100", "600x600"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_tracks() {
        let d = json!({"id": 7, "title": "T", "duration": 200, "artist": {"name": "A"}, "album": {"cover_big": "u"}});
        assert_eq!(deezer(&d).map(|h| (h.id, h.seconds)), Some(("7".into(), 200)));
        let a = json!({"trackId": 9, "trackName": "T", "artistName": "A", "trackTimeMillis": 201500,
            "artworkUrl100": "https://x/100x100bb.jpg"});
        let h = apple(&a).unwrap();
        assert_eq!((h.seconds, h.art.as_str()), (201, "https://x/600x600bb.jpg"));
        assert_eq!(apple(&json!({})), None);
    }

    #[test]
    #[ignore = "network: talks to Deezer and Apple"]
    fn live_search() {
        for which in [Which::Deezer, Which::Apple] {
            assert!(!search(which, "mazonakis ores mikres", None).unwrap().hits.is_empty());
        }
    }
}
