//! Add-ons as programs of their own: DJ Sugar starts the program and talks to
//! it over its stdin and stdout, one JSON message per line. An add-on built
//! on these traits calls [`serve`] from its `main`; inside the app a
//! [`RemotePlugin`] stands in for it, so the rail, Settings and the source
//! threads use it like a plugin compiled in.
//!
//! A request is `{"id": 1, "method": "source.start", "params": {...}}`; the
//! answer is `{"id": 1, "result": ...}` or `{"id": 1, "error": {"kind":
//! "network" | "format" | "unsupported", "message": "..."}}`. The first
//! request is always `initialize`, which says what the add-on is.
//!
//! Nothing else may be written to stdout: an add-on logs to stderr.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::source::{
    DownloadUpdate, Found, Prefer, Progress, QueryKind, SearchSource, Snapshot, SourceInfo,
    SourceQuery, StatusPage, StatusSection,
};
use crate::{Manifest, Plugin, PluginConfig, PluginError, Setting, SettingKind};

/// Bumped when a message changes shape; an add-on that answers with
/// another version isn't loaded.
pub const PROTOCOL: u64 = 1;

// ---- The add-on's side --------------------------------------------------

/// Runs `plugin` as an add-on program: answers DJ Sugar on stdin and stdout
/// until DJ Sugar closes stdin, then returns (dropping the plugin, so it can
/// stop anything it started).
pub fn serve(mut plugin: Box<dyn Plugin>) {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            eprintln!("add-on: not a message: {line}");
            continue;
        };
        let id = msg["id"].clone();
        let reply = match answer(
            plugin.as_mut(),
            msg["method"].as_str().unwrap_or(""),
            &msg["params"],
        ) {
            Ok(result) => json!({ "id": id, "result": result }),
            Err(e) => json!({ "id": id, "error": error_json(&e) }),
        };
        if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
}

fn answer(plugin: &mut dyn Plugin, method: &str, p: &Value) -> Result<Value, PluginError> {
    let unsupported = || PluginError::NotSupported;
    match method {
        "initialize" => {
            let m = plugin.manifest().clone();
            let source = plugin.source().map(|s| {
                let mut info = info_json(&s.info());
                info["sites"] = json!(s.sites());
                info["previews"] = json!(s.previews());
                info
            });
            Ok(json!({
                "protocol": PROTOCOL,
                "manifest": {
                    "id": m.id, "name": m.name, "hosts": m.hosts, "downloads": m.downloads,
                },
                "source": source,
            }))
        }
        "settings" => Ok(Value::Array(
            plugin.settings().iter().map(setting_json).collect(),
        )),
        "configure" => {
            plugin.configure(&config_from(p));
            Ok(Value::Null)
        }
        _ => {
            let source = plugin.source().ok_or_else(unsupported)?;
            let id = p["id"].as_str().unwrap_or_default();
            match method {
                "source.describe" => Ok(json!(source.describe(&query_from(&p["query"])))),
                "source.start" => Ok(json!(source.start(&query_from(&p["query"]))?)),
                "source.results" => Ok(snapshot_json(&source.results(id)?)),
                "source.stop" => {
                    source.stop(id);
                    Ok(Value::Null)
                }
                "source.open_folder" => {
                    let tracks = source.open_folder(id, &found_from(&p["folder"]))?;
                    Ok(Value::Array(tracks.iter().map(found_json).collect()))
                }
                "source.download" => {
                    let items: Vec<Found> = list(&p["items"]).iter().map(found_from).collect();
                    source.download(id, &items, Path::new(str_of(&p["dir"])))?;
                    Ok(Value::Null)
                }
                "source.progress" => Ok(Value::Array(
                    source.progress().iter().map(update_json).collect(),
                )),
                "source.retry" => {
                    source.retry(str_of(&p["key"]))?;
                    Ok(Value::Null)
                }
                "source.cancel" => {
                    source.cancel(str_of(&p["key"]))?;
                    Ok(Value::Null)
                }
                "source.status" => Ok(source.status().map_or(Value::Null, |s| status_json(&s))),
                "source.notes" => Ok(Value::Array(
                    source
                        .notes()
                        .iter()
                        .map(|(key, note)| json!({ "key": key, "note": note }))
                        .collect(),
                )),
                "source.preview" => {
                    source.preview(id, &found_from(&p["item"]))?;
                    Ok(Value::Null)
                }
                "source.preview_file" => Ok(source
                    .preview_file()?
                    .map_or(Value::Null, |f| json!(f.to_string_lossy()))),
                "source.stop_preview" => {
                    source.stop_preview();
                    Ok(Value::Null)
                }
                _ => Err(unsupported()),
            }
        }
    }
}

// ---- DJ Sugar's side ------------------------------------------------------

/// What an add-on program said it is, kept for the run so the many places
/// that make plugins don't each start the program just to ask.
#[derive(Clone)]
struct Described {
    manifest: Manifest,
    source: Option<SourceInfo>,
    sites: &'static [&'static str],
    previews: bool,
}

fn described() -> &'static Mutex<HashMap<PathBuf, Described>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Described>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Strings that must live as long as the app (a manifest's name, a setting's
/// key), made once each.
fn intern(s: &str) -> &'static str {
    static STRINGS: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut map = STRINGS.get_or_init(Default::default).lock().unwrap();
    if let Some(s) = map.get(s) {
        return s;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    map.insert(s.to_string(), leaked);
    leaked
}

/// A running add-on program.
struct Process {
    child: Child,
    /// Taken when the process is dropped, which ends `serve`.
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Process {
    fn start(program: &Path) -> Result<(Self, Value), PluginError> {
        let mut child = djsugar_engine::process::background(Command::new(program))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| PluginError::Network(format!("Can't start {}: {e}", program.display())))?;
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut process = Process {
            child,
            stdin,
            stdout,
            next: 0,
        };
        let hello = process.call("initialize", json!({ "protocol": PROTOCOL }))?;
        if hello["protocol"].as_u64() != Some(PROTOCOL) {
            return Err(PluginError::Format(format!(
                "{} speaks add-on protocol {}, DJ Sugar speaks {PROTOCOL}",
                program.display(),
                hello["protocol"]
            )));
        }
        Ok((process, hello))
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, PluginError> {
        self.next += 1;
        let id = self.next;
        let gone = |e: std::io::Error| PluginError::Network(format!("The add-on stopped: {e}"));
        let msg = json!({ "id": id, "method": method, "params": params });
        let stdin = self.stdin.as_mut().expect("stdin until dropped");
        writeln!(stdin, "{msg}").map_err(gone)?;
        stdin.flush().map_err(gone)?;
        loop {
            let mut line = String::new();
            if self.stdout.read_line(&mut line).map_err(gone)? == 0 {
                return Err(PluginError::Network("The add-on stopped.".into()));
            }
            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if reply["id"].as_u64() != Some(id) {
                continue;
            }
            if reply.get("error").is_some_and(|e| !e.is_null()) {
                return Err(error_from(&reply["error"]));
            }
            return Ok(reply["result"].clone());
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Closing stdin ends `serve`, so the add-on can stop what it
        // started; one that doesn't go is killed.
        drop(self.stdin.take());
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// An add-on program, as a [`Plugin`]. The program starts on the first call
/// that needs it, and again if it stopped. Each `RemotePlugin` has a
/// program of its own, as each compiled-in plugin is an instance of its own.
pub struct RemotePlugin {
    program: PathBuf,
    described: Described,
    /// Behind a lock only because `settings` and `describe` take `&self`.
    process: Mutex<Option<Process>>,
    config: Option<PluginConfig>,
}

impl RemotePlugin {
    /// The add-on at `program`, asking it what it is unless this run
    /// already did.
    pub fn new(program: &Path) -> Result<Self, PluginError> {
        if let Some(d) = described().lock().unwrap().get(program).cloned() {
            return Ok(Self {
                program: program.to_path_buf(),
                described: d,
                process: Mutex::new(None),
                config: None,
            });
        }
        let (process, hello) = Process::start(program)?;
        let d = described_from(&hello)?;
        described()
            .lock()
            .unwrap()
            .insert(program.to_path_buf(), d.clone());
        Ok(Self {
            program: program.to_path_buf(),
            described: d,
            process: Mutex::new(Some(process)),
            config: None,
        })
    }

    fn running(&self) -> bool {
        self.process.lock().unwrap().is_some()
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, PluginError> {
        let mut slot = self.process.lock().unwrap();
        if let Some(p) = slot.as_mut() {
            match p.call(method, params.clone()) {
                // It stopped: start it again below.
                Err(PluginError::Network(_)) if p.child.try_wait().ok().flatten().is_some() => {}
                other => return other,
            }
        }
        *slot = None;
        let (mut process, _) = Process::start(&self.program)?;
        if let Some(config) = &self.config {
            process.call("configure", config_json(config))?;
        }
        let result = process.call(method, params);
        *slot = Some(process);
        result
    }
}

impl Plugin for RemotePlugin {
    fn manifest(&self) -> &Manifest {
        &self.described.manifest
    }
    fn source(&mut self) -> Option<&mut dyn SearchSource> {
        if self.described.source.is_some() {
            Some(self)
        } else {
            None
        }
    }
    fn settings(&self) -> Vec<Setting> {
        match self.call("settings", Value::Null) {
            Ok(rows) => list(&rows).iter().map(setting_from).collect(),
            Err(e) => vec![Setting::new("", SettingKind::Note, "", &e.to_string())],
        }
    }
    fn configure(&mut self, config: &PluginConfig) {
        self.config = Some(config.clone());
        if let Some(p) = self.process.lock().unwrap().as_mut() {
            let _ = p.call("configure", config_json(config));
        }
    }
}

impl SearchSource for RemotePlugin {
    fn info(&self) -> SourceInfo {
        self.described.source.clone().unwrap_or(SourceInfo {
            name: self.described.manifest.name,
            icon_svg: b"",
            hint: "",
        })
    }
    fn sites(&self) -> &'static [&'static str] {
        self.described.sites
    }
    fn describe(&self, query: &SourceQuery) -> Vec<String> {
        self.call("source.describe", json!({ "query": query_json(query) }))
            .map(|v| list(&v).iter().map(|s| str_of(s).to_string()).collect())
            .unwrap_or_default()
    }
    fn start(&mut self, query: &SourceQuery) -> Result<String, PluginError> {
        let id = self.call("source.start", json!({ "query": query_json(query) }))?;
        Ok(str_of(&id).to_string())
    }
    fn results(&mut self, id: &str) -> Result<Snapshot, PluginError> {
        Ok(snapshot_from(
            &self.call("source.results", json!({ "id": id }))?,
        ))
    }
    fn stop(&mut self, id: &str) {
        let _ = self.call("source.stop", json!({ "id": id }));
    }
    fn open_folder(&mut self, id: &str, folder: &Found) -> Result<Vec<Found>, PluginError> {
        let v = self.call(
            "source.open_folder",
            json!({ "id": id, "folder": found_json(folder) }),
        )?;
        Ok(list(&v).iter().map(found_from).collect())
    }
    fn download(&mut self, id: &str, items: &[Found], dir: &Path) -> Result<(), PluginError> {
        let items: Vec<Value> = items.iter().map(found_json).collect();
        self.call(
            "source.download",
            json!({ "id": id, "items": items, "dir": dir.to_string_lossy() }),
        )
        .map(drop)
    }
    fn progress(&mut self) -> Vec<DownloadUpdate> {
        // Nothing is downloading in a program that isn't running.
        if !self.running() {
            return Vec::new();
        }
        self.call("source.progress", Value::Null)
            .map(|v| list(&v).iter().map(update_from).collect())
            .unwrap_or_default()
    }
    fn retry(&mut self, key: &str) -> Result<(), PluginError> {
        self.call("source.retry", json!({ "key": key })).map(drop)
    }
    fn cancel(&mut self, key: &str) -> Result<(), PluginError> {
        self.call("source.cancel", json!({ "key": key })).map(drop)
    }
    fn notes(&mut self) -> Vec<(String, String)> {
        if !self.running() {
            return Vec::new();
        }
        // An add-on from before notes answers "not supported": none.
        self.call("source.notes", Value::Null)
            .map(|v| {
                list(&v)
                    .iter()
                    .map(|n| (string(&n["key"]), string(&n["note"])))
                    .collect()
            })
            .unwrap_or_default()
    }
    fn previews(&self) -> bool {
        self.described.previews
    }
    fn preview(&mut self, id: &str, item: &Found) -> Result<(), PluginError> {
        self.call(
            "source.preview",
            json!({ "id": id, "item": found_json(item) }),
        )
        .map(drop)
    }
    fn preview_file(&mut self) -> Result<Option<PathBuf>, PluginError> {
        let v = self.call("source.preview_file", Value::Null)?;
        Ok(v.as_str().filter(|s| !s.is_empty()).map(PathBuf::from))
    }
    fn stop_preview(&mut self) {
        if self.running() {
            let _ = self.call("source.stop_preview", Value::Null);
        }
    }
    fn status(&mut self) -> Option<StatusPage> {
        // An add-on from before pages answers "not supported": none.
        self.call("source.status", Value::Null)
            .ok()
            .filter(|v| v.is_object())
            .map(|v| status_from(&v))
    }
}

/// The add-on programs to load: each one named in `DJSUGAR_ADDONS` (paths
/// separated like `PATH`), then every program in `dir`.
pub fn programs(dir: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::env::var_os("DJSUGAR_ADDONS")
        .map(|v| {
            std::env::split_paths(&v)
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default();
    if let Some(entries) = dir.and_then(|d| std::fs::read_dir(d).ok()) {
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| is_program(p))
            .collect();
        found.sort();
        out.extend(found);
    }
    out
}

fn is_program(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    }
}

// ---- Messages -----------------------------------------------------------

fn list(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

fn string(v: &Value) -> String {
    str_of(v).to_string()
}

fn error_json(e: &PluginError) -> Value {
    match e {
        PluginError::Network(m) => json!({ "kind": "network", "message": m }),
        PluginError::Format(m) => json!({ "kind": "format", "message": m }),
        PluginError::NotSupported => json!({ "kind": "unsupported", "message": "" }),
    }
}

fn error_from(v: &Value) -> PluginError {
    match str_of(&v["kind"]) {
        "unsupported" => PluginError::NotSupported,
        "format" => PluginError::Format(string(&v["message"])),
        _ => PluginError::Network(string(&v["message"])),
    }
}

fn described_from(hello: &Value) -> Result<Described, PluginError> {
    let m = &hello["manifest"];
    let id = str_of(&m["id"]);
    if id.is_empty() {
        return Err(PluginError::Format("the add-on has no id".into()));
    }
    let hosts: Vec<&'static str> = list(&m["hosts"])
        .iter()
        .map(|h| intern(str_of(h)))
        .collect();
    let manifest = Manifest {
        id: intern(id),
        name: intern(str_of(&m["name"])),
        hosts: Box::leak(hosts.into_boxed_slice()),
        downloads: m["downloads"].as_bool().unwrap_or(false),
    };
    let source = (!hello["source"].is_null()).then(|| info_from(&hello["source"]));
    let sites: Vec<&'static str> = list(&hello["source"]["sites"])
        .iter()
        .map(|s| intern(str_of(s)))
        .collect();
    Ok(Described {
        manifest,
        source,
        sites: Box::leak(sites.into_boxed_slice()),
        // An add-on from before previews says nothing: none.
        previews: hello["source"]["previews"].as_bool().unwrap_or(false),
    })
}

fn info_json(i: &SourceInfo) -> Value {
    json!({
        "name": i.name,
        "icon_svg": String::from_utf8_lossy(i.icon_svg),
        "hint": i.hint,
    })
}

fn info_from(v: &Value) -> SourceInfo {
    SourceInfo {
        name: intern(str_of(&v["name"])),
        icon_svg: intern(str_of(&v["icon_svg"])).as_bytes(),
        hint: intern(str_of(&v["hint"])),
    }
}

fn kind_name(k: SettingKind) -> &'static str {
    match k {
        SettingKind::Text => "text",
        SettingKind::Secret => "secret",
        SettingKind::Note => "note",
        SettingKind::Folder => "folder",
        SettingKind::Switch(_) => "switch",
    }
}

fn setting_json(s: &Setting) -> Value {
    let mut v =
        json!({ "key": s.key, "kind": kind_name(s.kind), "title": s.title, "detail": s.detail });
    if let SettingKind::Switch(on) = s.kind {
        v["on"] = json!(on);
    }
    v
}

fn setting_from(v: &Value) -> Setting {
    let kind = match str_of(&v["kind"]) {
        "secret" => SettingKind::Secret,
        "note" => SettingKind::Note,
        "folder" => SettingKind::Folder,
        "switch" => SettingKind::Switch(v["on"].as_bool().unwrap_or(false)),
        _ => SettingKind::Text,
    };
    Setting::new(
        intern(str_of(&v["key"])),
        kind,
        str_of(&v["title"]),
        str_of(&v["detail"]),
    )
}

fn config_json(c: &PluginConfig) -> Value {
    json!({
        "values": c.values,
        "dir": c.dir.as_ref().map(|d| d.to_string_lossy()),
        "music_folders": c.music_folders.iter().map(|d| d.to_string_lossy()).collect::<Vec<_>>(),
    })
}

fn config_from(v: &Value) -> PluginConfig {
    let values: BTreeMap<String, String> = v["values"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), string(v))).collect())
        .unwrap_or_default();
    PluginConfig {
        values,
        dir: v["dir"].as_str().map(PathBuf::from),
        music_folders: list(&v["music_folders"])
            .iter()
            .filter_map(|d| d.as_str().map(PathBuf::from))
            .collect(),
    }
}

fn query_json(q: &SourceQuery) -> Value {
    json!({
        "text": q.text,
        "kind": q.kind.name(),
        "prefer": q.prefer.name(),
        "skip": q.skip,
        "songs": q.songs,
        "restored": q.restored,
    })
}

fn query_from(v: &Value) -> SourceQuery {
    SourceQuery {
        text: string(&v["text"]),
        kind: QueryKind::from_name(str_of(&v["kind"])),
        prefer: Prefer::from_name(str_of(&v["prefer"])),
        skip: list(&v["skip"]).iter().map(string).collect(),
        songs: list(&v["songs"]).iter().map(string).collect(),
        restored: v["restored"].as_bool().unwrap_or_default(),
    }
}

/// A result as JSON, as add-ons send it (also how the app keeps results).
pub fn found_json(f: &Found) -> Value {
    json!({
        "key": f.key, "folder": f.folder, "artist": f.artist, "title": f.title,
        "album": f.album, "year": f.year, "number": f.number, "duration": f.duration,
        "format": f.format, "quality": f.quality, "size": f.size, "from": f.from,
        "tracks": f.tracks.iter().map(found_json).collect::<Vec<_>>(),
        "track_count": f.track_count, "group": f.group, "cover_url": f.cover_url, "file": f.file,
    })
}

/// A result read back from [`found_json`].
pub fn found_from(v: &Value) -> Found {
    Found {
        key: string(&v["key"]),
        folder: v["folder"].as_bool().unwrap_or(false),
        artist: string(&v["artist"]),
        title: string(&v["title"]),
        album: string(&v["album"]),
        year: string(&v["year"]),
        number: string(&v["number"]),
        duration: v["duration"].as_f64().unwrap_or(0.0),
        format: string(&v["format"]),
        quality: string(&v["quality"]),
        size: v["size"].as_u64().unwrap_or(0),
        from: string(&v["from"]),
        tracks: list(&v["tracks"]).iter().map(found_from).collect(),
        track_count: v["track_count"].as_u64().unwrap_or(0) as usize,
        group: string(&v["group"]),
        cover_url: string(&v["cover_url"]),
        file: string(&v["file"]),
    }
}

fn snapshot_json(s: &Snapshot) -> Value {
    json!({
        "items": s.items.iter().map(found_json).collect::<Vec<_>>(),
        "done": s.done,
        "note": s.note,
        "title": s.title,
    })
}

fn snapshot_from(v: &Value) -> Snapshot {
    Snapshot {
        items: list(&v["items"]).iter().map(found_from).collect(),
        done: v["done"].as_bool().unwrap_or(false),
        note: string(&v["note"]),
        title: string(&v["title"]),
    }
}

fn status_json(s: &StatusPage) -> Value {
    json!({
        "title": s.title,
        "line": s.line,
        "note": s.note,
        "sections": s.sections.iter().map(|sec| json!({
            "heading": sec.heading,
            "rows": sec.rows.iter().map(|(l, v)| json!([l, v])).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn status_from(v: &Value) -> StatusPage {
    StatusPage {
        title: string(&v["title"]),
        line: string(&v["line"]),
        note: string(&v["note"]),
        sections: list(&v["sections"])
            .iter()
            .map(|sec| StatusSection {
                heading: string(&sec["heading"]),
                rows: list(&sec["rows"])
                    .iter()
                    .map(|r| (string(&r[0]), string(&r[1])))
                    .collect(),
            })
            .collect(),
    }
}

fn update_json(u: &DownloadUpdate) -> Value {
    let progress = match &u.progress {
        Progress::Queued => json!({ "state": "queued" }),
        Progress::Running(p) => json!({ "state": "running", "progress": p }),
        Progress::Done(files) => json!({
            "state": "done",
            "files": files.iter().map(|f| f.to_string_lossy()).collect::<Vec<_>>(),
        }),
        Progress::Failed(why) => json!({ "state": "failed", "message": why }),
    };
    json!({ "key": u.key, "progress": progress })
}

fn update_from(v: &Value) -> DownloadUpdate {
    let p = &v["progress"];
    let progress = match str_of(&p["state"]) {
        "running" => Progress::Running(p["progress"].as_f64().unwrap_or(0.0) as f32),
        "done" => Progress::Done(
            list(&p["files"])
                .iter()
                .map(|f| PathBuf::from(str_of(f)))
                .collect(),
        ),
        "failed" => Progress::Failed(string(&p["message"])),
        _ => Progress::Queued,
    };
    DownloadUpdate {
        key: string(&v["key"]),
        progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_round_trips() {
        let f = Found {
            key: "k".into(),
            folder: true,
            artist: "Bonobo".into(),
            title: "Migration".into(),
            size: 9_000_000_000,
            duration: 312.5,
            tracks: vec![Found {
                key: "k1".into(),
                title: "Kerala".into(),
                ..Default::default()
            }],
            track_count: 12,
            group: "1. Bonobo – Kerala".into(),
            ..Default::default()
        };
        assert_eq!(found_from(&found_json(&f)), f);
    }

    #[test]
    fn reads_the_sites_a_source_takes_links_from() {
        let hello = json!({
            "protocol": PROTOCOL,
            "manifest": { "id": "x", "name": "X" },
            "source": { "name": "X", "icon_svg": "", "hint": "", "sites": ["open.spotify.com"] },
        });
        assert_eq!(described_from(&hello).unwrap().sites, ["open.spotify.com"]);
        // An add-on from before sites takes none.
        let old = json!({ "manifest": { "id": "x" }, "source": { "name": "X" } });
        assert!(described_from(&old).unwrap().sites.is_empty());
        assert!(!described_from(&old).unwrap().previews);
        let mut hello = hello;
        hello["source"]["previews"] = json!(true);
        assert!(described_from(&hello).unwrap().previews);
    }

    #[test]
    fn queries_round_trip() {
        let q = SourceQuery {
            text: "https://open.spotify.com/playlist/x".into(),
            kind: QueryKind::Playlist,
            prefer: Prefer::Mp3,
            skip: vec!["3. Bonobo – Kerala".into()],
            songs: vec!["Bonobo - Kerala".into(), "Bonobo - Kong".into()],
            restored: true,
        };
        assert_eq!(query_from(&query_json(&q)), q);
        // An older app sends no skip.
        let old = query_from(&json!({ "text": "Kerala", "kind": "track", "prefer": "any" }));
        assert!(old.skip.is_empty() && old.songs.is_empty() && !old.restored);
    }

    #[test]
    fn updates_round_trip() {
        for progress in [
            Progress::Queued,
            Progress::Running(0.5),
            Progress::Done(vec![PathBuf::from("/m/a.flac")]),
            Progress::Failed("Nobody could send it.".into()),
        ] {
            let u = DownloadUpdate {
                key: "x".into(),
                progress,
            };
            assert_eq!(update_from(&update_json(&u)), u);
        }
    }

    #[test]
    fn status_pages_round_trip() {
        let page = StatusPage {
            title: "Sharing".into(),
            line: "Online · 2 uploading".into(),
            note: "".into(),
            sections: vec![StatusSection {
                heading: "Uploads".into(),
                rows: vec![("Now".into(), "2".into()), ("Today".into(), "14".into())],
            }],
        };
        assert_eq!(status_from(&status_json(&page)), page);
    }

    #[test]
    fn errors_round_trip() {
        for e in [
            PluginError::Network("down".into()),
            PluginError::Format("odd".into()),
            PluginError::NotSupported,
        ] {
            assert_eq!(error_from(&error_json(&e)), e);
        }
    }

    #[test]
    fn settings_and_config_round_trip() {
        let s = Setting::new(
            "download_folder",
            SettingKind::Folder,
            "Download folder",
            "Where",
        );
        assert_eq!(setting_from(&setting_json(&s)), s);
        for on in [true, false] {
            let s = Setting::new("share", SettingKind::Switch(on), "Share", "");
            assert_eq!(setting_from(&setting_json(&s)), s);
        }
        let c = PluginConfig {
            values: [("username".to_string(), "dj".to_string())].into(),
            dir: Some(PathBuf::from("/c/plugins/x")),
            music_folders: vec![PathBuf::from("/m/DB"), PathBuf::from("/m/Sets")],
        };
        assert_eq!(config_from(&config_json(&c)), c);
        assert!(c.is_on("share", true));
        assert!(!c.is_on("share", false));
    }
}
