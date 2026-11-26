//! Per-key limiting with a bounded table.

use std::collections::HashMap;
use std::hash::Hash;

use crate::bucket::{ConfigError, TokenBucket};

/// One [`TokenBucket`] per key, with at most `max_keys` keys in memory.
///
/// A key that is not in the table while the table holds `max_keys` entries is
/// allowed and is not stored. Keys are never evicted.
#[derive(Debug)]
pub struct KeyedLimiter<K: Hash + Eq + Clone> {
    #[allow(dead_code)]
    table: HashMap<K, TokenBucket>,
    // Add the fields you need.
}

impl<K: Hash + Eq + Clone> KeyedLimiter<K> {
    /// Errors, checked in this order: `ZeroCapacity`, `ZeroRate`, `ZeroMaxKeys`.
    pub fn new(capacity: u32, refill_per_sec: u32, max_keys: usize) -> Result<Self, ConfigError> {
        let _ = (capacity, refill_per_sec, max_keys);
        todo!()
    }

    /// Takes one token from `key`'s bucket and returns whether it succeeded.
    /// A new key starts with a full bucket, and this call takes its first token.
    /// A new key that does not fit in the table returns `true` and stores nothing.
    pub fn check(&mut self, key: &K, now_ns: u64) -> bool {
        let _ = (key, now_ns);
        todo!()
    }

    /// Number of keys in the table. Never exceeds `max_keys`.
    pub fn len(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
