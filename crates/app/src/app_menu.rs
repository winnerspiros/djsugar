//! The app's menus, written once: [`MENUS`] lists File, Edit and View with their
//! items, shortcuts and submenus. macOS shows them in the native menu bar
//! (see [`crate::mac_menu`]); Windows and Linux open the same items from
//! the ☰ button at the left of the top bar, so the window spends no height
//! on a menu strip.
//!
//! Every item runs through [`run`], whichever way it was picked. An item's
//! shortcut is only shown beside it: the window's own key handling does the
//! work for the key (main.slint), so pressing it never runs both.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::{AppMenu, AppWindow, LibraryEdit, MenuEntry, Preferences};

/// What a menu item does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Settings,
    Quit,
    Undo,
    Find,
    ExportM3u,
    ExportUsb,
    ExportSpotify,
    ExportRekordboxXml,
    ExportTraktorNml,
    ExportSerato,
    ExportVirtualDj,
    ExportMixxx,
    ExportMixcloud,
    RecordMix,
    MiniPlayer,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    TallerWaveforms,
    ShorterWaveforms,
    ResetWaveformHeight,
    /// The whole guided tour.
    Tour,
    /// One part of the tour, by its place in tour::PART_NAMES.
    TourPart0,
    TourPart1,
    TourPart2,
    TourPart3,
    /// A tutorial, by its place in tutorials::TUTORIALS.
    Tutorial0,
    Tutorial1,
    Tutorial2,
    Tutorial3,
    Tutorial4,
    UserGuide,
    ReportProblem,
    SuggestFeature,
    ShowLogs,
    /// Looks for a new version when DJ Sugar starts; a tick.
    CheckForUpdates,
}

/// The tour's parts and the tutorials, in Help's order.
const TOUR_PARTS: [Action; 4] = [
    Action::TourPart0,
    Action::TourPart1,
    Action::TourPart2,
    Action::TourPart3,
];
const TUTORIALS: [Action; 5] = [
    Action::Tutorial0,
    Action::Tutorial1,
    Action::Tutorial2,
    Action::Tutorial3,
    Action::Tutorial4,
];
// One menu item for each part and tutorial.
const _: () = assert!(TOUR_PARTS.len() == crate::tour::PART_NAMES.len());
const _: () = assert!(TUTORIALS.len() == crate::tutorials::TUTORIALS.len());

/// The setting Check for Updates ticks.
const UPDATE_CHECK: &str = "update_check";

/// Where the user guide is.
const USER_GUIDE: &str = "https://github.com/winnerspiros/djsugar/tree/rust/docs/guide";

impl Action {
    const ALL: [Action; 36] = [
        Action::Settings,
        Action::Quit,
        Action::Undo,
        Action::Find,
        Action::ExportM3u,
        Action::ExportUsb,
        Action::ExportSpotify,
        Action::ExportMixcloud,
        Action::ExportRekordboxXml,
        Action::ExportTraktorNml,
        Action::ExportSerato,
        Action::ExportVirtualDj,
        Action::ExportMixxx,
        Action::RecordMix,
        Action::MiniPlayer,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ResetZoom,
        Action::TallerWaveforms,
        Action::ShorterWaveforms,
        Action::ResetWaveformHeight,
        Action::Tour,
        Action::TourPart0,
        Action::TourPart1,
        Action::TourPart2,
        Action::TourPart3,
        Action::Tutorial0,
        Action::Tutorial1,
        Action::Tutorial2,
        Action::Tutorial3,
        Action::Tutorial4,
        Action::UserGuide,
        Action::ReportProblem,
        Action::SuggestFeature,
        Action::ShowLogs,
        Action::CheckForUpdates,
    ];

    /// Its number, as the ☰ menu and the macOS menu items carry it.
    pub fn id(self) -> i32 {
        Self::ALL.iter().position(|a| *a == self).unwrap_or(0) as i32
    }

    pub fn from_id(id: i32) -> Option<Self> {
        usize::try_from(id)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    /// Off until there is something for it to do: Undo until a change,
    /// the exports until the library picks a list they work on.
    pub fn starts_enabled(self) -> bool {
        matches!(
            self,
            Action::Settings
                | Action::Quit
                | Action::Find
                | Action::Tour
                | Action::TourPart0
                | Action::TourPart1
                | Action::TourPart2
                | Action::TourPart3
                | Action::Tutorial0
                | Action::Tutorial1
                | Action::Tutorial2
                | Action::Tutorial3
                | Action::Tutorial4
                | Action::UserGuide
                | Action::ReportProblem
                | Action::SuggestFeature
                | Action::ShowLogs
                | Action::CheckForUpdates
                | Action::RecordMix
                | Action::MiniPlayer
                | Action::ZoomIn
                | Action::ZoomOut
                | Action::ResetZoom
                | Action::TallerWaveforms
                | Action::ShorterWaveforms
                | Action::ResetWaveformHeight
        )
    }
}

/// A line of a menu.
pub enum Entry {
    /// `key` is the shortcut's letter, with ⌘ on macOS and Ctrl elsewhere.
    Item {
        action: Action,
        title: &'static str,
        key: Option<&'static str>,
    },
    Separator,
    Submenu {
        title: &'static str,
        entries: &'static [Entry],
    },
}

pub struct Menu {
    pub title: &'static str,
    pub entries: &'static [Entry],
}

pub const MENUS: &[Menu] = &[
    Menu {
        title: "File",
        entries: &[
            // Starts or stops recording, as the REC button does (Player
            // shows that button only while recording).
            Entry::Item {
                action: Action::RecordMix,
                title: "Record Mix",
                key: None,
            },
            Entry::Separator,
            Entry::Submenu {
                title: "Export",
                entries: &[
                    Entry::Item {
                        action: Action::ExportM3u,
                        title: "M3U8…",
                        key: None,
                    },
                    Entry::Item {
                        action: Action::ExportUsb,
                        title: "USB…",
                        key: None,
                    },
                    Entry::Item {
                        action: Action::ExportSpotify,
                        title: "Spotify Playlist",
                        key: None,
                    },
                    Entry::Item {
                        action: Action::ExportRekordboxXml,
                        title: "rekordbox XML…",
                        key: None,
                    },
                    Entry::Item {
                        action: Action::ExportTraktorNml,
                        title: "Traktor NML…",
                        key: None,
                    },
                    Entry::Item {
                        action: Action::ExportSerato,
                        title: "Serato DJ…",
                        key: None,
                    },
                    Entry::Item {
                        action: Action::ExportVirtualDj,
                        title: "VirtualDJ…",
                        key: None,
                    },
                    Entry::Item {
                        action: Action::ExportMixxx,
                        title: "Mixxx…",
                        key: None,
                    },
                    Entry::Separator,
                    Entry::Item {
                        action: Action::ExportMixcloud,
                        title: "Mixcloud…",
                        key: None,
                    },
                ],
            },
        ],
    },
    Menu {
        title: "Edit",
        entries: &[
            Entry::Item {
                action: Action::Undo,
                title: "Undo",
                key: Some("z"),
            },
            Entry::Separator,
            Entry::Item {
                action: Action::Find,
                title: "Find",
                key: Some("f"),
            },
        ],
    },
    Menu {
        title: "View",
        entries: &[
            // ⇧⌘M (Ctrl+Shift+M), handled by the window's keys; the
            // shortcut text here has no Shift.
            Entry::Item {
                action: Action::MiniPlayer,
                title: "Mini Player",
                key: None,
            },
            Entry::Separator,
            // The large waveforms' zoom. ⌘= / ⌘− / ⌘0 (Ctrl elsewhere) are
            // the window's keys; a key here too would zoom twice on macOS.
            Entry::Item {
                action: Action::ZoomIn,
                title: "Zoom Waveforms In",
                key: None,
            },
            Entry::Item {
                action: Action::ZoomOut,
                title: "Zoom Waveforms Out",
                key: None,
            },
            Entry::Item {
                action: Action::ResetZoom,
                title: "Reset Waveform Zoom",
                key: None,
            },
            Entry::Separator,
            // How tall they are in this layout; ⌥⌘↑ / ⌥⌘↓ are the
            // window's keys, as for the zoom.
            Entry::Item {
                action: Action::TallerWaveforms,
                title: "Taller Waveforms",
                key: None,
            },
            Entry::Item {
                action: Action::ShorterWaveforms,
                title: "Shorter Waveforms",
                key: None,
            },
            Entry::Item {
                action: Action::ResetWaveformHeight,
                title: "Reset Waveform Height",
                key: None,
            },
        ],
    },
    Menu {
        title: "Help",
        entries: &[
            Entry::Item {
                action: Action::Tour,
                title: "Take the Tour",
                key: None,
            },
            Entry::Submenu {
                title: "Tour Part",
                entries: &[
                    Entry::Item {
                        action: Action::TourPart0,
                        title: crate::tour::PART_NAMES[0],
                        key: None,
                    },
                    Entry::Item {
                        action: Action::TourPart1,
                        title: crate::tour::PART_NAMES[1],
                        key: None,
                    },
                    Entry::Item {
                        action: Action::TourPart2,
                        title: crate::tour::PART_NAMES[2],
                        key: None,
                    },
                    Entry::Item {
                        action: Action::TourPart3,
                        title: crate::tour::PART_NAMES[3],
                        key: None,
                    },
                ],
            },
            Entry::Submenu {
                title: "Tutorials",
                entries: &[
                    Entry::Item {
                        action: Action::Tutorial0,
                        title: crate::tutorials::TUTORIALS[0].name,
                        key: None,
                    },
                    Entry::Item {
                        action: Action::Tutorial1,
                        title: crate::tutorials::TUTORIALS[1].name,
                        key: None,
                    },
                    Entry::Item {
                        action: Action::Tutorial2,
                        title: crate::tutorials::TUTORIALS[2].name,
                        key: None,
                    },
                    Entry::Item {
                        action: Action::Tutorial3,
                        title: crate::tutorials::TUTORIALS[3].name,
                        key: None,
                    },
                    Entry::Item {
                        action: Action::Tutorial4,
                        title: crate::tutorials::TUTORIALS[4].name,
                        key: None,
                    },
                ],
            },
            Entry::Item {
                action: Action::UserGuide,
                title: "User Guide",
                key: None,
            },
            Entry::Separator,
            Entry::Item {
                action: Action::ReportProblem,
                title: "Report a Problem…",
                key: None,
            },
            Entry::Item {
                action: Action::SuggestFeature,
                title: "Suggest a Feature…",
                key: None,
            },
            Entry::Item {
                action: Action::ShowLogs,
                title: "Show Logs and Crash Reports",
                key: None,
            },
        ],
    },
];

/// The app's own items, outside File and Edit: in macOS's app menu
/// (DJ Sugar, whose Quit Slint adds), and on Windows and Linux at the foot
/// of the ☰ list, under the menus' names.
pub const APP_ITEMS: &[Entry] = &[
    Entry::Item {
        action: Action::Settings,
        title: "Settings…",
        key: Some(","),
    },
    Entry::Item {
        action: Action::CheckForUpdates,
        title: "Check for Updates Automatically",
        key: None,
    },
    Entry::Item {
        action: Action::Quit,
        title: "Quit",
        key: Some("q"),
    },
];

/// The entries this platform shows, without separators left at either end
/// or doubled where items were left out.
pub fn shown(entries: &'static [Entry]) -> Vec<&'static Entry> {
    let mut out: Vec<&Entry> = Vec::new();
    for e in entries {
        match e {
            Entry::Submenu { entries, .. } if shown(entries).is_empty() => {}
            Entry::Separator if out.last().is_none_or(|l| matches!(l, Entry::Separator)) => {}
            _ => out.push(e),
        }
    }
    while out.last().is_some_and(|l| matches!(l, Entry::Separator)) {
        out.pop();
    }
    out
}

/// How a shortcut reads beside its item: ⌘Z on macOS, Ctrl+Z elsewhere.
pub fn shortcut_text(key: &str) -> String {
    let key = key.to_uppercase();
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

/// The ☰ menu's lines, in the order they show: each menu's items, then the
/// items of its submenus, each pointing at the line that opens it; then
/// the app's own items (menu -1), listed under the menus' names.
pub fn entries() -> Vec<MenuEntry> {
    fn add(out: &mut Vec<MenuEntry>, menu: i32, parent: i32, entries: &'static [Entry]) {
        let mut top = 0.0;
        for e in shown(entries) {
            let mut line = MenuEntry {
                menu,
                parent,
                top,
                enabled: true,
                action: -1,
                ..Default::default()
            };
            match e {
                Entry::Item { action, key, .. } => {
                    let (title, enabled) = state(*action);
                    line.kind = 0;
                    line.action = action.id();
                    line.text = title.into();
                    line.hint = match (checked(*action), tutorial(*action)) {
                        // A tick shows where a shortcut would.
                        (Some(on), _) => if on { "✓" } else { "" }.into(),
                        // A tutorial: ticked once done, else how long it takes.
                        (_, Some(t)) if crate::tutorials::done(t.id) => "✓".into(),
                        (_, Some(t)) => format!("{} min", t.minutes).into(),
                        _ => key.map(shortcut_text).unwrap_or_default().into(),
                    };
                    line.enabled = enabled;
                    out.push(line);
                    top += LINE_HEIGHT;
                }
                Entry::Separator => {
                    line.kind = 1;
                    out.push(line);
                    top += SEPARATOR_HEIGHT;
                }
                Entry::Submenu { title, entries } => {
                    line.kind = 2;
                    line.text = (*title).into();
                    let at = out.len() as i32;
                    out.push(line);
                    top += LINE_HEIGHT;
                    add(out, menu, at, entries);
                }
            }
        }
    }
    let mut out = Vec::new();
    for (m, menu) in MENUS.iter().enumerate() {
        add(&mut out, m as i32, -1, menu.entries);
    }
    add(&mut out, -1, -1, APP_ITEMS);
    out
}

/// The heights of a line and a separator in the ☰ menu (MenuLine and
/// MenuSeparator in library.slint), to place a submenu beside its line.
const LINE_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;

thread_local! {
    /// The ☰ menu's lines, kept to rename or grey out an item.
    static LINES: RefCell<Option<Rc<VecModel<MenuEntry>>>> = const { RefCell::new(None) };
    /// Items renamed or greyed out since the start, for menus built later.
    static STATE: RefCell<Vec<(Action, String, bool)>> = const { RefCell::new(Vec::new()) };
}

/// An item's title and whether it can be picked, as things stand.
pub fn state(action: Action) -> (String, bool) {
    STATE
        .with(|s| {
            s.borrow()
                .iter()
                .find(|(a, ..)| *a == action)
                .map(|(_, t, e)| (t.clone(), *e))
        })
        .unwrap_or_else(|| (default_title(action), action.starts_enabled()))
}

/// Fills the ☰ menu (Windows and Linux) or the native menus (macOS), with
/// every item running [`run`].
pub fn install(app: &AppWindow) {
    let ui = app.global::<AppMenu>();
    let lines = Rc::new(VecModel::from(entries()));
    let titles: Vec<SharedString> = MENUS
        .iter()
        .enumerate()
        .filter(|(m, _)| lines.iter().any(|l| l.menu == *m as i32))
        .map(|(_, menu)| menu.title.into())
        .collect();
    ui.set_menus(ModelRc::new(VecModel::from(titles)));
    // Menus left empty on this platform keep their number, so the lines
    // still point at the right title.
    let numbers: Vec<i32> = (0..MENUS.len() as i32)
        .filter(|m| lines.iter().any(|l| l.menu == *m))
        .collect();
    for i in 0..lines.row_count() {
        if let Some(mut line) = lines.row_data(i) {
            if line.menu >= 0 {
                line.menu = numbers.iter().position(|m| *m == line.menu).unwrap_or(0) as i32;
            }
            lines.set_row_data(i, line);
        }
    }
    ui.set_entries(ModelRc::from(lines.clone()));
    ui.set_shown(!cfg!(target_os = "macos"));
    ui.set_quit_action(Action::Quit.id());
    LINES.with(|l| *l.borrow_mut() = Some(lines));
    let weak = app.as_weak();
    ui.on_activated(move |id| {
        if let (Some(app), Some(action)) = (weak.upgrade(), Action::from_id(id)) {
            run(&app, action);
        }
    });
}

/// Does what the item says.
pub fn run(app: &AppWindow, action: Action) {
    match action {
        Action::Settings => {
            let prefs = app.global::<Preferences>();
            if !prefs.get_open() {
                prefs.invoke_show(prefs.get_page());
            }
        }
        Action::Quit => {
            let _ = slint::quit_event_loop();
        }
        Action::Undo => {
            let edit = app.global::<LibraryEdit>();
            if !edit.get_undo_text().is_empty() {
                edit.invoke_undo_stage();
            }
        }
        Action::Find => app.invoke_find(),
        // The list picked in the library's sidebar.
        Action::ExportM3u => app
            .global::<LibraryEdit>()
            .invoke_export_m3u(app.get_current_folder()),
        Action::ExportUsb => app
            .global::<LibraryEdit>()
            .invoke_export_usb(app.get_current_folder()),
        Action::ExportSpotify => app
            .global::<LibraryEdit>()
            .invoke_export_spotify(app.get_current_folder()),
        Action::ExportRekordboxXml => app
            .global::<LibraryEdit>()
            .invoke_export_rekordbox(app.get_current_folder()),
        Action::ExportTraktorNml => app
            .global::<LibraryEdit>()
            .invoke_export_traktor(app.get_current_folder()),
        Action::ExportSerato => app
            .global::<LibraryEdit>()
            .invoke_export_serato(app.get_current_folder()),
        Action::ExportVirtualDj => app
            .global::<LibraryEdit>()
            .invoke_export_virtualdj(app.get_current_folder()),
        Action::ExportMixxx => app
            .global::<LibraryEdit>()
            .invoke_export_mixxx(app.get_current_folder()),
        // The mixed set picked in Recordings.
        Action::ExportMixcloud => crate::export_mixcloud(app),
        // [Recording],toggle_recording, as the REC button and controllers.
        Action::MiniPlayer => app.invoke_mini_player(),
        Action::ZoomIn => crate::waveform_zoom::step(app, 1),
        Action::ZoomOut => crate::waveform_zoom::step(app, -1),
        Action::ResetZoom => crate::waveform_zoom::reset(app),
        Action::TallerWaveforms => crate::waveform_height::step(app, 1),
        Action::ShorterWaveforms => crate::waveform_height::step(app, -1),
        Action::ResetWaveformHeight => crate::waveform_height::reset(app),
        Action::Tour => app.global::<crate::Tour>().invoke_start(-1),
        Action::TourPart0 | Action::TourPart1 | Action::TourPart2 | Action::TourPart3 => {
            let part = TOUR_PARTS.iter().position(|a| *a == action).unwrap_or(0);
            app.global::<crate::Tour>().invoke_start(part as i32);
        }
        Action::Tutorial0
        | Action::Tutorial1
        | Action::Tutorial2
        | Action::Tutorial3
        | Action::Tutorial4 => {
            if let Some(t) = tutorial(action) {
                app.global::<crate::Tour>()
                    .invoke_start_tutorial(t.id.into());
            }
        }
        Action::UserGuide => {
            crate::spotify::open_in_browser(USER_GUIDE);
        }
        Action::ReportProblem => {
            crate::spotify::open_in_browser(&crate::diagnostics::issue_url("bug_report.yml"));
        }
        Action::SuggestFeature => {
            crate::spotify::open_in_browser(&crate::diagnostics::issue_url("feature_request.yml"));
        }
        Action::ShowLogs => {
            if let Some(dir) = crate::diagnostics::logs_dir() {
                let _ = std::fs::create_dir_all(&dir);
                crate::files::reveal(&dir);
            }
        }
        Action::CheckForUpdates => {
            let on = !crate::settings::flag(UPDATE_CHECK, true);
            crate::settings::set(UPDATE_CHECK, serde_json::json!(on));
            set_checked(action, on);
        }
        Action::RecordMix => {
            let controls = app.global::<crate::Controls>();
            let index = controls.invoke_resolve("[Recording]".into(), "toggle_recording".into());
            controls.invoke_set_value(index, 1.0);
        }
    }
}

/// The tutorial an item starts.
fn tutorial(action: Action) -> Option<&'static crate::tutorials::Tutorial> {
    TUTORIALS
        .iter()
        .position(|a| *a == action)
        .map(|i| &crate::tutorials::TUTORIALS[i])
}

/// Whether an item that is a tick is on; None for other items.
pub fn checked(action: Action) -> Option<bool> {
    (action == Action::CheckForUpdates).then(|| crate::settings::flag(UPDATE_CHECK, true))
}

/// Ticks an item, or takes its tick off, in whichever menus this platform
/// shows.
fn set_checked(action: Action, on: bool) {
    LINES.with(|l| {
        if let Some(lines) = l.borrow().as_ref() {
            for i in 0..lines.row_count() {
                if let Some(mut line) = lines.row_data(i) {
                    if line.kind == 0 && line.action == action.id() {
                        line.hint = if on { "✓" } else { "" }.into();
                        lines.set_row_data(i, line);
                    }
                }
            }
        }
    });
    #[cfg(target_os = "macos")]
    crate::mac_menu::set_checked(action, on);
}

/// Renames an item, or greys it out, in whichever menus this platform
/// shows. `title` None keeps the one in [`MENUS`].
pub fn set_item(action: Action, title: Option<&str>, enabled: bool) {
    let title = title
        .map(str::to_owned)
        .unwrap_or_else(|| default_title(action));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.retain(|(a, ..)| *a != action);
        s.push((action, title.clone(), enabled));
    });
    LINES.with(|l| {
        if let Some(lines) = l.borrow().as_ref() {
            for i in 0..lines.row_count() {
                if let Some(mut line) = lines.row_data(i) {
                    if line.kind == 0 && line.action == action.id() {
                        line.text = title.as_str().into();
                        line.enabled = enabled;
                        lines.set_row_data(i, line);
                    }
                }
            }
        }
    });
    #[cfg(target_os = "macos")]
    crate::mac_menu::set_item(action, &title, enabled);
}

/// Names Edit › Undo for what it would take back ("Undo Send to In Prep
/// (3 tracks)"), or greys it out when `text` is empty.
pub fn set_undo_text(text: &str) {
    if text.is_empty() {
        set_item(Action::Undo, None, false);
    } else {
        set_item(Action::Undo, Some(text), true);
    }
}

/// What File › Export can do for the list or source the library shows.
pub struct Exports {
    pub m3u: bool,
    pub usb: bool,
    pub spotify: bool,
    pub mixcloud: bool,
    pub rekordbox: bool,
    pub traktor: bool,
    pub serato: bool,
    pub virtualdj: bool,
    pub mixxx: bool,
}

/// Greys File › Export's items out or back in.
pub fn set_exports(e: Exports) {
    thread_local! {
        static LAST: std::cell::Cell<Option<[bool; 9]>> = const { std::cell::Cell::new(None) };
    }
    let now = [
        e.m3u,
        e.usb,
        e.spotify,
        e.mixcloud,
        e.rekordbox,
        e.traktor,
        e.serato,
        e.virtualdj,
        e.mixxx,
    ];
    if LAST.with(|l| l.replace(Some(now))) == Some(now) {
        return;
    }
    set_item(Action::ExportM3u, None, e.m3u);
    set_item(Action::ExportUsb, None, e.usb);
    set_item(Action::ExportSpotify, None, e.spotify);
    set_item(Action::ExportMixcloud, None, e.mixcloud);
    set_item(Action::ExportRekordboxXml, None, e.rekordbox);
    set_item(Action::ExportTraktorNml, None, e.traktor);
    set_item(Action::ExportSerato, None, e.serato);
    set_item(Action::ExportVirtualDj, None, e.virtualdj);
    set_item(Action::ExportMixxx, None, e.mixxx);
}

fn default_title(action: Action) -> String {
    fn find(entries: &'static [Entry], action: Action) -> Option<&'static str> {
        entries.iter().find_map(|e| match e {
            Entry::Item {
                action: a, title, ..
            } if *a == action => Some(*title),
            Entry::Submenu { entries, .. } => find(entries, action),
            _ => None,
        })
    }
    MENUS
        .iter()
        .find_map(|m| find(m.entries, action))
        .or_else(|| find(APP_ITEMS, action))
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for a in Action::ALL {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
        assert_eq!(Action::from_id(-1), None);
        assert_eq!(Action::from_id(99), None);
    }

    #[test]
    fn every_action_is_in_a_menu() {
        for a in Action::ALL {
            assert!(!default_title(a).is_empty(), "{a:?} has no menu item");
        }
    }

    #[test]
    fn no_separator_at_the_ends_or_twice() {
        for entries in MENUS.iter().map(|m| m.entries).chain([APP_ITEMS]) {
            let s = shown(entries);
            assert!(!matches!(s.first(), Some(Entry::Separator)));
            assert!(!matches!(s.last(), Some(Entry::Separator)));
            for pair in s.windows(2) {
                assert!(
                    !(matches!(pair[0], Entry::Separator) && matches!(pair[1], Entry::Separator))
                );
            }
        }
    }

    #[test]
    fn lines_place_submenus_beside_their_line() {
        let lines = entries();
        for l in &lines {
            if l.parent >= 0 {
                assert_eq!(lines[l.parent as usize].kind, 2);
                assert_eq!(lines[l.parent as usize].menu, l.menu);
            }
        }
        // Edit: Undo at 0, the separator under it, Find after that.
        let find = lines
            .iter()
            .find(|l| l.action == Action::Find.id())
            .unwrap();
        assert_eq!(find.top, LINE_HEIGHT + SEPARATOR_HEIGHT);
        // File › Export opens beside its line, holding every export.
        let export = lines
            .iter()
            .position(|l| l.kind == 2 && l.text == "Export")
            .unwrap();
        let inside: Vec<_> = lines
            .iter()
            .filter(|l| l.parent == export as i32 && l.kind == 0)
            .collect();
        // M3U, USB, Spotify, rekordbox, Traktor, Serato, VirtualDJ, Mixxx,
        // Mixcloud.
        assert_eq!(inside.len(), 9);
        // Settings… and Quit sit in the ☰ list itself, under the menus.
        for a in [Action::Settings, Action::Quit] {
            let line = lines.iter().find(|l| l.action == a.id()).unwrap();
            assert_eq!((line.menu, line.parent), (-1, -1));
        }
    }
}
