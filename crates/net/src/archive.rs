//! The Internet Archive: freely licensed audio (netlabels, live sets, public
//! domain). Search lists items under a Creative Commons or public domain
//! licence; fetching takes the item's first MP3 or Ogg file.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, net, read};
use crate::soundcloud::encode;
use crate::{Error, Hit, Result};

const SITE: &str = "https://archive.org";
const TIMEOUT: Duration = Duration::from_secs(20);
const PAGE: usize = 30;

#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

#[derive(Debug, Default)]
pub struct Archive;

impl Archive {
    /// `more` is the next page's number.
    pub fn search(&self, query: &str, more: Option<&str>) -> Result<Page> {
        let page: usize = more.and_then(|m| m.parse().ok()).unwrap_or(1);
        let q = format!(
            "({query}) AND mediatype:(audio) AND format:(MP3 OR \"VBR MP3\" OR \"Ogg Vorbis\" OR Flac) \
             AND licenseurl:(*creativecommons.org* OR *publicdomain*)"
        );
        let url = format!(
            "{SITE}/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&fl[]=creator&rows={PAGE}&page={page}&output=json",
            encode(&q)
        );
        let v = get(&url)?;
        let hits: Vec<Hit> =
            v["response"]["docs"].as_array().map(|a| a.iter().filter_map(hit).collect()).unwrap_or_default();
        let more = (hits.len() == PAGE).then(|| (page + 1).to_string());
        Ok(Page { hits, more })
    }

    /// Downloads an item's first playable file into `dir`. Returns the file.
    pub fn fetch(&self, id: &str, dir: &Path, progress: &(dyn Fn(u64, u64) + Sync)) -> Result<PathBuf> {
        if id.is_empty() || id.contains(['/', '\\']) || id.starts_with('.') {
            return Err(Error::Refused("Not an Archive item".into()));
        }
        for ext in ["mp3", "ogg", "flac"] {
            let f = dir.join(format!("ia-{id}.{ext}"));
            if f.is_file() {
                return Ok(f);
            }
        }
        std::fs::create_dir_all(dir).map_err(net)?;
        let meta = get(&format!("{SITE}/metadata/{}", encode(id)))?;
        let files = meta["files"].as_array().ok_or(Error::Parse("no files"))?;
        let (name, ext, size) = pick(files).ok_or_else(|| Error::Refused("This item has no audio file".into()))?;
        let to = dir.join(format!("ia-{id}.{ext}"));
        let path: Vec<String> = name.split('/').map(encode).collect();
        let url = format!("{SITE}/download/{}/{}", encode(id), path.join("/"));
        crate::http::download(&url, &[], &to, size, 4, progress).map(|_| to)
    }
}

fn get(url: &str) -> Result<Value> {
    let (status, body) = read(agent().get(url).config().timeout_global(Some(TIMEOUT)).build().call())?;
    if status >= 400 {
        return Err(Error::Status(status));
    }
    serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))
}

/// The item's first file a deck can play, best kind first: name, extension, size.
fn pick(files: &[Value]) -> Option<(String, &'static str, Option<u64>)> {
    for (format, ext) in [("VBR MP3", "mp3"), ("MP3", "mp3"), ("Ogg Vorbis", "ogg"), ("Flac", "flac")] {
        let found = files.iter().find(|f| f["format"].as_str().is_some_and(|x| x.starts_with(format)));
        if let Some(f) = found {
            let size = f["size"].as_str().and_then(|s| s.parse().ok());
            return Some((f["name"].as_str()?.to_string(), ext, size));
        }
    }
    None
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    }
}

fn hit(d: &Value) -> Option<Hit> {
    let id = d["identifier"].as_str()?.to_string();
    let title = text(&d["title"]);
    let art = format!("https://archive.org/services/img/{id}");
    Some(Hit {
        title: if title.is_empty() { id.clone() } else { title },
        artist: text(&d["creator"]),
        seconds: 0,
        id,
        art,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_the_best_playable_file() {
        let files = json!([
            {"name": "cover.jpg", "format": "JPEG"},
            {"name": "a.ogg", "format": "Ogg Vorbis", "size": "100"},
            {"name": "b.mp3", "format": "MP3 (other)"},
            {"name": "d/a.mp3", "format": "VBR MP3", "size": "200"},
        ]);
        assert_eq!(pick(files.as_array().unwrap()), Some(("d/a.mp3".into(), "mp3", Some(200))));
        assert_eq!(pick(&[json!({"name": "x.txt", "format": "Text"})]), None);
        let d = json!({"identifier": "id1", "title": "T", "creator": ["A", "B"]});
        assert_eq!(
            hit(&d).map(|h| (h.id, h.title, h.artist, h.art)),
            Some(("id1".into(), "T".into(), "A, B".into(), "https://archive.org/services/img/id1".into()))
        );
    }

    #[test]
    #[ignore = "network: talks to the Internet Archive"]
    fn live_search_and_fetch() {
        let a = Archive;
        let page = a.search("techno netlabel", None).unwrap();
        assert!(!page.hits.is_empty());
        let dir = std::env::temp_dir().join(format!("ia-{}", std::process::id()));
        let got = page.hits.iter().take(5).find_map(|h| a.fetch(&h.id, &dir, &|_, _| {}).ok());
        assert!(std::fs::metadata(got.expect("one of the first items downloads")).unwrap().len() > 100_000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
