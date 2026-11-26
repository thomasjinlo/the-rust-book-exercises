//! Randomized checks with a fixed seed, so every run sees the same inputs.

use ex01_rate_limiter::TokenBucket;

const SEC: u64 = 1_000_000_000;

/// xorshift64: small deterministic PRNG, no dependency needed.
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

// Tokens granted can never exceed the initial capacity plus what refill produced.
#[test]
fn granted_tokens_bounded_by_capacity_plus_refill() {
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

#[test]
fn available_never_exceeds_capacity() {
    let mut rng = Rng(0xfeed);
    for _ in 0..300 {
        let capacity = rng.range(1, 1000) as u32;
        let rate = rng.range(1, u64::from(u32::MAX)) as u32;
        let mut b = TokenBucket::new(capacity, rate).unwrap();
        for _ in 0..50 {
            assert!(b.available(rng.next()) <= capacity);
        }
    }
}

// Refill does not depend on how the interval is split into calls.
#[test]
fn splitting_an_interval_changes_nothing() {
    let mut rng = Rng(0xcafe);
    for _ in 0..300 {
        let capacity = rng.range(1, 100) as u32;
        let rate = rng.range(1, 5000) as u32;
        let mid = rng.range(0, 5 * SEC);
        let end = mid + rng.range(0, 5 * SEC);
        let mut one = TokenBucket::new(capacity, rate).unwrap();
        let mut two = TokenBucket::new(capacity, rate).unwrap();
        while one.try_acquire(0, 1) {}
        while two.try_acquire(0, 1) {}
        two.available(mid);
        assert_eq!(one.available(end), two.available(end));
    }
}
