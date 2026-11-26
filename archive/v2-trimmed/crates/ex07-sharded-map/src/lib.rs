//! A concurrent hash map split into independent `RwLock<HashMap>` shards.
//!
//! Design rule: no method holds two shard locks at the same time. Every method
//! locks one shard, finishes with it, releases it, and only then may lock
//! another. A deadlock needs a thread that holds one lock while waiting for
//! another, so this rule rules it out.
//!
//! Closures given to `update` run under the shard write lock. Calling the same
//! map from inside one is a caller error and may deadlock.
//!
//! A panic inside a closure may poison that shard's lock. Later calls that
//! touch the poisoned shard may then panic. This is accepted, not handled.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hash};
use std::sync::RwLock;

/// Returned by [`ShardedMap::new`] when `shards == 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroShards;

impl fmt::Display for ZeroShards {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("shard count must be at least 1")
    }
}

impl std::error::Error for ZeroShards {}

/// A hash map made of `shard_count()` independent `RwLock<HashMap<K, V>>`.
///
/// A key's shard is chosen from the HIGH bits of one 64-bit hash of the key.
/// The inner `HashMap` uses its own hasher, so shard choice and bucket choice
/// are independent.
#[allow(dead_code)] // fields are unused until you implement the methods
pub struct ShardedMap<K, V> {
    // You may change these private fields.
    shards: Vec<RwLock<HashMap<K, V>>>,
    hasher: RandomState,
    /// Right-shift that turns a 64-bit hash into a shard index.
    shift: u32,
}

impl<K: Hash + Eq, V> ShardedMap<K, V> {
    /// Creates a map with `shards.next_power_of_two()` shards.
    ///
    /// Returns `Err(ZeroShards)` for `0`. Behavior for `shards` above 2^20 is
    /// unspecified.
    pub fn new(shards: usize) -> Result<Self, ZeroShards> {
        let _ = shards;
        todo!()
    }

    /// Number of shards after rounding.
    pub fn shard_count(&self) -> usize {
        todo!()
    }

    /// Inserts `value`, returning the previous value for `key` if any.
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let _ = (key, value);
        todo!()
    }

    /// Returns a clone of the value for `key`. The lock is released before returning.
    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let _ = key;
        todo!()
    }

    /// Removes `key`, returning its value if present.
    pub fn remove(&self, key: &K) -> Option<V> {
        let _ = key;
        todo!()
    }

    /// Atomic read-modify-write under the shard WRITE lock, taken once.
    ///
    /// If `key` is absent, `default()` is inserted first. Then `f` runs on the
    /// stored value and its result is returned. `default` is not called when
    /// the key exists. Concurrent `update` calls on one key never lose a write.
    pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R {
        let _ = (key, default, f);
        todo!()
    }

    /// Sum of shard sizes. NOT a snapshot: each shard is read-locked one at a
    /// time, so concurrent writers can change shards that were already counted.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// `true` when `len() == 0`, with the same non-snapshot caveat as `len`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Entry count of each shard, in shard-index order. Length is `shard_count()`.
    pub fn shard_sizes(&self) -> Vec<usize> {
        todo!()
    }
}
