//! Token bucket.

use std::fmt;

/// Invalid limiter configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    ZeroCapacity,
    ZeroRate,
    ZeroMaxKeys,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("one short, distinct message per variant")
    }
}

impl std::error::Error for ConfigError {}

/// A rate limiter that allows a burst of `capacity` units and a sustained
/// `refill_per_sec` units per second. Time is passed in by the caller.
#[derive(Debug)]
pub struct TokenBucket {
    // Add the fields you need.
}

impl TokenBucket {
    /// Errors: `ZeroCapacity` if `capacity == 0` (checked first), `ZeroRate` if
    /// `refill_per_sec == 0`.
    pub fn new(capacity: u32, refill_per_sec: u32) -> Result<TokenBucket, ConfigError> {
        let _ = (capacity, refill_per_sec);
        todo!()
    }

    /// Returns whether a request for `n` units at time `now_ns` is allowed.
    pub fn try_acquire(&mut self, now_ns: u64, n: u32) -> bool {
        let _ = (now_ns, n);
        todo!()
    }
}
