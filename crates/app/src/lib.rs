//! DJ Sugar: the Slint UI wired to the control registry, the audio engine
//! and the GPU waveform renderer.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::cover::Cover;
use djsugar_engine::key::Key;
use djsugar_engine::scan::scan;
use djsugar_engine::stems;
use djsugar_engine::synth::SAMPLE_LIBRARY_SIZE;
use djsugar_engine::vocals::Levels;
use djsugar_engine::{
    deck_group, deck_outputs_split, default_output_device, effect_group, output_devices,
    sample_library, AudioEngine, AudioSetup, Role, Route, Source, Track, VisualPositions, Waveform,
    AVAILABLE_EFFECTS, COLOR_FX, EFFECTS_PER_UNIT, NUM_DECKS, WAVEFORM_RATE,
};
use djsugar_library::Library;
use djsugar_plugins::source::{QueryKind, SourceQuery};
use djsugar_waveform::{
    steady_grid, wgpu, Extras, Frame, GpuTrack, GridSection, Mode, Palette, PhraseKind,
    WaveformRenderer, WaveformView, GRID_SECTIONS,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smooth::SmoothClock;

slint::include_modules!();

mod analysis_queue;
mod app_menu;
mod appearance;
mod apple_music;
mod art;
mod audio_plugins;
mod automix;
mod automix_order;
mod automix_phrases;
mod backup;
mod beat_counter;
mod broadcast;
mod browser;
mod controller_editor;
mod cover_scan;
mod coverart;
mod diagnostics;
mod file_drop;
mod files;
mod groove_box;
mod inbox;
mod instrument_art;
mod instrument_knobs;
mod instrument_midi;
mod instruments;
mod keep_going;
mod lastfm;
mod layouts;
mod list_picker;
mod live_trace;
mod lookup;
mod lrclib;
#[cfg(target_os = "macos")]
mod mac_menu;
#[cfg(target_os = "macos")]
mod mac_titlebar;
mod macro_lanes;
mod map_mode;
mod media_controls;
mod midi_clock;
mod mini_player;
mod mixcloud;
mod mixcloud_upload;
mod mixer_keeper;
mod mixer_picker;
mod mixxx_settings;
mod now_playing;
mod now_playing_view;
mod osc;
mod osc_events;
mod prefs;
mod prep_click;
mod presets;
mod put_back;
mod quality;
mod quick_search;
mod remote;
mod sampler;
mod scrobble;
mod settings;
mod shortcuts;
mod sleep_timer;
mod smooth;
mod soundcloud;
mod sources;
mod spotify;
mod stem_levels;
mod step_lanes;
mod storage;
mod stores;
mod stream_cache;
mod tidal;
mod tour;
mod track_end;
mod transport;
mod tutorials;
mod ui_font;
mod update;
mod vocal_pass;
mod wave_look;
mod waveform_height;
mod waveform_zoom;
mod waves;

/// A DJ controller with its own sound card, plugged in while the master
/// still plays on the computer's default output, takes the master and the
/// headphones (see [`djsugar_engine::controller_routes`]). Once per
/// controller: if the DJ routes it otherwise, that stays.
fn route_controller_cards(
    engine: &Rc<AudioEngine>,
    devices: Rc<RefCell<Vec<djsugar_engine::OutputDevice>>>,
    show: Rc<dyn Fn()>,
) {
    let (tx, rx) = std::sync::mpsc::channel();
    // Listing devices can take a while on some systems: off the UI thread.
    let spawned = std::thread::Builder::new()
        .name("audio-devices".into())
        .spawn(move || loop {
            if tx.send(output_devices()).is_err() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        });
    if spawned.is_err() {
        return;
    }
    let engine = engine.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(500),
        move || {
            let Some(found) = rx.try_iter().last() else {
                return;
            };
            *devices.borrow_mut() = found.clone();
            let Some(routes) = djsugar_engine::controller_routes(&found) else {
                return;
            };
            let card = routes[0].device.clone();
            let key = format!("auto_routed:{card}");
            let mut setup = settings::audio_setup();
            let on_default = setup
                .routes
                .iter()
                .filter(|r| matches!(r.role, Role::Master | Role::Headphones))
                .all(|r| r.device.is_empty());
            if settings::flag(&key, false) || setup.external_mixer || !on_default {
                return;
            }
            settings::set(&key, serde_json::json!(true));
            setup
                .routes
                .retain(|r| !matches!(r.role, Role::Master | Role::Headphones));
            setup.routes.splice(0..0, routes);
            settings::set_audio_setup(&setup);
            engine.set_outputs(&setup);
            engine.alert(format!(
                "Playing through {card}: the master on outputs 1-2, headphones on 3-4. \
                 Settings › Audio changes it."
            ));
            show();
        },
    );
    // The timer runs as long as the app does.
    std::mem::forget(timer);
}

/// Fills the master and headphone output menus and switches devices when
/// one is picked. The menus list the devices present when they open.
/// The settings page's output table: shows the routes in use and applies
/// changes to them.
fn install_audio_settings(app: &AppWindow, engine: Rc<AudioEngine>) {
    let devices = Rc::new(RefCell::new(output_devices()));
    let default_device = Rc::new(RefCell::new(default_output_device()));
    // Kept between updates and changed row by row: replacing a list would
    // rebuild its rows and close a menu that is opening.
    let rows_model: Rc<VecModel<OutputRow>> = Default::default();
    let names_model: Rc<VecModel<SharedString>> = Default::default();
    // With an external mixer: a fixed row for each of EXTERNAL_ROLES, and
    // their device list, which starts with None.
    let fixed_model: Rc<VecModel<OutputRow>> = Default::default();
    let fixed_names_model: Rc<VecModel<SharedString>> = Default::default();
    {
        let g = app.global::<AudioSettings>();
        g.set_rows(ModelRc::from(rows_model.clone()));
        g.set_devices(ModelRc::from(names_model.clone()));
        g.set_fixed_rows(ModelRc::from(fixed_model.clone()));
        g.set_fixed_devices(ModelRc::from(fixed_names_model.clone()));
    }
    let show = {
        let app = app.as_weak();
        let default_device = default_device.clone();
        let engine = engine.clone();
        let devices = devices.clone();
        move || {
            let setup = settings::audio_setup();
            let devices = devices.borrow();
            let default_channels = default_device
                .borrow()
                .as_ref()
                .and_then(|name| devices.iter().find(|d| d.name == *name))
                .map_or(2, |d| d.channels);
            let status = engine.route_status();
            if let Some(app) = app.upgrade() {
                let g = app.global::<AudioSettings>();
                g.set_booth(setup.routes.iter().any(|r| r.role == Role::Booth));
                g.set_external(setup.external_mixer);
            }
            let channels_of = |device: &str| {
                if device.is_empty() {
                    default_channels
                } else {
                    devices
                        .iter()
                        .find(|d| d.name == device)
                        .map_or(2, |d| d.channels)
                }
            };
            let pair_names = |pairs: u16| {
                ModelRc::new(VecModel::from(
                    (0..pairs)
                        .map(|p| SharedString::from(format!("ch {}-{}", 2 * p + 1, 2 * p + 2)))
                        .collect::<Vec<_>>(),
                ))
            };
            let fixed: Vec<OutputRow> = EXTERNAL_ROLES
                .iter()
                .map(|&(role, name)| {
                    let at = setup.routes.iter().position(|r| r.role == role);
                    let Some(at) = at else {
                        return OutputRow {
                            role: Role::ALL.iter().position(|&x| x == role).unwrap_or(0) as i32,
                            name: name.into(),
                            device: 0,
                            device_name: "None".into(),
                            pair: 0,
                            pairs: pair_names(1),
                            single: true,
                            status: SharedString::new(),
                            ok: true,
                        };
                    };
                    let r = &setup.routes[at];
                    let found = devices.iter().position(|d| d.name == r.device);
                    let pairs = (channels_of(&r.device) / 2).max(1).max(r.pair + 1);
                    let status = status.get(at).cloned().unwrap_or_default();
                    OutputRow {
                        role: Role::ALL.iter().position(|&x| x == role).unwrap_or(0) as i32,
                        name: name.into(),
                        device: if r.device.is_empty() {
                            1
                        } else {
                            found.map_or(-1, |k| k as i32 + 2)
                        },
                        device_name: if r.device.is_empty() {
                            "System default".into()
                        } else if found.is_some() {
                            r.device.as_str().into()
                        } else {
                            format!("{} (not connected)", r.device).into()
                        },
                        pair: r.pair as i32,
                        pairs: pair_names(pairs),
                        single: pairs <= 1,
                        ok: status.starts_with(|c: char| c.is_ascii_digit()),
                        status: status.into(),
                    }
                })
                .collect();
            update_rows(&fixed_model, fixed);
            let fixed_names: Vec<SharedString> = ["None", "System default"]
                .into_iter()
                .map(String::from)
                .chain(devices.iter().map(|d| d.name.clone()))
                .map(Into::into)
                .collect();
            if fixed_names_model.iter().ne(fixed_names.iter().cloned()) {
                fixed_names_model.set_vec(fixed_names);
            }
            let rows: Vec<OutputRow> = setup
                .routes
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let found = devices.iter().position(|d| d.name == r.device);
                    let channels = if r.device.is_empty() {
                        default_channels
                    } else {
                        found.map_or(2, |k| devices[k].channels)
                    };
                    let pairs = (channels / 2).max(1).max(r.pair + 1);
                    let status = status.get(i).cloned().unwrap_or_default();
                    OutputRow {
                        role: Role::ALL.iter().position(|&x| x == r.role).unwrap_or(0) as i32,
                        name: SharedString::new(),
                        single: pairs <= 1 && r.pair == 0,
                        device: if r.device.is_empty() {
                            0
                        } else {
                            found.map_or(-1, |k| k as i32 + 1)
                        },
                        device_name: if r.device.is_empty() {
                            "System default".into()
                        } else if found.is_some() {
                            r.device.as_str().into()
                        } else {
                            format!("{} (not connected)", r.device).into()
                        },
                        pair: r.pair as i32,
                        pairs: ModelRc::new(VecModel::from(
                            (0..pairs)
                                .map(|p| {
                                    SharedString::from(format!("{} & {}", 2 * p + 1, 2 * p + 2))
                                })
                                .collect::<Vec<_>>(),
                        )),
                        ok: status.starts_with(|c: char| c.is_ascii_digit()),
                        status: status.into(),
                    }
                })
                .collect();
            let names: Vec<SharedString> = std::iter::once("System default".to_string())
                .chain(devices.iter().map(|d| d.name.clone()))
                .map(Into::into)
                .collect();
            if names_model.iter().ne(names.iter().cloned()) {
                names_model.set_vec(names);
            }
            update_rows(&rows_model, rows);
        }
    };
    show();
    let show = Rc::new(show);
    route_controller_cards(&engine, devices.clone(), show.clone());
    let g = app.global::<AudioSettings>();
    g.on_refresh({
        let devices = devices.clone();
        let show = show.clone();
        let default_device = default_device.clone();
        move || {
            *devices.borrow_mut() = output_devices();
            *default_device.borrow_mut() = default_output_device();
            show();
        }
    });
    let change = {
        let show = show.clone();
        let app = app.as_weak();
        move |edit: &dyn Fn(&mut AudioSetup)| {
            let mut setup = settings::audio_setup();
            // The deck output rows under the table come and go with them.
            let taps = |s: &AudioSetup| {
                (
                    s.routes.iter().any(|r| r.role.is_tap()),
                    deck_outputs_split(s),
                )
            };
            let before = taps(&setup);
            edit(&mut setup);
            settings::set_audio_setup(&setup);
            engine.set_outputs(&setup);
            show();
            if taps(&setup) != before {
                if let Some(app) = app.upgrade() {
                    app.global::<Preferences>().invoke_changed();
                }
            }
        }
    };
    let change = Rc::new(change);
    g.on_set_role({
        let change = change.clone();
        move |row, role| {
            change(&|s| {
                if let (Some(r), Some(&role)) =
                    (s.routes.get_mut(row as usize), Role::ALL.get(role as usize))
                {
                    r.role = role;
                }
            })
        }
    });
    g.on_set_device({
        let change = change.clone();
        let devices = devices.clone();
        move |row, device| {
            let name = match device {
                0 => String::new(),
                d => devices
                    .borrow()
                    .get(d as usize - 1)
                    .map(|d| d.name.clone())
                    .unwrap_or_default(),
            };
            change(&|s| {
                if let Some(r) = s.routes.get_mut(row as usize) {
                    r.device = name.clone();
                    r.pair = 0;
                }
            })
        }
    });
    g.on_set_pair({
        let change = change.clone();
        move |row, pair| {
            change(&|s| {
                if let Some(r) = s.routes.get_mut(row as usize) {
                    r.pair = pair.max(0) as u16;
                }
            })
        }
    });
    g.on_remove({
        let change = change.clone();
        move |row| {
            change(&|s| {
                if (row as usize) < s.routes.len() {
                    s.routes.remove(row as usize);
                }
            })
        }
    });
    g.on_set_fixed_device({
        let change = change.clone();
        let devices = devices.clone();
        move |row, device| {
            let Some(&(role, _)) = EXTERNAL_ROLES.get(row as usize) else {
                return;
            };
            let name = match device {
                0 => None,
                1 => Some(String::new()),
                d => devices.borrow().get(d as usize - 2).map(|d| d.name.clone()),
            };
            change(&|s| {
                let Some(name) = &name else {
                    s.routes.retain(|r| r.role != role);
                    return;
                };
                // The first pair no other output on the device plays on.
                let taken = |p: u16| {
                    s.routes
                        .iter()
                        .any(|r| r.role != role && s.in_use(r) && r.device == *name && r.pair == p)
                };
                let pair = (0..32).find(|&p| !taken(p)).unwrap_or(0);
                match s.routes.iter_mut().find(|r| r.role == role) {
                    Some(r) => {
                        r.device = name.clone();
                        r.pair = pair;
                    }
                    None => s.routes.push(Route {
                        role,
                        device: name.clone(),
                        pair,
                    }),
                }
            })
        }
    });
    g.on_set_fixed_pair({
        let change = change.clone();
        move |row, pair| {
            let Some(&(role, _)) = EXTERNAL_ROLES.get(row as usize) else {
                return;
            };
            change(&|s| {
                if let Some(r) = s.routes.iter_mut().find(|r| r.role == role) {
                    r.pair = pair.max(0) as u16;
                }
            })
        }
    });
    g.on_add(move || {
        let devices = devices.clone();
        change(&|s| {
            // The first mix not routed yet, on the first route's device, on
            // the first pair it leaves free.
            let role = Role::ALL
                .into_iter()
                .find(|role| s.routes.iter().all(|r| r.role != *role))
                .unwrap_or(Role::Booth);
            let device = s
                .routes
                .first()
                .map(|r| r.device.clone())
                .unwrap_or_default();
            let channels = devices
                .borrow()
                .iter()
                .find(|d| d.name == device)
                .map_or(2, |d| d.channels);
            let pair = (0..(channels / 2).max(1))
                .find(|p| !s.routes.iter().any(|r| r.device == device && r.pair == *p))
                .unwrap_or(0);
            s.routes.push(Route { role, device, pair });
        })
    });
}

/// The rows Settings › Audio shows with an external mixer, in order.
const EXTERNAL_ROLES: [(Role, &str); 6] = [
    (Role::Deck1, "Deck 1"),
    (Role::Deck2, "Deck 2"),
    (Role::Deck3, "Deck 3"),
    (Role::Deck4, "Deck 4"),
    (Role::Headphones, "Headphones"),
    (Role::Sampler, "Sampler"),
];

/// Puts `rows` in `model`, changing only the rows that differ: replacing
/// the list would rebuild its rows and close a menu that is opening.
fn update_rows(model: &VecModel<OutputRow>, rows: Vec<OutputRow>) {
    if model.row_count() != rows.len() {
        model.set_vec(rows);
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        let old = model.row_data(i).unwrap();
        let same = old.role == row.role
            && old.name == row.name
            && old.device == row.device
            && old.device_name == row.device_name
            && old.pair == row.pair
            && old.single == row.single
            && old.status == row.status
            && old.ok == row.ok
            && old.pairs.iter().eq(row.pairs.iter());
        if !same {
            model.set_row_data(i, row);
        }
    }
}

/// The top bar's height in points, as in main.slint; the macOS title bar
/// is made this tall.
#[cfg(target_os = "macos")]
const TOP_BAR_HEIGHT: f64 = 52.0;

/// The 4-Deck layout's value of `layout` (0 is 2-Deck, 1 Player).
const FOUR_DECK_LAYOUT: i32 = 2;
/// Deck 1 alone, for preparing tracks (prep.slint).
pub(crate) const PREP_LAYOUT: i32 = 3;
/// The Player layout's value of `layout`: one bar over Automix.
const PLAYER_LAYOUT: i32 = 1;
/// Behind the Player bar's overview: the bar's own colour (Theme.panel),
/// so the waveform sits on the bar rather than in a black box.
const PLAYER_OVERVIEW_BACKGROUND: [f32; 4] = [0.102, 0.102, 0.110, 1.0];
/// Whether the Prep layout is offered (the layout menu's chooser, the Cues and
/// Checklist tabs, the checklist settings). Offered from v0.6, switched off at
/// first: the DJ switches it on in the layout menu.
pub(crate) const PREP_OFFERED: bool = true;
/// 2-Deck Basic, the lighter two-deck layout (lite.slint): no jogs or tempo
/// faders, the pads and loop in a bar under the decks.
pub(crate) const LITE_LAYOUT: i32 = 4;
/// Whether 2-Deck Basic is offered. Hidden for now while the two-deck
/// layouts are redone: the code stays, the menu, its chooser and
/// controllers leave it out, and a saved 2-Deck Basic opens 2-Deck.
pub(crate) const LITE_OFFERED: bool = false;
/// The last layout the menu and controllers can pick.
const LAST_LAYOUT: i32 = LITE_LAYOUT;

/// Whether `layout` is one the menu and controllers offer: Prep and
/// 2-Deck Basic only while they are offered.
fn layout_offered(layout: i32) -> bool {
    (0..=LAST_LAYOUT).contains(&layout)
        && (layout != PREP_LAYOUT || PREP_OFFERED)
        && (layout != LITE_LAYOUT || LITE_OFFERED)
}

/// `[ChannelN],live_input` for a deck playing the instrument plugin.
const INSTRUMENT_INPUT: f64 = 3.0;
/// Each deck's colour (Theme.deck1 to deck4 in theme.slint), as RGBA, for
/// the marks on a live deck's waveform.
const DECK_COLORS: [[f32; 4]; NUM_DECKS] = [
    [0.039, 0.518, 1.0, 1.0],
    [1.0, 0.271, 0.227, 1.0],
    [0.188, 0.820, 0.345, 1.0],
    [1.0, 0.839, 0.039, 1.0],
];

/// Seconds shown across a live deck's scrolling waveform (an input, an
/// instrument, the Groove Box), whose trace keeps about this much. Track
/// decks follow the shared zoom (waveform_zoom.rs), which starts here too.
const VISIBLE_SECONDS: f64 = waveform_zoom::DEFAULT_SECONDS;

/// Opens the window filling the screen when its preferred size (1680 x 1050
/// in main.slint) doesn't fit, as on a 1512 x 982 MacBook: left at that
/// size it would run off the bottom of the screen, under the Dock. Maximized
/// keeps clear of the menu bar and the Dock or taskbar.
fn fit_to_screen(app: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;
    app.window().with_winit_window(|w| {
        let Some(monitor) = w.current_monitor() else {
            return;
        };
        let screen = monitor.size().to_logical::<f64>(monitor.scale_factor());
        let window = w.outer_size().to_logical::<f64>(w.scale_factor());
        // Room for the menu bar and the Dock or taskbar.
        if window.width > screen.width || window.height > screen.height - 80.0 {
            w.set_maximized(true);
        }
    });
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--list-controls") {
        list_controls();
        return Ok(());
    }
    diagnostics::install();
    // Before any thread starts: Beat This! takes a few cores, not all.
    djsugar_engine::background::configure();
    // Windows wakes sleeping threads only every 15.6 ms unless asked for
    // 1 ms: the controller threads' 1 kHz scratch filter and their timers
    // need it, or a jog is heard in 16 ms steps.
    #[cfg(windows)]
    // SAFETY: a plain system call; it lasts until the process ends.
    unsafe {
        windows_sys::Win32::Media::timeBeginPeriod(1);
    }
    // A restore picked in Settings goes in before anything reads the
    // settings or opens the library.
    let restored =
        settings::config_dir().and_then(|d| backup::apply_pending(&d, backup::folder().as_deref()));
    // Before anything is saved: no settings yet means a first start, which
    // gets the guided tour.
    let first_run = settings::load().as_object().is_none_or(|m| m.is_empty());
    // On Android the platform is Slint's Android backend, set up by the
    // entry point before this (`slint::android::init`).
    #[cfg(not(target_os = "android"))]
    let backend =
        slint::BackendSelector::new().require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default());
    // On macOS the app draws under a hidden title bar: only
    // the traffic lights stay, over the top bar.
    #[cfg(target_os = "macos")]
    let backend = backend.with_winit_window_attributes_hook(|attributes| {
        use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
        attributes
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
    });
    #[cfg(not(target_os = "android"))]
    backend.select()?;
    // Wayland and X11 match the window to io.github.winnerspiros.DJSugar.desktop by
    // this, for the dock icon and name.
    let _ = slint::set_xdg_app_id("io.github.winnerspiros.DJSugar");
    // San Francisco on macOS; registered before any window.
    eprintln!("startup: settings read");
    let ui_family = ui_font::install();
    eprintln!("startup: fonts ready");

    let registry = ControlRegistry::new();
    eprintln!("startup: starting the audio engine");
    let engine = Rc::new(AudioEngine::start(
        registry.clone(),
        &settings::audio_setup(),
    ));
    engine.recorder().configure(settings::recording());
    midi_clock::install(engine.midi_clock(), &registry);
    let recorder = engine.recorder().clone();
    let broadcast = broadcast::Broadcast::start(&registry, &engine);
    // The saved library, with the tracks it had last time; the built-in
    // samples until there is anything in it.
    eprintln!("startup: audio engine running");
    let library_path = settings::config_dir().map(|d| djsugar_library::default_path(&d));
    let db = library_path
        .as_deref()
        .and_then(|p| match Library::open(p) {
            Ok(db) => Some(db),
            Err(e) => {
                eprintln!("can't open the library at {}: {e}", p.display());
                None
            }
        });
    if let Some(db) = &db {
        // Tag groups made from the old numbered fields take their names.
        if let Err(e) = db.name_migrated_tag_groups(&settings::old_tag_field_names()) {
            eprintln!("can't name the tag groups: {e}");
        }
        settings::publish_tag_groups(db);
    }
    let saved: Vec<Arc<Track>> = db
        .as_ref()
        .and_then(|db| db.all_tracks(Path::exists).ok())
        .unwrap_or_default()
        .into_iter()
        .map(Arc::new)
        .collect();
    let samples = saved.is_empty();
    let library = if samples { sample_library() } else { saved };
    eprintln!("startup: engine and library ready");
    let app = AppWindow::new()?;
    eprintln!("startup: window made");
    {
        use slint::winit_030::{EventResult, WinitWindowAccessor};
        #[cfg(target_os = "macos")]
        app.set_traffic_light_inset(76.0);
        // Audio files dragged in from Finder or a file manager load on the
        // deck they're dropped on.
        let file_drop = file_drop::FileDrop::new(&app);
        // An image dropped on a deck playing an instrument becomes its art.
        let dropped = file_drop.files();
        instrument_art::install(&app, move || std::mem::take(&mut *dropped.borrow_mut()));
        // Dark, Light or Match system, as last picked.
        appearance::install(&app);
        let theme_app = app.as_weak();
        app.window().on_winit_window_event(move |window, event| {
            file_drop.window_event(window, event);
            if let slint::winit_030::winit::event::WindowEvent::ThemeChanged(theme) = event {
                if let Some(app) = theme_app.upgrade() {
                    appearance::system_changed(&app, *theme);
                }
            }
            // The traffic lights, centred in the top bar after anything that
            // might have moved them back.
            #[cfg(target_os = "macos")]
            {
                use slint::winit_030::winit::event::WindowEvent;
                if !matches!(
                    event,
                    WindowEvent::CursorMoved { .. } | WindowEvent::RedrawRequested
                ) {
                    window.with_winit_window(|w| {
                        mac_titlebar::center_traffic_lights(w, TOP_BAR_HEIGHT)
                    });
                }
            }
            EventResult::Propagate
        });
    }
    // 2-Deck, Player, 4-Deck, Prep or 2-Deck Basic, as last picked.
    app.global::<PrepUi>().set_offered(PREP_OFFERED);
    // A layout no longer offered (Prep or 2-Deck Basic, while hidden) opens
    // 2-Deck.
    let layout = settings::number("layout", 0.0) as i32;
    app.set_layout(if layout_offered(layout) { layout } else { 0 });
    // `[Skin],layout` lets controllers and the tour set it by number: 0
    // 2-Deck, 1 Player, 2 4-Deck, 3 Prep, 4 2-Deck Basic. Pressed as a
    // toggle, it goes between 2-Deck and Player.
    let layout_control = registry.create(
        "[Skin]",
        "layout",
        app.get_layout() as f64,
        0.0,
        LAST_LAYOUT as f64,
        djsugar_control::Behavior::Toggle,
    );
    // VIEW (the DDJ-FLX4's SHIFT + right LOAD): the next of the DJ's
    // layouts (layouts.rs), round to the first.
    let layout_next = registry.push("[Skin]", "layout_next");
    // The deck 3 and 4 switches for two-deck controllers (see
    // djsugar_controllers::LAYER_KEYS): clicking a strip's deck number in
    // 4-Deck moves that side over. Outside 4-Deck they go back to decks 1
    // and 2, which are the ones showing.
    let layers = djsugar_controllers::LAYER_KEYS
        .map(|k| registry.toggle(djsugar_controllers::LAYER_GROUP, k));
    let reset_layers = move |layout: i32| {
        if layout != FOUR_DECK_LAYOUT {
            layers.iter().for_each(|l| l.set_bool(false));
        }
    };
    reset_layers(app.get_layout());
    // The large waveforms, shown or hidden from the top bar.
    app.set_waveforms_visible(settings::flag("waveforms_visible", true));
    app.on_waveforms_visible_changed(|on| {
        settings::set("waveforms_visible", serde_json::json!(on))
    });
    // The FX bar, likewise.
    app.set_fx_bar_visible(settings::flag("fx_bar_visible", true));
    app.on_fx_bar_visible_changed(|on| settings::set("fx_bar_visible", serde_json::json!(on)));
    // The pad grid in the decks (the PADS button), likewise.
    app.set_pads_visible(settings::flag("pads_visible", true));
    app.on_pads_visible_changed(|on| settings::set("pads_visible", serde_json::json!(on)));
    app.set_jogs_visible(settings::flag("lite_jogs", false));
    app.on_jogs_visible_changed(|on| settings::set("lite_jogs", serde_json::json!(on)));
    // The Groove Boxes: their kits' names, and what they held last time,
    // saved again as it changes.
    groove_box::install(&app);
    instruments::install(&app);
    mixer_picker::install(&app, engine.clone());
    presets::install(&app, engine.clone(), registry.clone());
    instrument_knobs::install(&app, engine.clone());
    let groove_keeper = RefCell::new(groove_box::Keeper::new(&registry));
    // The Mixer's channels as they were left, saved again as they change.
    let mixer_keeper = RefCell::new(mixer_keeper::Keeper::new(&registry));
    // The large waveforms' zoom, from the window and controllers.
    let _zoom_timer = waveform_zoom::install(&app, &registry);
    // How tall they are, per layout, likewise.
    let _height_timer = waveform_height::install(&app, &registry);
    let groove_timer = slint::Timer::default();
    {
        let app = app.as_weak();
        groove_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
            move || {
                groove_keeper.borrow_mut().tick();
                mixer_keeper.borrow_mut().tick();
                // Each deck's preset name, as decks change what they play.
                if let Some(app) = app.upgrade() {
                    presets::show(&app);
                }
            },
        );
    }
    // The beat counter in the deck headers: off, beats, or beats and
    // phrases, picked in Settings › Decks.
    {
        let counter = app.global::<BeatCounter>();
        counter.set_mode(beat_counter_mode());
        track_end::load();
        wave_look::load();
        wave_look::install(&app);
        let app_weak = app.as_weak();
        vocal_pass::start(move || {
            let _ = app_weak.upgrade_in_event_loop(|app| app.window().request_redraw());
        });
        let app_weak = app.as_weak();
        stem_levels::start(move || {
            let _ = app_weak.upgrade_in_event_loop(|app| app.window().request_redraw());
        });
        counter.set_decks(ModelRc::new(VecModel::from(vec![
            BeatCount::default();
            NUM_DECKS
        ])));
    }
    {
        use slint::winit_030::WinitWindowAccessor;
        let weak = app.as_weak();
        app.on_drag_window(move || {
            if let Some(app) = weak.upgrade() {
                app.window().with_winit_window(|w| w.drag_window().ok());
            }
        });
        let weak = app.as_weak();
        app.on_zoom_window(move || {
            if let Some(app) = weak.upgrade() {
                app.window()
                    .with_winit_window(|w| w.set_maximized(!w.is_maximized()));
            }
        });
    }
    if let Some(family) = ui_family {
        app.global::<Theme>().set_font_family(family.into());
    }
    install_audio_settings(&app, engine.clone());

    let controls_version = bind_controls(&app, registry.clone());
    app.global::<EffectList>()
        .set_names(ModelRc::new(VecModel::from(
            AVAILABLE_EFFECTS
                .iter()
                .map(|&n| SharedString::from(n))
                .collect::<Vec<_>>(),
        )));
    app.global::<EffectList>()
        .set_color_fx(ModelRc::new(VecModel::from(
            COLOR_FX
                .iter()
                .map(|&n| SharedString::from(n))
                .collect::<Vec<_>>(),
        )));
    app.global::<EffectList>()
        .set_mic_unit(djsugar_engine::MIC_FX_UNIT as i32);
    app.global::<EffectList>()
        .set_builtin_rows(ModelRc::new(VecModel::from(audio_plugins::builtin_rows())));
    // Plugins the DJ ticked join the FX lists after the built-in effects.
    let audio_plugins = Rc::new(RefCell::new(audio_plugins::AudioPlugins::new(
        &app,
        engine.clone(),
    )));
    {
        let engine = engine.clone();
        app.global::<DeckActions>()
            .on_prepare_hq_stems(move |deck| {
                let loaded = engine.loaded_tracks();
                let track = loaded.read().unwrap().get(deck as usize).cloned().flatten();
                if let Some(t) = track {
                    engine.prepare_hq_stems(&t);
                }
            });
    }
    let browser = browser::Browser::install(
        &app,
        library.clone(),
        db,
        library_path.clone(),
        engine.clone(),
        &registry,
    );
    // Opening Prep shows the In Prep list (the layout menu; a controller's
    // VIEW is handled on the timer below).
    {
        let layout_control = layout_control.clone();
        let reset_layers = reset_layers.clone();
        let browser = browser.clone();
        let app_weak = app.as_weak();
        app.on_layout_changed(move |layout| {
            layout_control.set(layout as f64);
            settings::set("layout", serde_json::json!(layout));
            reset_layers(layout);
            if layout == PREP_LAYOUT {
                browser.borrow_mut().show_in_prep();
            }
            if let Some(app) = app_weak.upgrade() {
                layouts::refresh(&app);
            }
        });
    }
    // The layout menu's chooser.
    {
        let app_weak = app.as_weak();
        layouts::install(&app, move |layout| {
            if let Some(app) = app_weak.upgrade() {
                if app.get_layout() != layout && layout_offered(layout) {
                    app.set_layout(layout);
                    app.invoke_layout_changed(layout);
                }
            }
        });
    }
    let sources = sources::Sources::install(&app, library_path.clone());
    {
        // Spotify's "Open in Browser" and "Add to Downloads", the latter
        // when an add-on source searches Spotify links.
        const SPOTIFY_SITE: &str = "open.spotify.com";
        let ui = app.global::<StoreUi>();
        let to = sources
            .borrow()
            .takes_links(SPOTIFY_SITE)
            .unwrap_or_default();
        ui.set_downloads_to(to.into());
        let b = browser.clone();
        ui.on_open_playlist(move |i| {
            if let Some(link) = b.borrow().spotify_playlist_link(i as usize) {
                spotify::open_in_browser(&link);
            }
        });
        // "Download Track": a track search for each.
        let queue_tracks = {
            let (b, s) = (browser.clone(), sources.clone());
            move |tracks: Vec<String>| {
                let n = tracks.len();
                for text in tracks {
                    s.borrow_mut().add_download(
                        SPOTIFY_SITE,
                        SourceQuery {
                            text,
                            kind: QueryKind::Track,
                            ..Default::default()
                        },
                    );
                }
                if n > 0 {
                    let what = if n == 1 {
                        "the track".to_string()
                    } else {
                        format!("{n} tracks")
                    };
                    b.borrow_mut()
                        .wanted_note(format!("Downloading {what} in {to}."));
                }
            }
        };
        let queue = Rc::new(queue_tracks);
        let (b, s, q) = (browser.clone(), sources.clone(), queue.clone());
        ui.on_download_playlist(move |i| {
            let names = b.borrow_mut().list_download_names(i as usize);
            if names.is_empty() {
                return;
            }
            // A playlist search with the songs DJ Sugar read, so the source
            // needs no Spotify sign-in of its own, and downloads go into a
            // folder named after the list.
            let Some(name) = b.borrow().list_name(i as usize) else {
                return q(names);
            };
            s.borrow_mut().add_download(
                SPOTIFY_SITE,
                SourceQuery {
                    text: name,
                    kind: QueryKind::Playlist,
                    songs: names,
                    ..Default::default()
                },
            );
            b.borrow_mut()
                .wanted_note(format!("Downloading the playlist in {to}."));
        });
        let (b, q) = (browser.clone(), queue.clone());
        ui.on_download_tracks(move |first, last| {
            let names = b
                .borrow()
                .download_names(first.max(0) as usize, last.max(0) as usize);
            q(names);
        });
    }
    let sampler = sampler::Sampler::install(&app, engine.clone(), browser.clone(), &registry);
    let automix = Rc::new(RefCell::new(automix::AutoMix::new(&registry)));
    {
        let a = automix.clone();
        app.on_automix_toggle(move || a.borrow_mut().toggle());
        let a = automix.clone();
        app.on_automix_mix_now(move || a.borrow_mut().mix_now());
        let a = automix.clone();
        app.on_automix_cut_now(move || a.borrow_mut().cut_now());
        let a = automix.clone();
        app.on_automix_skip(move || a.borrow_mut().skip());
        let a = automix.clone();
        app.on_automix_set_repeat(move |on| a.borrow_mut().set_repeat(on));
        let a = automix.clone();
        app.on_automix_set_length(move |i| a.borrow_mut().set_length(i.max(0) as usize));
        let a = automix.clone();
        app.on_automix_set_style(move |i| a.borrow_mut().set_style(i.max(0) as usize));
        let a = automix.clone();
        let b = browser.clone();
        let weak = app.as_weak();
        app.on_automix_order(move |i| {
            let Some(&order) = automix_order::ORDERS.get(i.max(0) as usize) else {
                return;
            };
            let (Ok(mut b), Some(app)) = (b.try_borrow_mut(), weak.upgrade()) else {
                return;
            };
            let note = b.order_automix_for(&a.borrow(), order);
            app.set_automix_order_note(note.into());
        });
        let b = browser.clone();
        app.on_automix_order_undo(move || {
            if let Ok(mut b) = b.try_borrow_mut() {
                b.undo_automix_order();
            }
        });
    }

    // The Player layout's search box in the top bar.
    let quick = Rc::new(RefCell::new(quick_search::Quick::install(&app, &browser)));

    // The phone remote: REMOTE in the top bar.
    let remote = Rc::new(RefCell::new(remote::Remote::new(&registry)));
    remote::Remote::install(&remote, &app);

    // Keyboard shortcuts, each an DJSugarScript line.
    {
        let browser = browser.clone();
        let registry = registry.clone();
        app.on_shortcut(move |text, cmd, alt, shift, master, first, last| {
            let Some(key) = shortcuts::key_name(&text, cmd, alt, shift) else {
                return false;
            };
            let Some(script) = shortcuts::script_for(&key) else {
                return false;
            };
            let mut b = browser.borrow_mut();
            let mut host = shortcuts::AppHost {
                registry: &registry,
                browser: &mut b,
                picked: (first, last),
                master: master.clamp(0, 3) as u8 + 1,
            };
            if let Err(e) = djsugar_script::run(&script, &mut host) {
                eprintln!("{key} ({script}): {e}");
            }
            true
        });
    }

    // MIDI controllers, driven by controller mappings. Started after the browser
    // so the library controls the mappings use already exist.
    for key in settings::controllers_scaled() {
        djsugar_controllers::pickup::set_scale(&key, true);
    }
    djsugar_controllers::routing::load_deck_choices(settings::controller_decks());
    let controllers = match djsugar_controllers::default_mappings_dir() {
        Some(dir) => Some(Rc::new(djsugar_controllers::Controllers::start(
            registry.clone(),
            dir,
            settings::controller_mappings(),
            settings::controller_versions(),
        ))),
        None => {
            eprintln!("controller mappings not found; MIDI controllers are disabled");
            None
        }
    };
    // MIDI keyboards (devices with no mapping) play the Instrument input.
    if let Some(c) = &controllers {
        let notes = engine.note_input();
        c.set_note_sink(Some(std::sync::Arc::new(move |data| {
            if let Ok(mut n) = notes.lock() {
                n.send(data);
            }
        })));
    }

    // Start with two sample tracks loaded so the screen is not empty, but
    // leave a deck kept as a Groove Box as it is.
    if samples {
        for (deck, track) in [(0, 0), (1, 4)] {
            let groove = registry
                .get(&djsugar_engine::groove::groove_group(deck), "enabled")
                .is_some_and(|c| c.get_bool());
            if !groove {
                engine.load_track(deck, library[track].clone());
            }
        }
    }

    // Bring the library up to date in the background with new and changed
    // files in the music folders, every start. The browser shows the library
    // as saved meanwhile.
    let (scan_tx, scan_rx) = std::sync::mpsc::channel();
    sync_library(library_path.clone(), scan_tx.clone());
    let controller_editor = controller_editor::Editor::install(&app, controllers.clone());
    let map_mode = map_mode::Mapper::install(&app, controllers.clone());
    prefs::apply_saved(&registry);
    let _levels = prefs::remember_levels(registry.clone());
    if let Some(note) = restored {
        engine.alert(note);
    }
    // Which MIDI keyboard plays the instrument: shown on the instrument
    // deck, and said once when a keyboard plays it for the first time, so a
    // device that shouldn't is easy to spot (Settings › Controllers turns it
    // off or gives it a deck).
    let _midi_from = instrument_midi::install(&app, controllers.clone());
    // The instrument's notes and scope, for the deck playing it.
    let instrument_timer = slint::Timer::default();
    {
        let (weak, view) = (app.as_weak(), engine.instrument_view());
        let mut last_held = u128::MAX;
        let mut quiet = false;
        instrument_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(40),
            move || {
                let Some(app) = weak.upgrade() else {
                    return;
                };
                let ui = app.global::<InstrumentsUi>();
                let held = view.held();
                if held != last_held {
                    last_held = held;
                    let keys: Vec<bool> = (36..84).map(|n| held >> n & 1 == 1).collect();
                    ui.set_held(slint::ModelRc::from(keys.as_slice()));
                }
                let scope = view.scope();
                let silent = scope.iter().all(|&(lo, hi)| lo == 0.0 && hi == 0.0);
                if silent && quiet {
                    return;
                }
                quiet = silent;
                ui.set_scope(scope_path(&scope).into());
            },
        );
    }
    let players_timer = slint::Timer::default();
    {
        let (weak, engine) = (app.as_weak(), engine.clone());
        let mut param_names: Vec<String> = Vec::new();
        let mut knobs = Default::default();
        players_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
            move || {
                for port in djsugar_controllers::routing::take_new_players() {
                    engine.alert(format!(
                        "{} plays the instrument. Settings › Controllers changes that.",
                        short_port_name(&port)
                    ));
                }
                let Some(app) = weak.upgrade() else {
                    return;
                };
                let name = djsugar_controllers::routing::last_player()
                    .map(|(p, _)| short_port_name(&p).to_string())
                    .unwrap_or_default();
                let ui = app.global::<InstrumentsUi>();
                if ui.get_playing_from().as_str() != name {
                    ui.set_playing_from(name.into());
                }
                let names = engine.instrument_parameters();
                if names != param_names {
                    let model: Vec<slint::SharedString> =
                        names.iter().map(|n| n.as_str().into()).collect();
                    ui.set_param_names(slint::ModelRc::from(model.as_slice()));
                    param_names = names;
                }
                instrument_knobs::show(&app, &engine, &mut knobs);
            },
        );
    }
    // Automatic backups: at start, then checked every hour.
    backup::run_if_due();
    let backup_timer = slint::Timer::default();
    backup_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(3600),
        backup::run_if_due,
    );
    instruments::show_routes(&app, &engine.input_status(), &engine.status());
    let prefs = prefs::Prefs::install(
        &app,
        registry.clone(),
        controllers.clone(),
        engine.recorder().clone(),
        Box::new(move || sync_library(library_path.clone(), scan_tx.clone())),
        {
            let sources = sources.clone();
            Box::new(move |id| sources.borrow_mut().configure(id))
        },
        {
            let engine = engine.clone();
            let weak = app.as_weak();
            Box::new(move || {
                engine.set_outputs(&settings::audio_setup());
                // The main output's interface sets which strips run on
                // their own clock.
                if let Some(app) = weak.upgrade() {
                    instruments::show_routes(&app, &engine.input_status(), &engine.status());
                }
            })
        },
        prefs::InputHooks {
            changed: {
                let engine = engine.clone();
                let weak = app.as_weak();
                Box::new(move || {
                    engine.set_inputs(&settings::audio_setup());
                    if let Some(app) = weak.upgrade() {
                        instruments::show_routes(&app, &engine.input_status(), &engine.status());
                    }
                })
            },
            status: {
                let engine = engine.clone();
                Box::new(move || engine.input_status())
            },
        },
        audio_plugins.clone(),
        broadcast.clone(),
    );
    // The circling arrows over the Collection sidebar update the collection,
    // as Update collection did in Settings.
    {
        let weak = app.as_weak();
        app.global::<LibraryEdit>().on_update_collection(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<Preferences>().invoke_pressed("rescan".into());
            }
        });
    }
    // OSC for tablets, visuals and show software: Settings › Controllers.
    osc::install(&registry, engine.loaded_tracks(), engine.visual_positions());
    let _osc_timer = {
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(500),
            osc::tick,
        );
        timer
    };
    // For the system's Now Playing (installed with the mini player below).
    let media_tracks = engine.loaded_tracks();
    // For Player's Now Playing view (installed with the mini player below).
    let view_tracks = engine.loaded_tracks();
    // For the Player bar's sleep timer (installed with the mini player below).
    let sleep_tracks = engine.loaded_tracks();
    // The now-playing files and overlay follow the mix a few times a second.
    let _now_playing_timer = {
        let mut feed = now_playing::NowPlaying::new(registry.clone(), engine.loaded_tracks());
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(250),
            move || feed.tick(),
        );
        timer
    };
    // Scrobbling follows the mix once a second.
    let _scrobble_timer = {
        let mut scrobbler = scrobble::Scrobbler::new(registry.clone(), engine.loaded_tracks());
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_secs(1),
            move || scrobbler.tick(),
        );
        timer
    };
    // The ON AIR panel and saving typed passwords, a few times a second.
    let _broadcast_timer = broadcast.clone().map(|b| {
        let app = app.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(250),
            move || {
                if let Some(app) = app.upgrade() {
                    b.poll(&app);
                }
            },
        );
        timer
    });

    let tour = tour::Tour::install(
        &app,
        registry.clone(),
        engine.clone(),
        controllers.clone(),
        prefs.clone(),
        first_run,
    );

    let decks = Rc::new(Decks::new(&registry));
    let loaded = engine.loaded_tracks();
    let visual = engine.visual_positions();

    // The switch on an instrument or Groove Box deck's waveform between its
    // notes or steps and its knobs as macro lanes.
    {
        let app_weak = app.as_weak();
        app.global::<MacroLanes>().on_toggle(move |deck| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let ui = app.global::<MacroLanes>();
            let mut shown: Vec<bool> = ui.get_shown().iter().collect();
            if let Some(s) = shown.get_mut(deck.max(0) as usize) {
                *s = !*s;
            }
            ui.set_shown(ModelRc::new(VecModel::from(shown)));
            app.window().request_redraw();
        });
    }

    // Capture Slint's wgpu device once it exists; all waveform rendering uses
    // it. Waveforms are drawn just before Slint renders each frame, so their
    // play position is as close as possible to when the frame is shown.
    let gpu: Rc<RefCell<Option<Gpu>>> = Rc::new(RefCell::new(None));
    {
        let gpu = gpu.clone();
        let app_weak = app.as_weak();
        let decks = decks.clone();
        let loaded = loaded.clone();
        let mut live = engine.take_live_waves().map(live_trace::LiveTraces::new);
        let instrument = engine.instrument_view();
        let mut steps = engine.take_groove_hits().map(step_lanes::StepLanes::new);
        let mut grooves = Some(std::array::from_fn(|d| {
            step_lanes::GrooveHandles::new(&registry, d)
        }));
        let mut knobs = Some(macro_lanes::KnobHandles::new(&registry));
        let scale_mask = registry
            .get("[Instrument1]", "scale_mask")
            .expect("the instrument's scale mask");
        app.window()
            .set_rendering_notifier(move |state, api| match (state, api) {
                (
                    slint::RenderingState::RenderingSetup,
                    slint::GraphicsAPI::WGPU30 { device, queue, .. },
                ) => {
                    let renderer = WaveformRenderer::new(device, queue);
                    let make = |mode| WaveformView::new(device, mode, 1, 1);
                    *gpu.borrow_mut() = Some(Gpu {
                        device: device.clone(),
                        queue: queue.clone(),
                        renderer,
                        scrolling: std::array::from_fn(|_| make(Mode::Scrolling)),
                        overview: std::array::from_fn(|_| make(Mode::Overview)),
                        tracks: Default::default(),
                        phrases: Default::default(),
                        clocks: Default::default(),
                        live: live.take(),
                        instrument: instrument.clone(),
                        steps: steps.take(),
                        grooves: grooves.take().expect("groove handles"),
                        knobs: knobs.take().expect("knob handles"),
                        scale_mask: scale_mask.clone(),
                        macros: Default::default(),
                        knobs_moved: Default::default(),
                        live_tracks: Default::default(),
                        moving: false,
                        images_set: false,
                        look: 0,
                    });
                }
                (slint::RenderingState::BeforeRendering, _) => {
                    let mut gpu = gpu.borrow_mut();
                    let (Some(app), Some(gpu)) = (app_weak.upgrade(), gpu.as_mut()) else {
                        return;
                    };
                    let tracks = loaded.read().unwrap().clone();
                    gpu.render(&app, &decks, &tracks, &visual);
                }
                (slint::RenderingState::AfterRendering, _) => {
                    // While a deck moves, draw again as soon as the display
                    // can show it, rather than on the UI timer, which beats
                    // against the display's refresh and makes motion uneven.
                    let moving = gpu.borrow().as_ref().is_some_and(|g| g.moving);
                    if let (true, Some(app)) = (moving, app_weak.upgrade()) {
                        app.window().request_redraw();
                    }
                }
                _ => {}
            })?;
    }

    // The overview's tooltip: the phrase under the pointer.
    {
        let gpu = gpu.clone();
        app.global::<PhraseView>().on_describe(move |d, seconds| {
            let gpu = gpu.try_borrow().ok();
            let cached = gpu
                .as_ref()
                .and_then(|g| g.as_ref()?.phrases.get(d as usize)?.as_ref());
            cached
                .and_then(|(t, p)| phrase_tip(p, t.duration, t.bpm, seconds as f64))
                .unwrap_or_default()
        });
    }

    // A newer release: checked once per start, offered while nothing plays.
    update::install_model_fetch();
    let update_check = update::check();
    let update: Rc<RefCell<Option<update::Release>>> = Default::default();
    let installing: Rc<RefCell<Option<std::sync::mpsc::Receiver<update::Progress>>>> =
        Default::default();
    // Installed and waiting for the decks to stop before restarting.
    let update_ready = Cell::new(false);
    let play_controls: Vec<ControlHandle> = (0..NUM_DECKS)
        .filter_map(|d| registry.get(&deck_group(d), "play"))
        .collect();
    {
        let update = update.clone();
        let installing = installing.clone();
        let weak = app.as_weak();
        app.on_update_download(move || {
            let Some(release) = update.borrow().clone() else {
                return;
            };
            if let Some(package) = &release.package {
                spotify::open_in_browser(&package.url);
            } else if release.asset.is_none() {
                spotify::open_in_browser(&release.url);
            } else if installing.borrow().is_none() {
                if let Some(app) = weak.upgrade() {
                    app.set_update_status("Starting…".into());
                }
                *installing.borrow_mut() = Some(update::install(release));
            }
        });
    }
    {
        let update = update.clone();
        let weak = app.as_weak();
        app.on_update_dismiss(move || {
            if let Some(r) = update.borrow_mut().take() {
                update::dismiss(&r.version);
            }
            if let Some(app) = weak.upgrade() {
                app.set_update_version("".into());
            }
        });
    }

    let start = Instant::now();
    let timer = slint::Timer::default();
    let covers: RefCell<[CoverImage; NUM_DECKS]> = Default::default();
    let qualities: RefCell<quality::Qualities> = Default::default();
    let last_tick = Cell::new(Instant::now());
    // When the alert on screen goes away.
    let alert_until: Cell<Option<Instant>> = Cell::new(None);
    let alert_engine = engine.clone();
    let layout_version = Cell::new(layout_control.version());
    let layout_next_version = Cell::new(layout_next.version());
    // Whether the In Prep list was shown for a start in the Prep layout.
    let prep_shown = Cell::new(false);
    // Prep's metronome, on while its grid is open.
    let prep_click = RefCell::new(prep_click::PrepClick::new(&registry, "[Channel1]"));
    let put_back = Rc::new(RefCell::new(put_back::PutBack::new(&registry)));
    {
        let put_back = put_back.clone();
        let engine = engine.clone();
        app.global::<DeckActions>().on_put_back(move |deck| {
            put_back
                .borrow_mut()
                .put_back(deck.max(0) as usize, &engine);
        });
    }
    // Switched off or on from the crossfader's menu, a controller or a
    // script: remembered for next time, as the Settings switch is.
    let xfader_enabled = registry.create(
        "[Mixer]",
        "crossfader_enabled",
        1.0,
        0.0,
        1.0,
        djsugar_control::Behavior::Toggle,
    );
    let xfader_version = Cell::new(xfader_enabled.version());
    // The big browser (the button above the browser), also from a controller
    // using our `[Skin],show_maximized_library`.
    let big_browser = registry.toggle("[Skin]", "show_maximized_library");
    let big_browser_version = Cell::new(big_browser.version());
    // What the screen last showed, to redraw only when something changed.
    let shown_controls = Cell::new(u64::MAX);
    let shown_tracks: RefCell<[Option<Arc<Track>>; NUM_DECKS]> = Default::default();
    let shown_positions: Cell<[f64; NUM_DECKS]> = Cell::new([f64::NAN; NUM_DECKS]);
    let deck_rows = Rc::new(VecModel::from(vec![DeckInfo::default(); NUM_DECKS]));
    app.set_decks(ModelRc::from(deck_rows.clone()));
    let positions = engine.visual_positions();
    // Recordings finished so far, to add the next one's files to Mixed Sets.
    let recordings_seen = Cell::new(recorder.finished().0);
    let finished_recorder = recorder.clone();
    let quitting_engine = engine.clone();
    {
        let app_weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                // The skin reads controls through `tick`; moving it makes
                // every one of them look again, so only when one changed.
                let version = controls_version();
                let controls_changed = shown_controls.replace(version) != version;
                if controls_changed {
                    app.global::<Controls>()
                        .set_tick(app.global::<Controls>().get_tick().wrapping_add(1));
                }
                app.global::<Controls>()
                    .set_blink(start.elapsed().as_millis() / 500 % 2 == 1);
                // VIEW: on to the next of the DJ's layouts, applied below.
                if layout_next_version.replace(layout_next.version()) != layout_next.version()
                    && layout_next.get_bool()
                {
                    layout_control.set(layouts::next(app.get_layout()) as f64);
                }
                // Only when a controller moved it since the last look.
                let layout = layout_control.get() as i32;
                if layout_version.replace(layout_control.version()) != layout_control.version()
                    && layout != app.get_layout()
                    && layout_offered(layout)
                {
                    app.set_layout(layout);
                    settings::set("layout", serde_json::json!(layout));
                    reset_layers(layout);
                    if layout == PREP_LAYOUT {
                        browser.borrow_mut().show_in_prep();
                    }
                    layouts::refresh(&app);
                }
                if big_browser_version.get() != big_browser.version() {
                    app.set_library_expanded(big_browser.get_bool());
                } else if big_browser.get_bool() != app.get_library_expanded() {
                    big_browser.set_bool(app.get_library_expanded());
                }
                big_browser_version.set(big_browser.version());
                if xfader_version.replace(xfader_enabled.version()) != xfader_enabled.version() {
                    let on = xfader_enabled.get_bool();
                    if settings::flag("xfader_enabled", true) != on {
                        settings::set("xfader_enabled", serde_json::json!(on));
                    }
                }
                // The newest alert shows for a while; a click closes it.
                if let Some(message) = alert_engine.take_alerts().pop() {
                    app.set_alert(message.into());
                    alert_until.set(Some(Instant::now() + ALERT_SECONDS));
                } else if alert_until.get().is_some_and(|t| Instant::now() >= t) {
                    alert_until.set(None);
                    app.set_alert("".into());
                }
                if let Ok(release) = update_check.try_recv() {
                    app.set_update_action(
                        if release.asset.is_some() {
                            "Install & restart".to_string()
                        } else if let Some(package) = &release.package {
                            format!("Download {}", package.kind)
                        } else {
                            "Download".to_string()
                        }
                        .into(),
                    );
                    *update.borrow_mut() = Some(release);
                }
                let progress = installing
                    .borrow()
                    .as_ref()
                    .and_then(|rx| rx.try_recv().ok());
                match progress {
                    Some(update::Progress::Status(s)) => app.set_update_status(s.into()),
                    Some(update::Progress::Done) => update_ready.set(true),
                    Some(update::Progress::Failed(message)) => {
                        installing.take();
                        // The button opens the release page from now on.
                        if let Some(r) = update.borrow_mut().as_mut() {
                            r.asset = None;
                        }
                        app.set_update_status("".into());
                        app.set_update_action("Download".into());
                        app.set_alert(message.into());
                        alert_until.set(Some(Instant::now() + ALERT_SECONDS));
                    }
                    None => {}
                }
                let playing = play_controls.iter().any(|c| c.get() > 0.5);
                djsugar_engine::background::set_playing(playing);
                if update_ready.get() {
                    if playing {
                        app.set_update_status("Restarts when the decks stop".into());
                    } else {
                        update_ready.set(false);
                        update::relaunch();
                        let _ = slint::quit_event_loop();
                    }
                }
                // Never pops up mid-set; an install under way stays in view.
                let shown = match &*update.borrow() {
                    Some(r) if !playing || installing.borrow().is_some() => r.version.clone(),
                    _ => String::new(),
                };
                if app.get_update_version() != shown.as_str() {
                    app.set_update_version(shown.into());
                }
                if let Ok(scanned) = scan_rx.try_recv() {
                    browser.borrow_mut().set_library(scanned);
                    // Started in Prep: the In Prep list, once the library is in.
                    if app.get_layout() == PREP_LAYOUT && !prep_shown.replace(true) {
                        browser.borrow_mut().show_in_prep();
                    }
                }
                browser.borrow_mut().poll_controllers(&app);
                // A recording that just stopped shows in Mixed Sets.
                let (finished, files) = finished_recorder.finished();
                if finished != recordings_seen.get() {
                    recordings_seen.set(finished);
                    browser.borrow_mut().add_recordings(&files);
                }
                browser.borrow_mut().poll_mix_export(&app);
                browser.borrow_mut().poll_streams();
                browser.borrow_mut().poll_stores(&app);
                for track in sources.borrow_mut().poll() {
                    browser.borrow_mut().add_imported(track);
                }
                for path in sources.borrow_mut().take_loads() {
                    browser.borrow_mut().load_cached(&path);
                }
                for action in sources.borrow_mut().take_preview_actions() {
                    browser.borrow().preview_download(action);
                }
                browser.borrow_mut().poll_art();
                browser.borrow_mut().poll_waves();
                browser.borrow_mut().poll_analysis();
                browser.borrow_mut().poll_usb_export();
                browser.borrow_mut().poll_playlist_files();
                browser.borrow_mut().poll_inbox_albums();
                browser.borrow_mut().poll_rekordbox_xml();
                browser.borrow_mut().poll_traktor_nml();
                browser.borrow_mut().poll_serato_export();
                browser.borrow_mut().poll_virtualdj_export();
                browser.borrow_mut().poll_mixxx_export();
                browser.borrow_mut().poll_engine_export();
                browser.borrow_mut().poll_rekordbox_db_export();
                browser.borrow_mut().poll_itunes_xml();
                browser.borrow_mut().poll_sync_export();
                browser.borrow_mut().poll_covers();
                browser.borrow_mut().poll_located();
                browser.borrow_mut().poll_cleanup();
                browser.borrow_mut().poll_files();
                sampler.borrow_mut().poll();
                prefs.borrow_mut().poll();
                tour.borrow_mut().poll();
                controller_editor.borrow_mut().poll();
                map_mode.borrow_mut().poll();
                prep_click
                    .borrow_mut()
                    .tick(app.get_layout() == PREP_LAYOUT);
                if let Some(c) = &controllers {
                    let names: Vec<String> = c.connected().into_iter().map(|c| c.port).collect();
                    let status = if names.is_empty() {
                        "None".to_string()
                    } else {
                        names
                            .iter()
                            .map(|n| short_port_name(n))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    };
                    let settings = app.global::<AudioSettings>();
                    if settings.get_controllers() != status.as_str() {
                        settings.set_controllers(status.into());
                    }
                }
                let tracks = loaded.read().unwrap().clone();
                put_back.borrow_mut().tick(&app, &tracks);
                let camelot = app.global::<KeyNotation>().get_camelot();
                let mut b = browser.borrow_mut();
                b.set_camelot(camelot);
                b.set_decks_shown(if app.get_layout() == FOUR_DECK_LAYOUT {
                    4
                } else {
                    2
                });
                b.set_loaded(
                    &app,
                    std::array::from_fn(|d| tracks[d].as_ref().map(|t| t.id)),
                );
                b.learn_deck_phrases(&tracks);
                b.learn_analysis(&tracks);
                b.poll_prep();
                b.keep(&tracks, 0.016);
                let mut host = browser::AutomixHost {
                    browser: &mut b,
                    engine: &engine,
                };
                let now = Instant::now();
                let dt = now.duration_since(last_tick.replace(now)).as_secs_f64();
                if host.browser.take_album_request() {
                    automix.borrow_mut().play_album();
                }
                let player = app.get_layout() == PLAYER_LAYOUT;
                let shown = app.get_player_deck().clamp(0, NUM_DECKS as i32 - 1) as usize;
                let playing = tracks[shown].as_ref().map(|t| t.id);
                host.browser
                    .keep_going_tick(player, &automix.borrow(), playing);
                if let Some(mixes) = host.browser.take_mixes() {
                    automix.borrow_mut().set_overrides(mixes);
                }
                if host.browser.take_keep_going_automix() {
                    automix.borrow_mut().set_enabled(true);
                }
                automix.borrow_mut().tick(&mut host, &tracks, dt.min(0.1));
                remote.borrow_mut().tick(
                    &app,
                    &mut automix.borrow_mut(),
                    &mut b,
                    &tracks,
                    dt.min(0.1),
                );
                quick
                    .borrow_mut()
                    .tick(&app, &mut automix.borrow_mut(), &mut b, &tracks);
                automix.borrow().show(&app, &tracks);
                let automix_spans = automix.borrow().spans(&tracks);
                drop(b);
                // Row by row and only when it differs: a whole new model
                // would make everything on the decks lay out and draw again.
                for (d, track) in tracks.iter().enumerate() {
                    let track = track.as_deref();
                    let mut info = decks.info(d, track, camelot);
                    if let Some(image) = covers.borrow_mut()[d].get(track) {
                        info.cover = image;
                        info.has_cover = true;
                    }
                    let (tag, lossless) = qualities.borrow_mut().get(d, track);
                    info.quality = tag.into();
                    info.lossless = lossless;
                    // Automix's next mix, shaded on the overview. Rounded so
                    // small tempo changes don't lay the deck out again.
                    if let Some((from, to)) = automix_spans[d] {
                        let round = |f: f64| ((f * 2000.0).round() / 2000.0) as f32;
                        info.automix_from = round(from);
                        info.automix_to = round(to);
                    }
                    if !deck_rows
                        .row_data(d)
                        .is_some_and(|old| same_deck_info(&old, &info))
                    {
                        deck_rows.set_row_data(d, info);
                    }
                }
                let t = start.elapsed().as_secs();
                let clock = format!("{:02}:{:02}", t / 60 % 60, t % 60);
                if app.get_clock() != clock.as_str() {
                    app.set_clock(clock.into());
                }
                // The waveforms are drawn outside Slint, so it can't tell
                // when they change: redraw when a deck moved, a track or its
                // waveform changed, or a control did (cues, loops, zoom).
                // While a deck plays, each frame asks for the next itself.
                let now = Instant::now();
                let at: [f64; NUM_DECKS] = std::array::from_fn(|d| positions.position(d, now).0);
                let moved = at != shown_positions.replace(at);
                let mut shown = shown_tracks.borrow_mut();
                let loaded_changed = shown.iter().zip(tracks.iter()).any(|(a, b)| match (a, b) {
                    (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                    (None, None) => false,
                    _ => true,
                });
                if loaded_changed {
                    *shown = tracks.clone();
                }
                if controls_changed || moved || loaded_changed {
                    app.window().request_redraw();
                }
            },
        );
    }

    // File and Edit: the ☰ menu on Windows and Linux, and the native menu
    // bar on macOS. That one only exists once the event loop runs, and
    // Slint swaps in a fresh one for each window that comes to the front,
    // so keep filling whichever is showing.
    app_menu::install(&app);
    mini_player::install(&app);
    let _sleep_timer = sleep_timer::install(&app, registry.clone(), sleep_tracks);
    // Media keys and the system's Now Playing follow the Player bar.
    let _media_controls = media_controls::install(&app, media_tracks);
    let _now_playing_view = now_playing_view::install(&app, view_tracks);
    #[cfg(target_os = "macos")]
    let _mac_menu = {
        let weak = app.as_weak();
        mac_menu::on_action(move |action| {
            if let Some(app) = weak.upgrade() {
                app_menu::run(&app, action);
            }
        });
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            mac_menu::install,
        );
        timer
    };

    app.show()?;
    fit_to_screen(&app);
    // Once, when the first frame is on screen (the Android launch check
    // waits for this line).
    let started = std::time::Instant::now();
    app.window()
        .set_rendering_notifier(move |state, _| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                static ONCE: std::sync::Once = std::sync::Once::new();
                ONCE.call_once(|| {
                    eprintln!("startup: first frame drawn ({:.1?})", started.elapsed())
                });
            }
        })
        .ok();
    app.run()?;
    // Keeps each plugin's sound (tweaks made in its window) for next time:
    // the engine isn't always dropped on the way out.
    quitting_engine.shut_down_plugins();
    // Finishes the files, with the cue sheet, if the DJ quits mid-recording.
    recorder.stop();
    if let Some(b) = &broadcast {
        b.shut_down();
    }
    Ok(())
}

/// Brings the library up to date in the background: new and changed files
/// in the music folders, and the library of any app the DJ imports from whose
/// files have changed since last time. The tracks as saved are then sent to `tx`,
/// for the browser to show.
fn sync_library(library_path: Option<PathBuf>, tx: std::sync::mpsc::Sender<Vec<Arc<Track>>>) {
    let music: Vec<PathBuf> = settings::music_folders()
        .into_iter()
        .filter(|d| d.is_dir())
        .collect();
    let folders = settings::stage_folders();
    // The music folders, the stage folders (the albums folder with them)
    // and the mixed sets folder, each folder once: one inside another
    // (Downloads/Soulseek, a stage folder in a music folder) is scanned
    // with it.
    let mut wanted: Vec<PathBuf> = Vec::new();
    for dir in music.iter().cloned().chain(
        folders
            .all()
            .into_iter()
            .map(Path::to_path_buf)
            .chain(settings::mixed_sets_folder())
            .filter(|d| d.is_dir()),
    ) {
        if !wanted.contains(&dir) {
            wanted.push(dir);
        }
    }
    let dirs: Vec<PathBuf> = wanted
        .iter()
        .filter(|dir| !wanted.iter().any(|d| d != *dir && dir.starts_with(d)))
        .cloned()
        .collect();
    let move_files = settings::move_files();
    let importers = stores::enabled_importers();
    // Imports the DJ removed in Settings, to take out before anything else.
    let removed: Vec<String> = stores::importers()
        .iter()
        .map(|i| i.id().to_string())
        .filter(|id| settings::flag(&settings::import_remove_key(id), false))
        .collect();
    // ~/Music before the scan noted its folders: the only one it could
    // have looked through without the DJ choosing it.
    let before = library_path
        .as_deref()
        .and_then(|p| Library::open(p).ok())
        .map(|db| {
            db.scanned_folders().unwrap_or_else(|| {
                djsugar_engine::scan::default_music_dir()
                    .into_iter()
                    .collect()
            })
        })
        .unwrap_or_default();
    // Folders scanned before and not now: what was found only there goes.
    let dropped: Vec<PathBuf> = before
        .into_iter()
        .filter(|old| !dirs.iter().any(|d| old.starts_with(d)))
        .collect();
    if dirs.is_empty() && importers.is_empty() && removed.is_empty() && dropped.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let mut scanned = Vec::new();
        for dir in &dirs {
            let next = SAMPLE_LIBRARY_SIZE + scanned.len();
            scanned.extend(scan(dir, next));
        }
        let synced = library_path
            .as_deref()
            .map(|path| -> Result<Vec<Track>, String> {
                let mut db = Library::open(path).map_err(|e| e.to_string())?;
                for id in &removed {
                    // Files in the music folders stay, as a scan would bring them in again.
                    let keep = |p: &Path| music.iter().any(|m| p.starts_with(m));
                    match db.remove_import(id, keep) {
                        Ok(r) => eprintln!(
                            "{id}: import removed: {} lists, {} tracks; {} tracks kept",
                            r.lists, r.tracks, r.kept
                        ),
                        Err(e) => eprintln!("can't remove the {id} import: {e}"),
                    }
                    settings::set(&settings::import_remove_key(id), serde_json::json!(false));
                }
                for dir in &dropped {
                    match db.forget_scanned_under(dir, &dirs) {
                        Ok(n) if n > 0 => {
                            eprintln!("library: {n} tracks from {} taken out", dir.display())
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("can't take out {}: {e}", dir.display()),
                    }
                }
                if let Err(e) = db.set_scanned_folders(&dirs) {
                    eprintln!("can't note the scanned folders: {e}");
                }
                let scanned: Vec<Track> = scanned.iter().map(|t| (**t).clone()).collect();
                let report = db.sync(&scanned).map_err(|e| e.to_string())?;
                if report.added + report.updated > 0 {
                    eprintln!(
                        "library: {} tracks added, {} updated",
                        report.added, report.updated
                    );
                }
                // Own lists made inside an import's folder before those were
                // read-only move out, so the next import can't take them.
                match db.rescue_own_lists() {
                    Ok(0) => {}
                    Ok(n) => eprintln!("library: {n} own lists moved out of imported folders"),
                    Err(e) => eprintln!("can't move own lists out of imports: {e}"),
                }
                for importer in &importers {
                    let Some(database) = settings::import_database(importer.as_ref()) else {
                        continue;
                    };
                    let groups = settings::import_groups(importer.as_ref());
                    let theirs = settings::import_theirs_win(importer.id());
                    let want_streamed = settings::import_wants_streamed(importer.id());
                    let choices = settings::import_list_choices(importer.id());
                    // Made the DJ's own: no lists come in to replace them.
                    let lists_mine =
                        settings::flag(&settings::import_lists_mine_key(importer.id()), false);
                    let mut picked: Vec<_> = choices.iter().collect();
                    picked.sort();
                    // Changing what comes in, or whose tags win, brings it in again.
                    let stamp = format!(
                        "{}|{}|{theirs}|{want_streamed}|{picked:?}",
                        importer.stamp(&database),
                        groups.join(",")
                    );
                    match db.import_with(
                        importer.id(),
                        importer.title(),
                        &stamp,
                        theirs,
                        want_streamed,
                        || {
                            let mut library = importer.read(&database, &groups)?;
                            library.lists.retain(|l| {
                                !lists_mine && settings::import_list_wanted(&choices, l)
                            });
                            Ok(library)
                        },
                    ) {
                        Ok(Some(r)) => eprintln!(
                            "{}: {} tracks added, {} updated, {} matched to a copy already \
                             here; {} lists, folders and sessions ({} streamed tracks standing \
                             in, {} already in the collection, {} entries without a file here \
                             left out)",
                            importer.id(),
                            r.added,
                            r.updated,
                            r.matched,
                            r.lists,
                            r.streamed,
                            r.filled,
                            r.unmatched
                        ),
                        Ok(None) => {}
                        Err(e) => eprintln!("can't import {}: {e}", database.display()),
                    }
                }
                // An import may have made tag groups for tags seen first.
                settings::publish_tag_groups(&db);
                match db.assign_stages(&folders) {
                    Ok(0) => {}
                    Ok(n) => eprintln!("library: {n} tracks given a stage by their folder"),
                    Err(e) => eprintln!("can't set stages: {e}"),
                }
                if move_files {
                    match db.apply_moves() {
                        Ok(r) => {
                            if r.moved > 0 {
                                eprintln!("library: {} files moved between stage folders", r.moved);
                            }
                            for (path, why) in r.failed {
                                eprintln!("can't move {}: {why}", path.display());
                            }
                        }
                        Err(e) => eprintln!("can't move files: {e}"),
                    }
                }
                db.all_tracks(Path::exists).map_err(|e| e.to_string())
            });
        let tracks: Vec<Arc<Track>> = match synced {
            Some(Ok(tracks)) => tracks.into_iter().map(Arc::new).collect(),
            Some(Err(e)) => {
                eprintln!("can't update the library: {e}");
                // The saved collection stays, as it was last time: the bare
                // scan has no lists, stages or Collection Health.
                let saved = library_path
                    .as_deref()
                    .and_then(|p| Library::open(p).ok())
                    .and_then(|db| db.all_tracks(Path::exists).ok())
                    .unwrap_or_default();
                if saved.is_empty() {
                    scanned
                } else {
                    saved.into_iter().map(Arc::new).collect()
                }
            }
            // Without a library database, show what was found.
            None => scanned,
        };
        if !tracks.is_empty() {
            let _ = tx.send(tracks);
        }
    });
}

/// A MIDI port's device name: ALSA ports read "DDJ-FLX4:DDJ-FLX4 MIDI 1 24:0".
fn short_port_name(port: &str) -> &str {
    port.split(':').next().unwrap_or(port)
}

/// Prints every control DJ Sugar has, aliases included, as sorted `[Group],key`
/// lines. The website's controller pages use this list to mark mapping lines
/// that don't do anything in DJ Sugar yet (docs/supported-controls.txt).
fn list_controls() {
    print!("{}", control_list());
}

/// The lines `--list-controls` prints, one `[Group],key` per line.
fn control_list() -> String {
    let registry = ControlRegistry::new();
    djsugar_engine::create_controls(&registry);
    broadcast::create_controls(&registry);
    browser::LibraryControls::create(&registry);
    automix::AutoMix::new(&registry);
    waveform_zoom::create_controls(&registry);
    waveform_height::create_controls(&registry);
    let mut keys = registry.keys();
    keys.extend(registry.alias_keys());
    keys.sort();
    keys.iter()
        .map(|k| format!("{},{}\n", k.group, k.item))
        .collect()
}

/// Connects the `Controls` global used by every skin widget to the registry.
/// Names are resolved to an index once; per-frame reads are atomic loads.
/// Resolved controls, indexed by the number handed to the UI.
#[derive(Default)]
struct Resolved {
    handles: Vec<ControlHandle>,
    by_name: HashMap<(String, String), i32>,
}

/// Hooks the skin's controls up to the registry. Returns a count that goes
/// up whenever any control the skin shows changes, so the UI only looks
/// again (and redraws) when something did.
fn bind_controls(app: &AppWindow, registry: Arc<ControlRegistry>) -> impl Fn() -> u64 {
    let handles: Rc<RefCell<Resolved>> = Default::default();
    let controls = app.global::<Controls>();
    controls.set_rate_ranges(ModelRc::new(VecModel::from(
        prefs::RATE_RANGES.map(|r| r as f32).to_vec(),
    )));
    let get = {
        let handles = handles.clone();
        move |i: i32| -> Option<ControlHandle> { handles.borrow().handles.get(i as usize).cloned() }
    };
    {
        let handles = handles.clone();
        controls.on_resolve(move |group, key| {
            let k = (group.to_string(), key.to_string());
            let mut h = handles.borrow_mut();
            if let Some(i) = h.by_name.get(&k) {
                return *i;
            }
            let Some(handle) = registry.get(&k.0, &k.1) else {
                eprintln!("skin refers to unknown control {},{}", k.0, k.1);
                return -1;
            };
            let i = h.handles.len() as i32;
            h.handles.push(handle);
            h.by_name.insert(k, i);
            i
        });
    }
    let g = get.clone();
    controls.on_parameter(move |i, _| g(i).map(|c| c.get_parameter() as f32).unwrap_or(0.0));
    let g = get.clone();
    controls.on_value(move |i, _| g(i).map(|c| c.get() as f32).unwrap_or(0.0));
    // Where a controller's knob or fader is while it waits to pick the
    // control up; -1 while it is where the control is, or not known.
    let g = get.clone();
    controls.on_hardware(move |i, _| {
        g(i).and_then(|c| {
            let hw = djsugar_controllers::pickup::hardware_position(c.key())?;
            ((hw - c.get_parameter()).abs() > djsugar_controllers::pickup::THRESHOLD)
                .then_some(hw as f32)
        })
        .unwrap_or(-1.0)
    });
    let g = get.clone();
    controls.on_set_parameter(move |i, p| {
        if let Some(c) = g(i) {
            c.set_parameter(p as f64)
        }
    });
    let g = get.clone();
    controls.on_set_value(move |i, v| {
        if let Some(c) = g(i) {
            c.set(v as f64)
        }
    });
    let g = get.clone();
    controls.on_toggle(move |i| {
        if let Some(c) = g(i) {
            c.toggle()
        }
    });
    controls.on_reset(move |i| {
        if let Some(c) = get(i) {
            c.reset()
        }
    });
    move || {
        let h = handles.borrow();
        // Each version only goes up, so their sum moves when any one does;
        // a newly resolved control adds to it too.
        h.handles.iter().map(|c| c.version() as u64).sum::<u64>()
            + h.handles.len() as u64
            + djsugar_controllers::pickup::version()
    }
}

#[cfg(test)]
mod control_list_tests {
    /// The website marks mapping lines DJ Sugar doesn't support from this
    /// file. A test rather than a CI step running `--list-controls`, so CI
    /// doesn't build the whole app a second time just for this.
    #[test]
    fn website_list_is_up_to_date() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/supported-controls.txt"
        );
        let saved = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        assert!(
            saved == super::control_list(),
            "docs/supported-controls.txt is out of date; regenerate it with \
             `cargo run -q -p djsugar -- --list-controls > docs/supported-controls.txt`"
        );
    }
}

#[cfg(test)]
mod time_tests {
    #[test]
    fn whole_seconds() {
        assert_eq!(super::fmt_seconds(479.96, false), "8:00");
        assert_eq!(super::fmt_seconds(0.2, true), "0:01");
        assert_eq!(super::fmt_seconds(60.0, true), "1:00");
        assert_eq!(super::fmt_seconds(-1.0, true), "0:00");
    }
}

/// How long an alert stays on screen.
const ALERT_SECONDS: Duration = Duration::from_secs(8);

/// A deck's playing speed from its tempo fader (`rate`), range and
/// direction, as the engine works it out.
pub(crate) fn speed(rate: &ControlHandle, range: &ControlHandle, dir: &ControlHandle) -> f64 {
    let dir = if dir.get() < 0.0 { -1.0 } else { 1.0 };
    1.0 + rate.get() * range.get() * dir
}

pub(crate) fn fmt_time(seconds: f64) -> String {
    // Round to tenths first, so 479.96 s reads 8:00.0 rather than 7:60.0.
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// Like fmt_time with two-digit minutes, "02:00.8", as the jog shows times.
fn fmt_clock(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    format!(
        "{:02}:{:02}.{}",
        tenths / 600,
        tenths / 10 % 60,
        tenths % 10
    )
}

/// Whole seconds, as the deck header shows times: "7:59". Remaining time
/// counts down to 0:00 only when the track ends, so it rounds up.
fn fmt_seconds(seconds: f64, round_up: bool) -> String {
    let s = seconds.max(0.0);
    let s = if round_up {
        (s - 1e-6).ceil()
    } else {
        s.round()
    } as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Handles to the controls the deck displays and waveforms read every frame.
struct DeckHandles {
    play: ControlHandle,
    /// The input played instead of a track (0 for none), and whether the
    /// deck is a Groove Box: either way it plays live.
    live_input: ControlHandle,
    groove: ControlHandle,
    /// The deck plays its Looper.
    looper: ControlHandle,
    /// Non-zero on the sync leader.
    sync_leader: ControlHandle,
    playposition: ControlHandle,
    rate: ControlHandle,
    rate_range: ControlHandle,
    rate_dir: ControlHandle,
    bpm: ControlHandle,
    cue_point: ControlHandle,
    loop_enabled: ControlHandle,
    loop_start: ControlHandle,
    loop_end: ControlHandle,
    loop_in_pending: ControlHandle,
    /// The silent playhead while slipping, in seconds (-1 for none).
    slip_position: ControlHandle,
    beatloop_size: ControlHandle,
    /// The gain knob, which sizes the waveforms when Size follows gain is on.
    pregain: ControlHandle,
    keylock: ControlHandle,
    /// Grid edit mode (the pencil) and the grid lock: an unlocked grid in
    /// edit mode is drawn louder.
    beats_edit: ControlHandle,
    bpmlock: ControlHandle,
    /// Key shift in semitones, the key being played and the file's key.
    pitch: ControlHandle,
    key: ControlHandle,
    file_key: ControlHandle,
    hotcues: Vec<(ControlHandle, ControlHandle)>,
    /// loaded_effect of each slot of the effect unit shown under this deck.
    effects: Vec<ControlHandle>,
    /// The lists the deck shows, kept between frames: the UI rebuilds the
    /// rows of a replaced list, which would drop a click in progress.
    effect_names: Rc<VecModel<SharedString>>,
    key_menu: Rc<VecModel<KeyItem>>,
    key_menu_all: Rc<VecModel<KeyItem>>,
    stem_names: Rc<VecModel<SharedString>>,
    stem_colors: Rc<VecModel<slint::Color>>,
}

/// `model` as a list for the UI, with its rows replaced only when `items` differ.
fn kept<T: Clone + PartialEq + 'static>(model: &Rc<VecModel<T>>, items: Vec<T>) -> ModelRc<T> {
    if model.iter().ne(items.iter().cloned()) {
        model.set_vec(items);
    }
    ModelRc::from(model.clone())
}

struct Decks([DeckHandles; NUM_DECKS]);

impl Decks {
    fn new(reg: &ControlRegistry) -> Self {
        Self(std::array::from_fn(|d| {
            let g = deck_group(d);
            let c = |k: &str| {
                reg.get(&g, k)
                    .unwrap_or_else(|| panic!("missing control {g},{k}"))
            };
            DeckHandles {
                play: c("play"),
                live_input: c("live_input"),
                groove: reg
                    .get(&format!("[Groove{}]", d + 1), "enabled")
                    .expect("groove box control"),
                looper: reg
                    .get(&djsugar_engine::looper::looper_group(d), "enabled")
                    .expect("looper control"),
                sync_leader: c("sync_leader"),
                playposition: c("playposition"),
                rate: c("rate"),
                rate_range: c("rateRange"),
                rate_dir: c("rate_dir"),
                bpm: c("bpm"),
                cue_point: c("cue_point"),
                loop_enabled: c("loop_enabled"),
                loop_start: c("loop_start_position"),
                loop_end: c("loop_end_position"),
                loop_in_pending: c("loop_in_pending"),
                slip_position: c("slip_position"),
                beatloop_size: c("beatloop_size"),
                pregain: c("pregain"),
                keylock: c("keylock"),
                beats_edit: c("beats_edit"),
                bpmlock: c("bpmlock"),
                pitch: c("pitch"),
                key: c("key"),
                file_key: c("file_key"),
                hotcues: (1..=8)
                    .map(|n| {
                        (
                            c(&format!("hotcue_{n}_enabled")),
                            c(&format!("hotcue_{n}_position")),
                        )
                    })
                    .collect(),
                effects: (0..EFFECTS_PER_UNIT)
                    .map(|s| {
                        reg.get(&effect_group(d, s), "loaded_effect")
                            .expect("effect slot control")
                    })
                    .collect(),
                effect_names: Default::default(),
                key_menu: Default::default(),
                key_menu_all: Default::default(),
                stem_names: Default::default(),
                stem_colors: Default::default(),
            }
        }))
    }

    /// The key menu's entries: the keys the deck can be shifted to, from
    /// `range` semitones down to `range` up, marking the current shift and
    /// the keys that mix with the other deck's.
    fn key_menu(&self, deck: usize, range: i32, camelot: bool) -> Vec<KeyItem> {
        let h = &self.0[deck];
        let Some(file) = Key::from_control_number(h.file_key.get()) else {
            return Vec::new();
        };
        let speed = speed(&h.rate, &h.rate_range, &h.rate_dir);
        let base = file.playing(0.0, speed, h.keylock.get_bool());
        // The deck it pairs with: 1 with 2, 3 with 4.
        let other = Key::from_control_number(self.0[deck ^ 1].key.get());
        let current = h.pitch.get().round() as i32;
        (-range..=range)
            .map(|shift| {
                let key = base.transpose(shift);
                KeyItem {
                    shift,
                    text: format!("{shift:+}  {}", key.label(camelot)).into(),
                    compatible: other.is_some_and(|o| key.compatible(o)),
                    current: shift == current,
                }
            })
            .collect()
    }

    fn info(&self, deck: usize, track: Option<&Track>, camelot: bool) -> DeckInfo {
        let h = &self.0[deck];
        let effects = kept(
            &h.effect_names,
            h.effects
                .iter()
                .map(|c| SharedString::from(audio_plugins::effect_name(c.get())))
                .collect(),
        );
        let range = format!("{:.0}%", h.rate_range.get() * 100.0).into();
        let rate = format!(
            "{:+.1}%",
            (speed(&h.rate, &h.rate_range, &h.rate_dir) - 1.0) * 100.0
        )
        .into();
        let Some(t) = track else {
            return DeckInfo {
                rate,
                range,
                effects,
                ..Default::default()
            };
        };
        let pos = h.playposition.get() * t.duration;
        let end_flash = track_end::flash(
            t.duration - pos,
            h.play.get_bool(),
            h.loop_enabled.get_bool(),
            track_end::seconds(),
        );
        let size = h.beatloop_size.get();
        // A loop made with IN and OUT shows its own length while it plays;
        // the box goes back to the beat loop size when it ends.
        let manual_beats = h
            .loop_enabled
            .get_bool()
            .then(|| t.beat_at(h.loop_end.get()) - t.beat_at(h.loop_start.get()))
            .filter(|beats| (beats - size).abs() > 0.01);
        let stems = t.audio.as_ref().and_then(|a| a.stems());
        let initials: String = t
            .artist
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect();
        DeckInfo {
            loaded: true,
            title: t.title.clone().into(),
            artist: t.artist.clone().into(),
            initials: initials.into(),
            // Filled in by the caller, which keeps the decoded images.
            cover: Default::default(),
            has_cover: false,
            bpm: if t.bpm > 0.0 {
                format!("{:.2}", h.bpm.get())
            } else {
                "--".into()
            }
            .into(),
            rate,
            range,
            grid_sections: t.grid.markers().len().max(1) as i32,
            grid_source: grid_source(&t.info.grid_from).into(),
            grid_unsure: t.info.grid_unsure,
            stem_names: kept(
                &h.stem_names,
                stems.map_or(Vec::new(), |s| {
                    s.info.iter().map(|i| i.name.clone().into()).collect()
                }),
            ),
            stem_colors: kept(
                &h.stem_colors,
                stems.map_or(Vec::new(), |s| {
                    s.info
                        .iter()
                        .map(|i| slint::Color::from_argb_encoded(0xff00_0000 | i.color))
                        .collect()
                }),
            ),
            stem_status: match &t.source {
                Source::File(path) => match stems::separation(path) {
                    Some(stems::Separation::Failed(e)) => e.into(),
                    _ => "".into(),
                },
                _ => "".into(),
            },
            stem_hq: stems.is_some_and(|s| s.playing_quality() == stems::Quality::High),
            stem_preparing: match &t.source {
                // Running, or done and the stems still loading.
                Source::File(path) => {
                    matches!(
                        stems::separation(path),
                        Some(stems::Separation::Running | stems::Separation::Done)
                    ) && !stems.is_some_and(|s| s.playing_quality() == stems::Quality::High)
                }
                _ => false,
            },
            can_prepare_hq: match &t.source {
                Source::File(path) => {
                    !stems.is_some_and(|s| s.playing_quality() == stems::Quality::High)
                        && !matches!(
                            stems::separation(path),
                            Some(stems::Separation::Running | stems::Separation::Done)
                        )
                }
                _ => false,
            },
            // The key being played, with any shift; a tag that isn't a key
            // is shown as it is.
            key: match Key::from_control_number(h.key.get()) {
                Some(k) => k.label(camelot),
                None => t.key.clone(),
            }
            .into(),
            key_wheel: Key::from_control_number(h.key.get())
                .or_else(|| Key::parse(&t.key))
                .map_or(0, |k| k.camelot_number() as i32),
            key_shift: match h.pitch.get().round() as i32 {
                0 => String::new(),
                n => format!("{n:+}"),
            }
            .into(),
            key_menu: kept(&h.key_menu, self.key_menu(deck, 3, camelot)),
            key_menu_all: kept(&h.key_menu_all, self.key_menu(deck, 6, camelot)),
            elapsed: fmt_clock(pos).into(),
            remaining: fmt_clock(t.duration - pos).into(),
            remaining_seconds: fmt_seconds(t.duration - pos, true).into(),
            total: fmt_seconds(t.duration, false).into(),
            duration: t.duration as f32,
            loop_size: loop_label(match manual_beats {
                Some(beats) => beats,
                None => size,
            }),
            progress: (pos / t.duration) as f32,
            end_flash_time: end_flash && track_end::style().time(),
            end_flash_ring: end_flash && track_end::style().ring(),
            // Filled in by the caller, from Automix.
            automix_from: 0.0,
            automix_to: 0.0,
            effects,
            // Filled in by the caller, which reads each file once.
            quality: Default::default(),
            lossless: false,
        }
    }

    /// `seconds`: what the scrolling waveform shows across at normal speed
    /// (the shared zoom's).
    fn frame(
        &self,
        deck: usize,
        t: &Track,
        play_seconds: f64,
        phrases: Phrases,
        seconds: f64,
    ) -> Frame {
        let h = &self.0[deck];
        Frame {
            play_seconds,
            // Show more of the track when it plays faster, so synced decks'
            // beats are the same distance apart on screen.
            visible_seconds: seconds * speed(&h.rate, &h.rate_range, &h.rate_dir),
            duration: t.duration,
            first_bar_beat: first_bar_beat(t),
            grid: if t.grid.is_empty() {
                steady_grid(t.bpm, t.first_beat)
            } else {
                let half = seconds.max(VISIBLE_SECONDS) * 2.0;
                t.grid
                    .sections_in::<GRID_SECTIONS>(play_seconds - half, play_seconds + half)
                    .map(|s| s.map(|(seconds, bpm, beat)| GridSection { seconds, bpm, beat }))
            },
            cue: Some(h.cue_point.get()),
            loop_range: h
                .loop_enabled
                .get_bool()
                .then(|| (h.loop_start.get(), h.loop_end.get())),
            loop_marking: h.loop_in_pending.get_bool().then(|| h.loop_start.get()),
            hotcues: std::array::from_fn(|i| {
                let (enabled, position) = &h.hotcues[i];
                enabled.get_bool().then(|| position.get())
            }),
            slip: Some(h.slip_position.get()).filter(|&s| s >= 0.0),
            gain: wave_look::gain_scale(h.pregain.get()),
            phrases,
        }
    }
}

/// A deck's album art as a Slint image, made once per cover.
#[derive(Default)]
struct CoverImage(Option<(Arc<Cover>, slint::Image)>);

impl CoverImage {
    fn get(&mut self, track: Option<&Track>) -> Option<slint::Image> {
        let Some(cover) = track.and_then(|t| t.cover.clone()) else {
            self.0 = None;
            return None;
        };
        match &self.0 {
            Some((c, image)) if Arc::ptr_eq(c, &cover) => Some(image.clone()),
            _ => {
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &cover.rgba,
                    cover.width,
                    cover.height,
                );
                let image = slint::Image::from_rgba8(pixels);
                self.0 = Some((cover, image.clone()));
                Some(image)
            }
        }
    }
}

/// Whether two deck infos show the same. Their lists are new models each
/// time, which never compare equal, so they are compared by their rows; and
/// Slint never finds two empty images equal, so those are compared by size.
fn same_deck_info(old: &DeckInfo, new: &DeckInfo) -> bool {
    fn same<T: Clone + PartialEq + 'static>(a: &ModelRc<T>, b: &ModelRc<T>) -> bool {
        a.row_count() == b.row_count() && a.iter().eq(b.iter())
    }
    let lists_same = same(&old.key_menu, &new.key_menu)
        && same(&old.key_menu_all, &new.key_menu_all)
        && same(&old.stem_names, &new.stem_names)
        && same(&old.stem_colors, &new.stem_colors)
        && same(&old.effects, &new.effects);
    let no_cover = |i: &slint::Image| i.size() == Default::default();
    let covers_same = old.cover == new.cover || (no_cover(&old.cover) && no_cover(&new.cover));
    // Everything else, with the lists and covers (compared above) the same.
    let blank = slint::Image::from_rgba8(slint::SharedPixelBuffer::new(1, 1));
    let rest = |i: &DeckInfo| DeckInfo {
        key_menu: Default::default(),
        key_menu_all: Default::default(),
        stem_names: Default::default(),
        stem_colors: Default::default(),
        effects: Default::default(),
        cover: blank.clone(),
        ..i.clone()
    };
    lists_same && covers_same && rest(old) == rest(new)
}

/// Where each phrase starts and its kind, as the waveforms draw them.
type Phrases = Vec<(f64, PhraseKind)>;

/// A track on the GPU, with the waveform, vocal levels and stem levels it
/// came from.
type Uploaded = (
    Arc<GpuTrack>,
    Arc<Waveform>,
    Option<Arc<Levels>>,
    Option<Arc<stem_levels::Levels>>,
);

struct Gpu {
    /// The [`wave_look::version`] the views were last set to.
    look: u32,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: WaveformRenderer,
    scrolling: [WaveformView; NUM_DECKS],
    overview: [WaveformView; NUM_DECKS],
    /// Each deck's track on the GPU, shared by its two views, and the
    /// waveform and vocal levels it was uploaded from. While a track is
    /// being analysed its waveform grows, and each new version is uploaded
    /// again, as it is once its vocals are found.
    tracks: [Option<Uploaded>; NUM_DECKS],
    /// Each deck's phrases and the track they were found for, found again
    /// when the track changes (a new analysis or a grid edit replaces it).
    phrases: [Option<(Arc<Track>, Phrases)>; NUM_DECKS],
    /// Each deck's drawn position, steadier than the engine's as read.
    clocks: [SmoothClock; NUM_DECKS],
    /// The sound of decks playing live, and each one's waveform on the GPU.
    live: Option<live_trace::LiveTraces>,
    /// The notes the Instrument input holds, for its deck's note roll.
    instrument: Arc<djsugar_engine::InstrumentView>,
    /// The Groove Box decks' step lanes, and their Groove Boxes' controls.
    steps: Option<step_lanes::StepLanes>,
    grooves: [step_lanes::GrooveHandles; NUM_DECKS],
    /// The knobs the macro lanes follow, and their values so far.
    knobs: macro_lanes::KnobHandles,
    /// The instrument's scale now, whose rows its note roll tints.
    scale_mask: ControlHandle,
    macros: macro_lanes::MacroLanes,
    /// Which of the knobs drew a line last, as the list beside them shows.
    knobs_moved: [bool; macro_lanes::KNOBS],
    live_tracks: [Option<Arc<GpuTrack>>; NUM_DECKS],
    /// Whether a deck was moving in the last frame drawn.
    moving: bool,
    images_set: bool,
}

impl Gpu {
    /// Draws live deck `d`'s sound up to its play line on its scrolling
    /// waveform, on the beats of `leader` (a deck and its drawn position),
    /// with the leader's phrases where `flags` is set.
    #[allow(clippy::too_many_arguments)]
    fn draw_live(
        &mut self,
        d: usize,
        w: live_trace::Window,
        leader: Option<(usize, f64)>,
        flags: bool,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
        decks: &Decks,
        scale: f32,
    ) {
        let gpu = match &self.live_tracks[d] {
            Some(g) if g.rewrite(&self.queue, &w.levels) => g.clone(),
            _ => {
                let g = Arc::new(GpuTrack::new(
                    &self.device,
                    &self.queue,
                    &self.renderer,
                    usize::MAX - d,
                    &w.levels,
                    WAVEFORM_RATE,
                ));
                self.live_tracks[d] = Some(g.clone());
                g
            }
        };
        let lead = leader.and_then(|(l, seconds)| Some((tracks[l].as_ref()?, l, seconds)));
        let lead_speed = |l: usize| {
            let h = &decks.0[l];
            speed(&h.rate, &h.rate_range, &h.rate_dir)
        };
        let grid = lead.map_or([None; GRID_SECTIONS], |(t, l, seconds)| {
            live_trace::track_grid(t, seconds, lead_speed(l), w.play_seconds)
        });
        let phrases = match lead {
            Some((t, l, seconds)) if flags => {
                let found = self.phrases(l, t);
                live_trace::track_phrases(t, seconds, lead_speed(l), w.play_seconds, &found)
            }
            _ => Vec::new(),
        };
        let frame = Frame {
            play_seconds: w.play_seconds,
            visible_seconds: VISIBLE_SECONDS,
            duration: w.play_seconds,
            first_bar_beat: lead.map_or(0.0, |(t, _, _)| first_bar_beat(t)),
            grid,
            cue: None,
            loop_range: None,
            loop_marking: None,
            hotcues: [None; 8],
            slip: None,
            gain: 1.0,
            phrases,
        };
        self.scrolling[d].set_lanes(w.lanes);
        self.scrolling[d].set_track(&self.device, &self.renderer, Some(&gpu));
        self.scrolling[d].render(
            &self.device,
            &self.queue,
            &self.renderer,
            Some(&frame),
            scale,
        );
        self.overview[d].set_track(&self.device, &self.renderer, None);
        self.overview[d].render(&self.device, &self.queue, &self.renderer, None, scale);
        self.moving = true;
    }

    fn render(
        &mut self,
        app: &AppWindow,
        decks: &Decks,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
        visual: &VisualPositions,
    ) {
        // The waveforms button's look choices, when they change.
        if self.look != wave_look::version() {
            self.look = wave_look::version();
            let (style, palette, half) =
                (wave_look::style(), wave_look::palette(), wave_look::half());
            let vocals = wave_look::show_vocals();
            let needle = wave_look::needle();
            for v in self.scrolling.iter_mut().chain(self.overview.iter_mut()) {
                v.set_style(style);
                v.set_palette(palette);
                v.set_half(half);
                v.set_needle(needle);
                v.set_show_vocals(vocals);
            }
        }
        let scale = app.window().scale_factor();
        let px = |l: f32| (l * scale).round().max(1.0) as u32;
        let (ww, wh) = (px(app.get_waveform_width()), px(app.get_waveform_height()));
        let (ow, oh) = (px(app.get_overview_width()), px(app.get_overview_height()));
        let now = Instant::now();
        let mut changed = !self.images_set;
        self.moving = false;
        let four = app.get_layout() == FOUR_DECK_LAYOUT;
        let macros_shown = app.global::<MacroLanes>().get_shown();
        // The large waveforms' shared zoom, gliding to a new step: the next
        // frame comes at once until it gets there.
        let (zoom, gliding) = waveform_zoom::now(now);
        self.moving |= gliding;
        // Prep's large waveform (deck 1) shows, always editing the grid.
        let prep_shown = app.get_layout() == PREP_LAYOUT && !app.get_library_expanded();
        let ui_zoom = app.global::<WaveZoom>();
        if ui_zoom.get_seconds() != zoom as f32 {
            ui_zoom.set_seconds(zoom as f32);
        }
        // The Player bar shows the waveform alone, without the phrase bar.
        let show_phrases =
            app.global::<PhraseView>().get_visible() && app.get_layout() != PLAYER_LAYOUT;
        // Phrase flags on the scrolling waveforms, live decks' included.
        let show_flags = app.global::<PhraseView>().get_waveform_visible();
        let overview_background = if app.get_layout() == PLAYER_LAYOUT {
            PLAYER_OVERVIEW_BACKGROUND
        } else {
            Palette::default().background
        };
        // Every deck's drawn position first: a live deck draws the leading
        // deck's beats.
        let clocks: [(f64, f64); NUM_DECKS] = std::array::from_fn(|d| {
            let (position, speed) = visual.position(d, now);
            (self.clocks[d].next(position, speed, now), speed)
        });
        // Decks playing an input, an instrument or the Groove Box.
        let live: [bool; NUM_DECKS] = std::array::from_fn(|d| {
            let h = &decks.0[d];
            tracks[d].is_none()
                && (h.live_input.get() > 0.0 || h.groove.get_bool() || h.looper.get_bool())
        });
        if let Some(traces) = self.live.as_mut() {
            traces.poll(live);
        }
        // Decks playing their Groove Box, for the step lanes.
        let groove: [bool; NUM_DECKS] = std::array::from_fn(|d| {
            let h = &decks.0[d];
            live[d] && h.groove.get_bool() && h.live_input.get() <= 0.0 && !h.looper.get_bool()
        });
        if let Some(steps) = self.steps.as_mut() {
            steps.poll(groove);
        }
        // The leading deck: the sync leader if it plays, else the first
        // playing deck with a tempo.
        let playing = (0..NUM_DECKS).filter(|&d| {
            decks.0[d].play.get_bool() && tracks[d].as_ref().is_some_and(|t| t.bpm > 0.0)
        });
        let leader = playing
            .clone()
            .find(|&d| decks.0[d].sync_leader.get() > 0.0)
            .or_else(|| playing.clone().next());
        for (d, track) in tracks.iter().enumerate() {
            changed |= self.scrolling[d].resize(&self.device, ww, wh);
            changed |= self.overview[d].resize(&self.device, ow, oh);
            self.overview[d].set_background(overview_background);
            let track = track.as_ref().filter(|t| !t.waveform.is_empty());
            let show_vocals = wave_look::show_vocals();
            let stems_look = wave_look::style() == djsugar_waveform::Style::Stems;
            // The stems' levels once they are complete: for the Stems look,
            // and for Vocals › Bar from high-quality stems' vocal stem.
            let found = track
                .and_then(|t| t.audio.as_ref()?.stems())
                .filter(|s| {
                    stems_look || (show_vocals && s.current().quality == stems::Quality::High)
                })
                .and_then(stem_levels::levels);
            // Vocals › Bar: the vocal stem's level, else the vocal pass's
            // once it has the file.
            let hq_vocals = found
                .as_ref()
                .filter(|_| show_vocals)
                .and_then(|f| f.vocals.clone());
            let (vocals, vocal_rate) = match (hq_vocals, track.map(|t| &t.source)) {
                (Some(v), _) => (Some(v), stem_levels::RATE),
                (None, Some(Source::File(path))) if show_vocals => {
                    (vocal_pass::levels(path), djsugar_engine::vocals::RATE)
                }
                _ => (None, djsugar_engine::vocals::RATE),
            };
            let same_vocals = |v: &Option<Arc<Levels>>| match (v, &vocals) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                // Kept on the GPU when Vocals › Bar is off: the view hides them.
                (Some(_), None) => !show_vocals,
                _ => false,
            };
            let stems = found.filter(|_| stems_look);
            let same_stems = |v: &Option<Arc<stem_levels::Levels>>| match (v, &stems) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                // Kept on the GPU in other looks, which don't draw them.
                (Some(_), None) => !stems_look,
                _ => false,
            };
            let gpu_track = match (track, &self.tracks[d]) {
                (None, _) => None,
                // Uploaded already, unless the waveform has grown since or
                // its vocals or stem levels have come.
                (Some(t), Some((g, w, v, s)))
                    if g.id() == t.id
                        && Arc::ptr_eq(w, &t.waveform)
                        && same_vocals(v)
                        && same_stems(s) =>
                {
                    Some((g.clone(), w.clone(), v.clone(), s.clone()))
                }
                (Some(t), _) => Some((
                    Arc::new(GpuTrack::with_extras(
                        &self.device,
                        &self.queue,
                        &self.renderer,
                        t.id,
                        &t.waveform.levels,
                        t.waveform.bins_per_second,
                        Extras {
                            vocals: vocals.as_deref().map_or(&[], |v| v.as_slice()),
                            vocal_rate,
                            stems: stems.as_deref().map_or(&[], |s| s.levels.as_slice()),
                            stem_rate: stem_levels::RATE,
                            stem_colors: stems.as_deref().map_or(&[], |s| s.colors.as_slice()),
                        },
                    )),
                    t.waveform.clone(),
                    vocals.clone(),
                    stems.clone(),
                )),
            };
            self.tracks[d] = gpu_track.clone();
            let gpu_track = gpu_track.map(|(g, ..)| g);
            let (play_seconds, speed) = clocks[d];
            self.moving |= track.is_some() && speed != 0.0;
            // A slip's silent playhead runs on even while the deck is paused.
            self.moving |= track.is_some() && decks.0[d].slip_position.get() >= 0.0;
            let phrases = match track {
                Some(t) if show_phrases || show_flags => self.phrases(d, t),
                _ => Vec::new(),
            };
            self.beat_count(app, d, track, play_seconds, speed != 0.0, &phrases);
            let flags = if show_flags {
                phrases.clone()
            } else {
                Vec::new()
            };
            let phrases = if show_phrases { phrases } else { Vec::new() };
            let frame = track.map(|t| decks.frame(d, t, play_seconds, phrases, zoom));
            // The jog marker turns with the same smoothed clock as the
            // waveform, set here as each frame is drawn rather than on the UI
            // timer, whose ticks beat against the display. A 33 rpm record
            // turns once every 1.8 s.
            let spin = (play_seconds / 1.8).rem_euclid(1.0) as f32 * 360.0;
            let spin = if track.is_some() { spin } else { 0.0 };
            match d {
                0 => app.set_spin_1(spin),
                1 => app.set_spin_2(spin),
                // Decks 3 and 4 have no jog.
                _ => {}
            }
            // The lyrics under the waveform move with it.
            let lyrics = app.global::<WaveLyrics>();
            let at = play_seconds as f32;
            match d {
                0 => lyrics.set_now_1(at),
                1 => lyrics.set_now_2(at),
                2 => lyrics.set_now_3(at),
                _ => lyrics.set_now_4(at),
            }
            // Decks 3 and 4 show only in 4-Deck.
            if d >= 2 && !four {
                continue;
            }
            // An instrument deck's notes, for its note roll.
            let held = (decks.0[d].live_input.get().round() == INSTRUMENT_INPUT)
                .then(|| self.instrument.held());
            if let (Some(_), Some(traces)) = (held, self.live.as_mut()) {
                traces.set_scale(d, self.scale_mask.get().round() as u16);
            }
            let window = live[d]
                .then(|| {
                    let color = DECK_COLORS[d];
                    self.live
                        .as_mut()
                        .and_then(|t| t.window(d, now, held, color))
                })
                .flatten();
            // A Groove Box deck's step lanes, in place of a note roll.
            let window = window.map(|mut w| {
                if let (true, Some(steps)) = (groove[d], self.steps.as_mut()) {
                    let state = self.grooves[d].state();
                    let start = w.clock - w.play_seconds;
                    let ahead = VISIBLE_SECONDS * (1.0 - wave_look::needle() as f64);
                    w.lanes = Some(steps.lanes(d, &state, start, w.clock, ahead, DECK_COLORS[d]));
                }
                w
            });
            // Or an instrument deck's knobs as macro lanes, where its switch
            // is on (2-Deck Basic only).
            let page = (held.is_some() && !groove[d] && !four)
                .then(|| app.global::<InstrumentsUi>().get_knob_page().max(0) as usize)
                .filter(|_| macros_shown.row_data(d).unwrap_or(false));
            let window = match (window, page) {
                (Some(mut w), Some(page)) => {
                    let values = self.knobs.values(page);
                    self.macros.note(d, page, w.clock, values);
                    let start = w.clock - w.play_seconds;
                    let back = VISIBLE_SECONDS * wave_look::needle() as f64;
                    let (lanes, moved) = self.macros.lanes(d, start, w.clock, back, DECK_COLORS[d]);
                    if moved != self.knobs_moved {
                        self.knobs_moved = moved;
                        let model = VecModel::from(moved.to_vec());
                        app.global::<MacroLanes>().set_moved(ModelRc::new(model));
                    }
                    w.lanes = Some(lanes);
                    Some(w)
                }
                (window, _) => {
                    self.macros.clear(d);
                    window
                }
            };
            self.scrolling[d].set_live(window.is_some());
            // Grid edit mode (Prep's deck 1 always), while the grid is unlocked.
            let prep_deck = prep_shown && d == 0;
            let h = &decks.0[d];
            self.scrolling[d]
                .set_grid_edit((h.beats_edit.get_bool() || prep_deck) && !h.bpmlock.get_bool());
            if window.is_none() {
                self.scrolling[d].set_lanes(None);
            }
            if let Some(w) = window {
                let leader = leader.map(|l| (l, clocks[l].0));
                self.draw_live(d, w, leader, show_flags, tracks, decks, scale);
                continue;
            }
            // The overview's phrase bar, or the scrolling waveform's flags.
            let scrolling_frame = frame.as_ref().map(|f| Frame {
                phrases: flags,
                ..f.clone()
            });
            // Size follows gain sizes the large waveform only: the overview
            // keeps its height.
            let overview_frame = frame.as_ref().map(|f| Frame {
                gain: 1.0,
                ..f.clone()
            });
            for (view, frame) in [
                (&mut self.scrolling[d], scrolling_frame.as_ref()),
                (&mut self.overview[d], overview_frame.as_ref()),
            ] {
                view.set_track(&self.device, &self.renderer, gpu_track.as_ref());
                view.render(&self.device, &self.queue, &self.renderer, frame, scale);
            }
        }
        if changed {
            let img = |v: &WaveformView| {
                slint::Image::try_from(v.texture()).expect("import waveform texture")
            };
            app.set_waveform_1(img(&self.scrolling[0]));
            app.set_waveform_2(img(&self.scrolling[1]));
            app.set_waveform_3(img(&self.scrolling[2]));
            app.set_waveform_4(img(&self.scrolling[3]));
            app.set_overview_1(img(&self.overview[0]));
            app.set_overview_2(img(&self.overview[1]));
            app.set_overview_3(img(&self.overview[2]));
            app.set_overview_4(img(&self.overview[3]));
            self.images_set = true;
        }
    }
}

impl Gpu {
    /// Updates deck `d`'s beat counter from the same smoothed position the
    /// waveforms are drawn at, so its squares change on the beat.
    fn beat_count(
        &mut self,
        app: &AppWindow,
        d: usize,
        track: Option<&Arc<Track>>,
        seconds: f64,
        playing: bool,
        drawn: &Phrases,
    ) {
        let global = app.global::<BeatCounter>();
        let mode = global.get_mode();
        let count = match track {
            Some(t) if mode != beat_counter::MODE_OFF => {
                // The overview's phrases, found even while it hides them.
                let phrases = if mode != beat_counter::MODE_PHRASES {
                    Vec::new()
                } else if drawn.is_empty() {
                    self.phrases(d, t)
                } else {
                    drawn.clone()
                };
                beat_counter::count(t, seconds, &phrases)
            }
            _ => None,
        };
        let shown = match count {
            None => BeatCount::default(),
            Some(c) => BeatCount {
                shown: true,
                playing,
                beat: c.beat as i32,
                bar: c.bar_in_group as i32,
                last_bar: c.bars_left == 1,
                label: c.label().into(),
                countdown: c.countdown().into(),
                next_color: c
                    .next
                    .map_or(slint::Color::from_rgb_u8(0x9e, 0x9e, 0xa3), |k| {
                        slint::Color::from_argb_encoded(0xff00_0000 | beat_counter::kind_color(k))
                    }),
            },
        };
        let model = global.get_decks();
        if model.row_data(d).as_ref() != Some(&shown) {
            model.set_row_data(d, shown);
        }
    }

    /// Deck `d`'s phrases for the waveforms, found once per track: the
    /// library's until its waveform is ready, then found from the waveform
    /// and the grid as they are now.
    fn phrases(&mut self, d: usize, t: &Arc<Track>) -> Phrases {
        match &self.phrases[d] {
            Some((cached, p)) if Arc::ptr_eq(cached, t) => p.clone(),
            _ => {
                let p = beat_counter::track_phrases(t);
                self.phrases[d] = Some((t.clone(), p.clone()));
                p
            }
        }
    }
}

/// The tooltip for the phrase at `seconds`: its kind, then its length in
/// bars and where it starts and ends.
fn phrase_tip(phrases: &Phrases, duration: f64, bpm: f64, seconds: f64) -> Option<PhraseTip> {
    let i = phrases.iter().rposition(|(start, _)| *start <= seconds)?;
    let (start, kind) = phrases[i];
    let end = phrases.get(i + 1).map_or(duration, |(s, _)| *s);
    let title = match kind {
        PhraseKind::Intro => "INTRO",
        PhraseKind::Build => "BUILD-UP",
        PhraseKind::Drop => "DROP",
        PhraseKind::Breakdown => "BREAKDOWN",
        PhraseKind::Outro => "OUTRO",
        PhraseKind::Chorus => "CHORUS",
        PhraseKind::Verse => "VERSE",
    };
    let mmss = |s: f64| {
        let s = s.max(0.0).round() as u64;
        format!("{}:{:02}", s / 60, s % 60)
    };
    let range = format!("{} – {}", mmss(start), mmss(end));
    let bars = ((end - start) * bpm / 240.0).round();
    let body = if bpm > 0.0 && bars >= 1.0 {
        format!("{bars} bars · {range}")
    } else {
        range
    };
    Some(PhraseTip {
        title: title.into(),
        body: body.into(),
    })
}

/// Where a track's grid came from, as the grid menu says it.
fn grid_source(from: &str) -> String {
    match from {
        "" | "analysis" => "Detected".into(),
        "user" => "Edited".into(),
        "vdj" | "virtualdj" => "From VirtualDJ".into(),
        "mixxx" => "From Mixxx".into(),
        app => format!("From {app}"),
    }
}

use beat_counter::first_bar_beat;

/// What the deck headers' beat counter shows, as Settings › Decks keeps it.
pub fn beat_counter_mode() -> i32 {
    settings::number("beat_counter", beat_counter::MODE_PHRASES as f64).clamp(0.0, 2.0) as i32
}

/// A loop length in beats, as the loop box shows it: 4, 2.5 or 1/8.
fn loop_label(beats: f64) -> SharedString {
    if beats >= 1.0 {
        format!("{}", (beats * 100.0).round() / 100.0)
    } else {
        format!("1/{}", (1.0 / beats).round())
    }
    .into()
}

/// File › Export › Mixcloud…: makes the mixed set picked in the list into a
/// Mixcloud MP3 with its tracklist (see `browser::mix_export`). Says so if
/// no mixed set is picked. For any menu that offers it.
pub(crate) fn export_mixcloud(app: &AppWindow) {
    app.global::<FilesUi>()
        .invoke_export_mixcloud(app.get_current_track());
}

/// Path commands for the instrument's scope in a 128 by 32 box: its
/// outline, along each bin's highest sample and back along the lowest.
fn scope_path(bins: &[(f32, f32)]) -> String {
    use std::fmt::Write;
    let y = |v: f32| 16.0 - 15.0 * v.clamp(-1.0, 1.0);
    let last = bins.len().saturating_sub(1).max(1) as f32;
    let x = |i: usize| i as f32 * 128.0 / last;
    let mut out = String::with_capacity(bins.len() * 28);
    for (i, &(_, hi)) in bins.iter().enumerate() {
        let _ = write!(
            out,
            "{} {:.1} {:.1} ",
            if i == 0 { 'M' } else { 'L' },
            x(i),
            y(hi)
        );
    }
    for (i, &(lo, _)) in bins.iter().enumerate().rev() {
        let _ = write!(out, "L {:.1} {:.1} ", x(i), y(lo));
    }
    out.push('Z');
    out
}
