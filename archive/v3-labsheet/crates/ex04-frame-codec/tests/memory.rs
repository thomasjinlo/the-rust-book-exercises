//! Bounded-memory check. Kept in its own file because it installs a counting
//! global allocator for the whole test binary and must be the only test in it.

use ex04_frame_codec::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
            PEAK.fetch_max(live, Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            if new_size >= layout.size() {
                let live =
                    LIVE.fetch_add(new_size - layout.size(), Relaxed) + new_size - layout.size();
                PEAK.fetch_max(live, Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Relaxed);
            }
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const FRAMES: u32 = 1_000_000;
const CHUNK: usize = 4096;
/// Allowed growth of live heap bytes while decoding. The stream is about 9 MB
/// on the wire, far above this limit.
const BOUND: usize = 64 * 1024;

/// 1,000,000 frames with a 4-byte payload, fed in 4096-byte chunks, each chunk
/// drained before the next is fed. Frames are dropped as soon as they are checked.
#[test]
fn decoding_a_million_small_frames_keeps_memory_bounded() {
    let mut dec = Decoder::new(1024);
    let mut pending: Vec<u8> = Vec::with_capacity(2 * CHUNK);
    let mut next_id = 0u32;
    let mut decoded = 0u32;
    let baseline = LIVE.load(Relaxed);
    PEAK.store(baseline, Relaxed);

    while decoded < FRAMES {
        while pending.len() < CHUNK && next_id < FRAMES {
            let frame = Frame {
                kind: 1,
                payload: next_id.to_be_bytes().to_vec(),
            };
            encode(&frame, &mut pending);
            next_id += 1;
        }
        let n = pending.len().min(CHUNK);
        dec.feed(&pending[..n]);
        pending.drain(..n);
        while let Some(frame) = dec.next_frame().unwrap() {
            assert_eq!(frame.payload, decoded.to_be_bytes());
            decoded += 1;
        }
    }

    assert_eq!(decoded, FRAMES);
    let growth = PEAK.load(Relaxed).saturating_sub(baseline);
    assert!(
        growth <= BOUND,
        "live heap grew by {growth} bytes, limit is {BOUND}"
    );
}
