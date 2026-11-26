//! Summarise a buffer of access-log lines without allocating.

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

fn number(field: &[u8]) -> Option<u64> {
    if field.is_empty() {
        return None;
    }
    let mut n: u64 = 0;
    for &b in field {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    Some(n)
}

/// Returns `(status, bytes, latency_us)` for a valid line.
fn parse(line: &[u8]) -> Option<(u64, u64, u64)> {
    let mut fields = line.split(|&b| b == b' ');
    let path = fields.next()?;
    let status = fields.next()?;
    let bytes = fields.next()?;
    let latency = fields.next()?;
    if fields.next().is_some() || path.is_empty() || status.len() != 3 {
        return None;
    }
    let status = number(status)?;
    if !(100..=599).contains(&status) {
        return None;
    }
    Some((status, number(bytes)?, number(latency)?))
}

/// Reads `input` line by line and returns its `Summary`.
pub fn aggregate(input: &[u8]) -> Summary {
    let mut s = Summary::default();
    if input.is_empty() {
        return s;
    }
    // A final `\n` ends the last line; it does not start an empty one.
    let body = input.strip_suffix(b"\n").unwrap_or(input);
    for line in body.split(|&b| b == b'\n') {
        match parse(line) {
            Some((status, bytes, latency)) => {
                s.lines += 1;
                s.errors += u64::from(status >= 500);
                s.total_bytes = s.total_bytes.saturating_add(bytes);
                s.max_latency_us = s.max_latency_us.max(latency);
            }
            None => s.malformed += 1,
        }
    }
    s
}
