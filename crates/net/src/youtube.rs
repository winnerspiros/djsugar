//! YouTube as DJ Sugar does it: InnerTube search and player clients in its order,
//! then Piped. AAC first; paced 200 ms apart, 8 per 10 s; bot flags rest 30 s to 10 min.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::http::{agent, download, net, read};
use crate::{Error, Hit, Result};

/// One InnerTube client identity.
#[derive(Clone, Copy, Debug)]
pub struct Client {
    pub name: &'static str,
    pub version: &'static str,
    pub id: &'static str,
    pub agent: &'static str,
    pub make: &'static str,
    pub model: &'static str,
    pub sdk: u32,
    pub os: &'static str,
    pub os_version: &'static str,
}

const fn client(name: &'static str, version: &'static str, id: &'static str, agent: &'static str) -> Client {
    Client { name, version, id, agent, make: "", model: "", sdk: 0, os: "", os_version: "" }
}

const VR: Client = Client {
    make: "Oculus",
    model: "Quest 3",
    sdk: 32,
    os: "Android",
    os_version: "12L",
    ..client(
        "ANDROID_VR",
        "1.65.10",
        "28",
        "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
    )
};
const PLAYSTATION: Client = client(
    "TVHTML5_SIMPLY",
    "1.0",
    "75",
    "Mozilla/5.0 (PlayStation; PlayStation 4/12.00) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15",
);

/// Who to ask for audio, in order.
pub const PLAYER_CLIENTS: [Client; 7] = [
    // Needs no player script: its links carry no signature to solve.
    Client {
        make: "Apple",
        model: "RealityDevice17,1",
        os: "visionOS",
        os_version: "26.5.23O471",
        ..client(
            "VISIONOS",
            "1.02",
            "101",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
        )
    },
    client(
        "TVHTML5",
        "7.20260707.07.00",
        "7",
        "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/25.lts.30.1034943-gold (unlike Gecko), Unknown_TV_Unknown_0/Unknown (Unknown, Unknown)",
    ),
    client("TVHTML5", "5.20260707", "7", "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/Version"),
    VR,
    Client {
        make: "Apple",
        model: "iPhone16,2",
        os: "iPhone",
        os_version: "18.3.2.22D82",
        ..client("IOS", "21.26.4", "5", "com.google.ios.youtube/21.26.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)")
    },
    Client {
        sdk: 30,
        os: "Android",
        os_version: "11",
        ..client("ANDROID", "21.26.364", "3", "com.google.android.youtube/21.26.364 (Linux; U; Android 11) gzip")
    },
    PLAYSTATION,
];

/// Who to ask for searches, in order.
pub const SEARCH_CLIENTS: [Client; 3] = [
    VR,
    PLAYSTATION,
    client(
        "WEB",
        "2.20260708.00.00",
        "1",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    ),
];

pub const PIPED: [&str; 2] = ["https://api.piped.private.coffee", "https://pipedapi.ducks.party"];

const SEARCH_TIMEOUT: Duration = Duration::from_secs(10);
const PIPED_TIMEOUT: Duration = Duration::from_secs(6);
const MIN_GAP: Duration = Duration::from_millis(200);
const MAX_GAP: Duration = Duration::from_millis(500);
const WINDOW: Duration = Duration::from_secs(10);
const BURST: usize = 8;
const REST_FIRST: Duration = Duration::from_secs(30);
const REST_MAX: Duration = Duration::from_secs(600);
/// Files this big or bigger are downloaded as parallel ranges.
pub const CHUNKS: u64 = 16;
const RESULTS: usize = 60;

/// One page of search results and the token for the next page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

/// An audio file a client offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Format {
    pub url: String,
    pub m4a: bool,
    pub bitrate: u64,
    /// Bytes, when the listing says.
    pub size: Option<u64>,
}

impl Format {
    pub fn ext(&self) -> &'static str {
        if self.m4a { "m4a" } else { "webm" }
    }
}

#[derive(Debug, Default)]
struct Pacing {
    sent: VecDeque<Instant>,
    rest_until: Option<Instant>,
    rest: Duration,
    visitor: String,
}

/// A YouTube session: pacing, bot backoff and the visitor ID, shared by all calls.
#[derive(Debug, Default)]
pub struct YouTube {
    state: Mutex<Pacing>,
    /// Region for searches, two letters ("US", "GR").
    pub region: String,
}

/// Tokens minted on the device that let token-gated clients play a video.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PoTokens {
    pub player: String,
    pub streaming: String,
    pub visitor: String,
}

/// Mints tokens for (video, visitor ID); the platform layer installs it.
pub type PoTokenSource = fn(&str, &str) -> std::result::Result<PoTokens, String>;

static PO_TOKEN_SOURCE: OnceLock<PoTokenSource> = OnceLock::new();

pub fn set_po_token_source(f: PoTokenSource) {
    let _ = PO_TOKEN_SOURCE.set(f);
}

impl YouTube {
    pub fn new(region: &str, visitor: &str) -> Self {
        let s = Self { region: region.to_string(), ..Self::default() };
        s.lock().visitor = visitor.to_string();
        s
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Pacing> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The visitor ID to keep for the next start.
    pub fn visitor(&self) -> String {
        self.lock().visitor.clone()
    }

    /// Whether InnerTube is resting after a bot flag.
    pub fn resting(&self) -> bool {
        self.lock().rest_until.is_some_and(|t| t > Instant::now())
    }

    /// Waits for a slot: at least 200 to 500 ms after the last request, at most 8 in 10 s.
    fn pace(&self) {
        let wait = {
            let mut p = self.lock();
            let now = Instant::now();
            while p.sent.front().is_some_and(|t| now.duration_since(*t) > WINDOW) {
                p.sent.pop_front();
            }
            let jitter = MIN_GAP + (MAX_GAP - MIN_GAP).mul_f64(jitter());
            let after_last = p.sent.back().map_or(Duration::ZERO, |t| (*t + jitter).saturating_duration_since(now));
            let after_burst = if p.sent.len() >= BURST {
                (p.sent[0] + WINDOW).saturating_duration_since(now)
            } else {
                Duration::ZERO
            };
            let wait = after_last.max(after_burst);
            p.sent.push_back(now + wait);
            wait
        };
        std::thread::sleep(wait);
    }

    fn flag(&self) {
        let mut p = self.lock();
        p.rest = if p.rest.is_zero() { REST_FIRST } else { (p.rest * 2).min(REST_MAX) };
        p.rest_until = Some(Instant::now() + p.rest);
    }

    fn context(&self, c: &Client) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("clientName".into(), c.name.into());
        m.insert("clientVersion".into(), c.version.into());
        m.insert("hl".into(), "en".into());
        m.insert("gl".into(), if self.region.is_empty() { "US" } else { &self.region }.into());
        for (k, v) in [
            ("userAgent", c.agent),
            ("deviceMake", c.make),
            ("deviceModel", c.model),
            ("osName", c.os),
            ("osVersion", c.os_version),
        ] {
            if !v.is_empty() {
                m.insert(k.into(), v.into());
            }
        }
        if c.sdk > 0 {
            m.insert("androidSdkVersion".into(), c.sdk.into());
        }
        let visitor = self.visitor();
        if !visitor.is_empty() {
            m.insert("visitorData".into(), visitor.into());
        }
        json!({ "client": m })
    }

    fn post(&self, endpoint: &str, c: &Client, mut body: Value, timeout: Duration) -> Result<(u16, Value, String)> {
        self.pace();
        body["context"] = self.context(c);
        let mut req = agent()
            .post(format!("https://www.youtube.com/youtubei/v1/{endpoint}"))
            .config()
            .timeout_global(Some(timeout))
            .build()
            .header("Content-Type", "application/json")
            .header("User-Agent", c.agent)
            .header("X-YouTube-Client-Name", c.id)
            .header("X-YouTube-Client-Version", c.version)
            .header("Origin", "https://www.youtube.com");
        let visitor = self.visitor();
        if !visitor.is_empty() {
            req = req.header("X-Goog-Visitor-Id", &visitor);
        }
        let (status, text) = read(req.send(body.to_string()))?;
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if let Some(vd) = v["responseContext"]["visitorData"].as_str().filter(|s| !s.is_empty()) {
            self.lock().visitor = vd.to_string();
        }
        Ok((status, v, text))
    }

    /// Searches music videos; `more` continues a previous page.
    pub fn search(&self, query: &str, more: Option<&str>) -> Result<Page> {
        let mut last = Error::Refused("YouTube search failed".into());
        for c in &SEARCH_CLIENTS {
            let body = match more {
                Some(token) => json!({ "continuation": token }),
                None => json!({ "query": query, "params": "EgIQAQ==", "videoCategoryId": 10 }),
            };
            match self.post("search", c, body, SEARCH_TIMEOUT) {
                Ok((200, v, _)) => {
                    let hits = parse_search(&v, RESULTS);
                    if !hits.is_empty() {
                        return Ok(Page { hits, more: continuation(&v) });
                    }
                    last = Error::Parse("no videos");
                }
                Ok((status, ..)) => last = Error::Status(status),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// The audio formats YouTube offers for a video, best first.
    /// Offers each client's formats to `take` in turn, moving on when it fails:
    /// a stream 403 gates only that client, so the next one may still serve it.
    fn each_source<T>(
        &self,
        video: &str,
        tokens: Option<&PoTokens>,
        mut take: impl FnMut(Vec<Format>, &'static str) -> Result<T>,
    ) -> Result<T> {
        if !valid_id(video) {
            return Err(Error::Refused("Not a YouTube video".into()));
        }
        let mut last = Error::Refused("All InnerTube clients failed".into());
        let mut refused = 0;
        if !self.resting() {
            for c in &PLAYER_CLIENTS {
                let mut body = json!({
                    "videoId": video,
                    "contentCheckOk": true,
                    "racyCheckOk": true,
                    "playbackContext": { "contentPlaybackContext": { "html5Preference": "HTML5_PREF_WANTS" } },
                });
                if let Some(t) = tokens.filter(|t| !t.player.is_empty()) {
                    body["playbackContext"]["serviceIntegrityDimensions"] = json!({ "poToken": t.player });
                }
                let (status, v, raw) = match self.post("player", c, body, SEARCH_TIMEOUT) {
                    Ok(r) => r,
                    Err(e) => {
                        last = e;
                        continue;
                    }
                };
                // A bot check gates that client only; the next may still answer.
                if let Some(why) = bot_check(status, &v, &raw) {
                    refused += 1;
                    last = Error::Refused(why);
                    continue;
                }
                match player_formats(&v).and_then(|f| take(f, c.agent)) {
                    Ok(t) => return Ok(t),
                    Err(e) => last = e,
                }
            }
            // Every client refused: back off before asking again.
            if refused == PLAYER_CLIENTS.len() {
                self.flag();
            }
        }
        for host in PIPED {
            let resp = agent()
                .get(format!("{host}/streams/{video}"))
                .config()
                .timeout_global(Some(PIPED_TIMEOUT))
                .build()
                .call();
            match read(resp) {
                Ok((200, text)) => {
                    let v: Value = serde_json::from_str(&text).map_err(|_| Error::Parse("Piped"))?;
                    let f = piped_formats(&v);
                    if !f.is_empty() {
                        match take(f, "") {
                            Ok(t) => return Ok(t),
                            Err(e) => last = e,
                        }
                    }
                }
                Ok(_) | Err(_) if matches!(last, Error::Refused(_)) => {}
                Ok((s, _)) => last = Error::Status(s),
                Err(e) => last = e,
            }
        }
        Err(match last {
            Error::Refused(why) => Error::Refused(format!("{why}; the Piped servers refused too")),
            e => e,
        })
    }

    /// Downloads a video's audio into `dir` as `<id>.m4a` or `<id>.webm`,
    /// trying each offered format until one arrives. Returns the file.
    pub fn fetch(&self, video: &str, dir: &Path, progress: &(dyn Fn(u64, u64) + Sync)) -> Result<PathBuf> {
        for ext in ["m4a", "webm"] {
            let f = dir.join(format!("{video}.{ext}"));
            if f.is_file() {
                return Ok(f);
            }
        }
        std::fs::create_dir_all(dir).map_err(net)?;
        let minted = self.mint(video);
        let tokens = minted.as_ref().ok().cloned();
        let out = self.each_source(video, tokens.as_ref(), |formats, ua| {
            let mut last = Error::Refused("No usable audio stream URL".into());
            for f in formats {
                let to = dir.join(format!("{video}.{}", f.ext()));
                let headers: &[(&str, &str)] = if ua.is_empty() { &[] } else { &[("User-Agent", ua)] };
                // Proxied mirror links carry no signature, so they take no token.
                let url = match &tokens {
                    Some(t) if !ua.is_empty() && !t.streaming.is_empty() && !f.url.contains("pot=") => {
                        format!("{}{}pot={}", f.url, if f.url.contains('?') { '&' } else { '?' }, t.streaming)
                    }
                    _ => f.url.clone(),
                };
                match download(&url, headers, &to, f.size, CHUNKS, progress) {
                    Ok(_) => return Ok(to),
                    Err(Error::Status(403)) => return Err(Error::Status(403)),
                    Err(e) => last = e,
                }
            }
            Err(last)
        });
        // Without a token the real cause is usually the mint, so the DJ sees it.
        match (out, minted) {
            (Err(e), Err(why)) if PO_TOKEN_SOURCE.get().is_some() => Err(Error::Refused(format!("{e} (token: {why})"))),
            (out, _) => out,
        }
    }

    /// Mints tokens for one download; a fresh visitor ID from the mint is kept.
    fn mint(&self, video: &str) -> std::result::Result<PoTokens, String> {
        let source = PO_TOKEN_SOURCE.get().ok_or("no token source")?;
        let t = source(video, &self.visitor())?;
        if !t.visitor.is_empty() && t.visitor != video {
            self.lock().visitor.clone_from(&t.visitor);
        }
        Ok(t)
    }
}

fn jitter() -> f64 {
    let n = Instant::now().elapsed().subsec_nanos() ^ std::process::id().rotate_left(13);
    f64::from(n.wrapping_mul(2_654_435_761) >> 8) / f64::from(1u32 << 24)
}

/// Checks whether a video ID looks valid (11 chars, URL-safe base64 alphabet).
pub fn valid_id(id: &str) -> bool {
    id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Why YouTube turned a player request down as a bot check, if it did.
pub fn bot_check(status: u16, v: &Value, raw: &str) -> Option<String> {
    if status == 429 {
        return Some("YouTube rate limit (HTTP 429)".into());
    }
    let play = &v["playabilityStatus"];
    let state = play["status"].as_str().unwrap_or("");
    let reason = play["reason"].as_str().unwrap_or("");
    if state == "LOGIN_REQUIRED" {
        return Some("YouTube requires login (bot detection)".into());
    }
    let r = reason.to_lowercase();
    if ["sign in", "bot", "unusual traffic", "automated", "verify", "confirm"].iter().any(|w| r.contains(w)) {
        return Some(format!("YouTube bot detection: {reason}"));
    }
    if v.is_null() && raw.len() > 100 {
        let head = raw[..raw.len().min(4096)].to_lowercase();
        if ["consent.youtube", "captcha", "before you continue", "confirm your identity", "unusual traffic"]
            .iter()
            .any(|w| head.contains(w))
        {
            return Some("YouTube consent/captcha challenge".into());
        }
    }
    None
}

fn text(v: &Value) -> String {
    if let Some(s) = v["simpleText"].as_str() {
        return s.to_string();
    }
    v["runs"].as_array().map(|r| r.iter().filter_map(|x| x["text"].as_str()).collect()).unwrap_or_default()
}

/// "3:25" or "1:02:03" to seconds; None for live streams and junk.
fn clock(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    s.split(':').try_fold(0u32, |acc, p| Some(acc * 60 + p.trim().parse::<u32>().ok()?))
}

/// Videos in a search reply (`compactVideoRenderer` or `videoRenderer`),
/// in order, without live streams or repeats.
pub fn parse_search(v: &Value, cap: usize) -> Vec<Hit> {
    fn walk(v: &Value, out: &mut Vec<Hit>, seen: &mut HashSet<String>, cap: usize) {
        if out.len() >= cap {
            return;
        }
        match v {
            Value::Object(m) => {
                if let Some(r) =
                    m.get("compactVideoRenderer").or_else(|| m.get("videoRenderer")).filter(|r| r.is_object())
                {
                    let id = r["videoId"].as_str().unwrap_or("");
                    let title = text(&r["title"]);
                    let mut artist = text(&r["longBylineText"]);
                    if artist.is_empty() {
                        artist = text(&r["shortBylineText"]);
                    }
                    if let Some(seconds) = clock(&text(&r["lengthText"]))
                        && valid_id(id)
                        && !title.is_empty()
                        && seen.insert(id.to_string())
                    {
                        out.push(Hit { id: id.to_string(), title, artist, seconds });
                    }
                    return;
                }
                m.values().for_each(|x| walk(x, out, seen, cap));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out, seen, cap)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out, &mut HashSet::new(), cap);
    out
}

/// The first continuation token in a reply.
pub fn continuation(v: &Value) -> Option<String> {
    match v {
        Value::Object(m) => {
            if let Some(t) = m.get("nextContinuationData").and_then(|n| n["continuation"].as_str()) {
                return Some(t.to_string());
            }
            if let Some(t) = m.get("continuationCommand").and_then(|n| n["token"].as_str()) {
                return Some(t.to_string());
            }
            m.values().find_map(continuation)
        }
        Value::Array(a) => a.iter().find_map(continuation),
        _ => None,
    }
}

fn rank(mut f: Vec<Format>) -> Vec<Format> {
    f.sort_by(|a, b| b.m4a.cmp(&a.m4a).then(b.bitrate.cmp(&a.bitrate)));
    f
}

/// Plain audio URLs of a player reply, best first. Ciphered ones are skipped.
pub fn player_formats(v: &Value) -> Result<Vec<Format>> {
    let state = v["playabilityStatus"]["status"].as_str().unwrap_or("");
    if state != "OK" {
        let why = v["playabilityStatus"]["reason"].as_str().filter(|s| !s.is_empty()).unwrap_or(state);
        return Err(Error::Refused(format!("YouTube: {why}")));
    }
    let list = v["streamingData"]["adaptiveFormats"].as_array().ok_or(Error::Parse("no formats"))?;
    let f: Vec<Format> = list
        .iter()
        .filter_map(|f| {
            let mime = f["mimeType"].as_str()?;
            mime.starts_with("audio/").then_some(())?;
            Some(Format {
                url: f["url"].as_str()?.to_string(),
                m4a: mime.contains("mp4"),
                bitrate: f["bitrate"].as_u64().unwrap_or(0),
                size: f["contentLength"].as_str().and_then(|n| n.parse().ok()),
            })
        })
        .collect();
    if f.is_empty() {
        return Err(Error::Refused("YouTube: no plain audio stream for this client".into()));
    }
    Ok(rank(f))
}

/// Audio of a Piped `/streams` reply, best first.
pub fn piped_formats(v: &Value) -> Vec<Format> {
    let f = v["audioStreams"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    let mime = s["mimeType"].as_str().unwrap_or("").to_lowercase();
                    let codec = s["codec"].as_str().unwrap_or("").to_lowercase();
                    Some(Format {
                        url: s["url"].as_str().filter(|u| !u.is_empty())?.to_string(),
                        m4a: mime.contains("mp4") || codec.starts_with("mp4a"),
                        bitrate: s["bitrate"].as_u64().unwrap_or(0),
                        size: s["contentLength"].as_u64().filter(|&n| n > 0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    rank(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_renderers_and_skips_live_and_repeats() {
        let v = json!({"a": [
            {"compactVideoRenderer": {"videoId": "aaaaaaaaaaa", "title": {"runs": [{"text": "One"}]},
                "longBylineText": {"runs": [{"text": "Band"}]}, "lengthText": {"simpleText": "1:02:03"}}},
            {"videoRenderer": {"videoId": "bbbbbbbbbbb", "title": {"simpleText": "Live"}}},
            {"videoRenderer": {"videoId": "aaaaaaaaaaa", "title": {"simpleText": "Again"}, "lengthText": {"simpleText": "3:00"}}},
            {"videoRenderer": {"videoId": "ccccccccccc", "title": {"simpleText": "Two"},
                "shortBylineText": {"runs": [{"text": "Solo"}]}, "lengthText": {"simpleText": "3:25"}}},
            {"continuationItemRenderer": {"continuationEndpoint": {"continuationCommand": {"token": "NEXT"}}}}]});
        let h = parse_search(&v, 60);
        assert_eq!(
            h.iter().map(|h| (h.title.as_str(), h.artist.as_str(), h.seconds)).collect::<Vec<_>>(),
            [("One", "Band", 3723), ("Two", "Solo", 205)]
        );
        assert_eq!(continuation(&v).as_deref(), Some("NEXT"));
        assert_eq!(parse_search(&v, 1).len(), 1);
    }

    #[test]
    fn ranks_aac_first_then_bitrate_and_skips_ciphers() {
        let v = json!({"playabilityStatus": {"status": "OK"}, "streamingData": {"adaptiveFormats": [
            {"mimeType": "video/mp4", "url": "v", "bitrate": 9_000_000},
            {"mimeType": "audio/webm; codecs=\"opus\"", "url": "o", "bitrate": 160_000},
            {"mimeType": "audio/mp4; codecs=\"mp4a.40.5\"", "url": "lo", "bitrate": 48_000},
            {"mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "url": "hi", "bitrate": 128_000},
            {"mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "signatureCipher": "x", "bitrate": 256_000}]}});
        let f = player_formats(&v).unwrap();
        assert_eq!(f.iter().map(|f| f.url.as_str()).collect::<Vec<_>>(), ["hi", "lo", "o"]);
        assert_eq!(f[2].ext(), "webm");
    }

    #[test]
    fn spots_bot_checks() {
        assert!(bot_check(429, &Value::Null, "").is_some());
        let v = json!({"playabilityStatus": {"status": "UNPLAYABLE", "reason": "Sign in to confirm you're not a bot"}});
        assert!(bot_check(200, &v, "").unwrap().contains("not a bot"));
        assert!(bot_check(200, &json!({"playabilityStatus": {"status": "LOGIN_REQUIRED"}}), "").is_some());
        let page = format!("<html>{}Before you continue to YouTube</html>", " ".repeat(100));
        assert!(bot_check(200, &Value::Null, &page).is_some());
        assert!(bot_check(200, &json!({"playabilityStatus": {"status": "OK"}}), "").is_none());
    }

    #[test]
    fn reads_piped_streams() {
        let v = json!({"audioStreams": [
            {"url": "a", "mimeType": "audio/webm", "codec": "opus", "bitrate": 160_000},
            {"url": "b", "mimeType": "audio/mp4", "codec": "mp4a.40.2", "bitrate": 128_000},
            {"url": "", "mimeType": "audio/mp4", "bitrate": 999_999}]});
        assert_eq!(piped_formats(&v).iter().map(|f| f.url.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    }

    #[test]
    fn backs_off_when_flagged() {
        let yt = YouTube::new("GR", "vd");
        assert!(!yt.resting());
        yt.flag();
        assert!(yt.resting());
        yt.flag();
        assert_eq!(yt.lock().rest, REST_FIRST * 2);
        assert_eq!(yt.context(&VR)["client"]["gl"], "GR");
        assert_eq!(yt.context(&VR)["client"]["visitorData"], "vd");
        assert_eq!(yt.context(&PLAYER_CLIENTS[1])["client"].get("deviceMake"), None);
        assert_eq!(yt.context(&PLAYER_CLIENTS[0])["client"]["osName"], "visionOS");
        assert!(!valid_id("abc") && valid_id("dQw4w9WgXcQ"));
    }
}
