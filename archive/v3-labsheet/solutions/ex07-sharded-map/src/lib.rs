//! A hash map that many threads can use at the same time through `&self`.

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

/// A concurrent hash map. All methods take `&self`.
pub struct ShardedMap<K, V> {
    shards: Vec<RwLock<HashMap<K, V>>>,
    hasher: RandomState,
}

impl<K: Hash + Eq, V> ShardedMap<K, V> {
    /// Creates an empty map. `shards` is a hint for how much parallelism to
    /// support: any value >= 1 must work. `0` returns `Err(ZeroShards)`.
    pub fn new(shards: usize) -> Result<Self, ZeroShards> {
        if shards == 0 {
            return Err(ZeroShards);
        }
        Ok(Self {
            shards: (0..shards).map(|_| RwLock::new(HashMap::new())).collect(),
            hasher: RandomState::new(),
        })
    }

    /// Inserts `value`, returning the previous value for `key` if any.
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.shard(&key).write().unwrap().insert(key, value)
    }

    /// Returns a copy of the value for `key`.
    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        self.shard(key).read().unwrap().get(key).cloned()
    }

    /// Removes `key`, returning its value if present.
    pub fn remove(&self, key: &K) -> Option<V> {
        self.shard(key).write().unwrap().remove(key)
    }

    /// Atomic read-modify-write. If `key` is absent, `default()` is stored
    /// first. Then `f` runs on the stored value and its result is returned.
    /// `default` is not called when the key exists. Concurrent `update` calls
    /// on one key never lose a write.
    pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R {
        let mut shard = self.shard(&key).write().unwrap();
        f(shard.entry(key).or_insert_with(default))
    }

    /// Number of entries. Under concurrent writers the result is approximate.
    pub fn len(&self) -> usize {
        self.shards.iter().map(|s| s.read().unwrap().len()).sum()
    }

    /// `true` when the map holds no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Reduce in u64 so the choice does not depend on the pointer width.
    fn shard(&self, key: &K) -> &RwLock<HashMap<K, V>> {
        let i = self.hasher.hash_one(key) % self.shards.len() as u64;
        &self.shards[i as usize]
    }
}
