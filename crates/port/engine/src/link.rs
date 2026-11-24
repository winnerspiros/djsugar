//! Link: shares tempo and beat phase with other apps and devices on the network (music software, phones, drum machines). With `[Link],enabled` on, Link follows the master deck: the session takes
//! the tempo and beat of the deck that is playing (`[Link],master_deck`), so other apps play along with the mix. The decks themselves sync to each other as usual. Enabling Link opens network sockets, which
//! can't happen on the audio thread; `watch` does it on its own thread and keeps the peer count up to date. The audio thread only captures and commits session states, which Link makes real-time safe.

use std::sync::Arc;
use std::time::Duration;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};
use djsugar_link::{HostTimeFilter, LinkSession, SessionState};

pub const GROUP: &str = "[Link]";

/// Beats per bar Link aligns to, as the usual default.
pub const QUANTUM: f64 = 4.0;

/// `[Link],enabled` turns Link on and `[Link],num_peers` counts the other
/// apps in the session; `watch` looks after both. `[Link],master_deck` is
/// the deck the session follows (1 or 2), 0 for none.
pub(crate) struct LinkControls {
    /// The session's tempo.
    pub(crate) bpm: ControlHandle,
    /// Beats in a bar of the session, for phase.
    pub(crate) quantum: ControlHandle,
    /// The deck the session follows, counted from 1; 0 for none.
    pub(crate) master_deck: ControlHandle,
}

impl LinkControls {
    pub(crate) fn create(reg: &ControlRegistry) -> Self {
        reg.toggle(GROUP, "enabled");
        reg.indicator(GROUP, "num_peers", 1000.0);
        Self {
            bpm: reg.indicator(GROUP, "bpm", 999.0),
            quantum: reg.create(GROUP, "quantum", QUANTUM, 1.0, 16.0, Behavior::Free),
            master_deck: reg.indicator(GROUP, "master_deck", 2.0),
        }
    }
}

/// The session as this buffer will be heard.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Moment {
    pub(crate) tempo: f64,
    /// The session's beat when this buffer's first frame is heard.
    pub(crate) beat: f64,
    /// Link was turned on this buffer.
    pub(crate) just_enabled: bool,
}

/// The audio thread's side of Link.
pub(crate) struct Link {
    link: Arc<LinkSession>,
    state: SessionState,
    filter: HostTimeFilter,
    /// Frames rendered so far: the clock the host time filter smooths.
    frames: u64,
    /// Host time this buffer is heard, in microseconds.
    pub(crate) heard_at: i64,
    was_enabled: bool,
    pub(crate) c: LinkControls,
}

impl Link {
    pub(crate) fn new(reg: &ControlRegistry) -> Self {
        Self {
            link: Arc::new(LinkSession::new(120.0)),
            state: SessionState::new(),
            filter: HostTimeFilter::new(),
            frames: 0,
            heard_at: 0,
            was_enabled: false,
            c: LinkControls::create(reg),
        }
    }

    /// The Link instance, for `watch`.
    pub(crate) fn handle(&self) -> Arc<LinkSession> {
        self.link.clone()
    }

    /// Reads the session at the start of a buffer of `frames` frames at
    /// `sample_rate`; None while Link is off.
    pub(crate) fn begin(&mut self, frames: usize, sample_rate: f64) -> Option<Moment> {
        let now = self.link.clock_micros();
        let start = self.filter.sample_time_to_host_time(now, self.frames);
        self.frames += frames as u64;
        // The buffer being rendered plays after the one now playing. Kept
        // with Link off too: MIDI clock out times its pulses from it.
        self.heard_at = start + (frames as f64 / sample_rate * 1e6) as i64;
        let enabled = self.link.is_enabled();
        let just_enabled = enabled && !self.was_enabled;
        self.was_enabled = enabled;
        if !enabled {
            return None;
        }
        self.link.capture_audio_session_state(&mut self.state);
        let tempo = self.state.tempo();
        self.c.bpm.set(tempo);
        Some(Moment { tempo, beat: self.state.beat_at_time(self.heard_at, self.quantum()), just_enabled })
    }

    fn quantum(&self) -> f64 {
        self.c.quantum.get().max(1.0)
    }

    /// Sets the session's tempo to `bpm` as this buffer is heard and, with
    /// `beat`, puts the session on that beat then (the master deck's), so
    /// every app in the session follows.
    pub(crate) fn lead(&mut self, bpm: f64, beat: Option<f64>) {
        if bpm <= 0.0 {
            return;
        }
        self.state.set_tempo(bpm, self.heard_at);
        if let Some(beat) = beat {
            let quantum = self.quantum();
            self.state.force_beat_at_time(beat, self.heard_at, quantum);
        }
        self.link.commit_audio_session_state(&self.state);
        self.c.bpm.set(bpm);
    }
}

/// Turns Link on and off as `[Link],enabled` says and keeps
/// `[Link],num_peers` current. Runs until the registry is dropped.
pub(crate) fn watch(link: Arc<LinkSession>, reg: std::sync::Weak<ControlRegistry>) {
    let spawned = std::thread::Builder::new().name("link".into()).spawn(move || loop {
        let Some(reg) = reg.upgrade() else { return };
        let (Some(enabled), Some(peers)) = (reg.get(GROUP, "enabled"), reg.get(GROUP, "num_peers")) else {
            return;
        };
        drop(reg);
        let want = enabled.get_bool();
        if want != link.is_enabled() {
            link.enable(want);
        }
        peers.set(if want { link.num_peers() as f64 } else { 0.0 });
        std::thread::sleep(Duration::from_millis(100));
    });
    if let Err(e) = spawned {
        eprintln!("could not start Link: {e}");
    }
}

/// How far `deck_beat` is ahead of `link_beat` within a beat: -0.5..0.5.
pub(crate) fn phase_error(deck_beat: f64, link_beat: f64) -> f64 {
    let e = (deck_beat - link_beat).rem_euclid(1.0);
    if e >= 0.5 {
        e - 1.0
    } else {
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_error_wraps_to_the_nearest_beat() {
        assert!((phase_error(10.1, 3.0) - 0.1).abs() < 1e-12);
        assert!((phase_error(10.9, 3.0) + 0.1).abs() < 1e-12);
        assert!((phase_error(2.0, 7.0)).abs() < 1e-12);
    }
}
