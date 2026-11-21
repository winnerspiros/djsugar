//! Audius as a source in the rail: what's trending (an empty search) and
//! search, each track a single file downloaded into the collection.
//!
//! Audius needs no key. `https://api.audius.co` lists its servers; the
//! first that answers is used until it stops answering. Tracks come from
//! `/v1/tracks/search` and `/v1/tracks/trending`, audio from
//! `/v1/tracks/<id>/stream` (an MP3, at the quality the artist uploaded).

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
    id: "audius",
    name: "Audius",
    hosts: &["audius.co"],
    downloads: true,
};

/// Lists the servers.
const DIRECTORY: &str = "https://api.audius.co";
/// Used when the list can't be had.
const FALLBACK: &str = "https://discoveryprovider.audius.co";
const APP: &str = "app_name=djsugar";

/// How long connecting, and the first answer, may take.
const ASK_TIMEOUT: Duration = Duration::from_secs(8);

/// A triangle, in the rail's generic style.
const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 30"><rect width="30" height="30" rx="7" fill="#3a3a3f"/><path d="M15 7.5 23.5 22h-17z" fill="none" stroke="#e8e8ec" stroke-width="2.6" stroke-linejoin="round"/></svg>"##;

pub struct Audius {
    agent: ureq::Agent,
    /// The server in use, once one has answered.
    server: Option<String>,
    searches: HashMap<String, Snapshot>,
    next: u64,
    updates: Arc<Mutex<Vec<DownloadUpdate>>>,
    cancels: HashMap<String, Arc<AtomicBool>>,
    /// What each download was, so it can be tried again.
    jobs: HashMap<String, (Found, PathBuf)>,
}

impl Default for Audius {
    fn default() -> Self {
        Self::new()
    }
}

impl Audius {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(ASK_TIMEOUT))
            .timeout_recv_response(Some(ASK_TIMEOUT))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Self {
            agent,
            server: None,
            searches: HashMap::new(),
            next: 0,
            updates: Arc::default(),
            cancels: HashMap::new(),
            jobs: HashMap::new(),
        }
    }

    fn get(&self, url: &str) -> Result<Value, String> {
        self.agent
            .get(url)
            .call()
            .and_then(|mut r| r.body_mut().read_json::<Value>())
            .map_err(|e| e.to_string())
    }

    /// Asks for `path` from the server in use, or from each listed one in
    /// turn until one answers, which is then kept.
    fn ask(&mut self, path: &str) -> Result<Value, PluginError> {
        if let Some(server) = &self.server
            && let Ok(v) = self.get(&format!("{server}{path}"))
        {
            return Ok(v);
        }
        let mut servers: Vec<String> = self.get(DIRECTORY).map(|v| servers(&v)).unwrap_or_default();
        servers.push(FALLBACK.to_string());
        let mut last = String::new();
        for server in servers {
            match self.get(&format!("{server}{path}")) {
                Ok(v) => {
                    self.server = Some(server);
                    return Ok(v);
                }
                Err(e) => last = e,
            }
        }
        self.server = None;
        Err(PluginError::Network(format!(
            "Audius can't be reached right now ({last})."
        )))
    }

    fn start_job(&mut self, found: Found, dir: PathBuf) -> Result<(), PluginError> {
        let server = self.server.clone().unwrap_or_else(|| FALLBACK.to_string());
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.insert(found.key.clone(), cancel.clone());
        self.jobs
            .insert(found.key.clone(), (found.clone(), dir.clone()));
        push(&self.updates, &found.key, Progress::Queued);
        let job = Job {
            agent: self.agent.clone(),
            url: format!("{server}/v1/tracks/{}/stream?{APP}", encode(&found.key)),
            found,
            dir,
            updates: self.updates.clone(),
            cancel,
        };
        std::thread::Builder::new()
            .name("audius".into())
            .spawn(move || job.run())
            .map(|_| ())
            .map_err(|e| PluginError::Network(e.to_string()))
    }
}

impl Plugin for Audius {
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
                title: "Shared by the artists".into(),
                detail: "Audius tracks are put up by their artists to stream. Ask them \
                         before playing one at a paid gig."
                    .into(),
            },
            Setting {
                key: DOWNLOAD_FOLDER,
                kind: SettingKind::Folder,
                title: "Download folder".into(),
                detail: "Left empty, downloads go to Downloaded/Audius in your music folder."
                    .into(),
            },
        ]
    }
}

impl SearchSource for Audius {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            name: MANIFEST.name,
            icon_svg: ICON,
            hint: "An artist, a track or a style; empty for what's trending",
        }
    }

    fn sites(&self) -> &'static [&'static str] {
        &["audius.co"]
    }

    fn describe(&self, query: &SourceQuery) -> Vec<String> {
        match query.text.trim() {
            "" => vec!["What's trending on Audius".to_string()],
            text => vec![format!("Tracks matching {text}")],
        }
    }

    fn start(&mut self, query: &SourceQuery) -> Result<String, PluginError> {
        let text = query.text.trim();
        let path = if text.is_empty() {
            format!("/v1/tracks/trending?{APP}")
        } else {
            format!("/v1/tracks/search?query={}&{APP}", encode(text))
        };
        let v = self.ask(&path)?;
        let items = parse_tracks(&v);
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
                title: if text.is_empty() {
                    "Trending".to_string()
                } else {
                    String::new()
                },
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

fn push(updates: &Mutex<Vec<DownloadUpdate>>, key: &str, progress: Progress) {
    if let Ok(mut u) = updates.lock() {
        u.push(DownloadUpdate {
            key: key.to_string(),
            progress,
        });
    }
}

/// The servers the directory lists.
fn servers(v: &Value) -> Vec<String> {
    v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str())
        .filter(|s| s.starts_with("https://"))
        .map(|s| s.trim_end_matches('/').to_string())
        .collect()
}

/// A search's or the trending list's tracks.
fn parse_tracks(v: &Value) -> Vec<Found> {
    v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["is_streamable"].as_bool() != Some(false))
        .filter_map(|t| {
            let art = &t["artwork"];
            Some(Found {
                key: t["id"].as_str()?.to_string(),
                artist: t["user"]["name"].as_str().unwrap_or_default().to_string(),
                title: t["title"].as_str()?.to_string(),
                year: t["release_date"]
                    .as_str()
                    .and_then(|d| d.get(..4))
                    .unwrap_or_default()
                    .to_string(),
                duration: t["duration"].as_f64().unwrap_or_default().max(0.0),
                format: "MP3".into(),
                from: t["user"]["handle"].as_str().unwrap_or_default().to_string(),
                cover_url: art["480x480"]
                    .as_str()
                    .or_else(|| art["150x150"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                ..Found::default()
            })
        })
        .collect()
}

/// One track's download.
struct Job {
    agent: ureq::Agent,
    url: String,
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
        let folder = self
            .dir
            .join(safe_name(&self.found.artist, "Unknown Artist"));
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let to = folder.join(format!(
            "{}.mp3",
            safe_name(&self.found.title, &self.found.key)
        ));
        if !to.exists() {
            self.save(&to)?;
        }
        Ok(to)
    }

    /// Writes the track to `to` through a partial file, so a stopped
    /// download leaves nothing that looks finished.
    fn save(&self, to: &Path) -> Result<(), String> {
        let mut response = self
            .agent
            .get(&self.url)
            .call()
            .map_err(|e| e.to_string())?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn server_list() {
        let v = json!({"data": ["https://a.example/", "http://insecure", 3, "https://b.example"]});
        assert_eq!(servers(&v), vec!["https://a.example", "https://b.example"]);
        assert!(servers(&json!({})).is_empty());
    }

    #[test]
    fn tracks() {
        let v = json!({"data": [
            {"id": "D7KyD", "title": "Night Drive", "duration": 215,
             "release_date": "2024-05-01T00:00:00Z",
             "user": {"name": "Neon Sky", "handle": "neonsky"},
             "artwork": {"150x150": "https://c/150.jpg", "480x480": "https://c/480.jpg"}},
            {"id": "x", "title": "Gated", "is_streamable": false, "user": {"name": "A"}},
            {"title": "No id"}
        ]});
        let t = parse_tracks(&v);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].key, "D7KyD");
        assert_eq!(t[0].artist, "Neon Sky");
        assert_eq!(t[0].title, "Night Drive");
        assert_eq!(t[0].year, "2024");
        assert_eq!(t[0].duration, 215.0);
        assert_eq!(t[0].format, "MP3");
        assert_eq!(t[0].cover_url, "https://c/480.jpg");
        assert!(parse_tracks(&json!({"data": []})).is_empty());
    }
}
