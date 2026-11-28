//! hearthis.at: DJ mixes and tracks their makers put up to be heard, no account needed. Search and the tracks
//! themselves come from the site's open service; a track's stream is an MP3.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, net, read};
use crate::soundcloud::encode;
use crate::{Error, Hit, Result};

const API: &str = "https://api-v2.hearthis.at";
const TIMEOUT: Duration = Duration::from_secs(20);
const PAGE: usize = 20;

#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

#[derive(Debug, Default)]
pub struct HearThis;

impl HearThis {
    /// `more` is the next page's number.
    pub fn search(&self, query: &str, more: Option<&str>) -> Result<Page> {
        let page: usize = more.and_then(|m| m.parse().ok()).unwrap_or(1);
        // Nothing found comes as a note, not a list.
        let v = get(&format!("{API}/search?t={}&page={page}&count={PAGE}", encode(query)))?;
        let hits: Vec<Hit> = v.as_array().map(|a| a.iter().filter_map(hit).collect()).unwrap_or_default();
        let more = (v.as_array().is_some_and(|a| a.len() >= PAGE)).then(|| (page + 1).to_string());
        Ok(Page { hits, more })
    }

    /// `id` is the maker's name and the track's, as the site's addresses have them: `maker/track`.
    pub fn fetch(&self, id: &str, dir: &Path, progress: crate::Progress<'_>) -> Result<PathBuf> {
        let named = |n: &str| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
        let Some((maker, track)) = id.split_once('/').filter(|(m, t)| named(m) && named(t)) else {
            return Err(Error::Refused("Not a hearthis.at track".into()));
        };
        let to = dir.join(format!("ht-{maker}-{track}.mp3"));
        if to.is_file() {
            return Ok(to);
        }
        std::fs::create_dir_all(dir).map_err(net)?;
        let v = get(&format!("{API}/{maker}/{track}/"))?;
        let stream = v["stream_url"].as_str().filter(|s| s.starts_with("https://")).ok_or(Error::Parse("no stream"))?;
        crate::http::download(stream, &[], &to, None, 4, progress).map(|_| to)
    }
}

fn get(url: &str) -> Result<Value> {
    let (status, body) = read(agent().get(url).config().timeout_global(Some(TIMEOUT)).build().call())?;
    if status >= 400 {
        return Err(Error::Status(status));
    }
    serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))
}

fn hit(t: &Value) -> Option<Hit> {
    Some(Hit {
        id: format!("{}/{}", t["user"]["permalink"].as_str()?, t["permalink"].as_str()?),
        title: t["title"].as_str()?.to_string(),
        artist: t["user"]["username"].as_str().unwrap_or("").to_string(),
        // Lengths come written out.
        seconds: t["duration"].as_str().and_then(|d| d.parse().ok()).or(t["duration"].as_u64()).unwrap_or(0) as u32,
        art: t["artwork_url"].as_str().unwrap_or_default().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_a_hit() {
        let t = json!({"title": "Mix", "permalink": "a-mix", "duration": "4312", "user": {"username": "DJ", "permalink": "dj"}});
        let h = hit(&t).unwrap();
        assert_eq!((h.id.as_str(), h.title.as_str(), h.artist.as_str(), h.seconds), ("dj/a-mix", "Mix", "DJ", 4312));
        assert_eq!(hit(&json!({"title": "Mix"})), None);
        assert!(HearThis.fetch("../x", Path::new("."), &|_, _| true).is_err());
    }

    #[test]
    #[ignore = "network: talks to hearthis.at"]
    fn live_search() {
        let h = HearThis;
        let page = h.search("deep house", None).unwrap();
        assert!(page.hits.len() > 5 && page.more.is_some());
        assert!(h.search("zzzzqqqxxnohit", None).unwrap().hits.is_empty());
        let v = get(&format!("{API}/{}/", page.hits[0].id)).unwrap();
        assert!(v["stream_url"].as_str().is_some_and(|s| s.starts_with("https://")));
    }
}
