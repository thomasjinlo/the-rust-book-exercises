//! Implement `Aggregator` here. See the crate docs in `lib.rs` for the line format.

use crate::Summary;

/// Streaming aggregator. All memory is allocated in [`Aggregator::new`]; `ingest`
/// and `finish` must perform zero heap allocations and zero reallocations.
///
/// Add the fields you need. Required contents:
/// * a client table of `TABLE_SLOTS` `u32` slots (open addressing, 0 = empty);
/// * a `[u8; CARRY_CAP]` carry buffer for a line split across chunks.
pub struct Aggregator {}

impl Aggregator {
    /// Creates an empty aggregator. The only function allowed to allocate.
    pub fn new() -> Aggregator {
        todo!()
    }

    /// Consumes the next chunk of the stream. Chunks may end anywhere, including
    /// in the middle of a line. Line offsets in the result count from the first
    /// byte passed to the first `ingest` call after `new` or `finish`.
    ///
    /// A line longer than `CARRY_CAP` bytes is counted malformed once, at the
    /// moment it is known to exceed the limit, and its remaining bytes are
    /// skipped up to the next `\n`.
    pub fn ingest(&mut self, _chunk: &[u8]) {
        todo!()
    }

    /// Flushes a final unterminated line, returns the summary of the whole
    /// stream, and resets the aggregator to the state `new` returns. Must not
    /// allocate.
    pub fn finish(&mut self) -> Summary {
        todo!()
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}
