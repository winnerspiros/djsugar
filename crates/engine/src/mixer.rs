use std::sync::Arc;

use crate::biquad::{Biquad, Kind};
use crate::{Command, DECKS, Deck, DeckState, Telemetry, Track};

pub const VOICES: usize = 16;
const LOW_HZ: f32 = 250.0;
const HIGH_HZ: f32 = 3000.0;
const Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[derive(Debug)]
pub struct Channel {
    pub deck: Deck,
    pub gain: f32,
    pub volume: f32,
    eq: [f32; 3],
    kill: [bool; 3],
    filter: f32,
    bands: [Biquad; 3],
    filt: Biquad,
    peak: f32,
}

impl Channel {
    fn new(rate: u32) -> Self {
        Self {
            deck: Deck::new(rate),
            gain: 1.0,
            volume: 1.0,
            eq: [1.0; 3],
            kill: [false; 3],
            filter: 0.0,
            bands: [Biquad::identity(); 3],
            filt: Biquad::identity(),
            peak: 0.0,
        }
    }

    fn tune(&mut self, rate: f32) {
        let g = |i: usize| if self.kill[i] { 1e-4 } else { self.eq[i].max(1e-4) };
        self.bands[0].set(Kind::LowShelf(g(0)), LOW_HZ, Q, rate);
        self.bands[1].set(Kind::Peak(g(1)), (LOW_HZ * HIGH_HZ).sqrt(), 0.5, rate);
        self.bands[2].set(Kind::HighShelf(g(2)), HIGH_HZ, Q, rate);
        let f = self.filter;
        if f.abs() < 0.02 {
            self.filt = Biquad::identity();
        } else if f < 0.0 {
            self.filt.set(Kind::Low, 20.0 * 1000f32.powf(1.0 + f), Q, rate);
        } else {
            self.filt.set(Kind::High, 20.0 * 1000f32.powf(f), Q, rate);
        }
    }
}

#[derive(Debug)]
pub struct Mixer {
    rate: f32,
    pub channels: [Channel; DECKS],
    crossfader: f32,
    main: f32,
    scratch: Vec<[f32; 2]>,
    main_peak: [f32; 2],
    voices: [Option<Voice>; VOICES],
    pending: Option<Arc<Track>>,
}

#[derive(Debug)]
struct Voice {
    track: Arc<Track>,
    position: f64,
    gain: f32,
}

impl Mixer {
    pub fn new(rate: f32, max_block: usize) -> Self {
        Self {
            rate,
            channels: std::array::from_fn(|_| Channel::new(rate as u32)),
            crossfader: 0.0,
            main: 1.0,
            scratch: vec![[0.0; 2]; max_block],
            main_peak: [0.0; 2],
            voices: std::array::from_fn(|_| None),
            pending: None,
        }
    }

    pub fn apply(&mut self, c: Command) -> Option<Arc<Track>> {
        let rate = self.rate;
        match c {
            Command::Load(i, t) => {
                let c = self.channels.get_mut(i)?;
                c.deck.position = 0.0;
                c.deck.looping = None;
                c.deck.invalidate();
                c.deck.playing = false;
                return c.deck.track.replace(t);
            }
            Command::Play(i, p) => self.channels.get_mut(i)?.deck.playing = p,
            Command::Seek(i, s) => {
                let d = &mut self.channels.get_mut(i)?.deck;
                d.position = s.max(0.0);
                d.invalidate();
            }
            Command::KeyLock(i, on) => {
                let d = &mut self.channels.get_mut(i)?.deck;
                d.key_lock = on;
                d.invalidate();
            }
            Command::Loop(i, l) => {
                self.channels.get_mut(i)?.deck.looping = l.filter(|(a, b)| b - a >= 1.0);
            }
            Command::Key(i, semitones) => self.channels.get_mut(i)?.deck.semitones = semitones,
            Command::Rate(i, r) => self.channels.get_mut(i)?.deck.rate = r,
            Command::Nudge(i, n) => self.channels.get_mut(i)?.deck.nudge = n,
            Command::Gain(i, g) => self.channels.get_mut(i)?.gain = g,
            Command::Volume(i, v) => self.channels.get_mut(i)?.volume = v.clamp(0.0, 1.0),
            Command::Eq(i, e) => {
                let c = self.channels.get_mut(i)?;
                c.eq = e;
                c.tune(rate);
            }
            Command::Kill(i, k) => {
                let c = self.channels.get_mut(i)?;
                c.kill = k;
                c.tune(rate);
            }
            Command::Filter(i, f) => {
                let c = self.channels.get_mut(i)?;
                c.filter = f.clamp(-1.0, 1.0);
                c.tune(rate);
            }
            Command::Sample(track, gain) => {
                let free = self.voices.iter().position(Option::is_none);
                let slot = free.unwrap_or_else(|| {
                    let oldest = self.voices.iter().enumerate();
                    oldest.max_by(|a, b| voice_pos(a.1).total_cmp(&voice_pos(b.1))).map_or(0, |v| v.0)
                });
                let old = self.voices[slot].replace(Voice { track, position: 0.0, gain });
                return old.map(|v| v.track);
            }
            Command::StopSamples => {
                for v in &mut self.voices {
                    if let Some(v) = v.take() {
                        self.pending = Some(v.track);
                        break;
                    }
                }
            }
            Command::Crossfader(x) => self.crossfader = x.clamp(-1.0, 1.0),
            Command::Main(v) => self.main = v.max(0.0),
        }
        None
    }

    fn side_gain(&self, i: usize) -> f32 {
        let x = self.crossfader;
        match i % 2 {
            0 => (1.0 - x).min(1.0),
            _ => (1.0 + x).min(1.0),
        }
    }

    pub fn render(&mut self, out: &mut [f32], retired: &mut rtrb::Producer<Arc<Track>>) {
        out.fill(0.0);
        if let Some(t) = self.pending.take() {
            let _ = retired.push(t);
        }
        let mut done = 0;
        let total = out.len() / 2;
        while done < total {
            let n = (total - done).min(self.scratch.len());
            let dst = &mut out[done * 2..(done + n) * 2];
            for i in 0..DECKS {
                let xf = self.side_gain(i);
                let (rate, buf) = (self.rate, &mut self.scratch[..n]);
                let c = &mut self.channels[i];
                if !c.deck.playing {
                    c.peak = 0.0;
                    continue;
                }
                c.deck.render(rate, buf);
                let g = c.gain * c.volume * xf;
                let mut peak = 0.0f32;
                for (f, o) in buf.iter_mut().zip(dst.as_chunks_mut::<2>().0) {
                    for k in 0..2 {
                        let mut s = f[k];
                        for b in &mut c.bands {
                            s = b.run(k, s);
                        }
                        s = c.filt.run(k, s) * g;
                        peak = peak.max(s.abs());
                        o[k] += s;
                    }
                }
                c.peak = peak;
            }
            done += n;
        }
        for slot in &mut self.voices {
            let Some(v) = slot else { continue };
            let step = f64::from(v.track.rate) / f64::from(self.rate);
            let f = &v.track.frames;
            for o in out.as_chunks_mut::<2>().0 {
                let i = v.position as usize;
                let Some(s) = f.get(i) else { break };
                o[0] += s[0] * v.gain;
                o[1] += s[1] * v.gain;
                v.position += step;
            }
            if v.position as usize >= f.len()
                && let Some(v) = slot.take()
            {
                let _ = retired.push(v.track);
            }
        }
        let mut pk = [0.0f32; 2];
        for f in out.as_chunks_mut::<2>().0 {
            for k in 0..2 {
                let s = f[k] * self.main;
                let s = s.clamp(-1.0, 1.0);
                pk[k] = pk[k].max(s.abs());
                f[k] = s;
            }
        }
        self.main_peak = pk;
    }

    pub fn telemetry(&self) -> Telemetry {
        Telemetry {
            decks: std::array::from_fn(|i| {
                let c = &self.channels[i];
                DeckState { position: c.deck.position, playing: c.deck.playing, peak: c.peak }
            }),
            main_peak: self.main_peak,
        }
    }
}

fn voice_pos(v: &Option<Voice>) -> f64 {
    v.as_ref().map_or(0.0, |v| v.position)
}
