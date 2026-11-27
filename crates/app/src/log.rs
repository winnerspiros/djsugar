//! The log the DJ can switch on from the Help menu: what the app did and what the services answered, each line
//! with the seconds since the app started. Nothing is written while it is off.

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static ON: AtomicBool = AtomicBool::new(false);
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// A log longer than this starts afresh the next time it is switched on.
const LONGEST: u64 = 2 << 20;
/// The most of the log's end that is handed to another app.
const SENT: usize = 96 << 10;

/// Where the log is kept. On a phone that is the app's own folder on shared storage, `Android/data/app.djsugar/files`,
/// which a file manager or a computer can open.
pub fn path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "android")]
    if let Some(dir) = crate::android::app().and_then(|a| a.external_data_path()) {
        return Some(dir.join("log.txt"));
    }
    crate::store::dir().map(|d| d.join("log.txt"))
}

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Starts or stops the log.
pub fn set(on: bool) {
    let mut file = FILE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *file = None;
    ON.store(false, Ordering::Relaxed);
    let Some(path) = path().filter(|_| on) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let fresh = std::fs::metadata(&path).is_ok_and(|m| m.len() > LONGEST);
    let opened = std::fs::OpenOptions::new().create(true).append(!fresh).write(true).truncate(fresh).open(&path);
    if let Ok(f) = opened {
        *file = Some(f);
        ON.store(true, Ordering::Relaxed);
        drop(file);
        let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
        note(&format!("log on: DJ Sugar {} on {os} {arch}", env!("CARGO_PKG_VERSION")));
        let drawn = DRAWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        if !drawn.is_empty() {
            note(&format!("drawn by {drawn}"));
        }
    }
}

static DRAWN: Mutex<String> = Mutex::new(String::new());

/// Tells the log what draws the screen, for its first lines.
#[cfg_attr(target_os = "linux", allow(dead_code, reason = "Linux draws through its own window code"))]
pub fn drawn_by(about: &str) {
    about.clone_into(&mut DRAWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    crate::diag(&format!("drawn by {about}"));
}

/// Adds a line, when the log is on.
pub fn note(text: &str) {
    if !on() {
        return;
    }
    #[cfg(target_os = "android")]
    crate::system_log(text);
    keep(text);
}

/// Adds a line to the file alone, when the log is on.
pub fn keep(text: &str) {
    if !on() {
        return;
    }
    let t = START.get_or_init(Instant::now).elapsed();
    let mut file = FILE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(f) = file.as_mut() {
        let _ = writeln!(f, "{:>8.3} {text}", t.as_secs_f64());
    }
}

/// Sends what the services answer and what stops the app to the log.
pub fn hook() {
    START.get_or_init(Instant::now);
    sugar_net::notes_to(note);
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        note(&format!("PANIC {info}"));
        before(info);
    }));
}

/// The end of the log, from the start of a line.
pub fn tail() -> String {
    let text = path().and_then(|p| std::fs::read(p).ok()).unwrap_or_default();
    let from = text.len().saturating_sub(SENT);
    let from = text[from..].iter().position(|b| *b == b'\n').map_or(from, |n| from + n + 1);
    String::from_utf8_lossy(&text[from.min(text.len())..]).into_owned()
}
