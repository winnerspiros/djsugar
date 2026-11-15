//! Automix: plays the Automix list on two decks, crossfading from one track
//! into the next, like Mixxx's Auto DJ.
//!
//! Everything goes through the same controls a user or controller moves
//! (crossfader, volume, EQ, filter, play), so the mix shows on screen.
//! While Automix is on it holds the crossfader locked: it still slides on
//! screen, but a mouse, controller or script can't move it and break into a
//! mix. Switching Automix off hands it back. Mixxx's
//! `[AutoDJ]` controls are created too, so controller mappings that start
//! Auto DJ, fade now or skip the next track work.
//!
//! Times are measured on the outgoing track, in its own seconds, so a mix
//! follows the deck's tempo and pauses when the deck does.

use std::collections::HashMap;
use std::sync::Arc;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::{deck_group, eq_group, quick_effect_group, Track, NUM_DECKS};

use crate::automix_phrases::{self, Plan, Start};
use crate::AppWindow;

/// What Automix needs from the rest of the app: the Automix list and a way
/// to put tracks on decks.
///
/// Automix leaves a track on the list while it waits on a deck, so moving
/// tracks around or adding one at the top swaps what is waiting. The track
/// only comes off the list once it has played for [`PLAYED_AFTER`].
pub trait Host {
    /// The track at `index` on the Automix list.
    fn track_at(&self, index: usize) -> Option<Arc<Track>>;
    /// Tracks on the Automix list.
    fn queued(&self) -> usize;
    /// Takes the track off the Automix list: it has been played (or skipped).
    fn remove(&mut self, id: usize);
    /// Moves the track to the end of the Automix list: it has been played
    /// and Repeat is on.
    fn requeue(&mut self, id: usize);
    fn load(&mut self, deck: usize, track: Arc<Track>);
}

/// How long a track plays before it comes off the Automix list.
pub const PLAYED_AFTER: f64 = 5.0;

/// Automix's line while a hardware mixer does the mixing.
const NEEDS_MIXER: &str = "Automix needs DJ Sugar's mixer: set Mixer to DJ Sugar in Settings › Mixer.";

/// Mix lengths offered, in the order the panel shows them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Length {
    Seconds(f64),
    Bars(f64),
    /// From a phrase change of the outgoing track (its outro), as long as
    /// the incoming intro (see [`crate::automix_phrases`]).
    Phrases,
}

/// The first, 0 s, is no mix at all: the next track starts from its very
/// beginning the moment the playing one ends, for albums that play through
/// gaplessly.
/// Phrases comes last, so lengths saved before it keep their meaning.
pub const LENGTHS: [Length; 7] = [
    Length::Seconds(0.0),
    Length::Seconds(12.0),
    Length::Seconds(15.0),
    Length::Seconds(30.0),
    Length::Bars(16.0),
    Length::Bars(32.0),
    Length::Phrases,
];
/// The Phrases length's index in [`LENGTHS`], the default for new installs.
pub const PHRASES: usize = 6;
/// What Phrases mixes with when a track has no phrases to go by, and when
/// the tempos are too far apart to beatmatch.
const NO_PHRASES: Length = Length::Bars(16.0);
const NO_MATCH: Length = Length::Seconds(15.0);

/// How the crossfade moves, in the order the panel shows them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    /// The crossfader moves evenly across the whole mix.
    Smooth,
    /// As Smooth, with the bass of the two tracks swapped halfway.
    BassSwap,
    /// As Smooth, while the outgoing track is high-passed away.
    Filter,
    /// Switches over in one beat, where the mix would start.
    Cut,
}

/// How long Player's Next takes to cut over, in seconds: a click's worth,
/// so it doesn't pop.
const CUT_NOW: f64 = 0.03;

pub const STYLES: [Style; 4] = [Style::Smooth, Style::BassSwap, Style::Filter, Style::Cut];

/// The lengths' and styles' names, as the panel shows them.
pub const LENGTH_NAMES: [&str; 7] = [
    "0 s", "12 s", "15 s", "30 s", "16 bars", "32 bars", "Phrases",
];
pub const STYLE_NAMES: [&str; 4] = ["Smooth", "Bass swap", "Filter", "Cut"];

/// Waveform level under which a slice counts as silence: about -60 dB
/// relative to the track's peak (slices hold the square root of the level).
/// Higher than that cuts off quiet fade-outs, so a 15 s mix would start
/// well before the last 15 s the deck shows.
const SILENCE: u8 = 8;
/// Crossfader movement by someone else, beyond this, ends the slide back to
/// the centre after Automix is switched off.
const TAKEOVER: f64 = 0.02;
/// How fast Automix slides the crossfader outside a mix, in crossfader
/// units per second: from the centre to one side in a second and a half.
const GLIDE_SPEED: f64 = 1.0 / 1.5;
/// Seconds before a track's end that Automix, playing gaplessly, hands the
/// next deck to the engine, which starts it on the sample after the last.
const GAPLESS_AHEAD: f64 = 1.0;

struct DeckHandles {
    play: ControlHandle,
    play_next: ControlHandle,
    playposition: ControlHandle,
    volume: ControlHandle,
    rate: ControlHandle,
    rate_range: ControlHandle,
    rate_dir: ControlHandle,
    beatsync: ControlHandle,
    loop_enabled: ControlHandle,
    eq_low: ControlHandle,
    filter: ControlHandle,
}

impl DeckHandles {
    fn new(reg: &ControlRegistry, deck: usize) -> Self {
        let g = deck_group(deck);
        let get = |group: &str, key: &str| {
            reg.get(group, key)
                .unwrap_or_else(|| panic!("engine control {group},{key}"))
        };
        Self {
            play: get(&g, "play"),
            play_next: get(&g, "play_next"),
            playposition: get(&g, "playposition"),
            volume: get(&g, "volume"),
            rate: get(&g, "rate"),
            rate_range: get(&g, "rateRange"),
            rate_dir: get(&g, "rate_dir"),
            beatsync: get(&g, "beatsync"),
            loop_enabled: get(&g, "loop_enabled"),
            eq_low: get(&eq_group(deck), "parameter1"),
            filter: get(&quick_effect_group(deck), "super1"),
        }
    }

    fn speed(&self) -> f64 {
        crate::speed(&self.rate, &self.rate_range, &self.rate_dir)
    }
}

/// A press of a push control since the last look, even one shorter than a
/// UI frame.
struct Press {
    control: ControlHandle,
    version: u32,
}

impl Press {
    fn new(control: ControlHandle) -> Self {
        let version = control.version();
        Self { control, version }
    }

    fn pressed(&mut self) -> bool {
        let v = self.control.version();
        let changes = v.wrapping_sub(self.version);
        self.version = v;
        let pressed = changes > 0 && (self.control.get_bool() || changes >= 2);
        if self.control.get_bool() {
            // Leave it released so the next press counts again.
            self.control.set(0.0);
            self.version = self.control.version();
        }
        pressed
    }
}

/// A track Automix started, still on the list until it has played a while.
struct Played {
    id: usize,
    deck: usize,
    /// Seconds it has played so far.
    seconds: f64,
}

/// The next track, put on the silent deck and cued to where its sound starts.
#[derive(Clone, Copy)]
struct Next {
    id: usize,
    cued: bool,
}

struct Mix {
    from: usize,
    to: usize,
    style: Style,
    /// The mix's span on the outgoing track, in its seconds.
    start: f64,
    end: f64,
    /// Where the crossfader was when the mix started.
    xfade_from: f64,
    /// EQ low and filter of both decks before the mix, put back after it.
    saved_low: [f64; NUM_DECKS],
    saved_filter: [f64; NUM_DECKS],
    /// Where the incoming track was when the mix started, in its seconds.
    to_start: f64,
    /// Whether the incoming deck still needs starting (a takeover mixes
    /// into a deck that is already playing).
    start_incoming: bool,
    /// No mix: the engine starts the incoming deck on the sample after the
    /// outgoing track's last.
    gapless: bool,
    beat_match: bool,
    /// How far through the mix it has got, from 0 to 1.
    progress: f64,
    /// Where through the mix Bass swap swaps, from 0 to 1: halfway, or on
    /// a phrase change of the outgoing track.
    swap: f64,
}

enum Phase {
    Off,
    /// Nothing was playing: the first track is going on this deck.
    Opening {
        deck: usize,
        next: Option<Next>,
    },
    /// `live` plays; the next track waits on the other deck.
    Playing {
        live: usize,
        next: Option<Next>,
    },
    Mixing(Mix),
}

/// The crossfader sliding to `target` by itself, outside a mix: to the live
/// deck when Automix starts, to the centre when it is switched off.
struct Glide {
    target: f64,
    /// What the glide last set the crossfader to, to notice someone else moving it.
    set: f64,
}

pub struct AutoMix {
    decks: [DeckHandles; NUM_DECKS],
    crossfader: ControlHandle,
    enabled: ControlHandle,
    /// `[Master] external_mixer`: a hardware mixer does the mixing, so
    /// Automix, which needs DJ Sugar's crossfader, stays off.
    external: ControlHandle,
    fade_now: Press,
    /// The press of fade_now is Player's Next: cut straight over.
    cut_now: bool,
    skip_next: Press,
    phase: Phase,
    glide: Option<Glide>,
    length: usize,
    style: usize,
    /// Playing an album (Play Album): gapless until Automix stops.
    album: bool,
    /// A note for the panel when Automix stops by itself or can't start.
    note: String,
    /// The track Automix last started, until it comes off the list.
    played: Option<Played>,
    /// Tracks on the list still to come, for the panel.
    left: usize,
    /// Played tracks go back to the end of the list instead of coming off.
    repeat: bool,
    /// The Phrases length's plan for the playing track and the one
    /// waiting, by the tracks' addresses, so phrases are found once.
    plan: Option<((usize, usize), Option<Plan>)>,
    /// The mix length and style picked for the mix into a track, by its
    /// id, over the panel's (indices into [`LENGTHS`] and [`STYLES`]).
    overrides: HashMap<usize, (Option<usize>, Option<usize>)>,
    /// The track the next (or current) mix goes into, by id.
    into: Option<usize>,
}

/// Automix mixes between decks 1 and 2, whatever the layout.
const AUTOMIX_DECKS: usize = 2;

fn side(deck: usize) -> f64 {
    if deck == 0 {
        -1.0
    } else {
        1.0
    }
}

/// Where the track's sound starts, in seconds, once that much is analysed.
fn sound_start(t: &Track) -> Option<f64> {
    let w = &t.waveform;
    let bins = w.levels.first()?;
    let rate = w.bins_per_second;
    if rate <= 0.0 {
        return None;
    }
    match bins.iter().position(|b| b[3] >= SILENCE) {
        Some(i) => Some(i as f64 / rate),
        // Silent so far: wait unless the whole track is analysed.
        None if bins.len() as f64 / rate >= t.duration - 0.5 => Some(0.0),
        None => None,
    }
}

/// Where the track's sound ends, in seconds; the track's end until it is
/// fully analysed.
fn sound_end(t: &Track) -> f64 {
    let w = &t.waveform;
    let Some(bins) = w.levels.first() else {
        return t.duration;
    };
    let rate = w.bins_per_second;
    if rate <= 0.0 || (bins.len() as f64 / rate) < t.duration - 0.5 {
        return t.duration;
    }
    match bins.iter().rposition(|b| b[3] >= SILENCE) {
        Some(i) => ((i + 1) as f64 / rate).min(t.duration),
        None => t.duration,
    }
}

fn fmt_countdown(seconds: f64) -> String {
    let s = seconds.max(0.0).ceil() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl AutoMix {
    pub fn new(reg: &ControlRegistry) -> Self {
        let length = crate::settings::automix_length().min(LENGTHS.len() - 1);
        let style = crate::settings::automix_style().min(STYLES.len() - 1);
        Self {
            decks: std::array::from_fn(|d| DeckHandles::new(reg, d)),
            crossfader: reg.get("[Master]", "crossfader").expect("crossfader"),
            enabled: reg.toggle("[AutoDJ]", "enabled"),
            external: reg.toggle("[Master]", "external_mixer"),
            fade_now: Press::new(reg.push("[AutoDJ]", "fade_now")),
            skip_next: Press::new(reg.push("[AutoDJ]", "skip_next")),
            phase: Phase::Off,
            glide: None,
            length,
            style,
            album: false,
            note: String::new(),
            played: None,
            plan: None,
            left: 0,
            repeat: crate::settings::automix_repeat(),
            overrides: HashMap::new(),
            cut_now: false,
            into: None,
        }
    }

    pub fn toggle(&mut self) {
        self.enabled.toggle();
    }

    /// Switches Automix on or off, as AUTOMIX does.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled.set_bool(on);
    }

    /// Plays the Automix list as an album: gaplessly, whatever mix length
    /// is chosen, until Automix stops. Starts Automix if it is off.
    pub fn play_album(&mut self) {
        self.album = true;
        self.enabled.set_bool(true);
    }

    /// The mix length in use: none while playing an album, else the one
    /// picked for the mix into the next track, else the panel's.
    fn mix_length(&self) -> Length {
        if self.album {
            Length::Seconds(0.0)
        } else {
            LENGTHS[self.picked().0.unwrap_or(self.length)]
        }
    }

    /// The mix style in use: the one picked for the mix into the next
    /// track, else the panel's.
    fn mix_style(&self) -> Style {
        STYLES[self.picked().1.unwrap_or(self.style)]
    }

    /// The length and style picked for the mix into the next track.
    fn picked(&self) -> (Option<usize>, Option<usize>) {
        self.into
            .and_then(|id| self.overrides.get(&id))
            .map(|&(l, s)| {
                (
                    l.filter(|&l| l < LENGTHS.len()),
                    s.filter(|&s| s < STYLES.len()),
                )
            })
            .unwrap_or((None, None))
    }

    /// The mix lengths and styles picked for single mixes, by the id of the
    /// track mixed into (Up Next's rows).
    pub fn set_overrides(&mut self, overrides: HashMap<usize, (Option<usize>, Option<usize>)>) {
        self.overrides = overrides;
    }

    pub fn mix_now(&mut self) {
        self.cut_now = false;
        self.fade_now.control.set(1.0);
    }

    /// Player's Next: on to the next track now with a cut, whatever the
    /// mix length and style.
    pub fn cut_now(&mut self) {
        self.cut_now = true;
        self.fade_now.control.set(1.0);
    }

    pub fn skip(&mut self) {
        self.skip_next.control.set(1.0);
    }

    pub fn set_length(&mut self, i: usize) {
        if i < LENGTHS.len() {
            self.length = i;
            crate::settings::set_automix_length(i);
        }
    }

    /// Repeat: played tracks go back to the end of the list.
    pub fn set_repeat(&mut self, on: bool) {
        self.repeat = on;
        crate::settings::set_automix_repeat(on);
    }

    pub fn set_style(&mut self, i: usize) {
        if i < STYLES.len() {
            self.style = i;
            crate::settings::set_automix_style(i);
        }
    }

    /// Runs Automix for one UI frame, `dt` seconds after the last one.
    pub fn tick(&mut self, host: &mut dyn Host, tracks: &[Option<Arc<Track>>; NUM_DECKS], dt: f64) {
        self.into = match &self.phase {
            Phase::Playing { next: Some(n), .. } | Phase::Opening { next: Some(n), .. } => {
                Some(n.id)
            }
            Phase::Mixing(m) => tracks[m.to].as_ref().map(|t| t.id),
            _ => None,
        };
        if self.external.get_bool() {
            if self.enabled.get_bool() || !matches!(self.phase, Phase::Off) {
                self.stop(NEEDS_MIXER);
            }
            self.note = NEEDS_MIXER.into();
        } else if self.note == NEEDS_MIXER {
            self.note.clear();
        }
        let on = self.enabled.get_bool();
        match (&self.phase, on) {
            (Phase::Off, true) => self.start(host, tracks),
            (Phase::Off, false) => {}
            (_, false) => {
                // Switched off by the user: bring both decks back in.
                self.stop("");
                self.glide_to(0.0);
            }
            _ => {
                self.refresh_plan(tracks);
                self.step(host, tracks)
            }
        }
        self.slide(dt);
        self.count_played(host, tracks, dt);
        self.left = host.queued() - usize::from(self.played_at_top(host));
        // Nobody else moves the crossfader while Automix is on.
        self.crossfader.lock(!matches!(self.phase, Phase::Off));
        // Presses while nothing could use them don't wait for later.
        self.fade_now.pressed();
        self.cut_now = false;
        self.skip_next.pressed();
    }

    /// Counts how long the track Automix last started has played, and takes
    /// it off the list once that is long enough. Carries on after Automix
    /// is switched off, so a track it started doesn't play twice.
    fn count_played(
        &mut self,
        host: &mut dyn Host,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
        dt: f64,
    ) {
        let Some(p) = &mut self.played else {
            return;
        };
        if tracks[p.deck].as_ref().map(|t| t.id) != Some(p.id) {
            // Something else was loaded over it: it stays on the list.
            self.played = None;
            return;
        }
        if self.decks[p.deck].play.get_bool() {
            p.seconds += dt.max(0.0);
        }
        if p.seconds >= PLAYED_AFTER {
            if self.repeat {
                host.requeue(p.id);
            } else {
                host.remove(p.id);
            }
            self.played = None;
        }
    }

    /// Whether the top of the list is the track playing until it comes off.
    fn played_at_top(&self, host: &dyn Host) -> bool {
        self.played
            .as_ref()
            .is_some_and(|p| host.track_at(0).is_some_and(|t| t.id == p.id))
    }

    /// The track to play next: the top of the list, past the one playing.
    fn upcoming(&self, host: &dyn Host) -> Option<Arc<Track>> {
        host.track_at(usize::from(self.played_at_top(host)))
    }

    /// How many tracks at the top of the Automix list must stay there when
    /// it is put in order: the first, and also the one playing until it
    /// comes off and the one waiting on a deck.
    /// `top` holds the ids of the first few tracks on the list.
    pub fn fixed(&self, top: &[usize]) -> usize {
        let mut held: Vec<usize> = self.played.iter().map(|p| p.id).collect();
        if let Phase::Playing { next: Some(n), .. } | Phase::Opening { next: Some(n), .. } =
            &self.phase
        {
            held.push(n.id);
        }
        let leading = top
            .iter()
            .take(held.len())
            .take_while(|id| held.contains(id))
            .count();
        leading.max(1)
    }

    /// `deck` has started the track that waited on it. One started before
    /// it has played its part by now: it comes off the list at once.
    fn started(&mut self, host: &mut dyn Host, deck: usize, id: usize) {
        if let Some(p) = self.played.take() {
            host.remove(p.id);
        }
        self.played = Some(Played {
            id,
            deck,
            seconds: 0.0,
        });
    }

    /// Switches Automix off. A mix in progress stays where it is, with the
    /// EQ and filter Automix moved put back.
    fn stop(&mut self, note: &str) {
        if let Phase::Mixing(m) = &self.phase {
            self.restore(m);
        }
        for d in &self.decks {
            d.play_next.set(0.0);
        }
        self.glide = None;
        self.phase = Phase::Off;
        self.album = false;
        self.enabled.set_bool(false);
        self.note = note.to_string();
    }

    /// Automix was switched on.
    fn start(&mut self, host: &mut dyn Host, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        self.note.clear();
        self.glide = None;
        let playing: Vec<usize> = (0..AUTOMIX_DECKS)
            .filter(|&d| self.decks[d].play.get_bool() && tracks[d].is_some())
            .collect();
        if playing.is_empty() {
            if self.upcoming(host).is_none() {
                self.stop("Add tracks to the Automix list first.");
                return;
            }
            self.phase = Phase::Opening {
                deck: 0,
                next: None,
            };
            return;
        }
        // The live deck is the loudest one playing, counting the crossfader.
        let x = self.crossfader.get();
        let level = |d: usize| {
            // The engine's constant-power curve.
            let a = (x + 1.0) * std::f64::consts::FRAC_PI_4;
            let gain = if d == 0 { a.cos() } else { a.sin() };
            self.decks[d].volume.get() * (gain * std::f64::consts::SQRT_2).min(1.0)
        };
        let live = *playing
            .iter()
            .max_by(|&&a, &&b| level(a).total_cmp(&level(b)))
            .unwrap();
        let other = 1 - live;
        if self.decks[other].play.get_bool() {
            // Fade the other deck out, then carry on from the live one.
            self.decks[other].loop_enabled.set_bool(false);
            let pos = self.position(other, tracks);
            let len = self
                .length_on(other, tracks)
                .min(self.remaining(other, tracks));
            self.phase = Phase::Mixing(Mix {
                from: other,
                to: live,
                style: Style::Smooth,
                start: pos,
                end: pos + len,
                to_start: self.position(live, tracks),
                xfade_from: x,
                saved_low: self.decks.each_ref().map(|d| d.eq_low.get()),
                saved_filter: self.decks.each_ref().map(|d| d.filter.get()),
                start_incoming: false,
                gapless: false,
                beat_match: false,
                progress: 0.0,
                swap: 0.5,
            });
        } else {
            self.glide_to(self.side(live));
            self.phase = Phase::Playing { live, next: None };
        }
    }

    /// Starts the crossfader sliding to `target`.
    fn glide_to(&mut self, target: f64) {
        self.glide = Some(Glide {
            target,
            set: self.crossfader.get(),
        });
    }

    /// Moves a glide on by `dt` seconds. Moving the crossfader by hand ends it.
    fn slide(&mut self, dt: f64) {
        let Some(g) = &mut self.glide else {
            return;
        };
        let x = self.crossfader.get();
        if (x - g.set).abs() > TAKEOVER {
            self.glide = None;
            return;
        }
        let step = GLIDE_SPEED * dt.max(0.0);
        let next = if (g.target - x).abs() <= step {
            g.target
        } else {
            x + step * (g.target - x).signum()
        };
        self.crossfader.force(next);
        g.set = self.crossfader.get();
        if next == g.target {
            self.glide = None;
        }
    }

    fn step(&mut self, host: &mut dyn Host, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        match std::mem::replace(&mut self.phase, Phase::Off) {
            Phase::Off => {}
            Phase::Opening { deck, next } => {
                let next = self.prepare(deck, next, host, tracks);
                match next {
                    None => self.stop("Add tracks to the Automix list first."),
                    Some(Next { id, cued: true }) => {
                        self.glide_to(self.side(deck));
                        let d = &self.decks[deck];
                        if d.volume.get() < 0.05 {
                            d.volume.set(1.0);
                        }
                        d.play.set_bool(true);
                        self.started(host, deck, id);
                        self.phase = Phase::Playing {
                            live: deck,
                            next: None,
                        };
                    }
                    next => self.phase = Phase::Opening { deck, next },
                }
            }
            Phase::Playing { live, next } => self.wait(live, next, host, tracks),
            Phase::Mixing(mut m) => {
                self.glide = None;
                let done =
                    !self.decks[m.from].play.get_bool() || self.position(m.from, tracks) >= m.end;
                if done {
                    if m.gapless && !self.decks[m.to].play.get_bool() {
                        // The engine didn't start it (the outgoing deck
                        // was stopped first, say): start it now if the
                        // track did play out.
                        if self.remaining(m.from, tracks) < 1e-3 {
                            self.decks[m.to].play.set_bool(true);
                        }
                    }
                    self.finish(&m);
                    self.decks[m.from].play.set_bool(false);
                    self.phase = Phase::Playing {
                        live: m.to,
                        next: None,
                    };
                } else {
                    self.apply(&mut m, tracks);
                    self.phase = Phase::Mixing(m);
                }
            }
        }
    }

    /// While `live` plays: keeps the next track ready on the other deck and
    /// starts the mix when it is due.
    fn wait(
        &mut self,
        live: usize,
        next: Option<Next>,
        host: &mut dyn Host,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) {
        let other = 1 - live;
        let mut next = next;
        if self.skip_next.pressed() {
            if let Some(n) = next {
                // Skipped: off the list, and the one after it goes on.
                host.remove(n.id);
                next = None;
            }
        }
        // A gapless track plays to its very last frame.
        let ended = self.remaining(live, tracks) < if self.gapless() { 1e-3 } else { 0.3 };
        if ended || !self.decks[live].play.get_bool() {
            if !ended {
                self.stop("Stopped: the deck was paused.");
            } else if next.is_some() || self.upcoming(host).is_some() {
                // The track ran out before the next one was ready: start that
                // one as soon as it is.
                self.decks[live].play.set_bool(false);
                self.phase = Phase::Opening { deck: other, next };
            } else {
                self.decks[live].play.set_bool(false);
                self.stop("The Automix list has finished.");
            }
            return;
        }
        // Don't touch a deck the user has started playing.
        if !self.decks[other].play.get_bool() {
            next = self.prepare(other, next, host, tracks);
        }
        self.into = next.as_ref().map(|n| n.id);
        let Some(n) = next else {
            self.phase = Phase::Playing { live, next };
            return;
        };
        let pos = self.position(live, tracks);
        let end = tracks[live].as_deref().map_or(pos, |t| self.end_of(t));
        let len = self.length_on(live, tracks);
        let gapless = self.gapless();
        let style = if gapless {
            Style::Smooth
        } else {
            self.mix_style()
        };
        // Gapless, the engine is told a little ahead which deck follows.
        let lead = if gapless {
            GAPLESS_AHEAD * self.decks[live].speed().max(0.1)
        } else {
            0.0
        };
        let start = self.planned_start(live, end - len - lead, tracks);
        let now = self.fade_now.pressed();
        let cut = now && std::mem::take(&mut self.cut_now);
        let style = if cut { Style::Cut } else { style };
        if !n.cued || !(now || pos >= start) {
            self.phase = Phase::Playing { live, next };
            return;
        }
        let (start, mut end) = if gapless && !now {
            // The outgoing track plays out on its own, with the crossfader
            // in the centre so both decks are heard in full.
            (start.min(pos), end)
        } else if now || pos > start + 0.5 {
            (pos, (pos + len).min(end.max(pos + 1.0)))
        } else {
            (start, (start + len).min(end.max(start + 1.0)))
        };
        if cut {
            end = start + CUT_NOW;
        } else if style == Style::Cut {
            let beat = tracks[live].as_deref().map_or(0.5, Track::beat_length);
            end = start + beat;
        }
        let beat_match =
            !cut && matches!(
                self.length_for(live, tracks),
                Length::Bars(_) | Length::Phrases
            ) && self.matchable(live, tracks);
        let swap = match (self.length_for(live, tracks), self.current_plan()) {
            (Length::Phrases, Some(p)) => p.swap.map_or(0.5, |s| {
                ((s - start) / (end - start).max(1e-3)).clamp(0.1, 0.9)
            }),
            _ => 0.5,
        };
        self.glide = None;
        // A loop would keep the outgoing track from ever reaching the mix's
        // end, and each jump back would pull the crossfader back with it.
        self.decks[live].loop_enabled.set_bool(false);
        let x = self.crossfader.get();
        let mut m = Mix {
            from: live,
            to: other,
            style,
            start,
            end,
            to_start: self.position(other, tracks),
            xfade_from: x,
            saved_low: self.decks.each_ref().map(|d| d.eq_low.get()),
            saved_filter: self.decks.each_ref().map(|d| d.filter.get()),
            start_incoming: true,
            gapless: gapless && !now,
            beat_match,
            progress: 0.0,
            swap,
        };
        self.apply(&mut m, tracks);
        self.started(host, other, n.id);
        self.phase = Phase::Mixing(m);
    }

    /// Where a mix due to start at `start` on `live`'s track starts: on a
    /// bar of the outgoing track for mixes measured in bars.
    fn planned_start(
        &self,
        live: usize,
        start: f64,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) -> f64 {
        let start = start.max(0.0);
        match (self.length_for(live, tracks), tracks[live].as_deref()) {
            (Length::Phrases, _) if self.current_plan().is_some() => {
                self.current_plan().map_or(start, |p| p.start)
            }
            (Length::Bars(_), Some(t)) if t.bpm > 0.0 => {
                let bar = 4.0 * t.beat_length();
                (t.first_beat + ((start - t.first_beat) / bar).floor() * bar).max(0.0)
            }
            _ => start,
        }
    }

    /// The part of each deck's track the next (or current) mix covers, as
    /// fractions of the track from 0 to 1: on the playing deck from where
    /// the mix starts to where its sound ends, on the waiting deck from its
    /// cue to where the mix will be over. None for decks out of the mix,
    /// and while Automix is off or plays gaplessly.
    pub fn spans(
        &self,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) -> [Option<(f64, f64)>; NUM_DECKS] {
        let mut spans = [None; NUM_DECKS];
        if self.gapless() {
            return spans;
        }
        // (outgoing, its span, incoming, where it starts)
        let (from, start, end, to, to_start) = match &self.phase {
            Phase::Playing {
                live,
                next: Some(Next { cued: true, .. }),
            } => {
                let other = 1 - live;
                let Some(t) = tracks[*live].as_deref() else {
                    return spans;
                };
                let end = self.end_of(t);
                let len = self.length_on(*live, tracks);
                let start = self.planned_start(*live, end - len, tracks);
                let start = start.max(self.position(*live, tracks).min(end));
                let end = if self.mix_style() == Style::Cut {
                    (start + t.beat_length()).min(end)
                } else {
                    (start + len).min(end)
                };
                (*live, start, end, other, self.position(other, tracks))
            }
            Phase::Mixing(m) => (m.from, m.start, m.end, m.to, m.to_start),
            _ => return spans,
        };
        let fraction = |deck: usize, a: f64, b: f64| {
            let t = tracks[deck].as_deref().filter(|t| t.duration > 0.0)?;
            let a = (a / t.duration).clamp(0.0, 1.0);
            let b = (b / t.duration).clamp(0.0, 1.0);
            (b > a).then_some((a, b))
        };
        spans[from] = fraction(from, start, end);
        // The same stretch of time on the incoming track, at its own speed.
        let seconds = (end - start) / self.decks[from].speed().max(0.1);
        let to_len = seconds * self.decks[to].speed().max(0.1);
        spans[to] = fraction(to, to_start, to_start + to_len);
        spans
    }

    /// Makes sure the track at the top of the list is on `deck`, cued to
    /// where its sound starts: a track already waiting there is swapped
    /// for another that has moved to the top. None when the list is empty.
    fn prepare(
        &mut self,
        deck: usize,
        next: Option<Next>,
        host: &mut dyn Host,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) -> Option<Next> {
        let top = self.upcoming(host)?;
        let mut next = match next {
            Some(n) if n.id == top.id => n,
            _ => {
                let t = top;
                // A track already waiting on the deck stays: loading it
                // again would stop it and cue against the old load.
                if tracks[deck].as_ref().map(|x| x.id) != Some(t.id) {
                    host.load(deck, t.clone());
                }
                Next {
                    id: t.id,
                    cued: false,
                }
            }
        };
        if !next.cued {
            if let Some(t) = tracks[deck].as_deref().filter(|t| t.id == next.id) {
                let start = if self.gapless() {
                    Some(0.0)
                } else if self.mix_length() == Length::Phrases {
                    // On the first bar of the incoming phrase.
                    sound_start(t).map(|s| automix_phrases::in_point(t, &automix_phrases::of(t), s))
                } else {
                    sound_start(t)
                };
                if let Some(start) = start.filter(|_| t.duration > 0.0) {
                    let d = &self.decks[deck];
                    d.play.set_bool(false);
                    d.playposition.set(start / t.duration);
                    next.cued = true;
                }
            }
        }
        Some(next)
    }

    /// Moves the controls to where the mix is now.
    fn apply(&mut self, m: &mut Mix, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        let pos = self.position(m.from, tracks);
        // Never backwards: a jump back on the outgoing deck (a loop the user
        // turns on again, a hot cue) holds the mix rather than undoing it.
        let p = ((pos - m.start) / (m.end - m.start).max(1e-3))
            .clamp(0.0, 1.0)
            .max(m.progress);
        m.progress = p;
        let (from, to) = (&self.decks[m.from], &self.decks[m.to]);
        if m.start_incoming {
            m.start_incoming = false;
            to.volume.set(from.volume.get().max(0.05));
            if m.style == Style::BassSwap {
                to.eq_low.set(0.0);
            }
            if m.gapless {
                from.play_next.set((m.to + 1) as f64);
            } else {
                to.play.set_bool(true);
            }
            if m.beat_match {
                to.beatsync.set(1.0);
            }
        } else if m.beat_match && to.beatsync.get_bool() {
            to.beatsync.set(0.0);
        }
        // Even a cut slides, over the one beat it lasts.
        let x = m.xfade_from + (self.side(m.to) - m.xfade_from) * p;
        self.crossfader.force(x);
        match m.style {
            Style::BassSwap => {
                // Swapped over a tenth of the mix, around its middle or a
                // phrase change.
                let k = ((p - (m.swap - 0.05)) / 0.1).clamp(0.0, 1.0);
                from.eq_low.set(m.saved_low[m.from] * (1.0 - k));
                to.eq_low.set(m.saved_low[m.to] * k);
            }
            Style::Filter => from.filter.set(0.5 + 0.4 * p),
            Style::Smooth | Style::Cut => {}
        }
    }

    /// Ends a mix: the crossfader on its way to the incoming deck (it is
    /// there already unless the mix was cut short), the EQ and filter
    /// Automix moved put back.
    fn finish(&mut self, m: &Mix) {
        self.restore(m);
        if m.style == Style::Cut && m.end - m.start <= CUT_NOW + 1e-9 {
            // Player's Next: over at once, as a cut should be.
            self.crossfader.force(self.side(m.to));
        } else {
            self.glide_to(self.side(m.to));
        }
    }

    fn restore(&self, m: &Mix) {
        self.decks[m.from].play_next.set(0.0);
        for d in [m.from, m.to] {
            self.decks[d].eq_low.set(m.saved_low[d]);
            self.decks[d].filter.set(m.saved_filter[d]);
        }
    }

    /// Where `deck` is in its track, in the track's seconds.
    fn position(&self, deck: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> f64 {
        tracks[deck]
            .as_deref()
            .map_or(0.0, |t| self.decks[deck].playposition.get() * t.duration)
    }

    /// Seconds of sound left on `deck`'s track, in the track's seconds.
    fn remaining(&self, deck: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> f64 {
        tracks[deck]
            .as_deref()
            .map_or(0.0, |t| self.end_of(t) - self.position(deck, tracks))
    }

    /// Whether the 0 s length is chosen: tracks follow each other with no mix.
    fn gapless(&self) -> bool {
        self.mix_length() == Length::Seconds(0.0)
    }

    /// Where Automix takes `t` to end: where its sound ends, or for gapless
    /// playback its last frame, so nothing is cut off.
    fn end_of(&self, t: &Track) -> f64 {
        if self.gapless() {
            t.duration
        } else {
            sound_end(t)
        }
    }

    /// Where the crossfader rests while `deck` plays: on its side, or in the
    /// centre for gapless playback, so the next track is heard the moment it
    /// starts.
    fn side(&self, deck: usize) -> f64 {
        if self.gapless() {
            0.0
        } else {
            side(deck)
        }
    }

    /// The chosen mix length, in seconds of `deck`'s track.
    fn length_on(&self, deck: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> f64 {
        match self.length_for(deck, tracks) {
            Length::Seconds(s) => s * self.decks[deck].speed().max(0.1),
            Length::Bars(n) => {
                let beat = tracks[deck].as_deref().map_or(0.5, Track::beat_length);
                n * 4.0 * beat
            }
            Length::Phrases => self.current_plan().map_or(0.0, |p| p.end - p.start),
        }
    }

    /// The length a mix out of `live` takes: the one chosen, except that
    /// Phrases falls back to 16 bars without phrases to go by, and to 15 s
    /// when the tempos are too far apart to beatmatch.
    fn length_for(&self, live: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> Length {
        match self.mix_length() {
            Length::Phrases if !self.matchable(live, tracks) => NO_MATCH,
            Length::Phrases if self.current_plan().is_none() => NO_PHRASES,
            l => l,
        }
    }

    /// Whether the track on the other deck can be beatmatched to `live`'s
    /// within its tempo fader's range.
    fn matchable(&self, live: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> bool {
        let other = 1 - live;
        match (tracks[live].as_deref(), tracks[other].as_deref()) {
            (Some(a), Some(b)) if a.bpm > 0.0 && b.bpm > 0.0 => {
                let target = a.bpm * self.decks[live].speed();
                (target / b.bpm - 1.0).abs() <= self.decks[other].rate_range.get()
            }
            _ => false,
        }
    }

    /// The phrase plan for the tracks now on the decks, once worked out.
    fn current_plan(&self) -> Option<&Plan> {
        self.plan.as_ref().and_then(|(_, p)| p.as_ref())
    }

    /// Works out the Phrases plan when a new track waits cued on the other
    /// deck (or either track changes, a grid edit say), and keeps it until
    /// the mix starts.
    fn refresh_plan(&mut self, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        let Phase::Playing {
            live,
            next: Some(Next { cued: true, .. }),
        } = self.phase
        else {
            if !matches!(self.phase, Phase::Mixing(_)) {
                self.plan = None;
            }
            return;
        };
        if self.mix_length() != Length::Phrases {
            self.plan = None;
            return;
        }
        let (Some(out), Some(inc)) = (&tracks[live], &tracks[1 - live]) else {
            self.plan = None;
            return;
        };
        let key = (Arc::as_ptr(out) as usize, Arc::as_ptr(inc) as usize);
        if self.plan.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let cue = self.position(1 - live, tracks);
        let plan = automix_phrases::plan(
            out,
            &automix_phrases::of(out),
            self.end_of(out),
            inc,
            &automix_phrases::of(inc),
            cue,
        );
        self.plan = Some((key, plan));
    }

    /// What Automix is doing, in a line for the panel and the phone remote.
    pub fn status(&self, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> String {
        let queued = self.left;
        let title = |d: usize| {
            tracks[d]
                .as_deref()
                .map(|t| t.title.clone())
                .unwrap_or_default()
        };
        let status = match &self.phase {
            Phase::Off => self.note.clone(),
            Phase::Opening { .. } => "Starting…".to_string(),
            Phase::Playing { live, next } => match next {
                Some(Next { cued: true, .. }) => {
                    let left = self.seconds_to_mix(tracks).unwrap_or(0.0);
                    format!(
                        "Mixing into {}{} in {}",
                        title(1 - live),
                        self.how(*live, tracks),
                        fmt_countdown(left)
                    )
                }
                Some(_) => "Loading the next track…".to_string(),
                None if queued == 0 => "Last track. Add more to keep going.".to_string(),
                None => String::new(),
            },
            Phase::Mixing(m) if m.start_incoming || !self.decks[m.to].play.get_bool() => {
                String::new()
            }
            Phase::Mixing(m) => format!("Mixing into {}", title(m.to)),
        };
        status
    }

    /// Seconds before the next mix starts, while a track waits cued on the
    /// other deck; `None` otherwise.
    pub fn seconds_to_mix(&self, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> Option<f64> {
        match &self.phase {
            Phase::Playing {
                live,
                next: Some(Next { cued: true, .. }),
            } => {
                let end = tracks[*live].as_deref().map_or(0.0, |t| self.end_of(t));
                let start = self.planned_start(*live, end - self.length_on(*live, tracks), tracks);
                let left = start - self.position(*live, tracks);
                Some((left / self.decks[*live].speed().max(0.1)).max(0.0))
            }
            _ => None,
        }
    }

    /// How the next mix goes, for the status line, when Phrases is chosen:
    /// where it starts, or why it falls back to bars or seconds.
    fn how(&self, live: usize, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> &'static str {
        if self.mix_length() != Length::Phrases {
            return "";
        }
        match (self.length_for(live, tracks), self.current_plan()) {
            (Length::Phrases, Some(p)) => match p.from {
                Start::Outro => " at the outro",
                Start::Phrase => " on a phrase",
                Start::Imported => " at its mix point",
            },
            (Length::Seconds(_), _) => " (15 s: tempos too far apart)",
            _ => " (16 bars: no phrases)",
        }
    }

    /// The title of the track Automix plays next (cued, or being mixed in)
    /// and the time left before its mix starts ("" while mixing, or when
    /// nothing waits).
    pub fn next_up(&self, tracks: &[Option<Arc<Track>>; NUM_DECKS]) -> (String, String) {
        let title = |d: usize| {
            tracks[d]
                .as_deref()
                .map(|t| t.title.clone())
                .unwrap_or_default()
        };
        match &self.phase {
            Phase::Playing {
                live,
                next: Some(Next { cued: true, .. }),
            } => (
                title(1 - live),
                self.seconds_to_mix(tracks)
                    .map(fmt_countdown)
                    .unwrap_or_default(),
            ),
            Phase::Mixing(m) => (title(m.to), String::new()),
            _ => (String::new(), String::new()),
        }
    }

    /// Whether the track waiting on a deck can no longer be swapped for
    /// another: its mix is under `margin` seconds away or under way.
    pub fn next_locked(&self, tracks: &[Option<Arc<Track>>; NUM_DECKS], margin: f64) -> bool {
        match &self.phase {
            Phase::Mixing(_) | Phase::Opening { .. } => true,
            Phase::Playing { .. } => self.seconds_to_mix(tracks).is_some_and(|s| s < margin),
            Phase::Off => false,
        }
    }

    /// The deck Automix is playing from: the one mixed out of during a mix.
    pub fn live_deck(&self) -> Option<usize> {
        match &self.phase {
            Phase::Playing { live, .. } => Some(*live),
            Phase::Mixing(m) => Some(m.from),
            Phase::Opening { deck, .. } => Some(*deck),
            Phase::Off => None,
        }
    }

    /// The deck being mixed into, during a mix.
    pub fn mixing_into(&self) -> Option<usize> {
        match &self.phase {
            Phase::Mixing(m) => Some(m.to),
            _ => None,
        }
    }

    /// Whether Automix is running.
    pub fn is_on(&self) -> bool {
        !matches!(self.phase, Phase::Off)
    }

    /// The mix length shown, as an index into [`LENGTHS`]: an album shows
    /// as the 0 s length it plays with.
    pub fn shown_length(&self) -> usize {
        if self.album {
            0
        } else {
            self.length
        }
    }

    pub fn style(&self) -> usize {
        self.style
    }

    /// The track Automix started that is still on the list (playing), and
    /// the one waiting on the other deck, by id.
    pub fn held(&self) -> (Option<usize>, Option<usize>) {
        let waiting = match &self.phase {
            Phase::Playing { next: Some(n), .. } | Phase::Opening { next: Some(n), .. } => {
                Some(n.id)
            }
            _ => None,
        };
        (self.played.as_ref().map(|p| p.id), waiting)
    }

    /// Shows Automix's state on the panel.
    pub fn show(&self, app: &AppWindow, tracks: &[Option<Arc<Track>>; NUM_DECKS]) {
        let status = self.status(tracks);
        let on = self.is_on();
        if app.get_automix_on() != on {
            app.set_automix_on(on);
        }
        if app.get_automix_status() != status.as_str() {
            app.set_automix_status(status.into());
        }
        let length = self.shown_length() as i32;
        if app.get_automix_length() != length {
            app.set_automix_length(length);
        }
        if app.get_automix_style() != self.style as i32 {
            app.set_automix_style(self.style as i32);
        }
        if app.get_automix_repeat() != self.repeat {
            app.set_automix_repeat(self.repeat);
        }
        // For the Player layout's bar.
        let queued = self.left as i32;
        if app.get_automix_queued() != queued {
            app.set_automix_queued(queued);
        }
        let live = self.live_deck().map_or(-1, |d| d as i32);
        if app.get_automix_live() != live {
            app.set_automix_live(live);
        }
        let into = self.mixing_into().map_or(-1, |d| d as i32);
        if app.get_automix_into() != into {
            app.set_automix_into(into);
        }
        let (next, next_in) = self.next_up(tracks);
        if app.get_automix_next() != next.as_str() {
            app.set_automix_next(next.into());
        }
        if app.get_automix_next_in() != next_in.as_str() {
            app.set_automix_next_in(next_in.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::engine::{Command, Engine, EngineInput};
    use djsugar_engine::loader::prepare;
    use djsugar_engine::synth::sample_library;

    const SR: f64 = 48_000.0;
    const FPS: f64 = 60.0;

    struct TestHost {
        queue: Vec<Arc<Track>>,
        input: EngineInput,
        tracks: [Option<Arc<Track>>; NUM_DECKS],
    }

    impl Host for TestHost {
        fn track_at(&self, index: usize) -> Option<Arc<Track>> {
            self.queue.get(index).cloned()
        }
        fn remove(&mut self, id: usize) {
            if let Some(i) = self.queue.iter().position(|t| t.id == id) {
                self.queue.remove(i);
            }
        }
        fn requeue(&mut self, id: usize) {
            if let Some(i) = self.queue.iter().position(|t| t.id == id) {
                let t = self.queue.remove(i);
                self.queue.push(t);
            }
        }
        fn queued(&self) -> usize {
            self.queue.len()
        }
        fn load(&mut self, deck: usize, track: Arc<Track>) {
            let t = Arc::new(prepare(&track).unwrap());
            self.input
                .send(Command::Load {
                    deck,
                    track: t.clone(),
                })
                .ok()
                .unwrap();
            self.tracks[deck] = Some(t);
        }
    }

    fn track(n: usize, seconds: f64) -> Arc<Track> {
        let mut t = (*sample_library()[n]).clone();
        t.id = 1000 + n;
        t.bpm = 120.0;
        t.duration = seconds;
        Arc::new(t)
    }

    struct Rig {
        reg: Arc<ControlRegistry>,
        engine: Engine,
        host: TestHost,
        automix: AutoMix,
    }

    impl Rig {
        fn new(queue: Vec<Arc<Track>>) -> Self {
            let reg = ControlRegistry::new();
            let (engine, input) = Engine::new(&reg, SR);
            let mut automix = AutoMix::new(&reg);
            automix.length = 1;
            automix.style = 0;
            Self {
                reg,
                engine,
                host: TestHost {
                    queue,
                    input,
                    tracks: Default::default(),
                },
                automix,
            }
        }

        fn ctl(&self, group: &str, key: &str) -> ControlHandle {
            self.reg.get(group, key).unwrap()
        }

        fn deck(&self, d: usize, key: &str) -> ControlHandle {
            self.ctl(&deck_group(d), key)
        }

        fn xfade(&self) -> f64 {
            self.ctl("[Master]", "crossfader").get()
        }

        /// Runs the engine and Automix together, a UI frame at a time.
        fn run(&mut self, seconds: f64) {
            let frames = (SR / FPS) as usize;
            let mut buf = vec![0.0f32; frames * 2];
            for _ in 0..(seconds * FPS).round() as usize {
                self.engine.process(&mut buf, 2);
                let tracks = self.host.tracks.clone();
                self.automix.tick(&mut self.host, &tracks, 1.0 / FPS);
            }
        }

        fn loaded_id(&self, d: usize) -> Option<usize> {
            self.host.tracks[d].as_ref().map(|t| t.id)
        }
    }

    #[test]
    fn zero_seconds_plays_tracks_back_to_back() {
        let mut r = Rig::new(vec![track(0, 10.0), track(1, 10.0), track(2, 10.0)]);
        r.automix.length = 0;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        assert!(r.deck(0, "play").get_bool());
        assert_eq!(r.xfade(), 0.0, "crossfader rests in the centre");
        assert_eq!(
            r.deck(1, "playposition").get(),
            0.0,
            "next track cued at its very start"
        );

        // Frame by frame up to deck A's end: deck B starts within a frame of
        // it, and deck A plays to its last frame.
        let mut left_when_b_started = None;
        let mut a_stopped_at = None;
        for _ in 0..(10.0 * FPS) as usize {
            r.run(1.0 / FPS);
            if left_when_b_started.is_none() && r.deck(1, "play").get_bool() {
                left_when_b_started = Some((1.0 - r.deck(0, "playposition").get()) * 10.0);
            }
            if a_stopped_at.is_none() && !r.deck(0, "play").get_bool() {
                a_stopped_at = Some(r.deck(0, "playposition").get());
            }
        }
        let left = left_when_b_started.expect("deck B started");
        assert!(left < 1.0 / FPS, "deck B started {left} s before the end");
        assert_eq!(a_stopped_at, Some(1.0), "deck A played to its last frame");
        assert!(r.deck(1, "play").get_bool());
        assert_eq!(r.xfade(), 0.0, "no crossfade");
        assert_eq!(r.loaded_id(0), Some(1002), "the next track waits on deck A");
    }

    /// Writes `seconds` of a continuous `hz` sine from `from` seconds into
    /// it, as a 16-bit stereo WAV at `rate`: two such files written back to
    /// back join without a seam.
    fn sine_wav(path: &std::path::Path, rate: u32, hz: f64, from: f64, seconds: f64) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..(seconds * rate as f64) as usize {
            let t = from + i as f64 / rate as f64;
            let v = ((std::f64::consts::TAU * hz * t).sin() * 16_000.0) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
    }

    fn file_track(n: usize, path: std::path::PathBuf) -> Arc<Track> {
        let mut t = (*track(n, 0.0)).clone();
        t.source = djsugar_engine::track::Source::File(path);
        t.duration = 0.0;
        t.bpm = 0.0;
        Arc::new(t)
    }

    #[test]
    fn gapless_tracks_join_without_a_click() {
        // One steady 150 Hz tone cut in two mid-wave, so any gap, overlap or
        // step at the join shows up as a bend a sine never makes. Away from
        // 48 kHz the deck reads between samples, across the join; with key
        // lock or a key shift the time-stretcher plays across it.
        let cases = [
            (48_000u32, 48_000u32, false, 0.0),
            (44_100, 44_100, false, 0.0),
            (44_100, 48_000, false, 0.0),
            (48_000, 48_000, true, 0.0),
            (44_100, 44_100, true, 0.0),
            (48_000, 48_000, false, 1.0),
        ];
        for (rate_a, rate_b, keylock, semitones) in cases {
            let case = format!("{rate_a}/{rate_b} Hz, key lock {keylock}, key {semitones:+}");
            let dir = std::env::temp_dir().join(format!(
                "djsugar-gapless-{}-{rate_a}-{rate_b}-{keylock}-{semitones}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            // Cut mid-wave, not at a zero crossing, on a sample.
            let split = (3.0137 * rate_a as f64).round() / rate_a as f64;
            sine_wav(&dir.join("a.wav"), rate_a, 150.0, 0.0, split);
            sine_wav(&dir.join("b.wav"), rate_b, 150.0, split, 3.0);
            let mut r = Rig::new(vec![
                file_track(0, dir.join("a.wav")),
                file_track(1, dir.join("b.wav")),
            ]);
            for d in 0..2 {
                r.deck(d, "keylock").set_bool(keylock);
                r.deck(d, "pitch").set(semitones);
            }
            r.automix.length = 0;
            r.ctl("[AutoDJ]", "enabled").set(1.0);
            // Engine and Automix a UI frame at a time, keeping the output.
            let frames = (SR / FPS) as usize;
            let mut buf = vec![0.0f32; frames * 2];
            let mut out = Vec::new();
            for _ in 0..(5.0 * FPS) as usize {
                r.engine.process(&mut buf, 2);
                out.extend(buf.iter().step_by(2).copied());
                let tracks = r.host.tracks.clone();
                r.automix.tick(&mut r.host, &tracks, 1.0 / FPS);
            }
            std::fs::remove_dir_all(&dir).ok();
            assert!(
                r.deck(1, "play").get_bool(),
                "{case}: the second track plays"
            );
            // From a second in, past the first track starting from silence.
            let out = &out[SR as usize..];
            let peak = out.iter().fold(0.0f32, |p, v| p.max(v.abs()));
            let (at, bend) = out
                .windows(3)
                .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
                .enumerate()
                .fold((0, 0.0f32), |m, (i, v)| if v > m.1 { (i, v) } else { m });
            // A clean sine bends at most peak * (2 pi f / SR)^2 a sample.
            let hz = 150.0 * 2f32.powf(semitones as f32 / 12.0);
            let smooth = peak * (std::f32::consts::TAU * hz / SR as f32).powi(2);
            assert!(
                bend < 2.0 * smooth,
                "{case}: a click {:.3} s in ({bend} against {smooth})",
                1.0 + at as f64 / SR
            );
            // Nor a gap: the tone never stops.
            let quiet = out.windows(8).any(|w| w.iter().all(|v| v.abs() < 1e-4));
            assert!(!quiet, "{case}: a gap");
        }
    }

    #[test]
    fn an_album_plays_gaplessly_whatever_the_mix_length() {
        let mut r = Rig::new(vec![track(0, 10.0), track(1, 10.0)]);
        // 12 s mixes are chosen.
        r.automix.length = 1;
        r.automix.play_album();
        r.run(2.0);
        assert!(r.deck(0, "play").get_bool());
        assert_eq!(r.xfade(), 0.0, "crossfader rests in the centre");
        r.run(8.5);
        assert!(r.deck(1, "play").get_bool(), "second track follows on");
        assert_eq!(r.xfade(), 0.0, "no crossfade");
        // Once the album has played, the chosen length is back.
        r.run(12.0);
        assert!(!r.ctl("[AutoDJ]", "enabled").get_bool());
        assert_eq!(r.automix.mix_length(), LENGTHS[1]);
    }

    #[test]
    fn starts_the_track_already_waiting_on_the_deck() {
        // Loaded and paused, then put first on the list (an artist's
        // page's Play, say): Automix plays it rather than stopping.
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.host.load(0, track(0, 20.0));
        r.run(0.5);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(3.0);
        assert!(r.deck(0, "play").get_bool(), "{}", r.automix.note);
        assert!(r.ctl("[AutoDJ]", "enabled").get_bool());
    }

    #[test]
    fn a_mix_picked_on_a_row_beats_the_panel() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        // 12 s mixes on the panel, gapless into track 1 alone, in Cut.
        r.automix.length = 1;
        r.automix
            .set_overrides(HashMap::from([(1001, (Some(0), Some(3)))]));
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        assert_eq!(r.automix.mix_length(), LENGTHS[0]);
        assert_eq!(r.automix.mix_style(), Style::Cut);
        r.automix.set_overrides(HashMap::new());
        r.run(0.1);
        assert_eq!(r.automix.mix_length(), LENGTHS[1], "the panel's again");
    }

    #[test]
    fn plays_the_list_with_crossfades() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 40.0), track(2, 20.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        assert!(r.deck(0, "play").get_bool(), "first track plays on deck A");
        assert_eq!(r.xfade(), -1.0, "crossfader slid over to deck A");
        assert_eq!(r.loaded_id(0), Some(1000));
        assert_eq!(r.loaded_id(1), Some(1001), "next track waits on deck B");
        assert!(!r.deck(1, "play").get_bool());
        assert_eq!(r.host.queued(), 3, "both stay on the list for now");
        r.run(4.0);
        assert_eq!(r.host.queued(), 2, "the first came off once it had played");
        assert_eq!(r.host.queue[0].id, 1001, "the waiting one stays on");

        // A 12 s mix ends where deck A's sound ends, so it is under way by 14 s.
        r.run(8.0);
        assert!(r.deck(1, "play").get_bool(), "deck B has come in");
        let x = r.xfade();
        assert!(x > -0.9 && x < 0.9, "crossfader mid-way: {x}");

        r.run(7.0);
        assert_eq!(r.xfade(), 1.0);
        assert!(
            !r.deck(0, "play").get_bool(),
            "deck A stopped after the mix"
        );
        assert_eq!(r.loaded_id(0), Some(1002), "third track on deck A");
        assert_eq!(r.host.queued(), 1, "only the waiting one is left");
        assert!(r.ctl("[AutoDJ]", "enabled").get_bool());

        // The last track mixes in, plays out, and Automix stops.
        r.run(50.0);
        assert!(!r.ctl("[AutoDJ]", "enabled").get_bool());
        assert_eq!(r.automix.note, "The Automix list has finished.");
    }

    #[test]
    fn repeat_puts_played_tracks_back_at_the_end() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 40.0), track(2, 20.0)]);
        r.automix.repeat = true;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(7.0);
        assert_eq!(r.host.queued(), 3, "nothing comes off the list");
        let ids: Vec<usize> = r.host.queue.iter().map(|t| t.id).collect();
        assert_eq!(ids, [1001, 1002, 1000], "the played track went to the end");
    }

    #[test]
    fn a_new_track_at_the_top_swaps_the_waiting_one() {
        let mut r = Rig::new(vec![track(0, 40.0), track(1, 40.0), track(2, 40.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(7.0);
        assert_eq!(r.loaded_id(1), Some(1001));
        assert_eq!(r.host.queued(), 2);

        // Move the last track to the top: it goes on deck B instead.
        let t = r.host.queue.remove(1);
        r.host.queue.insert(0, t);
        r.run(1.0);
        assert_eq!(r.loaded_id(1), Some(1002), "swapped for the new top");
        assert!(!r.deck(1, "play").get_bool());
        assert_eq!(r.host.queued(), 2, "nothing came off the list");

        // Add one at the top: that swaps in too.
        r.host.queue.insert(0, track(3, 40.0));
        r.run(1.0);
        assert_eq!(r.loaded_id(1), Some(1003));

        // Skip takes it off the list and puts the next one on.
        r.automix.skip();
        r.run(1.0);
        assert_eq!(r.loaded_id(1), Some(1002));
        assert_eq!(r.host.queued(), 2);
    }

    #[test]
    fn the_track_mixed_in_stays_listed_for_a_few_seconds() {
        let mut r = Rig::new(vec![track(0, 40.0), track(1, 40.0), track(2, 40.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(7.0);
        r.automix.mix_now();
        r.run(2.0);
        assert!(r.deck(1, "play").get_bool());
        assert_eq!(r.host.queue[0].id, 1001, "still listed while it starts");
        let spans = r.automix.spans(&r.host.tracks.clone());
        assert!(
            spans[0].is_some() && spans[1].is_some(),
            "both decks shaded"
        );
        r.run(4.0);
        assert_eq!(r.host.queue[0].id, 1002, "off the list once it has played");
    }

    #[test]
    fn spans_show_where_the_next_mix_falls() {
        let mut r = Rig::new(vec![track(0, 40.0), track(1, 40.0)]);
        let tracks = |r: &Rig| r.host.tracks.clone();
        assert_eq!(r.automix.spans(&tracks(&r)), [None; NUM_DECKS], "off: none");
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        let spans = r.automix.spans(&tracks(&r));
        let (a0, a1) = spans[0].expect("playing deck shaded");
        assert!(a1 > 0.9 && a1 <= 1.0, "to the end: {a1}");
        assert!(
            ((a1 - a0) * 40.0 - 12.0).abs() < 0.1,
            "12 s long: {a0}..{a1}"
        );
        let (b0, b1) = spans[1].expect("waiting deck shaded");
        assert!(((b1 - b0) * 40.0 - 12.0).abs() < 0.1, "12 s from its cue");
        assert!(b0 < 0.1, "from its cue near the start: {b0}");
        assert!(spans[2..].iter().all(Option::is_none));
    }

    #[test]
    fn phrases_mixes_where_the_plan_says() {
        let mut r = Rig::new(vec![track(0, 96.0), track(1, 96.0)]);
        r.automix.length = PHRASES;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(3.0);
        let tracks = r.host.tracks.clone();
        let status = r.automix.status(&tracks);
        assert!(status.starts_with("Mixing into"), "{status}");
        let plan = r.automix.current_plan().copied();
        match plan {
            Some(p) => {
                assert!(p.end > p.start, "{p:?}");
                let spans = r.automix.spans(&tracks);
                let (a0, a1) = spans[0].expect("shaded");
                assert!((a0 * 96.0 - p.start).abs() < 0.05 && (a1 * 96.0 - p.end).abs() < 0.05);
                assert!(!status.contains("16 bars"), "{status}");
            }
            None => assert!(status.contains("16 bars: no phrases"), "{status}"),
        }
        // The mix starts where planned.
        let start = plan.map_or(96.0 - 32.0, |p| p.start);
        r.run(start - 3.0 + 0.5);
        assert!(r.automix.mixing_into() == Some(1), "mixing by {start}");
        if let Phase::Mixing(m) = &r.automix.phase {
            assert!((m.start - start).abs() < 0.6, "{} vs {start}", m.start);
            assert!(m.beat_match);
        }
    }

    #[test]
    fn phrases_without_a_tempo_match_mixes_15_seconds() {
        let mut r = Rig::new(vec![track(0, 60.0), track(1, 60.0)]);
        r.automix.length = PHRASES;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        // The waiting track far faster than the fader reaches.
        let mut fast = (**r.host.tracks[1].as_ref().unwrap()).clone();
        fast.bpm = 190.0;
        r.host.tracks[1] = Some(Arc::new(fast));
        r.run(0.1);
        let status = r.automix.status(&r.host.tracks.clone());
        assert!(status.contains("15 s: tempos too far apart"), "{status}");
    }

    #[test]
    fn carries_on_from_the_loudest_deck() {
        let mut r = Rig::new(vec![track(2, 40.0)]);
        r.host.load(0, track(0, 40.0));
        r.host.load(1, track(1, 40.0));
        r.run(0.1);
        r.deck(0, "play").set(1.0);
        r.deck(1, "play").set(1.0);
        r.deck(0, "volume").set(0.3);
        r.ctl("[Master]", "crossfader").set(0.0);
        r.run(0.5);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(6.0);
        assert!(r.xfade() > 0.0, "fading towards deck B: {}", r.xfade());
        r.run(8.0);
        assert_eq!(r.xfade(), 1.0);
        assert!(r.deck(1, "play").get_bool(), "deck B keeps playing");
        assert!(!r.deck(0, "play").get_bool(), "deck A stopped");
        assert_eq!(r.loaded_id(0), Some(1002), "first of the list on deck A");
    }

    #[test]
    fn next_in_player_cuts_straight_over() {
        let mut r = Rig::new(vec![track(0, 60.0), track(1, 60.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(1.0);
        r.automix.cut_now();
        r.run(0.1);
        assert!(r.deck(1, "play").get_bool(), "deck B plays");
        assert_eq!(r.xfade(), 1.0, "already all the way over");

        // Mix now still mixes over the mix's length.
        let mut r = Rig::new(vec![track(0, 60.0), track(1, 60.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(1.0);
        r.automix.mix_now();
        r.run(0.1);
        assert!(r.xfade() < 1.0, "still mixing: {}", r.xfade());
    }

    #[test]
    fn the_crossfader_is_locked_while_automix_is_on() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(1.0);
        r.automix.fade_now.control.set(1.0);
        r.run(2.0);
        assert!(r.deck(1, "play").get_bool());
        let x = r.xfade();
        // A mouse or controller can't pull it back mid-mix.
        r.ctl("[Master]", "crossfader").set(-1.0);
        assert_eq!(r.xfade(), x, "the write is ignored");
        r.run(0.1);
        assert!(
            r.ctl("[AutoDJ]", "enabled").get_bool(),
            "the mix carries on"
        );
        assert!(r.xfade() > x, "and keeps sliding towards deck B");

        // Switching Automix off hands it back.
        r.ctl("[AutoDJ]", "enabled").set(0.0);
        r.run(0.1);
        r.ctl("[Master]", "crossfader").set(-0.5);
        r.run(0.1);
        assert_eq!(r.xfade(), -0.5, "left where the user put it");
    }

    #[test]
    fn bass_swap_puts_the_eq_back() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.automix.style = 1;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(1.0);
        r.automix.fade_now.control.set(1.0);
        r.run(1.0);
        let low = |r: &Rig, d| r.ctl(&eq_group(d), "parameter1").get();
        assert_eq!(low(&r, 1), 0.0, "incoming bass out before the swap");
        assert_eq!(low(&r, 0), 1.0);
        r.run(12.0);
        assert_eq!(r.xfade(), 1.0);
        assert_eq!(low(&r, 0), 1.0);
        assert_eq!(low(&r, 1), 1.0);
    }

    #[test]
    fn slides_to_the_live_deck_and_back_to_the_centre() {
        let mut r = Rig::new(vec![track(1, 40.0)]);
        r.host.load(0, track(0, 40.0));
        r.run(0.1);
        r.deck(0, "play").set(1.0);
        r.ctl("[Master]", "crossfader").set(0.0);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(0.5);
        let x = r.xfade();
        assert!(x < -0.1 && x > -0.9, "sliding to deck A, not jumping: {x}");
        r.run(1.5);
        assert_eq!(r.xfade(), -1.0);

        // Switching Automix off slides the crossfader back to the middle.
        r.ctl("[AutoDJ]", "enabled").set(0.0);
        r.run(0.5);
        let x = r.xfade();
        assert!(x > -0.9 && x < -0.1, "sliding back: {x}");
        r.run(1.5);
        assert_eq!(r.xfade(), 0.0);
        assert!(r.deck(0, "play").get_bool(), "the music keeps playing");
    }

    #[test]
    fn a_cut_still_slides() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.automix.style = 3;
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        r.automix.fade_now.control.set(1.0);
        r.run(2.0 / FPS);
        let x = r.xfade();
        assert!(x > -1.0 && x < 0.9, "part way after a frame: {x}");
        r.run(2.0);
        assert_eq!(r.xfade(), 1.0);
    }

    #[test]
    fn a_loop_on_the_outgoing_deck_plays_out() {
        let mut r = Rig::new(vec![track(0, 20.0), track(1, 20.0)]);
        r.ctl("[AutoDJ]", "enabled").set(1.0);
        r.run(2.0);
        r.deck(0, "beatloop_4_activate").set(1.0);
        r.run(0.1);
        assert!(r.deck(0, "loop_enabled").get_bool());
        r.automix.fade_now.control.set(1.0);
        r.run(0.1);
        assert!(
            !r.deck(0, "loop_enabled").get_bool(),
            "the loop is let go when the mix starts"
        );
        // The crossfader only ever moves towards deck B, and gets there.
        let mut last = r.xfade();
        for _ in 0..(14.0 * FPS) as usize {
            r.run(1.0 / FPS);
            let x = r.xfade();
            assert!(x >= last, "crossfader went back from {last} to {x}");
            last = x;
        }
        assert_eq!(r.xfade(), 1.0);
        assert!(
            !r.deck(0, "play").get_bool(),
            "deck A stopped after the mix"
        );
        assert!(r.deck(1, "play").get_bool());
    }
}
