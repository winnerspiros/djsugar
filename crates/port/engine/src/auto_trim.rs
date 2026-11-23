//! Auto trim: the first seconds an input or instrument plays through a deck
//! set its TRIM (`[ChannelN],live_trim`), so quiet gear comes up and hot
//! gear comes down to the level tracks play at. It listens only while the
//! trim is untouched (0 dB) when the input goes on the deck, counts only
//! the time it hears something, and stops for good once the DJ turns TRIM.

/// How long it listens to the input playing before setting the trim.
pub(crate) const LISTEN_SECONDS: f64 = 3.0;
/// The peak the trim brings the input to, under the safety limiter's
/// ceiling with room to spare.
pub(crate) const TARGET_PEAK_DB: f64 = -6.0;
/// Quieter than this (-50 dBFS) is silence: not counted as heard.
const QUIET: f32 = 0.003;
/// TRIM's range, as `[ChannelN],live_trim` has it.
const MIN_DB: f64 = -12.0;
const MAX_DB: f64 = 24.0;

#[derive(Default)]
pub(crate) struct AutoTrim {
    armed: bool,
    peak: f32,
    /// Frames heard above silence.
    heard: u32,
}

impl AutoTrim {
    /// Starts listening afresh.
    pub(crate) fn arm(&mut self) {
        *self = Self {
            armed: true,
            ..Self::default()
        };
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }

    pub(crate) fn armed(&self) -> bool {
        self.armed
    }

    /// One frame of the input, before the trim.
    #[inline]
    pub(crate) fn listen(&mut self, x: [f32; 2]) {
        if self.armed {
            let a = x[0].abs().max(x[1].abs());
            if a > QUIET {
                self.peak = self.peak.max(a);
                self.heard += 1;
            }
        }
    }

    /// The trim in dB, to half a dB, once it has heard enough; then it
    /// stops listening.
    pub(crate) fn finish(&mut self, sample_rate: f64) -> Option<f64> {
        if !self.armed || f64::from(self.heard) < LISTEN_SECONDS * sample_rate {
            return None;
        }
        self.armed = false;
        let peak_db = 20.0 * f64::from(self.peak).log10();
        Some((((TARGET_PEAK_DB - peak_db) * 2.0).round() / 2.0).clamp(MIN_DB, MAX_DB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(t: &mut AutoTrim, amp: f32, frames: usize) {
        for _ in 0..frames {
            t.listen([amp, -amp]);
        }
    }

    #[test]
    fn it_sets_the_trim_after_hearing_enough() {
        let mut t = AutoTrim::default();
        play(&mut t, 0.1, 48_000 * 4);
        assert_eq!(t.finish(48_000.0), None, "not armed");
        t.arm();
        play(&mut t, 0.1, 48_000);
        // Silence doesn't count.
        play(&mut t, 0.0, 48_000 * 4);
        assert_eq!(t.finish(48_000.0), None);
        play(&mut t, 0.1, 48_000 * 2);
        // -20 dBFS peaks come up 14 dB to -6.
        assert_eq!(t.finish(48_000.0), Some(14.0));
        assert!(!t.armed());
        assert_eq!(t.finish(48_000.0), None);
    }

    #[test]
    fn it_stays_in_trims_range() {
        let mut t = AutoTrim::default();
        t.arm();
        play(&mut t, 0.004, 48_000 * 3);
        assert_eq!(t.finish(48_000.0), Some(MAX_DB));
        t.arm();
        play(&mut t, 1.0, 48_000 * 3);
        assert_eq!(t.finish(48_000.0), Some(-6.0));
    }
}
