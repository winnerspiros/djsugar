//! The user's folders on each platform: Home, and where an app keeps its
//! settings and caches.

use std::path::PathBuf;

/// The user's home folder: `HOME`, else (on Windows, which doesn't set it)
/// the profile folder, e.g. `C:\Users\dj`.
pub fn home_dir() -> Option<PathBuf> {
    match std::env::var_os("HOME") {
        Some(h) if !h.is_empty() => Some(PathBuf::from(h)),
        _ => std::env::home_dir(),
    }
}

/// Where apps keep their settings: `~/Library/Application Support` on
/// macOS, `%APPDATA%` (Roaming) on Windows, `$XDG_CONFIG_HOME` or
/// `~/.config` elsewhere.
pub fn config_base() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        home_dir().map(|h| h.join("Library/Application Support"))
    } else if cfg!(windows) {
        non_empty("APPDATA").or_else(|| home_dir().map(|h| h.join("AppData").join("Roaming")))
    } else {
        non_empty("XDG_CONFIG_HOME").or_else(|| home_dir().map(|h| h.join(".config")))
    }
}

/// Where apps keep caches: `~/Library/Caches` on macOS, `%LOCALAPPDATA%` on
/// Windows, `$XDG_CACHE_HOME` or `~/.cache` elsewhere.
pub fn cache_base() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        home_dir().map(|h| h.join("Library/Caches"))
    } else if cfg!(windows) {
        non_empty("LOCALAPPDATA").or_else(|| home_dir().map(|h| h.join("AppData").join("Local")))
    } else {
        non_empty("XDG_CACHE_HOME").or_else(|| home_dir().map(|h| h.join(".cache")))
    }
}

/// The name of DJ Sugar's own folder inside [`config_base`] and
/// [`cache_base`]: "DJ Sugar" on macOS and Windows, where folders there are
/// named like the app, and "djsugar" on Linux.
pub fn app_folder_name() -> &'static str {
    if cfg!(any(target_os = "macos", windows)) {
        "DJ Sugar"
    } else {
        "djsugar"
    }
}

/// Expands a leading `~` or `~/` (also `~\` on Windows) to the home folder.
pub fn expand_tilde(path: &str) -> Option<PathBuf> {
    if path == "~" {
        return home_dir();
    }
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\").filter(|_| cfg!(windows)));
    match rest {
        Some(rest) => home_dir().map(|h| h.join(rest)),
        None => Some(PathBuf::from(path)),
    }
}

fn non_empty(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expands_to_home() {
        let home = home_dir().unwrap();
        assert_eq!(expand_tilde("~"), Some(home.clone()));
        assert_eq!(expand_tilde("~/Music/In"), Some(home.join("Music/In")));
        assert_eq!(expand_tilde("/data/x"), Some(PathBuf::from("/data/x")));
    }
}
