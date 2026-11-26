//! Hands a web page or a folder to whatever the system opens it with.

/// The project's page, its guide and where problems are reported.
pub const HOME: &str = "https://github.com/winnerspiros/djsugar/tree/rust";
pub const ISSUES: &str = "https://github.com/winnerspiros/djsugar/issues/new";

/// Opens `what`, a link or a path; false when the system has nothing to open it with.
pub fn open(what: &str) -> bool {
    let tool = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(tool).arg(what).spawn().is_ok()
}
