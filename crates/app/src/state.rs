use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use sugar_engine::{Command, Control, DECKS, Telemetry, Track};
use sugar_layout::Rect;

include!(concat!(env!("OUT_DIR"), "/samples.rs"));

pub type Reader = fn(&str) -> Option<Vec<u8>>;

const OVERVIEW: usize = 1024;
pub const DETAIL_FRAMES: usize = 256;
const RATE_RANGE: f64 = 0.08;
const TAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Play(usize),
    Cue(usize),
    KeyLock(usize),
    Rate(usize),
    Volume(usize),
    Knob(usize, usize),
    Crossfader,
    Pad(usize, usize),
    Jog(usize),
    Overview(usize),
    Library,
    Browse(usize),
    CloseBrowser,
}

#[derive(Debug)]
pub struct Hit {
    pub rect: Rect,
    pub target: Target,
}

#[derive(Debug)]
pub struct Loaded {
    pub name: &'static str,
    pub track: Arc<Track>,
    pub overview: Box<[u8]>,
    pub detail: Box<[u8]>,
}

#[derive(Debug, Default)]
pub struct DeckUi {
    pub loaded: Option<Loaded>,
    pub loading: bool,
    pub rate: f32,
    pub volume: f32,
    pub knobs: [f32; 4],
    pub key_lock: bool,
    pub cue: f64,
}

impl DeckUi {
    fn new() -> Self {
        Self { volume: 0.85, knobs: [0.5; 4], ..Self::default() }
    }

    pub fn seconds(&self, frames: f64) -> f64 {
        self.loaded.as_ref().map_or(0.0, |l| frames / f64::from(l.track.rate))
    }

    pub fn length(&self) -> f64 {
        self.loaded.as_ref().map_or(0.0, |l| l.track.frames.len() as f64)
    }
}

type Done = (Job, Option<Loaded>);

enum Job {
    Deck(usize),
    Sample(usize, bool),
}

struct Drag {
    id: u64,
    target: Option<Target>,
    start: (f32, f32),
    last: (f32, f32),
    moved: bool,
    resume: bool,
}

pub struct State {
    control: Option<Control>,
    pub telemetry: Telemetry,
    pub decks: [DeckUi; DECKS],
    pub crossfader: f32,
    pub samples: Vec<Option<Arc<Track>>>,
    pub pads: [[usize; 8]; 2],
    pub focus: usize,
    pub browser: Option<usize>,
    pub scroll: f32,
    pub row_h: f32,
    pub library: Rect,
    pub hits: Vec<Hit>,
    drags: Vec<Drag>,
    read: Reader,
    jobs: (Sender<Done>, Receiver<Done>),
    pub status: Option<String>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State").field("focus", &self.focus).finish()
    }
}

fn peaks(t: &Track, bins: usize, per: usize) -> Box<[u8]> {
    let f = &t.frames;
    let per = if per == 0 { f.len().div_ceil(bins).max(1) } else { per };
    f.chunks(per)
        .map(|c| {
            let m = c.iter().fold(0.0f32, |m, s| m.max(s[0].abs()).max(s[1].abs()));
            (m.min(1.0) * 255.0) as u8
        })
        .collect()
}

impl State {
    pub fn new(control: Option<Control>, read: Reader, status: Option<String>) -> Self {
        let mut pads = [[0; 8]; 2];
        for (i, p) in pads.iter_mut().flatten().enumerate() {
            *p = i.min(SAMPLES.len().saturating_sub(1));
        }
        Self {
            control,
            telemetry: Telemetry::default(),
            decks: std::array::from_fn(|_| DeckUi::new()),
            crossfader: 0.0,
            samples: vec![None; SAMPLES.len()],
            pads,
            focus: 0,
            browser: None,
            scroll: 0.0,
            row_h: 22.0,
            library: Rect::new(0.0, 0.0, 0.0, 0.0),
            hits: Vec::with_capacity(128),
            drags: Vec::new(),
            read,
            jobs: channel(),
            status,
        }
    }

    fn send(&mut self, c: Command) {
        if let Some(ctl) = &mut self.control {
            ctl.send(c);
        }
    }

    pub fn animating(&self) -> bool {
        self.telemetry.decks.iter().any(|d| d.playing) || self.decks.iter().any(|d| d.loading)
    }

    pub fn tick(&mut self) {
        while let Ok((job, loaded)) = self.jobs.1.try_recv() {
            match (job, loaded) {
                (Job::Deck(d), Some(l)) => {
                    self.send(Command::Load(d, l.track.clone()));
                    self.decks[d].loaded = Some(l);
                    self.decks[d].loading = false;
                    self.decks[d].cue = 0.0;
                }
                (Job::Deck(d), None) => self.decks[d].loading = false,
                (Job::Sample(i, play), Some(l)) => {
                    if play {
                        self.send(Command::Sample(l.track.clone(), 0.9));
                    }
                    self.samples[i] = Some(l.track);
                }
                (Job::Sample(..), None) => {}
            }
        }
        if let Some(c) = &mut self.control {
            self.telemetry = c.poll();
        }
    }

    fn load(&mut self, job: Job, index: usize) {
        let Some(&name) = SAMPLES.get(index) else { return };
        let (tx, read) = (self.jobs.0.clone(), self.read);
        let detail = matches!(job, Job::Deck(_));
        std::thread::spawn(move || {
            let loaded = read(name)
                .and_then(|b| sugar_decode::bytes(b, name.rsplit('.').next().unwrap_or("ogg")).ok())
                .map(|t| {
                    let overview = if detail { peaks(&t, OVERVIEW, 0) } else { Box::default() };
                    let detail = if detail { peaks(&t, 0, DETAIL_FRAMES) } else { Box::default() };
                    Loaded { name, track: Arc::new(t), overview, detail }
                });
            let _ = tx.send((job, loaded));
        });
    }

    pub fn load_deck(&mut self, deck: usize, index: usize) {
        self.decks[deck].loading = true;
        self.load(Job::Deck(deck), index);
    }

    fn pad(&mut self, deck: usize, pad: usize) {
        let i = self.pads[deck % 2][pad];
        match self.samples.get(i).cloned().flatten() {
            Some(t) => self.send(Command::Sample(t, 0.9)),
            None => self.load(Job::Sample(i, true), i),
        }
    }

    fn hit(&self, x: f32, y: f32) -> Option<Target> {
        self.hits.iter().rev().find(|h| h.rect.hit(x, y)).map(|h| h.target)
    }

    pub fn pointer_down(&mut self, id: u64, x: f32, y: f32) {
        let target = self.hit(x, y);
        self.drags.retain(|d| d.id != id);
        let resume = matches!(target, Some(Target::Jog(d)) if self.telemetry.decks[d].playing);
        self.drags.push(Drag { id, target, start: (x, y), last: (x, y), moved: false, resume });
        let Some(t) = target else { return };
        match t {
            Target::Play(d) => {
                let on = !self.telemetry.decks[d].playing;
                self.focus = d;
                self.send(Command::Play(d, on));
                self.telemetry.decks[d].playing = on;
            }
            Target::Cue(d) => {
                self.focus = d;
                let cue = self.decks[d].cue;
                if self.telemetry.decks[d].playing {
                    self.send(Command::Play(d, false));
                    self.send(Command::Seek(d, cue));
                } else if (self.telemetry.decks[d].position - cue).abs() < 1.0 {
                    self.send(Command::Play(d, true));
                } else {
                    self.decks[d].cue = self.telemetry.decks[d].position;
                }
            }
            Target::KeyLock(d) => {
                let on = !self.decks[d].key_lock;
                self.decks[d].key_lock = on;
                self.send(Command::KeyLock(d, on));
            }
            Target::Pad(d, p) => {
                self.focus = d;
                self.pad(d, p);
            }
            Target::Jog(d) => {
                self.focus = d;
                self.send(Command::Play(d, false));
            }
            Target::Browse(d) => {
                self.focus = d;
                self.scroll = 0.0;
                self.browser = if self.browser == Some(d) { None } else { Some(d) };
            }
            Target::CloseBrowser => self.browser = None,
            _ => self.drag(t, x, y, (0.0, 0.0)),
        }
    }

    pub fn pointer_move(&mut self, id: u64, x: f32, y: f32) {
        let Some(i) = self.drags.iter().position(|d| d.id == id) else { return };
        let d = &mut self.drags[i];
        let delta = (x - d.last.0, y - d.last.1);
        d.last = (x, y);
        d.moved |= (x - d.start.0).hypot(y - d.start.1) > TAP;
        if let Some(t) = d.target {
            self.drag(t, x, y, delta);
        }
    }

    pub fn pointer_up(&mut self, id: u64, y: f32) {
        let Some(i) = self.drags.iter().position(|d| d.id == id) else { return };
        let d = self.drags.swap_remove(i);
        match d.target {
            Some(Target::Library) if !d.moved => {
                let row = ((y - self.library.y - 30.0 + self.scroll) / self.row_h).floor();
                if row >= 0.0 && (row as usize) < SAMPLES.len() && y > self.library.y + 30.0 {
                    let deck = match self.browser.take() {
                        Some(d) => d,
                        None if self.telemetry.decks[self.focus].playing => 1 - self.focus.min(1),
                        None => self.focus,
                    };
                    self.load_deck(deck, row as usize);
                }
            }
            Some(Target::Jog(deck)) if d.resume => {
                self.send(Command::Play(deck, true));
                self.telemetry.decks[deck].playing = true;
            }
            _ => {}
        }
    }

    pub fn wheel(&mut self, x: f32, y: f32, dy: f32) {
        if self.library.hit(x, y) {
            self.scroll_by(-dy);
        }
    }

    fn scroll_by(&mut self, dy: f32) {
        let max = (SAMPLES.len() as f32 * self.row_h - (self.library.h - 30.0)).max(0.0);
        self.scroll = (self.scroll + dy).clamp(0.0, max);
    }

    fn rect_of(&self, t: Target) -> Option<Rect> {
        self.hits.iter().find(|h| h.target == t).map(|h| h.rect)
    }

    fn drag(&mut self, t: Target, x: f32, y: f32, delta: (f32, f32)) {
        let Some(r) = self.rect_of(t) else { return };
        let vertical = |r: Rect| (1.0 - (y - r.y) / r.h).clamp(0.0, 1.0);
        match t {
            Target::Volume(d) => {
                let v = vertical(r);
                self.decks[d].volume = v;
                self.send(Command::Volume(d, v * v));
            }
            Target::Rate(d) => {
                let v = vertical(r) * 2.0 - 1.0;
                let v = if v.abs() < 0.04 { 0.0 } else { v };
                self.decks[d].rate = v;
                self.send(Command::Rate(d, f64::from(v) * RATE_RANGE));
            }
            Target::Crossfader => {
                let v = ((x - r.x) / r.w * 2.0 - 1.0).clamp(-1.0, 1.0);
                let v = if v.abs() < 0.04 { 0.0 } else { v };
                self.crossfader = v;
                self.send(Command::Crossfader(v));
            }
            Target::Knob(d, k) => {
                let v = (self.decks[d].knobs[k] - delta.1 / 160.0).clamp(0.0, 1.0);
                let v = if (v - 0.5).abs() < 0.02 { 0.5 } else { v };
                self.decks[d].knobs[k] = v;
                if k == 3 {
                    self.send(Command::Filter(d, v * 2.0 - 1.0));
                } else {
                    let eq = std::array::from_fn(|b| knob_gain(self.decks[d].knobs[2 - b]));
                    self.send(Command::Eq(d, eq));
                }
            }
            Target::Overview(d) => {
                let len = self.decks[d].length();
                let pos = f64::from(((x - r.x) / r.w).clamp(0.0, 1.0)) * len;
                self.send(Command::Seek(d, pos));
                self.telemetry.decks[d].position = pos;
            }
            Target::Jog(d) => {
                let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
                let (px, py) = (x - delta.0 - cx, y - delta.1 - cy);
                let turn = (px * (y - cy) - py * (x - cx)).atan2(px * (x - cx) + py * (y - cy));
                if let Some(l) = &self.decks[d].loaded {
                    let frames = f64::from(turn / std::f32::consts::TAU) * 1.8 * f64::from(l.track.rate);
                    let pos = (self.telemetry.decks[d].position + frames).clamp(0.0, self.decks[d].length());
                    self.telemetry.decks[d].position = pos;
                    self.send(Command::Seek(d, pos));
                }
            }
            Target::Library => self.scroll_by(-delta.1),
            _ => {}
        }
    }
}

pub fn knob_gain(v: f32) -> f32 {
    if v < 0.5 { (v * 2.0).powi(2) } else { 1.0 + (v - 0.5) * 2.0 }
}
