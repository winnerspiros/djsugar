//! The Jam source (formerly Instruments) in the library rail (`ui/instruments.slint`):
//! a tree (everything, the Groove Box, plugins by maker, inputs) and a
//! list of what the selected node holds, dragged onto a deck to play
//! through its channel. Dropping is handled on
//! the screen (`InstrumentsUi.drop`), which sets the deck's controls; the
//! engine does the rest (`[ChannelN],live_input`, `[GrooveN],enabled`).

use std::cell::RefCell;
use std::collections::HashSet;

use slint::{ComponentHandle, DataTransfer, ModelRc, SharedString, VecModel};

use djsugar_engine::inputs;

use crate::audio_plugins::Found;
use crate::presets::{self, Preset, GROOVE};
use crate::{
    settings, AppWindow, InstrumentNode, InstrumentsUi, LiveItem, MixerStripInfo, Preferences,
};

/// What dragging an item carries: `djsugar-live:<kind>:<index>`.
const LIVE_PAYLOAD: &str = "djsugar-live:";

/// Kinds of item, as `LiveItem.kind`.
const KIT: i32 = 1;
const PLUGIN: i32 = 2;
const MIC: i32 = 3;
// 4 was an aux input on its own, before the Mixer.
/// A preset, by its place in `crate::presets::all`.
const PRESET: i32 = 5;
/// The Looper (`[LooperN],enabled`).
const LOOPER: i32 = 6;
/// The Mixer: the aux inputs as strips, summed into one deck.
const MIXER: i32 = 7;
const LAST_KIND: i32 = 7;

fn payload(kind: i32, index: i32) -> String {
    format!("{LIVE_PAYLOAD}{kind}:{index}")
}

/// The item a drag's text names, as `kind * 1000 + index`.
fn parse(text: &str) -> Option<i32> {
    let (kind, index) = text.strip_prefix(LIVE_PAYLOAD)?.split_once(':')?;
    let (kind, index): (i32, i32) = (kind.parse().ok()?, index.parse().ok()?);
    ((1..=LAST_KIND).contains(&kind) && (0..1000).contains(&index)).then_some(kind * 1000 + index)
}

/// What the tree and the list were built from, kept to rebuild them when a
/// node is opened or selected, the search changes, or plugins and inputs do.
#[derive(Default)]
struct State {
    /// The ticked instrument plugins: picker name, maker, format, kind.
    plugins: Vec<Plugin>,
    /// Each input's device and channels, by the engine's input index.
    routes: Vec<String>,
    /// Nodes opened, by key; Plugins starts open.
    open: HashSet<String>,
    /// The selected node's key.
    current: String,
    search: String,
    /// The keys of the nodes as last shown, by row.
    keys: Vec<String>,
    /// The presets, as `crate::presets::all` lists them.
    presets: Vec<Preset>,
    /// The tag chips on, as `crate::presets::tags` numbers them: the list
    /// then shows only presets with every one.
    chips: Vec<bool>,
    /// What went onto a deck lately, newest first (see [`recent_key`]).
    recent: Vec<String>,
    /// The column the list is sorted by (0 name, 1 type, 2 maker,
    /// 3 format) and whether it is the other way round; none as listed.
    sort: Option<(i32, bool)>,
}

/// How many items Recently played keeps.
const RECENT: usize = 20;

/// What an item is, kept across restarts for Recently played: `groove`,
/// `plugin:<key>`, `preset:<path>` or `input:<engine input>`.
fn recent_key(s: &State, kind: i32, index: i32) -> Option<String> {
    let i = usize::try_from(index).ok()?;
    match kind {
        KIT => Some("groove".into()),
        LOOPER => Some("looper".into()),
        PLUGIN => s.plugins.get(i).map(|p| format!("plugin:{}", p.key)),
        PRESET => s
            .presets
            .get(i)
            .map(|p| format!("preset:{}", p.path.display())),
        MIC | MIXER => INPUTS
            .iter()
            .find(|(k, x, _, _)| *k == kind && *x == index)
            .map(|(_, _, input, _)| format!("input:{input}")),
        _ => None,
    }
}

/// Puts `key` first in Recently played.
fn remember(recent: &mut Vec<String>, key: String) {
    recent.retain(|k| *k != key);
    recent.insert(0, key);
    recent.truncate(RECENT);
}

struct Plugin {
    key: String,
    name: String,
    maker: String,
    format: &'static str,
    kind: &'static str,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        open: HashSet::from(["plugins".to_string()]),
        current: "all".to_string(),
        recent: crate::settings::load()["instruments_recent"]
            .as_array()
            .map(|a| a.iter().filter_map(|k| k.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        ..State::default()
    });
}

/// Where the Mixer's line sits in `State::routes`: one past the engine's
/// inputs, so a deck's `live_input` less one finds it as it finds an input's.
const MIXER_ROUTE: usize = inputs::MIXER - 1;

/// The microphone and the Mixer, with where their lines sit in
/// `State::routes` (the engine's input index for the microphone) and their
/// names. The aux inputs play as the Mixer's strips, not one by one.
const INPUTS: [(i32, i32, usize, &str); 2] =
    [(MIC, 0, 0, "Microphone"), (MIXER, 0, MIXER_ROUTE, "Mixer")];

pub fn install(app: &AppWindow) {
    let ui = app.global::<InstrumentsUi>();
    ui.set_audio_page(crate::prefs::AUDIO_PAGE);
    ui.set_plugins_page(crate::prefs::PLUGINS_PAGE);
    ui.on_payload(|kind, index| DataTransfer::from(SharedString::from(payload(kind, index))));
    ui.on_item(|data| data.plain_text().ok().and_then(|t| parse(&t)).unwrap_or(-1));
    let weak = app.as_weak();
    ui.on_open_settings(move |page, tab| {
        if let Some(app) = weak.upgrade() {
            app.global::<Preferences>().invoke_show_tab(page, tab);
        }
    });
    let weak = app.as_weak();
    ui.on_select_node(move |row| {
        STATE.with_borrow_mut(|s| {
            if let Some(key) = s.keys.get(row as usize).cloned() {
                s.current = key;
            }
        });
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    ui.on_toggle_node(move |row| {
        STATE.with_borrow_mut(|s| {
            if let Some(key) = s.keys.get(row as usize).cloned() {
                if !s.open.remove(&key) {
                    s.open.insert(key);
                }
            }
        });
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    ui.on_played(move |item| {
        STATE.with_borrow_mut(|s| {
            if let Some(key) = recent_key(s, item / 1000, item % 1000) {
                remember(&mut s.recent, key);
                crate::settings::set("instruments_recent", serde_json::json!(s.recent));
            }
        });
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    ui.on_sort_by(move |column| {
        STATE.with_borrow_mut(|s| {
            s.sort = match s.sort {
                Some((c, down)) if c == column => Some((column, !down)),
                _ => Some((column, false)),
            };
        });
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    ui.on_toggle_chip(move |tag| {
        STATE.with_borrow_mut(|s| {
            let n = presets::tags().count();
            s.chips.resize(n, false);
            if let Some(on) = s.chips.get_mut(tag as usize) {
                *on = !*on;
            }
        });
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    ui.on_search(move |text| {
        STATE.with_borrow_mut(|s| s.search = text.to_string());
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    refresh(app);
}

/// Lists the ticked instrument plugins, in the Instrument input's order,
/// under the names its picker shows.
pub fn show_plugins(app: &AppWindow, instruments: &[Found], names: &[String]) {
    STATE.with_borrow_mut(|s| {
        s.plugins = instruments
            .iter()
            .zip(names)
            .map(|(f, name)| Plugin {
                key: f.key.clone(),
                name: name.clone(),
                maker: f.vendor.clone(),
                format: f.format,
                kind: plugin_kind(&f.features),
            })
            .collect();
    });
    app.global::<InstrumentsUi>()
        .set_plugin_formats(ModelRc::new(VecModel::from(
            instruments
                .iter()
                .map(|f| SharedString::from(f.format))
                .collect::<Vec<_>>(),
        )));
    crate::instrument_art::show_plugins(
        app,
        instruments
            .iter()
            .map(|f| crate::instrument_art::Plugin {
                key: f.key.clone(),
                name: f.name.clone(),
                maker: f.vendor.clone(),
            })
            .collect(),
    );
    presets::set_plugins(
        app,
        instruments
            .iter()
            .zip(names)
            .map(|(f, name)| presets::Instrument {
                key: f.key.clone(),
                name: name.clone(),
                plugin_name: f.name.clone(),
                maker: f.vendor.clone(),
            })
            .collect(),
    );
    refresh(app);
}

/// Lists the presets again, after one was saved or renamed.
pub fn presets_changed(app: &AppWindow) {
    refresh(app);
}

/// What an instrument plugin says it is, in a word.
fn plugin_kind(features: &[String]) -> &'static str {
    let has = |word: &str| features.iter().any(|f| f.to_lowercase().contains(word));
    if has("drum") {
        "Drums"
    } else if has("sampler") {
        "Sampler"
    } else if has("synth") {
        "Synth"
    } else {
        "Instrument"
    }
}

/// The plugins' makers, in name order, "Other" for none.
fn makers(plugins: &[Plugin]) -> Vec<String> {
    let mut makers: Vec<String> = plugins.iter().map(|p| maker(p).to_string()).collect();
    makers.sort_by_key(|m| m.to_lowercase());
    makers.dedup_by_key(|m| m.to_lowercase());
    makers
}

fn maker(p: &Plugin) -> &str {
    if p.maker.is_empty() {
        "Other"
    } else {
        &p.maker
    }
}

/// The inputs listed: the microphone and the Mixer.
fn live_inputs() -> impl Iterator<Item = &'static (i32, i32, usize, &'static str)> {
    INPUTS.iter()
}

/// The tree: (key, node).
fn nodes(s: &State) -> Vec<(String, InstrumentNode)> {
    let node = |name: &str, depth, children, kind, count| InstrumentNode {
        name: name.into(),
        depth,
        children,
        kind,
        count,
    };
    let open = |key: &str| if s.open.contains(key) { 2 } else { 1 };
    let inputs = INPUTS.len() as i32;
    let total = 2 + s.plugins.len() as i32 + inputs;
    let mut out = vec![
        (
            "recent".to_string(),
            node("Recently played", 0, 0, 8, recent_items(s).len() as i32),
        ),
        ("all".to_string(), node("All instruments", 0, 0, 6, total)),
        (
            "presets".to_string(),
            node(
                "My presets",
                0,
                0,
                7,
                s.presets.iter().filter(|p| !p.from_plugin).count() as i32,
            ),
        ),
        ("groove".to_string(), node("Groove Box", 0, 0, 1, -1)),
        ("looper".to_string(), node("Looper", 0, 0, 9, -1)),
        (
            "plugins".to_string(),
            node(
                "Plugins",
                0,
                if s.plugins.is_empty() {
                    0
                } else {
                    open("plugins")
                },
                0,
                s.plugins.len() as i32,
            ),
        ),
    ];
    if s.open.contains("plugins") {
        for m in makers(&s.plugins) {
            let key = format!("maker:{m}");
            let mine: Vec<(usize, &Plugin)> = s
                .plugins
                .iter()
                .enumerate()
                .filter(|(_, p)| maker(p).eq_ignore_ascii_case(&m))
                .collect();
            out.push((key.clone(), node(&m, 1, open(&key), 5, mine.len() as i32)));
            if s.open.contains(&key) {
                for (i, p) in mine {
                    out.push((format!("plugin:{i}"), node(&p.name, 2, 0, 2, -1)));
                }
            }
        }
    }
    out.push(("inputs".to_string(), node("Inputs", 0, 0, 4, inputs)));
    out
}

/// The rows the selected node lists, narrowed by the search.
fn rows(s: &State) -> Vec<LiveItem> {
    let groove = || LiveItem {
        kind: KIT,
        index: 0,
        name: "Groove Box".into(),
        r#type: "Drum machine".into(),
        maker: "DJ Sugar".into(),
        format: "Built in".into(),
        problem: "".into(),
    };
    let looper = || LiveItem {
        kind: LOOPER,
        index: 0,
        name: "Looper".into(),
        r#type: "Looper".into(),
        maker: "DJ Sugar".into(),
        format: "Built in".into(),
        problem: "".into(),
    };
    let plugin = |i: usize, p: &Plugin| LiveItem {
        kind: PLUGIN,
        index: i as i32,
        name: p.name.as_str().into(),
        r#type: p.kind.into(),
        maker: p.maker.as_str().into(),
        format: p.format.into(),
        problem: "".into(),
    };
    let input = |&(kind, index, input, name): &(i32, i32, usize, &str)| {
        let route = s.routes.get(input).cloned().unwrap_or_default();
        LiveItem {
            kind,
            index,
            name: name.into(),
            r#type: if kind == MIC { "Microphone" } else { "Mixer" }.into(),
            problem: match (route.is_empty(), kind) {
                (false, _) => "",
                (true, MIC) => "No input picked",
                (true, _) => "No inputs picked",
            }
            .into(),
            maker: route.into(),
            format: "".into(),
        }
    };
    let preset = |i: usize, p: &Preset| LiveItem {
        kind: PRESET,
        index: i as i32,
        name: p.name.as_str().into(),
        r#type: if p.from_plugin {
            "Plugin preset"
        } else {
            "Preset"
        }
        .into(),
        maker: p.instrument_name.as_str().into(),
        format: "".into(),
        problem: "".into(),
    };
    // An instrument's presets, after it (its default sound).
    let presets_of = |key: &str| -> Vec<LiveItem> {
        s.presets
            .iter()
            .enumerate()
            .filter(|(_, p)| p.instrument == key)
            .map(|(i, p)| preset(i, p))
            .collect()
    };
    let plugins = |only: &dyn Fn(&Plugin) -> bool| -> Vec<LiveItem> {
        s.plugins
            .iter()
            .enumerate()
            .filter(|(_, p)| only(p))
            .map(|(i, p)| plugin(i, p))
            .collect()
    };
    let mut rows: Vec<LiveItem> = match s.current.as_str() {
        "groove" => {
            let mut rows = vec![groove()];
            rows.extend(presets_of(GROOVE));
            rows
        }
        "looper" => vec![looper()],
        "presets" => s
            .presets
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.from_plugin)
            .map(|(i, p)| preset(i, p))
            .collect(),
        "plugins" => plugins(&|_| true),
        "inputs" => live_inputs().map(input).collect(),
        "recent" => recent_items(s)
            .into_iter()
            .filter_map(|(kind, index)| {
                let i = index as usize;
                match kind {
                    KIT => Some(groove()),
                    LOOPER => Some(looper()),
                    PLUGIN => s.plugins.get(i).map(|p| plugin(i, p)),
                    PRESET => s.presets.get(i).map(|p| preset(i, p)),
                    _ => INPUTS
                        .iter()
                        .find(|(k, x, _, _)| *k == kind && *x == index)
                        .map(input),
                }
            })
            .collect(),
        key => {
            if let Some(m) = key.strip_prefix("maker:") {
                plugins(&|p| maker(p).eq_ignore_ascii_case(m))
            } else if let Some(i) = key
                .strip_prefix("plugin:")
                .and_then(|i| i.parse::<usize>().ok())
            {
                let mut rows: Vec<LiveItem> =
                    s.plugins.get(i).map(|p| plugin(i, p)).into_iter().collect();
                if let Some(p) = s.plugins.get(i) {
                    rows.extend(presets_of(&p.key));
                }
                rows
            } else {
                let mut all = vec![groove(), looper()];
                all.extend(plugins(&|_| true));
                all.extend(live_inputs().map(input));
                all
            }
        }
    };
    // With tag chips on, only presets that have every one.
    let wanted: Vec<&str> = presets::tags()
        .zip(&s.chips)
        .filter(|(_, on)| **on)
        .map(|(t, _)| t)
        .collect();
    if !wanted.is_empty() {
        // The presets of the instruments listed, and those listed.
        let listed: HashSet<&str> = rows
            .iter()
            .filter_map(|r| match r.kind {
                KIT => Some(GROOVE),
                PLUGIN => s.plugins.get(r.index as usize).map(|p| p.key.as_str()),
                PRESET => s
                    .presets
                    .get(r.index as usize)
                    .map(|p| p.instrument.as_str()),
                _ => None,
            })
            .collect();
        rows = s
            .presets
            .iter()
            .enumerate()
            .filter(|(_, p)| listed.contains(p.instrument.as_str()))
            .filter(|(_, p)| wanted.iter().all(|w| p.tags.iter().any(|t| t == w)))
            .map(|(i, p)| preset(i, p))
            .collect();
    }
    let words: Vec<String> = s.search.split_whitespace().map(str::to_lowercase).collect();
    if !words.is_empty() {
        rows.retain(|r| {
            let text = format!("{} {} {} {}", r.name, r.r#type, r.maker, r.format).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        });
    }
    if let Some((column, descending)) = s.sort {
        let text = |r: &LiveItem| {
            match column {
                0 => &r.name,
                1 => &r.r#type,
                2 if r.maker.is_empty() => &r.problem,
                2 => &r.maker,
                _ => &r.format,
            }
            .to_lowercase()
        };
        rows.sort_by_cached_key(|r| (text(r), r.name.to_lowercase()));
        if descending {
            rows.reverse();
        }
    }
    rows
}

/// Recently played, newest first, as (kind, index): what is still there
/// of it.
fn recent_items(s: &State) -> Vec<(i32, i32)> {
    let all: Vec<(i32, i32)> = [(KIT, 0), (LOOPER, 0)]
        .into_iter()
        .chain((0..s.plugins.len()).map(|i| (PLUGIN, i as i32)))
        .chain((0..s.presets.len()).map(|i| (PRESET, i as i32)))
        .chain(INPUTS.iter().map(|(k, i, _, _)| (*k, *i)))
        .collect();
    s.recent
        .iter()
        .filter_map(|key| {
            all.iter()
                .find(|(k, i)| recent_key(s, *k, *i).as_deref() == Some(key.as_str()))
                .copied()
        })
        .collect()
}

/// Shows the tree and the list again from the state.
fn refresh(app: &AppWindow) {
    let ui = app.global::<InstrumentsUi>();
    let found = presets::all();
    STATE.with_borrow_mut(|s| {
        s.presets = found;
        let nodes = nodes(s);
        // A node gone (a plugin unticked, a maker folded away) selects its parent.
        if !nodes.iter().any(|(k, _)| *k == s.current) {
            s.current = if s.current.starts_with("plugin") || s.current.starts_with("maker") {
                "plugins".to_string()
            } else {
                "all".to_string()
            };
        }
        let current = nodes.iter().position(|(k, _)| *k == s.current).unwrap_or(0);
        let rows = rows(s);
        let (empty, page, tab) = if !rows.is_empty() {
            ("", -1, 0)
        } else if s.chips.iter().any(|on| *on) {
            (
                "No presets have all the tags picked. Tag a preset with SAVE › Tags on its deck.",
                -1,
                0,
            )
        } else if !s.search.is_empty() {
            ("Nothing matches the search.", -1, 0)
        } else if s.current.starts_with("plugin") || s.current.starts_with("maker") {
            ("Tick the instruments you play in Settings › Plugins…", 7, 1)
        } else {
            ("", -1, 0)
        };
        s.keys = nodes.iter().map(|(k, _)| k.clone()).collect();
        let nodes: Vec<InstrumentNode> = nodes.into_iter().map(|(_, n)| n).collect();
        ui.set_nodes(ModelRc::new(VecModel::from(nodes)));
        ui.set_current_node(current as i32);
        ui.set_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_empty_text(empty.into());
        ui.set_empty_page(page);
        ui.set_empty_tab(tab);
        let mut chips = s.chips.clone();
        chips.resize(presets::tags().count(), false);
        ui.set_chips(ModelRc::new(VecModel::from(chips)));
        ui.set_sort_column(s.sort.map_or(-1, |(c, _)| c));
        ui.set_sort_descending(s.sort.is_some_and(|(_, down)| down));
    });
}

/// Shows on the inputs' tiles and live decks where each input comes from,
/// from the engine's input states ("M4 · In 3 · 44.1 kHz"): the device and
/// channels only, or nothing for an input that isn't recording. The
/// Mixer's line, after the inputs', sums up its strips (see [`mixer_route`]).
///
/// `clock` is the engine's status (the main output's device first): a
/// strip on another interface runs on its own clock.
pub fn show_routes(app: &AppWindow, status: &[String], clock: &str) {
    let mut routes: Vec<String> = status.iter().map(|s| route(s)).collect();
    let mixer = mixer_route(&routes);
    routes.resize(MIXER_ROUTE, String::new());
    routes.push(mixer);
    let shown: Vec<SharedString> = routes.iter().map(|r| r.as_str().into()).collect();
    let ui = app.global::<InstrumentsUi>();
    ui.set_routes(ModelRc::new(VecModel::from(shown)));
    ui.set_strips(ModelRc::new(VecModel::from(strips(
        status,
        &settings::mixer_names(),
        &settings::mixer_colors(),
        clock_device(clock),
    ))));
    ui.set_mixer_groove(
        inputs::AUXILIARIES
            .iter()
            .any(|&i| status.get(i).is_some_and(|s| s == inputs::GROOVE_BOX)),
    );
    STATE.with_borrow_mut(|s| s.routes = routes);
    refresh(app);
}

/// The Mixer's channels from the engine's input states: each aux input
/// that has a device, working or not, under the name given it ("Ch N"
/// until renamed), marked when its interface isn't `clock`'s (the main
/// output's device, "" when there is none).
fn strips(status: &[String], names: &[String], colors: &[i32], clock: &str) -> Vec<MixerStripInfo> {
    inputs::AUXILIARIES
        .iter()
        .enumerate()
        .filter_map(|(n, &i)| {
            let status = status.get(i).filter(|s| !s.is_empty())?;
            let route = route(status);
            let mut parts = route.split(" · ");
            let device = parts.next().unwrap_or_default();
            let channels = parts.next().map(channels).unwrap_or_default();
            // The Mixer deck's own Groove Box: no interface, no clock.
            let groove = device == inputs::GROOVE_BOX;
            Some(MixerStripInfo {
                number: n as i32 + 1,
                name: names
                    .get(n)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map_or_else(
                        || {
                            if groove {
                                "Groove".to_string()
                            } else {
                                format!("Ch {}", n + 1)
                            }
                        },
                        String::from,
                    )
                    .into(),
                channels: channels.as_str().into(),
                stereo: channels.contains('/'),
                own_clock: !groove && !route.is_empty() && !clock.is_empty() && device != clock,
                group: format!("[Auxiliary{}]", n + 1).into(),
                badge: (n + 1).to_string().into(),
                on_key: "main_mix".into(),
                aux: true,
                color: colors.get(n).copied().unwrap_or(-1),
                view: if groove { 2 } else { 0 },
                problem: if route.is_empty() {
                    status.as_str()
                } else {
                    ""
                }
                .into(),
            })
        })
        .collect()
}

/// A strip's channels as its input box shows them: "In 3" is "3", "In 1 &
/// 2" is "1/2".
fn channels(label: &str) -> String {
    label.trim_start_matches("In ").replace(" & ", "/")
}

/// The Mixer's line from the inputs' routes: the interfaces its strips
/// come from and how many there are ("M4 · 3 inputs"), or "" with none.
fn mixer_route(routes: &[String]) -> String {
    let mut devices: Vec<&str> = Vec::new();
    let mut count = 0;
    for route in inputs::AUXILIARIES.iter().filter_map(|&i| routes.get(i)) {
        if let Some(device) = route.split(" · ").next().filter(|_| !route.is_empty()) {
            count += 1;
            if !devices.contains(&device) {
                devices.push(device);
            }
        }
    }
    match count {
        0 => String::new(),
        1 => format!("{} · 1 input", devices[0]),
        n => format!("{} · {n} inputs", devices.join(", ")),
    }
}

/// The main output's device from the engine's status ("M4 · 48 kHz · …"),
/// or "" when it isn't playing ("No audio output", "M4 went away").
pub fn clock_device(status: &str) -> &str {
    match status.split_once(" · ") {
        Some((device, rest)) if rest.contains(" kHz") => device,
        _ => "",
    }
}

/// "M4 · In 3" from an open input's state; "" from anything else (not set
/// up, or an error).
fn route(status: &str) -> String {
    if status == inputs::GROOVE_BOX {
        return status.to_string();
    }
    let parts: Vec<&str> = status.split(" · ").collect();
    match parts.iter().position(|p| p.ends_with(" kHz")) {
        Some(k) if k > 0 => parts[..k].join(" · "),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_read_back_as_their_items() {
        assert_eq!(parse(&payload(1, 2)), Some(1002));
        assert_eq!(parse(&payload(2, 0)), Some(2000));
        assert_eq!(parse(&payload(MIXER, 0)), Some(7000));
        assert_eq!(parse("djsugar-row:3"), None);
        assert_eq!(parse("djsugar-live:8:0"), None);
        assert_eq!(parse("djsugar-live:1:x"), None);
    }

    fn state(current: &str) -> State {
        let plugin = |name: &str, maker: &str| Plugin {
            key: format!("clap:{name}"),
            name: name.into(),
            maker: maker.into(),
            format: "CLAP",
            kind: "Synth",
        };
        State {
            plugins: vec![
                plugin("Acid V", "Arturia"),
                plugin("Vital", "Vital Audio"),
                plugin("Jun-6 V", "arturia"),
            ],
            routes: vec![
                "".into(),
                "M4 · In 3".into(),
                "".into(),
                "M4 · In 4".into(),
                "".into(),
            ],
            open: HashSet::from(["plugins".to_string(), "maker:Arturia".to_string()]),
            current: current.into(),
            ..State::default()
        }
    }

    #[test]
    fn plugins_group_by_maker_and_inputs_list_when_set_up() {
        let keys: Vec<String> = nodes(&state("all")).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            [
                "recent",
                "all",
                "presets",
                "groove",
                "looper",
                "plugins",
                "maker:Arturia",
                "plugin:0",
                "plugin:2",
                "maker:Vital Audio",
                "inputs"
            ]
        );
        let names = |s: &State| {
            rows(s)
                .iter()
                .map(|r| r.name.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&state("maker:Arturia")), ["Acid V", "Jun-6 V"]);
        // The mic and the Mixer, which holds the aux inputs.
        assert_eq!(names(&state("inputs")), ["Microphone", "Mixer"]);
        assert_eq!(rows(&state("inputs"))[0].problem, "No input picked");
        assert_eq!(rows(&state("inputs"))[1].problem, "No inputs picked");
        let mut s = state("all");
        s.search = "vital".into();
        assert_eq!(names(&s), ["Vital"]);
        // Sorted by maker, then the other way.
        s.search.clear();
        s.current = "plugins".into();
        s.sort = Some((2, false));
        assert_eq!(names(&s), ["Acid V", "Jun-6 V", "Vital"]);
        s.sort = Some((0, true));
        assert_eq!(names(&s), ["Vital", "Jun-6 V", "Acid V"]);
    }

    #[test]
    fn the_mixer_sums_up_its_strips_interfaces() {
        let mut routes = vec![String::new(); inputs::NUM_INPUTS];
        assert_eq!(mixer_route(&routes), "");
        routes[inputs::AUXILIARY] = "M4 · In 3".into();
        assert_eq!(mixer_route(&routes), "M4 · 1 input");
        routes[inputs::AUXILIARY_3] = "M4 · In 4".into();
        // The mic's and the instrument's routes aren't strips.
        routes[inputs::MICROPHONE] = "MacBook Pro Microphone · In 1".into();
        routes[inputs::INSTRUMENT] = "Plugin".into();
        routes[inputs::AUXILIARIES[7]] = "iPhone · In 1".into();
        assert_eq!(mixer_route(&routes), "M4, iPhone · 3 inputs");
        // Its line sits where a deck's `live_input` less one finds it.
        let mut s = state("inputs");
        s.routes = routes;
        let mixer = mixer_route(&s.routes);
        s.routes.resize(MIXER_ROUTE, String::new());
        s.routes.push(mixer);
        assert_eq!(s.routes.len(), inputs::MIXER);
        let row = &rows(&s)[1];
        assert_eq!(
            (row.maker.as_str(), row.problem.as_str()),
            ("M4, iPhone · 3 inputs", "")
        );
    }

    #[test]
    fn strips_are_the_aux_inputs_with_a_device() {
        let mut status = vec![String::new(); inputs::NUM_INPUTS];
        status[inputs::MICROPHONE] = "MacBook Pro Microphone · In 1 · 48 kHz".into();
        status[inputs::AUXILIARY] = "M4 · In 1 & 2 · 48 kHz".into();
        status[inputs::AUXILIARIES[4]] = "M4 · In 3 · 48 kHz".into();
        status[inputs::AUXILIARIES[7]] = "iPhone is not connected".into();
        status[inputs::AUXILIARIES[2]] = "iPhone · In 1 · 48 kHz".into();
        status[inputs::AUXILIARIES[5]] = inputs::GROOVE_BOX.into();
        let mut names = vec![String::new(); 8];
        names[4] = "Vocals".into();
        let clock = clock_device("M4 · 48 kHz · 256 samples (5.3 ms)");
        let shown: Vec<(i32, String, String, bool, String)> = strips(&status, &names, &[], clock)
            .into_iter()
            .map(|s| {
                let (name, channels, problem) =
                    (s.name.into(), s.channels.into(), s.problem.into());
                (s.number, name, channels, s.own_clock, problem)
            })
            .collect();
        assert_eq!(
            shown,
            [
                (1, "Ch 1".into(), "1/2".into(), false, "".into()),
                (3, "Ch 3".into(), "1".into(), true, "".into()),
                (5, "Vocals".into(), "3".into(), false, "".into()),
                // The Mixer deck's Groove Box: no channels, no clock.
                (6, "Groove".into(), "".into(), false, "".into()),
                (
                    8,
                    "Ch 8".into(),
                    "".into(),
                    false,
                    "iPhone is not connected".into()
                ),
            ]
        );
        // No main output: nothing to drift from.
        assert!(
            strips(&status, &names, &[], clock_device("No audio output"))
                .iter()
                .all(|s| !s.own_clock)
        );
    }

    #[test]
    fn recently_played_lists_what_went_on_a_deck_newest_first() {
        let mut s = state("recent");
        for (kind, index) in [(PLUGIN, 1), (KIT, 0), (MIC, 0), (PLUGIN, 1)] {
            let key = recent_key(&s, kind, index).unwrap();
            remember(&mut s.recent, key);
        }
        assert_eq!(s.recent, ["plugin:clap:Vital", "input:0", "groove"]);
        let names: Vec<String> = rows(&s).iter().map(|r| r.name.to_string()).collect();
        assert_eq!(names, ["Vital", "Microphone", "Groove Box"]);
        // A plugin unticked drops out of the list, not out of the memory.
        s.plugins.remove(1);
        assert_eq!(rows(&s).len(), 2);
        assert_eq!(s.recent.len(), 3);
    }

    #[test]
    fn presets_list_under_their_instrument_and_in_my_presets() {
        let preset = |name: &str, instrument: &str, instrument_name: &str| Preset {
            name: name.into(),
            instrument: instrument.into(),
            instrument_name: instrument_name.into(),
            path: format!("/p/{name}.json").into(),
            tags: if name == "Ambient Ebow" {
                vec!["Pad".into(), "Evolving".into()]
            } else {
                vec!["Drums".into()]
            },
            from_plugin: false,
            reference: String::new(),
        };
        let mut s = state("groove");
        s.presets = vec![
            preset("Four Floor", GROOVE, "Groove Box"),
            preset("Ambient Ebow", "clap:Jun-6 V", "Jun-6 V"),
            Preset {
                tags: Vec::new(),
                from_plugin: true,
                ..preset("Factory Brass", "clap:Jun-6 V", "Jun-6 V")
            },
        ];
        let names = |s: &State| {
            rows(s)
                .iter()
                .map(|r| (r.kind, r.index, r.name.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&s),
            [
                (KIT, 0, "Groove Box".into()),
                (PRESET, 0, "Four Floor".into())
            ]
        );
        s.current = "plugin:2".into();
        assert_eq!(
            names(&s),
            [
                (PLUGIN, 2, "Jun-6 V".into()),
                (PRESET, 1, "Ambient Ebow".into()),
                (PRESET, 2, "Factory Brass".into())
            ]
        );
        // My presets are the user's: the plugin's own list under it only.
        s.current = "presets".into();
        assert_eq!(names(&s).len(), 2);
        let mine = nodes(&s).into_iter().find(|(k, _)| k == "presets").unwrap();
        assert_eq!(mine.1.count, 2);
        assert_eq!(parse(&payload(PRESET, 1)), Some(5001));
        // The Pad chip leaves the presets tagged Pad; Pad and Drums, none.
        let chip = |t: &str| presets::tags().position(|x| x == t).unwrap();
        s.chips = vec![false; presets::tags().count()];
        s.chips[chip("Pad")] = true;
        s.current = "all".into();
        assert_eq!(names(&s), [(PRESET, 1, "Ambient Ebow".into())]);
        s.chips[chip("Drums")] = true;
        assert!(names(&s).is_empty());
    }

    #[test]
    fn routes_keep_the_device_and_channels() {
        assert_eq!(route("M4 · In 3 · 44.1 kHz"), "M4 · In 3");
        assert_eq!(
            route("M4 · In 1 & 2 · 48 kHz · silent so far"),
            "M4 · In 1 & 2"
        );
        assert_eq!(route(""), "");
        assert_eq!(route("M4 unavailable"), "");
        assert_eq!(route("M4 can't record In 3 at 44.1 kHz"), "");
    }
}
