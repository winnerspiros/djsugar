//! The parts of a streamed track that are not music: what SponsorBlock's viewers marked, and silence at either end.

use std::path::{Path, PathBuf};

use sugar_engine::Track;

/// Silence shorter than this at a track's end is left alone.
const QUIET_SECONDS: f64 = 1.0;
/// Below this a frame counts as silent, about 50 dB down.
const QUIET: f32 = 0.003;

/// Where a track's marked parts are kept: beside it, under its own name.
pub fn file(track: &Path) -> PathBuf {
    track.with_extension("skip")
}

/// Keeps `parts` beside `track`, a line each; an empty file says the track was asked about and has none.
pub fn save(track: &Path, parts: &[(f64, f64)]) {
    let text: String = parts.iter().map(|(a, b)| format!("{a} {b}\n")).collect();
    let _ = std::fs::write(file(track), text);
}

/// The parts of `track` to skip, in seconds and in order. Only tracks with marks kept beside them have any, so the DJ's
/// own files are never skipped through.
pub fn find(path: &Path, track: &Track) -> Vec<(f64, f64)> {
    let Ok(text) = std::fs::read_to_string(file(path)) else { return Vec::new() };
    let pair = |l: &str| {
        let (a, b) = l.split_once(' ')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    };
    let mut parts: Vec<(f64, f64)> = text.lines().filter_map(pair).collect();
    let (rate, len) = (f64::from(track.rate.max(1)), track.frames.len());
    let loud = |f: &[f32; 2]| f[0].abs() > QUIET || f[1].abs() > QUIET;
    let first = track.frames.iter().position(loud).unwrap_or(len);
    let last = track.frames.iter().rposition(loud).map_or(0, |n| n + 1);
    if first as f64 / rate >= QUIET_SECONDS {
        parts.push((0.0, first as f64 / rate));
    }
    if last > first && (len - last) as f64 / rate >= QUIET_SECONDS {
        parts.push((last as f64 / rate, len as f64 / rate));
    }
    parts.sort_by(|a, b| a.0.total_cmp(&b.0));
    parts
}

/// Where to jump to from `at`, when it lies in a part to skip; parts that run into each other are crossed in one go.
pub fn past(parts: &[(f64, f64)], at: f64) -> Option<f64> {
    let mut to = None;
    for &(a, b) in parts {
        let here = to.unwrap_or(at);
        if here >= a && here < b {
            to = Some(b);
        }
    }
    to
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marked_parts_and_silent_ends_are_skipped() {
        let dir = std::env::temp_dir().join(format!("djsugar-skips-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("track.m4a");
        let mut frames = vec![[0.0f32; 2]; 1000];
        frames[200..700].fill([0.5, 0.5]);
        let track = Track::new(100, frames.into_boxed_slice());
        assert!(find(&path, &track).is_empty(), "a track nobody marked is left whole");
        save(&path, &[(3.0, 4.0), (4.0, 5.5)]);
        let parts = find(&path, &track);
        assert_eq!(parts, vec![(0.0, 2.0), (3.0, 4.0), (4.0, 5.5), (7.0, 10.0)]);
        assert_eq!(past(&parts, 0.5), Some(2.0));
        assert_eq!(past(&parts, 3.2), Some(5.5));
        assert_eq!(past(&parts, 2.5), None);
        assert_eq!(past(&parts, 5.5), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
