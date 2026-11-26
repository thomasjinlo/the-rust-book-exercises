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
            ConfigError::ZeroRate => "refill rate must be at least 1 token per second",
            ConfigError::ZeroMaxKeys => "max_keys must be at least 1",
        })
    }
}

impl std::error::Error for ConfigError {}

/// A token bucket. It starts full and refills at `refill_per_sec` tokens per second,
/// up to `capacity`.
///
/// Time is passed in by the caller as `now_ns` (nanoseconds on any monotonic scale).
/// Refill is exact: `elapsed_ns * refill_per_sec / 1_000_000_000` tokens. The
/// fractional part carries over to the next call, so splitting an interval into many
/// calls never loses or gains tokens. Use integers only (`u128` for the product).
///
/// A `now_ns` below the largest value seen so far adds no tokens and must not panic.
/// The largest value seen stays the reference point for the next refill.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: u32,
    rate: u32,
    // Level in units of 1/1e9 token: one nanosecond at rate 1 adds exactly one unit,
    // so the level stays an integer and keeps the fractional remainder.
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

    /// Takes `n` tokens if all are available, else takes none and returns `false`.
    ///
    /// `n == 0` returns `true`. `n > capacity` returns `false` and leaves the bucket
    /// unchanged. Must not panic or overflow for any `now_ns`, including `u64::MAX`.
    pub fn try_acquire(&mut self, now_ns: u64, n: u32) -> bool {
        if n > self.capacity {
            return false;
        }
        self.refill(now_ns);
        let cost = u128::from(n) * NS_PER_SEC;
        if self.level >= cost {
            self.level -= cost;
            true
        } else {
            false
        }
    }

    /// Whole tokens available at `now_ns`, after refilling.
    pub fn available(&mut self, now_ns: u64) -> u32 {
        self.refill(now_ns);
        // level <= capacity * 1e9, so the quotient fits in u32.
        (self.level / NS_PER_SEC) as u32
    }
}
