//! Fixed-capacity least-recently-used cache.
//!
//! Replace each `todo!()`. Keep the public signatures unchanged; the private
//! fields and any helper items are yours to design.

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

/// Returned by [`LruCache::new`] when `capacity == 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroCapacity;

impl fmt::Display for ZeroCapacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cache capacity must be at least 1")
    }
}

impl std::error::Error for ZeroCapacity {}

/// Fixed-capacity cache that evicts the least recently used entry.
#[allow(dead_code)]
pub struct LruCache<K, V> {
    // Placeholder fields: replace them with your own storage.
    capacity: usize,
    entries: PhantomData<(K, V)>,
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// Creates an empty cache holding at most `capacity` entries.
    pub fn new(capacity: usize) -> Result<Self, ZeroCapacity> {
        todo!()
    }

    /// Returns the value for `key` and makes the entry most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        todo!()
    }

    /// Like [`get`](Self::get) but returns a mutable reference.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        todo!()
    }

    /// Returns the value for `key` without changing recency.
    pub fn peek(&self, key: &K) -> Option<&V> {
        todo!()
    }

    /// Inserts or updates `key`; the entry becomes most recently used.
    /// Returns the evicted `(key, value)` when a new key displaced the least
    /// recently used entry.
    pub fn put(&mut self, key: K, value: V) -> Option<(K, V)> {
        todo!()
    }

    /// Removes `key` and returns its value.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        todo!()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// Maximum number of entries, as passed to `new`.
    pub fn capacity(&self) -> usize {
        todo!()
    }

    /// True when `len() == 0`.
    pub fn is_empty(&self) -> bool {
        todo!()
    }

    /// Iterates entries from most to least recently used without changing recency.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        todo!();
        #[allow(unreachable_code)]
        std::iter::empty()
    }
}
