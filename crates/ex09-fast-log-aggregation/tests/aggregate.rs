use ex09_fast_log_aggregation::{Summary, aggregate};

#[test]
fn four_lines_give_exact_totals() {
    let input = b"/ok 200 512 40\n/missing 404 0 900\ngarbage\n/x 500 100 700";
    assert_eq!(
        aggregate(input),
        Summary {
            lines: 3,
            malformed: 1,
            errors: 1,
            total_bytes: 612,
            max_latency_us: 900,
        }
    );
}

#[test]
fn empty_input_gives_the_default_summary() {
    assert_eq!(aggregate(b""), Summary::default());
}

#[test]
fn a_last_line_without_a_newline_still_counts() {
    assert_eq!(aggregate(b"/a 200 1 1").lines, 1);
    assert_eq!(aggregate(b"/a 200 1 1\n").lines, 1);
    assert_eq!(aggregate(b"/a 200 1 1\n/b 200 1 1").lines, 2);
}

#[test]
fn empty_lines_are_malformed() {
    let s = aggregate(b"\n\n/a 200 1 1\n");
    assert_eq!((s.lines, s.malformed), (1, 2));
}

#[test]
fn lines_that_break_the_format_are_malformed() {
    let bad: [&[u8]; 11] = [
        b"/a 99 5 5",
        b"/a 600 5 5",
        b"/a 2000 5 5",
        b"/a 20x 5 5",
        b"/a 200 -5 5",
        b"/a 200 5",
        b"/a 200 5 5 5",
        b"/a 200 5 5 ",
        b" /a 200 5 5",
        b"/a  200 5 5",
        b"/a 200 18446744073709551616 5",
    ];
    for line in bad {
        let s = aggregate(line);
        let text = String::from_utf8_lossy(line);
        assert_eq!((s.lines, s.malformed), (0, 1), "{text:?}");
    }
}

#[test]
fn edge_values_are_valid() {
    let s = aggregate(b"/ 100 18446744073709551615 0000\n/a 599 0 18446744073709551615");
    assert_eq!(s.lines, 2);
    assert_eq!(s.total_bytes, u64::MAX);
    assert_eq!(s.max_latency_us, u64::MAX);
}

#[test]
fn only_statuses_500_to_599_are_errors() {
    let s = aggregate(b"/a 499 1 1\n/a 500 1 1\n/a 599 1 1\n/a 200 1 1");
    assert_eq!((s.lines, s.errors), (4, 2));
}

#[test]
fn max_latency_is_zero_without_a_valid_line() {
    let s = aggregate(b"nope\n/a 200 1 7\n/a 200 1 3");
    assert_eq!(s.max_latency_us, 7);
    assert_eq!(aggregate(b"nope").max_latency_us, 0);
}

/// Same totals as a `String` and `str::split` model.
#[test]
fn random_input_matches_a_simple_model() {
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let pieces = [
        "/a",
        "/b/c",
        "200",
        "404",
        "503",
        "99",
        "7",
        "18446744073709551616",
        "x",
        "-1",
        "",
        " ",
    ];
    for _ in 0..300 {
        let mut text = String::new();
        for _ in 0..(next() % 12) {
            if next() % 3 == 0 {
                for i in 0..(next() % 6) {
                    if i > 0 {
                        text.push(' ');
                    }
                    text.push_str(pieces[(next() % pieces.len() as u64) as usize]);
                }
            } else {
                let (st, b, l) = (100 + next() % 500, next() % 10_000, next() % 5_000);
                text.push_str(&format!("/p {st} {b} {l}"));
            }
            text.push('\n');
        }
        if next() % 2 == 0 && text.ends_with('\n') {
            text.pop();
        }
        assert_eq!(aggregate(text.as_bytes()), model(&text), "{text:?}");
    }
}

fn model(text: &str) -> Summary {
    let mut s = Summary::default();
    let lines: Vec<&str> = text.split('\n').collect();
    let count = if text.ends_with('\n') || text.is_empty() {
        lines.len() - 1
    } else {
        lines.len()
    };
    for line in &lines[..count] {
        let f: Vec<&str> = line.split(' ').collect();
        let digits = |x: &&str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
        let ok = f.len() == 4
            && !f[0].is_empty()
            && f[1].len() == 3
            && f[1..].iter().all(digits)
            && (100..=599).contains(&f[1].parse::<u64>().unwrap())
            && f[2].parse::<u64>().is_ok()
            && f[3].parse::<u64>().is_ok();
        if ok {
            let status: u64 = f[1].parse().unwrap();
            s.lines += 1;
            s.errors += u64::from(status >= 500);
            s.total_bytes += f[2].parse::<u64>().unwrap();
            s.max_latency_us = s.max_latency_us.max(f[3].parse().unwrap());
        } else {
            s.malformed += 1;
        }
    }
    s
}
