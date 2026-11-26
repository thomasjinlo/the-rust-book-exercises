//! Traffic scenarios: "create a limiter, send requests at this rate, count the denials".

use ex01_rate_limiter::TokenBucket;

const SEC: u64 = 1_000_000_000;

/// Sends `count` single-unit requests, request `k` at `k * SEC / per_sec` ns, and
/// returns how many were allowed.
fn send(capacity: u32, rate: u32, per_sec: u64, count: u64) -> u64 {
    let mut b = TokenBucket::new(capacity, rate).unwrap();
    (0..count)
        .filter(|k| b.try_acquire(k * SEC / per_sec, 1))
        .count() as u64
}

// Capacity 10, 5/s, 20 requests per second for 10 s (200 requests, the last at 9.95 s).
// The bucket gains 49.75 units over the run, so at most 10 + 49.75 = 59.75 units can be
// spent, and one unit per request gives exactly 59 allowed and 141 throttled.
#[test]
fn twenty_requests_per_second_against_five_per_second() {
    let allowed = send(10, 5, 20, 200);
    assert_eq!(allowed, 59);
    assert_eq!(200 - allowed, 141);
}

// 100 requests per second for 10 s, last request at 9.99 s: floor(10 + 49.95) = 59.
#[test]
fn a_hundred_requests_per_second_against_five_per_second() {
    assert_eq!(send(10, 5, 100, 1000), 59);
}

// Requests at 6 per second for 10 s (60 requests, the last at 9.1666... s after
// flooring to whole ns): exactly one request is throttled.
#[test]
fn six_requests_per_second_against_five_per_second() {
    assert_eq!(send(10, 5, 6, 60), 59);
}

#[test]
fn a_client_at_exactly_the_sustained_rate_is_never_throttled() {
    assert_eq!(send(10, 5, 5, 500), 500);
}

#[test]
fn a_client_below_the_sustained_rate_is_never_throttled() {
    assert_eq!(send(10, 5, 4, 400), 400);
}

// Capacity 3, 2/s, 10 requests per second for 5 s (50 requests): 12 allowed.
#[test]
fn a_small_burst_with_a_slow_rate() {
    assert_eq!(send(3, 2, 10, 50), 12);
}

// Requests for 3 units every 0.5 s against capacity 10 and 5/s: 19 of 20 allowed.
#[test]
fn requests_for_several_units() {
    let mut b = TokenBucket::new(10, 5).unwrap();
    let allowed = (0..20u64).filter(|k| b.try_acquire(k * SEC / 2, 3)).count();
    assert_eq!(allowed, 19);
}

/// xorshift64: a small seeded PRNG, so every run sees the same inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo)
    }
}

// After a drain and an idle period of `idle` ns, exactly min(capacity, floor(rate * idle / SEC))
// single-unit requests are allowed at that instant.
#[test]
fn after_an_idle_period_the_allowed_burst_matches_the_refill() {
    let mut rng = Rng(0xfeed);
    for _ in 0..300 {
        let capacity = rng.range(1, 200) as u32;
        let rate = rng.range(1, 5000) as u32;
        let idle = rng.range(0, 3 * SEC);
        let mut b = TokenBucket::new(capacity, rate).unwrap();
        assert!(b.try_acquire(0, capacity));
        let expected =
            (u128::from(rate) * u128::from(idle) / u128::from(SEC)).min(u128::from(capacity));
        let got = (0..capacity).take_while(|_| b.try_acquire(idle, 1)).count() as u128;
        assert_eq!(got, expected, "capacity={capacity} rate={rate} idle={idle}");
    }
}

// Splitting an idle interval with requests that can never be allowed (larger than the
// burst) gives the same result as one request after the whole interval.
#[test]
fn two_intervals_split_at_any_point_behave_like_one() {
    let mut rng = Rng(0xcafe);
    for _ in 0..300 {
        let capacity = rng.range(1, 100) as u32;
        let rate = rng.range(1, 5000) as u32;
        let mid = rng.range(0, 5 * SEC);
        let end = mid + rng.range(0, 5 * SEC);
        let mut one = TokenBucket::new(capacity, rate).unwrap();
        let mut two = TokenBucket::new(capacity, rate).unwrap();
        assert!(one.try_acquire(0, capacity) && two.try_acquire(0, capacity));
        assert!(!two.try_acquire(mid, capacity + 1));
        let got_one = (0..capacity)
            .take_while(|_| one.try_acquire(end, 1))
            .count();
        let got_two = (0..capacity)
            .take_while(|_| two.try_acquire(end, 1))
            .count();
        assert_eq!(
            got_one, got_two,
            "capacity={capacity} rate={rate} mid={mid} end={end}"
        );
    }
}

// Requests of random size at random times never spend more than the burst plus the refill
// that the elapsed time can produce.
#[test]
fn granted_units_never_exceed_burst_plus_refill() {
    let mut rng = Rng(0x5eed);
    for _ in 0..300 {
        let capacity = rng.range(1, 50) as u32;
        let rate = rng.range(1, 2000) as u32;
        let mut b = TokenBucket::new(capacity, rate).unwrap();
        let (mut now, mut granted) = (0u64, 0u128);
        for _ in 0..rng.range(1, 200) {
            now += rng.range(0, 3 * SEC);
            let n = rng.range(1, 4) as u32;
            if b.try_acquire(now, n) {
                granted += u128::from(n);
            }
        }
        let bound = u128::from(capacity) + u128::from(rate) * u128::from(now) / u128::from(SEC);
        assert!(granted <= bound, "granted {granted} > bound {bound}");
    }
}

// A request pattern that wanders backwards in time must never be allowed more than the same
// pattern with each timestamp replaced by the latest one seen so far.
#[test]
fn a_clock_that_jumps_around_gets_no_extra_requests() {
    let mut rng = Rng(0xbeef);
    for _ in 0..200 {
        let capacity = rng.range(1, 20) as u32;
        let rate = rng.range(1, 100) as u32;
        let mut jumpy = TokenBucket::new(capacity, rate).unwrap();
        let mut sorted = TokenBucket::new(capacity, rate).unwrap();
        let mut high = 0u64;
        for _ in 0..100 {
            let t = rng.range(0, 20 * SEC);
            high = high.max(t);
            let n = rng.range(0, 4) as u32;
            assert_eq!(jumpy.try_acquire(t, n), sorted.try_acquire(high, n));
        }
    }
}
