mod common;

use common::{run, run_chunked};
use ex13_fast_log_aggregation::{Aggregator, CARRY_CAP, MAX_STORED_CLIENTS, SlowRequest, Summary};

const OK: &str = "1700000000000 10.0.0.9 GET /ok 200 1 1\n";

#[test]
fn empty_input() {
    assert_eq!(run(b""), Summary::default());
    assert_eq!(run(b"").slowest, None);
}

#[test]
fn hand_written_valid_lines() {
    let l1 = "1700000000000 10.0.0.1 GET /a 200 100 0\n";
    let l2 = "1700000000001 10.0.0.2 POST /b 404 50 1\n";
    let l3 = "1700000000002 0.0.0.0 GET /c 503 7 1000\n";
    // No trailing newline on the last line.
    let l4 = "1700000000003 10.0.0.1 PUT / 101 0 1000";
    let input = format!("{l1}{l2}{l3}{l4}");

    let mut hist = [0u64; 32];
    hist[0] = 1; // latency 0
    hist[1] = 1; // latency 1
    hist[9] = 2; // latency 1000: floor(log2(1001)) = 9
    let expected = Summary {
        lines: 4,
        malformed: 0,
        by_status_class: [1, 1, 0, 1, 1],
        total_bytes: 157,
        latency_hist: hist,
        distinct_clients_estimate: 3,
        clients_overflow: 0,
        slowest: Some(SlowRequest {
            latency_us: 1000,
            line_offset: l1.len() + l2.len(),
        }),
    };
    assert_eq!(run(input.as_bytes()), expected);
    assert_eq!(run_chunked(input.as_bytes(), 1), expected);
}

#[test]
fn each_malformed_variant_is_counted_and_skipped() {
    let cases: &[(&str, &str)] = &[
        ("missing field", "1 10.0.0.1 GET /a 200 5"),
        ("non-numeric status", "1 10.0.0.1 GET /a 2OO 5 5"),
        ("status 99", "1 10.0.0.1 GET /a 99 5 5"),
        ("status 099", "1 10.0.0.1 GET /a 099 5 5"),
        ("status 600", "1 10.0.0.1 GET /a 600 5 5"),
        ("status 1000", "1 10.0.0.1 GET /a 1000 5 5"),
        ("octet 256", "1 10.0.256.1 GET /a 200 5 5"),
        ("three octets", "1 10.0.1 GET /a 200 5 5"),
        ("five octets", "1 10.0.0.1.1 GET /a 200 5 5"),
        ("leading zero octet", "1 10.00.0.1 GET /a 200 5 5"),
        ("negative bytes", "1 10.0.0.1 GET /a 200 -5 5"),
        ("plus sign latency", "1 10.0.0.1 GET /a 200 5 +5"),
        ("negative timestamp", "-1 10.0.0.1 GET /a 200 5 5"),
        ("extra field", "1 10.0.0.1 GET /a 200 5 5 extra"),
        ("double space", "1 10.0.0.1  GET /a 200 5 5"),
        ("trailing space", "1 10.0.0.1 GET /a 200 5 5 "),
        ("lowercase method", "1 10.0.0.1 get /a 200 5 5"),
        ("path without slash", "1 10.0.0.1 GET a 200 5 5"),
        ("carriage return", "1 10.0.0.1 GET /a 200 5 5\r"),
        (
            "u64 overflow",
            "1 10.0.0.1 GET /a 200 18446744073709551616 5",
        ),
        ("empty line", ""),
        ("only a space", " "),
    ];
    for (name, bad) in cases {
        let input = format!("{bad}\n{OK}{bad}\n");
        for chunk in [input.len(), 3] {
            let s = run_chunked(input.as_bytes(), chunk);
            assert_eq!(s.malformed, 2, "{name} (chunk {chunk})");
            assert_eq!(s.lines, 1, "{name} (chunk {chunk})");
            assert_eq!(
                s.slowest,
                Some(SlowRequest {
                    latency_us: 1,
                    line_offset: bad.len() + 1
                }),
                "{name} (chunk {chunk})"
            );
        }
    }
}

#[test]
fn accepted_edge_values() {
    // Leading zeros are fine in numeric fields, "0" octets are fine, and
    // u64::MAX parses.
    let input = "0007 0.0.0.1 X / 100 18446744073709551615 0000\n\
                 1 255.255.255.255 GET /a 599 1 1\n";
    let s = run(input.as_bytes());
    assert_eq!((s.lines, s.malformed), (2, 0));
    assert_eq!(s.by_status_class, [1, 0, 0, 0, 1]);
    assert_eq!(s.total_bytes, u64::MAX);
    assert_eq!(s.distinct_clients_estimate, 2);
}

#[test]
fn slowest_tie_keeps_lowest_offset() {
    let a = "1 10.0.0.1 GET /a 200 1 500\n";
    let b = "2 10.0.0.1 GET /a 200 1 900\n";
    let c = "3 10.0.0.1 GET /a 200 1 900\n";
    let input = format!("{a}{b}{c}");
    let want = Some(SlowRequest {
        latency_us: 900,
        line_offset: a.len(),
    });
    assert_eq!(run(input.as_bytes()).slowest, want);
    assert_eq!(run_chunked(input.as_bytes(), 5).slowest, want);
}

#[test]
fn latency_buckets_and_saturation() {
    let lats: [u64; 10] = [
        0,
        1,
        2,
        3,
        6,
        7,
        (1 << 31) - 2,
        (1 << 31) - 1,
        1 << 40,
        u64::MAX,
    ];
    let mut input = String::new();
    for l in lats {
        input.push_str(&format!("1 10.0.0.1 GET /a 200 {} {l}\n", u64::MAX));
    }
    let s = run(input.as_bytes());
    let mut want = [0u64; 32];
    want[0] = 1; // 0
    want[1] = 2; // 1, 2
    want[2] = 2; // 3, 6
    want[3] = 1; // 7
    want[30] = 1; // 2^31 - 2: log2(2^31 - 1) < 31
    want[31] = 3; // 2^31 - 1, 2^40, u64::MAX
    assert_eq!(s.latency_hist, want);
    assert_eq!(s.total_bytes, u64::MAX, "total_bytes saturates");
    assert_eq!(s.slowest.unwrap().latency_us, u64::MAX);
}

fn line_of_len(n: usize) -> String {
    let prefix = "1 10.0.0.1 GET /";
    let suffix = " 200 1 1";
    let pad = n - prefix.len() - suffix.len();
    format!("{prefix}{}{suffix}", "p".repeat(pad))
}

#[test]
fn line_length_limit_is_carry_cap() {
    let at_limit = line_of_len(CARRY_CAP);
    let over = line_of_len(CARRY_CAP + 1);
    let input = format!("{at_limit}\n{over}\n{OK}{over}");
    for chunk in [input.len(), 1, 100, CARRY_CAP, CARRY_CAP + 1] {
        let s = run_chunked(input.as_bytes(), chunk);
        assert_eq!(s.lines, 2, "chunk {chunk}");
        assert_eq!(s.malformed, 2, "chunk {chunk}");
    }
    // An over-long line without any newline is counted exactly once.
    let s = run_chunked(over.as_bytes(), 10);
    assert_eq!((s.lines, s.malformed), (0, 1));
    // A very long garbage line is skipped up to its newline, then parsing resumes.
    let junk = "x".repeat(3 * CARRY_CAP + 17);
    let input = format!("{junk}\n{OK}");
    for chunk in [input.len(), 1, 4096, 5000] {
        let s = run_chunked(input.as_bytes(), chunk);
        assert_eq!((s.lines, s.malformed), (1, 1), "chunk {chunk}");
        assert_eq!(
            s.slowest.unwrap().line_offset,
            junk.len() + 1,
            "chunk {chunk}"
        );
    }
}

#[test]
fn offsets_count_from_stream_start_across_chunks() {
    let first = "1 10.0.0.1 GET /a 200 1 10\n";
    let second = "2 10.0.0.1 GET /a 200 1 99999\n";
    let input = format!("{first}{second}");
    for chunk in 1..=input.len() {
        let s = run_chunked(input.as_bytes(), chunk);
        assert_eq!(s.slowest.unwrap().line_offset, first.len(), "chunk {chunk}");
    }
}

#[test]
fn finish_resets_state() {
    let data = "1 10.0.0.1 GET /a 200 1 10\n2 10.0.0.2 GET /a 500 1 20\n3 10.0.0.3";
    let mut agg = Aggregator::new();
    agg.ingest(data.as_bytes());
    let first = agg.finish();
    assert_eq!((first.lines, first.malformed), (2, 1));
    assert_eq!(first.distinct_clients_estimate, 2);
    // Same input again gives the same summary: client table, carry buffer and
    // offsets were cleared.
    agg.ingest(data.as_bytes());
    assert_eq!(agg.finish(), first);
    assert_eq!(agg.finish(), Summary::default());
}

#[test]
fn client_table_limit_and_overflow() {
    let total = 1_000_000usize;
    let ip = |i: usize| {
        let v = (10u32 << 24) + i as u32;
        let b = v.to_be_bytes();
        format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
    };
    let mut input = String::with_capacity(total * 40);
    for i in 0..total {
        input.push_str(&format!("1 {} GET / 200 0 0\n", ip(i)));
    }
    let s = run_chunked(input.as_bytes(), 1 << 20);
    assert_eq!(MAX_STORED_CLIENTS, 943_718);
    assert_eq!(s.lines, total as u64);
    assert_eq!(s.distinct_clients_estimate, MAX_STORED_CLIENTS as u64);
    assert_eq!(s.clients_overflow, (total - MAX_STORED_CLIENTS) as u64);

    // A stored address never overflows. A rejected address counts every time.
    // 0.0.0.0 bypasses the table and still counts as distinct.
    let tail = format!(
        "1 {} GET / 200 0 0\n1 {} GET / 200 0 0\n1 {} GET / 200 0 0\n1 {} GET / 200 0 0\n",
        ip(0),
        ip(total - 1),
        ip(total - 1),
        "0.0.0.0"
    );
    input.push_str(&tail);
    let s = run_chunked(input.as_bytes(), 1 << 20);
    assert_eq!(s.distinct_clients_estimate, MAX_STORED_CLIENTS as u64 + 1);
    assert_eq!(s.clients_overflow, (total - MAX_STORED_CLIENTS) as u64 + 2);
}
