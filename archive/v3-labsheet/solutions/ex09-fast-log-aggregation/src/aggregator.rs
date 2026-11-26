//! Reference solution: one pass over the bytes, fields are borrowed slices.

use crate::{SlowRequest, Summary};

/// Access-log aggregator. `aggregate` performs no heap allocation.
pub struct Aggregator {}

struct Parsed {
    status: u64,
    bytes: u64,
    latency: u64,
}

/// Reads ASCII digits from the front of `s` and advances `s` past the space
/// that ends the field. With `last`, the field must end at the end of `s`
/// instead. `None` if there is no digit, the field does not end as required,
/// or the value overflows `u64`.
fn number(s: &mut &[u8], last: bool) -> Option<u64> {
    let mut v: u64 = 0;
    let mut i = 0;
    while i < s.len() {
        let d = s[i].wrapping_sub(b'0');
        if d > 9 {
            break;
        }
        // `checked_*` keeps overflow detection without a second pass.
        v = v.checked_mul(10)?.checked_add(u64::from(d))?;
        i += 1;
    }
    if i == 0 {
        return None;
    }
    match s.get(i) {
        None if last => *s = &s[i..],
        Some(b' ') if !last => *s = &s[i + 1..],
        _ => return None,
    }
    Some(v)
}

/// Reads `a.b.c.d ` and returns nothing: only validity matters here.
fn skip_ip(s: &mut &[u8]) -> Option<()> {
    let mut i = 0;
    for sep in [b'.', b'.', b'.', b' '] {
        let start = i;
        let mut v = 0u32;
        while let Some(&b) = s.get(i) {
            let d = b.wrapping_sub(b'0');
            if d > 9 {
                break;
            }
            v = v * 10 + u32::from(d);
            i += 1;
            if i - start > 3 {
                return None;
            }
        }
        let len = i - start;
        if len == 0 || (len > 1 && s[start] == b'0') || v > 255 || s.get(i) != Some(&sep) {
            return None;
        }
        i += 1;
    }
    *s = &s[i..];
    Some(())
}

fn parse_line(line: &[u8]) -> Option<Parsed> {
    let mut rest = line;
    number(&mut rest, false)?;
    skip_ip(&mut rest)?;

    let n = rest.iter().take_while(|b| b.is_ascii_uppercase()).count();
    if n == 0 || rest.get(n) != Some(&b' ') {
        return None;
    }
    rest = &rest[n + 1..];

    if rest.first() != Some(&b'/') {
        return None;
    }
    let n = rest.iter().position(|&b| b == b' ')?;
    rest = &rest[n + 1..];

    // Exactly three digits, then a space.
    if rest.len() < 4 || rest[3] != b' ' {
        return None;
    }
    let status = number(&mut rest, false)?;
    if !(100..=599).contains(&status) {
        return None;
    }
    let bytes = number(&mut rest, false)?;
    // A trailing space or an eighth field fails here.
    let latency = number(&mut rest, true)?;
    Some(Parsed {
        status,
        bytes,
        latency,
    })
}

impl Aggregator {
    /// Creates an aggregator.
    pub fn new() -> Aggregator {
        Aggregator {}
    }

    /// Aggregates a complete buffer of log lines.
    pub fn aggregate(&mut self, input: &[u8]) -> Summary {
        let mut s = Summary::default();
        let mut offset = 0;
        while offset < input.len() {
            let rest = &input[offset..];
            let len = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
            match parse_line(&rest[..len]) {
                None => s.malformed += 1,
                Some(p) => {
                    s.lines += 1;
                    s.by_status_class[(p.status / 100 - 1) as usize] += 1;
                    s.total_bytes = s.total_bytes.saturating_add(p.bytes);
                    // Strict `>` keeps the earliest line among equal maxima.
                    if s.slowest.is_none_or(|w| p.latency > w.latency_us) {
                        s.slowest = Some(SlowRequest {
                            latency_us: p.latency,
                            line_offset: offset,
                        });
                    }
                }
            }
            offset += len + 1;
        }
        s
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}
