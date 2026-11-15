//! Player mode's artist and album pages (ui/pages.slint): opened from the
//! bar's artist, the search box's artists, and Go to Artist or Go to Album,
//! they show over the track list. An artist's page lists their tracks in
//! the collection, most played first, and their albums; an album's page
//! its tracks in order. Play, Shuffle and Mix put them in Up Next and play
//! them now. Pages stack, so back goes to the page before, then the list.

use std::collections::HashMap;
use std::path::Path;

use djsugar_engine::key::Key;
use djsugar_engine::Track;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::home::{Mix, Tile};
use super::track_menu::main_artist;
use super::{file_path, is_browsed, short_time, Browser};
use crate::automix_order::{self as o, Item};
use crate::settings::show_key;
use crate::{PageAlbum, PageTrack, PageUi};

/// A page: an artist (their name as shown), or an album (its name and
/// its artist's).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Page {
    /// What Player opens on (see `home`).
    Home,
    Artist(String),
    Album(String, String),
    Mix(Mix),
}

/// The pages open, the last showing, and what the one showing holds.
#[derive(Default)]
pub(super) struct Pages {
    stack: Vec<Page>,
    /// The tracks shown, as library indices.
    tracks: Vec<usize>,
    /// An artist's albums: name and tracks (library indices).
    albums: Vec<(String, Vec<usize>)>,
    /// What to play from the page, taken by the Player's search box tick:
    /// track ids in order.
    plays: Vec<Vec<usize>>,
    /// What each tile on Home opens, shelf by shelf.
    pub(super) home: Vec<Vec<Tile>>,
}

/// The artists a track's artist field names: "Bicep, Hammer" is Bicep and
/// Hammer; "Bicep feat. X" is Bicep and X. Lower case.
pub(super) fn artists_of(artist: &str) -> Vec<String> {
    let mut lower = artist.to_lowercase();
    for sep in [
        " feat. ", " feat ", " ft. ", " ft ", " x ", " vs. ", " vs ", "; ", " / ",
    ] {
        lower = lower.replace(sep, ", ");
    }
    lower
        .split(", ")
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(String::from)
        .collect()
}

/// "48 min", or "1 h 12 min".
fn total_time(seconds: f64) -> String {
    let m = (seconds / 60.0).round() as u64;
    if m >= 60 {
        format!("{} h {} min", m / 60, m % 60)
    } else {
        format!("{m} min")
    }
}

pub(super) fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

pub(super) fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// The year in a year tag ("2024-03-01" is 2024).
pub(super) fn year(tag: &str) -> Option<u32> {
    tag.trim()
        .get(..4)
        .and_then(|y| y.parse().ok())
        .filter(|&y| y > 0)
}

impl Browser {
    pub(super) fn install_pages(
        ui: &PageUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        ui.on_back(move || w(&|b| b.page_back()));
        let w = with.clone();
        ui.on_play(move |mode| w(&|b| b.play_page(mode)));
        let w = with.clone();
        ui.on_play_track(move |i| w(&|b| b.play_page_track(i.max(0) as usize)));
        let w = with.clone();
        ui.on_open_album(move |i| {
            w(&|b| {
                let Some(Page::Artist(artist)) = b.pages.stack.last().cloned() else {
                    return;
                };
                if let Some((name, _)) = b.pages.albums.get(i.max(0) as usize).cloned() {
                    b.open_page(Page::Album(name, artist));
                }
            })
        });
        let w = with.clone();
        ui.on_open_artist(move || {
            w(&|b| {
                if let Some(Page::Album(_, artist)) = b.pages.stack.last().cloned() {
                    b.open_page(Page::Artist(artist));
                }
            })
        });
        let w = with.clone();
        ui.on_open_deck_artist(move |deck| {
            w(&|b| {
                let id = b.loaded.get(deck.max(0) as usize).copied().flatten();
                let artist = id
                    .and_then(|id| b.track_by_id(id))
                    .map(|t| main_artist(&t.artist).to_string());
                if let Some(a) = artist.filter(|a| !a.is_empty()) {
                    b.open_page(Page::Artist(a));
                }
            })
        });
        let w = with.clone();
        ui.on_open_tile(move |s, t| w(&|b| b.open_tile(s.max(0) as usize, t.max(0) as usize)));
        let w = with.clone();
        ui.on_go_home(move || w(&|b| b.go_home()));
        let w = with.clone();
        ui.on_open_artist_named(move |name| {
            let name = name.to_string();
            w(&|b| b.open_page(Page::Artist(name.clone())));
        });
    }

    /// Whether the app is in Player mode, where pages show.
    pub(super) fn in_player(&self) -> bool {
        self.keep_going_player()
    }

    /// Opens a page over the list (on top of the one showing).
    pub(super) fn open_page(&mut self, page: Page) {
        if self.pages.stack.last() == Some(&page) {
            return;
        }
        self.pages.stack.push(page);
        self.refresh_page();
    }

    /// Home, with nothing behind it.
    pub(super) fn show_home(&mut self) {
        self.pages.stack = vec![Page::Home];
        self.refresh_page();
    }

    /// Whether a page shows.
    pub(super) fn pages_open(&self) -> bool {
        !self.pages.stack.is_empty()
    }

    /// Back to the page before, or the list.
    fn page_back(&mut self) {
        self.pages.stack.pop();
        self.refresh_page();
    }

    /// Closes every page: the list shows again.
    pub(super) fn close_page(&mut self) {
        if !self.pages.stack.is_empty() {
            self.pages.stack.clear();
            self.refresh_page();
        }
    }

    /// The collection's tracks by `artist` (any of their artists).
    fn artist_tracks(&self, artist: &str) -> Vec<usize> {
        let key = artist.to_lowercase();
        (0..self.library.len())
            .filter(|&i| {
                let t = &self.library[i];
                !is_browsed(t) && artists_of(&t.artist).contains(&key)
            })
            .collect()
    }

    /// The album's tracks: those named for it in the folders where the
    /// artist's tracks of it are (so a compilation shows whole), else the
    /// artist's tracks of it; in disc and track order.
    fn album_tracks_of(&self, album: &str, artist: &str) -> Vec<usize> {
        let name = album.trim().to_lowercase();
        let same = |t: &Track| t.album.trim().to_lowercase() == name;
        let theirs: Vec<usize> = self
            .artist_tracks(artist)
            .into_iter()
            .filter(|&i| same(&self.library[i]))
            .collect();
        let dirs: Vec<&Path> = theirs
            .iter()
            .filter_map(|&i| file_path(&self.library[i]).and_then(Path::parent))
            .collect();
        let mut tracks: Vec<usize> = if dirs.is_empty() {
            theirs
        } else {
            (0..self.library.len())
                .filter(|&i| {
                    let t = &self.library[i];
                    !is_browsed(t)
                        && same(t)
                        && file_path(t)
                            .and_then(Path::parent)
                            .is_some_and(|d| dirs.contains(&d))
                })
                .collect()
        };
        tracks.sort_by_cached_key(|&i| {
            let t = &self.library[i];
            (
                t.info.disc_number.max(1),
                t.info.track_number,
                t.title.to_lowercase(),
            )
        });
        tracks
    }

    /// Fills the page showing (or none).
    pub(super) fn refresh_page(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<PageUi>();
        let Some(page) = self.pages.stack.last().cloned() else {
            self.pages.tracks.clear();
            self.pages.albums.clear();
            self.pages.home.clear();
            ui.set_kind(0);
            ui.set_shelves(ModelRc::default());
            return;
        };
        ui.set_can_back(self.pages.stack.len() > 1);
        if page == Page::Home {
            self.pages.tracks.clear();
            self.pages.albums.clear();
            ui.set_kind(3);
            ui.set_tint(super::home::tint(""));
            ui.set_tracks(ModelRc::default());
            ui.set_albums(ModelRc::default());
            self.refresh_home();
            return;
        }
        self.pages.home.clear();
        ui.set_shelves(ModelRc::default());
        let mix = match &page {
            Page::Mix(m) => Some(self.mix_name(m)),
            _ => None,
        };
        let (kind, title, artist) = match &page {
            Page::Artist(a) => (1, a.clone(), String::new()),
            Page::Album(album, a) => (2, album.clone(), a.clone()),
            _ => (4, mix.clone().unwrap_or_default().0, String::new()),
        };
        let tracks = match &page {
            Page::Artist(a) => {
                let mut t = self.artist_tracks(a);
                t.sort_by_cached_key(|&i| {
                    let t = &self.library[i];
                    (
                        std::cmp::Reverse(t.info.play_count),
                        std::cmp::Reverse(t.rating),
                        t.title.to_lowercase(),
                    )
                });
                t
            }
            Page::Album(album, a) => self.album_tracks_of(album, a),
            Page::Mix(m) => self.mix_tracks(m),
            Page::Home => Vec::new(),
        };
        let albums: Vec<(String, Vec<usize>)> = match &page {
            Page::Artist(_) => {
                let mut by: HashMap<String, (String, Vec<usize>)> = HashMap::new();
                for &i in &tracks {
                    let name = self.library[i].album.trim();
                    if name.is_empty() {
                        continue;
                    }
                    by.entry(name.to_lowercase())
                        .or_insert_with(|| (name.to_string(), Vec::new()))
                        .1
                        .push(i);
                }
                let mut albums: Vec<_> = by.into_values().collect();
                albums.sort_by_cached_key(|(name, ids)| {
                    let y = ids
                        .iter()
                        .filter_map(|&i| year(&self.library[i].info.year))
                        .max()
                        .unwrap_or(0);
                    (std::cmp::Reverse(y), name.to_lowercase())
                });
                albums
            }
            _ => Vec::new(),
        };
        let seconds: f64 = tracks.iter().map(|&i| self.library[i].duration).sum();
        let detail = match &page {
            Page::Artist(_) => {
                let mut parts = vec![plural(tracks.len(), "track")];
                if !albums.is_empty() {
                    parts.push(plural(albums.len(), "album"));
                }
                parts.push(total_time(seconds));
                parts.join(" · ")
            }
            Page::Album(..) => {
                let y = tracks
                    .iter()
                    .filter_map(|&i| year(&self.library[i].info.year))
                    .max();
                let mut parts: Vec<String> = y.map(|y| y.to_string()).into_iter().collect();
                parts.push(plural(tracks.len(), "track"));
                parts.push(total_time(seconds));
                parts.join(" · ")
            }
            _ => {
                let what = mix.as_ref().map_or("Mix", |m| m.2);
                format!(
                    "{what} · {} · {}",
                    plural(tracks.len(), "track"),
                    total_time(seconds)
                )
            }
        };
        // A mix shows four of its tracks' covers, most played first.
        let covers = if kind == 4 {
            let mut ids = tracks.clone();
            ids.sort_by_key(|&i| std::cmp::Reverse(self.library[i].info.play_count));
            self.covers(&ids)
        } else {
            Vec::new()
        };
        let cover = if kind == 4 {
            covers.first().cloned()
        } else {
            tracks
                .iter()
                .find_map(|&i| self.album_art.get(&self.library[i]))
        };
        ui.set_covers(ModelRc::new(VecModel::from(covers)));
        ui.set_kind(kind);
        ui.set_title(title.as_str().into());
        ui.set_artist(artist.into());
        ui.set_detail(detail.into());
        match &mix {
            Some((name, label, _)) => {
                ui.set_initials(label.as_str().into());
                ui.set_tint(super::home::tint(name));
            }
            None => {
                ui.set_initials(initials(&title).into());
                ui.set_tint(super::home::tint(""));
            }
        }
        ui.set_has_cover(cover.is_some());
        ui.set_cover(cover.unwrap_or_default());
        let several_discs = tracks.iter().any(|&i| self.library[i].info.disc_number > 1);
        let rows: Vec<PageTrack> = tracks
            .iter()
            .map(|&i| {
                let t = &self.library[i];
                let cover = if kind != 2 { self.art.get(t) } else { None };
                let number = match (kind, t.info.track_number) {
                    (2, n) if n > 0 && several_discs => {
                        format!("{}-{n}", t.info.disc_number.max(1))
                    }
                    (2, n) if n > 0 => n.to_string(),
                    _ => String::new(),
                };
                PageTrack {
                    title: t.title.clone().into(),
                    artist: t.artist.clone().into(),
                    length: short_time(t.duration),
                    bpm: if t.bpm > 0.0 {
                        format!("{:.0}", t.bpm).into()
                    } else {
                        SharedString::new()
                    },
                    key: show_key(&t.key, self.camelot).into(),
                    key_match: self.mixes_with_deck(&t.key),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                    number: number.into(),
                }
            })
            .collect();
        ui.set_tracks(ModelRc::new(VecModel::from(rows)));
        let tiles: Vec<PageAlbum> = albums
            .iter()
            .map(|(name, ids)| {
                let cover = ids
                    .iter()
                    .find_map(|&i| self.album_art.get(&self.library[i]));
                let y = ids
                    .iter()
                    .filter_map(|&i| year(&self.library[i].info.year))
                    .max();
                let count = plural(ids.len(), "track");
                PageAlbum {
                    name: name.as_str().into(),
                    detail: y.map_or(count.clone(), |y| format!("{y} · {count}")).into(),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                }
            })
            .collect();
        ui.set_albums(ModelRc::new(VecModel::from(tiles)));
        self.pages.tracks = tracks;
        self.pages.albums = albums;
    }

    /// Play (0), Shuffle (1) or Mix (2) the page's tracks: an album in its
    /// order, an artist's most played first.
    fn play_page(&mut self, mode: i32) {
        let mut ids: Vec<usize> = self.pages.tracks.clone();
        match mode {
            1 => {
                let mut random = crate::keep_going::random();
                for i in (1..ids.len()).rev() {
                    let j = (random() % (i as u64 + 1)) as usize;
                    ids.swap(i, j);
                }
            }
            2 => {
                let items: Vec<Item> = ids
                    .iter()
                    .map(|&i| {
                        let t = &self.library[i];
                        Item {
                            bpm: t.bpm,
                            key: Key::parse(&t.key),
                        }
                    })
                    .collect();
                let mut random = crate::keep_going::random();
                let picked = o::order(&items, 0, o::Order::Flow, self.key_rules.0, &mut random);
                ids = picked.into_iter().map(|p| ids[p]).collect();
            }
            _ => {}
        }
        self.queue_page_play(ids);
    }

    /// Plays from the page's track `index`: an album's on to its end, an
    /// artist's alone.
    fn play_page_track(&mut self, index: usize) {
        let ids: Vec<usize> = match self.pages.stack.last() {
            Some(Page::Album(..) | Page::Mix(_)) => {
                self.pages.tracks.iter().skip(index).copied().collect()
            }
            _ => self.pages.tracks.get(index).copied().into_iter().collect(),
        };
        self.queue_page_play(ids);
    }

    fn queue_page_play(&mut self, indices: Vec<usize>) {
        if indices.is_empty() {
            return;
        }
        self.adopt(&indices);
        let ids = indices.iter().map(|&i| self.library[i].id).collect();
        self.pages.plays.push(ids);
    }

    /// What the pages asked to play, as track ids in order: each plays now
    /// with the rest after it.
    pub fn take_page_plays(&mut self) -> Vec<Vec<usize>> {
        std::mem::take(&mut self.pages.plays)
    }

    /// The Player search box's track: opens its album's page and plays
    /// from it to the album's end, as streaming players do. A track with
    /// no album (or not on its album's page) plays alone.
    pub fn play_from_album(&mut self, id: usize) {
        let Some(&index) = self.index_of.get(&id) else {
            return;
        };
        let t = self.library[index].clone();
        if self.go_to_page(&t, true) {
            if let Some(at) = self.pages.tracks.iter().position(|&i| i == index) {
                self.play_page_track(at);
                return;
            }
        }
        self.queue_page_play(vec![index]);
    }

    /// The Player search box's album: its page, by name and artist.
    pub fn open_album_named(&mut self, album: &str, artist: &str) {
        if self.in_player() && !album.trim().is_empty() {
            self.open_page(Page::Album(
                album.trim().to_string(),
                main_artist(artist).to_string(),
            ));
        }
    }

    /// Whether Go to Artist / Go to Album should open a page (Player).
    pub(super) fn go_to_page(&mut self, t: &Track, album: bool) -> bool {
        if !self.in_player() {
            return false;
        }
        let artist = main_artist(&t.artist).to_string();
        let page = if album {
            if t.album.trim().is_empty() {
                return false;
            }
            Page::Album(t.album.trim().to_string(), artist)
        } else {
            if artist.is_empty() {
                return false;
            }
            Page::Artist(artist)
        };
        self.open_page(page);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artists_of_a_credit() {
        assert_eq!(artists_of("Bicep, Hammer"), ["bicep", "hammer"]);
        assert_eq!(
            artists_of("Bicep feat. Clara La San"),
            ["bicep", "clara la san"]
        );
        assert_eq!(artists_of("Bicep"), ["bicep"]);
        assert!(artists_of("").is_empty());
    }

    #[test]
    fn times_and_years() {
        assert_eq!(total_time(48.0 * 60.0), "48 min");
        assert_eq!(total_time(72.0 * 60.0), "1 h 12 min");
        assert_eq!(year("2024-03-01"), Some(2024));
        assert_eq!(year(""), None);
    }
}
