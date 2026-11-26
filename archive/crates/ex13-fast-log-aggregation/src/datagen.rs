//! Deterministic access-log generator. Do not edit.

/// splitmix64: small, fast, and good enough for test data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn push_u64(out: &mut Vec<u8>, mut v: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    out.extend_from_slice(&buf[i..]);
}

fn push_ip(out: &mut Vec<u8>, ip: u32) {
    for (i, b) in ip.to_be_bytes().iter().enumerate() {
        if i > 0 {
            out.push(b'.');
        }
        push_u64(out, u64::from(*b));
    }
}

const METHODS: [&str; 6] = ["GET", "GET", "GET", "POST", "PUT", "DELETE"];
const PATHS: [&str; 8] = [
    "/",
    "/index.html",
    "/api/v1/items",
    "/api/v1/items/",
    "/static/app.js",
    "/static/site.css",
    "/login",
    "/healthz",
];

/// Generates `lines` newline-terminated lines. About 2% are malformed.
///
/// Client addresses come from a pool of 50,000 with a skewed choice, so a few
/// clients are busy and most are rare. Latency is roughly log-uniform from 50 us
/// to 2 s. The same `(lines, seed)` always yields the same bytes.
pub fn generate(lines: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng(seed);
    let mut out = Vec::with_capacity(lines * 64);
    let mut ts = 1_700_000_000_000u64;
    for _ in 0..lines {
        ts += rng.below(5);
        let pool = rng.below(50_000);
        // Squaring the uniform draw skews toward low pool indexes.
        let idx = (pool * pool / 50_000) as u32;
        // Multiplicative scatter keeps the 24 host bits distinct for distinct idx.
        let ip = 0x0A00_0000u32 | (idx.wrapping_mul(2_654_435_761) >> 8);
        let method = METHODS[rng.below(METHODS.len() as u64) as usize];
        let status = match rng.below(100) {
            0..=79 => 200,
            80..=86 => 304,
            87..=90 => 301,
            91..=94 => 404,
            95 => 403,
            96 => 500,
            97 => 503,
            98 => 100,
            _ => 201,
        };
        let bytes_bits = 4 + rng.below(16);
        let bytes = rng.below(1u64 << bytes_bits);
        let lat_bits = 1 + rng.below(21);
        let latency = 50 + rng.below(1u64 << lat_bits);
        let path = PATHS[rng.below(PATHS.len() as u64) as usize];
        let path_id = rng.below(100_000);

        // 0 missing field, 1 non-numeric status, 2 status 99, 3 status 600,
        // 4 octet 256, 5 negative bytes, 6 extra field, 7 empty line.
        let kind = if rng.below(100) < 2 { rng.below(8) } else { 99 };
        if kind == 7 {
            out.push(b'\n');
            continue;
        }
        push_u64(&mut out, ts);
        out.push(b' ');
        if kind == 4 {
            out.extend_from_slice(b"10.0.256.1");
        } else {
            push_ip(&mut out, ip);
        }
        out.push(b' ');
        out.extend_from_slice(method.as_bytes());
        out.push(b' ');
        out.extend_from_slice(path.as_bytes());
        if path.len() > 1 && path.ends_with('/') {
            push_u64(&mut out, path_id);
        }
        out.push(b' ');
        match kind {
            1 => out.extend_from_slice(b"2OO"),
            2 => out.extend_from_slice(b"99"),
            3 => out.extend_from_slice(b"600"),
            _ => push_u64(&mut out, status),
        }
        out.push(b' ');
        if kind == 5 {
            out.push(b'-');
        }
        push_u64(&mut out, bytes);
        if kind == 0 {
            out.push(b'\n');
            continue;
        }
        out.push(b' ');
        push_u64(&mut out, latency);
        if kind == 6 {
            out.extend_from_slice(b" extra");
        }
        out.push(b'\n');
    }
    out
}
