//! The stages a track goes through on its way into the collection, each
//! with its folder on disk:
//!
//! - **Inbox**: anything that arrives in one of the watched folders
//!   (Downloads, a Soulseek folder, the purchase inbox).
//! - **Triage**: tracks the DJ picked from the inbox, being prepared:
//!   tags, cover, rating, tempo and key. Subfolders of the triage folder
//!   are *triage crates* (e.g. "Yoga STG"), tracks sorted by where they
//!   will go.
//! - **Collection**: prepared tracks. Subfolders are kept as they are.
//!   The albums folder, wherever it is, counts as part of the collection.
//!
//! A track's stage is kept in the library, so it can change before its
//! file moves (while the DJ still uses another app that points at the old
//! place, file moves wait in a queue).

use std::path::{Path, PathBuf};

use djsugar_engine::Track;

/// A step on the way into the collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Inbox,
    Triage,
    Collection,
}

impl Stage {
    pub const ALL: [Stage; 3] = [Stage::Inbox, Stage::Triage, Stage::Collection];

    /// The name kept in the library and used in searches (`stage:triage`).
    /// The titles changed since (Triage is In Prep, Collection is Play
    /// Ready), but these stay so saved libraries and searches keep working.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Inbox => "inbox",
            Stage::Triage => "triage",
            Stage::Collection => "collection",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|st| st.name() == s)
    }

    /// A stage as typed after `stage:`: its stored name, or its title
    /// without spaces (`stage:prep`, `stage:playready`).
    pub fn from_search(s: &str) -> Option<Self> {
        match s {
            "prep" | "inprep" => Some(Stage::Triage),
            "playready" => Some(Stage::Collection),
            _ => Self::from_name(s),
        }
    }

    /// What the browser calls it.
    pub fn title(self) -> &'static str {
        match self {
            Stage::Inbox => "Inbox",
            Stage::Triage => "In Prep",
            Stage::Collection => "Play Ready",
        }
    }
}

/// The folders the stages live in, as the DJ set them in Settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Folders {
    /// Watched for new music; any number.
    pub inbox: Vec<PathBuf>,
    pub triage: Option<PathBuf>,
    pub collection: Option<PathBuf>,
    /// Albums, one folder each: their tracks are in the collection, but
    /// stay where they are.
    pub albums: Option<PathBuf>,
}

impl Folders {
    /// Whether the DJ has set up the stages (the triage and collection
    /// folders; the inbox can be empty).
    pub fn is_set(&self) -> bool {
        self.triage.is_some() && self.collection.is_some()
    }

    /// Every stage folder, for scanning.
    pub fn all(&self) -> Vec<&Path> {
        self.inbox
            .iter()
            .chain(&self.triage)
            .chain(&self.collection)
            .chain(&self.albums)
            .map(PathBuf::as_path)
            .collect()
    }

    pub fn folder(&self, stage: Stage) -> Option<&Path> {
        match stage {
            Stage::Triage => self.triage.as_deref(),
            Stage::Collection => self.collection.as_deref(),
            Stage::Inbox => self.inbox.first().map(PathBuf::as_path),
        }
    }

    /// The stage a file belongs to by where it is: the deepest stage folder
    /// it is in, so a triage folder inside the collection's folder (or an
    /// inbox inside either) still counts as its own stage.
    pub fn stage_of(&self, path: &Path) -> Option<Stage> {
        let mut best: Option<(usize, Stage)> = None;
        let mut consider = |dir: &Path, stage| {
            let depth = dir.components().count();
            if path.starts_with(dir) && best.is_none_or(|(d, _)| depth > d) {
                best = Some((depth, stage));
            }
        };
        for dir in &self.inbox {
            consider(dir, Stage::Inbox);
        }
        if let Some(dir) = &self.triage {
            consider(dir, Stage::Triage);
        }
        if let Some(dir) = &self.collection {
            consider(dir, Stage::Collection);
        }
        if let Some(dir) = &self.albums {
            consider(dir, Stage::Collection);
        }
        best.map(|(_, s)| s)
    }

    /// The subfolder of `stage`'s folder that `path` is in (a triage crate
    /// such as "Yoga STG", or "Albums" in the collection), if it isn't
    /// right in the stage's folder.
    pub fn crate_of(&self, stage: Stage, path: &Path) -> Option<String> {
        let rest = path.strip_prefix(self.folder(stage)?).ok()?;
        let mut parts = rest.components();
        let first = parts.next()?;
        // A file right in the folder has nothing after its name.
        parts.next()?;
        Some(first.as_os_str().to_string_lossy().into_owned())
    }

    /// The subfolders of `stage`'s folder, sorted by name: the triage
    /// crates, or the collection's own folders.
    pub fn crates(&self, stage: Stage) -> Vec<String> {
        let Some(dir) = self.folder(stage) else {
            return Vec::new();
        };
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| !n.starts_with('.') && !n.starts_with('_'))
                    .collect()
            })
            .unwrap_or_default();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }
}

/// A clean file name for `track`: "Artist - Title (Remix).ext", from its
/// tags. None when the tags don't have both an artist and a title, so a
/// file is never renamed to something worse.
pub fn clean_name(track: &Track, path: &Path) -> Option<String> {
    let artist = track.artist.trim();
    let title = track.title.trim();
    if artist.is_empty() || title.is_empty() {
        return None;
    }
    let remix = track.info.remix.trim();
    let mut name = format!("{artist} - {title}");
    if !remix.is_empty() && !title.to_lowercase().contains(&remix.to_lowercase()) {
        name.push_str(&format!(" ({remix})"));
    }
    let name = safe(&name);
    if name.is_empty() {
        return None;
    }
    Some(match path.extension() {
        Some(ext) => format!("{name}.{}", ext.to_string_lossy().to_lowercase()),
        None => name,
    })
}

/// `s` as a file name on any system: no path separators or characters
/// Windows and macOS refuse, spaces collapsed, no dots at the end.
fn safe(s: &str) -> String {
    let replaced: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '|' => '-',
            '*' | '?' | '"' | '<' | '>' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_end_matches(['.', ' ']);
    // Long names fail on some file systems; 200 bytes leaves room.
    let mut end = trimmed.len().min(200);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    trimmed[..end].trim_end().to_string()
}

/// Where `path` goes when it moves into `stage` (into the subfolder
/// `crate_name` when given), named `name`, or its own name. A file already
/// there gets " (2)", " (3)"… added, as `taken` says.
pub fn destination(
    folders: &Folders,
    stage: Stage,
    crate_name: Option<&str>,
    path: &Path,
    name: Option<&str>,
    taken: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let mut dir = folders.folder(stage)?.to_path_buf();
    if let Some(c) = crate_name.map(safe).filter(|c| !c.is_empty()) {
        dir.push(c);
    }
    let name = match name {
        Some(n) => n.to_string(),
        None => path.file_name()?.to_string_lossy().into_owned(),
    };
    let first = dir.join(&name);
    if first == path || !taken(&first) {
        return Some(first);
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    (2..1000)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| p == path || !taken(p))
}

/// Where an album moves to in the albums folder: `Artist/Album`, with
/// " (2)" and so on after the album when a folder of that name is
/// `taken`. None without an album name.
pub fn album_destination(
    albums: &Path,
    artist: &str,
    album: &str,
    taken: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let album = safe(album);
    if album.is_empty() {
        return None;
    }
    let artist = Some(safe(artist)).filter(|a| !a.is_empty());
    let dir = artist.map_or_else(|| albums.to_path_buf(), |a| albums.join(a));
    std::iter::once(dir.join(&album))
        .chain((2..1000).map(|n| dir.join(format!("{album} ({n})"))))
        .find(|p| !taken(p))
}

/// What basic preparation a track has, as Triage shows it. A track with
/// all of these can go into the collection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prep {
    /// Artist and title.
    pub tags: bool,
    pub rating: bool,
    /// Tempo and key.
    pub analysed: bool,
}

impl Prep {
    pub fn of(t: &Track) -> Self {
        Prep {
            tags: !t.artist.trim().is_empty() && !t.title.trim().is_empty(),
            rating: t.rating > 0,
            analysed: t.bpm > 0.0 && !t.key.is_empty(),
        }
    }

    pub fn done(self) -> bool {
        self.tags && self.rating && self.analysed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::Source;

    #[test]
    fn albums_go_under_their_artist() {
        let root = Path::new("/u/Albums");
        let none = |_: &Path| false;
        assert_eq!(
            album_destination(root, "Air", "Moon Safari", none),
            Some(PathBuf::from("/u/Albums/Air/Moon Safari"))
        );
        assert_eq!(
            album_destination(root, "AC/DC", "Back in Black", none),
            Some(PathBuf::from("/u/Albums/AC-DC/Back in Black"))
        );
        let first = |p: &Path| p == Path::new("/u/Albums/Air/Moon Safari");
        assert_eq!(
            album_destination(root, "Air", "Moon Safari", first),
            Some(PathBuf::from("/u/Albums/Air/Moon Safari (2)"))
        );
        assert_eq!(album_destination(root, "Air", " ", none), None);
    }

    #[test]
    fn searches_take_old_and_new_names() {
        assert_eq!(Stage::from_search("prep"), Some(Stage::Triage));
        assert_eq!(Stage::from_search("triage"), Some(Stage::Triage));
        assert_eq!(Stage::from_search("playready"), Some(Stage::Collection));
        assert_eq!(Stage::from_search("collection"), Some(Stage::Collection));
        assert_eq!(Stage::from_search("ready"), None);
    }

    fn folders() -> Folders {
        Folders {
            inbox: vec!["/u/Downloads".into(), "/u/Soulseek Downloads".into()],
            triage: Some("/u/DB/_Staging".into()),
            collection: Some("/u/DB/Library".into()),
            albums: Some("/u/DB/Albums".into()),
        }
    }

    fn track(artist: &str, title: &str, remix: &str) -> Track {
        let mut t = Track {
            id: 1,
            artist: artist.into(),
            title: title.into(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 200.0,
            first_beat: 0.0,
            grid: Default::default(),
            source: Source::File(PathBuf::from("/u/Downloads/01_x.MP3")),
            waveform: Default::default(),
            audio: None,
            cover: None,
            info: Default::default(),
        };
        t.info.remix = remix.into();
        t
    }

    #[test]
    fn a_file_belongs_to_the_deepest_stage_folder_it_is_in() {
        let mut f = folders();
        assert_eq!(
            f.stage_of(Path::new("/u/Downloads/a.mp3")),
            Some(Stage::Inbox)
        );
        assert_eq!(
            f.stage_of(Path::new("/u/Downloads/pack/a.mp3")),
            Some(Stage::Inbox)
        );
        assert_eq!(
            f.stage_of(Path::new("/u/DB/_Staging/Yoga STG/a.flac")),
            Some(Stage::Triage)
        );
        assert_eq!(
            f.stage_of(Path::new("/u/DB/Library/a.flac")),
            Some(Stage::Collection)
        );
        assert_eq!(f.stage_of(Path::new("/u/Music/a.flac")), None);
        // Albums are in the collection, in their own folder.
        assert_eq!(
            f.stage_of(Path::new("/u/DB/Albums/Air/Moon Safari/01 La femme.flac")),
            Some(Stage::Collection)
        );
        // A triage folder inside the collection's folder is still triage.
        f.triage = Some("/u/DB/Library/_Staging".into());
        assert_eq!(
            f.stage_of(Path::new("/u/DB/Library/_Staging/a.flac")),
            Some(Stage::Triage)
        );
        assert_eq!(
            f.stage_of(Path::new("/u/DB/Library/b.flac")),
            Some(Stage::Collection)
        );
        // "_StagingOld" is not inside "_Staging".
        assert_eq!(
            f.stage_of(Path::new("/u/DB/Library/_StagingOld/a.flac")),
            Some(Stage::Collection)
        );
    }

    #[test]
    fn crates_are_the_first_subfolder() {
        let f = folders();
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(
            f.crate_of(Stage::Triage, &p("/u/DB/_Staging/Yoga STG/x/a.flac"))
                .as_deref(),
            Some("Yoga STG")
        );
        assert_eq!(f.crate_of(Stage::Triage, &p("/u/DB/_Staging/a.flac")), None);
        assert_eq!(
            f.crate_of(Stage::Collection, &p("/u/DB/_Staging/Yoga STG/a.flac")),
            None
        );
    }

    #[test]
    fn clean_names_come_from_the_tags() {
        let path = Path::new("/u/Downloads/01_x.MP3");
        assert_eq!(
            clean_name(&track("Bicep", "Glue", ""), path).as_deref(),
            Some("Bicep - Glue.mp3")
        );
        assert_eq!(
            clean_name(
                &track("Desert Dwellers", "Bodhi Mandala", "Drumspyder Remix"),
                path
            )
            .as_deref(),
            Some("Desert Dwellers - Bodhi Mandala (Drumspyder Remix).mp3")
        );
        // A title that already names the remix isn't given it twice.
        assert_eq!(
            clean_name(&track("A", "Song (Club Mix)", "Club Mix"), path).as_deref(),
            Some("A - Song (Club Mix).mp3")
        );
        assert_eq!(
            clean_name(&track("AC/DC", "What? Now: \"Live\"...", ""), path).as_deref(),
            Some("AC-DC - What Now- Live.mp3")
        );
        assert_eq!(clean_name(&track("", "Glue", ""), path), None);
    }

    #[test]
    fn destinations_never_overwrite() {
        let f = folders();
        let from = Path::new("/u/Downloads/01_x.mp3");
        let taken = |p: &Path| {
            p == Path::new("/u/DB/_Staging/Yoga STG/A - B.mp3")
                || p == Path::new("/u/DB/_Staging/Yoga STG/A - B (2).mp3")
        };
        assert_eq!(
            destination(
                &f,
                Stage::Triage,
                Some("Yoga STG"),
                from,
                Some("A - B.mp3"),
                taken
            ),
            Some(PathBuf::from("/u/DB/_Staging/Yoga STG/A - B (3).mp3"))
        );
        assert_eq!(
            destination(&f, Stage::Collection, None, from, None, taken),
            Some(PathBuf::from("/u/DB/Library/01_x.mp3"))
        );
    }

    #[test]
    fn prep_needs_tags_rating_tempo_and_key() {
        let mut t = track("Bicep", "Glue", "");
        assert!(!Prep::of(&t).done());
        t.rating = 4;
        t.bpm = 130.0;
        assert!(!Prep::of(&t).analysed);
        t.key = "8A".into();
        assert!(Prep::of(&t).done());
    }
}
