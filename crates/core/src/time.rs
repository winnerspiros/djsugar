#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Bpm(pub f64);

impl Bpm {
    pub fn beat_seconds(self) -> f64 {
        60.0 / self.0
    }

    pub fn ratio_to(self, target: Bpm) -> f64 {
        let mut r = target.0 / self.0;
        while r > 1.5 {
            r /= 2.0;
        }
        while r < 0.75 {
            r *= 2.0;
        }
        r
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Beat(pub f64);

impl Beat {
    pub fn bar(self, beats_per_bar: u32) -> i64 {
        (self.0 / f64::from(beats_per_bar)).floor() as i64
    }

    pub fn phase(self) -> f64 {
        self.0.rem_euclid(1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub first_beat_seconds: f64,
    pub bpm: Bpm,
}

impl Grid {
    pub fn beat_at(&self, seconds: f64) -> Beat {
        Beat((seconds - self.first_beat_seconds) / self.bpm.beat_seconds())
    }

    pub fn seconds_at(&self, beat: Beat) -> f64 {
        self.first_beat_seconds + beat.0 * self.bpm.beat_seconds()
    }

    pub fn phase_offset(&self, at: f64, other: &Grid, other_at: f64) -> f64 {
        let d = self.beat_at(at).phase() - other.beat_at(other_at).phase();
        d - d.round()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_folds_double_and_half_time() {
        assert!((Bpm(70.0).ratio_to(Bpm(140.0)) - 1.0).abs() < 1e-9);
        assert!((Bpm(128.0).ratio_to(Bpm(124.0)) - 124.0 / 128.0).abs() < 1e-9);
    }

    #[test]
    fn grid_round_trip_and_phase() {
        let g = Grid { first_beat_seconds: 0.25, bpm: Bpm(120.0) };
        assert!((g.seconds_at(g.beat_at(10.0)) - 10.0).abs() < 1e-9);
        let h = Grid { first_beat_seconds: 0.5, bpm: Bpm(120.0) };
        assert!(
            (g.phase_offset(10.0, &h, 10.0) - 0.5).abs() < 1e-9 || (g.phase_offset(10.0, &h, 10.0) + 0.5).abs() < 1e-9
        );
        assert!(g.phase_offset(10.0, &h, 10.25).abs() < 1e-9);
    }
}
