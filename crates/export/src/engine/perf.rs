//! A track's `PerformanceData` for Engine DJ: the beat grid, the overview
//! waveform, hot cues, saved loops and the track data (sample rate, length,
//! key and band peaks), made from DJ Sugar's own analysis.
//!
//! The byte layouts follow EngineX-OS's `pkg/enginedb/performance.go`
//! (<https://github.com/Cd1s/EngineX-OS>, MIT licence, Copyright (c) 2026
//! EngineX OS Contributors), which was measured against Engine OS 5 on
//! players, and libdjinterop's notes on the same blobs. Every blob but the
//! loops is "qCompressed": a big-endian length, then zlib.

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use djsugar_engine::key::Key;
use djsugar_engine::track::{Hotcue, Waveform};
use djsugar_engine::BeatGrid;

/// Engine has eight hot cues and eight saved loops.
pub const SLOTS: usize = 8;
/// Entries in the overview waveform, over the whole track.
pub const OVERVIEW_ENTRIES: usize = 1024;

/// The blobs of one `PerformanceData` row.
#[derive(Debug, Clone, Default)]
pub struct Performance {
    pub track_data: Vec<u8>,
    pub overview: Vec<u8>,
    pub beats: Option<Vec<u8>>,
    pub quick_cues: Vec<u8>,
    pub loops: Vec<u8>,
    /// A grid and a waveform are there, so Engine needn't analyse it.
    pub complete: bool,
}

/// What a track's performance data is made from.
pub struct Input<'a> {
    pub sample_rate: f64,
    /// Seconds.
    pub duration: f64,
    pub grid: &'a BeatGrid,
    pub waveform: &'a Waveform,
    pub cues: &'a [Hotcue],
    pub key: Option<Key>,
}

pub fn build(input: &Input) -> Performance {
    let sr = if input.sample_rate > 0.0 {
        input.sample_rate
    } else {
        44100.0
    };
    let samples = (input.duration.max(0.0) * sr).round();
    let beats = beat_data(input.grid, sr, samples, input.duration);
    let (overview, top) = overview(input.waveform, input.duration, samples);
    let peaks = band_peaks(top);
    Performance {
        track_data: q_compress(&track_data(sr, samples as u64, input.key, peaks)),
        complete: beats.is_some() && top != [0; 3],
        beats: beats.map(|b| q_compress(&b)),
        overview: q_compress(&overview),
        quick_cues: q_compress(&quick_cues(input.cues, sr)),
        loops: loops(input.cues, sr),
    }
}

/// Qt's `qCompress`: the length as a big-endian u32, then zlib.
pub fn q_compress(raw: &[u8]) -> Vec<u8> {
    let mut out = (raw.len() as u32).to_be_bytes().to_vec();
    let mut z = ZlibEncoder::new(out, Compression::default());
    z.write_all(raw).expect("writing to memory");
    out = z.finish().expect("writing to memory");
    out
}

/// Engine's key number: C is 0, A minor 1, G 2, E minor 3 and so on round
/// the circle of fifths, each major followed by its relative minor.
pub fn key_number(key: Key) -> u32 {
    let fifths = (key.camelot_number() as u32 + 4) % 12;
    fifths * 2 + key.minor as u32
}

/// 68 bytes: sample rate, sample count, key, the low, mid and high peaks
/// (0..1) and three stem peaks, which Engine fills itself while they're 0.
fn track_data(sr: f64, samples: u64, key: Option<Key>, peaks: [f64; 3]) -> Vec<u8> {
    let mut b = Vec::with_capacity(68);
    b.extend(sr.to_be_bytes());
    b.extend(samples.to_be_bytes());
    b.extend(key.map_or(0, key_number).to_be_bytes());
    for p in peaks {
        b.extend(p.to_be_bytes());
    }
    for _ in 0..3 {
        b.extend(0f64.to_be_bytes());
    }
    b
}

/// A grid marker: where a beat is, its number (a downbeat every fourth,
/// from 0), and how many beats on the next marker is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    pub sample: f64,
    pub beat: i64,
    pub beats_to_next: i32,
}

/// The grid as Engine's markers: the first beat in the track, a marker at
/// each tempo change, and the last beat, so Engine counts evenly between.
pub fn markers(grid: &BeatGrid, sr: f64, duration: f64) -> Vec<Marker> {
    if grid.is_empty() || duration <= 0.0 {
        return Vec::new();
    }
    let first = grid.beat_at(0.0).ceil() as i64;
    let mut last = grid.beat_at(duration).floor() as i64;
    if grid.seconds_at(last as f64) >= duration {
        last -= 1;
    }
    if last <= first {
        return Vec::new();
    }
    let mut at: Vec<i64> = vec![first, last];
    // The grid's first marker is where it starts, not a tempo change.
    for m in grid.markers().iter().skip(1) {
        let b = grid.beat_at(m.seconds).round() as i64;
        if b > first && b < last {
            at.push(b);
        }
    }
    at.sort_unstable();
    at.dedup();
    // DJ Sugar counts bars in fours from beat 0, as Engine does.
    at.iter()
        .enumerate()
        .map(|(i, &b)| Marker {
            sample: grid.seconds_at(b as f64) * sr,
            beat: b,
            beats_to_next: at.get(i + 1).map_or(0, |n| (n - b) as i32),
        })
        .collect()
}

/// Sample rate, sample count, "grid set", then the default and the adjusted
/// grid (the same here), then a closing zero.
fn beat_data(grid: &BeatGrid, sr: f64, samples: f64, duration: f64) -> Option<Vec<u8>> {
    let markers = markers(grid, sr, duration);
    if markers.len() < 2 {
        return None;
    }
    let mut b = Vec::new();
    b.extend(sr.to_be_bytes());
    b.extend(samples.to_be_bytes());
    b.push(1);
    for _ in 0..2 {
        b.extend((markers.len() as u64).to_be_bytes());
        for m in &markers {
            b.extend(m.sample.to_le_bytes());
            b.extend(m.beat.to_le_bytes());
            b.extend(m.beats_to_next.to_le_bytes());
            b.extend(0i32.to_le_bytes());
        }
    }
    b.push(0);
    Some(b)
}

/// What the loudest entry of each band comes to. Engine's own analysis
/// tops out near these (EngineX-OS measured them on Engine-analysed drives).
const OVERVIEW_TOP: [f64; 3] = [103.0, 114.0, 112.0];
/// Track data's band peak per unit of the overview's band maximum.
const PEAK_PER_BYTE: [f64; 3] = [0.0066, 0.0035, 0.0016];

/// The bins keep the square root of the level, scaled to 255.
fn level(b: u8) -> f64 {
    let x = b as f64 / 255.0;
    x * x
}

/// 1024 low, mid and high entries over the whole track, each band scaled
/// to its loudest, then the maximum of each band. Returns the maxima too.
fn overview(waveform: &Waveform, duration: f64, samples: f64) -> (Vec<u8>, [u8; 3]) {
    let bins = waveform.levels.first().map_or(&[][..], Vec::as_slice);
    let rate = waveform.bins_per_second.max(1.0);
    let n = OVERVIEW_ENTRIES;
    let covered = (duration * rate).round().max(0.0) as usize;
    let covered = covered.min(bins.len());
    let mut raw = vec![[0f64; 3]; n];
    for (i, entry) in raw.iter_mut().enumerate() {
        let a = i * covered / n;
        let b = ((i + 1) * covered / n).max(a + 1).min(covered);
        for bin in bins.get(a..b).unwrap_or(&[]) {
            for (band, &v) in entry.iter_mut().zip(bin) {
                *band = band.max(level(v));
            }
        }
    }
    let mut loudest = [0f64; 3];
    for e in &raw {
        for c in 0..3 {
            loudest[c] = loudest[c].max(e[c]);
        }
    }
    let mut out = Vec::with_capacity(24 + n * 3 + 3);
    out.extend((n as u64).to_be_bytes());
    out.extend((n as u64).to_be_bytes());
    out.extend((samples / n as f64).to_be_bytes());
    let mut top = [0u8; 3];
    for e in &raw {
        for c in 0..3 {
            let v = if loudest[c] > 0.0 {
                (e[c] / loudest[c] * OVERVIEW_TOP[c]).round() as u8
            } else {
                0
            };
            top[c] = top[c].max(v);
            out.push(v);
        }
    }
    out.extend(top);
    (out, top)
}

fn band_peaks(top: [u8; 3]) -> [f64; 3] {
    let mut p = [0.68, 0.40, 0.18];
    for c in 0..3 {
        if top[c] > 0 {
            p[c] = (top[c] as f64 * PEAK_PER_BYTE[c]).min(1.0);
        }
    }
    p
}

/// A label: one byte of length, then UTF-8, cut to 255 bytes.
fn label(b: &mut Vec<u8>, s: &str) {
    let mut end = s.len().min(255);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    b.push(end as u8);
    b.extend(&s.as_bytes()[..end]);
}

/// ARGB, opaque. Cues without a colour get green, loops orange.
fn argb(c: &Hotcue) -> [u8; 4] {
    match c.color {
        Some(rgb) => [255, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8],
        None if c.end.is_some() => [255, 255, 127, 0],
        None => [255, 0, 255, 0],
    }
}

/// Hot cues by pad, A to H.
fn hot_slots(cues: &[Hotcue]) -> [Option<&Hotcue>; SLOTS] {
    let mut hot = [None; SLOTS];
    for c in cues {
        if !c.is_memory() && c.number < SLOTS {
            hot[c.number] = Some(c);
        }
    }
    hot
}

/// Memory cues by time; the first is the main cue.
fn memory(cues: &[Hotcue]) -> Vec<&Hotcue> {
    let mut m: Vec<&Hotcue> = cues.iter().filter(|c| c.is_memory()).collect();
    m.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    m
}

/// Eight hot cues (label, sample or -1, colour), then the main cue twice
/// (as adjusted, a zero, as found).
fn quick_cues(cues: &[Hotcue], sr: f64) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend((SLOTS as u64).to_be_bytes());
    for c in hot_slots(cues) {
        match c {
            Some(c) => {
                label(&mut b, &c.name);
                b.extend((c.seconds * sr).to_be_bytes());
                b.extend(argb(c));
            }
            None => {
                label(&mut b, "");
                b.extend((-1f64).to_be_bytes());
                b.extend([0; 4]);
            }
        }
    }
    let main = memory(cues).first().map_or(-1.0, |c| c.seconds * sr);
    b.extend(main.to_be_bytes());
    b.push(0);
    b.extend(main.to_be_bytes());
    b
}

/// Eight saved loops: a hot cue with an end keeps its pad's slot, and
/// memory loops fill the slots left. Not compressed.
fn loops(cues: &[Hotcue], sr: f64) -> Vec<u8> {
    let mut slots: [Option<&Hotcue>; SLOTS] =
        hot_slots(cues).map(|c| c.filter(|c| c.end.is_some()));
    for c in memory(cues).into_iter().filter(|c| c.end.is_some()) {
        if let Some(free) = slots.iter_mut().find(|s| s.is_none()) {
            *free = Some(c);
        }
    }
    let mut b = Vec::new();
    b.extend((SLOTS as u64).to_le_bytes());
    for c in slots {
        match c {
            Some(c) => {
                label(&mut b, &c.name);
                b.extend((c.seconds * sr).to_le_bytes());
                b.extend((c.end.unwrap_or(c.seconds) * sr).to_le_bytes());
                b.extend([1, 1]);
                b.extend(argb(c));
            }
            None => {
                label(&mut b, "");
                b.extend((-1f64).to_le_bytes());
                b.extend((-1f64).to_le_bytes());
                b.extend([0; 6]);
            }
        }
    }
    b
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    pub fn unq(blob: &[u8]) -> Vec<u8> {
        let n = u32::from_be_bytes(blob[..4].try_into().unwrap()) as usize;
        let mut out = Vec::new();
        ZlibDecoder::new(&blob[4..]).read_to_end(&mut out).unwrap();
        assert_eq!(out.len(), n);
        out
    }

    fn cue(number: usize, seconds: f64, end: Option<f64>) -> Hotcue {
        Hotcue {
            number,
            seconds,
            name: format!("c{number}"),
            end,
            color: Some(0x112233),
        }
    }

    #[test]
    fn keys_go_round_the_circle_of_fifths() {
        let k = |s: &str| key_number(Key::parse(s).unwrap());
        assert_eq!(k("C"), 0);
        assert_eq!(k("Am"), 1);
        assert_eq!(k("G"), 2);
        assert_eq!(k("Em"), 3);
        assert_eq!(k("B"), 10);
        assert_eq!(k("F"), 22);
        assert_eq!(k("Dm"), 23);
    }

    #[test]
    fn a_constant_grid_is_two_markers_from_bar_one() {
        // 120 BPM, bar 1 at 1.0 s: beats every 0.5 s from 0.0.
        let grid = BeatGrid::constant(120.0, 1.0);
        let m = markers(&grid, 44100.0, 10.0);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].sample, 0.0);
        assert_eq!(m[0].beat, -2, "two beats before bar 1");
        assert_eq!(m[0].beats_to_next, 19);
        assert!((m[1].sample - 9.5 * 44100.0).abs() < 1e-6);
        assert_eq!(m[1].beat, 17);
        assert_eq!(m[1].beats_to_next, 0);
    }

    #[test]
    fn a_tempo_change_gets_a_marker() {
        let grid = BeatGrid::constant(120.0, 0.0).with_marker(4.0, 128.0);
        let m = markers(&grid, 1000.0, 10.0);
        assert_eq!(m.len(), 3);
        assert_eq!(m[1].beat, 8);
        assert!((m[1].sample - 4000.0).abs() < 1e-6);
        assert_eq!(m[0].beats_to_next, 8);
    }

    #[test]
    fn beat_data_lays_out_both_grids() {
        let grid = BeatGrid::constant(120.0, 0.0);
        let raw = beat_data(&grid, 44100.0, 441000.0, 10.0).unwrap();
        // 8 + 8 + 1, then two grids of 8 + 2 * 24, then the closing zero.
        assert_eq!(raw.len(), 17 + 2 * (8 + 2 * 24) + 1);
        assert_eq!(f64::from_be_bytes(raw[..8].try_into().unwrap()), 44100.0);
        assert_eq!(u64::from_be_bytes(raw[17..25].try_into().unwrap()), 2);
    }

    #[test]
    fn cues_and_loops_fill_their_slots() {
        let cues = vec![
            cue(0, 1.0, None),
            cue(2, 2.0, Some(3.0)),
            cue(9, 4.0, None),
            cue(1000, 0.5, None),
            cue(1001, 5.0, Some(6.0)),
        ];
        let q = quick_cues(&cues, 100.0);
        assert_eq!(u64::from_be_bytes(q[..8].try_into().unwrap()), 8);
        // Slot A: "c0", 100 samples, 0x112233.
        assert_eq!(&q[8..11], b"\x02c0");
        assert_eq!(f64::from_be_bytes(q[11..19].try_into().unwrap()), 100.0);
        assert_eq!(&q[19..23], &[255, 0x11, 0x22, 0x33]);
        // Slot B is empty.
        assert_eq!(q[23], 0);
        assert_eq!(f64::from_be_bytes(q[24..32].try_into().unwrap()), -1.0);
        // The main cue is the first memory cue.
        let main = f64::from_be_bytes(q[q.len() - 8..].try_into().unwrap());
        assert_eq!(main, 50.0);

        let l = loops(&cues, 100.0);
        assert_eq!(u64::from_le_bytes(l[..8].try_into().unwrap()), 8);
        // Pad C keeps its loop; the memory loop takes the first free slot, A.
        let a = 8;
        assert_eq!(&l[a..a + 6], b"\x05c1001");
        let start = f64::from_le_bytes(l[a + 6..a + 14].try_into().unwrap());
        assert_eq!(start, 500.0);
        let b = a + 6 + 16 + 6;
        assert_eq!(l[b], 0, "slot B is empty");
        let c = b + 1 + 16 + 6;
        assert_eq!(&l[c..c + 3], b"\x02c2");
    }

    #[test]
    fn overview_spans_the_track() {
        let wf = Waveform::from_finest(100.0, vec![[200, 100, 50, 200, 0, 0, 0, 100]; 1000]);
        let (raw, top) = overview(&wf, 10.0, 441000.0);
        assert_eq!(raw.len(), 24 + 1024 * 3 + 3);
        assert_eq!(top, [103, 114, 112]);
        let p = Performance {
            track_data: q_compress(&track_data(44100.0, 441000, None, band_peaks(top))),
            ..Performance::default()
        };
        let td = unq(&p.track_data);
        assert_eq!(td.len(), 68);
    }
}
