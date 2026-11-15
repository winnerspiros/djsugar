//! Finding the files of missing tracks again, after they were moved or
//! renamed outside DJ Sugar.
//!
//! The files in the DJ's folders are looked through in tiers, the surest
//! first, and the first tier that has any candidate gives the answer:
//!
//! 1. the same file name;
//! 2. the same file name in another case (`Song.MP3` for `song.mp3`);
//! 3. the same name once cleaned up: case, punctuation, a leading track
//!    number, a trailing `(1)` or `copy`, and the extension left out
//!    (`01 - Artist_Title (1).m4a` for `Artist - Title.mp3`);
//! 4. a track in the collection with the same artist, title and length
//!    (a file a scan added again under a new name).
//!
//! A candidate whose length is known and clearly differs is never taken,
//! and when a tier has several, the one with the same length is. A match
//! is *sure* (the "Auto-fix" button takes it) when it is the only file by
//! that name, or a cleaned-up name with the same length; the rest are
//! suggestions for the DJ to check.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Lengths closer than this (in seconds) are the same.
const SAME_LENGTH: f64 = 1.0;

/// How a missing track's file was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Name,
    NameAnyCase,
    CleanName,
    Tags,
}

impl Tier {
    /// What matched, for the Fix column.
    pub fn why(self) -> &'static str {
        match self {
            Tier::Name => "same name",
            Tier::NameAnyCase => "same name, other case",
            Tier::CleanName => "similar name",
            Tier::Tags => "same artist, title and length",
        }
    }
}

/// A file found for a missing track.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub path: PathBuf,
    pub tier: Tier,
    /// One clear answer, for fixing without a look.
    pub sure: bool,
}

/// What looking for a missing track's file came to.
#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    Found(Found),
    /// This many files match equally well.
    Several(usize),
    Nothing,
}

/// The files missing tracks are looked for among.
#[derive(Debug, Default)]
pub struct Index {
    by_name: HashMap<String, Vec<PathBuf>>,
    by_lower: HashMap<String, Vec<PathBuf>>,
    by_clean: HashMap<String, Vec<PathBuf>>,
    by_tags: HashMap<String, Vec<PathBuf>>,
    length: HashMap<PathBuf, f64>,
}

impl Index {
    /// Indexes these files by name.
    pub fn new(files: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut index = Self::default();
        for path in files {
            index.add_file(path);
        }
        index
    }

    fn add_file(&mut self, path: PathBuf) {
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        if let Some(clean) = clean_name(&path) {
            self.by_clean.entry(clean).or_default().push(path.clone());
        }
        self.by_lower
            .entry(name.to_lowercase())
            .or_default()
            .push(path.clone());
        self.by_name.entry(name).or_default().push(path);
    }

    /// What the collection knows of a file that is there: its artist,
    /// title and length (0 when unknown). A file not indexed by name yet
    /// is added.
    pub fn know(&mut self, path: &Path, artist: &str, title: &str, length: f64) {
        if !self.length.contains_key(path) {
            let known = path
                .file_name()
                .and_then(|n| self.by_name.get(n.to_string_lossy().as_ref()))
                .is_some_and(|v| v.iter().any(|p| p == path));
            if !known {
                self.add_file(path.to_path_buf());
            }
        }
        if length > 0.0 {
            self.length.insert(path.to_path_buf(), length);
        }
        if let Some(key) = tags_key(artist, title) {
            let paths = self.by_tags.entry(key).or_default();
            if !paths.iter().any(|p| p == path) {
                paths.push(path.to_path_buf());
            }
        }
    }

    /// Where the file of a missing track (at `path`, with these tags and
    /// length) may be now.
    pub fn find(&self, path: &Path, artist: &str, title: &str, length: f64) -> Lookup {
        // Some(true) same length, Some(false) clearly not, None unknown.
        let same_length = |p: &PathBuf| match self.length.get(p) {
            Some(&l) if length > 0.0 => Some((l - length).abs() <= SAME_LENGTH),
            _ => None,
        };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let tiers = [
            (Tier::Name, name.as_ref().and_then(|n| self.by_name.get(n))),
            (
                Tier::NameAnyCase,
                name.as_ref()
                    .and_then(|n| self.by_lower.get(&n.to_lowercase())),
            ),
            (
                Tier::CleanName,
                clean_name(path).and_then(|n| self.by_clean.get(&n)),
            ),
            (
                Tier::Tags,
                tags_key(artist, title).and_then(|k| self.by_tags.get(&k)),
            ),
        ];
        for (tier, paths) in tiers {
            let candidates: Vec<&PathBuf> = paths
                .into_iter()
                .flatten()
                .filter(|p| p.as_path() != path)
                .filter(|p| match tier {
                    // Tags alone are too loose without the length.
                    Tier::Tags => same_length(p) == Some(true),
                    _ => same_length(p) != Some(false),
                })
                .collect();
            let found = |p: &PathBuf, sure: bool| {
                Lookup::Found(Found {
                    path: p.clone(),
                    tier,
                    sure: sure && tier != Tier::Tags,
                })
            };
            match candidates.as_slice() {
                [] => continue,
                [one] => {
                    let sure = match tier {
                        Tier::Name | Tier::NameAnyCase => true,
                        _ => same_length(one) == Some(true),
                    };
                    return found(one, sure);
                }
                many => {
                    let same: Vec<&&PathBuf> = many
                        .iter()
                        .filter(|p| same_length(p) == Some(true))
                        .collect();
                    return match same.as_slice() {
                        [one] => found(one, true),
                        _ => Lookup::Several(many.len()),
                    };
                }
            }
        }
        Lookup::Nothing
    }
}

/// Lower-case letters and digits, one space between words.
fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// A file's name with what changes when a file is copied, renamed or
/// converted left out: case, punctuation, the extension, a leading track
/// number and a trailing `(1)` or `copy`.
pub fn clean_name(path: &Path) -> Option<String> {
    let mut stem = path.file_stem()?.to_string_lossy().trim().to_string();
    loop {
        let copy = stem.len() >= 4
            && stem.is_char_boundary(stem.len() - 4)
            && stem[stem.len() - 4..].eq_ignore_ascii_case("copy");
        let cut = if let Some(open) = stem.rfind('(').filter(|_| stem.ends_with(')')) {
            let inside = &stem[open + 1..stem.len() - 1];
            (!inside.is_empty() && inside.chars().all(|c| c.is_ascii_digit())).then_some(open)
        } else if copy {
            Some(stem.len() - "copy".len())
        } else {
            None
        };
        let Some(cut) = cut else { break };
        let rest = stem[..cut].trim_end_matches([' ', '-', '_']).to_string();
        if rest.is_empty() {
            break;
        }
        stem = rest;
    }
    let mut words = words(&stem);
    // "01 Song", "1-02 Song": up to two short numbers before the name.
    for _ in 0..2 {
        let number = words
            .first()
            .is_some_and(|w| w.len() <= 3 && w.chars().all(|c| c.is_ascii_digit()));
        if number && words.len() > 1 {
            words.remove(0);
        }
    }
    (!words.is_empty()).then(|| words.join(" "))
}

/// Artist and title, cleaned up as file names are; `None` without both.
fn tags_key(artist: &str, title: &str) -> Option<String> {
    let (a, t) = (words(artist), words(title));
    (!a.is_empty() && !t.is_empty()).then(|| format!("{}\u{1f}{}", a.join(" "), t.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn found(path: &str, tier: Tier, sure: bool) -> Lookup {
        Lookup::Found(Found {
            path: p(path),
            tier,
            sure,
        })
    }

    #[test]
    fn cleans_up_names() {
        let c = |s: &str| clean_name(Path::new(s)).unwrap();
        assert_eq!(c("/m/Artist - Title.mp3"), "artist title");
        assert_eq!(c("/m/01 - Artist_Title (1).m4a"), "artist title");
        assert_eq!(c("/m/1-02 Artist - Title copy.flac"), "artist title");
        assert_eq!(c("/m/Artist - Title - Copy (2).mp3"), "artist title");
        assert_eq!(c("/m/Song 2.mp3"), "song 2");
        assert_eq!(c("/m/2 Unlimited - No Limit.mp3"), "unlimited no limit");
        assert_eq!(c("/m/(1).mp3"), "1");
        assert_eq!(c("/m/808.wav"), "808");
    }

    #[test]
    fn the_same_name_comes_first() {
        let index = Index::new([p("/new/Song.mp3"), p("/new/song.mp3"), p("/x/Song (1).mp3")]);
        assert_eq!(
            index.find(Path::new("/old/Song.mp3"), "", "", 0.0),
            found("/new/Song.mp3", Tier::Name, true)
        );
    }

    #[test]
    fn then_the_name_in_any_case() {
        let index = Index::new([p("/new/SONG.MP3"), p("/x/Song (1).mp3")]);
        assert_eq!(
            index.find(Path::new("/old/Song.mp3"), "", "", 0.0),
            found("/new/SONG.MP3", Tier::NameAnyCase, true)
        );
    }

    #[test]
    fn a_cleaned_up_name_is_sure_only_with_the_same_length() {
        let mut index = Index::new([p("/new/01 Artist_Title (1).m4a")]);
        let old = Path::new("/old/Artist - Title.mp3");
        assert_eq!(
            index.find(old, "", "", 200.0),
            found("/new/01 Artist_Title (1).m4a", Tier::CleanName, false)
        );
        index.know(Path::new("/new/01 Artist_Title (1).m4a"), "", "", 200.4);
        assert_eq!(
            index.find(old, "", "", 200.0),
            found("/new/01 Artist_Title (1).m4a", Tier::CleanName, true)
        );
    }

    #[test]
    fn a_different_length_rules_a_file_out() {
        let mut index = Index::new([p("/a/Intro.mp3")]);
        index.know(Path::new("/a/Intro.mp3"), "", "", 30.0);
        assert_eq!(
            index.find(Path::new("/old/Intro.mp3"), "", "", 300.0),
            Lookup::Nothing
        );
    }

    #[test]
    fn the_same_length_picks_one_of_several() {
        let mut index = Index::new([p("/a/Intro.mp3"), p("/b/Intro.mp3")]);
        assert_eq!(
            index.find(Path::new("/old/Intro.mp3"), "", "", 61.0),
            Lookup::Several(2)
        );
        index.know(Path::new("/a/Intro.mp3"), "", "", 30.0);
        index.know(Path::new("/b/Intro.mp3"), "", "", 61.2);
        assert_eq!(
            index.find(Path::new("/old/Intro.mp3"), "", "", 61.0),
            found("/b/Intro.mp3", Tier::Name, true)
        );
    }

    #[test]
    fn same_tags_and_length_is_a_suggestion() {
        let mut index = Index::default();
        index.know(
            Path::new("/new/renamed.mp3"),
            "Daft Punk",
            "One More Time",
            320.0,
        );
        let old = Path::new("/old/track01.mp3");
        assert_eq!(
            index.find(old, "daft punk", "One more time", 320.5),
            found("/new/renamed.mp3", Tier::Tags, false)
        );
        assert_eq!(
            index.find(old, "Daft Punk", "One More Time", 250.0),
            Lookup::Nothing
        );
        assert_eq!(
            index.find(old, "Daft Punk", "One More Time", 0.0),
            Lookup::Nothing
        );
        assert_eq!(index.find(old, "", "One More Time", 320.0), Lookup::Nothing);
    }

    #[test]
    fn knowing_a_file_twice_lists_it_once() {
        let mut index = Index::new([p("/a/Song.mp3")]);
        index.know(Path::new("/a/Song.mp3"), "A", "B", 10.0);
        index.know(Path::new("/a/Song.mp3"), "A", "B", 10.0);
        assert_eq!(
            index.find(Path::new("/old/x.mp3"), "A", "B", 10.0),
            found("/a/Song.mp3", Tier::Tags, false)
        );
        assert_eq!(
            index.find(Path::new("/old/Song.mp3"), "", "", 0.0),
            found("/a/Song.mp3", Tier::Name, true)
        );
    }
}
