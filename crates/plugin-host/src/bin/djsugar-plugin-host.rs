//! The helper process. Two modes:
//!
//! - `djsugar-plugin-host --shm <path> (--test <id> | --clap <file> --id <id> |
//!   --vst3 <bundle> --id <cid>)`
//!   maps the block DJ Sugar shares, loads one effect, describes it on stdout
//!   (`name`, `latency`, `param` lines, then `ready`) and runs a block each
//!   time DJ Sugar writes a byte to stdin. It exits when stdin closes, so a
//!   helper never outlives DJ Sugar. `--title <text>` names the window the
//!   plugin's editor opens in, when DJ Sugar asks for it. `--state <file>`
//!   names the file the plugin's state is saved to and loaded from when
//!   DJ Sugar asks (see `shm::Header::state_cmd`); if it exists at start, the
//!   plugin starts from it.
//! - `djsugar-plugin-host --scan <file>` lists the plugins in a CLAP file or
//!   VST3 bundle, and `--scan-au` lists the installed Audio Units (macOS)
//!   (`plugin` lines, then `done`) and exits.
//! - `djsugar-plugin-host --presets (--clap <file> --id <id> | --au <id>)`
//!   lists the plugin's own presets (`preset` lines, name then reference, then `done`)
//!   and exits.
//!
//! The plugin's main-thread half stays on this process's main thread; the
//! audio runs on a second thread.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_plugin_host::effects::{self, Effect, MainThread, Plugin};
use djsugar_plugin_host::shm::{self, Midi, Shm, CHANNELS, MAX_EVENTS, MAX_PARAMS};
use djsugar_plugin_host::window::EditorWindow;
use djsugar_plugin_host::Transport;

/// macOS: makes this helper a background (accessory) app before any plugin
/// code runs. The helper sits inside DJSugar.app, so once a plugin touches
/// AppKit on load (many instruments do) macOS would otherwise show it as a
/// second "DJ Sugar" in the Dock and the ⌘-Tab switcher. Quitting that one
/// would kill the plugin.
#[cfg(target_os = "macos")]
fn stay_in_background() {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    // NSApplicationActivationPolicyAccessory: windows, but no Dock icon or
    // menu bar, and not in the app switcher.
    const ACCESSORY: isize = 1;
    // SAFETY: the shared NSApplication, on the main thread (main's first
    // line). AppKit is linked through winit.
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if !app.is_null() {
            let _: bool = msg_send![app, setActivationPolicy: ACCESSORY];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn stay_in_background() {}

fn main() {
    stay_in_background();
    let mut shm_path = None;
    let mut test = None;
    let mut clap = None;
    let mut vst3 = None;
    let mut au = None;
    let mut scan_au = false;
    let mut list_presets = false;
    let mut id = None;
    let mut scan = None;
    let mut title = None;
    let mut state = None;
    let mut picks = Vec::new();
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--shm") => shm_path = args.next().map(PathBuf::from),
            Some("--test") => test = args.next().and_then(|s| s.into_string().ok()),
            Some("--clap") => clap = args.next().map(PathBuf::from),
            Some("--vst3") => vst3 = args.next().map(PathBuf::from),
            Some("--au") => au = args.next().and_then(|s| s.into_string().ok()),
            Some("--scan-au") => scan_au = true,
            Some("--presets") => list_presets = true,
            Some("--id") => id = args.next().and_then(|s| s.into_string().ok()),
            Some("--scan") => scan = args.next().map(PathBuf::from),
            Some("--title") => title = args.next().and_then(|s| s.into_string().ok()),
            Some("--state") => state = args.next().map(PathBuf::from),
            // A knob the DJ picked a parameter for: `slot:id`.
            Some("--pick") => {
                let pick = args.next().and_then(|s| s.into_string().ok());
                match pick.as_deref().and_then(|p| p.split_once(':')) {
                    Some((slot, id)) => match (slot.parse(), id.parse()) {
                        (Ok(slot), Ok(id)) => picks.push((slot, id)),
                        _ => fail(&format!("bad --pick {pick:?}")),
                    },
                    None => fail(&format!("bad --pick {pick:?}")),
                }
            }
            _ => fail(&format!("unknown argument {a:?}")),
        }
    }
    if let Some(file) = scan {
        return run_scan(&file);
    }
    if scan_au {
        return run_scan_au();
    }
    if list_presets {
        let found = match (clap, id, au) {
            (Some(file), Some(id), None) => djsugar_plugin_host::clap::presets(&file, &id),
            #[cfg(target_os = "macos")]
            (None, _, Some(au)) => djsugar_plugin_host::au::presets(&au),
            _ => fail("usage: djsugar-plugin-host --presets (--clap <file> --id <id> | --au <id>)"),
        };
        return run_presets(found);
    }
    let plugin = match (test, clap, vst3, au, id) {
        (Some(t), None, None, None, _) => Plugin::Test(t),
        (None, Some(file), None, None, Some(id)) => Plugin::Clap { file, id },
        (None, None, Some(file), None, Some(id)) => Plugin::Vst3 { file, id },
        (None, None, None, Some(id), _) => Plugin::Au { id },
        _ => fail(
            "usage: djsugar-plugin-host --shm <path> (--test <id> | --clap <file> --id <id> | --vst3 <bundle> --id <cid> | --au <type:subtype:maker>)",
        ),
    };
    let Some(shm_path) = shm_path else {
        fail("--shm <path> is needed");
    };
    let shm =
        Arc::new(Shm::open(&shm_path).unwrap_or_else(|e| fail(&format!("shared memory: {e}"))));
    let h = shm.header();
    let sample_rate = h.sample_rate.load(Ordering::Relaxed);
    let loaded =
        effects::load(&plugin, sample_rate, shm.max_frames(), &picks).unwrap_or_else(|e| fail(&e));

    // Describe it, and start the shared parameters at its current values,
    // so DJ Sugar changes only what the DJ touches.
    let info = &loaded.info;
    say(&format!("name\t{}", clean(&info.name)));
    say(&format!("latency\t{}", info.latency));
    for (i, p) in info.params.iter().enumerate() {
        shm::store_f32(&h.params[i], p.value as f32);
        say(&format!(
            "param\t{}\t{}\t{}\t{}\t{}\t{}",
            p.min,
            p.max,
            p.default,
            p.value,
            p.id,
            clean(&p.name)
        ));
    }
    for (id, name) in &info.all {
        say(&format!("all\t{id}\t{}", clean(name)));
    }
    let mut main = loaded.main;
    let mut no_main = NoMain {
        state: Vec::new(),
        value: info.params.first().map(|p| p.value),
    };
    let mut params = ParamSync::new(info.params.iter().map(|p| p.value as f32).collect());
    // A saved sound to start from, before the first block.
    if let Some(data) = state.as_ref().and_then(|p| std::fs::read(p).ok()) {
        let m: &mut dyn MainThread = match main.as_mut() {
            Some(m) => m.as_mut(),
            None => &mut no_main,
        };
        if let Err(e) = m.load_state(&data) {
            say(&format!("note\tthe saved sound didn't load: {}", clean(&e)));
        }
    }
    let effect = loaded.effect;
    let audio_shm = shm.clone();
    let audio = std::thread::Builder::new()
        .name("plugin-audio".into())
        .spawn(move || run_audio(&audio_shm, effect))
        .unwrap_or_else(|e| fail(&format!("audio thread: {e}")));
    say("ready");

    // Main thread: serve the plugin's main-thread requests, and its editor
    // window, until DJ Sugar lets go. Exiting without tearing the plugin down
    // is deliberate: a plugin can't crash on the way out if it is never
    // asked to clean up.
    let title = title.unwrap_or_else(|| info.name.clone());
    let mut editor = EditorWindow::new(title);
    while !audio.is_finished() {
        let m: &mut dyn MainThread = match main.as_mut() {
            Some(m) => m.as_mut(),
            None => &mut no_main,
        };
        m.idle();
        if serve_state(m, h, state.as_deref()) {
            params.loaded(h);
        }
        params.step(m, h);
        let wait = m
            .next_idle()
            .map_or(IDLE, |d| d.min(IDLE))
            .max(Duration::from_millis(1));
        editor.step(m, &h.editor, &h.editor_shown, wait);
        if editor.wants_run() {
            break;
        }
    }
    if !editor.wants_run() {
        std::process::exit(0);
    }
    // macOS, once a window was made: the system's loop runs the main thread
    // from here, and this is done whenever it is idle.
    editor.run(move |win, el| {
        if audio.is_finished() {
            std::process::exit(0);
        }
        let h = shm.header();
        let m: &mut dyn MainThread = match main.as_mut() {
            Some(m) => m.as_mut(),
            None => &mut no_main,
        };
        m.idle();
        if serve_state(m, h, state.as_deref()) {
            params.loaded(h);
        }
        params.step(m, h);
        win.step_in(m, &h.editor, &h.editor_shown, el);
        m.next_idle()
            .map_or(IDLE, |d| d.min(IDLE))
            .max(Duration::from_millis(1))
    })
}

/// How often the plugin's main thread is served, at the longest.
const IDLE: Duration = Duration::from_millis(10);

/// Does what DJ Sugar asked about the plugin's state, if anything: saves it
/// to the state file (through a temporary file, so a reader never sees half
/// of it) or loads it from there. True when it loaded one.
fn serve_state(m: &mut dyn MainThread, h: &shm::Header, path: Option<&std::path::Path>) -> bool {
    let asked = h.state_asked.load(Ordering::Acquire);
    if asked == h.state_done.load(Ordering::Relaxed) {
        return false;
    }
    let cmd = h.state_cmd.load(Ordering::Relaxed);
    let result = match (cmd, path) {
        (shm::STATE_SAVE, Some(path)) => m.save_state().and_then(|data| {
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, data)
                .and_then(|_| std::fs::rename(&tmp, path))
                .map_err(|e| e.to_string())
        }),
        (shm::STATE_LOAD, Some(path)) => std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|data| m.load_state(&data)),
        (shm::STATE_PRESET, Some(path)) => std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|reference| m.load_preset(&reference)),
        _ => Err("no state file".into()),
    };
    if let Err(e) = &result {
        say(&format!("note\tsound: {}", clean(e)));
    }
    h.state_ok.store(result.is_ok() as u32, Ordering::Relaxed);
    h.state_done.store(asked, Ordering::Release);
    (cmd == shm::STATE_LOAD || cmd == shm::STATE_PRESET) && result.is_ok()
}

/// How often the plugin's parameter values are read back for DJ Sugar.
const READ_PARAMS: Duration = Duration::from_millis(50);

/// Keeps the plugin's parameters and DJ Sugar's knobs in step, on the main
/// thread: tells the plugin's main side what DJ Sugar set, and hands back
/// what the plugin has now (see `shm::Header::values`).
struct ParamSync {
    /// The shared values DJ Sugar last set.
    set: Vec<f32>,
    now: Vec<f64>,
    next: std::time::Instant,
}

impl ParamSync {
    fn new(set: Vec<f32>) -> Self {
        Self {
            now: vec![0.0; set.len()],
            set,
            next: std::time::Instant::now(),
        }
    }

    /// A sound was loaded: it set the plugin's values, so knob moves not
    /// passed on yet are dropped rather than undoing it.
    fn loaded(&mut self, h: &shm::Header) {
        for (i, set) in self.set.iter_mut().enumerate() {
            *set = shm::load_f32(&h.params[i]);
        }
        self.next = std::time::Instant::now();
    }

    fn step(&mut self, m: &mut dyn MainThread, h: &shm::Header) {
        if self.set.is_empty() || std::time::Instant::now() < self.next {
            return;
        }
        self.next = std::time::Instant::now() + READ_PARAMS;
        for (i, set) in self.set.iter_mut().enumerate() {
            let v = shm::load_f32(&h.params[i]);
            if v != *set {
                *set = v;
                m.param_set(i, v as f64);
            }
        }
        if m.param_values(&mut self.now) {
            for (i, v) in self.now.iter().enumerate() {
                shm::store_f32(&h.values[i], *v as f32);
            }
            h.values_seq.fetch_add(1, Ordering::Release);
        }
    }
}

/// The main thread of an effect that needs none (the test effects). Its
/// state is whatever was last loaded, so saving and loading can be tested;
/// a 4-byte state is also its first parameter's value (an `f32`), so a
/// sound changing the plugin's knobs can be tested too.
struct NoMain {
    state: Vec<u8>,
    value: Option<f64>,
}

impl MainThread for NoMain {
    fn idle(&mut self) {}

    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.state.clone())
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        self.state = data.to_vec();
        if let (Some(v), Ok(bytes)) = (&mut self.value, <[u8; 4]>::try_from(data)) {
            *v = f32::from_le_bytes(bytes) as f64;
        }
        Ok(())
    }

    fn param_values(&mut self, out: &mut [f64]) -> bool {
        match (self.value, out.first_mut()) {
            (Some(v), Some(o)) => {
                *o = v;
                true
            }
            _ => false,
        }
    }

    fn param_set(&mut self, index: usize, value: f64) {
        if let (0, Some(v)) = (index, &mut self.value) {
            *v = value;
        }
    }
}

fn run_audio(shm: &Shm, mut fx: Box<dyn Effect>) {
    let h = shm.header();
    let max = shm.max_frames();
    // All buffers up front: nothing allocates per block.
    let mut input = vec![vec![0.0f32; max]; CHANNELS];
    let mut output = vec![vec![0.0f32; max]; CHANNELS];
    let mut params = [0.0f32; MAX_PARAMS];
    let mut events = Vec::with_capacity(MAX_EVENTS);
    let mut stdin = std::io::stdin().lock();
    let mut wake = [0u8; 64];
    let mut promoted = false;
    loop {
        match stdin.read(&mut wake) {
            Ok(0) | Err(_) => return, // DJ Sugar closed the pipe or quit
            Ok(_) => {}
        }
        let seq = h.seq_in.load(Ordering::Acquire);
        if seq == h.seq_out.load(Ordering::Relaxed) {
            continue;
        }
        let start = Instant::now();
        let frames = (h.frames.load(Ordering::Relaxed) as usize).min(max);
        if !promoted && frames > 0 {
            // Now that the block length is known: the audio thread runs at
            // real-time priority, as DJ Sugar's own does, so a busy computer
            // (or the plugin's own window drawing) doesn't make it late.
            promoted = true;
            let rate = h.sample_rate.load(Ordering::Relaxed).max(1);
            realtime::promote(Duration::from_secs_f64(frames as f64 / rate as f64));
        }
        for (ch, buf) in input.iter_mut().enumerate() {
            shm.read(false, ch, &mut buf[..frames]);
        }
        for (p, a) in params.iter_mut().zip(&h.params) {
            *p = shm::load_f32(a);
        }
        events.clear();
        let n = (h.events_len.load(Ordering::Relaxed) as usize).min(MAX_EVENTS);
        events.extend(
            h.events[..n]
                .iter()
                .map(|e| Midi::unpack(e.load(Ordering::Relaxed)))
                .filter(|e| (e.frame as usize) < frames.max(1)),
        );
        let transport = Transport {
            bpm: shm::load_f64(&h.bpm),
            beat: shm::load_f64(&h.beat),
            playing: h.playing.load(Ordering::Relaxed) != 0,
        };
        {
            let [l, r] = &mut output[..] else {
                unreachable!()
            };
            fx.process(
                [&input[0][..frames], &input[1][..frames]],
                [&mut l[..frames], &mut r[..frames]],
                &params,
                &transport,
                &events,
            );
        }
        for (ch, buf) in output.iter().enumerate() {
            shm.write(true, ch, &buf[..frames]);
        }
        h.process_ns
            .store(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        h.seq_out.store(seq, Ordering::Release);
    }
}

fn run_scan_au() {
    #[cfg(target_os = "macos")]
    {
        print_found(&djsugar_plugin_host::au::scan());
        say("done");
    }
    #[cfg(not(target_os = "macos"))]
    fail("Audio Units only run on macOS");
}

fn print_found(found: &[djsugar_plugin_host::Descriptor]) {
    for d in found {
        say(&format!(
            "plugin\t{}\t{}\t{}\t{}\t{}",
            clean(&d.id),
            clean(&d.name),
            clean(&d.vendor),
            clean(&d.version),
            clean(&d.features.join(","))
        ));
    }
}

fn run_scan(file: &std::path::Path) {
    let found = if djsugar_plugin_host::client::is_vst3(file) {
        djsugar_plugin_host::vst3::scan(file)
    } else {
        djsugar_plugin_host::clap::scan(file)
    };
    match found {
        Ok(found) => {
            print_found(&found);
            say("done");
        }
        Err(e) => fail(&e),
    }
}

fn run_presets(found: Result<Vec<(String, String)>, String>) {
    match found {
        Ok(found) => {
            for (name, reference) in found {
                // A reference is read back whole, so one that the line
                // format would change is left out.
                if !reference.contains(['\t', '\n', '\r']) {
                    say(&format!("preset\t{}\t{reference}", clean(&name)));
                }
            }
            say("done");
        }
        Err(e) => fail(&e),
    }
}

/// Tabs and newlines would break the line format.
fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn fail(msg: &str) -> ! {
    say(&format!("error {}", clean(msg)));
    std::process::exit(2);
}

/// Real-time priority for the helper's audio thread.
mod realtime {
    use std::time::Duration;

    /// Asks for the calling thread to run every `period`, for up to half of
    /// it, ahead of ordinary threads. Without permission it stays as it is.
    #[cfg(target_os = "macos")]
    #[allow(deprecated)] // libc's Mach items point at the mach2 crate
    pub fn promote(period: Duration) {
        // SAFETY: plain Mach calls about the calling thread.
        unsafe {
            let mut tb = libc::mach_timebase_info { numer: 0, denom: 0 };
            if libc::mach_timebase_info(&mut tb) != 0 || tb.numer == 0 {
                return;
            }
            let ticks = |d: Duration| {
                (d.as_nanos() as f64 * tb.denom as f64 / tb.numer as f64).min(u32::MAX as f64)
                    as u32
            };
            let period = period.clamp(Duration::from_micros(500), Duration::from_millis(100));
            let mut policy = libc::thread_time_constraint_policy {
                period: ticks(period),
                computation: ticks(period / 2),
                constraint: ticks(period),
                preemptible: 1,
            };
            let r = libc::thread_policy_set(
                libc::mach_thread_self(),
                libc::THREAD_TIME_CONSTRAINT_POLICY as _,
                &mut policy as *mut _ as libc::thread_policy_t,
                libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
            );
            if r != 0 {
                eprintln!("plugin audio thread: no real-time priority ({r})");
            }
        }
    }

    /// Linux: real-time scheduling where the system allows it (a user in
    /// the audio group with rtprio set); otherwise as it is.
    #[cfg(all(unix, not(target_os = "macos")))]
    pub fn promote(_period: Duration) {
        // SAFETY: plain calls about the calling thread.
        unsafe {
            let param = libc::sched_param { sched_priority: 10 };
            let _ = libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &param);
        }
    }

    /// Windows: not yet (MMCSS "Pro Audio" is the way there).
    #[cfg(windows)]
    pub fn promote(_period: Duration) {}
}
