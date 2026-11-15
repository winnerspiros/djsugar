//! The Player layout's search box in the top bar: the best matches from the
//! collection in a dropdown, as music players show them: artists and albums
//! first (unless a track's title is the better match), then tracks, in a
//! list that scrolls. As in streaming players, picking a track opens its
//! album and plays from it on; an album or artist opens its page. Play next
//! and Add to queue are on a row's right-click menu (and ⌘↵, ⇧↵).
//! The queue is the Automix list; playing a track now has Automix mix into
//! it as soon as it is waiting on the other deck.
//! What is typed also filters the track list, so Player needs no second
//! search box over the list.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_engine::{Track, NUM_DECKS};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::automix::AutoMix;
use crate::browser::Browser;
use crate::remote::held_count;
use crate::settings::show_key;
use crate::{AppWindow, QuickRow, QuickSearch, StoreUi, TrackDrag};

/// At most this many tracks go in the dropdown (it scrolls).
const TRACKS: usize = 200;
/// At most this many artists, and this many albums, go first.
const ARTISTS: usize = 2;
const ALBUMS: usize = 3;
/// How many matches are ranked to pick those from.
const RANKED: usize = 2000;
/// A row's `kind`.
const TRACK: i32 = 0;
const ARTIST: i32 = 1;
const ALBUM: i32 = 2;
/// How long Play now waits for Automix to have the track ready.
const PLAY_NOW_WAIT: Duration = Duration::from_secs(20);

/// What a press asks for (QuickSearch.act's `mode` 1 and 2; 0 opens).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayNext,
    AddToQueue,
    PlayNow,
}

impl Action {
    /// None for 0: open the row (its album, or its page).
    fn from_mode(mode: i32) -> Option<Self> {
        match mode {
            1 => Some(Action::AddToQueue),
            2 => Some(Action::PlayNext),
            _ => None,
        }
    }
}

/// An artist or album row: what its buttons act on.
#[derive(Clone, Debug, PartialEq)]
enum Group {
    /// The artist's name, to search for.
    Artist(String),
    /// The album's name, its artist and track ids, in album order.
    Album(String, String, Vec<usize>),
}

/// An artist or album row's id: below -1, so it can't be a track's.
fn group_id(index: usize) -> i32 {
    -(index as i32) - 2
}

pub struct Quick {
    /// Presses from the dropdown, carried out on the next tick, when the
    /// decks' tracks and Automix are at hand.
    jobs: Rc<RefCell<Vec<(usize, Action)>>>,
    /// The track Play now waits to mix into, and since when.
    play_now: Option<(usize, Instant)>,
}

impl Quick {
    pub fn install(app: &AppWindow, browser: &Rc<RefCell<Browser>>) -> Self {
        let jobs: Rc<RefCell<Vec<(usize, Action)>>> = Rc::default();
        let groups: Rc<RefCell<Vec<Group>>> = Rc::default();
        let g = app.global::<QuickSearch>();
        let b = browser.clone();
        let weak = app.as_weak();
        let gs = groups.clone();
        g.on_search(move |text| {
            let groups = &gs;
            let Some(app) = weak.upgrade() else {
                return;
            };
            // Player has this one search box: it fills the dropdown and
            // filters the track list too, as the list's own box does.
            app.global::<StoreUi>().set_search_text(text.clone());
            let typed = text.trim();
            {
                let Ok(b) = b.try_borrow() else {
                    return;
                };
                let g = app.global::<QuickSearch>();
                if typed.is_empty() {
                    g.set_rows(ModelRc::default());
                    g.set_total(0);
                } else {
                    let (mut found, total) = b.find_tracks(typed, 0, RANKED);
                    rank(&mut found, typed, |t| t.title.as_str());
                    let (rows, found_groups) = rows_for(&b, &found, typed);
                    *groups.borrow_mut() = found_groups;
                    g.set_rows(ModelRc::new(VecModel::from(rows)));
                    g.set_total(total as i32);
                }
            }
            // After the borrow ends: the list's search borrows the browser.
            app.invoke_search(text);
        });
        let j = jobs.clone();
        let weak = app.as_weak();
        let b = browser.clone();
        g.on_act(move |id, mode| {
            let action = Action::from_mode(mode);
            let close = || {
                if let Some(app) = weak.upgrade() {
                    app.global::<QuickSearch>().set_open(false);
                }
            };
            if id >= 0 {
                match action {
                    Some(a) => j.borrow_mut().push((id as usize, a)),
                    // A track: its album's page, playing from it on.
                    None => {
                        if let Ok(mut b) = b.try_borrow_mut() {
                            b.play_from_album(id as usize);
                        }
                        close();
                    }
                }
                return;
            }
            let index = (-id - 2) as usize;
            let group = groups.borrow().get(index).cloned();
            match group {
                // An artist: their page, and their tracks in the list.
                Some(Group::Artist(name)) => {
                    if let Some(app) = weak.upgrade() {
                        // The artist's page over the list.
                        app.global::<crate::PageUi>()
                            .invoke_open_artist_named(name.as_str().into());
                        let g = app.global::<QuickSearch>();
                        g.set_text(name.as_str().into());
                        g.set_current(0);
                        g.invoke_search(name.into());
                        g.set_open(false);
                    }
                }
                Some(Group::Album(name, by, ids)) => match action {
                    Some(a) => j.borrow_mut().extend(album_jobs(&ids, a)),
                    None => {
                        if let Ok(mut b) = b.try_borrow_mut() {
                            b.open_album_named(&name, &by);
                        }
                        close();
                    }
                },
                None => {}
            }
        });
        // Tracks dropped on the Player bar: the first plays now and the
        // rest follow it, in order. Each Play next goes in at the same place,
        // so the rest go in last first, ahead of the one playing now.
        let j = jobs.clone();
        let b = browser.clone();
        app.global::<TrackDrag>().on_drop_player(move |data| {
            let Ok(mut b) = b.try_borrow_mut() else {
                return;
            };
            let ids = b.dropped_tracks(&data);
            let Some((&first, rest)) = ids.split_first() else {
                return;
            };
            let mut j = j.borrow_mut();
            j.extend(rest.iter().rev().map(|&id| (id, Action::PlayNext)));
            j.push((first, Action::PlayNow));
        });
        Quick {
            jobs,
            play_now: None,
        }
    }

    /// Carries out the dropdown's presses, and Play now's mix once its
    /// track waits on the other deck.
    pub fn tick(
        &mut self,
        app: &AppWindow,
        automix: &mut AutoMix,
        browser: &mut Browser,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) {
        // Play, Shuffle and Mix on an artist's or album's page.
        for ids in browser.take_page_plays() {
            self.jobs
                .borrow_mut()
                .extend(album_jobs(&ids, Action::PlayNow));
        }
        let jobs = std::mem::take(&mut *self.jobs.borrow_mut());
        for (id, action) in jobs {
            let at = match action {
                Action::AddToQueue => None,
                Action::PlayNext | Action::PlayNow => Some(held_count(automix, browser, tracks)),
            };
            let title = browser.track_by_id(id).map(|t| t.title.clone());
            if !browser.add_to_automix_at(id, at) {
                continue;
            }
            let title = title.unwrap_or_default();
            let note = match action {
                Action::PlayNext => format!("{title} plays next"),
                Action::AddToQueue => format!("{title} added to Up Next"),
                Action::PlayNow => format!("Mixing into {title}…"),
            };
            app.global::<QuickSearch>().set_note(note.into());
            if action == Action::PlayNow {
                self.play_now = Some((id, Instant::now()));
                if !automix.is_on() {
                    automix.set_enabled(true);
                }
            }
        }
        if let Some((id, since)) = self.play_now {
            let (playing, waiting) = automix.held();
            if playing == Some(id) || since.elapsed() > PLAY_NOW_WAIT {
                self.play_now = None;
            } else if waiting == Some(id) && automix.seconds_to_mix(tracks).is_some() {
                automix.mix_now();
                self.play_now = None;
            }
        }
    }
}

/// The jobs for an album's buttons: Add to queue puts its tracks at the
/// end in order; Play next and Play now put them next, in order (each Play
/// next goes in at the same place, so last first), and Play now mixes into
/// the first.
fn album_jobs(ids: &[usize], action: Action) -> Vec<(usize, Action)> {
    match action {
        Action::AddToQueue => ids.iter().map(|&id| (id, Action::AddToQueue)).collect(),
        Action::PlayNext => ids.iter().rev().map(|&id| (id, Action::PlayNext)).collect(),
        Action::PlayNow => {
            let Some((&first, rest)) = ids.split_first() else {
                return Vec::new();
            };
            let mut jobs: Vec<_> = rest
                .iter()
                .rev()
                .map(|&id| (id, Action::PlayNext))
                .collect();
            jobs.push((first, Action::PlayNow));
            jobs
        }
    }
}

/// An artist found, with their tracks among the matches.
type Named<'a> = (String, Vec<&'a Track>);
/// An album found: its name, artist and tracks.
type AlbumFound<'a> = (String, String, Vec<&'a Track>);

/// Whether `name` has every typed word.
fn has_words(name: &str, words: &[String]) -> bool {
    let name = name.to_lowercase();
    !words.is_empty() && words.iter().all(|w| name.contains(w.as_str()))
}

/// The artists and albums among `found` whose name has every typed word,
/// best first: names that start with the first word, then those with more
/// tracks. An album is listed with its artist and its tracks in album
/// order (one album per artist, so a compilation splits by artist).
fn artists_and_albums<'a>(
    found: &'a [Arc<Track>],
    text: &str,
) -> (Vec<Named<'a>>, Vec<AlbumFound<'a>>) {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    let first = words.first().cloned().unwrap_or_default();
    let mut artists: Vec<Named<'a>> = Vec::new();
    let mut albums: Vec<AlbumFound<'a>> = Vec::new();
    for t in found {
        let artist = t.artist.trim();
        if has_words(artist, &words) {
            match artists
                .iter_mut()
                .find(|(a, _)| a.eq_ignore_ascii_case(artist))
            {
                Some((_, ts)) => ts.push(t),
                None => artists.push((artist.to_string(), vec![t])),
            }
        }
        let album = t.album.trim();
        let by = artist;
        if !album.is_empty() && (has_words(album, &words) || has_words(by, &words)) {
            match albums
                .iter_mut()
                .find(|(a, b, _)| a.eq_ignore_ascii_case(album) && b.eq_ignore_ascii_case(by))
            {
                Some((_, _, ts)) => ts.push(t),
                None => albums.push((album.to_string(), by.to_string(), vec![t])),
            }
        }
    }
    let starts = |name: &str| name.to_lowercase().starts_with(first.as_str());
    artists.sort_by_key(|(a, ts)| (!starts(a), std::cmp::Reverse(ts.len())));
    // Albums named by what was typed before those found by their artist.
    albums.sort_by_key(|(a, _, ts)| {
        (
            !has_words(a, &words),
            !starts(a),
            std::cmp::Reverse(ts.len()),
        )
    });
    for (_, _, ts) in &mut albums {
        ts.sort_by_key(|t| (t.info.disc_number, t.info.track_number));
    }
    (artists, albums)
}

/// The dropdown's rows for `found` (tracks ranked by title), and what the
/// artist and album rows act on.
fn rows_for(b: &Browser, found: &[Arc<Track>], text: &str) -> (Vec<QuickRow>, Vec<Group>) {
    let (artists, albums) = artists_and_albums(found, text);
    let mut groups = Vec::new();
    let mut lead = Vec::new();
    for (name, ts) in artists.into_iter().take(ARTISTS) {
        let count = ts.len();
        lead.push(QuickRow {
            id: group_id(groups.len()),
            kind: ARTIST,
            title: name.as_str().into(),
            artist: "Artist".into(),
            detail: format!("{count} track{}", if count == 1 { "" } else { "s" }).into(),
            initials: initials(&name).into(),
            ..Default::default()
        });
        groups.push(Group::Artist(name));
    }
    for (name, by, ts) in albums.into_iter().take(ALBUMS) {
        let cover = ts.iter().find_map(|t| b.cover_of(t));
        let count = ts.len();
        lead.push(QuickRow {
            id: group_id(groups.len()),
            kind: ALBUM,
            title: name.as_str().into(),
            artist: format!("Album · {by}").into(),
            detail: format!("{count} track{}", if count == 1 { "" } else { "s" }).into(),
            has_cover: cover.is_some(),
            cover: cover.unwrap_or_default(),
            initials: initials(&by).into(),
        });
        groups.push(Group::Album(name, by, ts.iter().map(|t| t.id).collect()));
    }
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    // A track's title is the best match when it has every typed word and
    // no artist or album row's name does: tracks go first then.
    let title_best = found.first().is_some_and(|t| has_words(&t.title, &words))
        && !lead.iter().any(|r| has_words(&r.title, &words));
    let tracks: Vec<QuickRow> = found.iter().take(TRACKS).map(|t| row(b, t)).collect();
    let rows = if title_best {
        tracks.into_iter().chain(lead).collect()
    } else {
        lead.into_iter().chain(tracks).collect()
    };
    (rows, groups)
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// Puts tracks whose title has every typed word first, then those whose
/// title starts with the first word ahead of the rest of those, keeping the
/// collection's title order otherwise.
fn rank<T>(found: &mut [T], text: &str, title_of: impl Fn(&T) -> &str) {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    found.sort_by_cached_key(|t| {
        let title = title_of(t).to_lowercase();
        let all = words.iter().all(|w| title.contains(w.as_str()));
        let starts = words.first().is_some_and(|w| title.starts_with(w.as_str()));
        (!all, !starts)
    });
}

/// A dropdown row for `t`.
fn row(b: &Browser, t: &Track) -> QuickRow {
    let cover = b.cover_of(t);
    let key = show_key(&t.key, b.camelot());
    let mut detail = Vec::new();
    if t.bpm > 0.0 {
        detail.push(format!("{:.1}", t.bpm));
    }
    if !key.is_empty() {
        detail.push(key);
    }
    let s = t.duration.max(0.0).round() as u64;
    detail.push(format!("{}:{:02}", s / 60, s % 60));
    QuickRow {
        id: t.id as i32,
        kind: TRACK,
        title: t.title.as_str().into(),
        artist: t.artist.as_str().into(),
        detail: SharedString::from(detail.join(" · ")),
        has_cover: cover.is_some(),
        cover: cover.unwrap_or_default(),
        initials: initials(&t.artist).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{album_jobs, rank, Action};

    #[test]
    fn titles_first() {
        // Afterglow is by Lumen Drift: found by its artist only.
        let mut found = vec!["Afterglow", "Continental Drift", "Drift Theory"];
        rank(&mut found, "drift", |t| t);
        assert_eq!(found, ["Drift Theory", "Continental Drift", "Afterglow"]);
    }

    #[test]
    fn album_buttons_keep_album_order() {
        use Action::*;
        assert_eq!(
            album_jobs(&[1, 2, 3], AddToQueue),
            [(1, AddToQueue), (2, AddToQueue), (3, AddToQueue)]
        );
        // Each Play next goes in at the same place: last first.
        assert_eq!(
            album_jobs(&[1, 2, 3], PlayNext),
            [(3, PlayNext), (2, PlayNext), (1, PlayNext)]
        );
        assert_eq!(
            album_jobs(&[1, 2, 3], PlayNow),
            [(3, PlayNext), (2, PlayNext), (1, PlayNow)]
        );
        assert!(album_jobs(&[], PlayNow).is_empty());
    }

    #[test]
    fn modes() {
        // 0 opens the row: a track's album, playing from it.
        assert_eq!(Action::from_mode(0), None);
        assert_eq!(Action::from_mode(1), Some(Action::AddToQueue));
        assert_eq!(Action::from_mode(2), Some(Action::PlayNext));
        assert_eq!(Action::from_mode(9), None);
    }
}
