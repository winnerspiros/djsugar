//! Streaming hub: services searched and fetched on worker threads, their
//! audio cached on disk and loaded to the decks like any file.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use sugar_net::archive::Archive;
use sugar_net::audius::Audius;
use sugar_net::soundcloud::SoundCloud;
use sugar_net::youtube::YouTube;
use sugar_net::{Hit, Result};

use crate::state::Art;

/// Pixels on a side of a hit's picture in the list.
const THUMB: usize = 96;
/// Pictures fetched at once.
const FETCHERS: usize = 4;

/// A service tab, in DJ Sugar's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    YouTube,
    SoundCloud,
    Audius,
    Archive,
    Spotify,
}

impl Service {
    pub const ALL: [Self; 5] = [Self::YouTube, Self::SoundCloud, Self::Spotify, Self::Audius, Self::Archive];

    pub const fn name(self) -> &'static str {
        match self {
            Self::YouTube => "YOUTUBE",
            Self::SoundCloud => "SOUNDCLOUD",
            Self::Audius => "AUDIUS",
            Self::Archive => "ARCHIVE",
            Self::Spotify => "SPOTIFY",
        }
    }

    /// The name in running text.
    pub const fn title(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube",
            Self::SoundCloud => "SoundCloud",
            Self::Audius => "Audius",
            Self::Archive => "the Internet Archive",
            Self::Spotify => "Spotify",
        }
    }

    /// The name on its own, as a list shows it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Archive => "Internet Archive",
            _ => self.title(),
        }
    }

    /// The tab's underline.
    pub const fn colour(self) -> u32 {
        match self {
            Self::YouTube => 0xff0033,
            Self::SoundCloud => 0xff5500,
            Self::Audius => 0xcc0fe0,
            Self::Archive => 0x9a9a9e,
            Self::Spotify => 0x1db954,
        }
    }
}

/// One page of hits and where the next starts.
#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

/// Every service's client, shared by the worker threads.
#[derive(Debug)]
struct Clients {
    youtube: YouTube,
    soundcloud: SoundCloud,
    audius: Audius,
    archive: Archive,
    /// Where the Spotify account is kept.
    spotify: PathBuf,
}

impl Clients {
    fn search(&self, sv: Service, q: &str, more: Option<&str>) -> Result<Page> {
        match sv {
            Service::YouTube => self.youtube.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::SoundCloud => self.soundcloud.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::Audius => self.audius.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::Archive => self.archive.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::Spotify => crate::spotify::list(&self.spotify, q, more),
        }
    }

    fn fetch(
        &self,
        sv: Service,
        hit: &Hit,
        dir: &std::path::Path,
        progress: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<PathBuf> {
        let id = hit.id.as_str();
        match sv {
            Service::Spotify => crate::spotify::fetch(&self.spotify, &self.youtube, hit, dir, progress),
            Service::YouTube => self.youtube.fetch(id, dir, progress),
            Service::SoundCloud => self.soundcloud.fetch(id, dir, progress),
            Service::Audius => self.audius.fetch(id, dir, progress),
            Service::Archive => self.archive.fetch(id, dir, progress),
        }
    }
}

/// Why a list did not come, for the DJ: Spotify's own reasons are not for reading.
fn plain(sv: Service, e: &sugar_net::Error) -> String {
    if sv == Service::Spotify { "Spotify did not answer. Try again".into() } else { e.to_string() }
}

#[derive(Debug)]
pub enum Reply {
    /// A page, whether it follows the one shown, and the era it was asked in.
    Page(Page, bool, u64),
    /// The Spotify login ended: the account's name, or why not.
    Account(std::result::Result<String, String>, u64),
    Progress(String, f32),
    Ready(String, PathBuf, usize, Service),
    /// A hit's picture, for the search it was asked in.
    Thumb(String, Art, u64),
    Failed(Option<String>, String),
}

#[derive(Debug)]
pub struct Hub {
    pub service: Service,
    pub query: String,
    pub focus: bool,
    pub hits: Vec<Hit>,
    pub more: Option<String>,
    pub searching: bool,
    pub status: String,
    /// Downloads under way: video, deck, done 0..1.
    pub fetching: Vec<(String, usize, f32)>,
    pub scroll: f32,
    /// The hits' pictures by ID, fetched as soon as a search answers.
    pub thumbs: std::collections::HashMap<String, Art>,
    /// The Spotify account's name once signed in; empty until Spotify has told it.
    pub account: Option<String>,
    /// The Spotify login page is open.
    pub signing: bool,
    /// The Spotify list shown: liked songs, the playlists, one playlist or a search.
    pub list: String,
    /// Counts service switches and sign-outs, so answers to an earlier one are dropped.
    era: u64,
    /// Counts searches, so pictures for an old one are dropped.
    round: Arc<std::sync::atomic::AtomicU64>,
    clients: Arc<Clients>,
    cache: PathBuf,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
}

impl Hub {
    pub fn new(cache: PathBuf) -> Self {
        let (tx, rx) = channel();
        let spotify = cache.with_file_name("spotify");
        Self {
            service: Service::YouTube,
            query: String::new(),
            focus: false,
            hits: Vec::new(),
            more: None,
            searching: false,
            status: String::new(),
            fetching: Vec::new(),
            scroll: 0.0,
            thumbs: std::collections::HashMap::new(),
            account: djsugar_spotify::logged_in(&spotify).then(String::new),
            signing: false,
            list: crate::spotify::LIKED.to_owned(),
            era: 0,
            round: Arc::default(),
            clients: Arc::new(Clients {
                youtube: YouTube::new("US", ""),
                soundcloud: SoundCloud::new(),
                audius: Audius::new(),
                archive: Archive,
                spotify,
            }),
            cache,
            tx,
            rx,
        }
    }

    /// Opens Spotify's login page and waits for it; a second call gives the wait up.
    pub fn sign_in(&mut self) {
        if self.signing {
            djsugar_spotify::cancel_login();
            return;
        }
        self.signing = true;
        self.status = "Sign in on the page that opened".into();
        let (c, tx, era) = (self.clients.clone(), self.tx.clone(), self.era);
        std::thread::spawn(move || {
            let r = djsugar_spotify::login(&c.spotify, |url| {
                crate::open::open(url);
            });
            let _ = tx.send(Reply::Account(r, era));
        });
    }

    pub fn sign_out(&mut self) {
        djsugar_spotify::cancel_login();
        djsugar_spotify::logout(&self.clients.spotify);
        self.account = None;
        self.era += 1;
        self.searching = false;
        self.hits.clear();
        self.more = None;
        self.status = String::new();
        self.list = crate::spotify::LIKED.to_owned();
    }

    /// Shows one of the Spotify account's lists.
    pub fn show(&mut self, list: &str) {
        if self.account.is_none() || self.service != Service::Spotify {
            return;
        }
        self.list = list.to_owned();
        self.query.clear();
        self.hits.clear();
        self.more = None;
        self.era += 1;
        self.searching = false;
        self.search();
    }

    /// Opens the playlist among the hits.
    pub fn open(&mut self, hit: usize) {
        if let Some(id) = self.hits.get(hit).map(|h| h.id.clone()) {
            self.show(&id);
        }
    }

    /// What a search asks the service: the words, or for Spotify the list they stand for.
    fn request(&mut self) -> String {
        let q = self.query.trim().to_string();
        if self.service != Service::Spotify {
            return q;
        }
        if self.account.is_none() {
            return String::new();
        }
        if !q.is_empty() {
            self.list = crate::spotify::search(&q);
        }
        self.list.clone()
    }

    pub fn search(&mut self) {
        let q = self.request();
        if q.is_empty() || self.searching {
            return;
        }
        self.searching = true;
        self.status = if self.service == Service::Spotify && self.query.trim().is_empty() {
            String::new()
        } else {
            format!("Searching {} for \"{}\"", self.service.title(), self.query.trim())
        };
        self.scroll = 0.0;
        let (c, tx, sv, era) = (self.clients.clone(), self.tx.clone(), self.service, self.era);
        let nameless = sv == Service::Spotify && self.account.as_deref() == Some("");
        std::thread::spawn(move || {
            if nameless && let Some(name) = crate::spotify::name(&c.spotify) {
                let _ = tx.send(Reply::Account(Ok(name), era));
            }
            let _ = tx.send(match c.search(sv, &q, None) {
                Ok(p) => Reply::Page(p, false, era),
                Err(e) => Reply::Failed(None, plain(sv, &e)),
            });
        });
    }

    /// The next page, when the list is scrolled to its end.
    pub fn more(&mut self) {
        let Some(token) = self.more.take() else { return };
        if self.searching {
            self.more = Some(token);
            return;
        }
        self.searching = true;
        let (c, tx, q, sv, era) = (self.clients.clone(), self.tx.clone(), self.request(), self.service, self.era);
        std::thread::spawn(move || {
            let _ = tx.send(match c.search(sv, &q, Some(&token)) {
                Ok(p) => Reply::Page(p, true, era),
                Err(e) => Reply::Failed(None, plain(sv, &e)),
            });
        });
    }

    pub fn fetch(&mut self, hit: usize, deck: usize) {
        let Some(h) = self.hits.get(hit) else { return };
        if self.fetching.iter().any(|(id, ..)| *id == h.id) {
            return;
        }
        if h.id.starts_with(crate::spotify::LIST) {
            return;
        }
        let (id, art, wanted) = (h.id.clone(), h.art.clone(), h.clone());
        self.fetching.push((id.clone(), deck, 0.0));
        self.status = format!("Getting \"{}\" for deck {}", h.title, ["A", "B"][deck.min(1)]);
        let (c, tx, dir, sv) = (self.clients.clone(), self.tx.clone(), self.cache.clone(), self.service);
        std::thread::spawn(move || {
            let last = std::sync::Mutex::new(0u8);
            let progress = |done: u64, total: u64| {
                if let Some(pct) = (done * 100).checked_div(total) {
                    let pct = pct as u8;
                    let mut l = last.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    if pct != *l {
                        *l = pct;
                        let _ = tx.send(Reply::Progress(id.clone(), f32::from(pct) / 100.0));
                    }
                }
            };
            let r = c.fetch(sv, &wanted, &dir, &progress);
            if let Ok(path) = &r {
                // The largest picture there is; YouTube has not got the large ones for every video.
                let large = ["maxresdefault", "hq720"].map(|size| sugar_net::youtube::art(&id, size));
                let wanted = if sv == Service::YouTube { &large[..] } else { &[] };
                let _ = wanted.iter().chain([&art]).find(|url| sugar_net::http::save_art(url, path).is_ok());
                // Asked once a track; a failed ask is made again the next time it is got.
                if sv == Service::YouTube
                    && !crate::skips::file(path).is_file()
                    && let Ok(parts) = sugar_net::sponsorblock::segments(&id)
                {
                    crate::skips::save(path, &parts);
                }
            }
            let _ = tx.send(match r {
                Ok(path) => Reply::Ready(id, path, deck, sv),
                Err(_) if sv == Service::Spotify => {
                    Reply::Failed(Some(id), format!("Could not get \"{}\"", wanted.title))
                }
                Err(e) => Reply::Failed(Some(id), e.to_string()),
            });
        });
    }

    /// Fetches the pictures of the hits that have none yet, top of the list first, a few at a time.
    fn prefetch(&self) {
        use std::sync::atomic::Ordering;
        let want: Vec<(String, String)> = self
            .hits
            .iter()
            .filter(|h| !h.art.is_empty() && !self.thumbs.contains_key(&h.id))
            .map(|h| (h.id.clone(), h.art.clone()))
            .collect();
        let (want, next) = (Arc::new(want), Arc::new(std::sync::atomic::AtomicUsize::new(0)));
        let round = self.round.load(Ordering::Acquire);
        for _ in 0..FETCHERS.min(want.len()) {
            let (want, next, now, tx) = (want.clone(), next.clone(), self.round.clone(), self.tx.clone());
            std::thread::spawn(move || {
                while let Some((id, url)) = want.get(next.fetch_add(1, Ordering::AcqRel)) {
                    if now.load(Ordering::Acquire) != round {
                        return;
                    }
                    let art = sugar_net::http::picture(url)
                        .ok()
                        .and_then(|bytes| djsugar_engine::cover::decode(&bytes, 4 * THUMB as u32))
                        .and_then(|c| Art::square(url, &c, THUMB));
                    if let Some(art) = art
                        && tx.send(Reply::Thumb(id.clone(), art, round)).is_err()
                    {
                        return;
                    }
                }
            });
        }
    }

    /// Switches service; the query stays and is searched there.
    pub fn switch(&mut self, sv: Service) {
        if sv == self.service {
            return;
        }
        self.service = sv;
        self.hits.clear();
        self.more = None;
        self.searching = false;
        self.era += 1;
        self.search();
    }

    /// Replies since the last frame; finished downloads come back as (file, title, artist, deck, service).
    pub fn poll(&mut self) -> Vec<(PathBuf, String, String, usize, &'static str)> {
        let mut ready = Vec::new();
        while let Ok(r) = self.rx.try_recv() {
            match r {
                Reply::Page(_, _, era) | Reply::Account(Ok(_), era) if era != self.era => {}
                Reply::Account(r, _) => {
                    let fresh = self.signing;
                    self.signing = false;
                    match r {
                        Ok(name) => {
                            self.account = Some(name);
                            if fresh {
                                self.status = String::new();
                                self.show(crate::spotify::LIKED);
                            }
                        }
                        Err(why) if why == "cancelled" => self.status = String::new(),
                        Err(_) => self.status = "Sign-in did not finish. Try again".into(),
                    }
                }
                Reply::Page(p, append, _) => {
                    self.searching = false;
                    self.status = if p.hits.is_empty() && !append { "Nothing found".into() } else { String::new() };
                    if append {
                        let seen: std::collections::HashSet<String> = self.hits.iter().map(|h| h.id.clone()).collect();
                        self.hits.extend(p.hits.into_iter().filter(|h| !seen.contains(&h.id)));
                    } else {
                        self.hits = p.hits;
                        self.thumbs.clear();
                        self.round.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    }
                    self.more = p.more;
                    self.prefetch();
                }
                Reply::Progress(id, f) => {
                    if let Some(x) = self.fetching.iter_mut().find(|x| x.0 == id) {
                        x.2 = f;
                    }
                }
                Reply::Ready(id, path, deck, sv) => {
                    self.fetching.retain(|x| x.0 != id);
                    let h = self.hits.iter().find(|h| h.id == id);
                    let (title, artist) =
                        h.map_or_else(|| (id.clone(), String::new()), |h| (h.title.clone(), h.artist.clone()));
                    self.status = format!("Loaded \"{title}\" to deck {}", ["A", "B"][deck.min(1)]);
                    ready.push((path, title, artist, deck, sv.title()));
                }
                Reply::Thumb(id, art, round) => {
                    if round == self.round.load(std::sync::atomic::Ordering::Acquire) {
                        self.thumbs.insert(id, art);
                    }
                }
                Reply::Failed(id, why) => {
                    self.searching = id.is_some() && self.searching;
                    if let Some(id) = id {
                        self.fetching.retain(|x| x.0 != id);
                    }
                    self.status = why;
                }
            }
        }
        ready
    }
}
