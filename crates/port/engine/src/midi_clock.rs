//! MIDI clock out: drum machines, grooveboxes, synths and other apps follow
//! the master deck (the deck Ableton Link follows) at 24 pulses per beat.
//!
//! The audio thread only publishes a [`BeatClock`] snapshot each buffer: the
//! host time the buffer is heard, the master deck's beat then and its tempo.
//! A thread of its own ([`start`]) reads it, works out when each pulse is due
//! with a [`Generator`], and sends it at that moment: it sleeps until just
//! before, then spins on the clock, so pulses go out within a fraction of a
//! millisecond of when the audio is heard.
//!
//! The pulses follow the master deck's tempo and stay on its beat: drift (a
//! nudge, another deck taking over) is caught up by bending the pulse rate a
//! little, never by jumping, since gear reads the tempo from the spacing of
//! the pulses. Start goes out on the deck's next downbeat, after Song Position
//! 0, so pattern-based gear lands on the bar; RESTART ON BAR does the same
//! while running. When the master deck stops, Stop goes out, or with
//! `[MidiClock],keep_running` the clock carries on at the last tempo.
//!
//! Controls: `[MidiClock],enabled` (CLOCK OUT), `[MidiClock],restart`,
//! `[MidiClock],keep_running`, and the indicators `[MidiClock],running`,
//! `[MidiClock],bpm` and `[MidiClock],outputs` (outputs open).

use std::collections::VecDeque;
use std::sync::atomic::{fence, AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_link::AblLink;

pub const GROUP: &str = "[MidiClock]";

/// The port DJ Sugar makes for other apps on the same computer (macOS and
/// Linux).
pub const VIRTUAL_PORT: &str = "DJ Sugar Clock";

/// Pulses per quarter note (beat), as MIDI clock always has.
pub const PPQN: f64 = 24.0;

/// Beats in a bar: Start and RESTART ON BAR land on these.
const BAR: f64 = 4.0;

/// How far an output's offset may move its messages, in milliseconds.
pub const MAX_OFFSET_MS: f64 = 50.0;

/// How fast the pulses catch up with the deck's beat: the rate is bent by
/// this fraction for each beat of drift.
const PHASE_GAIN: f64 = 0.5;

/// The most the pulse rate is bent to catch up, as a fraction.
const MAX_BEND: f64 = 0.08;

/// Song Position and Start go out this long before the downbeat's pulse
/// (microseconds), and Stop before them when restarting.
const START_LEAD: f64 = 1000.0;
const POSITION_LEAD: f64 = 1500.0;
const RESTART_STOP_LEAD: f64 = 2000.0;

/// The master deck as a buffer is heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    /// Host time the buffer is heard, in microseconds (Link's clock).
    pub heard_at: f64,
    /// The master deck's beat then; beat 0 is bar 1.
    pub beat: f64,
    /// Its tempo as playing; 0 with no deck playing.
    pub tempo: f64,
}

impl Snapshot {
    /// The deck's beat at host time `t`.
    pub fn beat_at(&self, t: f64) -> f64 {
        self.beat + (t - self.heard_at) * self.tempo / 60e6
    }

    /// When the deck reaches `beat`.
    pub fn time_of(&self, beat: f64) -> f64 {
        self.heard_at + (beat - self.beat) * 60e6 / self.tempo
    }

    fn playing(&self) -> bool {
        self.tempo > 0.0
    }
}

/// The master deck's beat and tempo, written by the audio thread each
/// buffer and read by the clock thread, with no locks: a sequence count
/// tells the reader to read again if it caught a write half done.
#[derive(Default)]
pub struct BeatClock {
    seq: AtomicU64,
    heard_at: AtomicI64,
    beat: AtomicU64,
    tempo: AtomicU64,
}

impl BeatClock {
    pub(crate) fn publish(&self, heard_at: i64, beat: f64, tempo: f64) {
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        self.heard_at.store(heard_at, Ordering::Relaxed);
        self.beat.store(beat.to_bits(), Ordering::Relaxed);
        self.tempo.store(tempo.to_bits(), Ordering::Relaxed);
        self.seq.store(s.wrapping_add(2), Ordering::Release);
    }

    /// The last snapshot; None before the first buffer.
    pub fn read(&self) -> Option<Snapshot> {
        loop {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let heard_at = self.heard_at.load(Ordering::Relaxed);
            let beat = f64::from_bits(self.beat.load(Ordering::Relaxed));
            let tempo = f64::from_bits(self.tempo.load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) != s1 {
                continue;
            }
            return (s1 > 0).then_some(Snapshot {
                heard_at: heard_at as f64,
                beat,
                tempo,
            });
        }
    }
}

/// A MIDI real-time message the clock sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Clock,
    Start,
    Stop,
    /// Song Position Pointer, in sixteenth notes from the start.
    SongPosition(u16),
}

impl Message {
    pub fn bytes(self) -> Vec<u8> {
        match self {
            Message::Clock => vec![0xF8],
            Message::Start => vec![0xFA],
            Message::Stop => vec![0xFC],
            Message::SongPosition(p) => vec![0xF2, (p & 0x7F) as u8, ((p >> 7) & 0x7F) as u8],
        }
    }
}

/// A message and the host time (microseconds) it is due.
pub type Timed = (f64, Message);

/// Pulses going out with the transport stopped: the next at `t`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Free {
    t: f64,
    tempo: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    /// No pulses: off, or no deck has played yet.
    Idle,
    /// Waiting for the deck to reach `bar` to send Start; pulses carry on
    /// at the last tempo until then if they were going.
    Armed { bar: f64, free: Option<Free> },
    /// Running: pulse `n` (counted from the last Start) is due at `t`.
    Ticking {
        n: i64,
        t: f64,
        tempo: f64,
        /// Stop, Song Position and Start go out before this pulse, which
        /// becomes pulse 0.
        restart_at: Option<i64>,
    },
    /// Stop was sent; pulses carry on at the last tempo, so gear keeps
    /// showing it, until a deck plays again.
    Stopped(Free),
}

/// Works out the clock's messages and when each is due, from the master
/// deck's snapshots. It has no clock of its own: [`Generator::poll`] is
/// given the time, so it can be tested without waiting.
#[derive(Debug)]
pub struct Generator {
    state: State,
    restart: bool,
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            state: State::Idle,
            restart: false,
        }
    }
}

/// Wraps a beat difference to the nearest beat: -0.5..0.5.
fn wrap(beats: f64) -> f64 {
    let e = beats.rem_euclid(1.0);
    if e >= 0.5 {
        e - 1.0
    } else {
        e
    }
}

/// The first bar at or after `beat`.
fn bar_from(beat: f64) -> f64 {
    (beat / BAR - 1e-9).ceil() * BAR
}

/// Microseconds between pulses at `tempo`.
fn interval(tempo: f64) -> f64 {
    60e6 / (PPQN * tempo)
}

impl Generator {
    /// Asks for Start again on the master deck's next downbeat.
    pub fn restart(&mut self) {
        self.restart = true;
    }

    /// True between Start and Stop.
    pub fn running(&self) -> bool {
        matches!(self.state, State::Ticking { .. })
    }

    /// The tempo pulses go out at, while they do.
    pub fn tempo(&self) -> Option<f64> {
        match self.state {
            State::Ticking { tempo, .. } => Some(tempo),
            State::Stopped(f) | State::Armed { free: Some(f), .. } => Some(f.tempo),
            _ => None,
        }
    }

    /// When [`Generator::poll`] next has something to give, given the
    /// last snapshot: the next pulse, or the downbeat Start waits for.
    pub fn next_due(&self, snap: Option<Snapshot>) -> Option<f64> {
        let bar_time = |bar| snap.filter(Snapshot::playing).map(|s| s.time_of(bar));
        match self.state {
            State::Idle => None,
            State::Armed { bar, free: None } => bar_time(bar),
            State::Armed { bar, free: Some(f) } => Some(bar_time(bar).map_or(f.t, |t| t.min(f.t))),
            State::Ticking { t, .. } | State::Stopped(Free { t, .. }) => Some(t),
        }
    }

    /// Adds to `out` every message due by `horizon` (host time,
    /// microseconds), in the order to send them. `now` is the time now,
    /// for messages that go out at once (Stop).
    pub fn poll(
        &mut self,
        snap: Option<Snapshot>,
        enabled: bool,
        keep_running: bool,
        now: f64,
        horizon: f64,
        out: &mut Vec<Timed>,
    ) {
        let mut restart = std::mem::take(&mut self.restart);
        let master = snap.filter(Snapshot::playing);
        if !enabled {
            if self.running() {
                out.push((now, Message::Stop));
            }
            self.state = State::Idle;
            return;
        }
        loop {
            self.state = match self.state {
                State::Idle => match master {
                    Some(s) => State::Armed {
                        bar: bar_from(s.beat_at(now)),
                        free: None,
                    },
                    None => return,
                },
                State::Stopped(f) => match master {
                    Some(s) => State::Armed {
                        bar: bar_from(s.beat_at(now)),
                        free: Some(f),
                    },
                    None if f.t <= horizon => {
                        out.push((f.t, Message::Clock));
                        State::Stopped(Free {
                            t: f.t + interval(f.tempo),
                            ..f
                        })
                    }
                    None => return,
                },
                State::Armed { bar, free } => {
                    let Some(s) = master else {
                        self.state = free.map_or(State::Idle, State::Stopped);
                        continue;
                    };
                    let at = s.beat_at(now);
                    let start = s.time_of(bar);
                    // The deck jumped: wait for the next bar from where
                    // it is now.
                    if start < now || bar - at > BAR + 1e-6 {
                        State::Armed {
                            bar: bar_from(at),
                            free,
                        }
                    } else if let Some(f) = free.filter(|f| f.t <= horizon) {
                        // Free pulses up to half a pulse before Start's.
                        if f.t < start - interval(s.tempo) / 2.0 {
                            out.push((f.t, Message::Clock));
                            State::Armed {
                                bar,
                                free: Some(Free {
                                    t: f.t + interval(f.tempo),
                                    ..f
                                }),
                            }
                        } else {
                            State::Armed { bar, free: None }
                        }
                    } else if start <= horizon {
                        out.push((start - POSITION_LEAD, Message::SongPosition(0)));
                        out.push((start - START_LEAD, Message::Start));
                        out.push((start, Message::Clock));
                        State::Ticking {
                            n: 1,
                            t: start + interval(s.tempo),
                            tempo: s.tempo,
                            restart_at: None,
                        }
                    } else {
                        return;
                    }
                }
                State::Ticking {
                    mut n,
                    t,
                    mut tempo,
                    mut restart_at,
                } => {
                    if master.is_none() && !keep_running {
                        out.push((now, Message::Stop));
                        self.state = State::Stopped(Free { t, tempo });
                        continue;
                    }
                    if std::mem::take(&mut restart) {
                        restart_at = Some(match master {
                            Some(s) => {
                                let beat = s.beat_at(t);
                                n + ((bar_from(beat) - beat) * PPQN).round() as i64
                            }
                            // Running free: the clock's own next bar.
                            None => {
                                let per_bar = (PPQN * BAR) as i64;
                                (n + per_bar - 1).div_euclid(per_bar) * per_bar
                            }
                        });
                    }
                    if t > horizon {
                        self.state = State::Ticking {
                            n,
                            t,
                            tempo,
                            restart_at,
                        };
                        return;
                    }
                    if restart_at == Some(n) {
                        out.push((t - RESTART_STOP_LEAD, Message::Stop));
                        out.push((t - POSITION_LEAD, Message::SongPosition(0)));
                        out.push((t - START_LEAD, Message::Start));
                        n = 0;
                        restart_at = None;
                    }
                    out.push((t, Message::Clock));
                    // The next pulse: at the deck's tempo, bent towards its
                    // beat; running free, at the last tempo.
                    let mut bend = 0.0;
                    if let Some(s) = master {
                        tempo = s.tempo;
                        let error = wrap(s.beat_at(t) - n as f64 / PPQN);
                        bend = (error * PHASE_GAIN).clamp(-MAX_BEND, MAX_BEND);
                    }
                    State::Ticking {
                        n: n + 1,
                        t: t + interval(tempo * (1.0 + bend)),
                        tempo,
                        restart_at,
                    }
                }
            };
        }
    }
}

/// One place clock goes: a MIDI output, or the virtual port.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputConfig {
    /// The output's name, or [`VIRTUAL_PORT`].
    pub port: String,
    /// Moves its messages later (+) or earlier (-), for gear that sounds
    /// early or late; at most [`MAX_OFFSET_MS`] either way.
    pub offset_ms: f64,
}

/// An output's state, for Settings.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputStatus {
    pub port: String,
    /// Open and sending.
    pub open: bool,
}

#[derive(Default)]
struct Shared {
    outputs: Mutex<Vec<OutputConfig>>,
    generation: AtomicU64,
    status: Mutex<Vec<OutputStatus>>,
    /// The input clock is read from, by name.
    input: Mutex<Option<String>>,
    input_generation: AtomicU64,
    input_open: AtomicBool,
}

/// Picks the outputs the clock thread sends to.
#[derive(Clone, Default)]
pub struct MidiClock {
    shared: Arc<Shared>,
}

impl MidiClock {
    /// Sends the clock to `outputs` from now on.
    pub fn set_outputs(&self, outputs: Vec<OutputConfig>) {
        *self.shared.outputs.lock().unwrap() = outputs;
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Reads clock in from `port` from now on; None for no input.
    pub fn set_input(&self, port: Option<String>) {
        *self.shared.input.lock().unwrap() = port;
        self.shared.input_generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Whether the clock input is open.
    pub fn input_open(&self) -> bool {
        self.shared.input_open.load(Ordering::Relaxed)
    }

    /// The MIDI inputs clock can be read from.
    pub fn input_ports() -> Vec<String> {
        crate::clock_in::input_ports()
    }

    /// Which outputs are open.
    pub fn status(&self) -> Vec<OutputStatus> {
        self.shared.status.lock().unwrap().clone()
    }

    /// The MIDI outputs there are now, the virtual port first where there
    /// is one.
    pub fn output_ports() -> Vec<String> {
        let mut names = Vec::new();
        if cfg!(unix) {
            names.push(VIRTUAL_PORT.to_string());
        }
        if let Ok(out) = midir::MidiOutput::new("DJ Sugar clock ports") {
            names.extend(
                out.ports()
                    .iter()
                    .filter_map(|p| out.port_name(p).ok())
                    .filter(|n| !n.contains(VIRTUAL_PORT) && !n.contains("DJ Sugar")),
            );
        }
        names
    }
}

/// Creates the `[MidiClock]` controls.
pub(crate) fn create_controls(reg: &ControlRegistry) {
    reg.toggle(GROUP, "enabled");
    reg.push(GROUP, "restart");
    reg.toggle(GROUP, "keep_running");
    reg.indicator(GROUP, "running", 1.0);
    reg.indicator(GROUP, "bpm", 999.0);
    reg.indicator(GROUP, "outputs", 64.0);
    // Clock in: the tempo it reads and whether it has started.
    reg.indicator(GROUP, "in_bpm", 999.0);
    reg.indicator(GROUP, "in_running", 1.0);
    reg.create(
        LEADER_GROUP,
        "sync_leader_source",
        0.0,
        0.0,
        2.0,
        djsugar_control::Behavior::Free,
    );
}

/// Where `[Master],sync_leader_source` lives.
pub const LEADER_GROUP: &str = "[Master]";

/// What synced decks follow (`[Master],sync_leader_source`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderSource {
    /// The master deck, as decks always have.
    Deck,
    /// The Ableton Link session.
    Link,
    /// The MIDI clock input.
    Clock,
}

impl LeaderSource {
    pub fn from_value(v: f64) -> Self {
        match v.round() as i32 {
            1 => LeaderSource::Link,
            2 => LeaderSource::Clock,
            _ => LeaderSource::Deck,
        }
    }
}

/// Where messages go; a MIDI connection, or a recorder in tests.
trait Sink: Send {
    fn send(&mut self, bytes: &[u8]) -> bool;
}

impl Sink for midir::MidiOutputConnection {
    fn send(&mut self, bytes: &[u8]) -> bool {
        midir::MidiOutputConnection::send(self, bytes).is_ok()
    }
}

struct Output {
    config: OutputConfig,
    sink: Option<Box<dyn Sink>>,
    queue: VecDeque<Timed>,
}

impl Output {
    fn offset_us(&self) -> f64 {
        self.config.offset_ms.clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS) * 1000.0
    }
}

fn open(port: &str) -> Option<Box<dyn Sink>> {
    let out = midir::MidiOutput::new("DJ Sugar").ok()?;
    if port == VIRTUAL_PORT {
        #[cfg(unix)]
        {
            use midir::os::unix::VirtualOutput;
            return match out.create_virtual(VIRTUAL_PORT) {
                Ok(c) => Some(Box::new(c)),
                Err(e) => {
                    eprintln!("could not make the {VIRTUAL_PORT} MIDI port: {e}");
                    None
                }
            };
        }
        #[cfg(not(unix))]
        return None;
    }
    let p = out
        .ports()
        .into_iter()
        .find(|p| out.port_name(p).as_deref() == Ok(port))?;
    match out.connect(&p, "DJ Sugar clock") {
        Ok(c) => Some(Box::new(c)),
        Err(e) => {
            eprintln!("{port}: cannot open MIDI output for clock: {e}");
            None
        }
    }
}

/// Sleeps until this long before a message is due, then spins.
const SPIN: f64 = 1500.0;
/// Commits pulses this long ahead of when they are due, beyond any
/// negative offset.
const LOOKAHEAD: f64 = 3000.0;
/// Closed outputs are tried again this often (a device plugged in later).
const RETRY: f64 = 2e6;

/// The clock thread's work, apart from opening ports, so a test can drive
/// it with its own sinks.
struct Sender {
    generator: Generator,
    outputs: Vec<Output>,
    events: Vec<Timed>,
}

impl Sender {
    fn lookahead(&self) -> f64 {
        let earliest = self
            .outputs
            .iter()
            .map(Output::offset_us)
            .fold(0.0, f64::min);
        LOOKAHEAD - earliest
    }

    /// Generates what is due soon and queues it on each output.
    fn generate(&mut self, snap: Option<Snapshot>, enabled: bool, keep_running: bool, now: f64) {
        let horizon = now + self.lookahead();
        self.events.clear();
        self.generator
            .poll(snap, enabled, keep_running, now, horizon, &mut self.events);
        for o in &mut self.outputs {
            let offset = o.offset_us();
            o.queue
                .extend(self.events.iter().map(|&(t, m)| (t + offset, m)));
        }
    }

    /// The output whose next message is due first, and when.
    fn earliest(&self) -> Option<(usize, f64)> {
        self.outputs
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.queue.front().map(|&(t, _)| (i, t)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Sends every message due within [`SPIN`] of now, each at its time.
    fn send_due(&mut self, clock: &dyn Fn() -> f64) {
        while let Some((i, t)) = self.earliest() {
            if t - clock() > SPIN {
                return;
            }
            while clock() < t {
                std::hint::spin_loop();
            }
            let o = &mut self.outputs[i];
            let (_, m) = o.queue.pop_front().expect("queued");
            if let Some(sink) = o.sink.as_mut() {
                sink.send(&m.bytes());
            }
        }
    }

    /// How long to sleep before there's something to do.
    fn idle_for(&self, snap: Option<Snapshot>, enabled: bool, now: f64) -> f64 {
        let mut wake = now + if enabled { 5000.0 } else { 50_000.0 };
        if let Some((_, t)) = self.earliest() {
            wake = wake.min(t - SPIN);
        }
        if let Some(t) = self.generator.next_due(snap) {
            wake = wake.min(t - self.lookahead());
        }
        (wake - now).max(0.0)
    }
}

/// Starts the clock thread, which runs until the registry is dropped.
pub(crate) fn start(
    beat_clock: Arc<BeatClock>,
    clock_in: Arc<crate::clock_in::ClockIn>,
    link: Arc<AblLink>,
    reg: &Arc<ControlRegistry>,
) -> MidiClock {
    let clock = MidiClock::default();
    let shared = clock.shared.clone();
    let weak = Arc::downgrade(reg);
    let handle = |key: &str| reg.get(GROUP, key).expect("MIDI clock controls");
    let c = [
        handle("enabled"),
        handle("restart"),
        handle("keep_running"),
        handle("running"),
        handle("bpm"),
        handle("outputs"),
        handle("in_bpm"),
        handle("in_running"),
    ];
    let spawned = std::thread::Builder::new()
        .name("midi-clock".into())
        .spawn(move || run(beat_clock, clock_in, link, weak, shared, c));
    if let Err(e) = spawned {
        eprintln!("could not start MIDI clock: {e}");
    }
    clock
}

fn run(
    beat_clock: Arc<BeatClock>,
    clock_in: Arc<crate::clock_in::ClockIn>,
    link: Arc<AblLink>,
    reg: Weak<ControlRegistry>,
    shared: Arc<Shared>,
    [enabled, restart, keep_running, running, bpm, open_count, in_bpm, in_running]: [ControlHandle;
        8],
) {
    let link_for_input = link.clone();
    let clock = move || link.clock_micros() as f64;
    let mut input: Option<midir::MidiInputConnection<()>> = None;
    let mut input_generation = u64::MAX;
    let mut input_try = f64::MIN;
    let mut sender = Sender {
        generator: Generator::default(),
        outputs: Vec::new(),
        events: Vec::new(),
    };
    let mut generation = u64::MAX;
    let mut last_try = f64::MIN;
    while reg.strong_count() > 0 {
        let now = clock();
        let on = enabled.get_bool();
        // Outputs changed in Settings, or one is missing: open them.
        let g = shared.generation.load(Ordering::Relaxed);
        let missing = sender.outputs.iter().any(|o| o.sink.is_none());
        if g != generation || (on && missing && now - last_try > RETRY) {
            let wanted = shared.outputs.lock().unwrap().clone();
            let mut old = std::mem::take(&mut sender.outputs);
            sender.outputs = wanted
                .into_iter()
                .map(|config| {
                    // Keep a connection that is still wanted.
                    let kept = old
                        .iter()
                        .position(|o| o.config.port == config.port && o.sink.is_some())
                        .map(|i| old.swap_remove(i));
                    let sink = kept.and_then(|o| o.sink).or_else(|| open(&config.port));
                    Output {
                        config,
                        sink,
                        queue: VecDeque::new(),
                    }
                })
                .collect();
            // Gear on a dropped output shouldn't keep playing.
            for mut o in old {
                if let Some(sink) = o.sink.as_mut() {
                    if sender.generator.running() {
                        sink.send(&Message::Stop.bytes());
                    }
                }
            }
            *shared.status.lock().unwrap() = sender
                .outputs
                .iter()
                .map(|o| OutputStatus {
                    port: o.config.port.clone(),
                    open: o.sink.is_some(),
                })
                .collect();
            open_count.set(sender.outputs.iter().filter(|o| o.sink.is_some()).count() as f64);
            generation = g;
            last_try = now;
        }
        // The clock input changed in Settings, or isn't open yet.
        let g = shared.input_generation.load(Ordering::Relaxed);
        let wanted = shared.input.lock().unwrap().clone();
        if g != input_generation || (input.is_none() && wanted.is_some() && now - input_try > RETRY)
        {
            if g != input_generation {
                drop(input.take());
            }
            if input.is_none() {
                clock_in.clear();
                in_bpm.set(0.0);
                in_running.set_bool(false);
                input = wanted.as_deref().and_then(|port| {
                    crate::clock_in::open(
                        port,
                        clock_in.clone(),
                        link_for_input.clone(),
                        in_bpm.clone(),
                        in_running.clone(),
                    )
                });
            }
            shared.input_open.store(input.is_some(), Ordering::Relaxed);
            input_generation = g;
            input_try = now;
        }
        // A clock that stopped coming isn't shown as running.
        if clock_in
            .read()
            .is_some_and(|r| now - r.at.heard_at > crate::clock_in::STALE)
        {
            clock_in.clear();
            in_bpm.set(0.0);
            in_running.set_bool(false);
        }
        if restart.get_bool() {
            restart.set(0.0);
            sender.generator.restart();
        }
        let snap = beat_clock.read();
        sender.generate(snap, on, keep_running.get_bool(), now);
        sender.send_due(&clock);
        running.set_bool(sender.generator.running());
        bpm.set(sender.generator.tempo().unwrap_or(0.0));
        let wait = sender.idle_for(snap, on, clock());
        if wait > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(wait / 1e6));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const US_PER_PULSE_120: f64 = 60e6 / (PPQN * 120.0);

    fn snap(t: f64, beat: f64, tempo: f64) -> Option<Snapshot> {
        Some(Snapshot {
            heard_at: t,
            beat,
            tempo,
        })
    }

    /// Runs the generator every millisecond from `from` to `to` against a
    /// deck at `tempo` whose beat is `beat_at(t)`.
    fn run(
        g: &mut Generator,
        from: f64,
        to: f64,
        tempo: f64,
        beat_at: impl Fn(f64) -> f64,
        keep_running: bool,
    ) -> Vec<Timed> {
        let mut out = Vec::new();
        let mut now = from;
        while now < to {
            let s = snap(now + 5000.0, beat_at(now + 5000.0), tempo);
            g.poll(s, true, keep_running, now, now + LOOKAHEAD, &mut out);
            now += 1000.0;
        }
        out
    }

    fn clocks(events: &[Timed]) -> Vec<f64> {
        events
            .iter()
            .filter(|(_, m)| *m == Message::Clock)
            .map(|(t, _)| *t)
            .collect()
    }

    #[test]
    fn snapshots_read_back_what_was_published() {
        let c = BeatClock::default();
        assert_eq!(c.read(), None);
        c.publish(1_000_000, 3.25, 128.0);
        assert_eq!(c.read(), snap(1e6, 3.25, 128.0));
    }

    #[test]
    fn song_position_bytes() {
        assert_eq!(Message::SongPosition(0).bytes(), vec![0xF2, 0, 0]);
        assert_eq!(Message::SongPosition(200).bytes(), vec![0xF2, 72, 1]);
    }

    #[test]
    fn starts_on_the_next_downbeat_with_song_position() {
        let mut g = Generator::default();
        // At 120 BPM a beat is half a second; the deck is on beat 1.5 at
        // time 0, so beat 4 (bar 2) is at 1.25 s.
        let beat_at = |t: f64| 1.5 + t * 120.0 / 60e6;
        let out = run(&mut g, 0.0, 2e6, 120.0, beat_at, false);
        let start = out.iter().position(|(_, m)| *m == Message::Start).unwrap();
        assert_eq!(out[start - 1].1, Message::SongPosition(0));
        assert_eq!(out[start + 1].1, Message::Clock);
        assert!(
            (out[start + 1].0 - 1.25e6).abs() < 1.0,
            "{}",
            out[start + 1].0
        );
        assert!(out[start].0 < out[start + 1].0);
        // Nothing before Start.
        assert_eq!(start, 1);
        // Then a pulse every 1/24 of a beat.
        let c = clocks(&out);
        for w in c.windows(2) {
            assert!(
                (w[1] - w[0] - US_PER_PULSE_120).abs() < 1.0,
                "{}",
                w[1] - w[0]
            );
        }
    }

    #[test]
    fn catches_up_with_a_nudged_deck_smoothly() {
        let mut g = Generator::default();
        // The deck jumps a tenth of a beat ahead at 3 s.
        let beat_at = |t: f64| t * 120.0 / 60e6 + if t > 3e6 { 0.1 } else { 0.0 };
        let out = run(&mut g, 0.0, 12e6, 120.0, beat_at, false);
        let c = clocks(&out);
        // No pulse gap bends more than MAX_BEND.
        for w in c.windows(2) {
            let ratio = US_PER_PULSE_120 / (w[1] - w[0]);
            assert!((ratio - 1.0).abs() <= MAX_BEND + 1e-9, "{ratio}");
        }
        // Pulse n (from Start at beat 0, time 0) is back on the deck's beat.
        let (n, t) = (c.len() - 1, *c.last().unwrap());
        let error = wrap(beat_at(t) - n as f64 / PPQN);
        assert!(error.abs() < 0.002, "{error}");
    }

    #[test]
    fn follows_a_tempo_change() {
        let mut g = Generator::default();
        let beat_at = |t: f64| t * 126.0 / 60e6;
        let out = run(&mut g, 0.0, 4e6, 126.0, beat_at, false);
        let c = clocks(&out);
        let gap = c[c.len() - 1] - c[c.len() - 2];
        assert!((gap - 60e6 / (PPQN * 126.0)).abs() < 1.0);
        assert_eq!(g.tempo(), Some(126.0));
    }

    #[test]
    fn stops_when_the_master_deck_stops_unless_kept_running() {
        for keep in [false, true] {
            let mut g = Generator::default();
            let beat_at = |t: f64| t * 120.0 / 60e6;
            let mut out = run(&mut g, 0.0, 2e6, 120.0, beat_at, keep);
            assert!(g.running());
            for i in 0..1000 {
                let now = 2e6 + i as f64 * 1000.0;
                g.poll(
                    snap(now, 0.0, 0.0),
                    true,
                    keep,
                    now,
                    now + LOOKAHEAD,
                    &mut out,
                );
            }
            let stopped = out.iter().any(|(_, m)| *m == Message::Stop);
            assert_eq!(stopped, !keep);
            assert_eq!(g.running(), keep);
            // Pulses carry on at the last tempo either way, so gear keeps
            // showing it.
            let c = clocks(&out);
            assert!(*c.last().unwrap() > 2.9e6);
            assert!((c[c.len() - 1] - c[c.len() - 2] - US_PER_PULSE_120).abs() < 1.0);
        }
    }

    #[test]
    fn playing_again_after_stop_starts_on_the_bar_without_a_gap() {
        let mut g = Generator::default();
        let mut out = run(&mut g, 0.0, 2e6, 120.0, |t| t * 120.0 / 60e6, false);
        for i in 0..1000 {
            let now = 2e6 + i as f64 * 1000.0;
            g.poll(
                snap(now, 0.0, 0.0),
                true,
                false,
                now,
                now + LOOKAHEAD,
                &mut out,
            );
        }
        assert!(!g.running());
        // Played again at 3 s from beat 2.5, now at 124 BPM: Start on beat
        // 4, 1.5 beats later.
        out.clear();
        let beat_at = |t: f64| 2.5 + (t - 3e6) * 124.0 / 60e6;
        let out = run(&mut g, 3e6, 5e6, 124.0, beat_at, false);
        let start = out.iter().position(|(_, m)| *m == Message::Start).unwrap();
        let downbeat = 3e6 + 1.5 * 60e6 / 124.0;
        assert!((out[start + 1].0 - downbeat).abs() < 1.0);
        // Free pulses up to it, none closer than half a pulse.
        let before = clocks(&out[..start]);
        assert!(before.len() > 30, "{}", before.len());
        let last = *before.last().unwrap();
        assert!(
            downbeat - last >= US_PER_PULSE_120 * 0.4,
            "{}",
            downbeat - last
        );
        assert!(g.running());
    }

    #[test]
    fn turning_it_off_sends_stop() {
        let mut g = Generator::default();
        let mut out = run(&mut g, 0.0, 1e6, 120.0, |t| t * 120.0 / 60e6, false);
        out.clear();
        g.poll(snap(1e6, 2.0, 120.0), false, false, 1e6, 1e6, &mut out);
        assert_eq!(out, vec![(1e6, Message::Stop)]);
        assert!(!g.running());
    }

    #[test]
    fn restart_lands_on_the_next_bar() {
        let mut g = Generator::default();
        let beat_at = |t: f64| t * 120.0 / 60e6;
        run(&mut g, 0.0, 2.6e6, 120.0, beat_at, false);
        g.restart();
        // At 2.6 s the deck is on beat 5.2: the next bar is beat 8, at 4 s.
        let out = run(&mut g, 2.6e6, 5e6, 120.0, beat_at, false);
        let start = out.iter().position(|(_, m)| *m == Message::Start).unwrap();
        let kinds: Vec<Message> = out[start - 2..=start + 1].iter().map(|e| e.1).collect();
        assert_eq!(
            kinds,
            [
                Message::Stop,
                Message::SongPosition(0),
                Message::Start,
                Message::Clock
            ]
        );
        assert!(
            (out[start + 1].0 - 4e6).abs() < 50.0,
            "{}",
            out[start + 1].0
        );
        // The pulses didn't pause.
        let c = clocks(&out);
        for w in c.windows(2) {
            assert!((w[1] - w[0] - US_PER_PULSE_120).abs() < 50.0);
        }
    }

    #[test]
    fn a_jump_while_waiting_for_the_bar_waits_for_the_new_one() {
        let mut g = Generator::default();
        let mut out = Vec::new();
        // On beat 1 at time 0: armed for beat 4.
        g.poll(snap(0.0, 1.0, 120.0), true, false, 0.0, 3000.0, &mut out);
        // Cued back to bar 1 (beat 0) and heard at 0.505 s: Start goes out
        // on it, not on beat 4.
        g.poll(
            snap(0.5e6 + 5000.0, 0.0, 120.0),
            true,
            false,
            0.5e6,
            0.5e6 + 3000.0,
            &mut out,
        );
        assert!(out.is_empty());
        let out = run(
            &mut g,
            0.5e6,
            3e6,
            120.0,
            |t| (t - 0.5e6 - 5000.0) * 120.0 / 60e6,
            false,
        );
        let first = clocks(&out)[0];
        assert!((first - 0.505e6).abs() < 1.0, "{first}");
    }

    /// What a recorder got: when, and the bytes.
    type Sent = Arc<Mutex<Vec<(f64, Vec<u8>)>>>;

    struct Recorder(Sent, Arc<dyn Fn() -> f64 + Send + Sync>);

    impl Sink for Recorder {
        fn send(&mut self, bytes: &[u8]) -> bool {
            self.0.lock().unwrap().push(((self.1)(), bytes.to_vec()));
            true
        }
    }

    /// The clock thread's timing on this computer: how far each pulse goes
    /// out from when it's due. Target: under 0.5 ms spread while idle.
    /// Run with `cargo test -p djsugar-engine midi_clock -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore = "timing; run by hand"]
    fn jitter() {
        let start = std::time::Instant::now();
        let clock: Arc<dyn Fn() -> f64 + Send + Sync> =
            Arc::new(move || start.elapsed().as_secs_f64() * 1e6);
        let sent = Arc::new(Mutex::new(Vec::new()));
        let mut sender = Sender {
            generator: Generator::default(),
            outputs: vec![Output {
                config: OutputConfig {
                    port: "test".into(),
                    offset_ms: 0.0,
                },
                sink: Some(Box::new(Recorder(sent.clone(), clock.clone()))),
                queue: VecDeque::new(),
            }],
            events: Vec::new(),
        };
        let tempo = 128.0;
        let mut due = Vec::new();
        while clock() < 5e6 {
            let now = clock();
            let s = snap(now, now * tempo / 60e6, tempo);
            sender.generate(s, true, false, now);
            due.extend(clocks(&sender.events));
            sender.send_due(&*clock);
            let wait = sender.idle_for(s, true, clock());
            std::thread::sleep(Duration::from_secs_f64(wait / 1e6));
        }
        let sent = sent.lock().unwrap();
        let late: Vec<f64> = sent
            .iter()
            .filter(|(_, b)| b == &[0xF8])
            .zip(&due)
            .map(|((at, _), due)| at - due)
            .collect();
        let max = late.iter().cloned().fold(f64::MIN, f64::max);
        let min = late.iter().cloned().fold(f64::MAX, f64::min);
        println!("{} pulses, late by {min:.1}..{max:.1} µs", late.len());
        assert!(max - min < 500.0, "spread {:.1} µs", max - min);
    }
}
