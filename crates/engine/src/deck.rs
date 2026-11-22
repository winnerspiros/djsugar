use std::sync::Arc;

use sugar_stretch::Stretch;

#[derive(Debug)]
pub struct Track {
    pub rate: u32,
    pub frames: Box<[[f32; 2]]>,
}

impl Track {
    pub fn new(rate: u32, frames: Box<[[f32; 2]]>) -> Self {
        Self { rate, frames }
    }
}

#[derive(Debug)]
pub struct Deck {
    pub track: Option<Arc<Track>>,
    pub position: f64,
    pub playing: bool,
    pub rate: f64,
    pub nudge: f64,
    pub key_lock: bool,
    pub semitones: f32,
    stretch: Stretch,
    primed: bool,
}

impl Deck {
    pub fn new(out_rate: u32) -> Self {
        Self {
            track: None,
            position: 0.0,
            playing: false,
            rate: 0.0,
            nudge: 0.0,
            key_lock: false,
            semitones: 0.0,
            stretch: Stretch::standard(2, out_rate),
            primed: false,
        }
    }

    pub fn invalidate(&mut self) {
        self.primed = false;
    }

    pub fn render(&mut self, out_rate: f32, out: &mut [[f32; 2]]) {
        let Some(t) = &self.track else {
            out.fill([0.0; 2]);
            return;
        };
        if !self.playing {
            out.fill([0.0; 2]);
            return;
        }
        let speed = 1.0 + self.rate + self.nudge;
        let step = speed * f64::from(t.rate) / f64::from(out_rate);
        if self.key_lock || self.semitones != 0.0 {
            let t = t.clone();
            self.render_stretched(&t, step, out_rate, out);
            return;
        }
        self.primed = false;
        let f = &t.frames;
        let last = f.len() as isize - 1;
        let at = |i: isize| if (0..=last).contains(&i) { f[i as usize] } else { [0.0; 2] };
        let mut pos = self.position;
        for o in out.iter_mut() {
            let i = pos.floor() as isize;
            let x = (pos - i as f64) as f32;
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            for c in 0..2 {
                let c0 = p1[c];
                let c1 = 0.5 * (p2[c] - p0[c]);
                let c2 = p0[c] - 2.5 * p1[c] + 2.0 * p2[c] - 0.5 * p3[c];
                let c3 = 0.5 * (p3[c] - p0[c]) + 1.5 * (p1[c] - p2[c]);
                o[c] = ((c3 * x + c2) * x + c1) * x + c0;
            }
            pos += step;
        }
        self.position = pos;
        if pos as isize > last {
            self.playing = false;
            self.position = last.max(0) as f64;
        }
    }
}

impl Deck {
    fn render_stretched(&mut self, t: &Track, step: f64, out_rate: f32, out: &mut [[f32; 2]]) {
        let len = t.frames.len();
        let start = self.position.max(0.0) as usize;
        if start >= len {
            self.playing = false;
            out.fill([0.0; 2]);
            return;
        }
        let pitch = if self.key_lock { 1.0 } else { step as f32 * out_rate / t.rate as f32 };
        self.stretch.set_transpose(pitch * (self.semitones / 12.0).exp2() * t.rate as f32 / out_rate);
        if !self.primed {
            let lead = self.stretch.input_latency();
            let from = start.saturating_sub(lead);
            self.stretch.seek(t.frames[from..start].as_flattened());
            self.primed = true;
        }
        let next = self.position + step * out.len() as f64;
        let end = (next.max(0.0) as usize).min(len);
        self.stretch.process(t.frames[start..end.max(start)].as_flattened(), out.as_flattened_mut());
        self.position = next;
        if end >= len {
            self.playing = false;
            self.position = len.saturating_sub(1) as f64;
        }
    }
}
