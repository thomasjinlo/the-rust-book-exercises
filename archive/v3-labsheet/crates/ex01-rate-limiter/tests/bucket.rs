use ex01_rate_limiter::{ConfigError, TokenBucket};

const SEC: u64 = 1_000_000_000;

/// A limiter whose whole burst was used at t = 0.
fn drained(capacity: u32, rate: u32) -> TokenBucket {
    let mut b = TokenBucket::new(capacity, rate).unwrap();
    assert!(
        b.try_acquire(0, capacity),
        "a new limiter must allow a full burst"
    );
    b
}

/// Number of single-unit requests allowed at one instant before the first denial.
fn burst_at(b: &mut TokenBucket, t: u64, limit: u32) -> u32 {
    (0..limit).take_while(|_| b.try_acquire(t, 1)).count() as u32
}

#[test]
fn zero_capacity_is_rejected() {
    assert_eq!(
        TokenBucket::new(0, 5).unwrap_err(),
        ConfigError::ZeroCapacity
    );
}

#[test]
fn zero_rate_is_rejected() {
    assert_eq!(TokenBucket::new(5, 0).unwrap_err(), ConfigError::ZeroRate);
}

#[test]
fn zero_capacity_is_reported_before_zero_rate() {
    assert_eq!(
        TokenBucket::new(0, 0).unwrap_err(),
        ConfigError::ZeroCapacity
    );
}

#[test]
fn config_errors_have_distinct_messages_and_are_std_errors() {
    let all = [
        ConfigError::ZeroCapacity,
        ConfigError::ZeroRate,
        ConfigError::ZeroMaxKeys,
    ];
    let msgs: Vec<String> = all.iter().map(|e| e.to_string()).collect();
    for (i, m) in msgs.iter().enumerate() {
        assert!(!m.is_empty());
        assert!(
            msgs[i + 1..].iter().all(|o| o != m),
            "duplicate message {m:?}"
        );
    }
    let boxed: Box<dyn std::error::Error> = Box::new(ConfigError::ZeroRate);
    assert_eq!(boxed.to_string(), ConfigError::ZeroRate.to_string());
}

#[test]
fn an_idle_limiter_allows_a_full_burst_then_denies() {
    let mut b = TokenBucket::new(5, 1).unwrap();
    assert_eq!(burst_at(&mut b, 0, 100), 5);
    assert!(!b.try_acquire(0, 1));
}

#[test]
fn a_request_for_several_units_is_all_or_nothing() {
    let mut b = TokenBucket::new(5, 1).unwrap();
    assert!(b.try_acquire(0, 3));
    assert!(!b.try_acquire(0, 3), "only 2 units are left");
    assert!(
        b.try_acquire(0, 2),
        "the denied request must not have taken anything"
    );
    assert!(!b.try_acquire(0, 1));
}

#[test]
fn a_request_for_zero_units_is_always_allowed() {
    let mut b = drained(3, 1);
    assert!(b.try_acquire(0, 0));
    assert!(b.try_acquire(0, 0));
    assert!(
        !b.try_acquire(0, 1),
        "zero-unit requests must not add units"
    );
}

#[test]
fn requests_larger_than_the_burst_are_always_denied_and_change_nothing() {
    let (cap, rate) = (4, 3);
    let mut with_oversize = TokenBucket::new(cap, rate).unwrap();
    let mut plain = TokenBucket::new(cap, rate).unwrap();
    let schedule: [(u64, u32); 8] = [
        (0, 3),
        (SEC / 10, 1),
        (SEC / 2, 2),
        (SEC / 2, 1),
        (SEC, 3),
        (3 * SEC, 4),
        (3 * SEC, 1),
        (10 * SEC, 4),
    ];
    for (t, n) in schedule {
        assert!(!with_oversize.try_acquire(t, cap + 1));
        assert!(!with_oversize.try_acquire(t, u32::MAX));
        assert_eq!(
            with_oversize.try_acquire(t, n),
            plain.try_acquire(t, n),
            "t={t} n={n}"
        );
    }
}

#[test]
fn one_unit_per_second_at_rate_one() {
    let mut b = drained(1, 1);
    assert!(!b.try_acquire(SEC - 1, 1));
    assert!(b.try_acquire(SEC, 1));
    assert!(!b.try_acquire(SEC, 1));
}

#[test]
fn a_rate_of_three_per_second_allows_one_unit_per_third_of_a_second() {
    let mut b = drained(3, 3);
    assert!(!b.try_acquire(333_333_333, 1));
    assert!(b.try_acquire(333_333_334, 1));
    assert!(!b.try_acquire(333_333_334, 1));
}

#[test]
fn denied_requests_do_not_make_time_count_twice() {
    let mut b = drained(3, 1);
    assert!(!b.try_acquire(SEC / 2, 3));
    assert!(!b.try_acquire(SEC, 3));
    assert!(
        !b.try_acquire(SEC + SEC / 2, 3),
        "only 1.5 units have accumulated"
    );
    assert!(b.try_acquire(SEC + SEC / 2, 1));
    assert!(!b.try_acquire(SEC + SEC / 2, 1), "0.5 units are left");
}

#[test]
fn many_tiny_denied_requests_do_not_lose_time() {
    // Each denied probe, 1 ms apart, covers 0.003 units at 3 units/s. At t = 1 s the
    // limiter has gained exactly 3 units, however many probes came before.
    let mut b = drained(10, 3);
    for i in 1..1000u64 {
        assert!(!b.try_acquire(i * (SEC / 1000), 10));
    }
    assert!(b.try_acquire(SEC, 3));
    assert!(!b.try_acquire(SEC, 1));
}

#[test]
fn long_idle_never_allows_more_than_the_burst() {
    let mut b = drained(10, 5);
    let hour = 3600 * SEC;
    assert_eq!(burst_at(&mut b, hour, 1000), 10);
}

#[test]
fn a_clock_going_backwards_grants_nothing() {
    let mut b = TokenBucket::new(2, 1).unwrap();
    assert!(b.try_acquire(10 * SEC, 2));
    assert!(!b.try_acquire(5 * SEC, 1), "going back must not refill");
    assert!(
        !b.try_acquire(10 * SEC + SEC / 2, 1),
        "only 0.5 units since t=10s"
    );
    assert!(b.try_acquire(11 * SEC, 1));
    assert!(!b.try_acquire(11 * SEC, 1));
}

#[test]
fn time_is_not_counted_twice_after_the_clock_catches_up() {
    let mut b = TokenBucket::new(5, 1).unwrap();
    assert!(b.try_acquire(10 * SEC, 5));
    assert!(!b.try_acquire(8 * SEC, 5));
    assert!(!b.try_acquire(9 * SEC, 1));
    assert!(
        !b.try_acquire(10 * SEC, 1),
        "the 8s..10s span is already behind us"
    );
    assert!(b.try_acquire(11 * SEC, 1));
    assert!(!b.try_acquire(11 * SEC, 1));
}

#[test]
fn the_largest_timestamp_does_not_panic() {
    let mut b = TokenBucket::new(u32::MAX, u32::MAX).unwrap();
    assert!(b.try_acquire(0, u32::MAX));
    assert!(
        b.try_acquire(u64::MAX, u32::MAX),
        "a huge idle refills to the burst"
    );
    assert!(!b.try_acquire(u64::MAX, 1));
    assert!(!b.try_acquire(0, 1));
}

#[test]
fn a_small_limiter_near_the_largest_timestamp_stays_exact() {
    let mut b = TokenBucket::new(1, 1).unwrap();
    assert!(b.try_acquire(u64::MAX - 1, 1));
    assert!(
        !b.try_acquire(u64::MAX, 1),
        "one nanosecond adds almost nothing"
    );
    let mut c = TokenBucket::new(1, 1).unwrap();
    assert!(c.try_acquire(u64::MAX, 1));
    assert!(!c.try_acquire(u64::MAX, 1));
}
