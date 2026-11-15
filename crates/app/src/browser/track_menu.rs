//! The track menu's side panels and "go to" lines: adding tracks to a list
//! (found by typing, most recently added to first, as Spotify does it),
//! the lists a track is in, the artist's or album's tracks, loading on a
//! deck and the Info tab.

use djsugar_library::query::{keyword, quoted};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::{Browser, Contents, COLLECTION};
use crate::{LibraryEdit, MenuListRow};

/// How many lists the panel shows before it scrolls.
const RECENT: usize = 50;

/// One of the DJ's lists, as the track menu offers it.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuList {
    pub id: i64,
    pub name: String,
    /// The folders it is in, "GY March 2024 › Prep"; empty at the top.
    pub folder: String,
    pub is_crate: bool,
}

/// The first artist of a track's artists ("Bicep, Hammer" or "Bicep feat.
/// X" is Bicep), so "Go to Artist" finds their other tracks too.
pub fn main_artist(artist: &str) -> &str {
    let lower = artist.to_lowercase();
    let cut = [
        ", ", " feat. ", " feat ", " ft. ", " ft ", " x ", " vs. ", " vs ", "; ", " / ",
    ]
    .iter()
    .filter_map(|sep| lower.find(sep))
    .min()
    .unwrap_or(artist.len());
    // The separators are ASCII, so `cut` is a char boundary of `artist` too.
    artist[..cut].trim()
}

/// The lists matching `find` (in their name or folders), most recently
/// added to first.
pub fn find_lists<'a>(
    lists: &'a [MenuList],
    touched: &std::collections::HashMap<i64, i64>,
    find: &str,
) -> Vec<(usize, &'a MenuList)> {
    let find = find.trim().to_lowercase();
    let mut out: Vec<(usize, &MenuList)> = lists
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            find.is_empty()
                || l.name.to_lowercase().contains(&find)
                || l.folder.to_lowercase().contains(&find)
        })
        .collect();
    // Stable, so lists touched at the same time keep the tree's order.
    out.sort_by_key(|(_, l)| std::cmp::Reverse(touched.get(&l.id).copied().unwrap_or(0)));
    out
}

fn rows(found: &[(usize, &MenuList)]) -> ModelRc<MenuListRow> {
    let rows: Vec<MenuListRow> = found
        .iter()
        .take(RECENT)
        .map(|(i, l)| MenuListRow {
            index: *i as i32,
            name: l.name.as_str().into(),
            folder: l.folder.as_str().into(),
            is_crate: l.is_crate,
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

impl Browser {
    pub(super) fn install_track_menu(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_find_lists(move |text| w(&|b| b.find_menu_lists(&text)));
        let w = with.clone();
        edit.on_add_deck_to_list(move |deck, list| {
            w(&|b| b.add_deck_to_list(deck.max(0) as usize, list.max(0) as usize))
        });
        let w = with.clone();
        edit.on_new_list_with_deck(move |deck, name| {
            w(&|b| b.new_list_with_deck(deck.max(0) as usize, &name))
        });
        let w = with.clone();
        edit.on_favorite_deck(move |deck| w(&|b| b.toggle_favorite(deck.max(0) as usize)));
        let w = with.clone();
        edit.on_find_tags(move |text| w(&|b| b.find_menu_tags(&text)));
        let w = with.clone();
        edit.on_add_tag(move |first, last, id, tag| {
            w(&|b| b.add_tag_to_rows(first.max(0) as usize, last.max(0) as usize, &id, &tag))
        });
        let w = with.clone();
        edit.on_lists_with(move |row| w(&|b| b.menu_lists_with(row as usize)));
        let w = with.clone();
        edit.on_show_list(move |list| w(&|b| b.show_menu_list(list as usize)));
        let w = with.clone();
        edit.on_new_list_with(move |first, last, name| {
            w(&|b| b.new_list_with(first.max(0) as usize, last.max(0) as usize, &name))
        });
        let w = with.clone();
        edit.on_go_to(move |row, album| w(&|b| b.go_to(row as usize, album)));
        let w = with.clone();
        edit.on_load_on_deck(move |row, deck| {
            w(&|b| b.load_row_on_deck(row as usize, deck as usize))
        });
        let w = with.clone();
        edit.on_show_info(move |row| w(&|b| b.show_info(row)));
        edit.on_key_for(|script| crate::shortcuts::key_for(&script).into());
        let w = with.clone();
        edit.on_go_to_text(move |row| {
            let text = std::cell::RefCell::new((SharedString::new(), SharedString::new()));
            w(&|b| {
                if let Some(t) = b.row_track(row as usize) {
                    *text.borrow_mut() = (main_artist(&t.artist).into(), t.album.trim().into());
                }
            });
            let (artist, album) = text.into_inner();
            ModelRc::new(VecModel::from(vec![artist, album]))
        });
    }

    /// The library track at a row of the list.
    fn row_track(&self, row: usize) -> Option<&djsugar_engine::Track> {
        let i = self.rows.get(row).copied().flatten()?;
        self.library.get(i).map(|t| &**t)
    }

    /// Fills the "Add to playlist" panel with the lists matching `find`.
    fn find_menu_lists(&mut self, find: &str) {
        let touched = self
            .db
            .as_ref()
            .and_then(|db| db.playlists_touched().ok())
            .unwrap_or_default();
        let found = find_lists(&self.menu_lists, &touched, find);
        if let Some(app) = self.app.upgrade() {
            app.global::<LibraryEdit>().set_list_picks(rows(&found));
        }
    }

    /// Fills the "Show in playlist" panel with the lists the track at `row` is in.
    fn menu_lists_with(&mut self, row: usize) {
        let ids = match (self.rows.get(row).copied().flatten(), &self.db) {
            (Some(i), Some(db)) => db.playlists_with(self.library[i].id).unwrap_or_default(),
            _ => Vec::new(),
        };
        let found: Vec<(usize, &MenuList)> = self
            .menu_lists
            .iter()
            .enumerate()
            .filter(|(_, l)| ids.contains(&l.id))
            .collect();
        if let Some(app) = self.app.upgrade() {
            app.global::<LibraryEdit>().set_list_picks(rows(&found));
        }
    }

    /// Shows one of the menu's lists in the tree, with the folders above it open.
    fn show_menu_list(&mut self, list: usize) {
        if let Some(id) = self.menu_lists.get(list).map(|l| l.id) {
            self.show_list(id);
        }
    }

    /// Shows list `id` in the collection's tree, with the folders above it
    /// open, and lists its tracks.
    pub(super) fn show_list(&mut self, id: i64) {
        self.leave_for_collection();
        self.close_page();
        let Some(n) = self.nodes.iter().position(|n| n.playlist == Some(id)) else {
            return;
        };
        self.current_node = n;
        let mut depth = self.nodes[n].depth;
        for i in (0..n).rev() {
            if self.nodes[i].depth < depth {
                self.nodes[i].expanded = true;
                depth = self.nodes[i].depth;
            }
        }
        self.after_jump();
    }

    /// "New crate" in the panel: a crate named what was typed to find a
    /// list (or "New crate", to be named in the tree), with the tracks in it.
    fn new_list_with(&mut self, first: usize, last: usize, name: &str) {
        let indices = self.chosen_tracks(first, last);
        self.new_list_of(&indices, name);
    }

    /// The library index of the track on `deck`, if it is in the library.
    pub(super) fn deck_index(&self, deck: usize) -> Option<usize> {
        let id = (*self.loaded.get(deck)?)?;
        self.index_of.get(&id).copied()
    }

    /// The Player bar's ＋: the track on `deck` goes on list `list` of the panel.
    fn add_deck_to_list(&mut self, deck: usize, list: usize) {
        let (Some(index), Some(id)) = (
            self.deck_index(deck),
            self.menu_lists.get(list).map(|l| l.id),
        ) else {
            return;
        };
        self.add_to_playlist(&[index], id);
    }

    /// The Player bar's ＋, New crate: one with the track on `deck` in it.
    fn new_list_with_deck(&mut self, deck: usize, name: &str) {
        if let Some(index) = self.deck_index(deck) {
            self.new_list_of(&[index], name);
        }
    }

    fn new_list_of(&mut self, indices: &[usize], name: &str) {
        if !self.saved() || indices.is_empty() {
            return;
        }
        let typed = name.trim();
        let Some(db) = &self.db else { return };
        let id = match db.create_crate(if typed.is_empty() { "New crate" } else { typed }, None) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("can't make a crate: {e}");
                return;
            }
        };
        self.add_to_playlist(indices, id);
        if self.tree == COLLECTION && !self.health_view {
            self.reload_tree(None);
        } else {
            self.collection_stale = true;
        }
        // Named in the tree, as one made with +.
        if typed.is_empty() {
            let row = self
                .nodes
                .iter()
                .position(|n| n.playlist == Some(id))
                .and_then(|n| self.visible_nodes.iter().position(|&v| v == n));
            if let (Some(row), Some(app)) = (row, self.app.upgrade()) {
                app.global::<LibraryEdit>().set_name_row(row as i32);
            }
        }
    }

    /// Back to the collection's tree from another source, Health or the
    /// history, for a jump from the track menu.
    pub(super) fn leave_for_collection(&mut self) {
        if self.tree != COLLECTION {
            self.select_source(COLLECTION);
        }
        if self.health_view {
            self.show_health(false);
        }
        if self.history_tab {
            self.show_history(false);
            if let Some(app) = self.app.upgrade() {
                app.global::<LibraryEdit>().set_history_tab(false);
            }
        }
    }

    fn after_jump(&mut self) {
        self.health_status.clear();
        self.close_cleanup();
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_filter_ui();
        self.fetch_current_playlist();
    }

    /// "Go to Artist" / "Go to Album": the whole collection, narrowed to the
    /// track's artist or album by the filter bar's chip, so one click on the
    /// chip's clear button goes back to everything.
    fn go_to(&mut self, row: usize, album: bool) {
        let Some(t) = self.row_track(row).cloned() else {
            return;
        };
        // Player opens the artist's or album's page instead.
        if self.go_to_page(&t, album) {
            return;
        }
        let t = &t;
        let (field, value, term) = if album {
            let v = t.album.trim().to_string();
            let term = format!("{}:={}", keyword("album"), quoted(&v));
            ("album", v, term)
        } else {
            let v = main_artist(&t.artist).to_string();
            // Any track with this artist among its artists.
            let term = format!("{}:{}", keyword("artist"), quoted(&v));
            ("artist", v, term)
        };
        if value.is_empty() {
            return;
        }
        self.leave_for_collection();
        self.current_node = self
            .nodes
            .iter()
            .position(|n| matches!(n.contents, Contents::All))
            .unwrap_or(0);
        self.search.clear();
        if let Some(app) = self.app.upgrade() {
            app.global::<crate::StoreUi>()
                .set_search_text(SharedString::new());
        }
        self.filter.clear_all();
        self.filter
            .filters
            .picked
            .insert(field.to_string(), vec![(term, value)]);
        // The chip shows what the list is narrowed to, and clears it.
        if let Some(app) = self.app.upgrade() {
            app.global::<crate::FilterUi>().set_shown(true);
        }
        self.after_jump();
    }

    fn load_row_on_deck(&mut self, row: usize, deck: usize) {
        let Some(i) = self.rows.get(row).copied().flatten() else {
            return;
        };
        if deck < self.decks_shown && self.playable(i) {
            self.adopt(&[i]);
            self.engine.load_track(deck, self.library[i].clone());
        }
    }

    /// "Show Info": the side panel's Info tab, on the track picked.
    pub(crate) fn show_info(&mut self, row: i32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.set_current_track(row);
        self.info.subject = 0;
        app.global::<crate::TrackInfo>().set_subject(0);
        app.set_side_tab(super::info::INFO_TAB as i32);
        app.set_side_panel_visible(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_artist_is_the_first_of_several() {
        assert_eq!(main_artist("Bicep"), "Bicep");
        assert_eq!(main_artist("Bicep, Hammer"), "Bicep");
        assert_eq!(main_artist("Bicep feat. Clara La San"), "Bicep");
        // A duo stays whole.
        assert_eq!(main_artist("Simon & Garfunkel"), "Simon & Garfunkel");
        assert_eq!(main_artist("  "), "");
    }

    #[test]
    fn lists_most_recently_added_to_first() {
        let l = |id, name: &str, folder: &str| MenuList {
            id,
            name: name.into(),
            folder: folder.into(),
            is_crate: false,
        };
        let lists = [
            l(1, "Yoga", ""),
            l(2, "Dance", ""),
            l(3, "Set", "Gig Friday"),
        ];
        let touched = [(1, 10), (2, 30), (3, 20)].into_iter().collect();
        let names = |f: &str| -> Vec<i64> {
            find_lists(&lists, &touched, f)
                .iter()
                .map(|(_, l)| l.id)
                .collect()
        };
        assert_eq!(names(""), [2, 3, 1]);
        assert_eq!(names("friday"), [3]);
        assert_eq!(names("YO"), [1]);
    }
}
