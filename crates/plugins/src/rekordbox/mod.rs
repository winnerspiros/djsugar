//! Brings a rekordbox library into the collection at start, when the DJ
//! turns it on in Settings > Library: rekordbox 6 and 7's `master.db` (see
//! [`masterdb`]), or a rekordbox XML export (see [`xml`]) for rekordbox 5
//! or when the database can't be opened. It only reads rekordbox's files,
//! never writes them.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::{ImportedLibrary, ImportedList, LibraryImport, ListGroup};

pub mod common;
mod dates;
pub mod device;
pub mod master_export;
pub mod masterdb;
mod nulls;
pub mod xml;
pub mod xml_export;

pub const PLAYLISTS: &str = "playlists";
pub const SMART: &str = "smart";
pub const HISTORY: &str = "history";

pub struct Rekordbox;

/// Whether `path` is an XML export rather than `master.db`.
fn is_xml(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
}

/// rekordbox's database where rekordbox keeps it (or `DJSUGAR_RB_DATABASE`).
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DJSUGAR_RB_DATABASE") {
        return Some(PathBuf::from(p));
    }
    if let Ok(l) = rbox_core::locate::locate_library() {
        if l.available {
            return Some(l.db_path);
        }
    }
    let home = djsugar_engine::paths::home_dir()?;
    [
        "Library/Pioneer/rekordbox/master.db",
        "AppData/Roaming/Pioneer/rekordbox/master.db",
    ]
    .iter()
    .map(|p| home.join(p))
    .find(|p| p.is_file())
}

/// "1,788 tracks, 23 playlists, 4 intelligent playlists and 12 days of
/// history."
pub(crate) fn summary(tracks: usize, lists: usize, smart: usize, history: usize) -> String {
    let n = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    format!(
        "{}, {}, {} and {}.",
        n(tracks, "track", "tracks"),
        n(lists, "playlist", "playlists"),
        n(smart, "intelligent playlist", "intelligent playlists"),
        n(history, "day of history", "days of history"),
    )
}

fn modified(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

impl LibraryImport for Rekordbox {
    fn id(&self) -> &str {
        "rekordbox"
    }

    fn title(&self) -> &str {
        "rekordbox"
    }

    fn database(&self) -> Option<PathBuf> {
        default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        vec![
            ListGroup {
                id: PLAYLISTS,
                title: "Playlists",
                detail: "Your rekordbox playlists, in their folders.",
                on_by_default: true,
            },
            ListGroup {
                id: SMART,
                title: "Intelligent playlists",
                detail: "As smart lists, where DJ Sugar's search can say the same thing.",
                on_by_default: false,
            },
            ListGroup {
                id: HISTORY,
                title: "History",
                detail: "Every session you played in rekordbox, in the History tab.",
                on_by_default: true,
            },
        ]
    }

    /// The newest change to the database, its write-ahead log or the
    /// playlist file next to it.
    fn stamp(&self, database: &Path) -> String {
        let dir = database.parent().unwrap_or(Path::new("."));
        let mut wal = database.as_os_str().to_owned();
        wal.push("-wal");
        let newest = [
            database.to_path_buf(),
            PathBuf::from(wal),
            dir.join("masterPlaylists6.xml"),
        ]
        .iter()
        .map(|p| modified(p))
        .max()
        .unwrap_or(0);
        format!("{}:{newest}", database.display())
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        if is_xml(database) {
            xml::preview(database)
        } else if device::is_device(database) {
            device::preview(database)
        } else {
            masterdb::preview(database)
        }
    }

    fn lists(&self, database: &Path, groups: &[&str]) -> Result<Vec<ImportedList>, String> {
        if is_xml(database) {
            xml::lists(database, groups)
        } else if device::is_device(database) {
            Ok(device::read(database, groups, |_| true)?.lists)
        } else {
            masterdb::lists(database, groups)
        }
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let library = if is_xml(database) {
            xml::read(database, groups, Path::exists)?
        } else if device::is_device(database) {
            device::read(database, groups, Path::exists)?
        } else {
            masterdb::read(database, groups, Path::exists)?
        };
        eprintln!(
            "rekordbox library: {} tracks here, {} lists",
            library.tracks.len(),
            library.lists.len()
        );
        Ok(library)
    }
}
