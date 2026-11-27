//! Audius: open search and whole MP3s, no key. `https://api.audius.co` lists
//! its servers; the first that answers is kept until it stops answering.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, net, read};
use crate::{Error, Hit, Result};

const DIRECTORY: &str = "https://api.audius.co";
const APP: &str = "djsugar";
const AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
const TIMEOUT: Duration = Duration::from_secs(15);
const PAGE: usize = 30;

#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

#[derive(Debug, Default)]
pub struct Audius {
    server: Mutex<String>,
}

impl Audius {
    pub fn new() -> Self {
        Self::default()
    }

    /// `more` is the offset of the next page.
    pub fn search(&self, query: &str, more: Option<&str>) -> Result<Page> {
        let offset: usize = more.and_then(|m| m.parse().ok()).unwrap_or(0);
        let q = crate::soundcloud::encode(query);
        let v = self.api(&format!("/v1/tracks/search?query={q}&limit={PAGE}&offset={offset}"))?;
        let hits: Vec<Hit> = v["data"].as_array().map(|a| a.iter().filter_map(hit).collect()).unwrap_or_default();
        let more = (hits.len() == PAGE).then(|| (offset + PAGE).to_string());
        Ok(Page { hits, more })
    }

    /// Downloads a track into `dir` as `au<id>.mp3`. Returns the file.
    pub fn fetch(&self, id: &str, dir: &Path, progress: crate::Progress<'_>) -> Result<PathBuf> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(Error::Refused("Not an Audius track".into()));
        }
        let to = dir.join(format!("au{id}.mp3"));
        if to.is_file() {
            return Ok(to);
        }
        std::fs::create_dir_all(dir).map_err(net)?;
        let url = format!("{}/v1/tracks/{id}/stream?app_name={APP}", self.server(false)?);
        crate::http::download(&url, &[("User-Agent", AGENT)], &to, None, 4, progress).map(|_| to)
    }

    fn api(&self, path: &str) -> Result<Value> {
        let sep = if path.contains('?') { '&' } else { '?' };
        let mut last = Error::Refused("No Audius server answered".into());
        for fresh in [false, true] {
            let url = format!("{}{path}{sep}app_name={APP}", self.server(fresh)?);
            match read(
                agent().get(&url).header("User-Agent", AGENT).config().timeout_global(Some(TIMEOUT)).build().call(),
            ) {
                Ok((200..=299, body)) => return serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON")),
                Ok((s, _)) => last = Error::Status(s),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn server(&self, fresh: bool) -> Result<String> {
        let mut s = self.server.lock().unwrap_or_else(PoisonError::into_inner);
        if fresh || s.is_empty() {
            let (_, body) = read(
                agent()
                    .get(DIRECTORY)
                    .header("User-Agent", AGENT)
                    .config()
                    .timeout_global(Some(TIMEOUT))
                    .build()
                    .call(),
            )?;
            let v: Value = serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))?;
            *s = servers(&v).into_iter().next().unwrap_or_else(|| DIRECTORY.to_string());
        }
        Ok(s.clone())
    }
}

fn servers(v: &Value) -> Vec<String> {
    v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter(|s| s.starts_with("https://"))
                .map(|s| s.trim_end_matches('/').to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn hit(t: &Value) -> Option<Hit> {
    if t["is_streamable"] == false {
        return None;
    }
    Some(Hit {
        id: t["id"].as_str()?.to_string(),
        title: t["title"].as_str()?.to_string(),
        artist: t["user"]["name"].as_str().unwrap_or("").to_string(),
        seconds: t["duration"].as_u64().unwrap_or(0) as u32,
        art: ["480x480", "1000x1000", "150x150"]
            .iter()
            .find_map(|k| t["artwork"][k].as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keeps_secure_servers_and_streamable_tracks() {
        let v = json!({"data": ["https://a.example/", "http://insecure", 3, "https://b.example"]});
        assert_eq!(servers(&v), ["https://a.example", "https://b.example"]);
        let t = json!({"id": "x1", "title": "T", "user": {"name": "U"}, "duration": 90, "is_streamable": true});
        assert_eq!(
            hit(&t).map(|h| (h.id, h.title, h.artist, h.seconds)),
            Some(("x1".into(), "T".into(), "U".into(), 90))
        );
        assert_eq!(hit(&json!({"id": "x", "title": "T", "is_streamable": false})), None);
    }

    #[test]
    #[ignore = "network: talks to Audius"]
    fn live_search_and_fetch() {
        let a = Audius::new();
        let page = a.search("daft punk", None).unwrap();
        assert!(!page.hits.is_empty());
        let dir = std::env::temp_dir().join(format!("au-{}", std::process::id()));
        let f = a.fetch(&page.hits[0].id, &dir, &|_, _| true).unwrap();
        assert!(std::fs::metadata(&f).unwrap().len() > 500_000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
