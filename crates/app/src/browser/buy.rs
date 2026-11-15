//! Buy on Wanted rows: each track is looked up in every store Buy knows
//! (on a thread, so the UI never waits), the best store is chosen for it
//! (see [`djsugar_plugins::buy`]), and the Buy sheet groups the tracks by
//! store. Opening a store opens its pages one at a time (Next for the
//! next), marks the tracks
//! as being bought there, and the purchase inbox matches the files that
//! arrive back to them. DJ Sugar never takes payment.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver};

use djsugar_plugins::buy::{self, BuyTrack, Offer};
use djsugar_plugins::musicbrainz::MusicBrainz;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{AppWindow, BuyGroup, BuyRow, BuyUi, StoreUi};

/// One track's stores, best first, as the lookup thread finds them.
struct Found {
    track: usize,
    offers: Vec<Offer>,
}

pub(super) struct BuyState {
    /// The Wanted rows' ids, in the order of `tracks`.
    ids: Vec<i64>,
    tracks: Vec<BuyTrack>,
    /// Each track's stores, best first; empty until looked up.
    offers: Vec<Vec<Offer>>,
    /// The offer chosen for each track (an index into its offers).
    choices: Vec<usize>,
    /// How many pages of each store's group were opened.
    opened: HashMap<&'static str, usize>,
    looked: usize,
    found: Receiver<Found>,
}

impl Browser {
    pub(super) fn install_buy(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<BuyUi>();
        let w = Rc::downgrade(b);
        ui.on_start(move |first, last| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut()
                    .start_buy(first.max(0) as usize, last.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_open_group(move |i| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut().open_buy_group(i.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_open_track(move |row| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut().open_buy_track(row.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_pick_store(move |row, store| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut()
                    .pick_buy_store(row.max(0) as usize, store.max(0) as usize);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_set_inbox(move |on| {
            if let Some(b) = w.upgrade() {
                b.borrow_mut().set_inbox_on(on);
            }
        });
        let w = Rc::downgrade(b);
        ui.on_set_music_app(move |on| {
            if let Some(b) = w.upgrade() {
                let b = b.borrow();
                let folder = crate::settings::set_music_app_on(on);
                b.inbox.send(crate::inbox::Request::WatchMusicApp(folder));
                b.show_inbox_note();
            }
        });
        let w = Rc::downgrade(b);
        ui.on_close(move || {
            if let Some(b) = w.upgrade() {
                let mut b = b.borrow_mut();
                b.buy = None;
                if let Some(app) = b.app.upgrade() {
                    app.global::<BuyUi>().set_open(false);
                }
                // Tracks opened on a store show as being bought.
                if let Some(db) = &b.db {
                    b.store.wanted = db.wanted().unwrap_or_default();
                }
                b.refresh_rows();
            }
        });
    }

    /// Switches the purchase inbox on or off, wherever it was asked.
    pub(super) fn set_inbox_on(&mut self, on: bool) {
        let from = crate::settings::set_inbox_on(on);
        self.inbox.send(crate::inbox::Request::SetOn(on, from));
        if let Some(app) = self.app.upgrade() {
            app.global::<StoreUi>().set_inbox_on(on);
            app.global::<BuyUi>().set_inbox_on(on);
        }
        self.show_inbox_note();
    }

    /// What the Buy sheet says about where bought files go.
    fn show_inbox_note(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<BuyUi>();
        let settings = crate::settings::inbox();
        let folder = |p: &std::path::Path| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        };
        let note = match (&self.inbox.dir, settings.on) {
            (None, _) => "There's no Downloads folder to watch.".to_string(),
            (Some(d), true) => format!(
                "Tracks you download into {} go to Inbox and come off Wanted.",
                folder(d)
            ),
            (Some(d), false) => format!(
                "Off: tracks you download stay in {} and you add them yourself.",
                folder(d)
            ),
        };
        ui.set_inbox_on(settings.on);
        ui.set_music_app_offered(crate::settings::music_app_folder().is_some());
        ui.set_music_app_on(settings.music_app.is_some());
        ui.set_inbox_note(note.into());
    }

    /// "Buy…" on rows of the Wanted list: opens the Buy sheet and looks
    /// the tracks up.
    fn start_buy(&mut self, first: usize, last: usize) {
        let picked: Vec<_> = self.wanted_at(first, last).into_iter().cloned().collect();
        if picked.is_empty() {
            return;
        }
        let ids: Vec<i64> = picked.iter().map(|w| w.id).collect();
        let tracks: Vec<BuyTrack> = picked
            .iter()
            .map(|w| BuyTrack {
                artist: w.artist.clone(),
                title: w.title.clone(),
                duration: w.duration,
                isrc: w.isrc.clone(),
            })
            .collect();
        let (tx, found) = channel();
        let to_find = tracks.clone();
        std::thread::Builder::new()
            .name("buy".into())
            .spawn(move || {
                // The built-in stores only: add-on programs aren't started
                // for this.
                let mut stores = djsugar_plugins::builtin();
                stores.retain(|p| buy::store_traits(p.manifest().id).is_some());
                for p in &mut stores {
                    let id = p.manifest().id;
                    p.configure(&crate::settings::plugin_config(id));
                }
                let mut mb = MusicBrainz::new();
                for (track, t) in to_find.iter().enumerate() {
                    let offers = buy::find(t, &mut stores, Some(&mut mb));
                    if tx.send(Found { track, offers }).is_err() {
                        return;
                    }
                }
            })
            .expect("spawn buy thread");
        let n = tracks.len();
        self.buy = Some(BuyState {
            ids,
            tracks,
            offers: vec![Vec::new(); n],
            choices: vec![0; n],
            opened: HashMap::new(),
            looked: 0,
            found,
        });
        if let Some(app) = self.app.upgrade() {
            app.global::<BuyUi>().set_open(true);
        }
        self.show_inbox_note();
        self.show_buy();
    }

    /// Takes in what the lookup found. Called every frame.
    pub(super) fn poll_buy(&mut self) {
        let Some(state) = &mut self.buy else { return };
        let mut changed = false;
        for f in state.found.try_iter() {
            if let Some(o) = state.offers.get_mut(f.track) {
                *o = f.offers;
                state.looked += 1;
                changed = true;
            }
        }
        if changed {
            self.show_buy();
        }
    }

    /// The groups' pages, in their tracks' order.
    fn group_pages(state: &BuyState, tracks: &[usize]) -> Vec<(i64, String)> {
        tracks
            .iter()
            .filter_map(|&t| {
                let o = state.offers.get(t)?.get(state.choices[t])?;
                Some((state.ids[t], o.url.clone()))
            })
            .collect()
    }

    /// "Open" or "Next" on a store of the Buy sheet: its next page.
    fn open_buy_group(&mut self, index: usize) {
        let Some(state) = &mut self.buy else { return };
        let groups = buy::groups(&state.offers, &state.choices);
        let Some((store, tracks)) = groups.get(index) else {
            return;
        };
        let pages = Self::group_pages(state, tracks);
        let opened = state.opened.entry(store).or_insert(0);
        // All opened: start again from the first.
        if *opened >= pages.len() {
            *opened = 0;
        }
        let urls: Vec<String> = pages.iter().map(|(_, u)| u.clone()).collect();
        let batch = buy::next_batch(&urls, *opened).to_vec();
        let ids: Vec<i64> = pages[*opened..*opened + batch.len()]
            .iter()
            .map(|(id, _)| *id)
            .collect();
        for url in &batch {
            open_page(url);
        }
        *opened += batch.len();
        if let Some(db) = &self.db {
            if let Err(e) = db.mark_pending(&ids, store) {
                eprintln!("can't mark tracks as being bought: {e}");
            }
        }
        self.show_buy();
    }

    /// A click on a track's name: its page on the store chosen for it.
    fn open_buy_track(&mut self, row: usize) {
        let Some(state) = &self.buy else { return };
        let Some(offer) = state
            .offers
            .get(row)
            .and_then(|o| o.get(state.choices[row]))
        else {
            return;
        };
        open_page(&offer.url);
        let (id, store) = (state.ids[row], offer.store);
        if let Some(db) = &self.db {
            if let Err(e) = db.mark_pending(&[id], store) {
                eprintln!("can't mark a track as being bought: {e}");
            }
        }
    }

    /// A store picked from a track's store list.
    fn pick_buy_store(&mut self, row: usize, store: usize) {
        let Some(state) = &mut self.buy else { return };
        if state.offers.get(row).is_some_and(|o| store < o.len()) {
            state.choices[row] = store;
            self.show_buy();
        }
    }

    /// Shows the Buy sheet as it stands.
    fn show_buy(&self) {
        let (Some(state), Some(app)) = (&self.buy, self.app.upgrade()) else {
            return;
        };
        let ui = app.global::<BuyUi>();
        ui.set_count(state.tracks.len() as i32);
        ui.set_looked(state.looked as i32);
        let groups: Vec<BuyGroup> = buy::groups(&state.offers, &state.choices)
            .into_iter()
            .map(|(store, tracks)| {
                let found = tracks.iter().any(|&t| {
                    state.offers[t]
                        .get(state.choices[t])
                        .is_some_and(|o| o.found)
                });
                BuyGroup {
                    store: buy::store_traits(store).map_or(store, |t| t.name).into(),
                    count: tracks.len() as i32,
                    found,
                    opened: state.opened.get(store).copied().unwrap_or(0) as i32,
                }
            })
            .collect();
        ui.set_groups(ModelRc::new(VecModel::from(groups)));
        let rows: Vec<BuyRow> = state
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let offer = state.offers[i].get(state.choices[i]);
                BuyRow {
                    name: if t.artist.is_empty() {
                        t.title.clone().into()
                    } else {
                        format!("{} – {}", t.artist, t.title).into()
                    },
                    store: offer.map_or("", Offer::name).into(),
                    found: offer.is_some_and(|o| o.found),
                    others: state.offers[i].len().saturating_sub(1) as i32,
                    price: offer.map_or("", |o| o.price.as_str()).into(),
                    stores: ModelRc::new(VecModel::from(
                        state.offers[i]
                            .iter()
                            .map(|o| SharedString::from(offer_label(o)))
                            .collect::<Vec<_>>(),
                    )),
                    chosen: state.choices[i] as i32,
                }
            })
            .collect();
        ui.set_rows(ModelRc::new(VecModel::from(rows)));
    }
}

/// A store in a track's store list: "Bandcamp", "iTunes Store · $1.29", or
/// "Try Beatport" when only its search can be opened.
fn offer_label(o: &Offer) -> String {
    let name = if o.found {
        o.name().to_string()
    } else {
        format!("Try {}", o.name())
    };
    if o.price.is_empty() {
        name
    } else {
        format!("{name} · {}", o.price)
    }
}

/// Opens a store's page. On a Mac an iTunes Store page opens in the Music
/// app's iTunes Store, where it's bought, not on the Apple Music website.
pub(super) fn open_page(url: &str) {
    let app_link = if cfg!(target_os = "macos") {
        djsugar_plugins::itunes::store_app_link(url)
    } else {
        None
    };
    crate::spotify::open_in_browser(app_link.as_deref().unwrap_or(url));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_stores_with_prices() {
        let offer = |found, price: &str| Offer {
            store: "itunes",
            url: String::new(),
            found,
            price: price.into(),
        };
        assert_eq!(offer_label(&offer(true, "$1.29")), "iTunes Store · $1.29");
        assert_eq!(offer_label(&offer(false, "")), "Try iTunes Store");
    }
}
