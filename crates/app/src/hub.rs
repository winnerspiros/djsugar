//! Streaming hub: services searched and fetched on worker threads, their
//! audio cached on disk and loaded to the decks like any file.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use sugar_net::archive::Archive;
use sugar_net::audius::Audius;
use sugar_net::soundcloud::SoundCloud;
use sugar_net::youtube::YouTube;
use sugar_net::ytmusic::{self, Channel};
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

    /// The first list of the service's account; none where the tab starts with a search.
    const fn first(self) -> &'static str {
        match self {
            Self::Spotify => crate::spotify::LIKED,
            _ => "",
        }
    }

    /// The service's sign-in page, where it has one inside the app.
    fn form(self) -> Option<crate::signin::Form> {
        let ((url, host, cookie), wide) = match self {
            Self::YouTube => (ytmusic::SIGN_IN, false),
            Self::SoundCloud => (sugar_net::soundcloud::SIGN_IN, true),
            _ => return None,
        };
        Some(crate::signin::Form { url, host, cookie, wide })
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
    /// The YouTube Music account, and the file SoundCloud's token and name are kept in.
    ytmusic: ytmusic::Account,
    soundcloud_file: PathBuf,
    /// Where a sign-in page leaves its answer.
    answer: PathBuf,
}

impl Clients {
    /// A page of a search, or of the account's list `q` when `listed`.
    fn search(&self, sv: Service, q: &str, more: Option<&str>, listed: bool) -> Result<Page> {
        match sv {
            Service::YouTube if listed => self.ytmusic.list(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::SoundCloud if listed => match self.soundcloud.likes(more) {
                Ok(p) => Ok(Page { hits: p.hits, more: p.more }),
                // Asking whose account it is tells a token that ran out from a bad day at SoundCloud.
                Err(sugar_net::Error::Status(401 | 403)) => {
                    Err(self.soundcloud.me().err().unwrap_or(sugar_net::Error::Status(401)))
                }
                Err(e) => Err(e),
            },
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

impl Clients {
    /// Takes the cookies a sign-in page left; the account's name, or nothing when they are no sign-in.
    fn signed(&self, sv: Service, cookies: &str) -> Option<String> {
        if sv == Service::YouTube {
            return self.ytmusic.sign_in(cookies).then(|| self.ytmusic.whose().unwrap_or_default());
        }
        let token = cookies.split(';').find_map(|p| p.trim().strip_prefix("oauth_token="))?;
        self.soundcloud.sign_in(token);
        let Ok((_, name)) = self.soundcloud.me() else {
            self.soundcloud.sign_out();
            return None;
        };
        keep(&self.soundcloud_file, &format!("{token}\n{name}"));
        Some(name)
    }
}

/// Writes an account's file, for the user's eyes only.
fn keep(file: &std::path::Path, text: &str) {
    if std::fs::write(file, text).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600));
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
    /// A sign-in ended: the account's name, or why not; then the count of sign-outs it was started in.
    Account(Service, std::result::Result<String, String>, u64),
    /// The YouTube account's channels.
    Channels(Vec<Channel>, u64),
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
    /// Each service's account by `Service as usize`: its name once signed in, empty until the service has told it.
    accounts: [Option<String>; 5],
    /// The service whose sign-in page is open, and what ends the wait for it.
    signing: Option<Service>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    /// The account's list shown: liked songs, the playlists, one playlist or a Spotify search; none for a search.
    pub list: String,
    /// The YouTube account's channels, when it has more than its own.
    pub channels: Vec<Channel>,
    /// Counts sign-outs, so the answer to an earlier sign-in is dropped.
    signs: u64,
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
        let youtube = ytmusic::Account::open(cache.with_file_name("youtube_account"));
        let soundcloud_file = cache.with_file_name("soundcloud_account");
        let soundcloud = SoundCloud::new();
        let kept = std::fs::read_to_string(&soundcloud_file).unwrap_or_default();
        let (token, name) = kept.split_once('\n').unwrap_or((&kept, ""));
        soundcloud.sign_in(token);
        let mut accounts = [const { None }; 5];
        accounts[Service::YouTube as usize] = youtube.signed_in().then(|| youtube.name());
        accounts[Service::SoundCloud as usize] = soundcloud.signed_in().then(|| name.to_owned());
        accounts[Service::Spotify as usize] = djsugar_spotify::logged_in(&spotify).then(String::new);
        let hub = Self {
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
            accounts,
            signing: None,
            stop: Arc::default(),
            list: String::new(),
            channels: Vec::new(),
            signs: 0,
            era: 0,
            round: Arc::default(),
            clients: Arc::new(Clients {
                youtube: YouTube::new("US", ""),
                soundcloud,
                audius: Audius::new(),
                archive: Archive,
                spotify,
                ytmusic: youtube,
                soundcloud_file,
                answer: cache.with_file_name("signin_answer"),
            }),
            cache,
            tx,
            rx,
        };
        hub.ask_channels();
        hub
    }

    /// The shown service's account: its name, empty until the service has told it.
    pub fn account(&self) -> Option<&str> {
        self.accounts[self.service as usize].as_deref()
    }

    /// The shown service's sign-in page is open.
    pub fn signing(&self) -> bool {
        self.signing == Some(self.service)
    }

    /// Whether the shown service can be signed in to here.
    pub fn can_sign_in(&self) -> bool {
        self.service == Service::Spotify || (crate::signin::HERE && self.service.form().is_some())
    }

    /// Gives up the wait for a sign-in page.
    fn stop_waiting(&mut self) {
        match self.signing {
            Some(Service::Spotify) => djsugar_spotify::cancel_login(),
            Some(_) => self.stop.store(true, std::sync::atomic::Ordering::Release),
            None => {}
        }
    }

    /// Opens the service's sign-in page and waits for it; a second call gives the wait up.
    pub fn sign_in(&mut self) {
        if self.signing.is_some() {
            self.stop_waiting();
            return;
        }
        let (sv, c, tx, signs) = (self.service, self.clients.clone(), self.tx.clone(), self.signs);
        if sv == Service::Spotify {
            self.signing = Some(sv);
            self.status = "Sign in on the page that opened".into();
            std::thread::spawn(move || {
                let r = djsugar_spotify::login(&c.spotify, |url| {
                    crate::open::open(url);
                });
                let _ = tx.send(Reply::Account(sv, r, signs));
            });
            return;
        }
        let Some(form) = sv.form() else { return };
        let _ = std::fs::remove_file(&c.answer);
        if !crate::signin::open(form, &c.answer) {
            self.status = format!("The sign-in page for {} did not open", sv.title());
            return;
        }
        self.signing = Some(sv);
        self.status = String::new();
        self.stop = Arc::default();
        let stop = self.stop.clone();
        std::thread::spawn(move || {
            let r = match crate::signin::wait(&c.answer, &stop) {
                Some(cookies) if !cookies.trim().is_empty() => c.signed(sv, &cookies).ok_or_else(String::new),
                _ => Err("cancelled".to_owned()),
            };
            let _ = tx.send(Reply::Account(sv, r, signs));
        });
    }

    pub fn sign_out(&mut self) {
        let sv = self.service;
        if self.signing == Some(sv) {
            self.stop_waiting();
        }
        match sv {
            Service::Spotify => djsugar_spotify::logout(&self.clients.spotify),
            Service::YouTube => {
                self.clients.ytmusic.sign_out();
                self.channels.clear();
            }
            Service::SoundCloud => {
                self.clients.soundcloud.sign_out();
                let _ = std::fs::remove_file(&self.clients.soundcloud_file);
            }
            _ => return,
        }
        self.accounts[sv as usize] = None;
        self.signs += 1;
        self.era += 1;
        self.searching = false;
        self.hits.clear();
        self.more = None;
        self.status = String::new();
        sv.first().clone_into(&mut self.list);
    }

    /// Asks which channels the YouTube account has; the menu to choose one shows when there are several.
    fn ask_channels(&self) {
        if self.accounts[Service::YouTube as usize].is_none() {
            return;
        }
        let (c, tx, signs) = (self.clients.clone(), self.tx.clone(), self.signs);
        std::thread::spawn(move || {
            if let Ok(all) = c.ytmusic.channels() {
                let _ = tx.send(Reply::Channels(all, signs));
            }
        });
    }

    /// Whether the YouTube account is asked as its `n`th channel.
    pub fn chosen(&self, n: usize) -> bool {
        self.channels.get(n).is_some_and(|c| c.page == self.clients.ytmusic.page())
    }

    /// Asks YouTube as the account's `n`th channel from now on, and shows the list again as that one.
    pub fn choose(&mut self, n: usize) {
        let Some(channel) = self.channels.get(n) else { return };
        self.clients.ytmusic.choose(channel);
        self.accounts[Service::YouTube as usize] = Some(channel.name.clone());
        if self.service == Service::YouTube && !self.list.is_empty() {
            // A playlist belongs to the channel it was opened as.
            let list =
                if self.list.starts_with(crate::spotify::LIST) { crate::spotify::LISTS } else { &self.list }.to_owned();
            self.show(&list);
        }
    }

    /// An account the service no longer takes is signed out here too.
    fn dropped(&mut self) {
        let c = self.clients.clone();
        for (sv, signed) in [(Service::YouTube, c.ytmusic.signed_in()), (Service::SoundCloud, c.soundcloud.signed_in())]
        {
            if !signed && self.accounts[sv as usize].take().is_some() {
                self.signs += 1;
                if sv == Service::SoundCloud {
                    let _ = std::fs::remove_file(&c.soundcloud_file);
                } else {
                    self.channels.clear();
                }
                if sv == self.service {
                    self.list.clear();
                    self.hits.clear();
                    self.more = None;
                }
            }
        }
    }

    /// Shows one of the account's lists.
    pub fn show(&mut self, list: &str) {
        if self.account().is_none() {
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

    /// What a search asks the service, and whether that is a list of the account: the words, the list shown when
    /// there are none, or for Spotify the list the words stand for.
    fn request(&mut self) -> (String, bool) {
        let q = self.query.trim().to_string();
        let spotify = self.service == Service::Spotify;
        if self.account().is_none() {
            return (if spotify { String::new() } else { q }, false);
        }
        if spotify {
            if !q.is_empty() {
                self.list = crate::spotify::search(&q);
            }
            return (self.list.clone(), true);
        }
        if !q.is_empty() {
            self.list.clear();
            return (q, false);
        }
        (self.list.clone(), true)
    }

    pub fn search(&mut self) {
        let (q, listed) = self.request();
        if q.is_empty() || self.searching {
            return;
        }
        self.searching = true;
        self.status = if self.query.trim().is_empty() {
            String::new()
        } else {
            format!("Searching {} for \"{}\"", self.service.title(), self.query.trim())
        };
        self.scroll = 0.0;
        let (c, tx, sv, era, signs) = (self.clients.clone(), self.tx.clone(), self.service, self.era, self.signs);
        let nameless = sv == Service::Spotify && self.account() == Some("");
        std::thread::spawn(move || {
            if nameless && let Some(name) = crate::spotify::name(&c.spotify) {
                let _ = tx.send(Reply::Account(sv, Ok(name), signs));
            }
            let _ = tx.send(match c.search(sv, &q, None, listed) {
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
        let (q, listed) = self.request();
        let (c, tx, sv, era) = (self.clients.clone(), self.tx.clone(), self.service, self.era);
        std::thread::spawn(move || {
            let _ = tx.send(match c.search(sv, &q, Some(&token), listed) {
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
        sv.first().clone_into(&mut self.list);
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
                Reply::Page(_, _, era) if era != self.era => {}
                Reply::Account(sv, r, signs) => {
                    let fresh = self.signing == Some(sv);
                    if fresh {
                        self.signing = None;
                    }
                    match r {
                        Ok(_) if signs != self.signs => {}
                        Ok(name) => {
                            self.accounts[sv as usize] = Some(name);
                            if fresh && sv == Service::YouTube {
                                self.ask_channels();
                            }
                            if fresh && sv == self.service {
                                self.status = String::new();
                                self.show(crate::spotify::LIKED);
                            }
                        }
                        Err(why) if why == "cancelled" => self.status = String::new(),
                        Err(_) => self.status = "Sign-in did not finish. Try again".into(),
                    }
                }
                Reply::Channels(all, signs) => {
                    if signs == self.signs {
                        self.channels = if all.len() > 1 { all } else { Vec::new() };
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
                    self.dropped();
                }
            }
        }
        ready
    }
}
