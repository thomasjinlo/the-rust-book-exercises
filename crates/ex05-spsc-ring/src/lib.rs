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
use std::sync::atomic::AtomicUsize;

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
#[allow(dead_code)] // fields are unused until you implement the TODOs
pub(crate) struct Shared<T> {
    pub(crate) buf: Box<[UnsafeCell<MaybeUninit<T>>]>,
    pub(crate) mask: usize,
    pub(crate) head: CachePadded<AtomicUsize>,
    pub(crate) tail: CachePadded<AtomicUsize>,
}

// TODO(R9): replace this line with a `// SAFETY:` comment (4-6 lines) that
// states who accesses each slot, which atomic operations hand a slot from one
// thread to the other, and why `T: Send` is required. The impls are provided
// so the given tests compile.
unsafe impl<T: Send> Sync for Shared<T> {}
unsafe impl<T: Send> Send for Shared<T> {}

// TODO(R8): implement `Drop for Shared<T>`: drop every item still in the
// buffer exactly once.

/// Sending half. `Send` when `T: Send`, never `Sync`.
#[allow(dead_code)]
pub struct Producer<T> {
    pub(crate) shared: Arc<Shared<T>>,
    // `Arc<Shared<T>>` is `Sync` once `Shared<T>` is, which would make this
    // type `Sync`. `Cell<()>` is `!Sync` and removes that auto-impl.
    pub(crate) _not_sync: PhantomData<Cell<()>>,
}

/// Receiving half. `Send` when `T: Send`, never `Sync`.
#[allow(dead_code)]
pub struct Consumer<T> {
    pub(crate) shared: Arc<Shared<T>>,
    pub(crate) _not_sync: PhantomData<Cell<()>>,
}

/// Creates a queue with capacity `min_capacity.next_power_of_two()`.
///
/// `min_capacity == 0` yields capacity 1. Panics if the rounded capacity does
/// not fit in `usize`.
pub fn channel<T>(min_capacity: usize) -> (Producer<T>, Consumer<T>) {
    let _ = min_capacity;
    todo!("allocate cap uninitialized slots, build Shared, return both halves")
}

impl<T> Producer<T> {
    /// Appends `value`. Returns `Err(value)` without side effects when the
    /// queue holds `capacity()` items. Never blocks, never allocates.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let _ = value;
        todo!()
    }

    /// Best effort: `true` means the queue was full at some instant during the
    /// call. A `false` result may be stale by the time the caller acts on it,
    /// but a following `push` then succeeds, because only the producer adds
    /// items.
    pub fn is_full(&self) -> bool {
        todo!()
    }

    /// Number of slots, a power of two >= 1.
    pub fn capacity(&self) -> usize {
        todo!()
    }
}

impl<T> Consumer<T> {
    /// Removes and returns the oldest item, or `None` when empty.
    /// Never blocks, never allocates.
    pub fn pop(&mut self) -> Option<T> {
        todo!()
    }

    /// Best effort: `true` means the queue was empty at some instant during
    /// the call. A `false` result is a guarantee that a following `pop`
    /// returns `Some`, because only the consumer removes items.
    pub fn is_empty(&self) -> bool {
        todo!()
    }

    /// Number of slots, a power of two >= 1.
    pub fn capacity(&self) -> usize {
        todo!()
    }
}
