//! DJ Sugar's Android entry point: a NativeActivity running the app.
//!
//! Android drops what a native app writes to stdout and stderr, so both are
//! piped into the system log (`adb logcat -s DJSugar`), with panics, and
//! the app gets its own storage as its home folder.

#[cfg(target_os = "android")]
mod log {
    use std::ffi::{c_char, c_int, CString};
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }
    extern "C" {
        fn pipe(fds: *mut c_int) -> c_int;
        fn dup2(old: c_int, new: c_int) -> c_int;
    }

    pub const INFO: c_int = 4;
    pub const ERROR: c_int = 6;

    pub fn write(prio: c_int, line: &str) {
        if let Ok(text) = CString::new(line.replace('\0', " ")) {
            // SAFETY: both are valid NUL-terminated strings for the call.
            unsafe { __android_log_write(prio, c"DJSugar".as_ptr(), text.as_ptr()) };
        }
    }

    /// Sends stdout and stderr, and panics, to the system log.
    pub fn install() {
        let mut fds = [0 as c_int; 2];
        // SAFETY: `fds` has room for the two ends pipe() writes.
        if unsafe { pipe(fds.as_mut_ptr()) } == 0 {
            // SAFETY: the write end replaces fds 1 and 2; both stay open.
            let (out, err) = unsafe { (dup2(fds[1], 1), dup2(fds[1], 2)) };
            if out < 0 || err < 0 {
                write(ERROR, "could not send stdout and stderr to the log");
            }
            // SAFETY: the read end is owned by this File alone from here.
            let read = unsafe { std::fs::File::from_raw_fd(fds[0]) };
            let _ = std::thread::Builder::new()
                .name("logcat".into())
                .spawn(move || {
                    for line in BufReader::new(read).lines().map_while(Result::ok) {
                        write(INFO, &line);
                    }
                });
        }
        std::panic::set_hook(Box::new(|info| {
            write(ERROR, &format!("DJ Sugar crashed: {info}"));
        }));
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: slint::android::AndroidApp) {
    log::install();
    log::write(log::INFO, "DJ Sugar starting");
    eprintln!("stderr reaches the log");
    // Until the first frame: shows where a slow or stuck start is.
    let _ = std::thread::Builder::new()
        .name("heartbeat".into())
        .spawn(|| {
            for n in 1..=30 {
                std::thread::sleep(std::time::Duration::from_secs(10));
                log::write(log::INFO, &format!("still running, {}s", n * 10));
            }
        });
    // No HOME on Android: settings, the library and downloads go in the
    // app's own storage.
    if let Some(dir) = app.internal_data_path() {
        // Before any other thread of the app starts.
        std::env::set_var("HOME", &dir);
        std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
        std::env::set_var("XDG_DATA_HOME", dir.join("data"));
    }
    unpack_controllers(&app);
    if let Err(e) = slint::android::init(app) {
        log::write(log::ERROR, &format!("DJ Sugar stopped: {e}"));
        return;
    }
    if let Err(e) = djsugar::run() {
        log::write(log::ERROR, &format!("DJ Sugar stopped: {e}"));
    }
}

/// Copies the controller mappings out of the APK into the app's storage,
/// once per version, and points the app at them (`DJSUGAR_CONTROLLERS`).
#[cfg(target_os = "android")]
fn unpack_controllers(app: &slint::android::AndroidApp) {
    use std::io::Read;
    let Some(base) = app.internal_data_path() else {
        return;
    };
    let dir = base.join("controllers");
    let stamp = dir.join(".version");
    let version = env!("CARGO_PKG_VERSION");
    let assets = app.asset_manager();
    let read = |name: &str| -> Option<Vec<u8>> {
        let name = std::ffi::CString::new(format!("controllers/{name}")).ok()?;
        let mut bytes = Vec::new();
        assets.open(&name)?.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    };
    if std::fs::read_to_string(&stamp).ok().as_deref() != Some(version) {
        let Some(index) = read("index.txt") else {
            log::write(log::ERROR, "controller mappings missing from the APK");
            return;
        };
        let _ = std::fs::remove_dir_all(&dir);
        let mut copied = 0;
        for name in String::from_utf8_lossy(&index)
            .lines()
            .filter(|l| !l.is_empty())
        {
            let path = dir.join(name);
            let ok = read(name).is_some_and(|bytes| {
                path.parent()
                    .is_some_and(|p| std::fs::create_dir_all(p).is_ok())
                    && std::fs::write(&path, bytes).is_ok()
            });
            copied += usize::from(ok);
        }
        let _ = std::fs::write(&stamp, version);
        log::write(
            log::INFO,
            &format!("controller mappings unpacked: {copied} files"),
        );
    }
    std::env::set_var("DJSUGAR_CONTROLLERS", &dir);
}
