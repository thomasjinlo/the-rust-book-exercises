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
        todo!("write one short message per variant")
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
    // Add the fields you need.
}

impl TokenBucket {
    /// Errors: `ZeroCapacity` if `capacity == 0` (checked first), `ZeroRate` if
    /// `refill_per_sec == 0`.
    pub fn new(capacity: u32, refill_per_sec: u32) -> Result<TokenBucket, ConfigError> {
        let _ = (capacity, refill_per_sec);
        todo!()
    }

    /// Takes `n` tokens if all are available, else takes none and returns `false`.
    ///
    /// `n == 0` returns `true`. `n > capacity` returns `false` and leaves the bucket
    /// unchanged. Must not panic or overflow for any `now_ns`, including `u64::MAX`.
    pub fn try_acquire(&mut self, now_ns: u64, n: u32) -> bool {
        let _ = (now_ns, n);
        todo!()
    }

    /// Whole tokens available at `now_ns`, after refilling.
    pub fn available(&mut self, now_ns: u64) -> u32 {
        let _ = now_ns;
        todo!()
    }
}
