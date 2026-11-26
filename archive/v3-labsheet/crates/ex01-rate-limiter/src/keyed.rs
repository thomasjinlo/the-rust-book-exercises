//! Per-key limiting with a bounded number of tracked keys.

use std::hash::Hash;

use crate::bucket::ConfigError;

/// One limiter per key, with at most `max_keys` keys limited.
#[derive(Debug)]
pub struct KeyedLimiter<K: Hash + Eq + Clone> {
    // Add the fields you need. `PhantomData` keeps the starter compiling until you do.
    _key: std::marker::PhantomData<K>,
}

impl<K: Hash + Eq + Clone> KeyedLimiter<K> {
    /// Errors, checked in this order: `ZeroCapacity`, `ZeroRate`, `ZeroMaxKeys`.
    pub fn new(capacity: u32, refill_per_sec: u32, max_keys: usize) -> Result<Self, ConfigError> {
        let _ = (capacity, refill_per_sec, max_keys);
        todo!()
    }

    /// Returns whether one request from `key` at time `now_ns` is allowed.
    pub fn check(&mut self, key: &K, now_ns: u64) -> bool {
        let _ = (key, now_ns);
        todo!()
    }
}
