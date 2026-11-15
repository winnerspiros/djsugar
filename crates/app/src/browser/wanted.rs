//! The Wanted list: tracks the DJ means to get. Only the DJ puts tracks on
//! it: Buy on a store result, Copy to DJ Sugar on a streaming playlist, or
//! Add to Wanted on streaming tracks and playlists (Spotify, TIDAL…) and on
//! the streamed tracks standing in lists brought in from another app. A
//! track leaves it when the collection gets it, or with Remove from Wanted.
//!
//! The same menus offer Add to Downloads when an add-on source downloads
//! (see `StoreUi.downloads-to`). Buy on Wanted rows opens them on stores
//! (see `buy`).

use std::cell::RefCell;
use std::rc::Rc;

use djsugar_library::ListEntry;
use slint::ComponentHandle;

use super::{columns, Browser, Contents, StoreRow};
use crate::{AppWindow, WantedUi};

/// A track to want: its service or store, artist, title and page.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Want {
    pub store: String,
    pub artist: String,
    pub title: String,
    pub page: String,
    /// What matches it to the file bought for it, when the service knew.
    pub isrc: String,
    pub duration: f64,
}

impl Want {
    /// "Artist - Title", what a download source searches for.
    pub fn search_text(&self) -> String {
        search_text(&self.artist, &self.title)
    }
}

fn search_text(artist: &str, title: &str) -> String {
    if artist.is_empty() {
        title.to_string()
    } else {
        format!("{artist} - {title}")
    }
}

/// What "Add to Wanted" on a tree's list does now.
pub(super) enum ListWants {
    /// Its tracks the collection hasn't got.
    Now(Vec<Want>),
    /// A streaming playlist whose tracks haven't come yet: asked for, and
    /// wanted once they come.
    Later,
}

impl Browser {
    pub(super) fn install_wanted(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<WantedUi>();
        let w = Rc::downgrade(b);
        ui.on_add_tracks(move |first, last| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut()
                    .want_rows(first.max(0) as usize, last.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_add_list(move |row| {
            if let Some(b) = w.upgrade() {
                let mut b = b.borrow_mut();
                if let Some(node) = b.tree_node(row) {
                    b.want_list(node);
                }
            }
        });
        let w = Rc::downgrade(b);
        ui.on_list_streamed(move |row| {
            w.upgrade()
                .and_then(|b| {
                    let b = b.try_borrow().ok()?;
                    Some(b.list_streamed(b.tree_node(row)?))
                })
                .unwrap_or(false)
        });
        let w = Rc::downgrade(b);
        ui.on_list_synced(move |row| {
            w.upgrade()
                .and_then(|b| {
                    let b = b.try_borrow().ok()?;
                    let (stream, key) = b.stream_at(row)?;
                    Some(b.synced(stream, &key))
                })
                .unwrap_or(false)
        });
        let w = Rc::downgrade(b);
        ui.on_toggle_sync(move |row| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut().toggle_sync(row);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_remove(move |first, last| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut()
                    .unwant_rows(first.max(0) as usize, last.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_open(move |row| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut().open_wanted(row.max(0) as usize);
            }
        });
    }

    /// The node shown at `row` of the tree.
    fn tree_node(&self, row: i32) -> Option<usize> {
        self.visible_nodes.get(usize::try_from(row).ok()?).copied()
    }

    /// Whether the list shows the Wanted list.
    pub(super) fn shows_wanted(&self) -> bool {
        self.nodes
            .get(self.current_node)
            .is_some_and(|n| matches!(n.contents, Contents::Wanted))
    }

    /// The streamed tracks in rows `first` to `last` (every picked row,
    /// when `first` is one of several) that the collection hasn't got.
    pub(super) fn wants_at(&self, first: usize, last: usize) -> Vec<Want> {
        if self.shows_store_rows() {
            return Vec::new();
        }
        let service = self.shown_stream().map(|st| st.name.to_string());
        self.chosen_rows(first.min(last), first.max(last))
            .into_iter()
            .filter(|&row| self.rows.get(row).is_some_and(Option::is_none))
            .filter_map(|row| {
                let t = self.stream_track(row)?;
                // A streamed track in an imported list shows its service
                // where the album would be.
                let store = match (&service, self.placed.entries.is_some()) {
                    (_, true) | (None, _) => t.album.clone(),
                    (Some(s), false) => s.clone(),
                };
                Some(Want {
                    store,
                    artist: t.artist(),
                    title: t.title.clone(),
                    page: t.page.clone(),
                    isrc: t.isrc.clone(),
                    duration: t.duration,
                })
            })
            .collect()
    }

    /// The wanted tracks in rows `first` to `last` of the Wanted list.
    pub(super) fn wanted_at(&self, first: usize, last: usize) -> Vec<&djsugar_library::Wanted> {
        if !self.shows_wanted() {
            return Vec::new();
        }
        self.chosen_rows(first.min(last), first.max(last))
            .into_iter()
            .filter_map(|row| match self.store.rows.get(row) {
                Some(StoreRow::Wanted(w)) => Some(w),
                _ => None,
            })
            .collect()
    }

    /// "Artist - Title" for each track in rows `first` to `last`, for a
    /// download source to look for: streamed tracks, or wanted ones.
    pub fn download_names(&self, first: usize, last: usize) -> Vec<String> {
        if self.shows_wanted() {
            return self
                .wanted_at(first, last)
                .into_iter()
                .map(|w| search_text(&w.artist, &w.title))
                .collect();
        }
        if self.shows_store_rows() {
            return Vec::new();
        }
        self.chosen_rows(first.min(last), first.max(last))
            .into_iter()
            .filter_map(|row| self.stream_track(row))
            // The first artist only: a download source finds more that way.
            .map(|t| search_text(t.artists.first().map_or("", String::as_str), &t.title))
            .collect()
    }

    /// Puts tracks on the Wanted list; returns how many weren't on it.
    fn want(&mut self, wants: &[Want]) -> usize {
        let Some(db) = &self.db else { return 0 };
        let mut added = 0;
        for w in wants.iter().filter(|w| !w.page.is_empty()) {
            let result = db.add_wanted(&w.store, &w.artist, &w.title, &w.page);
            let _ = db.set_wanted_details(&w.page, &w.isrc, w.duration);
            match result {
                Ok(true) => added += 1,
                Ok(false) => {}
                Err(e) => {
                    eprintln!("can't add to the wanted list: {e}");
                    break;
                }
            }
        }
        self.store.wanted = db.wanted().unwrap_or_default();
        added
    }

    /// Says what adding (to Wanted or Downloads) did: under the streaming
    /// service's playlists, or in the header of the list shown.
    pub fn wanted_note(&mut self, text: String) {
        if self.shown_stream().is_some() {
            self.set_stream_status(text);
        } else if let Some(Contents::Playlist(id)) = self
            .nodes
            .get(self.current_node)
            .map(|n| n.contents.clone())
        {
            self.export_status(id, text);
        }
        // The tree's Wanted count.
        self.refresh_tree();
    }

    /// "Add to Wanted" on streamed tracks in the list.
    fn want_rows(&mut self, first: usize, last: usize) {
        let wants = self.wants_at(first, last);
        if wants.is_empty() {
            return;
        }
        let added = self.want(&wants);
        let text = added_text(added, wants.len());
        self.wanted_note(text);
    }

    /// The tracks the collection hasn't got in the tree's node `index`:
    /// a streaming playlist's, or the streamed tracks standing in an
    /// imported list. None when it isn't such a list.
    /// A streaming playlist not fetched yet is asked for; with
    /// `want_later` its tracks go on the Wanted list once they come.
    pub(super) fn list_wants(&mut self, index: usize, want_later: bool) -> Option<ListWants> {
        match self.nodes.get(index)?.contents.clone() {
            Contents::Stream(key) => {
                let st = self.shown_stream()?;
                let Some(tracks) = st.state.tracks.get(&key).cloned() else {
                    let st = self.shown_stream_mut()?;
                    if st.state.loading.insert(key.clone()) {
                        st.service
                            .send(crate::spotify::Request::Tracks(key.clone()));
                    }
                    if want_later {
                        st.wanted_later.insert(key);
                    }
                    return Some(ListWants::Later);
                };
                let name = st.name.to_string();
                let found = self.spotify_matches(&tracks);
                Some(ListWants::Now(
                    tracks
                        .iter()
                        .zip(found)
                        .filter(|(_, f)| f.is_none())
                        .map(|(t, _)| Want {
                            store: name.clone(),
                            artist: t.artist(),
                            title: t.title.clone(),
                            page: t.page.clone(),
                            isrc: t.isrc.clone(),
                            duration: t.duration,
                        })
                        .collect(),
                ))
            }
            Contents::Playlist(id) => {
                let entries = self.db.as_ref()?.playlist_entries(id).ok()?;
                Some(ListWants::Now(
                    entries
                        .into_iter()
                        .filter_map(|e| match e {
                            ListEntry::Streamed(s) => Some(Want {
                                store: s.service_name().to_string(),
                                artist: s.artist.clone(),
                                title: s.title.clone(),
                                page: s.page_url()?,
                                isrc: s.isrc.clone(),
                                duration: s.duration,
                            }),
                            ListEntry::Track(_) => None,
                        })
                        .collect(),
                ))
            }
            _ => None,
        }
    }

    /// "Add to Wanted" on a list in the tree, by its node.
    fn want_list(&mut self, index: usize) {
        match self.list_wants(index, true) {
            Some(ListWants::Now(wants)) if wants.is_empty() => {
                self.wanted_note("Your collection has every track of this list.".into())
            }
            Some(ListWants::Now(wants)) => {
                let added = self.want(&wants);
                let text = added_text(added, wants.len());
                self.wanted_note(text);
            }
            Some(ListWants::Later) => {
                self.wanted_note("Adding the playlist's tracks to Wanted once it loads…".into())
            }
            None => {}
        }
    }

    /// "Add to Downloads" on a list in the tree that has no link a download
    /// source takes: "Artist - Title" for each track the collection hasn't
    /// got.
    /// The name of the list at `row` in the tree.
    pub fn list_name(&self, row: usize) -> Option<String> {
        let node = self.tree_node(row as i32)?;
        Some(self.nodes[node].name.clone())
    }

    pub fn list_download_names(&mut self, row: usize) -> Vec<String> {
        let Some(node) = self.tree_node(row as i32) else {
            return Vec::new();
        };
        match self.list_wants(node, false) {
            Some(ListWants::Now(wants)) => wants.iter().map(Want::search_text).collect(),
            Some(ListWants::Later) => {
                self.wanted_note(
                    "Loading the playlist; pick Add to Downloads again once it shows.".into(),
                );
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    /// A streaming playlist asked for by "Add to Wanted" has come.
    pub(super) fn want_arrived(&mut self, stream: usize, key: &str) {
        if self.tree != self.streams[stream].source {
            return;
        }
        let Some(node) = self
            .nodes
            .iter()
            .position(|n| matches!(&n.contents, Contents::Stream(k) if k == key))
        else {
            return;
        };
        self.want_list(node);
    }

    /// Whether "Add to Wanted" fits the tree's node `index`: a streaming
    /// playlist, or a list with streamed tracks standing in it.
    pub(super) fn list_streamed(&self, index: usize) -> bool {
        match self.nodes.get(index).map(|n| &n.contents) {
            Some(Contents::Stream(_)) => self.shown_stream().is_some(),
            Some(Contents::Playlist(id)) => self.db.as_ref().is_some_and(|db| {
                db.playlist_entries(*id)
                    .is_ok_and(|e| e.iter().any(|e| matches!(e, ListEntry::Streamed(_))))
            }),
            _ => false,
        }
    }

    /// A streaming playlist's tracks the collection hasn't got, once its
    /// tracks have come; `stream` is the service's index.
    fn stream_wants(&self, stream: usize, key: &str) -> Option<Vec<Want>> {
        let st = &self.streams[stream];
        let tracks = st.state.tracks.get(key)?;
        let pairs: Vec<_> = tracks.iter().map(|t| (t, Some(&st.state.manual))).collect();
        let found = self.match_streamed(&pairs, false);
        Some(
            tracks
                .iter()
                .zip(found)
                .filter(|(_, f)| f.is_none())
                .map(|(t, _)| Want {
                    store: st.name.to_string(),
                    artist: t.artist(),
                    title: t.title.clone(),
                    page: t.page.clone(),
                    isrc: t.isrc.clone(),
                    duration: t.duration,
                })
                .collect(),
        )
    }

    /// Whether the streaming playlist `key` of service `stream` is kept in
    /// sync with the Wanted list.
    fn synced(&self, stream: usize, key: &str) -> bool {
        crate::settings::wanted_sync(self.streams[stream].id)
            .iter()
            .any(|k| k == key)
    }

    /// Puts a synced playlist's missing tracks on the Wanted list, but not
    /// ones the DJ took off it. Returns how many went on.
    fn sync_stream(&mut self, stream: usize, key: &str) -> usize {
        let Some(wants) = self.stream_wants(stream, key) else {
            return 0;
        };
        let Some(db) = &self.db else { return 0 };
        let mut added = 0;
        for w in wants.iter().filter(|w| !w.page.is_empty()) {
            let result = db.add_wanted_unless_dismissed(&w.store, &w.artist, &w.title, &w.page);
            if matches!(result, Ok(true)) {
                let _ = db.set_wanted_details(&w.page, &w.isrc, w.duration);
            }
            match result {
                Ok(true) => added += 1,
                Ok(false) => {}
                Err(e) => {
                    eprintln!("can't add to the wanted list: {e}");
                    break;
                }
            }
        }
        if added > 0 {
            self.store.wanted = db.wanted().unwrap_or_default();
            self.refresh_tree();
        }
        added
    }

    /// A playlist's tracks have come: a synced one adds what's new to the
    /// Wanted list.
    pub(super) fn synced_arrived(&mut self, stream: usize, key: &str) {
        if self.synced(stream, key) {
            self.sync_stream(stream, key);
        }
    }

    /// Fetches the synced playlists of a service the DJ is signed in to, so
    /// what was added to them since reaches the Wanted list.
    pub(super) fn fetch_synced(&mut self, stream: usize) {
        let st = &mut self.streams[stream];
        for key in crate::settings::wanted_sync(st.id) {
            if st.state.playlists.iter().any(|p| p.key == key)
                && st.state.loading.insert(key.clone())
            {
                st.service.send(crate::spotify::Request::Tracks(key));
            }
        }
    }

    /// The streaming playlist at the tree's `row`: its service's index and
    /// key.
    fn stream_at(&self, row: i32) -> Option<(usize, String)> {
        let Contents::Stream(key) = &self.nodes.get(self.tree_node(row)?)?.contents else {
            return None;
        };
        let stream = self.streams.iter().position(|s| s.source == self.tree)?;
        Some((stream, key.clone()))
    }

    /// "Add to Wanted" on a streaming playlist, which keeps adding what joins
    /// it later, or stopping that.
    fn toggle_sync(&mut self, row: i32) {
        let Some((stream, key)) = self.stream_at(row) else {
            return;
        };
        let on = !self.synced(stream, &key);
        crate::settings::set_wanted_sync(self.streams[stream].id, &key, on);
        if !on {
            return self.wanted_note(
                "New tracks in this playlist won't join Wanted any more. Its tracks stay on the list."
                    .into(),
            );
        }
        if self.streams[stream].state.tracks.contains_key(&key) {
            let text = match self.sync_stream(stream, &key) {
                0 => "Nothing missing yet. Tracks added to the playlist from now on join Wanted."
                    .to_string(),
                1 => "Added 1 track to Wanted. Tracks added to the playlist later join it."
                    .to_string(),
                n => format!(
                    "Added {n} tracks to Wanted. Tracks added to the playlist later join them."
                ),
            };
            self.wanted_note(text);
        } else {
            let st = &mut self.streams[stream];
            if st.state.loading.insert(key.clone()) {
                st.service.send(crate::spotify::Request::Tracks(key));
            }
            self.wanted_note("Adding its tracks to Wanted once it loads…".into());
        }
    }

    /// "Remove from Wanted" on rows of the Wanted list.
    fn unwant_rows(&mut self, first: usize, last: usize) {
        let ids: Vec<i64> = self.wanted_at(first, last).iter().map(|w| w.id).collect();
        if ids.is_empty() {
            return;
        }
        let Some(db) = &mut self.db else { return };
        if let Err(e) = db.remove_wanted_many(&ids) {
            eprintln!("can't take tracks off the wanted list: {e}");
        }
        self.store.wanted = db.wanted().unwrap_or_default();
        self.refresh_rows();
        self.refresh_tree();
    }

    /// Lists the Wanted rows again in the new sort, with the selected track
    /// still selected at its new place (the list then scrolls to it).
    pub(super) fn resort_wanted(&mut self, app: &AppWindow) {
        let id = usize::try_from(app.get_current_track())
            .ok()
            .and_then(|row| match self.store.rows.get(row) {
                Some(StoreRow::Wanted(w)) => Some(w.id),
                _ => None,
            });
        self.refresh_rows();
        let Some(id) = id else { return };
        let at = self
            .store
            .rows
            .iter()
            .position(|r| matches!(r, StoreRow::Wanted(w) if w.id == id));
        app.set_current_track(at.map_or(-1, |i| i as i32));
    }

    /// "Open Page" on a row of the Wanted list.
    fn open_wanted(&mut self, row: usize) {
        if let Some(StoreRow::Wanted(w)) = self.store.rows.get(row) {
            crate::spotify::open_in_browser(&w.page_url);
        }
    }
}

/// What the Wanted list shows in the store column: where the track is
/// being bought, or the service or store it came from.
pub(super) fn detail(w: &djsugar_library::Wanted) -> String {
    match djsugar_plugins::buy::store_traits(&w.pending_store) {
        Some(t) => format!("Buying on {}…", t.name),
        None => w.store.clone(),
    }
}

/// Whether a click on column `id` sorts the Wanted list: the columns it
/// fills, and when each track was added.
pub(super) fn sorts_by(id: i32) -> bool {
    matches!(
        id,
        columns::ARTIST | columns::TITLE_ID | columns::GENRE | columns::LENGTH | columns::ADDED
    )
}

/// Sorts Wanted rows, which come newest first, by column `id`; -1 (or a
/// column it can't sort by) keeps them newest first. Ties stay newest
/// first, and tracks of unknown length go last.
pub(super) fn sort(rows: &mut [djsugar_library::Wanted], id: i32, ascending: bool) {
    use std::cmp::Ordering;
    if !sorts_by(id) {
        return;
    }
    if id == columns::ADDED {
        // Oldest first going up.
        if ascending {
            rows.reverse();
        }
        return;
    }
    let text = |s: &str| djsugar_library::query::fold(s);
    let mut keyed: Vec<(String, djsugar_library::Wanted)> = rows
        .iter()
        .map(|w| {
            let key = match id {
                columns::ARTIST => text(&w.artist),
                columns::TITLE_ID => text(&w.title),
                columns::GENRE => text(&detail(w)),
                _ => String::new(),
            };
            (key, w.clone())
        })
        .collect();
    keyed.sort_by(|(ka, a), (kb, b)| {
        let o = if id == columns::LENGTH {
            match (a.duration > 0.0, b.duration > 0.0) {
                (true, true) => a.duration.total_cmp(&b.duration),
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                (false, false) => Ordering::Equal,
            }
        } else {
            // Blank names go last too.
            match (ka.is_empty(), kb.is_empty()) {
                (false, true) => return Ordering::Less,
                (true, false) => return Ordering::Greater,
                _ => ka.cmp(kb),
            }
        };
        if ascending {
            o
        } else {
            o.reverse()
        }
    });
    for (slot, (_, w)) in rows.iter_mut().zip(keyed) {
        *slot = w;
    }
}

/// What adding `asked` tracks to the Wanted list did, `added` of them new.
fn added_text(added: usize, asked: usize) -> String {
    let tracks = |n: usize| if n == 1 { "track" } else { "tracks" };
    match (added, asked) {
        (0, 1) => "It's on the Wanted list already.".into(),
        (0, _) => "They're on the Wanted list already.".into(),
        (a, n) if a == n => format!("Added {a} {} to Wanted.", tracks(a)),
        (a, n) => format!(
            "Added {a} {} to Wanted; {} were there already.",
            tracks(a),
            n - a
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_adding_did() {
        assert_eq!(added_text(1, 1), "Added 1 track to Wanted.");
        assert_eq!(added_text(0, 1), "It's on the Wanted list already.");
        assert_eq!(
            added_text(3, 5),
            "Added 3 tracks to Wanted; 2 were there already."
        );
    }

    fn wanted(id: i64, artist: &str, title: &str, duration: f64) -> djsugar_library::Wanted {
        djsugar_library::Wanted {
            id,
            store: "TIDAL".into(),
            artist: artist.into(),
            title: title.into(),
            page_url: String::new(),
            isrc: String::new(),
            duration,
            pending_store: String::new(),
        }
    }

    fn ids(rows: &[djsugar_library::Wanted]) -> Vec<i64> {
        rows.iter().map(|w| w.id).collect()
    }

    #[test]
    fn sorts_by_the_clicked_column() {
        // Newest first, as the library gives them.
        let list = vec![
            wanted(3, "bonobo", "Kerala", 245.0),
            wanted(2, "Ólafur Arnalds", "Saman", 0.0),
            wanted(1, "Burial", "Archangel", 238.0),
        ];
        let sorted = |id, up| {
            let mut rows = list.clone();
            sort(&mut rows, id, up);
            ids(&rows)
        };
        assert_eq!(sorted(-1, true), [3, 2, 1]);
        assert_eq!(sorted(columns::ARTIST, true), [3, 1, 2]);
        assert_eq!(sorted(columns::ARTIST, false), [2, 1, 3]);
        assert_eq!(sorted(columns::TITLE_ID, true), [1, 3, 2]);
        // Unknown length last, either way.
        assert_eq!(sorted(columns::LENGTH, true), [1, 3, 2]);
        assert_eq!(sorted(columns::LENGTH, false), [3, 1, 2]);
        assert_eq!(sorted(columns::ADDED, true), [1, 2, 3]);
        assert_eq!(sorted(columns::ADDED, false), [3, 2, 1]);
        // Not a column the Wanted list fills.
        assert_eq!(sorted(columns::BPM, true), [3, 2, 1]);
    }

    #[test]
    fn searches_by_artist_and_title() {
        let w = Want {
            store: "TIDAL".into(),
            artist: "Satorio".into(),
            title: "Crystal Bowls".into(),
            page: String::new(),
            isrc: String::new(),
            duration: 0.0,
        };
        assert_eq!(w.search_text(), "Satorio - Crystal Bowls");
    }
}
