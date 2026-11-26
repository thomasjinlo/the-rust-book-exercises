//! Per-key limiting with a bounded table.

use std::collections::HashMap;
use std::hash::Hash;

use crate::bucket::{ConfigError, TokenBucket};

/// Outcome of [`KeyedLimiter::check`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// One token was taken.
    Allowed,
    /// The key's bucket is empty.
    Limited,
    /// The table is full and the key is not in it. Callers treat this as allowed.
    Untracked,
}

/// One [`TokenBucket`] per key, with at most `max_keys` keys in memory.
///
/// A key that is not in the table while the table holds `max_keys` entries gets
/// `Untracked`, is not stored, and increments `untracked_count()`. Keys are never evicted.
#[derive(Debug)]
pub struct KeyedLimiter<K: Hash + Eq + Clone> {
    table: HashMap<K, TokenBucket>,
    // Validated once in `new`; cloned for each new key.
    template: TokenBucket,
    max_keys: usize,
    untracked: u64,
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
            untracked: 0,
        })
    }

    /// Takes one token from `key`'s bucket. A new key starts with a full bucket, and
    /// this call takes its first token.
    pub fn check(&mut self, key: &K, now_ns: u64) -> Decision {
        let bucket = match self.table.get_mut(key) {
            Some(b) => b,
            None => {
                if self.table.len() >= self.max_keys {
                    self.untracked += 1;
                    return Decision::Untracked;
                }
                self.table
                    .entry(key.clone())
                    .or_insert(self.template.clone())
            }
        };
        if bucket.try_acquire(now_ns, 1) {
            Decision::Allowed
        } else {
            Decision::Limited
        }
    }

    /// Number of keys in the table. Never exceeds `max_keys`.
    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    /// Number of `Untracked` decisions so far.
    pub fn untracked_count(&self) -> u64 {
        self.untracked
    }
}
