//! Where a track has vocals: a level along the track, from Spleeter's vocal network on its own
//! (see [`spleeter::vocal_levels`]), worked out once and kept in the cache beside the analysis.
//! The deck waveforms tint the sung parts with it (Settings › Appearance › Show vocals).

use std::fs;
use std::path::{Path, PathBuf};

use djsugar_separation::spleeter;

use crate::decode::decode;
use crate::separation::{channels, spleeter_net};
use crate::track::Audio;

/// Levels per second.
pub const RATE: f64 = spleeter::VOCAL_RATE;

/// Kept files hold each step's vocal share (0 to 255), not the levels
/// drawn, so the levels can be tuned without finding the vocals again.
/// Files from before (`ODJVOC01`, levels) are found again.
const MAGIC: &[u8; 8] = b"ODJVOC02";

/// How long a stretch the shares are averaged over, in seconds: a voice
/// holds for a while, while what leaks from hats, leads and pads comes and
/// goes.
const SMOOTH_SECONDS: f64 = 0.75;

/// A track's vocal level along it, [`RATE`] a second, 0 (none) to 255
/// (all vocals).
pub type Levels = Vec<u8>;

/// The vocal shares of decoded `audio` (0 to 255, see [`to_levels`]). Stops early, with an error, when
/// `keep_going` turns false.
fn detect(audio: &Audio, mut keep_going: impl FnMut() -> bool) -> Result<Vec<u8>, String> {
    let net = spleeter_net()?;
    let [l, r] = channels(audio);
    let model_rate = djsugar_separation::SAMPLE_RATE;
    let (l, r) = if audio.sample_rate == model_rate {
        (l, r)
    } else {
        (
            djsugar_separation::resample(&l, audio.sample_rate, model_rate)?,
            djsugar_separation::resample(&r, audio.sample_rate, model_rate)?,
        )
    };
    let mut stopped = false;
    let levels = spleeter::vocal_levels(&*net, [&l, &r], || {
        stopped |= !keep_going();
        !stopped
    });
    if stopped {
        return Err("stopped".into());
    }
    Ok(levels.iter().map(|s| (s.clamp(0.0, 1.0) * 255.0).round() as u8).collect())
}

/// The levels drawn from vocal shares (0..1, as amplitudes: the square root of the vocals' part of the power), `rate` a second. Averaged over
/// `SMOOTH_SECONDS` first; then under 0.4 (a sixth of the power) is what leaks from other instruments and reads as none, rising to full at
/// 0.75 (over half the power). A voice over a mix sits well above that; a lead synth or pad that leaks into the vocal network rarely holds it.
pub fn to_levels(shares: &[f32], rate: f64) -> Levels {
    let half = (SMOOTH_SECONDS * rate * 0.5).round() as usize;
    let mut sums = Vec::with_capacity(shares.len() + 1);
    sums.push(0.0f64);
    for s in shares {
        sums.push(sums.last().unwrap() + f64::from(*s));
    }
    (0..shares.len())
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + half + 1).min(shares.len());
            let s = (sums[b] - sums[a]) / (b - a) as f64;
            (((s - 0.4) / 0.35).clamp(0.0, 1.0) * 255.0).round() as u8
        })
        .collect()
}

fn shares_to_levels(shares: &[u8]) -> Levels {
    let s: Vec<f32> = shares.iter().map(|&v| f32::from(v) / 255.0).collect();
    to_levels(&s, RATE)
}

/// Where the vocal levels of `path` are kept, beside its analysis.
fn file(path: &Path) -> Option<PathBuf> {
    let entry = crate::cache::entry_path(path)?;
    Some(entry.parent()?.parent()?.join("vocals").join(entry.file_name()?))
}

/// The vocal levels of `path` from its kept shares, if it has them for the
/// file as it is.
pub fn cached(path: &Path) -> Option<Levels> {
    let data = fs::read(file(path)?).ok()?;
    let shares = data.strip_prefix(MAGIC)?;
    (!shares.is_empty()).then(|| shares_to_levels(shares))
}

fn store(path: &Path, levels: &[u8]) {
    let Some(file) = file(path) else { return };
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(file.parent().unwrap())?;
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        let mut data = MAGIC.to_vec();
        data.extend_from_slice(levels);
        fs::write(&tmp, &data)?;
        fs::rename(&tmp, &file)
    };
    if let Err(e) = write() {
        eprintln!("could not keep vocals {}: {e}", file.display());
    }
}

/// The vocal levels of the file at `path`: kept ones, or found now (which
/// decodes it and takes a few seconds) and kept. `audio` saves decoding
/// when the caller has it already.
pub fn analyse(path: &Path, audio: Option<&Audio>, keep_going: impl FnMut() -> bool) -> Result<Levels, String> {
    if let Some(l) = cached(path) {
        return Ok(l);
    }
    let decoded;
    let audio = match audio {
        Some(a) => a,
        None => {
            decoded = decode(path)?;
            &decoded
        }
    };
    let shares = detect(audio, keep_going)?;
    if !shares.is_empty() {
        store(path, &shares);
    }
    Ok(shares_to_levels(&shares))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leakage_reads_as_none_and_voices_as_full() {
        // No smoothing at one step a second.
        assert_eq!(to_levels(&[0.0, 0.3, 0.4, 0.75, 0.9], 1.0), vec![0, 0, 0, 255, 255]);
        let mid = to_levels(&[0.575], 1.0)[0];
        assert!((120..=135).contains(&mid), "{mid}");
    }

    #[test]
    fn short_bursts_are_smoothed_away() {
        // A held voice stays; a burst shorter than the smoothing doesn't.
        let rate = 40.0;
        let mut shares = vec![0.0f32; 400];
        shares[50..53].fill(0.95);
        shares[200..320].fill(0.9);
        let l = to_levels(&shares, rate);
        assert_eq!(l[51], 0);
        assert_eq!(l[260], 255);
        assert_eq!(l[380], 0);
    }
}
