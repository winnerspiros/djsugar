//! Settings › Collection › Storage: how much of the disk holding
//! the music is the music, DJ Sugar's caches and everything else, drawn as
//! macOS draws its storage bar.
//!
//! The music's size takes a walk through its folders, so it runs on its
//! own thread and is kept for a few minutes; [`take_fresh`] says when a new
//! one is in.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a measured music size is used before it is measured again.
const KEEP: Duration = Duration::from_secs(300);

static MUSIC: Mutex<Option<(Instant, u64)>> = Mutex::new(None);
static MEASURING: AtomicBool = AtomicBool::new(false);
static FRESH: AtomicBool = AtomicBool::new(false);

/// The room the files in `folders` take on disk, as last measured; starts
/// measuring again when that is old or there is none yet.
pub fn music_bytes(folders: Vec<PathBuf>) -> Option<u64> {
    let last = *MUSIC.lock().ok()?;
    let stale = last.is_none_or(|(at, _)| at.elapsed() > KEEP);
    if stale && !MEASURING.swap(true, Ordering::AcqRel) {
        let spawned = std::thread::Builder::new()
            .name("storage".into())
            .spawn(move || {
                let mut seen = Vec::new();
                let total = folders
                    .iter()
                    .filter(|f| !folders.iter().any(|o| o != *f && f.starts_with(o)))
                    .map(|f| folder_bytes(f, &mut seen))
                    .sum();
                if let Ok(mut m) = MUSIC.lock() {
                    *m = Some((Instant::now(), total));
                }
                MEASURING.store(false, Ordering::Release);
                FRESH.store(true, Ordering::Release);
            });
        if spawned.is_err() {
            MEASURING.store(false, Ordering::Release);
        }
    }
    last.map(|(_, bytes)| bytes)
}

/// Forgets the measured music size, after a clean-up or a folder change.
pub fn forget() {
    if let Ok(mut m) = MUSIC.lock() {
        *m = None;
    }
}

/// Whether a new music size came in since the last call.
pub fn take_fresh() -> bool {
    FRESH.swap(false, Ordering::AcqRel)
}

/// The room the files under `dir` take. Files kept only in iCloud take
/// none, so they count as nothing.
fn folder_bytes(dir: &Path, seen: &mut Vec<PathBuf>) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if seen.contains(&d) {
            continue;
        }
        seen.push(d.clone());
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else if meta.is_file() {
                total += on_disk(&meta);
            }
        }
    }
    total
}

#[cfg(unix)]
fn on_disk(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn on_disk(meta: &std::fs::Metadata) -> u64 {
    meta.len()
}

/// The size and free room of the disk `path` is on.
pub fn disk(path: &Path) -> Option<(u64, u64)> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| (d.total_space(), d.available_space()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_counts_its_files_once() {
        let dir = std::env::temp_dir().join(format!("djsugar-storage-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::write(dir.join("a/one.mp3"), vec![1u8; 10_000]).unwrap();
        std::fs::write(dir.join("two.mp3"), vec![1u8; 10_000]).unwrap();
        let bytes = folder_bytes(&dir, &mut Vec::new());
        assert!(bytes >= 20_000, "{bytes}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
