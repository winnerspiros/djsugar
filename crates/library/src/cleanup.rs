//! Clean-up: rules that tidy the names in a track's tags (track numbers,
//! web sites, "(Official Video)", underscores, a remix or a featured
//! artist in the title), and a log of every change made to tags in a
//! batch, so the batch can be taken back.
//!
//! The rules only suggest: the DJ looks at each change before it is
//! written to the library and the file.

use std::collections::HashSet;

use rusqlite::params;

use djsugar_engine::Track;

use crate::{now, row_id, track_id, Error, Library};

pub(crate) const CHANGES_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS tag_changes (
    id INTEGER PRIMARY KEY,
    batch INTEGER NOT NULL,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    old TEXT NOT NULL,
    new TEXT NOT NULL,
    at INTEGER NOT NULL,
    undone INTEGER
);
CREATE INDEX IF NOT EXISTS tag_changes_batch ON tag_changes(batch);
CREATE TABLE IF NOT EXISTS hand_edited (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    PRIMARY KEY (track_id, field)
);
";

/// Which rules run. Each can be turned off in Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// "01 - Title" and "Artist - Title" in the title: the track number
    /// and the artist go.
    pub numbers: bool,
    /// "www.site.com" and "site.net" go.
    pub sites: bool,
    /// "(Official Video)", "[HD]", "(Free Download)" and the like go.
    pub junk: bool,
    /// Underscores become spaces; double spaces become one.
    pub underscores: bool,
    /// "Title (X Remix)" moves the remix to its own field.
    pub remix: bool,
    /// "Title (feat. X)" moves the featured artist to the artist.
    pub feat: bool,
    /// Names all in capitals or all in small letters get Title Case.
    pub title_case: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            numbers: true,
            sites: true,
            junk: true,
            underscores: true,
            remix: true,
            feat: true,
            title_case: false,
        }
    }
}

impl Rules {
    /// Each rule: its settings key, name and what it does, with whether it
    /// is on by default.
    pub const ALL: [(&'static str, &'static str, &'static str, bool); 7] = [
        (
            "cleanup_numbers",
            "Track numbers",
            "\"01 - Title\" becomes \"Title\", and an artist repeated in the title goes.",
            true,
        ),
        (
            "cleanup_sites",
            "Web sites",
            "\"www.free-mp3.com\" and the like go.",
            true,
        ),
        (
            "cleanup_junk",
            "Video words",
            "\"(Official Video)\", \"[HD]\", \"(Free Download)\" and the like go.",
            true,
        ),
        (
            "cleanup_underscores",
            "Underscores and spaces",
            "Underscores become spaces, and double spaces one.",
            true,
        ),
        (
            "cleanup_remix",
            "Remix to its field",
            "\"Title (X Remix)\" puts \"X Remix\" in the Remix field.",
            true,
        ),
        (
            "cleanup_feat",
            "Featured artists",
            "\"Title (feat. X)\" becomes \"Title\" by \"Artist feat. X\".",
            true,
        ),
        (
            "cleanup_title_case",
            "Title Case",
            "Names in all capitals or all small letters get capitals on each word.",
            false,
        ),
    ];

    /// The rules as `on` says for each settings key.
    pub fn from(on: impl Fn(&str, bool) -> bool) -> Self {
        let [numbers, sites, junk, underscores, remix, feat, title_case] =
            Self::ALL.map(|(key, _, _, default)| on(key, default));
        Rules {
            numbers,
            sites,
            junk,
            underscores,
            remix,
            feat,
            title_case,
        }
    }
}

/// A tag field the clean-up, a genre merge or an edit of several tracks
/// can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Artist,
    Title,
    Remix,
    Album,
    Genre,
    Year,
    Label,
    Comment,
    Grouping,
    /// The track's tags in one of the DJ's tag groups (by its id), as one
    /// line: "Synth, Vocals".
    Tag(i64),
}

impl Field {
    /// The fields with a fixed name (the tag groups come after them).
    pub const ALL: [Field; 9] = [
        Field::Artist,
        Field::Title,
        Field::Remix,
        Field::Album,
        Field::Genre,
        Field::Year,
        Field::Label,
        Field::Comment,
        Field::Grouping,
    ];

    /// The name kept in the change log, which is also the Info tab's id
    /// for the field ("tag:3" for the tag group with id 3).
    pub fn name(self) -> std::borrow::Cow<'static, str> {
        match self {
            Field::Artist => "artist".into(),
            Field::Title => "title".into(),
            Field::Remix => "remix".into(),
            Field::Album => "album".into(),
            Field::Genre => "genre".into(),
            Field::Year => "year".into(),
            Field::Label => "label".into(),
            Field::Comment => "comment".into(),
            Field::Grouping => "grouping".into(),
            Field::Tag(group) => crate::tags::field_name(group).into(),
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        if let Some(group) = s.strip_prefix("tag:") {
            return group.parse().ok().map(Field::Tag);
        }
        Field::ALL.into_iter().find(|f| f.name() == s)
    }

    /// What the review table calls it.
    pub fn label(self) -> std::borrow::Cow<'static, str> {
        match self {
            Field::Artist => "Artist".into(),
            Field::Title => "Title".into(),
            Field::Remix => "Remix".into(),
            Field::Album => "Album".into(),
            Field::Genre => "Genre".into(),
            Field::Year => "Year".into(),
            Field::Label => "Label".into(),
            Field::Comment => "Comment".into(),
            Field::Grouping => "Grouping".into(),
            Field::Tag(group) => crate::query::field_names()
                .tags
                .into_iter()
                .find(|g| g.id == group)
                .map_or_else(|| "Tags".into(), |g| g.name.into()),
        }
    }

    pub fn get(self, t: &Track) -> std::borrow::Cow<'_, str> {
        match self {
            Field::Artist => t.artist.as_str().into(),
            Field::Title => t.title.as_str().into(),
            Field::Remix => t.info.remix.as_str().into(),
            Field::Album => t.album.as_str().into(),
            Field::Genre => t.genre.as_str().into(),
            Field::Year => t.info.year.as_str().into(),
            Field::Label => t.info.label.as_str().into(),
            Field::Comment => t.info.comment.as_str().into(),
            Field::Grouping => t.info.grouping.as_str().into(),
            Field::Tag(group) => t.info.tag_text(group).into(),
        }
    }

    pub fn set(self, t: &mut Track, value: String) {
        match self {
            Field::Artist => t.artist = value,
            Field::Title => t.title = value,
            Field::Remix => t.info.remix = value,
            Field::Album => t.album = value,
            Field::Genre => t.genre = value,
            Field::Year => t.info.year = value,
            Field::Label => t.info.label = value,
            Field::Comment => t.info.comment = value,
            Field::Grouping => t.info.grouping = value,
            Field::Tag(group) => t.info.set_tags_in(group, djsugar_engine::split_tags(&value)),
        }
    }
}

/// One field's value before and after.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub field: Field,
    pub old: String,
    pub new: String,
}

/// What the rules would change in `t`'s names; nothing when they're clean.
pub fn changes(t: &Track, rules: &Rules) -> Vec<Change> {
    let mut artist = tidy(&t.artist, rules);
    let mut title = tidy(&t.title, rules);
    let mut remix = tidy(&t.info.remix, rules);
    let mut album = tidy(&t.album, rules);
    if rules.numbers {
        title = without_number(&title);
        title = without_artist(&title, &artist);
    }
    if rules.feat {
        if let Some((rest, guest)) = split_feat(&title) {
            title = rest;
            if !artist.to_lowercase().contains(&guest.to_lowercase()) {
                artist = if artist.is_empty() {
                    guest
                } else {
                    format!("{artist} feat. {guest}")
                };
            }
        }
    }
    if rules.remix {
        if let Some((rest, found)) = split_remix(&title) {
            if remix.is_empty() || remix.eq_ignore_ascii_case(&found) {
                title = rest;
                if !found.eq_ignore_ascii_case("original mix") {
                    remix = found;
                }
            }
        }
    }
    if rules.title_case {
        for s in [&mut artist, &mut title, &mut remix, &mut album] {
            *s = title_case(s);
        }
    }
    // A rule never empties a name.
    let pick = |new: String, old: &str| if new.is_empty() { old.to_string() } else { new };
    let after = [
        (Field::Artist, pick(artist, &t.artist)),
        (Field::Title, pick(title, &t.title)),
        (Field::Remix, remix),
        (Field::Album, pick(album, &t.album)),
    ];
    after
        .into_iter()
        .filter_map(|(field, new)| {
            let old = field.get(t);
            (new != old).then(|| Change {
                field,
                old: old.to_string(),
                new,
            })
        })
        .collect()
}

/// Brackets and the words in them that are never part of a name.
const JUNK: [&str; 22] = [
    "official video",
    "official music video",
    "official audio",
    "official lyric video",
    "official visualizer",
    "official visualiser",
    "official clip",
    "official",
    "music video",
    "lyric video",
    "lyrics video",
    "lyrics",
    "video",
    "audio",
    "visualizer",
    "hd",
    "hq",
    "4k",
    "free download",
    "free dl",
    "clip officiel",
    "videoclip",
];

/// Endings that make a word a web site.
const SITES: [&str; 18] = [
    "com", "net", "org", "ru", "info", "biz", "me", "io", "co", "fm", "to", "cc", "xyz", "pl",
    "de", "uk", "su", "ws",
];

/// `s` with the rules that work on any name applied.
fn tidy(s: &str, rules: &Rules) -> String {
    let mut s = s.to_string();
    if rules.underscores {
        s = s.replace('_', " ");
    }
    if rules.sites {
        s = s
            .split(' ')
            .filter(|w| !is_site(w))
            .collect::<Vec<_>>()
            .join(" ");
    }
    if rules.junk {
        s = without_brackets(&s, |inner| JUNK.contains(&inner.to_lowercase().as_str()));
    }
    // Brackets the other rules emptied.
    s = without_brackets(&s, |inner| inner.trim().is_empty());
    let mut s = if rules.underscores || rules.sites || rules.junk {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        s.trim().to_string()
    };
    // Separators left dangling at the ends.
    while let Some(t) = s
        .strip_suffix(" -")
        .or_else(|| s.strip_suffix(" –"))
        .or_else(|| s.strip_prefix("- "))
    {
        s = t.trim().to_string();
    }
    s
}

/// Whether a word is a web address: "www.x", or a name ending in ".com"
/// and the like.
fn is_site(word: &str) -> bool {
    let w = word
        .trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | ',' | '"'))
        .to_lowercase();
    if w.starts_with("www.") || w.starts_with("http://") || w.starts_with("https://") {
        return true;
    }
    match w.rsplit_once('.') {
        Some((name, end)) => {
            !name.is_empty()
                && name.chars().any(|c| c.is_alphabetic())
                && !name.ends_with('.')
                && SITES.contains(&end)
        }
        None => false,
    }
}

/// `s` without the bracketed parts, "(…)" or "[…]", whose inner text
/// `drop` says to leave out.
fn without_brackets(s: &str, drop: impl Fn(&str) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(['(', '[']) {
        let close = if rest[start..].starts_with('(') {
            ')'
        } else {
            ']'
        };
        let Some(len) = rest[start + 1..].find(close) else {
            break;
        };
        let inner = &rest[start + 1..start + 1 + len];
        out.push_str(&rest[..start]);
        if !drop(inner.trim()) {
            out.push_str(&rest[start..start + len + 2]);
        }
        rest = &rest[start + len + 2..];
    }
    out.push_str(rest);
    out
}

/// A title without a leading track number: "01 - Title", "1. Title",
/// "03) Title", "07 Title". Titles that start with a number ("2 Unlimited",
/// "99 Luftballons") stay.
fn without_number(title: &str) -> String {
    let digits = title.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return title.to_string();
    }
    let rest = &title[digits..];
    let spaced = rest.trim_start();
    let after = if let Some(r) = spaced.strip_prefix(['-', '.', ')', '–']) {
        // "3.5 Degrees" isn't a number before a title.
        if r.starts_with(|c: char| c.is_ascii_digit()) {
            return title.to_string();
        }
        r
    } else if title.starts_with('0') && digits >= 2 && rest.starts_with(' ') {
        rest
    } else {
        return title.to_string();
    };
    let after = after.trim_start();
    if after.chars().any(char::is_alphabetic) {
        after.to_string()
    } else {
        title.to_string()
    }
}

/// A title without the artist in front of it: "Lane 8 - Fingerprint" by
/// Lane 8 is "Fingerprint".
fn without_artist(title: &str, artist: &str) -> String {
    if artist.is_empty() {
        return title.to_string();
    }
    for sep in [" - ", " – "] {
        if let Some((front, rest)) = title.split_once(sep) {
            if front.trim().eq_ignore_ascii_case(artist.trim()) && !rest.trim().is_empty() {
                return rest.trim().to_string();
            }
        }
    }
    title.to_string()
}

/// "Title (feat. X)" or "Title ft. X": the title and the guest.
fn split_feat(title: &str) -> Option<(String, String)> {
    let lower = title.to_lowercase();
    for word in ["feat. ", "feat ", "ft. ", "featuring "] {
        for open in ["(", "[", ""] {
            let needle = if open.is_empty() {
                format!(" {word}")
            } else {
                format!("{open}{word}")
            };
            let Some(start) = lower.find(&needle) else {
                continue;
            };
            let from = start + needle.len();
            let (guest, end) = if open.is_empty() {
                // Up to a bracket that follows, if any.
                let end = title[from..]
                    .find(['(', '['])
                    .map_or(title.len(), |e| from + e);
                (&title[from..end], end)
            } else {
                let close = if open == "(" { ')' } else { ']' };
                let e = title[from..].find(close)?;
                (&title[from..from + e], from + e + 1)
            };
            let guest = guest.trim();
            if guest.is_empty() {
                return None;
            }
            let rest = format!("{} {}", &title[..start], &title[end..]);
            let rest = rest.split_whitespace().collect::<Vec<_>>().join(" ");
            return Some((rest, guest.to_string()));
        }
    }
    None
}

/// Words that end a remix's name.
const REMIX_WORDS: [&str; 10] = [
    "remix", "mix", "edit", "rework", "dub", "vip", "bootleg", "flip", "version", "refix",
];

/// "Title (X Remix)", "Title [X Edit]" or "Title - X Remix": the title
/// and the remix.
fn split_remix(title: &str) -> Option<(String, String)> {
    let is_remix = |s: &str| {
        let last = s.split_whitespace().last().unwrap_or("").to_lowercase();
        s.split_whitespace().count() >= 2 && REMIX_WORDS.contains(&last.as_str())
    };
    let t = title.trim_end();
    for (open, close) in [('(', ')'), ('[', ']')] {
        if t.ends_with(close) {
            if let Some(start) = t.rfind(open) {
                let inner = t[start + 1..t.len() - 1].trim();
                if is_remix(inner) {
                    let rest = t[..start].trim();
                    if !rest.is_empty() {
                        return Some((rest.to_string(), inner.to_string()));
                    }
                }
            }
        }
    }
    let (rest, inner) = t.rsplit_once(" - ")?;
    (is_remix(inner) && !rest.trim().is_empty())
        .then(|| (rest.trim().to_string(), inner.trim().to_string()))
}

/// "DEEP HOUSE" or "deep house" as "Deep House"; names with both stay.
fn title_case(s: &str) -> String {
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    let upper = letters.iter().all(|c| c.is_uppercase());
    let lower = letters.iter().all(|c| c.is_lowercase());
    if letters.len() < 2 || !(upper || lower) {
        return s.to_string();
    }
    s.split(' ')
        .map(|w| {
            let lw = w.to_lowercase();
            if ["feat.", "ft.", "vs.", "x"].contains(&lw.as_str()) {
                return lw;
            }
            let mut c = lw.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A change logged for a track: its id, the field, the old and new values.
pub type Logged = (usize, Field, String, String);

impl Library {
    /// Logs changes made to tracks' tags as one batch, which is returned.
    pub fn log_changes(&mut self, changes: &[Logged]) -> Result<i64, Error> {
        let tx = self.db.transaction()?;
        let batch: i64 = tx.query_row(
            "SELECT COALESCE(MAX(batch), 0) + 1 FROM tag_changes",
            [],
            |r| r.get(0),
        )?;
        {
            let mut add = tx.prepare(
                "INSERT INTO tag_changes (batch, track_id, field, old, new, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let at = now();
            for (id, field, old, new) in changes {
                let Some(row) = row_id(*id) else { continue };
                add.execute(params![batch, row, field.name(), old, new, at])?;
            }
        }
        tx.commit()?;
        Ok(batch)
    }

    /// The changes in a batch not yet taken back, as they were logged.
    pub fn logged_changes(&self, batch: i64) -> Result<Vec<Logged>, Error> {
        let mut q = self.db.prepare(
            "SELECT track_id, field, old, new FROM tag_changes
             WHERE batch = ?1 AND undone IS NULL ORDER BY id",
        )?;
        let rows = q.query_map([batch], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, field, old, new) = row?;
            if let Some(field) = Field::from_name(&field) {
                out.push((track_id(id), field, old, new));
            }
        }
        Ok(out)
    }

    /// Notes that the DJ typed in a field of a track (by its name, as
    /// [`Field::name`] gives it), so online lookups leave it alone.
    pub fn mark_hand_edited(&self, track: usize, field: &str) -> Result<(), Error> {
        let Some(row) = row_id(track) else {
            return Ok(());
        };
        self.db.execute(
            "INSERT OR IGNORE INTO hand_edited (track_id, field) VALUES (?1, ?2)",
            params![row, field],
        )?;
        Ok(())
    }

    /// The fields the DJ typed in, by track id.
    pub fn hand_edited(&self) -> Result<HashSet<(usize, Field)>, Error> {
        let mut q = self.db.prepare("SELECT track_id, field FROM hand_edited")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = HashSet::new();
        for row in rows {
            let (id, field) = row?;
            if let Some(f) = Field::from_name(&field) {
                out.insert((track_id(id), f));
            }
        }
        Ok(out)
    }

    /// Marks a batch of changes as taken back.
    pub fn undo_changes(&mut self, batch: i64) -> Result<(), Error> {
        self.db.execute(
            "UPDATE tag_changes SET undone = ?2 WHERE batch = ?1 AND undone IS NULL",
            params![batch, now()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::file_track;

    fn named(artist: &str, title: &str) -> Track {
        let mut t = file_track("/m/a.mp3", title);
        t.artist = artist.into();
        t
    }

    fn after(t: &Track, rules: &Rules) -> Track {
        let mut t = t.clone();
        for c in changes(&t.clone(), rules) {
            c.field.set(&mut t, c.new);
        }
        t
    }

    #[test]
    fn strips_numbers_sites_and_video_words() {
        let r = Rules::default();
        let t = after(&named("", "01_bicep_glue_www.free-mp3.com"), &r);
        assert_eq!(t.title, "bicep glue");
        let t = after(&named("Lane 8", "02 - Lane 8 - Fingerprint"), &r);
        assert_eq!(t.title, "Fingerprint");
        let t = after(&named("Bonobo", "Kerala (Official Video) [HD]"), &r);
        assert_eq!(t.title, "Kerala");
        for keep in ["2 Unlimited", "99 Luftballons", "3.5 Degrees", "1999"] {
            assert_eq!(after(&named("X", keep), &r).title, keep);
        }
        assert!(changes(&named("Bonobo", "Kerala"), &r).is_empty());
    }

    #[test]
    fn moves_remix_and_guest_to_their_fields() {
        let r = Rules::default();
        let t = after(
            &named("Desert Dwellers", "Bodhi Mandala (Drumspyder Remix)"),
            &r,
        );
        assert_eq!(
            (t.title.as_str(), t.info.remix.as_str()),
            ("Bodhi Mandala", "Drumspyder Remix")
        );
        let t = after(
            &named("Lane 8", "Road (feat. Arctic Lake) [Extended Mix]"),
            &r,
        );
        assert_eq!(t.artist, "Lane 8 feat. Arctic Lake");
        assert_eq!(
            (t.title.as_str(), t.info.remix.as_str()),
            ("Road", "Extended Mix")
        );
        // "Original Mix" says there's no remix.
        let t = after(&named("X", "Song - Original Mix"), &r);
        assert_eq!((t.title.as_str(), t.info.remix.as_str()), ("Song", ""));
        // A remix field that says something else is left alone.
        let mut odd = named("X", "Song (Y Remix)");
        odd.info.remix = "Z Remix".into();
        assert!(changes(&odd, &r).is_empty());
    }

    #[test]
    fn rules_can_be_turned_off_and_title_case_on() {
        let off = Rules {
            numbers: false,
            ..Rules::default()
        };
        assert_eq!(after(&named("X", "01 - Song"), &off).title, "01 - Song");
        let on = Rules {
            title_case: true,
            ..Rules::default()
        };
        let t = after(&named("CARBON BASED LIFEFORMS", "clouds"), &on);
        assert_eq!(
            (t.artist.as_str(), t.title.as_str()),
            ("Carbon Based Lifeforms", "Clouds")
        );
        assert_eq!(
            after(&named("deadmau5", "Strobe"), &Rules::default()).artist,
            "deadmau5"
        );
        let none = Rules::from(|_, _| false);
        assert_eq!(after(&named("a_b", "c_d"), &none).artist, "a_b");
    }

    #[test]
    fn logs_and_takes_back_batches() {
        let mut lib = Library::in_memory().unwrap();
        let t = lib.add_file(&file_track("/m/a.mp3", "01 - Song")).unwrap();
        let batch = lib
            .log_changes(&[(t.id, Field::Title, "01 - Song".into(), "Song".into())])
            .unwrap();
        let logged = lib.logged_changes(batch).unwrap();
        assert_eq!(
            logged,
            [(t.id, Field::Title, "01 - Song".into(), "Song".into())]
        );
        lib.undo_changes(batch).unwrap();
        assert!(lib.logged_changes(batch).unwrap().is_empty());
        lib.mark_hand_edited(t.id, "album").unwrap();
        lib.mark_hand_edited(t.id, "bpm").unwrap();
        assert_eq!(
            lib.hand_edited().unwrap(),
            HashSet::from([(t.id, Field::Album)])
        );
    }

    #[test]
    fn names_every_field_both_ways() {
        for f in Field::ALL.into_iter().chain([Field::Tag(0), Field::Tag(4)]) {
            assert_eq!(Field::from_name(&f.name()), Some(f));
        }
        assert_eq!(Field::Tag(2).name(), "tag:2");
        assert_eq!(Field::from_name("tag2"), None);
        assert_eq!(Field::from_name("bpm"), None);
        let mut t = file_track("/m/a.mp3", "Song");
        Field::Tag(2).set(&mut t, "Synth, #vox, synth".into());
        assert_eq!(Field::Tag(2).get(&t), "Synth, vox");
        assert_eq!(t.info.tags.len(), 2);
    }
}
