//! YouTube through its InnerTube API, the one the official apps speak.
//!
//! Search asks as the web client, filtered to videos in the music category.
//! The audio comes from the player endpoint, tried client by client from
//! `res/inner_tube_clients.json` (TV first: it still returns plain stream
//! URLs) until one answers with a playable audio format; AAC in MP4 is
//! preferred since every platform decodes it.

use serde_json::{json, Value};

use crate::{agent, json, Error, Hit, Result, Stream};

const SEARCH_VERSION: &str = "2.20260708.00.00";
const WEB_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
/// Videos only (search filter), so playlists and channels don't crowd the results.
const VIDEOS_ONLY: &str = "EgIQAQ==";

/// One InnerTube client identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Client {
    pub name: String,
    pub version: String,
    pub id: String,
    pub user_agent: String,
    pub device_make: String,
    pub device_model: String,
    pub android_sdk: u32,
    pub os_name: String,
    pub os_version: String,
}

/// The bundled client list; the app may replace it with a newer copy.
pub fn clients(text: Option<&str>) -> Vec<Client> {
    let text = text.unwrap_or(include_str!("../res/inner_tube_clients.json"));
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let s = |c: &Value, k: &str| c[k].as_str().unwrap_or_default().to_string();
    v["clients"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| Client {
                    name: s(c, "name"),
                    version: s(c, "version"),
                    id: s(c, "id"),
                    user_agent: s(c, "user_agent"),
                    device_make: s(c, "device_make"),
                    device_model: s(c, "device_model"),
                    android_sdk: c["android_sdk"].as_u64().unwrap_or(0) as u32,
                    os_name: s(c, "os_name"),
                    os_version: s(c, "os_version"),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn context(c: &Client) -> Value {
    let mut client =
        json!({ "clientName": c.name, "clientVersion": c.version, "hl": "en", "gl": "US" });
    let o = client.as_object_mut().expect("object");
    let mut put = |k: &str, v: &str| {
        if !v.is_empty() {
            o.insert(k.into(), v.into());
        }
    };
    put("userAgent", &c.user_agent);
    put("deviceMake", &c.device_make);
    put("deviceModel", &c.device_model);
    put("osName", &c.os_name);
    put("osVersion", &c.os_version);
    if c.android_sdk > 0 {
        o.insert("androidSdkVersion".into(), c.android_sdk.into());
    }
    json!({ "client": client })
}

fn text(v: &Value) -> String {
    if let Some(s) = v["simpleText"].as_str() {
        return s.to_string();
    }
    v["runs"]
        .as_array()
        .map(|r| r.iter().filter_map(|x| x["text"].as_str()).collect())
        .unwrap_or_default()
}

/// "3:25" or "1:02:03" to seconds.
fn clock(s: &str) -> Option<u32> {
    s.split(':').try_fold(0u32, |acc, p| {
        Some(acc * 60 + p.trim().parse::<u32>().ok()?)
    })
}

/// Every `videoRenderer` in a search reply, in order.
pub fn parse_search(v: &Value) -> Vec<Hit> {
    let mut out = Vec::new();
    walk(v, &mut |r| {
        let Some(id) = r["videoId"].as_str() else {
            return;
        };
        out.push(Hit {
            id: id.to_string(),
            title: text(&r["title"]),
            artist: text(&r["ownerText"]),
            seconds: clock(&text(&r["lengthText"])),
            artwork: r["thumbnail"]["thumbnails"]
                .as_array()
                .and_then(|t| t.last()?["url"].as_str().map(str::to_string)),
        });
    });
    out
}

fn walk(v: &Value, f: &mut impl FnMut(&Value)) {
    match v {
        Value::Object(m) => {
            if let Some(r) = m.get("videoRenderer") {
                f(r);
            }
            for (k, x) in m {
                if k != "videoRenderer" {
                    walk(x, f);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, f)),
        _ => {}
    }
}

pub fn search(query: &str) -> Result<Vec<Hit>> {
    let web = Client {
        name: "WEB".into(),
        version: SEARCH_VERSION.into(),
        id: "1".into(),
        user_agent: WEB_AGENT.into(),
        device_make: String::new(),
        device_model: String::new(),
        android_sdk: 0,
        os_name: String::new(),
        os_version: String::new(),
    };
    let body = json!({ "context": context(&web), "query": query, "params": VIDEOS_ONLY });
    let resp = agent()
        .post("https://www.youtube.com/youtubei/v1/search")
        .header("User-Agent", WEB_AGENT)
        .header("X-YouTube-Client-Name", "1")
        .header("X-YouTube-Client-Version", SEARCH_VERSION)
        .header("Origin", "https://www.youtube.com")
        .send_json(&body);
    Ok(parse_search(&json(resp)?))
}

/// The best audio format of a player reply: AAC in MP4 first, then the highest bitrate.
pub fn pick_audio(player: &Value) -> Result<Stream> {
    let status = player["playabilityStatus"]["status"].as_str().unwrap_or("");
    if status != "OK" {
        let why = player["playabilityStatus"]["reason"]
            .as_str()
            .unwrap_or(status);
        return Err(Error::Refused(format!("YouTube: {why}")));
    }
    let formats = player["streamingData"]["adaptiveFormats"]
        .as_array()
        .ok_or(Error::Parse("no formats"))?;
    let best = formats
        .iter()
        .filter(|f| {
            f["mimeType"]
                .as_str()
                .is_some_and(|m| m.starts_with("audio/"))
                && f["url"].is_string()
        })
        .max_by_key(|f| {
            let m4a = f["mimeType"].as_str().is_some_and(|m| m.contains("mp4a"));
            (m4a, f["bitrate"].as_u64().unwrap_or(0))
        })
        .ok_or(Error::Refused(
            "YouTube: no plain audio stream for this client".into(),
        ))?;
    let mime = best["mimeType"].as_str().unwrap_or_default();
    Ok(Stream {
        url: best["url"].as_str().unwrap_or_default().to_string(),
        headers: Vec::new(),
        ext: if mime.contains("mp4a") { "m4a" } else { "webm" },
    })
}

/// Audio for a video, trying each client in turn.
pub fn stream(video: &str, clients: &[Client]) -> Result<Stream> {
    let mut last = Error::Refused("YouTube: no clients".into());
    for c in clients {
        let body = json!({
            "videoId": video,
            "context": context(c),
            "contentCheckOk": true,
            "racyCheckOk": true,
            "playbackContext": { "contentPlaybackContext": { "html5Preference": "HTML5_PREF_WANTS" } },
        });
        let resp = agent()
            .post("https://www.youtube.com/youtubei/v1/player")
            .header("User-Agent", &c.user_agent)
            .header("X-YouTube-Client-Name", &c.id)
            .header("X-YouTube-Client-Version", &c.version)
            .header("Origin", "https://www.youtube.com")
            .send_json(&body);
        match json(resp).and_then(|v| pick_audio(&v)) {
            Ok(mut s) => {
                s.headers.push(("User-Agent".into(), c.user_agent.clone()));
                return Ok(s);
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_clients_load() {
        let c = clients(None);
        assert!(c.len() >= 3);
        assert_eq!(c[0].name, "TVHTML5");
    }

    #[test]
    fn reads_search_results() {
        let v = json!({"contents": {"a": [{"videoRenderer": {
            "videoId": "abc", "title": {"runs": [{"text": "Song"}]},
            "ownerText": {"runs": [{"text": "Band"}]}, "lengthText": {"simpleText": "1:02:03"},
            "thumbnail": {"thumbnails": [{"url": "s"}, {"url": "l"}]}}}, {"channelRenderer": {}}]}});
        let h = parse_search(&v);
        assert_eq!(h.len(), 1);
        assert_eq!(
            (h[0].title.as_str(), h[0].artist.as_str(), h[0].seconds),
            ("Song", "Band", Some(3723))
        );
        assert_eq!(h[0].artwork.as_deref(), Some("l"));
    }

    #[test]
    fn prefers_aac_audio() {
        let v = json!({"playabilityStatus": {"status": "OK"}, "streamingData": {"adaptiveFormats": [
            {"mimeType": "video/mp4", "url": "v", "bitrate": 9_000_000},
            {"mimeType": "audio/webm; codecs=\"opus\"", "url": "o", "bitrate": 160_000},
            {"mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "url": "m", "bitrate": 128_000},
            {"mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "signatureCipher": "x", "bitrate": 256_000}]}});
        let s = pick_audio(&v).unwrap();
        assert_eq!((s.url.as_str(), s.ext), ("m", "m4a"));
    }

    #[test]
    fn reports_refusals() {
        let v = json!({"playabilityStatus": {"status": "LOGIN_REQUIRED", "reason": "Sign in"}});
        assert!(matches!(pick_audio(&v), Err(Error::Refused(m)) if m.contains("Sign in")));
    }
}
