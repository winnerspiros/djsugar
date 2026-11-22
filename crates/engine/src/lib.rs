mod biquad;
mod deck;
mod mixer;

use std::sync::Arc;

pub use deck::{Deck, Track};
pub use mixer::{Channel, Mixer, VOICES};

pub const DECKS: usize = 4;

#[derive(Debug)]
pub enum Command {
    Load(usize, Arc<Track>),
    Play(usize, bool),
    Seek(usize, f64),
    Rate(usize, f64),
    Nudge(usize, f64),
    KeyLock(usize, bool),
    Key(usize, f32),
    Gain(usize, f32),
    Volume(usize, f32),
    Eq(usize, [f32; 3]),
    Kill(usize, [bool; 3]),
    Filter(usize, f32),
    Sample(Arc<Track>, f32),
    StopSamples,
    Crossfader(f32),
    Main(f32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeckState {
    pub position: f64,
    pub playing: bool,
    pub peak: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Telemetry {
    pub decks: [DeckState; DECKS],
    pub main_peak: [f32; 2],
}

#[derive(Debug)]
pub struct Control {
    commands: rtrb::Producer<Command>,
    telemetry: triple_buffer::Output<Telemetry>,
    retired: rtrb::Consumer<Arc<Track>>,
    pub last: Telemetry,
}

impl Control {
    pub fn send(&mut self, c: Command) -> bool {
        self.commands.push(c).is_ok()
    }

    pub fn poll(&mut self) -> Telemetry {
        self.last = *self.telemetry.read();
        while self.retired.pop().is_ok() {}
        self.last
    }
}

#[derive(Debug)]
pub struct Engine {
    mixer: Mixer,
    commands: rtrb::Consumer<Command>,
    telemetry: triple_buffer::Input<Telemetry>,
    retired: rtrb::Producer<Arc<Track>>,
}

pub fn new(sample_rate: u32, max_block: usize) -> (Control, Engine) {
    let (cp, cc) = rtrb::RingBuffer::new(1024);
    let (tp, tc) = triple_buffer::triple_buffer(&Telemetry::default());
    let (rp, rc) = rtrb::RingBuffer::new(DECKS * 4 + mixer::VOICES * 2);
    (
        Control { commands: cp, telemetry: tc, retired: rc, last: Telemetry::default() },
        Engine { mixer: Mixer::new(sample_rate as f32, max_block), commands: cc, telemetry: tp, retired: rp },
    )
}

impl Engine {
    pub fn process(&mut self, out: &mut [f32]) {
        while let Ok(c) = self.commands.pop() {
            if let Some(old) = self.mixer.apply(c) {
                let _ = self.retired.push(old);
            }
        }
        self.mixer.render(out, &mut self.retired);
        self.telemetry.write(self.mixer.telemetry());
    }
}

#[cfg(test)]
mod tests;
