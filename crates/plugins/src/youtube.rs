//! YouTube as a source in the rail: songs searched (YouTube Music's first,
//! then videos), a pasted link searched as that one video, and the best
//! audio downloaded into the collection.
//!
//! Searches go through Piped's public API, trying each known server in
//! turn and starting from the last one that answered. Downloads ask
//! YouTube itself first, from the DJ's own connection, as its TV, VR and
//! iPhone apps do (those still get plain audio links without a sign-in),
//! then Piped, whose servers YouTube often turns away as bots.
//! After a download, SponsorBlock's community-marked parts that aren't
//! music (intros, outros, talk, sponsors) are saved next to the file as
//! `<file>.sponsorblock.json`, `[[start, end, "category"], ...]` in
//! seconds, so the app can leave them out.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::inbox::safe_name;
use crate::source::{
    DOWNLOAD_FOLDER, DownloadUpdate, Found, Progress, SearchSource, Snapshot, SourceInfo,
    SourceQuery,
};
use crate::{Manifest, Plugin, PluginError, Setting, SettingKind, USER_AGENT, encode};

const MANIFEST: Manifest = Manifest {
    id: "youtube",
    name: "YouTube",
    hosts: &[
        "youtube.com",
        "youtu.be",
        "music.youtube.com",
        "sponsor.ajay.app",
    ],
    downloads: true,
};

/// Piped servers, in the order they're tried.
const PIPED: &[&str] = &[
    "https://api.piped.private.coffee",
    "https://pipedapi.ducks.party",
    "https://pipedapi.kavin.rocks",
];

/// How long connecting to a server, and its first answer, may take before
/// the next server is tried. A download's body has no time limit.
const ASK_TIMEOUT: Duration = Duration::from_secs(6);

/// Pages of results read per filter.
const PAGES: usize = 2;

/// YouTube's own player API, and the apps asked through it, in order:
/// name, version, the number YouTube knows it by, and its user agent.
const PLAYER: &str = "https://www.youtube.com/youtubei/v1/player";
const CLIENTS: &[(&str, &str, &str, &str)] = &[
    (
        "TVHTML5",
        "7.20260707.07.00",
        "7",
        "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/25.lts.30.1034943-gold (unlike Gecko), \
         Unknown_TV_Unknown_0/Unknown (Unknown, Unknown)",
    ),
    (
        "ANDROID_VR",
        "1.65.10",
        "28",
        "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; \
         eureka-user Build/SQ3A.220605.009.A1) gzip",
    ),
    (
        "IOS",
        "21.26.4",
        "5",
        "com.google.ios.youtube/21.26.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
    ),
];

const SPONSORBLOCK: &str = "https://sponsor.ajay.app/api/skipSegments";
const SKIP: &str = r#"["music_offtopic","intro","outro","sponsor","selfpromo"]"#;

/// A play button, in the rail's generic style.
const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 30"><rect width="30" height="30" rx="7" fill="#3a3a3f"/><rect x="5" y="8" width="20" height="14" rx="4" fill="#e8e8ec"/><path d="M13 11.5v7l6-3.5z" fill="#3a3a3f"/></svg>"##;

pub struct YouTube {
    agent: ureq::Agent,
    /// The server that answered last, tried first.
    server: Arc<AtomicUsize>,
    searches: HashMap<String, Snapshot>,
    next: u64,
    updates: Arc<Mutex<Vec<DownloadUpdate>>>,
    cancels: HashMap<String, Arc<AtomicBool>>,
    /// What each download was, so it can be tried again.
    jobs: HashMap<String, (Found, PathBuf)>,
}

impl Default for YouTube {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTube {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(ASK_TIMEOUT))
            .timeout_recv_response(Some(ASK_TIMEOUT))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Self {
            agent,
            server: Arc::default(),
            searches: HashMap::new(),
            next: 0,
            updates: Arc::default(),
            cancels: HashMap::new(),
            jobs: HashMap::new(),
        }
    }

    fn search(&self, text: &str) -> Result<Vec<Found>, PluginError> {
        if let Some(id) = video_id(text) {
            let v = piped(&self.agent, &self.server, &format!("/streams/{id}"))?;
            return Ok(stream_found(&id, &v).into_iter().collect());
        }
        let mut out: Vec<Found> = Vec::new();
        let mut seen = HashSet::new();
        for filter in ["music_songs", "videos"] {
            let q = encode(text);
            let mut path = format!("/search?q={q}&filter={filter}");
            for _ in 0..PAGES {
                let v = piped(&self.agent, &self.server, &path)?;
                for f in parse_search(&v) {
                    if seen.insert(f.key.clone()) {
                        out.push(f);
                    }
                }
                match v["nextpage"].as_str() {
                    Some(next) if !next.is_empty() => {
                        path = format!(
                            "/nextpage/search?q={q}&filter={filter}&nextpage={}",
                            encode(next)
                        );
                    }
                    _ => break,
                }
            }
        }
        Ok(out)
    }

    fn start_job(&mut self, found: Found, dir: PathBuf) -> Result<(), PluginError> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.insert(found.key.clone(), cancel.clone());
        self.jobs
            .insert(found.key.clone(), (found.clone(), dir.clone()));
        push(&self.updates, &found.key, Progress::Queued);
        let job = Job {
            agent: self.agent.clone(),
            server: self.server.clone(),
            found,
            dir,
            updates: self.updates.clone(),
            cancel,
        };
        std::thread::Builder::new()
            .name("youtube".into())
            .spawn(move || job.run())
            .map(|_| ())
            .map_err(|e| PluginError::Network(e.to_string()))
    }
}

impl Plugin for YouTube {
    fn manifest(&self) -> &Manifest {
        &MANIFEST
    }

    fn source(&mut self) -> Option<&mut dyn SearchSource> {
        Some(self)
    }

    fn settings(&self) -> Vec<Setting> {
        vec![
            Setting {
                key: "",
                kind: SettingKind::Note,
                title: "For your own use".into(),
                detail: "Downloads from YouTube are for listening and practice. Playing them \
                         in public needs the rights holders' permission."
                    .into(),
            },
            Setting {
                key: DOWNLOAD_FOLDER,
                kind: SettingKind::Folder,
                title: "Download folder".into(),
                detail: "Left empty, downloads go to Downloaded/YouTube in your music folder."
                    .into(),
            },
        ]
    }
}

impl SearchSource for YouTube {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            name: MANIFEST.name,
            icon_svg: ICON,
            hint: "Artist - Title, or a YouTube link",
        }
    }

    fn sites(&self) -> &'static [&'static str] {
        &["youtube.com", "youtu.be", "music.youtube.com"]
    }

    fn describe(&self, query: &SourceQuery) -> Vec<String> {
        let text = query.text.trim();
        if text.is_empty() {
            Vec::new()
        } else if video_id(text).is_some() {
            vec!["That video".to_string()]
        } else {
            vec![format!("Songs, then videos, matching {text}")]
        }
    }

    fn start(&mut self, query: &SourceQuery) -> Result<String, PluginError> {
        let items = self.search(query.text.trim())?;
        self.next += 1;
        let id = self.next.to_string();
        let note = if items.is_empty() {
            "Nothing found.".to_string()
        } else {
            String::new()
        };
        self.searches.insert(
            id.clone(),
            Snapshot {
                items,
                done: true,
                note,
                title: String::new(),
            },
        );
        Ok(id)
    }

    fn results(&mut self, id: &str) -> Result<Snapshot, PluginError> {
        self.searches
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::Format("no such search".into()))
    }

    fn stop(&mut self, id: &str) {
        self.searches.remove(id);
    }

    fn download(&mut self, _id: &str, items: &[Found], dir: &Path) -> Result<(), PluginError> {
        for f in items {
            self.start_job(f.clone(), dir.to_path_buf())?;
        }
        Ok(())
    }

    fn progress(&mut self) -> Vec<DownloadUpdate> {
        let mut all = self
            .updates
            .lock()
            .map(|mut u| std::mem::take(&mut *u))
            .unwrap_or_default();
        // Only the latest news of each download matters.
        let mut seen = HashSet::new();
        all.reverse();
        all.retain(|u| seen.insert(u.key.clone()));
        all.reverse();
        for u in &all {
            if matches!(u.progress, Progress::Done(_) | Progress::Failed(_)) {
                self.cancels.remove(&u.key);
            }
        }
        all
    }

    fn retry(&mut self, key: &str) -> Result<(), PluginError> {
        let (found, dir) = self
            .jobs
            .get(key)
            .cloned()
            .ok_or(PluginError::NotSupported)?;
        self.start_job(found, dir)
    }

    fn cancel(&mut self, key: &str) -> Result<(), PluginError> {
        if let Some(c) = self.cancels.get(key) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

/// Asks the Piped servers for `path`, starting from the one that answered
/// last.
fn piped(agent: &ureq::Agent, server: &AtomicUsize, path: &str) -> Result<Value, PluginError> {
    let first = server.load(Ordering::Relaxed);
    let mut last = String::new();
    for i in 0..PIPED.len() {
        let at = (first + i) % PIPED.len();
        let answer = agent
            .get(&format!("{}{path}", PIPED[at]))
            .call()
            .and_then(|mut r| r.body_mut().read_json::<Value>())
            .map_err(|e| e.to_string());
        match answer {
            Ok(v) if v.get("error").is_none() => {
                server.store(at, Ordering::Relaxed);
                return Ok(v);
            }
            Ok(v) => last = v["error"].as_str().unwrap_or("refused").to_string(),
            Err(e) => last = e,
        }
    }
    Err(PluginError::Network(format!(
        "YouTube can't be reached right now ({last})."
    )))
}

fn push(updates: &Mutex<Vec<DownloadUpdate>>, key: &str, progress: Progress) {
    if let Ok(mut u) = updates.lock() {
        u.push(DownloadUpdate {
            key: key.to_string(),
            progress,
        });
    }
}

/// The video id in a YouTube link.
fn video_id(text: &str) -> Option<String> {
    let rest = text
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_start_matches("m.");
    let id = if let Some(r) = rest.strip_prefix("youtu.be/") {
        r.split(['?', '&', '/', '#']).next()
    } else if rest.starts_with("youtube.com/") || rest.starts_with("music.youtube.com/") {
        let (_, after) = rest.split_once('/')?;
        if let Some(r) = after.strip_prefix("shorts/") {
            r.split(['?', '&', '/', '#']).next()
        } else {
            let query = after.split_once('?')?.1;
            query.split(['&', '#']).find_map(|p| p.strip_prefix("v="))
        }
    } else {
        None
    }?;
    let ok = id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    ok.then(|| id.to_string())
}

/// Splits a video title into artist and title: a channel named
/// "Artist - Topic" is the artist's own, else "Artist - Title" in the
/// title, else the channel is taken as the artist.
fn artist_title(title: &str, channel: &str) -> (String, String) {
    if let Some(artist) = channel.strip_suffix(" - Topic") {
        return (artist.trim().to_string(), title.trim().to_string());
    }
    for dash in [" - ", " – ", " — "] {
        if let Some((a, t)) = title.split_once(dash) {
            return (a.trim().to_string(), t.trim().to_string());
        }
    }
    (channel.trim().to_string(), title.trim().to_string())
}

fn found(id: &str, title: &str, channel: &str, duration: f64, cover: &str) -> Found {
    let (artist, title) = artist_title(title, channel);
    Found {
        key: id.to_string(),
        artist,
        title,
        duration,
        format: "M4A".into(),
        from: channel.to_string(),
        cover_url: cover.to_string(),
        ..Found::default()
    }
}

/// A Piped search page's songs and videos.
fn parse_search(v: &Value) -> Vec<Found> {
    v["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|i| i["type"] == "stream")
        .filter_map(|i| {
            let id = video_id(&format!("youtube.com{}", i["url"].as_str()?))?;
            Some(found(
                &id,
                i["title"].as_str().unwrap_or_default(),
                i["uploaderName"].as_str().unwrap_or_default(),
                i["duration"].as_f64().unwrap_or_default().max(0.0),
                i["thumbnail"].as_str().unwrap_or_default(),
            ))
        })
        .collect()
}

/// One video, from Piped's stream list for it.
fn stream_found(id: &str, v: &Value) -> Option<Found> {
    Some(found(
        id,
        v["title"].as_str()?,
        v["uploader"].as_str().unwrap_or_default(),
        v["duration"].as_f64().unwrap_or_default().max(0.0),
        v["thumbnailUrl"].as_str().unwrap_or_default(),
    ))
}

/// The best audio in a stream list: AAC (M4A, which every platform's
/// decoder reads) at the highest bitrate, else the highest of any.
/// Returns its address and file extension.
fn best_audio(v: &Value) -> Option<(String, &'static str)> {
    let streams = v["audioStreams"].as_array()?;
    let aac = |s: &&Value| {
        s["mimeType"]
            .as_str()
            .is_some_and(|m| m.starts_with("audio/mp4"))
    };
    let rate = |s: &&Value| s["bitrate"].as_u64().unwrap_or_default();
    let best = streams
        .iter()
        .filter(aac)
        .max_by_key(rate)
        .or_else(|| streams.iter().max_by_key(rate))?;
    let ext = if aac(&best) { "m4a" } else { "webm" };
    Some((best["url"].as_str()?.to_string(), ext))
}

/// What YouTube's player API is asked for one video, as app `name`.
fn player_request(id: &str, name: &str, version: &str) -> Value {
    serde_json::json!({
        "videoId": id,
        "context": {"client": {"clientName": name, "clientVersion": version, "hl": "en"}},
        "contentCheckOk": true,
        "racyCheckOk": true,
        "playbackContext": {"contentPlaybackContext": {"html5Preference": "HTML5_PREF_WANTS"}}
    })
}

/// The best audio in YouTube's player answer, when it's playable and has
/// plain links (not ones that need YouTube's page script to unscramble).
fn player_audio(v: &Value) -> Option<(String, &'static str)> {
    if v["playabilityStatus"]["status"] != "OK" {
        return None;
    }
    let audio: Vec<Value> = v["streamingData"]["adaptiveFormats"]
        .as_array()?
        .iter()
        .filter(|f| f["url"].is_string())
        .filter(|f| {
            f["mimeType"]
                .as_str()
                .is_some_and(|m| m.starts_with("audio/"))
        })
        .cloned()
        .collect();
    best_audio(&serde_json::json!({ "audioStreams": audio }))
}

/// SponsorBlock's answer as `(start, end, category)` parts, in order.
fn parse_skips(v: &Value) -> Vec<(f64, f64, String)> {
    let mut out: Vec<(f64, f64, String)> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let seg = s["segment"].as_array()?;
            let (start, end) = (seg.first()?.as_f64()?, seg.get(1)?.as_f64()?);
            let category = s["category"].as_str().unwrap_or_default().to_string();
            (end > start).then_some((start, end, category))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// One video's download.
struct Job {
    agent: ureq::Agent,
    server: Arc<AtomicUsize>,
    found: Found,
    dir: PathBuf,
    updates: Arc<Mutex<Vec<DownloadUpdate>>>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn run(self) {
        let progress = match self.fetch() {
            Ok(path) => Progress::Done(vec![path]),
            Err(e) => Progress::Failed(e),
        };
        push(&self.updates, &self.found.key, progress);
    }

    fn fetch(&self) -> Result<PathBuf, String> {
        let id = &self.found.key;
        let (url, ext) = match self.direct() {
            Some(found) => found,
            None => {
                let v = piped(&self.agent, &self.server, &format!("/streams/{id}"))
                    .map_err(|e| e.to_string())?;
                best_audio(&v).ok_or("This video has no audio to download.")?
            }
        };
        let folder = self
            .dir
            .join(safe_name(&self.found.artist, "Unknown Artist"));
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let to = folder.join(format!("{}.{ext}", safe_name(&self.found.title, id)));
        if !to.exists() {
            self.save(&url, &to)?;
        }
        self.save_skips(&to);
        Ok(to)
    }

    /// The best audio link YouTube gives one of its own apps, if any does.
    fn direct(&self) -> Option<(String, &'static str)> {
        CLIENTS.iter().find_map(|&(name, version, number, agent)| {
            let body = player_request(&self.found.key, name, version);
            let v: Value = self
                .agent
                .post(PLAYER)
                .header("User-Agent", agent)
                .header("X-YouTube-Client-Name", number)
                .header("X-YouTube-Client-Version", version)
                .content_type("application/json")
                .send(body.to_string())
                .and_then(|mut r| r.body_mut().read_json())
                .ok()?;
            player_audio(&v)
        })
    }

    /// Writes `url` to `to` through a partial file, so a stopped download
    /// leaves nothing that looks finished.
    fn save(&self, url: &str, to: &Path) -> Result<(), String> {
        let mut response = self.agent.get(url).call().map_err(|e| e.to_string())?;
        let total = response.body().content_length().unwrap_or(0).max(1);
        let mut body = response.body_mut().as_reader();
        let part = to.with_extension("part");
        let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut read = 0u64;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                drop(out);
                let _ = std::fs::remove_file(&part);
                return Err("Cancelled.".into());
            }
            let n = body.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            read += n as u64;
            let p = (read as f32 / total as f32).min(1.0);
            push(&self.updates, &self.found.key, Progress::Running(p));
        }
        drop(out);
        std::fs::rename(&part, to).map_err(|e| e.to_string())
    }

    /// SponsorBlock's parts to leave out, next to the file. Best effort: a
    /// video nobody has marked (a 404) or a failure writes nothing.
    fn save_skips(&self, to: &Path) {
        let url = format!(
            "{SPONSORBLOCK}?videoID={}&categories={}",
            encode(&self.found.key),
            encode(SKIP)
        );
        let Ok(v) = self
            .agent
            .get(&url)
            .call()
            .and_then(|mut r| r.body_mut().read_json::<Value>())
        else {
            return;
        };
        let skips: Vec<Value> = parse_skips(&v)
            .into_iter()
            .map(|(s, e, c)| serde_json::json!([s, e, c]))
            .collect();
        if skips.is_empty() {
            return;
        }
        let mut name = to.as_os_str().to_owned();
        name.push(".sponsorblock.json");
        let _ = std::fs::write(PathBuf::from(name), Value::from(skips).to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn video_ids_from_links() {
        let id = Some("dQw4w9WgXcQ".to_string());
        assert_eq!(video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), id);
        assert_eq!(video_id("https://youtu.be/dQw4w9WgXcQ?t=10"), id);
        assert_eq!(
            video_id("https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=RD"),
            id
        );
        assert_eq!(video_id("youtube.com/watch?feature=x&v=dQw4w9WgXcQ"), id);
        assert_eq!(video_id("https://m.youtube.com/shorts/dQw4w9WgXcQ"), id);
        assert_eq!(video_id("Daft Punk - Get Lucky"), None);
        assert_eq!(video_id("https://youtube.com/watch?v=short"), None);
    }

    #[test]
    fn artist_and_title() {
        let pair = |a: &str, t: &str| (a.to_string(), t.to_string());
        assert_eq!(
            artist_title("Get Lucky", "Daft Punk - Topic"),
            pair("Daft Punk", "Get Lucky")
        );
        assert_eq!(
            artist_title("Bonobo – Kerala (Official Video)", "Ninja Tune"),
            pair("Bonobo", "Kerala (Official Video)")
        );
        assert_eq!(artist_title("Kerala", "Bonobo"), pair("Bonobo", "Kerala"));
    }

    #[test]
    fn search_page() {
        let v = json!({
            "items": [
                {"type": "stream", "url": "/watch?v=dQw4w9WgXcQ", "title": "Get Lucky",
                 "uploaderName": "Daft Punk - Topic", "duration": 248,
                 "thumbnail": "https://i/x.jpg"},
                {"type": "channel", "url": "/channel/UC123", "name": "Daft Punk"},
                {"type": "stream", "url": "/watch?v=bad", "title": "Broken"}
            ],
            "nextpage": null
        });
        let found = parse_search(&v);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, "dQw4w9WgXcQ");
        assert_eq!(found[0].artist, "Daft Punk");
        assert_eq!(found[0].title, "Get Lucky");
        assert_eq!(found[0].duration, 248.0);
        assert_eq!(found[0].cover_url, "https://i/x.jpg");
        assert!(!found[0].folder);
    }

    #[test]
    fn best_audio_prefers_aac() {
        let v = json!({"audioStreams": [
            {"url": "o", "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 160000},
            {"url": "lo", "mimeType": "audio/mp4; codecs=\"mp4a.40.5\"", "bitrate": 48000},
            {"url": "hi", "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 128000}
        ]});
        assert_eq!(best_audio(&v), Some(("hi".to_string(), "m4a")));
        let v = json!({"audioStreams": [
            {"url": "a", "mimeType": "audio/webm", "bitrate": 70000},
            {"url": "b", "mimeType": "audio/webm", "bitrate": 160000}
        ]});
        assert_eq!(best_audio(&v), Some(("b".to_string(), "webm")));
        assert_eq!(best_audio(&json!({"audioStreams": []})), None);
    }

    #[test]
    fn youtube_player_answer() {
        let ok = json!({
            "playabilityStatus": {"status": "OK"},
            "streamingData": {"adaptiveFormats": [
                {"url": "v", "mimeType": "video/mp4", "bitrate": 900000},
                {"url": "a1", "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 130000},
                {"signatureCipher": "s=x", "mimeType": "audio/mp4", "bitrate": 260000},
                {"url": "a2", "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 150000}
            ]}
        });
        assert_eq!(player_audio(&ok), Some(("a1".to_string(), "m4a")));
        let blocked = json!({"playabilityStatus": {"status": "LOGIN_REQUIRED"}});
        assert_eq!(player_audio(&blocked), None);
        let body = player_request("dQw4w9WgXcQ", "IOS", "21.26.4");
        assert_eq!(body["videoId"], "dQw4w9WgXcQ");
        assert_eq!(body["context"]["client"]["clientName"], "IOS");
    }

    #[test]
    fn sponsorblock_parts() {
        let v = json!([
            {"segment": [200.5, 230.0], "category": "outro"},
            {"segment": [0.0, 12.25], "category": "intro"},
            {"segment": [50.0, 50.0], "category": "sponsor"}
        ]);
        assert_eq!(
            parse_skips(&v),
            vec![(0.0, 12.25, "intro".into()), (200.5, 230.0, "outro".into())]
        );
        assert!(parse_skips(&json!({"message": "Not Found"})).is_empty());
    }
}
