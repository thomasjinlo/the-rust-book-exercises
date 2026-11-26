//! Bounded wait-free single-producer single-consumer (SPSC) queue.
//!
//! The queue is a power-of-two ring buffer with two monotonically increasing
//! `usize` indices. `tail` counts pushes and is written only by the producer.
//! `head` counts pops and is written only by the consumer. Slot `i` lives at
//! `buf[i & mask]`.
//!
//! Neither half is `Sync`:
//!
//! ```compile_fail
//! fn assert_sync<T: Sync>() {}
//! assert_sync::<ex05_spsc_ring::Producer<u32>>();
//! ```
//!
//! ```compile_fail
//! fn assert_sync<T: Sync>() {}
//! assert_sync::<ex05_spsc_ring::Consumer<u32>>();
//! ```

use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Forces its contents onto their own 64-byte cache line so the producer's
/// and consumer's index writes do not invalidate each other's line.
/// Provided. Do not edit.
#[repr(align(64))]
pub(crate) struct CachePadded<T>(pub(crate) T);

/// State shared by both halves.
///
/// Invariants (`cap == buf.len()`, a power of two):
/// - `tail.wrapping_sub(head) <= cap` at all times.
/// - Slots with logical index in `head..tail` are initialized. All others are not.
/// - Only the producer writes `tail` and slots in the free region.
///   Only the consumer writes `head` and moves values out of the full region.
pub(crate) struct Shared<T> {
    pub(crate) buf: Box<[UnsafeCell<MaybeUninit<T>>]>,
    pub(crate) mask: usize,
    pub(crate) head: CachePadded<AtomicUsize>,
    pub(crate) tail: CachePadded<AtomicUsize>,
}

// SAFETY: a slot is accessed by one thread at a time. The producer touches a
// slot only while it is outside head..tail, the consumer only while it is
// inside. Ownership moves between them through the Release store / Acquire
// load pair on `tail` (producer to consumer) and on `head` (consumer to
// producer). Values cross threads, so `T: Send` is required.
unsafe impl<T: Send> Sync for Shared<T> {}
unsafe impl<T: Send> Send for Shared<T> {}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        // `&mut self` means no other thread holds the Arc, so plain reads are enough.
        let head = *self.head.0.get_mut();
        let tail = *self.tail.0.get_mut();
        let mut i = head;
        while i != tail {
            // SAFETY: indices in head..tail are initialized and owned by the queue.
            unsafe { self.buf[i & self.mask].get_mut().assume_init_drop() };
            i = i.wrapping_add(1);
        }
    }
}

/// Sending half. `Send` when `T: Send`, never `Sync`.
pub struct Producer<T> {
    pub(crate) shared: Arc<Shared<T>>,
    // `Arc<Shared<T>>` is `Sync` once `Shared<T>` is, which would make this
    // type `Sync`. `Cell<()>` is `!Sync` and removes that auto-impl.
    pub(crate) _not_sync: PhantomData<Cell<()>>,
}

/// Receiving half. `Send` when `T: Send`, never `Sync`.
pub struct Consumer<T> {
    pub(crate) shared: Arc<Shared<T>>,
    pub(crate) _not_sync: PhantomData<Cell<()>>,
}

/// Creates a queue with capacity `min_capacity.next_power_of_two()`.
///
/// `min_capacity == 0` yields capacity 1. Panics if the rounded capacity does
/// not fit in `usize`.
pub fn channel<T>(min_capacity: usize) -> (Producer<T>, Consumer<T>) {
    let cap = min_capacity.max(1).next_power_of_two();
    let buf: Box<[UnsafeCell<MaybeUninit<T>>]> = (0..cap)
        .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
        .collect();
    let shared = Arc::new(Shared {
        buf,
        mask: cap - 1,
        head: CachePadded(AtomicUsize::new(0)),
        tail: CachePadded(AtomicUsize::new(0)),
    });
    (
        Producer {
            shared: Arc::clone(&shared),
            _not_sync: PhantomData,
        },
        Consumer {
            shared,
            _not_sync: PhantomData,
        },
    )
}

impl<T> Producer<T> {
    /// Appends `value`. Returns `Err(value)` without side effects when the
    /// queue holds `capacity()` items. Never blocks, never allocates.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let s = &*self.shared;
        // Only this thread writes `tail`, so a Relaxed read sees our last store.
        let tail = s.tail.0.load(Ordering::Relaxed);
        // Acquire pairs with the consumer's Release store of `head`: the
        // consumer's move-out of the slot happens before we overwrite it.
        let head = s.head.0.load(Ordering::Acquire);
        if tail.wrapping_sub(head) == s.buf.len() {
            return Err(value);
        }
        // SAFETY: the slot is outside head..tail, so only we access it.
        unsafe { (*s.buf[tail & s.mask].get()).write(value) };
        // Release publishes the slot write to the consumer's Acquire load.
        s.tail.0.store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Best effort: `true` means the queue was full at some instant during the
    /// call. A `false` result may be stale by the time the caller acts on it,
    /// but a following `push` then succeeds, because only the producer adds
    /// items.
    pub fn is_full(&self) -> bool {
        let s = &*self.shared;
        let tail = s.tail.0.load(Ordering::Relaxed);
        let head = s.head.0.load(Ordering::Acquire);
        tail.wrapping_sub(head) == s.buf.len()
    }

    /// Number of slots, a power of two >= 1.
    pub fn capacity(&self) -> usize {
        self.shared.buf.len()
    }
}

impl<T> Consumer<T> {
    /// Removes and returns the oldest item, or `None` when empty.
    /// Never blocks, never allocates.
    pub fn pop(&mut self) -> Option<T> {
        let s = &*self.shared;
        let head = s.head.0.load(Ordering::Relaxed);
        // Acquire pairs with the producer's Release store of `tail`: the slot
        // write happens before our read.
        let tail = s.tail.0.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        // SAFETY: the slot is inside head..tail, so it is initialized, and
        // only we access it until `head` advances.
        let value = unsafe { (*s.buf[head & s.mask].get()).assume_init_read() };
        // Release hands the slot back to the producer after the read above.
        s.head.0.store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    /// Best effort: `true` means the queue was empty at some instant during
    /// the call. A `false` result is a guarantee that a following `pop`
    /// returns `Some`, because only the consumer removes items.
    pub fn is_empty(&self) -> bool {
        let s = &*self.shared;
        let head = s.head.0.load(Ordering::Relaxed);
        head == s.tail.0.load(Ordering::Acquire)
    }

    /// Number of slots, a power of two >= 1.
    pub fn capacity(&self) -> usize {
        self.shared.buf.len()
    }
}
