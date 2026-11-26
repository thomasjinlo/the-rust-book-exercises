//! Token bucket. Replace each `todo!()`. Keep the public signatures.

/// Allows a burst of `capacity` units, then `refill_per_sec` units per second.
/// Time is passed in by the caller, in nanoseconds.
pub struct TokenBucket {
    // Add your own fields.
}

impl TokenBucket {
    /// Creates a full bucket. `capacity` and `refill_per_sec` are at least 1.
    pub fn new(capacity: u32, refill_per_sec: u32) -> TokenBucket {
        let _ = (capacity, refill_per_sec);
        todo!()
    }

    /// Returns whether a request for one unit at time `now_ns` is allowed.
    pub fn try_acquire(&mut self, now_ns: u64) -> bool {
        let _ = now_ns;
        todo!()
    }
}
