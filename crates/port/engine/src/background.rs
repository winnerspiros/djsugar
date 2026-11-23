//! Work done behind the DJ's back, such as analysing a batch of tracks, runs
//! at low priority and on few cores, so the audio and the UI always come
//! first. While a deck plays it goes gentler still: see [`playing`].

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

static PLAYING: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Whether this thread runs at low priority (see [`lower_priority`]).
    static LOW: Cell<bool> = const { Cell::new(false) };
}

/// How many threads Beat This! (on rten's thread pool) runs on: a quarter
/// of the cores, between 2 and 4, rather than every fast core.
pub fn beat_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |n| n.get() / 4)
        .clamp(2, 4)
}

/// Sizes rten's thread pool (Beat This! runs on it) with [`beat_threads`],
/// unless `RTEN_NUM_THREADS` is set already. rten reads the variable once,
/// when the pool is first used, so this is called first thing in `main`,
/// before any other thread starts.
pub fn configure() {
    if std::env::var_os("RTEN_NUM_THREADS").is_none() {
        std::env::set_var("RTEN_NUM_THREADS", beat_threads().to_string());
    }
}

/// Runs the calling thread at low priority: the utility QoS class on macOS
/// (it may move to the efficiency cores), nice 10 on Linux, below normal on
/// Windows. Threads it starts afterwards should call it too (see
/// [`is_low`]): macOS doesn't pass the class on to them.
pub fn lower_priority() {
    LOW.with(|l| l.set(true));
    #[cfg(target_os = "macos")]
    // SAFETY: only changes the calling thread's scheduling.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    // SAFETY: as above; on Linux the nice value belongs to each thread.
    unsafe {
        let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
        libc::setpriority(libc::PRIO_PROCESS, tid, 10);
    }
    #[cfg(windows)]
    // SAFETY: as above.
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

/// Whether the calling thread was put at low priority with
/// [`lower_priority`].
pub fn is_low() -> bool {
    LOW.with(Cell::get)
}

/// Puts each of rten's threads (Beat This! runs on them) at low priority,
/// once. Called from a low-priority thread: Beat This! on a deck's track
/// then waits a little more too, which the DSP grid covers meanwhile.
pub fn lower_rten_priority() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        rten::thread_pool().run(|| {
            rayon::broadcast(|_| lower_priority());
        });
    });
}

/// Tells background work whether a deck is playing; the UI calls it as
/// the decks start and stop.
pub fn set_playing(on: bool) {
    PLAYING.store(on, Ordering::Relaxed);
}

/// Whether a deck is playing: background work then takes one core and
/// rests between tracks.
pub fn playing() -> bool {
    PLAYING.load(Ordering::Relaxed)
}

/// How many threads background work may share a step out to, from `most`:
/// one while a deck plays.
pub fn threads(most: usize) -> usize {
    if is_low() && playing() {
        1
    } else {
        most.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_priority_is_per_thread() {
        std::thread::spawn(|| {
            lower_priority();
            assert!(is_low());
            assert_eq!(threads(4), 4);
        })
        .join()
        .unwrap();
        assert!(!is_low());
    }

    #[test]
    fn beat_this_takes_a_few_cores() {
        assert!((2..=4).contains(&beat_threads()));
    }
}
