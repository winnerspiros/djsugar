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

/// A service tab, in DJ Sugar's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    YouTube,
    SoundCloud,
    Audius,
    Archive,
}

impl Service {
    pub const ALL: [Self; 4] = [Self::YouTube, Self::SoundCloud, Self::Audius, Self::Archive];

    pub const fn name(self) -> &'static str {
        match self {
            Self::YouTube => "YOUTUBE",
            Self::SoundCloud => "SOUNDCLOUD",
            Self::Audius => "AUDIUS",
            Self::Archive => "ARCHIVE",
        }
    }

    /// The name in running text.
    pub const fn title(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube",
            Self::SoundCloud => "SoundCloud",
            Self::Audius => "Audius",
            Self::Archive => "the Internet Archive",
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
}

impl Clients {
    fn search(&self, sv: Service, q: &str, more: Option<&str>) -> Result<Page> {
        match sv {
            Service::YouTube => self.youtube.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::SoundCloud => self.soundcloud.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::Audius => self.audius.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
            Service::Archive => self.archive.search(q, more).map(|p| Page { hits: p.hits, more: p.more }),
        }
    }

    fn fetch(
        &self,
        sv: Service,
        id: &str,
        dir: &std::path::Path,
        progress: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<PathBuf> {
        match sv {
            Service::YouTube => self.youtube.fetch(id, dir, progress),
            Service::SoundCloud => self.soundcloud.fetch(id, dir, progress),
            Service::Audius => self.audius.fetch(id, dir, progress),
            Service::Archive => self.archive.fetch(id, dir, progress),
        }
    }
}

#[derive(Debug)]
pub enum Reply {
    Page(Page, bool),
    Progress(String, f32),
    Ready(String, PathBuf, usize),
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
    clients: Arc<Clients>,
    cache: PathBuf,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
}

impl Hub {
    pub fn new(cache: PathBuf) -> Self {
        let (tx, rx) = channel();
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
            clients: Arc::new(Clients {
                youtube: YouTube::new("US", ""),
                soundcloud: SoundCloud::new(),
                audius: Audius::new(),
                archive: Archive,
            }),
            cache,
            tx,
            rx,
        }
    }

    pub fn search(&mut self) {
        let q = self.query.trim().to_string();
        if q.is_empty() || self.searching {
            return;
        }
        self.searching = true;
        self.status = format!("Searching {} for \"{q}\"", self.service.title());
        self.scroll = 0.0;
        let (c, tx, sv) = (self.clients.clone(), self.tx.clone(), self.service);
        std::thread::spawn(move || {
            let _ = tx.send(match c.search(sv, &q, None) {
                Ok(p) => Reply::Page(p, false),
                Err(e) => Reply::Failed(None, e.to_string()),
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
        let (c, tx, q, sv) = (self.clients.clone(), self.tx.clone(), self.query.clone(), self.service);
        std::thread::spawn(move || {
            let _ = tx.send(match c.search(sv, &q, Some(&token)) {
                Ok(p) => Reply::Page(p, true),
                Err(e) => Reply::Failed(None, e.to_string()),
            });
        });
    }

    pub fn fetch(&mut self, hit: usize, deck: usize) {
        let Some(h) = self.hits.get(hit) else { return };
        if self.fetching.iter().any(|(id, ..)| *id == h.id) {
            return;
        }
        let id = h.id.clone();
        self.fetching.push((id.clone(), deck, 0.0));
        self.status = format!("Getting \"{}\" for deck {}", h.title, deck + 1);
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
            let r = c.fetch(sv, &id, &dir, &progress);
            let _ = tx.send(match r {
                Ok(path) => Reply::Ready(id, path, deck),
                Err(e) => Reply::Failed(Some(id), e.to_string()),
            });
        });
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
        self.search();
    }

    /// Replies since the last frame; finished downloads come back as (file, title, artist, deck).
    pub fn poll(&mut self) -> Vec<(PathBuf, String, String, usize)> {
        let mut ready = Vec::new();
        while let Ok(r) = self.rx.try_recv() {
            match r {
                Reply::Page(p, append) => {
                    self.searching = false;
                    self.status = if p.hits.is_empty() && !append { "Nothing found".into() } else { String::new() };
                    if append {
                        let seen: std::collections::HashSet<String> = self.hits.iter().map(|h| h.id.clone()).collect();
                        self.hits.extend(p.hits.into_iter().filter(|h| !seen.contains(&h.id)));
                    } else {
                        self.hits = p.hits;
                    }
                    self.more = p.more;
                }
                Reply::Progress(id, f) => {
                    if let Some(x) = self.fetching.iter_mut().find(|x| x.0 == id) {
                        x.2 = f;
                    }
                }
                Reply::Ready(id, path, deck) => {
                    self.fetching.retain(|x| x.0 != id);
                    let h = self.hits.iter().find(|h| h.id == id);
                    let (title, artist) =
                        h.map_or_else(|| (id.clone(), String::new()), |h| (h.title.clone(), h.artist.clone()));
                    self.status = format!("Loaded \"{title}\" to deck {}", deck + 1);
                    ready.push((path, title, artist, deck));
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
