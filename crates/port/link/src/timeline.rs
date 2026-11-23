//! Beat/time arithmetic on a timeline, and the phase rules that keep every
//! peer's bars lined up: beats are compared modulo the quantum (beats per
//! bar), counted from the session's beat 0.

/// A straight line from time (microseconds) to beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timeline {
    pub bpm: f64,
    /// The beat at `time_origin`.
    pub beat_origin: f64,
    pub time_origin: f64,
}

pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 999.0;

impl Timeline {
    pub fn beats_at(&self, time: f64) -> f64 {
        self.beat_origin + (time - self.time_origin) * self.bpm / 60e6
    }

    pub fn time_of(&self, beat: f64) -> f64 {
        self.time_origin + (beat - self.beat_origin) * 60e6 / self.bpm
    }

    /// The same line with its origin moved to `time`.
    pub fn rebased(&self, time: f64) -> Self {
        Self { bpm: self.bpm, beat_origin: self.beats_at(time), time_origin: time }
    }
}

/// Where `beats` falls within a bar of `quantum` beats: 0..quantum.
pub fn phase(beats: f64, quantum: f64) -> f64 {
    if quantum <= 0.0 {
        0.0
    } else {
        beats.rem_euclid(quantum)
    }
}

/// The first value at or after `x` with the same phase as `target`.
pub fn next_phase_match(x: f64, target: f64, quantum: f64) -> f64 {
    x + (phase(target, quantum) - phase(x, quantum)).rem_euclid(quantum.max(f64::MIN_POSITIVE))
}

/// The value nearest `x` with the same phase as `target`.
pub fn closest_phase_match(x: f64, target: f64, quantum: f64) -> f64 {
    if quantum <= 0.0 {
        return x;
    }
    next_phase_match(x - quantum / 2.0, target, quantum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line() {
        let t = Timeline { bpm: 120.0, beat_origin: 4.0, time_origin: 1e6 };
        assert_eq!(t.beats_at(1.5e6), 5.0);
        assert_eq!(t.time_of(6.0), 2e6);
        assert_eq!(t.rebased(3e6).beat_origin, 8.0);
    }

    #[test]
    fn phase_matching() {
        assert_eq!(phase(-1.0, 4.0), 3.0);
        assert_eq!(next_phase_match(5.5, 1.0, 4.0), 9.0);
        assert_eq!(closest_phase_match(5.5, 1.0, 4.0), 5.0);
        assert_eq!(closest_phase_match(7.5, 1.0, 4.0), 9.0);
        assert_eq!(closest_phase_match(7.5, 1.0, 0.0), 7.5);
    }
}
