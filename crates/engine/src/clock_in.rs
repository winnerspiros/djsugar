//! MIDI clock in: a drum machine, groovebox or another app leads the decks.
//!
//! The input picked in Settings › Controllers › MIDI Clock is opened by the
//! clock thread (see [`crate::midi_clock`]); its messages are read on the
//! MIDI input's own thread by a [`ClockReader`], which smooths the tempo
//! with a straight line fitted through the pulses of the last four bars, so
//! a jittery hardware clock still gives a steady tempo; when the last two
//! beats say the tempo changed, the fit starts over from them, so a tempo
//! change is followed within two beats. What it finds goes to
//! the audio thread through a [`ClockIn`] snapshot, with no locks: with
//! `[Master],sync_leader_source` on Clock, decks with SYNC on follow its
//! tempo and, while it runs, its beat, with bar 1 on its Start.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use djsugar_control::ControlHandle;
use djsugar_link::AblLink;

use crate::midi_clock::{BeatClock, Snapshot, PPQN};

/// Pulses the tempo is fitted over at most: four bars.
const FIT: usize = 384;

/// The recent pulses that tell a tempo change: two beats.
const RECENT: usize = 48;

/// The recent pulses' tempo differing from the whole fit's by more than
/// this fraction is a tempo change, well past what jitter moves it.
const CHANGE: f64 = 0.004;

/// Fewer pulses than this give no tempo yet.
const MIN_FIT: usize = 6;

/// A gap this long between pulses (microseconds) means the clock stopped
/// and started again: the fit starts over.
const GAP: f64 = 250_000.0;

/// No pulse for this long (microseconds): the clock is gone.
pub const STALE: f64 = 500_000.0;

/// The incoming clock as the audio thread sees it.
#[derive(Default)]
pub struct ClockIn {
    /// The fitted time of the last pulse, its beat and the tempo.
    clock: BeatClock,
    /// Between Start (or Continue) and Stop.
    running: AtomicBool,
}

/// The incoming clock at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub at: Snapshot,
    /// Started: beats count from its Start (beat 0 is bar 1).
    pub running: bool,
}

impl ClockIn {
    pub(crate) fn publish(&self, r: Reading) {
        self.running.store(r.running, Ordering::Relaxed);
        self.clock
            .publish(r.at.heard_at as i64, r.at.beat, r.at.tempo);
    }

    /// The last reading; None with no clock yet.
    pub fn read(&self) -> Option<Reading> {
        let at = self.clock.read().filter(|s| s.tempo > 0.0)?;
        Some(Reading {
            at,
            running: self.running.load(Ordering::Relaxed),
        })
    }

    /// Clears the reading, when the input closes.
    pub(crate) fn clear(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.clock.publish(0, 0.0, 0.0);
    }
}

/// Reads clock, Start, Continue, Stop and Song Position, and fits the tempo.
#[derive(Debug, Default)]
pub struct ClockReader {
    /// Recent pulses: (count, time in microseconds).
    pulses: VecDeque<(i64, f64)>,
    /// Pulses since the fit started.
    count: i64,
    /// The musical position (pulses from the song's start) the next pulse
    /// is at.
    position: i64,
    /// Where the last pulse was.
    last_position: i64,
    running: bool,
}

impl ClockReader {
    /// Takes one message received at `t` (microseconds); true when the
    /// reading changed.
    pub fn message(&mut self, bytes: &[u8], t: f64) -> bool {
        match bytes.first() {
            Some(0xF8) => {
                if self.pulses.back().is_some_and(|&(_, last)| t - last > GAP) {
                    self.pulses.clear();
                }
                self.count += 1;
                self.pulses.push_back((self.count, t));
                while self.pulses.len() > FIT {
                    self.pulses.pop_front();
                }
                // A new tempo: keep only the pulses at it.
                if self.pulses.len() > RECENT {
                    let all = per_pulse(self.pulses.iter());
                    let recent = per_pulse(self.pulses.iter().skip(self.pulses.len() - RECENT));
                    if let (Some(all), Some(recent)) = (all, recent) {
                        if ((recent - all) / all).abs() > CHANGE {
                            let drop = self.pulses.len() - RECENT;
                            self.pulses.drain(..drop);
                        }
                    }
                }
                self.last_position = self.position;
                self.position += 1;
                true
            }
            Some(0xFA) => {
                self.position = 0;
                self.running = true;
                true
            }
            Some(0xFB) => {
                self.running = true;
                true
            }
            Some(0xFC) => {
                self.running = false;
                true
            }
            // Song Position Pointer, in sixteenths: six pulses each.
            Some(0xF2) if bytes.len() >= 3 => {
                let sixteenths = (bytes[1] as i64 & 0x7F) | ((bytes[2] as i64 & 0x7F) << 7);
                self.position = sixteenths * 6;
                true
            }
            _ => false,
        }
    }

    /// The tempo and the last pulse's fitted time and beat; None until a
    /// few pulses have come.
    pub fn reading(&self) -> Option<Reading> {
        if self.pulses.len() < MIN_FIT {
            return None;
        }
        let (mean_i, mean_t, per_pulse) = fit(self.pulses.iter())?;
        let last = self.pulses.back()?.0 as f64;
        Some(Reading {
            at: Snapshot {
                heard_at: mean_t + (last - mean_i) * per_pulse,
                beat: self.last_position as f64 / PPQN,
                tempo: 60e6 / (PPQN * per_pulse),
            },
            running: self.running,
        })
    }
}

/// Least squares fit of time against pulse count, centred for precision:
/// the mean count, the mean time and the microseconds per pulse.
fn fit<'a>(pulses: impl Iterator<Item = &'a (i64, f64)> + Clone) -> Option<(f64, f64, f64)> {
    let n = pulses.clone().count() as f64;
    if n < 2.0 {
        return None;
    }
    let (mean_i, mean_t) = pulses
        .clone()
        .fold((0.0, 0.0), |(a, b), &(i, t)| (a + i as f64 / n, b + t / n));
    let (mut cov, mut var) = (0.0, 0.0);
    for &(i, t) in pulses {
        let di = i as f64 - mean_i;
        cov += di * (t - mean_t);
        var += di * di;
    }
    let per_pulse = cov / var;
    (per_pulse > 0.0).then_some((mean_i, mean_t, per_pulse))
}

fn per_pulse<'a>(pulses: impl Iterator<Item = &'a (i64, f64)> + Clone) -> Option<f64> {
    fit(pulses).map(|f| f.2)
}

/// Opens `port` for clock in; the connection reads until it is dropped.
pub(crate) fn open(
    port: &str,
    feed: Arc<ClockIn>,
    link: Arc<AblLink>,
    bpm: ControlHandle,
    running: ControlHandle,
) -> Option<midir::MidiInputConnection<()>> {
    let mut input = midir::MidiInput::new("DJ Sugar clock in").ok()?;
    // Clock and the other real-time messages are what this is for.
    input.ignore(midir::Ignore::SysexAndActiveSense);
    let p = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).as_deref() == Ok(port))?;
    let mut reader = ClockReader::default();
    let connected = input.connect(
        &p,
        "DJ Sugar clock in",
        move |_, bytes, _| {
            // Link's clock, which the audio thread times buffers by.
            let now = link.clock_micros() as f64;
            if !reader.message(bytes, now) {
                return;
            }
            if let Some(r) = reader.reading() {
                feed.publish(r);
                bpm.set(r.at.tempo);
            }
            running.set_bool(reader.running);
        },
        (),
    );
    match connected {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("{port}: cannot open MIDI input for clock: {e}");
            None
        }
    }
}

/// The MIDI inputs there are now, for clock in.
pub fn input_ports() -> Vec<String> {
    let Ok(input) = midir::MidiInput::new("DJ Sugar clock ports") else {
        return Vec::new();
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .filter(|n| !n.contains("DJ Sugar"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `pulses` at `bpm` starting at `t0`, each early or late by
    /// `jitter(i)` microseconds.
    fn feed(r: &mut ClockReader, t0: f64, bpm: f64, pulses: usize, jitter: impl Fn(usize) -> f64) {
        let gap = 60e6 / (PPQN * bpm);
        for i in 0..pulses {
            r.message(&[0xF8], t0 + i as f64 * gap + jitter(i));
        }
    }

    #[test]
    fn a_jittery_clock_gives_a_steady_tempo() {
        let mut r = ClockReader::default();
        // Up to 2 ms early or late, in no pattern.
        let jitter = |i: usize| ((i * 7919) % 41) as f64 / 40.0 * 4000.0 - 2000.0;
        feed(&mut r, 1e6, 120.0, 400, jitter);
        let tempo = r.reading().unwrap().at.tempo;
        assert!((tempo - 120.0).abs() < 0.05, "{tempo}");
        assert_eq!(format!("{tempo:.2}"), "120.00");
    }

    #[test]
    fn a_tempo_change_is_followed_within_two_beats() {
        let mut r = ClockReader::default();
        let jitter = |i: usize| ((i * 7919) % 41) as f64 / 40.0 * 2000.0 - 1000.0;
        feed(&mut r, 0.0, 120.0, 400, jitter);
        let t0 = 400.0 * 60e6 / (PPQN * 120.0);
        feed(&mut r, t0, 126.0, 60, jitter);
        let tempo = r.reading().unwrap().at.tempo;
        assert!((tempo - 126.0).abs() < 0.3, "{tempo}");
    }

    #[test]
    fn start_counts_beats_from_bar_one() {
        let mut r = ClockReader::default();
        feed(&mut r, 0.0, 132.0, 30, |_| 0.0);
        assert!(!r.reading().unwrap().running);
        r.message(&[0xFA], 0.6e6);
        // Two beats after Start: the 49th pulse is beat 2.
        feed(&mut r, 30.0 * 60e6 / (PPQN * 132.0), 132.0, 49, |_| 0.0);
        let reading = r.reading().unwrap();
        assert!(reading.running);
        assert_eq!(reading.at.beat, 2.0);
        assert!((reading.at.tempo - 132.0).abs() < 1e-6);
        r.message(&[0xFC], 2e6);
        assert!(!r.reading().unwrap().running);
    }

    #[test]
    fn song_position_and_continue_pick_up_mid_song() {
        let mut r = ClockReader::default();
        // Bar 3 (sixteenth 32), then Continue.
        r.message(&[0xF2, 32, 0], 0.0);
        r.message(&[0xFB], 0.0);
        feed(&mut r, 0.0, 120.0, 10, |_| 0.0);
        // The tenth pulse is 9 pulses after beat 8.
        let reading = r.reading().unwrap();
        assert!(reading.running);
        assert!((reading.at.beat - (8.0 + 9.0 / PPQN)).abs() < 1e-9);
    }

    #[test]
    fn a_new_tempo_after_a_pause_starts_a_new_fit() {
        let mut r = ClockReader::default();
        feed(&mut r, 0.0, 100.0, 60, |_| 0.0);
        feed(&mut r, 10e6, 140.0, 10, |_| 0.0);
        let tempo = r.reading().unwrap().at.tempo;
        assert!((tempo - 140.0).abs() < 1e-6, "{tempo}");
    }

    #[test]
    fn the_snapshot_reads_back() {
        let feed = ClockIn::default();
        assert_eq!(feed.read(), None);
        let r = Reading {
            at: Snapshot {
                heard_at: 5e6,
                beat: 3.0,
                tempo: 128.0,
            },
            running: true,
        };
        feed.publish(r);
        assert_eq!(feed.read(), Some(r));
        feed.clear();
        assert_eq!(feed.read(), None);
    }
}
