//! Sends the engine's mixes to output devices with cpal, as a list of
//! routes: each route takes the master,
//! headphone (cue) or booth mix, or one deck or the sampler on its own, to a
//! pair of channels on a device. Without
//! a usable device (a headless machine, CI) the engine runs on a timer
//! instead, so everything except the sound itself keeps working.
//!
//! The master route's device runs the engine in its callback. Other devices
//! are fed through ring buffers; as their clocks drift from the master
//! device's, they drop or pad a little audio to keep the delay short.
//!
//! The microphone and auxiliary inputs come in the same way, from input
//! devices: each device is opened once, with every channel its inputs take,
//! and its callback queues each input's frames for the engine (see
//! [`crate::inputs`]). Some drivers (ASIO, ALSA `hw:`) refuse a second
//! stream on a device, and two streams on one device drift apart.
//!
//! [`Health`] keeps watch: how much of each buffer the engine takes, the
//! dropouts the device reports or that show as late callbacks, and a device
//! that goes away, which is reopened once it is back.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, ErrorKind, FromSample, SampleFormat, SizedSample, SupportedBufferSize};
use djsugar_control::{ControlHandle, ControlRegistry};

use crate::engine::{Engine, MIX_CHANNELS, OUTPUT_CHANNELS};
use crate::inputs::{InputFeed, AUXILIARIES, GROOVE_BOX, MICROPHONE, NUM_AUXILIARIES, NUM_INPUTS};
use crate::rt_guard::{enter_audio_callback, exit_audio_callback};

/// What an output plays, each a stereo pair of the buffer the engine
/// renders: the mixes, or one deck or the sampler on its own (for a DAW or
/// an external mixer; what they carry is `[Master],deck_tap`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Master,
    Headphones,
    Booth,
    Deck1,
    Deck2,
    Deck3,
    Deck4,
    Sampler,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Master,
        Role::Headphones,
        Role::Booth,
        Role::Deck1,
        Role::Deck2,
        Role::Deck3,
        Role::Deck4,
        Role::Sampler,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Role::Master => "master",
            Role::Headphones => "headphones",
            Role::Booth => "booth",
            Role::Deck1 => "deck1",
            Role::Deck2 => "deck2",
            Role::Deck3 => "deck3",
            Role::Deck4 => "deck4",
            Role::Sampler => "sampler",
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }

    /// One deck or the sampler on its own, not a mix.
    pub fn is_tap(self) -> bool {
        self.source() * 2 >= MIX_CHANNELS
    }

    /// Its pair in the engine's buffer.
    fn source(self) -> usize {
        self as usize
    }
}

/// Channels the engine renders for `setup`: the mixes, and the deck and
/// sampler pairs up to the last one routed. Without deck outputs it is the
/// six of the mixes, so the engine does no tap work at all.
fn render_width(setup: &AudioSetup) -> usize {
    setup
        .routes
        .iter()
        .map(|r| 2 * r.role.source() + 2)
        .fold(MIX_CHANNELS, usize::max)
}

/// One output: a mix to channels `2 * pair + 1` and `2 * pair + 2` of the
/// device called `device` (empty for the system default).
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub role: Role,
    pub device: String,
    pub pair: u16,
}

/// The engine's inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputRole {
    /// Mono: one channel of the device, heard in both sides.
    Microphone,
    /// Stereo by default (a pair of channels), or mono. Up to eight, the
    /// Mixer deck's strips.
    Auxiliary,
    Auxiliary2,
    Auxiliary3,
    Auxiliary4,
    Auxiliary5,
    Auxiliary6,
    Auxiliary7,
    Auxiliary8,
}

impl InputRole {
    pub const ALL: [InputRole; 9] = [
        InputRole::Microphone,
        InputRole::Auxiliary,
        InputRole::Auxiliary2,
        InputRole::Auxiliary3,
        InputRole::Auxiliary4,
        InputRole::Auxiliary5,
        InputRole::Auxiliary6,
        InputRole::Auxiliary7,
        InputRole::Auxiliary8,
    ];

    /// The auxiliary inputs, in order.
    pub const AUXILIARIES: [InputRole; NUM_AUXILIARIES] = [
        InputRole::Auxiliary,
        InputRole::Auxiliary2,
        InputRole::Auxiliary3,
        InputRole::Auxiliary4,
        InputRole::Auxiliary5,
        InputRole::Auxiliary6,
        InputRole::Auxiliary7,
        InputRole::Auxiliary8,
    ];

    pub fn name(self) -> &'static str {
        match self {
            InputRole::Microphone => "microphone",
            InputRole::Auxiliary => "auxiliary",
            InputRole::Auxiliary2 => "auxiliary2",
            InputRole::Auxiliary3 => "auxiliary3",
            InputRole::Auxiliary4 => "auxiliary4",
            InputRole::Auxiliary5 => "auxiliary5",
            InputRole::Auxiliary6 => "auxiliary6",
            InputRole::Auxiliary7 => "auxiliary7",
            InputRole::Auxiliary8 => "auxiliary8",
        }
    }

    pub fn from_name(name: &str) -> Option<InputRole> {
        InputRole::ALL.into_iter().find(|r| r.name() == name)
    }

    /// Its input in the engine (see [`crate::inputs`]).
    pub fn index(self) -> usize {
        match self.aux_number() {
            Some(n) => AUXILIARIES[n - 1],
            None => MICROPHONE,
        }
    }

    /// 1 for the first auxiliary input to 8 for the last; none for the
    /// microphone.
    pub fn aux_number(self) -> Option<usize> {
        InputRole::AUXILIARIES
            .iter()
            .position(|r| *r == self)
            .map(|n| n + 1)
    }

    /// Whether it can take one channel or two (the auxiliary inputs).
    pub fn can_be_stereo(self) -> bool {
        self != InputRole::Microphone
    }
}

/// One input: from channel `channel + 1` of the device called `device`
/// (empty for the system default), and for a stereo input the channel after.
#[derive(Clone, Debug, PartialEq)]
pub struct InputRoute {
    pub role: InputRole,
    pub device: String,
    pub channel: u16,
    /// An auxiliary input taking one channel, heard in both sides. The
    /// microphone is always mono.
    pub mono: bool,
}

impl InputRoute {
    /// Channels it takes from the device.
    pub fn channels(&self) -> u16 {
        if self.mono || !self.role.can_be_stereo() {
            1
        } else {
            2
        }
    }
}

/// The routes in use, in the order the settings show them, and how the
/// devices run.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioSetup {
    pub routes: Vec<Route>,
    /// At most one for each input role; the first counts.
    pub inputs: Vec<InputRoute>,
    /// The rate to run at, if the clock device can; else the device's own.
    /// Read when the engine starts.
    pub sample_rate: Option<u32>,
    /// Frames per audio callback, within what each device allows; else the
    /// device's default. Smaller is less delay but more risk of dropouts.
    pub buffer_frames: Option<u32>,
    /// A hardware mixer does the mixing (`[Master],external_mixer`): the
    /// master and booth routes are left out, as they would be silent, so
    /// their channels are free for decks.
    pub external_mixer: bool,
}

impl AudioSetup {
    /// The routes that play: all of them, or without master and booth for
    /// an external mixer.
    pub fn in_use(&self, route: &Route) -> bool {
        !(self.external_mixer && matches!(route.role, Role::Master | Role::Booth))
    }
}

impl Default for AudioSetup {
    /// The master mix on the system's default device.
    fn default() -> Self {
        Self {
            routes: vec![Route {
                role: Role::Master,
                device: String::new(),
                pair: 0,
            }],
            inputs: Vec::new(),
            sample_rate: None,
            buffer_frames: None,
            external_mixer: false,
        }
    }
}

/// Sample rates offered in the settings.
pub const SAMPLE_RATES: [u32; 4] = [44_100, 48_000, 88_200, 96_000];
/// Buffer sizes offered in the settings, in frames.
pub const BUFFER_SIZES: [u32; 6] = [64, 128, 256, 512, 1024, 2048];

/// How the audio keeps up, as `[App],audio_latency_*` controls (for
/// the settings and controller mappings), and whether an output device went
/// away.
pub(crate) struct Health {
    /// Percent of the buffer's time the slowest callback of the last half
    /// second took to render. Over 100 the device runs dry.
    usage: ControlHandle,
    /// The highest `usage` since the outputs opened.
    peak: ControlHandle,
    /// On for half a second after a dropout.
    overload: ControlHandle,
    /// Dropouts since the start: callbacks that took longer than their
    /// buffer, came late, or that the device reported (xruns).
    overloads: ControlHandle,
    /// An output stream reported its device gone or its stream invalid.
    lost: AtomicBool,
    /// The system refused the audio thread real-time priority.
    realtime_denied: AtomicBool,
}

/// Seconds `usage` takes the worst of, and that `overload` stays on.
const HEALTH_WINDOW: f64 = 0.5;
/// Seconds after the outputs open before callbacks count: devices start
/// unevenly.
const HEALTH_WARMUP: f64 = 1.0;
/// A callback this many buffers after the one before means audio was missed.
const LATE_CALLBACK: f64 = 3.0;
/// How often a lost device is looked for.
const RETRY: Duration = Duration::from_secs(2);

impl Health {
    pub(crate) fn new(reg: &ControlRegistry) -> Arc<Self> {
        Arc::new(Self {
            usage: reg.indicator("[App]", "audio_latency_usage", 1000.0),
            peak: reg.indicator("[App]", "audio_latency_peak", 1000.0),
            overload: reg.indicator("[App]", "audio_latency_overload", 1.0),
            overloads: reg.indicator("[App]", "audio_latency_overload_count", f64::MAX),
            lost: AtomicBool::new(false),
            realtime_denied: AtomicBool::new(false),
        })
    }

    fn count_overload(&self) {
        self.overloads.set(self.overloads.get() + 1.0);
        self.overload.set_bool(true);
    }

    /// An output stream's error callback.
    fn stream_error(&self, err: &cpal::Error) {
        match err.kind() {
            // Counted, not logged: this can come from the audio thread.
            ErrorKind::Xrun => self.count_overload(),
            ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated => {
                self.lost.store(true, Ordering::Release);
                eprintln!("audio output lost: {err}");
            }
            ErrorKind::RealtimeDenied => {
                self.realtime_denied.store(true, Ordering::Relaxed);
                eprintln!("audio output: {err}");
            }
            _ => eprintln!("audio output error: {err}"),
        }
    }
}

/// Times the clock device's callbacks for [`Health`]. Lives in the callback.
struct Meter {
    health: Arc<Health>,
    /// Seconds of audio rendered since the outputs opened.
    elapsed: f64,
    last: Option<(Instant, f64)>,
    worst: f64,
    window: f64,
    overload_left: f64,
}

impl Meter {
    fn new(health: Arc<Health>) -> Self {
        Self {
            health,
            elapsed: 0.0,
            last: None,
            worst: 0.0,
            window: HEALTH_WINDOW,
            overload_left: 0.0,
        }
    }

    /// After a callback that started at `start` and rendered `period`
    /// seconds of audio.
    fn measure(&mut self, start: Instant, period: f64) {
        let now = Instant::now();
        let used = now.duration_since(start).as_secs_f64() / period;
        let late = self.last.is_some_and(|(at, before)| {
            start.duration_since(at).as_secs_f64() > LATE_CALLBACK * before.max(period)
        });
        self.last = Some((start, period));
        self.elapsed += period;
        if self.elapsed < HEALTH_WARMUP {
            return;
        }
        if used > 1.0 || late {
            self.health.count_overload();
            self.overload_left = HEALTH_WINDOW;
        } else if self.overload_left > 0.0 {
            self.overload_left -= period;
            if self.overload_left <= 0.0 {
                self.health.overload.set_bool(false);
            }
        }
        self.worst = self.worst.max(used);
        self.window -= period;
        if self.window <= 0.0 {
            let percent = (self.worst * 100.0).round();
            self.health.usage.set(percent);
            if percent > self.health.peak.get() {
                self.health.peak.set(percent);
            }
            self.worst = 0.0;
            self.window = HEALTH_WINDOW;
        }
    }
}

/// An output device and how many channels it can play.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputDevice {
    pub name: String,
    pub channels: u16,
}

/// What ASIO drivers' names start with in the device lists, next to the
/// Windows Audio (WASAPI) devices.
#[cfg(windows)]
const ASIO_PREFIX: &str = "ASIO: ";

/// The audio systems devices are listed from, each with what its devices'
/// names start with: Windows Audio (WASAPI) and, on Windows, ASIO drivers
/// ("ASIO: Focusrite USB ASIO"); elsewhere only the system's own.
fn hosts() -> Vec<(&'static str, cpal::Host)> {
    #[allow(unused_mut)]
    let mut hosts = vec![("", cpal::default_host())];
    #[cfg(windows)]
    if let Ok(asio) = cpal::host_from_id(cpal::HostId::Asio) {
        hosts.push((ASIO_PREFIX, asio));
    }
    hosts
}

/// The audio system a listed device name belongs to, and the device's own
/// name there.
fn host_of(name: &str) -> (cpal::Host, &str) {
    for (prefix, host) in hosts() {
        if let Some(rest) = name.strip_prefix(prefix).filter(|_| !prefix.is_empty()) {
            return (host, rest);
        }
    }
    (cpal::default_host(), name)
}

/// The output devices, for choosing between them.
pub fn output_devices() -> Vec<OutputDevice> {
    if no_audio() {
        return Vec::new();
    }
    let mut list: Vec<OutputDevice> = hosts()
        .into_iter()
        .filter_map(|(prefix, host)| Some((prefix, host.output_devices().ok()?)))
        .flat_map(|(prefix, devices)| devices.map(move |d| (prefix, d)))
        .map(|(prefix, d)| OutputDevice {
            name: format!("{prefix}{}", device_name(&d)),
            channels: d
                .supported_output_configs()
                .map(|c| c.map(|r| r.channels()).max().unwrap_or(2))
                .unwrap_or(2),
        })
        .collect();
    list.dedup_by(|a, b| a.name == b.name);
    list
}

/// The input devices, for choosing between them.
pub fn input_devices() -> Vec<OutputDevice> {
    if no_audio() {
        return Vec::new();
    }
    let mut list: Vec<OutputDevice> = hosts()
        .into_iter()
        .filter_map(|(prefix, host)| Some((prefix, host.input_devices().ok()?)))
        .flat_map(|(prefix, devices)| devices.map(move |d| (prefix, d)))
        .map(|(prefix, d)| OutputDevice {
            name: format!("{prefix}{}", device_name(&d)),
            channels: d
                .supported_input_configs()
                .map(|c| c.map(|r| r.channels()).max().unwrap_or(1))
                .unwrap_or(1),
        })
        .collect();
    list.dedup_by(|a, b| a.name == b.name);
    list
}

/// The input device called `name`, or the default one for an empty name.
fn find_input(name: &str) -> Option<cpal::Device> {
    if name.is_empty() {
        return cpal::default_host().default_input_device();
    }
    let (host, name) = host_of(name);
    host.input_devices().ok()?.find(|d| device_name(d) == name)
}

/// The system's default output device, by name.
pub fn default_output_device() -> Option<String> {
    if no_audio() {
        return None;
    }
    find("").map(|d| device_name(&d))
}

fn no_audio() -> bool {
    std::env::var_os("DJSUGAR_NO_AUDIO").is_some()
}

fn device_name(d: &cpal::Device) -> String {
    d.description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "audio device".into())
}

/// The device called `name`, or the default device for an empty name.
fn find(name: &str) -> Option<cpal::Device> {
    if name.is_empty() {
        return cpal::default_host().default_output_device();
    }
    let (host, name) = host_of(name);
    host.output_devices().ok()?.find(|d| device_name(d) == name)
}

/// The device whose callback runs the engine: the first master route's,
/// else the first route's, else the system default.
fn clock_device(setup: &AudioSetup) -> &str {
    let mut used = setup.routes.iter().filter(|r| setup.in_use(r));
    let first = used.clone().next();
    used.find(|r| r.role == Role::Master)
        .or(first)
        .map_or("", |r| r.device.as_str())
}

/// Whether a deck or sampler output goes to another device than the
/// master's: each device keeps its own clock, so those outputs can drift
/// apart from the master and from each other by a few samples.
pub fn deck_outputs_split(setup: &AudioSetup) -> bool {
    let clock = clock_device(setup);
    setup
        .routes
        .iter()
        .any(|r| r.role.is_tap() && r.device != clock)
}

/// The sample rate the engine runs at: the one chosen if the clock device
/// can play it, else the clock device's own.
pub(crate) fn sample_rate(setup: &AudioSetup) -> f64 {
    if no_audio() {
        return 44_100.0;
    }
    let Some(device) = find(clock_device(setup)).or_else(|| find("")) else {
        return 44_100.0;
    };
    if let Some(rate) = setup.sample_rate {
        if config_at(&device, rate as f64, 1).is_some() {
            return rate as f64;
        }
    }
    device
        .default_output_config()
        .map_or(44_100.0, |c| c.sample_rate() as f64)
}

/// A stream config at `rate` with at least `channels` channels, preferring
/// float samples and the fewest channels.
fn config_at(
    device: &cpal::Device,
    rate: f64,
    channels: u16,
) -> Option<cpal::SupportedStreamConfig> {
    let mut ranges: Vec<_> = device
        .supported_output_configs()
        .ok()?
        .filter(|r| r.channels() >= channels)
        .collect();
    ranges.sort_by_key(|r| (r.sample_format() != SampleFormat::F32, r.channels()));
    ranges
        .into_iter()
        .find_map(|r| r.try_with_sample_rate(rate as u32))
}

/// The routes going to one device: (engine pair, device pair, route index).
struct DeviceRoutes {
    name: String,
    device: cpal::Device,
    routes: Vec<(usize, usize, usize)>,
}

/// Copies `pairs` of `mix` (one frame of the engine's buffer) into `out`.
fn write_pairs(out: &mut [f32], mix: &[f32], pairs: &[(usize, usize)]) {
    out.fill(0.0);
    for &(src, dst) in pairs {
        for c in 0..2 {
            if let Some(o) = out.get_mut(2 * dst + c) {
                *o = mix[2 * src + c];
            }
        }
    }
}

struct Timer {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The running outputs. Dropping it stops them.
pub(crate) struct Outputs {
    engine: Arc<Mutex<Engine>>,
    rate: f64,
    streams: Vec<cpal::Stream>,
    timer: Option<Timer>,
    /// The clock device and rate, or why there is no sound.
    pub(crate) main_status: String,
    /// Each route's state, in the setup's order: its format, or what is wrong.
    pub(crate) route_status: Vec<String>,
    inputs: Vec<cpal::Stream>,
    /// Each open input device's channel meters (see [`Levels`]), by its
    /// name as the setup gives it.
    levels: Vec<(String, Levels)>,
    /// An input device opened only for its meters, while the input picker
    /// shows one that no input uses: its name and stream.
    watched: Option<(String, Option<cpal::Stream>)>,
    /// Each input's state, by [`InputRole::index`]: its device and format,
    /// what is wrong, or empty when it has no device.
    input_status: [String; NUM_INPUTS],
    /// Whether each open input has sent anything but silence yet. macOS
    /// sends silence to an app not allowed to use the microphone.
    heard: [Option<Arc<AtomicBool>>; NUM_INPUTS],
    /// Each open input's queue and callback size, in frames (see
    /// [`InputFeed`]): how far behind it is heard.
    delays: [Option<(Arc<AtomicUsize>, Arc<AtomicUsize>)>; NUM_INPUTS],
    health: Arc<Health>,
    /// The setup the outputs were last opened with, to reopen a lost device.
    setup: AudioSetup,
    /// When to look for a lost device next.
    retry_at: Option<Instant>,
    /// An input stream stopped (its device reset or went away): set from
    /// its error callback, so the inputs are reopened.
    input_lost: Arc<AtomicBool>,
    /// When to try reopening the inputs next.
    input_retry_at: Option<Instant>,
    /// The person was told the device went away.
    lost_reported: bool,
}

impl Outputs {
    pub(crate) fn new(engine: Engine, setup: &AudioSetup, health: Arc<Health>) -> Self {
        let rate = engine.sample_rate();
        let mut o = Self {
            // The callbacks only ever try-lock, so they never wait.
            engine: Arc::new(Mutex::new(engine)),
            rate,
            streams: Vec::new(),
            timer: None,
            main_status: String::new(),
            route_status: Vec::new(),
            inputs: Vec::new(),
            levels: Vec::new(),
            watched: None,
            input_status: Default::default(),
            heard: Default::default(),
            delays: Default::default(),
            health,
            setup: setup.clone(),
            retry_at: None,
            input_lost: Arc::new(AtomicBool::new(false)),
            input_retry_at: None,
            lost_reported: false,
        };
        o.configure(setup);
        o
    }

    /// Opens the devices `setup` names, closing the ones in use.
    pub(crate) fn configure(&mut self, setup: &AudioSetup) {
        self.configure_outputs(setup);
        self.configure_inputs(setup);
    }

    /// Opens the input devices `setup` names, closing the ones in use, and
    /// leaves the outputs as they are.
    pub(crate) fn configure_inputs(&mut self, setup: &AudioSetup) {
        // Stop the old streams before taking their queues from the engine.
        self.inputs.clear();
        self.levels.clear();
        self.heard = Default::default();
        self.delays = Default::default();
        self.input_lost.store(false, Ordering::Release);
        // Kept for reopening them after their device resets.
        self.setup.inputs = setup.inputs.clone();
        let mut feeds: [Option<InputFeed>; NUM_INPUTS] = Default::default();
        self.input_status = Default::default();
        // Strips playing the Mixer deck's Groove Box open no device.
        let mut grooves = [false; NUM_INPUTS];
        for r in setup.inputs.iter().filter(|r| r.device == GROOVE_BOX) {
            grooves[r.role.index()] = true;
            self.input_status[r.role.index()] = GROOVE_BOX.to_string();
        }
        for (device, routes) in group_inputs(&setup.inputs) {
            if no_audio() {
                for r in routes {
                    self.input_status[r.role.index()] = format!("{device} unavailable");
                }
                continue;
            }
            for (i, opened) in self.open_inputs(&device, &routes, setup.buffer_frames) {
                match opened {
                    Ok((feed, status, heard)) => {
                        self.delays[i] = Some((feed.queued.clone(), feed.period.clone()));
                        feeds[i] = Some(feed);
                        self.heard[i] = Some(heard);
                        self.input_status[i] = status;
                    }
                    Err(e) => self.input_status[i] = e,
                }
            }
        }
        let mut old = Vec::new();
        if let Ok(mut engine) = self.engine.lock() {
            for (i, feed) in feeds.into_iter().enumerate() {
                engine.set_input_groove(i, grooves[i]);
                old.push(engine.set_input(i, feed));
            }
        }
        drop(old);
        // A device watched for its meters that the inputs now open is
        // metered by their stream instead.
        let watched = self.watched.take().map(|(name, _)| name);
        self.watch_input(watched.as_deref());
    }

    /// Each input's state, by [`InputRole::index`]: its device and format,
    /// what is wrong, or empty when it has no device.
    pub(crate) fn input_status(&self) -> Vec<String> {
        self.input_status
            .iter()
            .zip(&self.heard)
            .map(|(status, heard)| match heard {
                Some(h) if !h.load(Ordering::Relaxed) => format!(
                    "{status} · silent so far{}",
                    if cfg!(target_os = "macos") {
                        ": allow DJ Sugar in System Settings › Privacy & Security › Microphone"
                    } else {
                        ""
                    }
                ),
                _ => status.clone(),
            })
            .collect()
    }

    /// How far behind each input is heard, in milliseconds, by
    /// [`InputRole::index`]: what waits in its queue plus one callback of
    /// the device. `None` for an input that isn't open.
    pub(crate) fn input_delays(&self) -> Vec<Option<f64>> {
        self.delays
            .iter()
            .map(|d| {
                d.as_ref().map(|(queued, period)| {
                    let frames = queued.load(Ordering::Relaxed) + period.load(Ordering::Relaxed);
                    frames as f64 * 1000.0 / self.rate
                })
            })
            .collect()
    }

    /// The peak of each channel of each open input device since the last
    /// call, by the device's name as the setup gives it (empty for the
    /// system default): for the input picker's meters.
    pub(crate) fn input_levels(&self) -> Vec<(String, Vec<f32>)> {
        self.levels
            .iter()
            .map(|(name, levels)| (name.clone(), levels.take()))
            .collect()
    }

    /// Meters the input device called `device` (empty for the system
    /// default) while the input picker shows it, opening it for that if no
    /// input uses it; `None` closes it again.
    pub(crate) fn watch_input(&mut self, device: Option<&str>) {
        let Some(device) = device else {
            self.drop_watched();
            return;
        };
        if self.watched.as_ref().is_some_and(|(d, _)| d == device) {
            return;
        }
        self.drop_watched();
        let open = self.levels.iter().any(|(d, _)| d == device);
        let stream = if open || no_audio() {
            None
        } else {
            self.open_meter(device)
        };
        self.watched = Some((device.to_string(), stream));
    }

    fn drop_watched(&mut self) {
        if let Some((name, Some(_))) = self.watched.take() {
            self.levels.retain(|(d, _)| *d != name);
        }
    }

    /// Opens `device` with all its channels just for their meters.
    fn open_meter(&mut self, device: &str) -> Option<cpal::Stream> {
        let dev = find_input(device)?;
        let config = input_config_wide(&dev, self.rate, 1)?;
        let channels = config.channels() as usize;
        let levels = Levels::new(channels);
        // Its stream stopping is no reason to reopen the inputs.
        let lost = Arc::new(AtomicBool::new(false));
        let stream = input_stream(&dev, &config, self.setup.buffer_frames, &lost, {
            let levels = levels.clone();
            move |data: &[f32]| levels.meter(channels, data)
        })
        .map_err(|e| eprintln!("audio input {device}: {e}"))
        .ok()?;
        self.levels.push((device.to_string(), levels));
        Some(stream)
    }

    /// Opens the input device called `device` once for all of `routes`,
    /// with buffers of `frames` as the outputs have (else the device's
    /// default), and fans its channels out to one queue per input. Returns,
    /// for each input by [`InputRole::index`], its queue, status and whether
    /// it has sent anything but silence yet, or what is wrong.
    #[allow(clippy::type_complexity)]
    fn open_inputs(
        &mut self,
        device: &str,
        routes: &[InputRoute],
        frames: Option<u32>,
    ) -> Vec<(usize, Result<(InputFeed, String, Arc<AtomicBool>), String>)> {
        let label = if device.is_empty() {
            "System default"
        } else {
            device
        };
        let Some(dev) = find_input(device) else {
            return routes
                .iter()
                .map(|r| (r.role.index(), Err(format!("{label} unavailable"))))
                .collect();
        };
        let name = device_name(&dev);
        let khz = self.rate / 1000.0;
        // Inputs past the device's channels at this rate can't open; the
        // others still do.
        let mut out = Vec::new();
        let mut fits = Vec::new();
        for r in routes {
            if input_config_at(&dev, self.rate, r.channel + r.channels()).is_some() {
                fits.push(r);
            } else {
                out.push((
                    r.role.index(),
                    Err(format!(
                        "{name} can't record {} at {khz} kHz",
                        channel_names(r.channel, r.channels())
                    )),
                ));
            }
        }
        let needed = fits.iter().map(|r| r.channel + r.channels()).max();
        let Some(config) = needed.and_then(|n| input_config_wide(&dev, self.rate, n)) else {
            return out;
        };
        let period = Arc::new(AtomicUsize::new(0));
        let mut taps = Vec::new();
        let mut opened = Vec::new();
        for r in &fits {
            // Half a second of room; the engine keeps it much emptier.
            let (tx, rx) = rtrb::RingBuffer::<f32>::new((self.rate * 0.5) as usize * 2);
            let heard = Arc::new(AtomicBool::new(false));
            taps.push(Tap {
                first: r.channel as usize,
                stereo: r.channels() == 2,
                tx,
                heard: heard.clone(),
            });
            let status = format!(
                "{name} · {} · {khz} kHz",
                channel_names(r.channel, r.channels())
            );
            let feed = InputFeed {
                rx,
                period: period.clone(),
                queued: Arc::default(),
            };
            opened.push((r.role.index(), feed, status, heard));
        }
        let channels = config.channels() as usize;
        let levels = Levels::new(channels);
        let stream = input_stream(&dev, &config, frames, &self.input_lost, {
            let levels = levels.clone();
            move |data: &[f32]| {
                period.store(data.len() / channels, Ordering::Relaxed);
                fan_out(&mut taps, channels, data);
                levels.meter(channels, data);
            }
        });
        match stream {
            Ok(stream) => {
                self.inputs.push(stream);
                self.levels.push((device.to_string(), levels));
                out.extend(
                    opened
                        .into_iter()
                        .map(|(i, feed, status, heard)| (i, Ok((feed, status, heard)))),
                );
            }
            Err(e) => out.extend(opened.into_iter().map(|(i, ..)| (i, Err(e.clone())))),
        }
        out
    }

    /// Reopens the outputs after a device went away or its stream stopped,
    /// once the clock device is there again. Call it every so often; it says
    /// what happened, for the person at the decks.
    pub(crate) fn recover(&mut self) -> Option<String> {
        if !self.health.lost.load(Ordering::Acquire) {
            return self.recover_inputs();
        }
        let now = Instant::now();
        if self.retry_at.is_some_and(|t| now < t) {
            return None;
        }
        self.retry_at = Some(now + RETRY);
        let name = clock_device(&self.setup).to_string();
        let label = if name.is_empty() {
            "The system's audio output".to_string()
        } else {
            name.clone()
        };
        if find(&name).is_none() {
            if std::mem::replace(&mut self.lost_reported, true) {
                return None;
            }
            // The engine ran in the lost stream's callback. Run it on the
            // timer instead, so the app keeps answering (load, seek, cue)
            // while the decks stand still: frozen, a playing deck's
            // waveform jumps back and forth as the screen guesses ahead.
            self.streams.clear();
            if let Ok(mut engine) = self.engine.lock() {
                engine.pause_decks();
            }
            if self.timer.is_none() {
                self.timer = Some(self.start_timer());
            }
            self.main_status = format!("{label} went away");
            return Some(format!(
                "{label} went away, so the decks stopped. DJ Sugar plays there again \
                 once it is back, or pick another in Settings › Audio."
            ));
        }
        self.health.lost.store(false, Ordering::Release);
        let setup = self.setup.clone();
        self.configure_outputs(&setup);
        // The device's inputs went with it (an interface that resets takes
        // both): open them again too, or nothing comes in until the input
        // is picked again.
        self.configure_inputs(&setup);
        Some(if std::mem::take(&mut self.lost_reported) {
            format!("{label} is back.")
        } else {
            format!("{label} stopped; DJ Sugar reopened it.")
        })
    }

    /// Reopens the inputs after one of their streams stopped, every so often
    /// until they open again.
    fn recover_inputs(&mut self) -> Option<String> {
        if !self.input_lost.load(Ordering::Acquire) {
            return None;
        }
        let now = Instant::now();
        if self.input_retry_at.is_some_and(|t| now < t) {
            return None;
        }
        self.input_retry_at = Some(now + RETRY);
        let setup = self.setup.clone();
        self.configure_inputs(&setup);
        let wanted: usize = group_inputs(&setup.inputs)
            .iter()
            .map(|(_, routes)| routes.len())
            .sum();
        let open = self.heard.iter().filter(|h| h.is_some()).count();
        if open < wanted {
            // Not back yet: try again in a while.
            self.input_lost.store(true, Ordering::Release);
            return None;
        }
        self.input_retry_at = None;
        Some("An audio input stopped; DJ Sugar reopened it.".to_string())
    }

    /// Whether the system refused the audio thread real-time priority.
    pub(crate) fn realtime_denied(&self) -> bool {
        self.health.realtime_denied.load(Ordering::Relaxed)
    }

    fn configure_outputs(&mut self, setup: &AudioSetup) {
        self.streams.clear();
        self.timer = None;
        self.setup = setup.clone();
        self.health.lost.store(false, Ordering::Release);
        self.lost_reported = false;
        self.retry_at = None;
        self.route_status = vec![String::new(); setup.routes.len()];
        if let Err(e) = self.open(setup) {
            eprintln!("audio output unavailable ({e}); running without sound");
            self.streams.clear();
            self.timer = Some(self.start_timer());
            self.main_status = if no_audio() {
                "No audio output".into()
            } else {
                e
            };
            for s in &mut self.route_status {
                if s.is_empty() || !s.contains("unavailable") {
                    *s = self.main_status.clone();
                }
            }
        }
    }

    fn open(&mut self, setup: &AudioSetup) -> Result<(), String> {
        if no_audio() {
            return Err("DJSUGAR_NO_AUDIO is set".into());
        }
        let khz = self.rate / 1000.0;
        let buffer = setup.buffer_frames;
        let clock = find(clock_device(setup))
            .or_else(|| find(""))
            .ok_or("no output device")?;
        let clock_name = device_name(&clock);

        // Group the routes by device, the clock device first. A route to
        // channels another route already uses is left out.
        let mut devices = vec![DeviceRoutes {
            name: clock_name.clone(),
            device: clock,
            routes: Vec::new(),
        }];
        for (i, r) in setup.routes.iter().enumerate() {
            if !setup.in_use(r) {
                self.route_status[i] = "Not used with an external mixer".into();
                continue;
            }
            let name = if r.device.is_empty() {
                find("").map(|d| device_name(&d)).unwrap_or_default()
            } else {
                r.device.clone()
            };
            let at = match devices.iter().position(|d| d.name == name) {
                Some(at) => at,
                None => match find(&name) {
                    Some(device) => {
                        devices.push(DeviceRoutes {
                            name: name.clone(),
                            device,
                            routes: Vec::new(),
                        });
                        devices.len() - 1
                    }
                    None => {
                        self.route_status[i] = format!("{name} unavailable");
                        continue;
                    }
                },
            };
            let pair = r.pair as usize;
            if let Some(&(_, _, other)) = devices[at].routes.iter().find(|x| x.1 == pair) {
                self.route_status[i] =
                    format!("same channels as {}", setup.routes[other].role.name());
                continue;
            }
            devices[at].routes.push((r.role.source(), pair, i));
        }

        // Open the other devices first, so the clock knows where to feed.
        let mut feeds = Vec::new();
        for d in devices.drain(1..).collect::<Vec<_>>() {
            if d.routes.is_empty() {
                continue;
            }
            let needed = d
                .routes
                .iter()
                .map(|r| r.1 as u16 * 2 + 2)
                .max()
                .unwrap_or(2);
            let Some(config) = config_at(&d.device, self.rate, needed) else {
                for &(_, pair, i) in &d.routes {
                    self.route_status[i] = format!(
                        "{} unavailable at {khz} kHz with ch {} & {}",
                        d.name,
                        2 * pair + 1,
                        2 * pair + 2
                    );
                }
                continue;
            };
            let channels = config.channels() as usize;
            let width = d.routes.len() * 2;
            // A quarter second of audio in flight at most.
            let (tx, mut rx) = rtrb::RingBuffer::<f32>::new((self.rate * 0.5) as usize * width);
            let pairs: Vec<(usize, usize)> =
                d.routes.iter().enumerate().map(|(k, r)| (k, r.1)).collect();
            let mut frame = [0.0f32; OUTPUT_CHANNELS];
            match stream(
                &d.device,
                &config,
                buffer,
                &self.health,
                move |data: &mut [f32]| {
                    let frames = data.len() / channels;
                    // Keep at most two buffers queued: drop the oldest beyond that.
                    let excess = rx.slots().saturating_sub(frames * width * 2) / width * width;
                    for _ in 0..excess {
                        let _ = rx.pop();
                    }
                    for out in data.chunks_exact_mut(channels) {
                        if rx.slots() >= width {
                            for v in &mut frame[..width] {
                                *v = rx.pop().unwrap_or(0.0);
                            }
                            write_pairs(out, &frame, &pairs);
                        } else {
                            out.fill(0.0);
                        }
                    }
                },
            ) {
                Ok(s) => {
                    self.streams.push(s);
                    for &(_, _, i) in &d.routes {
                        self.route_status[i] = format!("{khz} kHz · {channels} out");
                    }
                    let sources: Vec<usize> = d.routes.iter().map(|r| r.0).collect();
                    feeds.push((tx, sources));
                }
                Err(e) => {
                    for &(_, _, i) in &d.routes {
                        self.route_status[i] = format!("{}: {e}", d.name);
                    }
                }
            }
        }

        // The clock device runs the engine and feeds the others.
        let d = devices.pop().expect("clock device");
        let needed = d
            .routes
            .iter()
            .map(|r| r.1 as u16 * 2 + 2)
            .max()
            .unwrap_or(2);
        let config = config_at(&d.device, self.rate, needed)
            .or_else(|| config_at(&d.device, self.rate, 1))
            .ok_or_else(|| format!("{clock_name} can't play at {khz} kHz"))?;
        let channels = config.channels() as usize;
        let pairs: Vec<(usize, usize)> = d.routes.iter().map(|r| (r.0, r.1)).collect();
        for &(_, pair, i) in &d.routes {
            self.route_status[i] = if 2 * pair + 2 <= channels {
                format!("{khz} kHz · {channels} out")
            } else {
                format!("{clock_name} has no ch {} & {}", 2 * pair + 1, 2 * pair + 2)
            };
        }
        let engine = self.engine.clone();
        let width = render_width(setup);
        let mut mix = vec![0.0f32; 16_384 * width];
        let mut feeds = feeds;
        let mut meter = Meter::new(self.health.clone());
        let rate = self.rate;
        self.streams.push(stream(
            &d.device,
            &config,
            buffer,
            &self.health,
            move |data: &mut [f32]| {
                let start = Instant::now();
                let frames = data.len() / channels;
                if mix.len() < frames * width {
                    mix.resize(frames * width, 0.0);
                }
                let mix = &mut mix[..frames * width];
                match engine.try_lock() {
                    Ok(mut e) => {
                        enter_audio_callback();
                        e.process(mix, width);
                        exit_audio_callback();
                    }
                    Err(_) => mix.fill(0.0),
                }
                for (out, m) in data.chunks_exact_mut(channels).zip(mix.chunks_exact(width)) {
                    write_pairs(out, m, &pairs);
                    for (tx, sources) in &mut feeds {
                        if tx.slots() >= sources.len() * 2 {
                            for &s in sources.iter() {
                                let _ = tx.push(m[2 * s]);
                                let _ = tx.push(m[2 * s + 1]);
                            }
                        }
                    }
                }
                if frames > 0 {
                    meter.measure(start, frames as f64 / rate);
                }
            },
        )?);
        self.main_status = match buffer_frames(&config, buffer) {
            BufferSize::Fixed(n) => format!(
                "{clock_name} · {khz} kHz · {n} samples ({:.1} ms)",
                n as f64 / self.rate * 1000.0
            ),
            BufferSize::Default => format!("{clock_name} · {khz} kHz"),
        };
        Ok(())
    }

    fn start_timer(&self) -> Timer {
        let running = Arc::new(AtomicBool::new(true));
        let engine = self.engine.clone();
        let rate = self.rate;
        let thread = {
            let running = running.clone();
            std::thread::Builder::new()
                .name("engine-timer".into())
                .spawn(move || {
                    const FRAMES: usize = 512;
                    let mut buf = vec![0.0f32; FRAMES * 2];
                    let period = Duration::from_secs_f64(FRAMES as f64 / rate);
                    let mut next = Instant::now();
                    while running.load(Ordering::Relaxed) {
                        if let Ok(mut e) = engine.lock() {
                            enter_audio_callback();
                            e.process(&mut buf, 2);
                            exit_audio_callback();
                        }
                        next += period;
                        if let Some(wait) = next.checked_duration_since(Instant::now()) {
                            std::thread::sleep(wait);
                        } else {
                            next = Instant::now();
                        }
                    }
                })
                .expect("spawn engine timer")
        };
        Timer {
            running,
            thread: Some(thread),
        }
    }
}

/// The buffer size to ask `config`'s device for: `frames` within its range.
fn buffer_frames(config: &cpal::SupportedStreamConfig, frames: Option<u32>) -> BufferSize {
    match (frames, config.buffer_size()) {
        (None, _) => BufferSize::Default,
        (Some(n), &SupportedBufferSize::Range { min, max }) => BufferSize::Fixed(n.clamp(min, max)),
        (Some(n), SupportedBufferSize::Unknown) => BufferSize::Fixed(n),
    }
}

/// The inputs in use (the first route for each role), grouped by device,
/// so each device is opened once.
fn group_inputs(inputs: &[InputRoute]) -> Vec<(String, Vec<InputRoute>)> {
    let mut groups: Vec<(String, Vec<InputRoute>)> = Vec::new();
    for role in InputRole::ALL {
        // A strip playing the Groove Box opens no device.
        let Some(r) = inputs
            .iter()
            .find(|r| r.role == role && r.device != GROOVE_BOX)
        else {
            continue;
        };
        match groups.iter_mut().find(|(d, _)| *d == r.device) {
            Some((_, routes)) => routes.push(r.clone()),
            None => groups.push((r.device.clone(), vec![r.clone()])),
        }
    }
    groups
}

/// One input's share of a device's stream: its channel (and the next, for
/// stereo) into its queue for the engine.
struct Tap {
    first: usize,
    stereo: bool,
    tx: rtrb::Producer<f32>,
    /// Set once its channels carry anything but silence.
    heard: Arc<AtomicBool>,
}

/// Queues each tap's frames from `data`, `channels` interleaved, as stereo
/// pairs (a mono input is heard in both sides). A full queue drops the rest
/// of the callback for that input only.
fn fan_out(taps: &mut [Tap], channels: usize, data: &[f32]) {
    for tap in taps {
        let width = if tap.stereo { 2 } else { 1 };
        if !tap.heard.load(Ordering::Relaxed)
            && data
                .chunks_exact(channels)
                .any(|f| f[tap.first..tap.first + width].iter().any(|v| *v != 0.0))
        {
            tap.heard.store(true, Ordering::Relaxed);
        }
        for frame in data.chunks_exact(channels) {
            if tap.tx.slots() < 2 {
                break;
            }
            let l = frame[tap.first];
            let r = if tap.stereo { frame[tap.first + 1] } else { l };
            let _ = tap.tx.push(l);
            let _ = tap.tx.push(r);
        }
    }
}

/// "In 1", or "In 1 & 2" for a pair.
pub fn channel_names(first: u16, count: u16) -> String {
    if count >= 2 {
        format!("In {} & {}", first + 1, first + 2)
    } else {
        format!("In {}", first + 1)
    }
}

/// Each channel's peak on an input device since it was last read, shared
/// between the device's callback, which raises them, and the app, which
/// takes them. Held as f32 bits: for levels of 0 and up, their order as
/// integers is their order as numbers, so `fetch_max` works on them.
#[derive(Clone)]
struct Levels(Arc<[AtomicU32]>);

impl Levels {
    fn new(channels: usize) -> Self {
        Self((0..channels).map(|_| AtomicU32::new(0)).collect())
    }

    /// Raises each channel's peak to the loudest of `data`, `channels`
    /// interleaved.
    fn meter(&self, channels: usize, data: &[f32]) {
        let mut peaks = [0.0f32; 64];
        let n = channels.min(self.0.len()).min(peaks.len());
        for frame in data.chunks_exact(channels) {
            for (p, v) in peaks[..n].iter_mut().zip(frame) {
                *p = p.max(v.abs());
            }
        }
        for (level, p) in self.0.iter().zip(&peaks[..n]) {
            if p.is_finite() {
                level.fetch_max(p.to_bits(), Ordering::Relaxed);
            }
        }
    }

    /// Each channel's peak since the last time, starting over.
    fn take(&self) -> Vec<f32> {
        self.0
            .iter()
            .map(|l| f32::from_bits(l.swap(0, Ordering::Relaxed)))
            .collect()
    }
}

/// An input stream config at `rate` with at least `channels` channels, and
/// all the device's own if it has more (as many as its default config has),
/// so the input picker can meter every one.
fn input_config_wide(
    device: &cpal::Device,
    rate: f64,
    channels: u16,
) -> Option<cpal::SupportedStreamConfig> {
    let own = device.default_input_config().map_or(0, |c| c.channels());
    input_config_at(device, rate, wide_channels(own, channels))
        .or_else(|| input_config_at(device, rate, channels))
}

/// The channels to open a device with: those the inputs need, or the
/// device's own (`own`, 0 if unknown) when it has more, up to 64.
fn wide_channels(own: u16, needed: u16) -> u16 {
    own.min(64).max(needed)
}

/// An input stream config at `rate` with at least `channels` channels,
/// preferring float samples and the fewest channels.
fn input_config_at(
    device: &cpal::Device,
    rate: f64,
    channels: u16,
) -> Option<cpal::SupportedStreamConfig> {
    let mut ranges: Vec<_> = device
        .supported_input_configs()
        .ok()?
        .filter(|r| r.channels() >= channels)
        .collect();
    ranges.sort_by_key(|r| (r.sample_format() != SampleFormat::F32, r.channels()));
    ranges
        .into_iter()
        .find_map(|r| r.try_with_sample_rate(rate as u32))
}

/// Opens and starts an input stream that hands `take` its audio in f32,
/// interleaved, with buffers of `frames` if the device takes them (the
/// outputs' size: a bigger input buffer is heard as delay), else its
/// default. `lost` is set if the stream stops.
fn input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    frames: Option<u32>,
    lost: &Arc<AtomicBool>,
    take: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, String> {
    let take = Arc::new(Mutex::new(take));
    let open = |size: BufferSize| {
        let take = take.clone();
        // Only this stream's callback locks it, so it never waits.
        let take = move |data: &[f32]| {
            if let Ok(mut t) = take.try_lock() {
                t(data)
            }
        };
        let lost = lost.clone();
        let error = move |err: cpal::Error| match err.kind() {
            ErrorKind::Xrun => {}
            ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated => {
                lost.store(true, Ordering::Release);
                eprintln!("audio input lost: {err}");
            }
            _ => eprintln!("audio input error: {err}"),
        };
        match config.sample_format() {
            SampleFormat::F32 => build_input::<f32>(device, config, size, take, error),
            SampleFormat::I16 => build_input::<i16>(device, config, size, take, error),
            SampleFormat::I32 => build_input::<i32>(device, config, size, take, error),
            SampleFormat::U16 => build_input::<u16>(device, config, size, take, error),
            other => Err(format!("unsupported sample format {other:?}")),
        }
    };
    let size = buffer_frames(config, frames);
    let stream = match open(size) {
        Err(e) if size != BufferSize::Default => {
            eprintln!("audio input: {size:?} refused ({e}); using the device's default");
            open(BufferSize::Default)
        }
        other => other,
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn build_input<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    buffer_size: BufferSize,
    mut take: impl FnMut(&[f32]) + Send + 'static,
    error: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels() as usize;
    let mut scratch = vec![0.0f32; 16_384 * channels];
    device
        .build_input_stream::<T, _, _>(
            cpal::StreamConfig {
                buffer_size,
                ..config.config()
            },
            move |data: &[T], _| {
                if scratch.len() < data.len() {
                    // Rare: a device delivering more than asked for.
                    scratch.resize(data.len(), 0.0);
                }
                let buf = &mut scratch[..data.len()];
                for (s, d) in buf.iter_mut().zip(data) {
                    *s = <f32 as FromSample<T>>::from_sample_(*d);
                }
                take(buf);
            },
            error,
            None,
        )
        .map_err(|e| e.to_string())
}

/// Opens and starts an output stream that `fill` renders in f32, with
/// buffers of `frames` if the device takes them, else its default.
fn stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    frames: Option<u32>,
    health: &Arc<Health>,
    fill: impl FnMut(&mut [f32]) + Send + 'static,
) -> Result<cpal::Stream, String> {
    let fill = Arc::new(Mutex::new(fill));
    let open = |size: BufferSize| {
        let fill = fill.clone();
        // Only this stream's callback locks it, so it never waits.
        let fill = move |data: &mut [f32]| {
            if let Ok(mut f) = fill.try_lock() {
                f(data)
            }
        };
        let health = health.clone();
        let error = move |err: cpal::Error| health.stream_error(&err);
        match config.sample_format() {
            SampleFormat::F32 => build::<f32>(device, config, size, fill, error),
            SampleFormat::I16 => build::<i16>(device, config, size, fill, error),
            SampleFormat::I32 => build::<i32>(device, config, size, fill, error),
            SampleFormat::U16 => build::<u16>(device, config, size, fill, error),
            other => Err(format!("unsupported sample format {other:?}")),
        }
    };
    let size = buffer_frames(config, frames);
    let stream = match open(size) {
        Err(e) if size != BufferSize::Default => {
            eprintln!("audio output: {size:?} refused ({e}); using the device's default");
            open(BufferSize::Default)
        }
        other => other,
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    buffer_size: BufferSize,
    mut fill: impl FnMut(&mut [f32]) + Send + 'static,
    error: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    // Scratch buffer, sized generously up front so the callback does not allocate.
    let channels = config.channels() as usize;
    let mut scratch = vec![0.0f32; 16_384 * channels];
    device
        .build_output_stream::<T, _, _>(
            cpal::StreamConfig {
                buffer_size,
                ..config.config()
            },
            move |data: &mut [T], _| {
                if scratch.len() < data.len() {
                    scratch.resize(data.len(), 0.0);
                }
                let buf = &mut scratch[..data.len()];
                fill(buf);
                for (d, s) in data.iter_mut().zip(buf.iter()) {
                    *d = T::from_sample(*s);
                }
            },
            error,
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_route(role: InputRole, device: &str, channel: u16, mono: bool) -> InputRoute {
        InputRoute {
            role,
            device: device.to_string(),
            channel,
            mono,
        }
    }

    fn tap(first: usize, stereo: bool) -> (Tap, rtrb::Consumer<f32>) {
        let (tx, rx) = rtrb::RingBuffer::new(64);
        let heard = Arc::new(AtomicBool::new(false));
        (
            Tap {
                first,
                stereo,
                tx,
                heard,
            },
            rx,
        )
    }

    fn drain(rx: &mut rtrb::Consumer<f32>) -> Vec<f32> {
        std::iter::from_fn(|| rx.pop().ok()).collect()
    }

    /// Two frames of a four-channel device: channel c of frame f is
    /// `f * 10 + c + 1`.
    const FOUR: [f32; 8] = [1.0, 2.0, 3.0, 4.0, 11.0, 12.0, 13.0, 14.0];

    #[test]
    fn a_mono_input_hears_its_channel_in_both_sides() {
        let (t, mut rx) = tap(2, false);
        let mut taps = [t];
        fan_out(&mut taps, 4, &FOUR);
        assert_eq!(drain(&mut rx), [3.0, 3.0, 13.0, 13.0]);
        assert!(taps[0].heard.load(Ordering::Relaxed));
    }

    #[test]
    fn a_stereo_input_takes_its_pair() {
        let (t, mut rx) = tap(0, true);
        fan_out(&mut [t], 4, &FOUR);
        assert_eq!(drain(&mut rx), [1.0, 2.0, 11.0, 12.0]);
    }

    #[test]
    fn two_inputs_share_one_stream_of_their_device() {
        let (a, mut ra) = tap(0, true);
        let (b, mut rb) = tap(3, false);
        let mut taps = [a, b];
        fan_out(&mut taps, 4, &FOUR);
        assert_eq!(drain(&mut ra), [1.0, 2.0, 11.0, 12.0]);
        assert_eq!(drain(&mut rb), [4.0, 4.0, 14.0, 14.0]);

        // Silence on one input's channel leaves it unheard.
        let (c, _rc) = tap(0, false);
        let (d, _rd) = tap(1, false);
        let mut taps = [c, d];
        fan_out(&mut taps, 2, &[0.0, 0.5, 0.0, 0.5]);
        assert!(!taps[0].heard.load(Ordering::Relaxed));
        assert!(taps[1].heard.load(Ordering::Relaxed));
    }

    #[test]
    fn a_full_queue_drops_only_its_own_input() {
        let (tx, mut full) = rtrb::RingBuffer::new(2);
        let a = Tap {
            first: 0,
            stereo: true,
            tx,
            heard: Arc::new(AtomicBool::new(false)),
        };
        let (b, mut rb) = tap(2, true);
        fan_out(&mut [a, b], 4, &FOUR);
        assert_eq!(drain(&mut full), [1.0, 2.0]);
        assert_eq!(drain(&mut rb), [3.0, 4.0, 13.0, 14.0]);
    }

    #[test]
    fn eight_auxiliary_inputs_by_name_and_number() {
        assert_eq!(InputRole::AUXILIARIES.len(), 8);
        for (n, role) in (1..).zip(InputRole::AUXILIARIES) {
            assert_eq!(role.aux_number(), Some(n));
            assert_eq!(InputRole::from_name(role.name()), Some(role));
            assert_eq!(role.index(), AUXILIARIES[n - 1]);
            assert!(role.can_be_stereo());
        }
        // Saved names from before stay the same.
        assert_eq!(InputRole::Auxiliary.name(), "auxiliary");
        assert_eq!(InputRole::Auxiliary3.name(), "auxiliary3");
        assert_eq!(InputRole::Microphone.aux_number(), None);
        assert_eq!(InputRole::Microphone.index(), MICROPHONE);
    }

    #[test]
    fn each_channel_of_an_input_device_has_a_meter() {
        let levels = Levels::new(4);
        // Two frames of four channels; channel 3 is silent.
        levels.meter(4, &[0.1, -0.5, 0.0, 0.2, 0.3, 0.25, 0.0, -0.9]);
        levels.meter(4, &[0.05, 0.0, 0.0, 0.0]);
        assert_eq!(levels.take(), [0.3, 0.5, 0.0, 0.9]);
        // Read, they start over.
        assert_eq!(levels.take(), [0.0; 4]);
        // A clipping channel reads over 1; garbage doesn't stick.
        levels.meter(4, &[1.5, f32::NAN, 0.0, 0.0]);
        let got = levels.take();
        assert_eq!(got[0], 1.5);
        assert_eq!(got[1], 0.0);
    }

    #[test]
    fn input_devices_open_with_all_their_channels() {
        // A four-input interface opens with all four, for the meters, even
        // when one stereo input on 1/2 is all that plays.
        assert_eq!(wide_channels(4, 2), 4);
        // An input past what the default config says still gets its own.
        assert_eq!(wide_channels(2, 6), 6);
        // Unknown, or a driver offering hundreds: what the inputs need, or
        // at most 64.
        assert_eq!(wide_channels(0, 2), 2);
        assert_eq!(wide_channels(512, 2), 64);
    }

    #[test]
    fn inputs_open_each_device_once() {
        // The saved Aux 1 to 3 routes, as before: two on the M4, one on a
        // second interface, and the microphone on the system default.
        let inputs = [
            input_route(InputRole::Auxiliary, "M4", 0, false),
            input_route(InputRole::Auxiliary3, "Other", 0, true),
            input_route(InputRole::Auxiliary2, "M4", 2, true),
            input_route(InputRole::Microphone, "", 3, true),
            // A second route for a role doesn't count.
            input_route(InputRole::Auxiliary, "Other", 1, false),
        ];
        let groups = group_inputs(&inputs);
        let summary: Vec<(&str, Vec<(InputRole, u16)>)> = groups
            .iter()
            .map(|(d, rs)| (d.as_str(), rs.iter().map(|r| (r.role, r.channel)).collect()))
            .collect();
        assert_eq!(
            summary,
            [
                ("", vec![(InputRole::Microphone, 3)]),
                (
                    "M4",
                    vec![(InputRole::Auxiliary, 0), (InputRole::Auxiliary2, 2)]
                ),
                ("Other", vec![(InputRole::Auxiliary3, 0)]),
            ]
        );
    }

    /// A meter with `seconds` of callbacks behind it, each `period` long
    /// and taking `share` of it.
    fn metered(seconds: f64, period: f64, share: f64) -> (Arc<ControlRegistry>, Meter) {
        let reg = ControlRegistry::new();
        let mut meter = Meter::new(Health::new(&reg));
        for _ in 0..(seconds / period) as usize {
            meter.measure(
                Instant::now() - Duration::from_secs_f64(period * share),
                period,
            );
        }
        (reg, meter)
    }

    fn app(reg: &ControlRegistry, item: &str) -> f64 {
        reg.get("[App]", item).unwrap().get()
    }

    #[test]
    fn measures_the_share_of_each_buffer_used() {
        let (reg, _) = metered(2.0, 0.01, 0.2);
        let usage = app(&reg, "audio_latency_usage");
        assert!((19.0..30.0).contains(&usage), "{usage}");
        assert!(app(&reg, "audio_latency_peak") >= usage);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 0.0);
    }

    #[test]
    fn counts_slow_and_late_callbacks_as_dropouts() {
        let (reg, mut meter) = metered(2.0, 0.01, 0.2);
        // Took longer than its buffer.
        meter.measure(Instant::now() - Duration::from_millis(15), 0.01);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 1.0);
        assert_eq!(app(&reg, "audio_latency_overload"), 1.0);
        // Came four buffers after the one before.
        meter.measure(Instant::now() + Duration::from_millis(40), 0.01);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 2.0);
        // The light goes out half a second later.
        for _ in 0..60 {
            meter.measure(Instant::now(), 0.01);
        }
        assert_eq!(app(&reg, "audio_latency_overload"), 0.0);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 2.0);
    }

    #[test]
    fn ignores_the_first_second() {
        let (reg, mut meter) = metered(0.5, 0.01, 0.2);
        meter.measure(Instant::now() - Duration::from_millis(15), 0.01);
        assert_eq!(app(&reg, "audio_latency_overload_count"), 0.0);
        assert_eq!(app(&reg, "audio_latency_usage"), 0.0);
    }

    fn route(role: Role, device: &str, pair: u16) -> Route {
        Route {
            role,
            device: device.into(),
            pair,
        }
    }

    #[test]
    fn renders_deck_pairs_only_up_to_the_last_one_routed() {
        let mut setup = AudioSetup::default();
        setup.routes.push(route(Role::Headphones, "", 1));
        assert_eq!(render_width(&setup), MIX_CHANNELS);
        setup.routes.push(route(Role::Deck2, "", 2));
        assert_eq!(render_width(&setup), 10);
        setup.routes.push(route(Role::Sampler, "", 3));
        assert_eq!(render_width(&setup), OUTPUT_CHANNELS);
        assert!(Role::Deck1.is_tap() && Role::Sampler.is_tap());
        assert!(!Role::Booth.is_tap());
        assert!(Role::ALL
            .iter()
            .all(|r| Role::from_name(r.name()) == Some(*r)));
    }

    #[test]
    fn warns_when_deck_outputs_leave_the_master_device() {
        let mut setup = AudioSetup {
            routes: vec![route(Role::Master, "BlackHole 16ch", 0)],
            ..AudioSetup::default()
        };
        setup.routes.push(route(Role::Deck1, "BlackHole 16ch", 1));
        setup.routes.push(route(Role::Headphones, "DDJ-FLX4", 1));
        assert!(!deck_outputs_split(&setup));
        setup.routes.push(route(Role::Deck2, "DDJ-FLX4", 0));
        assert!(deck_outputs_split(&setup));
    }

    #[test]
    fn an_external_mixer_frees_the_master_device() {
        let mut setup = AudioSetup {
            routes: vec![
                route(Role::Master, "Mac speakers", 0),
                route(Role::Deck1, "DJM-A9", 0),
                route(Role::Deck2, "DJM-A9", 1),
            ],
            ..AudioSetup::default()
        };
        assert_eq!(clock_device(&setup), "Mac speakers");
        setup.external_mixer = true;
        assert!(!setup.in_use(&setup.routes[0]));
        assert!(setup.in_use(&setup.routes[1]));
        assert_eq!(clock_device(&setup), "DJM-A9");
        assert!(!deck_outputs_split(&setup));
    }

    #[test]
    fn device_errors_count_dropouts_and_losses() {
        let reg = ControlRegistry::new();
        let health = Health::new(&reg);
        health.stream_error(&ErrorKind::Xrun.into());
        assert_eq!(app(&reg, "audio_latency_overload_count"), 1.0);
        assert!(!health.lost.load(Ordering::Acquire));
        health.stream_error(&ErrorKind::DeviceNotAvailable.into());
        assert!(health.lost.load(Ordering::Acquire));
        health.stream_error(&ErrorKind::RealtimeDenied.into());
        assert!(health.realtime_denied.load(Ordering::Relaxed));
    }
}
