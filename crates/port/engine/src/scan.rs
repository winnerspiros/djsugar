//! Finds audio files under a folder and reads their tags with Lofty.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lofty::prelude::*;

use crate::decode::EXTENSIONS;
use crate::track::{Source, Track};

/// Stops a scan of a huge folder from running forever.
pub const MAX_FILES: usize = 50_000;
const MAX_DEPTH: usize = 12;

/// The folder to scan: `DJSUGAR_MUSIC` if set, otherwise `~/Music`.
pub fn default_music_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DJSUGAR_MUSIC") {
        return Some(PathBuf::from(dir));
    }
    crate::paths::home_dir().map(|h| h.join("Music"))
}

/// Lists audio files under `dir`, sorted by path.
pub fn find_audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            let name = e.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() && depth < MAX_DEPTH {
                stack.push((path, depth + 1));
            } else if kind.is_file() && is_audio(&path) {
                out.push(path);
                if out.len() >= MAX_FILES {
                    out.sort();
                    return out;
                }
            }
        }
    }
    out.sort();
    out
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether `path` is in a cloud drive and not downloaded: macOS marks such
/// files "dataless" (iCloud Drive with Optimise Mac Storage, and the File
/// Provider drives such as Google Drive, Dropbox and OneDrive), and reading
/// one downloads it.
pub fn is_cloud_only(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        // SF_DATALESS in <sys/stat.h>.
        const SF_DATALESS: u32 = 0x4000_0000;
        std::fs::symlink_metadata(path).is_ok_and(|m| m.st_flags() & SF_DATALESS != 0)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

/// Reads tags and length. Files whose tags cannot be read still get a row,
/// titled after the file name.
pub fn read_track(id: usize, path: &Path) -> Track {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    // "Song.stem.mp4" is an NI stem file of "Song".
    let stem = match stem.len().checked_sub(5) {
        Some(n) if crate::stems::is_ni_stem(path) => stem[..n].to_string(),
        _ => stem,
    };
    let mut track = Track {
        id,
        artist: String::new(),
        title: stem,
        album: String::new(),
        genre: String::new(),
        rating: 0,
        bpm: 0.0,
        key: String::new(),
        duration: 0.0,
        first_beat: 0.0,
        source: Source::File(path.to_path_buf()),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        grid: Default::default(),
        info: Default::default(),
    };
    // Reading tags would download the whole file from iCloud (or another
    // cloud drive) first; it's titled after its name until it's loaded.
    if is_cloud_only(path) {
        return track;
    }
    let Ok(file) = lofty::read_from_path(path) else {
        return track;
    };
    track.duration = file.properties().duration().as_secs_f64();
    if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
        let text = |v: Option<std::borrow::Cow<'_, str>>| v.map(|s| s.trim().to_string());
        if let Some(t) = text(tag.title()).filter(|t| !t.is_empty()) {
            track.title = t;
        }
        track.artist = text(tag.artist()).unwrap_or_default();
        track.album = text(tag.album()).unwrap_or_default();
        track.genre = text(tag.genre()).unwrap_or_default();
        track.key = tag_key(tag);
        if !track.key.is_empty() {
            track.info.key_detected = Some(false);
        }
        let item = |k: ItemKey| {
            tag.get_string(k)
                .map(str::trim)
                .unwrap_or_default()
                .to_string()
        };
        let info = &mut track.info;
        info.disc_number = tag.disk().unwrap_or(0);
        info.label = item(ItemKey::Label);
        if info.label.is_empty() {
            info.label = item(ItemKey::Publisher);
        }
        info.composer = item(ItemKey::Composer);
        info.remixer = item(ItemKey::Remixer);
        info.isrc = item(ItemKey::Isrc);
        track.bpm = tag
            .get_string(ItemKey::Bpm)
            .or_else(|| tag.get_string(ItemKey::IntegerBpm))
            .and_then(|b| b.trim().replace(',', ".").parse::<f64>().ok())
            .filter(|b| (40.0..=250.0).contains(b))
            .unwrap_or(0.0);
        let more = crate::tags::fields_of(tag);
        track.rating = more.rating;
        track.info.remix = more.remix;
        track.info.year = more.year;
        track.info.track_number = more.track_number;
        track.info.grouping = more.grouping;
        track.info.comment = more.comment;
    }
    track
}

fn tag_key(tag: &lofty::tag::Tag) -> String {
    tag.get_string(ItemKey::InitialKey)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// The key the file's tags give, or "" for none.
pub fn read_key(path: &Path) -> String {
    lofty::read_from_path(path)
        .ok()
        .and_then(|f| f.primary_tag().or_else(|| f.first_tag()).map(tag_key))
        .unwrap_or_default()
}

/// The comment tag, where stores leave notes such as Bandcamp's
/// "Visit https://artist.bandcamp.com". Lofty reads a Vorbis COMMENT field
/// (FLAC, Ogg) as a description, so that counts too.
pub fn read_comment(path: &Path) -> String {
    lofty::read_from_path(path)
        .ok()
        .and_then(|f| {
            let tag = f.primary_tag().or_else(|| f.first_tag())?;
            [ItemKey::Comment, ItemKey::Description]
                .into_iter()
                .find_map(|k| tag.get_string(k).map(|c| c.trim().to_string()))
        })
        .unwrap_or_default()
}

/// Scans `dir` and returns its tracks, numbered from `first_id` so their ids
/// do not collide with tracks already loaded on a deck.
pub fn scan(dir: &Path, first_id: usize) -> Vec<Arc<Track>> {
    find_audio_files(dir)
        .iter()
        .enumerate()
        .map(|(i, p)| Arc::new(read_track(first_id + i, p)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_reads_wav_files() {
        let dir = std::env::temp_dir().join(format!("djsugar-scan-{}", std::process::id()));
        let sub = dir.join("Artist/Album");
        std::fs::create_dir_all(&sub).unwrap();
        crate::decode::tests::write_wav(&sub.join("01 Tune.wav"), 44_100, 2, &vec![0; 88_200]);
        std::fs::write(dir.join("notes.txt"), "not audio").unwrap();
        let tracks = scan(&dir, 100);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "01 Tune");
        assert_eq!(tracks[0].id, 100);
        assert!(
            (tracks[0].duration - 1.0).abs() < 0.01,
            "{}",
            tracks[0].duration
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_the_comment() {
        use lofty::config::WriteOptions;
        use lofty::tag::{Tag, TagType};
        let dir = std::env::temp_dir().join(format!("djsugar-comment-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bought.wav");
        crate::decode::tests::write_wav(&path, 44_100, 2, &vec![0; 4_410]);
        assert_eq!(read_comment(&path), "");
        let mut tag = Tag::new(TagType::Id3v2);
        tag.insert_text(ItemKey::Comment, "Visit https://artist.bandcamp.com".into());
        tag.save_to_path(&path, WriteOptions::default()).unwrap();
        assert_eq!(read_comment(&path), "Visit https://artist.bandcamp.com");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
