use ex09_fast_log_aggregation::aggregate;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

static ALLOCS: AtomicU64 = AtomicU64::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn aggregate_makes_no_allocations() {
    let mut data = Vec::new();
    for i in 0..100_000u64 {
        if i % 50 == 0 {
            data.extend_from_slice(b"garbage\n");
        } else {
            let line = format!("/p/{} {} {} {}\n", i % 97, 200 + i % 4 * 100, i, i % 1000);
            data.extend_from_slice(line.as_bytes());
        }
    }

    let before = ALLOCS.load(Relaxed);
    let s = aggregate(&data);
    let after = ALLOCS.load(Relaxed);

    assert_eq!(s.lines + s.malformed, 100_000);
    assert!(s.malformed > 0 && s.errors > 0);
    // A correct version makes 0. One that turns each line into a `String` makes about 100,000.
    assert_eq!(after - before, 0, "allocations during aggregate");
}
