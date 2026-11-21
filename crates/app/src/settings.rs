//! Preferences kept between runs, in `settings.json` in the config folder.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::browser::columns::{Hidden, Layout};
use djsugar_controllers::routing::DeckChoice;
use djsugar_controllers::MappingChoice;
use djsugar_engine::key::{Key, Notation};
use djsugar_engine::scan::default_music_dir;
use djsugar_engine::{
    AudioSetup, InputRole, InputRoute, RecordFormat, RecordSettings, Role, Route,
};
use djsugar_library::query::FieldNames;
use djsugar_plugins::inbox::Mode;
use djsugar_plugins::PluginConfig;
use serde_json::{json, Value};

/// Where DJ Sugar keeps its settings: `DJSUGAR_CONFIG`, else
/// `~/Library/Application Support/DJSugar` on macOS, `%APPDATA%\DJ Sugar` on
/// Windows and `$XDG_CONFIG_HOME/djsugar` (or `~/.config/djsugar`) elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DJSUGAR_CONFIG") {
        return Some(PathBuf::from(d));
    }
    let base = djsugar_engine::paths::config_base()?;
    let name = djsugar_engine::paths::app_folder_name();
    Some(base.join(name))
}

fn path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings.json"))
}

pub fn load() -> Value {
    path().map_or_else(|| json!({}), |p| load_from(&p))
}

/// The settings saved in the file at `path`; empty if there are none.
pub fn load_from(path: &std::path::Path) -> Value {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}))
}

/// Whether keys are shown in Camelot notation (the default) or as notes.
pub fn camelot_keys() -> bool {
    load()["camelot_keys"].as_bool().unwrap_or(true)
}

pub fn set_camelot_keys(on: bool) {
    save(|v| v["camelot_keys"] = json!(on));
}

/// Which wheel and which note spelling keys are written with: Camelot,
/// Open Key, sharps (the default for notes) or flats.
pub fn key_notation() -> Notation {
    let default = if camelot_keys() { 0 } else { 2 };
    Notation::from_id(load()["key_notation"].as_u64().unwrap_or(default) as u8)
}

pub fn set_key_notation(n: Notation) {
    save(|v| v["key_notation"] = json!(n.id()));
}

/// The rail's sources after Collection, by name, in the DJ's order;
/// `None` until one is dragged.
pub fn rail_order() -> Option<Vec<String>> {
    load()["rail_order"].as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    })
}

pub fn set_rail_order(names: &[String]) {
    save(|v| v["rail_order"] = json!(names));
}

/// The side panel's tabs in the DJ's order, by name, with whether each
/// shows; `None` until a choice is saved.
pub fn side_tabs() -> Option<Vec<(String, bool)>> {
    load()["side_tabs"].as_array().map(|a| {
        a.iter()
            .filter_map(|t| Some((t["tab"].as_str()?.to_string(), t["on"].as_bool()?)))
            .collect()
    })
}

pub fn set_side_tabs(tabs: &[(&str, bool)]) {
    let tabs: Vec<Value> = tabs
        .iter()
        .map(|(name, on)| json!({ "tab": name, "on": on }))
        .collect();
    save(|v| v["side_tabs"] = json!(tabs));
}

/// The Automix mix length and style picked last time, as indices into
/// [`crate::automix::LENGTHS`] and [`crate::automix::STYLES`]. The length
/// is saved under a new name since 0 s went in first; an older
/// `automix_length` is moved up one to match. Without either, Phrases.
pub fn automix_length() -> usize {
    let v = load();
    match v["automix_mix_length"].as_u64() {
        Some(i) => i as usize,
        // New installs mix on phrases.
        None => v["automix_length"]
            .as_u64()
            .map_or(crate::automix::PHRASES, |i| i as usize + 1),
    }
}

pub fn set_automix_length(i: usize) {
    save(|v| v["automix_mix_length"] = json!(i));
}

pub fn automix_style() -> usize {
    load()["automix_style"].as_u64().unwrap_or(0) as usize
}

pub fn set_automix_style(i: usize) {
    save(|v| v["automix_style"] = json!(i));
}

/// Automix puts each track back at the end of its list once played.
pub fn automix_repeat() -> bool {
    load()["automix_repeat"].as_bool().unwrap_or(false)
}

pub fn set_automix_repeat(on: bool) {
    save(|v| v["automix_repeat"] = json!(on));
}

/// Where mixes are recorded, and in what format: in the mixed sets folder
/// once there is one, else the recordings folder.
pub fn recording() -> RecordSettings {
    let v = load();
    let d = RecordSettings::default();
    RecordSettings {
        dir: mixed_sets_folder()
            .map(|m| m.join(RECORDINGS_SUBFOLDER))
            .or_else(|| {
                v["recording_folder"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
            })
            .unwrap_or(d.dir),
        format: v["recording_format"]
            .as_str()
            .and_then(RecordFormat::from_name)
            .unwrap_or(d.format),
        bits: match v["recording_bits"].as_u64() {
            Some(16) => 16,
            _ => d.bits,
        },
    }
}

/// The output routes chosen last time, with the sample rate and buffer
/// size. Settings from before routes are read too: the main device, and a
/// headphone device or automatic 3/4.
pub fn audio_setup() -> AudioSetup {
    let v = load();
    let sample_rate = v["sample_rate"].as_u64().map(|r| r as u32);
    let buffer_frames = v["buffer_frames"].as_u64().map(|n| n as u32);
    let inputs = v["inputs"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|r| {
                    Some(InputRoute {
                        role: InputRole::from_name(r["role"].as_str()?)?,
                        device: r["device"].as_str().unwrap_or_default().to_string(),
                        channel: r["channel"].as_u64().unwrap_or(0) as u16,
                        mono: r["mono"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(list) = v["outputs"].as_array() {
        return AudioSetup {
            inputs,
            sample_rate,
            buffer_frames,
            external_mixer: v["external_mixer"].as_bool().unwrap_or(false),
            routes: list
                .iter()
                .filter_map(|r| {
                    Some(Route {
                        role: Role::from_name(r["role"].as_str()?)?,
                        device: r["device"].as_str().unwrap_or_default().to_string(),
                        pair: r["pair"].as_u64().unwrap_or(0) as u16,
                    })
                })
                .collect(),
        };
    }
    let main = v["main_device"].as_str().unwrap_or_default().to_string();
    let mut routes = vec![Route {
        role: Role::Master,
        device: main.clone(),
        pair: 0,
    }];
    match v["headphones_device"].as_str() {
        Some("") => {}
        Some(d) if d != main => routes.push(Route {
            role: Role::Headphones,
            device: d.to_string(),
            pair: 0,
        }),
        _ => routes.push(Route {
            role: Role::Headphones,
            device: main,
            pair: 1,
        }),
    }
    AudioSetup {
        routes,
        inputs,
        sample_rate,
        buffer_frames,
        external_mixer: v["external_mixer"].as_bool().unwrap_or(false),
    }
}

pub fn set_audio_setup(setup: &AudioSetup) {
    save(|v| {
        v["outputs"] = setup
            .routes
            .iter()
            .map(|r| json!({ "role": r.role.name(), "device": r.device, "pair": r.pair }))
            .collect();
        v["inputs"] = setup
            .inputs
            .iter()
            .map(|r| {
                json!({
                    "role": r.role.name(),
                    "device": r.device,
                    "channel": r.channel,
                    "mono": r.mono,
                })
            })
            .collect();
    });
}

/// The sources hidden from the library rail, by name; `None` until the DJ
/// (or DJ Sugar, on first start) has saved a choice.
/// Names saved before Mixed Sets was renamed Recordings, and Instruments
/// Jam, read as the new names.
pub fn hidden_sources() -> Option<Vec<String>> {
    load()["hidden_sources"].as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_str())
            .map(|n| match n {
                "Mixed Sets" => "Recordings".to_string(),
                "Instruments" => "Jam".to_string(),
                n => n.to_string(),
            })
            .collect()
    })
}

pub fn set_hidden_sources(names: Vec<String>) {
    save(|v| v["hidden_sources"] = json!(names));
}

/// A plugin source's saved searches (see [`crate::sources`]), oldest first:
/// what was typed, how it was read and which copies to prefer.
/// A saved search: its text, kind, preference, and when it was added (Unix
/// milliseconds).
/// A saved search: text, kind, prefer, when it was added, and a playlist's
/// songs when the app read them (see `SourceQuery::songs`).
pub type SavedSearch = (String, String, String, u64, Vec<String>);

/// A source's saved searches. Ones saved before searches had a time get
/// their place in the list instead, oldest first as they were kept then.
pub fn source_searches(source: &str) -> Vec<SavedSearch> {
    load()["source_searches"][source]
        .as_array()
        .map(|a| {
            a.iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    let text = s["text"].as_str()?.to_string();
                    let field = |k: &str| s[k].as_str().unwrap_or_default().to_string();
                    let added = s["added"].as_u64().unwrap_or(i as u64);
                    let songs = s["songs"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    Some((text, field("kind"), field("prefer"), added, songs))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_source_searches(source: &str, searches: &[SavedSearch]) {
    let list: Vec<Value> = searches
        .iter()
        .map(|(text, kind, prefer, added, songs)| {
            let mut v = json!({ "text": text, "kind": kind, "prefer": prefer, "added": added });
            if !songs.is_empty() {
                v["songs"] = json!(songs);
            }
            v
        })
        .collect();
    save(|v| {
        if !v["source_searches"].is_object() {
            v["source_searches"] = json!({});
        }
        v["source_searches"][source] = json!(list);
    });
}

/// The Mixer's channel names, by aux input from 0; "" keeps the
/// default ("Ch N").
pub fn mixer_names() -> Vec<String> {
    let v = load();
    let mut names: Vec<String> = v["mixer_names"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|n| n.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    names.resize(8, String::new());
    names
}

/// The Mixer's channel colors, by aux input from 0: a place in the
/// strips' 16-color palette, or -1 for the channel's own by its number.
pub fn mixer_colors() -> Vec<i32> {
    let v = load();
    let mut colors: Vec<i32> = v["mixer_colors"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| c.as_i64().map_or(-1, |c| c as i32))
                .collect()
        })
        .unwrap_or_default();
    colors.resize(8, -1);
    colors
}

/// Colors the Mixer's channel `n` (from 0) with palette color `color`;
/// -1 goes back to its own.
pub fn set_mixer_color(n: usize, color: i32) {
    let mut colors = mixer_colors();
    if let Some(slot) = colors.get_mut(n) {
        *slot = if (0..16).contains(&color) { color } else { -1 };
    }
    set("mixer_colors", json!(colors));
}

/// Names the Mixer's channel `n` (from 0); "" goes back to "Ch N".
pub fn set_mixer_name(n: usize, name: &str) {
    let mut names = mixer_names();
    if let Some(slot) = names.get_mut(n) {
        *slot = name.trim().chars().take(24).collect();
    }
    set("mixer_names", json!(names));
}

/// A saved number, or `default`.
pub fn number(key: &str, default: f64) -> f64 {
    load()[key].as_f64().unwrap_or(default)
}

/// The folders open in the collection's tree (see `Browser::node_key`):
/// everything else starts closed.
pub fn open_folders() -> Vec<String> {
    load()["open_folders"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_open_folders(keys: &[String]) {
    save(|v| v["open_folders"] = json!(keys));
}

/// The DJ's Discogs personal access token, which lets the cover search
/// look on Discogs; empty for none.
pub fn discogs_token() -> String {
    load()["discogs_token"].as_str().unwrap_or("").to_string()
}

pub fn set_discogs_token(token: &str) {
    save(|v| v["discogs_token"] = json!(token.trim()));
}

/// The DJ's AcoustID application key, which lets the online lookup
/// recognise tracks by their sound; empty for none.
pub fn acoustid_key() -> String {
    load()["acoustid_key"].as_str().unwrap_or("").to_string()
}

pub fn set_acoustid_key(key: &str) {
    save(|v| v["acoustid_key"] = json!(key.trim()));
}

/// A saved switch, or `default`.
pub fn flag(key: &str, default: bool) -> bool {
    load()[key].as_bool().unwrap_or(default)
}

/// A saved text, or "".
pub fn string(key: &str) -> String {
    load()[key].as_str().unwrap_or_default().to_string()
}

/// The setting that says whether an importer runs at all; off until the DJ
/// turns it on.
pub fn import_enabled_key(importer: &str) -> String {
    format!("import:{importer}")
}

pub fn import_enabled(importer: &str) -> bool {
    flag(&import_enabled_key(importer), false)
}

/// The database an importer reads: the one the DJ picked in the import
/// wizard, else the one the app keeps where it usually does.
pub fn import_database(importer: &dyn djsugar_plugins::LibraryImport) -> Option<PathBuf> {
    path_of(&import_database_key(importer.id())).or_else(|| importer.database())
}

pub fn import_database_key(importer: &str) -> String {
    format!("import_database:{importer}")
}

/// Whether the other app's tags, ratings and colours replace the library's,
/// or only fill in what is empty; they replace them unless the DJ says.
pub fn import_theirs_win(importer: &str) -> bool {
    flag(&import_theirs_key(importer), true)
}

/// Whether streamed tracks (TIDAL, Spotify…) in `importer`'s playlists
/// that the collection hasn't got go on the Wanted list; off unless the DJ
/// turns it on. History never does.
pub fn import_wants_streamed(importer: &str) -> bool {
    flag(&import_wants_streamed_key(importer), false)
}

/// Under `import:`, so changing it brings the import in again.
pub fn import_wants_streamed_key(importer: &str) -> String {
    format!("import:{importer}:streamed_to_wanted")
}

pub fn import_theirs_key(importer: &str) -> String {
    format!("import_theirs:{importer}")
}

/// Set when the DJ removes an import; the next library update takes it
/// out and clears it.
/// The setting that says the DJ made an importer's lists their own: the
/// import keeps bringing in tracks, cues and grids, but no lists.
pub fn import_lists_mine_key(importer: &str) -> String {
    format!("import_lists_mine:{importer}")
}

pub fn import_remove_key(importer: &str) -> String {
    format!("import_remove:{importer}")
}

/// The setting that says whether an importer brings in one kind of list.
pub fn import_group_key(importer: &str, group: &str) -> String {
    format!("import:{importer}:{group}")
}

/// The kinds of lists the DJ wants `importer` to bring in, by id.
pub fn import_groups(importer: &dyn djsugar_plugins::LibraryImport) -> Vec<&'static str> {
    let v = load();
    importer
        .list_groups()
        .into_iter()
        .filter(|g| {
            v[import_group_key(importer.id(), g.id).as_str()]
                .as_bool()
                .unwrap_or(g.on_by_default)
        })
        .map(|g| g.id)
        .collect()
}

/// The setting with the lists the DJ ticked or unticked for `importer`, by
/// [`ImportedList::key`](djsugar_plugins::ImportedList::key): only those that
/// differ from what comes in by default are kept.
pub fn import_lists_key(importer: &str) -> String {
    format!("import_lists:{importer}")
}

/// The DJ's choice for each list of `importer` they changed.
pub fn import_list_choices(importer: &str) -> HashMap<String, bool> {
    load()[import_lists_key(importer).as_str()]
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_bool()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `list` comes in, by the DJ's `choices`.
pub fn import_list_wanted(
    choices: &HashMap<String, bool>,
    list: &djsugar_plugins::ImportedList,
) -> bool {
    choices
        .get(&list.key())
        .copied()
        .unwrap_or_else(|| list.wanted_by_default())
}

/// A saved path, if one was chosen.
pub fn path_of(key: &str) -> Option<PathBuf> {
    load()[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// A saved list of paths.
pub fn paths(key: &str) -> Vec<PathBuf> {
    load()[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn set(key: &str, value: Value) {
    save(|v| v[key] = value);
}

fn wanted_sync_key(service: &str) -> String {
    format!("wanted_sync:{service}")
}

/// The streaming playlists (by key) of `service` ("spotify", "tidal")
/// kept in sync with the Wanted list: tracks added to them that the
/// collection hasn't got go on it.
pub fn wanted_sync(service: &str) -> Vec<String> {
    load()[wanted_sync_key(service).as_str()]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Keeps playlist `key` of `service` in sync with the Wanted list, or stops.
pub fn set_wanted_sync(service: &str, key: &str, on: bool) {
    let mut keys = wanted_sync(service);
    keys.retain(|k| k != key);
    if on {
        keys.push(key.to_string());
    }
    set(&wanted_sync_key(service), json!(keys));
}

/// Notes a controller as shown in Settings › Controllers, by its device key.
/// The page lists every controller plugged in now; this is kept for
/// settings files that read it.
pub fn set_controller_shown(key: &str, shown: bool) {
    save(|v| {
        if !v["controllers_shown"].is_object() {
            v["controllers_shown"] = json!({});
        }
        v["controllers_shown"][key] = json!(shown);
    });
}

/// Controllers whose knobs and faders use Scale rather than Pick-up, by
/// device key (see `djsugar_controllers::pickup`).
pub fn controllers_scaled() -> Vec<String> {
    load()["controller_scale"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| v.as_bool() == Some(true))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_controller_scaled(key: &str, on: bool) {
    save(|v| {
        if !v["controller_scale"].is_object() {
            v["controller_scale"] = json!({});
        }
        v["controller_scale"][key] = json!(on);
    });
}

/// The mapping picked for each controller, by its device key.
pub fn controller_mappings() -> HashMap<String, MappingChoice> {
    load()["controller_mappings"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let choice = match v.as_str()? {
                        "off" => MappingChoice::Off,
                        "instrument" => MappingChoice::Instrument,
                        own if own.starts_with("own:") => MappingChoice::Own(
                            own_mappings_dir()?.join(format!("{}.json", &own[4..])),
                        ),
                        file => MappingChoice::File(file.to_string()),
                    };
                    Some((k.clone(), choice))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_controller_mappings(choices: &HashMap<String, MappingChoice>) {
    save(|v| {
        v["controller_mappings"] = choices
            .iter()
            .filter_map(|(k, c)| match c {
                MappingChoice::Auto => None,
                MappingChoice::Off => Some((k.clone(), json!("off"))),
                MappingChoice::Instrument => Some((k.clone(), json!("instrument"))),
                MappingChoice::File(f) => Some((k.clone(), json!(f))),
                MappingChoice::Own(p) => {
                    let id = p.file_stem()?.to_string_lossy();
                    Some((k.clone(), json!(format!("own:{id}"))))
                }
            })
            .collect::<serde_json::Map<_, _>>()
            .into();
    });
}

/// The deck each controller plays (As mapped, Follow, a deck), by its
/// device key; Auto isn't kept.
pub fn controller_decks() -> HashMap<String, DeckChoice> {
    load()["controller_decks"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), DeckChoice::from_setting(v.as_str()?))))
                .filter(|(_, c)| *c != DeckChoice::Auto)
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_controller_decks(choices: &HashMap<String, DeckChoice>) {
    save(|v| {
        v["controller_decks"] = choices
            .iter()
            .filter_map(|(k, c)| Some((k.clone(), json!(c.to_setting()?))))
            .collect::<serde_json::Map<_, _>>()
            .into();
    });
}

/// The MIDI keyboard (its port name) and channel (1 to 16) that play the
/// instrument; none and 0 for any.
pub fn instrument_midi_from() -> (Option<String>, u8) {
    let v = &load()["instrument_midi_from"];
    let device = v["device"]
        .as_str()
        .filter(|d| !d.is_empty())
        .map(String::from);
    let channel = v["channel"].as_u64().filter(|&c| c <= 16).unwrap_or(0) as u8;
    (device, channel)
}

pub fn set_instrument_midi_from(device: Option<&str>, channel: u8) {
    save(|v| {
        v["instrument_midi_from"] = json!({ "device": device.unwrap_or(""), "channel": channel });
    });
}

/// Where the DJ's own controller mappings are kept.
pub fn own_mappings_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("controller-mappings"))
}

/// The DJ's own mapping used in place of a base mapping (by its file
/// name), if one was picked.
pub fn controller_version(based_on: &str) -> Option<String> {
    load()["controller_versions"][based_on]
        .as_str()
        .map(String::from)
}

/// Every own mapping picked in place of a base mapping: the base
/// mapping's file name, and the own mapping's file.
pub fn controller_versions() -> HashMap<String, PathBuf> {
    let Some(dir) = own_mappings_dir() else {
        return HashMap::new();
    };
    load()["controller_versions"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    Some((k.clone(), djsugar_controllers::own::path(&dir, v.as_str()?)))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_controller_version(based_on: &str, id: Option<&str>) {
    save(|v| {
        if !v["controller_versions"].is_object() {
            v["controller_versions"] = json!({});
        }
        let versions = v["controller_versions"].as_object_mut().unwrap();
        match id {
            Some(id) => {
                versions.insert(based_on.to_string(), json!(id));
            }
            None => {
                versions.remove(based_on);
            }
        }
    });
}

/// The folders scanned for music: `DJSUGAR_MUSIC`, else the ones chosen in
/// settings. There's no default: `~/Music` is where Ableton, GarageBand and
/// the like keep their samples, so it's only scanned when the DJ adds it.
pub fn music_folders() -> Vec<PathBuf> {
    if std::env::var_os("DJSUGAR_MUSIC").is_some() {
        return default_music_dir().into_iter().collect();
    }
    let v = load();
    if v[MUSIC_FOLDERS].is_array() {
        return paths(MUSIC_FOLDERS);
    }
    // Before there could be several, there was one.
    let saved: Vec<PathBuf> = path_of("music_folder").into_iter().collect();
    // On Android: the phone's own Music and Download folders until the DJ
    // picks others.
    #[cfg(target_os = "android")]
    if saved.is_empty() {
        return ["Music", "Download"]
            .iter()
            .map(|d| PathBuf::from("/storage/emulated/0").join(d))
            .collect();
    }
    saved
}

pub const MUSIC_FOLDERS: &str = "music_folders";

pub fn set_music_folders(folders: &[PathBuf]) {
    let folders: Vec<String> = folders
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    save(|v| v[MUSIC_FOLDERS] = json!(folders));
}

/// The stage folders (see [`djsugar_library::stages`]): any number of inbox
/// folders, one triage folder and one collection folder.
pub fn stage_folders() -> djsugar_library::stages::Folders {
    djsugar_library::stages::Folders {
        inbox: paths(STAGE_INBOX),
        triage: path_of(STAGE_TRIAGE),
        collection: path_of(STAGE_COLLECTION),
        albums: albums_folder(),
    }
}

/// Albums, a folder each (or Artist/Album): in the collection, and in the
/// rail's Albums source.
pub const ALBUMS_FOLDER: &str = "albums_folder";
/// Mixed sets: recordings of whole DJ sets, hours long. Kept out of the
/// collection, in the rail's Mixed Sets source; DJ Sugar's own recordings go
/// in a subfolder.
pub const MIXED_SETS_FOLDER: &str = "mixed_sets_folder";
/// The subfolder of the mixed sets folder that recordings are saved in.
pub const RECORDINGS_SUBFOLDER: &str = "DJ Sugar Recordings";

pub fn albums_folder() -> Option<PathBuf> {
    path_of(ALBUMS_FOLDER)
}

pub fn mixed_sets_folder() -> Option<PathBuf> {
    path_of(MIXED_SETS_FOLDER)
}

pub const STAGE_INBOX: &str = "stage_inbox";
pub const STAGE_TRIAGE: &str = "stage_triage";
pub const STAGE_COLLECTION: &str = "stage_collection";
/// Whether DJ Sugar moves files between the stage folders. Off until the DJ
/// turns it on, as another app may still point at where they are; moves
/// wait in a queue meanwhile.
pub const MOVE_FILES: &str = "move_files";
/// Whether a file going from the inbox to triage is renamed "Artist -
/// Title (Remix)".
pub const RENAME_TO_TRIAGE: &str = "rename_to_triage";

pub fn move_files() -> bool {
    flag(MOVE_FILES, false)
}

/// The clean-up rules turned on in Settings › Collection.
pub fn cleanup_rules() -> djsugar_library::cleanup::Rules {
    djsugar_library::cleanup::Rules::from(flag)
}

pub fn rename_to_triage() -> bool {
    flag(RENAME_TO_TRIAGE, true)
}

/// The purchase inbox: whether it's on, the folder it watches, and what it
/// does with what arrives (see [`djsugar_plugins::inbox::Mode`]).
pub struct InboxSettings {
    pub on: bool,
    pub dir: Option<PathBuf>,
    pub mode: Mode,
    /// Seconds since 1970 when it was last switched on: files older than
    /// this were already there and are left alone.
    pub since: u64,
    /// The Music app's media folder, watched too (for iTunes Store
    /// purchases) since when it was switched on; None when not.
    pub music_app: Option<(PathBuf, u64)>,
}

/// The inbox settings. `DJSUGAR_INBOX` sets the folder (and switches it
/// on); otherwise it watches Downloads. It's off until the DJ turns it on,
/// so nothing in Downloads moves unasked.
pub fn inbox() -> InboxSettings {
    let v = load();
    let env = std::env::var_os("DJSUGAR_INBOX").map(PathBuf::from);
    let dir = env
        .clone()
        .or_else(|| v["inbox_dir"].as_str().map(PathBuf::from))
        .or_else(|| djsugar_engine::paths::home_dir().map(|h| h.join("Downloads")));
    let since = v["inbox_since"].as_u64().unwrap_or_else(now_secs);
    let music_app = match (music_app_folder(), v["inbox_music_app_since"].as_u64()) {
        (Some(dir), Some(since)) => Some((dir, since)),
        _ => None,
    };
    InboxSettings {
        on: env.is_some() || v["inbox_on"].as_bool().unwrap_or(false),
        dir,
        mode: Mode::from_name(v["inbox_mode"].as_str().unwrap_or("move")),
        since,
        music_app,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Switches the inbox on or off. Switched on, it takes only files that
/// arrive from now; returns that time.
pub fn set_inbox_on(on: bool) -> u64 {
    let now = now_secs();
    save(|v| {
        v["inbox_on"] = json!(on);
        if on {
            v["inbox_since"] = json!(now);
        }
    });
    now
}

/// Where the Music app (or iTunes before it) keeps its music, on macOS:
/// iTunes Store purchases download there. None elsewhere or when missing.
pub fn music_app_folder() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let music = djsugar_engine::paths::home_dir()?.join("Music");
    [
        "Music/Media.localized/Music",
        "Music/Media/Music",
        "iTunes/iTunes Media/Music",
    ]
    .iter()
    .map(|p| music.join(p))
    .find(|p| p.is_dir())
}

/// Watches the Music app's folder for purchases from now, or stops.
/// Returns the folder and time when on.
pub fn set_music_app_on(on: bool) -> Option<(PathBuf, u64)> {
    let now = now_secs();
    save(|v| {
        v["inbox_music_app_since"] = if on { json!(now) } else { json!(null) };
    });
    if on {
        music_app_folder().map(|d| (d, now))
    } else {
        None
    }
}

pub fn set_inbox_mode(mode: Mode) {
    save(|v| v["inbox_mode"] = json!(mode.name()));
}

/// What the DJ filled in on a plugin's page in Settings, and the plugin's
/// own folder in the config folder.
pub fn plugin_config(id: &str) -> PluginConfig {
    let values = load()["plugins"][id]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    PluginConfig {
        values,
        dir: config_dir().map(|d| d.join("plugins").join(id)),
        music_folders: music_folders(),
    }
}

pub fn set_plugin_value(id: &str, key: &str, value: &str) {
    save(|v| {
        if !v["plugins"].is_object() {
            v["plugins"] = json!({});
        }
        if !v["plugins"][id].is_object() {
            v["plugins"][id] = json!({});
        }
        v["plugins"][id][key] = json!(value);
    });
}

fn strings(v: &Value) -> Option<Vec<String>> {
    v.as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    })
}

/// What the DJ calls grouping ("Trip Stage"; Grouping until they choose),
/// with their tag groups as the library has them (see
/// [`publish_tag_groups`]).
pub fn field_names() -> FieldNames {
    let v = load();
    FieldNames {
        grouping: v["grouping_name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map_or_else(|| FieldNames::default().grouping, String::from),
        ..djsugar_library::query::field_names()
    }
}

pub fn set_grouping_name(name: &str) {
    save(|v| v["grouping_name"] = json!(name));
    publish_grouping_name();
}

/// Makes searches, chips and columns know the grouping by its name.
fn publish_grouping_name() {
    djsugar_library::query::set_field_names(field_names());
}

/// What the DJ called the numbered tag fields before tag groups, for
/// naming the groups made from them.
pub fn old_tag_field_names() -> Vec<String> {
    strings(&load()["tag_fields"]).unwrap_or_default()
}

/// Makes searches, chips, columns and the Info tab know the library's tag
/// groups as they are now.
pub fn publish_tag_groups(db: &djsugar_library::Library) {
    if let (Ok(tags), Ok(tag_colors)) = (db.tag_groups(), db.tag_colors()) {
        let names = field_names();
        djsugar_library::query::set_field_names(FieldNames {
            tags,
            tag_colors,
            ..names
        });
    }
}

/// Tells the browser that tracks' tags changed in the library, so it reads
/// them again.
pub fn tags_changed() {
    let mut names = field_names();
    names.tags_revision += 1;
    djsugar_library::query::set_field_names(names);
}

/// The filter chips kept on the bar, by field. A bar saved before the
/// Crates chip existed gets it once.
pub fn filter_pinned() -> Vec<String> {
    let v = load();
    match strings(&v["filter_pinned"]) {
        Some(pinned) if !v["filter_crates_added"].as_bool().unwrap_or(false) => {
            let pinned = crate::browser::with_crates(pinned);
            save(|v| {
                v["filter_pinned"] = json!(pinned);
                v["filter_crates_added"] = json!(true);
            });
            pinned
        }
        Some(pinned) => pinned,
        None => crate::browser::DEFAULT_PINNED
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

pub fn set_filter_pinned(pinned: &[String]) {
    save(|v| {
        v["filter_pinned"] = json!(pinned);
        v["filter_crates_added"] = json!(true);
    });
}

/// How close a tempo must be when the BPM chip follows the playing track,
/// as an index into the tolerances offered.
pub fn filter_tolerance() -> usize {
    load()["filter_tolerance"].as_u64().unwrap_or(4) as usize
}

pub fn set_filter_tolerance(i: usize) {
    save(|v| v["filter_tolerance"] = json!(i));
}

/// How the Key chip matches the playing track: 0 by the key mixing
/// setting, 1 the same key, 2 Harmonic, 3 Fuzzy.
pub fn filter_key_match() -> usize {
    load()["filter_key_match"].as_u64().unwrap_or(0).min(3) as usize
}

pub fn set_filter_key_match(i: usize) {
    save(|v| v["filter_key_match"] = json!(i));
}

/// How track colours show in the track list: 0 a dot in the Colour
/// column (the default), 1 a strip at the row's edge, 2 the row tinted,
/// 3 not at all. `None` before it was ever set.
pub fn track_color() -> Option<u8> {
    load()["track_color"].as_u64().map(|m| m.min(3) as u8)
}

pub fn set_track_color(mode: u8) {
    save(|v| v["track_color"] = json!(mode.min(3)));
}

/// The track list's columns, in order, as (column id, width); empty for
/// the default set.
pub fn columns() -> Vec<(i32, f32)> {
    columns_at("columns")
}

/// The columns a history session shows, as [`columns`].
pub fn history_columns() -> Vec<(i32, f32)> {
    columns_at("history_columns")
}

fn columns_at(key: &str) -> Vec<(i32, f32)> {
    load()[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some((
                        c["id"].as_i64()? as i32,
                        c["width"].as_f64().unwrap_or(0.0) as f32,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The columns the DJ hid from the track list, and where they were.
pub fn hidden_columns() -> Vec<Hidden> {
    hidden_at("hidden_columns")
}

/// The columns hidden from a history session, as [`hidden_columns`].
pub fn hidden_history_columns() -> Vec<Hidden> {
    hidden_at("hidden_history_columns")
}

fn hidden_at(key: &str) -> Vec<Hidden> {
    load()[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(Hidden {
                        id: c["id"].as_i64()? as i32,
                        width: c["width"].as_f64().unwrap_or(0.0) as f32,
                        after: c["after"].as_i64().map(|a| a as i32),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_columns(layout: &Layout) {
    set_columns_at("columns", "hidden_columns", layout);
}

pub fn set_history_columns(layout: &Layout) {
    set_columns_at("history_columns", "hidden_history_columns", layout);
}

/// The In Prep list's columns, kept apart from the other lists'.
pub fn prep_columns() -> Vec<(i32, f32)> {
    columns_at("prep_columns")
}

pub fn hidden_prep_columns() -> Vec<Hidden> {
    hidden_at("hidden_prep_columns")
}

pub fn set_prep_columns(layout: &Layout) {
    set_columns_at("prep_columns", "hidden_prep_columns", layout);
}

/// The columns a Collection Health area shows.
pub fn health_columns() -> Vec<(i32, f32)> {
    columns_at("health_columns")
}

pub fn hidden_health_columns() -> Vec<Hidden> {
    hidden_at("hidden_health_columns")
}

pub fn set_health_columns(layout: &Layout) {
    set_columns_at("health_columns", "hidden_health_columns", layout);
}

fn set_columns_at(key: &str, hidden_key: &str, layout: &Layout) {
    let list: Vec<Value> = layout
        .shown
        .iter()
        .map(|(id, width)| json!({ "id": id, "width": width }))
        .collect();
    let hidden: Vec<Value> = layout
        .hidden
        .iter()
        .map(|h| json!({ "id": h.id, "width": h.width, "after": h.after }))
        .collect();
    save(|v| {
        v[key] = json!(list);
        v[hidden_key] = json!(hidden);
    });
}

fn shortcut_list(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some((
                        s["key"].as_str()?.to_string(),
                        s["script"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn shortcut_json(shortcuts: &[(String, String)]) -> Value {
    json!(shortcuts
        .iter()
        .map(|(key, script)| json!({ "key": key, "script": script }))
        .collect::<Vec<_>>())
}

/// The name DJ Sugar's own set of keyboard shortcuts goes by.
pub const DEFAULT_KEYBOARD_SET: &str = "Default";

/// The keyboard shortcut set in use; [`DEFAULT_KEYBOARD_SET`] unless the DJ
/// picked one of their own.
pub fn keyboard_set() -> String {
    let v = load();
    let name = v["keyboard_set"].as_str().unwrap_or("");
    let exists = v["keyboard_sets"]
        .as_array()
        .is_some_and(|a| a.iter().any(|s| s["name"].as_str() == Some(name)));
    if exists {
        name.to_string()
    } else {
        DEFAULT_KEYBOARD_SET.to_string()
    }
}

/// Every keyboard shortcut set: the default first, then the DJ's own.
pub fn keyboard_sets() -> Vec<String> {
    let mut names = vec![DEFAULT_KEYBOARD_SET.to_string()];
    if let Some(a) = load()["keyboard_sets"].as_array() {
        names.extend(
            a.iter()
                .filter_map(|s| s["name"].as_str().map(str::to_string)),
        );
    }
    names
}

/// The keyboard shortcuts of the set in use, as (key, DJSugarScript line)
/// pairs in the order saved. They replace the default for the same key
/// ([`crate::shortcuts::DEFAULTS`]); an empty line turns that key off.
pub fn keyboard_shortcuts() -> Vec<(String, String)> {
    let v = load();
    let set = keyboard_set();
    if set == DEFAULT_KEYBOARD_SET {
        return shortcut_list(&v["keyboard_shortcuts"]);
    }
    v["keyboard_sets"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["name"].as_str() == Some(set.as_str())))
        .map(|s| shortcut_list(&s["shortcuts"]))
        .unwrap_or_default()
}

pub fn set_keyboard_shortcuts(shortcuts: &[(String, String)]) {
    let set = keyboard_set();
    let list = shortcut_json(shortcuts);
    save(|v| {
        if set == DEFAULT_KEYBOARD_SET {
            v["keyboard_shortcuts"] = list;
        } else if let Some(a) = v["keyboard_sets"].as_array_mut() {
            if let Some(s) = a
                .iter_mut()
                .find(|s| s["name"].as_str() == Some(set.as_str()))
            {
                s["shortcuts"] = list;
            }
        }
    });
}

/// A name no set has yet: `name`, then "name 2".
fn free_set_name(name: &str) -> String {
    let taken = keyboard_sets();
    (1..)
        .map(|n| {
            if n == 1 {
                name.to_string()
            } else {
                format!("{name} {n}")
            }
        })
        .find(|n| !taken.contains(n))
        .unwrap_or_else(|| name.to_string())
}

/// Saves `shortcuts` as a new set called `name` (or "name 2" when taken)
/// and starts using it. Returns the name it got.
pub fn add_keyboard_set(name: &str, shortcuts: &[(String, String)]) -> String {
    let name = free_set_name(name.trim());
    let list = shortcut_json(shortcuts);
    let n = name.clone();
    save(|v| {
        if !v["keyboard_sets"].is_array() {
            v["keyboard_sets"] = json!([]);
        }
        if let Some(a) = v["keyboard_sets"].as_array_mut() {
            a.push(json!({ "name": n, "shortcuts": list }));
        }
        v["keyboard_set"] = json!(n);
    });
    name
}

pub fn select_keyboard_set(name: &str) {
    save(|v| v["keyboard_set"] = json!(name));
}

/// Renames the set in use; the default keeps its name.
pub fn rename_keyboard_set(new: &str) {
    let old = keyboard_set();
    let new = new.trim().to_string();
    if old == DEFAULT_KEYBOARD_SET || new.is_empty() || keyboard_sets().contains(&new) {
        return;
    }
    save(|v| {
        if let Some(a) = v["keyboard_sets"].as_array_mut() {
            if let Some(s) = a
                .iter_mut()
                .find(|s| s["name"].as_str() == Some(old.as_str()))
            {
                s["name"] = json!(new);
            }
        }
        v["keyboard_set"] = json!(new);
    });
}

/// Deletes the set in use and goes back to the default.
pub fn delete_keyboard_set() {
    let set = keyboard_set();
    if set == DEFAULT_KEYBOARD_SET {
        return;
    }
    save(|v| {
        if let Some(a) = v["keyboard_sets"].as_array_mut() {
            a.retain(|s| s["name"].as_str() != Some(set.as_str()));
        }
        v["keyboard_set"] = json!(DEFAULT_KEYBOARD_SET);
    });
}

fn save(change: impl FnOnce(&mut Value)) {
    let Some(path) = path() else { return };
    let mut v = load();
    change(&mut v);
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(path, v.to_string());
}

/// A track's key as the user wants keys written. Keys that can't be read
/// (an odd tag) are shown as they are.
pub fn show_key(key: &str, camelot: bool) -> String {
    match Key::parse(key) {
        Some(k) => k.label(camelot),
        None => key.to_string(),
    }
}

/// Whether the phone remote was on when DJ Sugar last quit; off at first.
pub fn remote_on() -> bool {
    load()["remote_on"].as_bool().unwrap_or(false)
}

pub fn set_remote_on(on: bool) {
    save(|v| v["remote_on"] = json!(on));
}

/// The phones paired with the remote. Each keeps only a hash of the token
/// its phone logs in with.
pub fn remote_devices() -> Vec<djsugar_remote::Device> {
    load()["remote_devices"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(djsugar_remote::Device::from_json)
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_remote_devices(devices: &[djsugar_remote::Device]) {
    let list: Vec<Value> = devices.iter().map(|d| d.to_json()).collect();
    save(|v| v["remote_devices"] = Value::Array(list));
}

/// The DJ's own Prep checklist lines (Settings › Collection › Stages), in
/// order; empty ones are left out.
pub fn prep_lines() -> Vec<String> {
    load()["prep_lines"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// The lines as typed, empty ones too, for the settings page.
pub fn prep_lines_typed() -> Vec<String> {
    load()["prep_lines"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_prep_lines(lines: &[String]) {
    set("prep_lines", json!(lines));
}

/// The Prep checklist lines the app ticks itself: key and name.
pub const PREP_APP_LINES: [(&str, &str); 5] = [
    ("grid", "Grid checked"),
    ("cues", "4 hot cues"),
    ("genre", "Genre"),
    ("rating", "Rating"),
    ("cover", "Cover"),
];

/// The settings key turning one of the app's checklist lines on or off.
pub fn prep_check_key(line: &str) -> String {
    format!("prep_check_{line}")
}

/// Quick review so far: seconds spent on tracks sent to Play Ready from
/// Prep, and how many.
pub fn prep_review() -> (f64, u64) {
    let v = load();
    (
        v["prep_review_seconds"].as_f64().unwrap_or(0.0),
        v["prep_review_tracks"].as_u64().unwrap_or(0),
    )
}

pub fn set_prep_review(seconds: f64, tracks: u64) {
    save(|v| {
        v["prep_review_seconds"] = json!(seconds);
        v["prep_review_tracks"] = json!(tracks);
    });
}
