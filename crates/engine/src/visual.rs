//! Play positions for drawing, published by the audio thread.
//!
//! `playposition` changes once per audio buffer, so a waveform drawn from it
//! moves in uneven steps. Like `VisualPlayPosition`, the engine also
//! publishes each deck's position together with its speed and the time it
//! was written; the UI extrapolates from those to the moment it draws, which
//! scrolls smoothly at any frame rate.

use std::sync::atomic::{fence, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use crate::controls::NUM_DECKS;

/// The longest the UI extrapolates past the last update, in case the audio
/// thread stalls (a device change, a suspended laptop).
const MAX_EXTRAPOLATION: f64 = 0.1;

#[derive(Default)]
struct Slot {
    // Seqlock: odd while the audio thread is writing.
    seq: AtomicU32,
    seconds: AtomicU64,
    speed: AtomicU64,
    at_nanos: AtomicU64,
}

pub struct VisualPositions {
    base: Instant,
    decks: [Slot; NUM_DECKS],
}

impl Default for VisualPositions {
    fn default() -> Self {
        Self {
            base: Instant::now(),
            decks: Default::default(),
        }
    }
}

impl VisualPositions {
    /// Called by the audio thread at the end of each buffer. Never blocks.
    pub(crate) fn publish(&self, deck: usize, seconds: f64, speed: f64, now: Instant) {
        let slot = &self.decks[deck];
        let s = slot.seq.load(Ordering::Relaxed);
        slot.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        slot.seconds.store(seconds.to_bits(), Ordering::Relaxed);
        slot.speed.store(speed.to_bits(), Ordering::Relaxed);
        let nanos = now.saturating_duration_since(self.base).as_nanos() as u64;
        slot.at_nanos.store(nanos, Ordering::Relaxed);
        slot.seq.store(s.wrapping_add(2), Ordering::Release);
    }

    /// Where `deck` is at `now`, in track seconds.
    pub fn seconds(&self, deck: usize, now: Instant) -> f64 {
        self.position(deck, now).0
    }

    /// Where `deck` is at `now`, in track seconds, and its speed in track
    /// seconds per second.
    pub fn position(&self, deck: usize, now: Instant) -> (f64, f64) {
        let slot = &self.decks[deck];
        loop {
            let s1 = slot.seq.load(Ordering::Acquire);
            if s1 % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let seconds = f64::from_bits(slot.seconds.load(Ordering::Relaxed));
            let speed = f64::from_bits(slot.speed.load(Ordering::Relaxed));
            let at = slot.at_nanos.load(Ordering::Relaxed);
            fence(Ordering::Acquire);
            if slot.seq.load(Ordering::Relaxed) != s1 {
                continue;
            }
            let at = self.base + std::time::Duration::from_nanos(at);
            let dt = now.saturating_duration_since(at).as_secs_f64();
            return (seconds + speed * dt.min(MAX_EXTRAPOLATION), speed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn extrapolates_from_the_last_update() {
        let v = VisualPositions::default();
        let t = Instant::now();
        v.publish(0, 10.0, 1.0, t);
        assert_eq!(v.seconds(0, t), 10.0);
        let later = v.seconds(0, t + Duration::from_millis(20));
        assert!((later - 10.02).abs() < 1e-6, "{later}");
        // Stops extrapolating if updates stop.
        let stalled = v.seconds(0, t + Duration::from_secs(5));
        assert!((stalled - 10.1).abs() < 1e-6, "{stalled}");
        // A stopped deck stays put.
        v.publish(1, 3.0, 0.0, t);
        assert_eq!(v.seconds(1, t + Duration::from_millis(50)), 3.0);
    }
}
