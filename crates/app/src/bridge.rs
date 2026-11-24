//! The screen's commands, carried out by the full engine through its named
//! controls, the same `[Group] item` controls the reference skin drives.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use djsugar_control::ControlRegistry;
use djsugar_engine::{AudioEngine, AudioSetup, SampleSource, deck_group, eq_group, quick_effect_group, scan};
use sugar_engine::{Command, DECKS, DeckState, Telemetry};

/// Sampler slots the pads cycle through.
const SLOTS: usize = 16;

#[derive(Debug)]
struct Slot {
    name: String,
    ready: Arc<AtomicU64>,
}

pub struct Bridge {
    engine: AudioEngine,
    reg: Arc<ControlRegistry>,
    /// Frames per second of the track on each deck, to turn the screen's
    /// frame positions into the engine's fractions.
    rate: [f64; DECKS],
    slots: Vec<Slot>,
    next: usize,
    pub last: Telemetry,
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge").field("last", &self.last).finish_non_exhaustive()
    }
}

/// The X-Pad's first bank: the app's own kick, snare, clap and hi-hat, written
/// once to a folder the engine can read.
fn xpad_kit(read: crate::state::Reader) -> [Option<PathBuf>; 4] {
    let dir = std::env::temp_dir().join("djsugar-samples");
    djsugar_engine::fxstation::instruments::FACTORY_FILES.map(|name| {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
        let bytes = read(name)?;
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, bytes).ok()?;
        Some(path)
    })
}

impl Bridge {
    pub fn start(read: crate::state::Reader) -> Self {
        let reg = ControlRegistry::new();
        let engine = AudioEngine::start(reg.clone(), &AudioSetup::default());
        engine.xpad_factory(xpad_kit(read));
        let slots = (0..SLOTS).map(|_| Slot { name: String::new(), ready: Arc::default() }).collect();
        Self { engine, reg, rate: [44_100.0; DECKS], slots, next: 0, last: Telemetry::default() }
    }

    pub fn rate(&self) -> u32 {
        self.engine.sample_rate() as u32
    }

    fn set(&self, group: &str, item: &str, v: f64) {
        if let Some(c) = self.reg.get(group, item) {
            c.set(v);
        }
    }

    fn get(&self, group: &str, item: &str) -> f64 {
        self.reg.get(group, item).map_or(0.0, |c| c.get())
    }

    fn deck(&self, d: usize, item: &str, v: f64) {
        self.set(&deck_group(d), item, v);
    }

    /// Loads the file on a deck; the engine decodes and analyses it.
    pub fn load(&mut self, d: usize, path: &Path, rate: u32) {
        self.rate[d] = f64::from(rate.max(1));
        let track = scan::read_track(d, path);
        self.engine.load_track(d, Arc::new(track));
    }

    /// Plays a sample file on the next free sampler slot, loading it first
    /// when the slot holds something else.
    pub fn sample(&mut self, name: &str, path: PathBuf, gain: f32) {
        let at = self.slots.iter().position(|s| s.name == name).unwrap_or_else(|| {
            let at = self.next;
            self.next = (self.next + 1) % SLOTS;
            at
        });
        let group = djsugar_engine::sampler::sampler_group(at);
        self.set(&group, "pregain", f64::from(gain));
        let slot = &mut self.slots[at];
        let generation = slot.ready.fetch_add(1, Ordering::AcqRel) + 1;
        if slot.name == name {
            self.set(&group, "cue_gotoandplay", 1.0);
            return;
        }
        slot.name = name.to_owned();
        let (reg, ready) = (self.reg.clone(), slot.ready.clone());
        self.engine.set_sample(at, SampleSource::File(path), move |r| {
            if r.is_ok()
                && ready.load(Ordering::Acquire) == generation
                && let Some(c) = reg.get(&group, "cue_gotoandplay")
            {
                c.set(1.0);
            }
        });
    }

    pub fn send(&mut self, c: Command) -> bool {
        match c {
            Command::Load(..) | Command::Sample(..) | Command::Station(_) | Command::Echo(..) => {}
            Command::Unload(d) => self.deck(d, "eject", 1.0),
            Command::Play(d, on) => self.deck(d, "play", f64::from(u8::from(on))),
            Command::Seek(d, frames) => {
                let length = self.get(&deck_group(d), "duration") * self.rate[d];
                if length > 0.0 {
                    self.deck(d, "playposition", (frames / length).clamp(0.0, 1.0));
                }
            }
            Command::Rate(d, r) => {
                let range = self.get(&deck_group(d), "rateRange").max(0.01);
                self.deck(d, "rate", (r / range).clamp(-1.0, 1.0));
            }
            Command::Nudge(d, n) => {
                self.deck(d, "rate_temp_up", f64::from(u8::from(n > 0.0)));
                self.deck(d, "rate_temp_down", f64::from(u8::from(n < 0.0)));
            }
            Command::KeyLock(d, on) => self.deck(d, "keylock", f64::from(u8::from(on))),
            Command::Pfl(d, on) => self.deck(d, "pfl", f64::from(u8::from(on))),
            Command::Quantize(d, on) => self.deck(d, "quantize", f64::from(u8::from(on))),
            Command::BeatSync(d) => self.deck(d, "beatsync", 1.0),
            Command::SyncLock(d, on) => self.deck(d, "sync_enabled", f64::from(u8::from(on))),
            Command::SyncLeader(d, on) => self.deck(d, "sync_leader", if on { 2.0 } else { 0.0 }),
            Command::Loop(d, l) => {
                let g = deck_group(d);
                match l {
                    Some((a, b)) => {
                        self.set(&g, "loop_start_position", a * 2.0);
                        self.set(&g, "loop_end_position", b * 2.0);
                        if self.get(&g, "loop_enabled") == 0.0 {
                            self.set(&g, "reloop_toggle", 1.0);
                        }
                    }
                    None if self.get(&g, "loop_enabled") != 0.0 => self.set(&g, "reloop_toggle", 1.0),
                    None => {}
                }
            }
            Command::Brake(d, _) => self.deck(d, "play", 0.0),
            Command::Spin(d, _) => {
                self.deck(d, "reverseroll", 1.0);
                self.deck(d, "reverseroll", 0.0);
            }
            Command::Reverse(d, on) => self.deck(d, "reverse", f64::from(u8::from(on))),
            Command::Slip(d, on) => self.deck(d, "slip_enabled", f64::from(u8::from(on))),
            Command::Key(d, st) => self.deck(d, "pitch", f64::from(st)),
            Command::Gain(d, g) => self.deck(d, "pregain", f64::from(g)),
            Command::Volume(d, v) => self.deck(d, "volume", f64::from(v)),
            Command::Eq(d, e) => {
                let g = eq_group(d);
                for (i, v) in e.into_iter().enumerate() {
                    self.set(&g, &format!("parameter{}", i + 1), f64::from(v));
                }
            }
            Command::Kill(d, k) => {
                let g = eq_group(d);
                for (i, on) in k.into_iter().enumerate() {
                    self.set(&g, &format!("button_parameter{}", i + 1), f64::from(u8::from(on)));
                }
            }
            Command::Filter(d, f) => self.set(&quick_effect_group(d), "super1", f64::from(f) * 0.5 + 0.5),
            Command::StopSamples => {
                for i in 0..SLOTS {
                    self.set(&djsugar_engine::sampler::sampler_group(i), "cue_gotoandstop", 1.0);
                }
            }
            Command::Crossfader(x) => self.set("[Master]", "crossfader", f64::from(x)),
            Command::Curve(c) => {
                use sugar_engine::Curve;
                self.set("[Mixer]", "crossfader_enabled", f64::from(u8::from(c != Curve::Off)));
                let (shape, cut) = match c {
                    Curve::Off | Curve::Smooth => (0.0, None),
                    Curve::Linear => (1.0, None),
                    Curve::Cut(c) => (2.0, Some(c)),
                };
                self.set("[Mixer Profile]", "xFaderShape", shape);
                if let Some(c) = cut {
                    self.set("[Mixer Profile]", "xFaderCutIn", f64::from(c));
                }
            }
            Command::Main(v) => self.set("[Master]", "gain", f64::from(v)),
            Command::Record(on) => {
                let r = self.engine.recorder();
                if on {
                    let _ = r.start();
                } else {
                    r.stop();
                }
            }
        }
        true
    }

    pub fn poll(&mut self) -> Telemetry {
        let mut t = Telemetry::default();
        for (d, s) in t.decks.iter_mut().enumerate() {
            let g = deck_group(d);
            *s = DeckState {
                position: self.get(&g, "playposition").max(0.0) * self.get(&g, "duration") * self.rate[d],
                playing: self.get(&g, "play") != 0.0,
                peak: self.get(&g, "VuMeter") as f32,
                sync_lock: self.get(&g, "sync_enabled") != 0.0,
                sync_leader: self.get(&g, "sync_leader").round() as u8,
            };
        }
        t.main_peak = [self.get("[Master]", "VuMeterL") as f32, self.get("[Master]", "VuMeterR") as f32];
        self.last = t;
        t
    }

    /// The engine writes recordings itself.
    pub fn drain_recording(&mut self, _out: &mut Vec<f32>) -> usize {
        0
    }
}
