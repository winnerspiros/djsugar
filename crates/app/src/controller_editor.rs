//! The Controller Editor: shows what a controller's mapping does, by
//! function, with the controller's picture. Reading a mapping means running
//! it in a sandbox (see `djsugar_controllers::probe`), which takes a moment,
//! so it happens on a thread of its own. While the editor is open, messages
//! from the controllers are watched: pressing a control finds it.
//!
//! A selected row is changed in the panel under the list. Changes go into
//! the DJ's own mapping (the first change to the original makes one, "My
//! …"), the controller restarts with it, and the view is read again.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use djsugar_control::ControlRegistry;
use djsugar_controllers::edit::{self, Binding, Buttons, Choice, Does, Encoder};
use djsugar_controllers::format::DjSugarMapping;
use djsugar_controllers::layout::Layout;
use djsugar_controllers::own::{self, OwnMapping};
use djsugar_controllers::probe::{message_key, MessageKey};
use djsugar_controllers::view::{self, LightRow, MappingView, Row};
use djsugar_controllers::{is_own_file, Controllers, MappingChoice};
use djsugar_controllers::{pickup, support};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{
    settings, short_port_name, AppWindow, ControllerEditor, EditorOption, EditorPanel, EditorPart,
    EditorRow, EditorSetting,
};

/// What a row can do, in the row editor's Does menu (see [`Does`]).
const DOES_MODES: [&str; 4] = [
    "A function",
    "An DJSugarScript line",
    "Shift: on while held",
    "Mode: on/off each press",
];

/// The deck orders a four-deck mapping can have, left to right.
pub const DECK_ORDERS: [(&str, [u8; 4]); 2] =
    [("3 1 2 4", [3, 1, 2, 4]), ("1 2 3 4", [1, 2, 3, 4])];

/// How long a part stays lit after its control is pressed.
const FLASH: Duration = Duration::from_millis(350);
/// The list's row heights (FunctionRow in controller_editor.slint).
const ROW_HEIGHT: f32 = 44.0;
const GROUP_HEIGHT: f32 = 38.0;
const CATEGORY_HEIGHT: f32 = 30.0;

/// The sections of the Controls and Lights lists: groups, each holding
/// categories of the function catalog.
const GROUPS: [(&str, &[&str]); 6] = [
    (
        "DECK",
        &["Transport", "Cues", "Jog & scratch", "Tempo & sync", "Key"],
    ),
    (
        "PADS",
        &[
            "Hot cues",
            "Loops",
            "Beat jump",
            "Samplers",
            "Stems",
            "Pads",
        ],
    ),
    ("MIXER", &["Mixer", "EQ & filter"]),
    ("EFFECTS", &["Effects"]),
    ("BROWSER", &["Browser", "Automix"]),
    ("MORE", &["Mic & aux", "Recording", "View", "Controller"]),
];

/// What the dialog over the editor is for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Dialog {
    Duplicate,
    Rename,
    Delete,
    /// What an import or export did; only to read.
    Report,
}

/// The Mapping menu's entries after the mappings themselves.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Duplicate,
    Rename,
    Delete,
    Import,
    Export,
    ExportMixxx,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::Duplicate => "Duplicate…",
            Action::Rename => "Rename…",
            Action::Delete => "Delete…",
            Action::Import => "Import…",
            Action::Export => "Export…",
            Action::ExportMixxx => "Export for Mixxx…",
        }
    }
}

/// A file or folder being picked in the system's dialog, for what.
enum Picking {
    Import(Receiver<PathBuf>),
    Export(Receiver<PathBuf>, bool),
}

/// What reading a mapping gives: its view, and the functions a row can be
/// given.
type Read = (Result<MappingView, String>, Vec<Choice>);

/// A row of the list: a section heading (by its key) or a row of the view.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    Heading(String),
    Row(usize),
}

pub struct Editor {
    app: slint::Weak<AppWindow>,
    controllers: Option<Rc<Controllers>>,
    dir: Option<PathBuf>,
    /// The mappings in the menu, by file name, and their names.
    choices: Vec<(String, String)>,
    /// The mapping being read, and where its view will come from.
    loading: Option<(String, Receiver<Read>)>,
    /// The mapping is read again after a change: the list stays as it was,
    /// and this row (its first message and layer) is selected again.
    reselect: Option<Option<(MessageKey, String)>>,
    /// The functions a row can be given.
    functions: Vec<Choice>,
    /// The function picker's rows: a category heading, or a function.
    picker: Vec<Option<usize>>,
    view: Option<MappingView>,
    /// The list narrowed to this part of the picture (or control name, when
    /// there is no picture).
    part: Option<String>,
    /// The selected row, as (tab, index into the view's rows or lights).
    selected: Option<(i32, usize)>,
    /// What each row of the list shown is.
    shown: Vec<Shown>,
    /// The sections closed, as "tab/group" or "tab/group/category".
    collapsed: HashSet<String>,
    /// The DJ's own mappings based on the one shown, and the one in use
    /// (None for the original).
    own: Vec<OwnMapping>,
    version: Option<String>,
    dialog: Option<Dialog>,
    /// The Mapping menu's entries after the mappings.
    actions: Vec<Action>,
    picking: Option<Picking>,
    flash: Option<(String, Instant)>,
    /// Buttons held down on the controller, so a control pressed while
    /// SHIFT (or another layer's button) is held finds that layer's row.
    held: HashSet<MessageKey>,
    scroll_serial: i32,
    was_open: bool,
    /// The last query and tab the list was filled for.
    filled_for: (String, i32),
}

impl Editor {
    pub fn install(app: &AppWindow, controllers: Option<Rc<Controllers>>) -> Rc<RefCell<Editor>> {
        let editor = Rc::new(RefCell::new(Editor {
            app: app.as_weak(),
            controllers,
            dir: djsugar_controllers::default_mappings_dir(),
            choices: Vec::new(),
            loading: None,
            reselect: None,
            functions: Vec::new(),
            picker: Vec::new(),
            view: None,
            part: None,
            selected: None,
            shown: Vec::new(),
            collapsed: HashSet::new(),
            own: Vec::new(),
            version: None,
            dialog: None,
            actions: Vec::new(),
            picking: None,
            flash: None,
            held: HashSet::new(),
            scroll_serial: 0,
            was_open: false,
            filled_for: (String::new(), 0),
        }));
        let g = app.global::<ControllerEditor>();
        let e = editor.clone();
        g.on_show(move || e.borrow_mut().show());
        let e = editor.clone();
        g.on_show_mapping(move |file| e.borrow_mut().show_mapping(&file));
        let e = editor.clone();
        g.on_reopen(move || e.borrow_mut().reopen());
        let e = editor.clone();
        g.on_pick(move |i| e.borrow_mut().pick(i.max(0) as usize));
        let e = editor.clone();
        g.on_pick_version(move |i| e.borrow_mut().pick_version(i.max(0) as usize));
        let e = editor.clone();
        g.on_dialog_done(move |ok| e.borrow_mut().dialog_done(ok));
        let e = editor.clone();
        g.on_part_clicked(move |id| e.borrow_mut().part_clicked(&id));
        let e = editor.clone();
        g.on_row_clicked(move |tab, i| e.borrow_mut().row_clicked(tab, i.max(0) as usize));
        let e = editor.clone();
        g.on_searched(move || e.borrow_mut().fill());
        let e = editor.clone();
        g.on_clear_filter(move || {
            let mut e = e.borrow_mut();
            e.part = None;
            e.fill();
        });
        let e = editor.clone();
        g.on_open_picker(move || e.borrow_mut().open_picker());
        let e = editor.clone();
        g.on_picker_searched(move || e.borrow_mut().fill_picker());
        let e = editor.clone();
        g.on_picker_picked(move |i| e.borrow_mut().picker_picked(i.max(0) as usize));
        let e = editor.clone();
        g.on_set_target(move |i| {
            let mut e = e.borrow_mut();
            let Some(choice) = e.current_choice() else {
                return;
            };
            let Some((_, control)) = choice.targets.get(i.max(0) as usize).cloned() else {
                return;
            };
            e.change(|b| b.control = Some(control));
        });
        let e = editor.clone();
        g.on_set_when(move |i| {
            let when = edit::WHENS[(i.max(0) as usize).min(edit::WHENS.len() - 1)];
            let mut e = e.borrow_mut();
            match e.does() {
                Some(Does::Script(line)) => {
                    e.apply(move |own, row| edit::set_does_at(own, row, &Does::Script(line), when))
                }
                _ => e.change(|b| b.when = when),
            }
        });
        let e = editor.clone();
        g.on_set_does(move |i| e.borrow_mut().set_does(i));
        let e = editor.clone();
        g.on_script_edited(move || e.borrow().check_script());
        let e = editor.clone();
        g.on_script_done(move || e.borrow_mut().script_done());
        let e = editor.clone();
        g.on_var_done(move || e.borrow_mut().var_done());
        let e = editor.clone();
        g.on_layer_edited(move || e.borrow().check_layer());
        let e = editor.clone();
        g.on_layer_done(move |moving| e.borrow_mut().layer_done(moving));
        let e = editor.clone();
        g.on_set_buttons(move |i| {
            let m = Buttons::ALL[(i.max(0) as usize).min(Buttons::ALL.len() - 1)];
            e.borrow_mut().change(|b| b.buttons = m);
        });
        let e = editor.clone();
        g.on_set_encoder(move |i| {
            let m = Encoder::ALL[(i.max(0) as usize).min(Encoder::ALL.len() - 1)];
            e.borrow_mut().change(|b| b.encoder = m);
        });
        let e = editor.clone();
        g.on_set_invert(move |on| e.borrow_mut().change(|b| b.invert = on));
        let e = editor.clone();
        g.on_set_nothing(move || e.borrow_mut().change(|b| b.control = None));
        let e = editor.clone();
        g.on_reset_row(move || e.borrow_mut().apply(edit::reset));
        let e = editor.clone();
        g.on_set_playing(move |id, i| e.borrow_mut().set_playing(&id, i.max(0) as usize));
        g.set_whens(labels(edit::WHENS.iter().map(|w| edit::when_label(*w))));
        g.set_button_modes(labels(Buttons::ALL.iter().map(|b| b.label())));
        g.set_encoder_modes(labels(Encoder::ALL.iter().map(|e| e.label())));
        g.set_does_modes(labels(DOES_MODES.iter().copied()));
        editor
    }

    /// The controllers plugged in, with the mapping each runs: its file
    /// name (unknown for a stand-in device) and its name.
    fn running(&self) -> Vec<(String, Option<String>, String)> {
        self.controllers
            .as_ref()
            .map(|c| {
                c.ports()
                    .into_iter()
                    // HID mappings are all script: nothing to show or edit.
                    .filter(|p| !p.hid)
                    .filter_map(|p| Some((p.name, p.mapping_file, p.running?)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The ports running the mapping shown.
    fn live_ports(&self) -> Vec<String> {
        let Some(v) = &self.view else {
            return Vec::new();
        };
        // Running the original, or an own mapping (named its own way).
        let original = self
            .choices
            .iter()
            .find(|(f, _)| *f == v.file)
            .map(|(_, n)| n.as_str());
        self.running()
            .into_iter()
            .filter(|(_, file, name)| {
                file.as_ref() == Some(&v.file) || *name == v.name || Some(name.as_str()) == original
            })
            .map(|(p, _, _)| p)
            .collect()
    }

    fn show(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let Some(c) = &self.controllers else {
            g.set_open(true);
            g.set_status("Controller mappings were not found.".into());
            return;
        };
        // The controllers plugged in first, then those with a picture, then
        // the rest by name.
        let running: Vec<(Option<String>, String)> =
            self.running().into_iter().map(|(_, f, n)| (f, n)).collect();
        let runs = |file: &str, name: &str| {
            running
                .iter()
                .any(|(f, n)| f.as_deref() == Some(file) || n == name)
        };
        let pictured = Layout::files();
        let mut choices = c.mappings();
        // Mappings of the DJ's own that stand alone (made with Quick map).
        choices.extend(
            settings::own_mappings_dir()
                .map(|d| own::all(&d))
                .unwrap_or_default()
                .into_iter()
                .filter(|m| m.based_on.is_empty())
                .map(|m| (m.name, format!("{}.json", m.id))),
        );
        choices.sort_by_key(|(name, file)| {
            (
                !runs(file, name),
                !pictured.contains(file),
                name.to_lowercase(),
            )
        });
        self.choices = choices.into_iter().map(|(n, f)| (f, n)).collect();
        let names: Vec<SharedString> = self.choices.iter().map(|(_, n)| n.into()).collect();
        g.set_mappings(ModelRc::new(VecModel::from(names)));
        g.set_open(true);
        let current = self.view.as_ref().map(|v| (v.file.clone(), v.name.clone()));
        let keep = current.filter(|(f, n)| running.is_empty() || runs(f, n));
        let i = keep
            .and_then(|(f, _)| self.choices.iter().position(|(c, _)| *c == f))
            .unwrap_or(0);
        if self.loading.is_none() && (self.view.is_none() || self.chosen() != Some(i)) {
            self.pick(i);
        } else {
            g.set_chosen(i as i32);
        }
    }

    /// Opens the editor on the mapping in `file`, or as [`Self::show`] does
    /// when it isn't one of the mappings.
    fn show_mapping(&mut self, file: &str) {
        self.show();
        let Some(i) = self.choices.iter().position(|(f, _)| f == file) else {
            return;
        };
        let reading = self.loading.as_ref().is_some_and(|(f, _)| f == file);
        if !reading && self.chosen() != Some(i) {
            self.pick(i);
        }
    }

    /// Opens the editor again after Map mode, which may have changed the
    /// mapping shown or made the DJ's own copy of it.
    fn reopen(&mut self) {
        self.show();
        if self.loading.is_none() && self.view.is_some() {
            if let Some(base) = self.base() {
                self.versions(&base);
                self.read(true);
            }
        }
    }

    fn chosen(&self) -> Option<usize> {
        let file = &self.view.as_ref()?.file;
        self.choices.iter().position(|(f, _)| f == file)
    }

    /// Reads the mapping at `index` in the menu.
    fn pick(&mut self, index: usize) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), self.dir.clone()) else {
            return;
        };
        let Some((file, name)) = self.choices.get(index).cloned() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        g.set_chosen(index as i32);
        g.set_name(name.into());
        self.versions(&file);
        g.set_author("".into());
        g.set_support(
            support::status(&file)
                .map(|s| s.label())
                .unwrap_or("")
                .into(),
        );
        let _ = dir;
        self.read(false);
    }

    /// Reads the mapping shown (the original or the DJ's own) on a thread.
    /// `again` keeps the list as it is meanwhile and selects the same row
    /// once read: after a change, or a switch between original and own.
    fn read(&mut self, again: bool) {
        let (Some(app), Some(dir), Some(file)) =
            (self.app.upgrade(), self.dir.clone(), self.base())
        else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        if again {
            self.reselect = Some(self.selected_row().map(|r| (r.keys[0], r.layer.clone())));
        } else {
            self.reselect = None;
            g.set_loading(true);
            g.set_status("Reading the mapping…".into());
            self.view = None;
            self.part = None;
            self.selected = None;
            self.fill();
            self.picture();
        }
        let own: Option<DjSugarMapping> = self.current_own().map(|m| m.mapping.clone());
        let (tx, rx) = mpsc::channel();
        let path = match settings::own_mappings_dir() {
            Some(d) if is_own_file(&file) => d.join(&file),
            _ => dir.join(&file),
        };
        std::thread::Builder::new()
            .name("controller-editor".into())
            .spawn(move || {
                // A sandbox with every control the app has, so the mapping
                // runs as it would for real without touching the decks.
                let registry = ControlRegistry::new();
                djsugar_engine::create_controls(&registry);
                let _library = crate::browser::LibraryControls::create(&registry);
                let _automix = crate::automix::AutoMix::new(&registry);
                let functions = edit::choices(&registry);
                let v = match own {
                    Some(own) => view::build_own(&path, &own, registry),
                    None => view::build(&path, registry),
                };
                let _ = tx.send((v, functions));
            })
            .expect("spawn controller editor thread");
        self.loading = Some((file, rx));
    }

    /// Uses the own mapping `id` for the base mapping `base`, or the
    /// original for `None`, here and on the controller.
    fn use_version(&self, base: &str, id: Option<&str>) {
        settings::set_controller_version(base, id);
        if let (Some(c), Some(dir)) = (&self.controllers, settings::own_mappings_dir()) {
            c.set_version(base, id.map(|id| own::path(&dir, id)));
        }
    }

    /// The base mapping shown, by file name.
    fn base(&self) -> Option<String> {
        let app = self.app.upgrade()?;
        let i = app.global::<ControllerEditor>().get_chosen();
        self.choices.get(i.max(0) as usize).map(|(f, _)| f.clone())
    }

    /// Fills the Mapping menu for the base mapping `base`: the original,
    /// the DJ's own copies of it, then what can be done with them.
    fn versions(&mut self, base: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        if is_own_file(base) {
            // One that stands alone: there's no original to go back to.
            let id = base.trim_end_matches(".json");
            self.own = settings::own_mappings_dir()
                .and_then(|d| OwnMapping::read(&own::path(&d, id)))
                .into_iter()
                .collect();
            self.version = self.own.first().map(|m| m.id.clone());
            let name = self.own.first().map(|m| m.name.clone()).unwrap_or_default();
            self.actions = vec![
                Action::Rename,
                Action::Delete,
                Action::Export,
                Action::ExportMixxx,
            ];
            let mut items: Vec<SharedString> = vec![name.as_str().into()];
            items.extend(self.actions.iter().map(|a| SharedString::from(a.label())));
            g.set_versions(ModelRc::new(VecModel::from(items)));
            g.set_version(0);
            g.set_version_name(name.into());
            g.set_original(false);
            return;
        }
        self.own = settings::own_mappings_dir()
            .map(|d| own::list(&d, base))
            .unwrap_or_default();
        self.version =
            settings::controller_version(base).filter(|id| self.own.iter().any(|m| &m.id == id));
        let mut items: Vec<SharedString> = vec!["Original mapping".into()];
        items.extend(self.own.iter().map(|m| SharedString::from(m.name.as_str())));
        self.actions = vec![Action::Duplicate];
        if self.version.is_some() {
            self.actions.extend([Action::Rename, Action::Delete]);
        }
        self.actions.push(Action::Import);
        if self.version.is_some() {
            self.actions.extend([Action::Export, Action::ExportMixxx]);
        }
        items.extend(self.actions.iter().map(|a| SharedString::from(a.label())));
        g.set_versions(ModelRc::new(VecModel::from(items)));
        let current = self
            .version
            .as_ref()
            .and_then(|id| self.own.iter().position(|m| &m.id == id));
        g.set_version(current.map_or(0, |i| i + 1) as i32);
        g.set_version_name(
            current
                .map_or("Original mapping".to_string(), |i| self.own[i].name.clone())
                .into(),
        );
        g.set_original(current.is_none());
    }

    fn current_own(&self) -> Option<&OwnMapping> {
        let id = self.version.as_ref()?;
        self.own.iter().find(|m| &m.id == id)
    }

    fn pick_version(&mut self, index: usize) {
        let (Some(app), Some(base)) = (self.app.upgrade(), self.base()) else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let alone = is_own_file(&base);
        let n = if alone { 0 } else { self.own.len() };
        let open_dialog = |kind: Dialog, title: &str, detail: &str, action: &str, text: String| {
            g.set_dialog_title(title.into());
            g.set_dialog_detail(detail.into());
            g.set_dialog_action(action.into());
            g.set_dialog_text(text.into());
            g.set_dialog(match kind {
                Dialog::Delete => 2,
                Dialog::Report => 3,
                _ => 1,
            });
            kind
        };
        let current_name = self
            .current_own()
            .map_or(g.get_name().to_string(), |m| m.name.clone());
        let action = index
            .checked_sub(n + 1)
            .and_then(|i| self.actions.get(i).copied());
        let before = self.version.clone();
        match (index, action) {
            (0, _) if alone => {}
            (0, _) => self.use_version(&base, None),
            (i, _) if i <= n => self.use_version(&base, Some(&self.own[i - 1].id)),
            (_, Some(Action::Duplicate)) => {
                let taken: Vec<String> = self.own.iter().map(|m| m.name.clone()).collect();
                self.dialog = Some(open_dialog(
                    Dialog::Duplicate,
                    "Duplicate mapping",
                    &format!(
                        "A copy of {current_name} that you can name, and later change. The original stays as it is."
                    ),
                    "Duplicate",
                    own::copy_name(&current_name, &taken),
                ));
            }
            (_, Some(Action::Rename)) => {
                self.dialog = Some(open_dialog(
                    Dialog::Rename,
                    "Rename mapping",
                    "",
                    "Rename",
                    current_name,
                ));
            }
            (_, Some(Action::Delete)) => {
                self.dialog = Some(open_dialog(
                    Dialog::Delete,
                    &format!("Delete {current_name}?"),
                    "The controller goes back to the original mapping. This can't be undone.",
                    "Delete",
                    String::new(),
                ));
            }
            (_, Some(Action::Import)) => {
                let (tx, rx) = mpsc::channel();
                crate::prefs::choose_file(
                    "Choose a mapping from VirtualDJ, Traktor, rekordbox, djay Pro or DJ Sugar",
                    move |p| {
                        let _ = tx.send(p);
                    },
                );
                self.picking = Some(Picking::Import(rx));
            }
            (_, Some(a @ (Action::Export | Action::ExportMixxx))) => {
                let (tx, rx) = mpsc::channel();
                crate::prefs::choose_folder("Choose where to save the mapping", move |p| {
                    let _ = tx.send(p);
                });
                self.picking = Some(Picking::Export(rx, a == Action::ExportMixxx));
            }
            _ => {}
        }
        self.versions(&base);
        if self.version != before {
            self.read(true);
        }
    }

    fn dialog_done(&mut self, ok: bool) {
        let (Some(app), Some(base), Some(dir)) = (
            self.app.upgrade(),
            self.base(),
            settings::own_mappings_dir(),
        ) else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        g.set_dialog(0);
        let Some(kind) = self.dialog.take() else {
            return;
        };
        let name = g.get_dialog_text().trim().to_string();
        if kind == Dialog::Report || !ok || (kind != Dialog::Delete && name.is_empty()) {
            return;
        }
        let id = self.version.clone();
        let result = match (kind, &id) {
            (Dialog::Duplicate, _) => own::create(&dir, &name, &base, self.current_own())
                .map(|m| self.use_version(&base, Some(&m.id))),
            (Dialog::Rename, Some(id)) => own::rename(&dir, id, &name),
            (Dialog::Delete, Some(id)) if is_own_file(&base) => {
                // Controllers using it go back to the mapping that matches.
                let path = own::path(&dir, id);
                if let Some(c) = &self.controllers {
                    for (port, choice) in c.choices() {
                        if choice == MappingChoice::Own(path.clone()) {
                            c.set_choice(&port, MappingChoice::Auto);
                        }
                    }
                    settings::set_controller_mappings(&c.choices());
                }
                own::delete(&dir, id)
            }
            (Dialog::Delete, Some(id)) => {
                own::delete(&dir, id).map(|_| self.use_version(&base, None))
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            g.set_status(format!("Couldn't save the mapping: {e}").into());
        }
        self.versions(&base);
        if self.version != id {
            self.read(true);
        }
    }

    /// Shows what an import or export did.
    fn report(&mut self, title: &str, detail: String) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        g.set_dialog_title(title.into());
        g.set_dialog_detail(detail.into());
        g.set_dialog_action("OK".into());
        g.set_dialog_text("".into());
        g.set_dialog(3);
        self.dialog = Some(Dialog::Report);
    }

    /// Reads a mapping from another app (or a shared DJ Sugar one) into a new
    /// mapping of the DJ's own, and switches the controller to it.
    fn import(&mut self, path: &std::path::Path) {
        let (Some(base), Some(dir)) = (self.base(), settings::own_mappings_dir()) else {
            return;
        };
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let result = if file.ends_with(".djsugar-mapping") {
            djsugar_controllers::export::read_djsugar(path, &dir).map(|m| {
                djsugar_controllers::import::Import {
                    mappings: vec![m],
                    ..Default::default()
                }
            })
        } else {
            djsugar_controllers::import::import_file(path)
        };
        let import = match result {
            Ok(i) => i,
            Err(e) => {
                self.report("Couldn't import this mapping", e);
                return;
            }
        };
        let mut lines = Vec::new();
        let summary = import.summary();
        if !summary.is_empty() {
            lines.push(summary);
        }
        let mut first = None;
        for m in import.mappings {
            // Rows it doesn't have stay as the mapping shown does them.
            let based_on = m.based_on.clone().unwrap_or_else(|| base.clone());
            let name = m.name.clone();
            let off = greyed(&m);
            match own::save_new(&dir, &name, &based_on, m) {
                Ok(saved) => {
                    if based_on == base && first.is_none() {
                        first = Some(saved.id.clone());
                    }
                    if !off.is_empty() {
                        lines.push(format!("Greyed in {name}:\n{off}"));
                    }
                }
                Err(e) => lines.push(format!("Couldn't save {name}: {e}")),
            }
        }
        if !import.shortcuts.is_empty() {
            let set = crate::shortcuts::import_set(&file, &import.shortcuts);
            lines.push(format!(
                "The keys are a new keyboard shortcut set, {set}, now in use; Settings › Keyboard switches sets."
            ));
        }
        if !import.skipped_shortcuts.is_empty() {
            let keys: Vec<String> = import
                .skipped_shortcuts
                .iter()
                .take(12)
                .map(|(k, a)| format!("{k}: {a}"))
                .collect();
            lines.push(format!("Keys left as they were:\n{}", keys.join("\n")));
        }
        lines.extend(import.notes);
        if let Some(id) = &first {
            self.use_version(&base, Some(id));
        }
        self.versions(&base);
        if first.is_some() {
            self.read(true);
        }
        self.report(&format!("Imported {file}"), lines.join("\n\n"));
    }

    /// Writes the own mapping in use as a shared `.djsugar-mapping`, or as a
    /// Mixxx-format mapping with its scripts, into `folder`.
    fn export(&mut self, folder: &std::path::Path, mixxx: bool) {
        let (Some(own_dir), Some(shipped)) = (settings::own_mappings_dir(), self.dir.clone())
        else {
            return;
        };
        let Some(current) = self.current_own().cloned() else {
            return;
        };
        let mut m = current.mapping.clone();
        m.name = current.name.clone();
        let result = if mixxx {
            let base = m
                .based_on
                .as_ref()
                .and_then(|b| djsugar_controllers::Mapping::load(&shipped.join(b)).ok());
            let built = m.build(base.as_ref(), &shipped);
            djsugar_controllers::export::write_mixxx(&m, &built, folder)
        } else {
            let stem: String = m
                .name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == ' ' || c == '-' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect();
            let path = folder.join(format!("{}.djsugar-mapping", stem.trim()));
            djsugar_controllers::export::write_djsugar(&m, &shipped, &own_dir, &path)
        };
        match result {
            Ok(r) => {
                let mut detail = format!(
                    "Saved {}.",
                    r.written
                        .iter()
                        .filter_map(|p| p.file_name())
                        .map(|f| f.to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                if !r.left_out.is_empty() {
                    let shown: Vec<&String> = r.left_out.iter().take(12).collect();
                    detail.push_str(&format!(
                        "\n\nMixxx can't hold {} of the rows:\n{}",
                        r.left_out.len(),
                        shown
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
                self.report("Exported", detail);
            }
            Err(e) => self.report("Couldn't export", e.to_string()),
        }
    }

    /// Called every frame.
    pub fn poll(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let open = g.get_open();
        if open != self.was_open {
            self.was_open = open;
            // Map mode watches the controllers too, and closes the editor
            // when it starts.
            let mapping = app.global::<crate::MapMode>().get_active();
            if let Some(c) = &self.controllers {
                c.set_monitor(open || mapping);
            }
        }
        if !open {
            return;
        }
        let picked = match &self.picking {
            Some(Picking::Import(rx)) => rx.try_recv().ok().map(|p| (p, None)),
            Some(Picking::Export(rx, mixxx)) => rx.try_recv().ok().map(|p| (p, Some(*mixxx))),
            None => None,
        };
        if let Some((path, export)) = picked {
            self.picking = None;
            match export {
                None => self.import(&path),
                Some(mixxx) => self.export(&path, mixxx),
            }
        }
        if let Some((_, rx)) = &self.loading {
            if let Ok((result, functions)) = rx.try_recv() {
                self.loading = None;
                self.functions = functions;
                g.set_loading(false);
                let again = self.reselect.take();
                match result {
                    Ok(v) => {
                        g.set_author(v.author.clone().into());
                        g.set_status(if v.notes.is_empty() {
                            "".into()
                        } else {
                            format!("Parts of this mapping's script failed: {}", v.notes[0]).into()
                        });
                        g.set_options(ModelRc::new(VecModel::from(
                            v.options
                                .iter()
                                .map(|o| EditorOption {
                                    label: o.label.clone().into(),
                                    detail: o.description.clone().into(),
                                    default_value: o.default.clone().into(),
                                    kind: o.kind.clone().into(),
                                })
                                .collect::<Vec<_>>(),
                        )));
                        match &again {
                            // The same row again: by its message and layer,
                            // else any row on that message.
                            Some(Some((key, layer))) => {
                                self.selected = v
                                    .rows
                                    .iter()
                                    .position(|r| r.keys.contains(key) && &r.layer == layer)
                                    .or_else(|| v.rows.iter().position(|r| r.keys.contains(key)))
                                    .map(|i| (0, i));
                            }
                            Some(None) => self.selected = None,
                            None => self.collapsed = closed_sections(&v),
                        }
                        // A row whose function changed may be in another
                        // section now: it opens.
                        if let Some(r) = again
                            .is_some()
                            .then_some(self.selected)
                            .flatten()
                            .and_then(|(_, i)| v.rows.get(i))
                        {
                            if let Some((group, _)) = GROUPS
                                .iter()
                                .find(|(_, c)| c.contains(&r.function.category))
                            {
                                self.collapsed.remove(&format!("0/{group}"));
                                self.collapsed
                                    .remove(&format!("0/{group}/{}", r.function.category));
                            }
                        }
                        self.view = Some(v);
                    }
                    Err(e) => g.set_status(format!("Can't read this mapping: {e}").into()),
                }
                self.playing();
                self.fill();
                self.picture();
                if again.is_some() {
                    self.scroll_to_selected();
                }
            }
        }
        if (g.get_query().to_string(), g.get_tab()) != self.filled_for {
            self.fill();
        }
        self.hint(&g);
        self.find_pressed();
        if self
            .flash
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > FLASH)
        {
            self.flash = None;
            self.picture();
        }
    }

    /// Says how to find a control by pressing it.
    fn hint(&self, g: &ControllerEditor) {
        let port = self.live_ports().into_iter().next();
        let (live, text) = match (&port, &self.view) {
            (Some(p), _) => (
                true,
                format!("Press a control on the {} to find it", short_port_name(p)),
            ),
            (None, Some(_)) => (
                false,
                format!(
                    "Plug in the {} to find a control by pressing it",
                    g.get_name()
                ),
            ),
            (None, None) => (false, String::new()),
        };
        if g.get_live() != live {
            g.set_live(live);
        }
        if g.get_find_hint() != text.as_str() {
            g.set_find_hint(text.into());
        }
    }

    /// Finds the control pressed on the controller this mapping is for.
    fn find_pressed(&mut self) {
        let Some(c) = &self.controllers else {
            return;
        };
        let seen = c.take_monitored();
        let Some(v) = &self.view else {
            return;
        };
        let ports: HashSet<String> = self.live_ports().into_iter().collect();
        // Letting a button go is not finding it: letting SHIFT go after
        // SHIFT + a control would take the selection away from that control.
        let mut found = None;
        let held = &mut self.held;
        for (_, m) in seen
            .iter()
            .filter(|(p, m)| ports.contains(p) && m.len() >= 2 && m[0] < 0xF0)
        {
            let key = message_key(m[0], m[1]);
            let note = matches!(m[0] & 0xF0, 0x80 | 0x90);
            let release = m[0] & 0xF0 == 0x80 || (note && m.get(2) == Some(&0));
            if release {
                held.remove(&key);
                continue;
            }
            if note {
                held.insert(key);
            }
            if v.rows.iter().any(|r| r.keys.contains(&key)) {
                found = Some(key);
            }
        }
        let Some(key) = found else {
            return;
        };
        let Some(index) = row_for(v, key, &self.held) else {
            return;
        };
        let row = &v.rows[index];
        let part = row.part.clone().unwrap_or_else(|| row.control.clone());
        let same = self.part.as_deref() == Some(part.as_str()) && self.selected == Some((0, index));
        self.flash = Some((part.clone(), Instant::now()));
        if !same {
            self.part = Some(part);
            self.selected = Some((0, index));
            if let Some(app) = self.app.upgrade() {
                let g = app.global::<ControllerEditor>();
                g.set_tab(0);
                g.set_query("".into());
            }
            self.fill();
            self.scroll_to_selected();
        }
        self.picture();
    }

    fn scroll_to_selected(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let Some((_, sel)) = self.selected else {
            return;
        };
        let mut y = 0.0;
        for s in &self.shown {
            match s {
                Shown::Row(i) if *i == sel => break,
                Shown::Row(_) => y += ROW_HEIGHT,
                Shown::Heading(key) if key.matches('/').count() == 1 => y += GROUP_HEIGHT,
                Shown::Heading(_) => y += CATEGORY_HEIGHT,
            }
        }
        let g = app.global::<ControllerEditor>();
        self.scroll_serial += 1;
        g.set_scroll_to(y);
        g.set_scroll_serial(self.scroll_serial);
    }

    fn part_clicked(&mut self, id: &str) {
        let mapped = self
            .view
            .as_ref()
            .is_some_and(|v| v.rows.iter().any(|r| r.part.as_deref() == Some(id)));
        if !mapped {
            return;
        }
        self.part = if self.part.as_deref() == Some(id) {
            None
        } else {
            Some(id.to_string())
        };
        self.selected = None;
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<ControllerEditor>();
            if g.get_tab() == 2 {
                g.set_tab(0);
            }
        }
        self.fill();
        self.picture();
    }

    fn row_clicked(&mut self, tab: i32, model_index: usize) {
        let i = match self.shown.get(model_index).cloned() {
            Some(Shown::Row(i)) => i,
            // A heading opens or closes its section.
            Some(Shown::Heading(key)) => {
                if !self.collapsed.remove(&key) {
                    self.collapsed.insert(key);
                }
                self.fill();
                return;
            }
            None => return,
        };
        self.selected = if self.selected == Some((tab, i)) {
            None
        } else {
            Some((tab, i))
        };
        self.fill();
        self.picture();
    }

    /// The part the selected row belongs to.
    fn selected_part(&self) -> Option<String> {
        let v = self.view.as_ref()?;
        match self.selected? {
            (0, i) => v.rows.get(i)?.part.clone(),
            (_, i) => v.lights.get(i)?.part.clone(),
        }
    }

    /// Fills the list for the tab, search and part shown.
    fn fill(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let query = g.get_query().to_string();
        let tab = g.get_tab();
        self.filled_for = (query.clone(), tab);
        let words: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();
        let part = self.part.clone();
        let mut model = Vec::new();
        self.shown.clear();
        let Some(v) = &self.view else {
            g.set_rows(ModelRc::default());
            g.set_lights(ModelRc::default());
            g.set_summary("".into());
            g.set_part_filter("".into());
            return;
        };
        let on_part = |p: &Option<String>, control: &str| match &part {
            None => true,
            Some(id) => p.as_deref() == Some(id.as_str()) || (p.is_none() && control == id),
        };
        let matches = |text: &str| {
            let text = text.to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        };
        // The rows that match, by category.
        let mut found: Vec<Found> = Vec::new();
        if tab == 1 {
            for (i, l) in v.lights.iter().enumerate() {
                let row = light_row(v, l, self.selected == Some((1, i)));
                if on_part(&l.part, &l.control) && matches(&search_text(&row, l.function.category))
                {
                    found.push((l.function.category, i, row));
                }
            }
        } else {
            for (i, r) in v.rows.iter().enumerate() {
                let row = function_row(r, self.selected == Some((0, i)));
                if on_part(&r.part, &r.control) && matches(&search_text(&row, r.function.category))
                {
                    found.push((r.function.category, i, row));
                }
            }
        }
        // Searching or narrowing to a part shows everything found; otherwise
        // closed sections show only their heading.
        let everything = !words.is_empty() || part.is_some();
        for (group, categories) in GROUPS {
            let in_group: Vec<&Found> = found
                .iter()
                .filter(|(c, _, _)| categories.contains(c))
                .collect();
            if in_group.is_empty() {
                continue;
            }
            let key = format!("{tab}/{group}");
            let open = everything || !self.collapsed.contains(&key);
            model.push(heading(0, group, open, in_group.len()));
            self.shown.push(Shown::Heading(key));
            if !open {
                continue;
            }
            for (sub, rows) in sections(categories, &in_group) {
                // A group that is one section needs no second heading.
                if let Some(name) = sub {
                    let key = format!("{tab}/{group}/{name}");
                    let open = everything || !self.collapsed.contains(&key);
                    model.push(heading(1, &name, open, rows.len()));
                    self.shown.push(Shown::Heading(key));
                    if !open {
                        continue;
                    }
                }
                for (_, i, row) in rows {
                    model.push(row.clone());
                    self.shown.push(Shown::Row(*i));
                }
            }
        }
        if tab == 1 {
            g.set_lights(ModelRc::new(VecModel::from(model)));
        } else {
            g.set_rows(ModelRc::new(VecModel::from(model)));
        }
        let controls: HashSet<&str> = v.rows.iter().map(|r| r.control.as_str()).collect();
        g.set_summary(
            match tab {
                0 => format!("{} functions on {} controls", v.rows.len(), controls.len()),
                1 => format!("{} lights", v.lights.len()),
                _ => format!("{} settings", v.options.len()),
            }
            .into(),
        );
        let filter = part.map(|id| {
            v.layout
                .as_ref()
                .and_then(|l| l.parts.iter().find(|p| p.id == id))
                .map_or(id.clone(), |p| p.label.clone())
        });
        g.set_part_filter(filter.unwrap_or_default().into());
        self.edit_panel();
    }

    /// The row selected on the Controls tab.
    fn selected_row(&self) -> Option<&Row> {
        match self.selected? {
            (0, i) => self.view.as_ref()?.rows.get(i),
            _ => None,
        }
    }

    /// The function the selected row is set to, among those to choose from.
    fn current_choice(&self) -> Option<&Choice> {
        let control = Binding::of(self.selected_row()?).control?;
        self.functions
            .iter()
            .find(|c| c.targets.iter().any(|(_, t)| *t == control))
    }

    /// Fills the panel that edits the selected row.
    fn edit_panel(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let Some(row) = self.selected_row() else {
            g.set_editing(false);
            g.set_picker_open(false);
            return;
        };
        let b = Binding::of(row);
        g.set_editing(true);
        g.set_edit_control(
            if row.layer.is_empty() {
                row.control.clone()
            } else {
                format!("{} · {}", row.control, row.layer)
            }
            .into(),
        );
        g.set_edit_midi(row.midi.clone().into());
        // Run by the mapping's JavaScript (not an DJSugarScript line).
        let script = row
            .sources
            .first()
            .is_some_and(|s| !s.is_control() && s.action.is_none());
        g.set_edit_function(
            match (&b.control, script) {
                _ if row.off => row.function.name.clone(),
                (Some(_), _) => row.function.name.clone(),
                (None, true) => format!("{} (by the script)", row.function.name),
                (None, false) => "Does nothing".to_string(),
            }
            .into(),
        );
        let choice = self.current_choice();
        let targets: Vec<&str> = choice
            .map(|c| c.targets.iter().map(|(t, _)| t.as_str()).collect())
            .unwrap_or_default();
        let target = choice
            .zip(b.control.as_ref())
            .and_then(|(c, ctl)| c.targets.iter().position(|(_, t)| t == ctl))
            .unwrap_or(0);
        g.set_edit_targets(labels(targets.iter().copied()));
        g.set_edit_target(target as i32);
        g.set_edit_target_name(targets.get(target).copied().unwrap_or("").into());
        let index_of = |w| edit::WHENS.iter().position(|x| *x == w).unwrap_or(0) as i32;
        g.set_edit_when(index_of(b.when));
        g.set_edit_buttons(
            Buttons::ALL
                .iter()
                .position(|x| *x == b.buttons)
                .unwrap_or(0) as i32,
        );
        g.set_edit_encoder(
            Encoder::ALL
                .iter()
                .position(|x| *x == b.encoder)
                .unwrap_or(0) as i32,
        );
        g.set_edit_invert(b.invert);
        let does = Does::of(self.current_own().map(|o| &o.mapping), row);
        g.set_edit_does(match &does {
            Does::Function => 0,
            Does::Script(_) => 1,
            Does::Hold(_) => 2,
            Does::Toggle(_) => 3,
        });
        g.set_edit_script(match &does {
            Does::Script(l) => l.as_str().into(),
            _ => "".into(),
        });
        g.set_edit_script_error("".into());
        g.set_edit_var(match &does {
            Does::Hold(v) | Does::Toggle(v) => v.as_str().into(),
            _ => "".into(),
        });
        let layer = row
            .sources
            .first()
            .map(|s| edit::layer_text(&s.layer))
            .unwrap_or_default();
        g.set_edit_layer(layer.as_str().into());
        g.set_edit_layer_now(layer.into());
        g.set_edit_layer_error("".into());
        g.set_variables(
            self.current_own()
                .map(|o| {
                    o.mapping
                        .variables
                        .iter()
                        .map(|v| v.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default()
                .into(),
        );
        let first = row.sources.first();
        g.set_edit_is_button(first.is_none_or(|s| matches!(s.status & 0xF0, 0x80 | 0x90)));
        g.set_edit_edited(row.edited);
        g.set_edit_enabled(!row.off && first.is_some());
        let mut hint = Vec::new();
        if row.off {
            hint.push(if row.note.is_empty() {
                "Kept from the import but not working yet.".to_string()
            } else {
                format!("Not working yet: {}", row.note)
            });
        } else if script {
            hint.push(
                "The mapping's script runs this. A function chosen here replaces what the \
                 script does with this control (SHIFT too); the script keeps running the rest."
                    .to_string(),
            );
        }
        if self.version.is_none() && !row.off {
            hint.push(format!(
                "A change makes your own copy, {}; the original stays as it is.",
                self.my_name()
            ));
        }
        g.set_edit_hint(hint.join(" ").into());
    }

    /// The name the first own copy of the mapping shown gets.
    fn my_name(&self) -> String {
        let name = self
            .app
            .upgrade()
            .map(|a| a.global::<ControllerEditor>().get_name().to_string())
            .unwrap_or_default();
        let taken: Vec<String> = self.own.iter().map(|m| m.name.clone()).collect();
        edit::my_name(&name, &taken)
    }

    fn open_picker(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        g.set_picker_query("".into());
        self.fill_picker();
        g.set_picker_open(true);
    }

    /// Fills the function picker: the functions that match the search, by
    /// category, the one in use lit.
    fn fill_picker(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let words: Vec<String> = g
            .get_picker_query()
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();
        let current = self.selected_row().and_then(|r| Binding::of(r).control);
        let mut rows = Vec::new();
        self.picker.clear();
        // By name and target; by category too only when no name matches
        // ("sync" finds Sync, not all of Tempo & sync).
        let text = |c: &Choice, category: bool| {
            let targets: Vec<&str> = c.targets.iter().map(|(t, _)| t.as_str()).collect();
            let mut t = format!("{} {}", c.name, targets.join(" "));
            if category {
                t = format!("{} {t}", c.category);
            }
            t.to_lowercase()
        };
        let found = |c: &Choice, category: bool| {
            let t = text(c, category);
            words.iter().all(|w| t.contains(w.as_str()))
        };
        let by_category = !self.functions.iter().any(|c| found(c, false));
        let mut category = "";
        for (i, c) in self.functions.iter().enumerate() {
            if !found(c, by_category) {
                continue;
            }
            if c.category != category {
                category = c.category;
                rows.push(EditorRow {
                    header: true,
                    function: c.category.to_uppercase().into(),
                    ..Default::default()
                });
                self.picker.push(None);
            }
            let target = match c.targets.as_slice() {
                [] => String::new(),
                [(t, _)] => t.clone(),
                [(a, _), .., (z, _)] => format!("{a} … {z}"),
            };
            rows.push(EditorRow {
                header: false,
                function: c.name.clone().into(),
                target: target.into(),
                kind: if c.continuous { "knob or fader" } else { "" }.into(),
                selected: current
                    .as_ref()
                    .is_some_and(|ctl| c.targets.iter().any(|(_, t)| t == ctl)),
                ..Default::default()
            });
            self.picker.push(Some(i));
        }
        g.set_picker_rows(ModelRc::new(VecModel::from(rows)));
    }

    /// Gives the selected row the function picked, on the target the row
    /// had (Deck 2 stays Deck 2) where the function has it.
    fn picker_picked(&mut self, index: usize) {
        let Some(Some(i)) = self.picker.get(index).copied() else {
            return;
        };
        let Some(choice) = self.functions.get(i).cloned() else {
            return;
        };
        let Some(row) = self.selected_row() else {
            return;
        };
        let target = row.function.target.clone();
        let Some((_, control)) = choice
            .targets
            .iter()
            .find(|(t, _)| *t == target)
            .or(choice.targets.first())
            .cloned()
        else {
            return;
        };
        if let Some(app) = self.app.upgrade() {
            app.global::<ControllerEditor>().set_picker_open(false);
        }
        self.change(|b| b.control = Some(control));
    }

    /// Changes what the selected row is set to.
    fn change(&mut self, f: impl FnOnce(&mut Binding)) {
        let Some(row) = self.selected_row() else {
            return;
        };
        let mut b = Binding::of(row);
        f(&mut b);
        self.apply(move |own, row| edit::set(own, row, &b));
    }

    /// Changes the selected row in the own mapping in use, making one first
    /// when the original is shown, then saves it, restarts the controller
    /// with it and reads the view again.
    fn apply(&mut self, f: impl FnOnce(&mut DjSugarMapping, &Row)) {
        let (Some(app), Some(base), Some(dir)) = (
            self.app.upgrade(),
            self.base(),
            settings::own_mappings_dir(),
        ) else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        if self.version.is_none() {
            match own::create(&dir, &self.my_name(), &base, None) {
                Ok(m) => self.use_version(&base, Some(&m.id)),
                Err(e) => {
                    g.set_status(format!("Couldn't save the mapping: {e}").into());
                    return;
                }
            }
            self.versions(&base);
        }
        let Some(mut mine) = self.current_own().cloned() else {
            return;
        };
        f(&mut mine.mapping, &row);
        if let Err(e) = own::save(&dir, &mine) {
            g.set_status(format!("Couldn't save the mapping: {e}").into());
            return;
        }
        self.versions(&base);
        if let Some(c) = &self.controllers {
            c.reload(&base);
        }
        self.read(true);
    }

    /// What the selected row does now.
    fn does(&self) -> Option<Does> {
        let row = self.selected_row()?;
        Some(Does::of(self.current_own().map(|o| &o.mapping), row))
    }

    /// The Does menu: a function opens the picker; a line waits for one to
    /// be typed; a shift or mode button is made at once, named "shift" or
    /// "mode" until another name is typed.
    fn set_does(&mut self, index: i32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        g.set_edit_does(index);
        match index {
            0 => {
                if !matches!(self.does(), Some(Does::Function)) {
                    self.open_picker();
                }
            }
            1 => {
                if g.get_edit_script().is_empty() {
                    g.set_edit_script(self.line_for_function().into());
                }
                self.check_script();
            }
            2 | 3 => {
                let name = match g.get_edit_var().to_string() {
                    n if !n.is_empty() => n,
                    _ if index == 2 => "shift".to_string(),
                    _ => "mode".to_string(),
                };
                g.set_edit_var(name.as_str().into());
                let does = if index == 2 {
                    Does::Hold(name)
                } else {
                    Does::Toggle(name)
                };
                self.apply(move |own, row| edit::set_does(own, row, &does));
            }
            _ => {}
        }
    }

    /// A line doing what the row's function does, to start from: `toggle
    /// 'play'` on deck 1 is `deck 1 toggle 'play'`.
    fn line_for_function(&self) -> String {
        let Some(control) = self.selected_row().and_then(|r| Binding::of(r).control) else {
            return String::new();
        };
        let (group, key) = control.split_once(',').unwrap_or(("", ""));
        let deck = group
            .strip_prefix("[Channel")
            .and_then(|d| d.strip_suffix(']'))
            .filter(|d| d.len() == 1);
        match deck {
            Some(d) if key == "play" => format!("deck {d} play_pause"),
            Some(d) => format!("deck {d} press '{key}'"),
            None => format!("press '{control}'"),
        }
    }

    fn check_script(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let line = g.get_edit_script().to_string();
        let error = match djsugar_script::parse(line.trim()) {
            _ if line.trim().is_empty() => "Type a line, such as deck 1 play_pause.".to_string(),
            Ok(_) => String::new(),
            Err(e) => e.message,
        };
        g.set_edit_script_error(error.into());
    }

    fn script_done(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        self.check_script();
        if !g.get_edit_script_error().is_empty() {
            return;
        }
        let line = g.get_edit_script().trim().to_string();
        if self.does() == Some(Does::Script(line.clone())) {
            return;
        }
        let when = edit::WHENS[(g.get_edit_when().max(0) as usize).min(edit::WHENS.len() - 1)];
        self.apply(move |own, row| edit::set_does_at(own, row, &Does::Script(line), when));
    }

    fn var_done(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let name = g.get_edit_var().trim().to_string();
        match edit::check_layer(&name) {
            Ok(n) if !n.is_empty() && !n.contains(['=', '&']) => {}
            _ => {
                g.set_edit_script_error(
                    "A variable is one word: letters, digits and _, such as shift.".into(),
                );
                return;
            }
        }
        let does = if g.get_edit_does() == 2 {
            Does::Hold(name)
        } else {
            Does::Toggle(name)
        };
        if self.does() != Some(does.clone()) {
            self.apply(move |own, row| edit::set_does(own, row, &does));
        }
    }

    fn check_layer(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let error = edit::check_layer(&g.get_edit_layer())
            .err()
            .unwrap_or_default();
        g.set_edit_layer_error(error.into());
    }

    fn layer_done(&mut self, moving: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let Ok(layer) = edit::check_layer(&g.get_edit_layer()) else {
            self.check_layer();
            return;
        };
        if layer == g.get_edit_layer_now().as_str() {
            return;
        }
        self.apply(move |own, row| edit::set_layer(own, row, &layer, moving));
    }

    /// The Options tab's first rows: two decks or four and the deck order,
    /// for a mapping with a layout for each, and how knobs and faders take
    /// over on the controllers running it.
    fn playing(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let strings = |list: &[&str]| -> ModelRc<SharedString> {
            ModelRc::new(VecModel::from(
                list.iter()
                    .map(|s| SharedString::from(*s))
                    .collect::<Vec<_>>(),
            ))
        };
        let mut rows = Vec::new();
        if let Some(own) = self.current_own().filter(|o| o.mapping.decks.is_some()) {
            let four = own.mapping.decks == Some(4);
            rows.push(EditorSetting {
                id: "decks".into(),
                label: "Used with".into(),
                detail: "This mapping has a layout for two decks and one for four.".into(),
                choices: strings(&["2 decks", "4 decks"]),
                chosen: four as i32,
            });
            if four {
                let order = own.mapping.deck_order.unwrap_or([1, 2, 3, 4]);
                rows.push(EditorSetting {
                    id: "deck_order".into(),
                    label: "Deck order".into(),
                    detail: "Which deck each column of the controller is, from left to right."
                        .into(),
                    choices: strings(&DECK_ORDERS.map(|(name, _)| name)),
                    chosen: DECK_ORDERS
                        .iter()
                        .position(|(_, o)| *o == order)
                        .unwrap_or(0) as i32,
                });
            }
        }
        if let Some(port) = self.live_ports().first() {
            rows.push(EditorSetting {
                id: "takeover".into(),
                label: "Knobs and faders".into(),
                detail: "After one moves on screen, Pick-up waits until the hardware reaches \
                         it; Scale takes over at once and closes the gap as you move."
                    .into(),
                choices: strings(&["Pick-up", "Scale"]),
                chosen: pickup::scale(&djsugar_controllers::device_key(port)) as i32,
            });
        }
        g.set_playing(ModelRc::new(VecModel::from(rows)));
    }

    fn set_playing(&mut self, id: &str, index: usize) {
        let (Some(app), Some(base), Some(dir)) = (
            self.app.upgrade(),
            self.base(),
            settings::own_mappings_dir(),
        ) else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        if id == "takeover" {
            for port in self.live_ports() {
                let key = djsugar_controllers::device_key(&port);
                pickup::set_scale(&key, index == 1);
                settings::set_controller_scaled(&key, index == 1);
            }
            self.playing();
            return;
        }
        let Some(mut mine) = self.current_own().cloned() else {
            return;
        };
        match id {
            "decks" => {
                mine.mapping.decks = Some(if index == 1 { 4 } else { 2 });
                mine.mapping.deck_order.get_or_insert([3, 1, 2, 4]);
            }
            "deck_order" => match DECK_ORDERS.get(index) {
                Some((_, o)) => mine.mapping.deck_order = Some(*o),
                None => return,
            },
            _ => return,
        }
        if let Err(e) = own::save(&dir, &mine) {
            g.set_status(format!("Couldn't save the mapping: {e}").into());
            return;
        }
        self.versions(&base);
        if let Some(c) = &self.controllers {
            c.reload(&base);
        }
        self.read(true);
    }

    /// Draws the controller: its parts, which are mapped, and which are lit.
    fn picture(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let Some((v, layout)) = self
            .view
            .as_ref()
            .and_then(|v| Some((v, v.layout.as_ref()?)))
        else {
            g.set_has_picture(false);
            g.set_parts(ModelRc::default());
            g.set_panels(ModelRc::default());
            return;
        };
        let mapped: HashSet<&str> = v.rows.iter().filter_map(|r| r.part.as_deref()).collect();
        let lit = [self.part.clone(), self.selected_part()];
        let flash = self.flash.as_ref().map(|(p, _)| p.as_str());
        g.set_picture_width(layout.width);
        g.set_picture_height(layout.height);
        g.set_panels(ModelRc::new(VecModel::from(
            layout
                .panels
                .iter()
                .map(|p| EditorPanel {
                    x: p[0],
                    y: p[1],
                    w: p[2],
                    h: p[3],
                    r: p[4],
                })
                .collect::<Vec<_>>(),
        )));
        g.set_parts(ModelRc::new(VecModel::from(
            layout
                .parts
                .iter()
                .map(|p| EditorPart {
                    id: p.id.clone().into(),
                    label: p.label.clone().into(),
                    kind: p.kind.clone().into(),
                    round: p.round,
                    x: p.x,
                    y: p.y,
                    w: p.w,
                    h: p.h,
                    mapped: mapped.contains(p.id.as_str()),
                    lit: lit.iter().any(|l| l.as_deref() == Some(p.id.as_str())),
                    flash: flash == Some(p.id.as_str()),
                })
                .collect::<Vec<_>>(),
        )));
        g.set_has_picture(true);
    }
}

type Found = (&'static str, usize, EditorRow);

/// A group's rows in sections, each with its heading: by category, or for a
/// group that is one category (Effects), by target ("FX 1", "FX 2") when
/// there are many. A group with one section has no second heading.
/// The greyed rows of an import, grouped by why, most first.
fn greyed(m: &djsugar_controllers::format::DjSugarMapping) -> String {
    let mut why: Vec<(String, usize)> = Vec::new();
    let notes = m
        .rows
        .iter()
        .filter(|r| r.off)
        .map(|r| &r.note)
        .chain(m.lights.iter().filter(|l| l.off).map(|l| &l.note));
    for note in notes {
        match why.iter_mut().find(|(n, _)| n == note) {
            Some((_, c)) => *c += 1,
            None => why.push((note.clone(), 1)),
        }
    }
    why.sort_by_key(|w| std::cmp::Reverse(w.1));
    why.iter()
        .take(8)
        .map(|(n, c)| format!("{c} × {n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn sections<'a>(categories: &[&str], rows: &[&'a Found]) -> Vec<(Option<String>, Vec<&'a Found>)> {
    let mut out: Vec<(Option<String>, Vec<&Found>)> = Vec::new();
    if categories.len() > 1 {
        for category in categories {
            let in_it: Vec<&Found> = rows
                .iter()
                .copied()
                .filter(|(c, _, _)| c == category)
                .collect();
            if !in_it.is_empty() {
                let name = if *category == "Pads" {
                    "Pad modes"
                } else {
                    category
                };
                out.push((Some(name.to_string()), in_it));
            }
        }
        return out;
    }
    let mut targets: Vec<String> = Vec::new();
    for (_, _, r) in rows {
        if !targets.iter().any(|t| t.as_str() == r.target.as_str()) {
            targets.push(r.target.to_string());
        }
    }
    if targets.len() < 2 || rows.len() <= 12 {
        return vec![(None, rows.to_vec())];
    }
    targets.sort_by(|a, b| natural(a, b));
    for t in targets {
        let in_it: Vec<&Found> = rows
            .iter()
            .copied()
            .filter(|(_, _, r)| r.target.as_str() == t)
            .collect();
        out.push((
            Some(if t.is_empty() { "Other".to_string() } else { t }),
            in_it,
        ));
    }
    out
}

/// "FX 2" before "FX 10".
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| {
        let digits: String = s.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits
            .chars()
            .rev()
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        (
            s.trim_end_matches(|c: char| c.is_ascii_digit()).to_string(),
            n,
        )
    };
    split(a).cmp(&split(b))
}

/// Every section closed but the first group and the first section in it,
/// on both lists.
fn closed_sections(v: &MappingView) -> HashSet<String> {
    let mut closed = HashSet::new();
    let lists: [Vec<Found>; 2] = [
        v.rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.function.category, i, function_row(r, false)))
            .collect(),
        v.lights
            .iter()
            .enumerate()
            .map(|(i, l)| (l.function.category, i, light_row(v, l, false)))
            .collect(),
    ];
    for (tab, found) in lists.iter().enumerate() {
        let mut first = true;
        for (group, categories) in GROUPS {
            let in_group: Vec<&Found> = found
                .iter()
                .filter(|(c, _, _)| categories.contains(c))
                .collect();
            if in_group.is_empty() {
                continue;
            }
            if !first {
                closed.insert(format!("{tab}/{group}"));
            }
            for (j, (sub, _)) in sections(categories, &in_group).into_iter().enumerate() {
                if let Some(name) = sub {
                    if !(first && j == 0) {
                        closed.insert(format!("{tab}/{group}/{name}"));
                    }
                }
            }
            first = false;
        }
    }
    closed
}

fn heading(level: i32, name: &str, open: bool, count: usize) -> EditorRow {
    EditorRow {
        header: true,
        level,
        open,
        count: count as i32,
        function: name.into(),
        ..Default::default()
    }
}

fn function_row(r: &Row, selected: bool) -> EditorRow {
    // The options, without "Script" (the row has a tag for that), and the
    // last part of the author's note ("PLAY/PAUSE (DECK1) - press -
    // Play/Pause" → "Play/Pause").
    let options = r.options.strip_prefix("Script").unwrap_or(&r.options);
    let options = options.trim_start_matches(", ");
    let mut detail: Vec<&str> = Vec::new();
    if !options.is_empty() {
        detail.push(options);
    }
    let note = r.description.rsplit(" - ").next().unwrap_or("").trim();
    let plain = |t: &str| -> String {
        t.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    if !note.is_empty() && plain(note) != plain(&r.function.name) {
        detail.push(note);
    }
    // The script function only on the selected row: it's long and only
    // tells someone who reads the script.
    if let Some(s) = r.script.as_ref().filter(|_| selected) {
        detail.push(s);
    }
    if r.off && !r.note.is_empty() {
        detail.push(&r.note);
    }
    EditorRow {
        header: false,
        level: 2,
        open: false,
        count: 0,
        function: r.function.name.clone().into(),
        target: r.function.target.clone().into(),
        control: r.control.clone().into(),
        layer: r.layer.clone().into(),
        kind: r.kind.clone().into(),
        midi: r.midi.clone().into(),
        script: r.script.clone().unwrap_or_default().into(),
        detail: detail.join(" · ").into(),
        part: r.part.clone().unwrap_or_default().into(),
        selected,
        edited: r.edited,
        off: r.off,
    }
}

fn labels<'a>(items: impl Iterator<Item = &'a str>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items.map(SharedString::from).collect::<Vec<_>>(),
    ))
}

fn light_row(v: &MappingView, l: &LightRow, selected: bool) -> EditorRow {
    let kind = l
        .part
        .as_ref()
        .and_then(|id| v.layout.as_ref()?.parts.iter().find(|p| &p.id == id))
        .map_or("button".to_string(), |p| p.kind.clone());
    // The layer of the control whose LED it is: a pad mode, a bank.
    let layer = v
        .layout
        .as_ref()
        .and_then(|layout| layout.find(l.key.0, l.key.1))
        .map(|(_, m)| m.layer.clone())
        .unwrap_or_default();
    EditorRow {
        header: false,
        level: 2,
        open: false,
        count: 0,
        function: l.function.name.clone().into(),
        target: l.function.target.clone().into(),
        control: l.control.clone().into(),
        layer: layer.into(),
        kind: kind.into(),
        midi: l.midi.clone().into(),
        script: SharedString::new(),
        detail: SharedString::new(),
        part: l.part.clone().unwrap_or_default().into(),
        selected,
        edited: false,
        off: false,
    }
}

fn search_text(r: &EditorRow, category: &str) -> String {
    [
        category,
        &r.function,
        &r.target,
        &r.control,
        &r.layer,
        &r.midi,
        &r.script,
        &r.detail,
    ]
    .join(" ")
}

/// The layers a held button turns on: an DJ Sugar mapping's `set_var '<name>'`
/// rows, and any control called SHIFT.
fn held_layers(v: &MappingView, held: &HashSet<MessageKey>) -> HashSet<String> {
    let mut on = HashSet::new();
    for r in v
        .rows
        .iter()
        .filter(|r| r.keys.iter().any(|k| held.contains(k)))
    {
        if r.control.to_uppercase().contains("SHIFT") && r.layer.is_empty() {
            on.insert("SHIFT".to_string());
        }
        for s in &r.sources {
            if let Some(name) = s.action.as_deref().and_then(set_var_name) {
                on.insert(name.to_uppercase());
            }
        }
    }
    on
}

/// `x` in a line `set_var 'x' 1`.
fn set_var_name(line: &str) -> Option<&str> {
    let rest = line.split("set_var").nth(1)?.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    rest[1..].split(quote).next()
}

/// The row a control pressed finds: of the rows on its message, the one
/// for the layers held now (SHIFT + the control finds the SHIFT row), the
/// plain press before a long press or other way of pressing.
fn row_for(v: &MappingView, key: MessageKey, held: &HashSet<MessageKey>) -> Option<usize> {
    let on = held_layers(v, held);
    let names: HashSet<String> = v
        .rows
        .iter()
        .flat_map(|r| r.sources.iter().flat_map(|s| s.layer.iter()))
        .map(|(n, _)| n.to_uppercase())
        .chain(std::iter::once("SHIFT".to_string()))
        .collect();
    let score = |r: &Row| -> Option<(usize, usize)> {
        let (mut layers, mut other) = (0, 0);
        for part in r.layer.split(" + ").filter(|p| !p.is_empty()) {
            let (name, value) = match part.rsplit_once(' ') {
                Some((n, v)) if v.parse::<f64>().is_ok() => (n, Some(v)),
                _ => (part, None),
            };
            if !names.contains(name) {
                other += 1;
                continue;
            }
            let wants_on = value.is_none_or(|v| v.parse::<f64>().unwrap_or(0.0) != 0.0);
            if wants_on != on.contains(name) {
                return None;
            }
            layers += usize::from(wants_on);
        }
        Some((layers, usize::MAX - other))
    };
    let mut best: Option<(usize, (usize, usize))> = None;
    let mut first = None;
    for (i, r) in v
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.keys.contains(&key))
    {
        first.get_or_insert(i);
        if let Some(sc) = score(r) {
            if best.is_none_or(|(_, b)| sc > b) {
                best = Some((i, sc));
            }
        }
    }
    best.map(|(i, _)| i).or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k2() -> MappingView {
        let reg = ControlRegistry::new();
        let _engine = djsugar_engine::engine::Engine::new(&reg, 44_100.0);
        let dir = djsugar_controllers::default_mappings_dir().unwrap();
        view::build(
            &dir.join("Allen and Heath Xone K2 - 2 Decks.djsugar-mapping"),
            reg,
        )
        .unwrap()
    }

    #[test]
    fn set_var_names_the_layer() {
        assert_eq!(set_var_name("set_var 'shift' 1"), Some("shift"));
        assert_eq!(set_var_name("deck 1 play"), None);
    }

    /// SHIFT + the bottom left encoder finds its SHIFT row; without SHIFT,
    /// the plain browse row.
    #[test]
    fn a_held_shift_finds_the_shift_row() {
        let v = k2();
        let encoder = (0xBE, 0x14);
        let plain = row_for(&v, encoder, &HashSet::new()).unwrap();
        assert!(v.rows[plain].layer.is_empty(), "{:?}", v.rows[plain]);
        let held = HashSet::from([(0x9E, 0x0F)]);
        let shifted = row_for(&v, encoder, &held).unwrap();
        assert_eq!(v.rows[shifted].layer, "SHIFT", "{:?}", v.rows[shifted]);
    }
}
