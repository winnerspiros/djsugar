//! Keeps each file's analysis (waveform, tempo, first beat) on disk, so a track loaded again shows its waveform at once. Entries live in the user's
//! cache folder (`~/Library/Caches/DJSugar` on macOS, `%LOCALAPPDATA%\DJ Sugar` on Windows, `$XDG_CACHE_HOME/djsugar` or `~/.cache/djsugar` elsewhere,
//! or `DJSUGAR_CACHE`), named by a hash of the file's path, size and modification time, so an edited file is analysed again.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::UNIX_EPOCH;

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::analysis::Analysis;
use crate::beats::{BeatGrid, GridMarker};
use crate::key::{Detector, Key};
use crate::loudness::Loudness;
use crate::track::{Chroma, OverviewSlice, Waveform, WaveformBin};

/// Changes whenever the analysis or this format changes, so old entries are ignored.
const MAGIC: &[u8; 8] = b"MXRSAN07";

/// Starts each stored overview, followed by the analysis's own [`MAGIC`],
/// so overviews of an older analysis are made again too.
const OVERVIEW_MAGIC: &[u8; 8] = b"ODJOVW02";

/// Slices in a stored overview: the library's mini waveforms.
pub const OVERVIEW_SLICES: usize = 256;

/// Where tests keep their cache, instead of the user's.
#[cfg(test)]
pub(crate) static TEST_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The cache folder the app was given, where the system has no place of its own for one to be worked out.
static GIVEN: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Sets the cache folder: a phone app is told its own, and has no home folder to look under.
pub fn set_dir(dir: PathBuf) {
    let _ = GIVEN.set(dir);
}

/// DJ Sugar's cache folder (`DJSUGAR_CACHE` overrides it).
pub fn cache_dir() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(dir) = TEST_DIR.get() {
        return Some(dir.clone());
    }
    if let Some(dir) = GIVEN.get() {
        return Some(dir.clone());
    }
    if let Some(dir) = std::env::var_os("DJSUGAR_CACHE") {
        return Some(PathBuf::from(dir));
    }
    let base = crate::paths::cache_base()?;
    let name = crate::paths::app_folder_name();
    Some(base.join(name))
}

/// Where downloaded models are kept (see [`crate::separation::models_dir`]).
pub(crate) fn models_dir() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("models"))
}

/// The folder of stored analyses (waveforms, tempo and key per file).
pub fn analysis_dir() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("analysis"))
}

/// How many stored analyses there are and the bytes they take.
pub fn analysis_usage() -> (usize, u64) {
    let Some(entries) = analysis_dir().and_then(|d| fs::read_dir(d).ok()) else {
        return (0, 0);
    };
    entries
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .fold((0, 0), |(n, bytes), m| (n + 1, bytes + m.len()))
}

/// Deletes every stored analysis; tracks are analysed again when next
/// loaded. Separated stems are kept. Returns how many were deleted.
pub fn clear_analysis() -> usize {
    // Their overviews go too, so the library shows them as not analysed.
    if let Some(dir) = cache_dir() {
        let _ = fs::remove_dir_all(dir.join("overviews"));
    }
    let Some(entries) = analysis_dir().and_then(|d| fs::read_dir(d).ok()) else {
        return 0;
    };
    entries.flatten().filter(|e| fs::remove_file(e.path()).is_ok()).count()
}

/// Where `path`'s analysis is kept, or None if the file can't be read.
/// Where the analysis of `path` is kept, named for the file as it is now.
pub(crate) fn entry_path(path: &Path) -> Option<PathBuf> {
    entry(path)
}

fn entry(path: &Path) -> Option<PathBuf> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    // FNV-1a: stable across Rust versions, unlike the standard hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let path_bytes = path.as_os_str().as_encoded_bytes();
    for b in path_bytes.iter().chain(&meta.len().to_le_bytes()).chain(&modified.to_le_bytes()) {
        h = (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    Some(analysis_dir()?.join(format!("{h:016x}")))
}

/// Where `path`'s analysis is kept now, to hand to [`tags_rewritten`]
/// after rewriting its tags.
pub(crate) fn before_retag(path: &Path) -> Option<PathBuf> {
    entry(path)
}

/// Moves the analysis and separated stems of `path` to where they belong
/// now that its tags were rewritten (which changes its size and time), so
/// neither is worked out again. `old` is what [`before_retag`] gave.
pub(crate) fn tags_rewritten(path: &Path, old: Option<PathBuf>) {
    let (Some(old), Some(new)) = (old, entry(path)) else {
        return;
    };
    if old == new {
        return;
    }
    let _ = fs::rename(&old, &new);
    let stems =
        |entry: &Path| -> Option<PathBuf> { Some(entry.parent()?.parent()?.join("stems").join(entry.file_name()?)) };
    for (from, to) in [(stems(&old), stems(&new)), (overview_file(&old), overview_file(&new))] {
        if let (Some(from), Some(to)) = (from, to) {
            if from.exists() {
                let _ = fs::rename(from, to);
            }
        }
    }
}

/// Where `path`'s analysis is kept now, to hand to [`file_moved`] once the
/// file has moved.
pub fn before_move(path: &Path) -> Option<PathBuf> {
    entry(path)
}

/// Moves the analysis and stems of a file that moved to `path` (the cache
/// is keyed by path), so neither is worked out again. `old` is what
/// [`before_move`] gave for its old path.
pub fn file_moved(path: &Path, old: Option<PathBuf>) {
    tags_rewritten(path, old)
}

/// Where the stems separated from `path` are kept.
pub(crate) fn stems_dir(path: &Path) -> Option<PathBuf> {
    let analysis = entry(path)?;
    let name = analysis.file_name()?.to_owned();
    Some(analysis.parent()?.parent()?.join("stems").join(name))
}

/// The folder of high-quality stems, a folder per track.
pub fn stems_root() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("stems"))
}

/// The file whose time says when a track's stems were last used.
const LAST_USED: &str = "last-used";

/// Marks the stems in `dir` as just used, so the cache limit keeps them
/// over ones not loaded for longer.
pub(crate) fn touch_stems(dir: &Path) {
    let _ = fs::write(dir.join(LAST_USED), b"");
}

/// A track's stems folder in `root`: its size and when it was last used.
struct StemsEntry {
    path: PathBuf,
    bytes: u64,
    used: std::time::SystemTime,
}

/// The finished stems folders in `root` (not ones still being written).
fn stems_entries(root: &Path) -> Vec<StemsEntry> {
    let Ok(dirs) = fs::read_dir(root) else {
        return Vec::new();
    };
    dirs.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().contains(".work"))
        .map(|e| {
            let path = e.path();
            let (mut bytes, mut used) = (0, std::time::SystemTime::UNIX_EPOCH);
            for f in fs::read_dir(&path).into_iter().flatten().flatten() {
                if let Ok(m) = f.metadata() {
                    bytes += m.len();
                    if let Ok(t) = m.modified() {
                        used = used.max(t);
                    }
                }
            }
            StemsEntry { path, bytes, used }
        })
        .collect()
}

/// How many tracks have high-quality stems kept and the bytes they take.
pub fn stems_usage() -> (usize, u64) {
    let entries = stems_root().map(|r| stems_entries(&r)).unwrap_or_default();
    (entries.len(), entries.iter().map(|e| e.bytes).sum())
}

/// Deletes every kept stem folder; returns how many.
pub fn clear_stems() -> usize {
    let entries = stems_root().map(|r| stems_entries(&r)).unwrap_or_default();
    entries.iter().filter(|e| fs::remove_dir_all(&e.path).is_ok()).count()
}

/// Deletes the stems used longest ago until the rest take at most
/// `limit` bytes, never the ones of `keep` (the track just separated or
/// playing). Returns how many were deleted.
pub fn trim_stems(limit: u64, keep: &[PathBuf]) -> usize {
    let Some(root) = stems_root() else { return 0 };
    let keep: Vec<PathBuf> = keep.iter().filter_map(|p| stems_dir(p)).collect();
    trim_stems_in(&root, limit, &keep)
}

fn trim_stems_in(root: &Path, limit: u64, keep: &[PathBuf]) -> usize {
    let mut entries = stems_entries(root);
    let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
    entries.sort_by_key(|e| e.used);
    let mut deleted = 0;
    for e in entries {
        if total <= limit {
            break;
        }
        if keep.contains(&e.path) {
            continue;
        }
        if fs::remove_dir_all(&e.path).is_ok() {
            total = total.saturating_sub(e.bytes);
            deleted += 1;
        }
    }
    deleted
}

/// Where the overview of the analysis kept at `entry` is kept: a small
/// file beside the analyses, so the library can draw a whole list of
/// mini waveforms without reading every track's full waveform.
fn overview_file(entry: &Path) -> Option<PathBuf> {
    Some(entry.parent()?.parent()?.join("overviews").join(entry.file_name()?))
}

/// The overview of `path` (see [`Waveform::overview`]), if it has been
/// analysed as the file is now. Made from the stored analysis and kept
/// the first time it is asked for, so this may take a moment once.
pub fn overview(path: &Path) -> Option<Vec<OverviewSlice>> {
    let file = overview_file(&entry(path)?)?;
    if let Some(o) = fs::read(&file).ok().and_then(|d| decode_overview(&d)) {
        return Some(o);
    }
    let o = load(path)?.waveform.overview(OVERVIEW_SLICES);
    store_overview(&file, &o);
    Some(o)
}

fn store_overview(file: &Path, o: &[OverviewSlice]) {
    if o.is_empty() {
        return;
    }
    let mut data = Vec::with_capacity(16 + o.len() * 4);
    data.extend_from_slice(OVERVIEW_MAGIC);
    data.extend_from_slice(MAGIC);
    data.extend(o.iter().flatten());
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(file.parent().unwrap())?;
        replace(file, &data, |d| decode_overview(d).is_some())
    };
    if let Err(e) = write() {
        eprintln!("could not keep overview {}: {e}", file.display());
    }
}

/// Writes `data` aside and renames it over `file`, so a reader never sees half of it. Where
/// the rename fails because the file is in use (Windows, or a scanner holding it) and what's
/// there already passes `valid`, another writer stored the same thing first and that stands.
fn replace(file: &Path, data: &[u8], valid: impl Fn(&[u8]) -> bool) -> std::io::Result<()> {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = file.with_extension(format!("tmp{}-{n}", std::process::id()));
    fs::write(&tmp, data)?;
    let moved = fs::rename(&tmp, file);
    if moved.is_err() {
        let _ = fs::remove_file(&tmp);
        if fs::read(file).is_ok_and(|d| valid(&d)) {
            return Ok(());
        }
    }
    moved
}

fn decode_overview(data: &[u8]) -> Option<Vec<OverviewSlice>> {
    let rest = data.strip_prefix(OVERVIEW_MAGIC)?.strip_prefix(MAGIC)?;
    let (slices, tail) = rest.as_chunks::<4>();
    (tail.is_empty() && !slices.is_empty()).then(|| slices.to_vec())
}

/// The stored analysis of `path`, if it has one for the file as it is now.
pub fn load(path: &Path) -> Option<Analysis> {
    let file = fs::File::open(entry(path)?).ok()?;
    let mut data = Vec::new();
    ZlibDecoder::new(file).read_to_end(&mut data).ok()?;
    decode(&data)
}

/// Stores `analysis` of `path`. Failures are ignored: the cache only saves time.
pub fn store(path: &Path, analysis: &Analysis) {
    let Some(entry) = entry(path) else { return };
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(entry.parent().unwrap())?;
        // Write aside and rename, so a reader never sees half an entry.
        let mut z = ZlibEncoder::new(Vec::new(), Compression::fast());
        z.write_all(&encode(analysis))?;
        replace(&entry, &z.finish()?, |d| {
            let mut out = Vec::new();
            ZlibDecoder::new(d).read_to_end(&mut out).is_ok() && decode(&out).is_some()
        })
    };
    if let Err(e) = write() {
        eprintln!("could not cache analysis of {}: {e}", path.display());
        return;
    }
    if let Some(file) = overview_file(&entry) {
        store_overview(&file, &analysis.waveform.overview(OVERVIEW_SLICES));
    }
}

fn encode(a: &Analysis) -> Vec<u8> {
    let finest = a.waveform.levels.first().map_or(&[][..], |l| &l[..]);
    let mut out = Vec::with_capacity(40 + finest.len() * 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&a.bpm.to_le_bytes());
    out.extend_from_slice(&a.first_beat.to_le_bytes());
    out.extend_from_slice(&a.waveform.bins_per_second.to_le_bytes());
    // The key's index (0xff for none), and above it the detector's number;
    // entries from before there was a choice of detector say u64::MAX.
    let key = a.key.map_or(0xff, |k| k.index() as u64) | (a.key_detector.id() as u64) << 8;
    out.extend_from_slice(&key.to_le_bytes());
    let (lufs, peak) = a.loudness.map_or((f64::NAN, f64::NAN), |l| (l.lufs as f64, l.peak as f64));
    out.extend_from_slice(&lufs.to_le_bytes());
    out.extend_from_slice(&peak.to_le_bytes());
    let markers = a.grid.markers();
    out.extend_from_slice(&(markers.len() as u64).to_le_bytes());
    for m in markers {
        out.extend_from_slice(&m.seconds.to_le_bytes());
        out.extend_from_slice(&m.bpm.to_le_bytes());
    }
    // Grid flags: unsure, then from Beat This!.
    let flags = u64::from(a.grid_unsure) | u64::from(a.neural_beats) << 1;
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(finest.len() as u64).to_le_bytes());
    for b in finest {
        out.extend_from_slice(b);
    }
    // The chroma comes last, so entries from before it was kept still load
    // (without one).
    let chroma = &a.waveform.chroma;
    out.extend_from_slice(&chroma.per_second.to_le_bytes());
    out.extend_from_slice(&(chroma.steps.len() as u64).to_le_bytes());
    for s in &chroma.steps {
        out.extend_from_slice(s);
    }
    out
}

fn decode(data: &[u8]) -> Option<Analysis> {
    let (magic, rest) = data.split_first_chunk::<8>()?;
    if magic != MAGIC {
        return None;
    }
    let mut rest = rest;
    let mut num = || -> Option<[u8; 8]> {
        let (n, r) = rest.split_first_chunk::<8>()?;
        rest = r;
        Some(*n)
    };
    let bpm = f64::from_le_bytes(num()?);
    let first_beat = f64::from_le_bytes(num()?);
    let bins_per_second = f64::from_le_bytes(num()?);
    let (key, key_detector) = match u64::from_le_bytes(num()?) {
        u64::MAX => (None, Detector::Standard),
        v => (Key::from_index((v & 0xff) as u8), Detector::from_id((v >> 8) as u8)?),
    };
    let lufs = f64::from_le_bytes(num()?);
    let peak = f64::from_le_bytes(num()?);
    let loudness = (lufs.is_finite() && peak.is_finite()).then_some(Loudness { lufs: lufs as f32, peak: peak as f32 });
    let markers = u64::from_le_bytes(num()?) as usize;
    let mut grid = Vec::with_capacity(markers.min(4096));
    for _ in 0..markers {
        grid.push(GridMarker { seconds: f64::from_le_bytes(num()?), bpm: f64::from_le_bytes(num()?) });
    }
    let flags = u64::from_le_bytes(num()?);
    let len = u64::from_le_bytes(num()?) as usize;
    let bytes = len.checked_mul(8).filter(|&b| b > 0 && b <= rest.len())?;
    let (bins, after) = rest.split_at(bytes);
    let finest: Vec<WaveformBin> = bins.as_chunks::<8>().0.to_vec();
    let mut waveform = Waveform::from_finest(bins_per_second, finest);
    waveform.chroma = decode_chroma(after).unwrap_or_default();
    Some(Analysis {
        waveform,
        key,
        key_detector,
        bpm,
        first_beat,
        grid: BeatGrid::new(grid),
        grid_unsure: flags & 1 != 0,
        neural_beats: flags & 2 != 0,
        loudness,
    })
}

/// The chroma stored after the waveform; None for entries from before.
fn decode_chroma(data: &[u8]) -> Option<Chroma> {
    let (rate, rest) = data.split_first_chunk::<8>()?;
    let (len, rest) = rest.split_first_chunk::<8>()?;
    let len = u64::from_le_bytes(*len) as usize;
    let (steps, _) = rest.as_chunks::<12>();
    (steps.len() == len).then(|| Chroma { per_second: f64::from_le_bytes(*rate), steps: steps.to_vec() })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_stems_used_longest_ago_go_first() {
        let root = std::env::temp_dir().join(format!("djsugar-stems-trim-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for (i, name) in ["a", "b", "c", "d.work1"].iter().enumerate() {
            let dir = root.join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("drums.flac"), vec![0u8; 1000]).unwrap();
            let f = fs::File::options().write(true).open(dir.join("drums.flac")).unwrap();
            // a is the oldest, c the newest.
            f.set_modified(old + std::time::Duration::from_secs(60 * i as u64)).unwrap();
        }
        // "a" is kept (playing), so "b" goes, then "c" isn't needed.
        let deleted = trim_stems_in(&root, 2000, &[root.join("a")]);
        assert_eq!(deleted, 1);
        assert!(root.join("a").exists() && root.join("c").exists());
        assert!(!root.join("b").exists());
        // Folders still being written are never counted or deleted.
        assert!(root.join("d.work1").exists());
        // Touching "a" makes it the newest: now "c" goes first.
        touch_stems(&root.join("a"));
        assert_eq!(trim_stems_in(&root, 1500, &[]), 1);
        assert!(root.join("a").exists() && !root.join("c").exists());
        let _ = fs::remove_dir_all(&root);
    }

    use super::*;
    use crate::analysis::analyse;
    use crate::synth;

    #[test]
    fn round_trips_and_notices_changed_files() {
        let dir = std::env::temp_dir().join(format!("djsugar-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let track = dir.join("track.wav");
        fs::write(&track, b"first version").unwrap();
        let mut a = analyse(&synth::render_with(124.0, 0.2, 20.0, 1), None);

        let data = encode(&a);
        let b = decode(&data).unwrap();
        assert_eq!((b.bpm, b.first_beat, b.key), (a.bpm, a.first_beat, a.key));
        assert!(a.loudness.is_some());
        assert_eq!(b.loudness, a.loudness);
        assert_eq!(b.waveform.levels, a.waveform.levels);
        assert_eq!(b.grid, a.grid);
        assert!(!a.waveform.chroma.steps.is_empty());
        assert_eq!(b.waveform.chroma, a.waveform.chroma);
        // Cut short inside the waveform: nothing; without the chroma (an
        // entry from before it was kept): the rest, without one.
        let chroma_bytes = 16 + 12 * a.waveform.chroma.steps.len();
        assert!(decode(&data[..data.len() - chroma_bytes - 3]).is_none());
        let old = decode(&data[..data.len() - chroma_bytes]).unwrap();
        assert_eq!(old.waveform.levels, a.waveform.levels);
        assert!(old.waveform.chroma.steps.is_empty());

        // Which detector found the key, and keys from before there was a choice.
        a.key_detector = Detector::Neural;
        let b = decode(&encode(&a)).unwrap();
        assert_eq!((b.key, b.key_detector), (a.key, Detector::Neural));
        let mut old = encode(&a);
        old[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        let b = decode(&old).unwrap();
        assert_eq!((b.key, b.key_detector), (None, Detector::Standard));

        // Its overview: peaks and RMS across the track, kept small.
        let o = a.waveform.overview(OVERVIEW_SLICES);
        assert_eq!(o.len(), OVERVIEW_SLICES);
        assert!(o.iter().all(|[peak, rms, mid, high]| rms <= peak && *mid as u16 + *high as u16 <= 256));
        let mut kept = OVERVIEW_MAGIC.to_vec();
        kept.extend_from_slice(MAGIC);
        kept.extend(o.iter().flatten());
        assert_eq!(decode_overview(&kept).unwrap(), o);
        assert!(decode_overview(&kept[..kept.len() - 1]).is_none());
        assert!(Waveform::default().overview(8).is_empty());

        let first = entry(&track).unwrap();
        fs::write(&track, b"second, longer version").unwrap();
        assert_ne!(entry(&track).unwrap(), first);
        fs::remove_dir_all(&dir).unwrap();
    }
}
