//! Per-key limiting with a bounded number of tracked keys.

use std::collections::HashMap;
use std::hash::Hash;

use crate::bucket::{ConfigError, TokenBucket};

/// One limiter per key, with at most `max_keys` keys limited.
#[derive(Debug)]
pub struct KeyedLimiter<K: Hash + Eq + Clone> {
    table: HashMap<K, TokenBucket>,
    capacity: u32,
    rate: u32,
    max_keys: usize,
}

impl<K: Hash + Eq + Clone> KeyedLimiter<K> {
    /// Errors, checked in this order: `ZeroCapacity`, `ZeroRate`, `ZeroMaxKeys`.
    pub fn new(capacity: u32, refill_per_sec: u32, max_keys: usize) -> Result<Self, ConfigError> {
        // Validates capacity and rate in the required order.
        TokenBucket::new(capacity, refill_per_sec)?;
        if max_keys == 0 {
            return Err(ConfigError::ZeroMaxKeys);
        }
        Ok(Self {
            table: HashMap::new(),
            capacity,
            rate: refill_per_sec,
            max_keys,
        })
    }

    /// Returns whether one request from `key` at time `now_ns` is allowed.
    pub fn check(&mut self, key: &K, now_ns: u64) -> bool {
        if let Some(bucket) = self.table.get_mut(key) {
            return bucket.try_acquire(now_ns, 1);
        }
        if self.table.len() >= self.max_keys {
            return true;
        }
        let mut bucket = TokenBucket::new(self.capacity, self.rate).expect("validated in new");
        let allowed = bucket.try_acquire(now_ns, 1);
        self.table.insert(key.clone(), bucket);
        allowed
    }
}
