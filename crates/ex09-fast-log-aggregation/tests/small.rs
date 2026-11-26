use ex09_fast_log_aggregation::{Aggregator, SlowRequest, Summary};

const OK: &str = "1700000000000 10.0.0.9 GET /ok 200 1 1\n";

fn run(input: &str) -> Summary {
    Aggregator::new().aggregate(input.as_bytes())
}

// Milestone 1

#[test]
fn empty_input() {
    assert_eq!(run(""), Summary::default());
}

#[test]
fn line_splitting() {
    // Only empty lines: each one is malformed, none is valid.
    let s = run("\n\n\n");
    assert_eq!((s.lines, s.malformed), (0, 3));
    // A final `\n` does not start another line.
    assert_eq!(run("\n").malformed, 1);
    assert_eq!(run("\n\n").malformed, 2);
}

// Milestone 2

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
        ("empty octet", "1 10..0.1 GET /a 200 5 5"),
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
        // The bad line comes first and last, around one valid line.
        let s = run(&format!("{bad}\n{OK}{bad}\n"));
        assert_eq!((s.lines, s.malformed), (1, 2), "{name}");
        // An unterminated last line counts too (an empty one is not a line).
        if !bad.is_empty() {
            let s = run(&format!("{OK}{bad}"));
            assert_eq!((s.lines, s.malformed), (1, 1), "{name}, unterminated");
        }
    }
}

#[test]
fn accepted_edge_values() {
    // Leading zeros are fine in numeric fields, "0" octets are fine, and
    // u64::MAX parses.
    let input = "0007 0.0.0.1 X / 100 18446744073709551615 0000\n\
                 1 255.255.255.255 GET /a 599 1 1\n";
    let s = run(input);
    assert_eq!((s.lines, s.malformed), (2, 0));
}

// Milestone 3

#[test]
fn status_classes_and_saturating_bytes() {
    let mut input = String::new();
    for status in [100, 199, 200, 299, 300, 404, 499, 500, 599, 204] {
        input.push_str(&format!("1 10.0.0.1 GET /a {status} {} 1\n", u64::MAX));
    }
    let s = run(&input);
    assert_eq!(s.by_status_class, [2, 3, 1, 2, 2]);
    assert_eq!(s.total_bytes, u64::MAX);
}

#[test]
fn slowest_tie_keeps_lowest_offset() {
    let a = "1 10.0.0.1 GET /a 200 1 500\n";
    let b = "2 10.0.0.1 GET /a 200 1 900\n";
    let c = "3 10.0.0.1 GET /a 200 1 900\n";
    let want = Some(SlowRequest {
        latency_us: 900,
        line_offset: a.len(),
    });
    assert_eq!(run(&format!("{a}{b}{c}")).slowest, want);
    // A malformed line before it moves the offset but never wins.
    let bad = "garbage\n";
    let want = Some(SlowRequest {
        latency_us: 900,
        line_offset: bad.len() + a.len(),
    });
    assert_eq!(run(&format!("{bad}{a}{b}{c}")).slowest, want);
}

#[test]
fn hand_written_valid_lines() {
    let l1 = "1700000000000 10.0.0.1 GET /a 200 100 0\n";
    let l2 = "1700000000001 10.0.0.2 POST /b 404 50 1\n";
    let l3 = "1700000000002 0.0.0.0 GET /c 503 7 1000\n";
    // No trailing newline on the last line.
    let l4 = "1700000000003 10.0.0.1 PUT / 101 0 1000";
    let input = format!("{l1}{l2}{l3}{l4}");
    let expected = Summary {
        lines: 4,
        malformed: 0,
        by_status_class: [1, 1, 0, 1, 1],
        total_bytes: 157,
        slowest: Some(SlowRequest {
            latency_us: 1000,
            line_offset: l1.len() + l2.len(),
        }),
    };
    let mut agg = Aggregator::new();
    assert_eq!(agg.aggregate(input.as_bytes()), expected);
    // A second call does not see the first call's data.
    assert_eq!(agg.aggregate(input.as_bytes()), expected);
}
