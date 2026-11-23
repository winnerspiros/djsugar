//! The Groove Box: a deck that plays a drum kit in a 16-step pattern, in time with the mix, instead of a track. `[GrooveN],enabled` turns deck N into a Groove Box. Its sound goes into the deck's channel where a track's would, so EQ, filter, fader, crossfader, effects and headphone cue work as on any deck, and `[ChannelN],play` starts and stops it. **Clock.** The groove keeps its own beat count and follows, in order: the leading deck while it plays, Link's session while Link is on, or its own tempo (`[GrooveN],bpm`). It never copies the leader's position; it bends its speed toward the leader's beat a little each buffer, as sync does, and when it is more than a quarter of a beat off (a hot cue or beat jump on the leader) it jumps to the leader's beat at its next bar line, never mid-bar. Started while a leader plays, it waits for the leader's next downbeat. **Steps.** Step k (of 16 to a bar) falls on beat k/4, every second one late by `swing`. Each buffer covers beats `[b0, b0 + frames × step)`; every step inside it starts its sounds on the frame it falls on, so the timing is exact to the sample whatever the buffer size. **Patterns.** Four grooves
//! while `chain` is on the two take turns, a bar each. `groove_playing` is the one sounding now. Each row has `rowR_mute`, `rowR_solo`, `rowR_level` (0-1.5), `rowR_pitch` (−12 to +12 semitones) and `rowR_decay` (0-1, 1 = the sound's own length), `rowR_tone` (−1 to 1: left a low-pass, right a high-pass, centre off) and `rowR_pan` (−1 left to 1 right), and `rowR_sound` picks which of the kit's sounds the row plays; `rowR_preview` plays it once, now, and `rowR_meter` (0-1) shows how loud the row sounds, for the screen. `[GrooveN],kit` picks the kit. `[GrooveN],position` is the step playing (0-15), −1 when stopped. **Fills.** `auto_fill` (1 or 2) plays a fill at the end of every 4th, 8th and 16th bar (or 2nd, 4th, 8th), `fill` held plays one now, and `remix` (a strip, 0 off) plays one of 16 figures while held; see [`crate::groove_fills`]. `fill_playing` shows when one sounds. **Duck.** With `[GrooveN],duck_deck` set to another deck (1-4), that deck dips by `duck_amount` (dB) on each step of row 1 and recovers over about a step. Kits are synthesised when the engine starts (see [`crate::kits`]); every buffer only reads them, and nothing allocates or frees here.
//! while `chain` is on the two take turns, a bar each. `groove_playing` is the one sounding now. Each row has `rowR_mute`, `rowR_solo`, `rowR_level` (0-1.5), `rowR_pitch` (−12 to +12 semitones) and `rowR_decay` (0-1, 1 = the sound's own length), `rowR_tone` (−1 to 1: left a low-pass, right a high-pass, centre off) and `rowR_pan` (−1 left to 1 right), and `rowR_sound` picks which of the kit's sounds the row plays; `rowR_preview` plays it once, now, and `rowR_meter` (0-1) shows how loud the row sounds, for the screen. `[GrooveN],kit` picks the kit. `[GrooveN],position` is the step playing (0-15), −1 when stopped. **Fills.** `auto_fill` (1 or 2) plays a fill at the end of every 4th, 8th and 16th bar (or 2nd, 4th, 8th), `fill` held plays one now, and `remix` (a strip, 0 off) plays one of 16 figures while held; see [`crate::groove_fills`]. `fill_playing` shows when one sounds. **Duck.** With `[GrooveN],duck_deck` set to another deck (1-4), that deck dips by `duck_amount` (dB) on each step of row 1 and recovers over about a step. Kits are synthesised when the engine starts (see [`crate::kits`]); every buffer only reads them, and nothing allocates or frees

use std::sync::Arc;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};

use crate::groove_dice::{self, Dice, Habits, Move, Track};
use crate::groove_fills;
use crate::groove_pads::{self, mode, PadSurface};
use crate::kits::{self, Kit};
use crate::patterns::{self, Level};
use crate::sampler::Sample;
use crate::NUM_DECKS;

/// Tracks: the first seven of [`kits::ROLES`], then Cymbal, which plays the
/// kit's crash or ride (its sound picked when a groove loads).
pub const ROWS: usize = 8;
/// The kit sounds track 8 switches between.
const CRASH: usize = 7;
const RIDE: usize = 8;
pub const STEPS: usize = 16;
pub const GROOVES: usize = 4;

/// The tempo a Groove Box plays at before anything else sets one.
pub const DEFAULT_BPM: f64 = 120.0;
/// Further off the leader's beat than this, the groove jumps at its next
/// bar instead of bending its speed.
const SNAP_BEATS: f64 = 0.25;
/// The most the groove bends its speed to catch up with the leader.
const MAX_BEND: f64 = 0.05;
/// Frames over which a sound that is cut off fades out.
const CHOKE_FRAMES: usize = 64;

/// The controls of one Groove Box.
pub struct GrooveControls {
    pub enabled: ControlHandle,
    pub kit: ControlHandle,
    pub groove: ControlHandle,
    pub groove_queued: ControlHandle,
    pub groove_playing: ControlHandle,
    pub genre: ControlHandle,
    pub pattern: ControlHandle,
    pub load: ControlHandle,
    pub chain: ControlHandle,
    pub load_chain: ControlHandle,
    pub bpm: ControlHandle,
    pub swing: ControlHandle,
    pub position: ControlHandle,
    /// The groove's beat at the end of the last buffer while it runs (beat 0
    /// a bar line), for the scrolling waveform's step lanes.
    pub beat: ControlHandle,
    /// On while the groove follows a playing deck or Link.
    pub following: ControlHandle,
    /// 1 (the default): follow the mix's tempo; 0: keep `bpm`.
    pub follow: ControlHandle,
    /// While following: 0.5 half time, 1, or 2 double time.
    pub tempo_scale: ControlHandle,
    /// The kick in the leading deck's key: 0 off, 1 its root, 2 its fifth.
    pub kick_key: ControlHandle,
    /// The note the kick is tuned to, 1-12 from C (0 untuned), to show.
    pub kick_note: ControlHandle,
    /// A variation to make on the next bar ([`groove_dice::Move`] by
    /// number, 1-7); back to 0 once it is waiting.
    pub dice: ControlHandle,
    /// Which track `dice` changes: 1-8, or 0 for every unlocked track.
    pub dice_row: ControlHandle,
    /// A variation is waiting for the bar (1) or not (0).
    pub dice_pending: ControlHandle,
    /// Puts back the groove as it was before the last variation.
    pub dice_undo: ControlHandle,
    /// 1 while there is a variation to undo.
    pub can_undo: ControlHandle,
    /// A locked track keeps its steps when every track is varied.
    pub lock: [ControlHandle; ROWS],
    pub duck_deck: ControlHandle,
    pub duck_amount: ControlHandle,
    pub patterns: [[ControlHandle; ROWS]; GROOVES],
    pub soft: [[ControlHandle; ROWS]; GROOVES],
    pub accent: [[ControlHandle; ROWS]; GROOVES],
    /// `patternG_rowR_maybe`: steps that play only half the time.
    pub maybe: [[ControlHandle; ROWS]; GROOVES],
    /// 0-1: how much the groove varies by itself as it plays (weak hits
    /// drop out, ghost notes come in, loudness wavers).
    pub vary: ControlHandle,
    /// Auto fill: 0 off, 1 Auto 1 (bars 4, 8, 16), 2 Auto 2 (2, 4, 8).
    pub auto_fill: ControlHandle,
    /// Held (1): a fill plays, the whole bar, until let go.
    pub fill: ControlHandle,
    /// The Remix strip, 0 off, else where it is held (to 1): 16 figures.
    pub remix: ControlHandle,
    /// A fill or Remix is playing.
    pub fill_playing: ControlHandle,
    pub mute: [ControlHandle; ROWS],
    pub solo: [ControlHandle; ROWS],
    pub level: [ControlHandle; ROWS],
    pub pitch: [ControlHandle; ROWS],
    pub decay: [ControlHandle; ROWS],
    pub tone: [ControlHandle; ROWS],
    pub pan: [ControlHandle; ROWS],
    pub sound: [ControlHandle; ROWS],
    /// How loud the row sounds now (0-1), for the screen.
    pub meter: [ControlHandle; ROWS],
    /// Plays the row's sound once, now (a step clicked while stopped).
    pub preview: [ControlHandle; ROWS],
}

pub fn groove_group(deck: usize) -> String {
    format!("[Groove{}]", deck + 1)
}

impl GrooveControls {
    pub fn new(reg: &ControlRegistry, deck: usize) -> Self {
        let g = groove_group(deck);
        let g = g.as_str();
        let rows = |f: &dyn Fn(usize) -> ControlHandle| std::array::from_fn(f);
        let kit_count = kits::KITS.len() as f64;
        let lib = patterns::library();
        let genres = lib.len().max(1) as f64;
        let most = lib.iter().map(|g| g.grooves.len()).max().unwrap_or(0) as f64;
        let c = Self {
            enabled: reg.toggle(g, "enabled"),
            kit: reg.create(g, "kit", 0.0, 0.0, kit_count - 1.0, Behavior::Free),
            groove: reg.create(g, "groove", 0.0, 0.0, (GROOVES - 1) as f64, Behavior::Free),
            groove_queued: reg.indicator(g, "groove_queued", (GROOVES - 1) as f64),
            groove_playing: reg.indicator(g, "groove_playing", (GROOVES - 1) as f64),
            genre: reg.create(g, "genre", 0.0, 0.0, genres - 1.0, Behavior::Free),
            pattern: reg.create(g, "pattern", 1.0, 0.0, most, Behavior::Free),
            load: reg.push(g, "load"),
            chain: reg.create(g, "chain", 0.0, 0.0, most, Behavior::Free),
            load_chain: reg.push(g, "load_chain"),
            bpm: reg.create(g, "bpm", DEFAULT_BPM, 40.0, 220.0, Behavior::Linear),
            swing: reg.create(g, "swing", 0.0, 0.0, 0.75, Behavior::Linear),
            position: reg.create(g, "position", -1.0, -1.0, (STEPS - 1) as f64, Behavior::Indicator),
            beat: reg.create(g, "beat", 0.0, f64::MIN, f64::MAX, Behavior::Indicator),
            following: reg.indicator(g, "following", 1.0),
            follow: reg.create(g, "follow", 1.0, 0.0, 1.0, Behavior::Free),
            tempo_scale: reg.create(g, "tempo_scale", 1.0, 0.5, 2.0, Behavior::Free),
            kick_key: reg.create(g, "kick_key", 0.0, 0.0, 2.0, Behavior::Free),
            kick_note: reg.indicator(g, "kick_note", 12.0),
            dice: reg.create(g, "dice", 0.0, 0.0, 7.0, Behavior::Free),
            dice_row: reg.create(g, "dice_row", 0.0, 0.0, ROWS as f64, Behavior::Free),
            dice_pending: reg.indicator(g, "dice_pending", 1.0),
            dice_undo: reg.push(g, "dice_undo"),
            can_undo: reg.indicator(g, "can_undo", 1.0),
            lock: rows(&|r| reg.toggle(g, &format!("row{}_lock", r + 1))),
            duck_deck: reg.create(g, "duck_deck", 0.0, 0.0, NUM_DECKS as f64, Behavior::Free),
            duck_amount: reg.create(g, "duck_amount", 6.0, 0.0, 24.0, Behavior::Linear),
            patterns: std::array::from_fn(|p| {
                rows(&|r| reg.create(g, &format!("pattern{}_row{}", p + 1, r + 1), 0.0, 0.0, 65535.0, Behavior::Free))
            }),
            soft: std::array::from_fn(|p| {
                rows(&|r| {
                    reg.create(g, &format!("pattern{}_row{}_soft", p + 1, r + 1), 0.0, 0.0, 65535.0, Behavior::Free)
                })
            }),
            accent: std::array::from_fn(|p| {
                rows(&|r| {
                    reg.create(g, &format!("pattern{}_row{}_accent", p + 1, r + 1), 0.0, 0.0, 65535.0, Behavior::Free)
                })
            }),
            maybe: std::array::from_fn(|p| {
                rows(&|r| {
                    reg.create(g, &format!("pattern{}_row{}_maybe", p + 1, r + 1), 0.0, 0.0, 65535.0, Behavior::Free)
                })
            }),
            vary: reg.create(g, "vary", 0.0, 0.0, 1.0, Behavior::Linear),
            auto_fill: reg.create(g, "auto_fill", 0.0, 0.0, 2.0, Behavior::Free),
            fill: reg.create(g, "fill", 0.0, 0.0, 1.0, Behavior::Free),
            remix: reg.create(g, "remix", 0.0, 0.0, 1.0, Behavior::Linear),
            fill_playing: reg.indicator(g, "fill_playing", 1.0),
            mute: rows(&|r| reg.toggle(g, &format!("row{}_mute", r + 1))),
            solo: rows(&|r| reg.toggle(g, &format!("row{}_solo", r + 1))),
            level: rows(&|r| reg.create(g, &format!("row{}_level", r + 1), 1.0, 0.0, 1.5, Behavior::Linear)),
            pitch: rows(&|r| reg.create(g, &format!("row{}_pitch", r + 1), 0.0, -12.0, 12.0, Behavior::Linear)),
            decay: rows(&|r| reg.create(g, &format!("row{}_decay", r + 1), 1.0, 0.0, 1.0, Behavior::Linear)),
            tone: rows(&|r| reg.create(g, &format!("row{}_tone", r + 1), 0.0, -1.0, 1.0, Behavior::Linear)),
            pan: rows(&|r| reg.create(g, &format!("row{}_pan", r + 1), 0.0, -1.0, 1.0, Behavior::Linear)),
            meter: rows(&|r| reg.create(g, &format!("row{}_meter", r + 1), 0.0, 0.0, 1.0, Behavior::Indicator)),
            sound: rows(&|r| {
                reg.create(
                    g,
                    &format!("row{}_sound", r + 1),
                    r as f64,
                    0.0,
                    (kits::MAX_SOUNDS - 1) as f64,
                    Behavior::Free,
                )
            }),
            preview: rows(&|r| reg.push(g, &format!("row{}_preview", r + 1))),
        };
        // A new Groove Box opens with the library's first groove.
        c.load_groove(0, 0, 1);
        c
    }

    /// Puts groove `index` (1-based) of genre `genre` into groove `slot`.
    pub fn load_groove(&self, slot: usize, genre: usize, index: usize) {
        let Some(p) = index.checked_sub(1).and_then(|i| patterns::groove(genre, i)) else {
            return;
        };
        let slot = slot.min(GROOVES - 1);
        // Cymbal plays the ride when the groove has one, else the crash.
        let ride = p.rows[RIDE].hits != 0;
        let cymbal = if ride { RIDE } else { CRASH };
        if ride || p.rows[CRASH].hits != 0 {
            self.sound[ROWS - 1].set(cymbal as f64);
        }
        for row in 0..ROWS {
            let steps = p.rows[if row == ROWS - 1 { cymbal } else { row }];
            self.patterns[slot][row].set(steps.hits as f64);
            self.soft[slot][row].set(steps.soft as f64);
            self.accent[slot][row].set(steps.accent as f64);
            self.maybe[slot][row].set(steps.maybe as f64);
        }
    }

    /// Groove `slot`'s tracks as they are now.
    pub fn tracks(&self, slot: usize) -> [Track; ROWS] {
        std::array::from_fn(|r| Track {
            hits: self.patterns[slot][r].get() as u32,
            soft: self.soft[slot][r].get() as u32,
            accent: self.accent[slot][r].get() as u32,
            maybe: self.maybe[slot][r].get() as u32,
        })
    }

    /// Writes track `row` of groove `slot`.
    pub fn set_track(&self, slot: usize, row: usize, t: Track) {
        self.patterns[slot][row].set((t.hits & 0xffff) as f64);
        self.soft[slot][row].set((t.soft & t.hits & 0xffff) as f64);
        self.accent[slot][row].set((t.accent & t.hits & 0xffff) as f64);
        self.maybe[slot][row].set((t.maybe & t.hits & 0xffff) as f64);
    }

    /// The kit sound (one of [`kits::ROLES`]) track `row` plays, for
    /// variations in its style.
    fn role(&self, row: usize) -> usize {
        if row == ROWS - 1 {
            if self.sound[row].get().round() as usize == RIDE {
                RIDE
            } else {
                CRASH
            }
        } else {
            row
        }
    }

    /// The groove the library's `load` and `chain` go into: the one picked,
    /// and its partner (A and B, C and D).
    fn slots(&self) -> (usize, usize) {
        let chosen = (self.groove.get().round() as usize).min(GROOVES - 1);
        (chosen, chosen ^ 1)
    }
}

/// Where the groove's beat comes from this buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// The tempo heard, in beats per minute.
    pub bpm: f64,
    /// The leader's beat at the buffer's first frame (beat 0 a downbeat);
    /// None when the groove keeps its own time.
    pub beat: Option<f64>,
}

/// The groove's sense of time, separate from the sound so it can be tested
/// on its own.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    /// The beat at the next buffer's first frame.
    pub beat: f64,
    pub running: bool,
    /// No step before this beat plays (waiting for the leader's downbeat).
    start_at: f64,
    /// A jump onto the leader's beat, taken at the next bar line.
    pending: Option<f64>,
}

impl Clock {
    /// Starts the groove: on the leader's next downbeat, or at once.
    pub fn start(&mut self, source: Source) {
        self.running = true;
        self.pending = None;
        match source.beat {
            Some(b) => {
                self.beat = b;
                // A hair of tolerance so a start right on the bar plays it.
                self.start_at = ((b - 1e-6) / 4.0).ceil() * 4.0;
            }
            None => {
                self.beat = 0.0;
                self.start_at = 0.0;
            }
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
        self.pending = None;
    }

    /// Advances over a buffer of `frames` at `rate`; returns the beat at its first frame and the beats per
    /// frame. A jump waiting for the bar line is taken here when the bar line falls in this buffer: the
    /// returned span then starts on the bar line (`b0` is moved so the bar's steps land on the same frames).
    pub fn advance(&mut self, frames: usize, rate: f64, source: Source) -> Span {
        let base = source.bpm.max(1.0) / 60.0 / rate;
        let mut per_frame = base;
        if let Some(target) = source.beat {
            // Off by how much, within a bar either way.
            let mut off = target - self.beat;
            off -= (off / 4.0).round() * 4.0;
            if off.abs() > SNAP_BEATS {
                self.pending = Some(off);
            } else {
                self.pending = None;
                // Catch up over about a beat.
                let span = base * frames as f64;
                per_frame = base * (1.0 + (off / span.max(1.0)).clamp(-MAX_BEND, MAX_BEND));
            }
        } else {
            self.pending = None;
        }
        let b0 = self.beat;
        let b1 = b0 + per_frame * frames as f64;
        let mut span = Span { beat: b0, per_frame, jump: None };
        if let Some(off) = self.pending {
            let bar = (b0 / 4.0).floor() * 4.0 + 4.0;
            if bar < b1 {
                // At the bar line the count moves by `off`: from the first
                // frame on or after it, the frames play the leader's beats.
                let at = (((bar - b0) / per_frame).ceil() as usize).min(frames);
                span.jump = Some((at, off));
                self.pending = None;
                self.beat = b1 + off;
                self.start_at = self.start_at.min(bar + off);
                return span;
            }
        }
        self.beat = b1;
        span
    }

    /// Whether a step at `beat` may play.
    pub fn plays(&self, beat: f64) -> bool {
        self.running && beat >= self.start_at - 1e-9
    }
}

/// One buffer's stretch of beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub beat: f64,
    pub per_frame: f64,
    /// From frame `at` on, the beat count is `off` further along.
    pub jump: Option<(usize, f64)>,
}

impl Span {
    /// The beat at frame `f` of the buffer.
    pub fn beat_at(&self, f: usize) -> f64 {
        let b = self.beat + self.per_frame * f as f64;
        match self.jump {
            Some((at, off)) if f >= at => b + off,
            _ => b,
        }
    }

    /// The steps that start in this buffer of `frames`: (frame, step) in
    /// order, with every second step `swing` (0-0.75) of a 16th late.
    pub fn steps(&self, frames: usize, swing: f64, mut each: impl FnMut(usize, usize, f64)) {
        let parts = match self.jump {
            Some((at, off)) => [(0, at, 0.0), (at, frames, off)],
            None => [(0, frames, 0.0), (frames, frames, 0.0)],
        };
        let late = swing.clamp(0.0, 0.75) * 0.25;
        for (from, to, off) in parts {
            if from >= to {
                continue;
            }
            let start = self.beat + off + self.per_frame * from as f64;
            let end = self.beat + off + self.per_frame * to as f64;
            // Steps are 16ths: step k at beat k/4 (plus swing when odd).
            let first = ((start - late) * 4.0).floor() as i64;
            let last = (end * 4.0).ceil() as i64;
            for k in first..=last {
                let t = k as f64 / 4.0 + if k.rem_euclid(2) == 1 { late } else { 0.0 };
                if t < start - 1e-9 || t >= end - 1e-9 {
                    continue;
                }
                let f = from + ((t - start) / self.per_frame).round() as usize;
                each(f.min(to - 1), k.rem_euclid(STEPS as i64) as usize, t);
            }
        }
    }
}

/// A sound playing on a row.
#[derive(Default)]
struct Voice {
    sample: Option<Arc<Sample>>,
    pos: f64,
    rate: f64,
    /// Left and right gain (level and pan).
    gain: [f32; 2],
    /// The tone filter: a one-pole low-pass's coefficient (0 = off), its
    /// state per side, and whether the output is what it lets through
    /// (low-pass) or what it takes away (high-pass).
    tone: f32,
    high: bool,
    z: [f32; 2],
    /// The decay envelope and how fast it falls each frame.
    env: f32,
    fall: f32,
    /// Frames of fade left when cut off (0 while it plays on).
    fade: usize,
}

impl Voice {
    fn next(&mut self) -> [f32; 2] {
        let Some(s) = &self.sample else {
            return [0.0; 2];
        };
        let i = self.pos as usize;
        if i + 1 >= s.len() || self.env < 1e-4 {
            self.sample = None;
            return [0.0; 2];
        }
        let t = (self.pos - i as f64) as f32;
        let (a, b) = (s.frame(i), s.frame(i + 1));
        let mut g = self.env;
        if self.fade > 0 {
            g *= self.fade as f32 / CHOKE_FRAMES as f32;
            self.fade -= 1;
            if self.fade == 0 {
                self.sample = None;
            }
        }
        self.env *= self.fall;
        self.pos += self.rate;
        let mut x = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        if self.tone > 0.0 {
            for (x, z) in x.iter_mut().zip(self.z.iter_mut()) {
                *z += self.tone * (*x - *z);
                *x = if self.high { *x - *z } else { *z };
            }
        }
        [x[0] * g * self.gain[0], x[1] * g * self.gain[1]]
    }
}

/// A hit due in this buffer.
#[derive(Clone, Copy, Default)]
struct Hit {
    frame: usize,
    row: usize,
    /// How loud: the step's level, or how hard a pad was hit.
    velocity: f32,
    /// Part of a fill or Remix.
    fill: bool,
}

/// A hit the Groove Box played, for the deck's scrolling waveform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrooveHit {
    /// The groove's beat it played on.
    pub beat: f64,
    pub row: u8,
    pub velocity: f32,
    /// Part of a fill or Remix.
    pub fill: bool,
}

/// The seed that varies deck `deck`'s fills (see [`groove_fills`]), so the
/// app can show the fills to come.
pub fn fill_seed(deck: usize) -> u32 {
    (deck as u32 + 1).wrapping_mul(0x85eb_ca6b)
}

const MAX_HITS: usize = 64;

pub struct GrooveBox {
    pub c: GrooveControls,
    pub clock: Clock,
    kits: Arc<[Kit]>,
    /// Each row's sound now and the one fading out after a choke.
    voices: [[Voice; 2]; ROWS],
    /// The groove playing and the one waiting for the bar.
    playing: usize,
    queued: Option<usize>,
    was_running: bool,
    /// A bar has started since PLAY, so the chain may turn at the next.
    bar_played: bool,
    /// The last tempo heard, kept when the leader stops.
    pub last_bpm: f64,
    rate: f64,
    hits: [Hit; MAX_HITS],
    /// A controller's pads (see [`crate::groove_pads`]).
    pub pads: PadSurface,
    /// A live hit recorded into a step still to come: that step skips the
    /// row once, so the hit isn't heard twice.
    skip: [Option<usize>; ROWS],
    /// The leading deck's key's tonic (0-11 from C), for the kick's KEY.
    pub key: Option<u8>,
    /// Variations waiting for the bar, and their undo steps.
    pub dice: Dice,
    /// Varies the fills from one Groove Box to another.
    fill_seed: u32,
    /// A fill just ended: the cymbal crashes on the next downbeat.
    crash_next: bool,
    /// The last step played was a fill or Remix.
    in_fill: bool,
    /// Hits per track in the last buffer.
    pub played: [u32; ROWS],
    /// Where the hits played go, for the app's waveform.
    pub(crate) hit_tx: Option<rtrb::Producer<GrooveHit>>,
}

impl GrooveBox {
    pub fn new(reg: &ControlRegistry, deck: usize, kits: Arc<[Kit]>, rate: f64) -> Self {
        let c = GrooveControls::new(reg, deck);
        let pads = PadSurface::new(reg, deck);
        Self {
            c,
            clock: Clock::default(),
            kits,
            voices: Default::default(),
            playing: 0,
            queued: None,
            was_running: false,
            bar_played: false,
            last_bpm: DEFAULT_BPM,
            rate,
            hits: [Hit::default(); MAX_HITS],
            pads,
            skip: [None; ROWS],
            key: None,
            dice: Dice::new(Habits::shared(), 0x9e37_79b9 ^ ((deck as u32 + 1) * 7919)),
            fill_seed: fill_seed(deck),
            crash_next: false,
            in_fill: false,
            played: [0; ROWS],
            hit_tx: None,
        }
    }

    /// Makes the variation waiting, on groove `slot`, keeping an undo step.
    fn roll(&mut self, slot: usize) {
        let Some((mv, row)) = self.dice.pending.take() else {
            return;
        };
        self.c.dice_pending.set(0.0);
        let before = self.c.tracks(slot);
        self.dice.remember(slot, before);
        let genre = self.c.genre.get().round() as usize;
        for (r, &was) in before.iter().enumerate() {
            let chosen = match row {
                Some(only) => only == r,
                None => !self.c.lock[r].get_bool(),
            };
            if !chosen {
                continue;
            }
            let mut t = was;
            groove_dice::apply(mv, &mut t, self.c.role(r), genre, &self.dice.habits, &mut self.dice.rng);
            self.c.set_track(slot, r, t);
        }
        self.c.can_undo.set(1.0);
    }

    /// Takes a new variation or an undo from the controls. A variation
    /// waits for the next bar while playing; stopped, it happens now.
    fn take_dice(&mut self, chosen: usize) {
        let n = self.c.dice.get().round() as u8;
        if n > 0 {
            self.c.dice.set(0.0);
            if let Some(mv) = Move::from_number(n) {
                let row = (self.c.dice_row.get().round() as usize).min(ROWS);
                self.dice.pending = Some((mv, row.checked_sub(1)));
                self.c.dice_pending.set(1.0);
            }
        }
        if self.c.dice_undo.get_bool() {
            self.c.dice_undo.set(0.0);
            self.dice.pending = None;
            self.c.dice_pending.set(0.0);
            if let Some((slot, tracks)) = self.dice.take_undo() {
                for (r, t) in tracks.into_iter().enumerate() {
                    self.c.set_track(slot, r, t);
                }
            }
            self.c.can_undo.set(self.dice.can_undo() as u8 as f64);
        }
        if !self.clock.running {
            self.roll(chosen);
        }
    }

    pub fn enabled(&self) -> bool {
        self.c.enabled.get_bool()
    }

    /// Whether any sound is still ringing.
    pub fn sounding(&self) -> bool {
        self.voices.iter().flatten().any(|v| v.sample.is_some())
    }

    /// Renders one buffer into `out` (cleared first). `play` is the deck's
    /// play button and `source` where the beat comes from; `kick` hears
    /// the frame of every row 1 hit, for ducking.
    pub fn render(&mut self, play: bool, source: Source, out: &mut [[f32; 2]], mut kick: impl FnMut(usize)) {
        out.iter_mut().for_each(|s| *s = [0.0; 2]);
        let frames = out.len();
        let genre = self.c.genre.get().round() as usize;
        let (chosen, partner) = self.c.slots();
        if self.c.load.get_bool() {
            self.c.load.set(0.0);
            self.c.load_groove(chosen, genre, self.c.pattern.get().round() as usize);
        }
        if self.c.load_chain.get_bool() {
            self.c.load_chain.set(0.0);
            self.c.load_groove(partner, genre, self.c.chain.get().round() as usize);
        }
        let chained = self.c.chain.get() >= 0.5;
        if play && !self.was_running {
            self.clock.start(source);
            self.playing = chosen;
            self.queued = None;
            self.bar_played = false;
        } else if !play && self.was_running {
            self.clock.stop();
            self.skip = [None; ROWS];
        }
        self.was_running = play;
        if !play {
            self.playing = chosen;
        }
        let in_turn = self.playing == chosen || (chained && self.playing == partner);
        self.queued = (!in_turn).then_some(chosen);
        self.c.groove_queued.set(self.queued.unwrap_or(self.playing) as f64);
        self.last_bpm = source.bpm;
        let mut n = self.take_pads(chosen);
        self.take_dice(chosen);
        let mut span_now = None;
        if self.clock.running {
            let span = self.clock.advance(frames, self.rate, source);
            span_now = Some(span);
            self.c.beat.set(span.beat_at(frames));
            // A variation waiting is made just before the bar line that
            // falls in this buffer, so the new bar plays it.
            if self.dice.pending.is_some() {
                let mut bar = false;
                span.steps(frames, 0.0, |_, step, beat| {
                    bar |= step == 0 && self.clock.plays(beat);
                });
                if bar {
                    self.roll(chosen);
                }
            }
            let swing = self.c.swing.get();
            let any_solo = self.c.solo.iter().any(|s| s.get_bool());
            let mut queued = self.queued;
            let mut playing = self.playing;
            let mut bar_played = self.bar_played;
            let mut last_step = None;
            let clock = &self.clock;
            let c = &self.c;
            let hits = &mut self.hits;
            let skip = &mut self.skip;
            let rng = &mut self.dice.rng;
            let habits = &*self.dice.habits;
            let vary = c.vary.get() as f32;
            let auto_fill = c.auto_fill.get().round() as u8;
            let fill_held = c.fill.get_bool();
            let remix = groove_fills::remix_level(c.remix.get());
            let seed = self.fill_seed;
            let crash_next = &mut self.crash_next;
            let in_fill = &mut self.in_fill;
            span.steps(frames, swing, |frame, step, beat| {
                if step == 0 {
                    if let Some(q) = queued.take() {
                        playing = q;
                    } else if chained && bar_played {
                        // The chain's two grooves take turns, a bar each.
                        playing = if playing == chosen { partner } else { chosen };
                    }
                }
                if !clock.plays(beat) {
                    return;
                }
                bar_played = true;
                last_step = Some(step);
                // What each track plays on this step as written (0 rests).
                let mut normal = [0f32; ROWS];
                let mut written = [0u32; ROWS];
                for (row, skip) in skip.iter_mut().enumerate() {
                    let bits = c.patterns[playing][row].get() as u32;
                    written[row] = bits;
                    let role = c.role(row);
                    let mut velocity = if bits & (1 << step) == 0 {
                        // Vary brings in ghost notes on the steps the genre likes.
                        if bits == 0 || !groove_dice::ghost(vary, role, step, genre, habits, rng) {
                            continue;
                        }
                        Level::Soft.gain() * 0.7
                    } else {
                        if *skip == Some(step) {
                            *skip = None;
                            continue;
                        }
                        let maybe = c.maybe[playing][row].get() as u32;
                        if maybe & (1 << step) != 0 && rng.unit() >= 0.5 {
                            continue;
                        }
                        if !groove_dice::keeps(vary, role, step, genre, habits, rng) {
                            continue;
                        }
                        let soft = c.soft[playing][row].get() as u32;
                        let accent = c.accent[playing][row].get() as u32;
                        Level::of(step, soft, accent).gain()
                    };
                    if vary > 0.0 {
                        velocity = groove_dice::humanize(velocity, vary, rng);
                    }
                    normal[row] = velocity;
                }
                // A fill (held, or Auto fill's) or Remix takes over the step.
                let bar = (beat / 4.0 + 1e-6).floor() as i64;
                let fill = if fill_held {
                    Some(groove_fills::held_fill(bar, seed))
                } else {
                    groove_fills::auto_fill(auto_fill, bar, seed).filter(|f| step >= f.from)
                };
                let mut play = normal;
                if remix > 0 {
                    play = groove_fills::remix(remix, step, normal, written);
                } else if let Some(f) = fill {
                    play = f.hits(step);
                    if f.steps() >= 8 && step == STEPS - 1 {
                        *crash_next = true;
                    }
                } else if step == 0 && std::mem::take(crash_next) {
                    // The crash after a fill, on the downbeat.
                    play[ROWS - 1] = play[ROWS - 1].max(1.0);
                }
                *in_fill = fill.is_some() || remix > 0;
                for (row, &velocity) in play.iter().enumerate() {
                    if velocity <= 0.0 {
                        continue;
                    }
                    let audible = if any_solo { c.solo[row].get_bool() } else { !c.mute[row].get_bool() };
                    if audible && n < MAX_HITS {
                        hits[n] = Hit { frame, row, velocity, fill: fill.is_some() || remix > 0 };
                        n += 1;
                    }
                }
            });
            self.playing = playing;
            self.queued = queued;
            self.bar_played = bar_played;
            self.c.fill_playing.set(self.in_fill as u8 as f64);
            // The step under the playhead, for the screen and LEDs.
            let now = span.beat_at(frames / 2);
            let step = ((now * 4.0).floor() as i64).rem_euclid(STEPS as i64);
            self.c.position.set(if self.clock.plays(now) { step as f64 } else { last_step.map_or(-1.0, |s| s as f64) });
        } else {
            self.c.position.set(-1.0);
            self.c.fill_playing.set(0.0);
            self.crash_next = false;
        }
        self.c.groove_playing.set(self.playing as f64);
        self.show_pads(chosen);
        let kit = (self.c.kit.get().round() as usize).min(self.kits.len().saturating_sub(1));
        for row in 0..ROWS {
            if self.c.preview[row].get_bool() {
                self.c.preview[row].set(0.0);
                self.trigger(kit, row, 1.0);
            }
        }
        let mut next_hit = 0;
        let mut peak = [0f32; ROWS];
        self.played = [0; ROWS];
        for h in &self.hits[..n] {
            self.played[h.row] += 1;
        }
        for (f, frame) in out.iter_mut().enumerate() {
            while next_hit < n && self.hits[next_hit].frame == f {
                let Hit { row, velocity, fill, .. } = self.hits[next_hit];
                next_hit += 1;
                if let (Some(tx), Some(span)) = (self.hit_tx.as_mut(), span_now) {
                    // A full queue means the app isn't reading: dropped.
                    let _ = tx.push(GrooveHit { beat: span.beat_at(f), row: row as u8, velocity, fill });
                }
                if row == 0 {
                    kick(f);
                }
                self.trigger(kit, row, velocity);
            }
            if !self.sounding_fast() {
                continue;
            }
            for (row, voices) in self.voices.iter_mut().enumerate() {
                for v in voices.iter_mut() {
                    if v.sample.is_some() {
                        let s = v.next();
                        frame[0] += s[0];
                        frame[1] += s[1];
                        peak[row] = peak[row].max(s[0].abs()).max(s[1].abs());
                    }
                }
            }
        }
        // Meters rise at once and fall over about a tenth of a second.
        let fall = (-(frames as f64) / (0.1 * self.rate)).exp();
        for (m, p) in self.c.meter.iter().zip(peak) {
            let v = (p as f64).min(1.0).max(m.get() * fall);
            m.set(if v < 1e-3 { 0.0 } else { v });
        }
    }

    #[inline]
    fn sounding_fast(&self) -> bool {
        self.voices.iter().flatten().any(|v| v.sample.is_some())
    }

    /// Acts on the pads pressed since the last buffer (see
    /// [`crate::groove_pads`]); `chosen` is the groove on screen. Sounds
    /// to play now go into the first hits; returns how many.
    fn take_pads(&mut self, chosen: usize) -> usize {
        let row = self.pads.row();
        if self.pads.take_clear() {
            self.c.patterns[chosen][row].set(0.0);
            self.c.soft[chosen][row].set(0.0);
            self.c.maybe[chosen][row].set(0.0);
            self.c.accent[chosen][row].set(0.0);
        }
        let mode = self.pads.mode();
        let mut n = 0;
        let mut presses = [None; groove_pads::PADS];
        let mut count = 0;
        self.pads.presses(|p| {
            presses[count] = Some(p);
            count += 1;
        });
        for p in presses.into_iter().flatten() {
            let k = p.pad;
            match mode {
                mode::PLAY => {
                    let row = groove_pads::play_row(k);
                    if n < MAX_HITS {
                        self.hits[n] =
                            Hit { frame: 0, row, velocity: 0.25 + 0.75 * p.velocity.clamp(0.0, 1.0), fill: false };
                        n += 1;
                    }
                    if self.pads.c.record.get_bool() && self.clock.running {
                        let beat = self.clock.beat;
                        let step = groove_pads::nearest_step(beat);
                        let now = ((beat * 4.0).floor() as i64).rem_euclid(STEPS as i64) as usize;
                        let cell = &self.c.patterns[self.playing][row];
                        let bits = cell.get() as u32;
                        cell.set((bits | (1 << step)) as f64);
                        if step != now {
                            self.skip[row] = Some(step);
                        }
                    }
                }
                mode::GROOVES => {
                    if k < GROOVES {
                        self.c.groove.set(k as f64);
                    } else if k < 4 + kits::KITS.len().min(4) {
                        self.c.kit.set((k - 4) as f64);
                    } else if k >= 8 && k - 8 < self.genre_size() {
                        // The genre's grooves, into the groove playing.
                        let genre = self.c.genre.get().round().max(0.0) as usize;
                        self.c.pattern.set((k - 7) as f64);
                        self.c.load_groove(self.playing, genre, k - 7);
                    }
                }
                _ => {
                    let cell = &self.c.patterns[chosen][row];
                    let bits = cell.get() as u32 ^ (1 << k);
                    cell.set(bits as f64);
                    // Stopped, a new hit is heard, as a click on the screen.
                    if !self.clock.running && bits & (1 << k) != 0 && n < MAX_HITS {
                        self.hits[n] = Hit { frame: 0, row, velocity: 1.0, fill: false };
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// How many grooves the genre picked has (up to the 8 pads for them).
    fn genre_size(&self) -> usize {
        let genre = self.c.genre.get().round().max(0.0) as usize;
        patterns::library().get(genre).map_or(0, |g| g.grooves.len().min(8))
    }

    /// Lights the pads for what they do now.
    fn show_pads(&self, chosen: usize) {
        let row = self.pads.row();
        let position = self.c.position.get();
        let playhead = (self.clock.running && self.playing == chosen && position >= 0.0).then_some(position as usize);
        let kit = (self.c.kit.get().round() as usize).min(self.kits.len().saturating_sub(1));
        let colors = groove_pads::colors(&groove_pads::View {
            mode: self.pads.mode(),
            row,
            steps: self.c.patterns[chosen][row].get() as u32,
            playhead,
            held: &|k| self.pads.held(k),
            muted: &|r| self.c.mute[r].get_bool(),
            playing: self.playing,
            queued: self.queued,
            kit,
            grooves: self.genre_size(),
        });
        self.pads.show(&colors);
    }

    /// Plays `row`'s sound from the start, fading out what it played;
    /// `velocity` scales the row's level (a soft step 0.5, an accented one
    /// 1.25, a pad 0.25-1).
    fn trigger(&mut self, kit: usize, row: usize, velocity: f32) {
        let Some(kit) = self.kits.get(kit) else {
            return;
        };
        let sound = (self.c.sound[row].get().round() as usize).min(kit.sounds.len() - 1);
        let sample = kit.sounds[sound].sample.clone();
        let decay = self.c.decay[row].get().clamp(0.0, 1.0);
        // Full decay: the sound as it is; less: an envelope from 20 ms up.
        let fall = if decay >= 0.999 {
            1.0
        } else {
            let tau = 0.02 + decay * decay * 1.2;
            (-1.0 / (tau * self.rate)).exp() as f32
        };
        let [now, tail] = &mut self.voices[row];
        if now.sample.is_some() {
            std::mem::swap(now, tail);
            tail.fade = CHOKE_FRAMES;
        }
        let mut semitones = self.c.pitch[row].get();
        if row == 0 {
            let shift = kick_shift(kit.sounds[sound].pitch, self.key, self.c.kick_key.get().round() as u8);
            self.c.kick_note.set(shift.map_or(0.0, |(_, note)| note as f64 + 1.0));
            semitones += shift.map_or(0.0, |(s, _)| s as f64);
        }
        let level = self.c.level[row].get() as f32;
        let pan = self.c.pan[row].get().clamp(-1.0, 1.0) as f32;
        // Equal power, so a sound panned to one side is as loud as centred.
        let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let (tone, high) = tone_filter(self.c.tone[row].get(), self.rate);
        *now = Voice {
            sample: Some(sample),
            pos: 0.0,
            rate: 2f64.powf(semitones / 12.0),
            gain: [
                level * velocity * angle.cos() * std::f32::consts::SQRT_2,
                level * velocity * angle.sin() * std::f32::consts::SQRT_2,
            ],
            tone,
            high,
            z: [0.0; 2],
            env: 1.0,
            fall,
            fade: 0,
        };
    }
}

/// How far to tune a kick that rings at `hz` so it plays the root (`choice` 1) or fifth (2) of
/// the key whose tonic is `key`: the nearest such note, never more than half an octave away,
/// as (semitones, note 0-11 from C). None when off, keyless or the kick has no clear pitch.
pub fn kick_shift(hz: f32, key: Option<u8>, choice: u8) -> Option<(i32, u8)> {
    let key = key?;
    if hz <= 0.0 || !(1..=2).contains(&choice) {
        return None;
    }
    let target = (key as i32 + if choice == 2 { 7 } else { 0 }).rem_euclid(12);
    let midi = 69.0 + 12.0 * (hz as f64 / 440.0).log2();
    let natural = midi.round() as i32;
    // -6..=5 semitones to the target's pitch class.
    let shift = (target - natural + 6).rem_euclid(12) - 6;
    Some((shift, target as u8))
}

/// The tone knob's filter: centre off, left a low-pass from 20 kHz down to
/// 200 Hz, right a high-pass from 20 Hz up to 4 kHz. Returns the one-pole
/// coefficient (0 = off) and whether it is a high-pass.
fn tone_filter(tone: f64, rate: f64) -> (f32, bool) {
    let t = tone.clamp(-1.0, 1.0);
    if t.abs() < 0.02 {
        return (0.0, false);
    }
    let cutoff = if t < 0.0 { 20_000.0 * (0.01f64).powf(-t) } else { 20.0 * 200f64.powf(t) };
    let a = 1.0 - (-std::f64::consts::TAU * cutoff.min(rate * 0.45) / rate).exp();
    (a as f32, t > 0.0)
}

/// The groove's source: the leading deck's tempo and beat while it plays,
/// else Link's, else the groove's own tempo.
pub fn pick_source(leader: Option<(f64, f64)>, link: Option<(f64, f64)>, own_bpm: f64) -> Source {
    match leader.or(link) {
        Some((bpm, beat)) if bpm > 0.0 => Source { bpm, beat: Some(beat) },
        _ => Source { bpm: own_bpm, beat: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn own(bpm: f64) -> Source {
        Source { bpm, beat: None }
    }

    /// The frames (counted from the start) on which each step starts, over
    /// `total` frames rendered in buffers of `buffer`.
    fn step_frames(buffer: usize, total: usize, swing: f64, source: impl Fn(f64) -> Source) -> Vec<(usize, usize)> {
        let mut clock = Clock::default();
        clock.start(source(0.0));
        let mut out = Vec::new();
        let mut done = 0;
        while done < total {
            let n = buffer.min(total - done);
            let span = clock.advance(n, RATE, source(done as f64));
            span.steps(n, swing, |f, step, beat| {
                if clock.plays(beat) {
                    out.push((done + f, step));
                }
            });
            done += n;
        }
        out
    }

    #[test]
    fn steps_land_on_the_same_frames_whatever_the_buffer_size() {
        let a = step_frames(64, 96_000, 0.0, |_| own(120.0));
        let b = step_frames(333, 96_000, 0.0, |_| own(120.0));
        let c = step_frames(2048, 96_000, 0.0, |_| own(120.0));
        assert_eq!(a, b);
        assert_eq!(a, c);
        // 120 BPM: a 16th is 6000 frames at 48 kHz; two seconds hold 16.
        assert_eq!(a.len(), 16);
        for (i, &(f, step)) in a.iter().enumerate() {
            assert_eq!(step, i);
            assert!((f as i64 - i as i64 * 6000).abs() <= 1, "step {i} at {f}");
        }
    }

    #[test]
    fn swing_delays_every_second_step() {
        let s = step_frames(512, 24_000, 0.5, |_| own(120.0));
        // Half of a 16th's quarter: 0.5 × 0.25 beats late = 1/8 beat = 3000 frames.
        assert_eq!(s[0], (0, 0));
        assert!((s[1].0 as i64 - 9000).abs() <= 1, "{:?}", s);
        assert!((s[2].0 as i64 - 12000).abs() <= 1, "{:?}", s);
    }

    #[test]
    fn follows_the_leader_and_starts_on_its_next_downbeat() {
        // The leader plays at 124 BPM and is on beat 2.5 when PLAY is pressed.
        let bpm = 124.0;
        let leader = |frame: f64| Source { bpm, beat: Some(2.5 + frame * bpm / 60.0 / RATE) };
        let s = step_frames(256, 96_000, 0.0, leader);
        // The first step heard is step 0, on the leader's beat 4.
        assert_eq!(s[0].1, 0);
        let beat4 = (1.5 * 60.0 / bpm * RATE) as i64;
        assert!((s[0].0 as i64 - beat4).abs() <= 1, "{} vs {beat4}", s[0].0);
        // Every later step stays on the leader's 16ths.
        for &(f, step) in &s {
            let beat = 2.5 + f as f64 * bpm / 60.0 / RATE;
            let off = (beat * 4.0 - (beat * 4.0).round()).abs();
            assert!(off < 0.002, "step {step} at beat {beat}");
        }
    }

    #[test]
    fn stays_on_the_leader_through_a_tempo_ramp() {
        // The leader speeds up from 120 to 130 BPM over 4 seconds.
        let beat_at = |t: f64| 120.0 / 60.0 * t + 10.0 / 60.0 * t * t / 8.0;
        let bpm_at = |t: f64| 120.0 + 10.0 * t / 4.0;
        let leader = |frame: f64| {
            let t = frame / RATE;
            Source { bpm: bpm_at(t), beat: Some(beat_at(t)) }
        };
        let s = step_frames(512, 4 * 48_000, 0.0, leader);
        assert!(s.len() > 30, "{}", s.len());
        for &(f, step) in s.iter().skip(4) {
            let beat = beat_at(f as f64 / RATE);
            // Within about a frame's worth of beat (well under a millisecond).
            let off = (beat * 4.0 - (beat * 4.0).round()).abs() / 4.0;
            assert!(off < 0.02, "step {step} off by {off} beats");
        }
    }

    #[test]
    fn a_jump_on_the_leader_moves_the_groove_at_the_next_bar() {
        // The leader plays at 120 BPM; half a second (one beat) in, it jumps
        // back by a beat and a half (a hot cue).
        let leader = |frame: f64| {
            let beat = frame * 2.0 / RATE;
            Source { bpm: 120.0, beat: Some(if frame >= 24_000.0 { beat - 1.5 } else { beat }) }
        };
        let s = step_frames(480, 144_000, 0.0, leader);
        // Through the first bar the steps run on: 0..15 every 6000 frames.
        for (i, &(f, step)) in s.iter().take(16).enumerate() {
            assert_eq!(step, i, "{:?}", &s[..18]);
            assert!((f as i64 - i as i64 * 6000).abs() <= 1);
        }
        // From the bar line on, the groove plays the leader's beats.
        assert!(s.len() > 20);
        for &(f, step) in s.iter().skip(16) {
            let beat = f as f64 * 2.0 / RATE - 1.5;
            let expected = ((beat * 4.0).round() as i64).rem_euclid(16) as usize;
            assert_eq!(step, expected, "frame {f}");
        }
    }

    #[test]
    fn own_tempo_without_a_leader_or_link() {
        assert_eq!(pick_source(None, None, 100.0), own(100.0));
        let link = Some((100.0, 7.0));
        assert_eq!(pick_source(None, link, 120.0).beat, Some(7.0));
        let leader = Some((124.0, 3.0));
        assert_eq!(pick_source(leader, link, 120.0).bpm, 124.0);
    }

    fn groove_box() -> (Arc<ControlRegistry>, GrooveBox) {
        let reg = Arc::new(ControlRegistry::default());
        let kits: Arc<[Kit]> = kits::build(RATE).into();
        let g = GrooveBox::new(&reg, 0, kits, RATE);
        (reg, g)
    }

    #[test]
    fn plays_the_kick_on_the_beat_and_mutes() {
        let (_reg, mut g) = groove_box();
        // One kit, only the kick, four to the bar.
        g.c.kit.set(1.0);
        for c in &g.c.patterns[0][1..] {
            c.set(0.0);
        }
        g.c.patterns[0][0].set(kits::parse_steps("x...x...x...x...") as f64);
        let mut out = vec![[0.0; 2]; 6000 * 4];
        let mut kicks = Vec::new();
        g.render(true, own(120.0), &mut out, |f| kicks.push(f));
        assert_eq!(kicks, vec![0]);
        assert!(out[..200].iter().any(|s| s[0].abs() > 0.05), "the kick sounds");
        // Muted, the next beat is silent once the first kick has died away.
        g.c.mute[0].set_bool(true);
        let mut out = vec![[0.0; 2]; 6000 * 4 * 12];
        let mut kicks = Vec::new();
        g.render(true, own(120.0), &mut out, |f| kicks.push(f));
        assert!(kicks.is_empty());
        assert!(out[out.len() - 100..].iter().all(|s| s[0].abs() < 1e-3));
    }

    /// Renders one bar of only `row`'s first step and returns the output.
    fn one_hit(g: &mut GrooveBox, row: usize) -> Vec<[f32; 2]> {
        for pattern in &g.c.patterns {
            for (r, c) in pattern.iter().enumerate() {
                c.set(if r == row { 1.0 } else { 0.0 });
            }
        }
        let mut out = vec![[0.0; 2]; 6000];
        g.render(true, own(120.0), &mut out, |_| {});
        out
    }

    fn energy(out: &[[f32; 2]], side: usize) -> f32 {
        out.iter().map(|s| s[side] * s[side]).sum()
    }

    #[test]
    fn eight_tracks_each_play_their_sound() {
        for row in 0..ROWS {
            let (_reg, mut g) = groove_box();
            let out = one_hit(&mut g, row);
            assert!(energy(&out, 0) > 1e-3, "track {} is silent", row + 1);
            assert!(g.c.meter[row].get() > 0.0, "track {} meter", row + 1);
            for other in (0..ROWS).filter(|&r| r != row) {
                assert_eq!(g.c.meter[other].get(), 0.0);
            }
        }
    }

    #[test]
    fn pan_moves_a_track_to_one_side() {
        let (_reg, mut g) = groove_box();
        g.c.pan[1].set(-1.0);
        let out = one_hit(&mut g, 1);
        assert!(energy(&out, 0) > 1e-3);
        assert!(energy(&out, 1) < energy(&out, 0) * 1e-6, "right side silent");
    }

    #[test]
    fn tone_darkens_or_thins_a_sound() {
        // The hat through the low-pass loses most of its energy; through a
        // full high-pass the kick does.
        let (_reg, mut g) = groove_box();
        let open = energy(&one_hit(&mut g, 2), 0);
        let (_reg, mut g) = groove_box();
        g.c.tone[2].set(-1.0);
        let dark = energy(&one_hit(&mut g, 2), 0);
        assert!(dark < open * 0.5, "{dark} vs {open}");
        let (_reg, mut g) = groove_box();
        let kick = energy(&one_hit(&mut g, 0), 0);
        let (_reg, mut g) = groove_box();
        g.c.tone[0].set(1.0);
        let thin = energy(&one_hit(&mut g, 0), 0);
        assert!(thin < kick * 0.5, "{thin} vs {kick}");
    }

    #[test]
    fn a_new_groove_waits_for_the_bar() {
        let (_reg, mut g) = groove_box();
        g.c.groove.set(0.0);
        let mut out = vec![[0.0; 2]; 6000];
        g.render(true, own(120.0), &mut out, |_| {});
        g.c.groove.set(1.0);
        g.render(true, own(120.0), &mut out, |_| {});
        assert_eq!(g.c.groove_queued.get(), 1.0);
        assert_eq!(g.playing, 0, "still the first groove mid-bar");
        // Render through to the bar line (16 steps of 6000 frames).
        for _ in 0..15 {
            g.render(true, own(120.0), &mut out, |_| {});
        }
        assert_eq!(g.playing, 1);
    }

    #[test]
    fn a_variation_waits_for_the_bar_and_undoes() {
        let (_reg, mut g) = groove_box();
        let before = g.c.tracks(0);
        let mut out = vec![[0.0; 2]; 6000];
        g.render(true, own(120.0), &mut out, |_| {});
        g.render(true, own(120.0), &mut out, |_| {});
        g.c.dice.set(1.0);
        g.render(true, own(120.0), &mut out, |_| {});
        assert_eq!(g.c.dice_pending.get(), 1.0);
        assert_eq!(g.c.tracks(0), before, "nothing changes mid-bar");
        for _ in 0..14 {
            g.render(true, own(120.0), &mut out, |_| {});
        }
        assert_eq!(g.c.dice_pending.get(), 0.0);
        assert_ne!(g.c.tracks(0), before, "a new groove from the bar");
        assert_eq!(g.c.can_undo.get(), 1.0);
        g.c.dice_undo.set(1.0);
        g.render(true, own(120.0), &mut out, |_| {});
        assert_eq!(g.c.tracks(0), before);
        assert_eq!(g.c.can_undo.get(), 0.0);
    }

    #[test]
    fn a_maybe_step_plays_about_half_the_bars() {
        let (_reg, mut g) = groove_box();
        for c in &g.c.patterns[0] {
            c.set(0.0);
        }
        g.c.patterns[0][0].set(1.0);
        g.c.maybe[0][0].set(1.0);
        let mut kicks = 0;
        let mut out = vec![[0.0; 2]; 6000];
        for _ in 0..16 * 40 {
            g.render(true, own(120.0), &mut out, |_| kicks += 1);
        }
        assert!((10..=30).contains(&kicks), "{kicks} kicks in 40 bars");
    }

    /// Plays `bars` bars of 120 BPM and counts each track's hits per step
    /// of each bar.
    fn count_hits(g: &mut GrooveBox, bars: usize) -> Vec<[[u32; ROWS]; STEPS]> {
        let mut out = vec![[0.0; 2]; 6000];
        let mut counts = vec![[[0u32; ROWS]; STEPS]; bars];
        for bar in counts.iter_mut() {
            for step in bar.iter_mut() {
                g.render(true, own(120.0), &mut out, |_| {});
                // Each buffer is one step.
                *step = g.played;
            }
        }
        counts
    }

    #[test]
    fn auto_fill_changes_the_last_beat_of_the_fourth_bar_and_crashes_after_the_sixteenth() {
        let (_reg, mut plain) = groove_box();
        let (_reg2, mut filled) = groove_box();
        filled.c.auto_fill.set(1.0);
        let a = count_hits(&mut plain, 17);
        let b = count_hits(&mut filled, 17);
        assert_eq!(a[0], b[0], "bar 1 as written");
        assert_eq!(a[3][..12], b[3][..12], "bar 4 as written until its last beat");
        assert_ne!(a[3][12..], b[3][12..], "then a fill");
        assert_ne!(a[15][..8], b[15][..8], "bar 16 is all fill");
        assert!(b[16][0][7] > 0 && a[16][0][7] == 0, "a crash on the next downbeat");
    }

    #[test]
    fn remix_plays_while_held_and_stops_when_let_go() {
        let (_reg, mut g) = groove_box();
        let plain = count_hits(&mut g, 1);
        g.c.remix.set(1.0);
        let rolled = count_hits(&mut g, 1);
        assert!(rolled[0].iter().all(|s| s[0] > 0), "16th kicks at the top figure");
        assert_eq!(g.c.fill_playing.get(), 1.0);
        g.c.remix.set(0.0);
        assert_eq!(count_hits(&mut g, 1), plain);
    }

    #[test]
    fn a_locked_track_keeps_its_steps_and_one_track_can_change_alone() {
        let (_reg, mut g) = groove_box();
        let before = g.c.tracks(0);
        let mut out = vec![[0.0; 2]; 64];
        g.c.lock[0].set_bool(true);
        g.c.dice.set(4.0);
        g.render(false, own(120.0), &mut out, |_| {});
        let after = g.c.tracks(0);
        assert_eq!(after[0], before[0], "the locked kick");
        assert!(after[2].hits.count_ones() < before[2].hits.count_ones(), "calmer hats");
        g.c.dice_row.set(1.0);
        g.c.dice.set(3.0);
        g.render(false, own(120.0), &mut out, |_| {});
        let only = g.c.tracks(0);
        assert_eq!(only[0].hits.count_ones(), before[0].hits.count_ones() + 1);
        assert_eq!(only[2], after[2]);
    }

    #[test]
    fn pads_add_steps_to_the_selected_row() {
        let (_reg, mut g) = groove_box();
        g.c.patterns[0][1].set(0.0);
        g.pads.c.row.set(2.0);
        let mut out = vec![[0.0; 2]; 256];
        g.pads.c.pads[3].set(1.0);
        g.render(false, own(120.0), &mut out, |_| {});
        assert_eq!(g.c.patterns[0][1].get() as u32, 1 << 3);
        assert_eq!(g.pads.c.colors[3].get() as u8, groove_pads::light::ROW + 1);
        assert!(out.iter().any(|s| s[0].abs() > 0.01), "heard while stopped");
        // Pressed again: the hit goes.
        g.pads.c.pads[3].set(0.0);
        g.render(false, own(120.0), &mut out, |_| {});
        g.pads.c.pads[3].set(0.5);
        g.render(false, own(120.0), &mut out, |_| {});
        assert_eq!(g.c.patterns[0][1].get() as u32, 0);
        // ROW CLEAR empties the row.
        g.c.patterns[0][1].set(0xffff as f64);
        g.pads.c.row_clear.set(1.0);
        g.render(false, own(120.0), &mut out, |_| {});
        assert_eq!(g.c.patterns[0][1].get() as u32, 0);
    }

    #[test]
    fn play_mode_records_into_the_nearest_step_once() {
        let (_reg, mut g) = groove_box();
        for c in &g.c.patterns[0] {
            c.set(0.0);
        }
        g.pads.c.mode.set(mode::PLAY as f64);
        g.pads.c.record.set_bool(true);
        // 120 BPM: a 16th is 6000 frames. Play into step 2, then press the
        // pad 9 (row 1 again) a little before step 3.
        let mut out = vec![[0.0; 2]; 6000];
        g.render(true, own(120.0), &mut out, |_| {});
        g.render(true, own(120.0), &mut out, |_| {});
        let mut out = vec![[0.0; 2]; 5000];
        g.render(true, own(120.0), &mut out, |_| {});
        g.pads.c.pads[8].set(0.8);
        let mut kicks = Vec::new();
        let mut out = vec![[0.0; 2]; 2000];
        g.render(true, own(120.0), &mut out, |f| kicks.push(f));
        assert_eq!(g.c.patterns[0][0].get() as u32, 1 << 3);
        assert_eq!(kicks, vec![0], "heard now, not again on step 3");
        // Next time round, step 3 plays.
        let mut kicks = Vec::new();
        let mut out = vec![[0.0; 2]; 6000 * 16];
        g.render(true, own(120.0), &mut out, |f| kicks.push(f));
        assert_eq!(kicks.len(), 1);
    }

    #[test]
    fn grooves_mode_picks_grooves_kits_and_library_grooves() {
        let (_reg, mut g) = groove_box();
        g.pads.c.mode.set(mode::GROOVES as f64);
        let mut out = vec![[0.0; 2]; 256];
        let press = |g: &mut GrooveBox, k: usize, out: &mut Vec<[f32; 2]>| {
            g.pads.c.pads[k].set(1.0);
            g.render(false, own(120.0), out, |_| {});
            g.pads.c.pads[k].set(0.0);
            g.render(false, own(120.0), out, |_| {});
        };
        press(&mut g, 2, &mut out);
        assert_eq!(g.c.groove.get(), 2.0);
        press(&mut g, 5, &mut out);
        assert_eq!(g.c.kit.get(), 1.0);
        // Pad 11: the genre's third groove, into the groove playing.
        press(&mut g, 10, &mut out);
        let third = patterns::groove(0, 2).unwrap();
        assert_eq!(g.c.patterns[g.playing][0].get() as u32, third.rows[0].hits);
        assert_eq!(g.c.pattern.get(), 3.0);
        assert_eq!(g.pads.c.colors[2].get() as u8, groove_pads::light::SELECTED);
    }

    #[test]
    fn grooves_load_from_the_library() {
        let (_reg, g) = groove_box();
        let first = patterns::groove(0, 0).unwrap();
        assert_eq!(g.c.patterns[0][0].get() as u32, first.rows[0].hits);
        g.c.groove.set(2.0);
        g.c.load_groove(2, 0, 3);
        let third = patterns::groove(0, 2).unwrap();
        assert_eq!(g.c.patterns[2][1].get() as u32, third.rows[1].hits);
        assert_eq!(g.c.slots(), (2, 3));
    }

    #[test]
    fn a_chain_takes_turns_a_bar_each() {
        let (_reg, mut g) = groove_box();
        g.c.chain.set(2.0);
        g.c.load_chain.set(1.0);
        let mut out = vec![[0.0; 2]; 6000];
        let mut heard = Vec::new();
        // Three bars of 16 steps at 120 BPM (6000 frames a step at 48 kHz).
        for _ in 0..48 {
            g.render(true, own(120.0), &mut out, |_| {});
            heard.push(g.c.groove_playing.get() as usize);
        }
        assert_eq!(&heard[..15], &[0; 15], "A plays the first bar");
        assert!(heard[17..31].iter().all(|&p| p == 1), "then B: {heard:?}");
        assert!(heard[33..47].iter().all(|&p| p == 0), "then A again");
        assert_ne!(g.c.patterns[1][0].get(), 0.0, "B holds the chained groove");
    }

    #[test]
    fn the_kick_tunes_to_the_nearest_root_or_fifth() {
        // 55 Hz is A1: in A it stays, in C it goes up 3, in F down 4.
        assert_eq!(kick_shift(55.0, Some(9), 1), Some((0, 9)));
        assert_eq!(kick_shift(55.0, Some(0), 1), Some((3, 0)));
        assert_eq!(kick_shift(55.0, Some(5), 1), Some((-4, 5)));
        // The fifth of D is A.
        assert_eq!(kick_shift(55.0, Some(2), 2), Some((0, 9)));
        assert_eq!(kick_shift(55.0, None, 1), None);
        assert_eq!(kick_shift(55.0, Some(0), 0), None);
        assert_eq!(kick_shift(0.0, Some(0), 1), None);
    }

    #[test]
    fn the_kick_plays_in_the_leaders_key() {
        let (_reg, mut g) = groove_box();
        g.c.kick_key.set(1.0);
        g.key = Some(0);
        g.trigger(0, 0, 1.0);
        let hz = g.kits[0].sounds[0].pitch as f64;
        let midi = 69.0 + 12.0 * (hz / 440.0).log2();
        let heard = midi + 12.0 * g.voices[0][0].rate.log2();
        assert!((heard - heard.round()).abs() < 0.5);
        assert_eq!((heard.round() as i32).rem_euclid(12), 0, "a C");
        assert_eq!(g.c.kick_note.get(), 1.0);
    }

    #[test]
    fn the_cymbal_track_plays_the_grooves_ride_or_crash() {
        let (_reg, g) = groove_box();
        let lib = patterns::library();
        let find = |role: usize| {
            lib.iter().enumerate().find_map(|(gi, genre)| {
                genre
                    .grooves
                    .iter()
                    .position(|p| p.rows[role].hits != 0)
                    .map(|i| (gi, i + 1, genre.grooves[i].rows[role].hits))
            })
        };
        let (genre, index, hits) = find(RIDE).expect("a groove with a ride");
        g.c.load_groove(0, genre, index);
        assert_eq!(g.c.sound[7].get(), RIDE as f64);
        assert_eq!(g.c.patterns[0][7].get(), hits as f64);
        let (genre, index, hits) = find(CRASH).expect("a groove with a crash");
        g.c.load_groove(0, genre, index);
        assert_eq!(g.c.sound[7].get(), CRASH as f64);
        assert_eq!(g.c.patterns[0][7].get(), hits as f64);
    }

    #[test]
    fn no_library_groove_needs_more_than_eight_tracks() {
        for genre in patterns::library() {
            for p in &genre.grooves {
                assert!(p.rows[CRASH].hits == 0 || p.rows[RIDE].hits == 0, "{} has both a crash and a ride", p.name);
            }
        }
    }

    #[test]
    fn soft_steps_play_quieter() {
        let (_reg, mut g) = groove_box();
        for c in &g.c.patterns[0] {
            c.set(0.0);
        }
        g.c.patterns[0][0].set(1.0);
        let mut loud = vec![[0.0; 2]; 2000];
        g.render(true, own(120.0), &mut loud, |_| {});
        let (_reg, mut g) = groove_box();
        for c in &g.c.patterns[0] {
            c.set(0.0);
        }
        g.c.patterns[0][0].set(1.0);
        g.c.soft[0][0].set(1.0);
        let mut soft = vec![[0.0; 2]; 2000];
        g.render(true, own(120.0), &mut soft, |_| {});
        let peak = |v: &[[f32; 2]]| v.iter().fold(0.0f32, |m, s| m.max(s[0].abs()));
        assert!((peak(&soft) / peak(&loud) - 0.5).abs() < 0.01);
    }
}
