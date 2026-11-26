//! Implement `Aggregator` here. The line format is defined in the crate docs in `lib.rs`.

use crate::Summary;

/// Access-log aggregator.
pub struct Aggregator {}

impl Aggregator {
    /// Creates an aggregator. Allocating here is allowed.
    pub fn new() -> Aggregator {
        todo!()
    }

    /// Aggregates a complete buffer of log lines. The last line may lack `\n`.
    /// `SlowRequest::line_offset` counts from `input[0]`. Each call is
    /// independent of earlier calls. Performs no heap allocation.
    pub fn aggregate(&mut self, _input: &[u8]) -> Summary {
        todo!()
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}
