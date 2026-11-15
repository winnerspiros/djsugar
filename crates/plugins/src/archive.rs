//! The Internet Archive as a source in the rail: its freely licensed audio
//! (netlabel releases, live recordings, public domain) searched from
//! DJ Sugar, each item listed as an album with its cover, and downloaded
//! into the collection with its cover next to the files.
//!
//! Only items with a Creative Commons or public domain licence are listed,
//! and the licence shows on each one. The Archive needs no key: searches go
//! to `advancedsearch.php`, an item's files come from `/metadata/<id>` and
//! files download from `/download/<id>/<name>`.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::inbox::safe_name;
use crate::source::{
    DownloadUpdate, Found, Prefer, Progress, QueryKind, SearchSource, Snapshot, SourceInfo,
    SourceQuery, DOWNLOAD_FOLDER,
};
use crate::{Manifest, Plugin, PluginError, Setting, SettingKind, USER_AGENT};

const MANIFEST: Manifest = Manifest {
    id: "internet-archive",
    name: "Internet Archive",
    hosts: &["archive.org"],
    downloads: true,
};

const SITE: &str = "https://archive.org";

/// How many items a search lists.
const ROWS: usize = 50;

/// Columns, as a generic rail icon.
const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 30"><rect width="30" height="30" rx="7" fill="#3a3a3f"/><path d="M6 11.5 15 6l9 5.5z" fill="#e8e8ec"/><g fill="#e8e8ec"><rect x="7.5" y="13" width="2.4" height="9"/><rect x="11.9" y="13" width="2.4" height="9"/><rect x="15.7" y="13" width="2.4" height="9"/><rect x="20.1" y="13" width="2.4" height="9"/><rect x="6" y="23" width="18" height="2" rx=".5"/></g></svg>"##;

pub struct InternetArchive {
    agent: ureq::Agent,
    /// Each search's results, and which copies it prefers.
    searches: HashMap<String, (Snapshot, Prefer)>,
    next: u64,
    updates: Arc<Mutex<Vec<DownloadUpdate>>>,
    cancels: HashMap<String, Arc<AtomicBool>>,
    /// What each download was, so it can be tried again.
    jobs: HashMap<String, (Found, PathBuf, Prefer)>,
}

impl Default for InternetArchive {
    fn default() -> Self {
        Self::new()
    }
}

impl InternetArchive {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Self {
            agent,
            searches: HashMap::new(),
            next: 0,
            updates: Arc::default(),
            cancels: HashMap::new(),
            jobs: HashMap::new(),
        }
    }

    fn get_json(&self, url: &str) -> Result<Value, PluginError> {
        self.agent
            .get(url)
            .call()
            .map_err(|e| PluginError::Network(e.to_string()))?
            .body_mut()
            .read_json::<Value>()
            .map_err(|e| PluginError::Format(e.to_string()))
    }

    /// An item's tracks (in the preferred format) and the file to use as its
    /// cover.
    fn item(&self, id: &str, prefer: Prefer) -> Result<Item, PluginError> {
        let v = self.get_json(&format!("{SITE}/metadata/{}", encode(id)))?;
        parse_item(id, &v, prefer).ok_or_else(|| PluginError::Format("no such item".into()))
    }
}

impl Plugin for InternetArchive {
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
                title: "Freely licensed music".into(),
                detail: "Lists only items under a Creative Commons or public domain licence, \
                         shown on each one. Many allow non-commercial use only (NC): check \
                         before playing them at a paid gig."
                    .into(),
            },
            Setting {
                key: DOWNLOAD_FOLDER,
                kind: SettingKind::Folder,
                title: "Download folder".into(),
                detail: "Left empty, downloads go to Downloaded/Internet Archive in your \
                         music folder."
                    .into(),
            },
        ]
    }
}

impl SearchSource for InternetArchive {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            name: MANIFEST.name,
            icon_svg: ICON,
            hint: "An artist, a release, a label or a style",
        }
    }

    fn describe(&self, query: &SourceQuery) -> Vec<String> {
        let text = query.text.trim();
        if text.is_empty() {
            return Vec::new();
        }
        vec![match query.kind {
            QueryKind::Artist => format!("Freely licensed audio by {text}"),
            QueryKind::Album => format!("Freely licensed releases called {text}"),
            _ => format!("Freely licensed audio matching {text}"),
        }]
    }

    fn start(&mut self, query: &SourceQuery) -> Result<String, PluginError> {
        let url = search_url(query);
        let v = self.get_json(&url)?;
        let items = parse_search(&v).ok_or_else(|| PluginError::Format("no results".into()))?;
        self.next += 1;
        let id = self.next.to_string();
        let note = match items.len() {
            0 => "Nothing freely licensed found.".to_string(),
            n if n >= ROWS => format!("The {ROWS} most downloaded."),
            _ => String::new(),
        };
        self.searches.insert(
            id.clone(),
            (
                Snapshot {
                    items,
                    done: true,
                    note,
                    title: String::new(),
                },
                query.prefer,
            ),
        );
        Ok(id)
    }

    fn results(&mut self, id: &str) -> Result<Snapshot, PluginError> {
        self.searches
            .get(id)
            .map(|(s, _)| s.clone())
            .ok_or_else(|| PluginError::Format("no such search".into()))
    }

    fn stop(&mut self, id: &str) {
        self.searches.remove(id);
    }

    fn open_folder(&mut self, id: &str, folder: &Found) -> Result<Vec<Found>, PluginError> {
        let prefer = self.searches.get(id).map_or_else(Prefer::default, |s| s.1);
        let item = self.item(&folder.key, prefer)?;
        let tracks: Vec<Found> = item.tracks.iter().map(|t| t.found(&item, folder)).collect();
        if let Some((s, _)) = self.searches.get_mut(id) {
            if let Some(f) = s.items.iter_mut().find(|f| f.key == folder.key) {
                f.tracks = tracks.clone();
                f.track_count = tracks.len();
                f.size = tracks.iter().map(|t| t.size).sum();
            }
        }
        Ok(tracks)
    }

    fn download(&mut self, id: &str, items: &[Found], dir: &Path) -> Result<(), PluginError> {
        let prefer = self.searches.get(id).map_or_else(Prefer::default, |s| s.1);
        for f in items {
            self.start_job(f.clone(), dir.to_path_buf(), prefer)?;
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
        let (found, dir, prefer) = self
            .jobs
            .get(key)
            .cloned()
            .ok_or(PluginError::NotSupported)?;
        self.start_job(found, dir, prefer)
    }

    fn cancel(&mut self, key: &str) -> Result<(), PluginError> {
        if let Some(c) = self.cancels.get(key) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

impl InternetArchive {
    fn start_job(&mut self, found: Found, dir: PathBuf, prefer: Prefer) -> Result<(), PluginError> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.insert(found.key.clone(), cancel.clone());
        self.jobs
            .insert(found.key.clone(), (found.clone(), dir.clone(), prefer));
        push(&self.updates, &found.key, Progress::Queued);
        let job = Job {
            agent: self.agent.clone(),
            found,
            dir,
            prefer,
            updates: self.updates.clone(),
            cancel,
        };
        std::thread::Builder::new()
            .name("internet-archive".into())
            .spawn(move || job.run())
            .map(|_| ())
            .map_err(|e| PluginError::Network(e.to_string()))
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

/// One download: a whole item, or one file of it.
struct Job {
    agent: ureq::Agent,
    found: Found,
    dir: PathBuf,
    prefer: Prefer,
    updates: Arc<Mutex<Vec<DownloadUpdate>>>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn run(self) {
        let key = self.found.key.clone();
        let progress = match self.fetch() {
            Ok(paths) => Progress::Done(paths),
            Err(e) => Progress::Failed(e),
        };
        push(&self.updates, &key, progress);
    }

    fn fetch(&self) -> Result<Vec<PathBuf>, String> {
        let (id, only) = match self.found.key.split_once('/') {
            Some((id, name)) => (id, Some(name)),
            None => (self.found.key.as_str(), None),
        };
        let v: Value = self
            .agent
            .get(&format!("{SITE}/metadata/{}", encode(id)))
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())?;
        let item = parse_item(id, &v, self.prefer).ok_or("The item has gone.")?;
        let tracks: Vec<&Track> = match only {
            Some(name) => item
                .tracks
                .iter()
                .filter(|t| t.files.iter().any(|f| f.name == name))
                .collect(),
            None => item.tracks.iter().collect(),
        };
        if tracks.is_empty() {
            return Err("No audio in this item.".into());
        }
        let folder = self
            .dir
            .join(safe_name(&item.creator, "Unknown Artist"))
            .join(safe_name(&item.title, id));
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        // The cover next to the files, so the collection shows it.
        if let Some(cover) = &item.cover {
            let ext = Path::new(cover)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg");
            let to = folder.join(format!("cover.{}", ext.to_lowercase()));
            if !to.exists() {
                let _ = self.save(&file_url(id, cover), &to, &mut |_| {});
            }
        }
        let total: u64 = tracks.iter().map(|t| t.pick(only).size).sum::<u64>().max(1);
        let mut done = 0u64;
        let mut paths = Vec::new();
        for t in tracks {
            if self.cancel.load(Ordering::Relaxed) {
                return Err("Cancelled.".into());
            }
            let file = t.pick(only);
            let base = Path::new(&file.name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&file.name);
            let to = folder.join(safe_name(base, "track"));
            if !to.exists() {
                let updates = self.updates.clone();
                let key = self.found.key.clone();
                let start = done;
                self.save(&file_url(id, &file.name), &to, &mut |n| {
                    let p = (start + n) as f32 / total as f32;
                    push(&updates, &key, Progress::Running(p.min(1.0)));
                })?;
            }
            done += file.size;
            paths.push(to);
        }
        Ok(paths)
    }

    /// Writes `url` to `to`, through a partial file so a stopped download
    /// leaves nothing that looks finished.
    fn save(&self, url: &str, to: &Path, progress: &mut dyn FnMut(u64)) -> Result<(), String> {
        let mut response = self.agent.get(url).call().map_err(|e| e.to_string())?;
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
            progress(read);
        }
        drop(out);
        std::fs::rename(&part, to).map_err(|e| e.to_string())
    }
}

/// An item as its metadata describes it.
#[derive(Debug, Default, PartialEq)]
struct Item {
    title: String,
    creator: String,
    year: String,
    licence: String,
    /// The file to use as its cover, if it has one.
    cover: Option<String>,
    tracks: Vec<Track>,
}

/// One recording in an item, in each format the Archive has it.
#[derive(Debug, Default, PartialEq)]
struct Track {
    title: String,
    artist: String,
    number: String,
    duration: f64,
    /// Best first, as preferred.
    files: Vec<File>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct File {
    name: String,
    format: String,
    size: u64,
}

impl Track {
    /// The file named, or else the best one.
    fn pick(&self, name: Option<&str>) -> &File {
        name.and_then(|n| self.files.iter().find(|f| f.name == n))
            .unwrap_or(&self.files[0])
    }

    fn found(&self, item: &Item, folder: &Found) -> Found {
        let f = &self.files[0];
        Found {
            key: format!("{}/{}", folder.key, f.name),
            artist: if self.artist.is_empty() {
                item.creator.clone()
            } else {
                self.artist.clone()
            },
            title: self.title.clone(),
            album: item.title.clone(),
            year: item.year.clone(),
            number: self.number.clone(),
            duration: self.duration,
            format: short_format(&f.format).into(),
            quality: quality(&f.format).into(),
            size: f.size,
            from: item.licence.clone(),
            cover_url: folder.cover_url.clone(),
            ..Default::default()
        }
    }
}

/// The search address for a query: freely licensed audio only, most
/// downloaded first.
fn search_url(query: &SourceQuery) -> String {
    let text = query.text.trim().replace(['(', ')', ':', '"'], " ");
    let words = match query.kind {
        QueryKind::Artist => format!("creator:({text})"),
        QueryKind::Album => format!("title:({text})"),
        _ => format!("({text})"),
    };
    let q = format!(
        "{words} AND mediatype:(audio) AND licenseurl:(*creativecommons.org* OR *publicdomain*)"
    );
    let fields = [
        "identifier",
        "title",
        "creator",
        "year",
        "date",
        "licenseurl",
        "item_size",
    ];
    let mut url = format!("{SITE}/advancedsearch.php?q={}", encode(&q));
    for f in fields {
        url.push_str("&fl%5B%5D=");
        url.push_str(f);
    }
    url.push_str(&format!(
        "&sort%5B%5D=downloads+desc&rows={ROWS}&page=1&output=json"
    ));
    url
}

/// A search's items, as albums with their covers.
fn parse_search(v: &Value) -> Option<Vec<Found>> {
    let docs = v["response"]["docs"].as_array()?;
    Some(
        docs.iter()
            .filter_map(|d| {
                let id = d["identifier"].as_str()?;
                let title = text(&d["title"]);
                let year = match text(&d["year"]) {
                    y if !y.is_empty() => y,
                    _ => text(&d["date"]).chars().take(4).collect(),
                };
                Some(Found {
                    key: id.into(),
                    folder: true,
                    artist: text(&d["creator"]),
                    title: title.clone(),
                    album: if title.is_empty() { id.into() } else { title },
                    year,
                    size: d["item_size"].as_u64().unwrap_or(0),
                    from: licence(&text(&d["licenseurl"])),
                    cover_url: format!("{SITE}/services/img/{}", encode(id)),
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// An item's metadata: its tracks grouped across formats, best first.
fn parse_item(id: &str, v: &Value, prefer: Prefer) -> Option<Item> {
    let meta = &v["metadata"];
    let files = v["files"].as_array()?;
    let mut item = Item {
        title: match text(&meta["title"]) {
            t if t.is_empty() => id.to_string(),
            t => t,
        },
        creator: text(&meta["creator"]),
        year: match text(&meta["year"]) {
            y if !y.is_empty() => y,
            _ => text(&meta["date"]).chars().take(4).collect(),
        },
        licence: licence(&text(&meta["licenseurl"])),
        ..Default::default()
    };
    // Each recording by the file it came from: derivatives name their original.
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Track> = HashMap::new();
    for f in files {
        let name = text(&f["name"]);
        let format = text(&f["format"]);
        if !is_audio(&format) {
            continue;
        }
        let root = match text(&f["source"]).as_str() {
            "derivative" => match text(&f["original"]) {
                o if !o.is_empty() => o,
                _ => name.clone(),
            },
            _ => name.clone(),
        };
        let t = groups.entry(root.clone()).or_insert_with(|| {
            order.push(root.clone());
            Track::default()
        });
        let title = text(&f["title"]);
        if t.title.is_empty() && !title.is_empty() {
            t.title = title;
        }
        if t.artist.is_empty() {
            t.artist = text(&f["creator"]);
        }
        if t.number.is_empty() {
            t.number = text(&f["track"])
                .split('/')
                .next()
                .unwrap_or("")
                .trim()
                .into();
        }
        if t.duration == 0.0 {
            t.duration = seconds(&text(&f["length"]));
        }
        t.files.push(File {
            size: text(&f["size"]).parse().unwrap_or(0),
            name,
            format,
        });
    }
    for root in order {
        let mut t = groups.remove(&root)?;
        if t.title.is_empty() {
            t.title = Path::new(&root)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&root)
                .to_string();
        }
        t.files.sort_by_key(|f| rank(&f.format, prefer));
        item.tracks.push(t);
    }
    item.tracks
        .sort_by_key(|t| t.number.parse::<u32>().unwrap_or(u32::MAX));
    item.cover = cover(files);
    Some(item)
}

/// The item's own picture: one called cover, front or folder first, then
/// any original image, then the Archive's item image.
fn cover(files: &[Value]) -> Option<String> {
    let images: Vec<(String, String, String)> = files
        .iter()
        .map(|f| (text(&f["name"]), text(&f["format"]), text(&f["source"])))
        .filter(|(_, format, _)| matches!(format.as_str(), "JPEG" | "PNG" | "Item Image"))
        .collect();
    let named = |n: &str| {
        let n = n.to_lowercase();
        ["cover", "front", "folder"].iter().any(|w| n.contains(w))
    };
    images
        .iter()
        .find(|(n, _, s)| s == "original" && named(n))
        .or_else(|| images.iter().find(|(_, _, s)| s == "original"))
        .or_else(|| images.iter().find(|(_, f, _)| f == "Item Image"))
        .map(|(n, _, _)| n.clone())
}

fn is_audio(format: &str) -> bool {
    let f = format.to_lowercase();
    f.contains("mp3")
        || f.contains("flac")
        || f.contains("ogg")
        || f.contains("wave")
        || f.contains("aiff")
        || f.contains("apple lossless")
}

/// Lower is better.
fn rank(format: &str, prefer: Prefer) -> u32 {
    let f = format.to_lowercase();
    let lossless =
        f.contains("flac") || f.contains("wave") || f.contains("aiff") || f.contains("lossless");
    let mp3 = match () {
        _ if f == "vbr mp3" || f.contains("320") => 0,
        _ if f.contains("256") => 1,
        _ if f.contains("mp3") && !f.contains("64") => 2,
        _ if f.contains("mp3") => 4,
        _ => 5,
    };
    match prefer {
        Prefer::Lossless | Prefer::Any if lossless => 0,
        Prefer::Lossless | Prefer::Any => 1 + mp3,
        Prefer::Mp3 if f.contains("mp3") => mp3,
        Prefer::Mp3 if lossless => 6,
        Prefer::Mp3 => 7,
    }
}

fn short_format(format: &str) -> &'static str {
    let f = format.to_lowercase();
    match () {
        _ if f.contains("flac") => "FLAC",
        _ if f.contains("wave") => "WAV",
        _ if f.contains("aiff") => "AIFF",
        _ if f.contains("apple lossless") => "ALAC",
        _ if f.contains("ogg") => "OGG",
        _ if f.contains("mp3") => "MP3",
        _ => "",
    }
}

fn quality(format: &str) -> &'static str {
    let f = format.to_lowercase();
    match () {
        _ if f.contains("24bit") => "24-bit",
        _ if f == "vbr mp3" => "VBR",
        _ if f.contains("320") => "320 kbps",
        _ if f.contains("256") => "256 kbps",
        _ if f.contains("128") => "128 kbps",
        _ if f.contains("64") => "64 kbps",
        _ => "",
    }
}

/// "CC BY-NC-SA 3.0" from a licence address; "Public domain" for one.
fn licence(url: &str) -> String {
    let u = url.to_lowercase();
    if u.contains("publicdomain") {
        return if u.contains("zero") {
            "CC0".into()
        } else {
            "Public domain".into()
        };
    }
    let Some(rest) = u.split("/licenses/").nth(1) else {
        return String::new();
    };
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let kind = parts.next().unwrap_or("").to_uppercase();
    match parts.next() {
        Some(v) if v.chars().next().is_some_and(|c| c.is_ascii_digit()) => {
            format!("CC {kind} {v}")
        }
        _ => format!("CC {kind}"),
    }
}

/// A length as the Archive writes it: seconds ("215.3"), or "3:35",
/// "01:02:03".
fn seconds(s: &str) -> f64 {
    if s.contains(':') {
        s.split(':')
            .try_fold(0.0, |acc, p| {
                p.trim().parse::<f64>().ok().map(|n| acc * 60.0 + n)
            })
            .unwrap_or(0.0)
    } else {
        s.trim().parse().unwrap_or(0.0)
    }
}

/// A metadata value as text: the first of a list, numbers as written.
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a.first().map(text).unwrap_or_default(),
        _ => String::new(),
    }
}

fn file_url(id: &str, name: &str) -> String {
    let path: Vec<String> = name.split('/').map(encode).collect();
    format!("{SITE}/download/{}/{}", encode(id), path.join("/"))
}

/// Percent-encodes everything but unreserved characters.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn licences() {
        assert_eq!(
            licence("http://creativecommons.org/licenses/by-nc-sa/3.0/us/"),
            "CC BY-NC-SA 3.0"
        );
        assert_eq!(
            licence("https://creativecommons.org/licenses/by/4.0"),
            "CC BY 4.0"
        );
        assert_eq!(
            licence("http://creativecommons.org/publicdomain/zero/1.0/"),
            "CC0"
        );
        assert_eq!(
            licence("http://creativecommons.org/publicdomain/mark/1.0/"),
            "Public domain"
        );
        assert_eq!(licence(""), "");
    }

    #[test]
    fn lengths() {
        assert_eq!(seconds("215.5"), 215.5);
        assert_eq!(seconds("3:35"), 215.0);
        assert_eq!(seconds("01:00:05"), 3605.0);
        assert_eq!(seconds("soon"), 0.0);
    }

    #[test]
    fn searches_free_audio() {
        let url = search_url(&SourceQuery {
            text: "Polish Ambassador".into(),
            kind: QueryKind::Artist,
            prefer: Prefer::Lossless,
            ..Default::default()
        });
        assert!(url.starts_with(
            "https://archive.org/advancedsearch.php?q=creator%3A%28Polish%20Ambassador%29"
        ));
        assert!(url.contains("licenseurl"));
        assert!(url.contains("output=json"));
    }

    #[test]
    fn reads_search_results() {
        let v = json!({ "response": { "docs": [
            { "identifier": "tpa-pttp", "title": "Pushing Through The Pavement",
              "creator": ["The Polish Ambassador"], "date": "2012-05-01T00:00:00Z",
              "licenseurl": "http://creativecommons.org/licenses/by-nc-sa/3.0/us/",
              "item_size": 123456 },
            { "title": "no id" },
        ]}});
        let items = parse_search(&v).unwrap();
        assert_eq!(items.len(), 1);
        let f = &items[0];
        assert!(f.folder);
        assert_eq!(f.artist, "The Polish Ambassador");
        assert_eq!(f.album, "Pushing Through The Pavement");
        assert_eq!(f.year, "2012");
        assert_eq!(f.from, "CC BY-NC-SA 3.0");
        assert_eq!(f.cover_url, "https://archive.org/services/img/tpa-pttp");
    }

    #[test]
    fn groups_formats_into_tracks() {
        let v = json!({
            "metadata": { "title": "Album", "creator": "Artist", "year": "2010",
                          "licenseurl": "http://creativecommons.org/licenses/by/3.0/" },
            "files": [
                { "name": "02 Second.flac", "source": "original", "format": "Flac",
                  "title": "Second", "track": "2/2", "length": "4:00", "size": "30000000" },
                { "name": "02 Second.mp3", "source": "derivative", "format": "VBR MP3",
                  "original": "02 Second.flac", "size": "8000000" },
                { "name": "01 First.mp3", "source": "original", "format": "VBR MP3",
                  "title": "First", "track": "1", "length": "180.2", "size": "7000000" },
                { "name": "front.jpg", "source": "original", "format": "JPEG" },
                { "name": "__ia_thumb.jpg", "source": "original", "format": "Item Image" },
                { "name": "Album_meta.xml", "source": "original", "format": "Metadata" },
            ]
        });
        let item = parse_item("x", &v, Prefer::Lossless).unwrap();
        assert_eq!(item.cover.as_deref(), Some("front.jpg"));
        assert_eq!(item.licence, "CC BY 3.0");
        let titles: Vec<_> = item.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["First", "Second"]);
        assert_eq!(item.tracks[1].files[0].format, "Flac");
        assert_eq!(item.tracks[1].duration, 240.0);
        let mp3 = parse_item("x", &v, Prefer::Mp3).unwrap();
        assert_eq!(mp3.tracks[1].files[0].format, "VBR MP3");
        let folder = Found {
            key: "x".into(),
            ..Default::default()
        };
        let f = item.tracks[1].found(&item, &folder);
        assert_eq!(f.key, "x/02 Second.flac");
        assert_eq!((f.format.as_str(), f.number.as_str()), ("FLAC", "2"));
    }

    #[test]
    fn download_addresses() {
        assert_eq!(
            file_url("my item", "disc 1/01 A&B.mp3"),
            "https://archive.org/download/my%20item/disc%201/01%20A%26B.mp3"
        );
    }
}
