//! Brings an iTunes or Music app library into the collection at start, when
//! the DJ turns it on in Settings > Library: the tracks in its Library XML
//! with their tags, ratings and play counts, and the playlist tree, folders
//! within folders. It only reads the file, never writes it.
//!
//! The Library XML is an Apple property list. iTunes (and iTunes on
//! Windows) keeps one up to date by itself; the Music app writes one when
//! Music › Settings › Advanced › "Share Library XML with other
//! applications" is on, or once with File › Library › Export Library….
//! `Tracks` is a dictionary of track dictionaries by `Track ID`, each with
//! its `Location` as a `file://` URL; `Playlists` is an array of playlists,
//! each naming its tracks by `Track ID` under `Playlist Items`, and its
//! folder by `Parent Persistent ID`. The file has no cues or beat grids.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use djsugar_engine::import::{ImportedEntry, ImportedList, ImportedListKind, StreamedTrack};
use djsugar_engine::{LibraryInfo, Source, Track};
use plist::{Dictionary, Value};

use crate::{ImportedLibrary, LibraryImport, ListGroup};

pub const PLAYLISTS: &str = "playlists";
pub const SMART: &str = "smart";

pub struct ItunesLibrary;

/// Where iTunes and the Music app leave their Library XML, newest first
/// when there are several (or `DJSUGAR_ITUNES_LIBRARY`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_ITUNES_LIBRARY") {
        return Some(PathBuf::from(p));
    }
    let home = djsugar_engine::paths::home_dir()?;
    newest(&candidates(&home))
}

/// The usual places, under the home folder: the Music app's shared XML,
/// iTunes' own (Mac and Windows), and an export saved in Music or on the
/// Desktop.
fn candidates(home: &Path) -> Vec<PathBuf> {
    [
        "Music/Music/Library.xml",
        "Music/Music/Music Library.xml",
        "Music/iTunes/iTunes Library.xml",
        "Music/iTunes/iTunes Music Library.xml",
        "Music/Library.xml",
        "Desktop/Library.xml",
    ]
    .iter()
    .map(|rel| home.join(rel))
    .collect()
}

fn newest(paths: &[PathBuf]) -> Option<PathBuf> {
    paths
        .iter()
        .filter_map(|p| {
            let modified = std::fs::metadata(p).ok()?.modified().ok()?;
            p.is_file().then(|| (modified, p.clone()))
        })
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

impl LibraryImport for ItunesLibrary {
    fn id(&self) -> &str {
        "itunes_library"
    }

    fn title(&self) -> &str {
        "iTunes"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn missing_hint(&self) -> &str {
        "In the Music app choose File › Library › Export Library…, then choose that Library.xml here."
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: PLAYLISTS,
                title: "Playlists",
                detail: "Your playlists, in their folders.",
                on_by_default: true,
            },
            ListGroup {
                id: SMART,
                title: "Smart playlists",
                detail: "As they are now; they don't update.",
                on_by_default: false,
            },
        ]
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        preview(database)
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = read(database, groups, Path::exists)?;
        eprintln!(
            "iTunes library: {} tracks here, {} lists",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}

fn load(path: &Path) -> Result<Dictionary, String> {
    let value =
        Value::from_file(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let dict = value
        .into_dictionary()
        .filter(|d| d.contains_key("Tracks"))
        .ok_or("not an iTunes or Music library (Library.xml)")?;
    Ok(dict)
}

/// "1,204 tracks and 37 playlists."
pub fn preview(path: &Path) -> Result<String, String> {
    let lib = load(path)?;
    let tracks = songs(&lib).count();
    let lists = playlists(&lib, &[PLAYLISTS], &HashMap::new()).len();
    let n = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    Ok(format!(
        "{} and {}.",
        n(tracks, "track", "tracks"),
        n(lists, "playlist", "playlists")
    ))
}

/// The tracks whose files `exists` says are here, and the playlists in
/// `groups`.
pub fn read(
    path: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let lib = load(path)?;
    // Playlists name a track by its Track ID.
    let mut entries: HashMap<i64, ImportedEntry> = HashMap::new();
    let mut tracks = Vec::new();
    for (id, t) in songs(&lib) {
        let entry = if let Some(s) = streamed(t) {
            ImportedEntry {
                path: PathBuf::from(&s.uri),
                played_at: None,
                streamed: Some(s),
            }
        } else {
            let Some(file) = string(t, "Location").and_then(crate::rekordbox::xml::location) else {
                continue;
            };
            if exists(&file) {
                tracks.push(track(t, file.clone()));
            }
            ImportedEntry::new(file, None)
        };
        entries.insert(id, entry);
    }
    let lists = playlists(&lib, groups, &entries);
    Ok(ImportedLibrary { tracks, lists })
}

fn string<'a>(d: &'a Dictionary, key: &str) -> Option<&'a str> {
    d.get(key)?.as_string()
}

fn text(d: &Dictionary, key: &str) -> String {
    string(d, key).unwrap_or_default().trim().to_string()
}

fn int(d: &Dictionary, key: &str) -> Option<i64> {
    let v = d.get(key)?;
    v.as_signed_integer()
        .or_else(|| v.as_unsigned_integer().map(|u| u as i64))
        .or_else(|| v.as_real().map(|r| r as i64))
}

fn flag(d: &Dictionary, key: &str) -> bool {
    d.get(key).and_then(Value::as_boolean).unwrap_or(false)
}

fn date(d: &Dictionary, key: &str) -> Option<i64> {
    let t: SystemTime = d.get(key)?.as_date()?.into();
    Some(match t.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_secs() as i64,
        Err(before) => -(before.duration().as_secs() as i64),
    })
}

/// The music in `Tracks`: podcasts, audiobooks, videos, voice memos and
/// internet radio stay out.
fn songs(lib: &Dictionary) -> impl Iterator<Item = (i64, &Dictionary)> {
    lib.get("Tracks")
        .and_then(Value::as_dictionary)
        .into_iter()
        .flat_map(|t| t.iter())
        .filter_map(|(key, v)| {
            let t = v.as_dictionary()?;
            let id = int(t, "Track ID").or_else(|| key.parse().ok())?;
            Some((id, t))
        })
        .filter(|(_, t)| is_music(t))
}

fn is_music(t: &Dictionary) -> bool {
    let not_music = [
        "Podcast",
        "Movie",
        "TV Show",
        "Music Video",
        "Has Video",
        "Voice Memo",
        "iTunesU",
    ];
    if not_music.iter().any(|k| flag(t, k)) || string(t, "Track Type") == Some("URL") {
        return false;
    }
    let kind = text(t, "Kind").to_lowercase();
    let genre = text(t, "Genre").to_lowercase();
    !(kind.contains("book")
        || kind.contains("audible")
        || genre == "audiobook"
        || genre == "audiobooks")
}

/// An Apple Music track (or one only in the DJ's iCloud library): not a
/// file here, or a protected download DJ Sugar can't play. It stands in the
/// lists until the collection has the song.
fn streamed(t: &Dictionary) -> Option<StreamedTrack> {
    let remote = string(t, "Track Type") == Some("Remote") || !t.contains_key("Location");
    if !remote && !flag(t, "Protected") {
        return None;
    }
    let pid = text(t, "Persistent ID");
    Some(StreamedTrack {
        service: "applemusic".into(),
        id: String::new(),
        uri: format!("applemusic:library:{pid}"),
        artist: text(t, "Artist"),
        title: text(t, "Name"),
        duration: int(t, "Total Time").unwrap_or(0).max(0) as f64 / 1000.0,
        isrc: String::new(),
    })
}

fn track(t: &Dictionary, file: PathBuf) -> Track {
    let year = int(t, "Year")
        .filter(|y| *y > 0)
        .map(|y| y.to_string())
        .unwrap_or_default();
    // 0 to 100, 20 a star; a rating the app worked out from the album's
    // isn't the DJ's.
    let rating = if flag(t, "Rating Computed") {
        0
    } else {
        (int(t, "Rating").unwrap_or(0) as f64 / 20.0)
            .round()
            .clamp(0.0, 5.0) as u8
    };
    let count = |key: &str| int(t, key).unwrap_or(0).max(0) as u32;
    Track {
        id: 0,
        artist: text(t, "Artist"),
        title: text(t, "Name"),
        album: text(t, "Album"),
        genre: text(t, "Genre"),
        rating,
        bpm: int(t, "BPM").unwrap_or(0).max(0) as f64,
        key: String::new(),
        duration: int(t, "Total Time").unwrap_or(0).max(0) as f64 / 1000.0,
        first_beat: 0.0,
        grid: Default::default(),
        source: Source::File(file),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo {
            comment: text(t, "Comments"),
            grouping: text(t, "Grouping"),
            play_count: count("Play Count"),
            year,
            track_number: count("Track Number"),
            disc_number: count("Disc Number"),
            composer: text(t, "Composer"),
            added: date(t, "Date Added").unwrap_or(0),
            last_played: date(t, "Play Date UTC"),
            ..Default::default()
        },
    }
}

/// The playlists in `groups`, in their folders, in the library's order.
/// The app's own lists (Library, Music, Purchased, Genius…) and hidden ones
/// stay out; smart playlists come in as they are now.
fn playlists(
    lib: &Dictionary,
    groups: &[&str],
    entries: &HashMap<i64, ImportedEntry>,
) -> Vec<ImportedList> {
    let all: Vec<&Dictionary> = lib
        .get("Playlists")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_dictionary)
        .collect();
    let folders: HashMap<String, (String, Option<String>)> = all
        .iter()
        .filter(|p| flag(p, "Folder"))
        .map(|p| {
            (
                text(p, "Playlist Persistent ID"),
                (
                    text(p, "Name"),
                    string(p, "Parent Persistent ID").map(str::to_string),
                ),
            )
        })
        .collect();
    let mut out = Vec::new();
    for p in all {
        let builtin = flag(p, "Master")
            || p.contains_key("Distinguished Kind")
            || p.get("Visible").and_then(Value::as_boolean) == Some(false);
        if builtin || flag(p, "Folder") {
            continue;
        }
        let smart = p.contains_key("Smart Info") || p.contains_key("Smart Criteria");
        if !groups.contains(&if smart { SMART } else { PLAYLISTS }) {
            continue;
        }
        let entries = p
            .get("Playlist Items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|i| entries.get(&int(i.as_dictionary()?, "Track ID")?).cloned())
            .collect();
        out.push(ImportedList {
            folders: path(string(p, "Parent Persistent ID"), &folders),
            name: text(p, "Name"),
            kind: ImportedListKind::List,
            entries,
        });
    }
    out
}

/// The folders above a playlist, outermost first.
fn path<'a>(
    mut parent: Option<&'a str>,
    folders: &'a HashMap<String, (String, Option<String>)>,
) -> Vec<String> {
    let mut out = Vec::new();
    // A loop in the parents would never end; no tree is this deep.
    while let Some((name, up)) = parent.and_then(|id| folders.get(id)) {
        if out.len() > 64 {
            break;
        }
        out.push(name.clone());
        parent = up.as_deref();
    }
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Major Version</key><integer>1</integer>
	<key>Application Version</key><string>1.4.5.7</string>
	<key>Music Folder</key><string>file:///Users/dj/Music/Music/Media.localized/</string>
	<key>Tracks</key>
	<dict>
		<key>101</key>
		<dict>
			<key>Track ID</key><integer>101</integer>
			<key>Name</key><string>Glue</string>
			<key>Artist</key><string>Bicep</string>
			<key>Composer</key><string>Matt McBriar</string>
			<key>Album</key><string>Bicep</string>
			<key>Grouping</key><string>Warm up</string>
			<key>Genre</key><string>Electronic</string>
			<key>Kind</key><string>MPEG audio file</string>
			<key>Total Time</key><integer>268900</integer>
			<key>Disc Number</key><integer>1</integer>
			<key>Track Number</key><integer>3</integer>
			<key>Year</key><integer>2017</integer>
			<key>BPM</key><integer>120</integer>
			<key>Date Added</key><date>2022-04-09T10:00:00Z</date>
			<key>Play Count</key><integer>7</integer>
			<key>Play Date UTC</key><date>2023-01-05T22:30:00Z</date>
			<key>Rating</key><integer>80</integer>
			<key>Comments</key><string>nice</string>
			<key>Persistent ID</key><string>AAAA000000000001</string>
			<key>Track Type</key><string>File</string>
			<key>Location</key><string>file:///Users/dj/Music/Music/Media.localized/Music/Bicep/Bicep/03%20Glue.mp3</string>
		</dict>
		<key>102</key>
		<dict>
			<key>Track ID</key><integer>102</integer>
			<key>Name</key><string>Away</string>
			<key>Artist</key><string>Y</string>
			<key>Rating</key><integer>60</integer>
			<key>Rating Computed</key><true/>
			<key>Track Type</key><string>File</string>
			<key>Location</key><string>file://localhost/C:/Users/dj/Music/away.m4a</string>
		</dict>
		<key>103</key>
		<dict>
			<key>Track ID</key><integer>103</integer>
			<key>Name</key><string>Streamed Song</string>
			<key>Artist</key><string>Z</string>
			<key>Total Time</key><integer>200000</integer>
			<key>Apple Music</key><true/>
			<key>Persistent ID</key><string>BBBB000000000002</string>
			<key>Track Type</key><string>Remote</string>
		</dict>
		<key>104</key>
		<dict>
			<key>Track ID</key><integer>104</integer>
			<key>Name</key><string>Episode 1</string>
			<key>Podcast</key><true/>
			<key>Track Type</key><string>File</string>
			<key>Location</key><string>file:///Users/dj/Music/Podcasts/ep1.mp3</string>
		</dict>
		<key>105</key>
		<dict>
			<key>Track ID</key><integer>105</integer>
			<key>Name</key><string>Chapter 1</string>
			<key>Kind</key><string>AAC audio book file</string>
			<key>Track Type</key><string>File</string>
			<key>Location</key><string>file:///Users/dj/Music/Books/c1.m4b</string>
		</dict>
	</dict>
	<key>Playlists</key>
	<array>
		<dict>
			<key>Master</key><true/>
			<key>Name</key><string>Library</string>
			<key>Playlist ID</key><integer>1</integer>
			<key>Playlist Persistent ID</key><string>P0</string>
			<key>Visible</key><false/>
			<key>Playlist Items</key>
			<array><dict><key>Track ID</key><integer>101</integer></dict></array>
		</dict>
		<dict>
			<key>Name</key><string>Music</string>
			<key>Distinguished Kind</key><integer>4</integer>
			<key>Playlist Persistent ID</key><string>P1</string>
		</dict>
		<dict>
			<key>Name</key><string>Gigs</string>
			<key>Folder</key><true/>
			<key>Playlist Persistent ID</key><string>F1</string>
			<key>Playlist Items</key>
			<array><dict><key>Track ID</key><integer>101</integer></dict></array>
		</dict>
		<dict>
			<key>Name</key><string>2023</string>
			<key>Folder</key><true/>
			<key>Playlist Persistent ID</key><string>F2</string>
			<key>Parent Persistent ID</key><string>F1</string>
		</dict>
		<dict>
			<key>Name</key><string>Friday</string>
			<key>Playlist Persistent ID</key><string>P2</string>
			<key>Parent Persistent ID</key><string>F2</string>
			<key>Playlist Items</key>
			<array>
				<dict><key>Track ID</key><integer>101</integer></dict>
				<dict><key>Track ID</key><integer>103</integer></dict>
				<dict><key>Track ID</key><integer>104</integer></dict>
				<dict><key>Track ID</key><integer>102</integer></dict>
			</array>
		</dict>
		<dict>
			<key>Name</key><string>Top Rated</string>
			<key>Playlist Persistent ID</key><string>P3</string>
			<key>Smart Info</key><data>AQEAAwAAAAI=</data>
			<key>Smart Criteria</key><data>U0xzdAABAAE=</data>
			<key>Playlist Items</key>
			<array><dict><key>Track ID</key><integer>101</integer></dict></array>
		</dict>
	</array>
</dict>
</plist>"#;

    fn write(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("djsugar-itunes-{name}-{}.xml", std::process::id()));
        std::fs::write(&path, XML).unwrap();
        path
    }

    fn names(lists: &[ImportedList]) -> Vec<String> {
        lists
            .iter()
            .map(|l| {
                let mut p = l.folders.clone();
                p.push(l.name.clone());
                p.join("/")
            })
            .collect()
    }

    #[test]
    fn reads_an_itunes_library() {
        let path = write("read");
        let lib = read(&path, &[PLAYLISTS], |_| true).unwrap();
        std::fs::remove_file(&path).ok();

        // The podcast and the audiobook stay out; the streamed song isn't
        // a file.
        assert_eq!(lib.tracks.len(), 2);
        let t = lib.tracks.iter().find(|t| t.title == "Glue").expect("Glue");
        assert!(matches!(&t.source, Source::File(p)
            if p == Path::new("/Users/dj/Music/Music/Media.localized/Music/Bicep/Bicep/03 Glue.mp3")));
        assert_eq!((t.artist.as_str(), t.album.as_str()), ("Bicep", "Bicep"));
        assert_eq!((t.rating, t.bpm, t.duration), (4, 120.0, 268.9));
        let i = &t.info;
        assert_eq!((i.play_count, i.track_number, i.disc_number), (7, 3, 1));
        assert_eq!((i.year.as_str(), i.comment.as_str()), ("2017", "nice"));
        assert_eq!(
            (i.grouping.as_str(), i.composer.as_str()),
            ("Warm up", "Matt McBriar")
        );
        assert_eq!(i.added, 1_649_498_400);
        assert_eq!(i.last_played, Some(1_672_957_800));
        assert!(!i.fixed_grid);

        // A rating worked out from the album isn't kept; Windows paths.
        let away = lib.tracks.iter().find(|t| t.title == "Away").unwrap();
        assert_eq!(away.rating, 0);
        assert!(
            matches!(&away.source, Source::File(p) if p == Path::new("C:/Users/dj/Music/away.m4a"))
        );

        // Folders within folders; the app's own lists, folders' own track
        // lists and (by default) smart playlists left out.
        assert_eq!(names(&lib.lists), ["Gigs/2023/Friday"]);
        let friday = &lib.lists[0].entries;
        assert_eq!(friday.len(), 3);
        assert_eq!(
            friday[0].path,
            PathBuf::from("/Users/dj/Music/Music/Media.localized/Music/Bicep/Bicep/03 Glue.mp3")
        );
        let s = friday[1].streamed.as_ref().expect("a placeholder");
        assert_eq!(
            (s.service.as_str(), s.title.as_str(), s.artist.as_str()),
            ("applemusic", "Streamed Song", "Z")
        );
        assert_eq!(s.duration, 200.0);
        assert_eq!(s.service_name(), "Apple Music");
        assert_eq!(friday[2].path, PathBuf::from("C:/Users/dj/Music/away.m4a"));
    }

    #[test]
    fn smart_playlists_when_ticked() {
        let path = write("smart");
        let lib = read(&path, &[SMART], |_| true).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(names(&lib.lists), ["Top Rated"]);
        assert_eq!(lib.lists[0].entries.len(), 1);
    }

    #[test]
    fn only_files_that_are_here() {
        let path = write("here");
        let lib = read(&path, &[PLAYLISTS], |p| p.starts_with("/Users")).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(lib.tracks.len(), 1);
        // The list still names the missing file.
        assert_eq!(lib.lists[0].entries.len(), 3);
    }

    #[test]
    fn previews_and_rejects_other_files() {
        let path = write("preview");
        assert_eq!(preview(&path).unwrap(), "3 tracks and 1 playlist.");
        std::fs::remove_file(&path).ok();
        let other =
            std::env::temp_dir().join(format!("djsugar-itunes-other-{}.xml", std::process::id()));
        std::fs::write(
            &other,
            "<plist version=\"1.0\"><dict><key>A</key><string>b</string></dict></plist>",
        )
        .unwrap();
        assert!(preview(&other).is_err());
        std::fs::remove_file(&other).ok();
    }

    #[test]
    fn finds_the_newest_library_xml() {
        let home = std::env::temp_dir().join(format!("djsugar-itunes-home-{}", std::process::id()));
        let all = candidates(&home);
        assert!(newest(&all).is_none());
        std::fs::create_dir_all(home.join("Music/iTunes")).unwrap();
        std::fs::write(home.join("Music/iTunes/iTunes Library.xml"), XML).unwrap();
        assert_eq!(
            newest(&all),
            Some(home.join("Music/iTunes/iTunes Library.xml"))
        );
        std::fs::remove_dir_all(home).ok();
    }
}
