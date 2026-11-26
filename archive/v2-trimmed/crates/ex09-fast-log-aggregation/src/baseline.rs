//! Naive reference implementation. Slow on purpose: it allocates a `Vec` per
//! line and uses generic `str` APIs. Do not edit.

use crate::{SlowRequest, Summary};

/// Aggregates `input` with the same definitions as [`crate::Aggregator`].
pub fn aggregate(input: &str) -> Summary {
    let mut s = Summary::default();
    let mut offset = 0usize;

    let mut pieces: Vec<&str> = input.split('\n').collect();
    // A trailing newline leaves one empty piece that is not a line.
    if pieces.last() == Some(&"") {
        pieces.pop();
    }

    for line in pieces {
        let start = offset;
        offset += line.len() + 1;
        let Some(rec) = parse(line) else {
            s.malformed += 1;
            continue;
        };
        s.lines += 1;
        s.by_status_class[(rec.status / 100 - 1) as usize] += 1;
        s.total_bytes = s.total_bytes.saturating_add(rec.bytes);
        if s.slowest.is_none_or(|w| rec.latency > w.latency_us) {
            s.slowest = Some(SlowRequest {
                latency_us: rec.latency,
                line_offset: start,
            });
        }
    }
    s
}

struct Record {
    status: u64,
    bytes: u64,
    latency: u64,
}

fn digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn valid_ip(s: &str) -> bool {
    let octets: Vec<String> = s.split('.').map(str::to_owned).collect();
    octets.len() == 4
        && octets.iter().all(|o| {
            o.len() <= 3
                && (o.len() == 1 || !o.starts_with('0'))
                && digits(o).is_some_and(|v| v <= 255)
        })
}

fn parse(line: &str) -> Option<Record> {
    // One owned `String` per field: the straightforward way to write it.
    let f: Vec<String> = line.split(' ').map(str::to_owned).collect();
    if f.len() != 7 {
        return None;
    }
    digits(&f[0])?;
    if !valid_ip(&f[1]) {
        return None;
    }
    if f[2].is_empty() || !f[2].bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    if !f[3].starts_with('/') {
        return None;
    }
    if f[4].len() != 3 {
        return None;
    }
    let status = digits(&f[4])?;
    if !(100..=599).contains(&status) {
        return None;
    }
    Some(Record {
        status,
        bytes: digits(&f[5])?,
        latency: digits(&f[6])?,
    })
}
