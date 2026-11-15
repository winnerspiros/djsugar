//! Looks for album art and the bitrate in the collection's files on a thread
//! of its own, one file at a time, for Collection Health's "No cover" and
//! "Low bitrate" checks. Each answer is
//! kept in the library with the file's modification time, so a file is
//! only looked at again once it changes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::UNIX_EPOCH;

use djsugar_engine::{cover, tags};
use djsugar_library::health::FileLook;

/// A file to look at: the track's id, its path and album.
type Request = (usize, PathBuf, String);

pub struct CoverScan {
    requests: Sender<Request>,
    /// Id and what was found.
    done: Receiver<(usize, FileLook)>,
    /// Ids sent and not yet back.
    queued: HashSet<usize>,
}

/// A file's modification time as a Unix time, 0 if it can't be read.
pub fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

/// Whether iCloud has taken the file's contents off this computer, leaving
/// only its name: reading it would download it, so it is left alone.
#[cfg(target_os = "macos")]
pub fn offloaded(path: &Path) -> bool {
    use std::os::macos::fs::MetadataExt;
    /// `SF_DATALESS` in `<sys/stat.h>`.
    const DATALESS: u32 = 0x4000_0000;
    std::fs::metadata(path).is_ok_and(|m| m.st_flags() & DATALESS != 0)
}

#[cfg(not(target_os = "macos"))]
pub fn offloaded(_path: &Path) -> bool {
    false
}

impl CoverScan {
    pub fn start() -> Self {
        let (requests, rx) = channel::<Request>();
        let (tx, done) = channel();
        std::thread::Builder::new()
            .name("cover-scan".into())
            .spawn(move || {
                for (id, path, album) in rx {
                    let look = FileLook {
                        mtime: mtime(&path),
                        cover: cover::has_cover(&path, &album),
                        // 0: looked, but the file doesn't say.
                        bitrate: Some(tags::file_info(&path).bitrate.unwrap_or(0)),
                    };
                    if tx.send((id, look)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn cover scan thread");
        Self {
            requests,
            done,
            queued: HashSet::new(),
        }
    }

    /// Queues a file to look at, unless it is queued already.
    pub fn add(&mut self, id: usize, path: &Path, album: &str) {
        if self.queued.insert(id) {
            let _ = self
                .requests
                .send((id, path.to_path_buf(), album.to_string()));
        }
    }

    /// Files looked at since the last call.
    pub fn take(&mut self) -> Vec<(usize, FileLook)> {
        let done: Vec<_> = self.done.try_iter().collect();
        for (id, ..) in &done {
            self.queued.remove(id);
        }
        done
    }
}
