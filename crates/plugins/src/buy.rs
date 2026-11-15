//! Buying Wanted tracks: where each track is sold, which store to offer
//! first, and the pages to open, one at a time, so each store's own cart
//! collects them for one checkout.
//!
//! No fixed store order. A store that has the track comes before one that
//! can only be searched; among those, artist-direct stores first, then the
//! one whose files reach the library fastest, then lossless ones. Links
//! carry no affiliate or referral tags. DJ Sugar never takes payment.

use crate::musicbrainz::MusicBrainz;
use crate::{ItemKind, Plugin, PluginError, StoreItem, StoreQuery};

/// How many pages one click opens: one, so the DJ buys a track, then asks
/// for the next, instead of facing a pile of tabs.
pub const BATCH: usize = 1;

/// How a store's files reach the library once bought.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Speed {
    /// DJ Sugar fetches them from the DJ's account.
    Pulled,
    /// The store's own app downloads them into a folder the inbox watches.
    App,
    /// The DJ downloads them in the browser, into Downloads.
    Browser,
}

/// What the ranking knows about a store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoreTraits {
    /// A built-in store's plugin id, or "artist" for the artist's own site.
    pub id: &'static str,
    pub name: &'static str,
    /// Most of the money goes to the artist (or their label) directly.
    pub artist_direct: bool,
    pub speed: Speed,
    /// Sells FLAC, WAV or AIFF.
    pub lossless: bool,
}

const fn traits(
    id: &'static str,
    name: &'static str,
    artist_direct: bool,
    speed: Speed,
    lossless: bool,
) -> StoreTraits {
    StoreTraits {
        id,
        name,
        artist_direct,
        speed,
        lossless,
    }
}

/// The stores Buy looks in. Their order only breaks ties.
pub const STORES: [StoreTraits; 12] = [
    traits("artist", "Artist's site", true, Speed::Browser, true),
    traits("bandcamp", "Bandcamp", true, Speed::Browser, true),
    traits("mirlo", "Mirlo", true, Speed::Browser, true),
    traits("itunes", "iTunes Store", false, Speed::App, false),
    traits("qobuz", "Qobuz", false, Speed::Browser, true),
    traits("bleep", "Bleep", false, Speed::Browser, true),
    traits("boomkat", "Boomkat", false, Speed::Browser, true),
    traits("beatport", "Beatport", false, Speed::Browser, true),
    traits("juno", "Juno Download", false, Speed::Browser, true),
    traits("traxsource", "Traxsource", false, Speed::Browser, true),
    traits("amazon", "Amazon Music", false, Speed::Browser, false),
    // Not searched by default; here so a link to it found elsewhere ranks.
    traits("beatsource", "Beatsource", false, Speed::Browser, true),
];

pub fn store_traits(id: &str) -> Option<&'static StoreTraits> {
    STORES.iter().find(|s| s.id == id)
}

/// A Wanted track to buy.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuyTrack {
    pub artist: String,
    pub title: String,
    /// Seconds; 0 when unknown.
    pub duration: f64,
    pub isrc: String,
}

/// A store to buy a track on.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub store: &'static str,
    pub url: String,
    /// The store has this track (its page); false for a search page to
    /// try ("Try on …").
    pub found: bool,
    /// What the store asks for the track ("$1.29"), when its search says;
    /// empty when not.
    pub price: String,
}

impl Offer {
    pub fn name(&self) -> &'static str {
        store_traits(self.store).map_or(self.store, |t| t.name)
    }
}

/// Lowercase letters and digits only.
fn simplify(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// A title without what stores add to it: "(Original Mix)", "- Original".
fn plain_title(title: &str) -> String {
    let t = simplify(title);
    for tail in ["originalmix", "original", "extendedmix"] {
        if let Some(rest) = t.strip_suffix(tail) {
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    t
}

/// The artists in a credit: "A, B & C feat. D" → A, B, C, D.
fn artists(credit: &str) -> Vec<String> {
    let mut s = credit.to_lowercase();
    for sep in [
        " feat. ",
        " feat ",
        " ft. ",
        " featuring ",
        " & ",
        " and ",
        " x ",
        " vs. ",
    ] {
        s = s.replace(sep, ",");
    }
    s.split([',', ';', '/'])
        .map(simplify)
        .filter(|a| !a.is_empty())
        .collect()
}

/// Whether a store's result is the wanted track: same title (give or take
/// "Original Mix"), an artist in common, and lengths within 2 seconds when
/// both are known.
pub fn is_match(want: &BuyTrack, artist: &str, title: &str, duration: f64) -> bool {
    if plain_title(&want.title).is_empty() || plain_title(&want.title) != plain_title(title) {
        return false;
    }
    let wanted = artists(&want.artist);
    let theirs = artists(artist);
    let same_artist = wanted.is_empty() || wanted.iter().any(|a| theirs.contains(a));
    let same_length =
        want.duration <= 0.0 || duration <= 0.0 || (want.duration - duration).abs() <= 2.0;
    same_artist && same_length
}

/// The store's result that is the wanted track, if any.
pub fn find_in<'a>(want: &BuyTrack, items: &'a [StoreItem]) -> Option<&'a StoreItem> {
    items.iter().find(|i| {
        i.kind == ItemKind::Track
            && i.page_url.is_some()
            && is_match(want, &i.artist, &i.title, i.duration)
    })
}

/// Best first: has it, artist-direct, fastest to the library, lossless,
/// then the order of [`STORES`]. One offer per store, a found page beating
/// a search.
pub fn rank(mut offers: Vec<Offer>) -> Vec<Offer> {
    let key = |o: &Offer| {
        let t = store_traits(o.store);
        (
            !o.found,
            !t.is_some_and(|t| t.artist_direct),
            t.map_or(Speed::Browser, |t| t.speed),
            !t.is_some_and(|t| t.lossless),
            STORES
                .iter()
                .position(|s| s.id == o.store)
                .unwrap_or(usize::MAX),
        )
    };
    offers.sort_by_key(key);
    let mut out: Vec<Offer> = Vec::new();
    for o in offers {
        // The artist's own site can have several pages (shops); keep each.
        let new_store = o.store == "artist" || !out.iter().any(|k| k.store == o.store);
        if new_store && !out.iter().any(|k| k.url == o.url) {
            out.push(o);
        }
    }
    out
}

/// Where a track is sold: pages MusicBrainz knows for its ISRC, then each
/// store's search. A store that can't say gets its search page, to try.
/// A store's search still runs when MusicBrainz knew its page, for the
/// price it gives.
pub fn find(
    track: &BuyTrack,
    stores: &mut [Box<dyn Plugin>],
    musicbrainz: Option<&mut MusicBrainz>,
) -> Vec<Offer> {
    let mut offers = Vec::new();
    if let (Some(mb), false) = (musicbrainz, track.isrc.is_empty()) {
        match mb.sold_at(&track.isrc) {
            Ok(found) => offers.extend(found.into_iter().map(|s| Offer {
                store: s.store,
                url: s.url,
                found: true,
                price: String::new(),
            })),
            Err(e) => eprintln!("buy: {e}"),
        }
    }
    let query = StoreQuery::track(&track.artist, &track.title);
    for plugin in stores.iter_mut() {
        let Some(t) = store_traits(plugin.manifest().id) else {
            continue;
        };
        let Some(store) = plugin.store() else {
            continue;
        };
        let known = offers.iter().position(|o| o.found && o.store == t.id);
        let found = match store.search(&query) {
            Ok(items) => {
                find_in(track, &items).and_then(|i| Some((i.page_url.clone()?, i.price.clone())))
            }
            Err(PluginError::NotSupported) => None,
            Err(e) => {
                eprintln!("buy: {}: {e}", t.name);
                None
            }
        };
        if let Some(k) = known {
            if let Some((_, price)) = found {
                offers[k].price = price;
            }
            continue;
        }
        match found {
            Some((url, price)) => offers.push(Offer {
                store: t.id,
                url,
                found: true,
                price,
            }),
            None => {
                if let Some(url) = store.search_url(&query) {
                    offers.push(Offer {
                        store: t.id,
                        url,
                        found: false,
                        price: String::new(),
                    });
                }
            }
        }
    }
    rank(offers)
}

/// Tracks grouped by the store chosen for each (`choices[track]` is an
/// index into `offers[track]`): stores that have them first, then by how
/// many tracks they hold.
pub fn groups(offers: &[Vec<Offer>], choices: &[usize]) -> Vec<(&'static str, Vec<usize>)> {
    let mut out: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (track, list) in offers.iter().enumerate() {
        let Some(o) = list.get(choices.get(track).copied().unwrap_or(0)) else {
            continue;
        };
        match out.iter_mut().find(|(s, _)| *s == o.store) {
            Some((_, tracks)) => tracks.push(track),
            None => out.push((o.store, vec![track])),
        }
    }
    let found = |store: &str, tracks: &[usize]| {
        tracks.iter().any(|&t| {
            offers[t]
                .get(choices.get(t).copied().unwrap_or(0))
                .is_some_and(|o| o.store == store && o.found)
        })
    };
    out.sort_by_key(|(s, t)| (!found(s, t), std::cmp::Reverse(t.len())));
    out
}

/// The next batch of up to [`BATCH`] pages, after `opened` were opened.
pub fn next_batch(urls: &[String], opened: usize) -> &[String] {
    let start = opened.min(urls.len());
    &urls[start..(start + BATCH).min(urls.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Manifest, StoreSearch};

    fn want(artist: &str, title: &str, duration: f64) -> BuyTrack {
        BuyTrack {
            artist: artist.into(),
            title: title.into(),
            duration,
            isrc: String::new(),
        }
    }

    fn offer(store: &'static str, found: bool) -> Offer {
        Offer {
            store,
            url: format!("https://{store}.example/{found}"),
            found,
            price: String::new(),
        }
    }

    #[test]
    fn matches_titles_artists_and_lengths() {
        let w = want(
            "Bonobo, Totally Enormous Extinct Dinosaurs",
            "Heartbreak",
            300.0,
        );
        assert!(is_match(&w, "Bonobo", "Heartbreak (Original Mix)", 301.5));
        assert!(is_match(
            &w,
            "Totally Enormous Extinct Dinosaurs & Bonobo",
            "Heartbreak",
            0.0
        ));
        assert!(!is_match(&w, "Bonobo", "Heartbreak", 310.0));
        assert!(!is_match(&w, "Someone Else", "Heartbreak", 300.0));
        assert!(!is_match(&w, "Bonobo", "Heartbreak (Remix)", 300.0));
        assert!(!is_match(&want("A", "", 0.0), "A", "", 0.0));
    }

    #[test]
    fn has_it_beats_artist_direct_beats_speed() {
        let ranked = rank(vec![
            offer("bandcamp", false),
            offer("beatport", true),
            offer("itunes", true),
            offer("boomkat", true),
            offer("mirlo", true),
            offer("beatport", false),
        ]);
        let order: Vec<_> = ranked.iter().map(|o| (o.store, o.found)).collect();
        assert_eq!(
            order,
            [
                ("mirlo", true),
                // The Music app downloads by itself: faster than a browser.
                ("itunes", true),
                ("boomkat", true),
                ("beatport", true),
                ("bandcamp", false),
            ]
        );
    }

    #[test]
    fn ambient_track_only_on_some_stores() {
        // Found on Bandcamp and Boomkat, not on Beatport: Beatport is
        // only something to try.
        let ranked = rank(vec![
            offer("beatport", false),
            offer("boomkat", true),
            offer("bandcamp", true),
        ]);
        let order: Vec<_> = ranked.iter().map(|o| o.store).collect();
        assert_eq!(order, ["bandcamp", "boomkat", "beatport"]);
        assert!(!ranked[2].found);
    }

    struct Fake {
        manifest: Manifest,
        items: Vec<StoreItem>,
    }

    impl Plugin for Fake {
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }
        fn store(&mut self) -> Option<&mut dyn StoreSearch> {
            Some(self)
        }
    }

    impl StoreSearch for Fake {
        fn search(&mut self, _: &StoreQuery) -> Result<Vec<StoreItem>, PluginError> {
            if self.items.is_empty() {
                Err(PluginError::NotSupported)
            } else {
                Ok(self.items.clone())
            }
        }
        fn search_url(&self, q: &StoreQuery) -> Option<String> {
            Some(format!(
                "https://{}.example/search?q={}",
                self.manifest.id,
                crate::encode(&q.text)
            ))
        }
    }

    fn fake(id: &'static str, items: Vec<StoreItem>) -> Box<dyn Plugin> {
        Box::new(Fake {
            manifest: Manifest {
                id,
                name: id,
                hosts: &[],
                downloads: false,
            },
            items,
        })
    }

    #[test]
    fn finds_pages_and_falls_back_to_searches() {
        let item = StoreItem {
            kind: ItemKind::Track,
            store: "Bandcamp".into(),
            artist: "Satorio".into(),
            title: "Crystal Bowls".into(),
            release: String::new(),
            duration: 0.0,
            artwork_url: None,
            page_url: Some("https://satorio.bandcamp.com/track/crystal-bowls".into()),
            download: None,
            price: "€1.00".into(),
        };
        let mut stores = vec![
            fake("beatport", Vec::new()),
            fake("bandcamp", vec![item]),
            // Not a store Buy looks in.
            fake("archive", Vec::new()),
        ];
        let offers = find(&want("Satorio", "Crystal Bowls", 0.0), &mut stores, None);
        assert_eq!(offers.len(), 2);
        assert_eq!(offers[0].store, "bandcamp");
        assert!(offers[0].found);
        assert_eq!(offers[0].price, "€1.00");
        assert_eq!(offers[1].store, "beatport");
        assert_eq!(
            offers[1].url,
            "https://beatport.example/search?q=Satorio+Crystal+Bowls"
        );
        assert!(!offers[1].found);
    }

    #[test]
    fn groups_by_chosen_store() {
        let offers = vec![
            vec![offer("bandcamp", true), offer("beatport", false)],
            vec![offer("beatport", false)],
            vec![offer("bandcamp", true)],
            vec![offer("bandcamp", true), offer("juno", true)],
        ];
        let g = groups(&offers, &[0, 0, 0, 1]);
        assert_eq!(
            g,
            [
                ("bandcamp", vec![0, 2]),
                ("juno", vec![3]),
                ("beatport", vec![1])
            ]
        );
    }

    #[test]
    fn opens_one_at_a_time() {
        let urls: Vec<String> = (0..23).map(|i| i.to_string()).collect();
        assert_eq!(next_batch(&urls, 0), ["0"]);
        assert_eq!(next_batch(&urls, 22), ["22"]);
        assert!(next_batch(&urls, 23).is_empty());
        assert!(next_batch(&urls, 40).is_empty());
    }
}
