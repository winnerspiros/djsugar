//! Audio plugins (CLAP, VST3 and Audio Units) for the effect slots:
//! finding them, remembering what was found, and which ones the DJ wants
//! in the FX lists.
//!
//! Nothing is scanned at start-up. Scan, in Settings › Plugins,
//! looks through the plugin folders and checks each new or changed file in
//! a throwaway helper process, so a plugin that crashes or hangs while it is
//! looked at only fails its own row. What was found is kept in
//! `audio-plugins.json` beside the settings, by file size and date, so the
//! next scan only opens files that changed. Only effects the DJ ticks show
//! in the FX lists, and only those are handed to the engine.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, UNIX_EPOCH};

use djsugar_engine::plugins::{scan, scan_audio_units, Descriptor, Plugin, PluginEntry, ScanError};
use djsugar_engine::{AudioEngine, AVAILABLE_EFFECTS};
use serde_json::{json, Value};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::settings;
use crate::{AppWindow, EffectList, FxRow};

/// Settings keys: the folders looked through, and the effects shown.
pub const FOLDERS: &str = "audio_plugin_folders";
pub const SHOWN: &str = "audio_plugins_shown";

/// How long one file may take to describe itself.
const SCAN_TIMEOUT: Duration = Duration::from_secs(10);
/// How many files are looked at side by side, at most.
const SCAN_WORKERS: usize = 6;
/// How deep under a folder plugins are looked for.
const DEPTH: usize = 4;

/// One effect a scan found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Format, file and id: the same between scans and sessions.
    pub key: String,
    pub name: String,
    pub vendor: String,
    /// "CLAP", "VST3" or "AU".
    pub format: &'static str,
    pub plugin: Plugin,
    /// What the plugin says it is (CLAP features, VST3 sub-categories).
    pub features: Vec<String>,
}

/// What a scan learned about one file (or bundle).
#[derive(Clone, Debug, PartialEq)]
struct FileEntry {
    path: PathBuf,
    size: u64,
    modified: u64,
    found: Vec<Descriptor>,
    /// Why it couldn't be read, if it couldn't.
    error: Option<String>,
}

/// Everything found, as kept between sessions.
#[derive(Clone, Debug, Default, PartialEq)]
struct Catalog {
    files: Vec<FileEntry>,
    audio_units: Vec<Descriptor>,
}

enum Progress {
    /// Index, total, and the file's name.
    Checking(usize, usize, String),
    Done(Catalog),
}

thread_local! {
    /// The names of the plugins in the FX lists, for [`effect_name`].
    static NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The name of what a slot's `loaded_effect` holds: a built-in effect or a
/// plugin from the FX lists ("" for none).
pub fn effect_name(loaded_effect: f64) -> String {
    match djsugar_engine::plugin_index(loaded_effect) {
        Some(i) => NAMES.with(|n| n.borrow().get(i).cloned().unwrap_or_default()),
        None => djsugar_engine::effect_name(loaded_effect).to_string(),
    }
}

pub struct AudioPlugins {
    engine: Rc<AudioEngine>,
    app: slint::Weak<AppWindow>,
    catalog: Catalog,
    scan: Option<Receiver<Progress>>,
    /// What the scan is doing, or what the last one found.
    pub note: String,
}

impl AudioPlugins {
    pub fn new(app: &AppWindow, engine: Rc<AudioEngine>) -> Self {
        let mut p = Self {
            engine,
            app: app.as_weak(),
            catalog: load_catalog(),
            scan: None,
            note: String::new(),
        };
        // Tweaks made in a plugin's window last between sessions.
        if let Some(dir) = settings::config_dir() {
            p.engine.set_plugin_sound_folder(dir.join("plugin-sounds"));
        }
        p.publish();
        p
    }

    /// Whether the plugin helper came with this copy of DJ Sugar.
    pub fn available(&self) -> bool {
        self.engine.plugins_available()
    }

    /// The folders looked through: the system's usual ones until the DJ
    /// changes the list.
    pub fn folders(&self) -> Vec<PathBuf> {
        if settings::load()[FOLDERS].is_array() {
            settings::paths(FOLDERS)
        } else {
            default_folders()
        }
    }

    pub fn set_folders(&self, folders: &[PathBuf]) {
        let list: Vec<String> = folders
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        settings::set(FOLDERS, json!(list));
    }

    /// Every effect found, by name.
    pub fn effects(&self) -> Vec<Found> {
        self.found(Descriptor::is_audio_effect)
    }

    /// Every instrument found, by name: for the Instrument input.
    pub fn instruments(&self) -> Vec<Found> {
        self.found(Descriptor::is_instrument)
    }

    fn found(&self, keep: fn(&Descriptor) -> bool) -> Vec<Found> {
        let mut out = Vec::new();
        for f in &self.catalog.files {
            let format = if is_vst3(&f.path) { "VST3" } else { "CLAP" };
            for d in f.found.iter().filter(|d| keep(d)) {
                let file = f.path.clone();
                let id = d.id.clone();
                out.push(Found {
                    key: format!("{}:{}#{}", format.to_lowercase(), f.path.display(), d.id),
                    name: d.name.clone(),
                    vendor: d.vendor.clone(),
                    format,
                    plugin: if format == "VST3" {
                        Plugin::Vst3 { file, id }
                    } else {
                        Plugin::Clap { file, id }
                    },
                    features: d.features.clone(),
                });
            }
        }
        for d in self.catalog.audio_units.iter().filter(|d| keep(d)) {
            out.push(Found {
                key: format!("au:{}", d.id),
                name: d.name.clone(),
                vendor: d.vendor.clone(),
                format: "AU",
                plugin: Plugin::Au { id: d.id.clone() },
                features: d.features.clone(),
            });
        }
        out.sort_by(|a, b| {
            (a.name.to_lowercase(), a.format).cmp(&(b.name.to_lowercase(), b.format))
        });
        out
    }

    /// Files that couldn't be read, with why.
    pub fn failures(&self) -> Vec<(PathBuf, String)> {
        self.catalog
            .files
            .iter()
            .filter_map(|f| f.error.clone().map(|e| (f.path.clone(), e)))
            .collect()
    }

    pub fn shown(&self) -> Vec<String> {
        settings::load()[SHOWN]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Shows or hides an effect in the FX lists.
    pub fn set_shown(&mut self, key: &str, on: bool) {
        self.set_shown_all(&[key.to_string()], on);
    }

    /// Shows or hides several at once, such as everything from one maker.
    pub fn set_shown_all(&mut self, keys: &[String], on: bool) {
        let mut shown = self.shown();
        shown.retain(|k| !keys.contains(k));
        if on {
            shown.extend(keys.iter().cloned());
        }
        settings::set(SHOWN, json!(shown));
        self.publish();
    }

    pub fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Looks through the folders again in the background. Files already
    /// known, unchanged, are not opened again.
    pub fn start_scan(&mut self) {
        let Some(helper) = djsugar_engine::plugin_helper() else {
            return;
        };
        if self.scan.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let folders = self.folders();
        let known = self.catalog.clone();
        let spawned = std::thread::Builder::new()
            .name("plugin scan".into())
            .spawn(move || {
                let catalog = run_scan(&helper, &folders, &known, |done, total, path| {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let _ = tx.send(Progress::Checking(done, total, name));
                });
                let _ = tx.send(Progress::Done(catalog));
            });
        if spawned.is_ok() {
            self.scan = Some(rx);
            self.note = "Looking for plugins…".into();
        }
    }

    /// Forgets what is known about `path`, then scans, so it is tried again.
    pub fn retry(&mut self, path: &Path) {
        self.catalog.files.retain(|f| f.path != path);
        self.start_scan();
    }

    /// Takes the scan's progress; true when there is something new to show.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.scan else {
            return false;
        };
        let mut changed = false;
        while let Ok(p) = rx.try_recv() {
            changed = true;
            match p {
                Progress::Checking(done, total, name) => {
                    self.note = format!("Checking {} of {total}: {name}", (done + 1).min(total));
                }
                Progress::Done(catalog) => {
                    self.catalog = catalog;
                    save_catalog(&self.catalog);
                    self.scan = None;
                    let effects = self.effects().len();
                    let instruments = self.instruments().len();
                    let failed = self.failures().len();
                    self.note = found_note(effects, instruments, failed);
                    self.publish();
                    break;
                }
            }
        }
        changed
    }

    /// The ticked effects that are still there, in the FX lists' order.
    fn chosen(&self) -> Vec<Found> {
        let shown = self.shown();
        self.effects()
            .into_iter()
            .filter(|f| shown.contains(&f.key))
            .collect()
    }

    /// The ticked instruments, in the instrument picker's order.
    pub fn chosen_instruments(&self) -> Vec<Found> {
        let shown = self.shown();
        self.instruments()
            .into_iter()
            .filter(|f| shown.contains(&f.key))
            .collect()
    }

    /// Hands the ticked effects to the engine and the FX lists, after the
    /// built-in effects.
    fn publish(&mut self) {
        let chosen = self.chosen();
        self.engine.set_plugins(chosen.iter().map(entry).collect());
        let effect_names = list_names(&chosen);
        NAMES.with(|n| *n.borrow_mut() = effect_names.clone());
        let instruments = self.chosen_instruments();
        self.engine
            .set_instruments(instruments.iter().map(entry).collect());
        if let Some(app) = self.app.upgrade() {
            let names: Vec<SharedString> = AVAILABLE_EFFECTS
                .iter()
                .map(|&n| SharedString::from(n))
                .chain(effect_names.iter().map(|n| SharedString::from(n.as_str())))
                .collect();
            app.global::<EffectList>()
                .set_names(ModelRc::new(VecModel::from(names)));
            app.global::<EffectList>()
                .set_builtin_count(AVAILABLE_EFFECTS.len() as i32);
            app.global::<EffectList>()
                .set_plugin_rows(ModelRc::new(VecModel::from(plugin_rows(
                    &chosen,
                    &effect_names,
                ))));
            let names: Vec<SharedString> = list_names(&instruments)
                .iter()
                .map(|n| SharedString::from(n.as_str()))
                .collect();
            app.global::<EffectList>()
                .set_instruments(ModelRc::new(VecModel::from(names)));
            crate::instruments::show_plugins(&app, &instruments, &list_names(&instruments));
        }
    }
}

/// The kinds of effect the menu groups by, in its order.
const KINDS: [&str; 8] = [
    "Delay & Echo",
    "Reverb",
    "Filter & EQ",
    "Modulation",
    "Distortion",
    "Dynamics",
    "Rhythm",
    "Other",
];
const OTHER: usize = KINDS.len() - 1;

/// The built-in effects' kinds, by name.
fn builtin_kind(name: &str) -> usize {
    match name {
        "Echo" | "Echo Out" | "Spiral" | "Filter Echo" => 0,
        "Reverb" | "Hall" | "Rev Roll" => 1,
        "Filter" | "Ladder" => 2,
        "Flanger" | "Phaser" | "Chorus" => 3,
        "Bitcrusher" | "Drive" | "Noise" => 4,
        "Gate" | "Roll" | "Brake" => 6,
        _ => OTHER,
    }
}

/// A plugin's kind, from what it says it is (CLAP features, VST3
/// sub-categories), else from words in its name (Audio Units say nothing).
fn plugin_kind(features: &[String], name: &str) -> usize {
    const WORDS: [(usize, &[&str]); 7] = [
        (0, &["delay", "echo"]),
        (
            1,
            &[
                "reverb", "verb", "hall", "plate", "room", "spatial", "shimmer",
            ],
        ),
        (2, &["filter", "eq", "equalizer"]),
        (
            3,
            &[
                "modulation",
                "chorus",
                "flanger",
                "phaser",
                "tremolo",
                "vibrato",
                "ensemble",
                "rotary",
                "pitch shifter",
                "pitch",
            ],
        ),
        (
            4,
            &[
                "distortion",
                "drive",
                "saturat",
                "crush",
                "fuzz",
                "amp",
                "tape",
                "lo-fi",
                "lofi",
            ],
        ),
        (
            5,
            &[
                "dynamics",
                "compressor",
                "limiter",
                "gate",
                "expander",
                "transient",
                "mastering",
                "comp",
            ],
        ),
        (6, &["glitch", "stutter", "slicer", "beat"]),
    ];
    let features: Vec<String> = features.iter().map(|f| f.to_lowercase()).collect();
    // What the plugin says first, then its name.
    for (kind, words) in WORDS {
        if features.iter().any(|f| words.contains(&f.as_str())) {
            return kind;
        }
    }
    // Long words anywhere in the name ("ValhallaDelay"); short ones only
    // as a word of their own, so "eq" doesn't match "Frequency".
    let name = name.to_lowercase();
    let tokens: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    WORDS
        .iter()
        .find(|(_, words)| {
            words.iter().any(|w| {
                if w.len() >= 5 {
                    name.contains(w)
                } else {
                    tokens.contains(w)
                }
            })
        })
        .map_or(OTHER, |(kind, _)| *kind)
}

/// Menu lines for `items` (a label, its place in EffectList.names and its
/// kind): a heading per kind, in [`KINDS`] order, then its effects A to Z.
fn grouped(items: Vec<(String, usize, usize)>) -> Vec<FxRow> {
    let mut rows = Vec::new();
    for (k, kind) in KINDS.iter().enumerate() {
        let mut these: Vec<&(String, usize, usize)> =
            items.iter().filter(|(_, _, i)| *i == k).collect();
        if these.is_empty() {
            continue;
        }
        these.sort_by_key(|(label, _, _)| label.to_lowercase());
        rows.push(FxRow {
            label: kind.to_uppercase().into(),
            index: -1,
        });
        rows.extend(these.into_iter().map(|(label, index, _)| FxRow {
            label: label.as_str().into(),
            index: *index as i32,
        }));
    }
    rows
}

/// The DJ Sugar FX tab: the built-in effects by kind.
pub fn builtin_rows() -> Vec<FxRow> {
    grouped(
        AVAILABLE_EFFECTS
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), i, builtin_kind(n)))
            .collect(),
    )
}

/// The Plugins tab: the ticked effects by kind; `names` are their names
/// in the lists (after the built-in effects).
fn plugin_rows(chosen: &[Found], names: &[String]) -> Vec<FxRow> {
    grouped(
        chosen
            .iter()
            .zip(names)
            .enumerate()
            .map(|(i, (f, n))| {
                (
                    n.clone(),
                    AVAILABLE_EFFECTS.len() + i,
                    plugin_kind(&f.features, &f.name),
                )
            })
            .collect(),
    )
}

/// The names to show in a chooser: a plugin ticked in more than one format
/// says which, as in "Pro-Q 3 · VST3".
fn list_names(list: &[Found]) -> Vec<String> {
    list.iter()
        .map(|f| {
            if list.iter().filter(|o| o.name == f.name).count() > 1 {
                format!("{} · {}", f.name, f.format)
            } else {
                f.name.clone()
            }
        })
        .collect()
}

fn entry(f: &Found) -> PluginEntry {
    PluginEntry {
        key: f.key.clone(),
        name: f.name.clone(),
        plugin: f.plugin.clone(),
    }
}

/// What a scan found, in a sentence.
fn found_note(effects: usize, instruments: usize, failed: usize) -> String {
    let mut parts = Vec::new();
    if effects > 0 {
        parts.push(format!("{effects} effect{}", plural(effects)));
    }
    if instruments > 0 {
        parts.push(format!("{instruments} instrument{}", plural(instruments)));
    }
    let found = if parts.is_empty() {
        "No plugins found in these folders".to_string()
    } else {
        format!("Found {}", parts.join(" and "))
    };
    if failed > 0 {
        format!("{found}; {failed} file{} couldn't be read.", plural(failed))
    } else {
        format!("{found}.")
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn is_vst3(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("vst3"))
}

fn is_plugin(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("vst3") || e.eq_ignore_ascii_case("clap"))
}

/// The usual plugin folders on this system.
fn default_folders() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut out = Vec::new();
    if cfg!(target_os = "macos") {
        for kind in ["CLAP", "VST3"] {
            out.push(PathBuf::from(format!("/Library/Audio/Plug-Ins/{kind}")));
            if let Some(h) = &home {
                out.push(h.join(format!("Library/Audio/Plug-Ins/{kind}")));
            }
        }
    } else if cfg!(windows) {
        if let Some(common) = std::env::var_os("COMMONPROGRAMFILES").map(PathBuf::from) {
            out.push(common.join("CLAP"));
            out.push(common.join("VST3"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            out.push(local.join("Programs").join("Common").join("CLAP"));
            out.push(local.join("Programs").join("Common").join("VST3"));
        }
    } else {
        if let Some(h) = &home {
            out.push(h.join(".clap"));
            out.push(h.join(".vst3"));
        }
        for p in [
            "/usr/lib/clap",
            "/usr/local/lib/clap",
            "/usr/lib/vst3",
            "/usr/local/lib/vst3",
        ] {
            out.push(PathBuf::from(p));
        }
    }
    out
}

/// `.clap` and `.vst3` files and bundles under `folders`.
fn find_plugins(folders: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if is_plugin(&p) {
                out.push(p);
            } else if depth > 0 && p.is_dir() {
                walk(&p, depth - 1, out);
            }
        }
    }
    let mut out = Vec::new();
    for f in folders {
        walk(f, DEPTH, &mut out);
    }
    out.dedup();
    out
}

/// Size and date that tell whether a plugin changed. A bundle's are its
/// binary folder's, which an update replaces.
fn stamp(path: &Path) -> (u64, u64) {
    let meta = |p: &Path| std::fs::metadata(p).ok();
    let target = if path.is_dir() {
        let contents = path.join("Contents");
        if contents.is_dir() {
            contents
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };
    let m = meta(&target);
    let size = m
        .as_ref()
        .map_or(0, |m| if m.is_file() { m.len() } else { 0 });
    let modified = m
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    (size, modified)
}

fn run_scan(
    helper: &Path,
    folders: &[PathBuf],
    known: &Catalog,
    progress: impl Fn(usize, usize, &Path) + Sync,
) -> Catalog {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    let files = find_plugins(folders);
    let total = files.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let entries: Mutex<Vec<Option<FileEntry>>> = Mutex::new(vec![None; total]);
    // Each file is opened in its own helper process, so several can be
    // looked at side by side; most of the time goes on loading libraries.
    let workers = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .clamp(1, SCAN_WORKERS);
    let mut audio_units = Vec::new();
    std::thread::scope(|s| {
        let au = cfg!(target_os = "macos")
            .then(|| s.spawn(|| scan_audio_units(helper, SCAN_TIMEOUT * 3).unwrap_or_default()));
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = files.get(i) else {
                    break;
                };
                progress(done.load(Ordering::Relaxed), total, path);
                let entry = scan_file(helper, path, known);
                entries.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(entry);
                done.fetch_add(1, Ordering::Relaxed);
            });
        }
        if let Some(au) = au {
            audio_units = au.join().unwrap_or_default();
        }
    });
    Catalog {
        files: entries
            .into_inner()
            .unwrap_or_else(|e| e.into_inner())
            .into_iter()
            .flatten()
            .collect(),
        audio_units,
    }
}

/// What one file holds: from the last scan when it hasn't changed since,
/// otherwise from a throwaway helper.
fn scan_file(helper: &Path, path: &Path, known: &Catalog) -> FileEntry {
    let (size, modified) = stamp(path);
    if let Some(k) = known
        .files
        .iter()
        .find(|f| f.path == path && f.size == size && f.modified == modified)
    {
        return k.clone();
    }
    let (found, error) = match scan(helper, path, SCAN_TIMEOUT) {
        Ok(found) => (found, None),
        Err(e) => (Vec::new(), Some(scan_error(&e))),
    };
    FileEntry {
        path: path.to_path_buf(),
        size,
        modified,
        found,
        error,
    }
}

fn scan_error(e: &ScanError) -> String {
    match e {
        ScanError::TimedOut => "It took too long to answer.".into(),
        ScanError::Crashed(how) => format!("It crashed while being looked at ({how})."),
        ScanError::Failed(why) => why.clone(),
    }
}

fn catalog_path() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("audio-plugins.json"))
}

fn descriptor_json(d: &Descriptor) -> Value {
    json!({
        "id": d.id,
        "name": d.name,
        "vendor": d.vendor,
        "version": d.version,
        "features": d.features,
    })
}

fn descriptor_from(v: &Value) -> Option<Descriptor> {
    let s = |k: &str| v[k].as_str().map(String::from);
    Some(Descriptor {
        id: s("id")?,
        name: s("name").unwrap_or_default(),
        vendor: s("vendor").unwrap_or_default(),
        version: s("version").unwrap_or_default(),
        features: v["features"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|f| f.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn catalog_json(c: &Catalog) -> Value {
    json!({
        "files": c.files.iter().map(|f| json!({
            "path": f.path.to_string_lossy(),
            "size": f.size,
            "modified": f.modified,
            "found": f.found.iter().map(descriptor_json).collect::<Vec<_>>(),
            "error": f.error,
        })).collect::<Vec<_>>(),
        "audio_units": c.audio_units.iter().map(descriptor_json).collect::<Vec<_>>(),
    })
}

fn catalog_from(v: &Value) -> Catalog {
    let list = |k: &str| v[k].as_array().cloned().unwrap_or_default();
    Catalog {
        files: list("files")
            .iter()
            .filter_map(|f| {
                Some(FileEntry {
                    path: PathBuf::from(f["path"].as_str()?),
                    size: f["size"].as_u64().unwrap_or(0),
                    modified: f["modified"].as_u64().unwrap_or(0),
                    found: f["found"]
                        .as_array()
                        .map(|a| a.iter().filter_map(descriptor_from).collect())
                        .unwrap_or_default(),
                    error: f["error"].as_str().map(String::from),
                })
            })
            .collect(),
        audio_units: list("audio_units")
            .iter()
            .filter_map(descriptor_from)
            .collect(),
    }
}

fn load_catalog() -> Catalog {
    catalog_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|v| catalog_from(&v))
        .unwrap_or_default()
}

fn save_catalog(c: &Catalog) {
    if let Some(p) = catalog_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(
            p,
            serde_json::to_vec_pretty(&catalog_json(c)).unwrap_or_default(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scan_note_names_effects_and_instruments() {
        assert_eq!(found_note(0, 0, 0), "No plugins found in these folders.");
        assert_eq!(found_note(1, 0, 0), "Found 1 effect.");
        assert_eq!(found_note(3, 2, 0), "Found 3 effects and 2 instruments.");
        assert_eq!(
            found_note(0, 1, 2),
            "Found 1 instrument; 2 files couldn't be read."
        );
    }

    #[test]
    fn the_catalog_round_trips_through_json() {
        let c = Catalog {
            files: vec![FileEntry {
                path: "/x/Delay.clap".into(),
                size: 10,
                modified: 20,
                found: vec![Descriptor {
                    id: "com.x.delay".into(),
                    name: "Delay".into(),
                    vendor: "X".into(),
                    version: "1".into(),
                    features: vec!["audio-effect".into()],
                }],
                error: None,
            }],
            audio_units: Vec::new(),
        };
        assert_eq!(catalog_from(&catalog_json(&c)), c);
    }

    #[test]
    fn finds_plugin_files_and_bundles_but_not_inside_them() {
        let dir = std::env::temp_dir().join(format!("djsugar-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Vendor/Reverb.vst3/Contents/x86_64-linux")).unwrap();
        std::fs::write(
            dir.join("Vendor/Reverb.vst3/Contents/x86_64-linux/Reverb.so"),
            b"",
        )
        .unwrap();
        std::fs::write(dir.join("Delay.clap"), b"").unwrap();
        std::fs::write(dir.join("readme.txt"), b"").unwrap();
        let found = find_plugins(std::slice::from_ref(&dir));
        assert_eq!(
            found,
            vec![dir.join("Delay.clap"), dir.join("Vendor/Reverb.vst3")]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;

    #[test]
    fn effects_are_grouped_by_kind() {
        let f = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(plugin_kind(&f(&["Fx", "Delay"]), "Anything"), 0);
        assert_eq!(plugin_kind(&f(&["audio-effect", "reverb"]), "X"), 1);
        assert_eq!(plugin_kind(&[], "ValhallaDelay"), 0);
        assert_eq!(plugin_kind(&[], "Rev PLATE-140"), 1);
        assert_eq!(plugin_kind(&[], "Pro-C 2 Comp"), 5);
        assert_eq!(plugin_kind(&[], "Frequency Shifter X"), OTHER);
        assert_eq!(plugin_kind(&[], "Chorus JUN-6"), 3);
        let rows = builtin_rows();
        // Every built-in effect once, under a heading; none in Other.
        let items: Vec<_> = rows.iter().filter(|r| r.index >= 0).collect();
        assert_eq!(items.len(), AVAILABLE_EFFECTS.len());
        assert_eq!(rows[0].label, "DELAY & ECHO");
        assert!(rows.iter().all(|r| r.label != "OTHER"));
        // A to Z within a kind: Echo, Echo Out, Filter Echo, Spiral.
        let first: Vec<&str> = rows[1..5].iter().map(|r| r.label.as_str()).collect();
        assert_eq!(first, ["Echo", "Echo Out", "Filter Echo", "Spiral"]);
    }
}
