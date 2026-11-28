//! Bandcamp: the tracks its artists let anyone hear in full on their pages. Search is the one behind the site's search
//! box; fetching reads the track's page for the stream its player plays, an MP3.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{agent, net, read};
use crate::{Error, Hit, Result};

const SEARCH: &str = "https://bandcamp.com/api/bcsearch_public_api/1/autocomplete_elastic";
/// The page's own note of the track, kept in this attribute.
const NOTE: &str = "data-tralbum=\"";
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

#[derive(Debug, Default)]
pub struct Bandcamp;

impl Bandcamp {
    /// Everything found comes at once, so there is never more.
    pub fn search(&self, query: &str, _more: Option<&str>) -> Result<Page> {
        let asked = json!({"search_text": query, "search_filter": "t", "full_page": true, "fan_id": null}).to_string();
        let call = agent().post(SEARCH).header("Content-Type", "application/json");
        let v: Value = match read(call.config().timeout_global(Some(TIMEOUT)).build().send(asked))? {
            (200..=299, body) => serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))?,
            (s, _) => return Err(Error::Status(s)),
        };
        let hits = v["auto"]["results"].as_array().map(|a| a.iter().filter_map(hit).collect()).unwrap_or_default();
        Ok(Page { hits, more: None })
    }

    /// `id` is the track's page.
    pub fn fetch(&self, id: &str, dir: &Path, progress: crate::Progress<'_>) -> Result<PathBuf> {
        if !id.starts_with("https://") {
            return Err(Error::Refused("Not a Bandcamp track".into()));
        }
        let (status, html) = read(agent().get(id).config().timeout_global(Some(TIMEOUT)).build().call())?;
        if status >= 400 {
            return Err(Error::Status(status));
        }
        let (number, stream) = stream(&html).ok_or_else(|| Error::Refused("This track is not open to hear".into()))?;
        let to = dir.join(format!("bc-{number}.mp3"));
        if to.is_file() {
            return Ok(to);
        }
        std::fs::create_dir_all(dir).map_err(net)?;
        crate::http::download(&stream, &[], &to, None, 4, progress).map(|_| to)
    }
}

/// The track's number and the stream of it, from its page.
fn stream(html: &str) -> Option<(u64, String)> {
    let note = html.split_once(NOTE)?.1.split_once('"')?.0;
    let note = note.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
    let v: Value = serde_json::from_str(&note).ok()?;
    let track = &v["trackinfo"][0];
    let stream = track["file"]["mp3-128"].as_str().filter(|s| s.starts_with("https://"))?;
    Some((track["track_id"].as_u64()?, stream.to_string()))
}

fn hit(t: &Value) -> Option<Hit> {
    Some(Hit {
        id: t["item_url_path"].as_str().filter(|u| u.starts_with("https://"))?.to_string(),
        title: t["name"].as_str()?.to_string(),
        artist: t["band_name"].as_str().unwrap_or("").to_string(),
        seconds: 0,
        art: t["img"].as_str().unwrap_or_default().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_hit_and_a_page() {
        let t = json!({"type": "t", "name": "Kerala", "band_name": "Bonobo", "item_url_path": "https://b.example/track/kerala", "img": "https://i.example/1.jpg"});
        let h = hit(&t).unwrap();
        assert_eq!(
            (h.id.as_str(), h.title.as_str(), h.artist.as_str()),
            ("https://b.example/track/kerala", "Kerala", "Bonobo")
        );
        assert_eq!(hit(&json!({"name": "x", "item_url_path": "javascript:1"})), None);
        let page = r#"<script data-tralbum="{&quot;trackinfo&quot;:[{&quot;track_id&quot;:7,&quot;file&quot;:{&quot;mp3-128&quot;:&quot;https://s.example/a?p=0&amp;t=1&quot;}}]}" src="x">"#;
        assert_eq!(stream(page), Some((7, "https://s.example/a?p=0&t=1".to_string())));
        assert_eq!(
            stream(
                r#"<script data-tralbum="{&quot;trackinfo&quot;:[{&quot;track_id&quot;:7,&quot;file&quot;:null}]}">"#
            ),
            None
        );
    }

    #[test]
    #[ignore = "network: talks to Bandcamp"]
    fn live_search_and_fetch() {
        let b = Bandcamp;
        let page = b.search("bonobo kerala", None).unwrap();
        assert!(!page.hits.is_empty());
        let dir = std::env::temp_dir().join(format!("bc-{}", std::process::id()));
        let got = page.hits.iter().take(5).find_map(|h| b.fetch(&h.id, &dir, &|_, _| true).ok());
        assert!(std::fs::metadata(got.expect("one of the first tracks downloads")).unwrap().len() > 500_000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
