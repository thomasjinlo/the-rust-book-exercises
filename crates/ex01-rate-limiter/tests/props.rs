use ex01_rate_limiter::TokenBucket;
use proptest::prelude::*;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5eed),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]

    // Over any non-decreasing timestamp sequence, tokens granted can never exceed
    // the initial capacity plus the tokens that refill produced between the first
    // and last timestamp.
    #[test]
    fn granted_tokens_bounded_by_capacity_plus_refill(
        capacity in 1u32..50,
        rate in 1u32..2000,
        deltas in proptest::collection::vec(0u64..3_000_000_000, 1..200),
        costs in proptest::collection::vec(1u32..4, 200),
    ) {
        let mut b = TokenBucket::new(capacity, rate).unwrap();
        let first = deltas[0];
        let mut now = 0u64;
        let mut granted = 0u64;
        for (i, d) in deltas.iter().enumerate() {
            now += d;
            if b.try_acquire(now, costs[i]) {
                granted += u64::from(costs[i]);
            }
        }
        let bound = u128::from(capacity)
            + u128::from(rate) * u128::from(now - first) / 1_000_000_000;
        prop_assert!(u128::from(granted) <= bound);
    }

    // `available` never exceeds capacity and never panics, whatever the timestamps.
    #[test]
    fn available_never_exceeds_capacity(
        capacity in 1u32..1000,
        rate in 1u32..u32::MAX,
        times in proptest::collection::vec(any::<u64>(), 1..100),
    ) {
        let mut b = TokenBucket::new(capacity, rate).unwrap();
        for t in times {
            prop_assert!(b.available(t) <= capacity);
        }
    }

    // Refill does not depend on how the interval is split into calls.
    #[test]
    fn splitting_an_interval_changes_nothing(
        capacity in 1u32..100,
        rate in 1u32..5000,
        mid in 0u64..5_000_000_000,
        end in 0u64..5_000_000_000,
    ) {
        let end = mid + end;
        let mut one = TokenBucket::new(capacity, rate).unwrap();
        let mut two = TokenBucket::new(capacity, rate).unwrap();
        while one.try_acquire(0, 1) {}
        while two.try_acquire(0, 1) {}
        two.available(mid);
        prop_assert_eq!(one.available(end), two.available(end));
    }
}
