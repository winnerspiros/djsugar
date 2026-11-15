//! Library search, in Mixxx's search syntax so DJs coming from Mixxx can type
//! what they already know, plus a few additions. Smart lists are saved
//! searches, so the same text drives both.
//!
//! - Plain words match the artist, title, album, genre, comment, grouping
//!   and tags; `"two words"` matches a phrase. Matching forgives what a DJ
//!   doesn't type: accents (`olafur` finds "Ólafur"), punctuation (`acdc`
//!   finds "AC/DC"), `&` for "and" and `ft`/`featuring` for "feat" (see
//!   [`fold`]). When nothing matches, [`Query::loosened`] lets each word of
//!   four letters or more be one letter off (`frahn` finds "Frahm").
//! - `field:value` searches one field: `artist:`, `title:`, `album:`,
//!   `genre:`, `comment:`, `grouping:`, `tag:` (every tag group), `key:`,
//!   `location:`, `publisher:` (the record label), `composer:`, `remixer:`.
//!   Each of the DJ's tag groups goes by its name (`mood:`, `tripstage:`
//!   for "Trip Stage") or its place (`tag1:`, `tag2:`…), and grouping goes
//!   by its own name too; see [`set_field_names`]. A tag group matches when
//!   one of the track's tags in it does.
//!   `field:=value` matches the whole field, `field:""` an empty one.
//! - Numbers: `bpm:`, `rating:`, `played:`, `year:` and `duration:` take a value,
//!   `>`, `>=`, `<`, `<=` or a range `a-b`; durations as `3:30`, `5m` or seconds.
//!   `added:` takes days or weeks ago (`added:<7d`, `added:<2w`) or a date
//!   (`added:>=2026-01-31`); `lastplayed:` the same for the last play.
//! - `~key:8A` finds keys that mix with 8A (`~harmonic:8A` and
//!   `~fuzzy:8A` by that rule, whatever the setting), `~bpm:124` tempos within 6%
//!   (also at half or double tempo). `~deck` finds tracks that mix with the
//!   track playing now, by key and tempo.
//! - `color:red` (or orange, yellow, green, cyan, blue, purple, pink),
//!   `is:missing`, `is:duplicate`, `is:played`, `is:cued`, `is:prepped`
//!   (artist, title, rating, tempo and key all there), and Collection
//!   Health's checks: `is:notmusic`, `is:stray`, `is:outside`, `is:unanalyzed`,
//!   `is:noinfo`, `is:junk`, `is:nocover`, `is:unrated`, `is:long` (see
//!   [`crate::health`]).
//! - `stage:inbox`, `stage:prep`, `stage:playready` (or the older
//!   `stage:triage`, `stage:collection`; see [`crate::stages`]); `stage:none` for tracks outside the stage folders.
//! - `crate:yoga` for tracks in a crate whose name has "yoga" in it,
//!   `crate:=yoga` for that crate only, `crate:""` for tracks in no crate
//!   (see [`crate::sets`]).
//! - `-` in front of any term excludes it. Terms must all match; `|` or `OR`
//!   between groups of terms matches either.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::RwLock;

use djsugar_engine::key::{Key, Mixing};
use djsugar_engine::{Source, Track};

pub use crate::tags::TagGroup;

/// Tempos within this fraction count as close for `~bpm` and `~deck`.
const BPM_TOLERANCE: f64 = 0.06;

/// Track colours by name, as the track menu offers them (0xRRGGBB).
pub const COLOR_NAMES: [(&str, u32); 8] = [
    ("red", 0xff3b30),
    ("orange", 0xff9500),
    ("yellow", 0xffcc00),
    ("green", 0x34c759),
    ("cyan", 0x5ac8fa),
    ("blue", 0x007aff),
    ("purple", 0xaf52de),
    ("pink", 0xff2d55),
];

/// A parsed search: any one of the alternatives matches, and an alternative
/// matches when all its terms do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    alternatives: Vec<Vec<Term>>,
}

#[derive(Clone, Debug, PartialEq)]
struct Term {
    negated: bool,
    test: Test,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextField {
    Artist,
    Title,
    Album,
    Genre,
    Comment,
    Grouping,
    Tag,
    Key,
    Location,
    Publisher,
    Composer,
    Remixer,
    /// One of the DJ's tag groups, by its id.
    UserTag(i64),
}

/// What the DJ calls the fields they fill in themselves: grouping (say
/// "Trip Stage"), and their tag groups (say "Mood", "Instruments") in
/// order.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldNames {
    pub grouping: String,
    pub tags: Vec<TagGroup>,
    /// Goes up when tracks' tags changed outside the browser (Settings),
    /// so it reads them again.
    pub tags_revision: u64,
    /// Tags given a color: group, name, color (0xRRGGBB).
    pub tag_colors: Vec<(i64, String, u32)>,
}

impl FieldNames {
    /// The color of tag `name` in `group`, if it has one.
    pub fn tag_color(&self, group: i64, name: &str) -> Option<u32> {
        self.tag_colors
            .iter()
            .find(|(g, n, _)| *g == group && djsugar_engine::same_tag(n, name))
            .map(|(_, _, c)| *c)
    }

    /// The group with this id.
    pub fn group(&self, id: i64) -> Option<&TagGroup> {
        self.tags.iter().find(|g| g.id == id)
    }
}

impl Default for FieldNames {
    /// The groups a new library starts with.
    fn default() -> Self {
        FieldNames {
            grouping: "Grouping".into(),
            tags: vec![TagGroup::new(1, "Tag 1"), TagGroup::new(2, "Tag 2")],
            tags_revision: 0,
            tag_colors: Vec::new(),
        }
    }
}

static FIELD_NAMES: RwLock<Option<FieldNames>> = RwLock::new(None);

/// Sets the names searches know the DJ's own fields by.
pub fn set_field_names(names: FieldNames) {
    *FIELD_NAMES.write().unwrap_or_else(|e| e.into_inner()) = Some(names);
}

pub fn field_names() -> FieldNames {
    FIELD_NAMES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

/// The id of the DJ's tag group at place `i` (from 0).
pub fn group_at(i: usize) -> Option<i64> {
    let names = FIELD_NAMES.read().unwrap_or_else(|e| e.into_inner());
    match names.as_ref() {
        Some(n) => n.tags.get(i).map(|g| g.id),
        None => FieldNames::default().tags.get(i).map(|g| g.id),
    }
}

/// The search word for a field the DJ named: lowercase letters and digits
/// only, so "Trip Stage" is `tripstage:`.
pub fn keyword(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Puts `value` in quotes when it needs them to stay one search word.
pub fn quoted(value: &str) -> String {
    if value.is_empty() || value.contains(char::is_whitespace) || value.contains(':') {
        format!("\"{}\"", value.replace('"', ""))
    } else {
        value.to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumberField {
    Bpm,
    Rating,
    Played,
    Duration,
    /// Days since the track was added.
    AddedDaysAgo,
    /// When the track was added, as a Unix time.
    AddedAt,
    Year,
    /// Days since the track was last played.
    PlayedDaysAgo,
    /// When the track was last played, as a Unix time.
    PlayedAt,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Compare {
    Eq(f64),
    Lt(f64),
    Le(f64),
    Gt(f64),
    Ge(f64),
    Range(f64, f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flag {
    Missing,
    Duplicate,
    Played,
    Cued,
    Prepped,
    /// A Collection Health check (see [`crate::health`]).
    Health(crate::health::Check),
}

#[derive(Clone, Debug, PartialEq)]
enum Test {
    /// Folded text (see [`fold`]) found in any of the searchable text fields.
    Anywhere(String),
    /// Folded text found, one letter off at most, in any of them: a plain
    /// word of a search that found nothing as typed.
    Near(String),
    Contains(TextField, String),
    Exactly(TextField, String),
    Empty(TextField),
    Number(NumberField, Compare),
    KeyIs(Key),
    KeyMixes(Key),
    /// Mixes by a given rule, not the app's setting.
    KeyMixesBy(Key, Mixing),
    BpmNear(f64),
    MixesWithDeck,
    Color(Option<u32>),
    Is(Flag),
    /// The track's stage name; empty for none.
    Stage(String),
    /// In a crate whose lowercased name has this in it (or is it, when
    /// exact); `Crate(None)` is in no crate.
    Crate(Option<(String, bool)>),
    /// A term that can't be understood matches nothing, so a typo shows an
    /// empty list rather than everything.
    Never,
}

/// What a search compares tracks against besides the tracks themselves.
#[derive(Default)]
pub struct Context<'a> {
    /// Now, as a Unix time.
    pub now: i64,
    /// The tempo and key of the track playing now, for `~deck`.
    pub deck: Option<(f64, Option<Key>)>,
    /// That track's id, which `~deck` leaves out.
    pub deck_track: Option<usize>,
    /// Ids of tracks that are in the library more than once.
    pub duplicates: Option<&'a HashSet<usize>>,
    /// What Collection Health's checks go by (`is:notmusic` and the like).
    pub health: Option<&'a crate::health::Facts>,
    /// The crates each track is in, by track id, for `crate:`.
    pub crates: Option<&'a HashMap<usize, Vec<String>>>,
}

/// A track's text fields folded once (see [`fold`]), so searching while
/// typing stays fast in a large library.
#[derive(Clone, Debug, Default)]
pub struct Indexed {
    artist: String,
    title: String,
    album: String,
    genre: String,
    comment: String,
    grouping: String,
    tags: String,
    location: String,
    publisher: String,
    composer: String,
    remixer: String,
    /// Each tag's group and folded name.
    user: Vec<(i64, String)>,
}

impl Indexed {
    pub fn new(t: &Track) -> Self {
        Indexed {
            artist: fold(&t.artist),
            title: fold(&t.title),
            album: fold(&t.album),
            genre: fold(&t.genre),
            comment: fold(&t.info.comment),
            grouping: fold(&t.info.grouping),
            tags: t
                .info
                .tags
                .iter()
                .map(|t| fold(&t.name))
                .collect::<Vec<_>>()
                .join(" "),
            // Paths keep their slashes and dots, so `location:/music/` works.
            location: match &t.source {
                Source::File(p) => p.to_string_lossy().chars().map(unaccent).collect(),
                Source::Synth { .. } => String::new(),
            },
            publisher: fold(&t.info.label),
            composer: fold(&t.info.composer),
            remixer: fold(&t.info.remixer),
            user: t
                .info
                .tags
                .iter()
                .map(|t| (t.group, fold(&t.name)))
                .collect(),
        }
    }
}

impl Query {
    pub fn parse(text: &str) -> Query {
        let mut alternatives = vec![Vec::new()];
        for word in split(text) {
            if word == "|" || word == "OR" {
                alternatives.push(Vec::new());
                continue;
            }
            if let Some(term) = parse_term(&word) {
                alternatives.last_mut().unwrap().push(term);
            }
        }
        alternatives.retain(|a| !a.is_empty());
        Query { alternatives }
    }

    /// The same search with each plain word of four letters or more
    /// allowed to be one letter off, to try when it finds nothing as
    /// typed; `None` when it has no such word.
    pub fn loosened(&self) -> Option<Query> {
        let mut changed = false;
        let alternatives = self
            .alternatives
            .iter()
            .map(|terms| {
                terms
                    .iter()
                    .map(|t| match &t.test {
                        Test::Anywhere(s) if !t.negated && s.chars().count() >= 4 => {
                            changed = true;
                            Term {
                                negated: false,
                                test: Test::Near(s.clone()),
                            }
                        }
                        _ => t.clone(),
                    })
                    .collect()
            })
            .collect();
        changed.then_some(Query { alternatives })
    }

    /// Whether the search has nothing in it, so everything matches.
    pub fn is_empty(&self) -> bool {
        self.alternatives.is_empty()
    }

    /// Whether the search asks for missing files, which are otherwise left
    /// out of every list but their own.
    pub fn wants_missing(&self) -> bool {
        self.alternatives
            .iter()
            .flatten()
            .any(|t| !t.negated && t.test == Test::Is(Flag::Missing))
    }

    /// Whether the search needs the track playing now.
    pub fn uses_deck(&self) -> bool {
        self.alternatives
            .iter()
            .flatten()
            .any(|t| t.test == Test::MixesWithDeck)
    }

    pub fn matches(&self, track: &Track, text: &Indexed, cx: &Context) -> bool {
        self.is_empty()
            || self.alternatives.iter().any(|terms| {
                terms
                    .iter()
                    .all(|t| t.matches(track, text, cx) != t.negated)
            })
    }
}

/// Two searches that must both match, as one search: each of `a`'s
/// alternatives with each of `b`'s (`x | y` and `z` is `x z | y z`).
pub fn and_text(a: &str, b: &str) -> String {
    let groups = |text: &str| {
        let mut out = vec![Vec::new()];
        for word in split(text) {
            if word == "|" || word == "OR" {
                out.push(Vec::new());
            } else {
                out.last_mut().unwrap().push(word);
            }
        }
        out.retain(|g| !g.is_empty());
        out
    };
    let (a, b) = (groups(a), groups(b));
    if a.is_empty() || b.is_empty() {
        let only = if a.is_empty() { b } else { a };
        return only
            .iter()
            .map(|g| g.join(" "))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    let mut out = Vec::new();
    for x in &a {
        for y in &b {
            out.push(format!("{} {}", x.join(" "), y.join(" ")));
        }
    }
    out.join(" | ")
}

/// Splits a search into words, keeping quoted text (`artist:"daft punk"`)
/// together. Quotes are kept so an empty `""` can be told apart.
fn split(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                word.push(c);
            }
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

fn parse_term(word: &str) -> Option<Term> {
    let (negated, word) = match word.strip_prefix('-') {
        Some(rest) if !rest.is_empty() => (true, rest),
        _ => (false, word),
    };
    let test = parse_test(word)?;
    Some(Term { negated, test })
}

fn parse_test(word: &str) -> Option<Test> {
    if word.eq_ignore_ascii_case("~deck") {
        return Some(Test::MixesWithDeck);
    }
    let Some((name, value)) = word.split_once(':').filter(|(n, _)| !n.contains('"')) else {
        let text = fold(&unquote(word));
        return (!text.is_empty()).then_some(Test::Anywhere(text));
    };
    let name = name.to_ascii_lowercase();
    let text_field = match name.as_str() {
        "artist" => Some(TextField::Artist),
        "title" => Some(TextField::Title),
        "album" => Some(TextField::Album),
        "genre" => Some(TextField::Genre),
        "comment" => Some(TextField::Comment),
        "grouping" => Some(TextField::Grouping),
        "tag" | "tags" | "label" => Some(TextField::Tag),
        "location" | "path" => Some(TextField::Location),
        "publisher" | "recordlabel" => Some(TextField::Publisher),
        "composer" => Some(TextField::Composer),
        "remixer" => Some(TextField::Remixer),
        n => user_field(n),
    };
    if let Some(field) = text_field {
        if value == "\"\"" {
            return Some(Test::Empty(field));
        }
        let text = |v: &str| {
            if field == TextField::Location {
                v.chars().map(unaccent).collect()
            } else {
                fold(v)
            }
        };
        if let Some(exact) = value.strip_prefix('=') {
            return Some(Test::Exactly(field, text(&unquote(exact))));
        }
        let v = text(&unquote(value));
        return (!v.is_empty()).then_some(Test::Contains(field, v));
    }
    let v = unquote(value);
    Some(match name.as_str() {
        "key" if v.is_empty() && value == "\"\"" => Test::Empty(TextField::Key),
        "key" => match Key::parse(&v) {
            Some(k) => Test::KeyIs(k),
            None if v.is_empty() => return None,
            None => Test::Contains(TextField::Key, v.to_lowercase()),
        },
        "~key" => Key::parse(&v).map_or(Test::Never, Test::KeyMixes),
        "~harmonic" => {
            Key::parse(&v).map_or(Test::Never, |k| Test::KeyMixesBy(k, Mixing::Harmonic))
        }
        "~fuzzy" => Key::parse(&v).map_or(Test::Never, |k| Test::KeyMixesBy(k, Mixing::Fuzzy)),
        "~bpm" => v
            .parse::<f64>()
            .ok()
            .filter(|b| *b > 0.0)
            .map_or(Test::Never, Test::BpmNear),
        "bpm" => number(&v, NumberField::Bpm, plain)?,
        "rating" => number(&v, NumberField::Rating, plain)?,
        "played" | "plays" => number(&v, NumberField::Played, plain)?,
        "duration" | "length" => number(&v, NumberField::Duration, seconds)?,
        "year" => number(&v, NumberField::Year, plain)?,
        "added" => added(&v, NumberField::AddedDaysAgo, NumberField::AddedAt)?,
        "lastplayed" => added(&v, NumberField::PlayedDaysAgo, NumberField::PlayedAt)?,
        "color" | "colour" => {
            if value == "\"\"" {
                Test::Color(None)
            } else {
                let name = v.to_ascii_lowercase();
                COLOR_NAMES
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map_or(Test::Never, |&(_, c)| Test::Color(Some(c)))
            }
        }
        "is" => match v.to_ascii_lowercase().as_str() {
            "missing" => Test::Is(Flag::Missing),
            "duplicate" | "dupe" | "duplicates" => Test::Is(Flag::Duplicate),
            "played" => Test::Is(Flag::Played),
            "cued" => Test::Is(Flag::Cued),
            "prepped" | "ready" => Test::Is(Flag::Prepped),
            "" => return None,
            other => crate::health::Check::from_name(other)
                .map_or(Test::Never, |c| Test::Is(Flag::Health(c))),
        },
        "crate" | "crates" if value == "\"\"" => Test::Crate(None),
        "crate" | "crates" => match value.strip_prefix('=') {
            Some(exact) => Test::Crate(Some((unquote(exact).to_lowercase(), true))),
            None if v.is_empty() => return None,
            None => Test::Crate(Some((v.to_lowercase(), false))),
        },
        "stage" => match v.to_ascii_lowercase().as_str() {
            "none" => Test::Stage(String::new()),
            "" => return None,
            name => match crate::stages::Stage::from_search(name) {
                Some(st) => Test::Stage(st.name().to_string()),
                None => Test::Never,
            },
        },
        // Not a field: search for the text as typed (a title with a colon).
        _ => {
            let text = fold(&unquote(word));
            if text.is_empty() {
                return None;
            }
            Test::Anywhere(text)
        }
    })
}

/// A tag group by its place (`tag2:`, or `user2:` as VirtualDJ's smart
/// lists say), or by the name the DJ gave it; grouping by its name.
fn user_field(name: &str) -> Option<TextField> {
    let names = field_names();
    let numbered = name
        .strip_prefix("tag")
        .or_else(|| name.strip_prefix("user"))
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|&n| n >= 1);
    if let Some(n) = numbered {
        // A place with no group matches no track.
        return Some(TextField::UserTag(
            names.tags.get(n - 1).map_or(-1, |g| g.id),
        ));
    }
    if keyword(&names.grouping) == name {
        return Some(TextField::Grouping);
    }
    names
        .tags
        .iter()
        .find(|g| keyword(&g.name) == name && !name.is_empty())
        .map(|g| TextField::UserTag(g.id))
}

/// Whether one of the track's tags (in `group`, or any with None)
/// contains (or is, if `whole`) `s`; a leading `#` is ignored.
fn has_tag(text: &Indexed, group: Option<i64>, s: &str, whole: bool) -> bool {
    let s = s.trim_start_matches('#');
    text.user
        .iter()
        .filter(|(g, _)| group.is_none_or(|group| *g == group))
        .any(|(_, t)| if whole { t == s } else { t.contains(s) })
}

fn plain(s: &str) -> Option<f64> {
    s.trim().replace(',', ".").parse().ok()
}

/// A duration: "3:30", "5m", "4m30s", "90s" or "240".
fn seconds(s: &str) -> Option<f64> {
    let s = s.trim().to_ascii_lowercase();
    if let Some((m, sec)) = s.split_once(':') {
        return Some(plain(m)? * 60.0 + plain(sec)?);
    }
    if let Some((m, rest)) = s.split_once('m') {
        let sec = rest.trim_end_matches('s');
        return Some(plain(m)? * 60.0 + if sec.is_empty() { 0.0 } else { plain(sec)? });
    }
    plain(s.trim_end_matches('s'))
}

/// `>5`, `<=3`, `120-130` or `128`, read with `read`.
fn compare(v: &str, read: fn(&str) -> Option<f64>) -> Option<Compare> {
    let v = v.trim();
    for (op, make) in [
        (">=", Compare::Ge as fn(f64) -> Compare),
        ("<=", Compare::Le),
        (">", Compare::Gt),
        ("<", Compare::Lt),
        ("=", Compare::Eq),
    ] {
        if let Some(rest) = v.strip_prefix(op) {
            return read(rest).map(make);
        }
    }
    if let Some((a, b)) = v.split_once('-') {
        if let (Some(a), Some(b)) = (read(a), read(b)) {
            return Some(Compare::Range(a.min(b), a.max(b)));
        }
    }
    read(v).map(Compare::Eq)
}

fn number(v: &str, field: NumberField, read: fn(&str) -> Option<f64>) -> Option<Test> {
    if v.is_empty() {
        return None;
    }
    Some(compare(v, read).map_or(Test::Never, |c| Test::Number(field, c)))
}

/// `added:<7d` (less than 7 days ago), `added:>2w`, `added:>=2026-01-31`;
/// `days_ago` and `at` are the fields compared.
fn added(v: &str, days_ago: NumberField, at: NumberField) -> Option<Test> {
    if v.is_empty() {
        return None;
    }
    let ago = |s: &str| {
        let s = s.trim().to_ascii_lowercase();
        if let Some(d) = s.strip_suffix('d') {
            plain(d)
        } else if let Some(w) = s.strip_suffix('w') {
            plain(w).map(|w| w * 7.0)
        } else {
            None
        }
    };
    if let Some(c) = compare(v, ago) {
        return Some(Test::Number(days_ago, c));
    }
    Some(compare(v, date).map_or(Test::Never, |c| Test::Number(at, c)))
}

/// A date as YYYY-MM-DD, as the Unix time it starts (UTC).
fn date(s: &str) -> Option<f64> {
    let mut parts = s.trim().splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400) as f64)
}

impl Compare {
    /// Whether `x` passes. An equal match on a whole number (`bpm:128`)
    /// takes anything that rounds to it.
    fn test(self, x: f64) -> bool {
        match self {
            Compare::Eq(v) if v.fract() == 0.0 => (x - v).abs() < 0.5,
            Compare::Eq(v) => (x - v).abs() < 0.05,
            Compare::Lt(v) => x < v,
            Compare::Le(v) => x <= v,
            Compare::Gt(v) => x > v,
            Compare::Ge(v) => x >= v,
            Compare::Range(a, b) => x >= a && x <= b,
        }
    }
}

/// Whether two tempos are within the tolerance, directly or at half or
/// double tempo.
pub fn tempos_mix(a: f64, b: f64) -> bool {
    a > 0.0
        && b > 0.0
        && [1.0, 2.0, 0.5]
            .iter()
            .any(|f| ((a * f - b) / b).abs() <= BPM_TOLERANCE)
}

impl Term {
    fn matches(&self, t: &Track, text: &Indexed, cx: &Context) -> bool {
        let field = |f: TextField| -> &str {
            match f {
                TextField::Artist => &text.artist,
                TextField::Title => &text.title,
                TextField::Album => &text.album,
                TextField::Genre => &text.genre,
                TextField::Comment => &text.comment,
                TextField::Grouping => &text.grouping,
                TextField::Tag => &text.tags,
                TextField::Key => "",
                TextField::Location => &text.location,
                TextField::Publisher => &text.publisher,
                TextField::Composer => &text.composer,
                TextField::Remixer => &text.remixer,
                TextField::UserTag(_) => "",
            }
        };
        let key = || Key::parse(&t.key);
        match &self.test {
            Test::Anywhere(s) => [
                &text.artist,
                &text.title,
                &text.album,
                &text.genre,
                &text.comment,
                &text.grouping,
                &text.tags,
            ]
            .iter()
            .any(|f| f.contains(s.as_str())),
            Test::Near(s) => {
                let s: Vec<char> = s.chars().collect();
                [
                    &text.artist,
                    &text.title,
                    &text.album,
                    &text.genre,
                    &text.comment,
                    &text.grouping,
                    &text.tags,
                ]
                .iter()
                .any(|f| near(&s, f))
            }
            Test::Contains(TextField::Key, s) => t.key.to_lowercase().contains(s.as_str()),
            Test::Contains(TextField::Tag, s) => has_tag(text, None, s, false),
            Test::Contains(TextField::UserTag(g), s) => has_tag(text, Some(*g), s, false),
            Test::Contains(f, s) => field(*f).contains(s.as_str()),
            Test::Exactly(TextField::Key, s) => t.key.to_lowercase() == *s,
            Test::Exactly(TextField::Tag, s) => has_tag(text, None, s, true),
            Test::Exactly(TextField::UserTag(g), s) => has_tag(text, Some(*g), s, true),
            Test::Exactly(f, s) => field(*f) == s,
            Test::Empty(TextField::Key) => t.key.trim().is_empty(),
            Test::Empty(TextField::Tag) => text.user.is_empty(),
            Test::Empty(TextField::UserTag(g)) => !text.user.iter().any(|(t, _)| t == g),
            Test::Empty(f) => field(*f).trim().is_empty(),
            Test::Number(f, c) => {
                let x = match f {
                    NumberField::Bpm => t.bpm,
                    NumberField::Rating => t.rating as f64,
                    NumberField::Played => t.info.play_count as f64,
                    NumberField::Duration => t.duration,
                    NumberField::AddedDaysAgo if t.info.added <= 0 => return false,
                    NumberField::AddedDaysAgo => (cx.now - t.info.added) as f64 / 86_400.0,
                    NumberField::AddedAt if t.info.added <= 0 => return false,
                    NumberField::AddedAt => t.info.added as f64,
                    // The year starts the field ("1997", "1997-05-02").
                    NumberField::Year => match t.info.year.get(..4).and_then(|y| y.parse().ok()) {
                        Some(y) => y,
                        None => return false,
                    },
                    NumberField::PlayedDaysAgo | NumberField::PlayedAt => {
                        match t.info.last_played.filter(|&p| p > 0) {
                            None => return false,
                            Some(p) if *f == NumberField::PlayedAt => p as f64,
                            Some(p) => (cx.now - p) as f64 / 86_400.0,
                        }
                    }
                };
                // A date matches the whole day: >=, < and = compare days.
                if matches!(f, NumberField::AddedAt | NumberField::PlayedAt) {
                    let day = (x / 86_400.0).floor() * 86_400.0;
                    return match *c {
                        Compare::Eq(v) => day == v,
                        Compare::Gt(v) => day > v,
                        Compare::Le(v) => day <= v,
                        other => other.test(day),
                    };
                }
                c.test(x)
            }
            Test::KeyIs(k) => key() == Some(*k),
            Test::KeyMixes(k) => key().is_some_and(|own| own.compatible(*k)),
            Test::KeyMixesBy(k, m) => key().is_some_and(|own| own.mixes(*k, *m)),
            Test::BpmNear(b) => tempos_mix(t.bpm, *b),
            Test::MixesWithDeck => match cx.deck {
                _ if cx.deck_track == Some(t.id) => false,
                Some((bpm, k)) => {
                    let key_ok = match (k, key()) {
                        (Some(a), Some(b)) => a.compatible(b),
                        // Without keys, tempo alone decides.
                        _ => true,
                    };
                    key_ok && (bpm <= 0.0 || tempos_mix(t.bpm, bpm))
                }
                None => false,
            },
            Test::Color(c) => t.info.color == *c,
            Test::Is(Flag::Missing) => t.info.missing,
            Test::Is(Flag::Duplicate) => cx.duplicates.is_some_and(|d| d.contains(&t.id)),
            Test::Is(Flag::Played) => t.info.play_count > 0,
            Test::Is(Flag::Cued) => !t.info.hotcues.is_empty(),
            Test::Is(Flag::Prepped) => crate::stages::Prep::of(t).done(),
            Test::Is(Flag::Health(check)) => {
                let none = HashSet::new();
                let duplicates = cx.duplicates.unwrap_or(&none);
                match cx.health {
                    Some(facts) => facts.finds(*check, t, duplicates),
                    None => crate::health::Facts::default().finds(*check, t, duplicates),
                }
            }
            Test::Stage(s) => t.info.stage == *s,
            Test::Crate(want) => {
                let names = cx.crates.and_then(|c| c.get(&t.id));
                match want {
                    None => names.is_none_or(|n| n.is_empty()),
                    Some((name, exact)) => names.is_some_and(|n| {
                        n.iter().any(|c| {
                            let c = c.to_lowercase();
                            if *exact {
                                c == *name
                            } else {
                                c.contains(name.as_str())
                            }
                        })
                    }),
                }
            }
            Test::Never => false,
        }
    }
}

/// Text as a search compares it: lowercase, without accents ("Ólafur
/// Arnalds" is "olafur arnalds"), without punctuation ("AC/DC" is "acdc",
/// "Don't" is "dont", a leading `#` goes), and with the words DJs write
/// several ways made one: "&" is "and", "ft", "ft." and "featuring" are
/// "feat". Words are kept apart by single spaces.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for word in s.split_whitespace() {
        let word: String = word
            .chars()
            .flat_map(char::to_lowercase)
            .flat_map(Unaccented::new)
            .filter(|c| c.is_alphanumeric() || *c == '&')
            .collect();
        let word = match word.as_str() {
            "&" => "and",
            "ft" | "featuring" => "feat",
            w => w.trim_matches('&'),
        };
        if word.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// A lowercase letter without its accent, as one or two letters.
struct Unaccented {
    first: Option<char>,
    second: Option<char>,
}

impl Unaccented {
    fn new(c: char) -> Self {
        let pair = |a, b| Unaccented {
            first: Some(a),
            second: Some(b),
        };
        match c {
            'æ' => pair('a', 'e'),
            'œ' => pair('o', 'e'),
            'ß' => pair('s', 's'),
            'þ' => pair('t', 'h'),
            'ĳ' => pair('i', 'j'),
            c => Unaccented {
                first: Some(unaccent(c)),
                second: None,
            },
        }
    }
}

impl Iterator for Unaccented {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        self.first.take().or_else(|| self.second.take())
    }
}

/// A letter without its accent: é is e, ø is o, ł is l. Lowercases too.
fn unaccent(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' | 'ð' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => 's',
        'ţ' | 'ť' | 'ŧ' | 'ț' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        c => c,
    }
}

/// Whether `word` is in `text` with at most one letter wrong, missing,
/// extra, or two letters swapped (Sellers' approximate search with the
/// optimal string alignment distance).
fn near(word: &[char], text: &str) -> bool {
    let m = word.len();
    if m == 0 {
        return true;
    }
    // Three rows of the table: the one before last, the last and this one.
    // Row i is how far word[..i] is from the best stretch of text ending
    // at each letter; a stretch may start anywhere, so row 0 is all zeros.
    let mut before = vec![0usize; m + 1];
    let mut last: Vec<usize> = (0..=m).collect();
    let mut this = vec![0usize; m + 1];
    let mut prev_c = None;
    for c in text.chars() {
        this[0] = 0;
        for i in 1..=m {
            let cost = usize::from(word[i - 1] != c);
            let mut d = (last[i - 1] + cost).min(last[i] + 1).min(this[i - 1] + 1);
            if i > 1 && Some(word[i - 1]) == prev_c && word[i - 2] == c {
                d = d.min(before[i - 2] + 1);
            }
            this[i] = d;
        }
        if this[m] <= 1 {
            return true;
        }
        std::mem::swap(&mut before, &mut last);
        std::mem::swap(&mut last, &mut this);
        prev_c = Some(c);
    }
    false
}

/// A title or artist reduced to what identifies the song: lowercase,
/// without punctuation, "the", or a trailing "(Original Mix)".
pub(crate) fn normalized(s: &str) -> String {
    let lower = s.to_lowercase();
    let lower = lower
        .replace("(original mix)", "")
        .replace("(original)", "")
        .replace("original mix", "");
    let words: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && *w != "the")
        .map(str::to_string)
        .collect();
    words.join(" ")
}

/// Ids of tracks that are in the library more than once: the same artist
/// and title (ignoring case, punctuation and "Original Mix") with lengths
/// within 3 seconds, or the same file. Missing files are not counted.
pub fn duplicates<'a>(tracks: impl IntoIterator<Item = &'a Track>) -> HashSet<usize> {
    let mut groups: std::collections::HashMap<(String, String), Vec<&Track>> = Default::default();
    for t in tracks {
        if t.info.missing || t.title.trim().is_empty() {
            continue;
        }
        groups
            .entry((normalized(&t.artist), normalized(&t.title)))
            .or_default()
            .push(t);
    }
    let mut out = HashSet::new();
    for group in groups.values().filter(|g| g.len() > 1) {
        for (i, a) in group.iter().enumerate() {
            let twin = group.iter().enumerate().any(|(j, b)| {
                i != j
                    && (a.duration <= 0.0
                        || b.duration <= 0.0
                        || (a.duration - b.duration).abs() <= 3.0)
            });
            if twin {
                out.insert(a.id);
            }
        }
    }
    out
}

/// Whether a track's file is on disk. Built-in sample tracks always are.
pub fn file_exists(t: &Track) -> bool {
    match &t.source {
        Source::File(p) => Path::exists(p),
        Source::Synth { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::Hotcue;
    use std::path::PathBuf;

    fn track(id: usize, artist: &str, title: &str, bpm: f64, key: &str) -> Track {
        let mut t = std::sync::Arc::unwrap_or_clone(djsugar_engine::sample_library().remove(0));
        t.id = id;
        t.artist = artist.into();
        t.title = title.into();
        t.album = "Album".into();
        t.genre = "House".into();
        t.bpm = bpm;
        t.key = key.into();
        t.duration = 300.0;
        t.rating = 0;
        t.source = Source::File(PathBuf::from(format!("/music/{artist} - {title}.mp3")));
        t
    }

    fn find(tracks: &[Track], q: &str) -> Vec<usize> {
        find_with(
            tracks,
            q,
            &Context {
                now: 1_800_000_000,
                ..Default::default()
            },
        )
    }

    fn find_with(tracks: &[Track], q: &str, cx: &Context) -> Vec<usize> {
        let q = Query::parse(q);
        tracks
            .iter()
            .filter(|t| q.matches(t, &Indexed::new(t), cx))
            .map(|t| t.id)
            .collect()
    }

    fn library() -> Vec<Track> {
        let mut a = track(1, "Daft Punk", "One More Time", 123.0, "8A");
        a.rating = 5;
        a.info.play_count = 3;
        a.info.set_tags_in(1, ["vocal".into()]);
        a.info.set_tags_in(2, ["peak".into()]);
        a.info.color = Some(0xff3b30);
        a.info.added = 1_800_000_000 - 2 * 86_400;
        let mut b = track(2, "Moodymann", "Shades of Jae", 118.0, "Dm");
        b.genre = "Deep House".into();
        b.duration = 420.0;
        b.info.comment = "warm up".into();
        b.info.added = 1_800_000_000 - 40 * 86_400;
        b.info.year = "1997".into();
        b.info.label = "KDJ".into();
        b.info.last_played = Some(1_800_000_000 - 3 * 86_400);
        let mut c = track(3, "Bicep", "Glue", 130.0, "9A");
        c.album = String::new();
        c.info.hotcues = vec![Hotcue {
            number: 0,
            seconds: 1.0,
            ..Default::default()
        }];
        let d = track(
            4,
            "Floating Points",
            "Silhouettes (I, II & III)",
            62.0,
            "3B",
        );
        vec![a, b, c, d]
    }

    #[test]
    fn forgives_accents_punctuation_and_spelling() {
        let tracks = vec![
            track(1, "Ólafur Arnalds", "Saman", 70.0, "1A"),
            track(2, "AC/DC", "Thunderstruck", 134.0, "5B"),
            track(3, "Sigur Rós", "Hoppípolla", 90.0, "2A"),
            track(4, "Simon & Garfunkel", "Don't Know", 100.0, "8B"),
            track(5, "Bonobo ft. Andreya Triana", "The Keeper", 120.0, "9A"),
            track(6, "Nils Frahm", "Says", 74.0, "4A"),
        ];
        assert_eq!(find(&tracks, "olafur"), vec![1]);
        assert_eq!(find(&tracks, "ÓLAFUR"), vec![1]);
        assert_eq!(find(&tracks, "acdc"), vec![2]);
        assert_eq!(find(&tracks, "ac/dc"), vec![2]);
        assert_eq!(find(&tracks, "sigur ros hoppipolla"), vec![3]);
        assert_eq!(find(&tracks, "simon and garfunkel"), vec![4]);
        assert_eq!(find(&tracks, "\"simon & garfunkel\""), vec![4]);
        assert_eq!(find(&tracks, "dont"), vec![4]);
        assert_eq!(find(&tracks, "\"bonobo feat andreya\""), vec![5]);
        assert_eq!(find(&tracks, "artist:featuring"), vec![5]);
        // Punctuation alone is not a search.
        assert_eq!(find(&tracks, "-").len(), 6);

        // A letter off finds nothing as typed, and something loosened.
        let loose = |q: &str| {
            let q = Query::parse(q).loosened().expect("has a long word");
            tracks
                .iter()
                .filter(|t| q.matches(t, &Indexed::new(t), &Context::default()))
                .map(|t| t.id)
                .collect::<Vec<_>>()
        };
        assert!(find(&tracks, "frahn").is_empty());
        assert_eq!(loose("frahn"), vec![6]); // wrong letter
        assert_eq!(loose("fraahm"), vec![6]); // extra letter
        assert_eq!(loose("thundrstruck"), vec![2]); // missing letter
        assert_eq!(loose("arnadls"), vec![1]); // swapped letters
        assert_eq!(loose("nils frahn"), vec![6]);
        assert!(loose("frxhx").is_empty()); // two letters off
                                            // Short words and left-out words stay as typed.
        assert_eq!(Query::parse("abc -frahn").loosened(), None);
        assert_eq!(loose("frahn says").len(), 1);
        assert!(loose("frahn moodymann").is_empty());
    }

    #[test]
    fn words_fields_and_negation() {
        let lib = library();
        assert_eq!(find(&lib, ""), [1, 2, 3, 4]);
        assert_eq!(find(&lib, "daft"), [1]);
        assert_eq!(find(&lib, "house"), [1, 2, 3, 4]);
        assert_eq!(find(&lib, "genre:deep"), [2]);
        assert_eq!(find(&lib, "-genre:deep"), [1, 3, 4]);
        assert_eq!(find(&lib, "artist:\"daft punk\" time"), [1]);
        assert_eq!(find(&lib, "\"one more\""), [1]);
        assert_eq!(find(&lib, "title:=glue"), [3]);
        assert_eq!(find(&lib, "title:=glu"), Vec::<usize>::new());
        assert_eq!(find(&lib, "album:\"\""), [3]);
        assert_eq!(find(&lib, "-album:\"\""), [1, 2, 4]);
        assert_eq!(find(&lib, "warm"), [2]);
        assert_eq!(find(&lib, "tag:vocal"), [1]);
        assert_eq!(find(&lib, "tag:#peak"), [1]);
        assert_eq!(find(&lib, "location:bicep"), [3]);
        // A colon that isn't a field is just text.
        assert_eq!(find(&lib, "re:edit"), Vec::<usize>::new());
    }

    #[test]
    fn tag_fields_by_number_and_name() {
        let mut lib = library();
        lib[1]
            .info
            .set_tags_in(1, ["Peak Time".into(), "dark".into()]);
        lib[1].info.set_tags_in(7, ["dusk".into()]);
        lib[1].info.grouping = "4 PEAK".into();
        set_field_names(FieldNames {
            grouping: "Trip Stage".into(),
            tags: vec![
                TagGroup::new(1, "Mood"),
                TagGroup::new(2, "Instrument"),
                TagGroup::new(7, "Time of day"),
            ],
            tags_revision: 0,
            tag_colors: Vec::new(),
        });
        assert_eq!(find(&lib, "tag1:peak"), [2]);
        assert_eq!(find(&lib, "tag:peak"), [1, 2], "tag: searches every group");
        assert_eq!(find(&lib, "tag:=\"peak time\""), [2]);
        assert_eq!(find(&lib, "tag2:=peak"), [1]);
        assert_eq!(find(&lib, "tag2:=#peak"), [1]);
        assert_eq!(find(&lib, "user1:=peak"), Vec::<usize>::new());
        assert_eq!(find(&lib, "tag3:dusk"), [2]);
        assert_eq!(find(&lib, "tag9:dusk"), Vec::<usize>::new());
        assert_eq!(find(&lib, "tag2:\"\""), [2, 3, 4]);
        assert_eq!(find(&lib, "mood:dark"), [2]);
        assert_eq!(find(&lib, "timeofday:=dusk"), [2]);
        assert_eq!(find(&lib, "tripstage:=\"4 peak\""), [2]);
        assert_eq!(
            find(&lib, "grouping:peak"),
            [2],
            "the built-in name still works"
        );
        set_field_names(FieldNames::default());
        assert_eq!(and_text("a | b", "c d | e"), "a c d | a e | b c d | b e");
        assert_eq!(
            and_text("", "genre:=\"deep house\""),
            "genre:=\"deep house\""
        );
        assert_eq!(and_text("x", ""), "x");
        assert_eq!(keyword("Trip Stage"), "tripstage");
        assert_eq!(quoted("4 PEAK"), "\"4 PEAK\"");
        assert_eq!(quoted("House"), "House");
    }

    #[test]
    fn numbers_ranges_and_or() {
        let lib = library();
        assert_eq!(find(&lib, "bpm:123"), [1]);
        assert_eq!(find(&lib, "bpm:120-130"), [1, 3]);
        assert_eq!(find(&lib, "bpm:>=123"), [1, 3]);
        assert_eq!(find(&lib, "bpm:<100"), [4]);
        assert_eq!(find(&lib, "rating:>=4"), [1]);
        assert_eq!(find(&lib, "played:0"), [2, 3, 4]);
        assert_eq!(find(&lib, "duration:>6m"), [2]);
        assert_eq!(find(&lib, "duration:<5:01"), [1, 3, 4]);
        assert_eq!(find(&lib, "bpm:118 | bpm:130"), [2, 3]);
        assert_eq!(find(&lib, "glue OR daft"), [1, 3]);
        assert_eq!(find(&lib, "added:<7d"), [1]);
        assert_eq!(find(&lib, "added:>4w"), [2]);
        // 1_800_000_000 is 2027-01-15; track 1 was added 2027-01-13.
        assert_eq!(find(&lib, "added:>=2027-01-13"), [1]);
        assert_eq!(find(&lib, "added:2027-01-13"), [1]);
        assert_eq!(find(&lib, "year:<2000"), [2]);
        assert_eq!(find(&lib, "publisher:kdj"), [2]);
        assert_eq!(find(&lib, "lastplayed:<7d"), [2]);
        // Something that can't be read finds nothing rather than everything.
        assert_eq!(find(&lib, "bpm:fast"), Vec::<usize>::new());
    }

    #[test]
    fn keys_tempos_colours_and_flags() {
        let lib = library();
        assert_eq!(find(&lib, "key:8A"), [1]);
        assert_eq!(find(&lib, "key:Am"), [1]);
        // Dm is 7A; it mixes with 8A, and so does 9A.
        assert_eq!(find(&lib, "~key:8A"), [1, 2, 3]);
        assert_eq!(find(&lib, "~bpm:125"), [1, 2, 3, 4]);
        // 62 BPM is 124 at double tempo.
        assert_eq!(find(&lib, "~bpm:130"), [1, 3, 4]);
        assert_eq!(find(&lib, "color:red"), [1]);
        assert_eq!(find(&lib, "color:\"\""), [2, 3, 4]);
        assert_eq!(find(&lib, "is:played"), [1]);
        assert_eq!(find(&lib, "is:cued"), [3]);
        assert_eq!(find(&lib, "is:unrated"), find(&lib, "rating:0"));
        assert!(find(&lib, "is:nosuchcheck").is_empty());
        let cx = Context {
            now: 0,
            deck: Some((128.0, Key::parse("8A"))),
            deck_track: None,
            duplicates: None,
            health: None,
            crates: None,
        };
        assert_eq!(find_with(&lib, "~deck", &cx), [1, 3]);
        let playing_3 = Context {
            deck_track: Some(3),
            ..cx
        };
        assert_eq!(
            find_with(&lib, "~deck", &playing_3),
            [1],
            "not the track itself"
        );
        assert_eq!(find_with(&lib, "~deck -artist:bicep", &cx), [1]);
        assert!(find(&lib, "~deck").is_empty(), "nothing playing");
    }

    #[test]
    fn finds_duplicates_and_missing_files() {
        let mut lib = library();
        let mut again = track(5, "Daft Punk", "One More Time (Original Mix)", 123.0, "8A");
        again.duration = 301.5;
        let mut edit = track(6, "Bicep", "Glue", 130.0, "9A");
        edit.duration = 200.0;
        let mut gone = track(7, "Daft punk", "One more time", 123.0, "8A");
        gone.info.missing = true;
        lib.extend([again, edit, gone]);
        let dupes = duplicates(&lib);
        let mut ids: Vec<_> = dupes.iter().copied().collect();
        ids.sort();
        assert_eq!(ids, [1, 5], "a different length is a different edit");
        let cx = Context {
            duplicates: Some(&dupes),
            ..Default::default()
        };
        assert_eq!(find_with(&lib, "is:duplicate", &cx), [1, 5]);
        assert_eq!(find(&lib, "is:missing"), [7]);
        assert!(Query::parse("is:missing artist:daft").wants_missing());
        assert!(!Query::parse("-is:missing").wants_missing());
    }

    #[test]
    fn finds_tracks_by_crate() {
        let lib = library();
        let crates = HashMap::from([
            (1, vec!["Yoga".to_string(), "Deep Yoga".to_string()]),
            (2, vec!["Dance".to_string()]),
        ]);
        let cx = Context {
            crates: Some(&crates),
            ..Default::default()
        };
        assert_eq!(find_with(&lib, "crate:yoga", &cx), [1]);
        assert_eq!(find_with(&lib, "crate:=dance", &cx), [2]);
        assert!(find_with(&lib, "crate:=danc", &cx).is_empty());
        assert_eq!(find_with(&lib, "crate:\"\"", &cx), [3, 4]);
        assert_eq!(find_with(&lib, "-crate:yoga", &cx), [2, 3, 4]);
    }
}
