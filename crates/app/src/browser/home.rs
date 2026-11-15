//! Player mode's Home (ui/pages.slint): what Player opens on, with the
//! collection's recently played and recently added albums, the DJ's
//! crates, and mixes made from the collection by genre and decade. Crates
//! and mixes show four of their tracks' covers, with their name on a bar.
//! A tile opens its album, artist or mix page, or lists its crate.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use djsugar_engine::key::Key;
use djsugar_library::sets::is_imported;
use djsugar_library::PlaylistKind;
use slint::{Color, ComponentHandle, Image, ModelRc, VecModel};

use super::pages::{year, Page};
use super::track_menu::main_artist;
use super::{file_path, is_browsed, Browser};
use crate::art::Art;
use crate::{HomeShelf, HomeTile, PageUi};

/// Tiles on a shelf (the view shows as many as fit across).
const SHELF: usize = 12;
/// The fewest tracks a mix is made from.
const MIX_MIN: usize = 8;
/// The most tracks in a mix.
const MIX_TRACKS: usize = 50;
/// How many tracks are looked at for a crate's or mix's covers.
const COVER_LOOK: usize = 40;

/// A mix made from the collection: tracks of a genre (lower case) or of a
/// decade (1990).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Mix {
    Genre(String),
    Decade(u32),
}

/// What a tile on Home opens.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tile {
    Album(String, String),
    Artist(String),
    List(i64),
    Mix(Mix),
}

/// The genres a genre tag names: "House; Deep House" is both.
fn genres_of(tag: &str) -> impl Iterator<Item = &str> {
    tag.split([',', ';', '/'])
        .map(str::trim)
        .filter(|g| !g.is_empty())
}

/// A mix's colour, the same each time for the same name.
pub(super) fn tint(name: &str) -> Color {
    const TINTS: [u32; 6] = [0x5b3fd1, 0x1f7f86, 0xb4582a, 0xa8336a, 0x2f5fd0, 0x3f7f3c];
    let h = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let rgb = TINTS[h as usize % TINTS.len()];
    Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// A crate's or mix's tile art: its covers, and its name on a bar over
/// them (without covers, its label shows big on its colour instead).
pub(super) fn with_covers(covers: Vec<Image>) -> HomeTile {
    HomeTile {
        has_cover: !covers.is_empty(),
        cover: covers.first().cloned().unwrap_or_default(),
        bar: !covers.is_empty(),
        covers: ModelRc::new(VecModel::from(covers)),
        ..Default::default()
    }
}

/// "Good morning" and so on, by the hour.
fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}

/// Random numbers that come out the same each time, so a mix's order
/// stays put while its page is redrawn.
fn steady() -> impl FnMut() -> u64 {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

impl Browser {
    /// Whether a track is the DJ's own and on disk, for Home.
    fn home_track(&self, i: usize) -> bool {
        let t = &self.library[i];
        !is_browsed(t) && !t.info.missing
    }

    /// Up to four covers of different albums among `tracks`, for a crate's
    /// or mix's art: four make a 2x2 grid, fewer show the first alone.
    pub(super) fn covers(&self, tracks: &[usize]) -> Vec<Image> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for &i in tracks.iter().take(COVER_LOOK) {
            let t = &self.library[i];
            let Some(key) = Art::key(t) else { continue };
            if seen.contains(&key) {
                continue;
            }
            if let Some(image) = self.album_art.get(t) {
                seen.insert(key);
                out.push(image);
                if out.len() == 4 {
                    break;
                }
            }
        }
        out
    }

    /// Albums (or artists, for tracks without an album) in the order of
    /// `order`, each once.
    fn release_tiles(&self, order: &[usize]) -> Vec<(Tile, HomeTile)> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for &i in order {
            let t = &self.library[i];
            let artist = main_artist(&t.artist).to_string();
            let album = t.album.trim();
            let (tile, key) = if !album.is_empty() {
                let dir = file_path(t).and_then(Path::parent).map(Path::to_path_buf);
                (
                    Tile::Album(album.to_string(), artist.clone()),
                    format!("{}\n{:?}", album.to_lowercase(), dir),
                )
            } else if !artist.is_empty() {
                (Tile::Artist(artist.clone()), artist.to_lowercase())
            } else {
                continue;
            };
            if !seen.insert(key) {
                continue;
            }
            let cover = self.album_art.get(t);
            let view = match &tile {
                Tile::Album(name, _) => HomeTile {
                    name: name.as_str().into(),
                    detail: artist.into(),
                    ..Default::default()
                },
                _ => HomeTile {
                    name: artist.into(),
                    detail: "Artist".into(),
                    round: true,
                    ..Default::default()
                },
            };
            out.push((
                tile,
                HomeTile {
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                    tint: tint(&view.name),
                    label: super::pages::initials(&view.name).into(),
                    ..view
                },
            ));
            if out.len() == SHELF {
                break;
            }
        }
        out
    }

    /// The DJ's crates and lists, most recently added to first.
    fn list_tiles(&self) -> Vec<(Tile, HomeTile)> {
        let Some(db) = &self.db else {
            return Vec::new();
        };
        let inbox = db.inbox_list_id();
        let touched = db.playlists_touched().unwrap_or_default();
        let mut lists: Vec<_> = db
            .playlists()
            .unwrap_or_default()
            .into_iter()
            .filter(|p| {
                matches!(p.kind, PlaylistKind::Crate | PlaylistKind::List)
                    && !is_imported(p)
                    && Some(p.id) != inbox
            })
            .collect();
        lists.sort_by_key(|p| Reverse(touched.get(&p.id).copied().unwrap_or(0)));
        let mut out = Vec::new();
        for p in lists {
            let ids: Vec<usize> = db
                .playlist_tracks(p.id)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|id| self.index_of.get(&id).copied())
                .collect();
            if ids.is_empty() {
                continue;
            }
            let covers = self.covers(&ids);
            out.push((
                Tile::List(p.id),
                HomeTile {
                    name: p.name.as_str().into(),
                    detail: super::pages::plural(ids.len(), "track").into(),
                    tint: tint(&p.name),
                    label: super::pages::initials(&p.name).into(),
                    ..with_covers(covers)
                },
            ));
            if out.len() == SHELF {
                break;
            }
        }
        out
    }

    /// Whether track `i` belongs in `mix`.
    fn in_mix(&self, i: usize, mix: &Mix) -> bool {
        let t = &self.library[i];
        match mix {
            Mix::Genre(g) => genres_of(&t.genre).any(|x| x.to_lowercase() == *g),
            Mix::Decade(d) => year(&t.info.year).is_some_and(|y| y / 10 * 10 == *d),
        }
    }

    /// Most played first, then best rated.
    fn by_plays(&self, ids: &mut [usize]) {
        ids.sort_by_cached_key(|&i| {
            let t = &self.library[i];
            (Reverse(t.info.play_count), Reverse(t.rating), t.id)
        });
    }

    /// What a mix is called, its big label, and what kind it is.
    pub(super) fn mix_name(&self, mix: &Mix) -> (String, String, &'static str) {
        match mix {
            Mix::Genre(g) => {
                let shown = self
                    .library
                    .iter()
                    .flat_map(|t| genres_of(&t.genre))
                    .find(|x| x.to_lowercase() == *g)
                    .unwrap_or(g)
                    .to_string();
                (format!("{shown} Mix"), shown, "Genre mix")
            }
            Mix::Decade(d) => {
                let label = if *d < 2000 {
                    format!("{:02}s", d % 100)
                } else {
                    format!("{d}s")
                };
                (format!("{label} Mix"), label, "Decade mix")
            }
        }
    }

    /// A mix's tracks: the most played and best rated that belong in it,
    /// in the smoothest order by key and tempo.
    pub(super) fn mix_tracks(&self, mix: &Mix) -> Vec<usize> {
        let mut ids: Vec<usize> = (0..self.library.len())
            .filter(|&i| self.home_track(i) && self.in_mix(i, mix))
            .collect();
        self.by_plays(&mut ids);
        ids.truncate(MIX_TRACKS);
        let items: Vec<crate::automix_order::Item> = ids
            .iter()
            .map(|&i| crate::automix_order::Item {
                bpm: self.library[i].bpm,
                key: Key::parse(&self.library[i].key),
            })
            .collect();
        let order = crate::automix_order::order(
            &items,
            0,
            crate::automix_order::Order::Flow,
            self.key_rules.0,
            &mut steady(),
        );
        order.into_iter().map(|p| ids[p]).collect()
    }

    /// The mixes worth making: the genres and decades with the most
    /// tracks, each with enough to mix.
    fn mix_tiles(&self, tracks: &[usize]) -> [Vec<(Tile, HomeTile)>; 2] {
        let mut genres: HashMap<String, Vec<usize>> = HashMap::new();
        let mut decades: HashMap<u32, Vec<usize>> = HashMap::new();
        for &i in tracks {
            let t = &self.library[i];
            let mut seen = HashSet::new();
            for g in genres_of(&t.genre) {
                let g = g.to_lowercase();
                if seen.insert(g.clone()) {
                    genres.entry(g).or_default().push(i);
                }
            }
            if let Some(y) = year(&t.info.year).filter(|&y| y >= 1900) {
                decades.entry(y / 10 * 10).or_default().push(i);
            }
        }
        fn top<T: Clone + Ord>(groups: &HashMap<T, Vec<usize>>) -> Vec<T> {
            let mut all: Vec<(T, usize)> = groups
                .iter()
                .map(|(t, ids)| (t.clone(), ids.len()))
                .filter(|(_, c)| *c >= MIX_MIN)
                .collect();
            all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            all.into_iter().take(SHELF).map(|(t, _)| t).collect()
        }
        let tile = |mix: Mix, mut ids: Vec<usize>| {
            let (name, label, kind) = self.mix_name(&mix);
            let n = ids.len().min(MIX_TRACKS);
            self.by_plays(&mut ids);
            (
                Tile::Mix(mix),
                HomeTile {
                    tint: tint(&name),
                    name: name.into(),
                    detail: format!("{kind} · {}", super::pages::plural(n, "track")).into(),
                    label: label.into(),
                    ..with_covers(self.covers(&ids))
                },
            )
        };
        let genre = top(&genres)
            .into_iter()
            .map(|g| {
                let ids = genres[&g].clone();
                tile(Mix::Genre(g), ids)
            })
            .collect();
        let mut by_decade = top(&decades);
        by_decade.sort_unstable_by(|a, b| b.cmp(a));
        let decade = by_decade
            .into_iter()
            .map(|d| tile(Mix::Decade(d), decades[&d].clone()))
            .collect();
        [genre, decade]
    }

    /// Fills Home: its shelves and what each tile opens.
    pub(super) fn refresh_home(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let tracks: Vec<usize> = (0..self.library.len())
            .filter(|&i| self.home_track(i))
            .collect();
        let mut played: Vec<usize> = tracks
            .iter()
            .copied()
            .filter(|&i| self.library[i].info.last_played.is_some())
            .collect();
        played.sort_by_key(|&i| Reverse(self.library[i].info.last_played));
        let mut added: Vec<usize> = tracks
            .iter()
            .copied()
            .filter(|&i| self.library[i].info.added > 0)
            .collect();
        added.sort_by_key(|&i| Reverse(self.library[i].info.added));
        let [genre, decade] = self.mix_tiles(&tracks);
        let shelves = [
            ("Recently played", self.release_tiles(&played)),
            ("Recently added", self.release_tiles(&added)),
            ("Your crates and playlists", self.list_tiles()),
            ("Genre mixes", genre),
            ("Decade mixes", decade),
        ];
        let mut opens = Vec::new();
        let mut shown = Vec::new();
        for (title, tiles) in shelves {
            if tiles.is_empty() {
                continue;
            }
            let (open, views): (Vec<Tile>, Vec<HomeTile>) = tiles.into_iter().unzip();
            opens.push(open);
            shown.push(HomeShelf {
                title: title.into(),
                tiles: ModelRc::new(VecModel::from(views)),
            });
        }
        let ui = app.global::<PageUi>();
        use chrono::Timelike;
        ui.set_greeting(greeting(chrono::Local::now().hour()).into());
        ui.set_shelves(ModelRc::new(VecModel::from(shown)));
        self.pages.home = opens;
    }

    /// A tile on Home was clicked: shelf `shelf`, tile `tile`.
    pub(super) fn open_tile(&mut self, shelf: usize, tile: usize) {
        let Some(t) = self
            .pages
            .home
            .get(shelf)
            .and_then(|s| s.get(tile))
            .cloned()
        else {
            return;
        };
        match t {
            Tile::Album(album, artist) => self.open_page(Page::Album(album, artist)),
            Tile::Artist(artist) => self.open_page(Page::Artist(artist)),
            Tile::Mix(mix) => self.open_page(Page::Mix(mix)),
            Tile::List(id) => self.show_list(id),
        }
    }

    /// The Home button: back to Home, from wherever Player was.
    pub(super) fn go_home(&mut self) {
        self.leave_for_collection();
        self.show_home();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genres_in_a_tag() {
        let g: Vec<&str> = genres_of("House; Deep House, Disco").collect();
        assert_eq!(g, ["House", "Deep House", "Disco"]);
        assert_eq!(genres_of(" ").count(), 0);
    }

    #[test]
    fn greetings_by_the_hour() {
        assert_eq!(greeting(8), "Good morning");
        assert_eq!(greeting(14), "Good afternoon");
        assert_eq!(greeting(22), "Good evening");
        assert_eq!(greeting(2), "Good evening");
    }

    #[test]
    fn a_mix_keeps_its_colour() {
        assert_eq!(tint("House Mix"), tint("House Mix"));
    }
}
