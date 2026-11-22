//! Audius: an open music network whose API lets any app stream its tracks.

use serde_json::Value;

use crate::{agent, enc, json, Hit, Result, Stream};

const HOST: &str = "https://discoveryprovider.audius.co/v1";
const APP: &str = "DJSugar";

pub fn parse(v: &Value) -> Vec<Hit> {
    v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|t| t["is_streamable"].as_bool() != Some(false))
                .filter_map(|t| {
                    Some(Hit {
                        id: t["id"].as_str()?.to_string(),
                        title: t["title"].as_str().unwrap_or_default().to_string(),
                        artist: t["user"]["name"].as_str().unwrap_or_default().to_string(),
                        seconds: t["duration"].as_u64().map(|d| d as u32),
                        artwork: t["artwork"]["480x480"].as_str().map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn search(query: &str) -> Result<Vec<Hit>> {
    let url = format!(
        "{HOST}/tracks/search?query={}&app_name={APP}",
        enc(query.trim())
    );
    Ok(parse(&json(agent().get(&url).call())?))
}

pub fn trending() -> Result<Vec<Hit>> {
    Ok(parse(&json(
        agent()
            .get(&format!("{HOST}/tracks/trending?app_name={APP}"))
            .call(),
    )?))
}

pub fn stream(id: &str) -> Stream {
    Stream {
        url: format!("{HOST}/tracks/{}/stream?app_name={APP}", enc(id)),
        headers: Vec::new(),
        ext: "mp3",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn reads_tracks_and_skips_unstreamable() {
        let v = json!({"data": [
            {"id": "a1", "title": "T", "user": {"name": "U"}, "duration": 200, "artwork": {"480x480": "art"}},
            {"id": "a2", "title": "Gated", "is_streamable": false}]});
        let h = super::parse(&v);
        assert_eq!(h.len(), 1);
        assert_eq!(
            (h[0].id.as_str(), h[0].artist.as_str(), h[0].seconds),
            ("a1", "U", Some(200))
        );
        assert!(super::stream("a1")
            .url
            .ends_with("/tracks/a1/stream?app_name=DJSugar"));
    }
}
