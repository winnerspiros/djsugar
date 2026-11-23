//! The built-in sample library: fictional tracks whose audio is synthesised on
//! load (kick, bass, snare, hats and a pad, arranged like a dance track), so the
//! app plays real sound with no music files.

use std::sync::Arc;

use crate::track::{Audio, Source, Track};

fn hash(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}

/// Sample rate of synthesised audio.
pub const SYNTH_RATE: u32 = 44_100;

const ARTISTS: &[&str] = &[
    "Night Transit",
    "Lumen Drift",
    "Kaspar Velde",
    "Oona Reyes",
    "Static Garden",
    "Mira Solano",
    "The Low End Theory Club",
    "Halvorsen",
    "Coastline FM",
    "Dua Okafor",
    "Parallel Hearts",
    "Ivo Brandt",
];
const TITLES: &[&str] = &[
    "Midnight Circuit",
    "Glass Horizon",
    "Afterglow",
    "Pressure Point",
    "Northbound",
    "Velvet Room",
    "Signal Lost",
    "Higher Ground",
    "Paper Moons",
    "Low Tide",
    "Copper Sun",
    "Second Wind",
    "Neon Rain",
    "Tunnel Vision",
    "Drift Theory",
    "Open Water",
    "Satellite Hearts",
    "Slow Burn",
    "Daybreak",
    "Heatwave",
    "Static Bloom",
    "Long Way Home",
    "Subway Lights",
    "Echo Chamber",
];
const GENRES: &[&str] = &[
    "House",
    "Techno",
    "Deep House",
    "Tech House",
    "Disco",
    "Breaks",
    "Progressive",
];
const KEYS: &[&str] = &[
    "1A", "2A", "3A", "4A", "5A", "6A", "7A", "8A", "9A", "10A", "11A", "12A", "1B", "4B", "8B",
    "11B",
];

const VERSIONS: &[&str] = &["", " (Extended Mix)", " (Dub)", " (Night Edit)"];

/// Number of tracks in [`sample_library`].
pub const SAMPLE_LIBRARY_SIZE: usize = 72;

/// A deterministic sample library of fictional tracks.
pub fn sample_library() -> Vec<Arc<Track>> {
    (0..SAMPLE_LIBRARY_SIZE)
        .map(|id| {
            let h = hash(id as u64 + 1);
            let bpm = [
                118.0, 120.0, 122.0, 124.0, 125.0, 126.0, 128.0, 130.0, 132.0,
            ][(h % 9) as usize];
            let duration = 240.0 + (h >> 8) as f64 % 180.0;
            let first_beat = ((h >> 16) % 400) as f64 / 1000.0;
            Arc::new(Track {
                id,
                artist: ARTISTS[(h >> 24) as usize % ARTISTS.len()].to_string(),
                title: format!(
                    "{}{}",
                    TITLES[id % TITLES.len()],
                    VERSIONS[id / TITLES.len() % VERSIONS.len()]
                ),
                rating: (h >> 48) as u8 % 6,
                album: format!("{} EP", TITLES[(id * 7 + 3) % TITLES.len()]),
                genre: GENRES[(h >> 32) as usize % GENRES.len()].to_string(),
                bpm,
                key: KEYS[(h >> 40) as usize % KEYS.len()].to_string(),
                duration,
                first_beat,
                source: Source::Synth { seed: h },
                waveform: Arc::default(),
                audio: None,
                cover: None,
                grid: Default::default(),
                info: Default::default(),
            })
        })
        .collect()
}

/// Uniform noise in -1..1.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 23) as f32) - 1.0
    }
}

/// Bass notes per bar, in Hz, for a few simple progressions.
const BASS_LINES: [[f32; 4]; 4] = [
    [55.0, 55.0, 65.41, 49.0],
    [49.0, 58.27, 55.0, 43.65],
    [61.74, 55.0, 49.0, 55.0],
    [43.65, 49.0, 55.0, 65.41],
];

/// Renders a track's audio: intro, build, drop, breakdown, drop, outro.
pub fn render(track: &Track, seed: u64) -> Audio {
    render_with(track.bpm, track.first_beat, track.duration, seed)
}

pub fn render_with(bpm: f64, first_beat: f64, duration: f64, seed: u64) -> Audio {
    let sr = SYNTH_RATE as f64;
    let n = (duration * sr) as usize;
    let beat = 60.0 / bpm;
    let total_beats = ((duration - first_beat) / beat).max(1.0);
    let bass_line = BASS_LINES[(seed % 4) as usize];
    // Pad chord: a minor triad on a seed-dependent root.
    let root = 196.0 * 2f32.powf(((seed >> 8) % 7) as f32 / 12.0);
    let tau = std::f32::consts::TAU;
    let mut noise = Noise(seed | 1);

    // Everything tied to the beat is rendered once into tables, indexed by
    // samples since the beat (or the 16th), which keeps a 7 minute track fast.
    let beat_len = (beat * sr).ceil() as usize + 2;
    let mut kick = Vec::with_capacity(beat_len);
    let mut snare = Vec::with_capacity(beat_len);
    let mut bass_env = Vec::with_capacity(beat_len);
    let mut kick_phase = 0.0f32;
    for k in 0..beat_len {
        let tb = (k as f64 / sr) as f32;
        kick_phase += tau * (48.0 + 110.0 * (-tb * 28.0).exp()) / sr as f32;
        kick.push(kick_phase.sin() * (-tb * 9.0).exp());
        snare.push((noise.next() * 0.7 + (tb * 190.0 * tau).sin() * 0.4) * (-tb * 22.0).exp());
        let off = tb - beat as f32 / 2.0;
        bass_env.push(if off >= 0.0 { (-off * 5.0).exp() } else { 0.0 });
    }
    let sixteenth_len = beat_len / 4 + 2;
    let (mut prev, mut hp) = (0.0f32, 0.0f32);
    let hat: Vec<f32> = (0..sixteenth_len)
        .map(|k| {
            let white = noise.next();
            hp = 0.6 * (hp + white - prev);
            prev = white;
            hp * (-(k as f32 / sr as f32) * 70.0).exp()
        })
        .collect();

    // Oscillators as rotating phasors: one complex multiply per sample instead of a sin().
    struct Osc {
        c: f32,
        s: f32,
        dc: f32,
        ds: f32,
    }
    impl Osc {
        fn new(freq: f32, sr: f32) -> Self {
            let w = std::f32::consts::TAU * freq / sr;
            Self {
                c: 1.0,
                s: 0.0,
                dc: w.cos(),
                ds: w.sin(),
            }
        }
        fn set(&mut self, freq: f32, sr: f32) {
            let w = std::f32::consts::TAU * freq / sr;
            (self.dc, self.ds) = (w.cos(), w.sin());
        }
        #[inline]
        fn step(&mut self) -> (f32, f32) {
            let (c, s) = (
                self.c * self.dc - self.s * self.ds,
                self.s * self.dc + self.c * self.ds,
            );
            // Keep the amplitude at 1 despite rounding.
            let k = 1.5 - 0.5 * (c * c + s * s);
            (self.c, self.s) = (c * k, s * k);
            (self.c, self.s)
        }
    }
    let srf = sr as f32;
    let mut pads = [root, root * 1.189, root * 1.498].map(|f| Osc::new(f, srf));
    let mut bass = Osc::new(bass_line[0], srf);
    let mut lfo = Osc::new(0.07, srf);
    let mut bar_playing = usize::MAX;

    let mut frames = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / sr;
        let beats = (t - first_beat) / beat;
        let beat_index = beats.floor() as i64;
        let section = (beats / total_beats * 8.0).floor() as i32;
        let (kick_amt, bass_amt, pad_amt, hats_amt) = match section {
            i32::MIN..=-1 => (0.0, 0.0, 0.3, 0.0),
            0 => (1.0, 0.0, 0.35, 0.5),
            1 => (1.0, 0.6, 0.5, 0.8),
            2 | 3 => (1.0, 1.0, 0.6, 1.0),
            4 => (0.0, 0.25, 1.0, 0.3),
            5 | 6 => (1.0, 1.0, 0.7, 1.0),
            _ => (1.0, 0.3, 0.4, 0.6),
        };
        let since = ((beats - beats.floor()) * beat * sr) as usize;
        let on_beat = beats >= 0.0;

        let kick_s = if on_beat { kick[since] } else { 0.0 };
        let snare_s = if on_beat && beat_index.rem_euclid(2) == 1 {
            snare[since]
        } else {
            0.0
        };

        let bar = beat_index.div_euclid(4).rem_euclid(4) as usize;
        if bar != bar_playing {
            bar_playing = bar;
            bass.set(bass_line[bar], srf);
        }
        let (bc, bs) = bass.step();
        let bass_s = (bs + 0.35 * 2.0 * bs * bc) * bass_env[since];

        let sixteenths = beats * 4.0;
        let since16 = ((sixteenths - sixteenths.floor()) * beat / 4.0 * sr) as usize;
        let accent = if sixteenths.floor() as i64 % 4 == 2 {
            1.0
        } else {
            0.45
        };
        let hat_s = if on_beat { hat[since16] * accent } else { 0.0 };

        let swell = 0.65 + 0.35 * lfo.step().1;
        let pad_s = pads.iter_mut().map(|o| o.step().1).sum::<f32>() * swell / 3.0;

        let fade = ((duration - t) / 4.0).clamp(0.0, 1.0) as f32 * (t / 0.5).clamp(0.0, 1.0) as f32;
        let centre = kick_amt * kick_s * 0.8 + bass_amt * bass_s * 0.32 + kick_amt * snare_s * 0.22;
        let (hat_l, hat_r) = (hats_amt * hat_s * 0.16, hats_amt * hat_s * 0.11);
        let (pad_l, pad_r) = (pad_amt * pad_s * 0.09, pad_amt * pad_s * 0.08);
        frames.push([
            (centre + hat_l + pad_l) * fade,
            (centre + hat_r + pad_r) * fade,
        ]);
    }
    Audio::from_frames(SYNTH_RATE, frames)
}
