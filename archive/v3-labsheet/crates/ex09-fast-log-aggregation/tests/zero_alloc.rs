use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use ex09_fast_log_aggregation::{Aggregator, datagen};

// Counters are per thread so the test harness's own threads cannot add noise.
// `const` initialisation means touching them never allocates.
thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static REALLOCS: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        REALLOCS.with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(p, l, new) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn counts() -> (u64, u64) {
    (ALLOCS.with(Cell::get), REALLOCS.with(Cell::get))
}

#[test]
fn aggregate_allocates_nothing() {
    // Generate and build everything before counting starts.
    let data = datagen::generate(1_000_000, 11);
    let mut agg = Aggregator::new();

    let before = counts();
    let summary = agg.aggregate(&data);
    let after = counts();

    assert_eq!(summary.lines + summary.malformed, 1_000_000);
    assert!(
        summary.malformed > 0,
        "input must exercise the malformed path"
    );
    assert_eq!(after.0 - before.0, 0, "allocations during aggregate");
    assert_eq!(after.1 - before.1, 0, "reallocations during aggregate");
}

#[test]
fn malformed_and_oversized_lines_allocate_nothing() {
    let mut data = datagen::generate(20_000, 8);
    data.extend(std::iter::repeat_n(b'z', 100_000));
    data.push(b'\n');
    data.extend_from_slice(b"1 10.0.0.1 GET /a 200 1 1");
    let mut agg = Aggregator::new();

    let before = counts();
    let summary = agg.aggregate(&data);
    let after = counts();

    assert!(summary.lines > 0 && summary.malformed > 0);
    assert_eq!(after, before, "allocation counters moved");
}

#[test]
fn the_counting_allocator_sees_allocations() {
    // Guards against a vacuous zero: the counter must see a real allocation.
    let before = counts();
    let v: Vec<u8> = Vec::with_capacity(1024);
    let after = counts();
    assert!(after.0 > before.0);
    drop(v);
}
