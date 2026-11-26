//! Reference solution: byte-level parser, open-addressing client table, fixed carry buffer.

use crate::{CARRY_CAP, HIST_BUCKETS, MAX_STORED_CLIENTS, SlowRequest, Summary, TABLE_SLOTS};

/// Streaming aggregator. All memory is allocated in [`Aggregator::new`]; `ingest`
/// and `finish` perform zero heap allocations and zero reallocations.
pub struct Aggregator {
    summary: Summary,
    /// Open-addressing set of non-zero client addresses; 0 marks an empty slot.
    table: Box<[u32]>,
    stored: usize,
    zero_seen: bool,
    /// Bytes of a line that continues in the next chunk.
    carry: [u8; CARRY_CAP],
    carry_len: usize,
    /// Stream offset of the first byte of the carried line.
    carry_start: usize,
    /// True while skipping the rest of an over-long line.
    discarding: bool,
    /// Total bytes passed to `ingest` so far.
    consumed: usize,
}

struct Parsed {
    ip: u32,
    status: u64,
    bytes: u64,
    latency: u64,
}

/// Reads ASCII digits up to the next space (consumed) or, when `last`, to the end.
/// One pass: validation and accumulation happen together.
fn number(s: &mut &[u8], last: bool) -> Option<u64> {
    let mut v: u64 = 0;
    let mut i = 0;
    while i < s.len() {
        let d = s[i].wrapping_sub(b'0');
        if d > 9 {
            break;
        }
        // Wraps for 20+ digits; those are re-checked below.
        v = v.wrapping_mul(10).wrapping_add(u64::from(d));
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let digits = &s[..i];
    if i == s.len() {
        if !last {
            return None;
        }
        *s = &s[i..];
    } else if s[i] == b' ' && !last {
        *s = &s[i + 1..];
    } else {
        return None;
    }
    if i > 19 {
        // 19 digits always fit in u64; longer inputs need an overflow check.
        digits.iter().try_fold(0u64, |v, &b| {
            v.checked_mul(10)?.checked_add(u64::from(b - b'0'))
        })
    } else {
        Some(v)
    }
}

/// Reads one octet (1 to 3 digits, no leading zero, at most 255) from `s[i..]`.
/// Returns the value and the index after the last digit.
#[inline(always)]
fn octet(s: &[u8], i: usize) -> Option<(u32, usize)> {
    let d0 = u32::from(*s.get(i)?).wrapping_sub(48);
    if d0 > 9 {
        return None;
    }
    let d1 = u32::from(*s.get(i + 1).unwrap_or(&0)).wrapping_sub(48);
    if d1 > 9 {
        return Some((d0, i + 1));
    }
    if d0 == 0 {
        return None;
    }
    let d2 = u32::from(*s.get(i + 2).unwrap_or(&0)).wrapping_sub(48);
    if d2 > 9 {
        return Some((d0 * 10 + d1, i + 2));
    }
    let v = d0 * 100 + d1 * 10 + d2;
    (v <= 255).then_some((v, i + 3))
}

/// Reads `a.b.c.d` followed by a space.
fn ip(s: &mut &[u8]) -> Option<u32> {
    let mut ip = 0u32;
    let mut i = 0;
    for sep in [b'.', b'.', b'.', b' '] {
        let (v, next) = octet(s, i)?;
        if s.get(next) != Some(&sep) {
            return None;
        }
        ip = (ip << 8) | v;
        i = next + 1;
    }
    *s = &s[i..];
    Some(ip)
}

fn parse_line(line: &[u8]) -> Option<Parsed> {
    let mut rest = line;
    number(&mut rest, false)?;
    let ip = ip(&mut rest)?;

    let mut i = 0;
    while i < rest.len() && rest[i].is_ascii_uppercase() {
        i += 1;
    }
    if i == 0 || rest.get(i) != Some(&b' ') {
        return None;
    }
    rest = &rest[i + 1..];

    if rest.first() != Some(&b'/') {
        return None;
    }
    let i = rest.iter().position(|&b| b == b' ')?;
    rest = &rest[i + 1..];

    // Exactly three digits, then a space.
    if rest.len() < 4 || rest[3] != b' ' {
        return None;
    }
    let mut status = 0u64;
    for &b in &rest[..3] {
        let d = b.wrapping_sub(b'0');
        if d > 9 {
            return None;
        }
        status = status * 10 + u64::from(d);
    }
    if !(100..=599).contains(&status) {
        return None;
    }
    rest = &rest[4..];

    let bytes = number(&mut rest, false)?;
    // The last field ends at the end of the line. A stray space fails here.
    let latency = number(&mut rest, true)?;
    Some(Parsed {
        ip,
        status,
        bytes,
        latency,
    })
}

impl Aggregator {
    /// Creates an empty aggregator. The only function allowed to allocate.
    pub fn new() -> Aggregator {
        Aggregator {
            summary: Summary::default(),
            // vec![0; n] uses calloc: the 4 MiB is not touched until used.
            table: vec![0u32; TABLE_SLOTS].into_boxed_slice(),
            stored: 0,
            zero_seen: false,
            carry: [0; CARRY_CAP],
            carry_len: 0,
            carry_start: 0,
            discarding: false,
            consumed: 0,
        }
    }

    fn note_client(&mut self, ip: u32) {
        if ip == 0 {
            self.zero_seen = true;
            return;
        }
        let mask = TABLE_SLOTS - 1;
        // Fibonacci hashing: the top 20 bits of the product index the table.
        let mut i = (ip.wrapping_mul(0x9E37_79B1) >> 12) as usize;
        loop {
            let slot = self.table[i];
            if slot == ip {
                return;
            }
            if slot == 0 {
                if self.stored < MAX_STORED_CLIENTS {
                    self.table[i] = ip;
                    self.stored += 1;
                } else {
                    self.summary.clients_overflow += 1;
                }
                return;
            }
            i = (i + 1) & mask;
        }
    }

    fn record(&mut self, line: &[u8], offset: usize) {
        let Some(p) = parse_line(line) else {
            self.summary.malformed += 1;
            return;
        };
        let s = &mut self.summary;
        s.lines += 1;
        s.by_status_class[(p.status / 100 - 1) as usize] += 1;
        s.total_bytes = s.total_bytes.saturating_add(p.bytes);
        let bucket = (63 - p.latency.saturating_add(1).leading_zeros()) as usize;
        s.latency_hist[bucket.min(HIST_BUCKETS - 1)] += 1;
        // Strict `>` keeps the earliest line among equal maxima.
        if s.slowest.is_none_or(|w| p.latency > w.latency_us) {
            s.slowest = Some(SlowRequest {
                latency_us: p.latency,
                line_offset: offset,
            });
        }
        self.note_client(p.ip);
    }

    /// Consumes the next chunk of the stream. Chunks may end anywhere, including
    /// in the middle of a line.
    pub fn ingest(&mut self, chunk: &[u8]) {
        let base = self.consumed;
        self.consumed += chunk.len();
        let mut pos = 0;

        if self.carry_len > 0 || self.discarding {
            let Some(i) = memchr::memchr(b'\n', chunk) else {
                if !self.discarding {
                    if self.carry_len + chunk.len() > CARRY_CAP {
                        self.summary.malformed += 1;
                        self.discarding = true;
                        self.carry_len = 0;
                    } else {
                        self.carry[self.carry_len..self.carry_len + chunk.len()]
                            .copy_from_slice(chunk);
                        self.carry_len += chunk.len();
                    }
                }
                return;
            };
            if self.discarding {
                self.discarding = false;
            } else if self.carry_len + i > CARRY_CAP {
                self.summary.malformed += 1;
            } else {
                self.carry[self.carry_len..self.carry_len + i].copy_from_slice(&chunk[..i]);
                let len = self.carry_len + i;
                // `record` borrows self mutably, so move the buffer out of the way.
                let line = self.carry;
                self.record(&line[..len], self.carry_start);
            }
            self.carry_len = 0;
            pos = i + 1;
        }

        while pos < chunk.len() {
            let rest = &chunk[pos..];
            let Some(i) = memchr::memchr(b'\n', rest) else {
                if rest.len() > CARRY_CAP {
                    self.summary.malformed += 1;
                    self.discarding = true;
                } else {
                    self.carry[..rest.len()].copy_from_slice(rest);
                    self.carry_len = rest.len();
                    self.carry_start = base + pos;
                }
                return;
            };
            if i > CARRY_CAP {
                self.summary.malformed += 1;
            } else {
                self.record(&rest[..i], base + pos);
            }
            pos += i + 1;
        }
    }

    /// Flushes a final unterminated line, returns the summary, and resets.
    pub fn finish(&mut self) -> Summary {
        if self.carry_len > 0 {
            let len = self.carry_len;
            let line = self.carry;
            self.record(&line[..len], self.carry_start);
        }
        let mut out = std::mem::take(&mut self.summary);
        out.distinct_clients_estimate = self.stored as u64 + u64::from(self.zero_seen);
        self.table.fill(0);
        self.stored = 0;
        self.zero_seen = false;
        self.carry_len = 0;
        self.carry_start = 0;
        self.discarding = false;
        self.consumed = 0;
        out
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}
