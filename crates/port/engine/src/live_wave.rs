//! The sound of decks playing a live source (an input, an instrument or the
//! Groove Box), as waveform slices for the app to draw while it plays.
//!
//! The audio thread splits each such deck's sound into the waveform's bands
//! and sends a slice every millisecond through a lock-free queue; the app
//! keeps the last seconds and draws them up to the play line.

use crate::dsp::Isolator;
use crate::track::WAVEFORM_RATE;

/// Slices queued per deck before the app reads them: about 4 s, far more
/// than a frame's worth.
const QUEUE: usize = 4096;

/// One slice of a live deck's sound: the peak of the low, mid and high bands
/// and of the whole signal, then their RMS in the same order, as linear
/// levels (1.0 is full scale).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiveSlice {
    pub peak: [f32; 4],
    pub rms: [f32; 4],
}

/// The audio thread's end, one per deck.
pub(crate) struct LiveWave {
    bands: Isolator,
    sample_rate: f64,
    /// Frames per slice.
    slice_len: f64,
    /// Frames into the slice being filled, and where it ends.
    filled: f64,
    peak: [f32; 4],
    power: [f32; 4],
    frames: u32,
    /// Whether the deck played live in the last buffer.
    live: bool,
    tx: rtrb::Producer<LiveSlice>,
}

/// A deck's queue of live slices: the audio thread's end and the app's.
pub(crate) fn channel(sample_rate: f64) -> (LiveWave, rtrb::Consumer<LiveSlice>) {
    let (tx, rx) = rtrb::RingBuffer::new(QUEUE);
    (
        LiveWave {
            bands: Isolator::new(sample_rate),
            sample_rate,
            slice_len: sample_rate / WAVEFORM_RATE,
            filled: 0.0,
            peak: [0.0; 4],
            power: [0.0; 4],
            frames: 0,
            live: false,
            tx,
        },
        rx,
    )
}

impl LiveWave {
    /// Takes the next frame of a deck playing live.
    #[inline]
    pub(crate) fn push(&mut self, x: [f32; 2]) {
        self.live = true;
        let [lo, mid, hi] = self.bands.split(x);
        for (k, b) in [lo, mid, hi, x].iter().enumerate() {
            self.peak[k] = self.peak[k].max(b[0].abs().max(b[1].abs()));
            self.power[k] += (b[0] * b[0] + b[1] * b[1]) * 0.5;
        }
        self.frames += 1;
        self.filled += 1.0;
        if self.filled >= self.slice_len {
            self.filled -= self.slice_len;
            let n = self.frames.max(1) as f32;
            let slice = LiveSlice {
                peak: self.peak,
                rms: self.power.map(|p| (p / n).sqrt()),
            };
            // A full queue means the app isn't reading: the slice is dropped.
            let _ = self.tx.push(slice);
            self.peak = [0.0; 4];
            self.power = [0.0; 4];
            self.frames = 0;
        }
    }

    /// Called once per buffer for a deck that doesn't play live: forgets the
    /// partial slice and the filters' state, so a new live part starts clean.
    #[inline]
    pub(crate) fn idle(&mut self) {
        if self.live {
            self.live = false;
            self.bands = Isolator::new(self.sample_rate);
            self.filled = 0.0;
            self.peak = [0.0; 4];
            self.power = [0.0; 4];
            self.frames = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_is_sent_every_millisecond() {
        let (mut wave, mut rx) = channel(48_000.0);
        for i in 0..4800 {
            let x = (i as f32 * 0.05).sin() * 0.5;
            wave.push([x, x]);
        }
        assert_eq!(rx.slots(), 100);
        let s = rx.pop().unwrap();
        assert!(s.peak[3] > 0.4 && s.peak[3] <= 0.5);
        assert!(s.rms[3] > 0.2 && s.rms[3] < s.peak[3]);
    }

    #[test]
    fn silence_sends_flat_slices() {
        let (mut wave, mut rx) = channel(44_100.0);
        for _ in 0..450 {
            wave.push([0.0, 0.0]);
        }
        assert_eq!(rx.slots(), 10);
        assert_eq!(rx.pop().unwrap(), LiveSlice::default());
    }
}
