//! Memory use measured from outside with a counting global allocator.
//! This file holds a single test so no other thread allocates while it measures.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use ex01_rate_limiter::KeyedLimiter;

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size, Ordering::Relaxed);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

// Tracking every key takes about 100 MB here; 1000 tracked keys take well under 1 MB.
#[test]
fn memory_stays_bounded_when_keys_keep_changing() {
    let before = LIVE.load(Ordering::Relaxed);
    let mut limiter = KeyedLimiter::new(1, 1, 1000).unwrap();
    for key in 0..1_000_000u64 {
        assert!(limiter.check(&key, 0));
    }
    let used = LIVE.load(Ordering::Relaxed).saturating_sub(before);
    assert!(
        used < 4 * 1024 * 1024,
        "{used} bytes live after 1,000,000 keys"
    );
}
