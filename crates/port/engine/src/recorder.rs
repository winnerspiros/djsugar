//! Records the master mix to a WAV or FLAC file, with a log of every control change next to it. The audio thread copies the master pair into a ring buffer while `[Recording],status` is on; nothing else happens there. A writer thread empties the buffer into the file and rewrites its header every couple of seconds, so a crash or power cut keeps everything up to then. WAV files move on to a new part
//! before they reach 4 GB. A logger thread writes `<name>.controls.jsonl`: one line per control change (`[Channel1],volume` and so on), stamped with the time in the recording, plus a line for each track loaded on a deck, and `pos` lines with each loaded deck's place in its track: one a second, and one whenever the deck jumps (a hot cue, a seek on the waveform, a loop going round). That is enough to
//! rebuild the set later, e.g. as a music-production project. When the recording stops it also writes `<name>.cue`, listing each track from the moment it became audible, and `<name>.tracklist.txt`, the same as `0:00 Artist - Title` lines. The controls: `[Recording],toggle_recording` starts and stops, `[Recording],status` is 0 (off) or 2 (recording). `[Recording],duration` counts the seconds recorded.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};
use serde_json::{json, Value};

use crate::track::{Source, Track};
use crate::{deck_group, LoadedTracks, NUM_DECKS};

/// Seconds of audio the ring buffer holds while the writer catches up.
const BUFFER_SECONDS: f64 = 8.0;

/// How often the file's header is brought up to date.
const SYNC_EVERY: Duration = Duration::from_secs(2);

/// How often the logger looks for control changes.
const LOG_EVERY: Duration = Duration::from_millis(5);

/// A WAV file's data stops short of this, then a new part starts.
const WAV_LIMIT: u64 = 4_000_000_000;

/// Controls the engine rewrites every buffer; logging them would only add
/// noise, and they follow from play, rate and seeks anyway.
const NOT_LOGGED: [&str; 1] = ["playposition"];

/// Read-only values that are still logged, because a rebuilt set needs
/// them: where loops, the cue point and hot cues sit. The rest (meters,
/// beat flashes and so on) only follow from what was played.
const LOGGED_INDICATORS: [&str; 3] = ["loop_start_position", "loop_end_position", "cue_point"];

/// How often each loaded deck's place in its track is logged.
const POS_EVERY: f64 = 1.0;

/// A deck that moves further than this between two looks, beyond what
/// playing at up to `MAX_SPEED` explains, has jumped.
const JUMP_SECONDS: f64 = 0.1;

/// Faster than any pitch fader or scratch moves through a track.
const MAX_SPEED: f64 = 4.0;

/// A deck counts as audible from this channel fader position up.
const AUDIBLE_VOLUME: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordFormat {
    Wav,
    Flac,
}

impl RecordFormat {
    pub fn name(self) -> &'static str {
        match self {
            RecordFormat::Wav => "wav",
            RecordFormat::Flac => "flac",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "wav" => Some(RecordFormat::Wav),
            "flac" => Some(RecordFormat::Flac),
            _ => None,
        }
    }
}

/// Where and how recordings are written.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordSettings {
    pub dir: PathBuf,
    pub format: RecordFormat,
    /// 16 or 24.
    pub bits: u16,
}

impl Default for RecordSettings {
    fn default() -> Self {
        Self { dir: default_dir(), format: RecordFormat::Wav, bits: 24 }
    }
}

/// `~/Music/DJSugar/Recordings`.
pub fn default_dir() -> PathBuf {
    crate::scan::default_music_dir().unwrap_or_else(std::env::temp_dir).join("DJ Sugar").join("Recordings")
}

/// What the audio thread and the writer share.
#[derive(Default)]
struct Shared {
    /// The audio thread copies the master into the ring buffer while set.
    active: AtomicBool,
    /// Frames handed to the ring buffer since recording started: the clock
    /// the control log is stamped with.
    frames: AtomicU64,
    /// Frames lost because the writer fell behind.
    dropped: AtomicU64,
}

/// The audio thread's end: copies the master pair out while recording.
pub(crate) struct RecordTap {
    tx: rtrb::Producer<f32>,
    shared: Arc<Shared>,
}

impl RecordTap {
    /// Whether the tap is copying: while recording, for the recorder's tap.
    pub(crate) fn is_active(&self) -> bool {
        self.shared.active.load(Ordering::Acquire)
    }

    /// Copies the first pair of `out` (the master) if recording.
    /// Wait-free; drops the buffer if the writer has fallen behind.
    pub(crate) fn push(&mut self, out: &[f32], channels: usize) {
        if !self.shared.active.load(Ordering::Acquire) {
            return;
        }
        let frames = out.len() / channels;
        if self.tx.slots() < frames * 2 {
            self.shared.dropped.fetch_add(frames as u64, Ordering::Relaxed);
        } else if let Ok(mut chunk) = self.tx.write_chunk_uninit(frames * 2) {
            let (a, b) = chunk.as_mut_slices();
            let samples = out.chunks_exact(channels).flat_map(|f| [f[0], if channels > 1 { f[1] } else { f[0] }]);
            for (slot, s) in a.iter_mut().chain(b.iter_mut()).zip(samples) {
                slot.write(s);
            }
            // SAFETY: every slot of both slices was written just above.
            unsafe { chunk.commit_all() };
        }
        self.shared.frames.fetch_add(frames as u64, Ordering::Release);
    }
}

/// Creates the audio thread's tap and the buffer's other end.
pub(crate) fn tap(sample_rate: f64) -> (RecordTap, Feed) {
    let shared = Arc::new(Shared::default());
    let (tx, rx) = rtrb::RingBuffer::new((sample_rate * BUFFER_SECONDS) as usize * 2);
    (RecordTap { tx, shared: shared.clone() }, Feed { rx, shared })
}

/// The ring buffer's reading end.
pub(crate) struct Feed {
    rx: rtrb::Consumer<f32>,
    shared: Arc<Shared>,
}

impl Feed {
    /// Starts or stops the audio thread copying into the buffer. The
    /// recorder does this itself; other readers (the loudness meter, the
    /// broadcast) switch their own tap on.
    pub(crate) fn set_active(&self, on: bool) {
        self.shared.active.store(on, Ordering::Release);
    }

    /// Moves every sample waiting in the buffer (interleaved stereo) onto
    /// the end of `out`.
    pub(crate) fn drain_into(&mut self, out: &mut Vec<f32>) {
        let n = self.rx.slots();
        if n == 0 {
            return;
        }
        if let Ok(chunk) = self.rx.read_chunk(n) {
            let (a, b) = chunk.as_slices();
            out.extend_from_slice(a);
            out.extend_from_slice(b);
            chunk.commit_all();
        }
    }
}

/// A track put on a deck, as the log and the cue sheet record it.
#[derive(Clone, Debug)]
struct Loaded {
    deck: usize,
    artist: String,
    title: String,
    path: Option<PathBuf>,
    bpm: f64,
    key: String,
}

impl Loaded {
    fn new(deck: usize, track: &Track) -> Self {
        Self {
            deck,
            artist: track.artist.clone(),
            title: track.title.clone(),
            path: match &track.source {
                Source::File(p) => Some(p.clone()),
                _ => None,
            },
            bpm: track.bpm,
            key: track.key.clone(),
        }
    }
}

/// One file of a recording, and the frame it starts at.
#[derive(Clone, Debug)]
struct Part {
    path: PathBuf,
    start: u64,
}

struct Session {
    stop: Arc<AtomicBool>,
    writer: JoinHandle<()>,
    logger: JoinHandle<()>,
    loads: Sender<Loaded>,
    first_file: PathBuf,
    parts: Arc<Mutex<Vec<Part>>>,
}

struct State {
    settings: RecordSettings,
    feed: Option<Feed>,
    session: Option<Session>,
    last_error: Option<String>,
    last_file: Option<PathBuf>,
    /// Every file of the last recording, once it has stopped.
    last_parts: Vec<PathBuf>,
    /// Bumped each time a recording stops, so the browser can pick up the
    /// new files.
    finished: u64,
}

struct Inner {
    rate: f64,
    shared: Arc<Shared>,
    registry: Weak<ControlRegistry>,
    loaded: LoadedTracks,
    status: ControlHandle,
    duration: ControlHandle,
    state: Mutex<State>,
}

/// Starts and stops recordings. Cheap to clone.
#[derive(Clone)]
pub struct Recorder(Arc<Inner>);

impl Recorder {
    /// Creates the `[Recording]` controls and a thread that answers
    /// `[Recording],toggle_recording`.
    pub(crate) fn new(registry: &Arc<ControlRegistry>, rate: f64, feed: Feed, loaded: LoadedTracks) -> Self {
        let toggle = registry.push("[Recording]", "toggle_recording");
        let inner = Arc::new(Inner {
            rate,
            shared: feed.shared.clone(),
            registry: Arc::downgrade(registry),
            loaded,
            status: registry.indicator("[Recording]", "status", 2.0),
            duration: registry.indicator("[Recording]", "duration", f64::MAX),
            state: Mutex::new(State {
                settings: RecordSettings::default(),
                feed: Some(feed),
                session: None,
                last_error: None,
                last_file: None,
                last_parts: Vec::new(),
                finished: 0,
            }),
        });
        let weak = Arc::downgrade(&inner);
        std::thread::Builder::new()
            .name("record-toggle".into())
            .spawn(move || {
                let mut seen = toggle.version();
                while let Some(inner) = weak.upgrade() {
                    let v = toggle.version();
                    // A press sets 1 then 0; between two looks either may show.
                    if v != seen && (toggle.get_bool() || v.wrapping_sub(seen) >= 2) {
                        let rec = Recorder(inner);
                        if rec.is_recording() {
                            rec.stop();
                        } else if let Err(e) = rec.start() {
                            eprintln!("could not start recording: {e}");
                        }
                        toggle.set(0.0);
                    }
                    seen = toggle.version();
                    std::thread::sleep(Duration::from_millis(30));
                }
            })
            .expect("spawn record toggle");
        Self(inner)
    }

    pub fn settings(&self) -> RecordSettings {
        self.0.state.lock().unwrap().settings.clone()
    }

    /// Takes effect from the next recording.
    pub fn configure(&self, settings: RecordSettings) {
        self.0.state.lock().unwrap().settings = settings;
    }

    pub fn is_recording(&self) -> bool {
        self.0.state.lock().unwrap().session.is_some()
    }

    /// Seconds recorded so far.
    pub fn elapsed(&self) -> f64 {
        self.0.shared.frames.load(Ordering::Acquire) as f64 / self.0.rate
    }

    /// The file being written, or the last one written.
    pub fn file(&self) -> Option<PathBuf> {
        let s = self.0.state.lock().unwrap();
        s.session.as_ref().map(|x| x.first_file.clone()).or_else(|| s.last_file.clone())
    }

    /// Why the last recording failed, if it did.
    pub fn last_error(&self) -> Option<String> {
        self.0.state.lock().unwrap().last_error.clone()
    }

    /// Starts recording the master to a new file in the settings' folder.
    /// Returns the file's path.
    pub fn start(&self) -> Result<PathBuf, String> {
        let mut state = self.0.state.lock().unwrap();
        if let Some(s) = &state.session {
            return Ok(s.first_file.clone());
        }
        let settings = state.settings.clone();
        std::fs::create_dir_all(&settings.dir).map_err(|e| format!("{}: {e}", settings.dir.display()))?;
        let base = unique_base(&settings.dir, &chrono::Local::now(), settings.format);
        let first_file = audio_path(&base, 1, settings.format);
        let sink =
            open_sink(&first_file, &settings, self.0.rate).map_err(|e| format!("{}: {e}", first_file.display()))?;
        let log = File::create(sibling(&base, ".controls.jsonl")).map_err(|e| format!("{}: {e}", base.display()))?;
        let Some(mut feed) = state.feed.take() else {
            return Err("the recorder is busy".into());
        };
        // Anything left over from the last recording.
        while feed.rx.pop().is_ok() {}
        self.0.shared.frames.store(0, Ordering::Release);
        self.0.shared.dropped.store(0, Ordering::Relaxed);

        let stop = Arc::new(AtomicBool::new(false));
        let parts: Arc<Mutex<Vec<Part>>> = Arc::new(Mutex::new(vec![Part { path: first_file.clone(), start: 0 }]));
        let (loads, loads_rx) = mpsc::channel();
        for (deck, t) in self.0.loaded.read().unwrap().iter().enumerate() {
            if let Some(t) = t {
                let _ = loads.send(Loaded::new(deck, t));
            }
        }
        let writer = {
            let inner = self.0.clone();
            let stop = stop.clone();
            let parts = parts.clone();
            let base = base.clone();
            std::thread::Builder::new()
                .name("record-writer".into())
                .spawn(move || write_audio(inner, feed, sink, settings, base, parts, stop))
                .map_err(|e| e.to_string())?
        };
        let logger = {
            let inner = self.0.clone();
            let stop = stop.clone();
            let parts = parts.clone();
            std::thread::Builder::new()
                .name("record-log".into())
                .spawn(move || log_controls(inner, log, loads_rx, base, parts, stop))
                .map_err(|e| e.to_string())?
        };
        self.0.shared.active.store(true, Ordering::Release);
        self.0.status.set(2.0);
        state.last_error = None;
        state.session = Some(Session { stop, writer, logger, loads, first_file: first_file.clone(), parts });
        Ok(first_file)
    }

    /// Stops recording and finishes the files. Blocks until they are written.
    pub fn stop(&self) {
        let session = self.0.state.lock().unwrap().session.take();
        let Some(session) = session else { return };
        self.0.shared.active.store(false, Ordering::Release);
        session.stop.store(true, Ordering::Release);
        let _ = session.writer.join();
        let _ = session.logger.join();
        self.0.status.set(0.0);
        let parts = session.parts.lock().unwrap().iter().map(|p| p.path.clone()).collect();
        let mut state = self.0.state.lock().unwrap();
        state.last_file = Some(session.first_file);
        state.last_parts = parts;
        state.finished += 1;
    }

    /// How many recordings have finished since the app started, and the
    /// files of the last one (its parts, in order; the `.cue` and
    /// `.tracklist.txt` sit beside the first).
    pub fn finished(&self) -> (u64, Vec<PathBuf>) {
        let s = self.0.state.lock().unwrap();
        (s.finished, s.last_parts.clone())
    }

    /// Notes a track going onto a deck, for the log and the cue sheet.
    pub(crate) fn track_loaded(&self, deck: usize, track: &Track) {
        if let Some(s) = &self.0.state.lock().unwrap().session {
            let _ = s.loads.send(Loaded::new(deck, track));
        }
    }

    fn fail(&self, e: String) {
        eprintln!("recording failed: {e}");
        self.0.state.lock().unwrap().last_error = Some(e);
    }
}

/// `<dir>/DJSugar 2026-10-19 21.05`, with " 2", " 3"... if that is taken.
fn unique_base(dir: &Path, now: &chrono::DateTime<chrono::Local>, format: RecordFormat) -> PathBuf {
    let stem = format!("DJ Sugar {}", now.format("%Y-%m-%d %H.%M"));
    let mut n = 1;
    loop {
        let name = if n == 1 { stem.clone() } else { format!("{stem} {n}") };
        let base = dir.join(name);
        if !audio_path(&base, 1, format).exists() {
            return base;
        }
        n += 1;
    }
}

/// `base` with `suffix` added to its name (not replacing what follows a
/// dot, as the time in the name has one).
fn sibling(base: &Path, suffix: &str) -> PathBuf {
    let mut name = base.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    base.with_file_name(name)
}

/// The audio file for `part` (from 1) of the recording at `base`.
fn audio_path(base: &Path, part: usize, format: RecordFormat) -> PathBuf {
    let mut name = base.file_name().unwrap_or_default().to_os_string();
    if part > 1 {
        name.push(format!(" (part {part})"));
    }
    name.push(".");
    name.push(format.name());
    base.with_file_name(name)
}

// ---------------------------------------------------------------- writing audio

trait Sink: Send {
    /// Interleaved stereo, -1..1.
    fn write(&mut self, samples: &[f32]) -> io::Result<()>;
    /// Makes the file on disk playable as far as it goes.
    fn sync(&mut self) -> io::Result<()>;
    fn finish(self: Box<Self>) -> io::Result<()>;
    /// Whether the next `frames` would take the file past what it can hold.
    fn full(&self, frames: usize) -> bool;
}

fn open_sink(path: &Path, s: &RecordSettings, rate: f64) -> io::Result<Box<dyn Sink>> {
    let bits = if s.bits <= 16 { 16 } else { 24 };
    Ok(match s.format {
        RecordFormat::Wav => Box::new(WavSink::create(path, rate as u32, bits)?),
        RecordFormat::Flac => Box::new(FlacSink::create(path, rate as u32, bits)?),
    })
}

fn to_int(s: f32, bits: u16) -> i32 {
    let full = ((1i32 << (bits - 1)) - 1) as f32;
    (s.clamp(-1.0, 1.0) * full).round() as i32
}

struct WavSink {
    out: BufWriter<File>,
    bits: u16,
    data_bytes: u64,
}

impl WavSink {
    fn create(path: &Path, rate: u32, bits: u16) -> io::Result<Self> {
        let mut out = BufWriter::new(File::create(path)?);
        let block = 2 * bits / 8;
        out.write_all(b"RIFF")?;
        out.write_all(&36u32.to_le_bytes())?;
        out.write_all(b"WAVEfmt ")?;
        out.write_all(&16u32.to_le_bytes())?;
        out.write_all(&1u16.to_le_bytes())?; // PCM
        out.write_all(&2u16.to_le_bytes())?;
        out.write_all(&rate.to_le_bytes())?;
        out.write_all(&(rate * block as u32).to_le_bytes())?;
        out.write_all(&block.to_le_bytes())?;
        out.write_all(&bits.to_le_bytes())?;
        out.write_all(b"data")?;
        out.write_all(&0u32.to_le_bytes())?;
        Ok(Self { out, bits, data_bytes: 0 })
    }
}

impl Sink for WavSink {
    fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        let bytes = self.bits as usize / 8;
        for &s in samples {
            self.out.write_all(&to_int(s, self.bits).to_le_bytes()[..bytes])?;
        }
        self.data_bytes += (samples.len() * bytes) as u64;
        Ok(())
    }

    fn sync(&mut self) -> io::Result<()> {
        let data = self.data_bytes as u32;
        self.out.seek(SeekFrom::Start(4))?;
        self.out.write_all(&(36 + data).to_le_bytes())?;
        self.out.seek(SeekFrom::Start(40))?;
        self.out.write_all(&data.to_le_bytes())?;
        self.out.seek(SeekFrom::End(0))?;
        self.out.flush()
    }

    fn finish(mut self: Box<Self>) -> io::Result<()> {
        self.sync()
    }

    fn full(&self, frames: usize) -> bool {
        self.data_bytes + (frames * 2 * self.bits as usize / 8) as u64 > WAV_LIMIT
    }
}

/// Writes a stereo FLAC file, 16-bit.
pub(crate) fn write_flac(path: &Path, rate: u32, channels: [&[f32]; 2]) -> io::Result<()> {
    let mut sink = Box::new(FlacSink::create(path, rate, 16)?);
    let mut buf = Vec::with_capacity(FLAC_BLOCK * 2);
    for (l, r) in channels[0].chunks(FLAC_BLOCK).zip(channels[1].chunks(FLAC_BLOCK)) {
        buf.clear();
        for (a, b) in l.iter().zip(r) {
            buf.extend_from_slice(&[*a, *b]);
        }
        sink.write(&buf)?;
    }
    sink.finish()
}

/// Samples per FLAC frame.
const FLAC_BLOCK: usize = 4096;

struct FlacSink {
    out: BufWriter<File>,
    bits: u16,
    rate: u32,
    config: flacenc::error::Verified<flacenc::config::Encoder>,
    info: flacenc::component::StreamInfo,
    fb: flacenc::source::FrameBuf,
    fb_size: usize,
    pending: Vec<i32>,
    frame_number: usize,
}

impl FlacSink {
    fn create(path: &Path, rate: u32, bits: u16) -> io::Result<Self> {
        use flacenc::error::Verify;
        let bad = |e: String| io::Error::new(io::ErrorKind::InvalidInput, e);
        let mut config = flacenc::config::Encoder::default();
        config.block_size = FLAC_BLOCK;
        config.multithread = false;
        let config = config.into_verified().map_err(|e| bad(format!("{e:?}")))?;
        let info =
            flacenc::component::StreamInfo::new(rate as usize, 2, bits as usize).map_err(|e| bad(format!("{e:?}")))?;
        let fb = flacenc::source::FrameBuf::with_size(2, FLAC_BLOCK).map_err(|e| bad(format!("{e:?}")))?;
        let mut sink = Self {
            out: BufWriter::new(File::create(path)?),
            bits,
            rate,
            config,
            info,
            fb,
            fb_size: FLAC_BLOCK,
            pending: Vec::with_capacity(FLAC_BLOCK * 2),
            frame_number: 0,
        };
        sink.out.write_all(b"fLaC")?;
        sink.write_stream_info()?;
        Ok(sink)
    }

    /// The STREAMINFO block, the only metadata, written by hand so it can be
    /// rewritten in place as the file grows.
    fn write_stream_info(&mut self) -> io::Result<()> {
        let i = &self.info;
        let (min_frame, max_frame) =
            if i.max_frame_size() == 0 { (0u32, 0u32) } else { (i.min_frame_size() as u32, i.max_frame_size() as u32) };
        let mut b = Vec::with_capacity(38);
        b.extend_from_slice(&[0x80, 0, 0, 34]); // last block, STREAMINFO, 34 bytes
        b.extend_from_slice(&(FLAC_BLOCK as u16).to_be_bytes()); // min block (fixed size)
        b.extend_from_slice(&(FLAC_BLOCK as u16).to_be_bytes());
        b.extend_from_slice(&min_frame.to_be_bytes()[1..]);
        b.extend_from_slice(&max_frame.to_be_bytes()[1..]);
        // 20 bits rate, 3 bits channels - 1, 5 bits bits - 1, 36 bits samples.
        let total = i.total_samples() as u64 & 0xF_FFFF_FFFF;
        let packed: u64 = (u64::from(self.rate) << 44) | (1u64 << 41) | (u64::from(self.bits - 1) << 36) | total;
        b.extend_from_slice(&packed.to_be_bytes());
        b.extend_from_slice(&[0; 16]); // no MD5
        self.out.write_all(&b)
    }

    fn encode(&mut self, samples: &[i32]) -> io::Result<()> {
        use flacenc::component::BitRepr;
        use flacenc::source::Fill;
        let bad = |e: String| io::Error::other(e);
        let frames = samples.len() / 2;
        if self.fb_size != frames {
            self.fb.resize(frames);
            self.fb_size = frames;
        }
        self.fb.fill_interleaved(samples).map_err(|e| bad(format!("{e:?}")))?;
        let frame = flacenc::encode_fixed_size_frame(&self.config, &self.fb, self.frame_number, &self.info)
            .map_err(|e| bad(format!("{e:?}")))?;
        self.frame_number += 1;
        self.info.update_frame_info(&frame);
        let mut sink = flacenc::bitsink::ByteSink::new();
        frame.write(&mut sink).map_err(|e| bad(format!("{e:?}")))?;
        self.out.write_all(sink.as_slice())
    }
}

impl Sink for FlacSink {
    fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        for &s in samples {
            self.pending.push(to_int(s, self.bits));
            if self.pending.len() == FLAC_BLOCK * 2 {
                let block = std::mem::take(&mut self.pending);
                self.encode(&block)?;
                self.pending = block;
                self.pending.clear();
            }
        }
        Ok(())
    }

    fn sync(&mut self) -> io::Result<()> {
        self.out.seek(SeekFrom::Start(4))?;
        self.write_stream_info()?;
        self.out.seek(SeekFrom::End(0))?;
        self.out.flush()
    }

    fn finish(mut self: Box<Self>) -> io::Result<()> {
        if !self.pending.is_empty() {
            let block = std::mem::take(&mut self.pending);
            self.encode(&block)?;
        }
        self.sync()
    }

    fn full(&self, _frames: usize) -> bool {
        false
    }
}

fn write_audio(
    inner: Arc<Inner>,
    mut feed: Feed,
    mut sink: Box<dyn Sink>,
    settings: RecordSettings,
    base: PathBuf,
    parts: Arc<Mutex<Vec<Part>>>,
    stop: Arc<AtomicBool>,
) {
    let rec = Recorder(inner.clone());
    let mut buf = Vec::with_capacity(16_384);
    let mut written: u64 = 0;
    let mut last_sync = Instant::now();
    let mut result = Ok(());
    loop {
        // Read the stop flag before emptying the buffer, so nothing the
        // audio thread pushed before it stopped is left behind.
        let stopping = stop.load(Ordering::Acquire);
        let n = feed.rx.slots() & !1;
        if n > 0 {
            buf.clear();
            if let Ok(chunk) = feed.rx.read_chunk(n) {
                let (a, b) = chunk.as_slices();
                buf.extend_from_slice(a);
                buf.extend_from_slice(b);
                chunk.commit_all();
            }
            let frames = buf.len() / 2;
            if sink.full(frames) {
                let part = parts.lock().unwrap().len() + 1;
                let path = audio_path(&base, part, settings.format);
                let next = match open_sink(&path, &settings, inner.rate) {
                    Ok(next) => next,
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                };
                if let Err(e) = std::mem::replace(&mut sink, next).finish() {
                    result = Err(e);
                    break;
                }
                parts.lock().unwrap().push(Part { path, start: written });
            }
            if let Err(e) = sink.write(&buf) {
                result = Err(e);
                break;
            }
            written += frames as u64;
            inner.duration.set(written as f64 / inner.rate);
        }
        if stopping {
            // One more buffer may land after the flag went down; it is
            // picked up above on the way round.
            if n == 0 {
                break;
            }
            continue;
        }
        if last_sync.elapsed() >= SYNC_EVERY {
            last_sync = Instant::now();
            if let Err(e) = sink.sync() {
                result = Err(e);
                break;
            }
        }
        if n == 0 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let result = match result {
        Ok(()) => sink.finish(),
        Err(e) => Err(e),
    };
    let dropped = inner.shared.dropped.load(Ordering::Relaxed);
    if dropped > 0 {
        eprintln!("recording: {:.2} s of audio lost while the disk was busy", dropped as f64 / inner.rate);
    }
    // Hand the buffer back for the next recording.
    inner.state.lock().unwrap().feed = Some(feed);
    if let Err(e) = result {
        rec.fail(e.to_string());
        // Stopping joins this thread, so leave that to another one.
        if !stop.load(Ordering::Acquire) {
            std::thread::spawn(move || rec.stop());
        }
    }
}

// ---------------------------------------------------------------- the control log

struct Watched {
    handle: ControlHandle,
    name: String,
    version: u32,
}

fn watched(registry: &ControlRegistry, known: &mut Vec<Watched>) -> Vec<usize> {
    let mut added = Vec::new();
    let keys = registry.keys();
    if keys.len() == known.len() {
        return added;
    }
    for key in keys {
        if key.group == "[Recording]" || NOT_LOGGED.contains(&key.item.as_str()) {
            continue;
        }
        let name = key.to_string();
        if known.iter().any(|w| w.name == name) {
            continue;
        }
        let Some(handle) = registry.get(&key.group, &key.item) else {
            continue;
        };
        if handle.behavior() == Behavior::Indicator && !logged_indicator(&key.item) {
            continue;
        }
        added.push(known.len());
        known.push(Watched { version: handle.version(), handle, name });
    }
    added
}

fn logged_indicator(item: &str) -> bool {
    LOGGED_INDICATORS.contains(&item)
        || item
            .strip_prefix("hotcue_")
            .and_then(|r| r.strip_suffix("_position"))
            .is_some_and(|n| n.parse::<u32>().is_ok())
}

/// Whether a deck that went from `from` to `to` seconds into its track
/// while `dt` seconds were recorded skipped rather than played there.
fn jumped(from: f64, to: f64, dt: f64) -> bool {
    (to - from).abs() > dt * MAX_SPEED + JUMP_SECONDS
}

/// A track on the cue sheet: when it became audible.
struct CueEntry {
    frame: u64,
    track: Loaded,
}

fn log_controls(
    inner: Arc<Inner>,
    file: File,
    loads: Receiver<Loaded>,
    base: PathBuf,
    parts: Arc<Mutex<Vec<Part>>>,
    stop: Arc<AtomicBool>,
) {
    let mut out = BufWriter::new(file);
    let Some(registry) = inner.registry.upgrade() else {
        return;
    };
    let rate = inner.rate;
    let now = || inner.shared.frames.load(Ordering::Acquire);
    let line = |out: &mut BufWriter<File>, v: Value| {
        let _ = serde_json::to_writer(&mut *out, &v);
        let _ = out.write_all(b"\n");
    };
    let started = chrono::Local::now();
    line(
        &mut out,
        json!({
            "djsugar_recording": 1,
            "started": started.to_rfc3339(),
            "sample_rate": rate,
            "audio": audio_name(&parts.lock().unwrap()[0].path),
        }),
    );
    let mut known = Vec::new();
    watched(&registry, &mut known);
    let snapshot: serde_json::Map<String, Value> =
        known.iter().map(|w| (w.name.clone(), json!(w.handle.get()))).collect();
    line(&mut out, json!({ "t": 0.0, "snapshot": snapshot }));

    let decks: [(ControlHandle, ControlHandle); NUM_DECKS] = std::array::from_fn(|d| {
        let g = deck_group(d);
        (registry.toggle(&g, "play"), registry.create(&g, "volume", 1.0, 0.0, 1.0, Behavior::Linear))
    });
    let places: [(ControlHandle, ControlHandle, ControlHandle); NUM_DECKS] = std::array::from_fn(|d| {
        let g = deck_group(d);
        (
            registry.create(&g, "playposition", 0.0, 0.0, 1.0, Behavior::Linear),
            registry.indicator(&g, "duration", 3600.0),
            registry.indicator(&g, "track_loaded", 1.0),
        )
    });
    // Per deck: the place last looked at, when, and when it was last logged.
    let mut place: [Option<(f64, f64, f64)>; NUM_DECKS] = [None; NUM_DECKS];
    let mut on_deck: [Option<(Loaded, bool)>; NUM_DECKS] = Default::default();
    let mut cues: Vec<CueEntry> = Vec::new();
    let mut last_rescan = Instant::now();
    let mut last_flush = Instant::now();

    loop {
        let stopping = stop.load(Ordering::Acquire);
        let frame = now();
        let t = (frame as f64 / rate * 1000.0).round() / 1000.0;
        while let Ok(l) = loads.try_recv() {
            line(
                &mut out,
                json!({
                    "t": t,
                    "load": {
                        "deck": l.deck + 1,
                        "artist": l.artist,
                        "title": l.title,
                        "path": l.path.as_ref().map(|p| p.display().to_string()),
                        "bpm": l.bpm,
                        "key": l.key,
                    }
                }),
            );
            if l.deck < NUM_DECKS {
                on_deck[l.deck] = Some((l.clone(), false));
            }
        }
        if last_rescan.elapsed() >= Duration::from_secs(1) {
            last_rescan = Instant::now();
            for i in watched(&registry, &mut known) {
                let w = &known[i];
                line(&mut out, json!({ "t": t, "c": w.name, "v": w.handle.get() }));
            }
        }
        for w in &mut known {
            let v = w.handle.version();
            if v != w.version {
                w.version = v;
                line(&mut out, json!({ "t": t, "c": w.name, "v": w.handle.get() }));
            }
        }
        for (d, (position, duration, loaded)) in places.iter().enumerate() {
            if !loaded.get_bool() || duration.get() <= 0.0 {
                place[d] = None;
                continue;
            }
            let s = position.get() * duration.get();
            let log = match place[d] {
                None => true,
                Some((from, seen, logged)) => jumped(from, s, t - seen) || t - logged >= POS_EVERY,
            };
            let logged = if log {
                let s = (s * 1000.0).round() / 1000.0;
                line(&mut out, json!({ "t": t, "pos": { "deck": d + 1, "s": s } }));
                t
            } else {
                place[d].map_or(t, |p| p.2)
            };
            place[d] = Some((s, t, logged));
        }
        for (d, (play, volume)) in decks.iter().enumerate() {
            if let Some((track, cued)) = &mut on_deck[d] {
                if !*cued && play.get_bool() && volume.get() > AUDIBLE_VOLUME {
                    *cued = true;
                    cues.push(CueEntry { frame, track: track.clone() });
                }
            }
        }
        if last_flush.elapsed() >= SYNC_EVERY {
            last_flush = Instant::now();
            let _ = out.flush();
        }
        if stopping {
            break;
        }
        std::thread::sleep(LOG_EVERY);
    }
    line(&mut out, json!({ "t": now() as f64 / rate, "end": true }));
    let _ = out.flush();
    let parts = parts.lock().unwrap().clone();
    if let Err(e) = write_cue(&base, &started, &parts, &cues, rate) {
        eprintln!("could not write the cue sheet: {e}");
    }
}

fn audio_name(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

fn cue_text(s: &str) -> String {
    s.replace('"', "'")
}

/// `mm:ss:ff`, ff in 75ths of a second, as CUE sheets count.
fn cue_time(seconds: f64) -> String {
    let frames = (seconds.max(0.0) * 75.0).round() as u64;
    format!("{:02}:{:02}:{:02}", frames / 75 / 60, frames / 75 % 60, frames % 75)
}

fn write_cue(
    base: &Path,
    started: &chrono::DateTime<chrono::Local>,
    parts: &[Part],
    cues: &[CueEntry],
    rate: f64,
) -> io::Result<()> {
    let mut s = String::new();
    s.push_str(&format!("REM DATE {}\n", started.format("%Y-%m-%d %H:%M")));
    s.push_str("REM COMMENT \"Recorded with DJ Sugar\"\n");
    s.push_str("PERFORMER \"DJ Sugar\"\n");
    s.push_str(&format!("TITLE \"{}\"\n", cue_text(&audio_name(base))));
    let mut current = usize::MAX;
    for (n, c) in cues.iter().enumerate() {
        let p = parts.iter().rposition(|p| p.start <= c.frame).unwrap_or(0);
        if p != current {
            current = p;
            s.push_str(&format!("FILE \"{}\" WAVE\n", cue_text(&audio_name(&parts[p].path))));
        }
        let at = (c.frame - parts[p].start) as f64 / rate;
        s.push_str(&format!("  TRACK {:02} AUDIO\n", n + 1));
        s.push_str(&format!("    TITLE \"{}\"\n", cue_text(&c.track.title)));
        s.push_str(&format!("    PERFORMER \"{}\"\n", cue_text(&c.track.artist)));
        if let Some(path) = &c.track.path {
            s.push_str(&format!("    REM FILE \"{}\"\n", cue_text(&path.display().to_string())));
        }
        s.push_str(&format!("    INDEX 01 {}\n", cue_time(at)));
    }
    if cues.is_empty() {
        s.push_str(&format!("FILE \"{}\" WAVE\n", cue_text(&audio_name(&parts[0].path))));
    }
    std::fs::write(sibling(base, ".cue"), s)?;
    std::fs::write(sibling(base, ".tracklist.txt"), tracklist_text(cues, rate))
}

/// `0:00 Artist - Title` per track, timed from the start of the recording,
/// ready to paste into a show's description.
fn tracklist_text(cues: &[CueEntry], rate: f64) -> String {
    let mut out = String::new();
    for c in cues {
        let s = (c.frame as f64 / rate) as u64;
        let at = if s >= 3600 {
            format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
        } else {
            format!("{}:{:02}", s / 60, s % 60)
        };
        let name = match (c.track.artist.trim(), c.track.title.trim()) {
            ("", t) => t.to_string(),
            (a, "") => a.to_string(),
            (a, t) => format!("{a} - {t}"),
        };
        out.push_str(&format!("{at} {name}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_times_count_75ths() {
        assert_eq!(cue_time(0.0), "00:00:00");
        assert_eq!(cue_time(61.5), "01:01:38");
        assert_eq!(cue_time(3725.0), "62:05:00");
    }

    /// Records a deck playing, in both formats, and reads the files back.
    #[test]
    fn records_master_log_and_cue() {
        use crate::engine::{Command, Engine};
        use crate::loader::prepare;

        const SR: f64 = 48_000.0;
        for (format, bits) in
            [(RecordFormat::Wav, 24), (RecordFormat::Wav, 16), (RecordFormat::Flac, 24), (RecordFormat::Flac, 16)]
        {
            let dir =
                std::env::temp_dir().join(format!("djsugar-rec-test-{}-{}-{bits}", std::process::id(), format.name()));
            let _ = std::fs::remove_dir_all(&dir);
            let reg = ControlRegistry::new();
            let (mut engine, mut input) = Engine::new(&reg, SR);
            let feed = engine.take_record_feed().unwrap();
            let loaded: LoadedTracks = Default::default();
            let rec = Recorder::new(&reg, SR, feed, loaded);
            rec.configure(RecordSettings { dir: dir.clone(), format, bits });
            let mut t = (*crate::synth::sample_library()[0]).clone();
            t.title = "Test \"Tone\"".into();
            t.artist = "Someone".into();
            let track = Arc::new(prepare(&t).unwrap());

            let file = rec.start().unwrap();
            assert_eq!(reg.get("[Recording]", "status").unwrap().get(), 2.0);
            let mut buf = vec![0.0f32; 512 * 6];
            let mut run = |engine: &mut Engine, seconds: f64| {
                for _ in 0..(seconds * SR / 512.0) as usize {
                    engine.process(&mut buf, 6);
                    // As fast as real time would be too slow for a test,
                    // but the writer must keep up with an 8 s buffer.
                    std::thread::sleep(Duration::from_micros(200));
                }
            };
            run(&mut engine, 0.5);
            rec.track_loaded(0, &track);
            input.send(Command::Load { deck: 0, track: track.clone() }).ok().unwrap();
            run(&mut engine, 0.1);
            reg.get("[Channel1]", "play").unwrap().set_bool(true);
            run(&mut engine, 0.9);
            reg.get("[Channel1]", "volume").unwrap().set(0.5);
            run(&mut engine, 0.5);
            // A seek on the waveform: the engine only sees playposition move.
            reg.get("[Channel1]", "playposition").unwrap().set(0.5);
            run(&mut engine, 0.5);
            rec.stop();
            assert_eq!(reg.get("[Recording]", "status").unwrap().get(), 0.0);
            let seconds = reg.get("[Recording]", "duration").unwrap().get();
            assert!((seconds - 2.5).abs() < 0.05, "{seconds}");

            // The audio decodes, is as long as recorded and isn't silent.
            let audio = crate::decode::decode(&file).unwrap();
            let frames = audio.len();
            let peak = audio.decoded_chunks().flatten().fold(0.0f32, |p, f| p.max(f[0].abs()).max(f[1].abs()));
            assert!((frames as f64 / SR - 2.5).abs() < 0.05, "{frames}");
            assert!(peak > 0.05, "{peak}");

            let base = file.with_extension("");
            let log = std::fs::read_to_string(sibling(&base, ".controls.jsonl")).unwrap();
            let lines: Vec<Value> = log.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
            assert_eq!(lines[0]["djsugar_recording"], 1);
            assert!(lines[1]["snapshot"]["[Channel1],volume"].is_number());
            assert!(lines.iter().any(|l| l["load"]["title"] == "Test \"Tone\""));
            let volume = lines.iter().find(|l| l["c"] == "[Channel1],volume").expect("volume change logged");
            // The test runs faster than real time, so the logger's 5 ms
            // polls are further apart in recorded time.
            let at = volume["t"].as_f64().unwrap();
            assert!((at - 1.5).abs() < 0.25, "{at}");
            assert!(!log.contains("playposition\""));
            // The deck's place is logged, including right after the seek.
            let places: Vec<f64> =
                lines.iter().filter(|l| l["pos"]["deck"] == 1).map(|l| l["pos"]["s"].as_f64().unwrap()).collect();
            assert!(places.len() >= 2, "{places:?}");
            let half = track.duration / 2.0;
            assert!(places.iter().any(|&s| (s - half).abs() < 1.0), "{places:?} {half}");

            let cue = std::fs::read_to_string(sibling(&base, ".cue")).unwrap();
            assert!(cue.contains("TITLE \"Test 'Tone'\""), "{cue}");
            assert!(cue.contains("INDEX 01 00:00:"), "{cue}");
            let list = std::fs::read_to_string(sibling(&base, ".tracklist.txt")).unwrap();
            assert!(list.starts_with("0:00 Someone - Test \"Tone\"\n"), "{list}");
            let (n, files) = rec.finished();
            assert_eq!(n, 1);
            assert_eq!(files, vec![file.clone()]);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn jumps_and_kept_indicators() {
        // 10 ms recorded: playing at normal or scratch speed isn't a jump.
        assert!(!jumped(10.0, 10.01, 0.01));
        assert!(!jumped(10.0, 9.97, 0.01));
        // A hot cue or a loop going back a beat is.
        assert!(jumped(10.0, 9.5, 0.01));
        assert!(jumped(10.0, 60.0, 0.01));
        assert!(logged_indicator("loop_start_position"));
        assert!(logged_indicator("hotcue_12_position"));
        assert!(!logged_indicator("hotcue_x_position"));
        assert!(!logged_indicator("VuMeter"));
    }

    #[test]
    fn parts_get_numbered() {
        let base = Path::new("/r/DJSugar 2026-10-19 21.05");
        assert_eq!(audio_path(base, 1, RecordFormat::Wav), Path::new("/r/DJSugar 2026-10-19 21.05.wav"));
        assert_eq!(audio_path(base, 2, RecordFormat::Flac), Path::new("/r/DJSugar 2026-10-19 21.05 (part 2).flac"));
    }
}
