use ex01_rate_limiter::{Clock, ManualClock, SystemClock, TokenBucket};

#[test]
fn manual_clock_set_and_advance() {
    let c = ManualClock::new(10);
    assert_eq!(c.now_ns(), 10);
    c.advance(5);
    assert_eq!(c.now_ns(), 15);
    c.set(3);
    assert_eq!(c.now_ns(), 3);
    c.advance(u64::MAX);
    assert_eq!(c.now_ns(), u64::MAX);
}

#[test]
fn manual_clock_clones_share_time() {
    let a = ManualClock::new(0);
    let b = a.clone();
    a.advance(7);
    assert_eq!(b.now_ns(), 7);
}

#[test]
fn system_clock_is_monotonic() {
    let c = SystemClock::new();
    let mut last = c.now_ns();
    for _ in 0..1000 {
        let t = c.now_ns();
        assert!(t >= last);
        last = t;
    }
}

#[test]
fn bucket_driven_by_a_manual_clock() {
    let clock = ManualClock::new(0);
    let mut b = TokenBucket::new(2, 10).unwrap();
    assert!(b.try_acquire(clock.now_ns(), 2));
    assert!(!b.try_acquire(clock.now_ns(), 1));
    clock.advance(99_999_999);
    assert!(!b.try_acquire(clock.now_ns(), 1));
    clock.advance(1);
    assert!(b.try_acquire(clock.now_ns(), 1));
}
