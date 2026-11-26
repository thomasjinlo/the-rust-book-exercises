use ex01_rate_limiter::{ConfigError, KeyedLimiter};

const SEC: u64 = 1_000_000_000;

#[test]
fn config_errors_in_order() {
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
fn new_limiter_is_empty() {
    let l = KeyedLimiter::<u32>::new(1, 1, 4).unwrap();
    assert!(l.is_empty());
    assert_eq!(l.len(), 0);
}

#[test]
fn first_call_takes_a_token() {
    let mut l = KeyedLimiter::new(2, 1, 4).unwrap();
    assert!(l.check(&"a", 0));
    assert!(l.check(&"a", 0));
    assert!(!l.check(&"a", 0));
    assert_eq!(l.len(), 1);
}

#[test]
fn keys_are_isolated() {
    let mut l = KeyedLimiter::new(1, 1, 4).unwrap();
    assert!(l.check(&"a", 0));
    assert!(!l.check(&"a", 0));
    assert!(l.check(&"b", 0));
    assert!(!l.check(&"b", 0));
}

#[test]
fn keys_refill_independently() {
    let mut l = KeyedLimiter::new(1, 1, 4).unwrap();
    assert!(l.check(&"a", 0));
    assert!(l.check(&"b", SEC / 2));
    assert!(l.check(&"a", SEC));
    assert!(!l.check(&"b", SEC));
    assert!(l.check(&"b", SEC + SEC / 2));
}

#[test]
fn full_table_allows_new_key_without_storing_it() {
    let mut l = KeyedLimiter::new(1, 1, 2).unwrap();
    assert!(l.check(&1, 0));
    assert!(l.check(&2, 0));
    assert!(l.check(&3, 0));
    assert!(l.check(&3, 0));
    assert_eq!(l.len(), 2);
}

#[test]
fn tracked_keys_still_limited_when_table_is_full() {
    let mut l = KeyedLimiter::new(1, 1, 1).unwrap();
    assert!(l.check(&1, 0));
    assert!(l.check(&2, 0));
    assert!(!l.check(&1, 0));
}

#[test]
fn works_with_owned_string_keys() {
    let mut l = KeyedLimiter::<String>::new(1, 1, 2).unwrap();
    assert!(l.check(&"x".to_string(), 0));
    assert!(!l.check(&"x".to_string(), 0));
}

#[test]
fn key_table_stays_bounded_under_one_million_distinct_keys() {
    let mut l = KeyedLimiter::new(1, 1, 1000).unwrap();
    for k in 0..1_000_000u32 {
        l.check(&k, 0);
        assert!(l.len() <= 1000);
    }
    assert_eq!(l.len(), 1000);
}
