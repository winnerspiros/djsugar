//! Mixcloud: DJ mixes and shows, no account needed. Search comes from the public API; a mix's stream from the
//! site's own query, whose addresses come scrambled with a key every visitor's page has.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{agent, net, read};
use crate::soundcloud::encode;
use crate::{Error, Hit, Result};

const API: &str = "https://api.mixcloud.com";
const QUERY: &str = "https://app.mixcloud.com/graphql";
const ASKED: &str =
    "query($user:String!,$slug:String!){cloudcastLookup(lookup:{username:$user,slug:$slug}){streamInfo{url hlsUrl}}}";
/// What the addresses are scrambled with, repeated along them.
const KEY: &[u8] = b"IFYOUWANTTHEARTISTSTOGETPAIDDONOTDOWNLOADFROMMIXCLOUD";
const TIMEOUT: Duration = Duration::from_secs(20);
const PAGE: usize = 20;

#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

#[derive(Debug, Default)]
pub struct Mixcloud;

impl Mixcloud {
    /// `more` is how many hits were already given.
    pub fn search(&self, query: &str, more: Option<&str>) -> Result<Page> {
        let from: usize = more.and_then(|m| m.parse().ok()).unwrap_or(0);
        let v = get(&format!("{API}/search/?q={}&type=cloudcast&limit={PAGE}&offset={from}", encode(query)))?;
        let hits: Vec<Hit> = v["data"].as_array().map(|a| a.iter().filter_map(hit).collect()).unwrap_or_default();
        let more = v["paging"]["next"].is_string().then(|| (from + PAGE).to_string());
        Ok(Page { hits, more })
    }

    /// `id` is the mix's address: `maker/mix`, as the site has it.
    pub fn fetch(&self, id: &str, dir: &Path, progress: crate::Progress<'_>) -> Result<PathBuf> {
        let named = |n: &str| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
        let Some((maker, mix)) = id.split_once('/').filter(|(m, t)| named(m) && named(t)) else {
            return Err(Error::Refused("Not a Mixcloud mix".into()));
        };
        let to = dir.join(format!("mc-{maker}-{mix}.m4a"));
        if to.is_file() {
            return Ok(to);
        }
        std::fs::create_dir_all(dir).map_err(net)?;
        let body = json!({ "query": ASKED, "variables": { "user": maker, "slug": mix } });
        let sent = agent()
            .post(QUERY)
            .config()
            .timeout_global(Some(TIMEOUT))
            .build()
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0")
            .send_json(body);
        let v = match read(sent)? {
            (200..=299, text) => serde_json::from_str::<Value>(&text).map_err(|_| Error::Parse("not JSON"))?,
            (s, _) => return Err(Error::Status(s)),
        };
        let info = &v["data"]["cloudcastLookup"]["streamInfo"];
        let url =
            info["url"].as_str().and_then(unscramble).ok_or(Error::Refused("This mix can't be streamed".into()))?;
        crate::http::download(&url, &[], &to, None, 4, progress).map(|_| to)
    }
}

/// An address as the site sends it: written in base64 over the key.
fn unscramble(sent: &str) -> Option<String> {
    let bytes = crate::botguard::b64_decode(sent)?;
    let plain: Vec<u8> = bytes.iter().zip(KEY.iter().cycle()).map(|(b, k)| b ^ k).collect();
    String::from_utf8(plain).ok().filter(|u| u.starts_with("https://"))
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
        id: t["key"].as_str()?.trim_matches('/').to_owned(),
        title: t["name"].as_str()?.to_owned(),
        artist: t["user"]["name"].as_str().unwrap_or_default().to_owned(),
        seconds: t["audio_length"].as_u64().unwrap_or(0) as u32,
        art: t["pictures"]["large"].as_str().unwrap_or_default().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_hit_and_an_address() {
        let t = json!({"key": "/dj/a-mix/", "name": "Mix", "audio_length": 3600,
            "user": {"name": "DJ"}, "pictures": {"large": "https://x/y.jpg"}});
        let h = hit(&t).unwrap();
        assert_eq!((h.id.as_str(), h.artist.as_str(), h.seconds), ("dj/a-mix", "DJ", 3600));
        assert_eq!(hit(&json!({"name": "x"})), None);
        let scrambled: Vec<u8> =
            b"https://dl.mixcloud.com/a.m4a".iter().zip(KEY.iter().cycle()).map(|(b, k)| b ^ k).collect();
        assert_eq!(unscramble(&crate::botguard::b64_url(&scrambled)).as_deref(), Some("https://dl.mixcloud.com/a.m4a"));
        assert_eq!(unscramble("!!"), None);
        assert!(Mixcloud.fetch("../x", Path::new("."), &|_, _| true).is_err());
    }

    #[test]
    #[ignore = "network: talks to Mixcloud"]
    fn live_search_and_stream() {
        let page = Mixcloud.search("deep house", None).unwrap();
        assert!(page.hits.len() > 5 && page.more.is_some());
        let (maker, mix) = page.hits[0].id.split_once('/').unwrap();
        let v = json!({ "query": ASKED, "variables": { "user": maker, "slug": mix } });
        let (_, body) = read(agent().post(QUERY).send_json(v)).unwrap();
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(unscramble(v["data"]["cloudcastLookup"]["streamInfo"]["url"].as_str().unwrap()).is_some());
    }
}
