use ex01_rate_limiter::TokenBucket;

const SEC: u64 = 1_000_000_000;

#[test]
fn a_new_bucket_allows_a_burst_of_its_capacity() {
    let mut b = TokenBucket::new(3, 1);
    assert!(b.try_acquire(0));
    assert!(b.try_acquire(0));
    assert!(b.try_acquire(0));
    assert!(!b.try_acquire(0));
}

#[test]
fn a_bucket_with_capacity_one_allows_one_request() {
    let mut b = TokenBucket::new(1, 1);
    assert!(b.try_acquire(5 * SEC));
    assert!(!b.try_acquire(5 * SEC));
}

#[test]
fn time_refills_the_bucket() {
    let mut b = TokenBucket::new(2, 2);
    assert!(b.try_acquire(0));
    assert!(b.try_acquire(0));
    assert!(!b.try_acquire(SEC / 4));
    assert!(b.try_acquire(SEC / 2));
    assert!(!b.try_acquire(SEC / 2));
}

#[test]
fn a_partial_unit_is_kept_for_the_next_call() {
    let mut b = TokenBucket::new(1, 1);
    assert!(b.try_acquire(0));
    assert!(!b.try_acquire(SEC / 2));
    assert!(b.try_acquire(SEC));
}

#[test]
fn the_bucket_never_holds_more_than_its_capacity() {
    let mut b = TokenBucket::new(2, 1);
    assert!(b.try_acquire(0));
    assert!(b.try_acquire(0));
    assert!(b.try_acquire(100 * SEC));
    assert!(b.try_acquire(100 * SEC));
    assert!(!b.try_acquire(100 * SEC));
}

#[test]
fn time_going_backwards_grants_nothing() {
    let mut b = TokenBucket::new(1, 1);
    assert!(b.try_acquire(10 * SEC));
    assert!(!b.try_acquire(3 * SEC));
    assert!(!b.try_acquire(10 * SEC));
    assert!(!b.try_acquire(10 * SEC + SEC / 2));
    assert!(b.try_acquire(11 * SEC));
}

/// Same answers as a model that counts units scaled by 1e9.
#[test]
fn random_requests_match_a_simple_model() {
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for _ in 0..200 {
        let capacity = 1 + (next() % 10) as u32;
        let rate = 1 + (next() % 20) as u32;
        let mut b = TokenBucket::new(capacity, rate);
        let (mut level, mut latest) = (u128::from(capacity) * SEC as u128, 0u64);
        let mut now = 0u64;
        for _ in 0..300 {
            let step = next() % (SEC / 2);
            now = if next() % 8 == 0 {
                now.saturating_sub(step)
            } else {
                now + step
            };
            if now > latest {
                level += u128::from(now - latest) * u128::from(rate);
                level = level.min(u128::from(capacity) * SEC as u128);
                latest = now;
            }
            let expected = level >= SEC as u128;
            if expected {
                level -= SEC as u128;
            }
            assert_eq!(b.try_acquire(now), expected);
        }
    }
}

/// Hard rule: no rounding, so a long run allows exactly the right number of requests.
#[test]
fn a_long_run_allows_exactly_the_right_count() {
    let run = |capacity, rate, per_sec: u64, count: u64| {
        let mut b = TokenBucket::new(capacity, rate);
        (0..count)
            .filter(|k| b.try_acquire(k * SEC / per_sec))
            .count()
    };
    assert_eq!(run(10, 5, 20, 200), 59);
    assert_eq!(run(1, 1, 3, 180), 60);
    assert_eq!(run(5, 7, 10, 300), 5 + 7 * 299 / 10);
}
