//! O(1) LRU cache: a `Vec` slab of nodes linked by `usize` indices, a free
//! list that reuses vacated slots, and a `HashMap<K, usize>` from key to slot.
//!
//! No `unsafe`, no `Rc`, no `RefCell`. Do not change the public API.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

/// Index value meaning "no node". Slab indices are always below this.
const NIL: usize = usize::MAX;

/// Returned by [`LruCache::new`] when `capacity == 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroCapacity;

impl fmt::Display for ZeroCapacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cache capacity must be at least 1")
    }
}

impl std::error::Error for ZeroCapacity {}

/// One list node. `prev` points toward the most recently used end, `next`
/// toward the least recently used end. `NIL` marks either end of the list.
#[allow(dead_code)]
struct Node<K, V> {
    key: K,
    value: V,
    prev: usize,
    next: usize,
}

/// A slab slot: a live node, or a link in the free list.
#[allow(dead_code)]
enum Slot<K, V> {
    Occupied(Node<K, V>),
    Vacant { next_free: usize },
}

/// Fixed-capacity cache that evicts the least recently used entry.
///
/// You may change the private fields and helper types. You may not change the
/// public API or add `unsafe`, `Rc` or `RefCell`.
#[allow(dead_code)]
pub struct LruCache<K, V> {
    map: HashMap<K, usize>,
    slots: Vec<Slot<K, V>>,
    /// Most recently used slot, or `NIL` when empty.
    head: usize,
    /// Least recently used slot, or `NIL` when empty.
    tail: usize,
    /// First vacant slot, or `NIL` when none.
    free: usize,
    capacity: usize,
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// Creates an empty cache holding at most `capacity` entries.
    ///
    /// Returns `Err(ZeroCapacity)` when `capacity == 0`.
    pub fn new(capacity: usize) -> Result<Self, ZeroCapacity> {
        todo!()
    }

    /// Returns the value for `key` and makes the entry most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        todo!()
    }

    /// Like [`get`](Self::get) but returns a mutable reference.
    /// A hit makes the entry most recently used.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        todo!()
    }

    /// Returns the value for `key` without changing recency.
    pub fn peek(&self, key: &K) -> Option<&V> {
        todo!()
    }

    /// Inserts or updates `key`; the entry becomes most recently used.
    ///
    /// - Existing key: the old value is dropped, the new one stored, returns `None`.
    /// - New key, cache full: evicts the least recently used entry first and
    ///   returns it as `Some((key, value))`.
    /// - New key, cache not full: returns `None`.
    pub fn put(&mut self, key: K, value: V) -> Option<(K, V)> {
        todo!()
    }

    /// Removes `key` and returns its value. The vacated slot joins the free list.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        todo!()
    }

    /// Number of live entries.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// Maximum number of live entries, as passed to `new`.
    pub fn capacity(&self) -> usize {
        todo!()
    }

    /// True when `len() == 0`.
    pub fn is_empty(&self) -> bool {
        todo!()
    }

    /// Number of slab slots currently allocated, live or vacant.
    /// Must never exceed `capacity()`.
    pub fn slab_len(&self) -> usize {
        todo!()
    }

    /// Iterates entries from most to least recently used. Does not change recency.
    pub fn iter(&self) -> Iter<'_, K, V> {
        todo!()
    }
}

/// Iterator returned by [`LruCache::iter`].
#[allow(dead_code)]
pub struct Iter<'a, K, V> {
    cache: &'a LruCache<K, V>,
    next: usize,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
