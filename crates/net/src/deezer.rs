//! Deezer: lyrics without an account. A search by artist and title, the hit of the track's length, then that song's
//! lyrics: timed word by word where Deezer has them so, else by line, else the plain words.

use std::fmt::Write as _;
use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{agent, read};
use crate::{Error, Result};

const SEARCH: &str = "https://api.deezer.com/search";
/// Hands out the pass a visitor reads lyrics with.
const VISITOR: &str = "https://auth.deezer.com/login/anonymous?jo=p&rto=c&i=c";
const LYRICS: &str = "https://pipe.deezer.com/api";
const ASKED: &str = "query($id:String!){track(trackId:$id){lyrics{text synchronizedLines{milliseconds line} \
                     synchronizedWordByWordLines{words{start end word}}}}}";
/// Seconds a hit's length may differ from the track's and still count.
const SLACK: f64 = 4.0;
const TIMEOUT: Duration = Duration::from_secs(12);

fn json_of(sent: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value> {
    match read(sent)? {
        (200..=299, body) => serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON")),
        (s, _) => Err(Error::Status(s)),
    }
}

fn get(url: &str) -> Result<Value> {
    json_of(agent().get(url).config().timeout_global(Some(TIMEOUT)).build().call())
}

fn stamp(ms: u64) -> String {
    format!("{:02}:{:02}.{:02}", ms / 60_000, ms / 1000 % 60, ms % 1000 / 10)
}

/// What Deezer has of a song's lyrics, the finest timing first.
fn lrc(lyrics: &Value) -> Option<String> {
    let mut out = String::new();
    for l in lyrics["synchronizedWordByWordLines"].as_array().into_iter().flatten() {
        let words: Vec<(u64, u64, &str)> = l["words"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| Some((w["start"].as_u64()?, w["end"].as_u64()?, w["word"].as_str()?)))
            .collect();
        let (Some(first), Some(last)) = (words.first(), words.last()) else { continue };
        let _ = write!(out, "[{}]", stamp(first.0));
        for (at, _, word) in &words {
            let _ = write!(out, "<{}>{word} ", stamp(*at));
        }
        let _ = writeln!(out, "<{}>", stamp(last.1));
    }
    if out.is_empty() {
        for l in lyrics["synchronizedLines"].as_array().into_iter().flatten() {
            if let (Some(at), Some(line)) = (l["milliseconds"].as_u64(), l["line"].as_str()) {
                let _ = writeln!(out, "[{}]{}", stamp(at), line.trim());
            }
        }
    }
    if out.is_empty() {
        out = lyrics["text"].as_str().unwrap_or_default().replace('\r', "");
    }
    let out = out.trim().to_string();
    (!out.is_empty()).then_some(out)
}

/// The found song of the track's length, any length when `seconds` is 0.
fn pick(found: &Value, seconds: f64) -> Option<u64> {
    let fits = |h: &&Value| seconds <= 0.0 || h["duration"].as_f64().is_some_and(|s| (s - seconds).abs() <= SLACK);
    found["data"].as_array()?.iter().find(fits)?["id"].as_u64()
}

/// A track's lyrics, timed where Deezer has them timed; None when it has none.
pub fn find(artist: &str, title: &str, seconds: f64) -> Result<Option<String>> {
    if artist.trim().is_empty() || title.trim().is_empty() {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(&format!("artist:\"{}\" track:\"{}\"", artist.trim(), title.trim()));
    let Some(id) = pick(&get(&format!("{SEARCH}?q={q}&limit=5"))?, seconds) else { return Ok(None) };
    let pass = get(VISITOR)?["jwt"].as_str().ok_or(Error::Parse("no pass"))?.to_string();
    let asked = json!({"query": ASKED, "variables": {"id": id.to_string()}}).to_string();
    let call = agent()
        .post(LYRICS)
        .header("Authorization", &format!("Bearer {pass}"))
        // The lyrics service leaves a caller that does not say who it is waiting.
        .header("User-Agent", crate::pages::OURS)
        .header("Content-Type", "application/json")
        .config()
        .timeout_global(Some(TIMEOUT))
        .build();
    Ok(lrc(&json_of(call.send(asked))?["data"]["track"]["lyrics"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_finest_timing_it_is_given() {
        let lines = json!([{"milliseconds": 30350, "line": " One more time "}]);
        let words = json!([{"words": [{"start": 30362, "end": 30662, "word": "One"}, {"start": 30812, "end": 31837, "word": "more"}]}]);
        let all = json!({"text": "One more time", "synchronizedLines": lines, "synchronizedWordByWordLines": words});
        assert_eq!(lrc(&all).unwrap(), "[00:30.36]<00:30.36>One <00:30.81>more <00:31.83>");
        let lined = json!({"text": "One more time", "synchronizedLines": lines, "synchronizedWordByWordLines": null});
        assert_eq!(lrc(&lined).unwrap(), "[00:30.35]One more time");
        assert_eq!(lrc(&json!({"text": "One\r\nmore"})).unwrap(), "One\nmore");
        assert_eq!(lrc(&Value::Null), None);
        let found = json!({"data": [{"id": 1, "duration": 235}, {"id": 2, "duration": 320}]});
        assert_eq!((pick(&found, 321.0), pick(&found, 0.0), pick(&found, 100.0)), (Some(2), Some(1), None));
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time", 320.0).unwrap().unwrap();
        assert!(l.contains(">One <") && l.contains(">more <"), "{l}");
        assert!(find("Daft Punk", "One More Time", 100.0).unwrap().is_none());
    }
}
