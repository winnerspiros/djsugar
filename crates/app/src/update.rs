//! Checks GitHub once per start for a newer release, so the top of the
//! window can say so. Pre-releases are skipped (GitHub's "latest" never
//! returns them), and a version dismissed once isn't shown again.
//!
//! The macOS app, the AppImage and a Windows install made by the setup can
//! install the update themselves: the download must match the checksum
//! GitHub publishes for it, and on macOS it must also be notarized and
//! signed by the same developer as the running app (on Windows, once
//! DJ Sugar is signed, by the same publisher). The old app is swapped out,
//! or on Windows the new setup runs silently, and DJ Sugar starts again.
//! A .deb or .rpm install gets a link to the new package, which the
//! system's software installer opens; a Flatpak doesn't check at all, as
//! Flathub updates it. Other installs (the Linux tarball, the Windows zip,
//! a build from source) get the release page.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use sha2::{Digest, Sha256};

use serde_json::Value;

use crate::diagnostics::VERSION;
use crate::settings;

const LATEST: &str = "https://api.github.com/repos/winnerspiros/djsugar/releases/latest";

/// A release newer than this build.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    /// The release page, with the notes and downloads.
    pub url: String,
    /// The download this install can update itself from.
    pub asset: Option<Asset>,
    /// For a .deb or .rpm install: the new package to download and open.
    pub package: Option<Package>,
}

/// A Linux package in a release.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    /// ".deb" or ".rpm", for the banner's button.
    pub kind: &'static str,
    pub url: String,
}

/// How DJ Sugar was installed on Linux, when a package manager did it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SystemPackage {
    Deb,
    Rpm,
}

impl SystemPackage {
    fn kind(self) -> &'static str {
        match self {
            SystemPackage::Deb => ".deb",
            SystemPackage::Rpm => ".rpm",
        }
    }

    /// Whether `name` is this kind of package for this computer, as
    /// scripts/bundle-linux.sh names them: djsugar_0.5.0_amd64.deb and
    /// djsugar-0.5.0-1.x86_64.rpm.
    fn wants(self, name: &str) -> bool {
        let arch = std::env::consts::ARCH;
        match self {
            SystemPackage::Deb => {
                let deb_arch = match arch {
                    "x86_64" => "amd64",
                    "aarch64" => "arm64",
                    other => other,
                };
                name.starts_with("djsugar_") && name.ends_with(&format!("_{deb_arch}.deb"))
            }
            SystemPackage::Rpm => {
                name.starts_with("djsugar-") && name.ends_with(&format!(".{arch}.rpm"))
            }
        }
    }
}

/// The package manager that put this DJ Sugar in /usr/bin, if any.
fn system_package() -> Option<SystemPackage> {
    if !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some() {
        return None;
    }
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    if exe != Path::new("/usr/bin/djsugar") {
        return None;
    }
    if Path::new("/var/lib/dpkg/info/djsugar.list").is_file() {
        return Some(SystemPackage::Deb);
    }
    let out = Command::new("rpm")
        .args(["-qf", "--queryformat", "%{NAME}", "/usr/bin/djsugar"])
        .output()
        .ok()?;
    (out.status.success() && out.stdout == b"djsugar").then_some(SystemPackage::Rpm)
}

/// Inside a Flatpak, where Flathub brings the updates.
fn in_flatpak() -> bool {
    cfg!(target_os = "linux") && Path::new("/.flatpak-info").exists()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub url: String,
    /// "sha256:<hex>", as GitHub lists it.
    pub digest: Option<String>,
}

/// What this copy of DJ Sugar is, as far as updating goes.
#[derive(Debug, Clone, PartialEq)]
enum Install {
    /// The DJSugar.app bundle at this path.
    MacApp(PathBuf),
    /// The AppImage file at this path.
    AppImage(PathBuf),
    /// Installed by the Windows setup, which left its uninstaller next to
    /// DJSugar.exe at this path.
    WindowsSetup(PathBuf),
}

fn this_install() -> Option<Install> {
    if cfg!(target_os = "macos") {
        // .../DJSugar.app/Contents/MacOS/djsugar
        let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
        let app = exe.parent()?.parent()?.parent()?;
        (app.extension()? == "app").then(|| Install::MacApp(untranslocated(app)))
    } else if cfg!(windows) {
        let exe = std::env::current_exe().ok()?;
        exe.with_file_name("unins000.exe")
            .is_file()
            .then_some(Install::WindowsSetup(exe))
    } else {
        std::env::var_os("APPIMAGE").map(|p| Install::AppImage(p.into()))
    }
}

/// Where the app really is. macOS runs a quarantined app that wasn't moved
/// by the Finder (still in Downloads, say) from a read-only copy under
/// AppTranslocation, which can't be replaced; the update goes to the
/// original instead. If that can't be found, it goes to /Applications.
fn untranslocated(app: &Path) -> PathBuf {
    if !is_translocated(app) {
        return app.to_path_buf();
    }
    #[cfg(target_os = "macos")]
    if let Some(original) = mac::original_path(app) {
        return original;
    }
    PathBuf::from("/Applications").join(
        app.file_name()
            .unwrap_or(std::ffi::OsStr::new("DJSugar.app")),
    )
}

fn is_translocated(app: &Path) -> bool {
    app.components()
        .any(|c| c.as_os_str() == "AppTranslocation")
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_void, CString, OsStr};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    type CFURLRef = *const c_void;
    type Original = unsafe extern "C" fn(CFURLRef, *mut *const c_void) -> CFURLRef;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: *const c_void,
            buffer: *const u8,
            length: isize,
            is_directory: u8,
        ) -> CFURLRef;
        fn CFURLGetFileSystemRepresentation(
            url: CFURLRef,
            resolve_against_base: u8,
            buffer: *mut u8,
            max_length: isize,
        ) -> u8;
        fn CFRelease(cf: *const c_void);
    }

    /// The path a translocated app was opened from, from the Security
    /// framework's SecTranslocateCreateOriginalPathForURL (looked up at
    /// run time, as it isn't in the public headers).
    pub(super) fn original_path(app: &Path) -> Option<PathBuf> {
        let lib = CString::new("/System/Library/Frameworks/Security.framework/Security").ok()?;
        let name = CString::new("SecTranslocateCreateOriginalPathForURL").ok()?;
        let bytes = app.as_os_str().as_bytes();
        // SAFETY: plain C calls; every CF object created here is released.
        unsafe {
            let handle = libc::dlopen(lib.as_ptr(), libc::RTLD_LAZY);
            if handle.is_null() {
                return None;
            }
            let sym = libc::dlsym(handle, name.as_ptr());
            if sym.is_null() {
                return None;
            }
            let original: Original = std::mem::transmute(sym);
            let url = CFURLCreateFromFileSystemRepresentation(
                std::ptr::null(),
                bytes.as_ptr(),
                bytes.len() as isize,
                1,
            );
            if url.is_null() {
                return None;
            }
            let out = original(url, std::ptr::null_mut());
            CFRelease(url);
            if out.is_null() {
                return None;
            }
            let mut buf = vec![0u8; libc::PATH_MAX as usize * 4];
            let ok = CFURLGetFileSystemRepresentation(out, 1, buf.as_mut_ptr(), buf.len() as isize);
            CFRelease(out);
            if ok == 0 {
                return None;
            }
            let len = buf.iter().position(|&b| b == 0)?;
            let path = PathBuf::from(OsStr::from_bytes(&buf[..len]));
            (path != app && !super::is_translocated(&path)).then_some(path)
        }
    }
}

/// The asset name this install updates from.
fn wants(install: &Install, name: &str) -> bool {
    match install {
        Install::MacApp(_) => name.ends_with("-macos.zip"),
        Install::AppImage(_) => name.ends_with(&format!("-{}.AppImage", std::env::consts::ARCH)),
        Install::WindowsSetup(_) => name.ends_with("-windows-x64-setup.exe"),
    }
}

/// Starts the check in the background, unless it is turned off in
/// Settings. The answer comes once, or never if there's nothing new.
pub fn check() -> Receiver<Release> {
    let (tx, rx) = channel();
    if settings::flag("update_check", true) && !in_flatpak() {
        std::thread::spawn(move || {
            if let Some(r) = latest().filter(|r| !dismissed(&r.version)) {
                let _ = tx.send(r);
            }
        });
    }
    rx
}

/// Hides the banner for this version; the next one shows again.
pub fn dismiss(version: &str) {
    settings::set("update_dismissed", serde_json::json!(version));
}

fn dismissed(version: &str) -> bool {
    settings::string("update_dismissed") == version
}

fn latest() -> Option<Release> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent(format!("DJSugar/{VERSION}"))
        .build()
        .into();
    let body: Value = agent
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| eprintln!("update check: {e}"))
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    newer(&body, VERSION, this_install().as_ref(), system_package())
}

/// The release in GitHub's answer, if it is newer than `current`.
fn newer(
    release: &Value,
    current: &str,
    install: Option<&Install>,
    system: Option<SystemPackage>,
) -> Option<Release> {
    let tag = release["tag_name"].as_str()?;
    let version = tag.trim_start_matches('v');
    if release["draft"].as_bool() == Some(true)
        || release["prerelease"].as_bool() == Some(true)
        || !is_newer(version, current)
    {
        return None;
    }
    let asset = install.and_then(|install| {
        release["assets"].as_array()?.iter().find_map(|a| {
            if !wants(install, a["name"].as_str()?) {
                return None;
            }
            Some(Asset {
                url: a["browser_download_url"].as_str()?.to_string(),
                digest: a["digest"].as_str().map(str::to_string),
            })
        })
    });
    let package = system.and_then(|system| {
        release["assets"].as_array()?.iter().find_map(|a| {
            if !system.wants(a["name"].as_str()?) {
                return None;
            }
            Some(Package {
                kind: system.kind(),
                url: a["browser_download_url"].as_str()?.to_string(),
            })
        })
    });
    Some(Release {
        version: version.to_string(),
        url: release["html_url"].as_str()?.to_string(),
        asset,
        package,
    })
}

/// News from an install running in the background.
pub enum Progress {
    /// What the banner says meanwhile.
    Status(String),
    /// The new version is in place: quit, and it starts.
    Done,
    /// Nothing changed; the reason for the alert.
    Failed(String),
}

/// Downloads, checks and puts the new version in place of this one.
pub fn install(release: Release) -> Receiver<Progress> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = (|| {
            let install = this_install().ok_or("this copy of DJ Sugar can't update itself")?;
            let asset = release
                .asset
                .as_ref()
                .ok_or("no download for this system")?;
            run(&install, asset, &tx)
        })();
        let _ = tx.send(match result {
            Ok(()) => Progress::Done,
            Err(e) => {
                eprintln!("update to {}: {e}", release.version);
                Progress::Failed(format!(
                    "Couldn't install DJ Sugar {}: {e}. The release page has the download.",
                    release.version
                ))
            }
        });
    });
    rx
}

fn run(install: &Install, asset: &Asset, tx: &Sender<Progress>) -> Result<(), String> {
    let dir = crate::settings::config_dir()
        .ok_or("no settings folder")?
        .join("updates");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("download");
    let _ = tx.send(Progress::Status("Downloading…".into()));
    download(&asset.url, &file)?;
    let _ = tx.send(Progress::Status("Checking…".into()));
    check_digest(&file, asset.digest.as_deref())?;
    match install {
        Install::MacApp(app) => {
            let unpacked = dir.join("new");
            command(
                Command::new("ditto")
                    .args(["-x", "-k"])
                    .arg(&file)
                    .arg(&unpacked),
            )?;
            let new_app = unpacked.join("DJSugar.app");
            check_signature(&new_app, app)?;
            let _ = tx.send(Progress::Status("Installing…".into()));
            swap_app(app, &new_app)
        }
        Install::WindowsSetup(exe) => {
            check_publisher(&file, exe)?;
            // Runs once DJ Sugar has quit (see `relaunch`), so it's kept.
            std::fs::rename(&file, windows_setup(&dir)).map_err(|e| e.to_string())?;
            return Ok(());
        }
        Install::AppImage(path) => {
            let _ = tx.send(Progress::Status("Installing…".into()));
            // Next to the old one, so the rename can't cross disks.
            let staged = path.with_extension("AppImage.new");
            std::fs::copy(&file, &staged).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
            std::fs::rename(&staged, path).map_err(|e| {
                let _ = std::fs::remove_file(&staged);
                e.to_string()
            })
        }
    }
    .map(|()| {
        let _ = std::fs::remove_dir_all(&dir);
    })
}

pub(crate) fn download(url: &str, to: &Path) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15 * 60)))
        .user_agent(format!("DJSugar/{VERSION}"))
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    let mut reader = response.body_mut().with_config().limit(1 << 30).reader();
    let mut out = std::fs::File::create(to).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut out).map_err(|e| e.to_string())?;
    Ok(())
}

/// The file must match the checksum GitHub published; a release without
/// one isn't installed.
/// The file's digest with hash `D`, in lowercase hex.
fn hash_file<D: Digest>(file: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut hasher = D::new();
    let mut buf = vec![0; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

pub(crate) fn check_digest(file: &Path, digest: Option<&str>) -> Result<(), String> {
    let digest = digest.ok_or("the release has no checksum")?;
    // SHA-256 as GitHub gives it; SHA-1 for a model pinned by its maker's
    // SHA-1 (Whisper's, as whisper.cpp publishes them).
    let (want, got) = if let Some(want) = digest.strip_prefix("sha256:") {
        (want, hash_file::<Sha256>(file)?)
    } else if let Some(want) = digest.strip_prefix("sha1:") {
        (want, hash_file::<sha1::Sha1>(file)?)
    } else {
        return Err("the release has no checksum".into());
    };
    if got.eq_ignore_ascii_case(want) {
        Ok(())
    } else {
        Err("the download is damaged (checksum mismatch)".into())
    }
}

/// The new app must be notarized, and signed by the same team as this
/// one when this one is signed (an ad-hoc build can't vouch for anyone).
fn check_signature(new_app: &Path, running: &Path) -> Result<(), String> {
    command(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(new_app),
    )
    .map_err(|_| "the new app's signature is broken".to_string())?;
    command(
        Command::new("spctl")
            .args(["--assess", "--type", "execute"])
            .arg(new_app),
    )
    .map_err(|_| "the new app isn't notarized".to_string())?;
    let new_team = team(new_app).ok_or("the new app has no developer signature")?;
    match team(running) {
        Some(ours) if ours != new_team => Err("the new app is signed by someone else".into()),
        _ => Ok(()),
    }
}

/// Where a downloaded Windows setup waits for DJ Sugar to quit.
fn windows_setup(updates: &Path) -> PathBuf {
    updates.join("DJSugar-setup.exe")
}

/// When the running DJSugar.exe is signed, the new setup must have a valid
/// signature from the same publisher. Unsigned builds (until the signing
/// certificate arrives) rely on the checksum alone.
fn check_publisher(setup: &Path, running: &Path) -> Result<(), String> {
    let Some(ours) = publisher(running).filter(|(_, valid)| *valid) else {
        return Ok(());
    };
    match publisher(setup) {
        Some((theirs, true)) if theirs == ours.0 => Ok(()),
        Some((_, true)) => Err("the new setup is signed by someone else".into()),
        _ => Err("the new setup isn't signed".into()),
    }
}

/// Who signed a Windows program and whether the signature is valid, from
/// PowerShell's Get-AuthenticodeSignature.
fn publisher(file: &Path) -> Option<(String, bool)> {
    let script = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath '{}'; \
         \"$($s.Status)|$($s.SignerCertificate.Subject)\"",
        file.display().to_string().replace('\'', "''")
    );
    let out = djsugar_engine::process::background(Command::new("powershell"))
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (status, subject) = text.trim().split_once('|')?;
    (!subject.is_empty()).then(|| (subject.to_string(), status == "Valid"))
}

/// The TeamIdentifier from `codesign -dv`, if there is one.
fn team(app: &Path) -> Option<String> {
    let out = Command::new("codesign").arg("-dv").arg(app).output().ok()?;
    team_in(&String::from_utf8_lossy(&out.stderr))
}

fn team_in(codesign_output: &str) -> Option<String> {
    codesign_output
        .lines()
        .find_map(|l| l.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "not set")
        .map(str::to_string)
}

/// Moves the running app aside, puts the new one in its place and removes
/// the old one; puts the old one back if the move fails.
fn swap_app(app: &Path, new_app: &Path) -> Result<(), String> {
    let parent = app.parent().ok_or("no folder")?;
    let old = parent.join(".DJSugar-previous.app");
    let _ = std::fs::remove_dir_all(&old);
    // The app may not be there when a translocated copy is running and its
    // original couldn't be found: the update then goes in fresh.
    let existed = app.exists();
    if existed {
        std::fs::rename(app, &old).map_err(|e| format!("can't replace {} ({e})", app.display()))?;
    }
    // A rename where it can, a copy across disks.
    let placed = std::fs::rename(new_app, app).is_ok()
        || command(Command::new("ditto").arg(new_app).arg(app)).is_ok();
    if !placed {
        let _ = std::fs::remove_dir_all(app);
        if existed {
            let _ = std::fs::rename(&old, app);
        }
        return Err(format!("can't write to {}", parent.display()));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

fn command(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Starts the new version once this process has quit.
pub fn relaunch() {
    let start = match this_install() {
        None => return,
        Some(Install::WindowsSetup(_)) => return run_windows_setup(),
        Some(Install::MacApp(app)) => format!("open {}", shell_quote(&app)),
        Some(Install::AppImage(path)) => format!("{} >/dev/null 2>&1 &", shell_quote(&path)),
    };
    let script = format!(
        "while kill -0 {} 2>/dev/null; do sleep 0.2; done; {start}",
        std::process::id()
    );
    let _ = Command::new("/bin/sh").arg("-c").arg(script).spawn();
}

/// Starts the downloaded setup, which closes what's left of this process,
/// installs over it without asking and, given /RELAUNCH, starts DJ Sugar
/// again.
fn run_windows_setup() {
    let Some(dir) = crate::settings::config_dir() else {
        return;
    };
    let _ = Command::new(windows_setup(&dir.join("updates")))
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
            "/RELAUNCH",
        ])
        .spawn();
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

/// Compares "major.minor.patch". A build of a pre-release such as
/// 0.3.0-beta.1 counts as older than 0.3.0 itself.
fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> Option<([u64; 3], bool)> {
        let (numbers, pre) = match v.split_once('-') {
            Some((n, _)) => (n, true),
            None => (v, false),
        };
        let mut out = [0; 3];
        for (slot, part) in out.iter_mut().zip(numbers.split('.')) {
            *slot = part.parse().ok()?;
        }
        Some((out, pre))
    }
    match (parts(candidate), parts(current)) {
        (Some((a, a_pre)), Some((b, b_pre))) => a > b || (a == b && b_pre && !a_pre),
        _ => false,
    }
}

/// Where the stem separation and beat detection models are published:
/// assets of the `stem-models` pre-release (so the updater and the website,
/// which look at the latest release, skip it).
const MODELS_URL: &str = "https://github.com/winnerspiros/djsugar/releases/download/stem-models";

/// Lets the engine download a model (stem separation, Beat This!, Whisper)
/// the first time it is needed, checked against the hash the engine knows
/// it by: SHA-256, or SHA-1 for the 40-digit hashes of Whisper models.
pub fn install_model_fetch() {
    djsugar_engine::separation::set_fetch(Box::new(|name, hash, to| {
        download(&format!("{MODELS_URL}/{name}"), to)?;
        let kind = if hash.len() == 40 { "sha1" } else { "sha256" };
        check_digest(to, Some(&format!("{kind}:{hash}")))
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.5"));
        assert!(is_newer("1.0.0", "0.99.0"));
        assert!(is_newer("0.3.0", "0.3.0-beta.1"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.3.0"));
        assert!(!is_newer("0.3.0-beta.2", "0.3.0"));
        assert!(!is_newer("garbage", "0.2.0"));
    }

    #[test]
    fn reads_githubs_answer() {
        let release = json!({
            "tag_name": "v0.3.0",
            "html_url": "https://github.com/winnerspiros/djsugar/releases/tag/v0.3.0",
            "draft": false,
            "prerelease": false,
        });
        assert_eq!(
            newer(&release, "0.2.0", None, None),
            Some(Release {
                version: "0.3.0".into(),
                url: "https://github.com/winnerspiros/djsugar/releases/tag/v0.3.0".into(),
                asset: None,
                package: None,
            })
        );
        assert_eq!(newer(&release, "0.3.0", None, None), None);
        let mut pre = release.clone();
        pre["prerelease"] = json!(true);
        assert_eq!(newer(&pre, "0.2.0", None, None), None);
    }

    #[test]
    fn picks_the_download_for_this_install() {
        let release = json!({
            "tag_name": "v0.3.0",
            "html_url": "https://example.com/r",
            "assets": [
                {"name": "DJSugar-0.3.0-macos.zip", "browser_download_url": "https://example.com/mac", "digest": "sha256:ab"},
                {"name": "DJSugar-0.3.0-x86_64.AppImage", "browser_download_url": "https://example.com/appimage"},
                {"name": "djsugar-0.3.0-linux-x86_64.tar.gz", "browser_download_url": "https://example.com/tar"},
                {"name": "djsugar-0.3.0-windows-x64.zip", "browser_download_url": "https://example.com/zip"},
                {"name": "DJSugar-0.3.0-windows-x64-setup.exe", "browser_download_url": "https://example.com/setup"},
            ],
        });
        let mac = newer(
            &release,
            "0.2.0",
            Some(&Install::MacApp("/A/DJSugar.app".into())),
            None,
        );
        assert_eq!(
            mac.unwrap().asset,
            Some(Asset {
                url: "https://example.com/mac".into(),
                digest: Some("sha256:ab".into()),
            })
        );
        let appimage = newer(
            &release,
            "0.2.0",
            Some(&Install::AppImage("/x.AppImage".into())),
            None,
        );
        let expect_appimage = std::env::consts::ARCH == "x86_64";
        assert_eq!(appimage.unwrap().asset.is_some(), expect_appimage);
        let windows = newer(
            &release,
            "0.2.0",
            Some(&Install::WindowsSetup("C:/DJSugar/DJSugar.exe".into())),
            None,
        );
        assert_eq!(
            windows.unwrap().asset.map(|a| a.url).as_deref(),
            Some("https://example.com/setup")
        );
    }

    #[test]
    fn links_the_package_for_a_deb_or_rpm_install() {
        let release = json!({
            "tag_name": "v0.5.0",
            "html_url": "https://example.com/r",
            "assets": [
                {"name": "djsugar_0.5.0_amd64.deb", "browser_download_url": "https://example.com/deb"},
                {"name": "djsugar_0.5.0_arm64.deb", "browser_download_url": "https://example.com/deb-arm"},
                {"name": "djsugar-0.5.0-1.x86_64.rpm", "browser_download_url": "https://example.com/rpm"},
                {"name": "djsugar-0.5.0-1.aarch64.rpm", "browser_download_url": "https://example.com/rpm-arm"},
                {"name": "djsugar-0.5.0-linux-x86_64.tar.gz", "browser_download_url": "https://example.com/tar"},
            ],
        });
        let deb = newer(&release, "0.4.6", None, Some(SystemPackage::Deb)).unwrap();
        let rpm = newer(&release, "0.4.6", None, Some(SystemPackage::Rpm)).unwrap();
        assert_eq!(deb.asset, None);
        let (deb_url, rpm_url) = match std::env::consts::ARCH {
            "x86_64" => (
                Some("https://example.com/deb"),
                Some("https://example.com/rpm"),
            ),
            "aarch64" => (
                Some("https://example.com/deb-arm"),
                Some("https://example.com/rpm-arm"),
            ),
            _ => (None, None),
        };
        assert_eq!(deb.package.as_ref().map(|p| p.url.as_str()), deb_url);
        assert_eq!(rpm.package.as_ref().map(|p| p.url.as_str()), rpm_url);
        if let Some(p) = &rpm.package {
            assert_eq!(p.kind, ".rpm");
        }
        assert_eq!(newer(&release, "0.4.6", None, None).unwrap().package, None);
    }

    #[test]
    fn checks_the_digest() {
        let dir = std::env::temp_dir().join(format!("djsugar-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("f");
        std::fs::write(&file, b"abc").unwrap();
        let abc = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(check_digest(&file, Some(abc)).is_ok());
        assert!(check_digest(&file, Some("sha256:00")).is_err());
        assert!(check_digest(&file, None).is_err());
        let sha1 = "sha1:a9993e364706816aba3e25717850c26c9cd0d89d";
        assert!(check_digest(&file, Some(sha1)).is_ok());
        assert!(check_digest(&file, Some("sha1:00")).is_err());
        assert!(check_digest(&file, Some("md5:900150983cd24fb0d6963f7d28e17f72")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_the_team() {
        let signed = "Executable=/A/DJSugar.app/Contents/MacOS/djsugar\nAuthority=Developer ID Application: Example (AB12CD34EF)\nTeamIdentifier=AB12CD34EF\n";
        assert_eq!(team_in(signed).as_deref(), Some("AB12CD34EF"));
        assert_eq!(team_in("Signature=adhoc\nTeamIdentifier=not set\n"), None);
    }

    #[test]
    fn spots_a_translocated_app() {
        let moved =
            Path::new("/private/var/folders/q2/x/T/AppTranslocation/2C83-7F26/d/DJSugar.app");
        assert!(is_translocated(moved));
        assert!(!is_translocated(Path::new("/Applications/DJSugar.app")));
        assert_eq!(
            untranslocated(Path::new("/Applications/DJSugar.app")),
            Path::new("/Applications/DJSugar.app")
        );
        if !cfg!(target_os = "macos") {
            assert_eq!(untranslocated(moved), Path::new("/Applications/DJSugar.app"));
        }
    }

    #[test]
    fn quotes_paths_for_the_shell() {
        assert_eq!(
            shell_quote(Path::new("/a b/it's.app")),
            "'/a b/it'\\''s.app'"
        );
    }
}
