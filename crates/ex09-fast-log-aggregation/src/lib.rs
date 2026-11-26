//! Summarise a buffer of access-log lines without allocating.
//!
//! Replace the `todo!()`. Keep the public signatures.

/// Totals for one buffer of log lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// Valid lines.
    pub lines: u64,
    /// Lines that do not match the format, including empty lines.
    pub malformed: u64,
    /// Valid lines with status 500 to 599.
    pub errors: u64,
    /// Sum of `bytes` over valid lines.
    pub total_bytes: u64,
    /// Largest `latency_us` over valid lines, 0 if there is none.
    pub max_latency_us: u64,
}

/// Reads `input` line by line and returns its `Summary`.
pub fn aggregate(input: &[u8]) -> Summary {
    let _ = input;
    todo!()
}
