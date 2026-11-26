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
    table: HashMap<K, TokenBucket>,
    // Validated once in `new`; cloned for each new key.
    template: TokenBucket,
    max_keys: usize,
}

impl<K: Hash + Eq + Clone> KeyedLimiter<K> {
    /// Errors, checked in this order: `ZeroCapacity`, `ZeroRate`, `ZeroMaxKeys`.
    pub fn new(capacity: u32, refill_per_sec: u32, max_keys: usize) -> Result<Self, ConfigError> {
        let template = TokenBucket::new(capacity, refill_per_sec)?;
        if max_keys == 0 {
            return Err(ConfigError::ZeroMaxKeys);
        }
        Ok(Self {
            table: HashMap::new(),
            template,
            max_keys,
        })
    }

    /// Takes one token from `key`'s bucket and returns whether it succeeded.
    /// A new key starts with a full bucket, and this call takes its first token.
    /// A new key that does not fit in the table returns `true` and stores nothing.
    pub fn check(&mut self, key: &K, now_ns: u64) -> bool {
        if let Some(bucket) = self.table.get_mut(key) {
            return bucket.try_acquire(now_ns, 1);
        }
        if self.table.len() >= self.max_keys {
            return true;
        }
        let mut bucket = self.template.clone();
        let allowed = bucket.try_acquire(now_ns, 1);
        self.table.insert(key.clone(), bucket);
        allowed
    }

    /// Number of keys in the table. Never exceeds `max_keys`.
    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}
