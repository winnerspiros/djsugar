//! The instrument's arpeggiator: the notes held play one at a time, in time with the mix's beat. `[Instrument1],arp` turns it on; `arp_mode` is Up, Down, Up-down, Random or Order (as played, see
//! keys are played. It takes the notes after scale and chord mode, so a chord held with one key is arpeggiated too. Steps fall on the master deck's beat grid while it plays, and run on at its tempo otherwise.
//! keys are played. It takes the notes after scale and chord mode, so a chord held with one key is arpeggiated too. Steps fall on the master deck's beat grid while it plays, and run on at its tempo

use djsugar_plugin_host::{Midi, Transport};

/// `arp_mode` values.
pub const MODES: [&str; 5] = ["Up", "Down", "Up-down", "Random", "Order"];
pub const UP: u8 = 0;
pub const DOWN: u8 = 1;
pub const UP_DOWN: u8 = 2;
pub const RANDOM: u8 = 3;
pub const ORDER: u8 = 4;

/// `arp_rate` values: the step's name and steps per beat.
pub const RATES: [(&str, f64); 6] =
    [("1/4", 1.0), ("1/8", 2.0), ("1/8T", 3.0), ("1/16", 4.0), ("1/16T", 6.0), ("1/32", 8.0)];
/// The rate it starts at: 1/16.
pub const DEFAULT_RATE: usize = 3;

/// The most notes it follows at once.
const MAX_HELD: usize = 32;

/// The arpeggiator's settings, from its controls each buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArpSettings {
    pub on: bool,
    pub mode: u8,
    pub rate: usize,
    pub gate: f64,
    pub octaves: u8,
    pub hold: bool,
}

impl Default for ArpSettings {
    fn default() -> Self {
        Self { on: false, mode: UP, rate: DEFAULT_RATE, gate: 0.5, octaves: 1, hold: false }
    }
}

pub struct Arp {
    settings: ArpSettings,
    /// The notes held (or latched with hold), as played: note, velocity.
    held: [(u8, u8); MAX_HELD],
    count: usize,
    /// Keys down now; with hold, the notes stay until new ones come.
    keys_down: usize,
    channel: u8,
    /// The note sounding, if any, and the beat its gate closes on.
    sounding: Option<(u8, u8)>,
    gate_end: f64,
    /// Steps played since it started.
    step: usize,
    /// Where it is when the mix's beat is not moving.
    beat: f64,
    /// The last step played, on the grid of steps from beat 0, so a beat
    /// that steps back a hair between buffers does not play it twice.
    last_k: f64,
    rng: u32,
}

impl Default for Arp {
    fn default() -> Self {
        Self {
            settings: ArpSettings::default(),
            held: [(0, 0); MAX_HELD],
            count: 0,
            keys_down: 0,
            channel: 0,
            sounding: None,
            gate_end: 0.0,
            step: 0,
            beat: 0.0,
            last_k: f64::NEG_INFINITY,
            rng: 0x9e37_79b9,
        }
    }
}

fn push(out: &mut Vec<Midi>, m: Midi) {
    if out.len() < out.capacity() {
        out.push(m);
    }
}

impl Arp {
    pub fn is_on(&self) -> bool {
        self.settings.on
    }

    /// New settings. Turning it on or off ends the notes sounding (all of
    /// them, on every channel, so keys held as it changes do not hang).
    pub fn set(&mut self, s: ArpSettings, out: &mut Vec<Midi>) {
        let s = ArpSettings {
            rate: s.rate.min(RATES.len() - 1),
            gate: s.gate.clamp(0.05, 1.0),
            octaves: s.octaves.clamp(1, 4),
            ..s
        };
        if s.on != self.settings.on {
            for ch in 0..16 {
                push(out, Midi { frame: 0, data: [0xb0 | ch, 123, 0] });
            }
            *self = Self { settings: s, rng: self.rng, beat: self.beat, ..Self::default() };
            return;
        }
        if !s.hold && self.settings.hold && self.keys_down == 0 {
            self.count = 0;
        }
        self.settings = s;
    }

    /// Takes in a note played: note-ons and note-offs change what it plays;
    /// anything else goes straight to `out`.
    fn take(&mut self, m: Midi, out: &mut Vec<Midi>) {
        if let Some((ch, key, _)) = m.as_note_on() {
            if self.keys_down == 0 && self.settings.hold {
                // New keys after letting go: a new latched chord.
                self.count = 0;
            }
            self.keys_down += 1;
            self.channel = ch;
            if !self.held[..self.count].iter().any(|h| h.0 == key) && self.count < MAX_HELD {
                if self.count == 0 {
                    self.step = 0;
                }
                self.held[self.count] = (key, m.data[2]);
                self.count += 1;
            }
        } else if let Some((_, key)) = m.as_note_off() {
            self.keys_down = self.keys_down.saturating_sub(1);
            if !self.settings.hold {
                if let Some(i) = self.held[..self.count].iter().position(|h| h.0 == key) {
                    self.held.copy_within(i + 1..self.count, i);
                    self.count -= 1;
                }
            }
        } else {
            if m.data[0] & 0xf0 == 0xb0 && matches!(m.data[1], 120 | 123) {
                self.count = 0;
                self.keys_down = 0;
            }
            push(out, m);
        }
    }

    /// The note for step `step`, from the notes held.
    fn note_at(&mut self, step: usize) -> Option<(u8, u8)> {
        let held = &self.held[..self.count];
        if held.is_empty() {
            return None;
        }
        let octaves = usize::from(self.settings.octaves);
        let len = held.len() * octaves;
        let mut sorted = [(0u8, 0u8); MAX_HELD];
        sorted[..held.len()].copy_from_slice(held);
        let sorted = &mut sorted[..held.len()];
        sorted.sort_unstable_by_key(|h| h.0);
        // The i-th note going up through the octaves.
        let up = |i: usize| {
            let (n, v) = sorted[i % sorted.len()];
            (n as usize + 12 * (i / sorted.len()), v)
        };
        let (n, v) = match self.settings.mode {
            DOWN => up(len - 1 - step % len),
            UP_DOWN if len > 1 => {
                let cycle = 2 * len - 2;
                let i = step % cycle;
                up(if i < len { i } else { cycle - i })
            }
            RANDOM => {
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 17;
                self.rng ^= self.rng << 5;
                up(self.rng as usize % len)
            }
            ORDER => {
                let (n, v) = held[step % held.len()];
                (n as usize + 12 * (step / held.len() % octaves), v)
            }
            _ => up(step % len),
        };
        (n <= 127).then_some((n as u8, v))
    }

    /// Takes in this buffer's notes (`incoming`) and adds what it plays
    /// over `frames` at `sample_rate` to `out`, in time with `transport`.
    pub fn process(
        &mut self,
        incoming: &[Midi],
        frames: usize,
        sample_rate: f32,
        transport: Transport,
        out: &mut Vec<Midi>,
    ) {
        for &m in incoming {
            self.take(m, out);
        }
        let bpm = if transport.bpm > 0.0 { transport.bpm } else { 120.0 };
        let start = if transport.playing { transport.beat } else { self.beat };
        let beats_per_frame = bpm / 60.0 / f64::from(sample_rate);
        let end = start + frames as f64 * beats_per_frame;
        self.beat = end;
        let per_beat = RATES[self.settings.rate].1;
        let frame_of =
            |beat: f64| (((beat - start) / beats_per_frame).max(0.0) as usize).min(frames.saturating_sub(1)) as u32;
        // Steps and gates closing in this buffer, in order.
        let mut k = (start * per_beat).ceil();
        if k <= self.last_k && self.last_k - k < 2.0 {
            k = self.last_k + 1.0;
        }
        loop {
            let step_beat = k / per_beat;
            let close = self.sounding.map(|_| self.gate_end);
            match close {
                Some(g) if g < end && g <= step_beat => {
                    let (n, _) = self.sounding.take().unwrap();
                    push(out, Midi::note_off(frame_of(g), self.channel, n));
                    continue;
                }
                _ => {}
            }
            if step_beat >= end {
                break;
            }
            let frame = frame_of(step_beat);
            if let Some((n, _)) = self.sounding.take() {
                push(out, Midi::note_off(frame, self.channel, n));
            }
            if let Some((n, v)) = self.note_at(self.step) {
                push(out, Midi::note_on(frame, self.channel, n, v));
                self.sounding = Some((n, v));
                self.gate_end = step_beat + self.settings.gate / per_beat;
                self.step += 1;
            }
            self.last_k = k;
            k += 1.0;
        }
        // Let go of everything: the last note stops at once.
        if self.count == 0 {
            if let Some((n, _)) = self.sounding.take() {
                push(out, Midi::note_off(0, self.channel, n));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// 120 BPM, playing from beat 0: a beat is 24,000 frames.
    fn transport(beat: f64) -> Transport {
        Transport { bpm: 120.0, beat, playing: true }
    }

    fn arp(s: ArpSettings) -> Arp {
        let mut a = Arp::default();
        let mut out = Vec::with_capacity(64);
        a.set(ArpSettings { on: true, ..s }, &mut out);
        a
    }

    /// Plays `beats` beats in buffers of 480 frames and gives the notes
    /// started, with the frame (from the first buffer) each started on.
    fn run(a: &mut Arp, incoming: &[Midi], from: f64, beats: f64) -> Vec<(u8, usize)> {
        let mut started = Vec::new();
        let buffers = (beats * 24_000.0 / 480.0) as usize;
        for b in 0..buffers {
            let mut out = Vec::with_capacity(64);
            let beat = from + b as f64 * 480.0 / 24_000.0;
            a.process(if b == 0 { incoming } else { &[] }, 480, SR, transport(beat), &mut out);
            for m in out {
                if let Some((_, n, _)) = m.as_note_on() {
                    started.push((n, b * 480 + m.frame as usize));
                }
            }
        }
        started
    }

    fn chord(keys: &[u8]) -> Vec<Midi> {
        keys.iter().map(|&k| Midi::note_on(0, 0, k, 100)).collect()
    }

    #[test]
    fn up_plays_the_notes_held_on_the_beat_grid() {
        let mut a = arp(ArpSettings { rate: 1, ..Default::default() });
        let started = run(&mut a, &chord(&[67, 60, 64]), 0.0, 2.0);
        // 1/8 notes: every 12,000 frames, low to high.
        assert_eq!(started, [(60, 0), (64, 12_000), (67, 24_000), (60, 36_000)]);
    }

    #[test]
    fn modes_and_octaves_order_the_notes() {
        let notes = |s: ArpSettings, keys: &[u8]| {
            let mut a = arp(s);
            run(&mut a, &chord(keys), 0.0, 1.0).into_iter().map(|n| n.0).collect::<Vec<_>>()
        };
        let s = ArpSettings { rate: 5, ..Default::default() };
        assert_eq!(notes(ArpSettings { mode: DOWN, ..s }, &[60, 64, 67]), [67, 64, 60, 67, 64, 60, 67, 64]);
        assert_eq!(notes(ArpSettings { mode: UP_DOWN, ..s }, &[60, 64, 67]), [60, 64, 67, 64, 60, 64, 67, 64]);
        assert_eq!(notes(ArpSettings { mode: ORDER, ..s }, &[67, 60, 64]), [67, 60, 64, 67, 60, 64, 67, 60]);
        assert_eq!(notes(ArpSettings { octaves: 2, ..s }, &[60, 64]), [60, 64, 72, 76, 60, 64, 72, 76]);
        let random = notes(ArpSettings { mode: RANDOM, ..s }, &[60, 64, 67]);
        assert!(random.iter().all(|n| [60, 64, 67].contains(n)));
    }

    #[test]
    fn the_gate_ends_each_note_and_letting_go_stops() {
        let mut a = arp(ArpSettings { rate: 0, gate: 0.25, ..Default::default() });
        let mut out = Vec::with_capacity(64);
        // A quarter-beat gate at 1/4: the note ends 6,000 frames in.
        a.process(&chord(&[60]), 480, SR, transport(0.0), &mut out);
        assert_eq!(out[0].data, [0x90, 60, 100]);
        let mut offs = Vec::new();
        for b in 1..20 {
            out.clear();
            a.process(&[], 480, SR, transport(b as f64 / 50.0), &mut out);
            offs.extend(out.iter().filter(|m| m.as_note_off().is_some()).map(|m| b * 480 + m.frame as usize));
        }
        assert_eq!(offs, [6_000]);
        // Let go, and nothing more plays.
        assert!(run(&mut a, &[Midi::note_off(0, 0, 60)], 1.0, 2.0).is_empty());
    }

    #[test]
    fn hold_keeps_playing_until_new_keys() {
        let mut a = arp(ArpSettings { rate: 0, hold: true, ..Default::default() });
        let mut keys = chord(&[60, 64]);
        keys.extend([Midi::note_off(0, 0, 60), Midi::note_off(0, 0, 64)]);
        let n: Vec<u8> = run(&mut a, &keys, 0.0, 2.0).into_iter().map(|n| n.0).collect();
        assert_eq!(n, [60, 64], "still playing after the keys are let go");
        let n: Vec<u8> = run(&mut a, &chord(&[62]), 2.0, 2.0).into_iter().map(|n| n.0).collect();
        assert_eq!(n, [62, 62], "a new key starts a new set");
    }

    #[test]
    fn a_beat_that_steps_back_a_hair_plays_no_step_twice() {
        let mut a = arp(ArpSettings { rate: 0, ..Default::default() });
        let mut out = Vec::with_capacity(64);
        a.process(&chord(&[60]), 480, SR, transport(0.99), &mut out);
        assert_eq!(out.len(), 1, "beat 1 starts in this buffer");
        out.clear();
        a.process(&[], 480, SR, transport(1.0 - 1e-9), &mut out);
        assert!(out.iter().all(|m| m.as_note_on().is_none()));
    }

    #[test]
    fn it_runs_on_while_the_mix_is_stopped() {
        let mut a = arp(ArpSettings { rate: 0, ..Default::default() });
        let stopped = Transport { bpm: 120.0, beat: 7.3, playing: false };
        let mut on = 0;
        for b in 0..99 {
            let mut out = Vec::with_capacity(64);
            let keys = chord(&[60]);
            a.process(if b == 0 { &keys } else { &[] }, 480, SR, stopped, &mut out);
            on += out.iter().filter(|m| m.as_note_on().is_some()).count();
        }
        // 99 buffers of 480 frames is just under two beats.
        assert_eq!(on, 2);
    }
}
