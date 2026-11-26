use ex01_rate_limiter::{ConfigError, KeyedLimiter};

const SEC: u64 = 1_000_000_000;

#[test]
fn config_errors_come_in_order() {
    assert_eq!(
        KeyedLimiter::<u32>::new(0, 0, 0).unwrap_err(),
        ConfigError::ZeroCapacity
    );
    assert_eq!(
        KeyedLimiter::<u32>::new(1, 0, 0).unwrap_err(),
        ConfigError::ZeroRate
    );
    assert_eq!(
        KeyedLimiter::<u32>::new(1, 1, 0).unwrap_err(),
        ConfigError::ZeroMaxKeys
    );
}

#[test]
fn the_first_request_of_a_key_uses_its_burst() {
    let mut l = KeyedLimiter::new(2, 1, 4).unwrap();
    assert!(l.check(&"a", 0));
    assert!(l.check(&"a", 0));
    assert!(!l.check(&"a", 0));
}

#[test]
fn keys_do_not_affect_each_other() {
    let mut l = KeyedLimiter::new(1, 1, 4).unwrap();
    assert!(l.check(&"a", 0));
    assert!(!l.check(&"a", 0));
    assert!(l.check(&"b", 0));
    assert!(!l.check(&"b", 0));
}

#[test]
fn each_key_refills_on_its_own_schedule() {
    let mut l = KeyedLimiter::new(1, 1, 4).unwrap();
    assert!(l.check(&"a", 0));
    assert!(l.check(&"b", SEC / 2));
    assert!(l.check(&"a", SEC));
    assert!(!l.check(&"b", SEC));
    assert!(l.check(&"b", SEC + SEC / 2));
}

#[test]
fn owned_string_keys_work() {
    let mut l = KeyedLimiter::new(1, 1, 4).unwrap();
    let key = String::from("10.0.0.1");
    assert!(l.check(&key, 0));
    assert!(!l.check(&key, 0));
}

#[test]
fn a_client_sending_ten_requests_at_once_gets_its_burst() {
    let mut l = KeyedLimiter::new(3, 1, 100).unwrap();
    let allowed = (0..10).filter(|_| l.check(&"10.0.0.1", 0)).count();
    assert_eq!(allowed, 3);
}

#[test]
fn keys_beyond_the_limit_are_always_allowed_and_tracked_keys_stay_limited() {
    let mut l = KeyedLimiter::new(1, 1, 2).unwrap();
    assert!(l.check(&1, 0));
    assert!(l.check(&2, 0));
    for _ in 0..5 {
        assert!(l.check(&3, 0), "key 3 is beyond the limit");
        assert!(l.check(&4, 0), "key 4 is beyond the limit");
    }
    assert!(!l.check(&1, 0));
    assert!(!l.check(&2, 0));
    assert!(l.check(&1, SEC), "tracked keys still refill");
}

#[test]
fn a_key_beyond_the_limit_never_displaces_a_tracked_key() {
    let mut l = KeyedLimiter::new(1, 1, 1).unwrap();
    assert!(l.check(&"a", 0));
    assert!(l.check(&"b", 0));
    assert!(!l.check(&"a", 0));
    assert!(l.check(&"b", 0));
}

#[test]
fn the_first_keys_seen_are_the_ones_that_are_limited() {
    let mut l = KeyedLimiter::new(1, 1, 10).unwrap();
    let mut denied = Vec::new();
    for key in 0..1000u32 {
        assert!(l.check(&key, 0));
    }
    for key in 0..1000u32 {
        if !l.check(&key, 0) {
            denied.push(key);
        }
    }
    assert_eq!(denied, (0..10).collect::<Vec<u32>>());
}

#[test]
fn a_million_distinct_keys_limit_exactly_the_configured_number() {
    let mut l = KeyedLimiter::new(1, 1, 1000).unwrap();
    for key in 0..1_000_000u64 {
        assert!(l.check(&key, 0));
    }
    let denied = (0..1_000_000u64).filter(|k| !l.check(k, 0)).count();
    assert_eq!(denied, 1000);
}
