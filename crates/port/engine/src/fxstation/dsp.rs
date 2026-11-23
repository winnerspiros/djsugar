//! DSP building blocks for the FX Station: small, allocation-free once prepared.
//! Stereo signals are interleaved.

use std::f32::consts::PI;

/// A fast tanh for saturation, accurate enough for audio up to |x| < 3.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// A one-pole smoother, e.g. for parameters (per sample or per buffer).
#[derive(Clone, Copy, Debug, Default)]
pub struct Smoother {
    pub value: f32,
}

impl Smoother {
    pub fn reset(&mut self, v: f32) {
        self.value = v;
    }

    /// Step towards target by coefficient (0..1). Returns the new value.
    pub fn step(&mut self, target: f32, coefficient: f32) -> f32 {
        self.value += (target - self.value) * coefficient;
        self.value
    }
}

/// A state variable filter (topology preserving transform, Zavalishin):
/// stable when its frequency moves on every buffer. Low, band and high
/// pass outputs at once.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1: [f32; 2],
    ic2: [f32; 2],
}

impl Svf {
    pub fn set(&mut self, frequency: f32, q: f32, sample_rate: f32) {
        let frequency = frequency.clamp(10.0, 0.45 * sample_rate);
        self.g = (PI * frequency / sample_rate).tan();
        self.k = 1.0 / q.max(0.1);
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    pub fn reset(&mut self) {
        self.ic1 = [0.0, 0.0];
        self.ic2 = [0.0, 0.0];
    }

    /// Processes a sample of a channel; returns low, band and high pass.
    pub fn process(&mut self, channel: usize, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2[channel];
        let v1 = self.a1 * self.ic1[channel] + self.a2 * v3;
        let v2 = self.ic2[channel] + self.a2 * self.ic1[channel] + self.a3 * v3;
        self.ic1[channel] = 2.0 * v1 - self.ic1[channel];
        self.ic2[channel] = 2.0 * v2 - self.ic2[channel];
        let low = v2;
        let band = v1;
        let high = x - self.k * v1 - v2;
        (low, band, high)
    }
}

/// A biquad (transposed direct form II) with Butterworth low and high
/// pass, for the fixed crossovers of Isolate.
#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl Biquad {
    pub fn set_low_pass(&mut self, frequency: f32, sample_rate: f32) {
        let w = 2.0 * PI * frequency / sample_rate;
        let alpha = (w.sin()) / (2.0 * 0.70710678);
        let c = w.cos();
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 - c) / 2.0 / a0;
        self.b1 = (1.0 - c) / a0;
        self.b2 = self.b0;
        self.a1 = -2.0 * c / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    pub fn set_high_pass(&mut self, frequency: f32, sample_rate: f32) {
        let w = 2.0 * PI * frequency / sample_rate;
        let alpha = (w.sin()) / (2.0 * 0.70710678);
        let c = w.cos();
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 + c) / 2.0 / a0;
        self.b1 = -(1.0 + c) / a0;
        self.b2 = self.b0;
        self.a1 = -2.0 * c / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    pub fn reset(&mut self) {
        self.z1 = [0.0, 0.0];
        self.z2 = [0.0, 0.0];
    }

    pub fn process(&mut self, channel: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.z1[channel];
        self.z1[channel] = self.b1 * x - self.a1 * y + self.z2[channel];
        self.z2[channel] = self.b2 * x - self.a2 * y;
        y
    }
}

/// A Linkwitz-Riley crossover (two Butterworth biquads per side): the low
/// and high parts sum to an all-pass of the input.
#[derive(Clone, Copy, Debug, Default)]
pub struct Crossover {
    low: [Biquad; 2],
    high: [Biquad; 2],
}

impl Crossover {
    pub fn set(&mut self, frequency: f32, sample_rate: f32) {
        for stage in &mut self.low {
            stage.set_low_pass(frequency, sample_rate);
        }
        for stage in &mut self.high {
            stage.set_high_pass(frequency, sample_rate);
        }
    }

    pub fn reset(&mut self) {
        for stage in &mut self.low {
            stage.reset();
        }
        for stage in &mut self.high {
            stage.reset();
        }
    }

    /// Splits a channel's input into low and high.
    pub fn split(&mut self, channel: usize, x: f32) -> (f32, f32) {
        let low0 = self.low[0].process(channel, x);
        let low = self.low[1].process(channel, low0);
        let high0 = self.high[0].process(channel, x);
        let high = self.high[1].process(channel, high0);
        (low, high)
    }
}

/// A stereo delay line read with a fractional delay.
#[derive(Clone, Debug, Default)]
pub struct DelayLine {
    buffer: Vec<f32>,
    frames: usize,
    write: usize,
}

impl DelayLine {
    pub fn allocate(&mut self, max_frames: usize) {
        self.buffer = vec![0.0; 2 * max_frames];
        self.frames = max_frames;
        self.write = 0;
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
    }

    pub fn max_frames(&self) -> usize {
        self.frames
    }

    /// The sample of a channel `delay_frames` before the next write.
    pub fn read(&self, channel: usize, delay_frames: f32) -> f32 {
        let delay_frames = delay_frames.clamp(1.0, (self.frames - 2) as f32);
        let mut position = self.write as f32 - delay_frames;
        if position < 0.0 {
            position += self.frames as f32;
        }
        let index = position as usize;
        let fraction = position - index as f32;
        let next = if index + 1 == self.frames { 0 } else { index + 1 };
        let a = self.buffer[2 * index + channel];
        let b = self.buffer[2 * next + channel];
        a + (b - a) * fraction
    }

    pub fn write(&mut self, left: f32, right: f32) {
        self.buffer[2 * self.write] = left;
        self.buffer[2 * self.write + 1] = right;
        self.write += 1;
        if self.write == self.frames {
            self.write = 0;
        }
    }

    /// Write a single channel (for mono delay lines like pre_delay in FDN).
    pub fn write_mono(&mut self, channel: usize, value: f32) {
        self.buffer[2 * self.write + channel] = value;
        self.write += 1;
        if self.write == self.frames {
            self.write = 0;
        }
    }

    /// For reading at any position (e.g. Release): the frame written last.
    pub fn write_index(&self) -> usize {
        self.write
    }

    pub fn at(&self, channel: usize, position: f64) -> f32 {
        let frames = self.frames as f64;
        let mut wrapped = position % frames;
        if wrapped < 0.0 {
            wrapped += frames;
        }
        let index = wrapped as usize;
        let fraction = (wrapped - index as f64) as f32;
        let next = if index + 1 == self.frames { 0 } else { index + 1 };
        let a = self.buffer[2 * index + channel];
        let b = self.buffer[2 * next + channel];
        a + (b - a) * fraction
    }
}

/// A pitch shifter with two crossfaded taps of a short delay (Doppler
/// shifting): cheap, for the repeats of the Spiral echoes.
#[derive(Clone, Debug, Default)]
pub struct PitchShifter {
    line: DelayLine,
    window: usize,
    phase: f32,
}

impl PitchShifter {
    pub fn allocate(&mut self, window_frames: usize) {
        self.window = window_frames;
        self.line.allocate(window_frames * 2 + 4);
        self.phase = 0.0;
    }

    pub fn clear(&mut self) {
        self.line.clear();
        self.phase = 0.0;
    }

    pub fn process(&mut self, left: &mut f32, right: &mut f32, ratio: f32) {
        self.line.write(*left, *right);
        self.phase += (1.0 - ratio) / self.window as f32;
        self.phase -= self.phase.floor();
        let phase2 = if self.phase + 0.5 >= 1.0 { self.phase - 0.5 } else { self.phase + 0.5 };
        // Triangular windows, summing to one
        let gain1 = 1.0 - (2.0 * self.phase - 1.0).abs();
        let gain2 = 1.0 - gain1;
        let delay1 = 1.0 + self.phase * self.window as f32;
        let delay2 = 1.0 + phase2 * self.window as f32;
        *left = gain1 * self.line.read(0, delay1) + gain2 * self.line.read(0, delay2);
        *right = gain1 * self.line.read(1, delay1) + gain2 * self.line.read(1, delay2);
    }
}
