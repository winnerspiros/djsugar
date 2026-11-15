//! The players' waveforms, made from DJ Sugar's own analysis.
//!
//! Every detail waveform has 150 columns a second. The previews are 400
//! (monochrome), 100 (tiny) and 1200 (colour and three-band) columns for the
//! whole track. DJ Sugar's analysis keeps band peaks and RMS 1000 times a
//! second, so each column gathers the slices it covers.
//!
//! How heights, whiteness, colours and the three bands are worked out
//! follows Bake'n Deck's `expressport` (MIT licence, Copyright (c) Masanari
//! Higashi), which fitted them to rekordbox's own files.

use djsugar_engine::track::{Waveform, WaveformBin};
use rbox_anlz::{
    TinyWaveformColumn, Waveform3BandCalibration, Waveform3BandColumn, Waveform3BandDetail,
    Waveform3BandPreview, WaveformColorDetail, WaveformColorDetailColumn, WaveformColorPreview,
    WaveformColorPreviewColumn, WaveformColumn, WaveformDetail, WaveformPreview,
};

const COLUMNS_PER_SECOND: f64 = 150.0;
const PREVIEW_COLUMNS: usize = 400;
const TINY_COLUMNS: usize = 100;
const COLOUR_COLUMNS: usize = 1200;

/// Per-column release of the band envelopes (low, mid, high).
const RELEASE: [f64; 3] = [0.97, 0.93, 0.90];
/// Gain rule per band: `clamp(round(target / max_envelope), floor, cap)`,
/// and the byte scale applied on top of the gain.
const GAIN_TARGET: [f64; 3] = [78.0, 105.0, 115.0];
const GAIN_FLOOR: [u16; 3] = [80, 80, 95];
const GAIN_CAP: [u16; 3] = [267, 300, 500];
const BAND_SCALE: [f64; 3] = [1.25, 1.15, 0.6];
/// The three-band preview is the column mean of the detail times these;
/// the colour preview's band bytes are the three-band preview times these.
const PWV6_SCALE: [f64; 3] = [0.56, 0.71, 2.0];
const PWV4_SCALE: [f64; 3] = [2.4, 1.6, 1.1];

/// One column: peaks of the whole signal and of the low, mid and high
/// bands, and the sum of squared RMS over its slices.
#[derive(Debug, Default, Clone, Copy)]
struct Column {
    peak: f64,
    band: [f64; 3],
    sumsq: f64,
    slices: u32,
}

/// A slice's value: bins keep the square root of the level, scaled to 255.
fn level(b: u8) -> f64 {
    let x = b as f64 / 255.0;
    x * x
}

fn columns(waveform: &Waveform, duration: f64) -> Vec<Column> {
    let bins: &[WaveformBin] = waveform.levels.first().map_or(&[], Vec::as_slice);
    let rate = waveform.bins_per_second.max(1.0);
    let n = ((duration * COLUMNS_PER_SECOND).ceil() as usize).max(1);
    (0..n)
        .map(|i| {
            let a = (i as f64 * rate / COLUMNS_PER_SECOND).round() as usize;
            let b = (((i + 1) as f64 * rate / COLUMNS_PER_SECOND).round() as usize).max(a + 1);
            let mut c = Column::default();
            for bin in bins
                .get(a.min(bins.len())..b.min(bins.len()))
                .unwrap_or(&[])
            {
                c.peak = c.peak.max(level(bin[3]));
                for (band, &b) in c.band.iter_mut().zip(bin) {
                    *band = band.max(level(b));
                }
                let rms = level(bin[7]);
                c.sumsq += rms * rms;
                c.slices += 1;
            }
            c
        })
        .collect()
}

/// `floor(31.5 * (peak / track peak)^2)`, rekordbox's height rule.
fn height(peak: f64, track_peak: f64) -> u8 {
    if track_peak <= 0.0 {
        return 0;
    }
    let q = peak / track_peak;
    ((31.5 * q * q).floor() as u8).min(31)
}

/// How white a column is drawn: the less of it is bass, the whiter.
fn whiteness(low: f64, peak: f64) -> u8 {
    if peak <= 0.0 {
        return 7;
    }
    let r = (low / peak).clamp(0.0, 1.0);
    (7 - ((8.0 * r).floor() as i32).min(7)) as u8
}

/// Splits `n` items into `parts` consecutive ranges, each at least one long.
fn ranges(n: usize, parts: usize) -> impl Iterator<Item = std::ops::Range<usize>> {
    (0..parts).map(move |k| {
        let a = (k * n / parts).min(n - 1);
        let b = ((k + 1) * n / parts).clamp(a + 1, n);
        a..b
    })
}

/// Every waveform section of one track.
pub struct Waveforms {
    /// `PWAV`, 400 columns.
    pub preview: WaveformPreview,
    /// `PWV2`, 100 columns.
    pub tiny: rbox_anlz::TinyWaveformPreview,
    /// `PWV3`, 150 a second.
    pub detail: WaveformDetail,
    /// `PWV5`, 150 a second.
    pub colour_detail: WaveformColorDetail,
    /// `PWV4`, 1200 columns.
    pub colour_preview: WaveformColorPreview,
    /// `PWV7`, 150 a second.
    pub band_detail: Waveform3BandDetail,
    /// `PWV6`, 1200 columns.
    pub band_preview: Waveform3BandPreview,
    /// `PWVC`.
    pub band_gains: Waveform3BandCalibration,
}

pub fn waveforms(waveform: &Waveform, duration: f64) -> Waveforms {
    let columns = columns(waveform, duration);
    let n = columns.len();
    let track_peak = columns.iter().map(|c| c.peak).fold(0.0, f64::max);

    let heights: Vec<u8> = columns.iter().map(|c| height(c.peak, track_peak)).collect();
    let whites: Vec<u8> = columns
        .iter()
        .map(|c| whiteness(c.band[0], c.peak))
        .collect();
    let detail: Vec<WaveformColumn> = heights
        .iter()
        .zip(&whites)
        .map(|(h, w)| WaveformColumn::from_bytes([(w << 5) | h]))
        .collect();

    // Band envelopes with release, then the gain that scales each band's
    // loudest column to its target.
    let mut env = vec![[0.0f64; 3]; n];
    for (i, c) in columns.iter().enumerate() {
        for j in 0..3 {
            let prev = if i == 0 {
                0.0
            } else {
                env[i - 1][j] * RELEASE[j]
            };
            env[i][j] = c.band[j].max(prev);
        }
    }
    let mut gains = [0u16; 3];
    for j in 0..3 {
        let max = env.iter().map(|e| e[j]).fold(0.0, f64::max);
        gains[j] = if max > 0.0 {
            ((GAIN_TARGET[j] / max).round() as u16).clamp(GAIN_FLOOR[j], GAIN_CAP[j])
        } else {
            GAIN_FLOOR[j]
        };
    }
    let bands: Vec<[u8; 3]> = env
        .iter()
        .map(|e| {
            let mut out = [0u8; 3];
            for j in 0..3 {
                out[j] = (BAND_SCALE[j] * gains[j] as f64 * e[j]).round().min(127.0) as u8;
            }
            out
        })
        .collect();

    // Colour: each band's share of the strongest, on a square root so faint
    // bands still tint the column. Stored as big-endian rrrgggbbbhhhhh00.
    let colour_detail: Vec<WaveformColorDetailColumn> = columns
        .iter()
        .zip(&heights)
        .map(|(c, &h)| {
            let max = c.band.iter().cloned().fold(0.0, f64::max);
            let share = |b: f64| -> u16 {
                if max <= 0.0 {
                    0
                } else {
                    (7.0 * (b / max).sqrt()).round().min(7.0) as u16
                }
            };
            let v = (share(c.band[0]) << 13)
                | (share(c.band[1]) << 10)
                | (share(c.band[2]) << 7)
                | ((h as u16) << 2);
            WaveformColorDetailColumn::from_bytes(v.to_be_bytes())
        })
        .collect();

    // Previews from the RMS of each group of columns.
    let rms_over = |r: std::ops::Range<usize>| -> f64 {
        let (sumsq, slices) = columns[r]
            .iter()
            .fold((0.0, 0u32), |(s, k), c| (s + c.sumsq, k + c.slices));
        if slices == 0 {
            0.0
        } else {
            (sumsq / slices as f64).sqrt()
        }
    };
    let rms400: Vec<f64> = ranges(n, PREVIEW_COLUMNS).map(rms_over).collect();
    let rms400_max = rms400.iter().cloned().fold(0.0, f64::max);
    let white200: Vec<u8> = ranges(n, PREVIEW_COLUMNS / 2)
        .map(|r| {
            let len = r.len() as f64;
            (whites[r].iter().map(|&w| w as f64).sum::<f64>() / len).round() as u8
        })
        .collect();
    let preview: Vec<WaveformColumn> = rms400
        .iter()
        .enumerate()
        .map(|(k, &rms)| {
            let h = if rms400_max > 0.0 {
                (24.0 * rms / rms400_max).round() as u8
            } else {
                0
            };
            WaveformColumn::from_bytes([(white200[k / 2].min(7) << 5) | h.min(31)])
        })
        .collect();
    let rms100: Vec<f64> = ranges(n, TINY_COLUMNS).map(rms_over).collect();
    let rms100_max = rms100.iter().cloned().fold(0.0, f64::max);
    let tiny: Vec<TinyWaveformColumn> = rms100
        .iter()
        .map(|&rms| {
            let h = if rms100_max > 0.0 {
                ((15.0 * (rms / rms100_max).sqrt()).round() as u8).min(15)
            } else {
                0
            };
            TinyWaveformColumn::from_bytes([h])
        })
        .collect();

    // Three-band and colour previews from the detail bytes.
    let band_preview: Vec<[u8; 3]> = ranges(n, COLOUR_COLUMNS)
        .map(|r| {
            let mut out = [0u8; 3];
            for j in 0..3 {
                let m = bands[r.clone()].iter().map(|b| b[j] as f64).sum::<f64>() / r.len() as f64;
                out[j] = (PWV6_SCALE[j] * m).round().min(127.0) as u8;
            }
            out
        })
        .collect();
    let colour_preview: Vec<WaveformColorPreviewColumn> = band_preview
        .iter()
        .map(|b6| {
            let mut b = [0u8; 6];
            for j in 0..3 {
                b[3 + j] = (PWV4_SCALE[j] * b6[j] as f64).round().min(127.0) as u8;
            }
            let (r, g, bl) = (b[3] as f64, b[4] as f64, b[5] as f64);
            let max = r.max(g).max(bl);
            if max > 0.0 {
                b[2] = (0.91 * (r * r + g * g + bl * bl).sqrt()).round().min(127.0) as u8;
                b[0] = (0.88 * max + 38.0).round().min(127.0) as u8;
                b[1] = (203.0 - 0.49 * b[0] as f64).round().clamp(0.0, 255.0) as u8;
            }
            WaveformColorPreviewColumn {
                u1: b[0],
                u2: b[1],
                energy_bottom_half_freq: b[2],
                energy_bottom_third_freq: b[3],
                energy_mid_third_freq: b[4],
                energy_top_third_freq: b[5],
            }
        })
        .collect();

    let three = |b: &[u8; 3]| Waveform3BandColumn::from_bytes(*b);
    Waveforms {
        preview: WaveformPreview::new(preview),
        tiny: rbox_anlz::TinyWaveformPreview::new(tiny),
        detail: WaveformDetail::new(detail),
        colour_detail: WaveformColorDetail::new(colour_detail),
        colour_preview: WaveformColorPreview::new(colour_preview),
        band_detail: Waveform3BandDetail::new(bands.iter().map(three).collect()),
        band_preview: Waveform3BandPreview::new(band_preview.iter().map(three).collect()),
        band_gains: Waveform3BandCalibration {
            low_gain: gains[0],
            mid_gain: gains[1],
            high_gain: gains[2],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A waveform `seconds` long whose every slice is `bin`.
    fn steady(seconds: f64, bin: WaveformBin) -> Waveform {
        let n = (seconds * 1000.0) as usize;
        Waveform::from_finest(1000.0, vec![bin; n])
    }

    #[test]
    fn column_counts() {
        let w = waveforms(&steady(2.0, [100, 100, 100, 200, 50, 50, 50, 120]), 2.0);
        assert_eq!(w.detail.data.len(), 300);
        assert_eq!(w.colour_detail.data.len(), 300);
        assert_eq!(w.band_detail.data.len(), 300);
        assert_eq!(w.preview.data.len(), 400);
        assert_eq!(w.tiny.data.len(), 100);
        assert_eq!(w.band_preview.data.len(), 1200);
        assert_eq!(w.colour_preview.data.len(), 1200);
    }

    #[test]
    fn loud_steady_signal_is_full_height() {
        let w = waveforms(&steady(1.0, [255, 60, 60, 255, 200, 50, 50, 200]), 1.0);
        let byte = w.detail.data[10].into_bytes()[0];
        assert_eq!(byte & 0x1f, 31);
        // all bass: not white
        assert_eq!(byte >> 5, 0);
    }

    #[test]
    fn silence_is_flat() {
        let w = waveforms(&steady(1.0, [0; 8]), 1.0);
        assert!(w.detail.data.iter().all(|c| c.into_bytes()[0] & 0x1f == 0));
        assert_eq!(w.band_gains.low_gain, GAIN_FLOOR[0]);
    }
}
