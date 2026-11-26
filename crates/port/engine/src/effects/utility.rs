//! The utility effects: a metronome, a loudness contour, balance, two equalizers and an automatic gain.

use std::f32::consts::TAU;

use super::{glide, lock_to_beat, Ctx, MAX_PARAMETERS};
use crate::dsp::{Biquad, Lr4};

type Knobs = [f32; MAX_PARAMETERS];

fn ratio(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// A knob's 0..1 as a value from `lo` to `hi`.
fn lin(v: f32, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * v.clamp(0.0, 1.0)
}

/// The same with even steps in ratio, for times and frequencies.
fn log(v: f32, lo: f32, hi: f32) -> f32 {
    lo * (hi / lo).powf(v.clamp(0.0, 1.0))
}

/// A click's length and pitch: a short sine burst that dies away.
const CLICK_SECONDS: f32 = 0.04;
const CLICK_HZ: f32 = 1600.0;
const CLICK_DECAY: f32 = 120.0;

/// A click on every beat of the track, or at its own BPM (40 to 208) when Sync is off or the track has no beats.
/// Level is -24 to +3 dB.
pub(super) struct Metronome {
    phase: f32,
    step: f32,
    locked: bool,
    level: f32,
    rate: f32,
    /// Frames into the click being played.
    at: f32,
}

impl Default for Metronome {
    fn default() -> Self {
        Self { phase: 0.0, step: 0.0, locked: false, level: 1.0, rate: 48_000.0, at: f32::MAX }
    }
}

impl Metronome {
    pub(super) fn prepare(&mut self, p: &Knobs, ctx: Ctx) {
        self.rate = ctx.sample_rate;
        self.level = 0.5 * ratio(lin(p[2], -24.0, 3.0));
        if p[1] >= 0.5 && ctx.beat.is_some() {
            self.step = 1.0 / (ctx.beat_seconds * ctx.sample_rate);
            let mut fresh = !self.locked;
            lock_to_beat(&mut self.phase, &mut self.step, &mut fresh, ctx.beat, 1.0);
            self.locked = true;
        } else {
            self.step = log(p[0], 40.0, 208.0) / (60.0 * ctx.sample_rate);
            self.locked = false;
        }
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.at = 0.0;
        }
        if self.at >= CLICK_SECONDS * self.rate {
            return x;
        }
        let t = self.at / self.rate;
        self.at += 1.0;
        let click = self.level * (TAU * CLICK_HZ * t).sin() * (-CLICK_DECAY * t).exp();
        [x[0] + click, x[1] + click]
    }
}

/// The most the contour lifts the bass, in dB; the treble gets a third of it.
const CONTOUR_DB: f32 = 30.0;

/// What the ear loses at low volume put back: bass and some treble lifted, and the level brought down by as much, so
/// quiet playback sounds full. Turning Loudness down asks for more of it.
#[derive(Default)]
pub(super) struct Loudness {
    low: Biquad,
    high: Biquad,
    set: Option<(f32, f32)>,
    gain: f32,
    target: f32,
}

impl Loudness {
    pub(super) fn prepare(&mut self, p: &Knobs, ctx: Ctx) {
        let lift = CONTOUR_DB * (1.0 - p[0].clamp(0.0, 1.0));
        if self.set == Some((lift, ctx.sample_rate)) {
            return;
        }
        let rate = f64::from(ctx.sample_rate);
        self.low.set_peaking(rate, 20.0, 0.2, f64::from(lift));
        self.high.set_high_shelf(rate, 3000.0, 0.7, f64::from(lift / 3.0));
        self.target = ratio(-lift);
        if self.set.is_none() {
            self.gain = self.target;
        }
        self.set = Some((lift, ctx.sample_rate));
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        glide(&mut self.gain, self.target);
        let y = self.high.process(self.low.process(x));
        [y[0] * self.gain, y[1] * self.gain]
    }
}

/// The lowest corner of the bass that balance leaves alone: at it, nothing is left alone.
const KEPT_MIN_HZ: f32 = 16.0;
const KEPT_MAX_HZ: f32 = 500.0;

/// Left against right, and middle against sides: Balance turns one side down, Mid/Side the middle (right) or the
/// sides (left). Bass under the Bypass frequency passes untouched, so it stays in the centre.
#[derive(Default)]
pub(super) struct Balance {
    low: Lr4,
    high: Lr4,
    corner: f32,
    rate: f32,
    balance: f32,
    mid_side: f32,
    to: [f32; 2],
}

impl Balance {
    pub(super) fn prepare(&mut self, p: &Knobs, ctx: Ctx) {
        self.to = [p[0].clamp(0.0, 1.0) * 2.0 - 1.0, p[1].clamp(0.0, 1.0) * 2.0 - 1.0];
        let corner = log(p[2], KEPT_MIN_HZ, KEPT_MAX_HZ);
        if corner != self.corner || ctx.sample_rate != self.rate {
            self.corner = corner;
            self.rate = ctx.sample_rate;
            self.low.set_lowpass(f64::from(self.rate), f64::from(corner));
            self.high.set_highpass(f64::from(self.rate), f64::from(corner));
        }
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let (kept, x) =
            if self.corner > KEPT_MIN_HZ { (self.low.process(x), self.high.process(x)) } else { ([0.0; 2], x) };
        glide(&mut self.balance, self.to[0]);
        glide(&mut self.mid_side, self.to[1]);
        let mut mid = (x[0] + x[1]) * 0.5;
        let mut side = (x[1] - x[0]) * 0.5;
        if self.mid_side > 0.0 {
            mid *= 1.0 - self.mid_side;
        } else {
            side *= 1.0 + self.mid_side;
        }
        let (mut left, mut right) = (mid - side, mid + side);
        if self.balance > 0.0 {
            left *= 1.0 - self.balance;
        } else {
            right *= 1.0 + self.balance;
        }
        [kept[0] + left, kept[1] + right]
    }
}

/// The graphic equalizer's bands: a shelf at each end and six bells between.
pub(super) const GRAPHIC_HZ: [f64; 8] = [81.0, 100.0, 222.0, 494.0, 1097.0, 2437.0, 5416.0, 9828.0];
const GRAPHIC_Q: f64 = 1.224_744_9;
const GRAPHIC_DB: f32 = 12.0;

/// Eight fixed bands, each cut or lifted by up to 12 dB. A band left in the middle costs nothing.
#[derive(Default)]
pub(super) struct GraphicEq {
    bands: [Biquad; 8],
    db: [f32; 8],
    rate: f32,
}

impl GraphicEq {
    pub(super) fn prepare(&mut self, p: &Knobs, ctx: Ctx) {
        let fresh = self.rate != ctx.sample_rate;
        self.rate = ctx.sample_rate;
        let rate = f64::from(self.rate);
        for (k, hz) in GRAPHIC_HZ.into_iter().enumerate() {
            let db = (p[k].clamp(0.0, 1.0) * 2.0 - 1.0) * GRAPHIC_DB;
            if !fresh && db == self.db[k] {
                continue;
            }
            self.db[k] = db;
            match k {
                0 => self.bands[k].set_low_shelf(rate, hz, GRAPHIC_Q, f64::from(db)),
                7 => self.bands[k].set_high_shelf(rate, hz, GRAPHIC_Q, f64::from(db)),
                _ => self.bands[k].set_peaking(rate, hz, GRAPHIC_Q, f64::from(db)),
            }
        }
    }

    #[inline]
    pub(super) fn process(&mut self, mut x: [f32; 2]) -> [f32; 2] {
        for (band, db) in self.bands.iter_mut().zip(self.db) {
            if db != 0.0 {
                x = band.process(x);
            }
        }
        x
    }
}

const PARAM_DB: f32 = 18.0;

/// Two bells, each with its own gain (18 dB either way), width (Q 0.5 to 3) and centre (100 Hz to 14 kHz).
#[derive(Default)]
pub(super) struct ParamEq {
    bands: [Biquad; 2],
    set: [[f32; 3]; 2],
    rate: f32,
}

impl ParamEq {
    pub(super) fn prepare(&mut self, p: &Knobs, ctx: Ctx) {
        let fresh = self.rate != ctx.sample_rate;
        self.rate = ctx.sample_rate;
        for k in 0..2 {
            let [gain, q, centre] = [p[3 * k], p[3 * k + 1], p[3 * k + 2]];
            let want = [(gain.clamp(0.0, 1.0) * 2.0 - 1.0) * PARAM_DB, lin(q, 0.5, 3.0), log(centre, 100.0, 14_000.0)];
            if fresh || want != self.set[k] {
                self.set[k] = want;
                let [db, q, hz] = want.map(f64::from);
                self.bands[k].set_peaking(f64::from(self.rate), hz, q, db);
            }
        }
    }

    #[inline]
    pub(super) fn process(&mut self, mut x: [f32; 2]) -> [f32; 2] {
        for (band, set) in self.bands.iter_mut().zip(self.set) {
            if set[0] != 0.0 {
                x = band.process(x);
            }
        }
        x
    }
}

/// Holds the level steady: sound over the Threshold is brought to the Target, by no more than Gain, with a Knee
/// (in dB) easing in around the threshold. Attack and Release are how fast the measured level rises and falls.
#[derive(Default)]
pub(super) struct AutoGain {
    level: f32,
    threshold: f32,
    target: f32,
    most: f32,
    knee: f32,
    attack: f32,
    release: f32,
}

impl AutoGain {
    pub(super) fn prepare(&mut self, p: &Knobs, ctx: Ctx) {
        let smooth = |ms: f32| (-1000.0 / (ms * ctx.sample_rate)).exp();
        self.threshold = lin(p[0], -70.0, 0.0);
        self.target = lin(p[1], -20.0, 10.0);
        self.most = lin(p[2], 1.0, 40.0);
        self.knee = lin(p[3], 0.0, 24.0);
        self.attack = smooth(log(p[4], 0.1, 250.0));
        self.release = smooth(lin(p[5], 1.0, 1500.0));
    }

    #[inline]
    pub(super) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let peak = x[0].abs().max(x[1].abs());
        if peak == 0.0 {
            return x;
        }
        let pull = if peak > self.level { self.attack } else { self.release };
        self.level = pull * self.level + (1.0 - pull) * peak;
        let db = 20.0 * self.level.log10();
        let (lower, upper) = (self.threshold - 0.5 * self.knee, self.threshold + 0.5 * self.knee);
        let wanted = if db > upper {
            self.target - db
        } else if db < lower {
            0.0
        } else {
            (self.target - upper) * (db - lower) / self.knee.max(1e-3)
        };
        let gain = ratio(wanted.min(self.most));
        [x[0] * gain, x[1] * gain]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn ctx(beat: Option<f64>) -> Ctx {
        Ctx { sample_rate: SR, beat_seconds: 0.5, beat }
    }

    fn knobs(p: &[f32]) -> Knobs {
        std::array::from_fn(|k| p.get(k).copied().unwrap_or(0.5))
    }

    /// The level a sine of `hz` comes out at, once settled.
    fn level(hz: f32, mut f: impl FnMut([f32; 2]) -> [f32; 2]) -> f32 {
        let out: Vec<f32> = (0..48_000).map(|i| f([(TAU * hz * i as f32 / SR).sin(); 2])[0]).collect();
        out[24_000..].iter().fold(0.0, |m, v| m.max(v.abs()))
    }

    fn starts(out: &[f32]) -> Vec<usize> {
        let mut at = Vec::new();
        for (i, v) in out.iter().enumerate() {
            if *v != 0.0 && at.last().is_none_or(|last| i - last > 4_000) {
                at.push(i);
            }
        }
        at
    }

    #[test]
    fn metronome_clicks_at_its_own_tempo_without_a_grid() {
        let mut m = Metronome::default();
        // 120 BPM sits at ln(3) / ln(5.2) of the knob.
        m.prepare(&knobs(&[0.666_37, 1.0, 0.889]), ctx(None));
        let out: Vec<f32> = (0..100_000).map(|_| m.process([0.0; 2])[0]).collect();
        let at = starts(&out);
        assert_eq!(at.len(), 4, "{at:?}");
        for pair in at.windows(2) {
            assert!((pair[1] - pair[0]).abs_diff(24_000) < 40, "{at:?}");
        }
        assert!(out.iter().all(|v| v.abs() <= 0.5));
    }

    #[test]
    fn metronome_clicks_on_the_tracks_beats() {
        let mut m = Metronome::default();
        let mut out = Vec::new();
        // The track is 0.25 beats in at the start; a beat is 24 000 frames.
        for block in 0..750 {
            m.prepare(&knobs(&[0.0, 1.0, 0.889]), ctx(Some(0.25 + block as f64 * 64.0 / 24_000.0)));
            out.extend((0..64).map(|_| m.process([0.0; 2])[0]));
        }
        // A click opens on a zero, so it is first heard a frame late.
        assert_eq!(starts(&out), [18_001, 42_001]);
    }

    #[test]
    fn loudness_lifts_the_bass_over_the_middle_and_is_flat_at_the_top() {
        let shaped = |knob: f32, hz: f32| {
            let mut l = Loudness::default();
            l.prepare(&knobs(&[knob]), ctx(None));
            level(hz, |x| l.process(x))
        };
        assert!((shaped(1.0, 60.0) - 1.0).abs() < 1e-3 && (shaped(1.0, 1000.0) - 1.0).abs() < 1e-3);
        let (bass, middle, treble) = (shaped(0.5, 40.0), shaped(0.5, 800.0), shaped(0.5, 10_000.0));
        assert!(bass > 2.0 * middle, "bass {bass}, middle {middle}");
        assert!(treble > 1.3 * middle && treble < bass, "treble {treble}, middle {middle}");
        assert!(bass <= 1.0, "never louder than it came in: {bass}");
    }

    #[test]
    fn balance_turns_a_side_down_and_mid_side_the_middle_or_the_sides() {
        let run = |p: &[f32], x: [f32; 2]| {
            let mut b = Balance::default();
            b.prepare(&knobs(p), ctx(None));
            (0..48_000).map(|_| b.process(x)).last().unwrap()
        };
        let near = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3;
        assert!(near(run(&[0.5, 0.5, 0.0], [0.3, -0.7]), [0.3, -0.7]), "centred changes nothing");
        assert!(near(run(&[1.0, 0.5, 0.0], [0.3, -0.7]), [0.0, -0.7]), "right: no left");
        assert!(near(run(&[0.0, 0.5, 0.0], [0.3, -0.7]), [0.3, 0.0]), "left: no right");
        assert!(near(run(&[0.5, 1.0, 0.0], [0.6, 0.2]), [0.2, -0.2]), "sides only");
        assert!(near(run(&[0.5, 0.0, 0.0], [0.6, 0.2]), [0.4, 0.4]), "middle only");
    }

    #[test]
    fn balance_leaves_the_bass_under_its_corner_alone() {
        let side = |hz: f32| {
            let mut b = Balance::default();
            // Hard right, the corner at 500 Hz: the left channel is what was kept.
            b.prepare(&knobs(&[1.0, 0.5, 1.0]), ctx(None));
            level(hz, |x| b.process(x))
        };
        assert!(side(60.0) > 0.9, "{}", side(60.0));
        assert!(side(5000.0) < 0.01, "{}", side(5000.0));
    }

    #[test]
    fn graphic_bands_move_their_own_frequency() {
        let shaped = |band: usize, knob: f32, hz: f32| {
            let mut p = [0.5; MAX_PARAMETERS];
            p[band] = knob;
            let mut eq = GraphicEq::default();
            eq.prepare(&p, ctx(None));
            level(hz, |x| eq.process(x))
        };
        for (band, hz) in GRAPHIC_HZ.into_iter().enumerate().skip(1).take(6) {
            let (up, down) = (shaped(band, 1.0, hz as f32), shaped(band, 0.0, hz as f32));
            assert!((up - ratio(12.0)).abs() < 0.1, "band {band} up: {up}");
            assert!((down - ratio(-12.0)).abs() < 0.02, "band {band} down: {down}");
        }
        assert!((shaped(3, 1.0, 9000.0) - 1.0).abs() < 0.05, "far from the band");
        assert!(shaped(0, 1.0, 30.0) > 3.5 && shaped(7, 0.0, 18_000.0) < 0.3, "the shelves");
        assert_eq!(shaped(3, 0.5, 494.0), level(494.0, |x| x), "flat is untouched");
    }

    #[test]
    fn param_bands_sit_where_their_centres_are_set() {
        let mut eq = ParamEq::default();
        // +18 dB at 100 Hz and -18 dB at 14 kHz.
        eq.prepare(&knobs(&[1.0, 0.5, 0.0, 0.0, 0.5, 1.0]), ctx(None));
        assert!((level(100.0, |x| eq.process(x)) - ratio(18.0)).abs() < 0.2);
        assert!((level(14_000.0, |x| eq.process(x)) - ratio(-18.0)).abs() < 0.02);
        assert!((level(1200.0, |x| eq.process(x)) - 1.0).abs() < 0.1);
    }

    #[test]
    fn auto_gain_brings_loud_and_quiet_sound_to_the_target() {
        let settled = |amp: f32| {
            let mut a = AutoGain::default();
            // Threshold -40 dB, target -5 dB, up to 20 dB of gain, a 10 dB knee.
            a.prepare(&knobs(&[0.4286, 0.5, 0.487, 0.4167, 0.2943, 0.3329]), ctx(None));
            level(1000.0, |x| a.process([x[0] * amp, x[1] * amp]))
        };
        assert!((settled(1.0) - ratio(-5.0)).abs() < 0.05, "loud comes down: {}", settled(1.0));
        assert!((settled(0.1) - ratio(-5.0)).abs() < 0.05, "quiet comes up: {}", settled(0.1));
        let hushed = settled(0.001);
        assert!((hushed - 0.001).abs() < 1e-4, "under the threshold is left alone: {hushed}");
    }
}
