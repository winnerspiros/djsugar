//! The sampler's sample packs and what each of its 16 pads plays.
//!
//! A pack is a folder in `<config>/samples/` with a `pack.json` naming its
//! samples, in slot order (see openspec/changes/add-sample-packs). The
//! built-in sounds are the Default DJ Sample Pack, which has no folder.
//! What the DJ changes on a pad (its sample, name, level and play mode) is
//! kept per pack in `settings.json` under `sampler`, never in the pack's
//! files, so Reset to Pack Sample just forgets it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::sampler::{sampler_group, Sample as Audio, NUM_SAMPLERS, SAMPLE_NAMES};
use djsugar_engine::{AudioEngine, SampleSource};
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, DataTransfer, Image, ModelRc, SharedString, VecModel, Weak};

use crate::browser::{file_stem, Browser};
use crate::{settings, AppWindow, SampleChoice, SamplePack, SamplerPad, SamplerUi};

pub const DEFAULT_PACK: &str = "default";
const DEFAULT_PACK_NAME: &str = "Default DJ Sample Pack";
const PACK_FILE: &str = "pack.json";
/// How long the level knob rests before a change is saved.
const SAVE_AFTER: Duration = Duration::from_millis(600);
/// What a pad dragged in edit mode carries: its slot.
const PAD_PAYLOAD: &str = "djsugar-sampler-pad:";

/// A sample on a pad or in a pack.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub name: String,
    pub source: SampleSource,
    /// `[SamplerN],pregain`: 0 silent, 1 as recorded, 2 +6 dB.
    pub level: f64,
    /// `[SamplerN],play_mode`.
    pub mode: u8,
}

impl Sample {
    fn new(name: String, source: SampleSource) -> Self {
        Sample {
            name,
            source,
            level: 1.0,
            mode: 0,
        }
    }

    fn missing(&self) -> bool {
        matches!(&self.source, SampleSource::File(p) if !p.exists())
    }
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub cover: Option<PathBuf>,
    /// Its samples by slot; shorter than 16 when the last slots are empty.
    pub samples: Vec<Option<Sample>>,
}

impl Pack {
    fn builtin() -> Pack {
        Pack {
            id: DEFAULT_PACK.into(),
            name: DEFAULT_PACK_NAME.into(),
            cover: None,
            samples: SAMPLE_NAMES
                .iter()
                .enumerate()
                .map(|(n, name)| Some(Sample::new((*name).into(), SampleSource::Builtin(n))))
                .collect(),
        }
    }

    /// The pack in folder `dir`, if it has a readable `pack.json`.
    fn read(dir: &Path) -> Option<Pack> {
        let text = std::fs::read_to_string(dir.join(PACK_FILE)).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        let id = dir.file_name()?.to_string_lossy().into_owned();
        let samples = v["samples"]
            .as_array()
            .map(|a| {
                a.iter()
                    .take(NUM_SAMPLERS)
                    .map(|s| {
                        if let Some(n) = s["builtin"].as_u64().map(|n| n as usize) {
                            // A built-in sound, from a pack exported off the Default one.
                            let name = s["name"].as_str().map_or_else(
                                || SAMPLE_NAMES.get(n).copied().unwrap_or("").to_owned(),
                                str::to_owned,
                            );
                            let mut sample = Sample::new(name, SampleSource::Builtin(n));
                            apply_settings(&mut sample, s);
                            return (n < SAMPLE_NAMES.len()).then_some(sample);
                        }
                        let file = s["file"].as_str().filter(|f| !f.is_empty())?;
                        let path = dir.join(file);
                        let name = s["name"]
                            .as_str()
                            .map_or_else(|| file_stem(&path), str::to_owned);
                        let mut sample = Sample::new(name, SampleSource::File(path));
                        apply_settings(&mut sample, s);
                        Some(sample)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Pack {
            name: v["name"].as_str().unwrap_or(&id).to_owned(),
            cover: v["cover"]
                .as_str()
                .map(|c| dir.join(c))
                .filter(|c| c.exists()),
            id,
            samples,
        })
    }

    fn sample(&self, slot: usize) -> Option<&Sample> {
        self.samples.get(slot).and_then(Option::as_ref)
    }
}

/// Where packs live: `<config>/samples`.
pub fn packs_dir() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("samples"))
}

/// The Default DJ Sample Pack, then the packs in `dir` by name.
pub fn load_packs(dir: Option<&Path>) -> Vec<Pack> {
    let mut packs: Vec<Pack> = dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Pack::read(&e.path()))
        .collect();
    packs.sort_by_key(|p| p.name.to_lowercase());
    packs.insert(0, Pack::builtin());
    packs
}

/// Makes an empty pack called `name` in `dir`; its id is the folder name.
pub fn create_pack(dir: &Path, name: &str) -> std::io::Result<String> {
    let id = new_pack_id(dir, name);
    let folder = dir.join(&id);
    std::fs::create_dir_all(&folder)?;
    let body = json!({ "name": name, "samples": [] });
    std::fs::write(
        folder.join(PACK_FILE),
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    )?;
    Ok(id)
}

/// The extension of a shared pack: a zip of `pack.json`, its samples and
/// its cover.
pub const PACK_EXTENSION: &str = "djsugarpack";

/// The most a pack may unpack to, so a broken or hostile file can't fill
/// the disk: sixteen minute-long WAVs fit easily.
const MAX_PACK_BYTES: u64 = 1 << 30;

/// The id (folder name) a pack called `name` gets in `dir`: its name in
/// lower case with dashes, numbered if taken.
fn new_pack_id(dir: &Path, name: &str) -> String {
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() || base == DEFAULT_PACK {
        "pack".to_string()
    } else {
        base
    };
    let mut id = base.clone();
    let mut n = 2;
    while dir.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Renames the pack in folder `folder` (its `pack.json` name).
pub fn rename_pack(folder: &Path, name: &str) -> Result<(), String> {
    let file = folder.join(PACK_FILE);
    let text = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let mut v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    v["name"] = json!(name);
    std::fs::write(&file, serde_json::to_string_pretty(&v).unwrap_or_default())
        .map_err(|e| e.to_string())
}

/// Moves the pack in `folder` out of the packs, into `.deleted` beside
/// them, so a slip can still be undone by hand.
pub fn delete_pack(folder: &Path) -> Result<(), String> {
    let (Some(dir), Some(id)) = (folder.parent(), folder.file_name()) else {
        return Err("not a pack folder".into());
    };
    let bin = dir.join(".deleted");
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    std::fs::rename(
        folder,
        bin.join(format!("{}-{stamp}", id.to_string_lossy())),
    )
    .map_err(|e| e.to_string())
}

/// Writes a pack called `name` with these pads (`samples`, by slot) and
/// cover to `to`, an `.djsugarpack` zip: `pack.json` and each sample file
/// once. Built-in sounds stay built in. Pads whose file is missing are left
/// out. Returns how many samples went in.
pub fn export_pack(
    name: &str,
    cover: Option<&Path>,
    samples: &[Option<Sample>],
    to: &Path,
) -> Result<usize, String> {
    use std::io::Write;
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let file = std::fs::File::create(to).map_err(|e| err(&e))?;
    let mut zip = zip::ZipWriter::new(file);
    // Audio is already compressed, or barely shrinks: store it.
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut names: Vec<String> = Vec::new();
    let mut entries = Vec::new();
    let mut count = 0;
    let add_file = |zip: &mut zip::ZipWriter<std::fs::File>,
                    names: &mut Vec<String>,
                    path: &Path|
     -> Result<String, String> {
        let base = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sample".into());
        let mut unique = base.clone();
        let mut n = 2;
        while names.contains(&unique) || unique == PACK_FILE {
            let p = Path::new(&base);
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            unique = match p.extension() {
                Some(e) => format!("{stem}-{n}.{}", e.to_string_lossy()),
                None => format!("{stem}-{n}"),
            };
            n += 1;
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        zip.start_file(unique.as_str(), stored)
            .map_err(|e| err(&e))?;
        zip.write_all(&bytes).map_err(|e| err(&e))?;
        names.push(unique.clone());
        Ok(unique)
    };
    for s in samples.iter().take(NUM_SAMPLERS) {
        let entry = match s {
            Some(s) if !s.missing() => {
                let mut e = Map::new();
                match &s.source {
                    SampleSource::File(p) => {
                        let f = add_file(&mut zip, &mut names, p)?;
                        e.insert("file".into(), json!(f));
                    }
                    SampleSource::Builtin(n) => {
                        e.insert("builtin".into(), json!(n));
                    }
                    SampleSource::Empty => {
                        entries.push(Value::Null);
                        continue;
                    }
                }
                e.insert("name".into(), json!(s.name));
                e.insert("level".into(), json!((s.level * 1000.0).round() / 1000.0));
                e.insert("mode".into(), json!(s.mode));
                count += 1;
                Value::Object(e)
            }
            _ => Value::Null,
        };
        entries.push(entry);
    }
    while entries.last() == Some(&Value::Null) {
        entries.pop();
    }
    let mut body = json!({ "name": name, "samples": entries });
    if let Some(c) = cover.filter(|c| c.exists()) {
        body["cover"] = json!(add_file(&mut zip, &mut names, c)?);
    }
    zip.start_file(PACK_FILE, zip::write::SimpleFileOptions::default())
        .map_err(|e| err(&e))?;
    zip.write_all(
        serde_json::to_string_pretty(&body)
            .unwrap_or_default()
            .as_bytes(),
    )
    .map_err(|e| err(&e))?;
    zip.finish().map_err(|e| err(&e))?;
    Ok(count)
}

/// Unpacks the `.djsugarpack` at `file` into a new pack folder in `dir`;
/// returns its id. Only `pack.json` and the files it names come out, each
/// by its plain file name, so nothing lands outside the pack's folder.
pub fn import_pack(file: &Path, dir: &Path) -> Result<String, String> {
    let bad = |what: &str| format!("{} isn't a sample pack: {what}", file.display());
    let f = std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| bad(&e.to_string()))?;
    let manifest: Value = {
        let mut entry = archive
            .by_name(PACK_FILE)
            .map_err(|_| bad("it has no pack.json"))?;
        if entry.size() > 1 << 20 {
            return Err(bad("its pack.json is too big"));
        }
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text).map_err(|e| bad(&e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| bad(&e.to_string()))?
    };
    let name = manifest["name"]
        .as_str()
        .filter(|n| !n.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| file_stem(file));
    // The files pack.json names, as plain file names.
    let plain = |v: &Value| -> Option<String> {
        let f = v.as_str()?;
        let base = Path::new(f).file_name()?.to_str()?;
        (base == f && base != PACK_FILE && !base.starts_with('.')).then(|| base.to_owned())
    };
    let mut wanted: Vec<String> = manifest["samples"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| plain(&s["file"]))
        .collect();
    wanted.extend(plain(&manifest["cover"]));
    let total: u64 = wanted
        .iter()
        .filter_map(|w| archive.by_name(w).ok().map(|e| e.size()))
        .sum();
    if total > MAX_PACK_BYTES {
        return Err(bad("it is bigger than 1 GB"));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let id = new_pack_id(dir, &name);
    let work = dir.join(format!(".{id}.importing"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let unpacked = (|| -> Result<(), String> {
        for w in &wanted {
            let Ok(mut entry) = archive.by_name(w) else {
                continue;
            };
            let mut out = std::fs::File::create(work.join(w)).map_err(|e| e.to_string())?;
            // Never more than the header promised.
            let mut limited = std::io::Read::take(&mut entry, MAX_PACK_BYTES);
            std::io::copy(&mut limited, &mut out).map_err(|e| e.to_string())?;
        }
        let mut manifest = manifest.clone();
        manifest["name"] = json!(name);
        // Samples naming files outside the pack are dropped.
        if let Some(samples) = manifest["samples"].as_array_mut() {
            for s in samples.iter_mut() {
                if !s["file"].is_null() && plain(&s["file"]).is_none() {
                    *s = Value::Null;
                }
            }
        }
        if !manifest["cover"].is_null() && plain(&manifest["cover"]).is_none() {
            manifest["cover"] = Value::Null;
        }
        std::fs::write(
            work.join(PACK_FILE),
            serde_json::to_string_pretty(&manifest).unwrap_or_default(),
        )
        .map_err(|e| e.to_string())
    })();
    match unpacked.and_then(|()| std::fs::rename(&work, dir.join(&id)).map_err(|e| e.to_string())) {
        Ok(()) => Ok(id),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            Err(e)
        }
    }
}

/// Takes `level` and `mode` from a pack.json sample or a pad's settings.
fn apply_settings(sample: &mut Sample, v: &Value) {
    if let Some(level) = v["level"].as_f64() {
        sample.level = level.clamp(0.0, 2.0);
    }
    if let Some(mode) = v["mode"].as_u64() {
        sample.mode = mode.min(3) as u8;
    }
}

/// What the DJ changed on a pad, as saved in settings: the sample (`file`,
/// `builtin`, or `empty`), `name`, `level` and `mode`.
pub fn edit_json(sample: Option<&Sample>, base: Option<&Sample>) -> Value {
    let mut m = Map::new();
    let Some(s) = sample else {
        m.insert("empty".into(), json!(true));
        return Value::Object(m);
    };
    if base.map(|b| &b.source) != Some(&s.source) {
        match &s.source {
            SampleSource::File(p) => m.insert("file".into(), json!(p.to_string_lossy())),
            SampleSource::Builtin(n) => m.insert("builtin".into(), json!(n)),
            SampleSource::Empty => m.insert("empty".into(), json!(true)),
        };
    }
    if base.map(|b| &b.name) != Some(&s.name) {
        m.insert("name".into(), json!(s.name));
    }
    if base.is_none_or(|b| (b.level - s.level).abs() > 1e-4) {
        m.insert("level".into(), json!((s.level * 1000.0).round() / 1000.0));
    }
    if base.is_none_or(|b| b.mode != s.mode) {
        m.insert("mode".into(), json!(s.mode));
    }
    Value::Object(m)
}

/// A pad: its pack's sample with the DJ's changes (`edit`) on top.
pub fn resolve(base: Option<&Sample>, edit: Option<&Value>) -> Option<Sample> {
    let Some(e) = edit else {
        return base.cloned();
    };
    if e["empty"].as_bool() == Some(true) {
        return None;
    }
    let source = if let Some(f) = e["file"].as_str() {
        Some(SampleSource::File(PathBuf::from(f)))
    } else {
        e["builtin"]
            .as_u64()
            .map(|n| SampleSource::Builtin(n as usize))
    };
    let mut s = match (source, base) {
        (Some(src), Some(b)) => Sample {
            source: src,
            ..b.clone()
        },
        (Some(src), None) => {
            let name = match &src {
                SampleSource::File(p) => file_stem(p),
                SampleSource::Builtin(n) => SAMPLE_NAMES.get(*n).copied().unwrap_or("").into(),
                SampleSource::Empty => String::new(),
            };
            Sample::new(name, src)
        }
        (None, Some(b)) => b.clone(),
        (None, None) => return None,
    };
    if let Some(name) = e["name"].as_str() {
        s.name = name.to_owned();
    }
    apply_settings(&mut s, e);
    Some(s)
}

/// "0:00.7", or "0:12" from ten seconds on.
pub fn length_text(seconds: f64) -> String {
    let m = (seconds / 60.0).floor();
    let s = seconds - m * 60.0;
    if seconds < 10.0 {
        format!("{m}:{s:04.1}")
    } else {
        format!("{m}:{:02}", s.floor() as u32)
    }
}

/// How long a sample is, read from its file's header.
fn sample_seconds(source: &SampleSource) -> Option<f64> {
    match source {
        SampleSource::Builtin(n) => Some(Audio::builtin(*n, 48_000.0).seconds()),
        SampleSource::File(p) => {
            let d = djsugar_engine::decode::Decoder::open(p).ok()?;
            let a = d.audio();
            (d.length_known() && a.sample_rate > 0).then(|| a.len() as f64 / a.sample_rate as f64)
        }
        SampleSource::Empty => None,
    }
}

/// A sample finished loading onto a slot (or didn't).
struct Loaded {
    slot: usize,
    sample: Option<Sample>,
    /// A change the DJ made, kept only if the sample loads.
    save: bool,
    result: Result<f64, String>,
}

struct Slot {
    pregain: ControlHandle,
    mode: ControlHandle,
    load_selected: ControlHandle,
    eject: ControlHandle,
}

pub struct Sampler {
    app: Weak<AppWindow>,
    engine: Rc<AudioEngine>,
    browser: Rc<RefCell<Browser>>,
    dir: Option<PathBuf>,
    packs: Vec<Pack>,
    pack: usize,
    /// What each slot plays.
    pads: Vec<Option<Sample>>,
    controls: Vec<Slot>,
    /// The pack listed in Choose Sample, and its samples' slots by row.
    choose_pack: usize,
    choose_rows: Vec<usize>,
    loaded_tx: Sender<Loaded>,
    loaded_rx: Receiver<Loaded>,
    picked_tx: Sender<(usize, PathBuf)>,
    picked_rx: Receiver<(usize, PathBuf)>,
    /// Packs picked in the file dialog to import.
    import_tx: Sender<PathBuf>,
    import_rx: Receiver<PathBuf>,
    /// A level or mode changed and isn't saved yet: when it last changed.
    unsaved: Option<Instant>,
}

impl Sampler {
    pub fn install(
        app: &AppWindow,
        engine: Rc<AudioEngine>,
        browser: Rc<RefCell<Browser>>,
        registry: &ControlRegistry,
    ) -> Rc<RefCell<Sampler>> {
        let dir = packs_dir();
        let packs = load_packs(dir.as_deref());
        let chosen = settings::load()["sampler"]["pack"]
            .as_str()
            .unwrap_or(DEFAULT_PACK)
            .to_owned();
        let pack = packs.iter().position(|p| p.id == chosen).unwrap_or(0);
        let controls = (0..NUM_SAMPLERS)
            .map(|n| {
                let g = sampler_group(n);
                let get = |key: &str| {
                    registry
                        .get(&g, key)
                        .unwrap_or_else(|| registry.free(&g, key))
                };
                Slot {
                    pregain: get("pregain"),
                    mode: get("play_mode"),
                    load_selected: get("LoadSelectedTrack"),
                    eject: get("eject"),
                }
            })
            .collect();
        let (loaded_tx, loaded_rx) = mpsc::channel();
        let (picked_tx, picked_rx) = mpsc::channel();
        let (import_tx, import_rx) = mpsc::channel();
        let s = Rc::new(RefCell::new(Sampler {
            app: app.as_weak(),
            engine,
            browser,
            dir,
            packs,
            pack,
            pads: vec![None; NUM_SAMPLERS],
            controls,
            choose_pack: pack,
            choose_rows: Vec::new(),
            loaded_tx,
            loaded_rx,
            picked_tx,
            picked_rx,
            import_tx,
            import_rx,
            unsaved: None,
        }));
        s.borrow_mut().apply_pack();
        let g = app.global::<SamplerUi>();
        let with = {
            let s = s.clone();
            move |f: Box<dyn FnOnce(&mut Sampler)>| f(&mut s.borrow_mut())
        };
        let w = with.clone();
        g.on_pick_pack(move |i| w(Box::new(move |s| s.pick_pack(i.max(0) as usize))));
        let w = with.clone();
        g.on_new_pack(move |name| w(Box::new(move |s| s.new_pack(&name))));
        let w = with.clone();
        g.on_rename_pack(move |name| w(Box::new(move |s| s.rename_pack(&name))));
        let w = with.clone();
        g.on_delete_pack(move || w(Box::new(|s| s.delete_pack())));
        let w = with.clone();
        g.on_export_pack(move || w(Box::new(|s| s.export_pack())));
        let w = with.clone();
        g.on_import_pack(move || w(Box::new(|s| s.pick_pack_file())));
        let w = with.clone();
        g.on_show_pack(move || w(Box::new(|s| s.show_pack())));
        let w = with.clone();
        g.on_browse(move |i| w(Box::new(move |s| s.browse(i.max(0) as usize))));
        let w = with.clone();
        g.on_choose(move |slot, row| w(Box::new(move |s| s.choose(slot as usize, row as usize))));
        let w = with.clone();
        g.on_preview(move |row| w(Box::new(move |s| s.preview(row as usize))));
        let w = with.clone();
        g.on_load_file(move |slot| w(Box::new(move |s| s.pick_file(slot as usize))));
        let w = with.clone();
        g.on_rename(move |slot, name| w(Box::new(move |s| s.rename(slot as usize, &name))));
        let w = with.clone();
        g.on_reset(move |slot| w(Box::new(move |s| s.reset(slot as usize))));
        let w = with.clone();
        g.on_clear(move |slot| w(Box::new(move |s| s.put(slot as usize, None))));
        g.on_pad_payload(|slot| {
            DataTransfer::from(SharedString::from(format!("{PAD_PAYLOAD}{slot}")))
        });
        g.on_accepts(|data| {
            dragged_pad(&data).is_some()
                || Browser::accepts_sample(&data)
                || dropped_pack(&data).is_some()
        });
        let w = with.clone();
        g.on_drop(move |slot, data| w(Box::new(move |s| s.dropped(slot as usize, &data))));
        s
    }

    /// Answers loads and file dialogs, controllers' load and eject, and
    /// saves level and mode changes once they settle.
    pub fn poll(&mut self) {
        while let Ok(l) = self.loaded_rx.try_recv() {
            self.finish(l);
        }
        while let Ok(path) = self.import_rx.try_recv() {
            self.import(&path);
        }
        while let Ok((slot, path)) = self.picked_rx.try_recv() {
            let name = file_stem(&path);
            self.put(slot, Some(Sample::new(name, SampleSource::File(path))));
        }
        let app = self.app.upgrade();
        for slot in 0..NUM_SAMPLERS {
            let c = &self.controls[slot];
            if c.load_selected.get_bool() {
                c.load_selected.set(0.0);
                let picked = app
                    .as_ref()
                    .and_then(|a| self.browser.borrow().selected_sample(a));
                if let Some((path, name)) = picked {
                    self.put(slot, Some(Sample::new(name, SampleSource::File(path))));
                }
            }
            let c = &self.controls[slot];
            if c.eject.get_bool() {
                c.eject.set(0.0);
                self.put(slot, None);
            }
            let c = &self.controls[slot];
            let (level, mode) = (c.pregain.get(), c.mode.get().round().clamp(0.0, 3.0) as u8);
            if let Some(p) = self.pads[slot].as_mut() {
                if (p.level - level).abs() > 1e-4 || p.mode != mode {
                    p.level = level;
                    p.mode = mode;
                    self.unsaved = Some(Instant::now());
                }
            }
        }
        if self.unsaved.is_some_and(|t| t.elapsed() >= SAVE_AFTER) {
            self.unsaved = None;
            self.save();
        }
    }

    fn pack(&self) -> &Pack {
        &self.packs[self.pack]
    }

    /// The DJ's changes to the chosen pack, by slot.
    fn edits(&self) -> BTreeMap<usize, Value> {
        let v = settings::load();
        v["sampler"]["packs"][&self.pack().id]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| {
                        Some((k.parse::<usize>().ok()?.checked_sub(1)?, v.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Puts the chosen pack, with the DJ's changes, on all 16 slots.
    fn apply_pack(&mut self) {
        let edits = self.edits();
        for slot in 0..NUM_SAMPLERS {
            let sample = resolve(self.pack().sample(slot), edits.get(&slot));
            self.load(slot, sample, false);
        }
        self.refresh();
    }

    /// Sends `sample` to the engine; [`Sampler::finish`] takes it when it's
    /// there. A missing file shows on the pad and plays nothing.
    fn load(&mut self, slot: usize, sample: Option<Sample>, save: bool) {
        let source = match &sample {
            Some(s) if !s.missing() => s.source.clone(),
            _ => SampleSource::Empty,
        };
        if !save {
            // The pad shows it straight away; a file that won't load says so.
            self.set_pad(slot, sample.clone());
        }
        let tx = self.loaded_tx.clone();
        self.engine.set_sample(slot, source, move |result| {
            let _ = tx.send(Loaded {
                slot,
                sample,
                save,
                result,
            });
        });
    }

    fn finish(&mut self, l: Loaded) {
        match l.result {
            Ok(_) if l.save => {
                self.set_pad(l.slot, l.sample);
                self.save();
                self.refresh();
            }
            Ok(_) => {}
            Err(e) => self.engine.alert(e),
        }
    }

    /// What slot `slot` plays, with its level and mode on its controls.
    fn set_pad(&mut self, slot: usize, sample: Option<Sample>) {
        if let Some(s) = &sample {
            self.controls[slot].pregain.set(s.level);
            self.controls[slot].mode.set(s.mode as f64);
        }
        self.pads[slot] = sample;
    }

    /// Puts `sample` on `slot` as the DJ's change, once it has loaded.
    fn put(&mut self, slot: usize, sample: Option<Sample>) {
        if slot >= NUM_SAMPLERS {
            return;
        }
        // A new sample keeps the pad's mode; its level starts as recorded.
        let sample = sample.map(|mut s| {
            if let Some(old) = &self.pads[slot] {
                if s.mode == 0 {
                    s.mode = old.mode;
                }
            }
            s
        });
        self.load(slot, sample, true);
    }

    /// Saves the chosen pack and every pad that differs from it.
    fn save(&self) {
        let pack = self.pack();
        let mut slots = Map::new();
        for (slot, pad) in self.pads.iter().enumerate() {
            let base = pack.sample(slot);
            if pad.as_ref() != base {
                slots.insert((slot + 1).to_string(), edit_json(pad.as_ref(), base));
            }
        }
        let mut all = settings::load()["sampler"].clone();
        if !all.is_object() {
            all = json!({});
        }
        all["pack"] = json!(pack.id);
        if !all["packs"].is_object() {
            all["packs"] = json!({});
        }
        if slots.is_empty() {
            if let Some(m) = all["packs"].as_object_mut() {
                m.remove(&pack.id);
            }
        } else {
            all["packs"][&pack.id] = Value::Object(slots);
        }
        settings::set("sampler", all);
    }

    /// Saves which pack is chosen, before its pads are on the slots.
    fn save_choice(&self) {
        let mut all = settings::load()["sampler"].clone();
        if !all.is_object() {
            all = json!({});
        }
        all["pack"] = json!(self.pack().id);
        settings::set("sampler", all);
    }

    fn pick_pack(&mut self, index: usize) {
        if index >= self.packs.len() || index == self.pack {
            return;
        }
        self.pack = index;
        self.save_choice();
        self.apply_pack();
    }

    fn new_pack(&mut self, name: &str) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        match create_pack(&dir, name.trim()) {
            Ok(id) => {
                self.packs = load_packs(Some(&dir));
                self.pack = self.packs.iter().position(|p| p.id == id).unwrap_or(0);
                self.save_choice();
                self.apply_pack();
            }
            Err(e) => self.engine.alert(format!("Couldn't make the pack: {e}")),
        }
    }

    /// The chosen pack's folder; None for the Default pack.
    fn pack_folder(&self) -> Option<PathBuf> {
        let id = &self.pack().id;
        (id != DEFAULT_PACK)
            .then(|| self.dir.as_ref().map(|d| d.join(id)))
            .flatten()
    }

    fn rename_pack(&mut self, name: &str) {
        let name = name.trim();
        let Some(folder) = self.pack_folder() else {
            return;
        };
        if name.is_empty() {
            return;
        }
        if let Err(e) = rename_pack(&folder, name) {
            self.engine.alert(format!("Couldn't rename the pack: {e}"));
            return;
        }
        self.packs[self.pack].name = name.to_owned();
        self.reload_packs();
    }

    /// Deletes the chosen pack (not the Default one) and the DJ's changes
    /// to it; the pads go back to the Default pack.
    fn delete_pack(&mut self) {
        let Some(folder) = self.pack_folder() else {
            return;
        };
        if let Err(e) = delete_pack(&folder) {
            self.engine.alert(format!("Couldn't delete the pack: {e}"));
            return;
        }
        let id = self.pack().id.clone();
        let mut all = settings::load()["sampler"].clone();
        if let Some(m) = all["packs"].as_object_mut() {
            m.remove(&id);
            settings::set("sampler", all);
        }
        self.packs = load_packs(self.dir.as_deref());
        self.pack = 0;
        self.save_choice();
        self.apply_pack();
    }

    /// Exports the pads as they are, under the chosen pack's name, to an
    /// `.djsugarpack` in Downloads, and shows it.
    fn export_pack(&self) {
        let pack = self.pack();
        let folder = djsugar_engine::paths::home_dir()
            .map(|h| h.join("Downloads"))
            .filter(|d| d.is_dir())
            .or_else(djsugar_engine::paths::home_dir);
        let Some(folder) = folder else {
            return;
        };
        let safe: String = pack
            .name
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
            .collect();
        let mut to = folder.join(format!("{safe}.{PACK_EXTENSION}"));
        let mut n = 2;
        while to.exists() {
            to = folder.join(format!("{safe} {n}.{PACK_EXTENSION}"));
            n += 1;
        }
        match export_pack(&pack.name, pack.cover.as_deref(), &self.pads, &to) {
            Ok(_) => {
                crate::files::reveal(&to);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&to);
                self.engine.alert(format!("Couldn't export the pack: {e}"));
            }
        }
    }

    fn pick_pack_file(&self) {
        let tx = self.import_tx.clone();
        crate::prefs::choose_file("Choose a sample pack (.djsugarpack)", move |path| {
            let _ = tx.send(path);
        });
    }

    /// Imports the pack at `path` and plays it.
    fn import(&mut self, path: &Path) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        match import_pack(path, &dir) {
            Ok(id) => {
                self.packs = load_packs(Some(&dir));
                self.pack = self.packs.iter().position(|p| p.id == id).unwrap_or(0);
                self.save_choice();
                self.apply_pack();
            }
            Err(e) => self.engine.alert(e),
        }
    }

    fn show_pack(&self) {
        if let Some(folder) = self.pack_folder() {
            crate::files::reveal(&folder);
        } else if let Some(dir) = &self.dir {
            let _ = std::fs::create_dir_all(dir);
            crate::files::reveal(dir);
        }
    }

    /// Reads the packs again, keeping the chosen one.
    fn reload_packs(&mut self) {
        let id = self.pack().id.clone();
        self.packs = load_packs(self.dir.as_deref());
        self.pack = self.packs.iter().position(|p| p.id == id).unwrap_or(0);
        self.refresh();
    }

    /// Lists pack `index` in Choose Sample.
    fn browse(&mut self, index: usize) {
        let index = index.min(self.packs.len() - 1);
        self.choose_pack = index;
        let pack = &self.packs[index];
        self.choose_rows = (0..pack.samples.len())
            .filter(|&i| pack.sample(i).is_some())
            .collect();
        let rows: Vec<SampleChoice> = self
            .choose_rows
            .iter()
            .filter_map(|&i| pack.sample(i))
            .map(|s| SampleChoice {
                name: s.name.clone().into(),
                length: sample_seconds(&s.source)
                    .map(length_text)
                    .unwrap_or_default()
                    .into(),
            })
            .collect();
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<SamplerUi>();
            g.set_choose_pack(index as i32);
            g.set_choices(ModelRc::new(VecModel::from(rows)));
        }
    }

    fn chosen(&self, row: usize) -> Option<Sample> {
        let i = *self.choose_rows.get(row)?;
        self.packs.get(self.choose_pack)?.sample(i).cloned()
    }

    fn choose(&mut self, slot: usize, row: usize) {
        if let Some(s) = self.chosen(row) {
            self.load(slot, Some(s), true);
        }
    }

    fn preview(&self, row: usize) {
        if let Some(s) = self.chosen(row) {
            self.engine.preview_sample(s.source);
        }
    }

    fn pick_file(&self, slot: usize) {
        let tx = self.picked_tx.clone();
        crate::prefs::choose_file("Choose a sample", move |path| {
            let _ = tx.send((slot, path));
        });
    }

    fn rename(&mut self, slot: usize, name: &str) {
        let name = name.trim();
        if let Some(p) = self.pads.get_mut(slot).and_then(Option::as_mut) {
            if !name.is_empty() && p.name != name {
                p.name = name.to_owned();
                self.save();
                self.refresh();
            }
        }
    }

    fn reset(&mut self, slot: usize) {
        let base = self.pack().sample(slot).cloned();
        self.load(slot, base, true);
    }

    fn dropped(&mut self, slot: usize, data: &DataTransfer) {
        if let Some(from) = dragged_pad(data) {
            self.swap(from, slot);
            return;
        }
        if let Some(pack) = dropped_pack(data) {
            self.import(&pack);
            return;
        }
        let found = self.browser.borrow().dropped_sample(data);
        if let Some((path, name)) = found {
            self.put(slot, Some(Sample::new(name, SampleSource::File(path))));
        }
    }

    /// Swaps two pads with their names, levels and modes.
    fn swap(&mut self, a: usize, b: usize) {
        if a == b || a >= NUM_SAMPLERS || b >= NUM_SAMPLERS {
            return;
        }
        let (pa, pb) = (self.pads[a].clone(), self.pads[b].clone());
        self.load(a, pb, true);
        self.load(b, pa, true);
    }

    /// Shows the packs and pads.
    fn refresh(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<SamplerUi>();
        let packs: Vec<SamplePack> = self
            .packs
            .iter()
            .map(|p| {
                let cover = p
                    .cover
                    .as_deref()
                    .and_then(|c| Image::load_from_path(c).ok());
                SamplePack {
                    name: p.name.clone().into(),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                }
            })
            .collect();
        g.set_packs(ModelRc::new(VecModel::from(packs)));
        g.set_pack(self.pack as i32);
        let pads: Vec<SamplerPad> = self
            .pads
            .iter()
            .map(|p| SamplerPad {
                name: p
                    .as_ref()
                    .map(|s| SharedString::from(s.name.as_str()))
                    .unwrap_or_default(),
                empty: p.is_none(),
                missing: p.as_ref().is_some_and(Sample::missing),
            })
            .collect();
        g.set_pads(ModelRc::new(VecModel::from(pads)));
    }
}

/// The slot of a pad dragged in edit mode.
fn dragged_pad(data: &DataTransfer) -> Option<usize> {
    parse_pad(&data.plain_text().ok()?)
}

/// An `.djsugarpack` dropped from the file manager.
fn dropped_pack(data: &DataTransfer) -> Option<PathBuf> {
    data.file_paths()
        .ok()?
        .find(|p| is_pack_file(p))
        .map(Path::to_path_buf)
}

fn is_pack_file(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(PACK_EXTENSION))
}

fn parse_pad(text: &str) -> Option<usize> {
    text.strip_prefix(PAD_PAYLOAD)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_payload_names_the_slot() {
        assert_eq!(parse_pad("djsugar-sampler-pad:12"), Some(12));
        assert_eq!(parse_pad("djsugar-sampler-pad:x"), None);
        assert_eq!(parse_pad("djsugar-track:3"), None);
    }

    fn kick() -> Sample {
        Sample::new("Kick".into(), SampleSource::Builtin(0))
    }

    #[test]
    fn default_pack_has_the_built_in_sounds() {
        let packs = load_packs(None);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "Default DJ Sample Pack");
        assert_eq!(packs[0].sample(4).unwrap().name, "Horn");
        assert!(packs[0].sample(8).is_none());
    }

    #[test]
    fn edits_round_trip() {
        let base = kick();
        let mut mine = Sample::new(
            "Pull Up".into(),
            SampleSource::File("/x/pull-up.wav".into()),
        );
        mine.level = 0.5;
        mine.mode = 1;
        let e = edit_json(Some(&mine), Some(&base));
        assert_eq!(e["file"], "/x/pull-up.wav");
        assert_eq!(resolve(Some(&base), Some(&e)), Some(mine.clone()));
        // Only the level changed: only the level is saved.
        let quieter = Sample {
            level: 0.25,
            ..base.clone()
        };
        let e = edit_json(Some(&quieter), Some(&base));
        assert_eq!(e, json!({ "level": 0.25 }));
        assert_eq!(resolve(Some(&base), Some(&e)), Some(quieter));
        // Cleared, and a sample on a slot the pack leaves empty.
        let e = edit_json(None, Some(&base));
        assert_eq!(resolve(Some(&base), Some(&e)), None);
        let e = edit_json(Some(&mine), None);
        assert_eq!(resolve(None, Some(&e)), Some(mine));
    }

    #[test]
    fn packs_are_read_and_made() {
        let dir = std::env::temp_dir().join(format!("djsugar-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = create_pack(&dir, "Dancehall Drops!").unwrap();
        assert_eq!(id, "dancehall-drops");
        assert_eq!(
            create_pack(&dir, "Dancehall Drops").unwrap(),
            "dancehall-drops-2"
        );
        std::fs::write(
            dir.join(&id).join(PACK_FILE),
            r#"{ "name": "Dancehall Drops", "samples": [
                { "file": "pull-up.wav", "name": "Pull Up", "level": 0.5, "mode": 3 },
                null,
                { "file": "horn.wav" } ] }"#,
        )
        .unwrap();
        let packs = load_packs(Some(&dir));
        let p = packs.iter().find(|p| p.id == id).unwrap();
        assert_eq!(p.name, "Dancehall Drops");
        let s = p.sample(0).unwrap();
        assert_eq!((s.name.as_str(), s.level, s.mode), ("Pull Up", 0.5, 3));
        assert!(s.missing());
        assert!(p.sample(1).is_none());
        assert_eq!(p.sample(2).unwrap().name, "horn");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn packs_export_import_rename_and_delete() {
        let root = std::env::temp_dir().join(format!("djsugar-pack-share-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (src, dir) = (root.join("src"), root.join("packs"));
        std::fs::create_dir_all(src.join("b")).unwrap();
        std::fs::write(src.join("drop.wav"), b"RIFF one").unwrap();
        // Same file name from another folder: both go in, renamed.
        std::fs::write(src.join("b").join("drop.wav"), b"RIFF two").unwrap();
        let file = |p: PathBuf, name: &str| {
            let mut s = Sample::new(name.into(), SampleSource::File(p));
            s.level = 0.5;
            s.mode = 3;
            Some(s)
        };
        let mut pads = vec![None; NUM_SAMPLERS];
        pads[0] = Some(kick());
        pads[1] = file(src.join("drop.wav"), "Pull Up");
        pads[3] = file(src.join("b").join("drop.wav"), "Big Tune");
        pads[5] = file(src.join("gone.wav"), "Gone");
        let to = root.join("My Drops.djsugarpack");
        assert!(is_pack_file(&to));
        assert_eq!(export_pack("My Drops", None, &pads, &to).unwrap(), 3);
        let id = import_pack(&to, &dir).unwrap();
        assert_eq!(id, "my-drops");
        let packs = load_packs(Some(&dir));
        let p = packs.iter().find(|p| p.id == id).unwrap();
        assert_eq!(p.name, "My Drops");
        assert_eq!(p.sample(0), Some(&kick()));
        let pull = p.sample(1).unwrap();
        assert_eq!(
            (pull.name.as_str(), pull.level, pull.mode),
            ("Pull Up", 0.5, 3)
        );
        let SampleSource::File(f) = &pull.source else {
            panic!()
        };
        assert_eq!(std::fs::read(f).unwrap(), b"RIFF one");
        let SampleSource::File(f) = &p.sample(3).unwrap().source else {
            panic!()
        };
        assert_eq!(std::fs::read(f).unwrap(), b"RIFF two");
        // The missing file is left out.
        assert!(p.sample(5).is_none());
        // Importing again makes a second pack.
        assert_eq!(import_pack(&to, &dir).unwrap(), "my-drops-2");
        rename_pack(&dir.join(&id), "Dancehall").unwrap();
        let packs = load_packs(Some(&dir));
        assert!(packs.iter().any(|p| p.id == id && p.name == "Dancehall"));
        delete_pack(&dir.join(&id)).unwrap();
        let packs = load_packs(Some(&dir));
        assert!(!packs.iter().any(|p| p.id == id));
        assert!(dir.join(".deleted").is_dir());
        // A zip without pack.json, and one naming files outside the pack.
        let bad = root.join("bad.djsugarpack");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(std::fs::File::create(&bad).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("x.wav", o).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        assert!(import_pack(&bad, &dir).is_err());
        let sneaky = root.join("sneaky.djsugarpack");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(std::fs::File::create(&sneaky).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("pack.json", o).unwrap();
            z.write_all(br#"{"name":"S","samples":[{"file":"../../evil.wav"}]}"#)
                .unwrap();
            z.finish().unwrap();
        }
        let id = import_pack(&sneaky, &dir).unwrap();
        assert!(!root.join("evil.wav").exists());
        let packs = load_packs(Some(&dir));
        let p = packs.iter().find(|p| p.id == id).unwrap();
        // Named outside the pack: dropped.
        assert!(p.sample(0).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lengths() {
        assert_eq!(length_text(0.7), "0:00.7");
        assert_eq!(length_text(2.0), "0:02.0");
        assert_eq!(length_text(12.4), "0:12");
        assert_eq!(length_text(61.0), "1:01");
    }
}
