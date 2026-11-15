//! Reads controller mappings made in other DJ apps into DJ Sugar's format:
//! VirtualDJ mapper files, Traktor `.tsi`, rekordbox MIDI Learn `.csv` and
//! djay Pro `.djayMidiMapping`. What has no DJ Sugar equivalent yet stays as
//! a greyed row holding the other app's text, so nothing is silently lost.

mod djay;
mod rekordbox;
mod traktor;
pub mod vdj;
mod words;

use std::path::Path;

use crate::format::DjSugarMapping;

/// The apps mappings can be imported from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum App {
    VirtualDj,
    Traktor,
    Rekordbox,
    Djay,
}

impl App {
    pub fn name(self) -> &'static str {
        match self {
            App::VirtualDj => "VirtualDJ",
            App::Traktor => "Traktor",
            App::Rekordbox => "rekordbox",
            App::Djay => "djay Pro",
        }
    }
}

/// What came out of a file: one mapping per controller in it, keyboard
/// shortcuts when it was a keyboard mapping, and notes for the report.
#[derive(Debug, Default)]
pub struct Import {
    pub mappings: Vec<DjSugarMapping>,
    /// `(key, DJSugarScript)` in DJ Sugar's key spelling (`cmd+alt+1`).
    pub shortcuts: Vec<(String, String)>,
    /// Shortcuts that couldn't come across: `(key, original action)`.
    pub skipped_shortcuts: Vec<(String, String)>,
    pub notes: Vec<String>,
}

impl Import {
    /// One line for the import report: "182 of 211 rows work; 29 are kept
    /// greyed until DJ Sugar can do them."
    pub fn summary(&self) -> String {
        let (mut ok, mut off) = (0, 0);
        for m in &self.mappings {
            let (a, b) = m.counts();
            ok += a;
            off += b;
        }
        let mut parts = Vec::new();
        if ok + off > 0 {
            parts.push(if off == 0 {
                format!("All {ok} rows came across.")
            } else {
                format!(
                    "{ok} of {} rows work; {off} are kept greyed until DJ Sugar can do them.",
                    ok + off
                )
            });
        }
        if !self.shortcuts.is_empty() || !self.skipped_shortcuts.is_empty() {
            parts.push(format!(
                "{} of {} keyboard shortcuts came across.",
                self.shortcuts.len(),
                self.shortcuts.len() + self.skipped_shortcuts.len()
            ));
        }
        parts.join(" ")
    }
}

/// Which app a file comes from, by its name and first bytes.
pub fn detect(path: &Path, bytes: &[u8]) -> Option<App> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    if name.ends_with(".tsi") || head.contains("DeviceIO.Config.Controller") {
        Some(App::Traktor)
    } else if name.ends_with(".djaymidimapping")
        || (head.contains("<plist") && head.contains("midiMessageType"))
    {
        Some(App::Djay)
    } else if head.contains("<mapper") {
        Some(App::VirtualDj)
    } else if name.ends_with(".csv") || head.starts_with("@file") {
        Some(App::Rekordbox)
    } else {
        None
    }
}

/// Reads a mapping file from another app.
pub fn import_file(path: &Path) -> Result<Import, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let app = detect(path, &bytes)
        .ok_or("This isn't a mapping DJ Sugar can read: it reads VirtualDJ, Traktor, rekordbox and djay Pro mappings.")?;
    let text = String::from_utf8_lossy(&bytes);
    let mut import = match app {
        App::Traktor => traktor::import(&text, &file)?,
        App::Rekordbox => rekordbox::import(&text)?,
        App::Djay => djay::import(&text, &file)?,
        App::VirtualDj => vdj::import(&text, &file)?,
    };
    let date = today();
    for m in &mut import.mappings {
        m.imported = Some(crate::format::Imported {
            app: app.name().into(),
            file: file.clone(),
            date: date.clone(),
        });
    }
    Ok(import)
}

/// Today as `2026-10-29`, from the system clock.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Days since 1970 to a civil date (Howard Hinnant's algorithm).
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
