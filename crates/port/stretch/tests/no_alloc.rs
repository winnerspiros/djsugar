//! Processing, seeking and resetting never allocate.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn processing_does_not_allocate() {
    let mut s = djsugar_stretch::Stretch::preset_default_split(2, 48_000);
    let input: Vec<f32> = (0..2 * 200_000).map(|i| ((i / 2) as f32 * 0.03).sin()).collect();
    let mut out = vec![0.0f32; 2 * 512];
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    s.set_transpose_factor(1.3, None);
    s.seek(&input[..2 * 9000], 1.0);
    let mut at = 9000;
    for n in [512, 700, 300, 0, 2048, 512] {
        s.process(&input[2 * at..2 * (at + n)], &mut out);
        at += n;
    }
    s.process(&input[2 * at..2 * (at + 50_000)], &mut out);
    s.reset();
    s.process(&input[..2 * 512], &mut out);
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), before);
}
