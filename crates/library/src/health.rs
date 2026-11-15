//! Collection Health: checks that find what needs fixing in the collection
//! (audiobooks and podcasts, files outside the stage folders or the music
//! folders, tracks not
//! analysed, missing their tags (or with tags that say nothing), cover or
//! rating, low bitrates, files gone, long recordings), so it can be fixed in batches. Each check is also a
//! search (`is:notmusic`, `is:stray`…), and a track can be set to be
//! ignored by a check, so it stops showing up there.
//!
//! What the DJ decided is kept in the library: tracks ignored per check,
//! tracks taken out of the library (their files stay, and an import or a
//! scan doesn't bring them back), and which files have a cover and at
//! what bitrate.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use djsugar_engine::{Source, Track};
use rusqlite::{params, OptionalExtension};

use crate::cleanup::{self, Rules};
use crate::{now, row_id, track_id, Error, Library};

/// Tracks longer than this (in seconds) are recordings of whole sets or
/// long ambient pieces, rather than tracks to mix.
pub const LONG_TRACK: f64 = 20.0 * 60.0;

/// Lossy files under this bitrate (kb/s) can sound dull on a big system.
pub const LOW_BITRATE: u32 = 192;

/// Tracks in one folder sharing a title, from which they are taken to be
/// an audiobook's chapters.
const CHAPTERS: usize = 4;

pub(crate) const HEALTH_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS health_ignored (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    check_name TEXT NOT NULL,
    PRIMARY KEY (track_id, check_name)
);
CREATE TABLE IF NOT EXISTS removed (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    removed INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS covers (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    file_mtime INTEGER NOT NULL,
    has_cover INTEGER NOT NULL,
    bitrate INTEGER
);
";

/// What a look at a track's file found: whether it has a cover and its
/// bitrate (kb/s; 0 when the file doesn't say, `None` from before bitrates
/// were looked at), and the file's modification time then.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FileLook {
    pub mtime: i64,
    pub cover: bool,
    pub bitrate: Option<u32>,
}

/// Something Collection Health looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Check {
    NotMusic,
    Stray,
    Outside,
    NotAnalyzed,
    /// Tags left empty, or that say nothing ("Unknown Artist", "Track 01").
    MissingInfo,
    LowBitrate,
    NoCover,
    NoRating,
    Missing,
    Long,
    Duplicate,
    Junk,
    /// Tracks in one album's folder that don't share its album tag.
    AlbumTags,
}

impl Check {
    pub const ALL: [Check; 13] = [
        Check::NotMusic,
        Check::Stray,
        Check::Outside,
        Check::Duplicate,
        Check::NotAnalyzed,
        Check::MissingInfo,
        Check::LowBitrate,
        Check::Junk,
        Check::AlbumTags,
        Check::NoCover,
        Check::NoRating,
        Check::Missing,
        Check::Long,
    ];

    /// The name kept in the library and used in searches (`is:notmusic`).
    pub fn name(self) -> &'static str {
        match self {
            Check::NotMusic => "notmusic",
            Check::Stray => "stray",
            Check::Outside => "outside",
            Check::NotAnalyzed => "unanalyzed",
            Check::MissingInfo => "noinfo",
            Check::LowBitrate => "lowbitrate",
            Check::NoCover => "nocover",
            Check::NoRating => "unrated",
            Check::Missing => "missing",
            Check::Long => "long",
            Check::Duplicate => "duplicate",
            Check::Junk => "junk",
            Check::AlbumTags => "albumtags",
        }
    }

    /// The check a search word names, with a few other spellings.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "notmusic" | "nonmusic" | "audiobook" | "podcast" => Check::NotMusic,
            "stray" | "strays" => Check::Stray,
            "outside" | "unwatched" => Check::Outside,
            "unanalyzed" | "unanalysed" | "notanalyzed" => Check::NotAnalyzed,
            "noinfo" | "untagged" | "incomplete" | "poorinfo" | "metadata" => Check::MissingInfo,
            "lowbitrate" | "bitrate" | "lowquality" => Check::LowBitrate,
            "nocover" | "noart" => Check::NoCover,
            "unrated" => Check::NoRating,
            "missing" => Check::Missing,
            "long" => Check::Long,
            "duplicate" | "dupe" | "duplicates" => Check::Duplicate,
            "junk" | "messy" | "cleanup" => Check::Junk,
            "albumtags" | "albumtag" => Check::AlbumTags,
            _ => return None,
        })
    }

    /// What the Health page calls it.
    pub fn title(self) -> &'static str {
        match self {
            Check::NotMusic => "Not music",
            Check::Stray => "Strays",
            Check::Outside => "Outside folders",
            Check::NotAnalyzed => "Not analyzed",
            Check::MissingInfo => "Missing or poor metadata",
            Check::LowBitrate => "Low bitrate",
            Check::NoCover => "No cover",
            Check::NoRating => "No rating",
            Check::Missing => "Missing files",
            Check::Long => "Long tracks",
            Check::Duplicate => "Duplicates",
            Check::Junk => "Junk in names",
            Check::AlbumTags => "Album tags differ",
        }
    }

    /// One line on what it finds.
    pub fn about(self) -> &'static str {
        match self {
            Check::NotMusic => {
                "Audiobooks, podcasts and spoken word that came in with another library."
            }
            Check::Stray => "Files outside your Inbox, In Prep and Play Ready folders.",
            Check::Outside => "Files outside your music folders and stage folders.",
            Check::NotAnalyzed => "No tempo or key yet.",
            Check::MissingInfo => {
                "No artist, title, album, year or genre, or one that says nothing, like \"Unknown Artist\" or \"Track 01\"."
            }
            Check::LowBitrate => "Compressed files under 192 kb/s: they can sound dull on a big system.",
            Check::NoCover => "No picture in the file's tags or its folder.",
            Check::NoRating => "No stars yet.",
            Check::Missing => "The file isn't where your collection expects it.",
            Check::Long => "Over 20 minutes: recorded sets and long pieces.",
            Check::Duplicate => "The same track more than once.",
            Check::Junk => {
                "Track numbers, web sites, \"(Official Video)\", underscores, or a remix or guest in the title."
            }
            Check::AlbumTags => "Tracks in one album's folder without the same album tag.",
        }
    }

    /// Whether a track this check finds counts against the other checks:
    /// an audiobook isn't also "missing a cover", and its chapters, which
    /// share a title, aren't duplicates.
    fn needs_music(self) -> bool {
        !matches!(self, Check::NotMusic | Check::Missing)
    }

    /// The area of the Health page the check belongs to.
    pub fn area(self) -> Area {
        match self {
            Check::Missing | Check::LowBitrate => Area::Files,
            Check::NotMusic | Check::Stray | Check::Outside | Check::Long => Area::OutOfPlace,
            Check::Duplicate => Area::Duplicates,
            Check::MissingInfo | Check::Junk | Check::AlbumTags | Check::NoCover => Area::Info,
            Check::NotAnalyzed | Check::NoRating => Area::Prep,
        }
    }
}

/// The checks grouped by the question a DJ asks of a track, in the order
/// they are best fixed: files first (a missing track may be the better
/// copy), then what doesn't belong (audiobook chapters look like copies),
/// then copies (so nothing is edited twice), then tags, then prep.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Area {
    Files,
    OutOfPlace,
    Duplicates,
    Info,
    Prep,
}

impl Area {
    pub const ALL: [Area; 5] = [
        Area::Files,
        Area::OutOfPlace,
        Area::Duplicates,
        Area::Info,
        Area::Prep,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Area::Files => "Files",
            Area::OutOfPlace => "Out of place",
            Area::Duplicates => "Duplicates",
            Area::Info => "Info",
            Area::Prep => "Prep",
        }
    }

    /// The question the area answers.
    pub fn question(self) -> &'static str {
        match self {
            Area::Files => "Can it play?",
            Area::OutOfPlace => "Should it be here?",
            Area::Duplicates => "Same track twice?",
            Area::Info => "Can you find it?",
            Area::Prep => "Ready to play?",
        }
    }

    /// Its checks, in the order its page offers them.
    pub fn checks(self) -> Vec<Check> {
        let order = [
            Check::Missing,
            Check::LowBitrate,
            Check::NotMusic,
            Check::Long,
            Check::Stray,
            Check::Outside,
            Check::Duplicate,
            Check::MissingInfo,
            Check::Junk,
            Check::AlbumTags,
            Check::NoCover,
            Check::NotAnalyzed,
            Check::NoRating,
        ];
        order.into_iter().filter(|c| c.area() == self).collect()
    }

    /// Whether what it finds is a problem: Prep is a goal, so a track
    /// without stars still counts as clean.
    pub fn problem(self) -> bool {
        self != Area::Prep
    }
}

/// Whether a track looks like it isn't music, from where it is and its tags
/// alone (chapters are found across tracks, in [`Facts::new`]).
pub fn looks_like_not_music(t: &Track) -> bool {
    let genre = t.genre.trim().to_lowercase();
    if [
        "audiobook",
        "audiobooks",
        "audio book",
        "hörbuch",
        "hörbücher",
        "podcast",
        "podcasts",
        "speech",
        "spoken word",
        "spoken",
        "books & spoken",
        "books and spoken",
    ]
    .contains(&genre.as_str())
    {
        return true;
    }
    if t.album.to_lowercase().contains("abridged") {
        return true;
    }
    let title = t.title.trim().to_lowercase();
    if ["chapter ", "kapitel ", "capítulo ", "chapitre "]
        .iter()
        .any(|w| title.starts_with(w))
    {
        return true;
    }
    let Source::File(path) = &t.source else {
        return false;
    };
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ["m4b", "aax", "aa"].contains(&ext.as_str()) {
        return true;
    }
    let p = path.to_string_lossy().to_lowercase();
    [
        "com.apple.bkagentservice",
        "com.apple.podcasts",
        "/podcasts/",
        "/audiobooks/",
        "/audible/",
        "/movies/",
        "/voice memos/",
    ]
    .iter()
    .any(|w| p.contains(w))
}

/// Whether a tag is empty or says nothing: "Unknown Artist", "Untitled",
/// "Track 01", "-".
pub fn says_nothing(tag: &str) -> bool {
    let t = tag
        .trim()
        .trim_matches(['<', '>', '[', ']', '(', ')'])
        .to_lowercase();
    if t.chars().all(|c| !c.is_alphanumeric()) {
        return true;
    }
    if [
        "unknown",
        "unknown artist",
        "unknown album",
        "unknown title",
        "unknown genre",
        "untitled",
        "no title",
        "no artist",
        "n/a",
        "none",
        "artist",
        "title",
        "album",
        "other",
        "0",
    ]
    .contains(&t.as_str())
    {
        return true;
    }
    // "Track 01", "Audio Track 3", "Track01"
    let rest = t
        .strip_prefix("audio track")
        .or_else(|| t.strip_prefix("audiotrack"))
        .or_else(|| t.strip_prefix("track"));
    rest.is_some_and(|r| {
        let r = r.trim();
        !r.is_empty() && r.chars().all(|c| c.is_ascii_digit())
    })
}

/// What the checks go by beyond each track: the DJ's choices, what was
/// found across tracks, and which files have a cover.
#[derive(Debug, Default)]
pub struct Facts {
    /// Ids of tracks that look like they aren't music.
    not_music: HashSet<usize>,
    /// Ids of tracks each check is told to leave alone.
    ignored: HashMap<Check, HashSet<usize>>,
    /// Whether each file has a cover, for those looked at.
    covers: HashMap<usize, bool>,
    /// Each file's bitrate in kb/s, for those looked at.
    bitrates: HashMap<usize, u32>,
    /// The DJ has set up the stage folders, so a track outside them is a stray.
    stages: bool,
    /// The music folders and stage folders: a file in none of them is outside.
    folders: Vec<PathBuf>,
    /// The clean-up rules turned on, for [`Check::Junk`].
    rules: Rules,
    /// The album tag each track in the albums folder should have, by id,
    /// for those that have another (see [`album_folders`]).
    album_tags: HashMap<usize, String>,
}

impl Facts {
    /// Whether any check of an area where findings are problems finds the
    /// track: such a track isn't clean.
    pub fn has_problem(&self, t: &Track, duplicates: &HashSet<usize>) -> bool {
        Check::ALL
            .into_iter()
            .any(|c| c.area().problem() && self.finds(c, t, duplicates))
    }

    pub fn new<'a>(
        tracks: impl IntoIterator<Item = &'a Track>,
        ignored: HashMap<Check, HashSet<usize>>,
        covers: HashMap<usize, bool>,
        stages: bool,
    ) -> Self {
        let mut not_music = HashSet::new();
        // Chapters: several tracks in one folder sharing a title.
        let mut titles: HashMap<(PathBuf, String), Vec<usize>> = HashMap::new();
        for t in tracks {
            if looks_like_not_music(t) {
                not_music.insert(t.id);
            }
            if let Source::File(path) = &t.source {
                let title = t.title.trim().to_lowercase();
                if !title.is_empty() {
                    let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
                    titles.entry((dir, title)).or_default().push(t.id);
                }
            }
        }
        for ids in titles.into_values().filter(|ids| ids.len() >= CHAPTERS) {
            not_music.extend(ids);
        }
        Facts {
            not_music,
            ignored,
            covers,
            bitrates: HashMap::new(),
            stages,
            folders: Vec::new(),
            rules: Rules::default(),
            album_tags: HashMap::new(),
        }
    }

    /// The albums in the albums folder, for [`Check::AlbumTags`].
    pub fn with_albums<'a>(
        mut self,
        albums: &Path,
        tracks: impl IntoIterator<Item = &'a Track>,
    ) -> Self {
        self.set_albums(albums, tracks);
        self
    }

    /// Looks at the albums in the albums folder again, after their tags
    /// changed.
    pub fn set_albums<'a>(&mut self, albums: &Path, tracks: impl IntoIterator<Item = &'a Track>) {
        self.album_tags.clear();
        let tracks: Vec<&Track> = tracks.into_iter().collect();
        let tags: HashMap<usize, &str> = tracks.iter().map(|t| (t.id, t.album.trim())).collect();
        for album in album_folders(albums, tracks.iter().copied()).into_values() {
            for &id in &album.tracks {
                if tags.get(&id).copied() != Some(album.name.as_str()) {
                    self.album_tags.insert(id, album.name.clone());
                }
            }
        }
    }

    /// The album tag `id` should have, when it has another.
    pub fn album_tag(&self, id: usize) -> Option<&str> {
        self.album_tags.get(&id).map(String::as_str)
    }

    /// The file's bitrate (kb/s), once the cover scan has read it.
    pub fn bitrate(&self, id: usize) -> Option<u32> {
        self.bitrates.get(&id).copied().filter(|&b| b > 0)
    }

    /// What the clean-up rules would change in `t`'s names.
    pub fn junk(&self, t: &Track) -> Vec<cleanup::Change> {
        cleanup::changes(t, &self.rules)
    }

    /// The music folders and stage folders, for [`Check::Outside`]. With
    /// none, that check finds nothing.
    pub fn with_folders(mut self, folders: Vec<PathBuf>) -> Self {
        self.folders = folders;
        self
    }

    /// The clean-up rules [`Check::Junk`] goes by.
    pub fn with_rules(mut self, rules: Rules) -> Self {
        self.rules = rules;
        self
    }

    /// Whether `check` finds `t`. `duplicates` are the ids of tracks that
    /// are in the library more than once.
    pub fn finds(&self, check: Check, t: &Track, duplicates: &HashSet<usize>) -> bool {
        if self.ignored.get(&check).is_some_and(|i| i.contains(&t.id)) {
            return false;
        }
        if check.needs_music() && (t.info.missing || self.not_music(t)) {
            return false;
        }
        match check {
            Check::NotMusic => self.not_music(t),
            Check::Stray => self.stages && t.info.stage.is_empty(),
            Check::Outside => match &t.source {
                Source::File(path) => {
                    !self.folders.is_empty() && !self.folders.iter().any(|d| path.starts_with(d))
                }
                _ => false,
            },
            Check::NotAnalyzed => t.bpm <= 0.0 || t.key.is_empty(),
            Check::MissingInfo => [&t.artist, &t.title, &t.album, &t.info.year, &t.genre]
                .iter()
                .any(|f| says_nothing(f)),
            Check::LowBitrate => self
                .bitrates
                .get(&t.id)
                .is_some_and(|&b| b > 0 && b < LOW_BITRATE),
            Check::NoCover => self.covers.get(&t.id) == Some(&false),
            Check::NoRating => t.rating == 0,
            Check::Missing => t.info.missing,
            Check::Long => t.duration > LONG_TRACK,
            Check::Duplicate => duplicates.contains(&t.id),
            Check::Junk => !cleanup::changes(t, &self.rules).is_empty(),
            Check::AlbumTags => self.album_tags.contains_key(&t.id),
        }
    }

    /// Whether `t` looks like it isn't music and the DJ hasn't said it is.
    fn not_music(&self, t: &Track) -> bool {
        self.not_music.contains(&t.id)
            && !self
                .ignored
                .get(&Check::NotMusic)
                .is_some_and(|i| i.contains(&t.id))
    }

    /// Whether `t`'s file has been looked at for a cover.
    pub fn cover_known(&self, id: usize) -> bool {
        self.covers.contains_key(&id)
    }

    /// Whether a track has a cover, once looked at.
    pub fn has_cover(&self, id: usize) -> Option<bool> {
        self.covers.get(&id).copied()
    }

    /// Notes whether a track has a cover.
    pub fn set_cover(&mut self, id: usize, has: bool) {
        self.covers.insert(id, has);
    }

    /// Notes a track's bitrate in kb/s.
    pub fn set_bitrate(&mut self, id: usize, kbps: u32) {
        self.bitrates.insert(id, kbps);
    }

    /// The checks told to leave `id` alone, in [`Check::ALL`] order.
    pub fn ignored_by(&self, id: usize) -> Vec<Check> {
        Check::ALL
            .iter()
            .copied()
            .filter(|c| self.ignored.get(c).is_some_and(|i| i.contains(&id)))
            .collect()
    }

    /// Whether any check was told to leave `id` alone.
    pub fn is_ignored(&self, id: usize) -> bool {
        self.ignored.values().any(|i| i.contains(&id))
    }

    /// Tells `check` to leave these tracks alone, or (with `on` false) to
    /// look at them again.
    pub fn set_ignored(&mut self, check: Check, ids: &[usize], on: bool) {
        let set = self.ignored.entry(check).or_default();
        for id in ids {
            if on {
                set.insert(*id);
            } else {
                set.remove(id);
            }
        }
    }
}

/// One album in the albums folder: a folder with tracks right in it.
#[derive(Clone, Debug, PartialEq)]
pub struct AlbumFolder {
    /// Its tracks' ids.
    pub tracks: Vec<usize>,
    /// The album tag they should share: the one most of them have, else
    /// the folder's name.
    pub name: String,
}

/// The albums under `albums`, by folder: each folder that has tracks right
/// in it is one album, whether it is `Album` or `Artist/Album`.
pub fn album_folders<'a>(
    albums: &Path,
    tracks: impl IntoIterator<Item = &'a Track>,
) -> HashMap<PathBuf, AlbumFolder> {
    let mut by_dir: HashMap<PathBuf, Vec<&Track>> = HashMap::new();
    for t in tracks {
        let Source::File(path) = &t.source else {
            continue;
        };
        let Some(dir) = path
            .parent()
            .filter(|d| d.starts_with(albums) && *d != albums)
        else {
            continue;
        };
        by_dir.entry(dir.to_path_buf()).or_default().push(t);
    }
    by_dir
        .into_iter()
        .map(|(dir, tracks)| {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for t in &tracks {
                let tag = t.album.trim();
                if !tag.is_empty() {
                    *counts.entry(tag).or_default() += 1;
                }
            }
            // The most common tag; between equals, the first in name order.
            let name = counts
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
                .map(|(tag, _)| tag.to_string())
                .unwrap_or_else(|| {
                    dir.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
            let album = AlbumFolder {
                tracks: tracks.iter().map(|t| t.id).collect(),
                name,
            };
            (dir, album)
        })
        .collect()
}

/// The artist and title in a file name such as "01 - Artist - Title
/// (Remix).mp3": a leading track number is left out, and what comes
/// before the first " - " is the artist. `None` when there is no " - ".
pub fn from_file_name(path: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_string_lossy().replace('_', " ");
    let mut rest = stem.trim();
    // "01 ", "01. ", "01 - ", "1-02 "
    let digits = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(0);
    // A number that starts a name ("808 State") isn't a track number: those
    // have a leading zero, two digits at most, or a "." or " - " after them.
    let marked = rest[digits..].starts_with(['.', ')']) || rest[digits..].starts_with(" - ");
    let number = &rest[..digits];
    if digits > 0 && digits <= 5 && (marked || number.starts_with('0') || digits <= 2) {
        let after = rest[digits..].trim_start_matches(['.', ' ', ')']);
        let after = after.strip_prefix("- ").unwrap_or(after);
        if after.len() < rest.len() && !after.is_empty() {
            rest = after.trim();
        }
    }
    let (artist, title) = rest.split_once(" - ")?;
    let (artist, title) = (artist.trim(), title.trim());
    (!artist.is_empty() && !title.is_empty()).then(|| (artist.to_string(), title.to_string()))
}

impl Library {
    /// The tracks each check is told to leave alone.
    pub fn health_ignored(&self) -> Result<HashMap<Check, HashSet<usize>>, Error> {
        let mut out: HashMap<Check, HashSet<usize>> = HashMap::new();
        let mut q = self
            .db
            .prepare("SELECT track_id, check_name FROM health_ignored")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, name) = row?;
            if let Some(check) = Check::from_name(&name) {
                out.entry(check).or_default().insert(track_id(id));
            }
        }
        Ok(out)
    }

    /// Tells `check` to leave these tracks alone, or (with `on` false) to
    /// look at them again.
    pub fn set_health_ignored(
        &mut self,
        check: Check,
        ids: &[usize],
        on: bool,
    ) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        {
            let mut add = tx.prepare(
                "INSERT OR IGNORE INTO health_ignored (track_id, check_name) VALUES (?1, ?2)",
            )?;
            let mut take =
                tx.prepare("DELETE FROM health_ignored WHERE track_id = ?1 AND check_name = ?2")?;
            for row in ids.iter().filter_map(|&id| row_id(id)) {
                if on {
                    add.execute(params![row, check.name()])?;
                } else {
                    take.execute(params![row, check.name()])?;
                }
            }
        }
        tx.commit()
    }

    /// Takes tracks out of the library. Their files stay where they are,
    /// and their rows are kept (so lists and history still know them),
    /// but they no longer show, and a scan or an import doesn't bring them
    /// back; adding the file again does.
    pub fn remove_tracks(&mut self, ids: &[usize]) -> Result<usize, Error> {
        let tx = self.db.transaction()?;
        let mut n = 0;
        {
            let mut add =
                tx.prepare("INSERT OR IGNORE INTO removed (track_id, removed) VALUES (?1, ?2)")?;
            let at = now();
            for row in ids.iter().filter_map(|&id| row_id(id)) {
                n += add.execute(params![row, at])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// Brings back tracks taken out with [`Library::remove_tracks`]: these,
    /// or all of them when `ids` is `None`. Returns how many came back.
    pub fn restore_tracks(&mut self, ids: Option<&[usize]>) -> Result<usize, Error> {
        let rows: Vec<i64> = match ids {
            None => {
                let mut q = self.db.prepare("SELECT track_id FROM removed")?;
                let rows = q.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
                rows
            }
            Some(ids) => ids.iter().filter_map(|&id| row_id(id)).collect(),
        };
        let tx = self.db.transaction()?;
        let mut n = 0;
        for row in rows {
            n += tx.execute("DELETE FROM removed WHERE track_id = ?1", [row])?;
            // A merged copy (see `dupes`) whose file hasn't gone to
            // quarantine yet stays where it is, with no stage.
            tx.execute(
                "DELETE FROM file_moves WHERE track_id = ?1 AND to_stage = 'quarantine'
                   AND done IS NULL AND undone IS NULL",
                [row],
            )?;
            tx.execute(
                "UPDATE tracks SET stage = '' WHERE id = ?1 AND stage = 'quarantine'",
                [row],
            )?;
        }
        tx.commit()?;
        Ok(n)
    }

    /// How many tracks have been taken out of the library.
    pub fn removed_count(&self) -> Result<usize, Error> {
        self.db
            .query_row("SELECT COUNT(*) FROM removed", [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
    }

    /// What was found in each track's file when it was last looked at,
    /// with the file's modification time then (to look again once it
    /// changes).
    pub fn covers(&self) -> Result<HashMap<usize, FileLook>, Error> {
        let mut q = self
            .db
            .prepare("SELECT track_id, file_mtime, has_cover, bitrate FROM covers")?;
        let rows = q.query_map([], |r| {
            Ok((
                track_id(r.get(0)?),
                FileLook {
                    mtime: r.get(1)?,
                    cover: r.get(2)?,
                    bitrate: r.get(3)?,
                },
            ))
        })?;
        rows.collect()
    }

    /// Notes what was found in tracks' files, by track id.
    pub fn save_covers(&mut self, found: &[(usize, FileLook)]) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        {
            let mut put = tx.prepare(
                "INSERT INTO covers (track_id, file_mtime, has_cover, bitrate)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(track_id) DO UPDATE SET file_mtime = ?2, has_cover = ?3, bitrate = ?4",
            )?;
            for (id, look) in found {
                if let Some(row) = row_id(*id) {
                    put.execute(params![row, look.mtime, look.cover, look.bitrate])?;
                }
            }
        }
        tx.commit()
    }

    /// Points a track at its file's new place (a missing file found
    /// elsewhere). A scan may have added the file there as a track of its
    /// own already: that one gives way if nothing was done with it yet (no
    /// lists, cues, plays or rating), so the track keeps its history; else
    /// it is left to the duplicates check and this fails. The track's
    /// stage is cleared, for [`Library::assign_stages`] to give it the one
    /// of its new folder.
    pub fn relink(&mut self, id: usize, path: &Path) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        let p = crate::path_text(path);
        let tx = self.db.transaction()?;
        let other: Option<(i64, bool)> = tx
            .query_row(
                "SELECT id,
                        EXISTS (SELECT 1 FROM playlist_tracks WHERE track_id = tracks.id)
                        OR EXISTS (SELECT 1 FROM cues WHERE track_id = tracks.id)
                        OR play_count > 0 OR rating > 0
                 FROM tracks WHERE path = ?1",
                [&p],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((other, used)) = other {
            if used {
                return Err(Error::QueryReturnedNoRows);
            }
            tx.execute("DELETE FROM tracks WHERE id = ?1", [other])?;
        }
        tx.execute(
            "UPDATE tracks SET path = ?2, file_mtime = 0, stage = '' WHERE id = ?1",
            params![row, p],
        )?;
        tx.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::file_track;

    fn track(id: usize, path: &str, title: &str) -> Track {
        let mut t = file_track(path, title);
        t.id = id;
        t
    }

    #[test]
    fn an_album_folder_shares_the_tag_most_of_its_tracks_have() {
        let album = |id, path: &str, tag: &str| {
            let mut t = track(id, path, &format!("Song {id}"));
            t.album = tag.into();
            t
        };
        let tracks = [
            album(1, "/m/Albums/Air/Moon Safari/01.flac", "Moon Safari"),
            album(2, "/m/Albums/Air/Moon Safari/02.flac", "Moon Safari"),
            album(
                3,
                "/m/Albums/Air/Moon Safari/03.flac",
                "Moon Safari (Remaster)",
            ),
            album(4, "/m/Albums/Untagged/01.mp3", ""),
            album(5, "/m/Albums/loose.mp3", ""),
            album(6, "/m/DB/Library/x.mp3", "Other"),
        ];
        let folders = album_folders(Path::new("/m/Albums"), &tracks);
        assert_eq!(folders.len(), 2);
        assert_eq!(
            folders[Path::new("/m/Albums/Air/Moon Safari")],
            AlbumFolder {
                tracks: vec![1, 2, 3],
                name: "Moon Safari".into()
            }
        );
        // No tags at all: the folder's name.
        assert_eq!(folders[Path::new("/m/Albums/Untagged")].name, "Untagged");

        let facts = Facts::new(&tracks, HashMap::new(), HashMap::new(), false)
            .with_albums(Path::new("/m/Albums"), &tracks);
        let found: Vec<usize> = tracks
            .iter()
            .filter(|t| facts.finds(Check::AlbumTags, t, &HashSet::new()))
            .map(|t| t.id)
            .collect();
        assert_eq!(found, vec![3, 4]);
        assert_eq!(facts.album_tag(3), Some("Moon Safari"));
    }

    #[test]
    fn finds_audiobooks_by_place_genre_and_chapters() {
        let book = track(
            1,
            "/Users/s/Library/Containers/com.apple.BKAgentService/b.mp3",
            "Breath",
        );
        assert!(looks_like_not_music(&book));
        let mut genre = track(2, "/m/a.mp3", "Talk");
        genre.genre = "Audiobook".into();
        assert!(looks_like_not_music(&genre));
        let m4b = track(3, "/m/book.m4b", "Book");
        assert!(looks_like_not_music(&m4b));
        let song = track(4, "/m/DB/Library/x.mp3", "Song");
        assert!(!looks_like_not_music(&song));

        let mut tracks: Vec<Track> = (0..5)
            .map(|i| track(10 + i, &format!("/m/Breath/{i:02}.mp3"), "Breath"))
            .collect();
        tracks.push(song.clone());
        let facts = Facts::new(&tracks, HashMap::new(), HashMap::new(), true);
        let none = HashSet::new();
        assert!(facts.finds(Check::NotMusic, &tracks[0], &none));
        assert!(!facts.finds(Check::NotMusic, &song, &none));
        // A chapter isn't also missing its rating.
        assert!(!facts.finds(Check::NoRating, &tracks[0], &none));
        assert!(facts.finds(Check::NoRating, &song, &none));
    }

    #[test]
    fn ignoring_takes_a_track_off_a_check() {
        let mut t = track(1, "/m/DB/Library/x.mp3", "Song");
        t.genre = "Podcast".into();
        let mut facts = Facts::new([&t], HashMap::new(), HashMap::new(), true);
        let none = HashSet::new();
        assert!(facts.finds(Check::NotMusic, &t, &none));
        assert!(!facts.finds(Check::NoRating, &t, &none));
        // Marked as music: no longer found, and the other checks look at it.
        facts.set_ignored(Check::NotMusic, &[1], true);
        assert!(!facts.finds(Check::NotMusic, &t, &none));
        assert!(facts.finds(Check::NoRating, &t, &none));
        facts.set_ignored(Check::NoRating, &[1], true);
        assert!(!facts.finds(Check::NoRating, &t, &none));
        assert!(facts.is_ignored(1));
        assert_eq!(facts.ignored_by(1), [Check::NotMusic, Check::NoRating]);
        assert!(!facts.is_ignored(2));
    }

    #[test]
    fn strays_and_covers() {
        let mut t = track(1, "/Users/s/Downloads/x.mp3", "Song");
        let none = HashSet::new();
        let facts = Facts::new([&t], HashMap::new(), HashMap::new(), false);
        assert!(
            !facts.finds(Check::Stray, &t, &none),
            "no stage folders yet"
        );
        let mut facts = Facts::new([&t], HashMap::new(), HashMap::new(), true);
        assert!(facts.finds(Check::Stray, &t, &none));
        t.info.stage = "inbox".into();
        assert!(!facts.finds(Check::Stray, &t, &none));
        assert!(!facts.finds(Check::NoCover, &t, &none), "not looked at yet");
        facts.set_cover(1, false);
        assert!(facts.finds(Check::NoCover, &t, &none));
    }

    #[test]
    fn poor_metadata_and_low_bitrates() {
        let mut t = track(1, "/m/DB/Library/x.mp3", "Song");
        t.artist = "Bonobo".into();
        t.album = "Migration".into();
        t.info.year = "2020".into();
        t.genre = "House".into();
        let none = HashSet::new();
        let mut facts = Facts::new([&t], HashMap::new(), HashMap::new(), true);
        assert!(!facts.finds(Check::MissingInfo, &t, &none));
        for poor in [
            "Unknown Artist",
            "<unknown>",
            "-",
            "Track 01",
            "Audio Track 3",
        ] {
            t.artist = poor.into();
            assert!(facts.finds(Check::MissingInfo, &t, &none), "{poor}");
        }
        t.artist = "Track Star".into();
        assert!(!facts.finds(Check::MissingInfo, &t, &none));
        assert!(!facts.finds(Check::LowBitrate, &t, &none), "not read yet");
        facts.set_bitrate(1, 128);
        assert!(facts.finds(Check::LowBitrate, &t, &none));
        facts.set_bitrate(1, 256);
        assert!(!facts.finds(Check::LowBitrate, &t, &none));
        assert_eq!(Check::from_name("lowbitrate"), Some(Check::LowBitrate));
    }

    #[test]
    fn files_outside_the_folders() {
        let inside = track(1, "/m/DB/Library/x.mp3", "Song");
        let outside = track(2, "/Users/s/Downloads/y.mp3", "Song");
        let none = HashSet::new();
        let facts = Facts::new([&inside, &outside], HashMap::new(), HashMap::new(), false);
        assert!(
            !facts.finds(Check::Outside, &outside, &none),
            "no folders yet"
        );
        let facts = facts.with_folders(vec![PathBuf::from("/m/DB"), PathBuf::from("/m/Inbox")]);
        assert!(!facts.finds(Check::Outside, &inside, &none));
        assert!(facts.finds(Check::Outside, &outside, &none));
        assert_eq!(Check::from_name("outside"), Some(Check::Outside));
    }

    #[test]
    fn reads_artist_and_title_from_file_names() {
        let f = |s: &str| from_file_name(Path::new(s));
        assert_eq!(
            f("/m/Desert Dwellers - Bodhi Mandala (Drumspyder Remix).mp3"),
            Some((
                "Desert Dwellers".into(),
                "Bodhi Mandala (Drumspyder Remix)".into()
            ))
        );
        assert_eq!(
            f("/m/01 - Bicep - Glue.flac"),
            Some(("Bicep".into(), "Glue".into()))
        );
        assert_eq!(
            f("/m/02. Lane_8 - Fingerprint.mp3"),
            Some(("Lane 8".into(), "Fingerprint".into()))
        );
        assert_eq!(f("/m/track 7.mp3"), None);
        assert_eq!(
            f("/m/808 State - Pacific.mp3").map(|a| a.0),
            Some("808 State".into())
        );
    }

    #[test]
    fn removed_tracks_stay_out_until_restored() {
        let mut lib = Library::in_memory().unwrap();
        let added = lib
            .add_files(&[file_track("/m/a.mp3", "One"), file_track("/m/b.mp3", "Two")])
            .unwrap();
        let ids: Vec<usize> = added.iter().map(|t| t.id).collect();
        assert_eq!(lib.remove_tracks(&ids[..1]).unwrap(), 1);
        let left = lib.all_tracks(|_| true).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].title, "Two");
        assert_eq!(lib.removed_count().unwrap(), 1);
        // A scan finds the file again, but it stays out.
        lib.sync(&[file_track("/m/a.mp3", "One")]).unwrap();
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 1);
        // Adding the file by hand brings it back.
        lib.add_files(&[file_track("/m/a.mp3", "One")]).unwrap();
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 2);
        lib.remove_tracks(&ids).unwrap();
        assert_eq!(lib.restore_tracks(None).unwrap(), 2);
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 2);
    }

    #[test]
    fn relinking_takes_over_a_fresh_scan_of_the_moved_file() {
        let mut lib = Library::in_memory().unwrap();
        let old = lib.add_file(&file_track("/m/old/a.mp3", "One")).unwrap();
        lib.set_rating(old.id, 4).unwrap();
        // The file moved, and a scan found it at its new place.
        lib.sync(&[file_track("/m/new/a.mp3", "One")]).unwrap();
        assert_eq!(lib.all_tracks(|_| true).unwrap().len(), 2);
        lib.relink(old.id, Path::new("/m/new/a.mp3")).unwrap();
        let left = lib.all_tracks(|_| true).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!((left[0].id, left[0].rating), (old.id, 4));
        // A track at the new place with ratings of its own stays.
        let other = lib.add_file(&file_track("/m/b.mp3", "Two")).unwrap();
        let moved = lib.add_file(&file_track("/m/c.mp3", "Two")).unwrap();
        lib.set_rating(moved.id, 3).unwrap();
        assert!(lib.relink(other.id, Path::new("/m/c.mp3")).is_err());
    }

    #[test]
    fn keeps_ignores_and_covers() {
        let mut lib = Library::in_memory().unwrap();
        let t = lib.add_file(&file_track("/m/a.mp3", "One")).unwrap();
        lib.set_health_ignored(Check::NoCover, &[t.id], true)
            .unwrap();
        let ignored = lib.health_ignored().unwrap();
        assert!(ignored[&Check::NoCover].contains(&t.id));
        lib.set_health_ignored(Check::NoCover, &[t.id], false)
            .unwrap();
        assert!(lib.health_ignored().unwrap().is_empty());
        let look = FileLook {
            mtime: 42,
            cover: true,
            bitrate: Some(320),
        };
        lib.save_covers(&[(t.id, look)]).unwrap();
        assert_eq!(lib.covers().unwrap()[&t.id], look);
    }
    #[test]
    fn every_check_is_in_one_area() {
        let mut all: Vec<Check> = Area::ALL.iter().flat_map(|a| a.checks()).collect();
        assert_eq!(all.len(), Check::ALL.len());
        all.dedup();
        assert_eq!(all.len(), Check::ALL.len());
        assert!(Check::ALL.iter().all(|c| c.area().checks().contains(c)));
        assert_eq!(Area::Files.checks()[0], Check::Missing);
        assert!(!Area::Prep.problem());
    }

    #[test]
    fn a_missing_file_is_a_problem() {
        let mut gone = track(1, "/m/a.mp3", "Song");
        gone.info.missing = true;
        let tracks = [gone];
        let facts = Facts::new(&tracks, HashMap::new(), HashMap::new(), false);
        assert!(facts.has_problem(&tracks[0], &HashSet::new()));
    }
}
