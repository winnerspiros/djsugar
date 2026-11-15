//! The purchase inbox: audio that lands in a watched folder (usually
//! Downloads) is unpacked, recognised and filed into the music folder.
//!
//! This module decides what to do with each file; the app runs it on a
//! thread and adds the results to the library.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use djsugar_engine::scan::is_audio;

use crate::{InboxFile, Manifest, Plugin};

/// How long a file must stay the same size before it counts as downloaded.
pub const SETTLE: Duration = Duration::from_secs(3);

/// Endings browsers and download tools give files still being written.
const PARTIAL: [&str; 6] = [
    "crdownload",
    "part",
    "download",
    "tmp",
    "partial",
    "incomplete",
];

/// What happens to a file once it's recognised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Into `<music folder>/Purchased/<Store>/<Artist>/<Album>/`.
    Move,
    /// A copy goes there; the download stays where it is.
    Copy,
    /// Added to the library where it is.
    Leave,
}

impl Mode {
    pub fn from_name(s: &str) -> Self {
        match s {
            "copy" => Mode::Copy,
            "leave" => Mode::Leave,
            _ => Mode::Move,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Mode::Move => "move",
            Mode::Copy => "copy",
            Mode::Leave => "leave",
        }
    }
}

/// Something the inbox can take: an audio file, or a zip of them.
pub fn wanted(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    if name.starts_with('.') {
        return false;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if PARTIAL.contains(&ext.as_str()) {
        return false;
    }
    ext == "zip" || is_audio(path)
}

/// Watches the top level of a folder for finished downloads (or, with
/// [`Watcher::deep`], its subfolders too, as a store app files them).
pub struct Watcher {
    pub dir: PathBuf,
    /// How many folders down files are looked for; 0 for the top only.
    depth: usize,
    /// Files older than this were there before the inbox was switched on,
    /// and are left alone.
    since: SystemTime,
    /// Size and modification time last seen, and since when unchanged.
    pending: HashMap<PathBuf, (u64, SystemTime, Instant)>,
    done: HashSet<PathBuf>,
}

impl Watcher {
    pub fn new(dir: PathBuf, since: SystemTime) -> Self {
        Self {
            dir,
            depth: 0,
            since,
            pending: HashMap::new(),
            done: HashSet::new(),
        }
    }

    /// Also looks `depth` folders down, as in a store app's
    /// `Artist/Album/` folders. Only folders changed since the watch began
    /// are looked into for files.
    pub fn deep(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// The files to look at: the top level's, and those of changed
    /// subfolders down to `depth`.
    fn candidates(&self) -> Vec<(PathBuf, std::fs::Metadata)> {
        let mut out = Vec::new();
        let mut dirs = vec![(self.dir.clone(), 0, true)];
        while let Some((dir, level, changed)) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let Ok(meta) = e.metadata() else { continue };
                let path = e.path();
                if meta.is_dir() {
                    let hidden = e.file_name().to_string_lossy().starts_with('.');
                    if level < self.depth && !hidden {
                        let sub_changed = meta.modified().is_ok_and(|m| m >= self.since);
                        dirs.push((path, level + 1, sub_changed));
                    }
                } else if meta.is_file() && changed {
                    out.push((path, meta));
                }
            }
        }
        out
    }

    /// Files that have finished arriving since the last call.
    pub fn poll(&mut self, now: Instant) -> Vec<PathBuf> {
        let mut ready = Vec::new();
        let mut seen = HashSet::new();
        for (path, meta) in self.candidates() {
            if self.done.contains(&path) || !wanted(&path) {
                continue;
            }
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if modified < self.since {
                self.done.insert(path);
                continue;
            }
            seen.insert(path.clone());
            let size = meta.len();
            match self.pending.get(&path) {
                Some(&(s, m, t)) if s == size && m == modified => {
                    if now.duration_since(t) >= SETTLE && size > 0 {
                        self.pending.remove(&path);
                        self.done.insert(path.clone());
                        ready.push(path);
                    }
                }
                _ => {
                    self.pending.insert(path, (size, modified, now));
                }
            }
        }
        self.pending.retain(|p, _| seen.contains(p));
        ready.sort();
        ready
    }

    /// Lets a file be picked up again (after a failed import).
    pub fn forget(&mut self, path: &Path) {
        self.done.remove(path);
    }
}

/// Extracts the audio and cover images of a zip into `into`, returning the
/// audio files. Paths that would leave `into` are skipped.
pub fn unpack(zip: &Path, into: &Path) -> io::Result<Vec<PathBuf>> {
    let file = std::fs::File::open(zip)?;
    let mut archive = zip::ZipArchive::new(file).map_err(io::Error::other)?;
    std::fs::create_dir_all(into)?;
    let mut audio = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(io::Error::other)?;
        if !entry.is_file() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        // Bandcamp zips are flat; keep just the file name either way.
        let Some(base) = name.file_name() else {
            continue;
        };
        let out = into.join(base);
        let ext = out
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let is_cover = matches!(ext.as_str(), "jpg" | "jpeg" | "png");
        if !is_audio(&out) && !is_cover {
            continue;
        }
        let mut f = std::fs::File::create(&out)?;
        io::copy(&mut entry, &mut f)?;
        if is_audio(&out) {
            audio.push(out);
        }
    }
    audio.sort();
    Ok(audio)
}

/// The store a file most likely came from, if any plugin is fairly sure.
pub fn recognise<'a>(file: &InboxFile, plugins: &'a [Box<dyn Plugin>]) -> Option<&'a Manifest> {
    plugins
        .iter()
        .filter_map(|p| Some((p.manifest(), p.inbox()?.recognise(file))))
        .filter(|(_, c)| *c >= 0.5)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(m, _)| m)
}

/// A name safe to use as one folder or file name.
pub fn safe_name(s: &str, fallback: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.chars().take(120).collect()
    }
}

/// Where a recognised file goes: `<music>/Purchased/<Store>/<Artist>/<Album>/`,
/// or "Downloaded" for a download add-on's files. Files from no known store
/// go under "Other".
pub fn destination(
    music: &Path,
    store: Option<&str>,
    downloads: bool,
    file: &InboxFile,
) -> PathBuf {
    let dir = music
        .join(if downloads { "Downloaded" } else { "Purchased" })
        .join(safe_name(store.unwrap_or("Other"), "Other"))
        .join(safe_name(&file.artist, "Unknown Artist"))
        .join(safe_name(&file.album, "Singles"));
    let name = safe_name(&file.file_name(), "Track");
    free_path(&dir.join(name))
}

/// `path`, or "name (2).ext" and so on if that's taken.
fn free_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap()
}

/// Puts a file where it belongs, returning where it is now.
pub fn file_into(src: &Path, dest: &Path, mode: Mode) -> io::Result<PathBuf> {
    if mode == Mode::Leave {
        return Ok(src.to_path_buf());
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match mode {
        Mode::Copy => {
            std::fs::copy(src, dest)?;
        }
        _ => {
            // A rename fails across disks; copy and remove then.
            if std::fs::rename(src, dest).is_err() {
                std::fs::copy(src, dest)?;
                std::fs::remove_file(src)?;
            }
        }
    }
    Ok(dest.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("djsugar-inbox-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn takes_audio_and_zips_but_not_partial_downloads() {
        assert!(wanted(Path::new("/d/a.flac")));
        assert!(wanted(Path::new("/d/Artist - Album.zip")));
        assert!(!wanted(Path::new("/d/a.flac.crdownload")));
        assert!(!wanted(Path::new("/d/.a.flac")));
        assert!(!wanted(Path::new("/d/notes.pdf")));
    }

    #[test]
    fn waits_for_files_to_settle() {
        let dir = temp("watch");
        let since = SystemTime::now() - Duration::from_secs(60);
        let mut w = Watcher::new(dir.clone(), since);
        let f = dir.join("a.mp3");
        std::fs::write(&f, b"abc").unwrap();
        let t0 = Instant::now();
        assert!(w.poll(t0).is_empty());
        assert!(w.poll(t0 + Duration::from_secs(1)).is_empty());
        assert_eq!(
            w.poll(t0 + SETTLE + Duration::from_secs(1)),
            vec![f.clone()]
        );
        // Only once.
        assert!(w.poll(t0 + SETTLE * 3).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn deep_watch_finds_new_files_in_album_folders() {
        let dir = temp("deep");
        let album = dir.join("Artist").join("Album");
        std::fs::create_dir_all(&album).unwrap();
        let since = SystemTime::now() - Duration::from_secs(60);
        let f = album.join("01 Track.m4a");
        std::fs::write(&f, b"abc").unwrap();
        let mut top = Watcher::new(dir.clone(), since);
        let mut deep = Watcher::new(dir.clone(), since).deep(2);
        let t0 = Instant::now();
        assert!(deep.poll(t0).is_empty());
        let later = t0 + SETTLE + Duration::from_secs(1);
        assert_eq!(deep.poll(later), vec![f]);
        top.poll(t0);
        assert!(top.poll(later).is_empty());
        // Folders not changed since the watch began aren't looked into.
        let mut after =
            Watcher::new(dir.clone(), SystemTime::now() + Duration::from_secs(60)).deep(2);
        after.poll(t0);
        assert!(after.poll(later).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn leaves_files_from_before() {
        let dir = temp("old");
        std::fs::write(dir.join("old.mp3"), b"abc").unwrap();
        let mut w = Watcher::new(dir.clone(), SystemTime::now() + Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(w.poll(t0).is_empty());
        assert!(w.poll(t0 + SETTLE * 2).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unpacks_audio_and_covers_only() {
        let dir = temp("zip");
        let zip_path = dir.join("Bonobo - Migration.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            for name in [
                "Bonobo - Migration - 01 Migration.flac",
                "cover.jpg",
                "readme.txt",
                "../evil.flac",
            ] {
                z.start_file(name, o).unwrap();
                z.write_all(b"x").unwrap();
            }
            z.finish().unwrap();
        }
        let out = dir.join("staging");
        let audio = unpack(&zip_path, &out).unwrap();
        assert_eq!(
            audio,
            vec![out.join("Bonobo - Migration - 01 Migration.flac")]
        );
        assert!(out.join("cover.jpg").exists());
        assert!(!out.join("readme.txt").exists());
        assert!(!dir.join("evil.flac").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn files_by_store_artist_and_album() {
        let dir = temp("dest");
        let f = InboxFile {
            path: dir.join("in/03 Kerala.flac"),
            artist: "Bonobo".into(),
            album: "Migration: Deluxe".into(),
            ..Default::default()
        };
        let d = destination(&dir, Some("Bandcamp"), false, &f);
        assert_eq!(
            d,
            dir.join("Purchased/Bandcamp/Bonobo/Migration_ Deluxe/03 Kerala.flac")
        );
        let none = InboxFile {
            path: dir.join("in/x.mp3"),
            ..Default::default()
        };
        assert_eq!(
            destination(&dir, None, false, &none),
            dir.join("Purchased/Other/Unknown Artist/Singles/x.mp3")
        );
        assert_eq!(
            destination(&dir, Some("Soulseek"), true, &none),
            dir.join("Downloaded/Soulseek/Unknown Artist/Singles/x.mp3")
        );
        // Moving, and not overwriting what's there.
        std::fs::create_dir_all(dir.join("in")).unwrap();
        std::fs::write(&f.path, b"1").unwrap();
        let moved = file_into(&f.path, &d, Mode::Move).unwrap();
        assert!(moved.exists() && !f.path.exists());
        std::fs::write(&f.path, b"2").unwrap();
        let d2 = destination(&dir, Some("Bandcamp"), false, &f);
        assert_eq!(d2.file_name().unwrap(), "03 Kerala (2).flac");
        let copied = file_into(&f.path, &d2, Mode::Copy).unwrap();
        assert!(copied.exists() && f.path.exists());
        assert_eq!(file_into(&f.path, &d2, Mode::Leave).unwrap(), f.path);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recognises_with_the_surest_plugin() {
        let plugins = crate::builtin();
        let f = InboxFile {
            path: "/in/a.flac".into(),
            comment: "Visit https://x.bandcamp.com".into(),
            ..Default::default()
        };
        assert_eq!(recognise(&f, &plugins).map(|m| m.name), Some("Bandcamp"));
        let g = InboxFile {
            path: "/in/a.flac".into(),
            ..Default::default()
        };
        assert!(recognise(&g, &plugins).is_none());
    }
}
