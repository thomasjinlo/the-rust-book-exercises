//! A concurrent hash map split into independent `RwLock<HashMap>` shards.
//!
//! Lock-order rule: no method holds two shard locks at the same time. Every
//! method takes one shard lock, finishes, releases it, and only then moves to
//! the next shard. Without a held-while-waiting cycle between locks, two
//! threads cannot deadlock on shard locks.
//!
//! Closures given to `with`, `update` and `retain` run under a shard lock.
//! Calling the same map from inside one is a caller error and may deadlock.
//!
//! Poisoning: a panic in a user closure poisons the shard lock. Every lock
//! acquisition recovers with `PoisonError::into_inner`, so the map stays usable.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hash};
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Largest shard count the map will allocate. Larger requests are clamped.
pub const MAX_SHARDS: usize = 65_536;

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
pub struct ShardedMap<K, V> {
    // You may change these private fields.
    shards: Vec<RwLock<HashMap<K, V>>>,
    hasher: RandomState,
    /// Right-shift that turns a 64-bit hash into a shard index.
    shift: u32,
}

impl<K: Hash + Eq, V> ShardedMap<K, V> {
    /// Creates a map with `shards` rounded up to the next power of two (R1, R2).
    ///
    /// Returns `Err(ZeroShards)` for `0`. Requests above [`MAX_SHARDS`] are
    /// clamped to [`MAX_SHARDS`].
    pub fn new(shards: usize) -> Result<Self, ZeroShards> {
        if shards == 0 {
            return Err(ZeroShards);
        }
        let n = shards.min(MAX_SHARDS).next_power_of_two();
        Ok(Self {
            shards: (0..n).map(|_| RwLock::new(HashMap::new())).collect(),
            hasher: RandomState::new(),
            // n == 1 gives shift 64, which `shard_index` special-cases.
            shift: 64 - n.trailing_zeros(),
        })
    }

    /// Number of shards after rounding.
    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Inserts `value`, returning the previous value for `key` if any.
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.write(self.shard_index(&key)).insert(key, value)
    }

    /// Returns a clone of the value for `key`.
    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        self.read(self.shard_index(key)).get(key).cloned()
    }

    /// Runs `f` on the value for `key` under the shard read lock.
    /// Returns `None` without calling `f` if the key is absent.
    pub fn with<R>(&self, key: &K, f: impl FnOnce(&V) -> R) -> Option<R> {
        self.read(self.shard_index(key)).get(key).map(f)
    }

    /// Removes `key`, returning its value if present.
    pub fn remove(&self, key: &K) -> Option<V> {
        self.write(self.shard_index(key)).remove(key)
    }

    /// Atomic read-modify-write under the shard WRITE lock.
    ///
    /// If `key` is absent, `default()` is inserted first, then `f` runs on the
    /// stored value. Concurrent `update` calls on one key never lose a write.
    /// If `f` panics, the default stays inserted and the panic propagates.
    pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R {
        let idx = self.shard_index(&key);
        let mut shard = self.write(idx);
        f(shard.entry(key).or_insert_with(default))
    }

    /// Sum of shard sizes. NOT a snapshot: each shard is read-locked one at a
    /// time, so concurrent writers can change shards that were already counted.
    pub fn len(&self) -> usize {
        (0..self.shards.len()).map(|i| self.read(i).len()).sum()
    }

    /// `true` when `len() == 0`, with the same non-snapshot caveat as `len`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Entry count of each shard, in shard-index order. Length is `shard_count()`.
    pub fn shard_sizes(&self) -> Vec<usize> {
        (0..self.shards.len()).map(|i| self.read(i).len()).collect()
    }

    /// Keeps only entries for which `f` returns `true`.
    /// Each shard is write-locked one at a time; `f` runs under that lock.
    pub fn retain(&self, f: impl FnMut(&K, &mut V) -> bool) {
        let mut f = f;
        for i in 0..self.shards.len() {
            self.write(i).retain(|k, v| f(k, v));
        }
    }

    /// Top bits of the hash pick the shard. `x >> 64` would overflow, so a
    /// single shard is handled separately.
    fn shard_index(&self, key: &K) -> usize {
        if self.shards.len() == 1 {
            return 0;
        }
        (self.hasher.hash_one(key) >> self.shift) as usize
    }

    // A panicking closure poisons the lock but cannot leave the HashMap
    // structurally broken, so the guard is recovered.
    fn read(&self, i: usize) -> RwLockReadGuard<'_, HashMap<K, V>> {
        self.shards[i]
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self, i: usize) -> RwLockWriteGuard<'_, HashMap<K, V>> {
        self.shards[i]
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
