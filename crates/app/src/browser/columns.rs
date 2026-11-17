//! The track list's columns: which are shown, in what order and how wide,
//! as the DJ sets them from the header (right-click to pick, drag an edge
//! to resize, drag a heading to move it). Kept in settings as `columns`.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use djsugar_engine::key::Key;
use djsugar_engine::Track;
use djsugar_library::query::FieldNames;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{settings, AppWindow, ColumnUi, ListColumn, TrackRow};

/// How a column is drawn.
pub const TEXT: i32 = 0;
/// Takes the room left over.
pub const TITLE: i32 = 1;
pub const RATING: i32 = 2;
pub const KEY: i32 = 3;
/// Quieter text: genre, comment and the like.
pub const DIM: i32 = 4;
/// A tag field's words, in lilac.
pub const TAG_WORDS: i32 = 5;
/// The track's mini waveform.
pub const WAVEFORM: i32 = 6;
/// The track's color, as a dot.
pub const COLOR_DOT: i32 = 7;

/// Column ids. The first eight are the list's original sort columns, so
/// sort orders saved before stay as they were.
pub const ARTIST: i32 = 0;
pub const TITLE_ID: i32 = 1;
pub const ALBUM: i32 = 2;
pub const GENRE: i32 = 3;
pub const BPM: i32 = 4;
pub const KEY_ID: i32 = 5;
pub const LENGTH: i32 = 6;
pub const RATING_ID: i32 = 7;
pub const GROUPING: i32 = 8;
pub const YEAR: i32 = 9;
pub const PLAYED: i32 = 10;
pub const ADDED: i32 = 11;
pub const LAST_PLAYED: i32 = 12;
pub const COMMENT: i32 = 13;
pub const LABEL: i32 = 14;
pub const REMIX: i32 = 15;
pub const COMPOSER: i32 = 16;
pub const TRACK_NUMBER: i32 = 17;
/// Basic preparation for the collection: tags, rating, tempo and key.
pub const PREP: i32 = 18;
/// The track's mini waveform, once analysed.
pub const WAVE: i32 = 19;
/// Where the file is: its folder and name, e.g. "_Staging/Kerala.flac".
pub const LOCATION: i32 = 20;
/// The crates the track is in (see [`djsugar_library::sets`]).
pub const CRATES: i32 = 21;
/// Where the track is in the list shown: for a history session, the order
/// it was played in.
pub const ORDER: i32 = 22;
/// When a history session's track was played.
pub const PLAYED_AT: i32 = 23;
/// The color the DJ gave the track.
pub const COLOR: i32 = 24;
/// The beat grid: checked (locked or edited), unsure, or in sections.
pub const GRID: i32 = 25;
/// How many hot cues the track has.
pub const CUES: i32 = 26;
/// Genre and at least one of the DJ's tags.
pub const TAGGED: i32 = 27;
/// Whether the file has a cover, once Collection Health has looked.
pub const COVER: i32 = 28;
/// How many Prep checklist lines are ticked, of how many.
pub const READY: i32 = 29;
/// The DJ's own score, from the Prep checklist.
pub const SCORE: i32 = 30;
/// In a Collection Health area: what the check found, and what its fix
/// would do.
pub const PROBLEM: i32 = 31;
pub const FIX: i32 = 32;
/// Tag field `i` (from 0) is column `TAGS + i`.
pub const TAGS: i32 = 100;

/// A column the list can show.
#[derive(Clone, Debug, PartialEq)]
pub struct Def {
    pub id: i32,
    pub label: String,
    pub width: f32,
    pub right: bool,
    pub kind: i32,
}

/// Every column, in the order the header's menu lists them; grouping and
/// the tag groups go by the names the DJ gave them (a group's column by
/// its place among them).
pub fn all(names: &FieldNames) -> Vec<Def> {
    let d = |id, label: &str, width, right, kind| Def {
        id,
        label: label.into(),
        width,
        right,
        kind,
    };
    let mut out = vec![
        d(COLOR, "Color", 30.0, false, COLOR_DOT),
        d(RATING_ID, "Rating", 64.0, false, RATING),
        d(TITLE_ID, "Title", 0.0, false, TITLE),
        d(ARTIST, "Artist", 170.0, false, TEXT),
        d(ALBUM, "Album", 150.0, false, TEXT),
        d(REMIX, "Remix", 120.0, false, TEXT),
        d(LENGTH, "Length", 60.0, true, TEXT),
        d(BPM, "Bpm", 56.0, true, TEXT),
        d(KEY_ID, "Key", 56.0, false, KEY),
        d(GENRE, "Genre", 110.0, false, DIM),
        d(GROUPING, &names.grouping, 90.0, false, TEXT),
        d(WAVE, "Waveform", 120.0, false, WAVEFORM),
    ];
    for (i, group) in names.tags.iter().enumerate() {
        out.push(d(TAGS + i as i32, &group.name, 140.0, false, TAG_WORDS));
    }
    out.extend([
        d(YEAR, "Year", 48.0, true, DIM),
        d(TRACK_NUMBER, "Track #", 52.0, true, DIM),
        d(PREP, "Prep", 52.0, false, TEXT),
        d(GRID, "Grid", 84.0, false, TEXT),
        d(CUES, "Cues", 44.0, true, TEXT),
        d(TAGGED, "Tags", 44.0, false, TEXT),
        d(COVER, "Cover", 48.0, false, TEXT),
        d(PLAYED, "Plays", 48.0, true, DIM),
        d(ADDED, "Added", 86.0, false, DIM),
        d(LAST_PLAYED, "Last Played", 86.0, false, DIM),
        d(LABEL, "Label", 110.0, false, DIM),
        d(COMPOSER, "Composer", 110.0, false, DIM),
        d(COMMENT, "Comment", 160.0, false, DIM),
        d(LOCATION, "Location", 200.0, false, DIM),
        d(CRATES, "Crates", 160.0, false, TAG_WORDS),
        d(ORDER, "#", 34.0, true, DIM),
        d(PLAYED_AT, "Played At", 70.0, false, DIM),
        d(PROBLEM, "Problem", 190.0, false, TEXT),
        d(FIX, "Fix", 260.0, false, DIM),
    ]);
    // The checklist's columns, with the Prep layout.
    if crate::PREP_OFFERED {
        out.extend([
            d(READY, "Ready", 52.0, true, TEXT),
            d(SCORE, "Score", 48.0, true, TEXT),
        ]);
    }
    out
}

/// The columns shown when the DJ hasn't chosen. Genre comes before the
/// mini waveform, so a narrow window cuts off the waveform, not the genre.
pub const DEFAULT: [i32; 9] = [
    COLOR, RATING_ID, TITLE_ID, ARTIST, LENGTH, BPM, KEY_ID, GENRE, WAVE,
];

/// The columns a history session shows until the DJ changes them: the
/// order and time each track was played first.
pub const HISTORY_DEFAULT: [i32; 8] = [
    ORDER, PLAYED_AT, TITLE_ID, ARTIST, LENGTH, BPM, KEY_ID, GENRE,
];

/// The columns the In Prep list shows until the DJ changes them: what is
/// left to prepare on each track.
/// (Ready and Score come back with the Prep layout, in v0.6.)
pub const PREP_DEFAULT: [i32; 11] = [
    COLOR, RATING_ID, TITLE_ID, ARTIST, BPM, KEY_ID, GRID, CUES, TAGGED, COVER, GENRE,
];

/// The columns a Collection Health area shows until the DJ changes them:
/// what's wrong with each track and what the fix would do.
pub const HEALTH_DEFAULT: [i32; 6] = [COLOR, TITLE_ID, ARTIST, PROBLEM, FIX, LENGTH];

/// Narrowest a column can be dragged.
pub const MIN_WIDTH: f32 = 30.0;

/// The columns shown, in order, with their widths, and where the hidden
/// ones were so they come back there.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Layout {
    pub shown: Vec<(i32, f32)>,
    pub hidden: Vec<Hidden>,
}

/// A column the DJ hid: its width and the column it was right of (`None`
/// when it was first), so showing it again puts it back there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hidden {
    pub id: i32,
    pub width: f32,
    pub after: Option<i32>,
}

impl Layout {
    /// The saved layout, or the default; unknown columns are dropped and
    /// the title is always there.
    pub fn new(saved: Vec<(i32, f32)>, hidden: Vec<Hidden>, defs: &[Def]) -> Self {
        Self::new_or(saved, hidden, defs, &DEFAULT)
    }

    /// The saved layout, or `default`.
    pub fn new_or(
        saved: Vec<(i32, f32)>,
        hidden: Vec<Hidden>,
        defs: &[Def],
        default: &[i32],
    ) -> Self {
        let mut l = Layout {
            shown: saved,
            hidden,
        };
        if l.shown.is_empty() {
            l = Self::with(defs, default);
        }
        l.fit(defs);
        l
    }

    fn with(defs: &[Def], ids: &[i32]) -> Self {
        Layout {
            shown: ids
                .iter()
                .filter_map(|id| defs.iter().find(|d| d.id == *id))
                .map(|d| (d.id, d.width))
                .collect(),
            hidden: Vec::new(),
        }
    }

    /// Drops columns that no longer exist (a tag field removed) and puts
    /// the title back if it went missing.
    pub fn fit(&mut self, defs: &[Def]) {
        let mut seen = Vec::new();
        self.shown.retain(|(id, _)| {
            defs.iter().any(|d| d.id == *id) && !seen.contains(id) && {
                seen.push(*id);
                true
            }
        });
        if !self.shown.iter().any(|(id, _)| *id == TITLE_ID) {
            self.shown.insert(0, (TITLE_ID, 0.0));
        }
        let shown: Vec<i32> = self.shown.iter().map(|(id, _)| *id).collect();
        let mut seen = Vec::new();
        self.hidden.retain(|h| {
            defs.iter().any(|d| d.id == h.id)
                && !shown.contains(&h.id)
                && !seen.contains(&h.id)
                && {
                    seen.push(h.id);
                    true
                }
        });
    }

    /// Shows the Color column first (`on`) or hides it. Returns whether
    /// that changed anything.
    pub fn show_color(&mut self, on: bool, defs: &[Def]) -> bool {
        if self.has(COLOR) == on {
            return false;
        }
        if on {
            let width = defs
                .iter()
                .find(|d| d.id == COLOR)
                .map_or(30.0, |d| d.width);
            self.shown.insert(0, (COLOR, width));
        } else {
            self.shown.retain(|(id, _)| *id != COLOR);
        }
        true
    }

    pub fn has(&self, id: i32) -> bool {
        self.shown.iter().any(|(c, _)| *c == id)
    }

    /// Hides a column, or shows it again where it was when hidden (a new
    /// one goes at the end); the title stays.
    pub fn toggle(&mut self, id: i32, defs: &[Def]) {
        if id == TITLE_ID {
            return;
        }
        match self.shown.iter().position(|(c, _)| *c == id) {
            Some(i) => {
                let (_, width) = self.shown.remove(i);
                let after = i.checked_sub(1).map(|l| self.shown[l].0);
                self.hidden.retain(|h| h.id != id);
                self.hidden.push(Hidden { id, width, after });
            }
            None => match self.hidden.iter().position(|h| h.id == id) {
                Some(h) => {
                    let h = self.hidden.remove(h);
                    let at = self.place_after(h.after);
                    self.shown.insert(at, (id, h.width));
                }
                None => {
                    if let Some(d) = defs.iter().find(|d| d.id == id) {
                        self.shown.push((id, d.width));
                    }
                }
            },
        }
    }

    /// Where a column that was right of `after` goes now: right of it if
    /// it's shown, else right of where that one was in turn (it may be
    /// hidden too), first when nothing was left of it.
    fn place_after(&self, mut after: Option<i32>) -> usize {
        for _ in 0..=self.hidden.len() {
            let Some(left) = after else {
                return 0;
            };
            if let Some(i) = self.shown.iter().position(|(c, _)| *c == left) {
                return i + 1;
            }
            match self.hidden.iter().find(|h| h.id == left) {
                Some(h) => after = h.after,
                None => break,
            }
        }
        self.shown.len()
    }

    pub fn resize(&mut self, index: usize, width: f32) {
        if let Some(c) = self.shown.get_mut(index) {
            let min = if c.0 == TITLE_ID { 80.0 } else { MIN_WIDTH };
            c.1 = width.max(min).round();
        }
    }

    /// Where a column dropped at `x` along the header goes among the columns
    /// as they are now (before the first whose middle is right of `x`), and
    /// where that slot starts, given the title's width.
    pub fn slot(&self, x: f32, title_width: f32, spacing: f32) -> (usize, f32) {
        let mut left = 0.0;
        for (i, (id, w)) in self.shown.iter().enumerate() {
            let w = if *id == TITLE_ID { title_width } else { *w };
            if x < left + w / 2.0 {
                return (i, left);
            }
            left += w + spacing;
        }
        (self.shown.len(), left)
    }

    /// Moves the column at `from` to where `x` falls along the header,
    /// given the title's width.
    pub fn move_to(&mut self, from: usize, x: f32, title_width: f32, spacing: f32) {
        if from >= self.shown.len() {
            return;
        }
        let (to, _) = self.slot(x, title_width, spacing);
        let to = if to > from { to - 1 } else { to };
        if to == from {
            return;
        }
        let c = self.shown.remove(from);
        // Hidden columns that were right of this one stay where they were.
        let left = from.checked_sub(1).map(|l| self.shown[l].0);
        for h in &mut self.hidden {
            if h.after == Some(c.0) {
                h.after = left;
            }
        }
        self.shown.insert(to.min(self.shown.len()), c);
    }
}

/// What a column shows for a track in the library. Columns the list fills
/// from its own row (title, artist, album, genre, length, BPM, key) aren't
/// asked for.
pub fn cell(id: i32, t: &Track, camelot: bool) -> String {
    let i = &t.info;
    let date = |at: i64| {
        use chrono::TimeZone;
        match chrono::Local.timestamp_opt(at, 0).single() {
            Some(d) if at > 0 => d.format("%Y-%m-%d").to_string(),
            _ => String::new(),
        }
    };
    let number = |n: u32| if n > 0 { n.to_string() } else { String::new() };
    match id {
        GROUPING => i.grouping.clone(),
        YEAR => i.year.get(..4).unwrap_or(&i.year).to_string(),
        PLAYED => number(i.play_count),
        ADDED => date(i.added),
        LAST_PLAYED => i.last_played.map(date).unwrap_or_default(),
        COMMENT => i.comment.replace('\n', " "),
        LABEL => i.label.clone(),
        REMIX => i.remix.clone(),
        COMPOSER => i.composer.clone(),
        TRACK_NUMBER => number(i.track_number),
        PREP => prep_dots(t),
        GRID => grid_state(t),
        CUES => match cue_count(t) {
            0 => "—".into(),
            n => n.to_string(),
        },
        TAGGED => if tagged(t) { "✓" } else { "—" }.into(),
        KEY_ID => crate::settings::show_key(&t.key, camelot),
        LOCATION => match &t.source {
            djsugar_engine::Source::File(p) => {
                let name = p.file_name().map(|n| n.to_string_lossy());
                let dir = p
                    .parent()
                    .and_then(|d| d.file_name())
                    .map(|n| n.to_string_lossy());
                match (dir, name) {
                    (Some(d), Some(n)) => format!("{d}/{n}"),
                    (None, Some(n)) => n.into_owned(),
                    _ => String::new(),
                }
            }
            _ => String::new(),
        },
        id if id >= TAGS => djsugar_library::query::group_at((id - TAGS) as usize)
            .map(|g| i.tags_in(g).collect::<Vec<_>>().join("  "))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The grid's state for the Grid column: locked, edited or imported (✓),
/// one the analysis is unsure of (?), sections, or plain detected.
pub fn grid_state(t: &Track) -> String {
    let i = &t.info;
    let sections = t.grid.markers().len();
    if t.bpm <= 0.0 && t.grid.is_empty() {
        "—".into()
    } else if i.grid_locked {
        "✓ locked".into()
    } else if i.grid_unsure && matches!(i.grid_from.as_str(), "" | "analysis") {
        "? check".into()
    } else if !matches!(i.grid_from.as_str(), "" | "analysis") {
        "✓ edited".into()
    } else if sections > 1 {
        format!("{sections} sections")
    } else {
        "detected".into()
    }
}

/// Hot cues (not memory cues or saved loops' places in the list).
pub fn cue_count(t: &Track) -> usize {
    t.info.hotcues.iter().filter(|c| !c.is_memory()).count()
}

/// A genre and at least one of the DJ's tags.
pub fn tagged(t: &Track) -> bool {
    !t.genre.trim().is_empty() && !t.info.tags.is_empty()
}

/// A dot for each part of basic preparation, filled when done: tags
/// (artist and title), rating, tempo and key.
pub fn prep_dots(t: &Track) -> String {
    let p = djsugar_library::stages::Prep::of(t);
    [p.tags, p.rating, p.analysed]
        .iter()
        .map(|&done| if done { '●' } else { '○' })
        .collect()
}

/// A row's cells for the columns `ids`: what the row already holds for
/// the list's own columns, the rest from its track (none for a store
/// result or a Spotify track the collection doesn't have) and the crates
/// it is in, and where it is in the list shown (see [`InList`]). The title
/// and rating are drawn from the row itself.
/// What a row's Prep columns show that the track alone doesn't say.
#[derive(Clone, Debug, Default)]
pub struct PrepFacts {
    /// Whether the file has a cover, once Collection Health has looked.
    pub cover: Option<bool>,
    pub score: Option<u8>,
    /// Checklist lines ticked, of how many.
    pub ready: Option<(usize, usize)>,
    /// In a Collection Health area: what's wrong, and the fix.
    pub problem: String,
    pub fix: String,
}

pub fn row_cells(
    ids: &[i32],
    row: &TrackRow,
    track: Option<&Track>,
    crates: Option<&[String]>,
    list: InList,
    prep: PrepFacts,
) -> ModelRc<SharedString> {
    let cells: Vec<SharedString> = ids
        .iter()
        .map(|&id| match id {
            ARTIST => row.artist.clone(),
            ALBUM => row.album.clone(),
            GENRE => row.genre.clone(),
            LENGTH => row.length.clone(),
            BPM => row.bpm.clone(),
            KEY_ID => row.key.clone(),
            TITLE_ID | RATING_ID | COLOR => SharedString::new(),
            CRATES => crates.map(|c| c.join("  ")).unwrap_or_default().into(),
            ORDER if list.order > 0 => list.order.to_string().into(),
            ORDER => SharedString::new(),
            PLAYED_AT => list.played_at.map(time_of_day).unwrap_or_default().into(),
            READY => prep
                .ready
                .map(|(done, total)| {
                    if done >= total {
                        "✓".to_string()
                    } else {
                        format!("{done}/{total}")
                    }
                })
                .unwrap_or_default()
                .into(),
            SCORE => prep.score.map(|s| s.to_string()).unwrap_or_default().into(),
            PROBLEM => prep.problem.as_str().into(),
            FIX => prep.fix.as_str().into(),
            COVER => match prep.cover {
                Some(true) => "✓".into(),
                Some(false) => "—".into(),
                None => SharedString::new(),
            },
            id => track.map(|t| cell(id, t, true).into()).unwrap_or_default(),
        })
        .collect();
    ModelRc::new(VecModel::from(cells))
}

/// Where a row is in the list shown: its place (from 1; 0 outside a list)
/// and, in a history session, when it was played (Unix seconds).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InList {
    pub order: usize,
    pub played_at: Option<i64>,
    /// The first track of a set, after the first, in a day of history.
    pub set_start: bool,
}

/// A play's time as the DJ's clock showed it, e.g. "23:41".
pub fn time_of_day(at: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(at, 0).single() {
        Some(d) if at > 0 => d.format("%H:%M").to_string(),
        _ => String::new(),
    }
}

/// Orders two tracks by a column the row data doesn't hold; numbers and
/// dates by value, the rest as text with a leading number counted as one
/// ("2 ONS" before "10 SONG"), empty last.
pub fn compare(id: i32, a: &Track, b: &Track) -> Ordering {
    let (x, y) = (&a.info, &b.info);
    match id {
        PLAYED => x.play_count.cmp(&y.play_count),
        ADDED => x.added.cmp(&y.added),
        LAST_PLAYED => x.last_played.cmp(&y.last_played),
        TRACK_NUMBER => x.track_number.cmp(&y.track_number),
        PREP => prep_count(a).cmp(&prep_count(b)),
        CUES => cue_count(a).cmp(&cue_count(b)),
        // Analysed tracks, which have a waveform, first.
        WAVE => analysed(b).cmp(&analysed(a)),
        KEY_ID => key_order(&a.key, true).cmp(&key_order(&b.key, true)),
        COLOR => color_order(x.color).cmp(&color_order(y.color)),
        _ => {
            let (s, t) = (cell(id, a, true), cell(id, b, true));
            match (s.is_empty(), t.is_empty()) {
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => leading_number(&s)
                    .cmp(&leading_number(&t))
                    .then_with(|| djsugar_library::query::collate(&s, &t)),
            }
        }
    }
}

/// Colors in the order the menu lists them (red, orange, yellow, green,
/// cyan, blue, purple, pink), others after them, none last.
pub fn color_order(color: Option<u32>) -> (usize, u32) {
    use djsugar_library::query::COLOR_NAMES;
    match color {
        Some(c) => (
            COLOR_NAMES
                .iter()
                .position(|&(_, n)| n == c)
                .unwrap_or(COLOR_NAMES.len()),
            c,
        ),
        None => (usize::MAX, 0),
    }
}

fn analysed(t: &Track) -> bool {
    djsugar_library::stages::Prep::of(t).analysed
}

fn prep_count(t: &Track) -> usize {
    let p = djsugar_library::stages::Prep::of(t);
    [p.tags, p.rating, p.analysed]
        .iter()
        .filter(|d| **d)
        .count()
}

/// Keys in the order the notation shown counts them, unknown keys last: by
/// number on a wheel (Camelot 1A, 1B, 2A …; Open Key 1m, 1d, 2m …), and up
/// the scale from C for notes (C, Cm, C#, C#m …). Tags are read whatever
/// notation they are in, so "Am", "8A" and "1m" sort together.
pub fn key_order(key: &str, camelot: bool) -> (u8, bool) {
    match Key::parse(key) {
        Some(k) if camelot => {
            let w = k.wheel();
            let n = w
                .trim_end_matches(char::is_alphabetic)
                .parse()
                .unwrap_or(99);
            // A and m (minor) before B and d.
            (n, !k.minor)
        }
        Some(k) => (k.tonic, k.minor),
        None => (99, false),
    }
}

/// The number text starts with, so "9" sorts before "10"; text without
/// one after.
fn leading_number(s: &str) -> u64 {
    let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    s[..digits].parse().unwrap_or(u64::MAX)
}

/// Which column set a list shows: the lists', a history session's, or
/// the In Prep list's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Columns {
    Lists = 0,
    History = 1,
    Prep = 2,
    Health = 3,
}

impl Columns {
    fn default_ids(self) -> &'static [i32] {
        match self {
            Columns::Lists => &DEFAULT,
            Columns::History => &HISTORY_DEFAULT,
            Columns::Prep => &PREP_DEFAULT,
            Columns::Health => &HEALTH_DEFAULT,
        }
    }
}

/// The list's columns in the browser.
pub(super) struct ColumnState {
    pub defs: Vec<Def>,
    /// The columns shown now: a history session's or In Prep's while one
    /// is shown.
    pub layout: Layout,
    /// The other layouts, by [`Columns`]; the one shown is kept in
    /// `layout` and its slot here is stale until it is swapped back.
    others: [Layout; 4],
    pub shown: Columns,
}

impl ColumnState {
    pub fn new(names: &FieldNames) -> Self {
        let defs = all(names);
        let mut layout = Layout::new(settings::columns(), settings::hidden_columns(), &defs);
        // Colors show as dots unless the DJ picked otherwise: a layout
        // saved before the Color column existed gets it once.
        if settings::track_color().is_none() {
            if layout.show_color(true, &defs) {
                settings::set_columns(&layout);
            }
            settings::set_track_color(0);
        }
        let history = Layout::new_or(
            settings::history_columns(),
            settings::hidden_history_columns(),
            &defs,
            &HISTORY_DEFAULT,
        );
        let prep = Layout::new_or(
            settings::prep_columns(),
            settings::hidden_prep_columns(),
            &defs,
            &PREP_DEFAULT,
        );
        let health = Layout::new_or(
            settings::health_columns(),
            settings::hidden_health_columns(),
            &defs,
            &HEALTH_DEFAULT,
        );
        ColumnState {
            defs,
            layout,
            others: [Layout::default(), history, prep, health],
            shown: Columns::Lists,
        }
    }

    /// Shows the columns for `kind` of list. Returns whether that changed
    /// anything.
    pub fn show(&mut self, kind: Columns) -> bool {
        if kind == self.shown {
            return false;
        }
        std::mem::swap(&mut self.layout, &mut self.others[self.shown as usize]);
        std::mem::swap(&mut self.layout, &mut self.others[kind as usize]);
        self.shown = kind;
        true
    }

    /// The layout shown back to what it starts as.
    pub fn reset(&mut self) {
        self.layout = Layout::with(&self.defs, self.shown.default_ids());
    }

    /// Keeps the layout shown in settings.
    pub fn save(&self) {
        match self.shown {
            Columns::Lists => settings::set_columns(&self.layout),
            Columns::History => settings::set_history_columns(&self.layout),
            Columns::Prep => settings::set_prep_columns(&self.layout),
            Columns::Health => settings::set_health_columns(&self.layout),
        }
    }

    fn def(&self, id: i32) -> Option<&Def> {
        self.defs.iter().find(|d| d.id == id)
    }

    /// The ids of the columns shown, in order.
    pub fn ids(&self) -> Vec<i32> {
        self.layout.shown.iter().map(|(id, _)| *id).collect()
    }

    /// How wide the list must be for the columns, with room for the title.
    fn min_width(&self) -> f32 {
        let fixed: f32 = self
            .layout
            .shown
            .iter()
            .map(|&(id, w)| shown_width(id, w) + SPACING)
            .sum();
        HEADER_LEAD + HEADER_TRAIL + fixed
    }

    /// The title's width: like the others, it keeps its own.
    fn title_width(&self) -> f32 {
        self.layout
            .shown
            .iter()
            .find(|(id, _)| *id == TITLE_ID)
            .map_or(TITLE_ROOM, |&(id, w)| shown_width(id, w))
    }
}

/// How the header lays out: padding before the columns, the album art and
/// deck badge, and the spacing between cells (as in `TrackList`).
const HEADER_LEAD: f32 = 6.0 + 26.0 + 8.0 + 22.0 + 8.0;
const HEADER_TRAIL: f32 = 14.0;
const SPACING: f32 = 8.0;
/// The title's width until it's resized.
const TITLE_ROOM: f32 = 220.0;

/// A column's width as laid out: the title's is unset (0) until resized.
fn shown_width(id: i32, width: f32) -> f32 {
    if id == TITLE_ID && width <= 0.0 {
        TITLE_ROOM
    } else {
        width
    }
}

impl Browser {
    pub(super) fn install_columns(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<ColumnUi>();
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        ui.on_toggle(move |i| {
            w(&|b| {
                let Some(id) = b.columns.defs.get(i.max(0) as usize).map(|d| d.id) else {
                    return;
                };
                let c = &mut b.columns;
                c.layout.toggle(id, &c.defs);
                b.columns_changed();
            })
        });
        let w = with.clone();
        ui.on_set_color_mode(move |mode| {
            w(&|b| {
                let mode = mode.clamp(0, 3);
                settings::set_track_color(mode as u8);
                // The dots come with the Color column, and go with it.
                let c = &mut b.columns;
                let defs = &c.defs;
                let lists = if c.shown == Columns::Lists {
                    &mut c.layout
                } else {
                    &mut c.others[Columns::Lists as usize]
                };
                if lists.show_color(mode == 0, defs) {
                    settings::set_columns(lists);
                }
                if let Some(app) = b.app.upgrade() {
                    app.global::<ColumnUi>().set_color_mode(mode);
                }
                b.columns_changed();
            })
        });
        let w = with.clone();
        ui.on_reset(move || {
            w(&|b| {
                b.columns.reset();
                b.columns_changed();
            })
        });
        let w = with.clone();
        ui.on_resize(move |i, width| {
            w(&|b| {
                // Only the widths change: the rows stay as they are, and the
                // layout is kept once the drag ends. The column is changed in
                // place, since a new list would rebuild the heading being
                // dragged and drop the drag.
                let i = i.max(0) as usize;
                b.columns.layout.resize(i, width);
                let Some(app) = b.app.upgrade() else {
                    return;
                };
                let ui = app.global::<ColumnUi>();
                let model = ui.get_columns();
                if let (Some(mut col), Some(&(_, width))) =
                    (model.row_data(i), b.columns.layout.shown.get(i))
                {
                    col.width = width;
                    model.set_row_data(i, col);
                }
                ui.set_min_width(b.columns.min_width());
            })
        });
        let w = with.clone();
        ui.on_resized(move || w(&|b| b.columns.save()));
        let w = with.clone();
        ui.on_drag(move |i, x, _header| {
            w(&|b| {
                // Mark where the heading would land, unless it stays put.
                let c = &b.columns;
                let (to, left) = c.layout.slot(x - HEADER_LEAD, c.title_width(), SPACING);
                let from = i.max(0) as usize;
                let at = if to == from || to == from + 1 {
                    -1.0
                } else {
                    HEADER_LEAD + left - SPACING / 2.0
                };
                if let Some(app) = b.app.upgrade() {
                    app.global::<ColumnUi>().set_drop_x(at);
                }
            })
        });
        let w = with.clone();
        ui.on_move(move |i, x, _header| {
            w(&|b| {
                let title = b.columns.title_width();
                b.columns
                    .layout
                    .move_to(i.max(0) as usize, x - HEADER_LEAD, title, SPACING);
                b.columns_changed();
            })
        });
        ui.set_color_mode(settings::track_color().unwrap_or(0) as i32);
        b.borrow().refresh_column_ui();
    }

    /// The columns shown changed: keep them and list again.
    fn columns_changed(&mut self) {
        self.columns.save();
        self.refresh_column_ui();
        self.refresh_rows();
    }

    /// Tag fields or grouping were renamed: the columns follow.
    pub(super) fn column_names_changed(&mut self, names: &FieldNames) {
        let c = &mut self.columns;
        c.defs = all(names);
        c.layout.fit(&c.defs);
        for l in &mut c.others {
            l.fit(&c.defs);
        }
        self.refresh_column_ui();
    }

    pub(super) fn refresh_column_ui(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<ColumnUi>();
        let c = &self.columns;
        let shown: Vec<ListColumn> = c
            .layout
            .shown
            .iter()
            .filter_map(|&(id, width)| {
                let d = c.def(id)?;
                Some(ListColumn {
                    id,
                    label: d.label.clone().into(),
                    width: shown_width(id, width),
                    right: d.right,
                    kind: d.kind,
                })
            })
            .collect();
        ui.set_min_width(c.min_width());
        ui.set_columns(ModelRc::new(VecModel::from(shown)));
        let labels: Vec<SharedString> = c.defs.iter().map(|d| d.label.clone().into()).collect();
        ui.set_all(ModelRc::new(VecModel::from(labels)));
        let on: Vec<bool> = c.defs.iter().map(|d| c.layout.has(d.id)).collect();
        ui.set_all_shown(ModelRc::new(VecModel::from(on)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(l: &Layout) -> Vec<i32> {
        l.shown.iter().map(|(id, _)| *id).collect()
    }

    #[test]
    fn every_column_has_its_own_id() {
        let defs = all(&FieldNames::default());
        let mut ids: Vec<i32> = defs.iter().map(|d| d.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), defs.len());
        for id in PREP_DEFAULT {
            assert!(ids.contains(&id), "In Prep column {id}");
        }
    }

    #[test]
    fn picks_resizes_and_moves_columns() {
        let defs = all(&FieldNames::default());
        let mut l = Layout::new(Vec::new(), Vec::new(), &defs);
        assert_eq!(ids(&l), DEFAULT);
        l.toggle(COLOR, &defs);
        l.toggle(GROUPING, &defs);
        l.toggle(GENRE, &defs);
        l.toggle(WAVE, &defs);
        l.toggle(TITLE_ID, &defs);
        assert_eq!(
            ids(&l),
            [RATING_ID, TITLE_ID, ARTIST, LENGTH, BPM, KEY_ID, GROUPING]
        );
        l.resize(2, 10.0);
        assert_eq!(l.shown[2].1, MIN_WIDTH);
        l.resize(2, 200.4);
        assert_eq!(l.shown[2].1, 200.0);

        // Rating 64, title 300, artist 200, length 60 ... with 8 between.
        // Dropping rating in the middle of the artist column puts it there.
        l.move_to(0, 64.0 + 8.0 + 300.0 + 8.0 + 150.0, 300.0, 8.0);
        assert_eq!(ids(&l)[..3], [TITLE_ID, ARTIST, RATING_ID]);
        // Dropped before everything: first.
        l.move_to(2, 0.0, 300.0, 8.0);
        assert_eq!(ids(&l)[..3], [RATING_ID, TITLE_ID, ARTIST]);
        // Dropped past the end: last.
        l.move_to(0, 5000.0, 300.0, 8.0);
        assert_eq!(ids(&l).last(), Some(&RATING_ID));
    }

    #[test]
    fn hidden_columns_come_back_where_they_were() {
        let defs = all(&FieldNames::default());
        let mut l = Layout::new(
            vec![
                (TITLE_ID, 0.0),
                (TRACK_NUMBER, 44.0),
                (ARTIST, 170.0),
                (BPM, 56.0),
            ],
            Vec::new(),
            &defs,
        );
        l.toggle(TRACK_NUMBER, &defs);
        assert_eq!(ids(&l), [TITLE_ID, ARTIST, BPM]);
        l.toggle(TRACK_NUMBER, &defs);
        assert_eq!(ids(&l), [TITLE_ID, TRACK_NUMBER, ARTIST, BPM]);
        assert_eq!(l.shown[1].1, 44.0, "keeps its width");
        assert!(l.hidden.is_empty());

        // Its left neighbour hidden too: it goes right of where that was.
        l.toggle(TRACK_NUMBER, &defs);
        l.toggle(TITLE_ID, &defs);
        l.toggle(ARTIST, &defs);
        l.toggle(BPM, &defs);
        l.toggle(TRACK_NUMBER, &defs);
        assert_eq!(ids(&l), [TITLE_ID, TRACK_NUMBER]);

        // First when hidden: first again.
        let mut l = Layout::new(
            vec![(BPM, 56.0), (TITLE_ID, 0.0), (ARTIST, 170.0)],
            Vec::new(),
            &defs,
        );
        l.toggle(BPM, &defs);
        l.toggle(BPM, &defs);
        assert_eq!(ids(&l), [BPM, TITLE_ID, ARTIST]);

        // Its neighbour moved away: it stays in place.
        let mut l = Layout::new(
            vec![
                (TITLE_ID, 300.0),
                (ARTIST, 200.0),
                (YEAR, 50.0),
                (BPM, 56.0),
            ],
            Vec::new(),
            &defs,
        );
        l.toggle(YEAR, &defs);
        l.move_to(1, 5000.0, 300.0, 8.0);
        assert_eq!(ids(&l), [TITLE_ID, BPM, ARTIST]);
        l.toggle(YEAR, &defs);
        assert_eq!(ids(&l), [TITLE_ID, YEAR, BPM, ARTIST]);

        // Never shown before: at the end.
        l.toggle(GENRE, &defs);
        assert_eq!(ids(&l).last(), Some(&GENRE));
    }

    #[test]
    fn keeps_the_title_and_drops_gone_columns() {
        let defs = all(&FieldNames::default());
        let l = Layout::new(
            vec![(ARTIST, 100.0), (TAGS + 5, 80.0), (ARTIST, 90.0)],
            Vec::new(),
            &defs,
        );
        assert_eq!(l.shown, [(TITLE_ID, 0.0), (ARTIST, 100.0)]);
    }

    #[test]
    fn shows_and_sorts_library_fields() {
        let mut a = std::sync::Arc::unwrap_or_clone(djsugar_engine::sample_library().remove(0));
        let mut b = a.clone();
        a.info.grouping = "10 SONG".into();
        b.info.grouping = "2 ONS".into();
        a.info
            .set_tags_in(1, ["uplifting".into(), "Bright Pads".into()]);
        a.info.play_count = 3;
        assert_eq!(cell(TAGS, &a, true), "uplifting  Bright Pads");
        assert_eq!(cell(PLAYED, &a, true), "3");
        assert_eq!(cell(PLAYED, &b, true), "");
        assert_eq!(compare(GROUPING, &b, &a), Ordering::Less);
        assert_eq!(compare(PLAYED, &b, &a), Ordering::Less);
        b.info.grouping.clear();
        assert_eq!(compare(GROUPING, &a, &b), Ordering::Less, "empty last");
    }

    #[test]
    fn shows_and_sorts_colours() {
        let defs = all(&FieldNames::default());
        let mut l = Layout::new(vec![(TITLE_ID, 0.0)], Vec::new(), &defs);
        assert!(l.show_color(true, &defs));
        assert!(!l.show_color(true, &defs));
        assert_eq!(ids(&l), [COLOR, TITLE_ID]);
        assert!(l.show_color(false, &defs));
        assert_eq!(ids(&l), [TITLE_ID]);

        let mut a = std::sync::Arc::unwrap_or_clone(djsugar_engine::sample_library().remove(0));
        let mut b = a.clone();
        a.info.color = Some(0x007aff);
        b.info.color = Some(0xff3b30);
        assert_eq!(compare(COLOR, &b, &a), Ordering::Less, "red before blue");
        b.info.color = None;
        assert_eq!(compare(COLOR, &a, &b), Ordering::Less, "none last");
    }

    #[test]
    fn sorts_keys_by_number_or_scale() {
        let sorted = |camelot| {
            let mut keys = vec!["10A", "Am", "2B", "1m", "C#", "Odd", "Bbm", "1A", "C"];
            keys.sort_by_key(|k| key_order(k, camelot));
            keys
        };
        // Am, 8A and Open Key's 1m are the same key.
        assert_eq!(
            sorted(true),
            ["1A", "2B", "Bbm", "C#", "Am", "1m", "C", "10A", "Odd"]
        );
        assert_eq!(
            sorted(false),
            ["C", "C#", "2B", "1A", "Am", "1m", "Bbm", "10A", "Odd"]
        );
    }
}
