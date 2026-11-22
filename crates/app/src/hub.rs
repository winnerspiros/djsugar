//! Streaming hub: services searched and fetched on worker threads, their
//! audio cached on disk and loaded to the decks like any file.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use sugar_net::Hit;
use sugar_net::youtube::{Page, YouTube};

/// A service tab, in DJ Sugar's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    YouTube,
}

impl Service {
    pub const ALL: [Self; 1] = [Self::YouTube];

    pub const fn name(self) -> &'static str {
        match self {
            Self::YouTube => "YOUTUBE",
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
    youtube: Arc<YouTube>,
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
            youtube: Arc::new(YouTube::new("US", "")),
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
        self.status = format!("Searching \"{q}\"");
        self.scroll = 0.0;
        let (yt, tx) = (self.youtube.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(match yt.search(&q, None) {
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
        let (yt, tx, q) = (self.youtube.clone(), self.tx.clone(), self.query.clone());
        std::thread::spawn(move || {
            let _ = tx.send(match yt.search(&q, Some(&token)) {
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
        let (yt, tx, dir) = (self.youtube.clone(), self.tx.clone(), self.cache.clone());
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
            let r = yt.fetch(&id, &dir, &progress);
            let _ = tx.send(match r {
                Ok(path) => Reply::Ready(id, path, deck),
                Err(e) => Reply::Failed(Some(id), e.to_string()),
            });
        });
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
