//! The parts of a streamed track that are not music: what SponsorBlock's viewers marked, and silence at either end.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

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

/// What viewers marked in the file at `track` as not the song, in seconds and in order.
pub fn marked(track: &Path) -> Vec<(f64, f64)> {
    let pair = |l: &str| {
        let (a, b) = l.split_once(' ')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    };
    std::fs::read_to_string(file(track)).map(|text| text.lines().filter_map(pair).collect()).unwrap_or_default()
}

/// When the second `at` of a song comes in a video of it with `parts` that aren't the song put in.
pub fn within(parts: &[(f64, f64)], at: f64) -> f64 {
    parts.iter().fold(at, |at, &(a, b)| if a <= at { at + (b - a) } else { at })
}

/// Asks what is marked in the video the file at `track` is of, once, and keeps it beside the file. Whether
/// anything was learnt; a failed ask is made again the next time.
pub fn ask(track: &Path) -> bool {
    let Some(video) = crate::lyrics::video(track).filter(|_| unasked(track)) else { return false };
    sugar_net::sponsorblock::segments(video).map(|parts| save(track, &parts)).is_ok()
}

/// Whether the file at `track` is of a video nobody has been asked about yet, and asking is on.
pub fn unasked(track: &Path) -> bool {
    ON.load(Ordering::Relaxed) && crate::lyrics::video(track).is_some() && !file(track).is_file()
}

static ON: AtomicBool = AtomicBool::new(true);

/// Sets whether SponsorBlock is asked at all: the DJ's switch.
pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// The parts of `track` to skip, in seconds and in order. Only tracks with marks kept beside them have any, so the DJ's
/// own files are never skipped through.
pub fn find(path: &Path, track: &Track) -> Vec<(f64, f64)> {
    if !file(path).is_file() {
        return Vec::new();
    }
    let mut parts = marked(path);
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
    fn a_song_second_is_found_in_its_video() {
        let parts = [(0.0, 12.0), (100.0, 130.0)];
        assert_eq!(within(&parts, 5.0), 17.0);
        assert_eq!(within(&parts, 87.9), 99.9);
        assert_eq!(within(&parts, 88.0), 130.0);
        assert_eq!(within(&[], 5.0), 5.0);
    }

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
