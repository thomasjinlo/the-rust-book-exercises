//! Token bucket.

use std::fmt;

const NS_PER_SEC: u128 = 1_000_000_000;

/// Invalid limiter configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    ZeroCapacity,
    ZeroRate,
    ZeroMaxKeys,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ConfigError::ZeroCapacity => "capacity must be at least 1",
            ConfigError::ZeroRate => "refill rate must be at least 1 unit per second",
            ConfigError::ZeroMaxKeys => "max_keys must be at least 1",
        })
    }
}

impl std::error::Error for ConfigError {}

/// A rate limiter that allows a burst of `capacity` units and a sustained
/// `refill_per_sec` units per second. Time is passed in by the caller.
#[derive(Debug)]
pub struct TokenBucket {
    capacity: u32,
    rate: u32,
    // Level in units of 1/1e9 token: one nanosecond at rate 1 adds exactly one unit,
    // so the level is an integer and keeps the fractional part between calls.
    level: u128,
    last_ns: u64,
}

impl TokenBucket {
    /// Errors: `ZeroCapacity` if `capacity == 0` (checked first), `ZeroRate` if
    /// `refill_per_sec == 0`.
    pub fn new(capacity: u32, refill_per_sec: u32) -> Result<TokenBucket, ConfigError> {
        if capacity == 0 {
            return Err(ConfigError::ZeroCapacity);
        }
        if refill_per_sec == 0 {
            return Err(ConfigError::ZeroRate);
        }
        Ok(TokenBucket {
            capacity,
            rate: refill_per_sec,
            level: u128::from(capacity) * NS_PER_SEC,
            last_ns: 0,
        })
    }

    fn refill(&mut self, now_ns: u64) {
        if now_ns <= self.last_ns {
            return;
        }
        // At most 2^64 * 2^32, which fits in u128.
        let added = u128::from(now_ns - self.last_ns) * u128::from(self.rate);
        let max = u128::from(self.capacity) * NS_PER_SEC;
        self.level = (self.level + added).min(max);
        self.last_ns = now_ns;
    }

    /// Returns whether a request for `n` units at time `now_ns` is allowed.
    pub fn try_acquire(&mut self, now_ns: u64, n: u32) -> bool {
        self.refill(now_ns);
        let cost = u128::from(n) * NS_PER_SEC;
        if self.level >= cost {
            self.level -= cost;
            true
        } else {
            false
        }
    }
}
