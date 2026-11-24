//! DJ Sugar's side: starts a helper and trades audio blocks with it. [`spawn`] returns two halves. The [`Connection`]
//! goes to the audio thread: [`Connection::process`] never allocates, never locks and never waits longer than the
//! deadline it is given. The [`Helper`] stays on a normal thread and watches the process (exit, kill).

use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::descriptor::Descriptor;
use crate::effects::{Info, Param, Plugin};
use crate::shm::{self, Midi, Shm, CHANNELS, MAX_EVENTS, MAX_PARAMS};

/// How long a new helper has to say it is ready.
pub const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// What to start.
#[derive(Clone, Debug)]
pub struct Options {
    /// The `djsugar-plugin-host` binary.
    pub helper: PathBuf,
    /// Which effect the helper loads.
    pub plugin: Plugin,
    pub sample_rate: u32,
    /// The longest block `process` will be given.
    pub max_frames: usize,
    /// The title of the plugin's editor window, when it opens (the
    /// plugin's name when None).
    pub title: Option<String>,
    /// A sound to start from: bytes [`Helper::save_state`] gave.
    pub state: Option<Vec<u8>>,
    /// Knobs the DJ picked a parameter for: (slot, the parameter's id, as
    /// [`Param::id`] and [`Info::all`] give it).
    pub picks: Vec<(usize, u32)>,
}

/// How long the helper has to save or load the plugin's state.
pub const STATE_TIMEOUT: Duration = Duration::from_secs(5);

/// The routed deck's tempo and position, for beat-synced plugins.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transport {
    pub bpm: f64,
    pub beat: f64,
    pub playing: bool,
}

/// A plugin's own window, as the helper reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editor {
    Closed,
    Shown,
    /// The plugin has no window of its own, or it failed to open.
    Missing,
}

/// What became of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The plugin's output.
    Wet,
    /// The helper missed the deadline; the block is dry.
    Late,
    /// The helper is still working on an earlier, late block; dry.
    Busy,
    /// The helper is gone (crashed, killed or never started); dry.
    Gone,
}

impl Outcome {
    pub fn is_wet(self) -> bool {
        self == Outcome::Wet
    }
}

/// The audio thread's half.
pub struct Connection {
    shm: Arc<Shm>,
    wake: ChildStdin,
    seq: u64,
    /// A handed-over block the helper hasn't finished.
    pending: bool,
    /// The block [`Connection::send`] just handed over, not yet collected.
    awaiting: bool,
    gone: bool,
    missed_in_row: u32,
    health: Arc<Health>,
}

/// What the audio thread last saw, for the control thread to act on.
#[derive(Default)]
struct Health {
    missed_in_row: AtomicU32,
    gone: AtomicBool,
}

/// The control thread's half.
pub struct Helper {
    health: Arc<Health>,
    child: Child,
    shm_path: PathBuf,
    /// Where the helper saves the plugin's state and loads it from.
    state_path: PathBuf,
    messages: Receiver<String>,
    shm: Arc<Shm>,
    info: Info,
}

/// The helper's arguments that pick the plugin.
fn plugin_args(plugin: &Plugin) -> Vec<std::ffi::OsString> {
    match plugin {
        Plugin::Test(id) => vec!["--test".into(), id.into()],
        Plugin::Clap { file, id } => vec!["--clap".into(), file.clone().into_os_string(), "--id".into(), id.into()],
        Plugin::Vst3 { file, id } => vec!["--vst3".into(), file.clone().into_os_string(), "--id".into(), id.into()],
        Plugin::Au { id } => vec!["--au".into(), id.into()],
    }
}

/// One line of the helper's description (see the helper's `describe`).
fn read_info(info: &mut Info, line: &str) {
    let mut f = line.split('\t');
    match f.next() {
        Some("name") => info.name = f.next().unwrap_or_default().to_string(),
        Some("latency") => info.latency = f.next().and_then(|v| v.parse().ok()).unwrap_or(0),
        Some("param") => {
            let mut num = || f.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let (min, max, default, value) = (num(), num(), num(), num());
            let id = f.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let name = f.next().unwrap_or_default().to_string();
            info.params.push(Param { id, name, min, max, default, value });
        }
        Some("all") => {
            if let (Some(id), Some(name)) = (f.next().and_then(|v| v.parse().ok()), f.next()) {
                info.all.push((id, name.to_string()));
            }
        }
        _ => {}
    }
}

/// Whether `file` is a VST3 plugin (by its `.vst3` name).
pub fn is_vst3(file: &Path) -> bool {
    file.extension().is_some_and(|e| e.eq_ignore_ascii_case("vst3"))
}

/// Why a file could not be scanned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanError {
    /// It took longer than the timeout (a licence dialog, a hang).
    TimedOut,
    /// The scanner died; the text says how.
    Crashed(String),
    /// The file loaded but isn't usable; the text says why.
    Failed(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::TimedOut => write!(f, "took too long to scan"),
            ScanError::Crashed(how) => write!(f, "crashed while scanning ({how})"),
            ScanError::Failed(why) => write!(f, "{why}"),
        }
    }
}

/// Lists the plugins in one CLAP file or VST3 bundle. A VST3 bundle's `moduleinfo.json` is read
/// directly when it has one; otherwise the file is loaded in a throwaway helper that is killed
/// after `timeout`. Loading a plugin runs its code, so this never happens inside DJ Sugar.
pub fn scan(helper: &Path, file: &Path, timeout: Duration) -> Result<Vec<Descriptor>, ScanError> {
    if is_vst3(file) {
        if let Some(found) = crate::descriptor::scan_moduleinfo(file) {
            return Ok(found);
        }
    }
    scan_with(helper, &["--scan".into(), file.as_os_str().to_owned()], timeout)
}

/// Lists the installed Audio Units (macOS), from the system registry, in a
/// throwaway helper like [`scan`].
pub fn scan_audio_units(helper: &Path, timeout: Duration) -> Result<Vec<Descriptor>, ScanError> {
    scan_with(helper, &["--scan-au".into()], timeout)
}

/// A plugin's own presets, as (name, reference for [`Helper::load_preset`]), read
/// in a throwaway helper like [`scan`]: a CLAP plugin's from its preset discovery
/// providers, an Audio Unit's factory presets. Other plugins list none.
pub fn presets(helper: &Path, plugin: &Plugin, timeout: Duration) -> Result<Vec<(String, String)>, ScanError> {
    let args: Vec<std::ffi::OsString> = match plugin {
        Plugin::Clap { file, id } => {
            vec!["--presets".into(), "--clap".into(), file.as_os_str().to_owned(), "--id".into(), id.into()]
        }
        Plugin::Au { id } => vec!["--presets".into(), "--au".into(), id.into()],
        _ => return Ok(Vec::new()),
    };
    Ok(lines(helper, &args, timeout)?
        .iter()
        .filter_map(|line| {
            let mut f = line.splitn(3, '\t');
            match (f.next(), f.next(), f.next()) {
                (Some("preset"), Some(name), Some(reference)) => Some((name.to_string(), reference.to_string())),
                _ => None,
            }
        })
        .collect())
}

fn scan_with(helper: &Path, args: &[std::ffi::OsString], timeout: Duration) -> Result<Vec<Descriptor>, ScanError> {
    Ok(lines(helper, args, timeout)?
        .iter()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            match f[..] {
                ["plugin", id, name, vendor, version, features] => Some(Descriptor {
                    id: id.into(),
                    name: name.into(),
                    vendor: vendor.into(),
                    version: version.into(),
                    features: features.split(',').filter(|s| !s.is_empty()).map(String::from).collect(),
                }),
                _ => None,
            }
        })
        .collect())
}

/// Runs a throwaway helper with `args` and gives the lines it wrote before
/// `done`, killing it after `timeout`.
fn lines(helper: &Path, args: &[std::ffi::OsString], timeout: Duration) -> Result<Vec<String>, ScanError> {
    let start =
        || Command::new(helper).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
    // A helper just written can't start while another thread's fork still holds it
    // open for writing ("text file busy"); that clears within milliseconds.
    let mut tries = 0;
    let mut child = loop {
        match start() {
            Err(e) if e.raw_os_error() == Some(26) && tries < 20 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(10));
            }
            r => break r.map_err(|e| ScanError::Failed(e.to_string()))?,
        }
    };
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let end = Instant::now() + timeout;
    let mut found = Vec::new();
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(line) if line == "done" => break,
            Ok(line) => {
                if let Some(msg) = line.strip_prefix("error ") {
                    stop(&mut child);
                    return Err(ScanError::Failed(msg.to_string()));
                }
                found.push(line);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                stop(&mut child);
                return Err(ScanError::TimedOut);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // The helper closed its output without "done". Give it the
                // rest of the time to exit, so its status says what happened,
                // but never wait on it for longer.
                let status = loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break status.to_string(),
                        Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(20)),
                        _ => {
                            stop(&mut child);
                            return Err(ScanError::TimedOut);
                        }
                    }
                };
                return Err(ScanError::Crashed(status));
            }
        }
    }
    // Everything needed has been read. Some plugins hang while the helper
    // exits (their unload or exit handlers wait on something that never
    // comes), so the helper is stopped rather than waited for.
    stop(&mut child);
    Ok(found)
}

/// Ends a scan helper and reaps it. `kill` makes `wait` return at once.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Where the shared block for a new helper goes: memory-backed `/dev/shm`
/// on Linux, the temp folder elsewhere.
fn shm_path() -> PathBuf {
    let dir = if cfg!(target_os = "linux") && Path::new("/dev/shm").is_dir() {
        PathBuf::from("/dev/shm")
    } else {
        std::env::temp_dir()
    };
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    dir.join(format!("djsugar-plugin-{}-{id}", std::process::id()))
}

/// The state file beside a helper's shared block, in the temp folder (the
/// plugin's state can be large, so not in `/dev/shm`).
fn state_path(shm: &Path) -> PathBuf {
    let name = shm.file_name().unwrap_or_default().to_string_lossy();
    std::env::temp_dir().join(format!("{name}.state"))
}

/// Starts a helper for `opts.plugin` and waits (up to [`READY_TIMEOUT`]) for
/// it to load. Call this off the audio thread.
pub fn spawn(opts: &Options) -> io::Result<(Connection, Helper)> {
    let path = shm_path();
    let state_path = state_path(&path);
    let _ = std::fs::remove_file(&state_path);
    if let Some(state) = &opts.state {
        std::fs::write(&state_path, state)?;
    }
    let shm = Arc::new(Shm::create(&path, opts.max_frames, opts.sample_rate)?);
    let child = Command::new(&opts.helper)
        .arg("--shm")
        .arg(&path)
        .arg("--state")
        .arg(&state_path)
        .args(plugin_args(&opts.plugin))
        .args(opts.picks.iter().flat_map(|(slot, id)| ["--pick".into(), format!("{slot}:{id}")]))
        .args(opts.title.iter().flat_map(|t| [std::ffi::OsString::from("--title"), t.into()]))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(&state_path);
            return Err(e);
        }
    };
    let wake = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, messages) = mpsc::channel();
    std::thread::Builder::new().name("plugin-host-messages".into()).spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    })?;
    let mut helper = Helper {
        health: Arc::default(),
        child,
        shm_path: path,
        state_path,
        messages,
        shm: shm.clone(),
        info: Info::default(),
    };
    // It describes the plugin, line by line, then says "ready".
    let end = Instant::now() + READY_TIMEOUT;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match helper.messages.recv_timeout(left) {
            Ok(line) if line == "ready" => break,
            Ok(line) => {
                if let Some(msg) = line.strip_prefix("error ") {
                    helper.kill();
                    return Err(io::Error::other(msg.to_string()));
                }
                read_info(&mut helper.info, &line);
            }
            Err(_) => {
                helper.kill();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "the plugin did not start in time"));
            }
        }
    }
    // Both sides have it mapped: on Unix the name can go now, so nothing is
    // left behind even if both processes die.
    #[cfg(unix)]
    let _ = std::fs::remove_file(&helper.shm_path);
    let conn = Connection {
        shm,
        wake,
        seq: 0,
        pending: false,
        awaiting: false,
        gone: false,
        missed_in_row: 0,
        health: helper.health.clone(),
    };
    Ok((conn, helper))
}

fn copy_dry(input: [&[f32]; CHANNELS], output: &mut [&mut [f32]; CHANNELS]) {
    for ch in 0..CHANNELS {
        output[ch].copy_from_slice(input[ch]);
    }
}

impl Connection {
    /// Runs one block through the plugin, or copies it dry if the helper
    /// can't deliver within `deadline`. Real-time safe.
    pub fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        transport: Transport,
        deadline: Duration,
    ) -> Outcome {
        self.process_events(input, output, transport, &[], deadline)
    }

    /// [`Connection::process`] with MIDI events for the block (notes for an
    /// instrument), in frame order. Up to [`MAX_EVENTS`] go through; a block
    /// that doesn't reach the plugin drops its events.
    pub fn process_events(
        &mut self,
        input: [&[f32]; CHANNELS],
        mut output: [&mut [f32]; CHANNELS],
        transport: Transport,
        events: &[Midi],
        deadline: Duration,
    ) -> Outcome {
        let start = Instant::now();
        let out = match self.send(input, transport, events) {
            Ok(()) => self.collect_until(&mut output, start + deadline),
            Err(out) => out,
        };
        if !out.is_wet() {
            copy_dry(input, &mut output);
        }
        out
    }

    /// The first half of [`Connection::process_events`]: hands the block to the helper and returns at once, so several helpers
    /// can work on their blocks side by side. [`Connection::collect`] picks the result up. `Err` when the block can't go (the
    /// helper is still on a late block, or gone): it plays dry and there is nothing to collect. Real-time safe.
    pub fn send(&mut self, input: [&[f32]; CHANNELS], transport: Transport, events: &[Midi]) -> Result<(), Outcome> {
        let frames = input[0].len();
        assert!(frames <= self.shm.max_frames(), "block longer than max_frames");
        assert!(input.iter().all(|c| c.len() == frames));
        let h = self.shm.header();
        if self.gone {
            return Err(Outcome::Gone);
        }
        if self.pending {
            if h.seq_out.load(Ordering::Acquire) == self.seq {
                // The late block finished; its output is stale, drop it.
                self.pending = false;
            } else {
                self.missed(1);
                return Err(Outcome::Busy);
            }
        }
        for (ch, data) in input.iter().enumerate() {
            self.shm.write(false, ch, data);
        }
        h.frames.store(frames as u32, Ordering::Relaxed);
        shm::store_f64(&h.bpm, transport.bpm);
        shm::store_f64(&h.beat, transport.beat);
        h.playing.store(transport.playing as u32, Ordering::Relaxed);
        let n = events.len().min(MAX_EVENTS);
        for (slot, e) in h.events.iter().zip(&events[..n]) {
            slot.store(e.pack(), Ordering::Relaxed);
        }
        h.events_len.store(n as u32, Ordering::Relaxed);
        self.seq += 1;
        h.seq_in.store(self.seq, Ordering::Release);
        // One byte per block, and only while the helper is idle, so the pipe
        // never fills and this write never blocks.
        if self.wake.write_all(&[1]).is_err() {
            self.gone = true;
            self.health.gone.store(true, Ordering::Relaxed);
            return Err(Outcome::Gone);
        }
        self.pending = true;
        self.awaiting = true;
        Ok(())
    }

    /// The second half: waits up to `deadline` for the block [`Connection::send`]
    /// handed over and writes the plugin's output. Anything but [`Outcome::Wet`]
    /// leaves `output` as it was (the caller plays the block dry). Real-time safe.
    pub fn collect(&mut self, mut output: [&mut [f32]; CHANNELS], deadline: Duration) -> Outcome {
        self.collect_until(&mut output, Instant::now() + deadline)
    }

    fn collect_until(&mut self, output: &mut [&mut [f32]; CHANNELS], end: Instant) -> Outcome {
        if self.gone {
            self.awaiting = false;
            return Outcome::Gone;
        }
        if !std::mem::take(&mut self.awaiting) {
            // Nothing was sent: send refused the block.
            return Outcome::Busy;
        }
        let frames = self.shm.header().frames.load(Ordering::Relaxed) as usize;
        assert!(output.iter().all(|c| c.len() == frames));
        let h = self.shm.header();
        let mut spins = 0u32;
        loop {
            if h.seq_out.load(Ordering::Acquire) == self.seq {
                self.pending = false;
                for (ch, data) in output.iter_mut().enumerate() {
                    self.shm.read(true, ch, data);
                }
                if self.missed_in_row != 0 {
                    self.missed_in_row = 0;
                    self.health.missed_in_row.store(0, Ordering::Relaxed);
                }
                return Outcome::Wet;
            }
            spins += 1;
            if spins.is_multiple_of(64) && Instant::now() >= end {
                break;
            }
            std::hint::spin_loop();
        }
        // Still pending: the next send drops its output once it is done.
        self.missed(1);
        Outcome::Late
    }

    fn missed(&mut self, n: u32) {
        self.missed_in_row += n;
        self.health.missed_in_row.store(self.missed_in_row, Ordering::Relaxed);
    }

    /// Sets parameter `index` (0..32); the helper sees it on its next block.
    pub fn set_param(&self, index: usize, value: f32) {
        if index < MAX_PARAMS {
            shm::store_f32(&self.shm.header().params[index], value);
        }
    }

    /// Blocks in a row that came back dry because the helper was late or busy.
    pub fn missed_in_row(&self) -> u32 {
        self.missed_in_row
    }

    pub fn is_gone(&self) -> bool {
        self.gone
    }
}

/// Whether a helper is still running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    /// It ended; the text says how (exit code or signal).
    Exited(String),
}

impl Helper {
    pub fn status(&mut self) -> Status {
        match self.child.try_wait() {
            Ok(None) => Status::Running,
            Ok(Some(s)) => Status::Exited(s.to_string()),
            Err(e) => Status::Exited(e.to_string()),
        }
    }

    /// Waits up to `timeout` for the helper to end.
    pub fn wait_exit(&mut self, timeout: Duration) -> Status {
        let end = Instant::now() + timeout;
        loop {
            let s = self.status();
            if s != Status::Running || Instant::now() >= end {
                return s;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Stops a hung or unwanted helper.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// [`Connection::missed_in_row`], as the audio thread last left it.
    pub fn missed_in_row(&self) -> u32 {
        self.health.missed_in_row.load(Ordering::Relaxed)
    }

    /// Whether the audio thread found the helper gone (its pipe closed).
    pub fn is_gone(&self) -> bool {
        self.health.gone.load(Ordering::Relaxed)
    }

    /// How much of the last block's time the plugin used (1.0 = all of it).
    pub fn load(&self) -> f32 {
        let h = self.shm.header();
        let frames = h.frames.load(Ordering::Relaxed) as f64;
        let rate = h.sample_rate.load(Ordering::Relaxed) as f64;
        if frames == 0.0 || rate == 0.0 {
            return 0.0;
        }
        let block_ns = frames / rate * 1e9;
        (h.process_ns.load(Ordering::Relaxed) as f64 / block_ns) as f32
    }

    /// Lines the helper printed after it started (errors, notes).
    pub fn messages(&self) -> impl Iterator<Item = String> + '_ {
        self.messages.try_iter()
    }

    /// Asks the helper to open (or close) the plugin's own window.
    pub fn set_editor(&self, open: bool) {
        self.shm.header().editor.store(open as u32, Ordering::Release);
    }

    /// Whether DJ Sugar's ask for the window still stands: the helper takes
    /// it back when the DJ closes the window or the plugin has none.
    pub fn editor_asked(&self) -> bool {
        self.shm.header().editor.load(Ordering::Acquire) == 1
    }

    /// What became of the window: shown, closed, or none to show.
    pub fn editor(&self) -> Editor {
        match self.shm.header().editor_shown.load(Ordering::Acquire) {
            1 => Editor::Shown,
            2 => Editor::Missing,
            _ => Editor::Closed,
        }
    }

    /// The plugin's whole state (its sound and settings), to start a later
    /// helper from ([`Options::state`]) or load with [`Helper::load_state`].
    /// Waits up to [`STATE_TIMEOUT`]; call it off the audio thread.
    pub fn save_state(&self) -> Result<Vec<u8>, String> {
        self.state(shm::STATE_SAVE)?;
        std::fs::read(&self.state_path).map_err(|e| e.to_string())
    }

    /// Brings the plugin back to a state [`Helper::save_state`] gave, while
    /// it plays. Waits up to [`STATE_TIMEOUT`]; call it off the audio thread.
    pub fn load_state(&self, data: &[u8]) -> Result<(), String> {
        std::fs::write(&self.state_path, data).map_err(|e| e.to_string())?;
        self.state(shm::STATE_LOAD)
    }

    /// Loads one of the plugin's own presets, by the reference [`presets`]
    /// gave, while it plays. Waits up to [`STATE_TIMEOUT`]; call it off the
    /// audio thread.
    pub fn load_preset(&self, reference: &str) -> Result<(), String> {
        std::fs::write(&self.state_path, reference).map_err(|e| e.to_string())?;
        self.state(shm::STATE_PRESET)
    }

    fn state(&self, cmd: u32) -> Result<(), String> {
        let h = self.shm.header();
        if h.state_asked.load(Ordering::Relaxed) != h.state_done.load(Ordering::Acquire) {
            return Err("the plugin is still busy with its last sound".into());
        }
        h.state_cmd.store(cmd, Ordering::Relaxed);
        let asked = h.state_asked.load(Ordering::Relaxed).wrapping_add(1);
        h.state_asked.store(asked, Ordering::Release);
        let end = Instant::now() + STATE_TIMEOUT;
        while h.state_done.load(Ordering::Acquire) != asked {
            if Instant::now() >= end || self.is_gone() {
                return Err("the plugin didn't answer".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        if h.state_ok.load(Ordering::Relaxed) == 1 {
            Ok(())
        } else {
            Err("the plugin couldn't do it".into())
        }
    }

    /// The parameters' values as the plugin has them now, in their own
    /// ranges, and a number that changes each time they are read back
    /// (none read yet: 0). They follow the plugin's window and loaded sounds.
    pub fn param_values(&self) -> (u32, Vec<f32>) {
        let h = self.shm.header();
        let seq = h.values_seq.load(Ordering::Acquire);
        let n = self.info.params.len().min(MAX_PARAMS);
        (seq, (0..n).map(|i| shm::load_f32(&h.values[i])).collect())
    }

    /// The plugin's name, parameters and latency, as it reported them.
    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_file(&self.shm_path);
        let _ = std::fs::remove_file(&self.state_path);
    }
}
