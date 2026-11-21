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
fn android_main(app: winit::platform::android::activity::AndroidApp) {
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
    let _ = djsugar::ANDROID_APP.set(app);
    if let Err(e) = djsugar::run() {
        log::write(log::ERROR, &format!("DJ Sugar stopped: {e}"));
    }
}
