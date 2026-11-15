//! Map mode, as in Ableton: with MIDI on, every control on screen that can
//! be mapped is tinted. Click one, then move or press a control on a
//! controller, and that control does it from then on: a row of the DJ's own
//! mapping for the controller (made, "My …", the first time), put over the
//! base mapping it's based on. Meanwhile the controllers' messages are only
//! watched, so a press teaches rather than plays. Each change can be undone.
//!
//! The mapping wizard (Quick map in the code) does the picking: it asks
//! which controller (a button for each one plugged in, or a press on it),
//! then two decks or four, then makes a new mapping that stands alone and
//! walks through the usual controls (see [`djsugar_controllers::quick`]),
//! deck 1's one by one and the other decks from one press each. Ports the
//! computer makes (macOS's Network Session) are never offered.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use djsugar_controllers::edit;
use djsugar_controllers::format::{Action, DjSugarMapping, Midi, Row};
use djsugar_controllers::learn::{self, Heard};
use djsugar_controllers::own::{self, OwnMapping};
use djsugar_controllers::quick;
use djsugar_controllers::{
    device_key, is_own_file, is_system_port, Controllers, Mapping, MappingChoice,
};
use slint::ComponentHandle;

use crate::{settings, short_port_name, AppWindow, ControllerEditor, MapMode};

/// How long a knob or fader is listened to before it's learned, if it
/// doesn't send enough to tell sooner.
const LISTEN: Duration = Duration::from_millis(350);

/// How long the walk waits after a press on a deck's PLAY step, in case a
/// deck button came first and PLAY is still to come.
const LAST_PRESS: Duration = Duration::from_millis(1200);
/// How long the control learned last has to be still before it can start
/// another.
const SETTLE: Duration = Duration::from_millis(600);

/// A change Map mode made, to undo it, or to do again keeping what the
/// control did before.
struct Step {
    /// The mapping file the controller runs: a Mixxx-format mapping, or an own
    /// mapping standing alone (`<id>.json`).
    file: String,
    id: String,
    before: DjSugarMapping,
    heard: Heard,
    control: String,
    name: String,
    /// The Quick map step it answered.
    walk_at: Option<usize>,
}

/// Quick map under way.
struct Walk {
    port: String,
    /// The mapping it makes, once the number of decks is known.
    file: Option<String>,
    steps: Vec<quick::Step>,
    at: usize,
    decks: u8,
}

pub struct Mapper {
    app: slint::Weak<AppWindow>,
    controllers: Option<Rc<Controllers>>,
    dir: Option<PathBuf>,
    active: bool,
    armed: Option<String>,
    /// What one controller has sent since the control on screen was picked.
    heard: Option<(String, Instant, Vec<Vec<u8>>)>,
    steps: Vec<Step>,
    walk: Option<Walk>,
    /// The wizard asking which controller: the ports offered.
    choosing: Option<Vec<String>>,
    /// The control learned last, and when it last sent: what it goes on
    /// sending as it's let go of or still turned doesn't start the next
    /// one, until it has been still for a moment.
    last_heard: Option<(String, Midi, Instant)>,
    /// The controls bound on the controllers plugged in, for the tint.
    mapped: Rc<RefCell<HashSet<String>>>,
    tick: i32,
    /// Map mode was started from the Controller Editor, which opens again
    /// when it's done.
    from_editor: bool,
}

impl Mapper {
    pub fn install(app: &AppWindow, controllers: Option<Rc<Controllers>>) -> Rc<RefCell<Mapper>> {
        let mapped = Rc::new(RefCell::new(HashSet::new()));
        let m = Rc::new(RefCell::new(Mapper {
            app: app.as_weak(),
            controllers,
            dir: djsugar_controllers::default_mappings_dir(),
            active: false,
            armed: None,
            heard: None,
            steps: Vec::new(),
            walk: None,
            choosing: None,
            last_heard: None,
            mapped: mapped.clone(),
            tick: 0,
            from_editor: false,
        }));
        let g = app.global::<MapMode>();
        g.on_mapped(move |key, _| mapped.borrow().contains(key.as_str()));
        let e = m.clone();
        g.on_start(move || e.borrow_mut().set_active(true));
        let e = m.clone();
        g.on_done(move || e.borrow_mut().set_active(false));
        let e = m.clone();
        g.on_pick(move |key| e.borrow_mut().pick(&key));
        let e = m.clone();
        g.on_undo(move || e.borrow_mut().undo());
        let e = m.clone();
        g.on_keep_both(move || e.borrow_mut().keep_both());
        let e = m.clone();
        g.on_quick_map(move |port| e.borrow_mut().quick_map(&port));
        let e = m.clone();
        g.on_choose_device(move |i| {
            let mut m = e.borrow_mut();
            let port = m
                .choosing
                .as_ref()
                .and_then(|ports| ports.get(i as usize).cloned());
            if let Some(port) = port {
                m.start_walk(&port);
            }
        });
        let e = m.clone();
        g.on_choose_decks(move |n| e.borrow_mut().choose_decks(n.clamp(2, 4) as u8));
        let e = m.clone();
        g.on_skip(move || e.borrow_mut().next_step());
        m
    }

    fn set_active(&mut self, on: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<MapMode>();
        if on {
            let editor = app.global::<ControllerEditor>();
            self.from_editor = editor.get_open();
            editor.set_open(false);
            self.steps.clear();
            g.set_last("".into());
            g.set_replaced("".into());
            g.set_can_undo(false);
            self.refresh_mapped();
        }
        self.active = on;
        self.armed = None;
        self.heard = None;
        self.walk = None;
        self.choosing = None;
        g.set_armed("".into());
        g.set_asking_device(false);
        g.set_asking_decks(false);
        g.set_walking(false);
        g.set_active(on);
        if let Some(c) = &self.controllers {
            c.set_monitor(on);
            c.set_learning(on);
            // Nothing heard before now counts.
            c.take_monitored();
        }
        self.say_status();
        if !on && std::mem::take(&mut self.from_editor) {
            app.global::<ControllerEditor>().invoke_reopen();
        }
    }

    /// The controllers plugged in that run a mapping: port and mapping
    /// file (a Mixxx-format mapping, or an own one standing alone).
    fn ports(&self) -> Vec<(String, String)> {
        self.controllers
            .as_ref()
            .map(|c| {
                c.ports()
                    .into_iter()
                    // HID controllers can't be mapped by hand.
                    .filter(|p| p.running.is_some() && !p.hid)
                    .filter_map(|p| Some((p.name, p.mapping_file?)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The MIDI devices plugged in with no mapping, and not turned off.
    fn unmapped(&self) -> Vec<String> {
        let Some(c) = &self.controllers else {
            return Vec::new();
        };
        c.ports()
            .into_iter()
            .filter(|p| {
                p.mapping_file.is_none()
                    && !p.hid
                    && !is_system_port(&p.name)
                    && c.choice(&p.name) != MappingChoice::Off
            })
            .map(|p| p.name)
            .collect()
    }

    fn say_status(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<MapMode>();
        let ports = self.ports();
        let device = match ports.as_slice() {
            [] => None,
            [(p, _)] => Some(short_port_name(p)),
            _ => Some("controller"),
        };
        let unmapped = self.unmapped();
        let text = match (&device, &self.armed, unmapped.first()) {
            (None, _, Some(p)) => format!(
                "The {} has no mapping yet: the Mapping wizard makes one.",
                short_port_name(p)
            ),
            (None, _, None) => "Plug in a controller to map its controls.".to_string(),
            (Some(d), None, _) => format!("Click a control on screen, then move one on the {d}."),
            (Some(d), Some(_), _) => format!("Now press or move a control on the {d}."),
        };
        g.set_status(text.into());
    }

    fn pick(&mut self, key: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if self.walk.is_some() {
            return;
        }
        let g = app.global::<MapMode>();
        self.armed = if self.armed.as_deref() == Some(key) {
            None
        } else {
            Some(key.to_string())
        };
        self.heard = None;
        if let Some(c) = &self.controllers {
            c.take_monitored();
        }
        g.set_armed(self.armed.clone().unwrap_or_default().into());
        g.set_last("".into());
        g.set_replaced("".into());
        self.say_status();
    }

    /// Called every frame.
    pub fn poll(&mut self) {
        if !self.active {
            return;
        }
        let Some(c) = self.controllers.clone() else {
            return;
        };
        let seen = c.take_monitored();
        if let Some(ports) = &self.choosing {
            // A press on one of them picks it.
            let pressed = seen
                .iter()
                .find(|(p, msg)| ports.contains(p) && learn::starts(msg))
                .map(|(p, _)| p.clone());
            if let Some(port) = pressed {
                self.start_walk(&port);
            }
            return;
        }
        if self.armed.is_none() {
            return;
        }
        let only = self.walk.as_ref().map(|w| w.port.clone());
        for (port, msg) in seen {
            if msg.len() < 2 || msg[0] >= 0xF0 || only.as_ref().is_some_and(|p| *p != port) {
                continue;
            }
            // A note-off is the same control as its note-on.
            let status = if msg[0] & 0xF0 == 0x80 {
                msg[0] | 0x10
            } else {
                msg[0]
            };
            if let Some((p, m, at)) = &mut self.last_heard {
                let same = *p == port
                    && [Some((m.status, m.number)), m.lsb].contains(&Some((status, msg[1])));
                if same && at.elapsed() < SETTLE {
                    *at = Instant::now();
                    if self.heard.is_none() {
                        continue;
                    }
                }
            }
            let takes_last = self.takes_last();
            match &mut self.heard {
                // On a deck's PLAY step, a deck button pressed first
                // gives way to the PLAY pressed after it.
                Some((p, at, msgs))
                    if *p == port
                        && takes_last
                        && learn::starts(&msg)
                        && msgs.first().is_some_and(|m| m[..2] != msg[..2]) =>
                {
                    *msgs = vec![msg];
                    *at = Instant::now();
                }
                Some((p, _, msgs)) if *p == port => msgs.push(msg),
                Some(_) => {}
                None if learn::starts(&msg) => {
                    self.heard = Some((port, Instant::now(), vec![msg]));
                }
                None => {}
            }
        }
        let takes_last = self.takes_last();
        let ready = self.heard.as_ref().is_some_and(|(_, at, msgs)| {
            if takes_last {
                at.elapsed() >= LAST_PRESS
            } else {
                learn::enough(msgs) || at.elapsed() >= LISTEN
            }
        });
        if ready {
            let (port, _, msgs) = self.heard.take().unwrap();
            self.learn(&port, &msgs);
        }
    }

    /// Binds what `port` sent to the control picked on screen.
    fn learn(&mut self, port: &str, msgs: &[Vec<u8>]) {
        let (Some(app), Some(control)) = (self.app.upgrade(), self.armed.clone()) else {
            return;
        };
        let g = app.global::<MapMode>();
        let Some(heard) = learn::hear_for(msgs, &control) else {
            return;
        };
        self.last_heard = Some((port.to_string(), heard.midi, Instant::now()));
        let Some((_, file)) = self.ports().into_iter().find(|(p, _)| p == port) else {
            g.set_status(
                format!(
                    "The {} has no mapping to add to yet.",
                    short_port_name(port)
                )
                .into(),
            );
            return;
        };
        let walking = self.walk.as_ref().map(|w| {
            (
                w.at,
                w.steps.get(w.at).map(|s| s.row_name()),
                w.file.clone(),
            )
        });
        if let Some((_, _, want)) = &walking {
            // The controller restarts with the new mapping first.
            if want.as_ref() != Some(&file) {
                g.set_status("Starting the controller with the new mapping…".into());
                return;
            }
        }
        // The control's name on the controller's picture, if it has one.
        let (status, number) = (heard.midi.status, heard.midi.number);
        let name = match &walking {
            Some((_, Some(name), _)) => name.clone(),
            _ => djsugar_controllers::layout::Layout::for_file(&file)
                .and_then(|l| l.find(status, number).map(|(p, _)| p.label.clone()))
                .unwrap_or_else(|| djsugar_controllers::view::message_text(status, number)),
        };
        let Some(mine) = self.own_for(&file) else {
            g.set_status("Couldn't make a mapping of your own.".into());
            return;
        };
        if let Some(deck) = self.copies_deck() {
            // The controller sends the same PLAY on this deck as on deck 1:
            // it doesn't tell its decks apart, so there's nothing to copy,
            // and learning it would take PLAY away from deck 1.
            let play_one = mine.mapping.rows.iter().find_map(|r| match &r.action {
                Action::Bind { control: c, .. } if c == "[Channel1],play" && r.layer.is_empty() => {
                    Some(r.midi)
                }
                _ => None,
            });
            if play_one.is_some_and(|m| (m.status, m.number) == (status, number)) {
                g.set_last(
                    format!(
                        "Deck {deck}'s PLAY sends the same as deck 1's, so the controller doesn't \
                         tell them apart. Use DJ Sugar's deck {deck} switch instead."
                    )
                    .into(),
                );
                self.armed = None;
                g.set_armed("".into());
                self.next_step();
                return;
            }
        }
        let step = Step {
            file,
            id: mine.id.clone(),
            before: mine.mapping.clone(),
            heard,
            control,
            name,
            walk_at: walking.as_ref().map(|(at, _, _)| *at),
        };
        self.apply(&step, false);
        if step.walk_at.is_some() {
            self.after_step(&step);
        }
        self.steps.push(step);
        g.set_can_undo(true);
        self.armed = None;
        g.set_armed("".into());
        if self.walk.is_some() {
            self.next_step();
        }
    }

    /// Learns `step` into its own mapping, from how it was before.
    fn apply(&mut self, step: &Step, keep: bool) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), settings::own_mappings_dir()) else {
            return;
        };
        let g = app.global::<MapMode>();
        let base = self.base(&step.file);
        let Some(mut mine) = own::all(&dir).into_iter().find(|m| m.id == step.id) else {
            return;
        };
        mine.mapping = step.before.clone();
        let was = learn::learn(
            &mut mine.mapping,
            base.as_ref(),
            &step.heard,
            &step.control,
            &step.name,
            keep,
        );
        if let Err(e) = own::save(&dir, &mine) {
            g.set_status(format!("Couldn't save the mapping: {e}").into());
            return;
        }
        if let Some(c) = &self.controllers {
            c.reload(&step.file);
        }
        let what = describe(&step.control);
        g.set_last(format!("{} now does {what}.", step.name).into());
        g.set_replaced(if keep { String::new() } else { was.join(", ") }.into());
        self.refresh_mapped();
    }

    /// The base mapping `file` is (Mixxx format), or the one the own mapping in `file`
    /// is put over.
    fn base(&self, file: &str) -> Option<Mapping> {
        let dir = self.dir.as_ref()?;
        let file = if is_own_file(file) {
            OwnMapping::read(&settings::own_mappings_dir()?.join(file))?
                .mapping
                .based_on?
        } else {
            file.to_string()
        };
        Mapping::load(&dir.join(file)).ok()
    }

    /// The own mapping that changes for the controller running `file`:
    /// that one, for an own mapping standing alone; otherwise the one in
    /// use for that base mapping, made and put in use first if there is
    /// none.
    fn own_for(&self, file: &str) -> Option<OwnMapping> {
        let dir = settings::own_mappings_dir()?;
        if is_own_file(file) {
            return OwnMapping::read(&dir.join(file));
        }
        let all = own::all(&dir);
        if let Some(id) = settings::controller_version(file) {
            if let Some(m) = all.iter().find(|m| m.id == id) {
                return Some(m.clone());
            }
        }
        let name = self.base(file)?.name;
        let taken: Vec<String> = all.iter().map(|m| m.name.clone()).collect();
        let m = own::create(&dir, &edit::my_name(&name, &taken), file, None).ok()?;
        settings::set_controller_version(file, Some(&m.id));
        if let Some(c) = &self.controllers {
            c.set_version(file, Some(own::path(&dir, &m.id)));
        }
        Some(m)
    }

    fn undo(&mut self) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), settings::own_mappings_dir()) else {
            return;
        };
        let g = app.global::<MapMode>();
        let Some(step) = self.steps.pop() else {
            return;
        };
        if let Some(mut mine) = own::all(&dir).into_iter().find(|m| m.id == step.id) {
            mine.mapping = step.before;
            if own::save(&dir, &mine).is_ok() {
                if let Some(c) = &self.controllers {
                    c.reload(&step.file);
                }
            }
        }
        g.set_last(format!("Undone: {}.", step.name).into());
        g.set_replaced("".into());
        g.set_can_undo(!self.steps.is_empty());
        self.refresh_mapped();
        // Quick map asks for that control again.
        if let (Some(w), Some(at)) = (&mut self.walk, step.walk_at) {
            w.at = at;
            self.ask();
        }
    }

    fn keep_both(&mut self) {
        let Some(step) = self.steps.pop() else {
            return;
        };
        self.apply(&step, true);
        self.steps.push(step);
    }

    /// The controllers the wizard offers: every MIDI device plugged in
    /// that isn't turned off or made by the computer, the ones with no
    /// mapping first.
    fn candidates(&self) -> Vec<String> {
        let Some(c) = &self.controllers else {
            return Vec::new();
        };
        let mut ports: Vec<_> = c
            .ports()
            .into_iter()
            .filter(|p| {
                !p.hid && !is_system_port(&p.name) && c.choice(&p.name) != MappingChoice::Off
            })
            .collect();
        ports.sort_by_key(|p| (p.mapping_file.is_some(), p.running.is_none()));
        ports.into_iter().map(|p| p.name).collect()
    }

    /// Starts the mapping wizard for the controller on `port`, or for ""
    /// asks which one first (a button for each, or a press on it); with one
    /// plugged in it goes straight on.
    fn quick_map(&mut self, port: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !self.active {
            self.set_active(true);
        }
        let g = app.global::<MapMode>();
        if !port.is_empty() {
            self.start_walk(port);
            return;
        }
        let ports = self.candidates();
        match ports.as_slice() {
            [] => g.set_status("Plug in a controller to map it.".into()),
            [one] => {
                let one = one.clone();
                self.start_walk(&one);
            }
            _ => {
                self.armed = None;
                g.set_armed("".into());
                g.set_last("".into());
                g.set_replaced("".into());
                let names: Vec<slint::SharedString> =
                    ports.iter().map(|p| short_port_name(p).into()).collect();
                g.set_devices(std::rc::Rc::new(slint::VecModel::from(names)).into());
                g.set_asking_device(true);
                g.set_status("Which controller do you want to map?".into());
                if let Some(c) = &self.controllers {
                    c.take_monitored();
                }
                self.choosing = Some(ports);
            }
        }
    }

    /// The wizard's second question, for the controller on `port`: two
    /// decks or four.
    fn start_walk(&mut self, port: &str) {
        let (Some(app), Some(c)) = (self.app.upgrade(), self.controllers.clone()) else {
            return;
        };
        let g = app.global::<MapMode>();
        self.choosing = None;
        g.set_asking_device(false);
        let had = c
            .ports()
            .into_iter()
            .any(|p| p.name == port && p.mapping_file.is_some());
        self.walk = Some(Walk {
            port: port.to_string(),
            file: None,
            steps: Vec::new(),
            at: 0,
            decks: 0,
        });
        self.armed = None;
        g.set_armed("".into());
        g.set_last("".into());
        g.set_replaced("".into());
        g.set_asking_decks(true);
        let keep = if had {
            " The mapping it runs now stays in Settings › Controllers."
        } else {
            ""
        };
        g.set_status(
            format!(
                "A new mapping for the {}.{keep} Is it used with two decks or four?",
                short_port_name(port)
            )
            .into(),
        );
    }

    /// Makes the new mapping and puts it in use, then walks.
    fn choose_decks(&mut self, decks: u8) {
        let (Some(app), Some(c), Some(dir)) = (
            self.app.upgrade(),
            self.controllers.clone(),
            settings::own_mappings_dir(),
        ) else {
            return;
        };
        let g = app.global::<MapMode>();
        let Some(w) = &mut self.walk else {
            return;
        };
        let taken: Vec<String> = own::all(&dir).into_iter().map(|m| m.name).collect();
        let name = edit::my_name(short_port_name(&w.port), &taken);
        let mut mapping = DjSugarMapping::new(&name);
        mapping.decks = Some(decks);
        if decks == 4 {
            mapping.deck_order = Some([1, 2, 3, 4]);
        }
        let mine = match own::save_new(&dir, &name, "", mapping) {
            Ok(m) => m,
            Err(e) => {
                g.set_status(format!("Couldn't save the mapping: {e}").into());
                return;
            }
        };
        c.set_choice(&w.port, MappingChoice::Own(own::path(&dir, &mine.id)));
        settings::set_controller_mappings(&c.choices());
        settings::set_controller_shown(&device_key(&w.port), true);
        w.file = Some(format!("{}.json", mine.id));
        w.steps = quick::steps(decks);
        w.decks = decks;
        w.at = 0;
        g.set_asking_decks(false);
        g.set_walking(true);
        self.ask();
    }

    /// Picks the control the walk is at, and says what to do.
    fn ask(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<MapMode>();
        let Some(w) = &self.walk else {
            return;
        };
        let Some(s) = w.steps.get(w.at) else {
            self.finish();
            return;
        };
        let place = match s.deck {
            0 => "Mixer".to_string(),
            d => format!("Deck {d}"),
        };
        g.set_step(format!("{} of {}", w.at + 1, w.steps.len()).into());
        g.set_status(format!("{place} · {}", s.prompt).into());
        self.armed = Some(s.control.clone());
        self.heard = None;
        g.set_armed(s.control.as_str().into());
        if let Some(c) = &self.controllers {
            c.take_monitored();
        }
    }

    /// The deck whose PLAY the walk is asking for, to copy deck 1's rows to.
    fn copies_deck(&self) -> Option<u8> {
        let w = self.walk.as_ref()?;
        w.steps.get(w.at).filter(|s| s.copies).map(|s| s.deck)
    }

    /// Whether the last control pressed counts, not the first: on a deck's
    /// PLAY step, where a deck button may come first.
    fn takes_last(&self) -> bool {
        self.copies_deck().is_some()
    }

    /// On to the walk's next control (Skip, or after one was learned).
    fn next_step(&mut self) {
        if let Some(w) = &mut self.walk {
            w.at += 1;
        }
        self.ask();
    }

    /// What the walk does beyond learning `step`: pads 3 to 8 follow pads
    /// 1 and 2, and another deck's PLAY copies deck 1's rows over.
    fn after_step(&mut self, step: &Step) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), settings::own_mappings_dir()) else {
            return;
        };
        let Some(w) = &self.walk else {
            return;
        };
        let Some(s) = w.steps.get(w.at).cloned() else {
            return;
        };
        let Some(mut mine) = OwnMapping::read(&own::path(&dir, &step.id)) else {
            return;
        };
        let midi_of = |m: &DjSugarMapping, control: &str| -> Option<Midi> {
            m.rows.iter().find_map(|r| match &r.action {
                Action::Bind { control: c, .. } if c == control && r.layer.is_empty() => {
                    Some(r.midi)
                }
                _ => None,
            })
        };
        let said = if s.control == "[Channel1],hotcue_2_activate" {
            let Some(one) = midi_of(&mine.mapping, "[Channel1],hotcue_1_activate") else {
                return;
            };
            let pads = quick::more_pads(one, step.heard.midi);
            for (i, midi) in pads.iter().enumerate() {
                let n = i + 3;
                let used = mine
                    .mapping
                    .rows
                    .iter()
                    .any(|r| (r.midi.status, r.midi.number) == (midi.status, midi.number));
                if !used {
                    let mut row = Row::bind(*midi, &format!("[Channel1],hotcue_{n}_activate"), &[]);
                    row.name = format!("Pad {n} · deck 1");
                    mine.mapping.rows.push(row);
                }
            }
            format!("Pads 1 to {} are hot cues.", pads.len() + 2)
        } else if s.copies {
            let Some(one) = midi_of(&mine.mapping, "[Channel1],play") else {
                return;
            };
            if s.deck > 2 {
                // Like the rest of decks 3 and 4, only with four decks.
                for r in &mut mine.mapping.rows {
                    if r.midi == step.heard.midi && r.layer.is_empty() {
                        r.layer = quick::FOUR_DECKS.to_string();
                    }
                }
            }
            match quick::copy_deck(&mut mine.mapping, one, step.heard.midi, s.deck) {
                0 => format!(
                    "Deck {} couldn't be told from deck 1: click its controls on screen to map them.",
                    s.deck
                ),
                n => format!("Deck {} follows deck 1: {n} controls.", s.deck),
            }
        } else {
            return;
        };
        if own::save(&dir, &mine).is_ok() {
            if let Some(c) = &self.controllers {
                c.reload(&step.file);
            }
        }
        app.global::<MapMode>().set_last(said.into());
        self.refresh_mapped();
    }

    /// The walk is over: Map mode stays on, to try the mapping and change
    /// any control.
    fn finish(&mut self) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), settings::own_mappings_dir()) else {
            return;
        };
        let g = app.global::<MapMode>();
        let Some(w) = self.walk.take() else {
            return;
        };
        let id = w
            .file
            .as_deref()
            .map(|f| f.trim_end_matches(".json").to_string());
        let mine = id.and_then(|id| OwnMapping::read(&own::path(&dir, &id)));
        if let (Some(mut mine), 4) = (mine.clone(), w.decks) {
            // With four decks, Settings' Deck order moves each deck's
            // controls to another deck.
            for r in &mut mine.mapping.rows {
                if let Action::Bind { control, .. } = &r.action {
                    r.by_position = control.contains("[Channel");
                }
            }
            if own::save(&dir, &mine).is_ok() {
                if let (Some(c), Some(f)) = (&self.controllers, &w.file) {
                    c.reload(f);
                }
            }
        }
        self.armed = None;
        g.set_armed("".into());
        g.set_walking(false);
        g.set_step("".into());
        g.set_last("".into());
        let name = mine.map_or_else(|| "The new mapping".to_string(), |m| m.name);
        g.set_status(
            format!("Done: {name} is in use. Try it, or click any control on screen to change it.")
                .into(),
        );
    }

    /// Reads which controls the controllers plugged in have bound.
    fn refresh_mapped(&mut self) {
        let mut bound = HashSet::new();
        let own_dir = settings::own_mappings_dir();
        for (_, file) in self.ports() {
            let base = self.base(&file);
            let own = if is_own_file(&file) {
                own_dir
                    .as_ref()
                    .and_then(|d| DjSugarMapping::read(&d.join(&file)).ok())
            } else {
                settings::controller_version(&file)
                    .zip(own_dir.as_ref())
                    .and_then(|(id, d)| DjSugarMapping::read(&own::path(d, &id)).ok())
            };
            let built = match (own, &self.dir) {
                (Some(o), Some(d)) => o.build(base.as_ref(), d),
                (None, _) => match base {
                    Some(b) => b,
                    None => continue,
                },
                (Some(_), None) => continue,
            };
            bound.extend(learn::bound_controls(&built));
        }
        *self.mapped.borrow_mut() = bound;
        self.tick += 1;
        if let Some(app) = self.app.upgrade() {
            app.global::<MapMode>().set_tick(self.tick);
        }
    }
}

/// What a control on screen does, in words: "Play · Deck 1", or for a pad
/// mode tab (an DJSugarScript line, see [`learn::SCRIPT`]) "Pad mode
/// button 2 · Deck 1".
fn describe(control: &str) -> String {
    if let Some(line) = control.strip_prefix(learn::SCRIPT) {
        let words: Vec<&str> = line.split_whitespace().collect();
        return match words.as_slice() {
            ["deck", d, "set", "'pad_tab'", t] => {
                let t: u8 = t.parse().unwrap_or(0);
                let shift = if t >= 4 { " with Shift" } else { "" };
                format!("Pad mode button {}{shift} · Deck {d}", t % 4 + 1)
            }
            _ => format!("\u{201c}{line}\u{201d}"),
        };
    }
    let (group, key) = control.split_once(',').unwrap_or(("", ""));
    let f = djsugar_controllers::catalog::describe(group, key);
    if f.target.is_empty() {
        f.name
    } else {
        format!("{} · {}", f.name, f.target)
    }
}
