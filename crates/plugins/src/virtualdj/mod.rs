//! Brings a VirtualDJ library into the collection at start, when the DJ
//! turns it on in Settings > Library: the tracks in `database.xml` (see
//! [`database`]), and the playlists, folders and history kept next to it
//! (see [`lists`]). Importing only reads VirtualDJ's files; File › Export
//! › VirtualDJ… writes to them (see [`export`]).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::{ImportedLibrary, ImportedList, LibraryImport, ListGroup};
use djsugar_engine::import::ImportedListKind;

pub mod database;
pub mod export;
pub mod lists;

/// Parses one of VirtualDJ's XML files. VirtualDJ sometimes writes an
/// attribute twice on one element, which XML doesn't allow, so on that
/// error the file is read again keeping only the first of each.
fn parse_xml(text: &str, f: impl FnOnce(&roxmltree::Document)) -> Result<(), roxmltree::Error> {
    match roxmltree::Document::parse(text) {
        Ok(doc) => f(&doc),
        Err(roxmltree::Error::DuplicatedAttribute(..)) => f(&roxmltree::Document::parse(
            &without_repeated_attributes(text),
        )?),
        Err(e) => return Err(e),
    }
    Ok(())
}

/// `text` with any attribute that repeats within a tag removed, keeping the
/// first. Comments, CDATA and processing instructions are left as they are.
fn without_repeated_attributes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        let skip_to = |rest: &str, open: &str, close: &str| {
            rest.starts_with(open)
                .then(|| rest.find(close).map_or(rest.len(), |i| i + close.len()))
        };
        if let Some(end) = skip_to(rest, "<!--", "-->")
            .or_else(|| skip_to(rest, "<![CDATA[", "]]>"))
            .or_else(|| skip_to(rest, "<?", "?>"))
            .or_else(|| skip_to(rest, "<!", ">"))
            .or_else(|| skip_to(rest, "</", ">"))
        {
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        // A start tag: its name, then each `name="value"`.
        let name_end = rest[1..]
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .map_or(rest.len(), |i| i + 1);
        out.push_str(&rest[..name_end]);
        rest = &rest[name_end..];
        let mut seen: Vec<&str> = Vec::new();
        loop {
            let ws = rest.len() - rest.trim_start().len();
            let after = &rest[ws..];
            let Some(eq) = after.find('=') else { break };
            let attr = after[..eq].trim();
            if attr.is_empty()
                || attr.contains(['>', '/', '<'])
                || attr.contains(char::is_whitespace)
            {
                break;
            }
            let value = after[eq + 1..].trim_start();
            let Some(quote) = value.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                break;
            };
            let Some(close) = value[1..].find(quote) else {
                break;
            };
            let end = after.len() - value.len() + 1 + close + 1;
            if !seen.contains(&attr) {
                seen.push(attr);
                out.push_str(&rest[..ws + end]);
            }
            rest = &after[end..];
        }
    }
    out.push_str(rest);
    out
}

/// VirtualDJ's `database.xml` and its lists, read whenever they have
/// changed since last time.
pub struct VirtualDj;

impl LibraryImport for VirtualDj {
    fn id(&self) -> &str {
        "vdj"
    }

    fn title(&self) -> &str {
        "VirtualDJ"
    }

    fn database(&self) -> Option<PathBuf> {
        database::default_database()
    }

    fn list_groups(&self) -> Vec<ListGroup> {
        lists::groups()
    }

    /// The newest change to the database or to anything in the list folders.
    fn stamp(&self, database: &Path) -> String {
        let home = database.parent().unwrap_or(Path::new("."));
        let mut newest = modified(database);
        let mut stack: Vec<PathBuf> = ["Playlists", "Folders", "History"]
            .iter()
            .map(|d| home.join(d))
            .collect();
        while let Some(dir) = stack.pop() {
            newest = newest.max(modified(&dir));
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    newest = newest.max(modified(&p));
                }
            }
        }
        format!("{}:{newest}", database.display())
    }

    fn preview(&self, database: &Path) -> Result<String, String> {
        let import = database::import(database, 0, Path::exists, |_| None)?;
        let home = database.parent().unwrap_or(Path::new("."));
        let all: Vec<&str> = lists::groups().iter().map(|g| g.id).collect();
        let found = lists::read(home, &all, &mut lists::Skipped::default());
        let count = |f: fn(&ImportedListKind) -> bool| found.iter().filter(|l| f(&l.kind)).count();
        let n = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        let mut out = format!(
            "{}, {}, {} and {}.",
            n(import.tracks.len(), "track", "tracks"),
            n(
                count(|k| matches!(k, ImportedListKind::List)),
                "list",
                "lists"
            ),
            n(
                count(|k| matches!(k, ImportedListKind::Smart { .. })),
                "filter folder",
                "filter folders"
            ),
            n(
                count(|k| matches!(k, ImportedListKind::History { .. })),
                "day of history",
                "days of history"
            ),
        );
        if import.streaming > 0 {
            out += &format!(
                " {} streamed tracks stay in their lists as placeholders.",
                import.streaming
            );
        }
        let left_out = import.cached + import.missing;
        if left_out > 0 {
            out += &format!(" {left_out} cached or missing files stay out.");
        }
        Ok(out)
    }

    fn lists(&self, database: &Path, groups: &[&str]) -> Result<Vec<ImportedList>, String> {
        let home = database.parent().unwrap_or(Path::new("."));
        Ok(lists::read(home, groups, &mut lists::Skipped::default()))
    }

    fn read(&self, database: &Path, groups: &[&str]) -> Result<ImportedLibrary, String> {
        let import = database::import(database, 0, Path::exists, database::read_file_tags)?;
        eprintln!(
            "VirtualDJ library: {} tracks; skipped {} streaming, {} cached, {} missing",
            import.tracks.len(),
            import.streaming,
            import.cached,
            import.missing
        );
        let home = database.parent().unwrap_or(Path::new("."));
        let mut skipped = lists::Skipped::default();
        let mut lists = lists::read(home, groups, &mut skipped);
        describe_streamed(&mut lists, &import.streamed);
        if !skipped.filters.is_empty() {
            eprintln!(
                "VirtualDJ filter folders DJ Sugar's search can't say yet: {}",
                skipped.filters.join("; ")
            );
        }
        Ok(ImportedLibrary {
            tracks: import.tracks.iter().map(|t| (**t).clone()).collect(),
            lists,
        })
    }
}

/// Fills in what a list didn't save of its streamed tracks from what the
/// database knows of them.
fn describe_streamed(
    lists: &mut [ImportedList],
    known: &std::collections::HashMap<String, (String, String, f64)>,
) {
    for e in lists.iter_mut().flat_map(|l| l.entries.iter_mut()) {
        let Some(s) = &mut e.streamed else { continue };
        let Some((artist, title, length)) = known.get(&s.uri) else {
            continue;
        };
        if s.artist.is_empty() {
            s.artist = artist.clone();
        }
        if s.title.is_empty() {
            s.title = title.clone();
        }
        if s.duration <= 0.0 {
            s.duration = *length;
        }
    }
}

fn modified(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}
