//! Implement `Aggregator` here. See the crate docs in `lib.rs` for the line format.

use crate::Summary;

/// Access-log aggregator. `aggregate` must perform zero heap allocations.
///
/// Add fields only if you need them; the reference solution needs none.
pub struct Aggregator {}

impl Aggregator {
    /// Creates an aggregator. Allocating here is allowed.
    pub fn new() -> Aggregator {
        todo!()
    }

    /// Aggregates a complete buffer of log lines. The last line may lack `\n`.
    /// `SlowRequest::line_offset` counts from `input[0]`. Each call is
    /// independent of earlier calls.
    ///
    /// Must not allocate, even when lines are malformed.
    pub fn aggregate(&mut self, _input: &[u8]) -> Summary {
        todo!()
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}
