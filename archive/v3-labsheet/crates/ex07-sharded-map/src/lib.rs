//! A hash map that many threads can use at the same time through `&self`.

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

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
    // Replace this placeholder with your own fields.
    _placeholder: PhantomData<(K, V)>,
}

impl<K: Hash + Eq, V> ShardedMap<K, V> {
    /// Creates an empty map. `shards` is a hint for how much parallelism to
    /// support: any value >= 1 must work. `0` returns `Err(ZeroShards)`.
    pub fn new(shards: usize) -> Result<Self, ZeroShards> {
        let _ = shards;
        todo!()
    }

    /// Inserts `value`, returning the previous value for `key` if any.
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let _ = (key, value);
        todo!()
    }

    /// Returns a copy of the value for `key`.
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

    /// Atomic read-modify-write. If `key` is absent, `default()` is stored
    /// first. Then `f` runs on the stored value and its result is returned.
    /// `default` is not called when the key exists. Concurrent `update` calls
    /// on one key never lose a write.
    pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R {
        let _ = (key, default, f);
        todo!()
    }

    /// Number of entries. Under concurrent writers the result is approximate.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// `true` when the map holds no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
