use std::sync::Arc;

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

#[derive(Debug, Default)]
pub struct Deck {
    pub track: Option<Arc<Track>>,
    pub position: f64,
    pub playing: bool,
    pub rate: f64,
    pub nudge: f64,
}

impl Deck {
    pub fn render(&mut self, out_rate: f32, out: &mut [[f32; 2]]) {
        let Some(t) = &self.track else {
            out.fill([0.0; 2]);
            return;
        };
        if !self.playing {
            out.fill([0.0; 2]);
            return;
        }
        let step = (1.0 + self.rate + self.nudge) * f64::from(t.rate) / f64::from(out_rate);
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
