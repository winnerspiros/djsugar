//! Vinyl motion and needle shaper for the Release effects.

use std::f64::consts::PI;

/// The motion of a back spin, in times the normal speed, at t = 0..1 of its length. The
/// hand catches the record and throws it back (the speed glides through zero, the "zip"
/// of a real spin), then the platter coasts to a stop against the friction of the mat.
#[derive(Clone, Copy, Debug, Default)]
pub struct BackSpinMotion;

impl BackSpinMotion {
    /// Part of the spin the hand takes to throw the record back.
    pub const K_CATCH: f64 = 0.08;

    /// peak: top backward speed.
    pub fn velocity(t: f64, peak: f64) -> f64 {
        if t >= 1.0 {
            return 0.0;
        }
        if t < Self::K_CATCH {
            let u = t / Self::K_CATCH;
            let smooth = u * u * (3.0 - 2.0 * u);
            return 1.0 - (1.0 + peak) * smooth;
        }
        let u = (t - Self::K_CATCH) / (1.0 - Self::K_CATCH);
        -peak * (1.0 - u) * (1.0 - u)
    }

    /// How far back the spin travels, in frames, for a length in frames
    /// and a peak speed: the throw (the speed eases from 1 to -peak) and
    /// the coast (a third of peak x its part).
    pub fn distance(frames: f64, peak: f64) -> f64 {
        frames * (peak * Self::K_BACK_PER_PEAK - Self::K_CATCH / 2.0)
    }

    /// The highest peak speed that stays within the audio there is.
    pub fn peak_for(available_frames: f64, frames: f64, max_peak: f64) -> f64 {
        ((available_frames / frames + Self::K_CATCH / 2.0) / Self::K_BACK_PER_PEAK).clamp(0.0, max_peak)
    }

    const K_BACK_PER_PEAK: f64 = (1.0 - Self::K_CATCH) / 3.0 + Self::K_CATCH / 2.0;
}

/// How a record played at any speed sounds through a cartridge, as in the scratch engine: the level
/// below 1x it removes the images a slowed record does not have, above 1x the aliasing of reading faster.
/// below 1x it removes the images a slowed record does not have, above 1x the aliasing of reading
#[derive(Clone, Debug, Default)]
pub struct NeedleShaper {
    state: [[f32; 4]; 2],
}

impl NeedleShaper {
    pub fn reset(&mut self) {
        self.state = [[0.0; 4]; 2];
    }

    /// Shapes a stereo frame read at velocity (times the normal speed).
    pub fn process(&mut self, left: &mut f32, right: &mut f32, velocity: f64) {
        let speed = velocity.abs();
        let level = speed.min(1.0) as f32;
        let g = (PI * (0.45 * speed.min(1.0 / speed.max(1e-3))).clamp(0.002, 0.45) as f64).tan() as f32;
        // Two 2-pole state variable stages (24 dB/oct, Butterworth).
        const K_DAMPING: f32 = 1.41421356;
        let a1 = 1.0 / (1.0 + g * (g + K_DAMPING));
        let mut channels = [*left, *right];
        for c in 0..2 {
            let mut x = channels[c];
            for stage in 0..2 {
                let v1 = a1 * (self.state[c][2 * stage] + g * (x - self.state[c][2 * stage + 1]));
                let v2 = self.state[c][2 * stage + 1] + g * v1;
                self.state[c][2 * stage] = 2.0 * v1 - self.state[c][2 * stage];
                self.state[c][2 * stage + 1] = 2.0 * v2 - self.state[c][2 * stage + 1];
                x = v2;
            }
            channels[c] = level * x;
        }
        *left = channels[0];
        *right = channels[1];
    }
}
