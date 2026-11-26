use ex01_rate_limiter::{ConfigError, TokenBucket};

const SEC: u64 = 1_000_000_000;

fn drain(b: &mut TokenBucket, now: u64) {
    while b.try_acquire(now, 1) {}
}

#[test]
fn rejects_zero_capacity() {
    assert_eq!(
        TokenBucket::new(0, 5).unwrap_err(),
        ConfigError::ZeroCapacity
    );
}

#[test]
fn rejects_zero_rate() {
    assert_eq!(TokenBucket::new(5, 0).unwrap_err(), ConfigError::ZeroRate);
}

#[test]
fn zero_capacity_wins_over_zero_rate() {
    assert_eq!(
        TokenBucket::new(0, 0).unwrap_err(),
        ConfigError::ZeroCapacity
    );
}

#[test]
fn errors_display_and_implement_error() {
    let e: Box<dyn std::error::Error> = Box::new(ConfigError::ZeroRate);
    assert!(!e.to_string().is_empty());
    assert_ne!(
        ConfigError::ZeroCapacity.to_string(),
        ConfigError::ZeroRate.to_string()
    );
    assert_ne!(
        ConfigError::ZeroRate.to_string(),
        ConfigError::ZeroMaxKeys.to_string()
    );
}

#[test]
fn starts_full() {
    let mut b = TokenBucket::new(5, 1).unwrap();
    assert_eq!(b.available(0), 5);
    for _ in 0..5 {
        assert!(b.try_acquire(0, 1));
    }
    assert!(!b.try_acquire(0, 1));
}

#[test]
fn acquire_is_all_or_nothing() {
    let mut b = TokenBucket::new(5, 1).unwrap();
    assert!(b.try_acquire(0, 3));
    assert!(!b.try_acquire(0, 3));
    assert_eq!(b.available(0), 2);
    assert!(b.try_acquire(0, 2));
}

#[test]
fn zero_tokens_always_succeeds() {
    let mut b = TokenBucket::new(1, 1).unwrap();
    drain(&mut b, 0);
    assert!(b.try_acquire(0, 0));
}

#[test]
fn more_than_capacity_fails_and_changes_nothing() {
    let mut b = TokenBucket::new(5, 1).unwrap();
    assert!(!b.try_acquire(0, 6));
    assert!(!b.try_acquire(0, u32::MAX));
    assert_eq!(b.available(0), 5);
}

#[test]
fn exact_boundary_one_token_per_second() {
    let mut b = TokenBucket::new(1, 1).unwrap();
    assert!(b.try_acquire(0, 1));
    assert!(
        !b.try_acquire(SEC - 1, 1),
        "999_999_999 ns is not a full token"
    );
    assert!(
        b.try_acquire(SEC, 1),
        "1_000_000_000 ns is exactly one token"
    );
}

#[test]
fn failed_acquire_does_not_lose_elapsed_time() {
    let mut b = TokenBucket::new(1, 1).unwrap();
    assert!(b.try_acquire(0, 1));
    assert!(!b.try_acquire(SEC - 1, 1));
    assert!(b.try_acquire(SEC, 1));
}

#[test]
fn fractional_carry_rate_three() {
    // 333_333_334 ns * 3 / 1e9 = 1.000000002 tokens, so each call after the first
    // gains one token. 333_333_333 ns would gain 0.999999999.
    let mut b = TokenBucket::new(1, 3).unwrap();
    assert!(b.try_acquire(0, 1));
    let step = 333_333_334;
    assert!(b.try_acquire(step, 1));
    assert!(b.try_acquire(2 * step, 1));
    assert!(b.try_acquire(3 * step, 1));
}

#[test]
fn fractional_remainder_accumulates_across_calls() {
    // Rate 3, 100 ms steps: 0.3 tokens each. Tokens arrive at 400 ms (1.2 total),
    // 700 ms (2.1 total) and 1000 ms (3.0 total).
    let mut b = TokenBucket::new(10, 3).unwrap();
    for _ in 0..10 {
        assert!(b.try_acquire(0, 1));
    }
    let mut got = Vec::new();
    for i in 1..=10u64 {
        got.push(b.try_acquire(i * 100_000_000, 1));
    }
    assert_eq!(
        got,
        [
            false, false, false, true, false, false, true, false, false, true
        ]
    );
}

#[test]
fn small_steps_equal_one_big_step() {
    let mut a = TokenBucket::new(100, 7).unwrap();
    let mut b = TokenBucket::new(100, 7).unwrap();
    drain(&mut a, 0);
    drain(&mut b, 0);
    for t in 1..=1000u64 {
        a.available(t * 1_000_003);
    }
    assert_eq!(a.available(1_000_003_000), b.available(1_000_003_000));
}

#[test]
fn cap_after_long_idle() {
    let mut b = TokenBucket::new(5, 100).unwrap();
    drain(&mut b, 0);
    assert_eq!(b.available(3600 * SEC), 5);
    for _ in 0..5 {
        assert!(b.try_acquire(3600 * SEC, 1));
    }
    assert!(!b.try_acquire(3600 * SEC, 1));
}

#[test]
fn clock_going_backwards_adds_nothing() {
    let mut b = TokenBucket::new(2, 1).unwrap();
    drain(&mut b, 10 * SEC);
    assert_eq!(b.available(5 * SEC), 0);
    assert!(!b.try_acquire(0, 1));
}

#[test]
fn backwards_step_does_not_double_count_time() {
    let mut b = TokenBucket::new(2, 1).unwrap();
    drain(&mut b, 10 * SEC);
    assert_eq!(b.available(9 * SEC), 0);
    // 10.5 s is 0.5 s after the last accepted time (10 s), not 1.5 s after 9 s.
    assert_eq!(b.available(10 * SEC + SEC / 2), 0);
    assert_eq!(b.available(11 * SEC), 1);
}

#[test]
fn max_timestamp_does_not_panic_or_overflow() {
    let mut b = TokenBucket::new(u32::MAX, u32::MAX).unwrap();
    drain_n(&mut b, 0, 10);
    assert_eq!(b.available(u64::MAX), u32::MAX);
    assert!(b.try_acquire(u64::MAX, u32::MAX));
    assert_eq!(b.available(u64::MAX), 0);
    assert!(!b.try_acquire(0, 1));
}

fn drain_n(b: &mut TokenBucket, now: u64, n: u32) {
    for _ in 0..n {
        assert!(b.try_acquire(now, 1));
    }
}

#[test]
fn max_timestamp_small_bucket() {
    let mut b = TokenBucket::new(3, 1).unwrap();
    drain(&mut b, 0);
    assert_eq!(b.available(u64::MAX), 3);
}

#[test]
fn large_burst_of_whole_capacity() {
    let mut b = TokenBucket::new(u32::MAX, 1).unwrap();
    assert!(b.try_acquire(0, u32::MAX));
    assert!(!b.try_acquire(0, 1));
}
