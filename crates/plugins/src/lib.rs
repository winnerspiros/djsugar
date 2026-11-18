//! Plugins extend the app at a few fixed points:
//!
//! - [`StoreSearch`]: search a store and send the DJ to its page to buy a
//!   track (or, for a download add-on, fetch it into the inbox). No plugin
//!   ever takes payment or plays audio.
//! - [`InboxHint`]: recognise files from a store when they land in the
//!   purchase inbox (see [`inbox`]).
//! - [`LibraryImport`]: bring in the library of another DJ app at start
//!   ([`importers`]).
//! - [`source::SearchSource`]: a source of its own in the rail, with saved
//!   searches and downloads, for an add-on that fetches files.
//! - [`Plugin::settings`]: a page of its own in Settings, for a sign-in or
//!   anything else the DJ fills in once.
//!
//! Plugins are compiled in ([`builtin`]) or are add-on programs of their own
//! ([`remote`]); each runs on a thread of its own in the app, so a slow
//! store never holds up the UI or the audio.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use djsugar_engine::import::{ImportedLibrary, ImportedList, ListGroup};

pub mod archive;
pub mod bandcamp;
pub mod buy;
pub mod djay;
pub mod engine;
pub mod inbox;
pub mod itunes;
pub mod itunes_library;
pub mod jellyfin;
pub mod links;
pub mod mixxx;
pub mod musicbrainz;
pub mod plex;
pub mod rekordbox;
pub mod remote;
pub mod serato;
mod server;
pub mod source;
pub mod subsonic;
pub mod traktor;
pub mod virtualdj;

/// What a plugin is and which web hosts it talks to, as Settings shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    /// Stable id, e.g. "bandcamp".
    pub id: &'static str,
    /// Name shown to the DJ, e.g. "Bandcamp".
    pub name: &'static str,
    /// Hosts the plugin contacts; empty for one that only opens links.
    pub hosts: &'static [&'static str],
    /// A download add-on rather than a store: its files are filed under
    /// "Downloaded" instead of "Purchased".
    pub downloads: bool,
}

pub trait Plugin: Send {
    fn manifest(&self) -> &Manifest;
    fn store(&mut self) -> Option<&mut dyn StoreSearch> {
        None
    }
    fn inbox(&self) -> Option<&dyn InboxHint> {
        None
    }
    /// A source of its own in the rail (see [`source`]).
    fn source(&mut self) -> Option<&mut dyn source::SearchSource> {
        None
    }
    /// What the plugin's page in Settings shows, in order; none gives it no
    /// page. Asked each time the page is shown, so a note can say how
    /// things stand now.
    fn settings(&self) -> Vec<Setting> {
        Vec::new()
    }
    /// What the DJ filled in on the plugin's page, given when the plugin is
    /// made and again whenever a value changes.
    fn configure(&mut self, _config: &PluginConfig) {}
}

/// One row on a plugin's page in Settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    /// Names the value in [`PluginConfig::values`]; unused for a note.
    pub key: &'static str,
    pub kind: SettingKind,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKind {
    /// A line of text, such as a user name.
    Text,
    /// A password: typed hidden.
    Secret,
    /// Text only, such as whether the plugin is signed in.
    Note,
    /// A folder, picked with the system's dialog. Its detail says what an
    /// empty one means.
    Folder,
    /// An on/off switch, on when the bool says so until the DJ flips it.
    /// Its value is "on" or "off"; see [`PluginConfig::is_on`].
    Switch(bool),
}

impl Setting {
    pub fn new(key: &'static str, kind: SettingKind, title: &str, detail: &str) -> Self {
        Setting {
            key,
            kind,
            title: title.into(),
            detail: detail.into(),
        }
    }
}

/// A plugin's saved settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PluginConfig {
    /// What was filled in, by [`Setting::key`]; a value never filled in is
    /// missing.
    pub values: BTreeMap<String, String>,
    /// A folder of the plugin's own in the app's config folder, for files
    /// it keeps between runs.
    pub dir: Option<PathBuf>,
    /// The DJ's music folders, the ones the collection is scanned from.
    pub music_folders: Vec<PathBuf>,
}

impl PluginConfig {
    /// A value, or "" when it was never filled in.
    pub fn get(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or_default()
    }

    /// A [`SettingKind::Switch`]: `default` until the DJ flips it.
    pub fn is_on(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            "on" => true,
            "off" => false,
            _ => default,
        }
    }
}

/// What the DJ searched for: free text, and the artist and title when the
/// search came from a track ("Find in store").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreQuery {
    pub text: String,
    pub artist: Option<String>,
    pub title: Option<String>,
}

impl StoreQuery {
    pub fn text(text: &str) -> Self {
        Self {
            text: text.trim().to_string(),
            ..Self::default()
        }
    }

    /// A search for one track.
    pub fn track(artist: &str, title: &str) -> Self {
        let (artist, title) = (artist.trim(), title.trim());
        Self {
            text: [artist, title]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" "),
            artist: Some(artist.to_string()).filter(|s| !s.is_empty()),
            title: Some(title.to_string()).filter(|s| !s.is_empty()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Track,
    Release,
    Artist,
    Label,
    /// A file someone offers on a download network.
    File,
}

/// One search result.
#[derive(Clone, Debug, PartialEq)]
pub struct StoreItem {
    pub kind: ItemKind,
    /// The store's name, as its manifest gives it.
    pub store: String,
    pub artist: String,
    pub title: String,
    /// The release a track is on, or details of a file (format, size).
    pub release: String,
    /// Length in seconds, when known.
    pub duration: f64,
    pub artwork_url: Option<String>,
    /// The page to buy it on; opened in the browser.
    pub page_url: Option<String>,
    /// For a download add-on: what to pass back to [`StoreSearch::download`].
    pub download: Option<String>,
    /// What the store asks for it, ready to show ("$1.29"), when its search
    /// says; empty when not.
    pub price: String,
}

#[derive(Debug, PartialEq)]
pub enum PluginError {
    /// The store couldn't be reached, or said no.
    Network(String),
    /// The store answered with something we don't understand (it may have
    /// changed its site).
    Format(String),
    /// The plugin doesn't do this (e.g. a store we only link to).
    NotSupported,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::Network(e) => write!(f, "{e}"),
            PluginError::Format(e) => write!(f, "unexpected answer: {e}"),
            PluginError::NotSupported => write!(f, "not supported"),
        }
    }
}

pub trait StoreSearch: Send {
    /// Results to show in the app. Stores we only link to return
    /// [`PluginError::NotSupported`] and are offered by [`Self::search_url`].
    fn search(&mut self, query: &StoreQuery) -> Result<Vec<StoreItem>, PluginError>;

    /// The store's own search page for this query, opened in the browser
    /// when in-app results aren't available.
    fn search_url(&self, query: &StoreQuery) -> Option<String>;

    /// Fetches a result into `dir` (the purchase inbox). Only download
    /// add-ons do this.
    fn download(&mut self, _item: &StoreItem, _dir: &Path) -> Result<(), PluginError> {
        Err(PluginError::NotSupported)
    }
}

/// A file that arrived in the inbox, with what its tags say.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InboxFile {
    pub path: PathBuf,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub comment: String,
    /// The name of the zip it came out of, if any.
    pub archive: Option<String>,
    /// Seconds; 0 when unknown.
    pub duration: f64,
    pub isrc: String,
}

impl InboxFile {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

pub trait InboxHint {
    /// How sure (0..1) the plugin is that the file came from its store.
    fn recognise(&self, file: &InboxFile) -> f32;
}

/// Another app's library that comes into the collection at start: its
/// tracks with their tags, ratings, colours, grids and cues, and the
/// playlists, folders and history it keeps. VirtualDJ's comes with the app
/// ([`importers`]).
pub trait LibraryImport: Send {
    /// A short, stable name, kept as the source of the tracks it brings in.
    fn id(&self) -> &str;
    /// The app's name, for the DJ: the folder its lists go in, and its
    /// heading in Settings > Library.
    fn title(&self) -> &str;
    /// The library file to read, if there is one on this computer.
    fn database(&self) -> Option<PathBuf>;
    /// How to get the library file when [`LibraryImport::database`] finds
    /// none, for the DJ; empty to just ask them to choose it.
    fn missing_hint(&self) -> &str {
        ""
    }
    /// The kinds of lists it can bring in, each of which the DJ can turn
    /// on or off.
    fn list_groups(&self) -> Vec<ListGroup> {
        Vec::new()
    }
    /// What [`LibraryImport::read`] would bring in from `database`, as a
    /// line for the DJ, before they choose to import it. Empty when the
    /// importer can't say.
    fn preview(&self, database: &Path) -> Result<String, String> {
        let _ = database;
        Ok(String::new())
    }
    /// Changes whenever the library (or anything else `read` looks at) has:
    /// by default, the database's modification time.
    fn stamp(&self, database: &Path) -> String {
        let modified = std::fs::metadata(database)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        format!("{}:{modified}", database.display())
    }
    /// The lists in `groups`, for the DJ to pick from; their entries may be
    /// left empty. By default, those [`LibraryImport::read`] finds.
    fn lists(&self, database: &Path, groups: &[&str]) -> Result<Vec<ImportedList>, String> {
        Ok(self.read(database, groups)?.lists)
    }
    /// Reads the tracks in `database` (local files only), and the lists in
    /// `groups` (ids from [`LibraryImport::list_groups`]).
    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String>;
}

/// The plugins that come with the app: Bandcamp, the Internet Archive, the
/// DJ's own music servers (Plex, Navidrome, Jellyfin), the iTunes Store, then
/// the stores we link to.
pub fn builtin() -> Vec<Box<dyn Plugin>> {
    let mut out: Vec<Box<dyn Plugin>> = vec![
        Box::new(bandcamp::Bandcamp::new()),
        Box::new(archive::InternetArchive::new()),
        Box::new(plex::Plex::new()),
        Box::new(subsonic::Navidrome::new()),
        Box::new(jellyfin::Jellyfin::new()),
        Box::new(itunes::Itunes::new()),
    ];
    out.extend(
        links::STORES
            .iter()
            .map(|s| Box::new(links::LinkStore(s)) as Box<dyn Plugin>),
    );
    out
}

/// The other DJ apps whose libraries DJ Sugar can bring in. Each is off until
/// the DJ turns it on in Settings > Library.
pub fn importers() -> Vec<Box<dyn LibraryImport>> {
    vec![
        Box::new(engine::Engine),
        Box::new(virtualdj::VirtualDj),
        Box::new(rekordbox::Rekordbox),
        Box::new(mixxx::Mixxx),
        Box::new(traktor::Traktor),
        Box::new(djay::Djay),
        Box::new(serato::Serato),
        Box::new(itunes_library::ItunesLibrary),
    ]
}

/// The User-Agent plugins send, so stores can see who is asking.
pub const USER_AGENT: &str = concat!(
    "DJSugar/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/winnerspiros/djsugar)"
);

/// Percent-encodes text for a URL query.
pub fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Only web pages are opened from a result: no `file:` or other schemes.
pub fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_queries() {
        assert_eq!(encode("Daft Punk & co/é"), "Daft+Punk+%26+co%2F%C3%A9");
    }

    #[test]
    fn track_query_joins_artist_and_title() {
        let q = StoreQuery::track(" Bonobo ", "Kerala");
        assert_eq!(q.text, "Bonobo Kerala");
        assert_eq!(q.artist.as_deref(), Some("Bonobo"));
        let q = StoreQuery::track("", "Kerala");
        assert_eq!(q.text, "Kerala");
        assert_eq!(q.artist, None);
    }

    #[test]
    fn builtin_has_bandcamp_first() {
        let p = builtin();
        assert_eq!(p[0].manifest().id, "bandcamp");
        assert!(p.len() > 1);
    }

    #[test]
    fn only_web_urls() {
        assert!(is_web_url("https://x.bandcamp.com/track/y"));
        assert!(!is_web_url("file:///etc/passwd"));
        assert!(!is_web_url("javascript:alert(1)"));
    }
}
