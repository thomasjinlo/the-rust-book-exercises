//! Fixed-capacity least-recently-used cache.
//!
//! Replace each `todo!()`. Keep the public signatures. Add any private fields and helpers you need.

use std::hash::Hash;
use std::marker::PhantomData;

/// A cache that holds at most `capacity` entries and evicts the least recently used one.
pub struct LruCache<K, V> {
    // Replace this field with your own.
    _todo: PhantomData<(K, V)>,
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// Creates an empty cache. `capacity` is at least 1.
    pub fn new(capacity: usize) -> Self {
        let _ = capacity;
        todo!()
    }

    /// Stores `value` under `key` and makes it the most recently used entry.
    /// When a new key arrives and the cache is full, the least recently used entry is removed.
    pub fn put(&mut self, key: K, value: V) {
        let _ = (key, value);
        todo!()
    }

    /// Returns the value for `key` and makes it the most recently used entry.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let _ = key;
        todo!()
    }
}
