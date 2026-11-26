//! Memory use stays bounded. Lives in its own test binary because it installs
//! a counting global allocator, and it holds the only test so that no other
//! test allocates concurrently.

use ex02_lru_cache::LruCache;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE_BYTES.fetch_add(new_size, Ordering::SeqCst);
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live() -> usize {
    LIVE_BYTES.load(Ordering::SeqCst)
}

#[test]
fn memory_stays_bounded_over_a_million_put_and_remove_cycles() {
    const CAPACITY: u64 = 1_000;
    let before = live();
    let mut cache: LruCache<u64, u64> = LruCache::new(CAPACITY as usize).unwrap();
    for k in 0..CAPACITY {
        cache.put(k, k);
    }
    let full = live() - before;

    // Each cycle inserts a fresh key (evicting when full) and removes it again.
    for k in CAPACITY..CAPACITY + 1_000_000 {
        cache.put(k, k);
        assert_eq!(cache.remove(&k), Some(k));
    }
    let after = live() - before;

    eprintln!("live bytes: full cache {full}, after 1,000,000 cycles {after}");
    // Allowed: at most 3x the live bytes of the full cache. That leaves room
    // for a table that resizes while entries come and go, and still fails a
    // leak of 8 bytes per cycle (8,000,000 bytes, over 100x the full cache).
    assert!(after <= full * 3, "live bytes grew from {full} to {after}");
}
