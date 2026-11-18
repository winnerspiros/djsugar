//! For tests: counts the allocations made on each thread, so a test can
//! check that a stretch of audio-thread code made none (tests run on
//! several threads).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: forwards to the system allocator; the counter is a plain
// thread-local `Cell` with const initialisation, which never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|a| a.set(a.get() + 1));
        // SAFETY: same contract as ours.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as ours.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.with(|a| a.set(a.get() + 1));
        // SAFETY: same contract as ours.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations made on this thread so far.
pub fn allocations() -> usize {
    ALLOCATIONS.with(Cell::get)
}

/// Runs `f` and panics if it allocated on this thread.
pub fn assert_no_alloc<R>(f: impl FnOnce() -> R) -> R {
    let before = allocations();
    let r = f();
    let n = allocations() - before;
    assert_eq!(n, 0, "{n} allocations in the audio callback");
    r
}
