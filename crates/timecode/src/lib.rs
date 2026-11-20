//! Timecode vinyl and CD decoding (DVS): turns the stereo signal of a
//! control record into a pitch (speed and direction) and an absolute
//! position on the record.
//!
//! A control record carries a sine carrier on both channels, a quarter
//! cycle apart, so which channel leads tells the direction. Each carrier
//! cycle holds one bit, as a loud or a quiet cycle. The bits come from a
//! linear feedback shift register, so any `bits` bits in a row name a
//! unique place on the record.

use std::sync::OnceLock;

/// One kind of control record or CD.
#[derive(Debug)]
pub struct Format {
    /// Shown in settings.
    pub name: &'static str,
    /// Carrier cycles per second at normal speed.
    pub carrier_hz: f64,
    /// Width of the shift register.
    pub bits: u32,
    /// The register at the start of the record.
    pub seed: u32,
    /// Feedback taps.
    pub taps: u32,
    /// Cycles on the record.
    pub length: u32,
}

/// The records and CDs DJ Sugar reads.
pub static FORMATS: [Format; 7] = [
    Format {
        name: "Serato CV02 vinyl, side A",
        carrier_hz: 1000.0,
        bits: 20,
        seed: 0x59017,
        taps: 0x361e4,
        length: 712_000,
    },
    Format {
        name: "Serato CV02 vinyl, side B",
        carrier_hz: 1000.0,
        bits: 20,
        seed: 0x8f3c6,
        taps: 0x4f0d8,
        length: 922_000,
    },
    Format {
        name: "Serato control CD",
        carrier_hz: 1000.0,
        bits: 20,
        seed: 0xd8b40,
        taps: 0x34d54,
        length: 950_000,
    },
    Format {
        name: "Traktor Scratch MK1 vinyl, side A",
        carrier_hz: 2000.0,
        bits: 23,
        seed: 0x134503,
        taps: 0x041040,
        length: 1_500_000,
    },
    Format {
        name: "Traktor Scratch MK1 vinyl, side B",
        carrier_hz: 2000.0,
        bits: 23,
        seed: 0x32066c,
        taps: 0x041040,
        length: 2_110_000,
    },
    Format {
        name: "MixVibes V2 vinyl",
        carrier_hz: 1300.0,
        bits: 20,
        seed: 0x22c90,
        taps: 0x00008,
        length: 950_000,
    },
    Format {
        name: "MixVibes 7-inch vinyl",
        carrier_hz: 1300.0,
        bits: 20,
        seed: 0x22c90,
        taps: 0x00008,
        length: 312_000,
    },
];

impl Format {
    fn mask(&self) -> u32 {
        (1 << self.bits) - 1
    }

    /// The register one cycle further into the record.
    pub fn next(&self, code: u32) -> u32 {
        let top = (code & (self.taps | 1)).count_ones() & 1;
        (code >> 1) | top << (self.bits - 1)
    }

    /// The register one cycle back towards the start.
    pub fn prev(&self, code: u32) -> u32 {
        let top = code >> (self.bits - 1);
        let rest = (code << 1) & self.mask();
        let low = (top ^ (rest & self.taps & !1).count_ones()) & 1;
        rest | low
    }

    /// What the record sends when played from `start` cycles in at `speed`
    /// for `seconds`, with hiss of `noise` peak: for tests, and to check a
    /// setup without a turntable.
    pub fn signal(
        &self,
        start: f64,
        speed: f64,
        rate: u32,
        seconds: f64,
        noise: f32,
    ) -> Vec<[f32; 2]> {
        let rate = f64::from(rate);
        let mut codes = vec![self.seed];
        for _ in 0..self.length {
            codes.push(self.next(*codes.last().expect("the seed")));
        }
        let mut seed = 1u32;
        let mut hiss = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32 - 0.5) * 2.0 * noise
        };
        (0..(seconds * rate) as usize)
            .map(|n| {
                let phase = start + speed * self.carrier_hz * n as f64 / rate;
                // A cycle's loudness changes away from where it is read.
                let cycle = (phase + 0.75).floor().max(0.0) as usize;
                let one = codes.get(cycle).is_some_and(|c| c >> (self.bits - 1) == 1);
                let amp = if one { 1.0 } else { 0.7 };
                let t = std::f64::consts::TAU * phase;
                [
                    (amp * t.sin()) as f32 + hiss(),
                    (amp * t.cos()) as f32 + hiss(),
                ]
            })
            .collect()
    }

    fn table(&self) -> &'static OnceLock<Vec<u32>> {
        static TABLES: [OnceLock<Vec<u32>>; 7] = [const { OnceLock::new() }; 7];
        let i = FORMATS
            .iter()
            .position(|f| std::ptr::eq(f, self))
            .expect("a listed format");
        &TABLES[i]
    }

    /// Whether [`Format::prepare`] has run, so decoders can tell positions.
    pub fn ready(&self) -> bool {
        self.table().get().is_some()
    }

    /// Builds the table of where each code sits on the record (up to 32 MB,
    /// a fraction of a second), off the audio thread. Until then decoders
    /// read the speed only.
    pub fn prepare(&self) -> &'static [u32] {
        self.table().get_or_init(|| {
            let mut places = vec![u32::MAX; 1 << self.bits];
            let mut code = self.seed;
            for at in 0..self.length {
                places[code as usize] = at;
                code = self.next(code);
            }
            places
        })
    }
}

/// Bits in a row that must follow the register's own sequence before a
/// position is trusted.
const TRUST: u32 = 16;

/// Reads one deck's timecode, a block of stereo frames at a time.
pub struct Decoder {
    format: &'static Format,
    rate: f64,
    last: [f32; 2],
    /// Samples since the last carrier cycle began.
    since: f64,
    /// Smoothed samples per cycle.
    period: f64,
    /// Running level that splits loud cycles (1) from quiet ones (0).
    level: f32,
    code: u32,
    /// Bits in a row that agreed with the register's sequence.
    run: u32,
    forward: bool,
    /// Position at the last cycle, in cycles from the record's start.
    at: Option<f64>,
}

impl Decoder {
    /// The record or CD this reads.
    pub fn format(&self) -> &'static Format {
        self.format
    }

    /// Whether the position is known, not only the speed.
    pub fn locked(&self) -> bool {
        self.at.is_some()
    }

    /// Never allocates, so the audio thread can make one.
    pub fn new(format: &'static Format, sample_rate: u32) -> Self {
        Self {
            format,
            rate: f64::from(sample_rate),
            last: [0.0; 2],
            since: 0.0,
            period: 0.0,
            level: 0.0,
            code: 0,
            run: 0,
            forward: true,
            at: None,
        }
    }

    /// Feeds left/right frames from the turntable's input.
    pub fn process(&mut self, frames: &[[f32; 2]]) {
        for &[l, r] in frames {
            self.since += 1.0;
            let [pl, pr] = self.last;
            self.last = [l, r];
            if pl >= 0.0 || l < 0.0 {
                continue;
            }
            // The left channel rose through zero: a new cycle.
            let frac = f64::from(-pl / (l - pl));
            let right = pr + (r - pr) * frac as f32;
            let samples = self.since - 1.0 + frac;
            self.since = 1.0 - frac;
            self.period = if self.period == 0.0 {
                samples
            } else {
                self.period + (samples - self.period) * 0.25
            };
            self.forward = right > 0.0;
            let loud = right.abs();
            if self.level == 0.0 {
                self.level = loud;
            }
            let bit = u32::from(loud > self.level);
            self.level += (loud - self.level) / 64.0;
            self.read(bit);
        }
    }

    fn read(&mut self, bit: u32) {
        let f = self.format;
        let (code, expected) = if self.forward {
            ((self.code >> 1) | bit << (f.bits - 1), f.next(self.code))
        } else {
            (((self.code << 1) & f.mask()) | bit, f.prev(self.code))
        };
        self.run = if code == expected { self.run + 1 } else { 0 };
        self.code = code;
        self.at = match self.at {
            // Trusted: step along, even past a misread bit.
            Some(at) if self.run < TRUST => Some(at + if self.forward { 1.0 } else { -1.0 }),
            _ if self.run < TRUST => None,
            _ => match f.table().get().map_or(u32::MAX, |t| t[code as usize]) {
                u32::MAX => None,
                place if self.forward => Some(f64::from(place)),
                // Read backwards, the register's oldest bit is the cycle
                // just passed.
                place => Some(f64::from(place) - f64::from(f.bits - 1) - 0.5),
            },
        };
    }

    /// Speed against the record's normal speed: 1.0 at normal speed,
    /// negative backwards, 0.0 before any carrier is heard.
    pub fn pitch(&self) -> f64 {
        if self.period == 0.0 || self.since > self.period * 4.0 {
            return 0.0;
        }
        let speed = self.rate / self.period / self.format.carrier_hz;
        if self.forward {
            speed
        } else {
            -speed
        }
    }

    /// Where the needle is, in seconds from the record's start at normal
    /// speed, once enough of the code has been read.
    pub fn position(&self) -> Option<f64> {
        let cycles = self.at? + self.since / self.period * self.pitch().signum();
        Some(cycles / self.format.carrier_hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(f: &'static Format, samples: &[[f32; 2]]) -> Decoder {
        f.prepare();
        let mut d = Decoder::new(f, 44_100);
        for block in samples.chunks(256) {
            d.process(block);
        }
        d
    }

    #[test]
    fn every_format_names_each_place_once() {
        for f in &FORMATS {
            let places = f.prepare();
            let named = places.iter().filter(|&&p| p != u32::MAX).count();
            assert_eq!(named, f.length as usize, "{} repeats a code", f.name);
        }
    }

    #[test]
    fn the_register_steps_both_ways() {
        for f in &FORMATS {
            let mut code = f.seed;
            for _ in 0..10_000 {
                assert_eq!(f.prev(f.next(code)), code, "{}", f.name);
                code = f.next(code);
            }
        }
    }

    #[test]
    fn reads_speed_and_place_forwards() {
        for f in &FORMATS {
            let start = 5_000.0;
            let seconds = 0.5;
            let d = decode(f, &f.signal(start, 1.0, 44_100, seconds, 0.0));
            assert!(
                (d.pitch() - 1.0).abs() < 0.01,
                "{}: pitch {}",
                f.name,
                d.pitch()
            );
            let want = (start + seconds * f.carrier_hz) / f.carrier_hz;
            let got = d
                .position()
                .unwrap_or_else(|| panic!("{}: no position", f.name));
            assert!(
                (got - want).abs() < 2.0 / f.carrier_hz,
                "{}: {got} vs {want}",
                f.name
            );
        }
    }

    #[test]
    fn reads_speed_and_place_backwards_and_off_pitch() {
        let f = &FORMATS[0];
        for speed in [-1.0, -0.5, 0.92, 1.08] {
            let start = 300_000.0;
            let seconds = 0.6;
            let d = decode(f, &f.signal(start, speed, 44_100, seconds, 0.0));
            assert!(
                (d.pitch() - speed).abs() < 0.01,
                "speed {speed}: pitch {}",
                d.pitch()
            );
            let want = (start + speed * seconds * f.carrier_hz) / f.carrier_hz;
            let got = d.position().expect("a position");
            assert!(
                (got - want).abs() < 2.0 / f.carrier_hz,
                "speed {speed}: {got} vs {want}"
            );
        }
    }

    #[test]
    fn hiss_does_not_lose_the_place() {
        let f = &FORMATS[3];
        let d = decode(f, &f.signal(1_000_000.0, 1.0, 44_100, 0.5, 0.05));
        let want = (1_000_000.0 + 0.5 * f.carrier_hz) / f.carrier_hz;
        assert!((d.position().expect("a position") - want).abs() < 2.0 / f.carrier_hz);
    }

    #[test]
    fn silence_is_standing_still() {
        let d = decode(&FORMATS[0], &vec![[0.0; 2]; 44_100]);
        assert_eq!((d.pitch(), d.position()), (0.0, None));
    }
}
