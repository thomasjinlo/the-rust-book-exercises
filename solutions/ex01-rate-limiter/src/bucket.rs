//! Token bucket.

const NS_PER_SEC: u128 = 1_000_000_000;

/// Allows a burst of `capacity` units, then `refill_per_sec` units per second.
/// Time is passed in by the caller, in nanoseconds.
pub struct TokenBucket {
    max: u128,
    rate: u128,
    // Units scaled by 1e9: one nanosecond at rate 1 adds exactly 1, so the
    // level is an integer and the fractional part carries between calls.
    level: u128,
    latest_ns: u64,
}

impl TokenBucket {
    /// Creates a full bucket. `capacity` and `refill_per_sec` are at least 1.
    pub fn new(capacity: u32, refill_per_sec: u32) -> TokenBucket {
        let max = u128::from(capacity) * NS_PER_SEC;
        TokenBucket {
            max,
            rate: u128::from(refill_per_sec),
            level: max,
            latest_ns: 0,
        }
    }

    /// Returns whether a request for one unit at time `now_ns` is allowed.
    pub fn try_acquire(&mut self, now_ns: u64) -> bool {
        if now_ns > self.latest_ns {
            let added = u128::from(now_ns - self.latest_ns) * self.rate;
            self.level = (self.level + added).min(self.max);
            self.latest_ns = now_ns;
        }
        if self.level >= NS_PER_SEC {
            self.level -= NS_PER_SEC;
            true
        } else {
            false
        }
    }
}
