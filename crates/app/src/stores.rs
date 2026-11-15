//! Store search in the browser: each plugin with a store runs on a thread of
//! its own, so a slow store never holds up the others, the UI or the audio.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use djsugar_plugins::{remote, LibraryImport, Plugin, PluginError, StoreItem, StoreQuery};

/// How long typing has to pause before a store is asked.
const DEBOUNCE: Duration = Duration::from_millis(400);

/// The plugins the app has: the built-in stores, then the add-on programs
/// in the `addons` folder of the app's config folder (and any named in
/// `DJSUGAR_ADDONS`).
pub fn plugins() -> Vec<Box<dyn Plugin>> {
    let mut all = djsugar_plugins::builtin();
    all.extend(addons());
    all
}

/// Where add-on programs are installed.
pub fn addons_dir() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("addons"))
}

fn addons() -> Vec<Box<dyn Plugin>> {
    let mut out: Vec<Box<dyn Plugin>> = Vec::new();
    for program in remote::programs(addons_dir().as_deref()) {
        match remote::RemotePlugin::new(&program) {
            // An add-on compiled in as well wins.
            Ok(p) if out.iter().any(|o| o.manifest().id == p.manifest().id) => {}
            Ok(p) => out.push(Box::new(p)),
            Err(e) => eprintln!("add-on {} not loaded: {e}", program.display()),
        }
    }
    out
}

/// [`plugins`], each told what the DJ filled in on its page in Settings.
pub fn configured() -> Vec<Box<dyn Plugin>> {
    let mut all = plugins();
    for p in &mut all {
        let id = p.manifest().id;
        p.configure(&crate::settings::plugin_config(id));
    }
    all
}

/// The other DJ apps whose libraries the app can bring in (see
/// [`djsugar_plugins::LibraryImport`]), turned on or not.
pub fn importers() -> Vec<Box<dyn LibraryImport>> {
    djsugar_plugins::importers()
}

/// The importers the DJ turned on in Settings > Library.
pub fn enabled_importers() -> Vec<Box<dyn LibraryImport>> {
    let mut all = importers();
    all.retain(|i| crate::settings::import_enabled(i.id()));
    all
}

enum Request {
    Search(u64, StoreQuery),
    Download(StoreItem, PathBuf),
}

pub enum Event {
    /// A store's answer to search number `generation`.
    Results {
        generation: u64,
        store: usize,
        items: Vec<StoreItem>,
        /// Why there are no results in the app, if the store failed.
        error: Option<String>,
        /// The store's own search page, offered as a link.
        link: Option<String>,
    },
    /// A download add-on started or failed a download.
    Download(String),
}

pub struct Store {
    pub name: String,
    requests: Sender<Request>,
}

pub struct Stores {
    pub stores: Vec<Store>,
    events: Receiver<Event>,
    generation: u64,
}

impl Stores {
    pub fn start(plugins: Vec<Box<dyn Plugin>>) -> Self {
        let (tx, events) = channel();
        let stores = plugins
            .into_iter()
            .filter_map(|mut p| p.store().is_some().then_some(p))
            .enumerate()
            .map(|(index, plugin)| {
                let (requests, rx) = channel();
                let manifest = plugin.manifest().clone();
                let tx = tx.clone();
                std::thread::Builder::new()
                    .name(format!("store {}", manifest.id))
                    .spawn(move || run(plugin, index, rx, tx))
                    .expect("spawn store thread");
                Store {
                    name: manifest.name.to_string(),
                    requests,
                }
            })
            .collect();
        Self {
            stores,
            events,
            generation: 0,
        }
    }

    /// Asks every store; answers come back from [`Stores::poll`] with the
    /// number this returns.
    pub fn search(&mut self, query: StoreQuery) -> u64 {
        self.generation += 1;
        for s in &self.stores {
            let _ = s
                .requests
                .send(Request::Search(self.generation, query.clone()));
        }
        self.generation
    }

    pub fn download(&self, store: usize, item: StoreItem, dir: PathBuf) {
        if let Some(s) = self.stores.get(store) {
            let _ = s.requests.send(Request::Download(item, dir));
        }
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

fn run(mut plugin: Box<dyn Plugin>, index: usize, rx: Receiver<Request>, tx: Sender<Event>) {
    let Some(store) = plugin.store() else { return };
    let mut next = rx.recv().ok();
    while let Some(request) = next.take() {
        match request {
            Request::Search(mut generation, mut query) => {
                // Only the last of a burst of searches (typing) is asked.
                loop {
                    match rx.recv_timeout(DEBOUNCE) {
                        Ok(Request::Search(g, q)) => (generation, query) = (g, q),
                        Ok(other) => {
                            next = Some(other);
                            break;
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                let link = store.search_url(&query);
                let (items, error) = match store.search(&query) {
                    Ok(items) => (items, None),
                    Err(PluginError::NotSupported) => (Vec::new(), None),
                    Err(e) => (Vec::new(), Some(e.to_string())),
                };
                let event = Event::Results {
                    generation,
                    store: index,
                    items,
                    error,
                    link: if query.text.is_empty() { None } else { link },
                };
                if tx.send(event).is_err() {
                    return;
                }
            }
            Request::Download(item, dir) => {
                let what = format!("{} – {}", item.artist, item.title);
                let message = match store.download(&item, &dir) {
                    Ok(()) => format!("Downloading {what} into the inbox…"),
                    Err(e) => format!("Couldn't download {what}: {e}"),
                };
                let _ = tx.send(Event::Download(message));
            }
        }
        if next.is_none() {
            next = rx.recv().ok();
        }
    }
}
