//! SoundCloud: search and fetch through the web player's own API. The player's
//! client ID is read from its scripts once and read again when it is refused.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, net, read};
use crate::{Error, Hit, Result};

const SITE: &str = "https://soundcloud.com/";
const API: &str = "https://api-v2.soundcloud.com";
const AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
const TIMEOUT: Duration = Duration::from_secs(15);
const PAGE: u32 = 30;

/// A page of results and the address of the next.
#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

#[derive(Debug, Default)]
pub struct SoundCloud {
    client: Mutex<String>,
    /// The signed in account's token; Go+ tracks play in full with it.
    token: Mutex<String>,
}

impl SoundCloud {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sign_in(&self, token: &str) {
        token.trim().clone_into(&mut self.token.lock().unwrap_or_else(PoisonError::into_inner));
    }

    pub fn search(&self, query: &str, more: Option<&str>) -> Result<Page> {
        let url = match more {
            Some(next) => next.to_string(),
            None => format!("{API}/search/tracks?q={}&limit={PAGE}", encode(query)),
        };
        let v = self.api(&url)?;
        let hits = v["collection"].as_array().map(|a| a.iter().filter_map(hit).collect()).unwrap_or_default();
        let more = v["next_href"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
        Ok(Page { hits, more })
    }

    /// Downloads a track into `dir` as `sc<id>.mp3` or `.m4a`. Returns the file.
    pub fn fetch(&self, id: &str, dir: &Path, progress: &(dyn Fn(u64, u64) + Sync)) -> Result<PathBuf> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::Refused("Not a SoundCloud track".into()));
        }
        for ext in ["mp3", "m4a"] {
            let f = dir.join(format!("sc{id}.{ext}"));
            if f.is_file() {
                return Ok(f);
            }
        }
        std::fs::create_dir_all(dir).map_err(net)?;
        let t = self.api(&format!("{API}/tracks/{id}"))?;
        let auth = t["track_authorization"].as_str().unwrap_or("");
        let list = t["media"]["transcodings"].as_array().ok_or(Error::Parse("no transcodings"))?;
        let (url, hls, ext) = pick(list).ok_or_else(|| {
            Error::Refused("SoundCloud only streams this track protected; it can not be saved".into())
        })?;
        let stream =
            self.api(&format!("{url}{}track_authorization={auth}", if url.contains('?') { '&' } else { '?' }))?;
        let to = dir.join(format!("sc{id}.{ext}"));
        let src = stream["url"].as_str().ok_or(Error::Parse("no stream address"))?;
        if hls { playlist(src, &to, progress) } else { crate::http::download(src, &[], &to, None, 4, progress) }
            .map(|_| to)
    }

    /// A GET on the API with the client ID; a refused ID is read again once.
    fn api(&self, url: &str) -> Result<Value> {
        for fresh in [false, true] {
            let id = self.client_id(fresh)?;
            let sep = if url.contains('?') { '&' } else { '?' };
            let mut req = agent().get(format!("{url}{sep}client_id={id}")).header("User-Agent", AGENT);
            let token = self.token.lock().unwrap_or_else(PoisonError::into_inner).clone();
            if !token.is_empty() {
                req = req.header("Authorization", &format!("OAuth {token}"));
            }
            let (status, body) = read(req.config().timeout_global(Some(TIMEOUT)).build().call())?;
            match status {
                401 | 403 if !fresh => continue,
                200..=299 => return serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON")),
                s => return Err(Error::Status(s)),
            }
        }
        Err(Error::Refused("SoundCloud refused its own player's key".into()))
    }

    fn client_id(&self, fresh: bool) -> Result<String> {
        let mut c = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        if fresh || c.is_empty() {
            *c = scrape()?;
        }
        Ok(c.clone())
    }
}

/// The client ID from the player's scripts; the last script usually has it.
fn scrape() -> Result<String> {
    let get = |u: &str| {
        read(agent().get(u).header("User-Agent", AGENT).config().timeout_global(Some(TIMEOUT)).build().call())
    };
    let (_, page) = get(SITE)?;
    let scripts: Vec<&str> = page
        .split("<script crossorigin src=\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .filter(|s| s.ends_with(".js"))
        .collect();
    for src in scripts.iter().rev() {
        let (_, js) = get(src)?;
        let id = js.split("client_id:\"").nth(1).and_then(|s| s.split('"').next());
        if let Some(id) = id.filter(|id| id.len() >= 20 && id.bytes().all(|b| b.is_ascii_alphanumeric())) {
            return Ok(id.to_string());
        }
    }
    Err(Error::Parse("no client ID in the player"))
}

/// The best open stream: a whole MP3, else an AAC playlist of the best
/// quality, else an MP3 playlist. Returns its address, if it is a playlist,
/// and the file's extension.
fn pick(list: &[Value]) -> Option<(String, bool, &'static str)> {
    let open = |t: &&Value, proto: &str, mime: &str| {
        t["format"]["protocol"] == proto
            && t["format"]["mime_type"].as_str().is_some_and(|m| m.starts_with(mime))
            && t["snipped"] != true
    };
    let url = |t: &Value| t["url"].as_str().map(str::to_string);
    if let Some(t) = list.iter().find(|t| open(t, "progressive", "audio/mpeg")) {
        return Some((url(t)?, false, "mp3"));
    }
    let aac = list.iter().filter(|t| open(t, "hls", "audio/mp4")).max_by_key(|t| t["quality"] == "hq");
    if let Some(t) = aac {
        return Some((url(t)?, true, "m4a"));
    }
    let t = list.iter().find(|t| open(t, "hls", "audio/mpeg"))?;
    Some((url(t)?, true, "mp3"))
}

/// Joins a playlist's header part and parts into one file that plays whole.
fn playlist(src: &str, to: &Path, progress: &(dyn Fn(u64, u64) + Sync)) -> Result<u64> {
    let (status, list) = read(agent().get(src).call())?;
    if status >= 400 {
        return Err(Error::Status(status));
    }
    if list.contains("#EXT-X-KEY") && !list.contains("METHOD=NONE") {
        return Err(Error::Refused("SoundCloud only streams this track protected; it can not be saved".into()));
    }
    let init = list
        .lines()
        .find_map(|l| l.strip_prefix("#EXT-X-MAP:"))
        .and_then(|m| m.split("URI=\"").nth(1))
        .and_then(|u| u.split('"').next());
    let parts: Vec<&str> =
        init.into_iter().chain(list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'))).collect();
    if parts.is_empty() {
        return Err(Error::Parse("empty playlist"));
    }
    let part = to.with_extension("part");
    let result = (|| {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&part).map_err(net)?);
        let mut bytes = 0;
        for (n, url) in parts.iter().enumerate() {
            let mut tries = 0;
            let body = loop {
                match read_bytes(url) {
                    Ok(b) => break b,
                    Err(Error::Status(s)) if s < 500 => return Err(Error::Status(s)),
                    Err(e) if tries >= 4 => return Err(e),
                    Err(_) => tries += 1,
                }
            };
            out.write_all(&body).map_err(net)?;
            bytes += body.len() as u64;
            progress(n as u64 + 1, parts.len() as u64);
        }
        out.flush().map_err(net)?;
        Ok(bytes)
    })();
    match result {
        Ok(n) => std::fs::rename(&part, to).map_err(net).map(|()| n),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

fn read_bytes(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent().get(url).config().timeout_global(Some(TIMEOUT)).build().call().map_err(net)?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(Error::Status(status));
    }
    resp.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(net)
}

fn hit(t: &Value) -> Option<Hit> {
    let id = t["id"].as_u64()?;
    let title = t["title"].as_str()?.to_string();
    let artist = t["publisher_metadata"]["artist"]
        .as_str()
        .filter(|a| !a.is_empty())
        .or_else(|| t["user"]["username"].as_str())
        .unwrap_or("")
        .to_string();
    let ms = t["full_duration"].as_u64().or_else(|| t["duration"].as_u64()).unwrap_or(0);
    Some(Hit { id: id.to_string(), title, artist, seconds: (ms / 1000) as u32 })
}

pub(crate) fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_a_plain_mp3_and_skips_locked_streams() {
        let locked = json!([{"url": "a", "format": {"protocol": "ctr-encrypted-hls", "mime_type": "audio/mpegurl"}}]);
        assert_eq!(pick(locked.as_array().unwrap()), None);
        let open = json!([
            {"url": "aac", "format": {"protocol": "hls", "mime_type": "audio/mp4; codecs=\"mp4a.40.2\""}},
            {"url": "mp3", "format": {"protocol": "hls", "mime_type": "audio/mpeg"}},
        ]);
        assert_eq!(pick(open.as_array().unwrap()), Some(("aac".into(), true, "m4a")));
        let whole = json!([
            {"url": "aac", "format": {"protocol": "hls", "mime_type": "audio/mp4"}},
            {"url": "file", "format": {"protocol": "progressive", "mime_type": "audio/mpeg"}},
        ]);
        assert_eq!(pick(whole.as_array().unwrap()), Some(("file".into(), false, "mp3")));
    }

    #[test]
    fn hits_name_the_artist_first() {
        let t = json!({"id": 7, "title": "T", "duration": 61_500,
            "user": {"username": "up"}, "publisher_metadata": {"artist": "Art"}});
        assert_eq!(hit(&t), Some(Hit { id: "7".into(), title: "T".into(), artist: "Art".into(), seconds: 61 }));
        assert_eq!(encode("a b&c"), "a%20b%26c");
    }

    #[test]
    #[ignore = "network: talks to SoundCloud"]
    fn live_search_and_fetch() {
        let sc = SoundCloud::new();
        let page = sc.search("daft punk around the world edit", None).unwrap();
        assert!(!page.hits.is_empty());
        let dir = std::env::temp_dir().join(format!("sc-{}", std::process::id()));
        let got = page.hits.iter().take(8).find_map(|h| {
            let r = sc.fetch(&h.id, &dir, &|_, _| {});
            println!("{} {}: {r:?}", h.id, h.title);
            r.ok()
        });
        let f = got.expect("one of the first hits downloads");
        assert!(std::fs::metadata(&f).unwrap().len() > 500_000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
