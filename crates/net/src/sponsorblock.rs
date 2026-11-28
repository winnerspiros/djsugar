//! SponsorBlock: the parts of a YouTube video its viewers have marked as not the music.

use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, read};
use crate::{Error, Result};

const API: &str = "https://sponsor.ajay.app/api/skipSegments";
/// Sponsors, self promotion, calls to subscribe, talk within a music video, and the intros, outros, previews and filler
/// around it.
const PARTS: &str = "%5B%22sponsor%22%2C%22selfpromo%22%2C%22interaction%22%2C%22music_offtopic%22%2C%22intro%22%2C%22outro%22%2C%22preview%22%2C%22filler%22%2C%22hook%22%5D";

/// The marked parts of `video` in seconds, in order and never overlapping; none when nobody has marked any.
/// The server is asked by the start of the ID's hash, and answers for every video sharing it: it isn't told
/// which one is played.
pub fn segments(video: &str) -> Result<Vec<(f64, f64)>> {
    use sha2::Digest;
    if !crate::youtube::valid_id(video) {
        return Err(Error::Parse("not a video ID"));
    }
    let hash = sha2::Sha256::digest(video.as_bytes());
    let url = format!("{API}/{:02x}{:02x}?categories={PARTS}", hash[0], hash[1]);
    let call = agent().get(&url).config().timeout_global(Some(Duration::from_secs(8))).build().call();
    match read(call)? {
        (200, body) => Ok(of(video, &serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))?)),
        (404, _) => Ok(Vec::new()),
        (s, _) => Err(Error::Status(s)),
    }
}

/// The parts of `video` among those of the videos a reply is about.
fn of(video: &str, v: &Value) -> Vec<(f64, f64)> {
    let about = v.as_array().into_iter().flatten().find(|about| about["videoID"] == video);
    about.map(|about| parse(&about["segments"])).unwrap_or_default()
}

/// The parts to skip in a reply, joined where they touch or overlap.
pub fn parse(v: &Value) -> Vec<(f64, f64)> {
    let part = |s: &Value| {
        let (a, b) = (s["segment"][0].as_f64()?, s["segment"][1].as_f64()?);
        (s["actionType"].as_str().unwrap_or("skip") == "skip" && b > a && a >= 0.0).then_some((a, b))
    };
    let mut parts: Vec<_> = v.as_array().into_iter().flatten().filter_map(part).collect();
    parts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut joined: Vec<(f64, f64)> = Vec::new();
    for (a, b) in parts {
        match joined.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => joined.push((a, b)),
        }
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_are_ordered_and_joined() {
        let v = serde_json::json!([
            {"segment": [200.0, 215.5], "category": "outro", "actionType": "skip"},
            {"segment": [0.0, 12.0], "category": "intro", "actionType": "skip"},
            {"segment": [10.0, 14.0], "category": "music_offtopic"},
            {"segment": [50.0, 60.0], "category": "sponsor", "actionType": "mute"},
            {"segment": [90.0, 80.0], "category": "filler", "actionType": "skip"},
        ]);
        assert_eq!(parse(&v), vec![(0.0, 14.0), (200.0, 215.5)]);
        assert!(parse(&serde_json::json!({"error": "none"})).is_empty());
        let shared =
            serde_json::json!([{"videoID": "aaaaaaaaaaa", "segments": []}, {"videoID": "bbbbbbbbbbb", "segments": v}]);
        assert_eq!(of("bbbbbbbbbbb", &shared).len(), 2);
        assert!(of("ccccccccccc", &shared).is_empty());
    }
}
