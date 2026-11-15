//! Clean-up in the browser (see [`djsugar_library::cleanup`]): the review
//! table of changes to names, the genres page, finding covers and looking
//! up tags online for many tracks at once, and making (and taking back)
//! changes to tags, in the library and in the files.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use djsugar_engine::{tags, Track};
use djsugar_library::cleanup::{self, Change, Field, Logged};
use djsugar_library::health::{self, says_nothing, Check};
use djsugar_library::query::Indexed;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::health::tracks;
use super::info::{tag_fields, writable};
use super::{file_path, is_browsed, Browser, Undo};
use crate::coverart::{self, Wanted};
use crate::lookup::{self, Found, Miss, Query};
use crate::{cover_scan, settings, CleanupRow, CleanupUi, FixNamesUi, GenreRow};
use djsugar_library::fix_names::{self, Ready, Tool};

/// How many changes the Fix Names dialog shows before Review.
const PREVIEW: usize = 6;

/// The fields of the Fix Names dialog's field menu: all names, then one.
fn fix_fields(field: i32) -> Vec<Field> {
    match field {
        1 => vec![Field::Title],
        2 => vec![Field::Artist],
        3 => vec![Field::Remix],
        4 => vec![Field::Album],
        5 => vec![Field::Genre],
        6 => vec![Field::Comment],
        7 => vec![Field::Grouping],
        _ => fix_names::NAMES.to_vec(),
    }
}

/// The tool the Fix Names dialog is set to.
fn fix_tool(ui: &FixNamesUi) -> Tool {
    match ui.get_tool() {
        1 => Tool::Case(match ui.get_case() {
            1 => fix_names::Case::Sentence,
            2 => fix_names::Case::Upper,
            3 => fix_names::Case::Lower,
            _ => fix_names::Case::Title,
        }),
        2 => Tool::RemoveBrackets,
        3 => Tool::Swap,
        _ => Tool::Replace {
            find: ui.get_find().to_string(),
            with: ui.get_with().to_string(),
            match_case: ui.get_match_case(),
            whole_words: ui.get_whole_words(),
            pattern: ui.get_pattern(),
        },
    }
}

/// A change waiting in the review table, for a track (an index into
/// `library`).
struct Pending {
    index: usize,
    change: Change,
    on: bool,
    /// Where the new value came from, such as "MusicBrainz 92%"; empty
    /// for the clean-up rules.
    note: String,
}

/// A look-up at least this sure ticks its changes to empty fields.
const SURE: f32 = 0.8;

/// Tracks being looked up online, one at a time (the look-up paces its
/// requests as each service asks).
struct LookupBatch {
    /// Track ids and what was found for each, as they come back.
    done: Receiver<(usize, Result<Found, Miss>)>,
    total: usize,
    found: Vec<(usize, Found)>,
    back: usize,
    /// Tracks nothing matched, and those whose service couldn't be reached.
    not_found: usize,
    unreachable: usize,
    /// Of `total`, how many were found by the sound and by name.
    by_sound: usize,
    /// Files in iCloud only: looked up by name, not fingerprinted.
    offloaded: usize,
    /// Artist and title from each track's file name, by track id, for
    /// those whose tags lack them.
    names: HashMap<usize, (String, String)>,
}

/// What the review table is of, which also names its undo line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Review {
    #[default]
    CleanUp,
    LookUp,
    FileNames,
    AlbumTags,
    FixNames,
}

impl Review {
    fn heading(self) -> &'static str {
        match self {
            Review::CleanUp => "Clean up",
            Review::LookUp => "Look-up results for",
            Review::FileNames => "From file names for",
            Review::AlbumTags => "Album tags for",
            Review::FixNames => "Fix names for",
        }
    }

    /// For "Undo …".
    fn what(self) -> &'static str {
        match self {
            Review::CleanUp => "clean-up",
            Review::LookUp => "look-up",
            Review::FileNames => "file names",
            Review::AlbumTags => "album tags",
            Review::FixNames => "fix names",
        }
    }
}

/// Whether a field counts as empty: nothing in it, or nothing worth
/// keeping ("Unknown Artist", "Track 01").
fn blank(value: &str) -> bool {
    value.trim().is_empty() || says_nothing(value)
}

/// The artist and title in `t`'s file name ("Artist - Title.mp3") for
/// whichever of the two its tags lack. A file without tags is listed by
/// its name, as its title: that title counts as lacking too.
fn name_from_file(t: &Track) -> Option<(String, String)> {
    let path = file_path(t)?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    let named = Some(t.title.trim()) == stem.as_deref().map(str::trim);
    if !(blank(&t.artist) || blank(&t.title) || named) {
        return None;
    }
    health::from_file_name(path)
}

/// The changes a file name proposes: an artist or title only where the
/// tags have none (or one that says nothing), ticked, since that is
/// what the file name says and Apply is the DJ's call.
fn name_proposals(t: &Track, artist: &str, title: &str) -> Vec<Change> {
    let stem = file_path(t)
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().trim().to_string());
    let named = stem.as_deref() == Some(t.title.trim());
    let mut out = Vec::new();
    if blank(&t.artist) {
        out.push(Change {
            field: Field::Artist,
            old: t.artist.clone(),
            new: artist.to_string(),
        });
    }
    if blank(&t.title) || named {
        out.push(Change {
            field: Field::Title,
            old: t.title.clone(),
            new: title.to_string(),
        });
    }
    out.retain(|c| c.old.trim() != c.new.trim());
    out
}

/// Covers being looked for online, one track at a time.
struct CoverBatch {
    /// Track ids and the cover found for each, as they come back.
    done: Receiver<(usize, Option<Vec<u8>>)>,
    total: usize,
    back: usize,
    found: usize,
}

#[derive(Default)]
pub(super) struct CleanupState {
    review: Vec<Pending>,
    genres_open: bool,
    /// Every genre, its track count, and whether it is ticked.
    genres: Vec<(String, usize, bool)>,
    covers: Option<CoverBatch>,
    lookups: Option<LookupBatch>,
    /// What the review table is of.
    kind: Review,
    /// The tracks the Fix Names dialog is open for (indices into
    /// `library`).
    fix_tracks: Vec<usize>,
}

/// A genre as compared for "spelled like another": letters and digits
/// only, in small letters.
/// The changes a look-up proposes for a track, each with whether it is
/// ticked: only for a sure match filling an empty field. Fields the DJ
/// typed in by hand are left alone.
fn proposals(t: &Track, f: &Found, hand: &HashSet<(usize, Field)>) -> Vec<(Change, bool)> {
    [
        (Field::Artist, &f.artist),
        (Field::Title, &f.title),
        (Field::Album, &f.album),
        (Field::Year, &f.year),
        (Field::Label, &f.label),
        (Field::Genre, &f.genre),
    ]
    .into_iter()
    .filter(|(field, new)| !new.trim().is_empty() && !hand.contains(&(t.id, *field)))
    .filter_map(|(field, new)| {
        let old = field.get(t).trim().to_string();
        // Same but for capitals is not worth a change.
        if old.to_lowercase() == new.trim().to_lowercase() {
            return None;
        }
        // A guessed genre (the artist's) is shown, never ticked.
        let guess = field == Field::Genre && f.genre_guessed;
        let on = blank(&old) && f.score >= SURE && !guess;
        Some((
            Change {
                field,
                old,
                new: new.trim().to_string(),
            },
            on,
        ))
    })
    .collect()
}

/// A track's look-up, and what else it needs to know.
pub(super) struct QueryFor {
    pub(super) query: Query,
    /// The artist and title its file name gives, for what the tags lack.
    pub(super) name: Option<(String, String)>,
    /// Its file is in iCloud only, so it isn't fingerprinted.
    pub(super) in_icloud: bool,
}

/// What to look `t` up by: its tags, with its file name for an artist or
/// title they lack, and its sound when there's an AcoustID key and the
/// file is here. Nothing when there is nothing to look it up by.
pub(super) fn query_for(t: &Track, key: &str, token: &str) -> Option<QueryFor> {
    // Only a file that's here is fingerprinted; one in iCloud
    // would be downloaded for it.
    let here = writable(t).map(|p| p.to_path_buf());
    let path = here.clone().filter(|p| !cover_scan::offloaded(p));
    let in_icloud = here.is_some() && path.is_none();
    // "Unknown Artist" is no name to search by; the file's name may have
    // a better one.
    let mut q = Query {
        path,
        artist: if blank(&t.artist) {
            String::new()
        } else {
            t.artist.clone()
        },
        title: if blank(&t.title) {
            String::new()
        } else {
            t.title.clone()
        },
        duration: t.duration,
        acoustid_key: key.to_string(),
        discogs_token: token.to_string(),
        ..Default::default()
    };
    let mut name = None;
    if let Some((artist, title)) = name_from_file(t) {
        if q.artist.is_empty() {
            q.artist = artist.clone();
            q.by_file_name = true;
        }
        let stem = here
            .as_deref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().trim().to_string());
        if q.title.is_empty() || stem.as_deref() == Some(q.title.trim()) {
            q.title = title.clone();
            q.by_file_name = true;
        }
        name = Some((artist, title));
    }
    let by_sound = q.path.is_some() && !key.is_empty();
    let by_name = !(q.artist.trim().is_empty() || q.title.trim().is_empty());
    (by_sound || by_name).then_some(QueryFor {
        query: q,
        name,
        in_icloud,
    })
}

/// What one track's look-up, asked for from its Info tab, would change:
/// a sure match's values (an empty field's, or one the DJ didn't type),
/// never a guessed genre, then the file name's artist and title for what
/// the match didn't give. Fields typed by hand are left alone.
pub(super) fn info_proposals(
    t: &Track,
    found: Option<&Found>,
    name: Option<&(String, String)>,
    hand: &HashSet<(usize, Field)>,
) -> Vec<Change> {
    let mut out: Vec<Change> = found
        .map(|f| {
            proposals(t, f, hand)
                .into_iter()
                .filter(|(c, on)| {
                    *on || (f.score >= SURE && !(c.field == Field::Genre && f.genre_guessed))
                })
                .map(|(c, _)| c)
                .collect()
        })
        .unwrap_or_default();
    if let Some((a, b)) = name {
        let from_name: Vec<Change> = name_proposals(t, a, b)
            .into_iter()
            .filter(|c| !out.iter().any(|o| o.field == c.field))
            .filter(|c| !hand.contains(&(t.id, c.field)))
            .collect();
        out.extend(from_name);
    }
    out
}

fn genre_key(g: &str) -> String {
    g.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl Browser {
    pub(super) fn install_cleanup(
        ui: &CleanupUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        ui.on_toggle(move |row| {
            w(&|b| {
                if let Some(p) = b.cleanup.review.get_mut(row.max(0) as usize) {
                    p.on = !p.on;
                }
                b.refresh_cleanup_ui();
            })
        });
        let w = with.clone();
        ui.on_all(move |on| {
            w(&|b| {
                for p in &mut b.cleanup.review {
                    p.on = on;
                }
                b.refresh_cleanup_ui();
            })
        });
        let w = with.clone();
        ui.on_apply(move || w(&|b| b.apply_review()));
        let w = with.clone();
        ui.on_cancel(move || {
            w(&|b| {
                b.cleanup.review.clear();
                b.refresh_cleanup_ui();
            })
        });
        let w = with.clone();
        ui.on_open_genres(move || w(&|b| b.open_genres()));
        let w = with.clone();
        ui.on_toggle_genre(move |row| w(&|b| b.toggle_genre(row.max(0) as usize)));
        let w = with.clone();
        ui.on_merge_genres(move || w(&|b| b.merge_genres()));
        let w = with.clone();
        ui.on_close_genres(move || {
            w(&|b| {
                b.cleanup.genres_open = false;
                b.refresh_cleanup_ui();
            })
        });
    }

    pub(super) fn install_fix_names(
        app: &crate::AppWindow,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let ui = app.global::<FixNamesUi>();
        let w = with.clone();
        ui.on_start(move |first, last| {
            w(&|b| {
                let tracks: Vec<usize> = b
                    .chosen_tracks(first.max(0) as usize, last.max(0) as usize)
                    .into_iter()
                    .filter(|&i| !is_browsed(&b.library[i]))
                    .collect();
                if tracks.is_empty() {
                    return;
                }
                b.cleanup.fix_tracks = tracks;
                b.fix_names_preview(true);
            })
        });
        let w = with.clone();
        ui.on_update(move || w(&|b| b.fix_names_preview(false)));
        let w = with.clone();
        ui.on_review(move || w(&|b| b.fix_names_review()));
        let w = with.clone();
        ui.on_cancel(move || {
            w(&|b| {
                b.cleanup.fix_tracks.clear();
                if let Some(app) = b.app.upgrade() {
                    app.global::<FixNamesUi>().set_open(false);
                }
            })
        });
    }

    /// The changes the Fix Names dialog's tool makes to its tracks, or why
    /// it can't run.
    fn fix_names_changes(&self) -> Result<Vec<Pending>, String> {
        let Some(app) = self.app.upgrade() else {
            return Ok(Vec::new());
        };
        let ui = app.global::<FixNamesUi>();
        let ready = Ready::new(fix_tool(&ui))?;
        let fields = fix_fields(ui.get_field());
        Ok(self
            .cleanup
            .fix_tracks
            .iter()
            .flat_map(|&i| {
                ready
                    .changes(&self.library[i], &fields)
                    .into_iter()
                    .map(move |change| Pending {
                        index: i,
                        change,
                        on: true,
                        note: String::new(),
                    })
            })
            .collect())
    }

    /// Opens the Fix Names dialog (`open`), or shows what its tool, as set
    /// now, would change.
    fn fix_names_preview(&mut self, open: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<FixNamesUi>();
        if open {
            ui.set_count(self.cleanup.fix_tracks.len() as i32);
            ui.set_open(true);
        }
        // An empty Find box isn't a mistake yet.
        let typing = ui.get_tool() == 0 && ui.get_find().is_empty();
        let (changes, error) = match self.fix_names_changes() {
            Ok(c) => (c, String::new()),
            Err(_) if typing => (Vec::new(), String::new()),
            Err(e) => (Vec::new(), e),
        };
        let rows: Vec<CleanupRow> = changes
            .iter()
            .take(PREVIEW)
            .map(|p| CleanupRow {
                track: SharedString::new(),
                field: p.change.field.label().as_ref().into(),
                old: p.change.old.clone().into(),
                new: p.change.new.clone().into(),
                on: true,
            })
            .collect();
        let changing: HashSet<usize> = changes.iter().map(|p| p.index).collect();
        ui.set_preview(ModelRc::new(VecModel::from(rows)));
        ui.set_changing(changing.len() as i32);
        ui.set_error(error.into());
    }

    /// Puts the Fix Names dialog's changes in the review table.
    fn fix_names_review(&mut self) {
        let Ok(changes) = self.fix_names_changes() else {
            return;
        };
        if changes.is_empty() {
            return;
        }
        if let Some(app) = self.app.upgrade() {
            app.global::<FixNamesUi>().set_open(false);
        }
        self.cleanup.fix_tracks.clear();
        self.cleanup.review = changes;
        self.cleanup.kind = Review::FixNames;
        self.refresh_cleanup_ui();
    }

    /// Closes the review table and the genres page, as another list is
    /// picked.
    pub(super) fn close_cleanup(&mut self) {
        self.cleanup.review.clear();
        self.cleanup.genres_open = false;
    }

    /// Opens the review table with what the clean-up rules would change in
    /// these tracks (indices into `library`).
    pub(super) fn open_cleanup(&mut self, indices: &[usize]) -> String {
        let rules = settings::cleanup_rules();
        self.cleanup.review = indices
            .iter()
            .flat_map(|&i| {
                cleanup::changes(&self.library[i], &rules)
                    .into_iter()
                    .map(move |change| Pending {
                        index: i,
                        change,
                        on: true,
                        note: String::new(),
                    })
            })
            .collect();
        self.cleanup.kind = Review::CleanUp;
        self.refresh_cleanup_ui();
        if self.cleanup.review.is_empty() {
            "Nothing to clean up with the rules turned on in Settings › Collection.".into()
        } else {
            String::new()
        }
    }

    /// Opens the review table with each of these tracks (indices into
    /// `library`) given the album tag its album folder should share.
    pub(super) fn review_album_tags(&mut self, indices: &[usize]) -> String {
        self.cleanup.review = indices
            .iter()
            .filter_map(|&i| {
                let t = &self.library[i];
                let new = self.health.album_tag(t.id)?;
                Some(Pending {
                    index: i,
                    change: Change {
                        field: Field::Album,
                        old: t.album.clone(),
                        new: new.to_string(),
                    },
                    on: true,
                    note: "Most of its album".into(),
                })
            })
            .collect();
        self.cleanup.kind = Review::AlbumTags;
        self.refresh_cleanup_ui();
        if self.cleanup.review.is_empty() {
            "Every album already shares one album tag.".into()
        } else {
            String::new()
        }
    }

    /// Shows the review table and the genres as things are now.
    pub(super) fn refresh_cleanup_ui(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<CleanupUi>();
        let review = &self.cleanup.review;
        let rows: Vec<CleanupRow> = review
            .iter()
            .map(|p| {
                let t = &self.library[p.index];
                let mut track = format!("{} - {}", t.artist, t.title);
                if !p.note.is_empty() {
                    track += &format!(" · {}", p.note);
                }
                CleanupRow {
                    track: track.into(),
                    field: p.change.field.label().as_ref().into(),
                    old: p.change.old.clone().into(),
                    new: p.change.new.clone().into(),
                    on: p.on,
                }
            })
            .collect();
        let n: HashSet<usize> = review.iter().map(|p| p.index).collect();
        ui.set_open(!review.is_empty());
        ui.set_heading(format!("{} {}", self.cleanup.kind.heading(), tracks(n.len())).into());
        ui.set_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_picked(review.iter().filter(|p| p.on).count() as i32);
        let all = self.genre_counts();
        let mut keys: HashMap<String, usize> = HashMap::new();
        for g in all.keys() {
            *keys.entry(genre_key(g)).or_default() += 1;
        }
        let alike = |g: &str| keys.get(&genre_key(g)).is_some_and(|&n| n > 1);
        ui.set_genre_count(if self.saved() { all.len() as i32 } else { 0 });
        ui.set_alike(all.keys().filter(|g| alike(g)).count() as i32);
        ui.set_genres_open(self.cleanup.genres_open);
        let genres: Vec<GenreRow> = self
            .cleanup
            .genres
            .iter()
            .map(|(name, count, picked)| GenreRow {
                name: name.clone().into(),
                count: *count as i32,
                picked: *picked,
                alike: alike(name),
            })
            .collect();
        ui.set_genres(ModelRc::new(VecModel::from(genres)));
        ui.set_genres_picked(self.cleanup.genres.iter().filter(|g| g.2).count() as i32);
    }

    /// Makes the ticked changes in the review table.
    fn apply_review(&mut self) {
        let review = std::mem::take(&mut self.cleanup.review);
        let changes: Vec<(usize, Field, String)> = review
            .into_iter()
            .filter(|p| p.on)
            .map(|p| (p.index, p.change.field, p.change.new))
            .collect();
        let what = self.cleanup.kind.what();
        self.health_status = self.make_changes(changes, what);
        self.refresh_cleanup_ui();
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Sets fields of tracks (indices into `library`), in the library and
    /// the files' tags, as one batch the undo line takes back. Says what
    /// was done.
    pub(super) fn make_changes(
        &mut self,
        changes: Vec<(usize, Field, String)>,
        what: &str,
    ) -> String {
        let (logged, unwritten) = self.set_fields(changes);
        if logged.is_empty() {
            return "Nothing changed.".into();
        }
        let n = logged.iter().map(|l| l.0).collect::<HashSet<_>>().len();
        if let Some(db) = &mut self.db {
            match db.log_changes(&logged) {
                Ok(batch) => {
                    self.undo = Some(Undo {
                        batch: None,
                        text: format!("Undo {what} ({})", tracks(n)),
                        restore: Vec::new(),
                        changes: Some(batch),
                        relinked: Vec::new(),
                    });
                }
                Err(e) => eprintln!("can't log the changes: {e}"),
            }
        }
        let mut s = format!("Changed {}.", tracks(n));
        if unwritten > 0 {
            s += &format!(
                " {} couldn't be written to the file (not on this computer, or locked), only to your collection.",
                tracks(unwritten)
            );
        }
        s
    }

    /// Sets fields of tracks (indices into `library`) in the library and,
    /// where the file is here, its tags. Returns what changed, by track
    /// id, and how many tracks' files couldn't be written.
    fn set_fields(&mut self, changes: Vec<(usize, Field, String)>) -> (Vec<Logged>, usize) {
        let mut by_track: BTreeMap<usize, Vec<(Field, String)>> = BTreeMap::new();
        for (i, field, value) in changes {
            by_track.entry(i).or_default().push((field, value));
        }
        let (mut logged, mut unwritten) = (Vec::new(), 0);
        for (i, fields) in by_track {
            let Some(current) = self.library.get(i).filter(|t| !is_browsed(t)) else {
                continue;
            };
            let mut t = (**current).clone();
            let mut these = Vec::new();
            for (field, value) in fields {
                let old = field.get(&t).to_string();
                if old != value {
                    these.push((t.id, field, old, value.clone()));
                    field.set(&mut t, value);
                }
            }
            if these.is_empty() {
                continue;
            }
            match writable(&t) {
                Some(path) => {
                    if let Err(e) = tags::write(path, &tag_fields(&t)) {
                        eprintln!("can't write tags to {}: {e}", path.display());
                        unwritten += 1;
                    }
                }
                None => unwritten += 1,
            }
            if let Some(db) = &self.db {
                if let Err(e) = db.save_info(t.id, &t) {
                    eprintln!("can't save: {e}");
                    continue;
                }
            }
            self.indexed[i] = Indexed::new(&t);
            self.library[i] = Arc::new(t);
            logged.extend(these);
        }
        if logged.iter().any(|(_, field, _, _)| *field == Field::Album) {
            self.refresh_album_tags();
        }
        (logged, unwritten)
    }

    /// Takes back a batch of changes to tags: each field gets its old
    /// value again.
    pub(super) fn undo_tag_changes(&mut self, batch: i64) {
        let Some(db) = &mut self.db else { return };
        let logged = match db.logged_changes(batch) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("can't read the changes: {e}");
                return;
            }
        };
        if let Err(e) = db.undo_changes(batch) {
            eprintln!("can't undo: {e}");
            return;
        }
        // Last change first, so a field changed twice ends as it began.
        let back: Vec<(usize, Field, String)> = logged
            .into_iter()
            .rev()
            .filter_map(|(id, field, old, _)| Some((*self.index_of.get(&id)?, field, old)))
            .collect();
        let n = back.iter().map(|b| b.0).collect::<HashSet<_>>().len();
        self.set_fields(back);
        self.health_status = format!("{} as they were.", tracks(n));
        self.refresh_cleanup_ui();
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Every genre in the collection and how many tracks have it.
    fn genre_counts(&self) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for t in self.library.iter().filter(|t| self.in_collection(t)) {
            let g = t.genre.trim();
            if !g.is_empty() {
                *out.entry(g.to_string()).or_default() += 1;
            }
        }
        out
    }

    /// Opens the genres page, every genre in name order.
    fn open_genres(&mut self) {
        let mut genres: Vec<(String, usize, bool)> = self
            .genre_counts()
            .into_iter()
            .map(|(g, n)| (g, n, false))
            .collect();
        genres.sort_by(|a, b| {
            a.0.to_lowercase()
                .cmp(&b.0.to_lowercase())
                .then(a.0.cmp(&b.0))
        });
        self.cleanup.genres = genres;
        self.cleanup.genres_open = true;
        if let Some(app) = self.app.upgrade() {
            app.global::<CleanupUi>().set_into(SharedString::new());
        }
        self.refresh_cleanup_ui();
    }

    /// Ticks or unticks a genre; the first one ticked (the most used of
    /// those ticked) fills in the name to merge into.
    fn toggle_genre(&mut self, row: usize) {
        let Some(g) = self.cleanup.genres.get_mut(row) else {
            return;
        };
        g.2 = !g.2;
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<CleanupUi>();
            let picked: Vec<&(String, usize, bool)> =
                self.cleanup.genres.iter().filter(|g| g.2).collect();
            let into = ui.get_into().to_string();
            let named = picked.iter().any(|g| g.0 == into);
            if into.trim().is_empty() || named {
                // The most used; between equals, one with capitals.
                let best = picked
                    .iter()
                    .max_by_key(|g| (g.1, g.0.chars().any(char::is_uppercase)))
                    .map(|g| g.0.clone());
                ui.set_into(best.unwrap_or_default().into());
            }
        }
        self.refresh_cleanup_ui();
    }

    /// Gives every track with a ticked genre the name typed in.
    fn merge_genres(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let into = app.global::<CleanupUi>().get_into().trim().to_string();
        let picked: HashSet<String> = self
            .cleanup
            .genres
            .iter()
            .filter(|g| g.2)
            .map(|g| g.0.clone())
            .collect();
        if into.is_empty() || picked.is_empty() {
            return;
        }
        let changes: Vec<(usize, Field, String)> = (0..self.library.len())
            .filter(|&i| {
                let t = &self.library[i];
                !is_browsed(t) && picked.contains(t.genre.trim()) && t.genre != into
            })
            .map(|i| (i, Field::Genre, into.clone()))
            .collect();
        let s = self.make_changes(changes, "genre merge");
        self.health_status = format!("Merged {} into \"{into}\". {s}", genre_word(picked.len()));
        self.open_genres();
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Looks online for covers for these tracks (indices into `library`),
    /// one at a time in the background, and embeds the largest found.
    pub(super) fn batch_covers(&mut self, indices: &[usize]) -> String {
        if self.cleanup.covers.is_some() {
            return "Still looking for covers for the last batch.".into();
        }
        let token = settings::discogs_token();
        let jobs: Vec<(usize, Wanted)> = indices
            .iter()
            .map(|&i| &self.library[i])
            .filter(|t| writable(t).is_some_and(|p| !cover_scan::offloaded(p)))
            .map(|t| {
                (
                    t.id,
                    Wanted {
                        path: None,
                        artist: t.artist.clone(),
                        album: t.album.clone(),
                        title: t.title.clone(),
                        discogs_token: token.clone(),
                    },
                )
            })
            .collect();
        if jobs.is_empty() {
            return "None of these files is on this computer.".into();
        }
        let total = jobs.len();
        let (tx, done) = channel();
        std::thread::Builder::new()
            .name("cover-batch".into())
            .spawn(move || {
                for (id, w) in jobs {
                    let found = if w.artist.trim().is_empty() {
                        None
                    } else {
                        coverart::best_cover(&w)
                    };
                    if tx.send((id, found)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn cover batch");
        self.cleanup.covers = Some(CoverBatch {
            done,
            total,
            back: 0,
            found: 0,
        });
        format!("Looking online for covers for {}…", tracks(total))
    }

    /// Looks these tracks (indices into `library`) up online, one at a
    /// time in the background; the review table opens with what was found
    /// once all are back.
    pub(super) fn batch_lookup(&mut self, indices: &[usize]) -> String {
        if self.cleanup.lookups.is_some() {
            return "Still looking up the last batch.".into();
        }
        let (key, token) = (settings::acoustid_key(), settings::discogs_token());
        let mut names = HashMap::new();
        let mut offloaded = 0;
        let mut jobs: Vec<(usize, Query)> = Vec::new();
        for &i in indices {
            let t = &self.library[i];
            if is_browsed(t) {
                continue;
            }
            let Some(QueryFor {
                query,
                name,
                in_icloud,
            }) = query_for(t, &key, &token)
            else {
                continue;
            };
            if let Some(name) = name {
                names.insert(t.id, name);
            }
            if in_icloud && !key.is_empty() {
                offloaded += 1;
            }
            jobs.push((t.id, query));
        }
        if jobs.is_empty() {
            return if key.is_empty() {
                "These tracks have no artist and title to look up by, in their tags or file names. \
                 An AcoustID key in Settings › Collection lets DJ Sugar recognise them by their sound."
                    .into()
            } else {
                "None of these files is on this computer, and they have no artist and title to look up by."
                    .into()
            };
        }
        let total = jobs.len();
        let (tx, done) = channel();
        std::thread::Builder::new()
            .name("lookup-batch".into())
            .spawn(move || {
                for (id, q) in jobs {
                    if tx.send((id, lookup::look_up(&q))).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn lookup batch");
        self.cleanup.lookups = Some(LookupBatch {
            done,
            total,
            found: Vec::new(),
            back: 0,
            not_found: 0,
            unreachable: 0,
            by_sound: 0,
            offloaded,
            names,
        });
        format!("Looking up {} online…", tracks(total))
    }

    /// Opens the review table with the artist and title each of these
    /// tracks' file names give, where their tags have none. Nothing is
    /// written until Apply.
    pub(super) fn review_file_names(&mut self, indices: &[usize]) -> String {
        self.cleanup.review = indices
            .iter()
            .filter(|&&i| !is_browsed(&self.library[i]))
            .flat_map(|&i| {
                let t = &self.library[i];
                let changes = name_from_file(t)
                    .map(|(a, b)| name_proposals(t, &a, &b))
                    .unwrap_or_default();
                changes.into_iter().map(move |change| Pending {
                    index: i,
                    change,
                    on: true,
                    note: "File name".into(),
                })
            })
            .collect();
        self.cleanup.kind = Review::FileNames;
        self.refresh_cleanup_ui();
        if self.cleanup.review.is_empty() {
            "None of these has an \"Artist - Title\" file name for an artist or title it lacks."
                .into()
        } else {
            "Check the names before Apply: a file called \"Title - Artist\" gives them the wrong way round."
                .into()
        }
    }

    /// Takes in what the look-up found since last time, and opens the
    /// review table when the batch is done.
    fn poll_lookups(&mut self) {
        let Some(batch) = &mut self.cleanup.lookups else {
            return;
        };
        let mut news = false;
        for (id, found) in batch.done.try_iter() {
            news = true;
            batch.back += 1;
            match found {
                Ok(f) => {
                    if f.source == "AcoustID" {
                        batch.by_sound += 1;
                    }
                    batch.found.push((id, f));
                }
                Err(Miss::NotFound) => batch.not_found += 1,
                Err(Miss::Unreachable) => batch.unreachable += 1,
            }
        }
        if !news {
            return;
        }
        let (back, total) = (batch.back, batch.total);
        if back < total {
            let found = batch.found.len();
            self.health_status = format!("Looking up: {back} of {total}, {found} found…");
            self.refresh_health_ui();
            return;
        }
        let Some(batch) = self.cleanup.lookups.take() else {
            return;
        };
        let hand = self
            .db
            .as_ref()
            .and_then(|db| db.hand_edited().ok())
            .unwrap_or_default();
        let mut review = Vec::new();
        let found_ids: HashSet<usize> = batch.found.iter().map(|(id, _)| *id).collect();
        for (id, f) in &batch.found {
            let Some(&i) = self.index_of.get(id) else {
                continue;
            };
            let t = &self.library[i];
            let proposed = proposals(t, f, &hand);
            let has = |field: Field| proposed.iter().any(|(c, _)| c.field == field);
            // What the file name says, for a name the match didn't give.
            let from_name: Vec<Change> = batch
                .names
                .get(id)
                .map(|(a, b)| name_proposals(t, a, b))
                .unwrap_or_default()
                .into_iter()
                .filter(|c| !has(c.field) && !hand.contains(&(*id, c.field)))
                .collect();
            review.extend(proposed.into_iter().map(|(change, on)| Pending {
                index: i,
                change,
                on,
                note: format!("{} {:.0}%", f.source, f.score * 100.0),
            }));
            review.extend(from_name.into_iter().map(|change| Pending {
                index: i,
                change,
                on: true,
                note: "File name".into(),
            }));
        }
        // Not found online: the file name is still worth offering.
        for (id, (a, b)) in &batch.names {
            if found_ids.contains(id) {
                continue;
            }
            let Some(&i) = self.index_of.get(id) else {
                continue;
            };
            let t = &self.library[i];
            review.extend(
                name_proposals(t, a, b)
                    .into_iter()
                    .filter(|c| !hand.contains(&(*id, c.field)))
                    .map(|change| Pending {
                        index: i,
                        change,
                        on: true,
                        note: "File name, not found online".into(),
                    }),
            );
        }
        self.health_status = lookup_summary(&batch, review.is_empty());
        self.cleanup.review = review;
        self.cleanup.kind = Review::LookUp;
        self.refresh_cleanup_ui();
        self.refresh_health_ui();
    }

    /// Embeds the covers found since last time, and takes in look-ups.
    pub fn poll_cleanup(&mut self) {
        self.poll_lookups();
        let Some(batch) = &mut self.cleanup.covers else {
            return;
        };
        let back: Vec<(usize, Option<Vec<u8>>)> = batch.done.try_iter().collect();
        if back.is_empty() {
            return;
        }
        batch.back += back.len();
        let mut found = 0;
        for (id, data) in back {
            let (Some(data), Some(&i)) = (data, self.index_of.get(&id)) else {
                continue;
            };
            match self.keep_cover(i, &data) {
                Ok(()) => found += 1,
                Err(e) => eprintln!("{e}"),
            }
        }
        let Some(batch) = &mut self.cleanup.covers else {
            return;
        };
        batch.found += found;
        let (found, back, total) = (batch.found, batch.back, batch.total);
        self.health_status = if back >= total {
            self.cleanup.covers = None;
            let mut s = format!("Found covers for {found} of {}.", tracks(total));
            if found < total {
                s += " Search the rest one by one from the track's info panel.";
            }
            s
        } else {
            format!("Looking for covers: {back} of {total}, {found} found…")
        };
        self.refresh_tree();
        if matches!(
            self.current_shown(),
            Some(super::Contents::Health(Some(Check::NoCover)))
        ) {
            self.refresh_rows();
        } else {
            self.refresh_health_ui();
        }
    }
}

/// What a finished look-up says: how many were found and how, and why
/// the rest weren't.
fn lookup_summary(batch: &LookupBatch, nothing_new: bool) -> String {
    let (found, total) = (batch.found.len(), batch.total);
    let mut s = if found == 0 {
        format!("Nothing found online for {}.", tracks(total))
    } else {
        let mut s = format!("Found {found} of {}", tracks(total));
        if batch.by_sound > 0 {
            s += &format!(" ({} by their sound)", batch.by_sound);
        }
        s + "."
    };
    if batch.not_found > 0 {
        s += &format!(
            " {} aren't on MusicBrainz: often DJ edits, promos and bootlegs.",
            batch.not_found
        );
    }
    if batch.unreachable > 0 {
        s += &format!(
            " {} couldn't be looked up (no connection, or MusicBrainz busy): try them again later.",
            batch.unreachable
        );
    }
    if batch.offloaded > 0 {
        s += &format!(
            " {} are in iCloud only, so they were looked up by name, not by their sound.",
            batch.offloaded
        );
    }
    if settings::acoustid_key().is_empty() && found < total {
        s += " An AcoustID key (Settings › Collection) recognises tracks by their sound.";
    }
    if nothing_new && found > 0 {
        s += " Nothing new for them.";
    } else if !nothing_new {
        s += " Ticked changes only fill empty fields from sure matches; tick others by hand.";
    }
    s
}

/// "1 genre" or "3 genres".
fn genre_word(n: usize) -> String {
    format!("{n} genre{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genres_alike_ignore_case_and_punctuation() {
        assert_eq!(genre_key("Electronica"), "electronica");
        assert_eq!(genre_key("Deep-House"), genre_key("deep house"));
        assert_ne!(genre_key("Electronic"), genre_key("Electronica"));
    }

    #[test]
    fn ticks_only_sure_fills_of_empty_fields() {
        let t = Track {
            id: 7,
            artist: "Bonobo".into(),
            title: "kerala".into(),
            album: String::new(),
            genre: "Electronica".into(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 300.0,
            first_beat: 0.0,
            source: djsugar_engine::Source::Synth { seed: 1 },
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        };
        let f = Found {
            artist: "Bonobo".into(),
            title: "Kerala".into(),
            album: "Migration".into(),
            year: "2017".into(),
            label: "Ninja Tune".into(),
            genre: "Downtempo".into(),
            score: 0.93,
            source: "AcoustID",
            ..Default::default()
        };
        let hand = HashSet::from([(7, Field::Label)]);
        let got: Vec<(Field, bool)> = proposals(&t, &f, &hand)
            .into_iter()
            .map(|(c, on)| (c.field, on))
            .collect();
        // Same title but for capitals: left; label typed by hand: left;
        // genre already set: shown unticked.
        assert_eq!(
            got,
            [
                (Field::Album, true),
                (Field::Year, true),
                (Field::Genre, false)
            ]
        );
        let unsure = Found { score: 0.5, ..f };
        assert!(proposals(&t, &unsure, &HashSet::new())
            .iter()
            .all(|(_, on)| !on));
    }

    fn file_track(path: &str, artist: &str, title: &str) -> Track {
        Track {
            id: 3,
            artist: artist.into(),
            title: title.into(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 300.0,
            first_beat: 0.0,
            source: djsugar_engine::Source::File(path.into()),
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        }
    }

    #[test]
    fn file_names_fill_only_what_says_nothing() {
        let t = file_track("/m/02 - Bonobo - Kerala.mp3", "Unknown Artist", "Kerala");
        let (a, b) = name_from_file(&t).unwrap();
        let got: Vec<(Field, String)> = name_proposals(&t, &a, &b)
            .into_iter()
            .map(|c| (c.field, c.new))
            .collect();
        assert_eq!(got, [(Field::Artist, "Bonobo".to_string())]);
        // Untagged: listed by its file name, as its title.
        let t = file_track("/m/Bonobo - Kerala.mp3", "", "Bonobo - Kerala");
        let (a, b) = name_from_file(&t).unwrap();
        let got: Vec<Field> = name_proposals(&t, &a, &b).iter().map(|c| c.field).collect();
        assert_eq!(got, [Field::Artist, Field::Title]);
        // Good tags: the file name has nothing to add.
        let t = file_track("/m/Bonobo - Kerala.mp3", "Bonobo", "Kerala");
        assert!(name_from_file(&t).is_none());
    }

    #[test]
    fn placeholders_count_as_empty_and_guessed_genres_stay_unticked() {
        let t = file_track("/m/x.mp3", "Unknown Artist", "Kerala");
        let f = Found {
            artist: "Bonobo".into(),
            title: "Kerala".into(),
            genre: "Downtempo".into(),
            genre_guessed: true,
            score: 0.95,
            source: "MusicBrainz",
            ..Default::default()
        };
        let got: Vec<(Field, bool)> = proposals(&t, &f, &HashSet::new())
            .into_iter()
            .map(|(c, on)| (c.field, on))
            .collect();
        assert_eq!(got, [(Field::Artist, true), (Field::Genre, false)]);
    }

    #[test]
    fn info_lookup_offers_sure_replacements_but_not_guesses_or_typed_fields() {
        let mut t = file_track("/m/05 Dope.mp3", "", "05 Dope");
        t.album = "Old Album".into();
        let f = Found {
            artist: "Some Artist".into(),
            title: "Dope".into(),
            album: "New Album".into(),
            genre: "Hip-Hop".into(),
            genre_guessed: true,
            score: 0.9,
            source: "MusicBrainz",
            ..Default::default()
        };
        let hand: HashSet<(usize, Field)> = [(t.id, Field::Album)].into();
        let got: Vec<Field> = info_proposals(&t, Some(&f), None, &hand)
            .iter()
            .map(|c| c.field)
            .collect();
        assert_eq!(got, [Field::Artist, Field::Title]);
        // Not sure enough: only empty fields would be filled, and none are ticked.
        let unsure = Found { score: 0.6, ..f };
        assert!(info_proposals(&t, Some(&unsure), None, &HashSet::new()).is_empty());
        // Nothing found: the file name fills what the tags lack.
        let t = file_track("/m/Bonobo - Kerala.mp3", "", "Bonobo - Kerala");
        let name = name_from_file(&t);
        let got: Vec<Field> = info_proposals(&t, None, name.as_ref(), &HashSet::new())
            .iter()
            .map(|c| c.field)
            .collect();
        assert_eq!(got, [Field::Artist, Field::Title]);
    }
}
