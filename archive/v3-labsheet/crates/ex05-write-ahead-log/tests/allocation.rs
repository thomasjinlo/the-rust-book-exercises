//! A damaged length must not make `open` or `read_all` allocate a buffer
//! the size of the damaged value. Measured from outside with a counting
//! global allocator. This test has its own file because the allocator is
//! global to the test binary.

use ex05_write_ahead_log::Wal;
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static LARGEST_REQUEST: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST_REQUEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LARGEST_REQUEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LARGEST_REQUEST.fetch_max(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn a_damaged_length_never_causes_a_large_allocation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    // No practical size limit, so only the file's real size can bound a record.
    let (mut wal, _) = Wal::open(&path, u32::MAX).unwrap();
    for i in 0..6u8 {
        wal.append(&vec![i; usize::from(i) * 9]).unwrap();
    }
    drop(wal);
    let bytes = fs::read(&path).unwrap();

    let damaged_path = dir.path().join("damaged");
    let mut worst = 0;
    for pos in 0..bytes.len() {
        for bit in 0..8 {
            let mut damaged = bytes.clone();
            damaged[pos] ^= 1 << bit;
            fs::write(&damaged_path, &damaged).unwrap();
            LARGEST_REQUEST.store(0, Ordering::Relaxed);
            let _ = Wal::read_all(&damaged_path, u32::MAX);
            let _ = Wal::open(&damaged_path, u32::MAX);
            worst = worst.max(LARGEST_REQUEST.load(Ordering::Relaxed));
        }
    }
    assert!(
        worst < 1 << 20,
        "one allocation of {worst} bytes for a {}-byte file",
        bytes.len()
    );
}
