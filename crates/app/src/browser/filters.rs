//! The filter bar above the track list: a chip per field, as djay Pro
//! has, that narrows the list along with the search. Each chip turns into
//! search text (see `djsugar_library::query`), so chips, typed searches and
//! smart lists all agree.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use djsugar_engine::key::Key;
use djsugar_engine::Track;
use djsugar_library::query::{self, and_text, keyword, quoted, FieldNames, Query, COLOR_NAMES};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{settings, AppWindow, FilterChip, FilterChoice, FilterSuggestion, FilterUi};

/// How close a tempo must be to the playing track's, when BPM follows it.
pub const TOLERANCES: [f64; 5] = [0.02, 0.04, 0.06, 0.08, 0.10];

/// What a field's chip edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Pick values of a text field (genre, artist, ...).
    Text,
    /// Pick tags of one of the DJ's tag groups (by its id); all of them
    /// or any.
    Tags(i64),
    /// A tempo range, or near the playing track's.
    Bpm,
    /// Keys, or those that mix with the playing track's.
    Key,
    /// Pick crates; a track in any of them is in.
    Crates,
    Rating,
    Color,
    Year,
    Played,
    Added,
}

/// A field the bar can filter on.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Names the field in settings and searches: "genre", "bpm", or
    /// "tag1" for the first tag group.
    pub id: String,
    pub label: String,
    pub kind: Kind,
}

/// The fields offered, in the order "+" lists them; grouping and the tag
/// groups go by the names the DJ gave them.
pub fn fields(names: &FieldNames) -> Vec<Field> {
    let f = |id: &str, label: &str, kind| Field {
        id: id.into(),
        label: label.into(),
        kind,
    };
    let mut out = vec![
        f("grouping", &names.grouping, Kind::Text),
        f("genre", "Genre", Kind::Text),
    ];
    for (i, group) in names.tags.iter().enumerate() {
        out.push(f(
            &format!("tag{}", i + 1),
            &group.name,
            Kind::Tags(group.id),
        ));
    }
    out.extend([
        f("crate", "Crates", Kind::Crates),
        f("bpm", "BPM", Kind::Bpm),
        f("key", "Key", Kind::Key),
        f("rating", "Rating", Kind::Rating),
        f("color", "Color", Kind::Color),
        f("artist", "Artist", Kind::Text),
        f("album", "Album", Kind::Text),
        f("year", "Year", Kind::Year),
        f("played", "Play Count", Kind::Played),
        f("added", "Date Added", Kind::Added),
    ]);
    out
}

/// The Key chip's ways to match the playing track, as the popup names
/// them: the app's setting first.
pub const KEY_MATCHES: [&str; 4] = ["Setting", "Exact", "Harmonic", "Fuzzy"];

/// How many values with tracks a long list shows before "Show N more".
pub const SHORT_LIST: usize = 8;

/// How many picks are remembered to order values with as many tracks.
const RECENT: usize = 64;

/// The chips on the bar when the DJ hasn't chosen.
/// Genre, the DJ's tag groups (as many as there are), Crates, BPM and
/// Key; Grouping and the rest wait under "+" › More.
pub const DEFAULT_PINNED: [&str; 11] = [
    "genre", "tag1", "tag2", "tag3", "tag4", "tag5", "tag6", "tag7", "crate", "bpm", "key",
];

/// Puts the Crates chip on a bar saved before it existed, before BPM (or
/// at the end), once: taken off later, it stays off.
pub fn with_crates(mut pinned: Vec<String>) -> Vec<String> {
    if !pinned.iter().any(|p| p == "crate") {
        let at = pinned
            .iter()
            .position(|p| p == "bpm")
            .unwrap_or(pinned.len());
        pinned.insert(at, "crate".into());
    }
    pinned
}

/// Where "+" lists a field: 0 core, 1 the DJ's tag groups, 2 more.
pub fn section(field: &Field) -> i32 {
    match (field.kind, field.id.as_str()) {
        (Kind::Tags(_), _) => 1,
        (_, "genre" | "crate" | "bpm" | "key" | "rating" | "color") => 0,
        _ => 2,
    }
}

/// One value a chip offers: what it says, the search it stands for, how
/// many tracks of the list have it, a color swatch for colors, and a
/// crate's look.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub label: String,
    pub term: String,
    pub count: usize,
    pub swatch: Option<u32>,
    pub look: Option<CrateLook>,
}

/// How a crate looks in the sidebar: its icon (an index into
/// `LIST_ICONS`, 0 the crate's own) and the color it's drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CrateLook {
    pub icon: usize,
    pub color: Option<u32>,
}

/// What the bar is set to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filters {
    /// Fields always on the bar, in order.
    pub pinned: Vec<String>,
    /// Fields added with "+" for now, after the pinned ones.
    pub added: Vec<String>,
    /// The values picked per field, as (search, label).
    pub picked: HashMap<String, Vec<(String, String)>>,
    /// Tag fields whose picked words must all be on a track, not any.
    pub match_all: HashSet<String>,
    /// A typed tempo range; either end may be empty.
    pub bpm_low: String,
    pub bpm_high: String,
    /// BPM follows the playing track, within `TOLERANCES[tolerance]`.
    pub bpm_follow: bool,
    pub tolerance: usize,
    /// Only keys that mix with the playing track's.
    pub key_follow: bool,
    /// How keys must match the playing track's: 0 by the app's key
    /// mixing setting, 1 the same key, 2 Harmonic, 3 Fuzzy.
    pub key_match: usize,
}

impl Filters {
    pub fn new(pinned: Vec<String>, tolerance: usize) -> Self {
        Filters {
            pinned,
            tolerance: tolerance.min(TOLERANCES.len() - 1),
            ..Default::default()
        }
    }

    /// Whether a field narrows the list now.
    pub fn active(&self, field: &Field) -> bool {
        match field.kind {
            Kind::Bpm => {
                self.bpm_follow
                    || !self.bpm_low.trim().is_empty()
                    || !self.bpm_high.trim().is_empty()
            }
            Kind::Key if self.key_follow => true,
            _ => self.picked.get(&field.id).is_some_and(|p| !p.is_empty()),
        }
    }

    pub fn any_active(&self, fields: &[Field]) -> bool {
        fields.iter().any(|f| self.active(f))
    }

    /// The fields with a chip on the bar: pinned ones, then any other
    /// that is added or narrowing the list.
    pub fn shown<'a>(&self, fields: &'a [Field]) -> Vec<&'a Field> {
        let by_id = |id: &String| fields.iter().find(|f| f.id == *id);
        let mut out: Vec<&Field> = self.pinned.iter().filter_map(by_id).collect();
        for f in self.added.iter().filter_map(by_id).chain(fields) {
            if (self.added.contains(&f.id) || self.active(f)) && !out.contains(&f) {
                out.push(f);
            }
        }
        out
    }

    /// Takes a field off the bar, clearing what it was set to.
    pub fn remove(&mut self, field: &Field) {
        self.clear(field);
        self.pinned.retain(|p| *p != field.id);
        self.added.retain(|p| *p != field.id);
    }

    /// Moves the chip at `from` among the chips shown to just before the
    /// one at `to` (the end when `to` is past the last). Every chip shown
    /// is kept on the bar in the new order.
    pub fn move_chip(&mut self, fields: &[Field], from: usize, to: usize) {
        let mut ids: Vec<String> = self.shown(fields).iter().map(|f| f.id.clone()).collect();
        if from >= ids.len() {
            return;
        }
        let id = ids.remove(from);
        let to = if to > from { to - 1 } else { to }.min(ids.len());
        ids.insert(to, id);
        self.added.retain(|a| !ids.contains(a));
        self.pinned = ids;
    }

    /// Where a chip dragged to `x` along the bar lands, given each chip's
    /// left edge and width: the place before the first chip whose middle
    /// is past `x`. None when it would land where it is.
    pub fn drop_place(from: usize, x: f32, placed: &[(f32, f32)]) -> Option<usize> {
        let to = placed
            .iter()
            .position(|(left, width)| x < left + width / 2.0)
            .unwrap_or(placed.len());
        (to != from && to != from + 1).then_some(to)
    }

    /// Clears a field's chip.
    pub fn clear(&mut self, field: &Field) {
        self.picked.remove(&field.id);
        match field.kind {
            Kind::Bpm => {
                self.bpm_follow = false;
                self.bpm_low.clear();
                self.bpm_high.clear();
            }
            Kind::Key => self.key_follow = false,
            _ => {}
        }
    }

    /// A click on the BPM or Key chip itself: off when it filters,
    /// otherwise match the playing track (tempo within the tolerance,
    /// keys that mix). Other fields aren't changed.
    pub fn quick(&mut self, field: &Field) {
        if !matches!(field.kind, Kind::Bpm | Kind::Key) {
            return;
        }
        if self.active(field) {
            self.clear(field);
            return;
        }
        match field.kind {
            Kind::Bpm => self.bpm_follow = true,
            _ => self.key_follow = true,
        }
    }

    pub fn clear_all(&mut self, fields: &[Field]) {
        for f in fields {
            self.clear(f);
        }
        self.added.clear();
    }

    /// Picks a value of a field, or unpicks it.
    pub fn toggle(&mut self, field: &str, choice: &Choice) {
        let picked = self.picked.entry(field.to_string()).or_default();
        match picked.iter().position(|(t, _)| *t == choice.term) {
            Some(i) => {
                picked.remove(i);
            }
            None => picked.push((choice.term.clone(), choice.label.clone())),
        }
    }

    pub fn is_picked(&self, field: &str, term: &str) -> bool {
        self.picked
            .get(field)
            .is_some_and(|p| p.iter().any(|(t, _)| t == term))
    }

    /// What the chip shows: its value, or "--" when it doesn't filter.
    pub fn summary(
        &self,
        field: &Field,
        deck: Option<(f64, Option<Key>)>,
        camelot: bool,
    ) -> String {
        match field.kind {
            Kind::Bpm if self.bpm_follow => {
                let pct = (TOLERANCES[self.tolerance] * 100.0).round();
                match deck.filter(|(bpm, _)| *bpm > 0.0) {
                    Some((bpm, _)) => format!("{bpm:.0} ±{pct}%"),
                    None => format!("±{pct}%"),
                }
            }
            Kind::Bpm => match (self.bpm_low.trim(), self.bpm_high.trim()) {
                ("", "") => "--".into(),
                (low, "") => format!("{low}+"),
                ("", high) => format!("≤ {high}"),
                (low, high) => format!("{low} – {high}"),
            },
            Kind::Key if self.key_follow => match deck.and_then(|(_, k)| k) {
                Some(k) => format!("Match {}", k.label(camelot)),
                None => "Match".into(),
            },
            _ => {
                let picked = self.picked.get(&field.id).map_or(&[][..], Vec::as_slice);
                match picked {
                    [] => "--".into(),
                    [(_, one)] => one.clone(),
                    [(_, first), rest @ ..] => format!("{first} +{}", rest.len()),
                }
            }
        }
    }

    /// The search the bar stands for, given the playing track's tempo and
    /// key; empty when nothing is filtered.
    pub fn text(&self, fields: &[Field], deck: Option<(f64, Option<Key>)>) -> String {
        self.text_except(fields, deck, "")
    }

    /// The bar's search without one field's chip: what that chip's values
    /// are counted over, so the other chips narrow it but its own picks
    /// don't.
    pub fn text_except(
        &self,
        fields: &[Field],
        deck: Option<(f64, Option<Key>)>,
        skip: &str,
    ) -> String {
        let mut text = String::new();
        for f in fields.iter().filter(|f| f.id != skip) {
            text = and_text(&text, &self.field_text(f, deck));
        }
        text
    }

    fn field_text(&self, field: &Field, deck: Option<(f64, Option<Key>)>) -> String {
        let picked = self.picked.get(&field.id).map_or(&[][..], Vec::as_slice);
        let terms = || picked.iter().map(|(t, _)| t.as_str());
        let any = || terms().collect::<Vec<_>>().join(" | ");
        match field.kind {
            Kind::Bpm if self.bpm_follow => match deck.filter(|(bpm, _)| *bpm > 0.0) {
                Some((bpm, _)) => {
                    let t = TOLERANCES[self.tolerance];
                    format!("bpm:{:.2}-{:.2}", bpm * (1.0 - t), bpm * (1.0 + t))
                }
                // Nothing playing: nothing to follow, so nothing is left out.
                None => String::new(),
            },
            Kind::Bpm => {
                let num = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok();
                match (num(&self.bpm_low), num(&self.bpm_high)) {
                    (Some(a), Some(b)) => format!("bpm:{a}-{b}"),
                    (Some(a), None) => format!("bpm:>={a}"),
                    (None, Some(b)) => format!("bpm:<={b}"),
                    (None, None) => String::new(),
                }
            }
            Kind::Key => {
                let follow = match deck.and_then(|(_, k)| k) {
                    Some(k) if self.key_follow => {
                        let rule = ["~key", "key", "~harmonic", "~fuzzy"][self.key_match.min(3)];
                        format!("{rule}:{}", k.camelot())
                    }
                    _ => String::new(),
                };
                and_text(&follow, &any())
            }
            Kind::Tags(_) if self.match_all.contains(&field.id) => {
                terms().collect::<Vec<_>>().join(" ")
            }
            _ => any(),
        }
    }

    /// Whether the bar's search depends on the playing track.
    pub fn follows_deck(&self) -> bool {
        self.bpm_follow || self.key_follow
    }
}

/// The bar's state in the browser: the fields, the chips and the popup open.
pub(super) struct FilterState {
    pub names: FieldNames,
    pub fields: Vec<Field>,
    pub filters: Filters,
    /// The field whose popup is open, every value it offers, and the text
    /// typed to find one.
    editing: Option<String>,
    choices: Vec<Choice>,
    find: String,
    /// The popup lists every value, not just the first few with tracks.
    show_all: bool,
    /// What the finder offers for the text typed: (field, value).
    suggestions: Vec<(String, Choice)>,
    /// Each chip's left edge and width along the bar, as last laid out.
    placed: Vec<(f32, f32)>,
    /// Values picked lately, newest first, as (field, search): they come
    /// first among values with as many tracks.
    recent: Vec<(String, String)>,
    /// Each crate's icon and color, by its name in lowercase.
    pub crate_looks: HashMap<String, CrateLook>,
}

impl FilterState {
    pub fn new() -> Self {
        let names = settings::field_names();
        query::set_field_names(names.clone());
        FilterState {
            fields: fields(&names),
            names,
            filters: Filters {
                key_match: settings::filter_key_match(),
                ..Filters::new(settings::filter_pinned(), settings::filter_tolerance())
            },
            editing: None,
            choices: Vec::new(),
            find: String::new(),
            show_all: false,
            placed: Vec::new(),
            suggestions: Vec::new(),
            recent: Vec::new(),
            crate_looks: HashMap::new(),
        }
    }

    pub fn clear_all(&mut self) {
        let fields = self.fields.clone();
        self.filters.clear_all(&fields);
    }

    /// The bar as a smart list's search: following the playing track is
    /// saved as `~deck` (key and tempo), not as today's tempo.
    pub fn saved_text(&self) -> String {
        let fixed = self.filters.text(&self.fields, None);
        if self.filters.follows_deck() {
            and_text(&fixed, "~deck")
        } else {
            fixed
        }
    }

    fn field(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.id == id)
    }

    /// The values the popup lists, narrowed by what was typed to find one
    /// (words that start with it first), and how many more "Show N more"
    /// reveals.
    fn shown_choices(&self) -> (Vec<&Choice>, usize) {
        let find = self.find.trim().to_lowercase();
        if !find.is_empty() {
            let mut found: Vec<&Choice> = self
                .choices
                .iter()
                .filter(|c| c.label.to_lowercase().contains(&find))
                .collect();
            found.sort_by_key(|c| !c.label.to_lowercase().starts_with(&find));
            return (found, 0);
        }
        // Names and tags show the first few with tracks; keys and years
        // all of theirs; the rest of each behind "Show N more". Short
        // fixed lists (ratings, colors) show everything.
        let cap = match self
            .editing
            .as_deref()
            .and_then(|id| self.field(id))
            .map(|f| f.kind)
        {
            Some(Kind::Text | Kind::Tags(_) | Kind::Crates) => SHORT_LIST,
            Some(Kind::Key | Kind::Year) => usize::MAX,
            _ => return (self.choices.iter().collect(), 0),
        };
        if self.show_all {
            return (self.choices.iter().collect(), 0);
        }
        let id = self.editing.as_deref().unwrap_or_default();
        let mut shown = 0;
        let list: Vec<&Choice> = self
            .choices
            .iter()
            .filter(|c| {
                let keep = self.filters.is_picked(id, &c.term) || (c.count > 0 && shown < cap);
                if keep && c.count > 0 {
                    shown += 1;
                }
                keep
            })
            .collect();
        let more = self.choices.len() - list.len();
        (list, more)
    }

    /// Notes a value as picked just now.
    fn picked_now(&mut self, field: &str, term: &str) {
        self.recent.retain(|(f, t)| !(f == field && t == term));
        self.recent.insert(0, (field.to_string(), term.to_string()));
        self.recent.truncate(RECENT);
    }
}

impl Browser {
    pub(super) fn install_filters(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<FilterUi>();
        // The bar starts hidden; the filter button lights up while chips
        // narrow the list.
        ui.set_shown(false);
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        {
            let app = app.as_weak();
            ui.on_set_shown(move |on| {
                if let Some(app) = app.upgrade() {
                    app.global::<FilterUi>().set_shown(on);
                }
            });
        }
        let w = with.clone();
        ui.on_add(move |id| {
            let chip = std::cell::Cell::new(-1);
            w(&|b| chip.set(b.add_filter(&id)));
            chip.get()
        });
        let w = with.clone();
        ui.on_edit(move |chip| w(&|b| b.edit_filter(chip)));
        let w = with.clone();
        ui.on_quick(move |chip| {
            w(&|b| {
                let Some(id) = b.chip_field(chip) else { return };
                let Some(field) = b.filter.field(&id).cloned() else {
                    return;
                };
                b.filter.filters.quick(&field);
                b.update_reference();
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_toggle(move |i| {
            w(&|b| {
                let f = &mut b.filter;
                let (Some(id), Some(c)) = (
                    f.editing.clone(),
                    f.shown_choices()
                        .0
                        .get(i.max(0) as usize)
                        .map(|c| (*c).clone()),
                ) else {
                    return;
                };
                f.filters.toggle(&id, &c);
                if f.filters.is_picked(&id, &c.term) {
                    f.picked_now(&id, &c.term);
                }
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_find(move |text| w(&|b| b.find_values(&text)));
        let w = with.clone();
        ui.on_pick_suggestion(move |i| w(&|b| b.pick_suggestion(i.max(0) as usize)));
        let w = with.clone();
        ui.on_show_more(move || {
            w(&|b| {
                b.filter.show_all = true;
                b.refresh_filter_ui();
            })
        });
        let w = with.clone();
        ui.on_search_choices(move |text| {
            w(&|b| {
                b.filter.find = text.to_string();
                b.refresh_filter_ui();
            })
        });
        let w = with.clone();
        ui.on_set_match_all(move |on| {
            w(&|b| {
                let Some(id) = b.filter.editing.clone() else {
                    return;
                };
                let all = &mut b.filter.filters.match_all;
                if on {
                    all.insert(id);
                } else {
                    all.remove(&id);
                }
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_follow(move |on| {
            w(&|b| {
                let f = &mut b.filter;
                match f
                    .editing
                    .as_deref()
                    .and_then(|id| f.field(id))
                    .map(|f| f.kind)
                {
                    Some(Kind::Bpm) => f.filters.bpm_follow = on,
                    Some(Kind::Key) => f.filters.key_follow = on,
                    _ => return,
                }
                b.update_reference();
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_bpm(move |low, high| {
            w(&|b| {
                b.filter.filters.bpm_low = low.to_string();
                b.filter.filters.bpm_high = high.to_string();
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_key_match(move |i| {
            w(&|b| {
                let i = (i.max(0) as usize).min(KEY_MATCHES.len() - 1);
                b.filter.filters.key_match = i;
                settings::set_filter_key_match(i);
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_set_tolerance(move |i| {
            w(&|b| {
                let i = (i.max(0) as usize).min(TOLERANCES.len() - 1);
                b.filter.filters.tolerance = i;
                settings::set_filter_tolerance(i);
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_remove(move |chip| {
            w(&|b| {
                let Some(id) = b.chip_field(chip) else { return };
                let Some(field) = b.filter.field(&id).cloned() else {
                    return;
                };
                b.filter.filters.remove(&field);
                b.filter.editing = None;
                settings::set_filter_pinned(&b.filter.filters.pinned);
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_placed(move |chip, x, width| {
            w(&|b| {
                let (Ok(i), placed) = (usize::try_from(chip), &mut b.filter.placed) else {
                    return;
                };
                if placed.len() <= i {
                    placed.resize(i + 1, (0.0, 0.0));
                }
                placed[i] = (x, width);
            })
        });
        let weak = app.as_weak();
        let w = with.clone();
        ui.on_drag(move |chip, x| {
            let line = std::cell::Cell::new(-1.0);
            w(&|b| {
                let n = b.filter.filters.shown(&b.filter.fields).len();
                let placed = &b.filter.placed[..b.filter.placed.len().min(n)];
                if let Some(to) = Filters::drop_place(chip.max(0) as usize, x, placed) {
                    line.set(match placed.get(to) {
                        Some((left, _)) => left - 4.0,
                        None => placed.last().map_or(0.0, |(l, w)| l + w + 4.0),
                    });
                }
            });
            if let Some(app) = weak.upgrade() {
                app.global::<FilterUi>().set_drop_x(line.get());
            }
        });
        let w = with.clone();
        ui.on_drop(move |chip, x| {
            w(&|b| {
                let n = b.filter.filters.shown(&b.filter.fields).len();
                let placed = &b.filter.placed[..b.filter.placed.len().min(n)];
                let from = chip.max(0) as usize;
                if let Some(to) = Filters::drop_place(from, x, placed) {
                    let fields = b.filter.fields.clone();
                    b.filter.filters.move_chip(&fields, from, to);
                    b.filter.editing = None;
                    settings::set_filter_pinned(&b.filter.filters.pinned);
                    b.refresh_filter_ui();
                }
            })
        });
        let w = with.clone();
        ui.on_clear(move |chip| {
            w(&|b| {
                let Some(id) = b.chip_field(chip) else { return };
                if let Some(field) = b.filter.field(&id).cloned() {
                    b.filter.filters.clear(&field);
                }
                b.filters_changed();
            })
        });
        let w = with.clone();
        ui.on_clear_all(move || {
            w(&|b| {
                b.filter.clear_all();
                b.search.clear();
                if let Some(app) = b.app.upgrade() {
                    app.global::<crate::StoreUi>()
                        .set_search_text(SharedString::new());
                }
                b.filters_changed();
            })
        });
        b.borrow_mut().refresh_filter_ui();
    }

    /// The field of the chip at `chip` on the bar.
    fn chip_field(&self, chip: i32) -> Option<String> {
        let f = &self.filter;
        f.filters
            .shown(&f.fields)
            .get(usize::try_from(chip).ok()?)
            .map(|field| field.id.clone())
    }

    /// Puts a field on the bar for now; returns its chip.
    fn add_filter(&mut self, id: &str) -> i32 {
        let f = &mut self.filter;
        // A field added stays on the bar until it is taken off.
        f.filters.added.retain(|p| p != id);
        if !f.filters.pinned.iter().any(|p| p == id) {
            f.filters.pinned.push(id.to_string());
            settings::set_filter_pinned(&f.filters.pinned);
        }
        self.refresh_filter_ui();
        let f = &self.filter;
        f.filters
            .shown(&f.fields)
            .iter()
            .position(|field| field.id == id)
            .map_or(-1, |i| i as i32)
    }

    /// Opens a chip's popup: lists the values of its field in the list.
    fn edit_filter(&mut self, chip: i32) {
        self.filter.editing = self.chip_field(chip);
        self.filter.find.clear();
        self.filter.show_all = false;
        self.update_reference();
        self.refresh_choices();
        self.refresh_filter_ui();
    }

    /// The values the open popup offers: every value the list has, each
    /// counted over the tracks the search and the other chips leave (its
    /// own picks don't narrow it, so picking one doesn't hide the rest).
    fn refresh_choices(&mut self) {
        let Some(field) = self
            .filter
            .editing
            .as_deref()
            .and_then(|id| self.filter.field(id))
        else {
            self.filter.choices.clear();
            return;
        };
        let field = field.clone();
        let tracks: Vec<usize> = self.row_data().iter().filter_map(|r| r.local).collect();
        let f = &self.filter;
        let others = and_text(
            &self.search,
            &f.filters.text_except(&f.fields, self.deck_ref, &field.id),
        );
        let q = Query::parse(&others);
        let cx = self.context();
        let left: Vec<usize> = tracks
            .iter()
            .copied()
            .filter(|&id| q.is_empty() || q.matches(&self.library[id], &self.indexed[id], &cx))
            .collect();
        let recent: Vec<&str> = f
            .recent
            .iter()
            .filter(|(id, _)| *id == field.id)
            .map(|(_, t)| t.as_str())
            .collect();
        let mut choices = linked_choices(
            &field,
            tracks.iter().map(|&i| &*self.library[i]),
            left.iter().map(|&i| &*self.library[i]),
            &f.filters,
            &self.crates,
            &recent,
            self.camelot,
        );
        if field.kind == Kind::Crates {
            for c in &mut choices {
                c.look = (c.label != "(none)").then(|| {
                    f.crate_looks
                        .get(&c.label.to_lowercase())
                        .copied()
                        .unwrap_or_default()
                });
            }
        }
        self.filter.choices = choices;
    }

    /// Values of any field that match `text`, for the finder: counted over
    /// the tracks the bar leaves (the typed search aside, since that's what
    /// is being typed), best first.
    pub(super) fn find_values(&mut self, text: &str) {
        let tracks: Vec<usize> = self.row_data().iter().filter_map(|r| r.local).collect();
        let f = &self.filter;
        let q = Query::parse(&f.filters.text(&f.fields, self.deck_ref));
        let cx = self.context();
        let left: Vec<&djsugar_engine::Track> = tracks
            .iter()
            .filter(|&&id| q.is_empty() || q.matches(&self.library[id], &self.indexed[id], &cx))
            .map(|&id| &*self.library[id])
            .collect();
        self.filter.suggestions = suggestions(
            &f.fields,
            &left,
            &f.filters,
            &self.crates,
            text,
            self.camelot,
        );
        self.refresh_filter_ui();
    }

    /// Adds the finder's suggestion `i` to the bar.
    pub(super) fn pick_suggestion(&mut self, i: usize) {
        let Some((id, choice)) = self.filter.suggestions.get(i).cloned() else {
            return;
        };
        let f = &mut self.filter;
        if !f.filters.is_picked(&id, &choice.term) {
            f.filters.toggle(&id, &choice);
            f.picked_now(&id, &choice.term);
        }
        f.suggestions.clear();
        self.filters_changed();
    }

    /// The search and the bar together, as one search.
    pub(super) fn full_search(&self) -> String {
        let f = &self.filter;
        and_text(&self.search, &f.filters.text(&f.fields, self.deck_ref))
    }

    pub(super) fn filtering(&self) -> bool {
        self.filter.filters.any_active(&self.filter.fields)
    }

    /// A chip changed: list again and show it.
    fn filters_changed(&mut self) {
        self.refresh_rows();
        self.refresh_filter_ui();
    }

    /// Grouping or the tag groups changed in Settings (or an import).
    pub(super) fn field_names_changed(&mut self, names: FieldNames) {
        // Groups came or went (Settings, an import): tracks take their tags
        // as the library has them now.
        let groups: HashSet<i64> = names.tags.iter().map(|g| g.id).collect();
        let before: HashSet<i64> = self.filter.names.tags.iter().map(|g| g.id).collect();
        if groups != before || names.tags_revision != self.filter.names.tags_revision {
            if let Some(Ok(mut tags)) = self.db.as_ref().map(|db| db.all_track_tags()) {
                for t in &mut self.library {
                    let now = tags.remove(&t.id).unwrap_or_default();
                    if t.info.tags != now {
                        std::sync::Arc::make_mut(t).info.tags = now;
                    }
                }
            }
        }
        // A group taken away takes its tags off every track.
        for t in &mut self.library {
            if t.info.tags.iter().any(|x| !groups.contains(&x.group)) {
                std::sync::Arc::make_mut(t)
                    .info
                    .tags
                    .retain(|x| groups.contains(&x.group));
            }
        }
        let f = &mut self.filter;
        f.fields = fields(&names);
        self.column_names_changed(&names);
        let f = &mut self.filter;
        f.names = names;
        // Fields that are gone can't filter.
        let ids: HashSet<String> = f.fields.iter().map(|x| x.id.clone()).collect();
        f.filters.picked.retain(|id, _| ids.contains(id));
        if f.editing.as_ref().is_some_and(|id| !ids.contains(id)) {
            f.editing = None;
        }
        self.refresh_rows();
        self.refresh_filter_ui();
        if let Some(app) = self.app.upgrade() {
            self.refresh_info(&app);
        }
    }

    /// Shows the chips, "+"'s list and the open popup.
    pub(super) fn refresh_filter_ui(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<FilterUi>();
        let f = &self.filter;
        let shown = f.filters.shown(&f.fields);
        let chips: Vec<FilterChip> = shown
            .iter()
            .map(|field| FilterChip {
                field: field.id.clone().into(),
                label: field.label.clone().into(),
                value: f.filters.summary(field, self.deck_ref, self.camelot).into(),
                active: f.filters.active(field),
                pinned: f.filters.pinned.contains(&field.id),
                kind: match field.kind {
                    Kind::Tags(_) => 1,
                    Kind::Bpm => 2,
                    Kind::Key => 3,
                    _ => 0,
                },
            })
            .collect();
        let addable: Vec<&Field> = f.fields.iter().filter(|x| !shown.contains(x)).collect();
        let strings = |v: Vec<String>| {
            ModelRc::new(VecModel::from(
                v.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))
        };
        // Core fields first in their own order, then the DJ's tag groups
        // in theirs, then the rest A–Z.
        let mut sorted = addable.clone();
        sorted.sort_by_key(|x| {
            let s = section(x);
            let name = if s == 2 {
                x.label.to_lowercase()
            } else {
                String::new()
            };
            (s, name)
        });
        ui.set_addable_sections(ModelRc::new(VecModel::from(
            sorted.iter().map(|x| section(x)).collect::<Vec<_>>(),
        )));
        ui.set_addable(strings(sorted.iter().map(|x| x.label.clone()).collect()));
        ui.set_addable_ids(strings(sorted.iter().map(|x| x.id.clone()).collect()));
        ui.set_editing(
            f.editing
                .as_ref()
                .and_then(|id| shown.iter().position(|x| x.id == *id))
                .map_or(-1, |i| i as i32),
        );
        ui.set_chips(ModelRc::new(VecModel::from(chips)));
        ui.set_any_active(f.filters.any_active(&f.fields));
        let editing = f.editing.as_deref().and_then(|id| f.field(id));
        let (shown_choices, more) = f.shown_choices();
        ui.set_more(more as i32);
        let choices: Vec<FilterChoice> = shown_choices
            .into_iter()
            .map(|c| FilterChoice {
                label: c.label.clone().into(),
                count: c.count as i32,
                on: editing.is_some_and(|e| f.filters.is_picked(&e.id, &c.term)),
                swatch: c.swatch.map_or_else(Default::default, |rgb| {
                    slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
                }),
                has_swatch: c.swatch.is_some(),
                icon: c.look.map_or(-1, |l| l.icon as i32),
                icon_tint: c
                    .look
                    .and_then(|l| l.color)
                    .and_then(|rgb| COLOR_NAMES.iter().position(|&(_, c)| c == rgb))
                    .map_or(-1, |i| i as i32),
            })
            .collect();
        ui.set_choices(ModelRc::new(VecModel::from(choices)));
        ui.set_choice_search(f.find.clone().into());
        ui.set_match_all(editing.is_some_and(|e| f.filters.match_all.contains(&e.id)));
        ui.set_follow(match editing.map(|e| e.kind) {
            Some(Kind::Bpm) => f.filters.bpm_follow,
            Some(Kind::Key) => f.filters.key_follow,
            _ => false,
        });
        ui.set_follow_note(
            match (editing.map(|e| e.kind), self.deck_ref) {
                (_, None) => "Nothing is playing, so every track is in.".to_string(),
                (Some(Kind::Bpm), Some((bpm, _))) if bpm > 0.0 => {
                    let t = TOLERANCES[f.filters.tolerance];
                    format!(
                        "Playing {bpm:.1} BPM: tracks from {:.1} to {:.1}.",
                        bpm * (1.0 - t),
                        bpm * (1.0 + t)
                    )
                }
                (Some(Kind::Key), Some((_, Some(k)))) => match f.filters.key_match {
                    1 => format!("Playing in {}: the same key only.", k.label(self.camelot)),
                    2 => format!(
                        "Playing in {}: keys that mix with it (Harmonic).",
                        k.label(self.camelot)
                    ),
                    3 => format!(
                        "Playing in {}: keys that mix with it (Fuzzy).",
                        k.label(self.camelot)
                    ),
                    _ => format!(
                        "Playing in {}: keys that mix with it, as set in Settings.",
                        k.label(self.camelot)
                    ),
                },
                _ => "The playing track has no tempo or key yet.".to_string(),
            }
            .into(),
        );
        ui.set_bpm_low(f.filters.bpm_low.clone().into());
        ui.set_bpm_high(f.filters.bpm_high.clone().into());
        ui.set_tolerance(f.filters.tolerance as i32);
        ui.set_key_match(f.filters.key_match as i32);
        let suggestions: Vec<FilterSuggestion> = f
            .suggestions
            .iter()
            .map(|(id, c)| FilterSuggestion {
                field: f
                    .field(id)
                    .map_or_else(String::new, |x| x.label.clone())
                    .into(),
                value: c.label.clone().into(),
                count: c.count as i32,
            })
            .collect();
        ui.set_suggestions(ModelRc::new(VecModel::from(suggestions)));
    }
}

/// How many values the finder offers at most.
pub const SUGGESTIONS: usize = 8;

/// Values of names and tag fields that match `text` (folded, so accents
/// and case don't matter), not picked yet, with how many of `tracks` have
/// them: words that start with it first, then most tracks.
pub fn suggestions(
    fields: &[Field],
    tracks: &[&Track],
    filters: &Filters,
    crates: &HashMap<usize, Vec<String>>,
    text: &str,
    camelot: bool,
) -> Vec<(String, Choice)> {
    let find = query::fold(text.trim());
    if find.chars().count() < 2 {
        return Vec::new();
    }
    let mut out: Vec<(bool, String, Choice)> = Vec::new();
    for field in fields {
        if !matches!(field.kind, Kind::Text | Kind::Tags(_) | Kind::Crates) {
            continue;
        }
        for c in choices(field, tracks.iter().copied(), filters, crates, camelot) {
            if c.count == 0 || c.label == "(none)" || filters.is_picked(&field.id, &c.term) {
                continue;
            }
            let label = query::fold(&c.label);
            if !label.contains(&find) {
                continue;
            }
            let starts = label.split_whitespace().any(|w| w.starts_with(&find));
            out.push((starts, field.id.clone(), c));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then(b.2.count.cmp(&a.2.count)));
    out.into_iter()
        .take(SUGGESTIONS)
        .map(|(_, id, c)| (id, c))
        .collect()
}

/// The values `field` can be set to: those `list` has, counted over the
/// tracks `left` by the other filters. Lists of names (genres, tags...)
/// go picked first, then by tracks, then by `recent` picks (newest
/// first), then in their own order; sequences (keys, ratings, years)
/// keep their order. Values `left` has none of stay, with no tracks.
pub fn linked_choices<'a>(
    field: &Field,
    list: impl Iterator<Item = &'a Track>,
    left: impl Iterator<Item = &'a Track>,
    filters: &Filters,
    crates: &HashMap<usize, Vec<String>>,
    recent: &[&str],
    camelot: bool,
) -> Vec<Choice> {
    let mut out = choices(field, list, filters, crates, camelot);
    let counted = choices(field, left, filters, crates, camelot);
    for c in out.iter_mut() {
        c.count = counted
            .iter()
            .find(|x| x.term == c.term)
            .map_or(0, |x| x.count);
    }
    if matches!(field.kind, Kind::Text | Kind::Tags(_) | Kind::Crates) {
        let rank = |c: &Choice| {
            (
                c.label == "(none)",
                !filters.is_picked(&field.id, &c.term),
                std::cmp::Reverse(c.count),
                recent
                    .iter()
                    .position(|t| *t == c.term)
                    .unwrap_or(usize::MAX),
            )
        };
        // Stable: values level on all of these keep their own order.
        out.sort_by_key(rank);
    }
    out
}

/// The values `field` can be set to, counted over `tracks`, with those
/// already picked kept even when the list doesn't have them. `crates` has
/// the crates each track is in, by track id.
pub fn choices<'a>(
    field: &Field,
    tracks: impl Iterator<Item = &'a Track>,
    filters: &Filters,
    crates: &HashMap<usize, Vec<String>>,
    camelot: bool,
) -> Vec<Choice> {
    let choice = |label: String, term: String| Choice {
        label,
        term,
        count: 0,
        swatch: None,
        look: None,
    };
    let tracks: Vec<&Track> = tracks.collect();
    let count = |out: &mut Vec<Choice>, test: &dyn Fn(&Track, &Choice) -> bool| {
        for c in out.iter_mut() {
            c.count = tracks.iter().filter(|t| test(t, c)).count();
        }
    };
    let mut out: Vec<Choice> = match field.kind {
        Kind::Text | Kind::Tags(_) | Kind::Crates | Kind::Year => {
            // Distinct values, counted, in the case first seen.
            let mut seen: HashMap<String, (String, usize)> = HashMap::new();
            let mut none = 0;
            for t in &tracks {
                let values = match field.kind {
                    Kind::Crates => crates.get(&t.id).cloned().unwrap_or_default(),
                    _ => values_of(field, t),
                };
                if values.is_empty() {
                    none += 1;
                }
                for v in values {
                    seen.entry(v.to_lowercase()).or_insert((v, 0)).1 += 1;
                }
            }
            let name = keyword(&field.id);
            let mut list: Vec<Choice> = seen
                .into_values()
                .map(|(v, n)| Choice {
                    count: n,
                    ..match field.kind {
                        Kind::Year => choice(v.clone(), format!("year:{v}")),
                        _ => choice(v.clone(), format!("{name}:={}", quoted(&v))),
                    }
                })
                .collect();
            if field.kind == Kind::Year {
                list.sort_by(|a, b| b.label.cmp(&a.label));
            } else {
                list.sort_by_key(|c| natural(&c.label));
            }
            if none > 0 && field.kind != Kind::Year {
                list.push(Choice {
                    count: none,
                    ..choice("(none)".into(), format!("{name}:\"\""))
                });
            }
            list
        }
        Kind::Key => {
            let mut keys: Vec<Key> = tracks.iter().filter_map(|t| Key::parse(&t.key)).collect();
            keys.sort_by_key(|k| (camelot_order(*k), k.camelot()));
            keys.dedup();
            let mut list: Vec<Choice> = keys
                .into_iter()
                .map(|k| {
                    let label = k.label(camelot);
                    choice(label, format!("key:{}", k.camelot()))
                })
                .collect();
            count(&mut list, &|t, c| {
                Key::parse(&t.key).map(|k| format!("key:{}", k.camelot())) == Some(c.term.clone())
            });
            list
        }
        Kind::Rating => {
            let mut list: Vec<Choice> = (0..=5u8)
                .rev()
                .map(|n| match n {
                    0 => choice("No rating".into(), "rating:0".into()),
                    n => choice("★".repeat(n as usize), format!("rating:{n}")),
                })
                .collect();
            count(&mut list, &|t, c| c.term == format!("rating:{}", t.rating));
            list
        }
        Kind::Color => {
            let mut list: Vec<Choice> = COLOR_NAMES
                .iter()
                .map(|&(name, rgb)| Choice {
                    swatch: Some(rgb),
                    ..choice(capitalized(name), format!("color:{name}"))
                })
                .chain([choice("No color".into(), "color:\"\"".into())])
                .collect();
            count(&mut list, &|t, c| t.info.color == c.swatch);
            list
        }
        Kind::Played => {
            let mut list = vec![
                choice("Never played".into(), "played:0".into()),
                choice("Played".into(), "played:>0".into()),
            ];
            count(&mut list, &|t, c| {
                (t.info.play_count > 0) == (c.term == "played:>0")
            });
            list
        }
        Kind::Added => [7, 30, 90, 365]
            .into_iter()
            .map(|d| {
                let label = match d {
                    365 => "In the last year".to_string(),
                    d => format!("In the last {d} days"),
                };
                choice(label, format!("added:<{d}d"))
            })
            .collect(),
        Kind::Bpm => Vec::new(),
    };
    // Rarely used values stay out of short lists' way, but a picked one is
    // always there to unpick.
    let fixed = matches!(
        field.kind,
        Kind::Rating | Kind::Color | Kind::Played | Kind::Added
    );
    out.retain(|c| fixed || c.count > 0 || filters.is_picked(&field.id, &c.term));
    if let Some(picked) = filters.picked.get(&field.id) {
        for (term, label) in picked {
            if !out.iter().any(|c| c.term == *term) {
                out.push(choice(label.clone(), term.clone()));
            }
        }
    }
    out
}

/// The values a track has for a list field: one for text, each tag for a
/// tag group, the year.
fn values_of(field: &Field, t: &Track) -> Vec<String> {
    let one = |s: &str| {
        let s = s.trim();
        if s.is_empty() {
            Vec::new()
        } else {
            vec![s.to_string()]
        }
    };
    match (field.kind, field.id.as_str()) {
        (Kind::Tags(group), _) => t.info.tags_in(group).map(str::to_string).collect(),
        (Kind::Year, _) => t
            .info
            .year
            .get(..4)
            .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
            .map(|y| vec![y.to_string()])
            .unwrap_or_default(),
        (_, "genre") => one(&t.genre),
        (_, "artist") => one(&t.artist),
        (_, "album") => one(&t.album),
        (_, "grouping") => one(&t.info.grouping),
        _ => Vec::new(),
    }
}

/// Sorts "2 ONS" before "10 SONG": a leading number counts as a number.
fn natural(s: &str) -> (u64, String) {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(u64::MAX), s.to_lowercase())
}

/// Keys in Camelot order: 1A, 1B, 2A ...
fn camelot_order(k: Key) -> (u32, bool) {
    let c = k.camelot();
    let n = c.trim_end_matches(['A', 'B']).parse().unwrap_or(99);
    (n, c.ends_with('B'))
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_library::query::{Indexed, Query};

    fn track(id: usize, genre: &str, grouping: &str, tags: &str, bpm: f64, key: &str) -> Track {
        let mut t = std::sync::Arc::unwrap_or_clone(djsugar_engine::sample_library().remove(0));
        t.id = id;
        t.genre = genre.into();
        t.bpm = bpm;
        t.key = key.into();
        t.info.grouping = grouping.into();
        t.info.set_tags_in(1, djsugar_engine::hashtags(tags));
        t.info.color = None;
        t.rating = 0;
        t
    }

    fn library() -> Vec<Track> {
        vec![
            track(1, "Ambient", "10 SONG", "#uplifting #bright", 120.0, "8A"),
            track(2, "Ambient", "2 ONS", "#lullaby", 90.0, "Dm"),
            track(3, "Deep House", "", "#uplifting", 124.0, "3B"),
        ]
    }

    fn find(lib: &[Track], text: &str) -> Vec<usize> {
        let q = Query::parse(text);
        let cx = Default::default();
        lib.iter()
            .filter(|t| q.matches(t, &Indexed::new(t), &cx))
            .map(|t| t.id)
            .collect()
    }

    fn no_crates() -> HashMap<usize, Vec<String>> {
        HashMap::new()
    }

    fn field<'a>(fields: &'a [Field], id: &str) -> &'a Field {
        fields.iter().find(|f| f.id == id).unwrap()
    }

    #[test]
    fn other_chips_narrow_the_counts_and_order() {
        let lib = library();
        let fields = fields(&FieldNames::default());
        let mut f = Filters::default();
        let tags = field(&fields, "tag1");
        // Uplifting is on two tracks, so it goes first.
        let all = linked_choices(tags, lib.iter(), lib.iter(), &f, &no_crates(), &[], true);
        let labels: Vec<_> = all.iter().map(|c| (c.label.as_str(), c.count)).collect();
        assert_eq!(labels, [("uplifting", 2), ("bright", 1), ("lullaby", 1)]);
        // A recent pick goes first among values with as many tracks.
        let lullaby = all
            .iter()
            .find(|c| c.label == "lullaby")
            .unwrap()
            .term
            .clone();
        let again = linked_choices(
            tags,
            lib.iter(),
            lib.iter(),
            &f,
            &no_crates(),
            &[&lullaby],
            true,
        );
        assert_eq!(again[1].label, "lullaby");
        // Genre Ambient leaves tracks 1 and 2: lullaby and bright have one
        // each, uplifting one; nothing is dropped.
        let genre = field(&fields, "genre");
        let ambient = choices(genre, lib.iter(), &f, &no_crates(), true)
            .into_iter()
            .find(|c| c.label == "Ambient")
            .unwrap();
        f.toggle("genre", &ambient);
        let left: Vec<_> = lib
            .iter()
            .filter(|t| find(&lib, &f.text_except(&fields, None, "tag1")).contains(&t.id))
            .collect();
        let narrowed = linked_choices(
            tags,
            lib.iter(),
            left.into_iter(),
            &f,
            &no_crates(),
            &[],
            true,
        );
        let labels: Vec<_> = narrowed
            .iter()
            .map(|c| (c.label.as_str(), c.count))
            .collect();
        assert_eq!(labels, [("bright", 1), ("lullaby", 1), ("uplifting", 1)]);
        // The genre chip itself still counts every genre.
        assert_eq!(f.text_except(&fields, None, "genre"), "");
    }

    #[test]
    fn the_key_chip_can_match_its_own_way() {
        let fields = fields(&FieldNames::default());
        let mut f = Filters {
            key_follow: true,
            ..Default::default()
        };
        let deck = Some((120.0, Key::parse("8A")));
        let text = |f: &Filters| f.text(&fields, deck);
        assert_eq!(text(&f), "~key:8A");
        f.key_match = 1;
        assert_eq!(text(&f), "key:8A");
        f.key_match = 3;
        assert_eq!(text(&f), "~fuzzy:8A");
        // Fuzzy lets 9B in; Harmonic doesn't.
        let lib = vec![track(1, "", "", "", 120.0, "9B")];
        assert_eq!(find(&lib, &text(&f)), [1]);
        f.key_match = 2;
        assert!(find(&lib, &text(&f)).is_empty());
    }

    #[test]
    fn core_fields_then_tags_then_the_rest() {
        let names = FieldNames {
            tags: vec![djsugar_library::tags::TagGroup::new(7, "Mood")],
            ..FieldNames::default()
        };
        let fields = fields(&names);
        let sec = |id: &str| section(field(&fields, id));
        assert_eq!((sec("genre"), sec("key"), sec("bpm")), (0, 0, 0));
        assert_eq!(sec("tag1"), 1);
        assert_eq!((sec("grouping"), sec("album"), sec("year")), (2, 2, 2));
        // The default bar skips Grouping and tag groups the DJ hasn't got.
        let f = Filters::new(DEFAULT_PINNED.iter().map(|s| s.to_string()).collect(), 4);
        let shown: Vec<_> = f.shown(&fields).iter().map(|x| x.id.as_str()).collect();
        assert_eq!(shown, ["genre", "tag1", "crate", "bpm", "key"]);
        assert_eq!(sec("crate"), 0);
    }

    #[test]
    fn the_finder_offers_values_from_any_field() {
        let lib = library();
        let tracks: Vec<&Track> = lib.iter().collect();
        let fields = fields(&FieldNames::default());
        let mut f = Filters::default();
        let found = |f: &Filters, text: &str| -> Vec<(String, String, usize)> {
            suggestions(&fields, &tracks, f, &no_crates(), text, true)
                .into_iter()
                .map(|(id, c)| (id, c.label, c.count))
                .collect()
        };
        // "up" starts a tag; "ambient" is a genre on two tracks.
        assert_eq!(found(&f, "up"), [("tag1".into(), "uplifting".into(), 2)]);
        assert_eq!(found(&f, "AMBI"), [("genre".into(), "Ambient".into(), 2)]);
        // One letter is too little to suggest anything.
        assert!(found(&f, "a").is_empty());
        // A value already on the bar isn't offered again.
        let pick = suggestions(&fields, &tracks, &f, &no_crates(), "ambi", true).remove(0);
        f.toggle(&pick.0, &pick.1);
        assert!(found(&f, "ambi").is_empty());
    }

    #[test]
    fn chips_move_and_come_off_the_bar() {
        let fields = fields(&FieldNames::default());
        let mut f = Filters::new(vec!["genre".into(), "bpm".into(), "key".into()], 4);
        // Key dragged before Genre.
        f.move_chip(&fields, 2, 0);
        assert_eq!(f.pinned, ["key", "genre", "bpm"]);
        // Key dragged past the end.
        f.move_chip(&fields, 0, 3);
        assert_eq!(f.pinned, ["genre", "bpm", "key"]);
        // Taking a chip off clears it too, so it doesn't come back.
        f.key_follow = true;
        f.remove(field(&fields, "key"));
        assert_eq!(f.pinned, ["genre", "bpm"]);
        assert!(!f.active(field(&fields, "key")));
        // Three chips at 0, 60 and 120, 50 wide: x = 100 lands before the
        // third, past the last middle at the end; next to where a chip already is, it stays.
        let placed = [(0.0, 50.0), (60.0, 50.0), (120.0, 50.0)];
        assert_eq!(Filters::drop_place(0, 100.0, &placed), Some(2));
        assert_eq!(Filters::drop_place(0, 150.0, &placed), Some(3));
        assert_eq!(Filters::drop_place(0, 300.0, &placed), Some(3));
        assert_eq!(Filters::drop_place(1, 70.0, &placed), None);
        assert_eq!(Filters::drop_place(1, 100.0, &placed), None);
    }

    #[test]
    fn lists_values_in_order_with_counts() {
        let lib = library();
        let fields = fields(&FieldNames::default());
        let f = Filters::default();
        let grouping = choices(
            field(&fields, "grouping"),
            lib.iter(),
            &f,
            &no_crates(),
            true,
        );
        let labels: Vec<_> = grouping
            .iter()
            .map(|c| (c.label.as_str(), c.count))
            .collect();
        assert_eq!(labels, [("2 ONS", 1), ("10 SONG", 1), ("(none)", 1)]);
        let tags = choices(field(&fields, "tag1"), lib.iter(), &f, &no_crates(), true);
        let labels: Vec<_> = tags.iter().map(|c| (c.label.as_str(), c.count)).collect();
        assert_eq!(labels, [("bright", 1), ("lullaby", 1), ("uplifting", 2)]);
        let keys = choices(field(&fields, "key"), lib.iter(), &f, &no_crates(), true);
        let labels: Vec<_> = keys.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["3B", "7A", "8A"]);
    }

    #[test]
    fn chips_narrow_the_list_together() {
        let lib = library();
        let fields = fields(&FieldNames::default());
        let mut f = Filters::default();
        let pick = |f: &mut Filters, id: &str, label: &str| {
            let c = choices(field(&fields, id), lib.iter(), f, &no_crates(), true)
                .into_iter()
                .find(|c| c.label == label)
                .unwrap();
            f.toggle(id, &c);
        };
        pick(&mut f, "genre", "Ambient");
        assert_eq!(find(&lib, &f.text(&fields, None)), [1, 2]);
        pick(&mut f, "tag1", "uplifting");
        assert_eq!(find(&lib, &f.text(&fields, None)), [1]);
        pick(&mut f, "tag1", "lullaby");
        assert_eq!(find(&lib, &f.text(&fields, None)), [1, 2], "any word");
        f.match_all.insert("tag1".into());
        assert_eq!(
            find(&lib, &f.text(&fields, None)),
            Vec::<usize>::new(),
            "all words"
        );
        f.match_all.clear();
        assert_eq!(
            f.summary(field(&fields, "tag1"), None, true),
            "uplifting +1"
        );
        f.clear_all(&fields);
        assert_eq!(f.text(&fields, None), "");

        pick(&mut f, "grouping", "(none)");
        assert_eq!(find(&lib, &f.text(&fields, None)), [3]);
        f.clear_all(&fields);

        f.bpm_low = "100".into();
        assert_eq!(find(&lib, &f.text(&fields, None)), [1, 3]);
        f.bpm_high = "122".into();
        assert_eq!(find(&lib, &f.text(&fields, None)), [1]);
        f.clear_all(&fields);

        // Following the playing track: 122 BPM in 8A.
        let deck = Some((122.0, Key::parse("8A")));
        f.bpm_follow = true;
        f.tolerance = 0;
        assert_eq!(find(&lib, &f.text(&fields, deck)), [1, 3]);
        f.key_follow = true;
        assert_eq!(find(&lib, &f.text(&fields, deck)), [1]);
        assert_eq!(f.summary(field(&fields, "bpm"), deck, true), "122 ±2%");
        assert_eq!(f.summary(field(&fields, "key"), deck, true), "Match 8A");
        assert_eq!(f.text(&fields, None), "", "nothing playing leaves all in");
    }

    #[test]
    fn a_click_on_bpm_or_key_matches_the_playing_track() {
        let lib = library();
        let fields = fields(&FieldNames::default());
        let (bpm, key) = (field(&fields, "bpm"), field(&fields, "key"));
        let deck = Some((122.0, Key::parse("8A")));
        let mut f = Filters::new(Vec::new(), 4);
        f.quick(bpm);
        assert!(f.bpm_follow);
        assert_eq!(f.summary(bpm, deck, true), "122 ±10%");
        f.quick(key);
        assert_eq!(find(&lib, &f.text(&fields, deck)), [1]);
        // Again: off.
        f.quick(bpm);
        f.quick(key);
        assert_eq!(f.text(&fields, deck), "");
        // A typed range or picked keys count as on: the click clears them.
        f.bpm_low = "100".into();
        f.quick(bpm);
        assert!(!f.active(bpm) && !f.bpm_follow);
        // Other fields don't take quick clicks.
        f.quick(field(&fields, "genre"));
        assert!(!f.any_active(&fields));
    }

    #[test]
    fn pinned_chips_then_the_ones_in_use() {
        let fields = fields(&FieldNames::default());
        let mut f = Filters::new(vec!["bpm".into(), "genre".into()], 2);
        let ids = |f: &Filters| {
            f.shown(&fields)
                .iter()
                .map(|x| x.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&f), ["bpm", "genre"]);
        f.added.push("year".into());
        f.key_follow = true;
        assert_eq!(ids(&f), ["bpm", "genre", "year", "key"]);
        f.clear_all(&fields);
        assert_eq!(ids(&f), ["bpm", "genre"]);
    }

    #[test]
    fn crates_list_with_counts_and_match_any_picked() {
        let lib = library();
        let fields = fields(&FieldNames::default());
        let crates_field = field(&fields, "crate");
        let crates = HashMap::from([
            (1, vec!["Yoga".to_string()]),
            (2, vec!["Dance".to_string(), "Yoga".to_string()]),
        ]);
        let mut f = Filters::default();
        let list = linked_choices(crates_field, lib.iter(), lib.iter(), &f, &crates, &[], true);
        let labels: Vec<_> = list.iter().map(|c| (c.label.as_str(), c.count)).collect();
        assert_eq!(labels, [("Yoga", 2), ("Dance", 1), ("(none)", 1)]);
        // Any of the crates picked: a track in either is in.
        f.toggle("crate", &list[0]);
        f.toggle("crate", &list[1]);
        let q = Query::parse(&f.text(&fields, None));
        let names = crates.clone();
        let cx = djsugar_library::query::Context {
            crates: Some(&names),
            ..Default::default()
        };
        let found: Vec<usize> = lib
            .iter()
            .filter(|t| q.matches(t, &Indexed::new(t), &cx))
            .map(|t| t.id)
            .collect();
        assert_eq!(found, [1, 2]);
        // Not in a crate.
        f.picked.clear();
        f.toggle("crate", &list[2]);
        let q = Query::parse(&f.text(&fields, None));
        assert!(lib
            .iter()
            .filter(|t| q.matches(t, &Indexed::new(t), &cx))
            .map(|t| t.id)
            .eq([3]));
    }

    #[test]
    fn a_saved_bar_gets_crates_before_bpm() {
        let v = |ids: &[&str]| ids.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            with_crates(v(&["genre", "bpm", "key"])),
            v(&["genre", "crate", "bpm", "key"])
        );
        assert_eq!(with_crates(v(&["genre"])), v(&["genre", "crate"]));
        assert_eq!(with_crates(v(&["crate", "genre"])), v(&["crate", "genre"]));
    }
}
