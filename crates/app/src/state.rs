use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use sugar_engine::{Command, Control, DECKS, Telemetry, Track};
use sugar_layout::Rect;

use crate::library::{self, Entry, Source};

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
    Mode(usize, Mode),
    Sync(usize),
    Jog(usize),
    Overview(usize),
    Library,
    Browse(usize),
    CloseBrowser,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Cues,
    Loops,
    Samples,
    Fx,
}

pub const ECHO_BEATS: [f64; 5] = [0.25, 0.5, 0.75, 1.0, 2.0];
pub const FX_PADS: [&str; 8] = ["1/4", "1/2", "3/4", "1", "2", "OUT", "BRK", "REV"];

pub const LOOP_BEATS: [f64; 8] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];

#[derive(Debug)]
pub struct Hit {
    pub rect: Rect,
    pub target: Target,
}

#[derive(Debug)]
pub struct Loaded {
    pub entry: Entry,
    pub track: Arc<Track>,
    pub overview: Box<[u8]>,
    pub detail: Box<[[u8; 4]]>,
    pub grid: Option<sugar_analysis::Grid>,
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
    pub mode: Mode,
    pub cues: [Option<f64>; 8],
    pub looping: Option<usize>,
    pub loop_range: Option<(f64, f64)>,
    pub echo: Option<usize>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Escape,
    Enter,
    Play(usize),
}

type Done = (Job, Option<Loaded>);

enum Job {
    Deck(usize),
    Sample(usize, bool),
    Scan(Vec<Entry>),
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
    pub entries: Vec<Entry>,
    pub shown: Vec<u32>,
    pub query: String,
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
    pub out_rate: u32,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State").field("focus", &self.focus).finish()
    }
}

fn bands(t: &Track, per: usize) -> Box<[[u8; 4]]> {
    let k = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz / t.rate as f32).exp();
    let (kl, kh) = (k(220.0), k(2500.0));
    let (mut lo, mut lohi) = (0.0f32, 0.0f32);
    t.frames
        .chunks(per)
        .map(|c| {
            let mut m = [0.0f32; 4];
            for s in c {
                let x = (s[0] + s[1]) * 0.5;
                lo += kl * (x - lo);
                lohi += kh * (x - lohi);
                let (l, mid, h) = (lo, lohi - lo, x - lohi);
                m[0] = m[0].max(x.abs());
                m[1] = m[1].max(l.abs());
                m[2] = m[2].max(mid.abs());
                m[3] = m[3].max(h.abs());
            }
            m.map(|v| (v.min(1.0) * 255.0) as u8)
        })
        .collect()
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
    pub fn new(control: Option<Control>, read: Reader, status: Option<String>, out_rate: u32) -> Self {
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
            entries: library::samples(),
            shown: (0..SAMPLES.len() as u32).collect(),
            query: String::new(),
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
            out_rate,
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
                    self.decks[d].cues = [None; 8];
                    self.decks[d].looping = None;
                    self.decks[d].loop_range = None;
                }
                (Job::Deck(d), None) => self.decks[d].loading = false,
                (Job::Sample(i, play), Some(l)) => {
                    if play {
                        self.send(Command::Sample(l.track.clone(), 0.9));
                    }
                    self.samples[i] = Some(l.track);
                }
                (Job::Sample(..), None) => {}
                (Job::Scan(found), _) => {
                    self.entries.truncate(SAMPLES.len());
                    self.entries.extend(found);
                    let q = std::mem::take(&mut self.query);
                    self.search(q);
                }
            }
        }
        if let Some(c) = &mut self.control {
            self.telemetry = c.poll();
        }
    }

    fn load(&mut self, job: Job, index: usize) {
        let Some(entry) = self.entries.get(index).cloned() else { return };
        let (tx, read) = (self.jobs.0.clone(), self.read);
        let detail = matches!(job, Job::Deck(_));
        std::thread::spawn(move || {
            let bytes = match &entry.source {
                Source::Sample(name) => read(name),
                Source::File(path) => std::fs::read(path).ok(),
            };
            let loaded = bytes.and_then(|b| sugar_decode::bytes(b, entry.extension()).ok()).map(|t| {
                let overview = if detail { peaks(&t, OVERVIEW, 0) } else { Box::default() };
                let detail = if detail { bands(&t, DETAIL_FRAMES) } else { Box::default() };
                let grid = if detail.is_empty() { None } else { sugar_analysis::grid(t.rate, &t.frames) };
                Loaded { entry, track: Arc::new(t), overview, detail, grid }
            });
            let _ = tx.send((job, loaded));
        });
    }

    pub fn scan(&mut self) {
        let roots = library::roots();
        if roots.is_empty() {
            return;
        }
        let tx = self.jobs.0.clone();
        std::thread::spawn(move || {
            let _ = tx.send((Job::Scan(library::scan(&roots)), None));
        });
    }

    pub fn search(&mut self, query: String) {
        self.query = query;
        self.scroll = 0.0;
        let q = &self.query;
        let (n, s) = (self.entries.len() as u32, SAMPLES.len() as u32);
        let order = (s..n).chain(0..s);
        self.shown = order.filter(|&i| library::matches(&self.entries[i as usize], q)).collect();
    }

    pub fn type_text(&mut self, text: &str) {
        let mut q = std::mem::take(&mut self.query);
        q.push_str(text);
        self.search(q);
    }

    pub fn key(&mut self, key: Key) {
        match key {
            Key::Backspace => {
                let mut q = std::mem::take(&mut self.query);
                q.pop();
                self.search(q);
            }
            Key::Escape => {
                self.browser = None;
                self.search(String::new());
            }
            Key::Enter => {
                if let Some(&first) = self.shown.first() {
                    let deck = self.browser.take().unwrap_or(self.focus);
                    self.load_deck(deck, first as usize);
                }
            }
            Key::Play(d) if self.decks[d].loaded.is_some() => {
                let on = !self.telemetry.decks[d].playing;
                self.send(Command::Play(d, on));
                self.telemetry.decks[d].playing = on;
            }
            Key::Play(_) => {}
        }
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

    fn hotcue(&mut self, d: usize, p: usize) {
        let Some(l) = &self.decks[d].loaded else { return };
        let rate = l.track.rate;
        let grid = l.grid;
        match self.decks[d].cues[p] {
            Some(at) => {
                self.send(Command::Seek(d, at));
                self.send(Command::Play(d, true));
                self.telemetry.decks[d].position = at;
            }
            None => {
                let pos = self.telemetry.decks[d].position;
                let at = grid.map_or(pos, |g| g.snap(rate, pos)).max(0.0);
                self.decks[d].cues[p] = Some(at);
            }
        }
    }

    fn fx(&mut self, d: usize, p: usize) {
        let Some(l) = &self.decks[d].loaded else { return };
        let rate = l.track.rate;
        let speed = 1.0 + f64::from(self.decks[d].rate) * RATE_RANGE;
        let beat = l.grid.map_or(0.5 * f64::from(rate), |g| g.beat_frames(rate)) / speed;
        let out_beat = beat * f64::from(self.out_rate) / f64::from(rate);
        match p {
            0..5 if self.decks[d].echo == Some(p) => {
                self.decks[d].echo = None;
                self.send(Command::Echo(d, 0.0, out_beat));
            }
            0..5 => {
                self.decks[d].echo = Some(p);
                self.send(Command::Echo(d, 0.6, out_beat * ECHO_BEATS[p]));
            }
            5 => {
                self.decks[d].echo = Some(3);
                self.send(Command::Echo(d, 0.75, out_beat));
                self.send(Command::Play(d, false));
                self.telemetry.decks[d].playing = false;
            }
            6 => self.send(Command::Brake(d, 1.2)),
            _ => self.send(Command::Spin(d, 0.8)),
        }
    }

    fn sync(&mut self, d: usize) {
        let o = 1 - d.min(1);
        let (Some(me), Some(other)) = (&self.decks[d].loaded, &self.decks[o].loaded) else { return };
        let (Some(g), Some(og)) = (me.grid, other.grid) else { return };
        let (rate, orate) = (me.track.rate, other.track.rate);
        let other_speed = 1.0 + f64::from(self.decks[o].rate) * RATE_RANGE;
        let mut target = og.bpm * other_speed;
        while target / g.bpm > 1.5 {
            target /= 2.0;
        }
        while target / g.bpm < 0.67 {
            target *= 2.0;
        }
        let r = ((target / g.bpm - 1.0) / RATE_RANGE).clamp(-1.0, 1.0);
        self.decks[d].rate = r as f32;
        self.send(Command::Rate(d, r * RATE_RANGE));
        if self.telemetry.decks[o].playing {
            let op = self.telemetry.decks[o].position;
            let phase = ((op - og.first_beat) / og.beat_frames(orate)).rem_euclid(1.0);
            let beat = g.beat_frames(rate);
            let pos = self.telemetry.decks[d].position;
            let base = g.first_beat + ((pos - g.first_beat) / beat).floor() * beat;
            let to = (base + phase * beat).max(0.0);
            self.send(Command::Seek(d, to));
            self.telemetry.decks[d].position = to;
        }
    }

    fn auto_loop(&mut self, d: usize, p: usize) {
        let Some(l) = &self.decks[d].loaded else { return };
        if self.decks[d].looping == Some(p) {
            self.decks[d].looping = None;
            self.decks[d].loop_range = None;
            self.send(Command::Loop(d, None));
            return;
        }
        let rate = l.track.rate;
        let g = l.grid.unwrap_or(sugar_analysis::Grid { bpm: 120.0, first_beat: 0.0 });
        let beat = g.beat_frames(rate);
        let pos = self.telemetry.decks[d].position;
        let start = g.first_beat + ((pos - g.first_beat) / beat).floor() * beat;
        let start = start.max(0.0);
        let range = (start, start + beat * LOOP_BEATS[p]);
        self.decks[d].looping = Some(p);
        self.decks[d].loop_range = Some(range);
        self.send(Command::Loop(d, Some(range)));
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
            Target::Play(d) if self.decks[d].loaded.is_some() => {
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
                match self.decks[d].mode {
                    Mode::Cues => self.hotcue(d, p),
                    Mode::Loops => self.auto_loop(d, p),
                    Mode::Samples => self.pad(d, p),
                    Mode::Fx => self.fx(d, p),
                }
            }
            Target::Mode(d, m) => self.decks[d].mode = m,
            Target::Sync(d) => self.sync(d),
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
                if let Some(&index) = self.shown.get(row.max(0.0) as usize)
                    && row >= 0.0
                    && y > self.library.y + 30.0
                {
                    let deck = match self.browser.take() {
                        Some(d) => d,
                        None if self.telemetry.decks[self.focus].playing => 1 - self.focus.min(1),
                        None => self.focus,
                    };
                    self.load_deck(deck, index as usize);
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
        let max = (self.shown.len() as f32 * self.row_h - (self.library.h - 30.0)).max(0.0);
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
