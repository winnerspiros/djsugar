//! Shares tempo and beat phase with other apps and devices on the local network, joining Link sessions (music software, phones, drum machines). Each app keeps a timeline (tempo and
//! where beats fall in time). Peers find each other by multicast, measure each other's clocks, and settle on one session whose timeline they all follow; a change any of them commits reaches the rest.
//! [`LinkSession`] is shared between threads. The audio thread captures and commits a [`SessionState`] without blocking or allocating; enabling opens the sockets and starts a network thread.

mod net;
pub mod timeline;
pub mod wire;

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use timeline::{closest_phase_match, Timeline, MAX_BPM, MIN_BPM};

/// Microseconds on this process's monotonic clock, the clock every
/// timeline here is in.
fn now_micros() -> i64 {
    static START: OnceLock<Instant> = OnceLock::new();
    // Offset so times are never near zero, which the protocol treats as unset.
    START.get_or_init(Instant::now).elapsed().as_micros() as i64 + 1_000_000
}

/// The session timeline, readable and writable from any thread without
/// locks: a sequence number, odd while a write is under way.
struct SharedTimeline {
    seq: AtomicU64,
    writing: AtomicBool,
    bpm: AtomicU64,
    beat_origin: AtomicU64,
    time_origin: AtomicU64,
    /// The app's beats minus the session's: a whole number of bars, so the
    /// app can count from where it likes while the phase stays shared.
    offset: AtomicU64,
}

impl SharedTimeline {
    fn new(t: Timeline) -> Self {
        let s = Self {
            seq: AtomicU64::new(0),
            writing: AtomicBool::new(false),
            bpm: AtomicU64::new(0),
            beat_origin: AtomicU64::new(0),
            time_origin: AtomicU64::new(0),
            offset: AtomicU64::new(0),
        };
        s.write(t, 0.0);
        s
    }

    fn read(&self) -> (Timeline, f64) {
        loop {
            let before = self.seq.load(Ordering::Acquire);
            if before % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let t = Timeline {
                bpm: f64::from_bits(self.bpm.load(Ordering::Relaxed)),
                beat_origin: f64::from_bits(self.beat_origin.load(Ordering::Relaxed)),
                time_origin: f64::from_bits(self.time_origin.load(Ordering::Relaxed)),
            };
            let offset = f64::from_bits(self.offset.load(Ordering::Relaxed));
            std::sync::atomic::fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == before {
                return (t, offset);
            }
        }
    }

    fn write(&self, t: Timeline, offset: f64) {
        while self.writing.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
        self.seq.fetch_add(1, Ordering::AcqRel);
        self.bpm.store(t.bpm.to_bits(), Ordering::Relaxed);
        self.beat_origin.store(t.beat_origin.to_bits(), Ordering::Relaxed);
        self.time_origin.store(t.time_origin.to_bits(), Ordering::Relaxed);
        self.offset.store(offset.to_bits(), Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::Release);
        self.writing.store(false, Ordering::Release);
    }
}

pub(crate) struct Shared {
    timeline: SharedTimeline,
    /// Set by a commit; the network thread announces the change.
    committed: AtomicBool,
    peers: AtomicUsize,
}

/// A Link instance: one per app.
pub struct LinkSession {
    shared: Arc<Shared>,
    net: Mutex<Option<net::Running>>,
    enabled: AtomicBool,
}

impl LinkSession {
    /// A Link instance, off, with its timeline at `bpm`.
    pub fn new(bpm: f64) -> Self {
        let t = Timeline { bpm: bpm.clamp(MIN_BPM, MAX_BPM), beat_origin: 0.0, time_origin: now_micros() as f64 };
        Self {
            shared: Arc::new(Shared {
                timeline: SharedTimeline::new(t),
                committed: AtomicBool::new(false),
                peers: AtomicUsize::new(0),
            }),
            net: Mutex::new(None),
            enabled: AtomicBool::new(false),
        }
    }

    /// Joins the network (opening sockets, so not from the audio thread) or
    /// leaves it. The timeline keeps working either way.
    pub fn enable(&self, on: bool) {
        let mut net = self.net.lock().unwrap_or_else(|e| e.into_inner());
        if on && net.is_none() {
            match net::Running::start(self.shared.clone()) {
                Ok(r) => *net = Some(r),
                Err(e) => eprintln!("Link: can't open the network: {e}"),
            }
        } else if !on {
            if let Some(r) = net.take() {
                r.stop();
            }
            self.shared.peers.store(0, Ordering::Relaxed);
        }
        self.enabled.store(on, Ordering::Release);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Other apps in this session.
    pub fn num_peers(&self) -> usize {
        self.shared.peers.load(Ordering::Relaxed)
    }

    /// Now on the clock timelines are in, in microseconds.
    pub fn clock_micros(&self) -> i64 {
        now_micros()
    }

    /// Reads the session; real-time safe.
    pub fn capture_audio_session_state(&self, state: &mut SessionState) {
        let (t, offset) = self.shared.timeline.read();
        state.session = t;
        state.offset = offset;
    }

    /// Makes `state` the session's; real-time safe.
    pub fn commit_audio_session_state(&self, state: &SessionState) {
        self.shared.timeline.write(state.session, state.offset);
        self.shared.committed.store(true, Ordering::Release);
    }

    /// [`Self::capture_audio_session_state`] for other threads.
    pub fn capture_app_session_state(&self, state: &mut SessionState) {
        self.capture_audio_session_state(state);
    }

    /// [`Self::commit_audio_session_state`] for other threads.
    pub fn commit_app_session_state(&self, state: &SessionState) {
        self.commit_audio_session_state(state);
    }
}

impl Drop for LinkSession {
    fn drop(&mut self) {
        let net = self.net.get_mut().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = net.take() {
            r.stop();
        }
    }
}

/// A copy of the session to read and change, then commit.
#[derive(Clone, Copy, Debug)]
pub struct SessionState {
    session: Timeline,
    offset: f64,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    pub fn new() -> Self {
        Self { session: Timeline { bpm: 120.0, beat_origin: 0.0, time_origin: 0.0 }, offset: 0.0 }
    }

    pub fn tempo(&self) -> f64 {
        self.session.bpm
    }

    /// Changes the tempo from `at` (microseconds) on; the beat at `at` stays.
    pub fn set_tempo(&mut self, bpm: f64, at: i64) {
        let at = at as f64;
        self.session =
            Timeline { bpm: bpm.clamp(MIN_BPM, MAX_BPM), beat_origin: self.session.beats_at(at), time_origin: at };
    }

    /// The beat at `time`, in phase with the session for bars of `quantum`
    /// beats.
    pub fn beat_at_time(&self, time: i64, quantum: f64) -> f64 {
        let session = self.session.beats_at(time as f64);
        closest_phase_match(session + self.offset, session, quantum)
    }

    /// Puts `beat` at `time`, moving the whole session's phase with it, so
    /// every peer follows.
    pub fn force_beat_at_time(&mut self, beat: f64, time: i64, quantum: f64) {
        let t = time as f64;
        let current = self.beat_at_time(time, quantum);
        let closest = closest_phase_match(current, beat, quantum);
        self.session = Timeline {
            bpm: self.session.bpm,
            beat_origin: self.session.beats_at(t) + (closest - current),
            time_origin: t,
        };
        self.offset += beat - closest;
    }
}

/// Smooths the audio callback's timing into host time: a least-squares line
/// through the last few hundred (frames rendered, host time) pairs, so the
/// time a buffer plays doesn't jitter with the callback's scheduling.
pub struct HostTimeFilter {
    points: Box<[(f64, f64)]>,
    len: usize,
    next: usize,
}

const FILTER_POINTS: usize = 512;

impl Default for HostTimeFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl HostTimeFilter {
    pub fn new() -> Self {
        Self { points: vec![(0.0, 0.0); FILTER_POINTS].into_boxed_slice(), len: 0, next: 0 }
    }

    pub fn reset(&mut self) {
        self.len = 0;
        self.next = 0;
    }

    /// Records that `sample_time` frames had been rendered at `host_time`
    /// and returns the smoothed host time for it.
    pub fn sample_time_to_host_time(&mut self, host_time: i64, sample_time: u64) -> i64 {
        let x = sample_time as f64;
        self.points[self.next] = (x, host_time as f64);
        self.next = (self.next + 1) % FILTER_POINTS;
        self.len = (self.len + 1).min(FILTER_POINTS);
        let points = &self.points[..self.len];
        let n = self.len as f64;
        let (mx, my) = points.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x / n, sy + y / n));
        let (sxy, sxx) = points
            .iter()
            .fold((0.0, 0.0), |(sxy, sxx), (px, py)| (sxy + (px - mx) * (py - my), sxx + (px - mx) * (px - mx)));
        if sxx == 0.0 {
            return host_time;
        }
        let slope = sxy / sxx;
        (my + slope * (x - mx)).round() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_change_keeps_the_beat() {
        let mut s = SessionState::new();
        s.set_tempo(120.0, 0);
        let b = s.beat_at_time(2_000_000, 4.0);
        s.set_tempo(90.0, 2_000_000);
        assert!((s.beat_at_time(2_000_000, 4.0) - b).abs() < 1e-9);
        assert!((s.beat_at_time(4_000_000, 4.0) - b - 3.0).abs() < 1e-9);
    }

    #[test]
    fn forced_beat_is_exact_and_moves_the_phase() {
        let mut s = SessionState::new();
        s.force_beat_at_time(17.25, 3_000_000, 4.0);
        assert!((s.beat_at_time(3_000_000, 4.0) - 17.25).abs() < 1e-9);
        // The session itself moved to the same phase within the bar.
        let session = s.session.beats_at(3_000_000.0);
        assert!((session.rem_euclid(4.0) - 1.25).abs() < 1e-9);
    }

    #[test]
    fn commit_reaches_capture() {
        let link = LinkSession::new(120.0);
        let mut s = SessionState::new();
        link.capture_app_session_state(&mut s);
        s.set_tempo(128.0, link.clock_micros());
        link.commit_app_session_state(&s);
        let mut back = SessionState::new();
        link.capture_audio_session_state(&mut back);
        assert_eq!(back.tempo(), 128.0);
    }

    #[test]
    fn filter_follows_a_steady_clock_through_jitter() {
        let mut f = HostTimeFilter::new();
        let mut last = 0;
        for i in 0..2000u64 {
            let jitter = if i % 2 == 0 { 300 } else { -300 };
            let host = 1_000_000 + (i * 512 * 1_000_000 / 48_000) as i64 + jitter;
            last = f.sample_time_to_host_time(host, i * 512);
        }
        let exact = 1_000_000 + (1999i64 * 512 * 1_000_000 / 48_000);
        assert!((last - exact).abs() < 50, "{last} vs {exact}");
    }
}
