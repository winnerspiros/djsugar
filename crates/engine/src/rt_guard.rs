//! Real-time guard: tracks allocations inside the audio callback for testing.
//!
//! In production this is a no-op. Under `cfg(test)` it installs a counting
//! global allocator and a thread-local flag. The test helper
//! [`assert_no_allocs_in_audio`] runs a closure with the flag set and asserts
//! that the allocator saw no allocations or deallocations while the flag was up.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Thread-local flag: true while the current thread is inside the audio callback.
/// Only meaningful under `cfg(test)`.
thread_local! {
    static IN_AUDIO_CALLBACK: Cell<bool> = const { Cell::new(false) };
}

/// Returns true if the current thread is inside the audio callback region.
/// Only meaningful under `cfg(test)`; always false otherwise.
#[inline]
pub fn in_audio_callback() -> bool {
    #[cfg(test)]
    {
        IN_AUDIO_CALLBACK.get()
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Enters the audio callback region on the current thread.
#[inline]
pub fn enter_audio_callback() {
    #[cfg(test)]
    {
        IN_AUDIO_CALLBACK.set(true);
    }
}

/// Exits the audio callback region on the current thread.
#[inline]
pub fn exit_audio_callback() {
    #[cfg(test)]
    {
        IN_AUDIO_CALLBACK.set(false);
    }
}

/// A RAII guard that sets the audio callback flag on creation and clears it on drop.
#[cfg(test)]
pub struct AudioCallbackGuard;

#[cfg(test)]
impl AudioCallbackGuard {
    #[inline]
    pub fn new() -> Self {
        enter_audio_callback();
        Self
    }
}

#[cfg(test)]
impl Drop for AudioCallbackGuard {
    #[inline]
    fn drop(&mut self) {
        exit_audio_callback();
    }
}

/// Counting allocator used only under `cfg(test)`. Wraps the system allocator
/// and increments counters when `IN_AUDIO_CALLBACK` is set.
#[cfg(test)]
struct CountingAlloc;

#[cfg(test)]
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static DEALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static DEALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
unsafe impl GlobalAlloc for CountingAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() && IN_AUDIO_CALLBACK.get() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if IN_AUDIO_CALLBACK.get() {
            DEALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            DEALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        System.dealloc(ptr, layout);
    }
}

/// Global allocator instance, only used under `cfg(test)`.
#[cfg(test)]
#[global_allocator]
static COUNTING_ALLOC: CountingAlloc = CountingAlloc;

/// Resets the allocation counters.
#[cfg(test)]
pub fn reset_alloc_counters() {
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    DEALLOC_COUNT.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    DEALLOC_BYTES.store(0, Ordering::Relaxed);
}

/// Returns the current allocation counts since the last reset.
#[cfg(test)]
pub fn alloc_counts() -> (usize, usize, usize, usize) {
    (
        ALLOC_COUNT.load(Ordering::Relaxed),
        DEALLOC_COUNT.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
        DEALLOC_BYTES.load(Ordering::Relaxed),
    )
}

/// Runs `f` with the audio callback flag set and asserts that no allocations
/// or deallocations occurred while it was set.
///
/// Panics with a detailed message if any allocations or deallocations are detected.
#[cfg(test)]
pub fn assert_no_allocs_in_audio<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    reset_alloc_counters();
    let _guard = AudioCallbackGuard::new();
    let result = f();
    let (allocs, deallocs, alloc_bytes, dealloc_bytes) = alloc_counts();
    if allocs > 0 || deallocs > 0 {
        panic!(
            "allocations detected in audio callback: {} allocs ({} bytes), {} deallocs ({} bytes)",
            allocs, alloc_bytes, deallocs, dealloc_bytes
        );
    }
    result
}

/// Non-test stubs so the crate compiles without the test cfg.
#[cfg(not(test))]
pub fn assert_no_allocs_in_audio<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

#[cfg(not(test))]
pub fn reset_alloc_counters() {}

#[cfg(not(test))]
pub fn alloc_counts() -> (usize, usize, usize, usize) {
    (0, 0, 0, 0)
}

#[cfg(not(test))]
pub struct AudioCallbackGuard;

#[cfg(not(test))]
impl AudioCallbackGuard {
    #[inline]
    pub fn new() -> Self {
        Self
    }
}
