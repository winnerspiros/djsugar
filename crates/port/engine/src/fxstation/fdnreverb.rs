//! Stereo feedback delay network reverb. Structure (after Geraint Luff, "Let's write a reverb", ADC 2021, and Jot's FDN work): a 4-step Hadamard diffuser
//! builds echo density fast, then an 8-line feedback network with a Householder matrix sustains the tail. Every line has a decay gain derived from RT60, a
//! one-pole high damping and slow, decorrelated delay modulation that removes the metallic ringing of static FDNs. Freeze holds the tank at unity gain.

use std::f32::consts::TAU;

use crate::fxstation::dsp::{DelayLine, Smoother};

const TWO_PI: f32 = TAU;

/// A one-pole filter for damping and low cut.
#[derive(Clone, Copy, Debug, Default)]
struct OnePole {
    state: f32,
    a1: f32,
    b0: f32,
}

impl OnePole {
    fn reset(&mut self) {
        self.state = 0.0;
    }

    fn set_cutoff(&mut self, hz: f32, sample_rate: f32) {
        let g = (TWO_PI * hz / sample_rate).tan();
        self.b0 = g / (1.0 + g);
        self.a1 = 1.0 / (1.0 + g);
    }

    fn lowpass(&mut self, x: f32) -> f32 {
        self.state = self.b0 * x + self.a1 * self.state;
        self.state
    }

    fn highpass(&mut self, x: f32) -> f32 {
        let y = x - self.state;
        self.state = self.b0 * x + self.a1 * self.state;
        y
    }
}

fn clampf(x: f32, min: f32, max: f32) -> f32 {
    x.clamp(min, max)
}

fn sanitize(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

fn flush(x: f32) -> f32 {
    if x.is_normal() {
        x
    } else {
        0.0
    }
}

/// Stereo feedback delay network reverb.
#[derive(Clone, Debug, Default)]
pub struct FdnReverb {
    sample_rate: f32,
    freeze: bool,
    diffuse: [[DelayLine; 8]; 4],
    diffuse_len: [[usize; 8]; 4],
    flip: [[bool; 8]; 4],
    lines: [DelayLine; 8],
    line_damp: [OnePole; 8],
    line_low_cut: [OnePole; 8],
    length: [Smoother; 8],
    gain: [f32; 8],
    mod_phase: [f32; 8],
    mod_rate: [f32; 8],
    pre_delay: [DelayLine; 2],
    input_low_cut: [OnePole; 2],
    pre_delay_samples: f32,
    mod_depth_samples: f32,
    rt60: f32,
    damp_hz: f32,
    energy: f32,
}

impl FdnReverb {
    const LINES: usize = 8;
    const DIFFUSION_STEPS: usize = 4;

    pub fn setup(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let ms_to_samples = sample_rate * 0.001;
        let mut rnd = fastrand::Rng::with_seed(0x5EED1234);

        // Diffuser step k spans about 10, 20, 40, 80 ms: early echoes
        // arrive densely without a flam.
        for step in 0..Self::DIFFUSION_STEPS {
            let range_ms = 10.0 * (1 << step) as f32;
            for c in 0..Self::LINES {
                let lo = range_ms * c as f32 / Self::LINES as f32;
                let hi = range_ms * (c + 1) as f32 / Self::LINES as f32;
                let ms = lo + (hi - lo) * rnd.f32();
                let len = (ms * ms_to_samples).max(1.0) as usize;
                self.diffuse_len[step][c] = len;
                self.diffuse[step][c].allocate(len + 1);
                self.flip[step][c] = rnd.bool();
            }
        }

        for c in 0..Self::LINES {
            // Up to 300 ms base * 1.6 size scale, plus modulation headroom.
            self.lines[c].allocate((sample_rate * 0.55) as usize);
            self.line_damp[c].reset();
            self.line_low_cut[c].reset();
            self.mod_phase[c] = c as f32 / Self::LINES as f32;
            self.length[c].reset(0.0);
        }

        self.pre_delay[0].allocate((sample_rate * 0.25) as usize);
        self.pre_delay[1].allocate((sample_rate * 0.25) as usize);
        self.input_low_cut[0].reset();
        self.input_low_cut[1].reset();

        self.set_size(0.5, true);
        self.set_decay(2.0);
        self.set_damping(8000.0);
        self.set_low_cut(80.0);
        self.mod_depth_samples = 0.0;
    }

    pub fn clear(&mut self) {
        for step in 0..Self::DIFFUSION_STEPS {
            for c in 0..Self::LINES {
                self.diffuse[step][c].clear();
            }
        }
        for c in 0..Self::LINES {
            self.lines[c].clear();
            self.line_damp[c].reset();
            self.line_low_cut[c].reset();
        }
        self.pre_delay[0].clear();
        self.pre_delay[1].clear();
        self.input_low_cut[0].reset();
        self.input_low_cut[1].reset();
        self.energy = 0.0;
    }

    // size in [0, 1]: delay lengths from small room to large hall.
    pub fn set_size(&mut self, size: f32, immediate: bool) {
        let scale = 0.35 + 1.25 * size.clamp(0.0, 1.0);
        // Base lengths between 100 and 200 ms, exponentially spread so no
        // two lines share a common factor.
        for c in 0..Self::LINES {
            let r = c as f32 / (Self::LINES - 1) as f32;
            let ms = 100.0 * 2.0_f32.powf(r) * scale;
            let len = ms * 0.001 * self.sample_rate;
            if immediate {
                self.length[c].reset(len);
            } else {
                self.length[c].step(len, 1.0); // Will be smoothed per buffer
            }
        }
        self.update_gains();
    }

    // RT60 in seconds. A value <= 0 or freeze means infinite.
    pub fn set_decay(&mut self, rt60_seconds: f32) {
        self.rt60 = rt60_seconds.max(0.05);
        self.update_gains();
    }

    pub fn set_damping(&mut self, hz: f32) {
        self.damp_hz = hz;
        for f in &mut self.line_damp {
            f.set_cutoff(hz, self.sample_rate);
        }
    }

    pub fn set_low_cut(&mut self, hz: f32) {
        for f in &mut self.line_low_cut {
            f.set_cutoff(hz, self.sample_rate);
        }
        self.input_low_cut[0].set_cutoff(hz, self.sample_rate);
        self.input_low_cut[1].set_cutoff(hz, self.sample_rate);
    }

    pub fn set_pre_delay(&mut self, seconds: f32) {
        self.pre_delay_samples = clampf(seconds * self.sample_rate, 1.0, (self.pre_delay[0].max_frames() - 2) as f32);
    }

    // Depth in milliseconds of the slow chorus on the feedback lines.
    pub fn set_modulation(&mut self, ms: f32) {
        self.mod_depth_samples = ms * 0.001 * self.sample_rate;
    }

    pub fn set_freeze(&mut self, freeze: bool) {
        self.freeze = freeze;
        self.update_gains();
    }

    // Approximate energy of the tank, used to tell when a tail has died.
    pub fn energy(&self) -> f32 {
        self.energy
    }

    // Processes one stereo frame. in_l/in_r feed the tank; outputs are wet.
    pub fn process(&mut self, in_l: f32, in_r: f32, out_l: &mut f32, out_r: &mut f32) {
        let (in_l, in_r) = if self.freeze { (0.0, 0.0) } else { (in_l, in_r) };

        self.pre_delay[0].write_mono(0, self.input_low_cut[0].highpass(in_l));
        self.pre_delay[1].write_mono(1, self.input_low_cut[1].highpass(in_r));
        let l = self.pre_delay[0].read(0, self.pre_delay_samples);
        let r = self.pre_delay[1].read(1, self.pre_delay_samples);

        let mut v = [0.0; Self::LINES];
        for c in 0..Self::LINES {
            v[c] = if (c & 1) != 0 { r } else { l };
        }

        for step in 0..Self::DIFFUSION_STEPS {
            for c in 0..Self::LINES {
                self.diffuse[step][c].write_mono(0, v[c]);
                let d = self.diffuse[step][c].at(0, self.diffuse_len[step][c] as f64);
                v[c] = if self.flip[step][c] { -d } else { d };
            }
            Self::hadamard(&mut v);
        }

        // Feedback network.
        let mut fb = [0.0; Self::LINES];
        let mut sum = 0.0;
        for c in 0..Self::LINES {
            self.mod_phase[c] += self.mod_rate[c] / self.sample_rate;
            if self.mod_phase[c] >= 1.0 {
                self.mod_phase[c] -= 1.0;
            }
            let mod_depth = self.mod_depth_samples * (1.0 + (TWO_PI * self.mod_phase[c]).sin());
            let len = self.length[c].value + mod_depth;
            let mut y = self.lines[c].read(c % 2, len);
            if !self.freeze {
                y = self.line_damp[c].lowpass(y);
                y = self.line_low_cut[c].highpass(y);
            }
            fb[c] = y * self.gain[c];
            sum += fb[c];
        }

        // Householder reflection: energy preserving, every line feeds every
        // other one.
        let h = sum * (2.0 / Self::LINES as f32);
        let mut energy = 0.0;
        for c in 0..Self::LINES {
            let x = sanitize(fb[c] - h + v[c]);
            // Write to both channels (stereo)
            self.lines[c].write(flush(x), flush(x));
            energy += x * x;
        }
        self.energy = energy;

        // Decorrelated stereo outputs from the delay outputs.
        *out_l = 0.5 * (fb[0] - fb[2] + fb[4] - fb[6] + fb[1] + fb[5]) * 0.6;
        *out_r = 0.5 * (fb[1] - fb[3] + fb[5] - fb[7] + fb[2] + fb[6]) * 0.6;
    }

    fn hadamard(v: &mut [f32; Self::LINES]) {
        let mut h = 1;
        while h < Self::LINES {
            for i in (0..Self::LINES).step_by(h << 1) {
                for j in i..i + h {
                    let a = v[j];
                    let b = v[j + h];
                    v[j] = a + b;
                    v[j + h] = a - b;
                }
            }
            h <<= 1;
        }
        const SCALE: f32 = 0.35355339; // 1 / sqrt(8)
        for x in v.iter_mut() {
            *x *= SCALE;
        }
    }

    fn update_gains(&mut self) {
        for c in 0..Self::LINES {
            if self.freeze {
                self.gain[c] = 1.0;
                continue;
            }
            let len_seconds = self.length[c].value / self.sample_rate;
            self.gain[c] = 10.0_f32.powf(-3.0 * len_seconds / self.rt60);
        }
    }
}
