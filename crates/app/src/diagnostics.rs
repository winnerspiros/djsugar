//! Logs and crash reports, so there is something to attach to a bug report.
//!
//! When DJ Sugar isn't started from a terminal (the macOS app, a desktop
//! launcher), everything it prints goes to `logs/djsugar.log` in the
//! settings folder, and the previous run's log is kept as `djsugar.1.log`.
//! A panic on any thread also writes `logs/crash-<time>.txt` with the
//! backtrace.

use std::backtrace::Backtrace;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY: &str = "https://github.com/winnerspiros/djsugar";

pub fn logs_dir() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("logs"))
}

/// Starts logging and installs the crash handler. Call first thing.
pub fn install() {
    let Some(dir) = logs_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    if !std::io::stderr().is_terminal() {
        redirect_stderr(&dir);
    }
    eprintln!("DJ Sugar {VERSION} on {}", platform());

    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let thread = std::thread::current();
        let report = format!(
            "DJ Sugar {VERSION} on {}\nThread: {}\n\n{info}\n\n{}\n",
            platform(),
            thread.name().unwrap_or("unnamed"),
            Backtrace::force_capture(),
        );
        let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
        let path = dir.join(format!("crash-{stamp}.txt"));
        if let Ok(mut file) = std::fs::File::create(&path) {
            let _ = file.write_all(report.as_bytes());
            eprintln!("crash report written to {}", path.display());
        }
    }));
}

/// Keeps the last run's log and sends stderr to a new one.
#[cfg(unix)]
fn redirect_stderr(dir: &Path) {
    use std::os::fd::AsRawFd;
    let log = dir.join("djsugar.log");
    let _ = std::fs::rename(&log, dir.join("djsugar.1.log"));
    let Ok(file) = std::fs::File::create(&log) else {
        return;
    };
    // SAFETY: dup2 onto stderr with a descriptor that is open; the file is
    // leaked below so the descriptor stays valid for the whole run.
    if unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) } >= 0 {
        std::mem::forget(file);
    }
}

/// Keeps the last run's log and sends stderr to a new one. Rust looks the
/// stderr handle up on every write, so swapping it is enough.
#[cfg(windows)]
fn redirect_stderr(dir: &Path) {
    use std::os::windows::io::IntoRawHandle;
    use windows_sys::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE};
    let log = dir.join("djsugar.log");
    let _ = std::fs::rename(&log, dir.join("djsugar.1.log"));
    let Ok(file) = std::fs::File::create(&log) else {
        return;
    };
    // SAFETY: the handle is open and, taken out of the File, stays open for
    // the whole run.
    unsafe {
        SetStdHandle(STD_ERROR_HANDLE, file.into_raw_handle());
    }
}

#[cfg(not(any(unix, windows)))]
fn redirect_stderr(_dir: &Path) {}

/// "macOS aarch64", "Linux x86_64 (Ubuntu 24.04)" and so on.
pub fn platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macOS".to_string(),
        "linux" => "Linux".to_string(),
        "windows" => "Windows".to_string(),
        other => other.to_string(),
    };
    let distro = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| format!(" ({})", v.trim_matches('"')))
        })
        .unwrap_or_default();
    format!("{os} {}{distro}", std::env::consts::ARCH)
}

/// A new GitHub issue from one of the templates, with the version and
/// system filled in.
pub fn issue_url(template: &str) -> String {
    let encode = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!(
        "{REPOSITORY}/issues/new?template={template}&version={}&system={}",
        encode(VERSION),
        encode(&platform())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_links_carry_the_version() {
        let url = issue_url("bug_report.yml");
        assert!(url.starts_with(
            "https://github.com/winnerspiros/djsugar/issues/new?template=bug_report.yml&version="
        ));
        assert!(url.contains(VERSION));
        assert!(!url.contains(' '));
    }
}
