//! The Files source: folders on the computer, browsed like Mixxx's
//! "Computer" view.
//!
//! The tree starts at pinned folders, the music folder, Home, Desktop,
//! Downloads and the mounted drives, and reads a folder's subfolders when it
//! is opened. Listing a folder and reading its tags happen on a thread of
//! their own, so a big folder or a slow drive never stalls the window.
//! Files the collection already has are not read again: the browser shows
//! what the library knows about them.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use djsugar_engine::scan::{find_audio_files, is_audio, read_track};
use djsugar_engine::Track;
use serde_json::json;

/// What a place in the tree is, for its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Folder,
    Drive,
    Pinned,
    /// iCloud Drive, Google Drive, Dropbox and the like.
    Cloud,
}

/// A folder the tree starts from.
#[derive(Clone, Debug, PartialEq)]
pub struct Root {
    pub name: String,
    pub path: PathBuf,
    pub place: Place,
}

/// The folders the tree starts from: pinned folders, the music folder,
/// Home, Desktop, Downloads, cloud drives, then the mounted drives.
pub fn roots() -> Vec<Root> {
    let mut out: Vec<Root> = pinned()
        .into_iter()
        .filter(|p| p.is_dir())
        .map(|path| Root {
            name: folder_name(&path),
            path,
            place: Place::Pinned,
        })
        .collect();
    let home = djsugar_engine::paths::home_dir();
    let mut places = Vec::new();
    for (i, music) in crate::settings::music_folders().into_iter().enumerate() {
        let name = if i == 0 {
            "Music".to_string()
        } else {
            folder_name(&music)
        };
        places.push((name, music));
    }
    if let Some(home) = &home {
        places.push(("Home".into(), home.clone()));
        places.push(("Desktop".into(), home.join("Desktop")));
        places.push(("Downloads".into(), home.join("Downloads")));
    }
    for (name, path) in places {
        if path.is_dir() && !out.iter().any(|r| r.path == path) {
            out.push(Root {
                name,
                path,
                place: Place::Folder,
            });
        }
    }
    if let Some(home) = &home {
        for (name, path) in cloud_drives(home) {
            if !out.iter().any(|r| r.path == path) {
                out.push(Root {
                    name,
                    path,
                    place: Place::Cloud,
                });
            }
        }
    }
    out.extend(drives().into_iter().map(|path| Root {
        name: folder_name(&path),
        path,
        place: Place::Drive,
    }));
    out
}

/// Cloud drives synced to this computer, with the names their apps use.
///
/// On macOS, iCloud Drive lives in `~/Library/Mobile Documents`, and Google
/// Drive, Dropbox, OneDrive, Box and others (through Apple's File Provider)
/// in `~/Library/CloudStorage`, one folder per account, e.g.
/// `GoogleDrive-dj@example.com`. On Linux, the desktop apps sync into a
/// folder in Home, and GNOME's online accounts mount under
/// `/run/user/<uid>/gvfs`.
pub fn cloud_drives(home: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if cfg!(target_os = "macos") {
        let icloud = home.join("Library/Mobile Documents/com~apple~CloudDocs");
        if icloud.is_dir() {
            out.push(("iCloud Drive".to_string(), icloud));
        }
        let accounts = subfolders(&home.join("Library/CloudStorage"));
        let names: Vec<String> = accounts
            .iter()
            .map(|p| cloud_name(&folder_name(p)))
            .collect();
        for (path, name) in accounts.iter().zip(&names) {
            // Two accounts of one service: say which is which.
            let name = if names.iter().filter(|n| *n == name).count() > 1 {
                let account = folder_name(path);
                match account.split_once('-') {
                    Some((_, who)) => format!("{name} ({who})"),
                    None => account,
                }
            } else {
                name.clone()
            };
            out.push((name, path.clone()));
        }
    } else {
        for name in [
            "Dropbox",
            "Google Drive",
            "OneDrive",
            "pCloudDrive",
            "Nextcloud",
            "MEGA",
        ] {
            let path = home.join(name);
            if path.is_dir() {
                out.push((cloud_name(name), path));
            }
        }
        #[cfg(unix)]
        if let Ok(meta) = std::fs::metadata(home) {
            use std::os::unix::fs::MetadataExt;
            let gvfs = PathBuf::from(format!("/run/user/{}/gvfs", meta.uid()));
            for path in subfolders(&gvfs) {
                out.push((cloud_name(&folder_name(&path)), path));
            }
        }
    }
    out
}

/// What a cloud storage folder's service is called: "GoogleDrive-a@b.com"
/// is Google Drive, "OneDrive-Personal" OneDrive, and GNOME's
/// "google-drive:host=gmail.com,user=dj" Google Drive.
fn cloud_name(folder: &str) -> String {
    let service = folder
        .split(['-', ':'])
        .next()
        .unwrap_or(folder)
        .to_lowercase()
        .replace([' ', '_'], "");
    match service.as_str() {
        "googledrive" | "google" => "Google Drive",
        "dropbox" => "Dropbox",
        "onedrive" => "OneDrive",
        "box" => "Box",
        "pclouddrive" | "pcloud" => "pCloud",
        "nextcloud" => "Nextcloud",
        "mega" => "MEGA",
        "icloud" => "iCloud Drive",
        _ => return folder.to_string(),
    }
    .to_string()
}

/// Mounted drives: `/Volumes` on macOS (but not the startup disk, which is
/// a link to `/`); on Linux, what desktop systems mount under `/media` and
/// `/run/media`, and `/mnt`; on Windows, the drive letters other than the
/// system drive.
pub fn drives() -> Vec<PathBuf> {
    if cfg!(windows) {
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        return ('C'..='Z')
            .map(|l| format!("{l}:"))
            .filter(|d| !d.eq_ignore_ascii_case(&system))
            .map(|d| PathBuf::from(format!("{d}\\")))
            .filter(|p| p.is_dir())
            .collect();
    }
    let mut parents: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        parents.push("/Volumes".into());
    } else {
        let user = std::env::var("USER").unwrap_or_default();
        if !user.is_empty() {
            parents.push(Path::new("/media").join(&user));
            parents.push(Path::new("/run/media").join(&user));
        }
        parents.push("/media".into());
        parents.push("/mnt".into());
    }
    let mut out = Vec::new();
    for parent in &parents {
        for dir in subfolders(parent) {
            // The startup disk on macOS, and /media/$USER itself on Linux.
            let is_root = std::fs::read_link(&dir).is_ok_and(|t| t == Path::new("/"));
            if is_root || parents.contains(&dir) || out.contains(&dir) {
                continue;
            }
            out.push(dir);
        }
    }
    out
}

/// The folders directly in `dir`, sorted by name, without hidden ones.
pub fn subfolders(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        // Follows links, so a linked folder opens like any other.
        .filter(|p| p.is_dir())
        // macOS apps and libraries are folders too, but not ones to browse.
        .filter(|p| {
            !matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("app" | "bundle" | "photoslibrary" | "musiclibrary" | "logicx")
            )
        })
        .collect();
    sort_by_name(&mut out);
    out
}

/// The audio files directly in `dir`, sorted by name.
pub fn audio_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_audio(p))
        .collect();
    sort_by_name(&mut out);
    out
}

fn sort_by_name(paths: &mut [PathBuf]) {
    paths.sort_by_cached_key(|p| folder_name(p).to_lowercase());
}

/// What a folder is called in the tree: its last part, or the whole path
/// for `/`.
pub fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Folders the DJ pinned to the top of the tree.
pub fn pinned() -> Vec<PathBuf> {
    crate::settings::paths("files_pinned")
}

pub fn set_pinned(paths: &[PathBuf]) {
    let list: Vec<_> = paths.iter().map(|p| p.display().to_string()).collect();
    crate::settings::set("files_pinned", json!(list));
}

/// The folder open in Files last time.
pub fn last_folder() -> Option<PathBuf> {
    crate::settings::path_of("files_last").filter(|p| p.is_dir())
}

pub fn set_last_folder(path: &Path) {
    crate::settings::set("files_last", json!(path.display().to_string()));
}

/// Whether listings include subfolders' files, as last chosen.
pub fn recursive() -> bool {
    crate::settings::flag("files_recursive", false)
}

pub fn set_recursive(on: bool) {
    crate::settings::set("files_recursive", json!(on));
}

/// Shows `path` in Finder or File Explorer (selected) or opens the folder
/// holding it in the file manager.
#[cfg(windows)]
pub fn reveal(path: &Path) -> bool {
    use std::os::windows::process::CommandExt;
    // Explorer reads "/select," and the quoted path as one argument, which
    // Rust's own quoting would split up.
    std::process::Command::new("explorer")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .is_ok()
}

/// Shows `path` in Finder or File Explorer (selected) or opens the folder
/// holding it in the file manager.
#[cfg(not(windows))]
pub fn reveal(path: &Path) -> bool {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(path);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        });
        c
    };
    cmd.spawn().is_ok()
}

/// What Finder or the file manager is called here, for menus.
pub fn file_manager() -> &'static str {
    if cfg!(target_os = "macos") {
        "Finder"
    } else if cfg!(windows) {
        "File Explorer"
    } else {
        "file manager"
    }
}

/// Asks for one folder's files to be listed and read, or for a folder's
/// files to be added to the collection.
enum Job {
    List {
        generation: u64,
        dir: PathBuf,
        recursive: bool,
    },
    Add(PathBuf),
}

pub enum Found {
    /// The audio files in the listed folder, in order. Their tags follow.
    Listed {
        generation: u64,
        paths: Vec<PathBuf>,
    },
    /// Tags of files in the listing for `generation`.
    Read {
        generation: u64,
        tracks: Vec<Track>,
        done: bool,
    },
    /// Files to add to the collection: ones read before (by path) and ones
    /// read now.
    ToAdd {
        known: Vec<PathBuf>,
        tracks: Vec<Track>,
        done: bool,
    },
}

/// Sends tags back in batches of this many, or this often, whichever comes
/// first, so rows fill in as they're read.
const BATCH: usize = 64;
const BATCH_TIME: Duration = Duration::from_millis(120);

/// The thread that lists folders and reads tags.
pub struct Reader {
    jobs: Sender<Job>,
    found: Receiver<Found>,
    /// Files the collection has, which are not read again.
    pub collection: Arc<RwLock<HashSet<PathBuf>>>,
    generation: u64,
}

impl Reader {
    pub fn start() -> Self {
        let (jobs, rx) = channel();
        let (tx, found) = channel();
        let collection = Arc::new(RwLock::new(HashSet::new()));
        let known = collection.clone();
        std::thread::Builder::new()
            .name("files".into())
            .spawn(move || work(rx, tx, known))
            .expect("files thread");
        Reader {
            jobs,
            found,
            collection,
            generation: 0,
        }
    }

    /// Lists `dir`; earlier listings still being read are dropped. Returns
    /// the listing's generation, which its answers carry.
    pub fn list(&mut self, dir: &Path, recursive: bool) -> u64 {
        self.generation += 1;
        let _ = self.jobs.send(Job::List {
            generation: self.generation,
            dir: dir.to_path_buf(),
            recursive,
        });
        self.generation
    }

    pub fn add(&self, dir: &Path) {
        let _ = self.jobs.send(Job::Add(dir.to_path_buf()));
    }

    pub fn poll(&self) -> Vec<Found> {
        self.found.try_iter().collect()
    }
}

fn work(jobs: Receiver<Job>, found: Sender<Found>, collection: Arc<RwLock<HashSet<PathBuf>>>) {
    // Files already read and sent; the browser keeps them.
    let mut sent: HashSet<PathBuf> = HashSet::new();
    let mut queue: VecDeque<Job> = VecDeque::new();
    loop {
        let job = match queue.pop_front() {
            Some(j) => j,
            None => match jobs.recv() {
                Ok(j) => j,
                Err(_) => return,
            },
        };
        let (generation, paths) = match &job {
            Job::List {
                generation,
                dir,
                recursive,
            } => {
                let paths = if *recursive {
                    find_audio_files(dir)
                } else {
                    audio_files(dir)
                };
                let _ = found.send(Found::Listed {
                    generation: *generation,
                    paths: paths.clone(),
                });
                (Some(*generation), paths)
            }
            Job::Add(dir) => (None, find_audio_files(dir)),
        };
        let known = collection.read().map(|c| c.clone()).unwrap_or_default();
        let mut batch = Vec::new();
        let mut already = Vec::new();
        let mut last = Instant::now();
        let mut dropped = false;
        let send = |batch: &mut Vec<Track>, already: &mut Vec<PathBuf>, done: bool| {
            let tracks = std::mem::take(batch);
            let _ = found.send(match generation {
                Some(generation) => Found::Read {
                    generation,
                    tracks,
                    done,
                },
                None => Found::ToAdd {
                    known: std::mem::take(already),
                    tracks,
                    done,
                },
            });
        };
        for path in paths {
            // A newer listing replaces this one.
            if generation.is_some() {
                loop {
                    match jobs.try_recv() {
                        Ok(j @ Job::List { .. }) => {
                            queue.retain(|q| !matches!(q, Job::List { .. }));
                            queue.push_back(j);
                            dropped = true;
                        }
                        Ok(j) => queue.push_back(j),
                        Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                    }
                }
                if dropped {
                    break;
                }
            }
            if known.contains(&path) {
                continue;
            }
            if sent.contains(&path) {
                if generation.is_none() {
                    already.push(path);
                }
                continue;
            }
            batch.push(read_track(0, &path));
            sent.insert(path);
            if batch.len() >= BATCH || last.elapsed() >= BATCH_TIME {
                send(&mut batch, &mut already, false);
                last = Instant::now();
            }
        }
        if !dropped {
            send(&mut batch, &mut already, true);
        } else if !batch.is_empty() {
            // Read already: keep them, though nobody is looking.
            send(&mut batch, &mut already, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("djsugar-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_cloud_storage_like_their_apps() {
        assert_eq!(cloud_name("GoogleDrive-dj@example.com"), "Google Drive");
        assert_eq!(cloud_name("OneDrive-Personal"), "OneDrive");
        assert_eq!(cloud_name("Dropbox"), "Dropbox");
        assert_eq!(
            cloud_name("google-drive:host=gmail.com,user=dj"),
            "Google Drive"
        );
        assert_eq!(cloud_name("SomethingElse-x"), "SomethingElse-x");
    }

    #[test]
    fn lists_folders_and_audio_by_name() {
        let dir = temp_dir("list");
        for d in ["b", "A", ".hidden", "Tool.app"] {
            std::fs::create_dir(dir.join(d)).unwrap();
        }
        for f in ["z.mp3", "a.FLAC", "notes.txt", ".x.mp3"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let names = |v: Vec<PathBuf>| v.iter().map(|p| folder_name(p)).collect::<Vec<_>>();
        assert_eq!(names(subfolders(&dir)), ["A", "b"]);
        assert_eq!(names(audio_files(&dir)), ["a.FLAC", "z.mp3"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_tags_off_the_window_thread_and_skips_the_collection() {
        let dir = temp_dir("read");
        std::fs::create_dir(dir.join("sub")).unwrap();
        for f in ["one.mp3", "two.mp3", "sub/three.mp3"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let mut reader = Reader::start();
        reader
            .collection
            .write()
            .unwrap()
            .insert(dir.join("two.mp3"));
        let wait = |reader: &Reader| {
            let mut all = Vec::new();
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                let got = reader.poll();
                let done = got.iter().any(|f| {
                    matches!(
                        f,
                        Found::Read { done: true, .. } | Found::ToAdd { done: true, .. }
                    )
                });
                all.extend(got);
                if done {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            all
        };
        let g = reader.list(&dir, false);
        let got = wait(&reader);
        let Some(Found::Listed { generation, paths }) = got.first() else {
            panic!("no listing");
        };
        assert_eq!(*generation, g);
        assert_eq!(paths.len(), 2);
        let read: Vec<String> = got
            .iter()
            .flat_map(|f| match f {
                Found::Read { tracks, .. } => tracks.iter().map(|t| t.title.clone()).collect(),
                _ => Vec::new(),
            })
            .collect();
        // Untagged files are titled after their names; two.mp3 is known.
        assert_eq!(read, ["one"]);

        // Adding the folder: one.mp3 was read already, three.mp3 is new.
        reader.add(&dir);
        let got = wait(&reader);
        let (known, tracks): (Vec<_>, Vec<_>) = got
            .into_iter()
            .filter_map(|f| match f {
                Found::ToAdd { known, tracks, .. } => Some((known, tracks)),
                _ => None,
            })
            .unzip();
        assert_eq!(known.concat(), [dir.join("one.mp3")]);
        let titles: Vec<String> = tracks.concat().into_iter().map(|t| t.title).collect();
        assert_eq!(titles, ["three"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
