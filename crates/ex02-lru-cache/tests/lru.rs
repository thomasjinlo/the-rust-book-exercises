use ex02_lru_cache::LruCache;
use std::time::{Duration, Instant};

#[test]
fn put_then_get_returns_the_value() {
    let mut c = LruCache::new(2);
    c.put(1, "a");
    assert_eq!(c.get(&1), Some(&"a"));
}

#[test]
fn a_missing_key_returns_none() {
    let mut c: LruCache<u32, &str> = LruCache::new(2);
    assert_eq!(c.get(&1), None);
    c.put(1, "a");
    assert_eq!(c.get(&2), None);
}

#[test]
fn a_full_cache_drops_the_oldest_entry() {
    let mut c = LruCache::new(2);
    c.put(1, "a");
    c.put(2, "b");
    c.put(3, "c");
    assert_eq!(c.get(&1), None);
    assert_eq!(c.get(&2), Some(&"b"));
    assert_eq!(c.get(&3), Some(&"c"));
}

#[test]
fn get_keeps_an_entry_from_being_dropped() {
    let mut c = LruCache::new(2);
    c.put(1, "a");
    c.put(2, "b");
    c.get(&1);
    c.put(3, "c");
    assert_eq!(c.get(&2), None);
    assert_eq!(c.get(&1), Some(&"a"));
    assert_eq!(c.get(&3), Some(&"c"));
}

#[test]
fn put_on_an_existing_key_replaces_the_value_and_drops_nothing() {
    let mut c = LruCache::new(2);
    c.put(1, "a");
    c.put(2, "b");
    c.put(1, "A");
    assert_eq!(c.get(&1), Some(&"A"));
    assert_eq!(c.get(&2), Some(&"b"));
    c.put(3, "c");
    assert_eq!(c.get(&1), None, "1 was used before 2");
    assert_eq!(c.get(&2), Some(&"b"));
}

#[test]
fn a_cache_of_one_keeps_only_the_last_key() {
    let mut c = LruCache::new(1);
    c.put(1, "a");
    c.put(2, "b");
    assert_eq!(c.get(&1), None);
    assert_eq!(c.get(&2), Some(&"b"));
}

/// Same answers as a simple list kept in use order, newest first.
#[test]
fn random_calls_match_a_simple_model() {
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for _ in 0..200 {
        let cap = 1 + (next() % 8) as usize;
        let mut cache = LruCache::new(cap);
        let mut model: Vec<(u64, u64)> = Vec::new();
        for step in 0..300u64 {
            let r = next();
            let key = r % 16;
            let pos = model.iter().position(|(k, _)| *k == key);
            if r & (1 << 40) == 0 {
                cache.put(key, step);
                if let Some(p) = pos {
                    model.remove(p);
                } else if model.len() == cap {
                    model.pop();
                }
                model.insert(0, (key, step));
            } else {
                let expected = pos.map(|p| {
                    let e = model.remove(p);
                    model.insert(0, e);
                    e.1
                });
                assert_eq!(cache.get(&key).copied(), expected);
            }
        }
    }
}

#[test]
fn many_calls_on_a_large_cache_finish_quickly() {
    let start = Instant::now();
    let mut c = LruCache::new(10_000);
    for i in 0..200_000u64 {
        c.put(i % 20_000, i);
        c.get(&(i % 15_000));
    }
    // A correct cache takes about 0.4 s here in a debug build. One that scans its entries
    // on every call takes about 20 s.
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "took {:?}",
        start.elapsed()
    );
}
