//! Instrument presets: a sound kept under a name, to find and play again.
//! One JSON file per preset in `Instruments/Presets/<instrument>/` under
//! the config folder. A plugin's preset holds its whole state (from the
//! plugin host); a Groove Box's holds its kit, patterns, swing and tracks'
//! sound settings (its controls).
//!
//! On a deck, ‹ › step through the playing instrument's presets in name
//! order and SAVE keeps, saves as new or renames (`PresetUi`); the
//! Instruments browser lists them (see `crate::instruments`).
//!
//! A plugin's own presets are listed too, after the user's: VST3
//! `.vstpreset` and AU `.aupreset` files from the folders those formats
//! keep them in, what a CLAP plugin's preset discovery lists, and an Audio
//! Unit's factory presets (those two asked in a throwaway helper in the
//! background, and kept in `Instruments/Plugin presets.json` to show at
//! once next time). They are read only: SAVE on one keeps a copy as the
//! user's.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use base64::Engine as _;
use djsugar_control::ControlRegistry;
use djsugar_engine::groove::groove_group;
use djsugar_engine::plugins::{own_presets, plugin_presets, Plugin};
use djsugar_engine::AudioEngine;
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{groove_box, settings, AppWindow, PresetItem, PresetUi};

/// The instrument key of the Groove Box's presets.
pub const GROOVE: &str = "groove";

/// The Groove Box's controls a preset leaves alone: whether it plays, its
/// tempo and ducking belong to the mix, not the sound.
const GROOVE_SKIPPED: [&str; 6] = [
    "enabled",
    "bpm",
    "follow",
    "tempo_scale",
    "duck_deck",
    "duck_amount",
];

/// Tags a preset can have: what kind of sound it is, then what it sounds
/// like. The browser's chips narrow the list to presets with them.
pub const TYPES: [&str; 10] = [
    "Bass", "Lead", "Pad", "Keys", "Pluck", "Brass", "Strings", "Arp", "Drums", "FX",
];
pub const CHARACTERS: [&str; 10] = [
    "Warm", "Bright", "Dark", "Soft", "Hard", "Clean", "Dirty", "Evolving", "Vintage", "Wide",
];

/// Every tag, types first, as the screens number them.
pub fn tags() -> impl Iterator<Item = &'static str> {
    TYPES.into_iter().chain(CHARACTERS)
}

/// A preset as listed: what the browser and ‹ › need, without its sound.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    /// [`GROOVE`] or the plugin's key.
    pub instrument: String,
    /// The instrument's name when saved ("Jun-6 V", "Groove Box").
    pub instrument_name: String,
    pub path: PathBuf,
    /// Its tags, from [`TYPES`] and [`CHARACTERS`] (others kept as written).
    pub tags: Vec<String>,
    /// One of the plugin's own, not one of the user's: a `.vstpreset` or
    /// `.aupreset` file at `path`, or one a CLAP plugin listed.
    pub from_plugin: bool,
    /// A preset the plugin listed itself (CLAP, AU factory), as it gave it
    /// (`path` is then this too, to tell presets apart); empty for a file.
    pub reference: String,
}

/// An instrument plugin the Instrument input lists.
#[derive(Clone, Debug, Default)]
pub struct Instrument {
    pub key: String,
    /// As its picker shows it.
    pub name: String,
    /// As the plugin names itself and its maker: its presets' folders.
    pub plugin_name: String,
    pub maker: String,
}

fn root() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("Instruments").join("Presets"))
}

/// A name made safe for a file or folder name.
fn file_safe(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if r#"/\:*?"<>|"#.contains(c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim_matches(['.', ' ']).to_string();
    if s.is_empty() {
        "Preset".into()
    } else {
        s
    }
}

/// Every preset under `root`, by instrument then name.
fn list_in(root: &Path) -> Vec<Preset> {
    let mut out = Vec::new();
    let Ok(folders) = std::fs::read_dir(root) else {
        return out;
    };
    for folder in folders.flatten() {
        let Ok(files) = std::fs::read_dir(folder.path()) else {
            continue;
        };
        for f in files.flatten() {
            let path = f.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Some(v) = std::fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            else {
                continue;
            };
            let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
            if text("instrument").is_empty() {
                continue;
            }
            out.push(Preset {
                name: text("name"),
                instrument: text("instrument"),
                instrument_name: text("instrument_name"),
                path,
                tags: v["tags"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                from_plugin: false,
                reference: String::new(),
            });
        }
    }
    sort(&mut out);
    out
}

/// The plugin's own presets of each of `plugins`: files where its format
/// keeps them, or what it listed itself (`listed`, by key).
fn plugins_own(
    plugins: &[Instrument],
    listed: &HashMap<String, Vec<(String, String)>>,
) -> Vec<Preset> {
    let mut out = Vec::new();
    for p in plugins {
        for (name, reference) in listed.get(&p.key).into_iter().flatten() {
            out.push(Preset {
                name: name.clone(),
                instrument: p.key.clone(),
                instrument_name: p.name.clone(),
                path: PathBuf::from(reference),
                tags: Vec::new(),
                from_plugin: true,
                reference: reference.clone(),
            });
        }
        let Some((format, class)) = format_of(&p.key) else {
            continue;
        };
        for f in plugin_presets::find(format, &p.maker, &p.plugin_name, class) {
            out.push(Preset {
                name: f.name,
                instrument: p.key.clone(),
                instrument_name: p.name.clone(),
                path: f.path,
                tags: Vec::new(),
                from_plugin: true,
                reference: String::new(),
            });
        }
    }
    out
}

/// The plugin a key names, for the formats that list their own presets:
/// a CLAP plugin (its file and id) or an Audio Unit.
fn listing_plugin(key: &str) -> Option<Plugin> {
    if let Some(id) = key.strip_prefix("au:") {
        return Some(Plugin::Au { id: id.to_string() });
    }
    let (file, id) = key.strip_prefix("clap:")?.rsplit_once('#')?;
    Some(Plugin::Clap {
        file: PathBuf::from(file),
        id: id.to_string(),
    })
}

/// Where the plugins' own lists are kept between runs.
fn listed_file() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("Instruments").join("Plugin presets.json"))
}

fn read_listed() -> HashMap<String, Vec<(String, String)>> {
    listed_file()
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_listed(listed: &HashMap<String, Vec<(String, String)>>) {
    let Some(file) = listed_file() else {
        return;
    };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_vec_pretty(listed) {
        let _ = std::fs::write(file, text);
    }
}

/// How long one CLAP plugin may take to list its presets.
const LIST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Asks each CLAP plugin and Audio Unit of `plugins` not asked yet this
/// run for its own presets, one after another in the background; each answer is kept and
/// shown as it comes.
fn ask_plugins(app: &AppWindow, plugins: &[Instrument]) {
    let Some(helper) = djsugar_engine::plugin_helper() else {
        return;
    };
    let todo: Vec<(String, Plugin)> = STATE.with_borrow_mut(|s| {
        plugins
            .iter()
            .filter(|p| s.asked.insert(p.key.clone()))
            .filter_map(|p| listing_plugin(&p.key).map(|plugin| (p.key.clone(), plugin)))
            .collect()
    });
    if todo.is_empty() {
        return;
    }
    let weak = app.as_weak();
    let _ = std::thread::Builder::new()
        .name("plugin presets".into())
        .spawn(move || {
            for (key, plugin) in todo {
                let found = match own_presets(&helper, &plugin, LIST_TIMEOUT) {
                    Ok(found) => found,
                    Err(e) => {
                        eprintln!("presets of {key}: {e}");
                        continue;
                    }
                };
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let changed = STATE.with_borrow_mut(|s| {
                        let changed = s.listed.get(&key) != Some(&found);
                        if found.is_empty() {
                            s.listed.remove(&key);
                        } else {
                            s.listed.insert(key, found);
                        }
                        if changed {
                            write_listed(&s.listed);
                        }
                        changed
                    });
                    if let (true, Some(app)) = (changed, weak.upgrade()) {
                        rescan();
                        crate::instruments::presets_changed(&app);
                    }
                });
            }
        });
}

/// A plugin key's format ("vst3" or "au"), and its VST3 class id, for the
/// formats whose presets are files.
fn format_of(key: &str) -> Option<(&'static str, &str)> {
    if let Some(rest) = key.strip_prefix("vst3:") {
        Some(("vst3", rest.rsplit_once('#').map_or("", |(_, id)| id)))
    } else if key.starts_with("au:") {
        Some(("au", ""))
    } else {
        None
    }
}

/// The sound a plugin's own preset file holds, as the plugin host takes it.
fn file_state(preset: &Preset) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(&preset.path).map_err(|e| format!("can't read it ({e})"))?;
    match format_of(&preset.instrument) {
        Some(("vst3", _)) => plugin_presets::vst3_state(&bytes),
        // An `.aupreset` is the unit's class info, as the AU helper keeps it.
        Some(("au", _)) => Ok(bytes),
        _ => Err("its format has no preset files".into()),
    }
}

/// By instrument, the user's before the plugin's own, then by name.
fn sort(presets: &mut [Preset]) {
    presets.sort_by_cached_key(|p| {
        (
            p.instrument_name.to_lowercase(),
            p.from_plugin,
            p.name.to_lowercase(),
        )
    });
}

/// Writes a new preset file under `root` and returns its path; a name
/// already taken for the instrument gets " 2", " 3"...
fn write_new(root: &Path, mut body: Map<String, Value>, name: &str) -> Result<PathBuf, String> {
    let instrument_name = body["instrument_name"].as_str().unwrap_or("Instrument");
    let folder = root.join(file_safe(instrument_name));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let base = name.trim();
    let base = if base.is_empty() { "Preset" } else { base };
    let mut n = 1;
    let (name, path) = loop {
        let name = if n == 1 {
            base.to_string()
        } else {
            format!("{base} {n}")
        };
        let path = folder.join(format!("{}.json", file_safe(&name)));
        if !path.exists() {
            break (name, path);
        }
        n += 1;
    };
    body.insert("name".into(), json!(name));
    write(&path, &body)?;
    Ok(path)
}

/// Writes `body` to `path` through a temporary file.
fn write(path: &Path, body: &Map<String, Value>) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    let text = serde_json::to_vec_pretty(body).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn read(path: &Path) -> Result<Map<String, Value>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match serde_json::from_slice(&bytes) {
        Ok(Value::Object(m)) => Ok(m),
        _ => Err("the preset file is damaged".into()),
    }
}

/// Renames the preset at `path`; returns its new path.
/// Gives the preset at `path` tag `tag`, or takes it away if it has it.
fn toggle_tag(path: &Path, tag: &str) -> Result<(), String> {
    let mut body = read(path)?;
    let mut tags: Vec<Value> = body
        .get("tags")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    match tags.iter().position(|t| t.as_str() == Some(tag)) {
        Some(i) => {
            tags.remove(i);
        }
        None => tags.push(json!(tag)),
    }
    body.insert("tags".into(), Value::Array(tags));
    write(path, &body)
}

fn rename_file(path: &Path, name: &str) -> Result<PathBuf, String> {
    let mut body = read(path)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("a preset needs a name".into());
    }
    let folder = path.parent().ok_or("no folder")?;
    let new = folder.join(format!("{}.json", file_safe(name)));
    if new != path && new.exists() {
        return Err(format!("there is already a preset called {name}"));
    }
    body.insert("name".into(), json!(name));
    write(&new, &body)?;
    if new != path {
        let _ = std::fs::remove_file(path);
    }
    Ok(new)
}

/// A preset's file body, before its name.
fn body(instrument: &str, instrument_name: &str) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("format".into(), json!(1));
    m.insert("instrument".into(), json!(instrument));
    m.insert("instrument_name".into(), json!(instrument_name));
    m.insert("tags".into(), json!([]));
    m
}

/// The sound a plugin preset's body holds.
fn plugin_state(body: &Map<String, Value>) -> Result<Vec<u8>, String> {
    let text = body["state"].as_str().ok_or("the preset holds no sound")?;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| "the preset's sound is damaged".to_string())
}

/// A plugin preset's knob picks, (knob, parameter id); none in a preset
/// saved before knobs could be picked.
fn knob_picks(body: &Map<String, Value>) -> Option<Vec<(usize, u32)>> {
    let list = body.get("knobs")?.as_array()?;
    Some(
        list.iter()
            .filter_map(|p| {
                let knob = p.get(0)?.as_u64()?;
                let id = p.get(1)?.as_u64()?;
                Some((usize::try_from(knob).ok()?, u32::try_from(id).ok()?))
            })
            .collect(),
    )
}

/// What a deck plays that can have presets.
#[derive(Clone, Debug, PartialEq)]
enum Playing {
    Groove,
    /// The instrument plugin's place in the Instrument input's list.
    Plugin(usize),
    Nothing,
}

#[derive(Default)]
struct State {
    presets: Vec<Preset>,
    /// The instrument plugins the Instrument input lists.
    plugins: Vec<Instrument>,
    /// The presets plugins listed themselves, by key.
    listed: HashMap<String, Vec<(String, String)>>,
    /// The plugins asked for their list this run.
    asked: HashSet<String>,
    /// Each deck's Groove Box preset.
    groove: Vec<Option<PathBuf>>,
    /// The Instrument input's preset.
    instrument: Option<PathBuf>,
    /// The names shown, by deck, to skip setting them when unchanged, and
    /// their tags (see [`show`]).
    shown: Vec<String>,
    shown_tags: Vec<bool>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The presets found, as last listed.
pub fn all() -> Vec<Preset> {
    STATE.with_borrow(|s| s.presets.clone())
}

/// Lists the presets again from the folder.
fn rescan() {
    let mut presets = root().map(|r| list_in(&r)).unwrap_or_default();
    presets.extend(STATE.with_borrow(|s| plugins_own(&s.plugins, &s.listed)));
    sort(&mut presets);
    STATE.with_borrow_mut(|s| s.presets = presets);
}

/// The instrument plugins the Instrument input lists, in its order; their
/// own presets are listed again, and CLAP plugins asked for theirs.
pub fn set_plugins(app: &AppWindow, plugins: Vec<Instrument>) {
    ask_plugins(app, &plugins);
    STATE.with_borrow_mut(|s| s.plugins = plugins);
    rescan();
}

struct Ctx {
    engine: Rc<AudioEngine>,
    registry: std::sync::Arc<ControlRegistry>,
}

impl Ctx {
    fn value(&self, group: &str, key: &str) -> f64 {
        self.registry.get(group, key).map_or(0.0, |c| c.get())
    }

    fn set(&self, group: &str, key: &str, v: f64) {
        if let Some(c) = self.registry.get(group, key) {
            c.set(v);
        }
    }

    fn playing(&self, deck: usize) -> Playing {
        if self.value(&groove_group(deck), "enabled") > 0.0 {
            Playing::Groove
        } else if self.value(&format!("[Channel{}]", deck + 1), "live_input") == 3.0 {
            match self.value("[Instrument1]", "instrument").round() as usize {
                0 => Playing::Nothing,
                i => Playing::Plugin(i - 1),
            }
        } else {
            Playing::Nothing
        }
    }

    /// The instrument key and name of what deck `deck` plays.
    fn instrument(&self, deck: usize) -> Option<(String, String)> {
        match self.playing(deck) {
            Playing::Groove => Some((GROOVE.into(), "Groove Box".into())),
            Playing::Plugin(i) => {
                STATE.with_borrow(|s| s.plugins.get(i).map(|p| (p.key.clone(), p.name.clone())))
            }
            Playing::Nothing => None,
        }
    }

    /// The preset deck `deck` plays, if it still matches what it plays.
    fn current(&self, deck: usize) -> Option<Preset> {
        let (key, _) = self.instrument(deck)?;
        STATE.with_borrow(|s| {
            let path = if key == GROOVE {
                s.groove.get(deck).cloned().flatten()
            } else {
                s.instrument.clone()
            }?;
            s.presets
                .iter()
                .find(|p| p.path == path && p.instrument == key)
                .cloned()
        })
    }

    fn set_current(&self, deck: usize, key: &str, path: Option<PathBuf>) {
        STATE.with_borrow_mut(|s| {
            if key == GROOVE {
                if s.groove.len() <= deck {
                    s.groove.resize(deck + 1, None);
                }
                s.groove[deck] = path;
            } else {
                s.instrument = path;
            }
        });
    }

    /// Plays `preset` on deck `deck`: a Groove Box takes its controls; a
    /// plugin its sound, on the Instrument input, which the deck then plays.
    fn load(&self, deck: usize, preset: &Preset) -> Result<(), String> {
        if preset.from_plugin {
            let index = self.plugin_index(preset)?;
            if preset.reference.is_empty() {
                self.engine
                    .load_instrument_sound(&preset.instrument, file_state(preset)?)?;
            } else {
                self.engine
                    .load_instrument_preset(&preset.instrument, &preset.reference)?;
            }
            self.set("[Instrument1]", "instrument", (index + 1) as f64);
            self.set(&format!("[Channel{}]", deck + 1), "live_input", 3.0);
            self.set_current(deck, &preset.instrument, Some(preset.path.clone()));
            return Ok(());
        }
        let body = read(&preset.path)?;
        if preset.instrument == GROOVE {
            let g = groove_group(deck);
            let mut values = body["controls"]
                .as_object()
                .cloned()
                .ok_or("the preset holds no groove")?;
            groove_box::upgrade(&mut values);
            for key in groove_box::kept_keys() {
                if GROOVE_SKIPPED.contains(&key.as_str()) {
                    continue;
                }
                if let Some(v) = values.get(&key).and_then(Value::as_f64) {
                    self.set(&g, &key, v);
                }
            }
            self.set(&g, "enabled", 1.0);
        } else {
            let index = self.plugin_index(preset)?;
            // Its knob picks first, so a restart to take them starts from
            // the sound below. A preset from before picks leaves them be.
            if let Some(picks) = knob_picks(&body) {
                self.engine
                    .set_instrument_picks(&preset.instrument, picks)?;
            }
            self.engine
                .load_instrument_sound(&preset.instrument, plugin_state(&body)?)?;
            self.set("[Instrument1]", "instrument", (index + 1) as f64);
            self.set(&format!("[Channel{}]", deck + 1), "live_input", 3.0);
        }
        self.set_current(deck, &preset.instrument, Some(preset.path.clone()));
        Ok(())
    }

    /// The place in the Instrument input's list of `preset`'s plugin.
    fn plugin_index(&self, preset: &Preset) -> Result<usize, String> {
        STATE
            .with_borrow(|s| s.plugins.iter().position(|p| p.key == preset.instrument))
            .ok_or_else(|| {
                format!(
                    "{} isn't ticked in Settings › Plugins, so its preset can't play",
                    preset.instrument_name
                )
            })
    }

    /// A new preset of what deck `deck` plays now, named `name`.
    fn save_new(&self, deck: usize, name: &str) -> Result<PathBuf, String> {
        let (key, instrument_name) = self.instrument(deck).ok_or("nothing to save")?;
        let mut body = body(&key, &instrument_name);
        self.fill(deck, &key, &mut body)?;
        let root = root().ok_or("no folder for presets")?;
        let path = write_new(&root, body, name)?;
        self.set_current(deck, &key, Some(path.clone()));
        Ok(path)
    }

    /// Saves what deck `deck` plays over its preset.
    /// The plugin's own presets stay as they are: a copy is kept as the
    /// user's, under the same name.
    fn save_over(&self, deck: usize) -> Result<(), String> {
        let preset = self.current(deck).ok_or("no preset to save over")?;
        if preset.from_plugin {
            return self.save_new(deck, &preset.name).map(|_| ());
        }
        let mut body = read(&preset.path)?;
        self.fill(deck, &preset.instrument, &mut body)?;
        write(&preset.path, &body)
    }

    /// Puts what deck `deck` plays now into a preset's body.
    fn fill(&self, deck: usize, key: &str, body: &mut Map<String, Value>) -> Result<(), String> {
        if key == GROOVE {
            let g = groove_group(deck);
            let mut controls = Map::new();
            for k in groove_box::kept_keys() {
                if !GROOVE_SKIPPED.contains(&k.as_str()) {
                    controls.insert(k.clone(), json!(self.value(&g, &k)));
                }
            }
            controls.insert("layout".into(), json!(groove_box::LAYOUT));
            body.insert("controls".into(), Value::Object(controls));
        } else {
            let (playing, data) = self.engine.instrument_sound()?;
            if playing != key {
                return Err("the instrument changed while saving".into());
            }
            body.insert(
                "state".into(),
                json!(base64::engine::general_purpose::STANDARD.encode(data)),
            );
            body.insert("knobs".into(), json!(self.engine.instrument_picks()));
        }
        Ok(())
    }

    /// Steps deck `deck` to the next (`by` 1) or previous (-1) preset of
    /// what it plays, in name order, round the ends.
    fn step(&self, deck: usize, by: i32) -> Result<(), String> {
        let (key, name) = self.instrument(deck).ok_or("nothing to step through")?;
        let mine: Vec<Preset> = all().into_iter().filter(|p| p.instrument == key).collect();
        if mine.is_empty() {
            return Err(format!("{name} has no presets yet: SAVE makes one"));
        }
        let n = mine.len() as i32;
        let next = match self
            .current(deck)
            .and_then(|c| mine.iter().position(|p| *p == c))
        {
            Some(i) => (i as i32 + by).rem_euclid(n),
            None if by > 0 => 0,
            None => n - 1,
        };
        self.load(deck, &mine[next as usize])
    }
}

thread_local! {
    static CTX: RefCell<Option<Rc<Ctx>>> = const { RefCell::new(None) };
}

fn ctx() -> Option<Rc<Ctx>> {
    CTX.with_borrow(|c| c.clone())
}

pub fn install(
    app: &AppWindow,
    engine: Rc<AudioEngine>,
    registry: std::sync::Arc<ControlRegistry>,
) {
    CTX.with_borrow_mut(|c| *c = Some(Rc::new(Ctx { engine, registry })));
    STATE.with_borrow_mut(|s| s.listed = read_listed());
    rescan();
    let ui = app.global::<PresetUi>();
    let names: Vec<SharedString> = tags().map(SharedString::from).collect();
    ui.set_tag_names(ModelRc::new(VecModel::from(names)));
    ui.set_type_count(TYPES.len() as i32);
    let weak = app.as_weak();
    let act = move |what: &dyn Fn(&Ctx) -> Result<(), String>| {
        let Some(ctx) = ctx() else {
            return;
        };
        if let Err(e) = what(&ctx) {
            ctx.engine.alert(format!("Preset: {e}."));
        }
        rescan();
        if let Some(app) = weak.upgrade() {
            show_list(&app);
            show(&app);
            crate::instruments::presets_changed(&app);
        }
    };
    let a = act.clone();
    ui.on_step(move |deck, by| a(&|c| c.step(deck as usize, by)));
    let a = act.clone();
    ui.on_save(move |deck| a(&|c| c.save_over(deck as usize)));
    let a = act.clone();
    ui.on_save_as(move |deck, name| a(&|c| c.save_new(deck as usize, &name).map(|_| ())));
    let a = act.clone();
    ui.on_rename(move |deck, name| {
        a(&|c| {
            let preset = c.current(deck as usize).ok_or("no preset to rename")?;
            if preset.from_plugin {
                return Err(format!(
                    "{} is {}'s own preset: SAVE keeps a copy you can rename",
                    preset.name, preset.instrument_name
                ));
            }
            let path = rename_file(&preset.path, &name)?;
            c.set_current(deck as usize, &preset.instrument, Some(path));
            Ok(())
        })
    });
    let a = act.clone();
    ui.on_toggle_tag(move |deck, tag| {
        a(&|c| {
            let preset = c
                .current(deck as usize)
                .ok_or("save a preset first, then tag it")?;
            if preset.from_plugin {
                return Err(format!(
                    "{} is {}'s own preset: SAVE keeps a copy you can tag",
                    preset.name, preset.instrument_name
                ));
            }
            let tag = tags().nth(tag as usize).ok_or("no such tag")?;
            toggle_tag(&preset.path, tag)
        })
    });
    let a = act;
    ui.on_load(move |deck, index| {
        a(&|c| {
            let preset = all().get(index as usize).cloned().ok_or("it's gone")?;
            c.load(deck as usize, &preset)
        })
    });
    show_list(app);
    show(app);
}

/// Shows the Groove Box's presets in its menu.
fn show_list(app: &AppWindow) {
    let items: Vec<PresetItem> = all()
        .iter()
        .enumerate()
        .filter(|(_, p)| p.instrument == GROOVE)
        .map(|(i, p)| PresetItem {
            name: p.name.as_str().into(),
            index: i as i32,
        })
        .collect();
    app.global::<PresetUi>()
        .set_groove(ModelRc::new(VecModel::from(items)));
}

/// Shows each deck's preset name ("" for none). Cheap when nothing
/// changed: called often, as decks change what they play.
pub fn show(app: &AppWindow) {
    let Some(ctx) = ctx() else {
        return;
    };
    let current: Vec<Option<Preset>> = (0..djsugar_engine::NUM_DECKS)
        .map(|d| ctx.current(d))
        .collect();
    let names: Vec<String> = current
        .iter()
        .map(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default())
        .collect();
    // Each deck's preset's tags, a flag per tag, deck after deck.
    let flags: Vec<bool> = current
        .iter()
        .flat_map(|p| {
            tags().map(move |t| p.as_ref().is_some_and(|p| p.tags.iter().any(|x| x == t)))
        })
        .collect();
    let changed = STATE.with_borrow_mut(|s| {
        let changed = s.shown != names || s.shown_tags != flags;
        s.shown = names.clone();
        s.shown_tags = flags.clone();
        changed
    });
    if changed {
        let names: Vec<SharedString> = names.iter().map(|n| n.as_str().into()).collect();
        let ui = app.global::<PresetUi>();
        ui.set_current(ModelRc::new(VecModel::from(names)));
        ui.set_deck_tags(ModelRc::new(VecModel::from(flags)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("djsugar-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_presets_knob_picks_read_back() {
        let body = |v: Value| v.as_object().unwrap().clone();
        assert_eq!(
            knob_picks(&body(json!({"knobs": [[0, 42], [3, 7]]}))),
            Some(vec![(0, 42), (3, 7)])
        );
        assert_eq!(knob_picks(&body(json!({"knobs": []}))), Some(vec![]));
        assert_eq!(knob_picks(&body(json!({"state": ""}))), None);
    }

    #[test]
    fn presets_are_written_listed_and_renamed() {
        let root = temp("presets");
        let mut b = body("clap:/x/Jun.clap:jun6", "Jun-6 V");
        b.insert(
            "state".into(),
            json!(base64::engine::general_purpose::STANDARD.encode(b"abc")),
        );
        let first = write_new(&root, b.clone(), "Ambient Ebow").unwrap();
        // The same name again gets a number.
        let second = write_new(&root, b, "Ambient Ebow").unwrap();
        assert_ne!(first, second);
        write_new(&root, body(GROOVE, "Groove Box"), "house: 4/4").unwrap();
        let names: Vec<(String, String)> = list_in(&root)
            .into_iter()
            .map(|p| (p.instrument_name, p.name))
            .collect();
        assert_eq!(
            names,
            [
                ("Groove Box".into(), "house: 4/4".into()),
                ("Jun-6 V".into(), "Ambient Ebow".into()),
                ("Jun-6 V".into(), "Ambient Ebow 2".into()),
            ]
        );
        assert_eq!(plugin_state(&read(&first).unwrap()).unwrap(), b"abc");
        // Renaming keeps the sound; a taken name is refused.
        assert!(rename_file(&second, "Ambient Ebow").is_err());
        let renamed = rename_file(&second, "Dark Strings").unwrap();
        assert!(!second.exists());
        assert_eq!(read(&renamed).unwrap()["name"], json!("Dark Strings"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_plugins_own_presets_follow_the_users_and_read_by_format() {
        let preset = |name: &str, from_plugin: bool| Preset {
            name: name.into(),
            instrument: "vst3:/x/Jun.vst3#ABC".into(),
            instrument_name: "Jun-6 V".into(),
            path: PathBuf::new(),
            tags: Vec::new(),
            from_plugin,
            reference: String::new(),
        };
        let mut list = vec![
            preset("Airy", true),
            preset("Zed", false),
            preset("Bass", true),
        ];
        sort(&mut list);
        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Zed", "Airy", "Bass"]);
        assert_eq!(format_of("vst3:/x/Jun.vst3#ABC"), Some(("vst3", "ABC")));
        assert_eq!(format_of("au:aumu:jun6:Artu"), Some(("au", "")));
        assert_eq!(format_of("clap:/x/Jun.clap#jun6"), None);
        assert_eq!(
            listing_plugin("clap:/x/Jun 6.clap#com.x.jun6"),
            Some(Plugin::Clap {
                file: PathBuf::from("/x/Jun 6.clap"),
                id: "com.x.jun6".into()
            })
        );
        assert_eq!(
            listing_plugin("au:aumu:jun6:Artu"),
            Some(Plugin::Au {
                id: "aumu:jun6:Artu".into()
            })
        );
        assert_eq!(listing_plugin("vst3:/x/Jun.vst3#ABC"), None);
        // An AU preset file is the sound as it is.
        let root = temp("aupreset");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("Warm.aupreset");
        std::fs::write(&path, b"<plist/>").unwrap();
        let au = Preset {
            instrument: "au:aumu:jun6:Artu".into(),
            path,
            ..preset("Warm", true)
        };
        assert_eq!(file_state(&au).unwrap(), b"<plist/>");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tags_are_given_and_taken_away() {
        let root = temp("preset-tags");
        let path = write_new(&root, body(GROOVE, "Groove Box"), "Tight").unwrap();
        assert!(list_in(&root)[0].tags.is_empty());
        toggle_tag(&path, "Drums").unwrap();
        toggle_tag(&path, "Dirty").unwrap();
        assert_eq!(list_in(&root)[0].tags, ["Drums", "Dirty"]);
        toggle_tag(&path, "Drums").unwrap();
        assert_eq!(list_in(&root)[0].tags, ["Dirty"]);
        assert_eq!(tags().count(), TYPES.len() + CHARACTERS.len());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_are_made_safe_for_files() {
        assert_eq!(file_safe("house: 4/4"), "house- 4-4");
        assert_eq!(file_safe("  ..  "), "Preset");
        assert_eq!(file_safe("Jun-6 V"), "Jun-6 V");
    }
}
